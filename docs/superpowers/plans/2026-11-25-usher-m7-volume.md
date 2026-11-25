# usher M7: Volume and mute Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The panel gets a volume slider and a mute button for the active output and the active input. Both follow changes made elsewhere (volume keys, Control Center) live. The menu-bar icon shows a small dot while the active input is muted. Ships as 0.3.0.

**Architecture:** `usher-coreaudio` gains level reads, writes and per-device level listeners that send `AudioEvent::LevelChanged(role)`. The engine owns every write, as it owns `set_default` (ADR 0006). A new `Event::Level(LevelRequest, Reply)` handles writes without a store write or a reconcile. A refusal (`NotReady`, `Rebinding`, or the new `Stale`) leaves the volume alone. After every reconcile pass the engine re-points the level listeners at each role's current device and re-reads its levels. `LevelChanged` re-reads one role and emits the view at once. The view carries `levels` and `levelErrors`. The panel draws one strip per role, and its slider writes go through a latest-value pump. The shell derives the mic-muted dot from the view.

**Tech Stack:** existing only: `objc2-core-audio` 0.3.2 (`AudioObjectHasProperty`, `AudioObjectIsPropertySettable`, the property get/set and listener-block calls already in use), React 19, Vitest with happy-dom, `sips` for the tray PNGs. No new dependency.

**Spec:** `docs/superpowers/specs/2026-11-24-usher-v2-design.md`. Sections: §2 (goals, non-goals, anti-goals), §6 (M7), §8, §9 (ADR 0013), §10, §11 (release 0.3.0) and §12. The v1 spec `docs/superpowers/specs/2026-11-23-usher-design.md` still holds, including ADR 0006 as amended here.

**Branch:** `feat/m7-volume` (exists, from `main` at db6290c). One PR.

## Global Constraints

- **Size limits:**
  - Non-test `.rs`, `.ts`, `.tsx` files stay at 300 lines or fewer; functions at 40 lines or fewer (`clippy::too_many_lines`).
  - `src-tauri/src/engine/mod.rs` is at 271 lines: new engine logic goes in `engine/level.rs`, and `mod.rs` gains only the `Event` variant, one field and the dispatch arms.
  - `src-tauri/src/store.rs` is at 300 lines: do not touch it.
  - `crates/usher-coreaudio/src/listen.rs` is at 180 lines: level listeners go in a new `watch.rs`.
- `unsafe` only in `crates/usher-coreaudio`. Every `unsafe` block carries a `// SAFETY:` line (`clippy::undocumented_unsafe_blocks` is denied). `usher-core` stays pure.
- **Engine-only writes:** only the engine thread writes CoreAudio state (ADR 0006). Volume and mute writes go through `Event::Level`. They never mark the store dirty, never schedule a reconcile, never change which device is active, and never make an Activity entry or a notice.
- **AudioObjectID rule (ADR 0013):** an `AudioObjectID` lives only inside one call. The one exception is the level watch, which holds the watched device's ID from registration to removal; nothing else holds one.
- **Copy (verbatim):**
  - Error lines: `No volume control`, `Couldn't set volume: <error>`, `Couldn't set mute: <error>`.
  - Button text `Mute`. Accessible names `Output volume`, `Input volume`, `Mute output`, `Mute input`.
  - Tooltips:
    - `usher` (auto-switch on, mic not muted)
    - `usher: auto-switch off`
    - `usher: microphone muted`
    - `usher: auto-switch off, microphone muted`
  - `Refusal::Stale` displays `the active device changed; try again`.
- **Values:**
  - Panel slider: 0–100 in steps of 1, shown as `round(volume × 100)`.
  - Engine and CoreAudio: `f32` from 0.0 to 1.0.
  - Panel pump: at most one write in flight, at least 50 ms between write starts, and the last value always sent.
- **IPC:** command `set_level`, in `PANEL_COMMANDS` only. JSON shapes:
  - `{"kind":"volume","role":"output","key":"<uid>","value":0.5}`
  - `{"kind":"mute","role":"input","key":"<uid>","on":true}`
- **Tray:** four 44×36 template PNGs, made from 22×18-viewBox SVGs with `sips`:
  - `tray-template.png` and `tray-off-template.png` (they exist);
  - `tray-muted-template.png` and `tray-off-muted-template.png` (new).
- **Hardware safety:**
  - Agents never change the owner's volume or mute.
  - No smoke test writes a level; the new smoke test only reads.
  - Agents never run the app, `pnpm tauri dev`, or `cargo test -- --ignored` on the write smoke tests.
  - Briefs forbid reading `~/Library/Application Support/com.cef62.usher/`.
- **Quality gate (CONTRIBUTING):**
  - Verify: `pnpm build`, `pnpm test`, `pnpm check:size`, `pnpm docs:build`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
  - App build: `pnpm build:app`.
  - Report the gate as a table.
- One `minor` changeset (Task 5). The site Usage page, README and screenshots move with the UI (Task 5).

## Owner prerequisites

None. Release 0.3.0 follows the usual Version Packages PR after the M7 merge, on the owner's word.

## Review Focus

1. **A `LevelChanged` must never reach the reconcile path.**
   - `Engine::on_audio` maps every non-`DefaultChanged` event to `Trigger::DevicesChanged` through a `_` arm. A volume-key press would then scan, reconcile and maybe switch.
   - Expected: no scan, no reconcile. The `usb_rechecked` re-arm does not run.
   - Pinned in Task 2 (`level_changed_emits_without_a_scan`).
2. **The active device changes mid-drag**, for example a headset is unplugged while the slider is held.
   - Expected: the request carries the device the drag started on, the live check refuses it as `Stale`, and the new device's volume is untouched.
   - Pinned in Task 2 (`a_request_for_a_device_that_is_no_longer_default_is_stale`), and in Task 4 (`the drag keeps its key`).
3. **Slider events overtake each other.** Each `invoke` runs its own `spawn_blocking`, so two in-flight writes can reach the engine in either order.
   - Expected: the device ends at the value where the drag stopped.
   - Pinned in Task 4 (`pump sends the last value after the in-flight write settles`).
4. **coreaudiod restarts, or a device is unplugged and re-plugged within one 150 ms window** (same UID, new `AudioObjectID`).
   - Expected: volume keys still move the slider afterwards.
   - Pinned in Task 1 (`Holding::needs` is true for a new ID with the same key), Task 2 (`watches_follow_every_reconcile`), and QA rows 48–49.
5. **A device with a read-only or missing volume or mute** (HDMI display, some USB DACs, AirPlay).
   - Expected: a disabled slider with `No volume control`, and no mute button when mute is not settable.
   - Never a slider that moves without effect.
   - Pinned in Task 1 (`volume_selector_prefers_virtual_main_then_scalar`) and Task 4 (`no control renders disabled`).

## Decisions the spec leaves open

1. **VirtualMainVolume goes through the AudioObject API.**
   - A read-only probe on 2026-11-25 (macOS 27.2, built-in speaker and microphone) found `kAudioHardwareServiceDeviceProperty_VirtualMainVolume` (`'vmvc'`, `0x766d7663`) on the main element:
     - `AudioObjectHasProperty` is true;
     - `IsPropertySettable` is true;
     - `AudioObjectGetPropertyData` succeeds;
     - add and remove of a listener block return 0, in both output and input scope.
   - So no AudioToolbox `AudioHardwareService*` call is needed. Define the constant locally in `level.rs`, with that comment.
   - Fallback: `kAudioDevicePropertyVolumeScalar` on element 0. Mute: `kAudioDevicePropertyMute` (`u32`) on element 0.
   - A property counts as a control only when it is present AND settable (`None` otherwise).
2. **Trait signatures.** These differ from spec §6.2, and ADR 0013 records them.
   - `levels(role, key)` takes the key, as writes do, so the strip's name and its levels always describe the same device. The spec form, which reads the live default, would show one device's name with another's volume during the 150 ms window.
   - `default_key(role)` is added. It is the live read behind `Stale`.
   - `watch_levels(role, key)` moves the role's listeners. With the same `AudioObjectID` it does nothing. The engine calls it after every reconcile pass. That covers `DefaultChanged`, `DevicesChanged` (including a same-UID replug), launch and rebind without tracking which event happened.
3. **Gates:**
   - Each role's level listeners have their own `Gate`. `watch_levels` bumps only that role's gate, so the four system listeners are untouched.
   - `unsubscribe` and `rebind` remove the level listeners too (all gates bumped), and the next reconcile re-adds them.
   - A failed remove (the device vanished) is logged at `debug` and ignored.
4. **Failed writes and replies.**
   - The reply to `Event::Level` carries refusals only. A failed write replies `Ok(())`: the error reaches the panel through `levelErrors`, which persists until the next successful write for that role or until the role's current device changes.
   - The engine re-reads the role's levels after every write, successful or not, and emits only when they changed.
   - A refusal is logged at `debug` when it differs from the previous refusal; a success resets this. That gives one line per drag.
5. **Validation at the IPC boundary.**
   - `set_level` rejects a non-finite `value` with `invalid volume`.
   - The engine clamps to 0.0–1.0 (`usher_core::clamp_volume`).
6. **When there is no strip.**
   - The panel shows a role's strip only when that role has an `active` row.
   - `levels[role] = null` (read failed) renders like a device with no controls.
   - Auto-switch off, a pause or a read-only store do not hide or disable the strip.
7. **Drag behaviour.**
   - While the pointer is down, the slider shows the local value and ignores view echoes for that role. The drag keeps the key it started with.
   - On release it sends the final value, then shows the view value again.
   - Keyboard steps (arrows) go through the same pump with no hold.
8. **Tray.**
   - `MicMuted(AtomicBool)` is managed next to `LastAuto`.
   - `Shell::state_changed` sets it from `view.levels.input.and_then(|l| l.muted) == Some(true)`, and redraws only when the bit flips.
   - The icon and the tooltip are pure functions of `(auto, muted)`.
   - The dot is a filled circle `cx="18.5" cy="14.5" r="2.5"`, appended to each existing SVG (bottom right, clear of the bars and waves). Builder's call per spec §12.

---

## File structure

| File | Responsibility | Budget |
|---|---|---|
| `crates/usher-core/src/level.rs` | `Level`, `LevelRequest`, `clamp_volume` | 60 |
| `crates/usher-coreaudio/src/level.rs` | volume/mute selector choice, `read_levels`, `write_volume`, `write_mute`, `default_key` | 130 |
| `crates/usher-coreaudio/src/watch.rs` | `LevelWatch`: per-role holding, gate, add/remove on a device | 110 |
| `crates/usher-coreaudio/src/{lib,system,listen,write}.rs` | `AudioEvent::LevelChanged`, trait methods, `translate` and `listener` made `pub(crate)` | +30 |
| `src-tauri/src/engine/level.rs` | `Levels` state, `on_level`, `on_level_changed`, `sync_levels` | 130 |
| `src-tauri/src/engine/{mod,exec,command,thread,rebind}.rs` | `Event::Level`, field, dispatch, `Refusal::Stale`, `EngineHandle::level`, the explicit `on_audio` arms | +25 |
| `src-tauri/src/{commands,lib,shell,tray}.rs` | `set_level`, list, handler, mic dot | +60 |
| `src-tauri/icons/tray{,-off}-muted-template.{svg,png}` | dot glyphs | assets |
| `src-tauri/tests/{common/mod.rs,levels.rs}` | fake levels, engine scenarios | tests |
| `src/types.ts`, `src/api.ts` | `Level`, `LevelRequest`, `View.levels`, `View.levelErrors`, `panelApi.setLevel` | +10 |
| `src/panel/pump.ts` | latest-value write pump | 40 |
| `src/panel/LevelStrip.tsx` | one role's strip | 90 |
| `src/panel/PanelView.tsx`, `src/panel/panel.css` | strips under the header | +20 |
| docs, site, ADR 0013, QA, changeset | Task 5 | — |

---

### Task 1: Level types and the CoreAudio binding

**Files:**
- Create: `crates/usher-core/src/level.rs`, `crates/usher-coreaudio/src/level.rs`, `crates/usher-coreaudio/src/watch.rs`
- Modify:
  - `crates/usher-core/src/lib.rs`;
  - `crates/usher-coreaudio/src/{lib,system,listen,write}.rs`;
  - `crates/usher-coreaudio/tests/smoke.rs`;
  - `src-tauri/tests/common/mod.rs` (the fake must keep compiling).
- Test: inline in each new file; `smoke.rs` (ignored, read-only).

**Interfaces:**
- Produces in `usher_core`:
  - `#[derive(Serialize, Clone, Copy, PartialEq, Debug)] #[serde(rename_all = "camelCase")] pub struct Level { pub volume: Option<f32>, pub muted: Option<bool> }`. `None` means no settable control.
  - `#[derive(Deserialize, Clone, PartialEq, Debug)] #[serde(tag = "kind", rename_all = "camelCase")] pub enum LevelRequest { Volume { role: Role, key: DeviceKey, value: f32 }, Mute { role: Role, key: DeviceKey, on: bool } }`.
  - `impl LevelRequest { pub fn role(&self) -> Role; pub fn key(&self) -> &DeviceKey; pub fn valid(&self) -> bool }`. `valid` is false only for a non-finite `value`.
  - `pub fn clamp_volume(v: f32) -> f32` clamps to 0.0..=1.0.
- Produces in `usher_coreaudio`:
  - `AudioEvent::LevelChanged(Role)`.
  - `AudioSystem` gains:
    ```rust
    fn levels(&self, role: Role, key: &DeviceKey) -> Result<Level, AudioError>;
    fn default_key(&self, role: Role) -> Result<Option<DeviceKey>, AudioError>;
    fn set_volume(&self, role: Role, key: &DeviceKey, value: f32) -> Result<(), AudioError>;
    fn set_mute(&self, role: Role, key: &DeviceKey, on: bool) -> Result<(), AudioError>;
    /// Move this role's level listeners to `key`'s device (`None`: remove them). Same AudioObjectID: no-op.
    fn watch_levels(&mut self, role: Role, key: Option<&DeviceKey>) -> Result<(), AudioError>;
    ```
  - `CoreAudioSystem::{unsubscribe, rebind, drop}` also clear the level watch.
  - `level.rs`: `pub(crate) fn volume_selector(vmvc: (bool, bool), scalar: (bool, bool)) -> Option<u32>`, where each pair is (present, settable).
  - `watch.rs`: `pub(crate) struct Holding { id: u32, key: DeviceKey, .. }` with `fn needs(held: Option<&Holding>, id: u32) -> bool`.
- Fake (`src-tauri/tests/common/mod.rs`):
  - `World` gains:
    - `levels: BTreeMap<DeviceKey, Level>` (a missing key → `Err(NotFound)`);
    - `level_writes: Vec<LevelRequest>`;
    - `fail_level: Option<AudioError>`;
    - `fail_default_read: bool`;
    - `watches: Vec<(Role, Option<DeviceKey>)>`.
  - `default_key` returns `current[role]`, or `Err` when `fail_default_read` is set.
  - A successful write updates `levels`, as the real device would.

- [ ] **Step 1: Write the failing tests**

```rust
// usher-core level.rs
#[test] fn level_serializes_camel_case_with_nulls() {
    assert_eq!(serde_json::to_string(&Level { volume: Some(0.5), muted: None }).unwrap(), r#"{"volume":0.5,"muted":null}"#);
}
#[test] fn requests_parse_from_the_panel_shape() {
    let v: LevelRequest = serde_json::from_str(r#"{"kind":"volume","role":"output","key":"k","value":0.25}"#).unwrap();
    assert_eq!(v, LevelRequest::Volume { role: Role::Output, key: "k".into(), value: 0.25 });
    let m: LevelRequest = serde_json::from_str(r#"{"kind":"mute","role":"input","key":"k","on":true}"#).unwrap();
    assert_eq!((m.role(), m.key().as_str()), (Role::Input, "k"));
}
#[test] fn clamp_and_validity() {
    assert_eq!((clamp_volume(-0.2), clamp_volume(1.5), clamp_volume(0.4)), (0.0, 1.0, 0.4));
    assert!(!LevelRequest::Volume { role: Role::Output, key: "k".into(), value: f32::NAN }.valid());
    assert!(!LevelRequest::Volume { role: Role::Output, key: "k".into(), value: f32::INFINITY }.valid());
}
// usher-coreaudio level.rs
#[test] fn volume_selector_prefers_virtual_main_then_scalar() {
    assert_eq!(volume_selector((true, true), (true, true)), Some(VIRTUAL_MAIN_VOLUME));
    assert_eq!(volume_selector((true, false), (true, true)), Some(kAudioDevicePropertyVolumeScalar));
    assert_eq!(volume_selector((false, false), (true, false)), None); // readable but not settable: no control
}
// usher-coreaudio watch.rs
#[test] fn holding_needs_a_new_registration_only_for_a_new_id() {
    // None → true; same id → false; same key, new id (replug in one window) → true
}
// smoke.rs (ignored, read-only)
#[test] #[ignore = "needs real audio hardware"]
fn reads_levels_of_the_current_output() { /* default_key(Output) == scan current; levels(...) is Ok; volume, when Some, is in 0..=1 */ }
```

- [ ] **Step 2: Run to verify they fail.** Run `cargo test --workspace`. Expected: compile errors.
- [ ] **Step 3: Implement.**
  - Translate the key at call time with the existing `write::translate` (make it `pub(crate)`).
  - Add `get_f32` / `set_f32` / `has` / `settable` helpers in `props.rs` beside `get_u32`, with the scope as a parameter.
  - `default_key` reads the default device ID (`0` → `Ok(None)`), then its UID, as `scan.rs` does.
  - `watch.rs` reuses `listen::Gate` and `listen::listener` (made `pub(crate)`). Add listeners only for the properties that are present.
- [ ] **Step 4: Run the full gate.** Expected: pass.
- [ ] **Step 5: Commit:** `feat(coreaudio): volume and mute reads, writes and level listeners`

---

### Task 2: Engine level handling and the `set_level` command

**Files:**
- Create: `src-tauri/src/engine/level.rs`, `src-tauri/tests/levels.rs`
- Modify: `src-tauri/src/engine/{mod,exec,command,thread}.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: everything Task 1 produces.
- Produces:
  - `Event::Level(LevelRequest, Option<Reply>)`.
  - `Refusal::Stale`.
  - `EngineHandle::level(&self, req: LevelRequest, timeout: Duration) -> Result<(), Refusal>`. You may share one private `ask` helper with `request`.
  - `PanelView` gains `levels: PerRole<Option<Level>>` and `level_errors: PerRole<Option<String>>` (JSON `levels`, `levelErrors`).
  - Engine field `levels: Levels`, holding the per-role `(DeviceKey, Level)`, the errors and the last refusal.
  - `engine/level.rs` methods:
    - `sync_levels(&mut self)`: called in `act` after `self.snap = Some(..)`, before `emit`. For each role it runs `watch_levels(role, current)` (a `warn` on error), then reads that role's levels.
    - `on_level_changed(&mut self, role)`: ignored with no snap or while rebinding; re-reads; emits only on change.
    - `on_level(&mut self, req) -> Result<(), Refusal>`: `NotReady` → `Rebinding` → live `default_key` ≠ `Some(key)` or a read error → `Stale`; then clamp, write, re-read, emit if changed.
  - `on_audio`: `LevelChanged` returns before the rebind and coalesce logic. The `_` arm becomes explicit (`DevicesChanged`).
  - `#[tauri::command] pub async fn set_level(engine, req: LevelRequest) -> Result<(), String>`:
    - `!req.valid()` → `Err("invalid volume")`;
    - else `spawn_blocking` `engine.level(req, REQUEST_TIMEOUT)`;
    - add it to `PANEL_COMMANDS` and to `generate_handler!`.
  - Logs:
    - `debug!("set {role} volume to {pct}% on {name}")` / `debug!("set {role} mute {on|off} on {name}")`;
    - `warn!("couldn't set {role} volume on {name} ({key}): {e}")` (same for mute);
    - `debug!("level request refused: {refusal}")`.

- [ ] **Step 1: Write the failing tests** (`src-tauri/tests/levels.rs`, the fake from Task 1)

```rust
#[test] fn level_changed_emits_without_a_scan() {
    // started engine, spk current, levels[spk] = 0.5; set levels[spk] = 0.8, send Audio(LevelChanged(Output))
    // scans unchanged; next_due() is None; last view levels.output == Some(Level { volume: Some(0.8), .. })
}
#[test] fn an_unchanged_level_emits_nothing() { /* LevelChanged with the same value: no new Call::View */ }
#[test] fn a_volume_request_writes_the_active_device_only() {
    // Level(Volume{Output, spk, 0.4}) → Ok; level_writes == [that]; saves() unchanged; entries() empty; scans unchanged
}
#[test] fn values_are_clamped() { /* value 1.5 → written 1.0 */ }
#[test] fn a_request_for_a_device_that_is_no_longer_default_is_stale() {
    // world.current.output = pods (live), request key spk → Err(Stale), no write; fail_default_read → Err(Stale)
}
#[test] fn requests_before_the_first_scan_or_during_a_rebind_are_refused() { /* NotReady; ServiceRestarted → Rebinding */ }
#[test] fn a_failed_write_shows_the_error_until_the_next_success() {
    // fail_level → reply Ok; view.level_errors.output == Some("Couldn't set volume: set volume failed (OSStatus -50)"); levels re-read
    // clear fail_level, write again → level_errors.output None
}
#[test] fn watches_follow_every_reconcile() {
    // after start: watches contains (Output, Some(spk)); DefaultChanged(Output) with current = pods, tick → last Output watch is pods
    // ServiceRestarted, tick → rebinds == 1 and a watch call after it; a device leaving with no default → (Output, None)
}
#[test] fn auto_switch_off_or_read_only_still_sets_levels() { /* SetAutoSwitch(false) then a volume request → written */ }
```

Inline in `commands.rs`: extend `each_window_calls_only_its_own_commands` so that `set_level` is allowed for `panel` only.

- [ ] **Step 2: Run to verify they fail.** Run `cargo test -p usher --test levels`. Expected: compile errors.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the full gate.** Expected: pass.
- [ ] **Step 5: Commit:** `feat: engine-owned volume and mute with live level updates`

---

### Task 3: Mic-muted dot on the menu-bar icon

**Files:**
- Create: `src-tauri/icons/tray-muted-template.{svg,png}`, `src-tauri/icons/tray-off-muted-template.{svg,png}`
- Modify: `src-tauri/src/tray.rs`, `src-tauri/src/shell.rs`

**Interfaces:**
- Consumes: `PanelView.levels` from Task 2.
- Produces:
  - `pub struct MicMuted(pub AtomicBool)`, managed in `tray::build`.
  - `pub fn icon(auto: bool, muted: bool) -> &'static [u8]`.
  - `pub fn tooltip(auto: bool, muted: bool) -> &'static str`.
  - `pub fn set_mic_muted(app: &AppHandle, muted: bool)`: it does nothing unless the stored bit flips.
  - `set_auto` and `set_mic_muted` share one main-thread redraw that reads both bits.

- [ ] **Step 1: Write the failing tests** (inline `tray.rs`)

```rust
#[test] fn all_four_tray_glyphs_are_44x36() { /* icon(a, m) for all four pairs decodes to 44×36 */ }
#[test] fn tooltip_names_each_state() {
    assert_eq!(tooltip(true, false), "usher");
    assert_eq!(tooltip(false, false), "usher: auto-switch off");
    assert_eq!(tooltip(true, true), "usher: microphone muted");
    assert_eq!(tooltip(false, true), "usher: auto-switch off, microphone muted");
}
#[test] fn each_state_has_its_own_glyph() { /* the four byte slices are pairwise different */ }
```

- [ ] **Step 2: Run to verify they fail.**
- [ ] **Step 3: Draw the glyphs.** Append `<circle cx="18.5" cy="14.5" r="2.5"/>` to copies of both SVGs. Render each with `sips -s format png <svg> --out <png>`. Check with `file` that the result is 44 × 36 RGBA. Look at each PNG.
- [ ] **Step 4: Implement**, then run the full gate. Expected: pass.
- [ ] **Step 5: Commit:** `feat: dot on the menu-bar icon while the microphone is muted`

---

### Task 4: Panel level strips

**Files:**
- Create: `src/panel/pump.ts`, `src/panel/pump.test.ts`, `src/panel/LevelStrip.tsx`, `src/panel/LevelStrip.test.tsx`
- Modify: `src/types.ts`, `src/api.ts`, `src/panel/PanelView.tsx`, `src/panel/Panel.tsx`, `src/panel/PanelView.test.tsx`, `src/panel/panel.css`

**Interfaces:**
- Consumes: the `set_level` JSON shapes and `View.levels` / `View.levelErrors` from Task 2.
- Produces:
  - TS types:
    - `Level = { volume: number | null; muted: boolean | null }`;
    - `LevelRequest` (the two shapes);
    - `View.levels: { output: Level | null; input: Level | null }`;
    - `View.levelErrors: { output: string | null; input: string | null }`.
  - `panelApi.setLevel(req)`, which invokes `set_level` with `{ req }`.
  - `Actions.setLevel(req: LevelRequest): Promise<void>`. It never rejects: errors are swallowed, because the engine logs them.
  - `export function pump<T>(send: (v: T) => Promise<unknown>, gapMs = 50): { push(v: T): void }`.
  - `export function LevelStrip({ role, row, level, error, setLevel }): JSX.Element`.
  - `PanelView` renders, after `<header>`, one `LevelStrip` per role that has an `active` row.
- Markup per strip:
  - the role title (`Output` / `Input`) and the device name;
  - `<input type="range" min=0 max=100 step=1 aria-label="Output volume">` with `aria-valuetext="<n>%"`;
  - `<button type="button" aria-pressed aria-label="Mute output">Mute</button>`;
  - decorative speaker or mic glyphs (inline SVG, `currentColor`, `aria-hidden`);
  - `No volume control` beside a disabled slider when `volume` is null;
  - no button when `muted` is null;
  - the error line (`role="alert"`) when `error` is set.

- [ ] **Step 1: Write the failing tests**

```ts
// pump.test.ts (vi.useFakeTimers)
it("sends the first value at once and the last value after the in-flight write settles", ...); // push 1,2,3 while 1 in flight → sends [1,3]
it("keeps 50 ms between write starts", ...);
it("never has two writes in flight", ...);
// LevelStrip.test.tsx
it("shows the active device and its volume rounded to a percent", ...);       // 0.6875 → value "69"
it("no control renders disabled", ...);                                     // volume null → disabled + "No volume control"; muted null → no button
it("mute sends the opposite state for the strip's key", ...);               // aria-pressed false → click → {kind:"mute", role:"output", key, on:true}
it("the drag keeps its key", ...);                                          // pointerdown, rerender with another active key, input → request carries the first key
it("ignores view echoes while dragging and shows them after release", ...);
it("shows the write error under the strip", ...);
// PanelView.test.tsx
it("renders a strip only for a role with an active row", ...);
```

- [ ] **Step 2: Run to verify they fail.** Run `pnpm test src/panel`. Expected: failures.
- [ ] **Step 3: Implement.** The strip matches the panel's native look: the CSS variables already in use, an accent-coloured track, and a pressed state for Mute. It must work in light and dark.
- [ ] **Step 4: Run the full gate.** Expected: pass.
- [ ] **Step 5: Commit:** `feat: volume and mute strips in the panel`

---

### Task 5: Docs, site, ADR 0013, QA and changeset

**Files:**
- Create: `docs/adr/0013-level-listeners.md`, `.changeset/<name>.md`
- Modify:
  - `docs/adr/README.md`, `README.md` (Using usher), `site/src/pages/usage.astro`;
  - `site/scripts/screens/mock.js` and the panel screenshots it feeds;
  - `docs/qa.md`, `docs/architecture/overview.md`, `docs/superpowers/plans/2026-11-23-usher-roadmap.md`.

**Content pinned here:**
- **ADR 0013 Level listeners** (Accepted). It amends ADR 0006 and records:
  - the five §6.2 listener rules;
  - Decisions 1–4 (the probe result for `'vmvc'`, the trait signatures, per-role gates, a re-watch after every reconcile that does nothing for the same ID, and refusals only in the reply);
  - the exception to "no `AudioObjectID` outlives one call".
- **`usage.astro`:** a toc entry `volume` (`Volume and mute`) after the panel section. The section covers:
  - the strips;
  - live sync with the volume keys and Control Center;
  - `No volume control`;
  - that volume never changes which device is active;
  - the mic dot and its tooltip.

  `README.md` "Using usher" says the same in brief.
- **`mock.js`:** `view.levels = { output: { volume: 0.69, muted: false }, input: { volume: 0.45, muted: true } }` and `levelErrors` both null. Regenerate every panel shot (`site/scripts/screens/capture.mjs`, per its README) and check each by eye.
- **QA rows 44–49:**

  | Row | Action | Expected |
  |---|---|---|
  | 44 | Press the volume keys with the panel open. | The output slider follows. |
  | 45 | Drag the output slider. | The volume in Control Center follows, and the final value sticks. |
  | 46 | Mute and unmute the output and the input from the panel. | Control Center agrees. |
  | 47 | Mute the input. | The menu-bar dot appears, and the tooltip reads `usher: microphone muted`. Unmuting in System Settings removes it. |
  | 48 | Use a device with no volume control (HDMI display or a fixed-volume DAC). | A disabled slider with `No volume control`. |
  | 49 | Unplug the active headset while dragging, then run `sudo killall coreaudiod` and press the volume keys. | The new device's volume is untouched; after the restart the slider still follows the keys. |
- **`overview.md`:** the engine node gains `levels`; an `AudioEvent::LevelChanged` edge.
- **Roadmap:** a v2 table row `M7 Volume and mute · 2026-11-25-usher-m7-volume.md`.
- **Changeset (`minor`):** `Added volume and mute controls for the active output and input to the panel, and a dot on the menu-bar icon while the microphone is muted.`

- [ ] **Step 1: Write the docs, the mock and the ADR.**
- [ ] **Step 2: Regenerate the screenshots** and check each by eye.
- [ ] **Step 3: Verify:** the quality gate table, including `pnpm docs:build` and `pnpm build:app`.
- [ ] **Step 4: Commit:** `docs: volume and mute usage, ADR 0013, QA rows and changeset`
- [ ] **Step 5: Open the PR.**
  - Title: `feat: volume and mute from the menu bar (M7)`, with `## Summary` and `## Test plan`.
  - Run the adversarial review on Opus (`unsafe` CoreAudio and an engine invariant).
  - The attack list enumerates every path that can write a level or move a listener:
    - each `Event::Level` gate (`NotReady`, `Rebinding`, `Stale` including a failed live read);
    - `LevelChanged` at each engine state (no scan, rebind pending, USB deferral pending, reconcile pending);
    - `watch_levels` on launch, `DefaultChanged`, `DevicesChanged`, a same-UID replug, rebind, shutdown and a vanished device;
    - every `unsafe` block's `SAFETY` claim;
    - the pump ordering;
    - the drag key;
    - the guard list.

---

## Self-review notes

- **Spec coverage:**
  - §6.1 strip, slider, mute, no control, live follow, no reconcile: Tasks 2 and 4.
  - §6.1 mic dot with four images: Task 3.
  - §6.2 properties, trait, listeners, `LevelChanged`: Task 1 and Decisions 1–3. The ADR 0006 amendment: Task 5.
  - §6.3 `Event::Level`, gating, `Stale` by a live read, failed write, silent refusal, immediate echo, `PanelView.levels`, 50 ms throttle: Tasks 2 and 4, Decisions 4–7.
  - §9 ADR 0013 and §11 0.3.0 `minor`: Task 5.
  - §10 M7 engine tests: Task 2.
- **Departures from the spec text:** `levels` takes the key, `default_key` is added, and the re-watch runs after every reconcile pass (Decisions 2 and 3). ADR 0013 records all three.
- **Not built:**
  - per-row sliders, shortcuts and remembered volumes (v2 non-goals);
  - per-channel mute;
  - balance;
  - a percent label (not in the spec).
