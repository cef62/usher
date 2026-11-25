# usher v2 — design

- Status: owner-reviewed draft, 2026-11-24 (builder check done; owner answered its questions)
- Builds on: `docs/superpowers/specs/2026-11-23-usher-design.md` (the v1 spec; "v1 §n" below). Everything in v1
  stays true unless this document says otherwise.
- Evidence: Bluetooth spike report, `docs/audit/bluetooth-spike.md` (summarised in §7.0).

## 1. Intent

v1 does one thing: it keeps the top-ranked connected device active. In daily use the owner still reaches
elsewhere for three things. v2 adds them around the switching core, without changing the policy:

1. **See what usher did and why**: a notice when usher switches, a history, and a diagnostics view.
2. **Volume and mute** of the active devices, from the menu bar.
3. **Bluetooth**: battery levels, and connect or disconnect a paired headset from its row.

The order is the build order. Each part ships as its own milestone and release.

## 2. Goals, non-goals, anti-goals

**Goals (owner)**

1. M6 Activity: a macOS notification when usher switches (on by default, with a setting), a session
   history, and a diagnostics view with "Copy diagnostics".
2. M7 Volume and mute: volume slider and mute for the active output and the active input in the panel; a
   small dot on the menu-bar icon while the input is muted.
3. M8 Bluetooth: battery level of connected Bluetooth audio devices, any brand macOS reports (best effort,
   owner: a device with no data gets no badge); Connect and Disconnect for remembered Bluetooth devices.

**Non-goals (v2)**

- Volume or mute shortcuts, option-click on the icon, scroll on the icon (they need Accessibility and
  global event monitors; the fork's version was a large source of bugs).
- Per-row volume sliders; remembering a volume per device (macOS does that already).
- A persisted history (the log file is the permanent record, v1 §5.11).
- Selecting a device on connect: Connect only connects; the priority list decides what becomes active.
- Profiles, Focus modes, separate speaker and headphone lists, vendor HID, Intel builds (still v1 non-goals).

**Anti-goals** (all of v1 §2 still apply), plus:

- A notice for a change the user made in usher (promote, reorder, toggle).
- Any path other than the engine thread that writes CoreAudio state (ADR 0006).
- Bluetooth code that blocks the engine thread or the main thread.
- Anything in "Copy diagnostics" that is unsafe to paste into a public GitHub issue (owner: the text is
  meant for public issues; §5.4 lists what is redacted).

## 3. Constraints

- All v1 constraints (v1 §3): Apple Silicon, macOS 14+, ad-hoc signing, size limits, `unsafe` only in
  `crates/usher-coreaudio`.
- Ad-hoc signing changes the code hash every release. macOS probably asks for the Bluetooth permission again
  after each update. **Owner: accepted.**
- `system_profiler` Bluetooth JSON keys are undocumented (spike). A change in macOS hides the battery badges;
  it must never affect switching.

## 4. Approaches

- **Spec shape.** One v2 spec with three milestones (chosen) vs. three specs. One spec keeps the shared panel
  layout, icon states and ADRs in one place. **Owner: one spec.**
- **Who owns volume and mute.** The engine thread (chosen) vs. a direct path from the panel to CoreAudio. The
  engine already owns every CoreAudio write and listener, and re-attaches listeners after a coreaudiod
  restart (v1 §5.9); a second owner would need its own restart and vanished-device handling, the bug class
  ADR 0006 removed. **Owner: engine-owned.**
- **Battery source.** `system_profiler SPBluetoothDataType -json` (chosen: no permission, no `unsafe`, pure
  testable parser) vs. private IOBluetooth selectors (private, need the Bluetooth permission) vs. CoreBluetooth
  GATT (BLE only). Spike verdict.
- **Connect.** Public `IOBluetoothDevice deviceWithAddressString:` + `openConnection` via `objc2-io-bluetooth`
  (chosen) vs. private frameworks or shelling out to `blueutil`. Spike verdict.

## 5. M6 Activity

### 5.1 What counts as an event

The engine already computes a `Reason` per role on every reconcile (`crates/usher-core/src/reason.rs`). M6 adds
the *trigger* of each reconcile and turns the outcome into activity entries. A pure function in usher-core
does it:

`explain(trigger, prev: Option<&Snapshot>, snap: &Snapshot, reason: &Reason, outcome) -> Option<Entry>`

Triggers: `Launch`, `DevicesChanged`, `DefaultChanged(role)`, `Command`, `Rebind`.

| Entry | When | Text (history; notice body) |
|---|---|---|
| `Switched { role, from, to, why }` | usher set a new default | `Output: <from> → <to>` |
| `Failed { role, to, error }` | the write failed (v1 §5.3 error rule) | `Couldn't switch output to <to>: <error>` |
| `Paused { reason }` | loop guard tripped (v1 §5.4) | the v1 pause text |
| `Resumed` | Auto-switch turned back on | `Auto-switch on` |

`why` comes from **comparing the previous snapshot with this one**, never from which event arrived first:
an unplug fires devices-changed and default-changed inside one 150 ms window, and reading causes from event
order is the heuristic class ADR 0003 rejects. First match wins:

1. `Launch` / `Rebind`: the trigger is `Launch` or `Rebind`, or `prev` is `None` (history only, no notice;
   login and a coreaudiod restart reset defaults, which is not news).
2. `Disconnected(key)`: the role's previous current device is not connected in `snap` ("<name> disconnected").
3. `Connected(key)`: `desired` was not connected in `prev` and is in `snap` ("<name> connected").
4. `Ranked`: the trigger is `Command` (the user changed the list in usher; history only, no notice).
5. `Corrected(key)`: current ≠ desired and both were already connected; the taker is `snap`'s current
   ("<taker> took output; usher put it back"). It names the device that holds the role, not who moved it.

The trigger only tells `Command`, `Launch` and `Rebind` apart from device events; when the window merges a
`Command` with device events, the trigger is `Command`. A reconcile that changes nothing makes no entry.

### 5.2 Notices

- A macOS notification (the `tauri-plugin-notification` already used for the pause, v1 §5.4) for
  `Switched` with `why` `Disconnected`, `Connected` or `Corrected`, for `Failed`, and for `Paused` (as today).
  `Ranked`, `Launch` and `Rebind` are history only, so login and a coreaudiod restart never notify.
- At most one notice per role per 10 s; later entries in that window are history only (a fight with another
  app makes one notice, then the loop guard's pause notice).
- No notice for 10 s after launch and after a coreaudiod rebind completes (devices settle then); those entries stay in history and the log. Paused still notifies.
- Output and input entries from one reconcile make **one** notice: title `usher`, body one line per role.
- No notice while Auto-switch is off or paused (nothing switches then anyway).
- A `Failed` caused by a user command shows in the panel as today and makes no notice (anti-goal).
- `Paused` always notifies, whatever the setting (call: it is the v1 behaviour and stops switching).
- Setting: **Notify when usher switches** (Settings › General), default **on** (owner). Stored as
  `State.notify_switches: bool` in `state.json`, default `true` for old files, changed by
  `Command::SetNotify(bool)` through the engine, like `SetUpdateCheck` (ADR 0008 pattern). A serde default,
  no `SCHEMA_VERSION` bump.
- Permission: requested the first time a notice would show, as the pause does today. Denied: no notice,
  one `info` log line, history still records it.

### 5.3 History

- The last **200** entries of this session, newest first, each with a wall-clock time. Memory only.
- Stored on the shell side (an `ActivityLog` next to `LastView`), filled through a new
  `Shell::activity(&Entry)`; the engine stays free of UI state.
- Also logged at `info` (one line per entry), so the log keeps the permanent record.

### 5.4 Diagnostics

A snapshot built by the engine on request: a new `Event::Diagnostics(reply)` variant with its own reply
channel (the command `Reply` carries only `Result<(), Refusal>`). It contains:

- per role: desired, current, and every ranked device with its skip reason (`Skip`: disconnected, not alive,
  hidden, no streams, never use);
- per device in the last scan: name, key, UID, transport, alive, hidden, serial-less USB verdict, aliases;
- engine: subscribed / rebinding (attempt n) / read-only store / USB verdict unresolved;
- app version, macOS version, log path.

**Copy diagnostics** puts a plain-text rendering on the clipboard, meant to be pasted into a public GitHub
issue (owner). The Activity window shows exactly that text, so the user reads it before pasting. The
renderer is a pure function and works as an **allowlist** for UIDs (keys and aliases use the same rendering):

- built-in UIDs (`BuiltIn…`) as is;
- `AppleUSBAudioEngine:<vendor>:<product>:<4th>:<n>`: vendor and product kept; the 4th field kept only when
  the device is confirmed serial-less (then it is the port location, which diagnostics debug), otherwise
  `<serial>`; an unresolved verdict counts as not confirmed;
- the Bluetooth address shape `xx-xx-xx-xx-xx-xx:output|:input`, to the last two bytes
  (`xx-xx-xx-xx-CF-9D:output`);
- any other UID shape (display, AirPlay, network, aggregate, FireWire/Thunderbolt) becomes `uid#<n>`, an index
  per report, the same index wherever that UID appears.

It also drops a leading `<name>'s ` / `<name>’s ` from device names ("Matteo's AirPods Pro" → "AirPods Pro")
and shows the home folder in paths as `~`. Other custom names stay; the on-screen preview is how the user
catches them.

Vendor and product are the names already in the UID; no new IOKit reads (call).

### 5.5 The Activity window

- Label `activity`, built at runtime like Settings (Liquid Glass, overlay title bar, drag strip; the
  `glass::apply` path), 480×560, resizable, not maximizable.
- Top: History list (time, role, text, `why` in dim text). Bottom: Diagnostics, collapsed by default, with
  **Copy diagnostics**.
- Opens from: the panel footer **Activity…**, the right-click menu **Activity…** (above Settings…), and
  Settings › Troubleshooting **Open Activity…**.
- Live: the window listens to an `activity` event for new entries; diagnostics refresh on open and on
  **Refresh**.
- Capability `activity`: `log:default`, event listen/unlisten, start-dragging, internal-toggle-maximize. IPC
  guard list `ACTIVITY_COMMANDS`: `get_activity`, `get_diagnostics`.

## 6. M7 Volume and mute

### 6.1 Behaviour

- The panel shows, under the Auto-switch header, one strip per role for the **active** device:
  ```
  Output  Studio Display Speakers
    🔈 ━━━━━━━━━●━━━━ 🔊   [mute]
  Input   Scarlett 2i2 USB
    🎙 ━━━━━━●━━━━━━━      [mute]
  ```
- Slider: 0–100 %, the device's scalar volume. Mute: a toggle button (`aria-pressed`).
- A device without a settable volume shows a disabled slider and `No volume control`; without a settable
  mute, no mute button.
- The strip follows changes made elsewhere (volume keys, Control Center) live.
- Volume and mute never trigger a reconcile and never change which device is active.
- Menu-bar icon: a small dot while the active input is muted. Four template images: on, off, on + dot,
  off + dot (owner).

### 6.2 CoreAudio

- Volume: `kAudioHardwareServiceDeviceProperty_VirtualMainVolume` (output scope for output, input scope for
  input); fall back to channel 0 `kAudioDevicePropertyVolumeScalar`. Mute: `kAudioDevicePropertyMute`.
  `AudioObjectIsPropertySettable` decides "no control".
- New `AudioSystem` methods: `levels(role) -> Result<Level, AudioError>`,
  `set_volume(role, key, f32)`, `set_mute(role, key, bool)`. Writes resolve the key to an `AudioObjectID`
  at call time, as `set_default` does.
- Listeners on the current default device of each role for volume and mute. This **amends ADR 0006** (four
  listeners, all on the system object; no `AudioObjectID` outlives one call), in a new ADR (§9):
  - the level listener holds the default device's `AudioObjectID` from registration to removal, and nothing
    else does;
  - on `DefaultChanged(role)`, `DevicesChanged` and rebind, the old listeners are removed first, then new
    ones are added on the device that is now the default (the UID is translated again);
  - removing a listener from a device that has vanished returns an error: it is logged at `debug` and
    ignored;
  - the existing generation counter drops callbacks from an older registration;
  - after registering, the engine re-reads the role's levels once, so a change made between removal and
    registration is not lost.
  They send a new `AudioEvent::LevelChanged(role)`.

### 6.3 Engine and view

- Volume and mute are **not** `usher_core::Command`s: `apply` is pure state, and `on_command` marks the
  store dirty and schedules a reconcile. They are a new engine event, `Event::Level(LevelRequest, reply)`
  with `LevelRequest::{Volume { role, key, value }, Mute { role, key, on }}`, handled directly by the engine:
  no store write, no reconcile.
- Gating as for commands: refused `NotReady` before the first scan and `Rebinding` during a rebind; a new
  `Refusal::Stale` when `key` is no longer the role's default, checked by a **live read** of the role's
  default device in the handler (the last scan can lag a `DefaultChanged` by the 150 ms window; a failed
  read counts as `Stale`), so a drag never moves the volume of a device
  that just stopped being active.
- A failed write logs a `warn`, re-reads the role's levels (the slider snaps back to the real value) and shows
  `Couldn't set volume: <error>` (or mute) under that role's strip until the next successful change.
- A refusal (`NotReady`, `Rebinding`, `Stale`) is silent in the panel: the slider re-syncs from the next
  view, and the refusal is logged at `debug` once per drag.
- `LevelChanged(role)` reads that role's levels and emits the view at once, without a scan or the 150 ms
  coalescing window, so the slider echo feels immediate.
- `PanelView` gains `levels: PerRole<Option<Level>>`, `Level { volume: Option<f32>, muted: Option<bool> }`
  (`None` = no control).
- The panel throttles slider writes to one per 50 ms while dragging (≤ 20 events/s on the engine channel)
  and always sends the final value.

## 7. M8 Bluetooth

### 7.0 Spike summary

Both features are worth it with caveats. The CoreAudio UID of a Bluetooth device has the shape
`<address with '-'>:output|:input`: a key usher remembered in `state.json` while the Pixel Buds were
connected matches the address `system_profiler` prints for them. It was not matched against a live device
(none was connected during the spike), and battery data was not seen live; the entry gate below settles
both. Line counts and "blocks for seconds" are estimates from other projects, and the private selectors
were found by runtime introspection, not used.

### 7.0.1 M8 entry gate

The first M8 task settles what the spike could not verify, before any M8 code:

1. The owner captures `system_profiler SPBluetoothDataType -json` with AirPods, Pixel Buds Pro and QC35
   connected (redacted, they become the parser fixtures). This also shows which brands report battery.
2. A probe run by the owner proves `openConnection` / `closeConnection` from a background thread with no run
   loop, in an `LSUIElement` app. If IOBluetooth needs the main run loop, the design changes to a main-thread
   dispatch that returns at once and is completed by a connection notification; nothing may block.
3. The UID ↔ address rule is checked on AirPods and on any Bluetooth LE Audio device available.
4. The permission prompt and the abort without `NSBluetoothAlwaysUsageDescription` are observed on an
   ad-hoc-signed build, and once more after an update.

### 7.1 Battery

- Source: `system_profiler SPBluetoothDataType -json`, run with a 5 s timeout on a worker thread (never the
  engine or main thread). Keys `device_batteryLevelLeft/Right/Case/Main` under `device_connected`; all brands
  (owner).
- Pure parser in usher-core: JSON → `Vec<BtDevice { address, name, minor_type, connected, battery }>`.
  Matching to rows: normalise both addresses to 12 hex digits.
- When: on panel open, every 60 s while the panel is open, and 3 s and 15 s after a `DevicesChanged` that
  added a Bluetooth device (AirPods often report battery late). Results reach the engine as
  `Event::Bluetooth(report)` and land in the view.
- Failure (timeout, non-zero exit, unparsable JSON): the child is killed at the timeout, one `warn` per
  failure streak, the last good report is dropped, so badges **and** Connect/Disconnect hide (paired state is
  unknown). Switching is never affected.
- Row badge for a connected Bluetooth row: `80%`, or `L 80 · R 75 · case 40` for buds. ≤ 20 % shows in the
  warning colour. No data: no badge.

### 7.2 Connect and Disconnect

- The `…` menu of a **remembered** Bluetooth row that is disconnected and paired (from the same report) gets
  **Connect**; a connected one gets **Disconnect** (owner). Paired devices usher has never seen are not
  listed (the lists stay the remembered devices, v1 §5.5).
- Like volume, these are engine events, not `usher_core::Command`s: `Event::BtAction(BtRequest, reply)` with
  `BtRequest::{Connect { key }, Disconnect { key }}` (`Event::Bluetooth(report)` stays the poll result); the engine resolves the address from the UID
  and runs `openConnection` / `closeConnection` on a short-lived worker thread (the call blocks for seconds).
- The row shows `Connecting…` (or `Disconnecting…`) for up to 10 s. Success is the device appearing in a scan; on timeout or error
  the row shows `Couldn't connect <name>` / `Couldn't disconnect <name>` (or the error). Disconnect succeeds
  when the device leaves a scan. Connect does not select the device: the priority
  list decides, as for any device that appears. The engine owns the 10 s window (testable with the fake
  clock, call).
- `unsafe` lives in `crates/usher-coreaudio/src/bluetooth.rs` (about 15 lines, estimate), using
  `objc2-io-bluetooth` 0.3.2.
- Permission: `NSBluetoothAlwaysUsageDescription` in `src-tauri/Info.plist` (third-party sources report that
  without it the process aborts on first IOBluetooth use; the entry gate observes it). The prompt appears on the first Connect or Disconnect. Denied: the row error says
  to allow usher in System Settings › Privacy & Security › Bluetooth. A re-prompt after each update is
  accepted (§3).

## 8. Architecture changes

- usher-core: `Entry`, `explain`, notice and diagnostics text renderers, redaction, `Level`, the Bluetooth
  parser and address matcher, the `SetNotify` command.
- usher-coreaudio: level reads, writes and listeners; `bluetooth.rs`.
- src-tauri: engine trigger tracking, `Event::Diagnostics`, `Event::Level`, `Event::Bluetooth`, `Event::BtAction`, `Shell::activity`, `ActivityLog`, the Activity window, the Bluetooth
  poller, the tray's four icon images.
- src: Activity page (`activity.html`), the panel level strip and battery badges, row menu items, Settings
  rows (Notify when usher switches, Open Activity…).
- Size limits (v1 §2 anti-goals) apply; split files before they reach 300 lines.

## 9. ADRs

- **0011 v2 scope**: lifts the v1 non-goals for switch notices and history, a diagnostics UI, volume and mute,
  and Bluetooth connect and battery; records the non-goals that stay.
- **0012 Bluetooth**: `system_profiler` for battery and paired state, IOBluetooth for connect in
  `usher-coreaudio` (its `unsafe` scope widens to IOBluetooth), the permission and re-prompt trade-off.
- **0013 Level listeners** (with M7): amends ADR 0006's listener and `AudioObjectID` rule as §6.2 says.

## 10. Testing

The v1 rule holds: no logic that can only be tested by running the app (v1 §9).

- `explain`: every trigger × outcome, including no-change, corrected, disconnected, failed, and the
  user-command case that must make no notice.
- Notice coalescing (output + input → one notice) and the "no notice while off or paused" rule.
- Diagnostics renderer and redaction (no full Bluetooth address, no USB serial in the output).
- Engine (fake `AudioSystem`): `LevelChanged` emits without a scan; a stale `SetVolume` is refused; level
  listeners re-attach after `DefaultChanged` and after rebind.
- Bluetooth parser on JSON fixtures (owner captures AirPods, Pixel Buds Pro, QC35; until then fixtures
  built from the spike's observed shape); address matching; the 10 s connecting window and its timeout.
- QA rows in `docs/qa.md` for what needs hardware: notices on real switches, volume keys moving the slider,
  the mic dot, a real connect and disconnect, the permission prompt.

## 11. Releases

M6 → 0.2.0, M7 → 0.3.0, M8 → 0.4.0 (`minor` changesets). Each milestone gets its own plan, PRs, reviews
and docs-site update (Usage page and screenshots).

## 12. Left to the builder

- Exact copy beyond the strings above, and the Activity window layout details.
- Clipboard mechanism (`navigator.clipboard` on the button click, or the Tauri clipboard plugin).
- The dot's exact size and position on the 22×18 glyph.
- Poll timer ownership details, as long as nothing blocks the engine or main thread.
