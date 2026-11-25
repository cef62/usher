# 0013. Level listeners

- Status: Accepted
- Date: 2026-11-25
- Amends: [0006](0006-engine-threading-and-coreaudio-binding.md) ("four listeners, all on the system object" and "No `AudioObjectID` outlives one call")
- Spec: `docs/superpowers/specs/2026-11-24-usher-v2-design.md` §6.2, §6.3

## Context

M7 adds a volume slider and a mute button for the active output and input. The strip must follow
changes made elsewhere (volume keys, Control Center), so usher needs listeners for volume and mute on
the current default device of each role. ADR 0006 allows four listeners, all on the system object, and
forbids any `AudioObjectID` that outlives one call. A level listener lives on a device, so both rules
need an exception. A listener removed from a device that has vanished also returns an error.

## Options

1. **Poll the levels.** No new listeners and no exception to 0006. The slider lags, and polling
   while the panel is closed wastes wakeups.
2. **Level listeners on the default device, with a narrow `AudioObjectID` exception.** Chosen. The slider
   follows live, and the exception has one holder.
3. **Listeners on every device.** Many registrations, and most of them would never be read.

## Decision

Option 2. Five rules for the level listeners (spec §6.2):

1. The level watch holds the watched device's `AudioObjectID` from registration to removal. Nothing
   else holds one; every other call still translates the UID again.
2. When the role's default device changes, the old listeners are removed first, then new ones are added
   on the device that is now the default.
3. A failed remove (the device vanished) is logged at `debug` and ignored.
4. The existing generation counter drops callbacks from an older registration.
5. After registering, the engine re-reads the role's levels once, so a change between removal and
   registration is not lost.

A listener sends `AudioEvent::LevelChanged(role)`. The engine reads that role's levels and emits the
view at once, with no scan and no 150 ms window.

Decisions the spec left open, or where this ADR departs from its text:

1. **`VirtualMainVolume` goes through the AudioObject API.** A read-only probe on 2026-11-25 (macOS
   27.2, built-in speaker and microphone) found `kAudioHardwareServiceDeviceProperty_VirtualMainVolume`
   (`'vmvc'`, `0x766d7663`) on the main element. `AudioObjectHasProperty` and `IsPropertySettable`
   were true, `AudioObjectGetPropertyData` succeeded, and adding and removing a listener block returned 0
   in both output and input scope. No AudioToolbox `AudioHardwareService*` call is needed. The constant is
   defined locally. The fallback is `kAudioDevicePropertyVolumeScalar` on element 0. Mute is
   `kAudioDevicePropertyMute` on element 0. A property counts as a control only when present and settable.
2. **Trait signatures differ from the spec.** `levels(role, key)` takes the key, as writes do, so the
   strip's name and its levels always describe the same device. The spec form reads the live default,
   which could show one device's name with another's volume for 150 ms. `default_key(role)` is added: it
   is the live read behind `Refusal::Stale`. `watch_levels(role, Option<&key>)` moves the role's listeners.
   `set_volume` and `set_mute` are as in the spec.
3. **Re-watch after every reconcile pass, and separate gates.** The engine calls `watch_levels` after
   every reconcile pass. It does nothing for the same `AudioObjectID` and nothing when unsubscribed. That
   covers launch, `DefaultChanged`, `DevicesChanged` (including a same-UID replug) and rebind, without
   tracking which event happened. Each role has its own `Gate`, so `watch_levels` bumps only that role's
   gate and leaves the four system listeners alone. `subscribe`, `unsubscribe`, `rebind` and Drop clear the
   level watch; the next reconcile adds it again.
4. **Refusals live only in the reply.** `Event::Level` is refused `NotReady` before the first scan,
   `Rebinding` during a rebind, and `Stale` when `key` is no longer the role's default (live read; a failed
   read counts as `Stale`). The panel shows nothing for a refusal; the reply is logged at `debug` once per
   run of the same refusal, until a write succeeds. A failed write replies `Ok` and shows `Couldn't set volume: <error>` (or mute) under the strip
   until the next successful write for that role or a change of that role's device. The engine re-reads the
   role's levels after every write and emits only when they changed. A level write never marks the store
   dirty, never schedules a reconcile and never changes which device is active.

## Consequences

- **The exception is narrow.** Only the level watch keeps an `AudioObjectID` across calls. If the device
  vanishes, the remove fails, is logged and ignored, and the next reconcile registers on the new default.
- **The slider can lag by one reconcile.** After a default change, the new device's listeners are added
  after the reconcile pass, within the 150 ms window plus the scan. A deferred USB recheck adds up to 500 ms. `Stale` stops a write in that gap.
- **Some devices have no control.** HDMI displays and fixed-volume DACs show a disabled slider and
  `No volume control`.
- **Tray.** The menu-bar icon shows a dot while the active input is muted. Four template images cover
  auto-switch on and off, with and without the dot.
- **Not built.** Per-row sliders, shortcuts, remembered volumes, per-channel mute and balance (v2
  non-goals, ADR 0011).
- The Usage page, README and screenshots move with the UI (ADR 0010).
