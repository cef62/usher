# usher roadmap

- Spec: `docs/superpowers/specs/2026-11-23-usher-design.md` (approved 2026-11-23)
- Reference repo: `/Volumes/Dev/matteo/hop`

There is one plan per milestone. Each milestone ends with working, tested software and is merged as
one or more PRs, following the hop workflow (feature branch, quality gate, adversarial review).

| # | Milestone | Plan | Deliverable | Depends on |
|---|---|---|---|---|
| M1 | Foundation | `2026-11-23-usher-m1-foundation.md` | The repo mirrors hop. The Cargo workspace builds. The Tauri app launches as an agent app with a tray icon and a Quit item, and writes logs, with debug level from `USHER_LOG`. CI runs build, test, clippy, fmt and the size check. | none |
| M2 | Core domain | `2026-11-23-usher-m2-core.md` | `usher-core` covers everything in spec §5 and is fully unit-tested: the model, `desired`, `reconcile` with `Reason`s, the loop guard, the new-device and first-launch seeding, the alias registry, forget, and state serde. | M1 workspace |
| M3 | Audio engine | `2026-11-23-usher-m3-engine.md` | `usher-coreaudio` and the engine thread. The JSON store is wired in. The app switches devices for real with no panel: the tray menu has the auto-switch toggle. Also coreaudiod rebind and loop-guard notifications. Usable day to day. | M1, M2 |
| M4 | Panel and settings | `2026-11-23-usher-m4-panel.md` | The tray-anchored panel (React, dnd-kit, keyboard), the Settings window (login, debug toggle, show log), and the icon states. | M3 |
| M5 | Updates and release | `2026-11-23-usher-m5-release.md` | The updater, What's new and the release workflows from hop, plus the owner prerequisites (repo, key, secrets). First release. | M4 |

## Shared conventions (all plans)

**Workspace**

The root `Cargo.toml` is a workspace with these members:

- `crates/usher-core` (lib `usher_core`);
- `crates/usher-coreaudio` (lib `usher_coreaudio`, from M3);
- `src-tauri` (package `usher`, lib `usher_lib`, bin `usher`).

**Identity and naming**

- Bundle id `com.cef62.usher`. Product name `usher`. Repo `cef62/usher`.

**Quality gate commands** (CONTRIBUTING.md, CI)

- `pnpm build`
- `pnpm test`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `pnpm check:size`

**Size limits**

- Non-test `.rs`, `.ts` and `.tsx` files: 300 lines or fewer.
- Functions: 40 lines or fewer. Enforced by `clippy.toml` `too-many-lines-threshold = 40`.

**Safety**

- `#![forbid(unsafe_code)]` in `usher-core` and in `src-tauri`.
- `unsafe` is allowed only in `usher-coreaudio`.

**Tests**

- Rust uses inline `#[cfg(test)] mod tests` for unit tests, plus `tests/` for crate-level scenarios.
- TypeScript uses Vitest, with test files next to the modules.

**ADRs**

- Any decision that departs from the spec gets a new ADR in `docs/adr/`.

## Carry-overs from M1 (for the M4 plan)

- The panel window label must be `panel`: `src-tauri/capabilities/default.json` grants `log:default` only to that label, otherwise webview log forwarding is silently denied.
- Never call plugin-log `attachConsole()` alongside `forwardConsole` (log loop).
- Re-check the CSP once the first window loads (copied from hop, untested without a window).
- Call `logging::apply(debug_setting)` when the Debug toggle changes.

## Carry-overs from M2 (for the M3 and M4 plans)

- `apply` returns `Ok(true)` when the state changed: the engine schedules a save, then runs `reconcile`.
- The engine saves the seeded state immediately after `seed`.
- After an AirPods-style guard trip, the input stays on the device that kept taking it (auto-switch is off).
- M4 builds the paused text from `PanelState.paused` (`{role, key, name}`) with the copy `usher paused auto-switch: <name> kept taking <role>`.
- Skip reasons now include `hidden` (a ranked device that macOS hides is not auto-selected).

## Carry-overs from M3 (for the M4 plan)

- The `state-changed` payload is `PanelView` (`PanelState` flattened plus `errors.{output,input}`).
- Panel commands go through `EngineHandle::send(Event::Command(..))`. M4 adds a reply channel if the panel needs the `CommandError`.
- On `SetDebug`, the engine must call `logging::apply`.
- The tray icon state is set by the engine through `Shell::auto_switch_changed`.

## Carry-overs from M4 (for the M5 plan)

- Settings gains `Check for updates daily` and `Check now` (spec §7.3) in `src/settings/Settings.tsx`. `SettingsView` gains the update fields. `SETTINGS_COMMANDS` gains `check_update`.
- The panel footer gains `Update to usher X…` (spec §7.1) from a new `View.update` field. `PANEL_COMMANDS` gains `open_whatsnew`.
- The What's new window gets label `whatsnew`, its own capability and guard list.
- The right-click menu is where hop's updater tray item goes.

## Backlog (post-v1)

- Done: unresolved USB at launch no longer switches away (ADR 0009).
- Closed: the panel shows over full-screen apps (owner check, 2026-11-24).
