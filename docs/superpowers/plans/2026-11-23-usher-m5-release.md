# usher M5: updates and release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** usher checks for signed updates (30 s after launch, then daily, or on request), shows a pending update in the panel footer, the tray menu and Settings, and installs it only from a What's new window. The release pipeline from hop (changesets → version PR → tag → draft release → signed build → guarded publish) ships the first release.

**Architecture:** hop's `update.rs` and `whatsnew.rs` are ported into `src-tauri/src/update/` and `src-tauri/src/whatsnew.rs` with `tauri-plugin-updater` (minisign key in `tauri.conf.json`, feed `latest.json` on the latest GitHub Release). The daily-check setting lives in `state.json` (new `State.update_check`, default on) and is changed through the engine like `SetDebug`. The pending version reaches the panel as `PanelView.update`, filled by the shell (`shell::publish`) whenever it stores and emits a view, so the engine never knows about updates. A relaunch after install goes through `request_restart`, so the existing `RunEvent::Exit` flush saves `state.json`, and the new process waits up to 5 s for the instance lock when a fresh restart mark exists (hop's `lock_wait`). The release workflows are hop's, renamed, with one addition: `scripts/check-feed.mjs` refuses to publish a release whose `latest.json` is signed by a key the app does not trust.

**Tech Stack:** `tauri-plugin-updater 2.14.1`, `tauri-plugin-dialog 2.8.4` (hop's versions; Rust-side only, no JS package, no capability); `changesets/action v2.1.2`, `tauri-apps/tauri-action v0.5.x` (hop's pinned SHAs); React 19, Vitest 5 with happy-dom (existing).

**Spec:** `docs/superpowers/specs/2026-11-23-usher-design.md` (§2 goal 10, anti-goal "silent update installs"; §5.11 "update checks" at info; §7.1 `Update to usher 1.2.0…`; §7.3 Check for updates daily + Check now; §8 release prerequisites and workflows). Also read: `docs/adr/0002-update-mechanism.md`, `docs/superpowers/plans/2026-11-23-usher-roadmap.md` ("Carry-overs from M4"), `docs/audit/hop-reuse.md` (update.rs / whatsnew.rs, §workflows, rename list). hop source, read-only: `/Volumes/Dev/matteo/hop` (`src-tauri/src/{update,whatsnew,login,lib}.rs`, `src/whatsnew/*`, `whatsnew.html`, `scripts/release-notes.*`, `.github/workflows/{release,build-release}.yml`, `CONTRIBUTING.md` "Testing updates against a local feed" and "Releases", `src/settings/Settings.tsx` Updates section).

**Branch:** `feat/m5-release`, created from `main` after PR #4 (M4) is merged. Do not start before that merge.

## Global Constraints

- Non-test `.rs`, `.ts`, `.tsx` files ≤300 lines; functions ≤40 lines (`clippy::too_many_lines`). hop's `update.rs` is 331 lines at hop's line width: it must be split (Task 3). `src-tauri/src/panel/mod.rs` is already near the limit: its change in Task 4 must be net zero or negative.
- `#![forbid(unsafe_code)]` stays in `src-tauri`. No objc2 in `src-tauri`.
- `usher-core` stays pure (serde only).
- Quality gate: `pnpm build`, `pnpm test`, `pnpm check:size`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`; app build `pnpm build:app`.
- Logging goes through `log` (`info!`/`warn!`/`error!`), never `eprintln!` (hop uses `eprintln!`; replace every one).
- Every `hop` string in ported code becomes `usher`. After each porting task, `grep -rnw hop <ported files>` prints nothing.
- Feed: `https://github.com/cef62/usher/releases/latest/download/latest.json`, `requireSignedVersion: true`, no `dangerousInsecureTransportProtocol`. Key files on the owner's Mac: `~/.tauri/usher.key`, `~/.tauri/usher.key.pub`, `~/.tauri/usher.password`.
- No silent install: nothing downloads or installs without the What's new window's `Install and restart` click (ADR 0002).
- Updates are off in debug builds (`cfg!(debug_assertions)`): no feed request, `check` returns `Updates are off in development builds.`
- Copy (verbatim): panel footer and tray item `Update to usher <v>…`; tray idle item `Check for Updates…`; Settings rows `Check for updates daily`, `Check now` / `Checking…`; results `usher <v> is up to date.`, `usher <v> is available.`, `Could not check for updates: <e>`; errors `An update check or install is already running.`, `No update to install.`, `usher found a newer version since this window opened.`; window title `What's new in usher`.
- Log lines (spec §5.11): `info!("daily update check {on|off}")`, `info!("update check: usher {v} available")`, `info!("update check: up to date")`, `warn!("update check failed: {e}")`, `info!("installing usher {v}")`, `error!("update install failed: {e}")`.
- Window labels: `panel`, `settings`, `whatsnew`. Each window invokes only its own list (`commands::allowed`).
- Tests never touch `~/Library/Application Support/com.cef62.usher/`, `~/Library/LaunchAgents/` or `~/.tauri/`: Rust file tests use `tempfile`, manual update tests use a separate identifier (Task 8, CONTRIBUTING).
- Every user-visible change ships a changeset (one, `minor`, Task 8).

## Owner prerequisites

Only the owner can do these. They hold secrets or change repository settings. An agent never reads, prints or moves `~/.tauri/usher.key` or `~/.tauri/usher.password`.

The repository `cef62/usher` already exists (public, `origin` points at it). Nothing to do there.

**P1. Generate the updater key pair** (once; never regenerate after the first release, or installed copies stop accepting updates).

```bash
mkdir -p ~/.tauri && chmod 700 ~/.tauri
pnpm tauri signer generate -w ~/.tauri/usher.key
# type a password when asked, then store the same password:
printf '%s' 'THE-PASSWORD' > ~/.tauri/usher.password && chmod 600 ~/.tauri/usher.key ~/.tauri/usher.password
```

Back up `usher.key` and the password in a password manager. Losing the key means no installed usher can ever update again.

**P2. Add the repository secrets.**

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY -R cef62/usher < ~/.tauri/usher.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD -R cef62/usher < ~/.tauri/usher.password
gh secret list -R cef62/usher   # both names listed
```

**P3. Hand over the public key.** It is public and safe to paste in the chat or the PR.

```bash
cat ~/.tauri/usher.key.pub   # one base64 line: the value of plugins.updater.pubkey (Task 9)
```

**P4. Repository settings.**

- Actions may open the version PR: Settings › Actions › General › Workflow permissions › tick "Allow GitHub Actions to create and approve pull requests". Or:
  `gh api -X PUT repos/cef62/usher/actions/permissions/workflow -f default_workflow_permissions=read -F can_approve_pull_request_reviews=true`
- Release immutability on, as in hop: Settings › General › Releases › "Enable release immutability". Confirm it in the web UI.

**P5. Merge decisions.** Say "merge" for the M5 PR, then for the `chore: version packages` PR (Task 9). The version PR merge tags the release and starts the build.

**Blocked on the owner:** Tasks 1–8 need nothing from the owner. Task 9 needs P1–P4 done (P3's value in hand) and P5's word. Until P3, `tauri.conf.json` holds the placeholder pubkey `REPLACE-WITH-~/.tauri/usher.key.pub` (spec §8), and `check-feed.mjs` keeps any release built with it a draft.

## Review Focus

1. **The relaunch after an install.** The old process still holds `instance.lock` while it exits. Expected: the new usher waits up to 5 s for the lock and starts, `state.json` holds the last change made before the install, and a plain second launch (no fresh mark) still exits at once. Pinned in Task 2 (`only_a_fresh_restart_mark_waits_and_it_is_consumed`, `a_held_lock_is_taken_once_released_within_the_wait`) and Task 3 (`install_restarts_through_the_exit_path`, a source check), plus QA row 29.
2. **A newer version appears while What's new is open.** Expected: `Install and restart` is refused with `usher found a newer version since this window opened.`, the window reloads and shows the newer notes; nothing installs unseen. Pinned in Task 3 (`install_refuses_a_stale_or_missing_update`) and Task 5 (`a stale refusal reloads the window`).
3. **The engine never published a view** (dead engine, or the first scan is slow). Expected: the daily check still runs (an update may be the fix), with the default `on`. Pinned in Task 3 (`daily_check_follows_the_last_view_and_defaults_on`).
4. **The panel hides while What's new or Settings is open.** Expected: `app.hide()` is skipped, so the other window stays on screen and keeps focus. Pinned in Task 4 (`panel_keeps_the_app_shown_for_settings_and_whatsnew`).
5. **A release built with the wrong key, the placeholder key, or without the arm64 entry.** Expected: `publish` fails, the release stays a draft, no installed usher sees it. Pinned in Task 7 (`check-feed` tests).

## Decisions the spec leaves open

1. **Where `Check for updates daily` lives.** `State.update_check: bool` in `state.json`, default `true` (spec §5.7 lists the file's fields; this adds one, ADR 0008). Changed by `Command::SetUpdateCheck(bool)` through the engine, like `SetDebug`. Exposed as `PanelState.update_check` (`updateCheck`). Old files without the key read `true`; an older usher keeps the key through `State.extra`.
2. **Who reads the setting.** The checker thread reads `LastView`: `daily_check_on(view: Option<&PanelView>) -> bool` returns `view.is_none_or(|v| v.panel.update_check)`.
3. **Pending version in the panel.** `PanelView` gains `pub update: Option<String>` (`update`). The engine always sets `None`. `shell::publish(app, view)` fills it with `update::pending(app)`, stores it in `LastView` and emits `state-changed`, all under the `LastView` lock, so a republish never emits an older view after a newer one. `update::check` calls `shell::republish(app)` after it stores a result.
4. **Tray item.** hop's: a `MenuItem` with id `updates` between `Settings…` and the separator. Text `Check for Updates…`, or `Update to usher <v>…` when one is pending. Click: pending → What's new; else a check whose result shows in a dialog (`tauri-plugin-dialog`, Rust only) titled `usher`; a found update opens What's new.
5. **Relaunch.** `install` ends with `app.request_restart()` (Tauri 2.12: emits `ExitRequested`/`Exit`, then restarts), so `lib.rs`'s `RunEvent::Exit` flush runs unchanged. hop's "wait while the palette is shown" loop is dropped: the panel has no text input.
6. **Restart mark.** Ported from hop's `login.rs` into `instance.rs`: `restart-update` file in the app data dir, 10 min TTL; `hold_instance_lock(dir, wait)` retries every 100 ms up to `wait`.
7. **What's new window.** Built at runtime, label `whatsnew`, `whatsnew.html`, 520×560, min 420×360, not minimizable or maximizable, centred; `on_navigation` allows only the bundled page (hop's `own_page`), `on_new_window` denies. Capability `whatsnew` grants `log:default` only (the page forwards its console like the others; hop granted nothing). Progress arrives over a `Channel<Phase>`.
8. **IPC lists.** `panel`: M4 list + `open_whatsnew`. `settings`: M4 list + `set_update_check`, `check_update`, `open_whatsnew`. `whatsnew`: `whatsnew_info`, `whatsnew_install`, `whatsnew_close`. Only `open_whatsnew` is shared.
9. **Local app build.** `pnpm build:app` stays unsigned: it passes `--config '{"bundle":{"createUpdaterArtifacts":false}}'`, so agents and the quality gate need no key. Only CI (`tauri-action` with the secrets) builds signed updater artifacts. hop's `build:app` read the key from `~/.tauri`; usher's does not (ADR 0008).
10. **Release guard.** `publish` runs `node scripts/check-feed.mjs latest.json src-tauri/tauri.conf.json <version>` instead of hop's `jq` line. It checks the `darwin-aarch64` entry, the version, and that the signature's minisign key id equals the pubkey's key id (ADR 0008).
11. **First version.** Changesets bumps `0.0.0` with the pending `minor` changesets to `0.1.0`. `Cargo.toml`'s workspace version stays as is (`sync-version.mjs` only syncs `tauri.conf.json`; nothing reads `CARGO_PKG_VERSION`).

---

## File structure

| File | Responsibility | Budget |
|---|---|---|
| `crates/usher-core/src/state.rs`, `commands.rs`, `panel.rs` | `update_check`, `SetUpdateCheck`, `PanelState.update_check` | +15 |
| `src-tauri/src/engine/command.rs`, `engine/mod.rs` | log line; `PanelView.update` | +8 |
| `src-tauri/src/instance.rs` | lock wait, restart mark | 100 |
| `src-tauri/src/update/mod.rs` | `UpdateState`, `Phase`, `UpdateInfo`, `bucket`, `Since`, `due`, `daily_check_on`, `state`, `pending`, `setup`, checker, `check`, `check_update` | 220 |
| `src-tauri/src/update/install.rs` | `stale`, `install`, `TrayItem`, `tray_text`, `refresh_tray`, `tray_clicked` | 150 |
| `src-tauri/src/whatsnew.rs` | window, `own_page`, `open_whatsnew`, `whatsnew_info/install/close` | 110 |
| `src-tauri/src/shell.rs` | `publish`, `republish` | +25 |
| `src-tauri/src/settings.rs` | `SettingsView.update_check/update`, `set_update_check` | +20 |
| `src-tauri/src/commands.rs` | lists, `allowed` for `whatsnew` | +8 |
| `src-tauri/src/panel/mod.rs` | `KEEP_FOCUS`, `hide_for_window` (rename) | ±0 |
| `src-tauri/src/tray.rs` | `updates` item | +6 |
| `src-tauri/src/lib.rs` | plugins, setup order, handlers, config test | +15 |
| `src-tauri/Cargo.toml`, `tauri.conf.json`, `capabilities/whatsnew.json` | deps, updater config, capability | |
| `whatsnew.html`, `vite.config.ts` | third page | |
| `src/whatsnew/{api,notes,WhatsNew,main}.ts(x)`, `whatsnew.css` + tests | ported from hop | ≤130 each |
| `src/types.ts`, `src/api.ts` | `View.update`, `SettingsView.updateCheck/update`, new calls | +8 |
| `src/panel/PanelView.tsx`, `Panel.tsx` | footer `Update to usher X…` | +6 |
| `src/settings/Settings.tsx` (+ `Updates.tsx` if it passes 150) | Updates section | +45 |
| `scripts/release-notes.mjs` + test, `scripts/check-feed.mjs` + test | release notes, publish guard | 40 / 70 |
| `.github/workflows/release.yml`, `build-release.yml`, `ci.yml` | release pipeline | |
| `package.json` | `release:tag`, `build:app` | |
| `docs/adr/0008-updater-settings-and-release-guard.md`, `docs/adr/README.md`, `CONTRIBUTING.md`, `README.md`, `docs/qa.md`, `docs/architecture/overview.md`, roadmap, `.changeset/*.md` | docs | |

---

### Task 1: The daily-check setting (core and engine)

**Files:**
- Modify: `crates/usher-core/src/state.rs`, `crates/usher-core/src/commands.rs`, `crates/usher-core/src/panel.rs`, `src-tauri/src/engine/command.rs`
- Test: `crates/usher-core/tests/commands.rs`, inline in `state.rs`, `src-tauri/tests/requests.rs`

**Interfaces:**
- Produces:
  - `State.update_check: bool` (default `true` in `impl Default for State`, before `extra`).
  - `Command::SetUpdateCheck(bool)`; JSON `{"kind":"setUpdateCheck","args":false}`. `apply` returns `Ok(changed)`.
  - `PanelState.update_check: bool` (serialised `updateCheck`), set in `panel_state`.
  - Engine logs `info!("daily update check {}", word(on))` when it changed.

- [ ] **Step 1: Write the failing tests**

```rust
// state.rs
#[test]
fn a_file_without_update_check_reads_on() {
    let s: State = serde_json::from_str(r#"{"schema_version":1,"auto_switch":true}"#).unwrap();
    assert!(s.update_check);
    let off = State { update_check: false, ..State::default() };
    let back: State = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.update_check);
}
// tests/commands.rs, added to panel_json_parses_into_commands
assert_eq!(p(r#"{"kind":"setUpdateCheck","args":false}"#).unwrap(), Command::SetUpdateCheck(false));
#[test] fn set_update_check_reports_a_change_once() {
    let mut s = State::default();
    assert_eq!(apply(&mut s, &snap, Command::SetUpdateCheck(false)), Ok(true));
    assert_eq!(apply(&mut s, &snap, Command::SetUpdateCheck(false)), Ok(false));
    assert!(!panel_state(&s, &snap).update_check);
}
// src-tauri/tests/requests.rs
#[test] fn update_check_off_is_saved_and_in_the_view() {
    assert_eq!(ask(&mut e, Command::SetUpdateCheck(false), 10), Ok(()));
    assert!(!e.view().panel.update_check);
    // after the debounce, state_file() contains "\"update_check\": false" (or false at that key)
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p usher-core && cargo test -p usher --test requests`
Expected: compile errors (`update_check`, `SetUpdateCheck` unknown).

- [ ] **Step 3: Add the field, variant and log line** following `SetDebug` everywhere it appears (`grep -rn SetDebug crates src-tauri`).

- [ ] **Step 4: Run the tests and clippy**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 5: Commit** `feat(core): persisted daily update check setting`

---

### Task 2: Instance lock wait and restart mark

**Files:**
- Modify: `src-tauri/src/instance.rs`, `src-tauri/src/lib.rs` (`setup`: read the mark, pass the wait, consume the mark)

**Interfaces:**
- Produces (ported from hop `login.rs`, logs via `log`):
  - `pub fn hold_instance_lock(dir: &Path, wait: Duration) -> bool` — retries `try_lock` every 100 ms until `wait` has passed.
  - `pub fn lock_wait(fresh_mark: bool) -> Duration` — 5 s or zero.
  - `pub fn mark_restart(dir: &Path)`, `pub fn clear_restart_mark(dir: &Path)`, `pub fn restart_mark_fresh(dir: &Path) -> bool` (TTL 600 s), `pub fn take_restart_mark(dir: &Path) -> bool`.
- `setup`: `hold_instance_lock(dir, lock_wait(restart_mark_fresh(dir)))`, then `take_restart_mark(dir)` once the lock is held.

- [ ] **Step 1: Write the failing tests** (tempdir only)

```rust
#[test] fn only_a_fresh_restart_mark_waits_and_it_is_consumed() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(lock_wait(restart_mark_fresh(d.path())), Duration::ZERO);
    mark_restart(d.path());
    assert_eq!(lock_wait(restart_mark_fresh(d.path())), Duration::from_secs(5));
    assert!(take_restart_mark(d.path()));
    assert!(!take_restart_mark(d.path()));
}
#[test] fn a_held_lock_is_taken_once_released_within_the_wait() {
    // thread A: hold_instance_lock in a child process is not possible here; instead open+lock the file directly,
    // drop it from another thread after 200 ms, and assert hold_instance_lock(dir, 2 s) is true and took >= 200 ms
}
#[test] fn without_a_wait_a_held_lock_is_refused_at_once() {
    // lock the file directly; hold_instance_lock(dir, Duration::ZERO) is false in < 50 ms
}
```

Keep M4's two tests, passing `Duration::ZERO`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p usher instance`
Expected: compile errors (`lock_wait`, arity of `hold_instance_lock`).

- [ ] **Step 3: Implement** the functions and the `setup` change.

- [ ] **Step 4: Run** `cargo test -p usher instance && cargo clippy -p usher --all-targets -- -D warnings`. Expected: pass.

- [ ] **Step 5: Commit** `feat: relaunch after an update waits for the instance lock`

---

### Task 3: Updater backend, config and pending version in the view

**Files:**
- Create: `src-tauri/src/update/mod.rs`, `src-tauri/src/update/install.rs`
- Modify: `src-tauri/Cargo.toml` (`tauri-plugin-updater = "2.14.1"`, `tauri-plugin-dialog = "2.8.4"`), `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`, `src-tauri/src/shell.rs`, `src-tauri/src/engine/mod.rs` (`PanelView.update`), `src-tauri/src/tray.rs`

**Interfaces:**
- Consumes: Task 1 `PanelState.update_check`; Task 2 `instance::{mark_restart, clear_restart_mark}`.
- Produces (hop's names unless stated; `hop` → `usher`, `eprintln!` → `log`):
  - `update/mod.rs`: `FIRST_CHECK_DELAY` (30 s), `CHECK_INTERVAL` (24 h), `WAKE_POLL` (600 s), `READ_TIMEOUT` (60 s); `DEBUG_BUILD`, `BUSY`, `NO_UPDATE`, `NEWER` (copy above); `UpdateState`, `Phase`, `UpdateInfo`, `Since`; `fn bucket`, `pub fn due(enabled: bool, last: Option<Since>) -> bool`; new `pub fn daily_check_on(view: Option<&PanelView>) -> bool`; `pub(crate) fn state`, `pub(crate) fn set_phase`, `pub fn pending(app) -> Option<String>`, `pub fn setup(app)`, `pub async fn check(app) -> Result<Option<UpdateInfo>, String>` (calls `shell::republish` and `install::refresh_tray` after storing), `#[tauri::command] pub async fn check_update`.
  - `update/install.rs`: new `pub fn stale(available: Option<&str>, expected: &str) -> Option<&'static str>` (`NO_UPDATE` when none, `NEWER` when it differs); `pub async fn install(app, expected: &str) -> Result<(), String>` (uses `stale`; ends with `app.request_restart()`); `pub struct TrayItem(pub MenuItem<Wry>)`; `pub fn tray_text(pending: Option<&str>) -> String`; `pub fn refresh_tray(app)`; `pub fn tray_clicked(app)`.
  - `engine::PanelView.update: Option<String>` (engine sets `None`).
  - `shell::publish(app: &AppHandle, view: PanelView)` and `shell::republish(app: &AppHandle)` (Decision 3). `TauriShell::state_changed` calls `publish`.
  - `tauri.conf.json`: `bundle.createUpdaterArtifacts: true`; `plugins.updater = { "pubkey": "REPLACE-WITH-~/.tauri/usher.key.pub", "endpoints": ["https://github.com/cef62/usher/releases/latest/download/latest.json"], "requireSignedVersion": true }`.
  - `lib.rs`: `.plugin(tauri_plugin_dialog::init())`, `.plugin(tauri_plugin_updater::Builder::new().build())`; `update::setup(app.handle())` before `tray::build`; `update::check_update` in the handler.
  - `tray.rs`: item `updates` with `tray_text(None)` after `Settings…`; managed as `update::TrayItem`; `"updates" => update::tray_clicked(app)`.

- [ ] **Step 1: Write the failing tests** (inline in `update/`; port hop's `checks_first_then_daily_and_only_while_on`, `a_second_check_or_install_is_refused_while_one_runs`, `phase_wire_format`, `progress_buckets` unchanged), plus:

```rust
#[test] fn daily_check_follows_the_last_view_and_defaults_on() {
    assert!(daily_check_on(None), "no view yet: an update may be the fix");
    let mut v = view_fixture(); // PanelView with update_check true
    assert!(daily_check_on(Some(&v)));
    v.panel.update_check = false;
    assert!(!daily_check_on(Some(&v)));
}
#[test] fn install_refuses_a_stale_or_missing_update() {
    assert_eq!(stale(None, "0.2.0"), Some(NO_UPDATE));
    assert_eq!(stale(Some("0.3.0"), "0.2.0"), Some(NEWER));
    assert_eq!(stale(Some("0.2.0"), "0.2.0"), None);
}
#[test] fn tray_item_names_the_pending_version() {
    assert_eq!(tray_text(None), "Check for Updates…");
    assert_eq!(tray_text(Some("0.2.0")), "Update to usher 0.2.0…");
}
#[test] fn install_restarts_through_the_exit_path() {
    // request_restart runs RunEvent::Exit, which flushes state.json; restart() from the main thread would skip it
    let src = include_str!("install.rs");
    assert!(src.contains("request_restart()") && !src.contains(".restart();"));
}
#[test] fn updater_config_is_the_signed_github_feed() {
    // hop's test with the usher endpoint; also bundle.createUpdaterArtifacts == true
}
```

`lib.rs`: delete `assert!(c["plugins"]["updater"].is_null(), "updater is M5");` (the feed test above replaces it). `engine` view tests: `e.view().update == None`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p usher update`
Expected: compile errors (module missing).

- [ ] **Step 3: Port the code** from hop `src-tauri/src/update.rs` into the two files, apply the Interfaces changes, wire `lib.rs`, `shell.rs`, `tray.rs`, and add the config. `whatsnew` does not exist yet: `tray_clicked` calls `crate::whatsnew::open` in Task 4, so leave a `// Task 4` stub that logs and returns.

- [ ] **Step 4: Run the gate for Rust and the app build**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check && pnpm check:size`
Expected: pass. `pnpm build:app` is fixed in Task 7; until then `cargo build --release -p usher` must pass.

- [ ] **Step 5: Commit** `feat: update checks with tauri-plugin-updater`

---

### Task 4: What's new window (Rust) and window guards

**Files:**
- Create: `src-tauri/src/whatsnew.rs`, `src-tauri/capabilities/whatsnew.json`
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/panel/mod.rs`, `src-tauri/src/settings.rs` (call the renamed hide), `src-tauri/src/update/install.rs` (drop the stub), `src-tauri/src/lib.rs` (handlers)

**Interfaces:**
- Consumes: Task 3 `update::{state, install, Phase, UpdateInfo}`.
- Produces:
  - `whatsnew::LABEL = "whatsnew"`, `pub fn open(app)`, `fn own_page(u: &Url, dev: Option<&Url>) -> bool`, commands `open_whatsnew`, `whatsnew_info(app, on_phase: Channel<Phase>) -> WhatsNew`, `whatsnew_install(app, version: String) -> Result<(), String>`, `whatsnew_close(app)` (hop's, renamed).
  - `commands::WHATSNEW_COMMANDS = ["whatsnew_info", "whatsnew_install", "whatsnew_close"]`; `PANEL_COMMANDS` + `open_whatsnew`; `SETTINGS_COMMANDS` + `set_update_check`, `check_update`, `open_whatsnew` (`set_update_check` lands in Task 6; list it now).
  - `panel::KEEP_FOCUS: [&str; 2] = [settings::LABEL, whatsnew::LABEL]`; `settings_visible` becomes `other_window_visible` over `KEEP_FOCUS`; `hide_for_settings` is renamed `hide_for_window` and used by both windows.
  - `capabilities/whatsnew.json`: `windows: ["whatsnew"]`, `permissions: ["log:default"]`.

- [ ] **Step 1: Write the failing tests**

```rust
// whatsnew.rs: hop's only_the_bundled_page_loads, unchanged list of bad URLs
#[test] fn capability_grants_only_the_log() {
    // windows == ["whatsnew"], permissions == ["log:default"]
}
// commands.rs, replacing each_window_calls_only_its_own_commands
#[test] fn each_window_calls_only_its_own_commands() {
    let lists = [("panel", PANEL_COMMANDS), ("settings", SETTINGS_COMMANDS), ("whatsnew", WHATSNEW_COMMANDS)];
    for (label, list) in lists { for c in list {
        assert!(allowed(label, c));
        for (other, _) in lists { if other != label {
            assert_eq!(allowed(other, c), *c == "open_whatsnew" && other != "whatsnew", "{other} {c}");
        }}
    }}
    assert!(!allowed("whatsnew", "get_view") && !allowed("panel", "whatsnew_install") && !allowed("", "quit"));
}
// panel/mod.rs
#[test] fn panel_keeps_the_app_shown_for_settings_and_whatsnew() {
    assert_eq!(KEEP_FOCUS, ["settings", "whatsnew"]);
}
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -p usher` → compile errors.

- [ ] **Step 3: Implement** (port `whatsnew.rs`, rename, `log` instead of `eprintln!`; `open` calls `panel::hide_for_window` then `app.show()`).

- [ ] **Step 4: Run** `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && pnpm check:size`. Expected: pass; `panel/mod.rs` non-test lines not above M4's count.

- [ ] **Step 5: Commit** `feat: What's new window with install progress`

---

### Task 5: What's new page (frontend)

**Files:**
- Create: `whatsnew.html`, `src/whatsnew/{api.ts, notes.ts, notes.test.ts, WhatsNew.tsx, WhatsNew.test.tsx, main.tsx, whatsnew.css}` (ported from hop)
- Modify: `vite.config.ts` (input `whatsnew`)

**Interfaces:**
- Consumes: Task 4 commands; Rust `Phase` wire format (Task 3 `phase_wire_format`).
- Produces: `api.info(onPhase)`, `api.install(version)`, `api.close()`, `api.STALE = ["No update to install.", "usher found a newer version since this window opened."]`; `progressText(p, version)` with `usher`.
- `main.tsx` calls `forwardConsole(console, sink)` first (as `src/settings/main.tsx`), imports `../settings/settings.css` and `./whatsnew.css` (no `<link>` tags in `whatsnew.html`).

- [ ] **Step 1: Port the tests first** (`notes.test.ts`, `WhatsNew.test.tsx`, renamed), and add:

```ts
// WhatsNew.test.tsx (happy-dom, src/test/dom.ts helpers)
it("a stale refusal reloads the window", async () => {
  // mock invoke: whatsnew_info → {update:{version:"0.2.0",…}}, whatsnew_install rejects STALE[1], second info → 0.3.0
  // click "Install and restart"; expect heading to name 0.3.0 and a second whatsnew_info call
});
it("progress names usher", () => {
  expect(progressText({ kind: "installing" }, "0.2.0")).toBe("Installing usher 0.2.0… usher restarts when it's done.");
});
```

The Rust test `the_whatsnew_window_knows_the_stale_install_errors` (hop, in `update/mod.rs`) reads `src/whatsnew/api.ts`: add it now.

- [ ] **Step 2: Run** `pnpm test src/whatsnew` → fails (files missing).

- [ ] **Step 3: Port the files** and add the vite input. If `main.tsx` grows the `App` past 40 lines, move `App` into `WhatsNewApp.tsx`.

- [ ] **Step 4: Run** `pnpm build && pnpm test && pnpm check:size && cargo test -p usher stale_install`. Expected: pass; `grep -rnw hop src/whatsnew whatsnew.html` prints nothing.

- [ ] **Step 5: Commit** `feat(ui): What's new page`

---

### Task 6: Settings Updates section and panel footer

**Files:**
- Modify: `src-tauri/src/settings.rs`, `src-tauri/src/lib.rs` (handler), `src/types.ts`, `src/api.ts`, `src/settings/Settings.tsx` (+ `src/settings/Updates.tsx` if `Settings.tsx` passes 150 lines), `src/panel/PanelView.tsx`, `src/panel/Panel.tsx`
- Test: `src/settings/Settings.test.tsx`, `src/panel/PanelView.test.tsx`, inline in `settings.rs`

**Interfaces:**
- Consumes: Task 1 `Command::SetUpdateCheck`; Task 3 `update::pending`, `check_update`; Task 4 `open_whatsnew`.
- Produces:
  - `SettingsView` gains `update_check: bool` (from `LastView`, default `true` when `None`) and `update: Option<String>` (`update::pending`). TS: `updateCheck: boolean; update: string | null`.
  - `#[tauri::command] pub async fn set_update_check(engine, on: bool) -> Result<(), String>` via `commands::request`.
  - TS `View.update: string | null`. `panelApi.openWhatsNew()`; `settingsApi.setUpdateCheck(on)`, `settingsApi.checkNow(): Promise<{ version: string } | null>`, `settingsApi.openWhatsNew()`.
  - `Actions.openWhatsNew(): void` in `PanelView`.

- [ ] **Step 1: Write the failing tests**

```tsx
// PanelView.test.tsx
it("shows the pending update in the footer and opens What's new", () => {
  // view.update = "0.2.0": a button "Update to usher 0.2.0…"; click → actions.openWhatsNew called once
  // view.update = null: no such button
});
// Settings.test.tsx
it("turns the daily check off through the engine", () => { /* switch "Check for updates daily" unchecked → setUpdateCheck(false) */ });
it("Check now reports up to date, available or the error", async () => {
  // checkNow resolves null → "usher 0.1.0 is up to date."; button reads "Checking…" and is disabled while pending
  // resolves {version:"0.2.0"} → "usher 0.2.0 is available." and a "What's new…" button calling openWhatsNew
  // rejects "offline" → "Could not check for updates: offline"
});
```

- [ ] **Step 2: Run** `pnpm test src/panel src/settings` → fail.

- [ ] **Step 3: Implement.** The Updates section sits between General and Troubleshooting. The result line is a `role="status"` paragraph. The footer button sits before `Settings…`.

- [ ] **Step 4: Run the full gate** (all six commands). Expected: pass.

- [ ] **Step 5: Commit** `feat(ui): update rows in Settings and the panel footer`

---

### Task 7: Release scripts and workflows

**Files:**
- Create: `scripts/release-notes.mjs`, `scripts/release-notes.test.ts` (hop, renamed; HINTS names `/Applications/usher.app`), `scripts/check-feed.mjs`, `scripts/check-feed.test.ts`, `.github/workflows/release.yml`, `.github/workflows/build-release.yml`
- Modify: `package.json`, `.github/workflows/ci.yml` (comment only: "release.yml dispatches this job on the version PR")

**Interfaces:**
- Produces:
  - `releaseNotes(changelog: string, version: string): string` (hop's).
  - `keyId(box: string): string` — `box` is base64 of a minisign text file (pubkey or signature); returns the hex of bytes 2..10 of its second line's base64 payload; throws on malformed input.
  - `checkFeed(feed: object, pubkey: string, version: string): string[]` — problems, empty when fine:
    - `latest.json has no darwin-aarch64 entry`
    - `latest.json is for <feed.version>, not <version>`
    - `tauri.conf.json has no valid updater public key`
    - `the package is signed by key <sig id>, but tauri.conf.json trusts <pub id>`
  - CLI: `node scripts/check-feed.mjs <latest.json> <tauri.conf.json> <version>`; prints problems, exits 1 if any.
  - `package.json`: `"release:tag": "changeset git-tag"`, `"build:app": "tauri build --target aarch64-apple-darwin --bundles app --config '{\"bundle\":{\"createUpdaterArtifacts\":false}}'"`.
  - `release.yml`: hop's, unchanged apart from names.
  - `build-release.yml`: hop's with: title `"usher $TAG"`; `cargo test --workspace` at the repo root; `Swatinem/rust-cache` without `workspaces: src-tauri`; `publish` checks out the tag (`persist-credentials: false`), downloads `latest.json`, runs `check-feed.mjs` with `${TAG#v}`, then undrafts; no `pages.yml` dispatch. All actions pinned to hop's SHAs.

- [ ] **Step 1: Write the failing tests** (`check-feed.test.ts`; fixtures built in the test, no real keys)

```ts
const box = (comment: string, payload: Buffer) =>
  Buffer.from(`untrusted comment: ${comment}\n${payload.toString("base64")}\n`).toString("base64");
const id = (n: number) => Buffer.alloc(8, n);
const pub = (n: number) => box("minisign public key", Buffer.concat([Buffer.from("Ed"), id(n), Buffer.alloc(32)]));
const sig = (n: number) => box("signature from tauri secret key", Buffer.concat([Buffer.from("ED"), id(n), Buffer.alloc(64)]));
const feed = (s: string, v = "0.1.0") => ({ version: v, platforms: { "darwin-aarch64": { signature: s, url: "u" } } });

it("passes a feed signed by the trusted key", () => expect(checkFeed(feed(sig(1)), pub(1), "0.1.0")).toEqual([]));
it("refuses another key", () => expect(checkFeed(feed(sig(2)), pub(1), "0.1.0")).toEqual([
  "the package is signed by key 0202020202020202, but tauri.conf.json trusts 0101010101010101"]));
it("refuses the placeholder key", () => expect(checkFeed(feed(sig(1)), "REPLACE-WITH-~/.tauri/usher.key.pub", "0.1.0"))
  .toContain("tauri.conf.json has no valid updater public key"));
it("refuses a missing platform", () => expect(checkFeed({ version: "0.1.0", platforms: {} }, pub(1), "0.1.0"))
  .toEqual(["latest.json has no darwin-aarch64 entry"]));
it("refuses another version", () => expect(checkFeed(feed(sig(1), "0.1.1"), pub(1), "0.1.0"))
  .toEqual(["latest.json is for 0.1.1, not 0.1.0"]));
```

`release-notes.test.ts`: hop's, with `usher` in the expected HINTS.

- [ ] **Step 2: Run** `pnpm test scripts` → fail.

- [ ] **Step 3: Implement** the scripts (Node built-ins only), the workflows and `package.json`.

- [ ] **Step 4: Verify**

Run: `pnpm test && pnpm check:size && pnpm build:app`
Expected: tests pass; `build:app` produces `src-tauri/target/aarch64-apple-darwin/release/bundle/macos/usher.app` with no signing-key error. Also `node scripts/release-notes.mjs 0.1.0 /dev/null` prints the fallback, the `---` line and the usher hint. Check workflow syntax with `actionlint` if installed, else by reading against hop's files.

- [ ] **Step 5: Commit** `ci: release workflows with a signed-feed publish guard`

---

### Task 8: Docs, ADR, QA and changeset

**Files:**
- Create: `docs/adr/0008-updater-settings-and-release-guard.md`, `.changeset/<name>.md`
- Modify: `docs/adr/README.md` (index row), `CONTRIBUTING.md` ("Testing updates against a local feed", "Releases"), `README.md` (Install: download from Releases, `xattr -cr /Applications/usher.app` or Open Anyway; Updates), `docs/qa.md`, `docs/architecture/overview.md` (`UPD` node no longer "M5"), `docs/superpowers/plans/2026-11-23-usher-roadmap.md` (M5 row plan name; "Carry-overs from M5" if any)

**Content pinned here:**
- ADR 0008 (Accepted) records the departures: `update_check` in `state.json` (spec §5.7 field list); `build:app` unsigned locally, signed builds only in CI; `check-feed.mjs` key-id guard before publish; What's new capability grants `log:default` (hop: none); `request_restart` instead of hop's `restart` plus palette wait; tray check result via `tauri-plugin-dialog`.
- CONTRIBUTING local-feed recipe: hop's four steps with `usher`, plus `"identifier":"com.cef62.usher.updatetest"` in both `--config` overrides, so the test copy uses its own app data, log folder and lock and never touches the installed usher's `state.json`. Throwaway key under `/tmp/usher-test/`, never `~/.tauri/usher.key`. State: never toggle Open at login in the test copy (it shares the LaunchAgent name).
- CONTRIBUTING Releases: hop's section, renamed; add "the `publish` job refuses a feed signed by another key; fix the key or secret, then re-run the workflow".
- QA rows (next free numbers, 27+): 27 Check now in a release build (up to date / error offline); 28 daily check off persists across relaunch (`state.json` via the app, not by hand); 29 install from What's new relaunches usher, panel order kept, one usher in the menu bar; 30 a pending update shows in panel footer, tray item and Settings; 31 What's new Escape closes only that window; 32 first-run Gatekeeper on the downloaded release.
- Changeset (`minor`): `Added update checks (daily or on request) and a What's new window that installs a signed update when you choose to.`

- [ ] **Step 1: Write the docs.**
- [ ] **Step 2: Verify** `pnpm test` (release-notes still passes) and the quality gate table from CONTRIBUTING.
- [ ] **Step 3: Commit** `docs: updater ADR, release guide, QA rows`
- [ ] **Step 4: Open the PR** `feat: updates, What's new and the release pipeline (M5)` with `## Summary`, `## Test plan`; run the adversarial review (Opus: security-sensitive). Attack list must cover every path to an install (tray item, panel footer, Settings, What's new retry), every path that can relaunch or exit during an install (Quit from tray/panel, a second launch, a failed download), the guard lists, `own_page`, the notes renderer with hostile feed text, and every workflow step that runs with a write token.

---

### Task 9: Real public key and first release — BLOCKED on owner P1–P5

**Files:**
- Modify: `src-tauri/tauri.conf.json` (`plugins.updater.pubkey` = P3's line)

- [ ] **Step 1:** Put P3's value in `pubkey`. If the M5 PR is still open, commit it there (`chore: updater public key`); else open a small PR with that title.
- [ ] **Step 2: Verify the key locally without the private key.** Run `node -e 'import("./scripts/check-feed.mjs").then(m=>console.log(m.keyId(require("./src-tauri/tauri.conf.json").plugins.updater.pubkey)))'`. Expected: a 16-hex-digit id, no error.
- [ ] **Step 3:** After the owner merges the M5 PR, `release.yml` opens `chore: version packages` (0.1.0). Check its CI with `gh run list --branch changeset-release/main`, then report and wait for the owner's merge word.
- [ ] **Step 4:** After that merge: `gh run list --workflow build-release.yml` until done. Expected: `publish` passed `check-feed.mjs`, release `v0.1.0` is published with `usher.app.tar.gz`, `.sig` and `latest.json`.
- [ ] **Step 5:** `curl -sL https://github.com/cef62/usher/releases/latest/download/latest.json | jq .version` prints `"0.1.0"`. Hand QA rows 27–32 to the owner. Row 29 needs a newer release (see open questions).

---

## Self-review notes

- Spec coverage: §2 goal 10 (Tasks 3–7), anti-goal silent installs (Decision 4, Task 3 `stale`), §5.11 update-check logs (Global Constraints, Task 3), §7.1 footer (Task 6), §7.3 rows (Task 6), §8 prerequisites, placeholder pubkey, pinned-endpoint test, workflows last (Owner prerequisites, Tasks 3, 7, 9). M4 carry-overs: Settings rows, `View.update`, `open_whatsnew`, `whatsnew` label/capability/guard list, tray item (Tasks 3, 4, 6).
- Not built: notarization (ADR 0002 deferred), a docs site, Intel builds, a separate `build:signed` script.
