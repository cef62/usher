# usher M3: audio engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** usher switches audio devices for real. A new `usher-coreaudio` crate talks to CoreAudio and IOKit. An engine thread in `src-tauri` owns the state, persists it to `state.json`, survives coreaudiod restarts, and posts a notification when the loop guard trips. The tray menu has an "Auto-switch" toggle. There is no panel yet.

**Architecture:** `usher-coreaudio` is the only crate with `unsafe`. It exposes a safe `AudioSystem` trait and its implementation, `CoreAudioSystem`. CoreAudio listener blocks only call a `Notify` closure, gated by a generation counter. That closure sends an `Event` into the engine's mpsc channel. The engine is one `std::thread`. It coalesces events (150 ms), scans, calls `usher_core::reconcile`, executes the `Action`s through the trait, logs `Reason`s, and saves through a debounced `Store`. Every timer (coalescing, save, rebind backoff, USB recheck) is a deadline on the engine. The thread loop is `recv_timeout(next deadline)`, so tests drive the engine with fixed `Instant`s and never sleep.

**Tech Stack:** Rust 2024 workspace; `objc2-core-audio 0.3.2`, `objc2-io-kit 0.3.2`, `objc2-core-foundation 0.3.2`, `block2 0.6.2` (the objc2 0.6 family already in `Cargo.lock`); tauri 2.12.7 (`tray-icon`, `image-png`); `tauri-plugin-notification 2.8.1` (3.x is alpha only); `log`; `tempfile` (dev).

**Spec:** `docs/superpowers/specs/2026-11-23-usher-design.md` (§5.2, §5.3, §5.4, §5.7, §5.9, §5.10, §5.11, §6, §7.2, §9). Also read: `docs/superpowers/plans/2026-11-23-usher-roadmap.md` (conventions and carry-overs), `crates/usher-core/src/lib.rs` (the crate doc is the M3 contract), `docs/audit/core-audio-and-switching.md` §1, §2.1-2.4, §3.3 (IOKit steps), §6, §7, §8, and `docs/audit/hop-reuse.md` (settings.rs and history.rs patterns; hop source at `/Volumes/Dev/matteo/hop`, read-only).

**Branch:** `feat/m3-engine`, created from `feat/m2-core`.

## Global Constraints

- Non-test files ≤300 lines, functions ≤40 lines (clippy too_many_lines, workspace lints).
- `#![forbid(unsafe_code)]` everywhere except usher-coreaudio.
- `unsafe` blocks in usher-coreaudio each carry a `// SAFETY:` comment.
- usher-core stays pure.
- Quality gate: pnpm build, pnpm test, cargo test --workspace, cargo clippy --workspace --all-targets -- -D warnings, cargo fmt --all --check, pnpm check:size.
- macOS 14+, aarch64 only.
- Workspace members: `crates/usher-core`, `crates/usher-coreaudio` (lib `usher_coreaudio`), `src-tauri`. Bundle id `com.cef62.usher`.
- Nothing is keyed by `AudioObjectID` beyond a single call. The only key is `DeviceKey` (the CoreAudio device UID).
- Coalescing window 150 ms. Save debounce 500 ms. Rebind retry after 1, 2, 4, 8 and 16 s, then every 60 s forever.
- State file `~/Library/Application Support/com.cef62.usher/state.json` (`app.path().app_data_dir()` + `state.json`), `schema_version: 1`, written as temp file plus rename.
- Notification copy: `PausedReason::message()`, i.e. `usher paused auto-switch: <device> kept taking <role>`, title `usher`.
- Logging follows spec §5.11: `info` for each default set with its result, guard trips, alias merges, restarts and each rebind attempt, a corrupt file moved aside, and auto-switch/debug toggles. `debug` for raw events, coalescing, full snapshots, every `Reason` list, reconcile timing, and each save. `error` for failures, `warn` for recovered surprises.
- Rust unit tests go inline (`#[cfg(test)] mod tests`). Scenarios go in `tests/`. `pnpm check:size` ignores everything after the first bare `#[cfg(test)]` line and every path under `tests/`.

## Review Focus

1. **coreaudiod restarts in the middle of an event.** `set_default` or `scan` returns an OSStatus error, and `ServiceRestarted` arrives next. Expected: an error line, no panic, one rebind, then one reconcile that sets the desired device. Pinned in Task 7 (`restart_mid_event_recovers_after_rebind`).
2. **A device vanishes between `scan` and `set_default`.** Expected: `AudioError::NotFound`, an error line for that role, no retry in the same event, no panic. Pinned in Task 4 (`unknown_object_is_not_found`) and Task 6 (`vanished_device_is_an_error_line_not_a_panic`).
3. **Events arrive while a save is in flight.** Saves run synchronously on the engine thread, so new events wait in the channel and are never lost. A change made after a save goes into the next save. A failed save keeps the changes and retries. Pinned in Task 6 (`change_after_a_save_is_in_the_next_save`) and Task 5 (`failed_save_keeps_dirty_and_retries_after_5s`).
4. **Quit during the 500 ms debounce window.** Expected: the pending change is on disk before the process exits. Pinned in Task 7 (`quit_inside_debounce_window_flushes`).
5. **state.json is unreadable, corrupt, or has a newer schema at launch.**
   - Decision: a corrupt file (not valid JSON, or the wrong types) is moved aside to `state.json.corrupt`, replacing an older one, and usher seeds fresh.
   - A newer `schema_version`, or a file that exists but cannot be read, is never moved or overwritten. usher seeds an in-memory state and runs read-only (saves are disabled) for this session. It logs one `error` line. This refuses a downgrade.
   - Pinned in Task 5 (`corrupt_file_is_moved_aside`, `newer_schema_is_left_alone_and_read_only`, `unreadable_file_is_left_alone_and_read_only`).
6. **AudioObjectIDs are recycled after a rebind.** Expected: no ID survives a call. `set_default` resolves the UID with `kAudioHardwarePropertyTranslateUIDToDevice` on every call. Listeners are only on the system object. Pinned in Task 4 (`unknown_object_is_not_found`, and the `#[ignore]` `set_default_after_rebind_is_ok`).
7. **A channel sender outlives the engine.** A CoreAudio callback or a tray click arrives after the engine stopped. Expected: the send fails silently (debug log), with no panic. Pinned in Task 3 (`stale_generation_is_dropped`) and Task 7 (`notify_after_engine_stopped_is_silent`, `send_after_quit_returns_false`).

## Decisions the spec leaves open

1. **The `AudioSystem` trait, `AudioEvent` and `AudioError` live in `usher-coreaudio`**, not in core. usher-core is untouched; `src-tauri` depends on both crates.
2. **`subscribe` takes a `Notify = Arc<dyn Fn(AudioEvent) + Send + Sync>`**, not a `Sender`. CoreAudio events and tray commands therefore share one engine channel.
3. **Listeners use `AudioObjectAddPropertyListenerBlock`** with a `None` dispatch queue, so CoreAudio calls them on its own notification thread. Each block is kept as an `RcBlock` so it can be removed. There are 4 system-object listeners: devices, default output, default input, and `ServiceRestarted`. There are no per-device listeners in v1.
4. **The generation counter** is an `Arc<AtomicU64>`. It is bumped before listeners are removed, so a callback still in flight from an older registration is dropped.
5. **For `Output`, `set_default` also sets `kAudioHardwarePropertyDefaultSystemOutputDevice`.** If that second write fails, usher logs a `warn`. It does not set an error line, because the main default did change.
6. **Coalescing uses a fixed window from the first event, not a trailing one.** The maximum delay is 150 ms, and churn cannot starve it. Commands from the tray or UI use the same window. The launch reconcile runs at once.
7. **The save debounce is also fixed from the first change.** The maximum delay is 500 ms. `apply → Ok(true)` schedules this debounced save, as the M2 carry-over says. A seed saves immediately. A failed save keeps the changes and retries 5 s later.
8. **A failed `scan` or `subscribe` at launch uses the rebind backoff**, as the fork does (audit §1.3). A failed `scan` on an ordinary event logs `warn`, and the next event tries again. While a rebind is pending, coalesced reconciles are skipped.
9. **Without a state file, seeding waits for the first successful scan.** Seeding from an empty snapshot would rank devices without the current default first, and could switch audio at launch. Commands received before that scan are dropped with a `warn`.
10. **A USB device whose IOKit node is not visible yet** (verdict unresolved) triggers one extra scan 500 ms later. This mirrors the fork's one-shot re-read (audit §6). `DeviceSnapshot.serialless_usb` stays false until then.
11. **The IOKit check walks every `IOUSBHostDevice` once per scan**, and only when a USB device is present. It applies audit §3.3 steps 1-5 as a pure function. Matching is by `locationID`, never by vendor name.
12. **Error lines are per role and in memory only.** A failed set adds one. It is cleared when a later set for that role succeeds, or when a reconcile finds `desired == current`. They are emitted as `PanelView.errors` beside the flattened `PanelState`. Copy: `couldn't switch <role> to <name>: <error>`.
13. **A failed corrective write still counts toward the loop guard**, because core records the write before the engine executes it. So three failed writes in 10 s pause auto-switch. This is accepted: it stops a hopeless write loop.
14. **Quit** sends `Event::Quit(ack)` from `RunEvent::Exit` and waits at most 1 s. The engine flushes, unsubscribes and acks.
15. **The tray toggle** sends `Event::ToggleAutoSwitch`. The engine turns it into `Command::SetAutoSwitch(!state.auto_switch)`. The engine, not muda's own check state, sets the check mark and the icon. Menu: `Auto-switch` (check), separator, `Quit usher`. The tooltip is `usher` when on and `usher: auto-switch off` when off.
16. **Notification permission** is checked when the guard trips. If the state is not `Granted`, usher calls `request_permission()` and posts only on `Granted`. This runs on a short-lived thread so the engine never blocks on it.
17. **The engine calls `logging::apply(state.debug)` after loading the state**, so the persisted Debug setting takes effect from M3.

---

## File structure

| File | Responsibility | Budget |
|---|---|---|
| `Cargo.toml` | add `crates/usher-coreaudio` to members | |
| `crates/usher-coreaudio/Cargo.toml` | deps, `[lints] workspace = true` | |
| `crates/usher-coreaudio/src/lib.rs` | crate lints, `AudioSystem`, `AudioEvent`, `AudioError`, `Notify`, `Scan`, `check()`, re-exports | 110 |
| `crates/usher-coreaudio/src/props.rs` | the unsafe property wrappers: `get_u32`, `get_ids`, `get_string`, `set_u32`, `stream_count` | 160 |
| `crates/usher-coreaudio/src/scan.rs` | `transport_from()`, device reads, defaults, `scan()` | 200 |
| `crates/usher-coreaudio/src/usb.rs` | `UsbNode`, pure `serialless()`, IOKit walk `usb_nodes()` | 180 |
| `crates/usher-coreaudio/src/write.rs` | `found()`, `set_default()` | 90 |
| `crates/usher-coreaudio/src/listen.rs` | `Gate`, `Listeners` (subscribe / unsubscribe) | 170 |
| `crates/usher-coreaudio/src/system.rs` | `CoreAudioSystem` impl of `AudioSystem` | 80 |
| `crates/usher-coreaudio/tests/smoke.rs` | `#[ignore]` real-device tests | — |
| `src-tauri/Cargo.toml` | path deps, `tauri-plugin-notification`, `serde` | |
| `src-tauri/src/store.rs` | `Store`: load, corrupt-aside, read-only, atomic save, debounce | 180 |
| `src-tauri/src/engine/mod.rs` | `Event`, `Shell`, `PanelView`, `Clock`, `Engine` (start/handle/tick/next_due) | 260 |
| `src-tauri/src/engine/exec.rs` | reconcile run, action execution, error lines, logging | 200 |
| `src-tauri/src/engine/rebind.rs` | `retry_delay()`, rebind attempt | 100 |
| `src-tauri/src/engine/thread.rs` | `EngineHandle`, `run()` loop | 100 |
| `src-tauri/src/shell.rs` | `TauriShell` (emit, tray, notification), `spawn()` | 120 |
| `src-tauri/src/tray.rs` | toggle item, on/off icons, `set_auto()` | 110 |
| `src-tauri/src/lib.rs` | wiring: notification plugin, engine start, exit flush | 120 |
| `src-tauri/icons/tray-off-template.{svg,png}` | off glyph (speaker with a slash) | |
| `src-tauri/tests/common/mod.rs` | `Fake` (AudioSystem), `Recorder` (Shell), fixtures | — |
| `src-tauri/tests/engine.rs`, `tests/lifecycle.rs` | engine scenarios | — |
| `docs/adr/0006-engine-threading-and-coreaudio-binding.md`, `docs/adr/README.md` | ADR + index | |
| `docs/qa.md` | manual checklist | |
| `docs/architecture/overview.md`, roadmap | doc updates | |

---

### Task 1: `usher-coreaudio` crate and its safe API

**Files:**
- Create: `crates/usher-coreaudio/Cargo.toml`, `crates/usher-coreaudio/src/lib.rs`
- Modify: `Cargo.toml` (members)

**Interfaces:**
- Consumes: `usher_core::{Role, DeviceKey, Snapshot, DeviceSnapshot, Transport}`.
- Produces:
  - `pub enum AudioEvent { DevicesChanged, DefaultChanged(Role), ServiceRestarted }` (`Clone, Copy, PartialEq, Eq, Debug`).
  - `pub enum AudioError { NotFound(DeviceKey), Status { call: &'static str, status: i32 } }` (`Clone, PartialEq, Debug`), with `Display` and `std::error::Error`.
  - `pub type Notify = std::sync::Arc<dyn Fn(AudioEvent) + Send + Sync>;`
  - `pub struct Scan { pub snapshot: Snapshot, pub usb_unresolved: bool }`
  - The trait:
    ```rust
    pub trait AudioSystem {
        fn scan(&self) -> Result<Scan, AudioError>;
        fn set_default(&self, role: Role, key: &DeviceKey) -> Result<(), AudioError>;
        fn subscribe(&mut self, notify: Notify) -> Result<(), AudioError>;
        fn unsubscribe(&mut self);
        /// unsubscribe, then subscribe again with the last `Notify`
        fn rebind(&mut self) -> Result<(), AudioError>;
    }
    ```
  - `pub(crate) fn check(call: &'static str, status: i32) -> Result<(), AudioError>`.

- [ ] **Step 1: Branch and scaffold**

Run: `git switch -c feat/m3-engine feat/m2-core`. Then create `crates/usher-coreaudio/Cargo.toml`: package `usher-coreaudio`, `[lib] name = "usher_coreaudio"`, workspace version, edition and license, `[lints] workspace = true`. Deps:

```toml
usher-core = { path = "../usher-core" }
log = "0.4.35"
objc2-core-audio = "0.3.2"
objc2-core-foundation = { version = "0.3.2", features = ["CFString", "CFNumber"] }
objc2-io-kit = { version = "0.3.2", default-features = false, features = ["std", "libc"] }
block2 = "0.6.2"
```

Add a feature only when a symbol needs it. `lib.rs` starts with `#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]`, so clippy `-D warnings` enforces the `// SAFETY:` rule. Add `"crates/usher-coreaudio"` to the root `members`.

- [ ] **Step 2: Write the failing tests (inline in `lib.rs`)**

```rust
#[test]
fn zero_status_is_ok() { assert_eq!(check("set default", 0), Ok(())); }
#[test]
fn nonzero_status_is_error_with_code() {
    assert_eq!(check("set default", 560227702),
        Err(AudioError::Status { call: "set default", status: 560227702 }));
    assert_eq!(AudioError::Status { call: "set default", status: -50 }.to_string(),
        "set default failed (OSStatus -50)");
}
#[test]
fn not_found_names_the_uid() {
    assert_eq!(AudioError::NotFound("BuiltInSpeakerDevice".into()).to_string(),
        "device BuiltInSpeakerDevice is gone");
}
```

- [ ] **Step 3: Run them to verify they fail**

Run: `cargo test -p usher-coreaudio`
Expected: compile error, `cannot find function check`.

- [ ] **Step 4: Implement the types, the trait and `check`**

- [ ] **Step 5: Run the tests and clippy**

Run: `cargo test -p usher-coreaudio && cargo clippy -p usher-coreaudio --all-targets -- -D warnings`
Expected: `test result: ok. 3 passed`, no clippy output.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/usher-coreaudio
git commit -m "feat(coreaudio): crate skeleton and AudioSystem trait"
```

---

### Task 2: Reads (enumerate, defaults, transport, IOKit serial-less check)

**Files:**
- Create: `crates/usher-coreaudio/src/props.rs`, `src/scan.rs`, `src/usb.rs`, `tests/smoke.rs`

**Interfaces:**
- Consumes: Task 1 types.
- Produces:
  - `pub(crate) fn transport_from(fourcc: u32) -> Transport`
  - `pub(crate) struct UsbNode { pub location: u32, pub serial: Option<String>, pub serial_index: u8 }`
  - `pub(crate) fn serialless(uid: &str, nodes: &[UsbNode]) -> Option<bool>`
  - `pub(crate) fn usb_nodes() -> Vec<UsbNode>`
  - `pub(crate) fn scan() -> Result<Scan, AudioError>`
  - `props.rs`, with `id: u32` meaning an `AudioObjectID` that is only valid during this call:
    - `get_u32(id, selector, scope) -> Result<u32, AudioError>`
    - `get_ids(id, selector) -> Result<Vec<u32>, AudioError>`
    - `get_string(id, selector) -> Result<String, AudioError>`
    - `stream_count(id, scope) -> Result<u32, AudioError>`
    - `set_u32(id, selector, value) -> Result<(), AudioError>`

- [ ] **Step 1: Write the failing pure tests**

`scan.rs`:

```rust
#[test]
fn transport_fourccs_map_to_core_transports() {
    assert_eq!(transport_from(kAudioDeviceTransportTypeBuiltIn), Transport::BuiltIn);
    assert_eq!(transport_from(kAudioDeviceTransportTypeUSB), Transport::Usb);
    assert_eq!(transport_from(kAudioDeviceTransportTypeBluetoothLE), Transport::Bluetooth);
    assert_eq!(transport_from(kAudioDeviceTransportTypeHDMI), Transport::Hdmi);
    assert_eq!(transport_from(kAudioDeviceTransportTypeDisplayPort), Transport::DisplayPort);
    assert_eq!(transport_from(kAudioDeviceTransportTypeAutoAggregate), Transport::Aggregate);
    assert_eq!(transport_from(kAudioDeviceTransportTypeVirtual), Transport::Virtual);
    assert_eq!(transport_from(kAudioDeviceTransportTypeContinuityCaptureWired), Transport::Other);
    assert_eq!(transport_from(0), Transport::Other);
}
```

Also cover `Bluetooth`, `Thunderbolt`, `AirPlay` and `Aggregate`.

`usb.rs` (audit §3.3 steps 1-5; `U` is `"AppleUSBAudioEngine:Unknown Manufacturer:USB Audio:14200000:2,1"`):

```rust
fn node(location: u32, serial: Option<&str>, idx: u8) -> UsbNode { /* … */ }
#[test] fn non_five_part_uid_is_unresolved() { assert_eq!(serialless("BuiltInSpeakerDevice", &[]), None); }
#[test] fn hex_that_is_a_real_serial_is_not_serialless() {
    assert_eq!(serialless(U, &[node(0x1, Some("14200000"), 3)]), Some(false)); }
#[test] fn no_node_at_that_location_is_unresolved() {
    assert_eq!(serialless(U, &[node(0x14300000, None, 0)]), None); }
#[test] fn node_with_a_serial_is_not_serialless() {
    assert_eq!(serialless(U, &[node(0x14200000, Some("ABC123"), 3)]), Some(false)); }
#[test] fn node_without_serial_and_index_zero_is_serialless() {
    assert_eq!(serialless(U, &[node(0x14200000, None, 0)]), Some(true));
    assert_eq!(serialless(U, &[node(0x14200000, Some(""), 0)]), Some(true)); }
#[test] fn node_without_serial_but_nonzero_index_is_not() {
    assert_eq!(serialless(U, &[node(0x14200000, None, 2)]), Some(false)); }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p usher-coreaudio`
Expected: compile errors, `transport_from` and `serialless` not found.

- [ ] **Step 3: Implement `transport_from` and `serialless`**

Map `BluetoothLE` to `Bluetooth`, and `AutoAggregate` to `Aggregate`. Every other code becomes `Other`. In `serialless`, a location that does not parse as hex gives `None`.

- [ ] **Step 4: Implement `props.rs`**

Each `unsafe` block gets a `// SAFETY:` line covering three things:
- the address points to a stack `AudioObjectPropertyAddress`;
- `io_data_size` matches the buffer;
- a returned `CFStringRef` is +1 and is wrapped with `CFRetained::from_raw`.

A nonzero status goes through `check(call, status)`. `get_ids` uses `AudioObjectGetPropertyDataSize` then `AudioObjectGetPropertyData`, and truncates to the size returned.

- [ ] **Step 5: Implement `usb_nodes` and `scan`**

`usb_nodes` iterates `IOServiceMatching("IOUSBHostDevice")` and reads three properties:
- `locationID` (CFNumber);
- `USB Serial Number` (CFString, optional);
- `iSerialNumber` (CFNumber).

It releases each object with `IOObjectRelease`. If IOKit fails, it returns an empty Vec, which reads as unresolved.

`scan`:
- Read `kAudioHardwarePropertyDevices` in enumeration order.
- For each device, read the UID, name, transport, `DeviceIsAlive`, `IsHidden`, and the stream counts for input and output scope.
- If a read fails because the device vanished, skip that device and log at `debug`. The whole scan does not fail.
- Call `usb_nodes()` once, and only if some device is `Usb`. `serialless_usb = serialless(..) == Some(true)`. `usb_unresolved` is true when any USB verdict is `None`.
- Defaults: read the IDs of `DefaultOutputDevice` and `DefaultInputDevice`. ID `0` becomes `None`; any other ID becomes its UID.
- Log at `debug` one line per device: UID, name, transport, alive, hidden, in/out streams, serialless.

- [ ] **Step 6: Write the `#[ignore]` smoke test (`tests/smoke.rs`)**

```rust
#[test] #[ignore = "needs real audio hardware"]
fn enumerates_real_devices() {
    let scan = CoreAudioSystem::new().scan().unwrap();
    assert!(scan.snapshot.devices.iter().any(|d| d.output_streams > 0));
    assert!(scan.snapshot.devices.iter().all(|d| !d.uid.as_str().is_empty()));
    let out = scan.snapshot.current.output.clone().expect("a default output");
    assert!(scan.snapshot.is_connected(&out));
}
```

`CoreAudioSystem` comes from Task 4. Until then, mark this file `#![cfg(any())]` and remove that line in Task 4.

- [ ] **Step 7: Run the tests and clippy**

Run: `cargo test -p usher-coreaudio && cargo clippy -p usher-coreaudio --all-targets -- -D warnings`
Expected: all pass, no clippy output (in particular no `undocumented_unsafe_blocks`).

- [ ] **Step 8: Commit**

```bash
git add crates/usher-coreaudio
git commit -m "feat(coreaudio): enumerate devices, defaults and the IOKit serial-less check"
```

---

### Task 3: Listeners with a generation gate

**Files:**
- Create: `crates/usher-coreaudio/src/listen.rs`

**Interfaces:**
- Consumes: `Notify`, `AudioEvent`, `check`.
- Produces:
  - `pub(crate) struct Gate { generation: Arc<AtomicU64> }`, with:
    - `fn new() -> Self`
    - `fn callback(&self, notify: Notify, event: AudioEvent) -> impl Fn() + Send + Sync + 'static`. It captures the current generation and calls `notify(event)` only while that generation is unchanged.
    - `fn bump(&self)`
  - `pub(crate) struct Listeners`, with:
    - `fn subscribe(&mut self, notify: Notify) -> Result<(), AudioError>`
    - `fn unsubscribe(&mut self)` (idempotent)
    - `fn rebind(&mut self) -> Result<(), AudioError>`

- [ ] **Step 1: Write the failing tests**

```rust
fn counter() -> (Notify, Arc<Mutex<Vec<AudioEvent>>>) { /* Notify pushing into the Vec */ }
#[test]
fn live_generation_forwards_the_event() {
    let (n, got) = counter(); let g = Gate::new();
    g.callback(n, AudioEvent::DefaultChanged(Role::Input))();
    assert_eq!(*got.lock().unwrap(), vec![AudioEvent::DefaultChanged(Role::Input)]);
}
#[test]
fn stale_generation_is_dropped() {
    let (n, got) = counter(); let g = Gate::new();
    let old = g.callback(n, AudioEvent::DevicesChanged);
    g.bump();
    old();
    assert!(got.lock().unwrap().is_empty());
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p usher-coreaudio listen`
Expected: compile error, `Gate` not found.

- [ ] **Step 3: Implement `Gate` and `Listeners`**

`subscribe` registers 4 listeners on `kAudioObjectSystemObject`, in this order:
- `kAudioHardwarePropertyDevices` → `DevicesChanged`;
- `kAudioHardwarePropertyDefaultOutputDevice` → `DefaultChanged(Output)`;
- `kAudioHardwarePropertyDefaultInputDevice` → `DefaultChanged(Input)`;
- `kAudioHardwarePropertyServiceRestarted` → `ServiceRestarted`.

Each uses `AudioObjectAddPropertyListenerBlock(.., None, block)`. Keep each `RcBlock` and selector in a Vec. If one add fails, remove the ones already added in reverse order and return the `Err` (audit §1.2, rollback).

`unsubscribe`:
- `bump()` first;
- then `AudioObjectRemovePropertyListenerBlock` with the same block pointer;
- clear the Vec.

`rebind` is `unsubscribe` plus `subscribe` with the stored `Notify`. The block body is only the gated `notify` call: no logging, no locks.

- [ ] **Step 4: Run the tests and clippy**

Run: `cargo test -p usher-coreaudio && cargo clippy -p usher-coreaudio --all-targets -- -D warnings`
Expected: all pass, no clippy output.

- [ ] **Step 5: Commit**

```bash
git add crates/usher-coreaudio
git commit -m "feat(coreaudio): system listeners gated by a generation counter"
```

---

### Task 4: Writes and `CoreAudioSystem`

**Files:**
- Create: `crates/usher-coreaudio/src/write.rs`, `src/system.rs`
- Modify: `crates/usher-coreaudio/src/lib.rs` (`pub use system::CoreAudioSystem`), `tests/smoke.rs`

**Interfaces:**
- Consumes: `props::{get_u32, set_u32}`, `scan::scan`, `listen::Listeners`.
- Produces:
  - `pub(crate) fn found(id: u32, key: &DeviceKey) -> Result<u32, AudioError>`: `0` (`kAudioObjectUnknown`) gives `NotFound`.
  - `pub(crate) fn set_default(role: Role, key: &DeviceKey) -> Result<(), AudioError>`
  - `pub struct CoreAudioSystem`, with `pub fn new() -> Self` and `impl AudioSystem`.

- [ ] **Step 1: Write the failing test (`write.rs`)**

```rust
#[test]
fn unknown_object_is_not_found() {
    let k = DeviceKey::from("gone-uid");
    assert_eq!(found(0, &k), Err(AudioError::NotFound(k.clone())));
    assert_eq!(found(73, &k), Ok(73));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p usher-coreaudio write`
Expected: compile error, `found` not found.

- [ ] **Step 3: Implement `found`, `set_default` and `CoreAudioSystem`**

`set_default`:
1. Translate the UID with `kAudioHardwarePropertyTranslateUIDToDevice`. The qualifier is a `CFString` of the UID.
2. Pass the result to `found`.
3. Set `DefaultOutputDevice` or `DefaultInputDevice`, and return `Err` on a nonzero status.
4. For `Output` only, also set `DefaultSystemOutputDevice`. On failure, log `warn!("system output not moved to {key}: {e}")` and still return `Ok`.

Nothing stores the ID.

`CoreAudioSystem` holds `Listeners` and delegates `scan`, `set_default`, `subscribe`, `unsubscribe` and `rebind` to them. `Drop` calls `unsubscribe`.

- [ ] **Step 4: Finish the smoke tests**

Remove `#![cfg(any())]` from `tests/smoke.rs`. Add these `#[ignore = "needs real audio hardware"]` tests:
- `set_default_to_current_output_is_ok`: scan, then `set_default(Output, current)` is `Ok`.
- `subscribe_then_unsubscribe_is_clean`: `subscribe(noop)` is `Ok`; `unsubscribe()` twice does not panic.
- `set_default_after_rebind_is_ok`: subscribe, rebind, scan, then `set_default(Output, current)` is `Ok`. This proves nothing is held across a rebind.

- [ ] **Step 5: Run everything, including the hardware tests on this Mac**

Run: `cargo test -p usher-coreaudio && cargo test -p usher-coreaudio -- --ignored && cargo clippy -p usher-coreaudio --all-targets -- -D warnings`
Expected: both runs show `test result: ok`, with 0 failed. The second run shows `4 passed` in `smoke`. No clippy output.

- [ ] **Step 6: Commit**

```bash
git add crates/usher-coreaudio
git commit -m "feat(coreaudio): set default (and system output) by UID; CoreAudioSystem"
```

---

### Task 5: `Store` (state.json)

**Files:**
- Create: `src-tauri/src/store.rs`
- Modify: `src-tauri/src/lib.rs` (`pub mod store;`), `src-tauri/Cargo.toml` (`usher-core = { path = "../crates/usher-core" }`)

**Interfaces:**
- Consumes: `usher_core::{State, from_json, to_json, StateError}`.
- Produces:
  - `pub const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);` and `pub const SAVE_RETRY: Duration = Duration::from_secs(5);`
  - `pub struct Store`, with:
    - `pub fn open(path: PathBuf) -> (Store, Option<State>)`: `None` means "seed from the first scan".
    - `pub fn writable(&self) -> bool`
    - `pub fn mark_dirty(&mut self, now: Instant)`: sets `due = now + 500 ms` only when nothing is pending.
    - `pub fn due(&self) -> Option<Instant>`
    - `pub fn save_if_due(&mut self, state: &State, now: Instant)`
    - `pub fn save_now(&mut self, state: &State)`
    - `pub fn flush(&mut self, state: &State)`: saves only if dirty.
    - `pub fn saves(&self) -> u32`: a count of successful writes, for tests and the debug log.

Load rules (hop `settings.rs` pattern):

| File | Result |
|---|---|
| missing | `(writable, None)`, debug log |
| valid | `(writable, Some(state))`, unknown keys kept by `State.extra` |
| `StateError::Corrupt` | renamed to `state.json.corrupt` (replacing an old one), `warn!("state.json is corrupt ({e}); moved aside to state.json.corrupt")`, `(writable, None)` |
| `StateError::UnsupportedVersion(v)` | untouched, `error!("state.json has schema {v}, newer than this usher; running without saving")`, `(read-only, None)` |
| read error other than NotFound | untouched, `error!("state.json unreadable ({e}); running without saving")`, `(read-only, None)` |

Save rules:
- `create_dir_all(parent)`;
- write `state.json.tmp`, then `sync_all`, then rename it over `state.json`;
- log `debug!("state saved")`;
- on error: `error!`, keep dirty, `due = now + SAVE_RETRY`;
- a read-only store never writes.

- [ ] **Step 1: Write the failing tests (inline, `tempfile`)**

```rust
#[test] fn missing_file_needs_a_seed() {
    let d = tempdir(); let (s, st) = Store::open(d.path().join("state.json"));
    assert!(st.is_none()); assert!(s.writable()); }
#[test] fn round_trip_keeps_unknown_keys() {
    // write {"schema_version":1,"future_key":{"x":1}}; open; save_now; re-read
    assert_eq!(reread["future_key"], json!({"x":1}));
    assert!(!d.path().join("state.json.tmp").exists()); }
#[test] fn corrupt_file_is_moved_aside() {
    // write b"{not json"
    assert!(st.is_none()); assert!(s.writable());
    assert_eq!(fs::read(d.path().join("state.json.corrupt")).unwrap(), b"{not json");
    assert!(!d.path().join("state.json").exists()); }
#[test] fn newer_schema_is_left_alone_and_read_only() {
    // write {"schema_version":2}
    assert!(st.is_none()); assert!(!s.writable());
    s.save_now(&State::default());
    assert_eq!(fs::read_to_string(&p).unwrap(), r#"{"schema_version":2}"#);
    assert_eq!(s.saves(), 0); }
#[test] fn unreadable_file_is_left_alone_and_read_only() {
    // create state.json as a directory (read fails with an error other than NotFound)
    assert!(st.is_none()); assert!(!s.writable());
    assert!(!d.path().join("state.json.corrupt").exists()); }
#[test] fn debounce_is_fixed_from_the_first_change() {
    s.mark_dirty(at(0)); s.mark_dirty(at(400));
    assert_eq!(s.due(), Some(at(500)));
    s.save_if_due(&st, at(499)); assert_eq!(s.saves(), 0);
    s.save_if_due(&st, at(500)); assert_eq!(s.saves(), 1); assert_eq!(s.due(), None); }
#[test] fn failed_save_keeps_dirty_and_retries_after_5s() {
    // path under a regular file, so create_dir_all fails
    s.mark_dirty(at(0)); s.save_if_due(&st, at(500));
    assert_eq!(s.saves(), 0); assert_eq!(s.due(), Some(at(5_500))); }
#[test] fn flush_writes_only_when_dirty() {
    s.flush(&st); assert_eq!(s.saves(), 0);
    s.mark_dirty(at(0)); s.flush(&st); assert_eq!(s.saves(), 1); }
```

`at(ms)` is a fixed `Instant` plus `ms`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p usher store`
Expected: compile error, `Store` not found.

- [ ] **Step 3: Implement `store.rs`**

- [ ] **Step 4: Run the tests**

Run: `cargo test -p usher store`
Expected: `test result: ok. 8 passed`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri
git commit -m "feat(app): state.json store with atomic, debounced saves"
```

---

### Task 6: Engine core (coalesce, reconcile, execute, errors, seed)

**Files:**
- Create: `src-tauri/src/engine/mod.rs`, `src-tauri/src/engine/exec.rs`, `src-tauri/tests/common/mod.rs`, `src-tauri/tests/engine.rs`
- Modify: `src-tauri/src/lib.rs` (`pub mod engine;`), `src-tauri/Cargo.toml` (`usher-coreaudio` path dep, `serde = { version = "1.0.229", features = ["derive"] }`)

**Interfaces:**
- Consumes: `AudioSystem`, `AudioEvent`, `AudioError`, `Notify`, `Scan` (Tasks 1-4); `Store` (Task 5); `usher_core::{reconcile, apply, seed, panel_state, Action, Command, Runtime, State, Snapshot, PanelState, PausedReason, PerRole, Role, Timestamp}`.
- Produces:
  - `pub const COALESCE: Duration = Duration::from_millis(150);` and `pub const USB_RECHECK: Duration = Duration::from_millis(500);`
  - `pub enum Event { Audio(AudioEvent), Command(Command), ToggleAutoSwitch, Quit(std::sync::mpsc::Sender<()>) }`
  - `pub enum Flow { Continue, Stop }`
  - The shell trait:
    ```rust
    pub trait Shell {
        fn state_changed(&self, view: &PanelView);
        fn auto_switch_changed(&self, on: bool);
        fn paused(&self, reason: &PausedReason);
    }
    ```
  - `#[derive(Serialize, Clone, PartialEq, Debug)] #[serde(rename_all = "camelCase")] pub struct PanelView { #[serde(flatten)] pub panel: PanelState, pub errors: PerRole<Option<String>> }`
  - `pub struct Clock { pub start: Instant, pub wall_at_start: Timestamp }`, with `pub fn wall(&self, now: Instant) -> Timestamp`.
  - `pub struct Engine<A: AudioSystem, S: Shell>`, with:
    - `pub fn start(audio: A, shell: S, store: Store, state: Option<State>, notify: Notify, clock: Clock, now: Instant) -> Self`: subscribe, launch reconcile at once, `auto_switch_changed(state.auto_switch)`, emit the view.
    - `pub fn handle(&mut self, ev: Event, now: Instant) -> Flow`
    - `pub fn tick(&mut self, now: Instant)`: runs every deadline `<= now`.
    - `pub fn next_due(&self) -> Option<Instant>`: the minimum of the reconcile, save, rebind and USB-recheck deadlines.
    - `pub fn state(&self) -> &State`, `pub fn view(&self) -> PanelView`, `pub fn saves(&self) -> u32`.

Engine behaviour:
- **Audio events.** `DevicesChanged` and `DefaultChanged` log `debug!("event {ev:?}")` and set `reconcile_due = now + COALESCE` if none is pending. A later event in the window logs `debug!("coalesced {ev:?}")`.
- **Reconcile.**
  - Scan. On `Err`: `warn!` and return.
  - If a seed is pending: `seed(&snap, wall)`, then `store.save_now`, then `info!("first launch: seeded from current defaults")`.
  - Run `reconcile`, then log every `Reason` at `debug`, and `debug!("reconcile took {ms} ms")`.
  - Execute the actions in order:
    - `SetDefault`: `info!("set {role} default to {name} ({key}): ok")`; on `Err`, `error!("… failed: {e}")` and set `errors[role]`.
    - `PauseAuto`: `warn!("loop guard tripped: {message}")`, `shell.auto_switch_changed(false)`, `shell.paused(..)`.
    - `AliasMerged`: `info!("alias merged {from} -> {to}")`.
    - `Persist`: `store.mark_dirty`.
  - Clear `errors[role]` when the reason has `desired == current`.
  - If `scan.usb_unresolved` and no recheck was done since the last device event, set `usb_due = now + USB_RECHECK`.
  - Emit `shell.state_changed(&view)`.
- **Commands.** `Command(cmd)` runs `apply`.
  - `Ok(true)`: `store.mark_dirty(now)`. If `auto_switch` changed: `info!("auto-switch {on|off}")` and `shell.auto_switch_changed`. Then schedule the coalesced reconcile.
  - `Ok(false)`: nothing.
  - `Err(e)`: `warn!`.
  - Before the first seed, the command is dropped with `warn!`.
- **Tray toggle.** `ToggleAutoSwitch` becomes `Command::SetAutoSwitch(!state.auto_switch)`.
- **Error copy.** `couldn't switch {role} to {name}: {e}`. `name` comes from the snapshot, else the record, else the key.

`tests/common/mod.rs` provides:
- `Fake(Arc<Mutex<World>>)`, which is `Clone` and implements `AudioSystem`. `World` has these public fields:
  - `devices`, `current: PerRole<Option<DeviceKey>>`;
  - `fail_set: BTreeMap<DeviceKey, AudioError>`, `vanish_on_set: BTreeSet<DeviceKey>`, `sticky: PerRole<Option<DeviceKey>>` (after a successful set, `current` snaps back to this);
  - `fail_scan`, `fail_subscribe`, `fail_rebind` (each a `u32` count of failures to come);
  - `usb_unresolved`;
  - recorded: `scans: u32`, `sets: Vec<(Role, DeviceKey)>`, `rebinds: u32`, `subscribed: bool`.
- `Recorder(Arc<Mutex<Vec<Call>>>)`, which implements `Shell`, with `enum Call { View(PanelView), Auto(bool), Paused(PausedReason) }`.
- Fixtures:
  - `spk()` = `"BuiltInSpeakerDevice"` / `"MacBook Pro Speakers"`, BuiltIn, 0 in / 2 out;
  - `pods()` = `"AirPods-uid"` / `"AirPods"`, Bluetooth, 1 in / 1 out;
  - `mic()` = `"BuiltInMicrophoneDevice"` / `"MacBook Pro Microphone"`, BuiltIn, 1 in / 0 out.
- Helpers:
  - `at(ms) -> Instant`;
  - `state_file(dir, &State)`, which writes JSON with `to_json`;
  - `boot(fake, dir) -> (Engine<Fake, Recorder>, Recorder)`, which opens the store at `dir/state.json` and starts at `at(0)`.

- [ ] **Step 1: Write the failing scenarios (`tests/engine.rs`)**

```rust
#[test] fn launch_without_file_seeds_saves_and_switches_nothing() {
    // devices [spk, pods, mic], current out=spk in=mic, no state.json
    let (e, _) = boot(fake.clone(), dir);
    assert_eq!(e.saves(), 1);
    assert_eq!(read_state(dir).output.order[0].as_str(), "BuiltInSpeakerDevice");
    assert!(fake.world().sets.is_empty()); }
#[test] fn events_inside_150ms_coalesce_into_one_scan() {
    // scans == 1 after boot
    e.handle(Event::Audio(AudioEvent::DevicesChanged), at(1_000));
    e.handle(Event::Audio(AudioEvent::DefaultChanged(Role::Output)), at(1_100));
    assert_eq!(e.next_due(), Some(at(1_150)));
    e.tick(at(1_149)); assert_eq!(fake.world().scans, 1);
    e.tick(at(1_150)); assert_eq!(fake.world().scans, 2); }
#[test] fn desired_output_is_set_once() {
    // state order output [pods, spk]; current out=spk
    assert_eq!(fake.world().sets, vec![(Role::Output, "AirPods-uid".into())]); }
#[test] fn failed_set_shows_an_error_and_is_not_retried_in_the_event() {
    // fail_set[pods] = Status{call:"set default", status:-50}
    assert_eq!(fake.world().sets.len(), 1);
    assert_eq!(e.view().errors.output.as_deref(),
        Some("couldn't switch output to AirPods: set default failed (OSStatus -50)"));
    assert_eq!(fake.world().current.output.as_ref().unwrap().as_str(), "BuiltInSpeakerDevice"); }
#[test] fn vanished_device_is_an_error_line_not_a_panic() {
    // vanish_on_set = {pods}: Fake returns NotFound and removes it from devices
    assert_eq!(e.view().errors.output.as_deref(),
        Some("couldn't switch output to AirPods: device AirPods-uid is gone")); }
#[test] fn error_clears_once_desired_equals_current() {
    // after the failure: clear fail_set, set current out = pods, DefaultChanged, tick +150
    assert_eq!(e.view().errors.output, None); }
#[test] fn guard_trip_pauses_and_tells_the_shell() {
    // order output [spk, pods]; current=pods; sticky.output = pods
    // boot (write 1), DefaultChanged at 200 → tick 350 (write 2), at 500 → tick 650 (trip)
    assert_eq!(fake.world().sets.len(), 2);
    assert!(rec.calls().contains(&Call::Auto(false)));
    assert!(matches!(rec.last_paused(), Some(p) if p.name == "AirPods" && p.role == Role::Output));
    assert!(!e.state().auto_switch); }
#[test] fn toggle_saves_after_500ms_not_before() {
    let base = e.saves();
    e.handle(Event::ToggleAutoSwitch, at(1_000));
    assert!(rec.calls().contains(&Call::Auto(false)));
    e.tick(at(1_499)); assert_eq!(e.saves(), base);
    e.tick(at(1_500)); assert_eq!(e.saves(), base + 1);
    assert!(!read_state(dir).auto_switch); }
#[test] fn change_after_a_save_is_in_the_next_save() {
    e.handle(Event::ToggleAutoSwitch, at(1_000)); e.tick(at(1_500));
    e.handle(Event::ToggleAutoSwitch, at(1_501)); e.tick(at(2_001));
    assert!(read_state(dir).auto_switch); }
#[test] fn unresolved_usb_rescans_once_after_500ms() {
    // usb_unresolved = true; boot scans once and sets usb_due
    e.tick(at(500)); assert_eq!(fake.world().scans, 2);
    e.tick(at(1_000)); assert_eq!(fake.world().scans, 2); }
#[test] fn panel_view_serialises_errors_beside_the_panel() {
    let v = serde_json::to_value(e.view()).unwrap();
    assert!(v.get("autoSwitch").is_some()); assert!(v["errors"].get("output").is_some()); }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p usher --test engine`
Expected: compile errors, `usher_lib::engine` not found.

- [ ] **Step 3: Implement `engine/mod.rs` and `engine/exec.rs`**

Follow the behaviour list above. Keep each function under 40 lines: split `run_reconcile`, `execute`, `set_one`, `clear_errors`, `on_command`.

- [ ] **Step 4: Run the tests and clippy**

Run: `cargo test -p usher && cargo clippy -p usher --all-targets -- -D warnings`
Expected: all pass, no clippy output.

- [ ] **Step 5: Commit**

```bash
git add src-tauri
git commit -m "feat(app): engine coalesces events, reconciles and applies actions"
```

---

### Task 7: Engine lifecycle (rebind backoff, quit flush, thread loop)

**Files:**
- Create: `src-tauri/src/engine/rebind.rs`, `src-tauri/src/engine/thread.rs`, `src-tauri/tests/lifecycle.rs`
- Modify: `src-tauri/src/engine/mod.rs`

**Interfaces:**
- Consumes: Task 6 `Engine`, `Event`, `Flow`.
- Produces:
  - `pub fn rebinding(&self) -> bool` on `Engine`: true while a rebind deadline is pending.
  - `pub fn retry_delay(attempt: u32) -> Duration`: attempt 0 → 1 s, 1 → 2 s, 2 → 4 s, 3 → 8 s, 4 → 16 s, 5 and up → 60 s.
  - `#[derive(Clone)] pub struct EngineHandle(Sender<Event>)`, with:
    - `pub fn new() -> (EngineHandle, Receiver<Event>)`
    - `pub fn send(&self, ev: Event) -> bool`: false when the engine is gone, with `debug!`.
    - `pub fn notify(&self) -> Notify`: a closure that sends `Event::Audio` and ignores a send error.
    - `pub fn quit(&self, timeout: Duration) -> bool`
  - `pub fn run<A: AudioSystem, S: Shell>(engine: Engine<A, S>, rx: Receiver<Event>)`. It loops on `recv_timeout(next_due − now)` (or `recv()` with no deadline): an `Event` goes to `handle`, a timeout to `tick`. It stops on `Flow::Stop`. If every sender is dropped, it flushes and stops.

Rebind behaviour:
- **`ServiceRestarted`.** `info!("coreaudiod restarted; rebinding")`, `attempt = 0`, `rebind_due = now` (it runs on the next tick), and any pending reconcile is cancelled.
- **Attempt.** `info!("rebind attempt {n}")`, then `audio.rebind()` then `scan`.
  - On success: `info!("rebound after {n} attempt(s)")`, clear the backoff, reconcile with that scan.
  - On failure: `warn!("rebind failed: {e}; next try in {d:?}")` and `rebind_due = now + retry_delay(attempt)`, then `attempt += 1`.
- **Launch.** A failed `subscribe` or first `scan` in `start` enters the same backoff. Here the attempt is `subscribe` (rebind when already subscribed) then `scan`.
- **While a rebind is pending**, coalesced audio events are ignored with `debug!`.
- **`Quit(ack)`.** `store.flush`, `audio.unsubscribe()`, `ack.send(())` (ignoring an error), `Flow::Stop`.

- [ ] **Step 1: Write the failing tests**

Inline in `rebind.rs`:

```rust
#[test] fn retry_schedule_is_1_2_4_8_16_then_60_forever() {
    let secs: Vec<u64> = (0..8).map(|a| retry_delay(a).as_secs()).collect();
    assert_eq!(secs, vec![1, 2, 4, 8, 16, 60, 60, 60]);
    assert_eq!(retry_delay(u32::MAX), Duration::from_secs(60)); }
```

`tests/lifecycle.rs`:

```rust
#[test] fn service_restarted_rebinds_then_reconciles() {
    e.handle(Event::Audio(AudioEvent::ServiceRestarted), at(1_000)); e.tick(at(1_000));
    assert_eq!(fake.world().rebinds, 1); assert_eq!(fake.world().scans, 2); }
#[test] fn failed_rebinds_back_off_1_2_4_8_16_then_60() {
    // fail_rebind = 6; restart at 0 → attempts at 0, 1s, 3s, 7s, 15s, 31s, 91s
    for (t, n) in [(0,1),(999,1),(1_000,2),(3_000,3),(7_000,4),(15_000,5),(31_000,6),(90_999,6),(91_000,7)] {
        e.tick(at(t)); assert_eq!(fake.world().rebinds, n, "at {t}"); }
    assert!(!e.rebinding()); // the 7th attempt succeeded
}
#[test] fn restart_mid_event_recovers_after_rebind() {
    // order [pods, spk]; current spk; fail_set[pods] = Status{call:"set default", status:560947818}
    // boot: set fails → error line; then ServiceRestarted; clear fail_set; tick
    assert_eq!(fake.world().rebinds, 1);
    assert_eq!(fake.world().sets.len(), 2);
    assert_eq!(e.view().errors.output, None); }
#[test] fn events_during_backoff_do_not_scan() {
    // fail_rebind = 1; restart at 0; tick 0 (fails); DevicesChanged at 100; tick 250
    assert_eq!(fake.world().scans, 1); }
#[test] fn start_failure_enters_backoff() {
    // fail_subscribe = 1 → no scan at boot; tick at(1_000) → subscribed, scans == 1
}
#[test] fn quit_inside_debounce_window_flushes() {
    let (tx, rx) = mpsc::channel();
    e.handle(Event::ToggleAutoSwitch, at(1_000));
    assert!(matches!(e.handle(Event::Quit(tx), at(1_100)), Flow::Stop));
    assert!(!read_state(dir).auto_switch); assert!(rx.try_recv().is_ok());
    assert!(!fake.world().subscribed); }
#[test] fn notify_after_engine_stopped_is_silent() {
    let (h, rx) = EngineHandle::new(); let n = h.notify(); drop(rx);
    n(AudioEvent::DevicesChanged); } // no panic
#[test] fn send_after_quit_returns_false() {
    let (h, rx) = EngineHandle::new(); drop(rx);
    assert!(!h.send(Event::ToggleAutoSwitch)); assert!(!h.quit(Duration::from_millis(10))); }
#[test] fn run_loop_flushes_and_stops_when_senders_drop() {
    // spawn run(engine, rx) on a thread; send ToggleAutoSwitch; drop handle; join
    assert!(!read_state(dir).auto_switch); }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p usher --test lifecycle && cargo test -p usher rebind`
Expected: compile errors, `retry_delay`, `EngineHandle` and `run` not found.

- [ ] **Step 3: Implement `rebind.rs`, `thread.rs` and the `mod.rs` hooks**

- [ ] **Step 4: Run the tests and clippy**

Run: `cargo test -p usher && cargo clippy -p usher --all-targets -- -D warnings`
Expected: all pass, no clippy output.

- [ ] **Step 5: Commit**

```bash
git add src-tauri
git commit -m "feat(app): coreaudiod rebind backoff, quit flush and the engine thread"
```

---

### Task 8: App wiring (tray toggle and icons, notification, start and exit)

**Files:**
- Create: `src-tauri/src/shell.rs`, `src-tauri/icons/tray-off-template.svg`, `src-tauri/icons/tray-off-template.png`
- Modify: `src-tauri/src/tray.rs`, `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml` (`tauri-plugin-notification = "2.8.1"`)

**Interfaces:**
- Consumes: `EngineHandle`, `run`, `Engine::start`, `Clock`, `Store::open`, `CoreAudioSystem`, `logging::apply`.
- Produces:
  - `tray::build(app: &AppHandle) -> tauri::Result<()>`: the menu is `auto` (CheckMenuItem "Auto-switch", checked), a separator, then `quit`. Clicking `auto` sends `Event::ToggleAutoSwitch` through `app.state::<EngineHandle>()`.
  - `pub fn set_auto(app: &AppHandle, on: bool)`: sets the icon, the check mark and the tooltip.
  - `pub fn tooltip(on: bool) -> &'static str`: `"usher"` or `"usher: auto-switch off"`.
  - `pub struct TauriShell(AppHandle)`, with `impl Shell`:
    - `state_changed` → `app.emit("state-changed", view)`;
    - `auto_switch_changed` → `tray::set_auto`;
    - `paused` → spawns a thread that checks or requests notification permission, then shows `title "usher"`, `body reason.message()` on `Granted`.
  - `pub fn spawn(app: &AppHandle, rx: Receiver<Event>, notify: Notify)`:
    - path: `app_data_dir()/state.json`;
    - `Store::open`, then `logging::apply(state.debug or false)`;
    - starts a thread named `usher-engine` that runs `Engine::start(CoreAudioSystem::new(), TauriShell(app), …)`, then `run`.

`lib.rs` setup order, after the instance lock:
1. `let (handle, rx) = EngineHandle::new();`
2. `app.manage(handle.clone())`
3. `tray::build`
4. `shell::spawn(app.handle(), rx, handle.notify())`

The builder adds `.plugin(tauri_plugin_notification::init())`. `app.run` handles `RunEvent::Exit` with `app.state::<EngineHandle>().quit(Duration::from_secs(1))`. `logging::apply(false)` stays right after build; the engine re-applies it with the persisted setting.

- [ ] **Step 1: Draw the off glyph and generate the PNG**

`tray-off-template.svg` is the M1 speaker (`src-tauri/icons/tray-template.svg`, same 44×36 size and 22×18 viewBox) with the two sound waves replaced by a diagonal slash: `<path d="M13 3l8 12" stroke="#000" stroke-width="1.8" stroke-linecap="round"/>`.

Run: `sips -s format png src-tauri/icons/tray-off-template.svg --out src-tauri/icons/tray-off-template.png && file src-tauri/icons/tray-off-template.png`
Expected: `PNG image data, 44 x 36, 8-bit/color RGBA`.

- [ ] **Step 2: Write the failing tests (inline in `tray.rs`)**

```rust
#[test] fn both_tray_glyphs_are_44x36() {
    for bytes in [ICON_ON, ICON_OFF] {
        let i = tauri::image::Image::from_bytes(bytes).unwrap();
        assert_eq!((i.width(), i.height()), (44, 36)); } }
#[test] fn tooltip_says_when_auto_switch_is_off() {
    assert_eq!(tooltip(true), "usher"); assert_eq!(tooltip(false), "usher: auto-switch off"); }
```

- [ ] **Step 3: Run them to verify they fail**

Run: `cargo test -p usher tray`
Expected: compile error, `ICON_OFF` and `tooltip` not found.

- [ ] **Step 4: Implement `tray.rs`, `shell.rs` and the `lib.rs` wiring**

- [ ] **Step 5: Run the full quality gate**

Run each command in turn: `pnpm build && pnpm test && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check && pnpm check:size`
Expected: every command exits 0, and `check:size` reports no oversized file.

- [ ] **Step 6: Smoke run**

Run: `USHER_LOG=debug pnpm tauri dev`
Expected:
- The log shows `first launch: seeded from current defaults` (or a loaded state), then one `Reason` line per role, and no `set … default` line.
- The tray menu shows `Auto-switch ✓`, a separator, then `Quit usher`.
- Clicking Auto-switch swaps to the slashed icon. Within 1 s, `state.json` has `"auto_switch": false`.
- Quit exits cleanly, and `state.json.tmp` is absent.

- [ ] **Step 7: Commit**

```bash
git add src-tauri Cargo.lock
git commit -m "feat(app): tray auto-switch toggle, loop-guard notification, engine wiring"
```

---

### Task 9: Docs (ADR 0006, QA checklist, architecture, roadmap)

**Files:**
- Create: `docs/adr/0006-engine-threading-and-coreaudio-binding.md`, `docs/qa.md`
- Modify: `docs/adr/README.md` (index row), `docs/architecture/overview.md`, `docs/superpowers/plans/2026-11-23-usher-roadmap.md`

- [ ] **Step 1: Write ADR 0006** (template from `docs/adr/README.md`, Status Accepted, Date 2026-11-23)

It covers four things.

**Engine threading model.** There is one `std::thread`, one mpsc channel, and deadlines served through `recv_timeout`. Saves run synchronously on that thread. Quit is a handshake with a 1 s timeout. Compare it with an async runtime and with a Mutex-shared state, both rejected because there is one owner and nothing async to wait on.

**Binding crate.** The choice is `objc2-core-audio 0.3.2`, from the objc2 0.6 family already in the lock, with no bindgen or clang. The alternatives were:
- `coreaudio-sys 0.2.18`, rejected: bindgen at build time;
- `coreaudio-rs 0.14`, rejected: it targets AudioUnits, not device properties.

**Listener API.** Blocks with a `None` queue, plus the generation gate. The C-proc variant was rejected because it needs a manually managed context pointer.

**IOKit check.** `objc2-io-kit 0.3.2`. There is one `IOUSBHostDevice` walk per scan, the audit §3.3 verdict, an unresolved device rescanned once after 500 ms, and lookup by `locationID`.

Also record the state-file policy from Review Focus item 5, and Decisions 5, 6, 7 and 13.

- [ ] **Step 2: Write `docs/qa.md`**

Each check is a numbered row with an action and an expected result. Run them with `USHER_LOG=debug` and watch `~/Library/Logs/com.cef62.usher/usher.log`.

1. **AirPods connect.** Rank the built-in mic above AirPods, then put the AirPods in. The input goes back to the built-in mic within about 1 s, and the log shows `set input default … ok`.
2. **AirPods output ranked first.** Connect them: the output moves to AirPods, and system alert sounds play on the AirPods too.
3. **USB interface on two ports.** Plug it into port A and rank it first. Quit, move it to port B, relaunch. It keeps its rank (`alias merged` in the log) and no duplicate row appears in `state.json`.
4. **Display reconnect.** Unplug and replug an HDMI/DP display that has audio. Its rank is kept, and the default returns to the top-ranked device.
5. **coreaudiod restart.** Run `sudo killall coreaudiod`. The log shows `coreaudiod restarted; rebinding`, then `rebound after 1 attempt(s)`. Connecting a device afterwards still switches.
6. **Loop guard.** Rank the speakers first and play audio to AirPods. Re-pick the AirPods in Control Center 3 times within 10 s. Auto-switch turns off, the icon gets its slash, and on the first trip the notification permission is asked, then `usher paused auto-switch: AirPods kept taking output` appears.
7. **Toggle back on.** Use the tray Auto-switch item: the icon is restored, the check mark is set, and the top-ranked device is applied.
8. **Corrupt file.** Quit, write `{oops` into `state.json`, relaunch. `state.json.corrupt` exists, a fresh `state.json` is seeded, and audio is not switched.
9. **Newer schema.** Quit, set `"schema_version": 2`, relaunch. The log has an error line, and the file is byte-identical after a toggle and a quit.
10. **Quit flush.** Toggle Auto-switch and quit within 0.5 s. After relaunch the toggle has persisted.

- [ ] **Step 3: Update the architecture overview and the roadmap**

In `docs/architecture/overview.md`:
- In the sequence diagram, replace `reconcile(devices, state, now)` with `reconcile(state, runtime, snapshot, now)`, and replace the action list with `Outcome {actions: SetDefault | PauseAuto | AliasMerged | Persist, reasons}`.
- Add `ServiceRestarted -> rebind` to the event line.
- Add a "Timers" note: coalesce 150 ms, save 500 ms, USB recheck 500 ms, backoff 1/2/4/8/16/60 s.

In the roadmap:
- Set the M3 Plan cell to `2026-11-23-usher-m3-engine.md`.
- Add a "Carry-overs from M3 (for the M4 plan)" list:
  - the `state-changed` payload is `PanelView` (`PanelState` flattened plus `errors.{output,input}`);
  - panel commands go through `EngineHandle::send(Event::Command(..))`, and M4 adds a reply channel if the panel needs the `CommandError`;
  - on `SetDebug`, the engine must call `logging::apply`;
  - the tray icon state is set by the engine through `Shell::auto_switch_changed`.

- [ ] **Step 4: Run the size check and commit**

Run: `pnpm check:size`
Expected: exit 0.

```bash
git add docs
git commit -m "docs: ADR 0006, QA checklist, M3 architecture and roadmap updates"
```

---

## Self-review

1. **Spec coverage:**

| Spec item | Task |
|---|---|
| §5.2 system output | Task 4 |
| §5.2 OSStatus check and no retry | Tasks 1, 4, 6 |
| §5.3 150 ms coalescing and the event list | Task 6 (launch, devices, defaults, prefs); Task 7 (restart) |
| §5.4 notification on first trip | Task 8 |
| §5.4 icon off | Tasks 6, 8 |
| §5.7 path, atomic write, corrupt aside, unknown keys, 500 ms, quit flush | Tasks 5, 7 |
| §5.9 rebind and backoff | Task 7 |
| §5.10 seed and save | Task 6 |
| §5.11 log lines | Tasks 2, 5, 6, 7 |
| §6 unsafe only in coreaudio, engine owns state, `AudioSystem` | Tasks 1-4, 6 |
| §7.2 icon glyphs | Task 8 |
| §9 FakeAudioSystem engine tests, the `#[ignore]` smoke test, `docs/qa.md` | Tasks 6, 7, 2/4, 9 |

2. **Step scan:** every code step names its file, signature and pinned values. Bodies appear only for the IOKit verdict rules and the backoff table.
3. **Type consistency:**
   - `Notify`, `Scan`, `AudioError::{NotFound, Status{call,status}}`, `Event::{Audio, Command, ToggleAutoSwitch, Quit}`, `PanelView{panel, errors}` and `EngineHandle::{new, send, notify, quit}` are used with the same names in every task.
   - `Engine::rebinding()` is added in Task 7.
4. **Review Focus:** each of the 7 items names its test and owning task.
5. **Proportion:** the code blocks are test assertions and signatures. There are no function bodies.

## Execution handoff

Recommended: **Subagent-driven**. Tasks 1-4 (`unsafe` FFI) and 6-7 (timing state machine) can each be rejected on their own. A shipped mistake there either switches someone's audio wrongly or leaves usher deaf after a coreaudiod restart.
