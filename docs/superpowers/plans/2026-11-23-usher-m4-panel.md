# usher M4: panel and settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** usher gets its UI. A left click on the menu-bar icon opens a tray-anchored panel (React) with the two priority lists, drag and keyboard reorder, a row menu, the auto-switch switch, the paused banner, per-role error lines and a read-only notice. A Settings window has Open at login, Debug logging and Show log in Finder. The engine replies to panel commands, and the M3 review items deferred to M4 are closed.

**Architecture:** The panel is a plain Tauri window (label `panel`) declared in `tauri.conf.json`, hidden at launch, placed under the tray icon rect on each show and hidden on blur, Escape or a second icon click. There is no NSPanel and no `unsafe` in `src-tauri`. The webview sends `Command`s (now `Deserialize` in core) through one guarded tauri command, `run_command`, which calls `EngineHandle::request`: the engine applies the command and answers on a reply channel with `Result<(), Refusal>`. Views keep flowing through the `state-changed` event; the shell caches the last `PanelView` so a fresh window can ask for it. The Settings window is built at runtime (label `settings`) and talks to `login.rs` (tauri-plugin-autostart, copied from hop) and to the engine (`SetDebug`). React components are pure views that take callbacks, so Vitest drives them in happy-dom with real events.

**Tech Stack:** tauri 2.12.7 (`tray-icon`, `image-png`, `macos-private-api`); `tauri-plugin-autostart 2.7.0` (hop's version); React 19.3.1 and react-dom 19.3.1; `@dnd-kit/core 6.3.1`, `@dnd-kit/sortable 10.0.0`, `@dnd-kit/utilities 3.2.2`; `@vitejs/plugin-react ^6.2.1`; Vitest 5 with `happy-dom ^20` (dev, DOM tests only); `@types/react ^19.3.0`, `@types/react-dom ^19.3.0`.

**Spec:** `docs/superpowers/specs/2026-11-23-usher-design.md` (§2 goals 4, 5, 8, 9, 11, 12; §5.4 paused line; §5.8 Forget; §5.11 debug toggle, badge, Show log; §6 command rule; §7.1, §7.2, §7.3; §9 frontend). Also read: `docs/superpowers/plans/2026-11-23-usher-roadmap.md` (carry-overs from M1, M2, M3), `docs/adr/0006-engine-threading-and-coreaudio-binding.md`, `docs/audit/hop-reuse.md` §1 (panel.rs, login.rs, settings.rs, lib.rs guard) and §4 (frontend), `docs/audit/app-shell-and-ui.md` §1.3-1.8 and §3 (row anatomy, dismissal). hop source, read-only: `/Volumes/Dev/matteo/hop` (`src-tauri/src/{panel,login,settings,lib}.rs`, `src/settings/Settings.tsx`, `src/settings/settings.css`).

**Branch:** `feat/m4-panel`, already created from `main` (d24b0a4).

## Global Constraints

- Non-test `.rs`, `.ts`, `.tsx` files ≤300 lines; functions ≤40 lines (`clippy::too_many_lines`). `pnpm check:size` ignores `*.test.ts(x)`, paths under `tests/`, and Rust after the first bare `#[cfg(test)]`.
- `#![forbid(unsafe_code)]` stays in `src-tauri`. No objc2 in `src-tauri`.
- `usher-core` stays pure (serde derive only).
- Quality gate: `pnpm build`, `pnpm test`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm check:size`.
- React 19 + Vite, plain CSS with variables on `:root`, `color-scheme: light dark`. Native HTML controls (`<input type="checkbox" switch>`, `<button>`). No component library, no ESLint, no testing-library.
- Panel window label is `panel` (capability `log:default` is bound to it). Never call plugin-log `attachConsole()`; both entry points call `forwardConsole` only.
- CSP stays `default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:`.
- Panel width 360 pt. Spec keys: Up/Down move focus, Return promotes, Alt+Up/Alt+Down move the row, Escape closes.
- Copy: paused line `usher paused auto-switch: <name> kept taking <role>` (`PausedReason::message`, mirrored in TS). Error lines are the engine's strings verbatim (`couldn't switch <role> to <name>: <error>`).
- Log lines (spec §5.11): `info!("auto-switch {on|off}")`, `info!("debug logging {on|off}")`, `info!("forgot {name} ({key})")`. Snapshot device lines at `debug` include the alias family.
- Every user-visible change ships a changeset (one, `minor`, Task 9).

## Review Focus

1. **A view arrives mid-drag** (a device plugs in, or the engine reorders after a promote). Expected: the drop builds its `Reorder` from the rows on screen at drop time, never from the rows at drag start. If the engine still refuses (`StaleOrder`), the panel shows `the output list changed; reload and retry` and the rows go back to the engine's order. Pinned in Task 7 (`a view that arrives mid-drag is used for the drop`, `a refused command shows its text and the rows go back to the engine's order`).
2. **The menu bar sits on a second display with another scale.** Expected: the panel opens under the icon on that display, kept 8 pt inside its edges. Pinned in Task 4 (`icon_on_a_1x_display_right_of_a_2x_primary`, `anchor_clamps_to_the_right_edge`).
3. **A click on the icon while the panel is open.** The click blurs the panel first, then arrives as a tray click. Expected: the panel closes and stays closed. Pinned in Task 4 (`tray_click_right_after_blur_hide_stays_hidden`, `tray_click_after_the_guard_shows_again`).
4. **The engine is gone or slow.** Expected: the command returns at once (gone) or within 2 s (no answer) with readable copy, the panel shows it, and the tray check mark shows the last value the engine reported, not "on". Pinned in Task 2 (`request_to_a_gone_engine_is_gone`, `request_without_answer_times_out`) and Task 7 (`a refused command shows its text …`).
5. **A hidden device is the active default, or a role has no devices.** Expected: the unranked active row is shown (goal 11) but cannot be promoted, moved, dragged or forgotten; an empty list shows a placeholder, not a blank section. Pinned in Task 7 (`the unranked active row is shown but inert`, `an empty list shows its placeholder`).

## Decisions the spec leaves open

1. **Panel window.** Declared in `tauri.conf.json`: label `panel`, `url` `index.html`, 360×400 (the frontend sets the height), `visible: false`, `transparent: true`, `decorations: false`, `resizable: false`, `skipTaskbar: true`, `alwaysOnTop: true`, `visibleOnAllWorkspaces: true`, `acceptFirstMouse: true`, `shadow: true`, `windowEffects: { effects: ["popover"], state: "active", radius: 10 }`; `app.macOSPrivateApi: true` with the tauri feature `macos-private-api`. No NSPanel and no objc2 level change, so `src-tauri` keeps `forbid(unsafe_code)`. Known limit: over a full-screen app's Space the panel may not appear (QA row 20; upgrade path in ADR 0007).
2. **Tray clicks.** `show_menu_on_left_click(false)`. Left click (button up) toggles the panel. Right click shows the menu: `Auto-switch` (check), `Settings…`, separator, `Quit usher`. The icon glyphs and tooltip from M3 are the spec §7.2 icon states; M4 does not change them.
3. **Placement.** Computed at every show from `TrayIconEvent::Click.rect`: centred under the icon, 4 pt below it (`GAP`), clamped 8 pt inside the icon's display (`MARGIN`). Physical px are converted per monitor, as hop's `place` does. The anchor is not recomputed while open.
4. **Height.** The panel's frontend measures `#root` with a `ResizeObserver` and calls `getCurrentWindow().setSize(new LogicalSize(360, fitHeight(content, screen.availHeight)))`. `fitHeight` caps at `availHeight − 48` with a floor of 120; the lists scroll inside. Needs `core:window:allow-set-size` on `panel` only.
5. **Dismissal.** hop's `Visibility` state machine: a blur hides only after a focus arrived since the show. A tray click within `REOPEN_GUARD = 300 ms` after a blur-hide is the click that caused the blur and does nothing. Escape (frontend) calls `hide_panel`. Opening Settings hides the panel first.
6. **Focus.** Show = place, `app.show()`, `w.show()`, `w.set_focus()`, then emit `panel-shown`; the frontend focuses the tabbable row of the Output list (else the Auto-switch switch). Hide = `w.hide()`, then `give_back`: `app.hide()` (macOS refocuses the previous app) unless the Settings window is visible, then leave focus alone. hop's `Activate(pid)` case is dropped (it needs objc2).
7. **Command wire format.** `usher_core::Command` derives `Deserialize` with `#[serde(tag = "kind", content = "args", rename_all = "camelCase")]`. JSON: `{"kind":"promote","args":{"role":"output","key":"AirPods-uid"}}`, `{"kind":"setAutoSwitch","args":false}`. TS mirrors it by hand in `src/types.ts`; the Rust test pins the shapes.
8. **Replies.** `Event::Command(Command, Option<Reply>)`. The engine answers `Ok(())` for `Ok(true)` and `Ok(false)`, `Err(Refusal)` otherwise. `EngineHandle::request` waits `REQUEST_TIMEOUT = 2 s`. The tray toggle keeps `Event::ToggleAutoSwitch` with no reply.
9. **Gating.** Before the first scan every command is `Refusal::NotReady`. While a rebind is pending only `Forget` is refused (`Refusal::Rebinding`): it is the only command that reads the snapshot (`is_connected`). Reorder, Promote, SetNeverUse, SetAutoSwitch and SetDebug still apply.
10. **Initial view.** `TauriShell::state_changed` stores the view in managed `LastView(Mutex<Option<PanelView>>)` before emitting. `get_view` returns it; `null` renders `Reading audio devices…`.
11. **Optimistic order.** After a drop or Alt+Arrow, `RoleList` shows the new order at once and keeps it until the next `rows` prop or a refused send. No other optimistic state.
12. **Error lines while paused.** When `auto_switch` is off (toggle or guard trip), the engine clears both error lines, at the end of every reconcile and in the command path. They cannot go stale: nothing writes while off.
13. **Tray re-assert.** `Event::Command` calls `Shell::auto_switch_changed` only when the value changed. `Event::ToggleAutoSwitch` still re-asserts every time (muda flips the check on click). After a failed send the tray restores the last engine-reported value, kept in managed `LastAuto(AtomicBool)` (starts `true`), and logs `warn!("engine is gone; auto-switch unchanged")`.
14. **Debug toggle.** `Shell::debug_changed(on)` is called when `SetDebug` changed the state; `TauriShell` calls `logging::apply(on)`. `USHER_LOG` and debug builds still win (`log_level`), so in `pnpm tauri dev` the toggle does not lower the level (QA note).
15. **Read-only store.** `PanelView.read_only` (`readOnly`) is `!store.writable()`. The panel shows `Changes are not saved: state.json is from a newer usher or could not be read.`
16. **Rebind wording.** `Engine.restarted` is set by `begin_rebind` and cleared on success. The retry line reads `rebind failed: …` after a coreaudiod restart and `audio setup failed: …` for a launch bind.
17. **Snapshot family.** `usher-coreaudio`'s per-device debug line gains `family=<Some("…")|None>` from `usher_core::identity::alias_identity`, logged after the serial-less verdict is set.
18. **Login item.** hop's `login.rs`: a LaunchAgent through `tauri-plugin-autostart`, `AUTOSTART_NAME = "com.cef62.usher"`, no launch args. Off until the user turns it on (no first-run enable, no new state field). At startup an enabled item whose path moved is rewritten (`refresh`). The checkbox shows the OS state, re-read on Settings window focus. The approval state of SMAppService is not available: when on, a hint says where to allow it. This departs from spec §7.3 and gets ADR 0007.
19. **Settings window.** Built at runtime: label `settings`, `settings.html`, title `usher Settings`, 420×300, not resizable, minimizable or maximizable, centred. Its pane has Open at login, Debug logging, Show log in Finder and Version. The update rows of §7.3 are M5. Its console is forwarded too: capability `settings` grants `log:default` only.
20. **Show log in Finder.** `open -R <app_log_dir>/usher.log` (`/usr/bin/open`, no opener plugin); `open <app_log_dir>` when the file is missing.
21. **IPC guard.** hop's pattern: no app ACL manifest, so `with_window_guard` checks a per-window allowlist. `panel`: `get_view`, `run_command`, `hide_panel`, `open_settings`, `quit`. `settings`: `get_settings`, `set_login`, `set_debug`, `reveal_log`. Anything else is rejected with `not allowed from this window`.
22. **Frontend tests.** Pure logic runs in the default node environment. Component tests start with `// @vitest-environment happy-dom` and render with `createRoot` + `act` (React 19) through a small helper, `src/test/dom.ts`. No testing-library. dnd-kit drags run against a stubbed row layout.
23. **Accessibility.**
    - Tab order: Auto-switch switch → Output list (one tab stop: roving tabindex) → that row's `…` button → Input list (one tab stop) → its `…` button → notices have no stops → `Settings…` → `Quit`.
    - Up/Down move focus inside one list and stop at the ends (no wrap, no jump between lists). Alt+Up/Down move the focused row, and focus stays on it. Return and Space promote (native `<button>`).
    - Each row button's accessible name is `rowLabel(row, count)`. A polite live region announces `Moved <name> to position <n> of <count>.`
    - The drag handle (the rank number) is `aria-hidden` and not focusable: the keyboard and the row menu are the alternative (goal 4). dnd-kit's own `attributes` are not spread; only its `listeners` are.
    - The row menu is a `role="menu"` of `role="menuitem"` buttons. It opens with focus on its first item, Up/Down move inside it, and Escape closes it and returns focus to `…` without closing the panel (`preventDefault`). Forget opens an inline confirmation with focus on `Cancel`.
    - Paused, error and read-only lines: error lines and the command error use `role="alert"`, the paused and read-only lines use `role="status"`.
24. **Unresolved USB at launch (up to 500 ms of switch-away).** Not addressed in M4. A fix would defer the launch reconcile until the USB verdicts resolve, which reverses ADR 0006 ("the launch reconcile runs at once") and touches the seed path. It only bites when the top device is a serial-less USB device that moved port while usher was off. It moves to the roadmap backlog.

---

## File structure

| File | Responsibility | Budget |
|---|---|---|
| `crates/usher-core/src/commands.rs` | `Command` derives `Deserialize` | 155 |
| `crates/usher-core/tests/commands.rs` | wire-format test | — |
| `crates/usher-coreaudio/src/scan.rs` | `device_line()` with family | 195 |
| `src-tauri/src/engine/mod.rs` | `Event` (with reply), `Reply`, `Refusal`, `Shell::debug_changed`, `PanelView.read_only`, `handle` | 250 |
| `src-tauri/src/engine/command.rs` | `on_command`: gating, apply, logs, tray/debug side effects, reply | 110 |
| `src-tauri/src/engine/exec.rs` | errors cleared while off | 95 |
| `src-tauri/src/engine/rebind.rs` | `restarted`, `retry_line()` | 95 |
| `src-tauri/src/engine/thread.rs` | `EngineHandle::request`, `REQUEST_TIMEOUT` | 90 |
| `src-tauri/src/panel/mod.rs` | `Visibility`, `Step`, `GiveBack`, show/hide glue, `hide_panel` | 200 |
| `src-tauri/src/panel/place.rs` | `Frame`, `to_points()`, `anchor()` | 80 |
| `src-tauri/src/commands.rs` | window guard, `get_view`, `run_command`, `open_settings`, `quit` | 110 |
| `src-tauri/src/settings.rs` | Settings window, `SettingsView`, `get_settings`, `set_debug`, `reveal_log` | 120 |
| `src-tauri/src/login.rs` | hop's login item, renamed | 140 |
| `src-tauri/src/shell.rs` | `LastView`, `debug_changed`, `spawn() -> bool` | 100 |
| `src-tauri/src/tray.rs` | Settings item, left click → panel, `LastAuto` | 150 |
| `src-tauri/src/lib.rs` | plugins, guard, window events, wiring | 150 |
| `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` | panel window, private API, autostart | |
| `src-tauri/capabilities/default.json`, `settings.json` | panel and settings grants | |
| `src-tauri/tests/requests.rs` | reply, gating, side-effect scenarios | — |
| `package.json`, `vite.config.ts` | deps, React plugin, two pages | |
| `index.html`, `settings.html` | entry pages | |
| `src/types.ts` | `Role`, `Row`, `Paused`, `View`, `Command`, `SettingsView` | 50 |
| `src/api.ts` | `panelApi`, `settingsApi` (tauri calls only) | 70 |
| `src/panel/logic.ts` | `moveKey`, `dragToOrder`, `stepOrder`, `rowIntent`, `fitHeight` | 80 |
| `src/panel/text.ts` | `pausedText`, `lastSeen`, `rowLabel`, `READ_ONLY`, `movedText` | 60 |
| `src/panel/PanelView.tsx` | header, notices, two lists, footer | 150 |
| `src/panel/RoleList.tsx` | dnd-kit context, roving focus, optimistic order, live region | 170 |
| `src/panel/DeviceRow.tsx` | one row, `…` menu, Forget confirmation | 170 |
| `src/panel/Panel.tsx` | container: view subscription, `send`, Escape, `panel-shown`, height fit | 100 |
| `src/panel/panel.css` | panel styles | |
| `src/main.tsx` (replaces `src/main.ts`) | panel entry: `forwardConsole`, render `<Panel/>` | 15 |
| `src/settings/Settings.tsx`, `main.tsx`, `settings.css` | Settings page | 150 / 15 |
| `src/test/dom.ts` | `mount`, `click`, `key`, `stubRowLayout`, `drag` | 80 |
| `docs/adr/0007-panel-window-and-login-item.md`, `docs/adr/README.md` | ADR and index | |
| `docs/qa.md`, `docs/architecture/overview.md`, roadmap, `README.md`, `.changeset/*.md` | docs | |

---

### Task 1: Wire formats (Command JSON, snapshot family line)

**Files:**
- Modify: `crates/usher-core/src/commands.rs` (derive), `crates/usher-coreaudio/src/scan.rs` (`log_device` → `device_line`)
- Test: `crates/usher-core/tests/commands.rs`, inline in `scan.rs`

**Interfaces:**
- Consumes: `usher_core::identity::alias_identity(&DeviceSnapshot) -> Option<PortIdentity>`.
- Produces:
  - `#[derive(Clone, PartialEq, Debug, Deserialize)] #[serde(tag = "kind", content = "args", rename_all = "camelCase")] pub enum Command` (variants unchanged).
  - `pub(crate) fn device_line(d: &DeviceSnapshot) -> String` in `scan.rs`; `scan` logs it at `debug` after `mark_serialless`.

- [ ] **Step 1: Write the failing tests**

`crates/usher-core/tests/commands.rs`:

```rust
#[test]
fn panel_json_parses_into_commands() {
    let p = |s: &str| serde_json::from_str::<Command>(s);
    assert_eq!(p(r#"{"kind":"promote","args":{"role":"output","key":"AirPods-uid"}}"#).unwrap(),
        Command::Promote { role: Role::Output, key: "AirPods-uid".into() });
    assert_eq!(p(r#"{"kind":"reorder","args":{"role":"input","order":["a","b"]}}"#).unwrap(),
        Command::Reorder { role: Role::Input, order: vec!["a".into(), "b".into()] });
    assert_eq!(p(r#"{"kind":"setNeverUse","args":{"role":"output","key":"k","on":true}}"#).unwrap(),
        Command::SetNeverUse { role: Role::Output, key: "k".into(), on: true });
    assert_eq!(p(r#"{"kind":"forget","args":{"key":"k"}}"#).unwrap(), Command::Forget { key: "k".into() });
    assert_eq!(p(r#"{"kind":"setAutoSwitch","args":false}"#).unwrap(), Command::SetAutoSwitch(false));
    assert!(p(r#"{"kind":"format","args":{}}"#).is_err());
    assert!(p(r#"{"kind":"promote","args":{"role":"speaker","key":"k"}}"#).is_err());
}
```

`scan.rs` (a display UID with a non-zero UUID, transport `DisplayPort`):

```rust
#[test]
fn device_line_includes_the_alias_family() {
    let d = DeviceSnapshot { uid: "37D8832A-2D66-02CA-B9F7-8F30A301B230".into(), transport: Transport::DisplayPort, /* name "LG", alive, 0 in, 2 out */ .. };
    let line = device_line(&d);
    assert!(line.contains("family=Some(\""), "{line}");
    let spk = /* BuiltIn "BuiltInSpeakerDevice" */;
    assert!(device_line(&spk).ends_with("family=None"));
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p usher-core --test commands panel_json && cargo test -p usher-coreaudio device_line`
Expected: compile errors: `Deserialize` not implemented for `Command`; `device_line` not found.

- [ ] **Step 3: Add the derive and `device_line`**

Keep the existing fields of the debug line and append `family={:?}` with `alias_identity(d).map(|i| i.family)`. Move the log call after the serial-less verdict so USB families show.

- [ ] **Step 4: Run the tests and clippy**

Run: `cargo test -p usher-core -p usher-coreaudio && cargo clippy -p usher-core -p usher-coreaudio --all-targets -- -D warnings`
Expected: all pass, no clippy output.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat(core): panel command JSON and alias family in the snapshot log"
```

---

### Task 2: Engine replies and gating

**Files:**
- Create: `src-tauri/src/engine/command.rs`, `src-tauri/tests/requests.rs`
- Modify: `src-tauri/src/engine/mod.rs` (move `on_command` out; `Event`, `Refusal`, `PanelView.read_only`), `src-tauri/src/engine/thread.rs`, `src-tauri/tests/common/mod.rs` (`Call::Debug`), `src-tauri/tests/engine.rs` (`Event::Command(.., None)`), `src-tauri/src/shell.rs` (empty `debug_changed` until Task 5 so the crate builds)

**Interfaces:**
- Consumes: Task 1 `Command`; `usher_core::{apply, CommandError}`; `Store::writable()`.
- Produces:
  - `pub type Reply = std::sync::mpsc::Sender<Result<(), Refusal>>;`
  - `pub enum Event { Audio(AudioEvent), Command(Command, Option<Reply>), ToggleAutoSwitch, Quit(Sender<()>) }`
  - `#[derive(Clone, PartialEq, Debug)] pub enum Refusal { Rejected(CommandError), NotReady, Rebinding, Gone, NoAnswer }` with `Display`:
    - `Rejected(e)` → `e.to_string()`
    - `NotReady` → `usher has not read the audio devices yet; try again in a moment`
    - `Rebinding` → `usher is reconnecting to the audio system; try again in a moment`
    - `Gone` → `usher's audio engine stopped; quit and reopen usher`
    - `NoAnswer` → `usher did not answer in time; try again`
  - `Shell` gains `fn debug_changed(&self, on: bool);`
  - `PanelView` gains `pub read_only: bool` (serialised `readOnly`).
  - `pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);` and `EngineHandle::request(&self, cmd: Command, timeout: Duration) -> Result<(), Refusal>`: `Gone` when the send fails or the reply sender is dropped, `NoAnswer` on timeout.
  - `pub(super) fn on_command(&mut self, cmd: Command, now: Instant) -> Result<(), Refusal>` in `command.rs`; `handle` sends its result on the `Reply` (a send error is ignored).

- [ ] **Step 1: Write the failing scenarios (`tests/requests.rs`)**

Fixtures from `tests/common` (`trio()`, `boot`, `at`, `state_file`). `ask(e, cmd, t)` is a local helper: a fresh channel, `e.handle(Event::Command(cmd, Some(tx)), at(t))`, return `rx.try_recv().unwrap()`.

```rust
#[test] fn request_replies_ok_after_apply() {
    assert_eq!(ask(&mut e, Command::SetNeverUse { role: Role::Output, key: "AirPods-uid".into(), on: true }, 10), Ok(()));
    assert!(e.state().output.never_use.contains(&"AirPods-uid".into())); }
#[test] fn a_no_op_still_replies_ok() {
    assert_eq!(ask(&mut e, Command::SetAutoSwitch(true), 10), Ok(())); }
#[test] fn request_before_first_scan_is_not_ready() {
    // fail_scan = 1
    assert_eq!(ask(&mut e, Command::SetDebug(true), 10), Err(Refusal::NotReady)); }
#[test] fn rejected_command_replies_with_the_core_error() {
    let r = ask(&mut e, Command::Forget { key: "BuiltInSpeakerDevice".into() }, 10);
    assert_eq!(r, Err(Refusal::Rejected(CommandError::StillConnected("BuiltInSpeakerDevice".into()))));
    assert_eq!(r.unwrap_err().to_string(), "device BuiltInSpeakerDevice is still connected"); }
#[test] fn forget_is_refused_while_rebinding() {
    // state: pods remembered, devices [spk, mic] (pods disconnected); fail_rebind = 1
    e.handle(Event::Audio(AudioEvent::ServiceRestarted), at(0)); e.tick(at(0));
    assert!(e.rebinding());
    assert_eq!(ask(&mut e, Command::Forget { key: "AirPods-uid".into() }, 10), Err(Refusal::Rebinding));
    assert_eq!(ask(&mut e, Command::Promote { role: Role::Output, key: "AirPods-uid".into() }, 20), Ok(())); }
#[test] fn read_only_store_shows_in_the_view() {
    // write {"schema_version":2} as state.json, boot
    assert!(e.view().read_only);
    assert_eq!(serde_json::to_value(e.view()).unwrap()["readOnly"], true); }
#[test] fn writable_store_is_not_read_only() { assert!(!e.view().read_only); }
#[test] fn request_to_a_gone_engine_is_gone() {
    let (h, rx) = EngineHandle::new(); drop(rx);
    assert_eq!(h.request(Command::SetDebug(true), Duration::from_millis(10)), Err(Refusal::Gone)); }
#[test] fn request_without_answer_times_out() {
    let (h, _rx) = EngineHandle::new();
    assert_eq!(h.request(Command::SetDebug(true), Duration::from_millis(10)), Err(Refusal::NoAnswer)); }
#[test] fn request_through_the_run_loop_gets_the_engine_answer() {
    // spawn run(engine, rx) on a thread; h.request(SetDebug(true), REQUEST_TIMEOUT) == Ok(()); drop h; join
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p usher --test requests`
Expected: compile errors: `Refusal`, `EngineHandle::request` not found; `Event::Command` takes 1 field.

- [ ] **Step 3: Implement `command.rs`, the `mod.rs` and `thread.rs` changes**

`on_command` order: no snapshot → `NotReady` (keep the M3 `warn!`); `Forget` while `rebinding()` → `Rebinding`; `apply`; `Err(e)` → `warn!("command rejected: {e}")` and `Rejected(e)`. Update `engine.rs` and `Recorder` (`Call::Debug(bool)`, pushed by `debug_changed`).

- [ ] **Step 4: Run the tests and clippy**

Run: `cargo test -p usher && cargo clippy -p usher --all-targets -- -D warnings && pnpm check:size`
Expected: all pass; `engine/mod.rs` under 300 lines.

- [ ] **Step 5: Commit**

```bash
git add src-tauri
git commit -m "feat(engine): reply to panel commands; refuse Forget while rebinding"
```

---

### Task 3: Engine side effects (debug, tray, errors, logs, wording)

**Files:**
- Modify: `src-tauri/src/engine/command.rs`, `src-tauri/src/engine/mod.rs` (`handle`), `src-tauri/src/engine/exec.rs`, `src-tauri/src/engine/rebind.rs`
- Test: `src-tauri/tests/requests.rs`, inline in `rebind.rs`

**Interfaces:**
- Consumes: Task 2 `on_command`, `Shell::debug_changed`, `Call::Debug`.
- Produces:
  - `pub(super) fn retry_line(restarted: bool, why: &str, d: Duration) -> String` in `rebind.rs`.
  - Engine field `restarted: bool` (set by `begin_rebind`, cleared after a successful attempt).

Behaviour, all in the `Ok(true)` branch of `on_command`:
- `auto_switch` changed → `info!("auto-switch {on|off}")`, `shell.auto_switch_changed(on)`; when it turned off, clear both error lines.
- `debug` changed → `info!("debug logging {on|off}")`, `shell.debug_changed(on)`.
- `Forget { key }` → `info!("forgot {name} ({key})")`. Take `name` with `name_of` before `apply` removes the record.
- `handle(Event::Command)` no longer calls `auto_switch_changed`. `ToggleAutoSwitch` still does, every time.
- `run_reconcile` clears both error lines at its end when `!state.auto_switch` (covers a guard trip after a failed write).

- [ ] **Step 1: Write the failing tests**

`tests/requests.rs`:

```rust
#[test] fn set_debug_tells_the_shell_once() {
    ask(&mut e, Command::SetDebug(true), 10); ask(&mut e, Command::SetDebug(true), 20);
    assert_eq!(rec.calls().iter().filter(|c| **c == Call::Debug(true)).count(), 1); }
#[test] fn promote_does_not_touch_the_tray() {
    let before = autos(&rec); // count of Call::Auto
    ask(&mut e, Command::Promote { role: Role::Output, key: "AirPods-uid".into() }, 10);
    assert_eq!(autos(&rec), before); }
#[test] fn panel_toggle_updates_the_tray_once() {
    let before = autos(&rec);
    ask(&mut e, Command::SetAutoSwitch(false), 10);
    assert_eq!(autos(&rec), before + 1);
    assert_eq!(rec.calls().last(), Some(&Call::Auto(false))); }
#[test] fn turning_auto_switch_off_clears_error_lines() {
    // order output [pods, spk]; fail_set[pods] → boot leaves errors.output = Some(..)
    ask(&mut e, Command::SetAutoSwitch(false), 10);
    assert_eq!(e.view().errors, PerRole::default());
    e.handle(Event::Audio(AudioEvent::DevicesChanged), at(20)); e.tick(at(170));
    assert_eq!(e.view().errors, PerRole::default()); }
#[test] fn guard_trip_clears_error_lines() {
    // guard scenario from engine.rs with fail_set[spk]: three failed writes trip the guard
    assert!(!e.state().auto_switch); assert_eq!(e.view().errors.output, None); }
```

`rebind.rs`:

```rust
#[test] fn retry_line_names_launch_or_rebind() {
    assert_eq!(retry_line(false, "subscribe failed (OSStatus -1)", Duration::from_secs(1)),
        "audio setup failed: subscribe failed (OSStatus -1); next try in 1s");
    assert_eq!(retry_line(true, "scan failed", Duration::from_secs(60)),
        "rebind failed: scan failed; next try in 60s"); }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p usher --test requests && cargo test -p usher retry_line`
Expected: `set_debug_tells_the_shell_once`, `promote_does_not_touch_the_tray` and the error tests FAIL; `retry_line` not found.

- [ ] **Step 3: Implement the behaviour list above**

- [ ] **Step 4: Run the whole crate and clippy**

Run: `cargo test -p usher && cargo clippy -p usher --all-targets -- -D warnings`
Expected: all pass, including every M3 test in `engine.rs` and `lifecycle.rs` (`toggle_before_first_scan_reasserts_the_check_mark` proves the tray toggle still re-asserts).

- [ ] **Step 5: Commit**

```bash
git add src-tauri
git commit -m "fix(engine): runtime debug level, tray only on change, no stale error lines while off"
```

---

### Task 4: Panel window (placement, visibility, tray clicks)

**Files:**
- Create: `src-tauri/src/panel/mod.rs`, `src-tauri/src/panel/place.rs`
- Modify: `src-tauri/src/tray.rs`, `src-tauri/src/lib.rs` (`mod panel;`, `panel::setup`, `on_window_event`), `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` (tauri feature `macos-private-api`), `src-tauri/capabilities/default.json`

**Interfaces:**
- Consumes: `EngineHandle`, `tray::set_auto`.
- Produces:
  - `place.rs`: `#[derive(Clone, Copy, PartialEq, Debug)] pub struct Frame { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }`; `pub const GAP: f64 = 4.0; pub const MARGIN: f64 = 8.0;`
    - `pub fn to_points(icon_px: Frame, monitors: &[(Frame, f64)]) -> Option<(Frame, Frame)>`: monitors are physical frames with their scale; returns the icon and its screen in points. The screen is the monitor whose point frame holds the icon's point centre, else the first; `None` for no monitors.
    - `pub fn anchor(icon: Frame, screen: Frame, width: f64) -> (f64, f64)`: `x = clamp(icon.x + icon.w/2 − width/2, screen.x + MARGIN, screen.x + screen.w − width − MARGIN)`, `y = icon.y + icon.h + GAP`.
  - `mod.rs`:
    - `pub const LABEL: &str = "panel"; pub const WIDTH: f64 = 360.0; pub const REOPEN_GUARD: Duration = Duration::from_millis(300);`
    - `#[derive(Debug, PartialEq)] pub enum Step { Show, Hide, Stay }`
    - `#[derive(Default)] pub struct Visibility { shown: bool, focused: bool, blur_hidden_at: Option<Instant> }` with `pub fn tray_click(&mut self, now: Instant) -> Step`, `pub fn hide(&mut self) -> Step`, `pub fn focus(&mut self, focused: bool, now: Instant) -> Step`.
    - `#[derive(Debug, PartialEq)] pub enum GiveBack { HideApp, Stay }` and `pub fn give_back(settings_visible: bool) -> GiveBack`.
    - Glue (main thread): `pub fn setup(app: &AppHandle)` (manages `Mutex<Visibility>`), `pub fn tray_click(app: &AppHandle, icon: tauri::Rect)`, `pub fn focus_changed(app: &AppHandle, focused: bool)`, `pub fn hide(app: &AppHandle)`, `#[tauri::command] pub fn hide_panel(app: AppHandle)`. A Show emits `panel-shown` to `panel`.
  - `tray.rs`: menu `auto` / `settings` ("Settings…") / separator / `quit`; `pub struct LastAuto(pub AtomicBool)` managed in `build` (starts `true`), written by `set_auto`; left click → `panel::tray_click`; the `settings` item exists from this task; Task 5 adds its match arm (`crate::settings::open`).
  - `default.json` permissions: `log:default`, `core:event:allow-listen`, `core:event:allow-unlisten`, `core:window:allow-set-size`. Description: `Panel window: forwards its console to the log, listens for state-changed and panel-shown, sets its own height. Custom commands are guarded in lib.rs.`

- [ ] **Step 1: Write the failing tests**

`place.rs` (`f(x,y,w,h)` builds a `Frame`):

```rust
#[test] fn anchor_centres_under_the_icon() {
    assert_eq!(anchor(f(1000.,0.,30.,24.), f(0.,0.,1512.,982.), 360.), (835., 28.)); }
#[test] fn anchor_clamps_to_the_right_edge() {
    assert_eq!(anchor(f(1490.,0.,20.,24.), f(0.,0.,1512.,982.), 360.), (1144., 28.)); }
#[test] fn anchor_clamps_to_the_left_edge() {
    assert_eq!(anchor(f(2.,0.,20.,24.), f(0.,0.,1512.,982.), 360.).0, 8.); }
#[test] fn icon_on_a_2x_primary() {
    let (icon, screen) = to_points(f(2000.,0.,60.,48.), &[(f(0.,0.,3024.,1964.), 2.)]).unwrap();
    assert_eq!((icon, screen), (f(1000.,0.,30.,24.), f(0.,0.,1512.,982.))); }
#[test] fn icon_on_a_1x_display_right_of_a_2x_primary() {
    let ms = [(f(0.,0.,3024.,1964.), 2.), (f(1512.,0.,1920.,1080.), 1.)];
    // 3400 px is 1700 pt on the 2x primary (outside it), so only the 1x display holds it
    let (icon, screen) = to_points(f(3400.,0.,30.,24.), &ms).unwrap();
    assert_eq!(screen, f(1512.,0.,1920.,1080.)); assert_eq!(icon.x, 3400.); }
#[test] fn no_monitor_is_none_and_off_screen_falls_back_to_first() {
    assert!(to_points(f(0.,0.,1.,1.), &[]).is_none());
    assert_eq!(to_points(f(-9000.,0.,30.,24.), &[(f(0.,0.,3024.,1964.), 2.)]).unwrap().1, f(0.,0.,1512.,982.)); }
```

`mod.rs`:

```rust
#[test] fn tray_click_toggles() {
    let mut v = Visibility::default();
    assert_eq!(v.tray_click(at(0)), Step::Show); assert_eq!(v.tray_click(at(1_000)), Step::Hide); }
#[test] fn blur_after_focus_hides_and_stale_blur_does_not() {
    let mut v = Visibility::default(); v.tray_click(at(0));
    assert_eq!(v.focus(false, at(10)), Step::Stay);
    v.focus(true, at(20)); assert_eq!(v.focus(false, at(30)), Step::Hide); }
#[test] fn tray_click_right_after_blur_hide_stays_hidden() {
    let mut v = Visibility::default(); v.tray_click(at(0)); v.focus(true, at(10)); v.focus(false, at(500));
    assert_eq!(v.tray_click(at(799)), Step::Stay); }
#[test] fn tray_click_after_the_guard_shows_again() {
    // same setup: blur-hide at 500; a click at 800 is outside the 300 ms guard
    assert_eq!(v.tray_click(at(800)), Step::Show); }
#[test] fn give_back_leaves_focus_when_settings_is_open() {
    assert_eq!(give_back(false), GiveBack::HideApp); assert_eq!(give_back(true), GiveBack::Stay); }
```

`lib.rs` (extend `bundle_config_pins_identity_and_platform`, or add `panel_window_and_capabilities_are_pinned`):

```rust
let w = &c["app"]["windows"][0];
assert_eq!(w["label"], "panel"); assert_eq!(w["visible"], false); assert_eq!(w["width"], 360);
assert_eq!(c["app"]["macOSPrivateApi"], true);
assert_eq!(c["app"]["security"]["csp"], "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:");
let cap: serde_json::Value = serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
assert_eq!(cap["windows"], serde_json::json!(["panel"]));
assert_eq!(cap["permissions"], serde_json::json!(["log:default", "core:event:allow-listen", "core:event:allow-unlisten", "core:window:allow-set-size"]));
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p usher panel && cargo test -p usher bundle_config`
Expected: compile errors (`panel` module missing), then the config assertions FAIL.

- [ ] **Step 3: Implement `place.rs`, `mod.rs`, the config and the tray changes**

`tray_click` glue: `Visibility::tray_click`; for `Show`, read `app.available_monitors()` (fallback `primary_monitor`, as hop does when the list is empty), `to_points`, `anchor`, `set_position(LogicalPosition)` before `show`. `focus_changed` is wired in `lib.rs` through `.on_window_event` for the `panel` label only. Hide never runs `app.hide()` while `settings` is visible (`give_back`). In the tray's failed-send path use `LastAuto` and the `warn!` from Decision 13.

- [ ] **Step 4: Run the tests, clippy and a dev launch**

Run: `cargo test -p usher && cargo clippy -p usher --all-targets -- -D warnings`
Expected: all pass.

Run: `pnpm tauri dev`
Expected: a left click opens an empty 360 pt panel under the icon; a click elsewhere closes it; a second icon click closes it; a right click shows `Auto-switch`, `Settings…`, `Quit usher`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri Cargo.lock
git commit -m "feat(app): tray-anchored panel window with blur, Escape and icon dismissal"
```

---

### Task 5: IPC commands, Settings window and login item (Rust)

**Files:**
- Create: `src-tauri/src/commands.rs`, `src-tauri/src/settings.rs`, `src-tauri/src/login.rs`, `src-tauri/capabilities/settings.json`
- Modify: `src-tauri/src/shell.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/tray.rs` (`settings` arm), `src-tauri/Cargo.toml` (`tauri-plugin-autostart = "2.7.0"`)

**Interfaces:**
- Consumes: `EngineHandle::request`, `REQUEST_TIMEOUT`, `Refusal`, `PanelView`, `panel::{hide, LABEL}`, `logging::apply`.
- Produces:
  - `shell.rs`: `pub struct LastView(pub Mutex<Option<PanelView>>)` (managed in `setup`); `TauriShell::state_changed` stores, then emits; `debug_changed` → `logging::apply(on)`; `pub fn spawn(app, rx, notify, dir) -> bool`. `lib.rs` manages `EngineStarted` only on `true`.
  - `commands.rs`:
    - `const PANEL_COMMANDS: &[&str] = &["get_view", "run_command", "hide_panel", "open_settings", "quit"];`
    - `const SETTINGS_COMMANDS: &[&str] = &["get_settings", "set_login", "set_debug", "reveal_log"];`
    - `pub fn allowed(label: &str, command: &str) -> bool`, `pub fn with_window_guard<R: Runtime>(h: impl Fn(Invoke<R>) -> bool + Send + Sync + 'static) -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static` (rejects with `not allowed from this window`).
    - `#[tauri::command] pub fn get_view(view: State<LastView>) -> Option<PanelView>`
    - `#[tauri::command(async)] pub fn run_command(engine: State<EngineHandle>, cmd: Command) -> Result<(), String>` (`Refusal` → `to_string()`).
    - `#[tauri::command] pub fn open_settings(app: AppHandle)`, `#[tauri::command] pub fn quit(app: AppHandle)` (`info!("quit from panel")`, `app.exit(0)`).
  - `settings.rs`:
    - `pub const LABEL: &str = "settings";`
    - `#[derive(Serialize)] #[serde(rename_all = "camelCase")] pub struct SettingsView { pub login: bool, pub login_refusal: Option<String>, pub debug: bool, pub version: String }`
    - `pub fn open(app: &AppHandle)`: `panel::hide`, `app.show()`, reuse or build the window (Decision 19), `set_focus`.
    - `#[tauri::command] pub fn get_settings(app: AppHandle) -> SettingsView` (`debug` from `LastView`, false when none).
    - `#[tauri::command(async)] pub fn set_debug(engine: State<EngineHandle>, on: bool) -> Result<(), String>`
    - `#[tauri::command] pub fn reveal_log(app: AppHandle) -> Result<(), String>` and `pub fn reveal_args(log_dir: &Path, file_exists: bool) -> Vec<OsString>`.
  - `login.rs` (hop `src-tauri/src/login.rs` without the instance lock and restart mark, which usher keeps in `instance.rs`):
    - `pub const AUTOSTART_NAME: &str = "com.cef62.usher";`
    - `pub fn login_item_refusal(exe: &str, debug_build: bool) -> Option<&'static str>`, `pub fn refusal() -> Option<&'static str>`, `pub fn enabled(app: &AppHandle) -> bool`, `pub fn refresh(app: &AppHandle)` (hop's `startup` with `first_run = false`), `#[tauri::command(async)] pub fn set_login(app: AppHandle, enabled: bool) -> Result<bool, String>`.
    - Refusal copy:
      - `Open at login is not available in a development build.`
      - `This usher runs from a build folder. Open at login is only for an usher installed in Applications.`
      - `macOS is running usher from a temporary location. Move usher to Applications, open it from there, then turn Open at login on.`
      - `usher's folder name contains & < or >, which a login item cannot hold. Move usher to Applications, then turn Open at login on.`
    - Error copy: `Could not turn on Open at login: {e}` / `Could not turn off Open at login: {e}` / `… : the system kept the previous state.` Logs use `log::warn!`, not `eprintln!`.
  - `capabilities/settings.json`: `windows: ["settings"]`, `permissions: ["log:default"]`, description `Settings window: forwards its console to the log. Its custom commands are guarded in lib.rs.`
  - `lib.rs`: `.plugin(tauri_plugin_autostart::Builder::new().app_name(login::AUTOSTART_NAME).build())`, `.invoke_handler(commands::with_window_guard(tauri::generate_handler![commands::get_view, commands::run_command, panel::hide_panel, commands::open_settings, commands::quit, settings::get_settings, login::set_login, settings::set_debug, settings::reveal_log]))`, `login::refresh` at the end of `setup`. Remove the M1 comment "M4 passes the persisted Debug setting" (the engine already does).

- [ ] **Step 1: Write the failing tests (inline)**

```rust
// commands.rs
#[test] fn each_window_calls_only_its_own_commands() {
    for c in PANEL_COMMANDS { assert!(allowed("panel", c)); assert!(!allowed("settings", c), "{c}"); }
    for c in SETTINGS_COMMANDS { assert!(allowed("settings", c)); assert!(!allowed("panel", c), "{c}"); }
    assert!(!allowed("whatsnew", "get_view")); assert!(!allowed("panel", "plugin:log|log")); }
// settings.rs
#[test] fn reveal_selects_the_log_or_opens_the_folder() {
    let d = Path::new("/Users/u/Library/Logs/com.cef62.usher");
    assert_eq!(reveal_args(d, true), vec![OsString::from("-R"), d.join("usher.log").into()]);
    assert_eq!(reveal_args(d, false), vec![OsString::from(d)]); }
// login.rs (ported from hop, with usher paths)
#[test] fn refuses_dev_builds_translocated_and_unwritable_paths() { /* the four copies above, and None for /Applications/usher.app/Contents/MacOS/usher */ }
#[test] fn refreshes_only_an_enabled_item_whose_path_moved() { /* hop's should_refresh table */ }
#[test] fn autostart_name_is_the_bundle_identifier() { /* == tauri.conf.json identifier */ }
// lib.rs
#[test] fn settings_capability_grants_log_only() {
    // capabilities/settings.json: windows ["settings"], permissions ["log:default"] }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p usher`
Expected: compile errors: modules `commands`, `settings`, `login` not found.

- [ ] **Step 3: Implement `shell.rs`, `commands.rs`, `settings.rs`, `login.rs` and the wiring**

- [ ] **Step 4: Run the tests and clippy**

Run: `cargo test -p usher && cargo clippy -p usher --all-targets -- -D warnings && pnpm check:size`
Expected: all pass; `lib.rs`, `tray.rs` and `shell.rs` within budget.

- [ ] **Step 5: Commit**

```bash
git add src-tauri Cargo.lock
git commit -m "feat(app): guarded panel and settings commands, Settings window, login item"
```

---

### Task 6: Frontend scaffold and pure panel logic

**Files:**
- Create: `src/types.ts`, `src/api.ts`, `src/panel/logic.ts`, `src/panel/logic.test.ts`, `src/panel/text.ts`, `src/panel/text.test.ts`, `src/test/dom.ts`, `settings.html`
- Modify: `package.json`, `vite.config.ts` (React plugin; `rollupOptions.input` `main: index.html`, `settings: settings.html`), `src/main.ts` → `src/main.tsx` (still only `forwardConsole` until Task 7), `index.html` (`/src/main.tsx`)

**Interfaces:**
- Produces:
  - `src/types.ts`:
    ```ts
    export type Role = "output" | "input";
    export type Row = { key: string; name: string; rank: number | null; connected: boolean; active: boolean; neverUse: boolean; lastSeen: number | null; canForget: boolean };
    export type Paused = { role: Role; key: string; name: string };
    export type View = { autoSwitch: boolean; debug: boolean; paused: Paused | null; output: Row[]; input: Row[]; errors: { output: string | null; input: string | null }; readOnly: boolean };
    export type Command =
      | { kind: "reorder"; args: { role: Role; order: string[] } }
      | { kind: "promote"; args: { role: Role; key: string } }
      | { kind: "setNeverUse"; args: { role: Role; key: string; on: boolean } }
      | { kind: "forget"; args: { key: string } }
      | { kind: "setAutoSwitch"; args: boolean };
    export type SettingsView = { login: boolean; loginRefusal: string | null; debug: boolean; version: string };
    ```
  - `src/api.ts` (thin, untested): `panelApi = { getView(): Promise<View | null>; onView(cb: (v: View) => void): Promise<() => void>; onShown(cb: () => void): Promise<() => void>; run(cmd: Command): Promise<void>; hide(): Promise<void>; openSettings(): Promise<void>; quit(): Promise<void>; fit(contentHeight: number): Promise<void> }` (events `state-changed`, `panel-shown`; commands as named in Task 5; `run` rejects with the Rust string). `settingsApi = { get(): Promise<SettingsView>; setLogin(on: boolean): Promise<boolean>; setDebug(on: boolean): Promise<void>; revealLog(): Promise<void> }`.
  - `src/panel/logic.ts`:
    - `moveKey(keys: string[], from: number, to: number): string[]`
    - `dragToOrder(keys: string[], active: string, over: string | null): string[] | null`
    - `stepOrder(keys: string[], key: string, delta: -1 | 1): string[] | null`
    - `type Intent = "prev" | "next" | "moveUp" | "moveDown"`; `rowIntent(e: { key: string; altKey: boolean; metaKey: boolean; ctrlKey: boolean; shiftKey: boolean }): Intent | null`
    - `fitHeight(content: number, avail: number): number`
  - `src/panel/text.ts`:
    - `pausedText(p: Paused): string`
    - `lastSeen(ts: number | null, now: number): string`
    - `rowLabel(row: Row, count: number, now: number): string`
    - `movedText(name: string, position: number, count: number): string`
    - `export const READ_ONLY = "Changes are not saved: state.json is from a newer usher or could not be read.";`
  - `src/test/dom.ts` (sets `globalThis.IS_REACT_ACT_ENVIRONMENT = true`):
    - `mount(ui: ReactNode): { root: HTMLElement; rerender(ui: ReactNode): void; unmount(): void }`
    - `click(el: Element): void`, `key(el: Element, key: string, mods?: { altKey?: boolean; metaKey?: boolean; shiftKey?: boolean }): void` (both inside `act`)
    - `stubRowLayout(rowHeight: number): () => void`: stubs `getBoundingClientRect` so an element with `data-index="i"` sits at `top = i * rowHeight`, width 360; returns the restore function.
    - `drag(handle: Element, dy: number): Promise<void>`: `pointerdown` (button 0, `isPrimary`) at the handle centre, `pointermove` by 5 px (past the 4 px activation), `pointermove` by `dy`, `pointerup`, each in `act`, then flushes one animation frame.

- [ ] **Step 1: Install the dependencies**

Run: `pnpm add react@19.3.1 react-dom@19.3.1 @dnd-kit/core@6.3.1 @dnd-kit/sortable@10.0.0 @dnd-kit/utilities@3.2.2 && pnpm add -D @vitejs/plugin-react@^6.2.1 @types/react@^19.3.0 @types/react-dom@^19.3.0 happy-dom@^20`
Expected: `package.json` lists them; `pnpm build` still passes.

- [ ] **Step 2: Write the failing tests**

`logic.test.ts`:

```ts
test("moveKey moves one key and keeps the rest in order", () => {
  expect(moveKey(["a", "b", "c"], 0, 2)).toEqual(["b", "c", "a"]);
  expect(moveKey(["a", "b", "c"], 2, 0)).toEqual(["c", "a", "b"]);
});
test("dragToOrder ignores a drop on itself or outside", () => {
  expect(dragToOrder(["a", "b", "c"], "a", "a")).toBeNull();
  expect(dragToOrder(["a", "b", "c"], "a", null)).toBeNull();
  expect(dragToOrder(["a", "b", "c"], "a", "c")).toEqual(["b", "c", "a"]);
  expect(dragToOrder(["a", "b"], "zz", "a")).toBeNull();
});
test("stepOrder stays inside the list", () => {
  expect(stepOrder(["a", "b", "c"], "a", -1)).toBeNull();
  expect(stepOrder(["a", "b", "c"], "a", 1)).toEqual(["b", "a", "c"]);
  expect(stepOrder(["a", "b", "c"], "c", 1)).toBeNull();
});
test("rowIntent maps the spec keys and nothing else", () => {
  const k = (key: string, m = {}) => rowIntent({ key, altKey: false, metaKey: false, ctrlKey: false, shiftKey: false, ...m });
  expect([k("ArrowUp"), k("ArrowDown"), k("ArrowUp", { altKey: true }), k("ArrowDown", { altKey: true })])
    .toEqual(["prev", "next", "moveUp", "moveDown"]);
  expect([k("ArrowUp", { metaKey: true }), k("ArrowDown", { shiftKey: true }), k("Enter"), k("Escape")])
    .toEqual([null, null, null, null]);
});
test("fitHeight caps at the screen and keeps a floor", () => {
  expect(fitHeight(400, 900)).toBe(400);
  expect(fitHeight(2000, 900)).toBe(852);
  expect(fitHeight(50, 900)).toBe(120);
});
```

`text.test.ts` (`H = 3_600_000`, `now = 10 * 24 * H`):

```ts
test("paused text is the M2 copy", () => {
  expect(pausedText({ role: "output", key: "AirPods-uid", name: "AirPods" }))
    .toBe("usher paused auto-switch: AirPods kept taking output");
});
test("last seen rounds down to min, h or d", () => {
  expect(lastSeen(now - 30_000, now)).toBe("last seen just now");
  expect(lastSeen(now - 5 * 60_000, now)).toBe("last seen 5 min");
  expect(lastSeen(now - 3 * H, now)).toBe("last seen 3 h");
  expect(lastSeen(now - 50 * H, now)).toBe("last seen 2 d");
  expect(lastSeen(null, now)).toBe("");
});
test("row labels read rank, state and flags", () => {
  expect(rowLabel(row({ name: "MacBook Pro Speakers", rank: 1, active: true }), 4, now)).toBe("MacBook Pro Speakers, 1 of 4, active");
  expect(rowLabel(row({ name: "AirPods", rank: 3, connected: false, lastSeen: now - 50 * H }), 4, now)).toBe("AirPods, 3 of 4, disconnected, last seen 2 d");
  expect(rowLabel(row({ name: "BlackHole 2ch", rank: 4, neverUse: true }), 4, now)).toBe("BlackHole 2ch, 4 of 4, never use");
  expect(rowLabel(row({ name: "Hidden", rank: null, active: true }), 4, now)).toBe("Hidden, not ranked, active");
  expect(movedText("AirPods", 2, 4)).toBe("Moved AirPods to position 2 of 4.");
});
```

`row(partial)` is a local fixture with `connected: true`, `active: false`, `neverUse: false`, `lastSeen: null`, `canForget: false`.

- [ ] **Step 3: Run them to verify they fail**

Run: `pnpm test`
Expected: FAIL, modules `./logic` and `./text` not found.

- [ ] **Step 4: Implement `types.ts`, `api.ts`, `logic.ts`, `text.ts`, `dom.ts`, `settings.html` and the Vite config**

`lastSeen` uses floor division and `<1 min` → `just now`. `rowLabel` order: name, rank (`n of count` or `not ranked`), `active`, `disconnected` plus last seen, `never use`; joined with `, `.

- [ ] **Step 5: Run the tests, the build and the size check**

Run: `pnpm test && pnpm build && pnpm check:size`
Expected: all pass; `dist/` holds `index.html` and `settings.html`.

- [ ] **Step 6: Commit**

```bash
git add package.json pnpm-lock.yaml vite.config.ts index.html settings.html src
git commit -m "feat(ui): React scaffold, panel types, order, key and copy logic"
```

---

### Task 7: Panel UI (lists, drag, keyboard, menu, notices)

**Files:**
- Create: `src/panel/PanelView.tsx`, `src/panel/RoleList.tsx`, `src/panel/DeviceRow.tsx`, `src/panel/Panel.tsx`, `src/panel/panel.css`, `src/panel/PanelView.test.tsx`, `src/panel/RoleList.test.tsx`
- Modify: `src/main.tsx` (render `<Panel/>` into `#root` after `forwardConsole`)

**Interfaces:**
- Consumes: Task 6 types, `panelApi`, `logic.ts`, `text.ts`, `dom.ts`.
- Produces:
  - `type Actions = { send(cmd: Command): Promise<boolean>; hide(): void; openSettings(): void; quit(): void }` (exported from `PanelView.tsx`; `send` resolves `false` after a refusal).
  - `PanelView(p: { view: View | null; error: string | null; now: number; actions: Actions }): JSX.Element`
  - `RoleList(p: { role: Role; rows: Row[]; now: number; send: Actions["send"] }): JSX.Element`. The `<ol>` carries `data-role`; each `<li>` carries `data-index` and `data-key`; the row button has class `row-main`.
  - `DeviceRow` is internal to `RoleList.tsx`'s task (props are the implementer's choice).
  - `Panel(): JSX.Element`: holds `view` and `error`; `send` = `panelApi.run(cmd)` → `setError(null)`, `true`; on rejection `setError(String(e))`, `false`. A window `keydown` Escape calls `hide()` unless `defaultPrevented`. `onShown` focuses `[data-role="output"] .row-main[tabindex="0"]`, else the Auto-switch switch. A `ResizeObserver` on `#root` calls `panelApi.fit`. A new view clears `error`.

Rendering (copy exact):
- Header: `<label>` `Auto-switch` with `<input type="checkbox" switch role="switch">`, caption `Keeps the top connected device active.` Change → `setAutoSwitch`.
- Notices under the header: paused (`pausedText`, `role="status"`), read-only (`READ_ONLY`, `role="status"`).
- Sections `Output`, `Input` (`<h2>`), each a `RoleList`. Empty list: `No output devices yet.` / `No input devices yet.`
- Row: rank number (drag handle, `aria-hidden`, shown as a grip on hover), dot (`●` connected, `○` not), name (`title` = full name, ellipsis), badges `active`, `never use` (name struck through), `lastSeen`. Disconnected rows are dimmed. Row button click → `promote` (ranked rows only; the unranked row's button has `aria-disabled="true"` and sends nothing).
- `…` button: `aria-label="Actions for <name>"`, `aria-haspopup="menu"`, `aria-expanded`. Items: `Move up`, `Move down` (`aria-disabled` at the ends), `Never use` or `Use` (`setNeverUse`), `Forget…` (only when `canForget`). The unranked row has no `…` button.
- Forget confirmation, inline under the row: `Forget <name>? usher removes it from both lists.` with buttons `Forget` (sends `forget`) and `Cancel` (focused on open).
- Error lines under the lists: `view.errors.output`, then `.input`, each `<p role="alert" class="error">` with the engine's text only (the `⚠` glyph comes from CSS `::before`). The command error (`error` prop) uses the same element.
- Footer: `Debug` badge when `view.debug`; buttons `Settings…` and `Quit`.
- `view === null`: `Reading audio devices…` plus the footer.

- [ ] **Step 1: Write the failing component tests**

Both files start with `// @vitest-environment happy-dom`. `v(partial)` builds a `View` with output `[spk (1, active), studio (2), pods (3, disconnected, canForget, lastSeen 2 d ago)]`, input `[mic (1, active)]`, all flags off. `rec()` returns `Actions` whose `send` pushes to `sent` and resolves `true`.

`PanelView.test.tsx`:

```tsx
test("paused banner shows the M2 copy only while paused", () => {
  const { root, rerender } = mount(<PanelView view={v({ paused: { role: "output", key: "AirPods-uid", name: "AirPods" } })} error={null} now={now} actions={a} />);
  expect(root.querySelector('[role="status"]')?.textContent).toBe("usher paused auto-switch: AirPods kept taking output");
  rerender(<PanelView view={v({ autoSwitch: true, paused: null })} error={null} now={now} actions={a} />);
  expect(root.textContent).not.toContain("usher paused auto-switch");
});
test("error lines show per role with the engine's text", () => {
  const { root } = mount(<PanelView view={v({ errors: { output: "couldn't switch output to AirPods: set default failed (OSStatus -50)", input: null } })} … />);
  const alerts = [...root.querySelectorAll('[role="alert"]')].map((e) => e.textContent);
  expect(alerts).toEqual(["couldn't switch output to AirPods: set default failed (OSStatus -50)"]);
});
test("the read-only notice appears only for a read-only store", () => {
  expect(mount(<PanelView view={v({ readOnly: true })} … />).root.textContent).toContain(READ_ONLY);
  expect(mount(<PanelView view={v({})} … />).root.textContent).not.toContain(READ_ONLY);
});
test("the Debug badge shows only in debug mode", () => { /* "Debug" in footer iff view.debug */ });
test("clicking a row promotes it and the switch sends setAutoSwitch", () => {
  click(rowButton(root, "AirPods"));
  click(root.querySelector('input[role="switch"]')!);
  expect(sent).toEqual([{ kind: "promote", args: { role: "output", key: "AirPods-uid" } }, { kind: "setAutoSwitch", args: true }]);
});
test("the row menu moves, flags and forgets only disconnected rows after a confirmation", () => {
  click(menuButton(root, "Studio Display")); click(menuItem(root, "Move down"));
  expect(sent.at(-1)).toEqual({ kind: "reorder", args: { role: "output", order: ["BuiltInSpeakerDevice", "AirPods-uid", "StudioDisplay-uid"] } });
  click(menuButton(root, "Studio Display")); expect(menuItem(root, "Forget…")).toBeNull();
  click(menuItem(root, "Never use")); expect(sent.at(-1)).toEqual({ kind: "setNeverUse", args: { role: "output", key: "StudioDisplay-uid", on: true } });
  click(menuButton(root, "AirPods")); click(menuItem(root, "Forget…"));
  expect(root.textContent).toContain("Forget AirPods? usher removes it from both lists.");
  expect(document.activeElement?.textContent).toBe("Cancel");
  const before = sent.length; click(button(root, "Cancel")); expect(sent.length).toBe(before);
  click(menuButton(root, "AirPods")); click(menuItem(root, "Forget…")); click(button(root, "Forget"));
  expect(sent.at(-1)).toEqual({ kind: "forget", args: { key: "AirPods-uid" } });
});
test("Escape in an open menu closes the menu and is marked handled", () => {
  click(menuButton(root, "AirPods"));
  const ev = new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true });
  act(() => document.activeElement!.dispatchEvent(ev));
  expect(ev.defaultPrevented).toBe(true); expect(root.querySelector('[role="menu"]')).toBeNull();
  expect(document.activeElement).toBe(menuButton(root, "AirPods"));
});
test("the unranked active row is shown but inert", () => {
  // output += { key: "Hidden-uid", name: "Hidden", rank: null, active: true }
  expect(rowButton(root, "Hidden").getAttribute("aria-label")).toBe("Hidden, not ranked, active");
  click(rowButton(root, "Hidden")); expect(sent).toEqual([]);
  expect(menuButton(root, "Hidden")).toBeNull();
});
test("an empty list shows its placeholder", () => {
  expect(mount(<PanelView view={v({ input: [] })} … />).root.textContent).toContain("No input devices yet.");
});
test("no view yet reads the devices message and keeps Quit", () => { /* "Reading audio devices…", button "Quit" present */ });
```

`RoleList.test.tsx` (rows `a`, `b`, `c` with names `A`, `B`, `C`; `restore = stubRowLayout(40)` in `beforeEach`):

```tsx
test("ArrowDown and ArrowUp move focus between rows without wrapping", () => {
  rowButton(root, "A").focus();
  key(document.activeElement!, "ArrowDown"); expect(document.activeElement).toBe(rowButton(root, "B"));
  key(document.activeElement!, "ArrowUp"); key(document.activeElement!, "ArrowUp");
  expect(document.activeElement).toBe(rowButton(root, "A"));
});
test("one row per list is tabbable and it follows focus", () => {
  expect(tabbable(root)).toEqual(["A"]);
  rowButton(root, "A").focus(); key(document.activeElement!, "ArrowDown");
  expect(tabbable(root)).toEqual(["B"]);
});
test("Alt+ArrowDown moves the focused row, keeps focus on it and announces it", () => {
  rowButton(root, "A").focus(); key(document.activeElement!, "ArrowDown", { altKey: true });
  expect(sent).toEqual([{ kind: "reorder", args: { role: "output", order: ["b", "a", "c"] } }]);
  expect(names(root)).toEqual(["B", "A", "C"]); // optimistic
  expect(document.activeElement).toBe(rowButton(root, "A"));
  expect(root.querySelector('[aria-live="polite"]')?.textContent).toBe("Moved A to position 2 of 3.");
});
test("Alt+ArrowUp on the first row sends nothing", () => { /* sent stays [] */ });
test("dragging the first row onto the third sends a Reorder", async () => {
  await drag(handle(root, "A"), 80);
  expect(sent).toEqual([{ kind: "reorder", args: { role: "output", order: ["b", "c", "a"] } }]);
});
test("a press on the handle without movement is not a reorder", async () => {
  // pointerdown then pointerup on the handle, no pointermove: below the 4 px activation distance
  expect(sent).toEqual([]);
});
test("a view that arrives mid-drag is used for the drop", async () => {
  // pointerdown + activation move on A, then rerender with rows [a, b, n, c], then move by 80 and pointerup
  expect(sent.at(-1)!.args).toEqual({ role: "output", order: ["b", "n", "a", "c"] });
});
test("a refused command shows its text and the rows go back to the engine's order", async () => {
  // render <PanelView> through a tiny harness whose send resolves false and sets error "the output list changed; reload and retry"
  // Alt+ArrowDown on A
  expect(alertText(root)).toBe("the output list changed; reload and retry");
  expect(names(root)).toEqual(["A", "B", "C"]);
});
```

`rowButton`, `menuButton`, `menuItem`, `button`, `handle`, `names`, `tabbable` and `alertText` are local query helpers in each test file.

- [ ] **Step 2: Run them to verify they fail**

Run: `pnpm test src/panel`
Expected: FAIL, `./PanelView` and `./RoleList` not found.

- [ ] **Step 3: Implement `RoleList.tsx` and `DeviceRow.tsx`**

One `DndContext` per list (no cross-list drag), `PointerSensor` with `activationConstraint: { distance: 4 }`, `closestCenter`, `verticalListSortingStrategy`. `SortableContext` items are the ranked keys only. `onDragEnd` uses `dragToOrder` over the keys rendered at drop time. Optimistic order per Decision 11. Keyboard per Decision 23 and `rowIntent`. If dnd-kit cannot measure rows in happy-dom, stop and report: do not delete or weaken the drag tests.

- [ ] **Step 4: Implement `PanelView.tsx`, `Panel.tsx`, `panel.css` and `main.tsx`**

CSS variables on `:root` (`--fg`, `--fg-dim`, `--bg`, `--sep`, `--accent`, `--danger`, `--row-h: 28px`) with a dark override under `@media (prefers-color-scheme: dark)`. Background is translucent so the `popover` effect shows. A visible `:focus-visible` ring on row and menu buttons.

- [ ] **Step 5: Run the tests, build and size check**

Run: `pnpm test && pnpm build && pnpm check:size`
Expected: all pass; every `.tsx` under 300 lines.

- [ ] **Step 6: Smoke run**

Run: `USHER_LOG=debug pnpm tauri dev`
Expected:
- The panel lists both roles with the active rows marked. A drag and an Alt+Down both reorder, and the log shows `set … default` only when the top changes.
- Web Inspector (right click › Inspect) shows no `Refused to` CSP line and no React warning.
- Escape closes the panel and focus returns to the previous app.

- [ ] **Step 7: Commit**

```bash
git add src
git commit -m "feat(ui): panel with drag and keyboard reorder, row menu, notices"
```

---

### Task 8: Settings UI and the full gate

**Files:**
- Create: `src/settings/Settings.tsx`, `src/settings/Settings.test.tsx`, `src/settings/main.tsx`, `src/settings/settings.css`

**Interfaces:**
- Consumes: `settingsApi`, `SettingsView`.
- Produces: `SettingsPage(p: { initial: SettingsView; api: typeof settingsApi }): JSX.Element`. `main.tsx` calls `forwardConsole`, loads `settingsApi.get()`, renders `SettingsPage`, and reloads on window `focus` (re-reads the OS login state).

Copy (exact):
- `<h2>` `General`: checkbox `Open at login` (`switch`), disabled when `loginRefusal` is set and `login` is off. Under it the refusal note (`<p class="note">`), or, while on, `If macOS lists usher as not allowed in System Settings › General › Login Items, allow it there.`
- `<h2>` `Troubleshooting`: checkbox `Debug logging`, caption `Adds detail to the log. The panel shows a Debug badge while it is on.`; button `Show log in Finder`.
- Footer line `Version <version>`.
- A failed call shows its text in `<p role="alert" class="error">` and the control shows the value the OS or engine reports.
- Style after hop's `src/settings/settings.css` (native controls, system font, `color-scheme: light dark`).

- [ ] **Step 1: Write the failing tests (`// @vitest-environment happy-dom`)**

```tsx
test("Open at login shows the OS state and the refusal note", () => {
  const { root } = mount(<SettingsPage initial={{ login: false, loginRefusal: "Open at login is not available in a development build.", debug: false, version: "0.1.0" }} api={api} />);
  const box = checkbox(root, "Open at login");
  expect(box.checked).toBe(false); expect(box.disabled).toBe(true);
  expect(root.textContent).toContain("Open at login is not available in a development build.");
  expect(root.textContent).toContain("Version 0.1.0");
});
test("turning login on calls setLogin and shows the approval hint", async () => {
  // api.setLogin resolves true
  await act(async () => click(checkbox(root, "Open at login")));
  expect(calls).toEqual([["setLogin", true]]);
  expect(root.textContent).toContain("System Settings › General › Login Items");
});
test("a refused login change is announced and the box shows the OS state again", async () => {
  // api.setLogin rejects "Could not turn on Open at login: the system kept the previous state."
  expect(alertText(root)).toBe("Could not turn on Open at login: the system kept the previous state.");
  expect(checkbox(root, "Open at login").checked).toBe(false);
});
test("Debug logging and Show log in Finder call the engine and Finder", async () => {
  await act(async () => click(checkbox(root, "Debug logging")));
  await act(async () => click(button(root, "Show log in Finder")));
  expect(calls).toEqual([["setDebug", true], ["revealLog"]]);
  expect(checkbox(root, "Debug logging").checked).toBe(true);
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `pnpm test src/settings`
Expected: FAIL, `./Settings` not found.

- [ ] **Step 3: Implement `Settings.tsx`, `main.tsx` and `settings.css`**

- [ ] **Step 4: Run the full quality gate**

Run each in turn: `pnpm build && pnpm test && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check && pnpm check:size`
Expected: every command exits 0.

Run: `pnpm build:app`
Expected: `usher.app` builds for `aarch64-apple-darwin`.

- [ ] **Step 5: Smoke run of the built app**

Open the built `usher.app` (quit any running usher first). Expected:
- The right-click menu `Settings…` and the panel footer `Settings…` both open one Settings window (the panel hides first).
- `Debug logging` on: `usher.log` shows `debug logging on` then `log level DEBUG` without a restart; the panel shows `Debug`.
- `Show log in Finder` selects `usher.log` in `~/Library/Logs/com.cef62.usher`.
- From `/Applications`, `Open at login` on creates `~/Library/LaunchAgents/com.cef62.usher.plist`; off removes it.

- [ ] **Step 6: Commit**

```bash
git add src
git commit -m "feat(ui): Settings window with Open at login, Debug logging and Show log"
```

---

### Task 9: Docs (ADR 0007, QA, architecture, roadmap, README, changeset)

**Files:**
- Create: `docs/adr/0007-panel-window-and-login-item.md`, `.changeset/<name>.md`
- Modify: `docs/adr/README.md` (index row), `docs/qa.md`, `docs/architecture/overview.md`, `docs/superpowers/plans/2026-11-23-usher-roadmap.md`, `README.md`

- [ ] **Step 1: Write ADR 0007** (template from `docs/adr/README.md`, Status Accepted, Date 2026-11-23)

- **Login item (departs from spec §7.3).** Options: `SMAppService.mainApp` (approval state readable; needs objc2 `unsafe` outside `usher-coreaudio`, or a new crate, and its behaviour with an ad-hoc signature is untested, audit §5 pitfall 4) versus hop's LaunchAgent through `tauri-plugin-autostart` 2.7.0 (no `unsafe`, proven in hop, no approval API). Decision: LaunchAgent, off by default, refreshed when the app moved, state re-read on Settings focus, and a static hint instead of the approval state.
- **Panel window.** A plain Tauri window with `alwaysOnTop` and `visibleOnAllWorkspaces`, focus given back with `app.hide()`. Rejected: tauri-nspanel (new dependency) and hop's objc2 status level (needs `unsafe` in `src-tauri`). Consequence: the panel may not show over a full-screen app's Space. Upgrade path: move the window level call into a crate allowed `unsafe`, with a new ADR.
- **Command replies.** `Event::Command(cmd, Option<Reply>)`, `EngineHandle::request` with a 2 s timeout, `Refusal` copy; views still flow by event, cached in `LastView`. Spec §6 says the engine "replies with a full `PanelState` snapshot": the snapshot arrives by `state-changed`, the reply carries only success or the refusal.
- Also record Decisions 9, 12, 13 and 22.

- [ ] **Step 2: Add QA rows to `docs/qa.md`**

| # | Action | Expected result |
|---|---|---|
| 12 | Left-click the icon on the built-in display, then on an external display with another scale. | The panel opens under the icon on that display, inside its edges. |
| 13 | Click outside the panel; reopen; press Escape; reopen; click the icon. | Each closes it. The icon click does not reopen it. Focus returns to the app that was frontmost. |
| 14 | Keyboard only: Tab, Down, Alt+Down, Return. | Focus order is switch, Output row, its `…`, Input row, its `…`, Settings…, Quit. Alt+Down moves the row and keeps focus on it. Return promotes. |
| 15 | Drag the third Output row to the top. | The order changes at once and persists after a relaunch. The output switches to that device. |
| 16 | Forget a disconnected device from its `…` menu. | A confirmation appears first. After Forget the row is gone from both lists and the log shows `forgot <name> (<uid>)`. |
| 17 | Trip the loop guard (QA 6). | The panel shows `usher paused auto-switch: …` until the switch is turned back on. No error line remains. |
| 18 | Newer schema (QA 9), then open the panel. | The panel shows `Changes are not saved: state.json is from a newer usher or could not be read.` |
| 19 | VoiceOver on: move through the Output list. | Each row reads like `AirPods, 3 of 4, disconnected, last seen 2 d`. A move announces `Moved … to position …`. |
| 20 | Open the panel while a full-screen app is frontmost. | Known limit (ADR 0007): the panel may not appear over the full-screen Space. Record the result. |
| 21 | Settings: Debug logging on, then off. | The log shows `debug logging on`, `log level DEBUG`, then `debug logging off`, `log level INFO`, with no restart. The panel badge follows. |
| 22 | Settings: Open at login on (installed app), log out and in. | usher starts and its icon appears. System Settings › Login Items lists it. |

- [ ] **Step 3: Update the architecture overview, the roadmap, the README and add the changeset**

`overview.md`: in the components diagram split `SHELL` into `Tray`, `Panel window`, `Settings + Login`, keep `Updater + WhatsNew (M5)`; add a `Panel command` sequence: Panel → `run_command` → `EngineHandle::request` → Engine `apply` → reply → `state-changed` → Panel.

Roadmap:
- M4 Plan cell: `2026-11-23-usher-m4-panel.md`.
- "Carry-overs from M4 (for the M5 plan)":
  - Settings gains `Check for updates daily` and `Check now` (spec §7.3) in `src/settings/Settings.tsx`; `SettingsView` gains the update fields; `SETTINGS_COMMANDS` gains `check_update`.
  - The panel footer gains `Update to usher X…` (spec §7.1) from a new `View.update` field; `PANEL_COMMANDS` gains `open_whatsnew`.
  - The What's new window gets label `whatsnew`, its own capability and guard list.
  - The right-click menu is where hop's updater tray item goes.
- "Backlog (post-v1)": unresolved USB at launch can switch away for up to 500 ms (Decision 24); the panel over full-screen Spaces (ADR 0007).

`README.md`: one paragraph on the panel (left click), the right-click menu, and Settings.

Changeset, `minor`: `Added a panel under the menu-bar icon to reorder devices by drag or keyboard, with a Settings window for Open at login, Debug logging and Show log in Finder.`

- [ ] **Step 4: Run the size check and commit**

Run: `pnpm check:size`
Expected: exit 0.

```bash
git add docs README.md .changeset
git commit -m "docs: ADR 0007, M4 QA rows, architecture and roadmap updates"
```

---

## Self-review

1. **Spec coverage:**

| Spec item | Task |
|---|---|
| Goal 4 drag + Move up/down | 6, 7 |
| Goal 5 toggle visible on the icon | M3 icons; 3 (tray only on change), 7 (switch) |
| Goal 8 tray-anchored panel, focus | 4, 7 |
| Goal 9 launch at login | 5, 8 (ADR 0007) |
| Goal 11 hidden active device shown | 7 (`unranked active row`) |
| §5.4 paused line until back on | 3, 7 |
| §5.8 Forget only disconnected, confirmed | 2, 7 |
| §5.11 debug toggle at runtime, badge, Show log, info lines | 3, 5, 7, 8 |
| §6 UI sends commands only | 2, 5 |
| §7.1 layout, row menu, keys, dismissal, focus give-back | 4, 7 |
| §7.2 icon states | M3 (unchanged) |
| §7.3 login, debug, show log, version (updates: M5) | 5, 8 |
| §9 Vitest ordering, drag-to-command, keyboard | 6, 7 |
| M1 carry-overs (label, attachConsole, CSP, `logging::apply`) | 4, 5, 7 |
| M2 carry-over (paused copy) | 6 |
| M3 carry-overs and deferred items | 1, 2, 3, 4, 5; USB 500 ms deferred (Decision 24) |

2. **Step scan:** every code step names its file, signature and pinned values. Bodies appear only for the anchor formula and the copy strings.
3. **Type consistency:** `Refusal`, `Reply`, `Event::Command(cmd, reply)`, `EngineHandle::request`, `LastView`, `LastAuto`, `PanelView.read_only`/`readOnly`, `Visibility::{tray_click, hide, focus}`, `Frame`, `panelApi`, `settingsApi`, `Actions.send -> Promise<boolean>` and the command names in `PANEL_COMMANDS`/`SETTINGS_COMMANDS` match across Tasks 2-8.
4. **Review Focus:** each of the 5 items names its test and owning task.
5. **Proportion:** code blocks are tests, signatures and copy.

## Execution handoff

Recommended: **Subagent-driven**. Tasks 2-5 change the engine contract and the IPC boundary that Tasks 6-8 depend on, and Task 7's drag tests may need a judgement call on dnd-kit in happy-dom; a reviewer per task catches a wrong contract before three UI tasks build on it.
