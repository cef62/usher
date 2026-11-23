# 0006. Engine threading and the CoreAudio binding

- Status: Accepted
- Date: 2026-11-23

## Context

M3 adds the engine, the CoreAudio calls and the JSON store. Three questions are open in the spec:
how the engine runs, which crate talks to CoreAudio and IOKit, and how listener callbacks reach
the engine. The engine owns all mutable state (design §6). CoreAudio calls its listeners on its own
thread. A mistake here either switches someone's audio wrongly or leaves usher deaf after a
`coreaudiod` restart.

## Options

**Engine threading**

1. **One `std::thread`, one mpsc channel, deadlines served by `recv_timeout`.** Saves run
   synchronously on that thread. There is one owner and no shared state.
2. **An async runtime (tokio).** Adds a dependency and a second scheduler. Nothing here waits on
   I/O in parallel, so there is nothing for it to do.
3. **State in a `Mutex` shared by the listeners, the tray and the UI.** Every caller can race with
   a reconcile. It needs lock ordering rules, and a callback could block on a save.

**Binding crate**

1. **`objc2-core-audio` 0.3.2.** It is in the objc2 0.6 family already in the lock. It needs no
   bindgen and no clang.
2. **`coreaudio-sys` 0.2.18.** It runs bindgen at build time, which needs clang on every machine
   and in CI.
3. **`coreaudio-rs` 0.14.** It targets AudioUnits, not device properties.

**Listener API**

1. **Blocks** (`AudioObjectAddPropertyListenerBlock`) with a `None` dispatch queue. The closure
   captures what it needs. We keep each `RcBlock` so we can remove it.
2. **C procs** (`AudioObjectAddPropertyListener`). They need a manually managed context pointer
   whose lifetime we must prove against the removal call.

## Decision

**Threading.** Option 1. The engine thread owns the state. It reads one channel, `Event`, and
waits with `recv_timeout` until the next deadline: coalescing, save, USB recheck or rebind.
CoreAudio callbacks, tray clicks and (from M4) panel commands only send on the channel. A send
after the engine has stopped fails silently. Quit is a handshake: the shell sends `Event::Quit`
and waits at most 1 s. The engine flushes, unsubscribes and acks. Both alternatives are rejected
because there is one owner and nothing async to wait on.

**Binding.** `objc2-core-audio` 0.3.2. The other two are rejected for the reasons above.

**Listeners.** Blocks with a `None` queue, so CoreAudio calls them on its own thread. There are
four listeners, all on the system object: devices, default output, default input and
`ServiceRestarted`. A generation counter (an `AtomicU64`) is bumped before the listeners are
removed. A callback still in flight from an older registration sees a stale generation and is
dropped. No `AudioObjectID` outlives one call: the UID is translated again on every write. The
C-proc variant is rejected because of its context pointer.

**IOKit.** `objc2-io-kit` 0.3.2. Each scan walks every `IOUSBHostDevice` once, and only when a USB
device is present. A pure function applies the audit §3.3 verdict, matching by `locationID`. A
device whose node is not visible yet has an unresolved verdict. One rescan runs 500 ms later.

**State file.** A corrupt file (not valid JSON, or the wrong types) is moved aside to
`state.json.corrupt`, replacing an older one, and usher seeds fresh. A newer `schema_version`, or
a file that exists but cannot be read, is never moved or overwritten. usher seeds an in-memory
state and runs read-only for the session, and it logs one `error` line. This refuses a downgrade.
Writes go to a temp file and are renamed into place.

**Timing and error rules.**
- Coalescing is a fixed 150 ms window from the first event. Churn cannot starve it. Commands use
  the same window. The launch reconcile runs at once.
- The save debounce is a fixed 500 ms window from the first change. A seed saves at once.
- For `Output`, `set_default` also sets the system output device. If that second write fails,
  usher logs a `warn` and sets no error line, because the main default did change.
- A failed corrective write still counts toward the loop guard, because core records the write
  before the engine runs it. Three failed writes in 10 s pause auto-switch. This stops a hopeless
  write loop.

## Implementation notes

These rulings changed behaviour relative to the plan.

- `Store::save_now` takes `now`. A failed save, the seed save included, schedules a retry 5 s
  later (`SAVE_RETRY`). A failed write also removes its temp file.
- `CoreAudioSystem` holds `RcBlock`s, so it is `!Send`. The shell builds it inside the engine
  thread. Callbacks run on a CoreAudio thread, so the closure must be `Send + Sync`. block2 0.6
  cannot express that in the block type, so the `listener` constructor helper takes
  `impl Fn() + Send + Sync + 'static` and enforces it.
- The engine posts tray and menu updates to the main thread with `run_on_main_thread` and does not
  wait. The tray setters block on the main thread, and the main thread may be waiting on the
  engine in `RunEvent::Exit`. Waiting would stall the quit handshake and lose the flush.
- muda flips the check mark on click. The engine therefore calls `auto_switch_changed` after
  every command and every toggle, so the mark shows the real state when the toggle was dropped or
  rejected.
- If `app_data_dir` fails, the shell logs an error and runs without the engine. The tray and Quit
  still work.
- An absent `iSerialNumber` gives an unresolved USB verdict, not "serial-less". This avoids a
  wrong alias merge. A default device that vanishes during a scan is skipped, not a scan failure.
- A coreaudiod restart cancels a pending USB recheck. A coalesced reconcile that fires while a
  recheck is already due absorbs it, so there is no second scan back to back.

## Consequences

- Events that arrive during a save wait in the channel and are never lost.
- Only `usher-coreaudio` contains `unsafe`. Each block carries a `// SAFETY:` comment.
- The engine cannot be unit-tested against real hardware. It runs against `FakeAudioSystem`, and
  `docs/qa.md` covers the hardware cases by hand.
- If M4 needs a reply from the engine, it must add a reply channel to `Event::Command`.
