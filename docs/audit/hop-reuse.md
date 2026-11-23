# hop reuse audit

Source: /Volumes/Dev/matteo/hop (read-only). Paths below are relative to it. Audited 2026-11-23.

## Summary table

| hop module | LOC / tests | verdict | what changes |
|---|---|---|---|
| src-tauri/src/lib.rs | 176 / 1 | ADAPT | Keep window guard (`allowed`, `with_window_guard`, lib.rs:15-40), Accessory policy (:144-147), tray builder (:50-78), single-instance + setup order. Replace command lists, drop palette/hotkey/history/apps/icons wiring, rename strings |
| src-tauri/src/panel.rs | 419 / 8 | ADAPT (heavily) | It is a centered palette, NOT a tray-anchored popover. Keep `Visibility` state machine (:19-61), `give_back` (:111-131), `mac::ns_window` + collection behavior/level (:254-283), `card_frame` vibrancy fit. Rewrite `place()` to anchor under the tray icon rect; drop perf marks, palette `Look` |
| src-tauri/src/login.rs | 228 / 4 | COPY AS-IS (rename consts) | LaunchAgent via tauri-plugin-autostart, NOT SMAppService. Change `AUTOSTART_NAME` (:14) to new bundle id, messages "hop" -> new name; test at :224 pins it to tauri.conf.json identifier |
| src-tauri/src/settings.rs | 466 / 9 | ADAPT | Keep load/merge/save/corrupt-aside/atomic write/unknown-key preservation, `set_settings` all-or-nothing + `SAVING` lock. Replace `Settings` fields (hotkey/folders/ignored/style) with priority lists etc.; drop `pick_path`, hotkey swap, apps reload. Window opener (:295-320) reusable |
| src-tauri/src/update.rs | 331 / 7 | COPY AS-IS (strings) | Daily checker, `due()`, busy flag, install with expected version, tray item text. Only `hop` strings (:27-30), `palette::pending` callers change |
| src-tauri/src/whatsnew.rs | 131 / 2 | COPY AS-IS (strings) | What's new window + Channel progress, `own_page` URL check. Rename label strings; pairs with src/whatsnew/* |
| src-tauri/src/hotkey.rs | 111 / 2 | NOT NEEDED (optional) | Global hotkey. Only keep if a "open panel" hotkey is wanted; also pulls tauri-plugin-global-shortcut |
| src-tauri/src/apps.rs | 365 / 7 | NOT NEEDED | App scanner + notify watcher. Pattern (background thread, emit `apps-changed`, debounce) is the template for a device-change listener |
| src-tauri/src/history.rs | 149 / 2 | NOT NEEDED | Frecency. Its debounced JSON persistence + `flush_now` on exit is a template if priority state needs persisting |
| src-tauri/src/icons.rs | 149 / 3 | NOT NEEDED | `icon://` URI scheme + NSWorkspace icons |
| src-tauri/src/launch.rs | 37 / 0 | NOT NEEDED | `open` wrapper |
| src-tauri/src/clipboard.rs | 57 / 1 | NOT NEEDED | calculator copy |
| src-tauri/src/main.rs | 6 | COPY AS-IS | calls `hop_lib::run()`; rename crate |
| src-tauri/build.rs | 3 | COPY AS-IS | `tauri_build::build()`; no app ACL manifest |
| src-tauri/Info.plist | LSUIElement=true | COPY AS-IS | |
| src-tauri/tests/perf.rs | | NOT NEEDED | |
| scripts/release-notes.mjs (+test), sync-version.mjs | 38/82, 34 | COPY AS-IS | rename only |
| .github/workflows/* | 4 files | ADAPT | see section 5 |
| .changeset/, AGENTS.md, CLAUDE.md, CONTRIBUTING.md, .claude/settings.json, rules dirs | | COPY, edit names | see section 5 |
| site/ (Astro docs, Pages) | | OPTIONAL | skip for v1 unless a site is wanted; pages.yml and the release `publish` step both reference it |

## 1. Rust modules in detail

### lib.rs (src-tauri/src/lib.rs)
- Module list :3-13. Public: `run()` (:80). Helpers: `allowed(label, command)` (:23), `with_window_guard` (:32), `quit` (:44), `tray` (:50).
- **IPC security model**: no app ACL manifest in build.rs, so Tauri capabilities do not restrict custom commands. Instead every invoke passes `with_window_guard`, which checks a per-window allowlist (`MAIN_COMMANDS`, `SETTINGS_COMMANDS`, `WHATSNEW_COMMANDS`, :17-19). Unknown command or label is rejected. Unit test (:160-175) asserts each window only calls its own commands. Reuse as-is conceptually.
- **Tray** (:50-78): `TrayIconBuilder::with_id("main")`, tooltip, `icon_as_template(true)` with `include_bytes!("../icons/tray-template.png")` (44x36 RGBA, black glyph from tray-template.svg 22x18 viewBox; macOS tints for light/dark). Menu (Open / Settings / Check updates / separator / Quit). Update state changes the tray: `update::TrayItem(MenuItem)` is managed state, `update::refresh_tray` (update.rs:229) rewrites its text. **No icon state changes** (no swapping icons); for an audio switcher showing current device you would call `TrayIcon::set_icon`/`set_title` via `app.tray_by_id("main")`. Tray here is a menu (`.menu(&menu)`), not a click-to-open panel: for a popover use `on_tray_icon_event` with `TrayIconEvent::Click { rect, .. }` (rect gives the anchor) and `show_menu_on_left_click(false)`.
- Startup order (:89-114): instance lock first, settings, panel, hotkey, history, apps, update (before tray), tray, login. `app.set_activation_policy(Accessory)` set before `run` (:144-147) because tao applies Regular in applicationDidFinishLaunching and overrides LSUIElement.
- `RunEvent::Reopen` -> show panel (:150); `RunEvent::Exit` flush (:151).

### panel.rs (src-tauri/src/panel.rs, 419 LOC; 8 tests :313-418)
**Not an NSPanel.** Spike chose plain Tauri window + objc2 (no tauri-nspanel). tao's borderless window already returns YES from `canBecomeKeyWindow`.
- Window config (tauri.conf.json): transparent, no decorations, not resizable, hidden at start, skipTaskbar, alwaysOnTop, fixed 640x460, `windowEffects: hudWindow, state active, radius 12`, `macOSPrivateApi: true` + tauri feature `macos-private-api`.
- objc2 bits (`mod mac`, :254-311): `ns_window()` (unsafe cast of `ns_window()` ptr, :266), `float_over_everything` (:274) sets collection behavior `CanJoinAllSpaces | FullScreenAuxiliary | Transient | IgnoresCycle` and level `NSStatusWindowLevel` (25; tauri alwaysOnTop only gives 3, below full-screen apps), `frontmost_other()` (NSWorkspace, :286), `activate(pid)` (:292), `fit_effect_view` (:298) refits window-vibrancy's NSVisualEffectView (tag 91376254) to the card via `setCornerRadius:` msg_send.
- **Positioning**: `place()` (:70-86) = monitor under cursor, centered horizontally, top at 1/3 height, clamped. Handles tao's mixed scales (cursor = points x primary scale; monitors = points x own scale), returns `LogicalPosition`. Empty monitor list (display asleep) falls back to primary. **For a tray popover replace with: x = tray rect center - width/2 (clamped to monitor), y = tray rect bottom.** The scale-conversion lessons and tests carry over; `TrayIconEvent::Click.rect` is in physical px of the tray's monitor (verify per-monitor).
- **Show/hide**: pure state machine `Visibility {shown, focused}` (:22-61) returning `Step {Show, Hide, Stay}`; `apply()` (:140) performs it. Show: position first (no flash on old monitor), `app.show()` (undo previous `app.hide()`), `w.show()`, `set_focus()`, emit `palette-shown`. Hide: `w.hide()`, then give focus back.
- **Outside-click dismiss**: two mechanisms. (a) window blur: `WindowEvent::Focused(false)` -> `focus_changed` (lib.rs:115-121); a blur only hides after a focus arrived since the last show (stale blur during activation ignored) and our own hide's blur is ignored. (b) transparent margin around the card still takes clicks, so the frontend hides on `pointerdown` whose `composedPath` misses the card (src/Palette.tsx:205). A popover sized exactly to its card avoids (b).
- **Focus return**: `app.hide()` after `w.hide()` makes macOS reactivate the previous app ("velo's trick"); `give_back()` (:125) chooses HideApp / Activate(pid) / Stay when Settings or What's new is open (hiding the app would hide them).
- **Multi-display**: see `place`; tests for 1x left of 2x primary at negative origin, 2x right of 1x primary, short monitor, cursor off all monitors.
- Rapid double-press race: `is_visible`/`is_focused` lag a queued show, hence own state (tests :335-380).

### login.rs (src-tauri/src/login.rs, 228 LOC; 4 tests)
- **LaunchAgent through tauri-plugin-autostart 2.7.0, not SMAppService** (objc2-service-management is in Cargo.lock only transitively). Plist `~/Library/LaunchAgents/<AUTOSTART_NAME>.plist`, arg `--hidden`.
- Truth is the OS (`enabled()` :113), nothing stored in settings except `loginApplied` first-run marker.
- Guards: `login_item_refusal` (:78) refuses debug builds, cargo `target/` paths, App Translocation, `& < >` in path (crate writes path unescaped). `should_refresh` (:108) rewrites plist only when the app moved (avoids "Background Items Added" notices). `set_login` command (:143) verifies the OS state after toggling.
- **Single-instance** via `flock` on `instance.lock` in app data dir (:23), with `restart-update` mark so the post-update relaunch waits 5 s. Replaces tauri-plugin-single-instance deliberately (/tmp socket squatting).
- Possible upgrade for the new app: SMAppService.mainApp (macOS 13+) avoids the plist-path staleness problem; hop did not do this.

### settings.rs (src-tauri/src/settings.rs, 466 LOC; 9 tests)
- File: `settings.json` in `app_config_dir()` (`~/Library/Application Support/<identifier>/`). camelCase JSON, `#[serde(default)]`, `version: u32 = 1`, `#[serde(flatten)] other` keeps unknown keys from newer versions and writes them back.
- `load` (:130): missing file = defaults + first-run; wrong-typed key = default for that key only (via `merge`); non-JSON file = copied to `settings.json.corrupt`, start from defaults. `save` (:166) is atomic (tmp + rename). **No migration code yet** (only `VERSION` stamp); migration would hook in `load` keyed on `version`.
- `merge` (:109) validates each patch key by round-tripping through serde plus `Settings::valid`; `set_settings` (:254) is all-or-nothing and serialized by `SAVING` mutex; returns a `View` DTO (settings + login + update + version) to the window.
- Settings window (:295): `WebviewWindowBuilder` on `settings.html`, 520x720, not minimizable/maximizable, hides palette first, `app.show()` to undo hide.
- `apply_theme` -> `app.set_theme(Light/Dark/None)` so vibrancy and `prefers-color-scheme` follow.

### update.rs / whatsnew.rs
- tauri-plugin-updater with minisign pubkey in tauri.conf.json (`requireSignedVersion: true`). Check 30 s after launch then daily (`due()` uses wall clock + awake monotonic), release builds only (`DEBUG_BUILD`). `install(app, expected)` refuses if a newer version appeared unseen. Progress phases (`Phase`) go over a Tauri `Channel` scoped to the What's new webview. Tray item text "Check for Updates…" / "Update to hop X…" (update.rs:222-256). Tests for bucket, due, tray text, committed feed config.
- whatsnew.rs: window `whatsnew`, `own_page()` rejects any non-bundled URL, commands `whatsnew_info/install/close`. Release notes cut at a `---` line (see build-release.yml).

### hotkey.rs
`tauri-plugin-global-shortcut`, default `Alt+Space`, `change()` swaps with rollback on failure. Carbon never reports cross-process conflicts (spike finding). Skip unless wanted.

## 2. docs/spikes/panel.md findings and gotchas
- Chose plain window + objc2 over tauri-nspanel: no new deps, every requirement maps to an NSWindow property. nspanel stays the fallback if Space switching or no keyboard focus over full-screen shows up.
- Accessory policy must be set before `run`; plus `LSUIElement` in Info.plist.
- Need level 25 (`NSStatusWindowLevel`) + collection behavior to appear over full-screen apps without Space switch; `alwaysOnTop` alone (level 3) is not enough.
- Vibrancy: tauri `windowEffects` covers whole window; if window > card, refit the effect view (tag lookup + private `setCornerRadius:`).
- Monitor scale mixing in tao (cursor vs monitors); use LogicalPosition. Empty monitor list while display asleep.
- Rapid toggle/blur race; keep own visibility state.
- Transparent margin takes clicks (consumed, not passed through); true click-through needs per-region `ignoresMouseEvents` + global mouse monitor, judged not worth it.
- Focus return via `app.hide()`.
- Carbon hotkey conflicts invisible across processes.
- Verification limits: screen locked in CI-like session gives black screenshots; owner did manual 12-step checklist. Measured: cold start 131-159 ms, bundle 3.9 MB (1.7 MB zipped), idle RSS 93 MB (172 MB with WebKit helpers), idle CPU 0%.
- Known failing: step 12 (Cmd+Q with palette open) is a known limit.

## 3. Config

### src-tauri/Cargo.toml
- package `hop` 0.1.0 (version not read; tauri.conf.json is the truth, synced by scripts/sync-version.mjs), edition **2024**, lib `hop_lib` crate-type staticlib/cdylib/rlib.
- tauri-build 2; deps: tauri 2 (features `macos-private-api`, `tray-icon`, `image-png`; locked 2.12.7), tauri-plugin-autostart 2.7.0, tauri-plugin-dialog 2.8.4, tauri-plugin-global-shortcut 2.4.1, tauri-plugin-updater 2.14.1, notify 8.2.2, serde 1.0.229 (derive), serde_json 1.0.154.
- macOS-only: objc2 0.6.5; objc2-app-kit 0.3.2 (default-features off; features std, NSBitmapImageRep, NSGraphics, NSGraphicsContext, NSImage, NSImageRep, NSPasteboard, NSResponder, NSRunningApplication, NSView, NSWindow, NSWorkspace, libc); objc2-foundation 0.3.2 (std, NSData, NSDictionary, NSFileManager, NSGeometry, NSString). Comment: "the versions tauri already pulls in".
- dev: tempfile 3.27.0. Release profile: codegen-units 1, lto, opt-level 3, panic abort, strip.
- Needed for audio app: drop notify, dialog (unless pickers), global-shortcut (optional).

### src-tauri/tauri.conf.json
- productName hop, version 0.2.0, identifier `dev.hop.app`; build: `pnpm dev` / devUrl :1420 / `pnpm build` / `../dist`.
- One window `main` (label), visible false, transparent, decorations false, resizable false, skipTaskbar, alwaysOnTop, 640x460, `windowEffects` hudWindow/active/radius 12. Settings and whatsnew windows created at runtime.
- `macOSPrivateApi: true`. CSP `default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: icon:` (drop `icon:`).
- bundle: active, `createUpdaterArtifacts: true`, targets all, icons 32/128/128@2x/icns, `macOS.signingIdentity: "-"` (ad-hoc signing; no notarization). **No `minimumSystemVersion` set**, no entitlements, no `hardenedRuntime`. LSUIElement comes from Info.plist (auto-merged from src-tauri/Info.plist).
- updater: minisign pubkey, endpoint `https://github.com/cef62/hop/releases/latest/download/latest.json` (rename repo owner/name), `requireSignedVersion: true`.
- New app must: generate its own updater keypair (`pnpm tauri signer generate`), store secrets `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)`, change pubkey/endpoint, and set `minimumSystemVersion` explicitly. CoreAudio needs no entitlement for device listing/default-device set; no mic permission unless capturing.

### capabilities (src-tauri/capabilities/)
- default.json (window main): `core:event:allow-listen`, `core:event:allow-unlisten`.
- settings.json (window settings): empty permissions.
- whatsnew.json (window whatsnew): empty permissions.
- Custom commands are guarded in Rust (lib.rs), not here. Quality gate item 5 in CONTRIBUTING requires capabilities to grant exactly what each window uses.

## 4. Frontend
- Layout: three Vite entry pages (index.html -> src/main.tsx -> Palette; settings.html -> src/settings/main.tsx; whatsnew.html -> src/whatsnew/main.tsx), one window each (vite.config.ts rollupOptions input). Alias `@` -> src. Pure logic split into small modules with colocated tests (rank.ts, fuzzy.ts, calc.ts, keys.ts, settings/form.ts, settings/chord.ts, whatsnew/notes.ts).
- **IPC typing**: hand-written TS types next to usage, no codegen (no tauri-specta/ts-rs). `invoke<T>("cmd", args)` direct in components (Palette.tsx:163-240, Settings.tsx:36-56); only whatsnew has a typed wrapper (src/whatsnew/api.ts: `info`, `install`, `close`, `Phase`, `UpdateInfo` mirroring Rust `Phase` in update.rs, plus a `STALE` string constant that must match Rust word for word). Events: `listen` for `palette-shown`/`palette-hidden`/`apps-changed`; progress via `Channel<Phase>`. Types can drift silently from Rust; a new app could add ts-rs/specta or keep the manual style with a pinned-test.
- State: React hooks only (useState/useRef/useEffect), no store library. Settings uses a `latest` ref + serial queue (`serial()` in settings/form.ts) so rapid saves apply in order against the last result; errors reload from backend.
- Views are split into pure `*View` components (e.g. `PaletteView`, `WhatsNew`) taking props, so tests use `renderToStaticMarkup` with `vi.mock("@tauri-apps/api/core")`.
- Styling: plain CSS with variables on `:root`, `color-scheme: light dark`, `@media (prefers-color-scheme: dark)` overrides, `[data-style=...]` attribute switches variable sets (src/palette.css). Settings/What's new use native HTML controls styled to look like macOS; no component library, no Panda/Park UI. Transparent window + vibrancy, card fill is semi-transparent rgba.
- Tests: Vitest 5 (node env, no jsdom), `.test.ts(x)` next to source, static-markup rendering for components, pure-function tests for logic; Rust tests inline `#[cfg(test)]`. No e2e. Coverage of a11y: focus management comment in Settings.tsx (VoiceOver).

## 5. package.json, workflows, repo setup
### package.json
Scripts: `dev` vite; `build` `tsc && vite build`; `preview`; `test` `vitest run`; `changeset`; `version-packages` (`changeset version && node scripts/sync-version.mjs`); `release:tag` (`changeset git-tag`); `tauri`; `build:app` (sets `TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/hop.key)"` and `..._PASSWORD` from `~/.tauri/hop.password`, then `tauri build`; **rename key paths**); `site:dev|site:build|site:preview|docs:build`. packageManager `pnpm@12.3.4`. Deps: react/react-dom 19.3.1, @tauri-apps/api ^2.12.4; dev: @tauri-apps/cli ^2.12.4, vite ^8.3.6, vitest ^5.1.0, typescript ~7.0.2, @vitejs/plugin-react ^6.2.1, @changesets/cli 3.0.3, astro 7.4.7 + @astrojs/markdown-remark (site only), @types/*. pnpm-workspace.yaml: `allowBuilds: esbuild: true`. tsconfig strict, noUnused*, `types: ["node"]`.

### Workflows (.github/workflows/)
- **ci.yml** "CI": PR/push main/workflow_dispatch; macos-latest; `pnpm build`, `pnpm test`, `cargo test` (working-directory src-tauri). Nothing app-specific except `workspaces: src-tauri` for rust-cache.
- **release.yml** "Release": push main; changesets/action opens "Version Packages" PR (`pnpm version-packages`) or runs `pnpm release:tag`; dispatches ci.yml on `changeset-release/main` (bot PRs get no pull_request CI) and dispatches build-release.yml with the tag (GITHUB_TOKEN tags do not trigger push events). Nothing app-specific.
- **build-release.yml** "Build release": tag `v*` or dispatch. Jobs: create-release (draft; title `"hop $TAG"` **rename**; notes from `scripts/release-notes.mjs`), build (macos-latest, aarch64-apple-darwin only, max-parallel 1, runs pnpm test + cargo test, tauri-action with signing secrets, `uploadUpdaterJson`), publish (checks `latest.json` has `darwin-aarch64`, undrafts, then `gh workflow run pages.yml` **drop if no site**). Requires repo secrets `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; repo setting release immutability on.
- **pages.yml** "Pages": builds Astro site to GitHub Pages; paths filter. Drop with site/.
- All actions pinned by commit SHA with version comments; node 26; `persist-credentials: false` in release build.
- Rename list: release title "hop", updater endpoint in tauri.conf.json, `AUTOSTART_NAME`/identifier `dev.hop.app`, `~/.tauri/hop.key|password`, `hop_lib` crate/lib name and main.rs call, strings "hop" in Rust messages, package.json name, README/CONTRIBUTING/AGENTS text, site base path `/hop/`.

### Agent config
- CLAUDE.md is one line `@AGENTS.md`. AGENTS.md (6.1k) = caveman + ponytail blocks wrapped in `<!-- caveman-begin/end -->` markers, then Versioning (changeset per user-visible change), Stack (React 19 + Vite, plain CSS vars, native HTML controls), Workflow (feature branch, quality gate table, conventional-commit PR title, `## Summary`/`## Test plan`, adversarial review by fresh agent Opus complex/Sonnet simple with attack list, strike rule 2 preferred/4 max, answer Copilot comments, squash-merge with `gh pr merge --squash --admin` only on owner's word).
- Same caveman/ponytail rules duplicated for other IDEs: .clinerules/, .cursor/rules/*.mdc, .windsurf/rules/, .opencode/AGENTS.md, .github/copilot-instructions.md.
- .claude/settings.json: enables plugins caveman@caveman and ponytail@ponytail with marketplaces from GitHub (JuliusBrussee/caveman, DietrichGebert/ponytail). settings.local.json and worktrees ignored by .gitignore.
- .changeset/config.json: baseBranch main, `privatePackages: {version:true, tag:true}`, changelog `@changesets/cli/changelog`, `commit:false`.
- .gitignore: node_modules, target, dist, .DS_Store, .superpowers, src-tauri/gen, .claude/settings.local.json, .claude/worktrees, .playwright-mcp, site/dist, .astro.
- Note: hop's AGENTS.md has **no small-files / small-functions rule**; add one for the new repo.

## 6. File sizes and rule violations ("small files, small functions")
Largest files (LOC): settings.rs 466, panel.rs 419, apps.rs 365, update.rs 331, Palette.tsx 270, Settings.tsx 250, login.rs 228, palette.css 218, lib.rs 176. Rust LOC include inline tests (panel.rs ~100 of 419 are tests; settings.rs ~120).
Longest functions (approximate, incl. tests): `SettingsPage` 235 lines (Settings.tsx:21), `Palette` 151 (Palette.tsx:140), `PaletteView` 92 (:48), `WhatsNew` 83, `lib::run` 81 (lib.rs:80), `update::install` 54, `launch::launch` 48, `panel::apply` 42, `settings::load` 36.
Patterns that would violate a strict rule (e.g. <=150-200 LOC files, <=30-40 line functions):
- Settings.tsx 235-line component with 7 useState and inline handlers; Palette 151-line component. Split into hooks (`useSettings`, `useRecorder`) plus view.
- `lib::run` 81 lines of builder + setup closure; extract `setup()` and `plugins()`.
- settings.rs mixes model, validation, IO, Tauri commands, window opening (4 concerns): split into model/store/commands/window.
- panel.rs mixes pure state machine, placement math, Tauri glue and objc2: split `visibility.rs`, `placement.rs`, `mac.rs`.
- Heavy comment blocks and long one-line statements (>140 chars, rustfmt not enforced; no rustfmt.toml, no clippy in CI).
- CI runs no lint (no clippy, no eslint/prettier). Add `cargo clippy -D warnings` and `cargo fmt --check` for new repo.

## 7. CoreAudio crates
- Not in hop's Cargo.lock (grep coreaudio = 0 hits). Present transitively: objc2 0.6.5, objc2-core-foundation 0.3.2, objc2-core-graphics 0.3.2, objc2-app-kit 0.3.2, objc2-foundation 0.3.2, block2 0.6.2, core-foundation 0.9.4 and 0.10.1, core-foundation-sys 0.8.7, objc2-service-management 0.3.2, window-vibrancy 0.8.1.
- `cargo search` (crates.io, today): objc2-core-audio 0.3.2, objc2-core-audio-types 0.3.2, coreaudio-sys 0.2.18, coreaudio-rs 0.14.2 (also `coreaudio` 0.6.2 listed as a separate wrapper crate).
- objc2-core-audio 0.3.2 matches hop's objc2 0.3.x family (same objc2 0.6 line), so it fits the "versions tauri already pulls in" policy. Property-listener callbacks (AudioObjectAddPropertyListenerBlock) pair with block2 0.6.2 already in the lock. coreaudio-sys is raw bindgen (needs clang at build), coreaudio-rs is higher level but focused on AudioUnits/streams rather than device-list/default-device properties.

## Reuse plan (suggested order)
1. Scaffold from hop: package.json, vite/vitest/tsconfig, build.rs, main.rs, Info.plist, .gitignore, .changeset, scripts/, workflows ci/release/build-release, AGENTS.md/CLAUDE.md/.claude, other-IDE rule files. Rename per list in section 5; add size/lint rules.
2. Copy login.rs, update.rs, whatsnew.rs (+ src/whatsnew, capabilities/whatsnew.json) with name changes.
3. Adapt lib.rs guard + tray, settings.rs store, panel.rs state machine and mac helpers; write tray-anchored `place()`.
4. New: audio module (CoreAudio listing, default in/out set, property listeners emitting `devices-changed`), priority logic as pure tested functions.
