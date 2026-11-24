# 0007. Panel window, login item and command replies

- Status: Accepted
- Date: 2026-11-23

## Context

M4 adds the panel under the menu-bar icon, the Settings window and Open at login. Three choices
depart from or extend the spec. The spec §7.3 reads the login state from `SMAppService`. Spec §6
says the engine replies to a command with a full `PanelState` snapshot. The spec does not say how
the panel window floats above other apps. `unsafe` is allowed only in `usher-coreaudio`.

## Options

**Login item**

1. **`SMAppService.mainApp`.** The approval state is readable. It needs objc2 `unsafe` outside
   `usher-coreaudio`, or a new crate. Its behaviour with an ad-hoc signature is untested (audit §5,
   pitfall 4).
2. **hop's LaunchAgent through `tauri-plugin-autostart` 2.7.0.** No `unsafe`. Proven in hop. It has
   no approval API.

**Panel window**

1. **A plain Tauri window** with `alwaysOnTop` and `visibleOnAllWorkspaces`. Focus goes back with
   `app.hide()`.
2. **tauri-nspanel.** A new dependency.
3. **hop's objc2 status level.** It needs `unsafe` in `src-tauri`.

**Command replies**

1. **A reply channel on `Event::Command`.** Views keep flowing by event.
2. **The reply carries the full snapshot.** This is the spec §6 wording. It duplicates the
   `state-changed` path and gives two sources of truth.

## Decision

**Login item.** Option 2, which departs from spec §7.3. It is off by default. At startup an
enabled item whose app path moved is rewritten. The checkbox shows the OS state, read again when
the Settings window gains focus. A static hint says where to allow the item, because the approval
state is not available.

**Panel window.** Option 1. The window is declared in `tauri.conf.json` with label `panel`. A blur
hides it only after a focus arrived since the show. A tray click within 300 ms of a blur-hide does
nothing, because that click caused the blur. Hide gives focus back with `app.hide()`, unless the
Settings window is visible. tauri-nspanel and the objc2 level are rejected for the reasons above.

**Command replies.** Option 1. `Event::Command(cmd, Option<Reply>)` and `EngineHandle::request`
with a 2 s timeout. The reply carries only success or a `Refusal` with its copy. The view still
arrives by `state-changed` and is cached in `LastView`. Spec §6 said the reply carries the
snapshot. It does not. This keeps one path for views.

**Related decisions from the M4 plan.**
- Decision 9, gating. Before the first scan every command is `NotReady`. While a rebind is pending
  only `Forget` is refused (`Rebinding`), because only it reads the snapshot.
- Decision 12, error lines. When auto-switch is off the engine clears both error lines, so they
  cannot go stale.
- Decision 13, tray re-assert. A command calls `auto_switch_changed` only when the value changed.
  After a failed send the tray restores the last engine-reported value from `LastAuto` and logs
  `engine is gone; auto-switch unchanged`.
- Decision 22, frontend tests. Pure logic runs in the node environment. Component tests use
  happy-dom with `createRoot` and `act`, with no testing-library. dnd-kit drags run against a
  stubbed row layout.

## Implementation notes

These rulings changed behaviour relative to the plan.

- `Command` JSON rejects unknown fields (`deny_unknown_fields`).
- The panel screen is chosen from the cursor position in primary-scale points, as in hop. The
  tray-rect pixels are ambiguous on mixed DPI, so they cannot pick the monitor alone.
- `run_command` and `set_debug` are async. They run `EngineHandle::request` on `spawn_blocking`,
  so the main thread never waits for the engine.
- The row menu prevents default on mousedown, because WKWebView does not focus clicked buttons.
  The menu sits in normal flow, so the window grows to fit it.
- The `…` button follows the row's roving tabindex. Each list has one tab stop.
- A debug build refuses to turn Open at login on or off. This protects the installed app's
  LaunchAgent, which shares the name.
- `reveal_log` reaps the `open` child in a detached thread, so no zombie remains.

## Consequences

- No new `unsafe` and no new dependency for the window or the login item.
- The panel may not show over a full-screen app's Space (`docs/qa.md` row 20). Upgrade path: move
  the window level call into a crate allowed `unsafe`, with a new ADR. Owner check on 2026-11-24 (0.1.2): the panel shows over full-screen apps.
- Settings cannot show an "awaiting approval" state. The hint is static.
- Mixed-DPI placement and the row menu with a real mouse are proven only by hand (`docs/qa.md`
  rows 12 and 23 to 25).
