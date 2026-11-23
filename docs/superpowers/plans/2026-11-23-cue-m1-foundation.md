# cue M1 Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A repo that mirrors hop, a Cargo workspace that builds, and a tray-only Tauri app that launches, writes `~/Library/Logs/com.cef62.cue/cue.log`, and passes a CI quality gate with clippy, fmt and a size check.

**Architecture:** Root Cargo workspace (`crates/cue-core` empty lib, `src-tauri` app `cue` / lib `cue_lib`). The app is an Accessory (agent) app with a tray menu and no windows. Logging goes through the `log` facade into `tauri-plugin-log`; one pure function picks the level, and `log::set_max_level` changes it at runtime. Node tooling (pnpm, Vite, Vitest, changesets) is copied from hop and adds `scripts/check-size.mjs`.

**Tech Stack:** Tauri 2.12.7, tauri-build 2.7.6, tauri-plugin-log 2.10.2 (Rust) / `@tauri-apps/plugin-log` ^2.10.2 (JS), log 0.4.35, Rust edition 2024, pnpm 12.3.4, TypeScript ~7.0.2, Vite ^8.3.6, Vitest ^5.1.0, @changesets/cli 3.0.3.

**Spec:** `docs/superpowers/specs/2026-11-23-cue-design.md` (§3, §5.11, §6, §8). Roadmap: `docs/superpowers/plans/2026-11-23-cue-roadmap.md` (shared conventions apply). Reference repo: `/Volumes/Dev/matteo/hop` (read-only; written `hop:` below). Reuse notes: `docs/audit/hop-reuse.md`.

## Global Constraints

- Bundle id `com.cef62.cue`. Product name `cue`. Repo `cef62/cue`.
- macOS on Apple Silicon only (`aarch64-apple-darwin`). `minimumSystemVersion` `14.0`.
- Ad-hoc signing: `bundle.macOS.signingIdentity` `"-"`. No updater, no `createUpdaterArtifacts`, no release workflows in M1 (M5).
- Workspace members: `crates/cue-core` (lib `cue_core`), `src-tauri` (package `cue`, lib `cue_lib`, bin `cue`). `crates/cue-coreaudio` arrives in M3.
- `#![forbid(unsafe_code)]` in `cue-core` and `src-tauri`.
- Non-test `.rs`, `.ts`, `.tsx` files: 300 lines or fewer. Functions: 40 lines or fewer (`clippy.toml` `too-many-lines-threshold = 40`).
- Quality gate: `pnpm build`, `pnpm test`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm check:size`.
- Log file `~/Library/Logs/com.cef62.cue/cue.log`, rotates at 1 MB, keeps 5 files. Debug detail only for cue crates; dependencies stay at info.
- Debug level when `CUE_LOG=debug`, in debug builds, or when the Debug setting is on (setting arrives in M4).
- Rust unit tests inline `#[cfg(test)] mod tests`; TS tests Vitest, next to the module.
- No ESLint. TypeScript `strict`.
- Any departure from the spec gets an ADR in `docs/adr/`.

## Review Focus

1. A second cue starts while one runs (login item, or `pnpm tauri dev` beside the installed app): the second exits at once and logs why; the first keeps its tray icon. Pinned in Task 4.
2. `CUE_LOG` holds `DEBUG`, ` debug `, an empty string or `verbose`: case and spaces are ignored, an unknown value falls back to the default, nothing panics. Pinned in Task 5.
3. Debug mode is on: `tao`, `wry` and other dependencies still log at info only; a target that only starts with the same letters (`cue_libx`, `cuex`) is not a cue crate. Pinned in Task 5.
4. `check:size` meets CRLF files, a trailing newline, `#[cfg(test)]` inside a string, and new files not yet added to git: CRLF and the trailing newline do not change the count, the string does not cut the count, untracked files are checked, gitignored files are not. Pinned in Task 2.
5. The webview logs an `Error` or an object: the log line shows the stack or JSON, not `[object Object]` or `{}`, and the devtools console still prints it. Pinned in Task 5.

---

## File map

| Path | Responsibility | Task |
|---|---|---|
| `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `README.md`, `LICENSE`, `.gitignore` | repo rules and docs | 1 |
| `.claude/settings.json`, `.cursor/rules/*`, `.windsurf/rules/*`, `.clinerules/*`, `.opencode/AGENTS.md`, `.github/copilot-instructions.md` | agent config | 1 |
| `.changeset/config.json`, `.changeset/README.md` | versioning | 1 |
| `package.json`, `pnpm-workspace.yaml`, `pnpm-lock.yaml`, `tsconfig.json`, `vite.config.ts`, `vitest.config.ts`, `index.html`, `src/main.ts` | Node tooling, empty frontend | 2 |
| `scripts/sync-version.mjs` | package.json version into tauri.conf.json | 2 |
| `scripts/check-size.mjs`, `scripts/check-size.test.ts` | 300-line limit | 2 |
| `Cargo.toml`, `Cargo.lock`, `clippy.toml` | workspace, lints, release profile | 3 |
| `crates/cue-core/{Cargo.toml,src/lib.rs}` | empty core (M2 fills it) | 3 |
| `src-tauri/{Cargo.toml,build.rs,Info.plist,tauri.conf.json}` | app package and bundle config | 4 |
| `src-tauri/icons/*` | app icons (placeholders), tray template glyph | 4 |
| `src-tauri/src/{main.rs,lib.rs}` | entry, builder, setup order | 4, 5 |
| `src-tauri/src/instance.rs` | single-instance `flock` lock | 4 |
| `src-tauri/src/tray.rs` | tray icon and "Quit cue" | 4 |
| `src-tauri/src/logging.rs` | level choice, target filter, plugin builder, runtime level | 5 |
| `src/log.ts`, `src/log.test.ts`, `src-tauri/capabilities/default.json` | webview console forwarding | 5 |
| `.github/workflows/ci.yml` | CI quality gate | 6 |
| `docs/adr/0005-cargo-workspace-and-size-limits.md`, `docs/adr/README.md` | ADR | 7 |

---

### Task 1: Branches and repo skeleton

**Files:**
- Create: `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `README.md`, `LICENSE`, `.gitignore`, `.claude/settings.json`, `.cursor/rules/caveman.mdc`, `.cursor/rules/ponytail.mdc`, `.windsurf/rules/{caveman,ponytail}.md`, `.clinerules/{caveman,ponytail}.md`, `.opencode/AGENTS.md`, `.github/copilot-instructions.md`, `.changeset/config.json`, `.changeset/README.md`

**Interfaces:**
- Consumes: nothing.
- Produces: branch `feat/m1-foundation`; the quality gate table in `CONTRIBUTING.md` that Task 7 runs.

- [ ] **Step 1: Commit the plans and create the branches**

The repo is on `docs/design` (2 commits) with `docs/superpowers/plans/` untracked. `main` does not exist yet.

```bash
cd /Volumes/Dev/matteo/macos-audio-priority
git add docs/superpowers/plans
git commit -m "docs: add cue roadmap and M1 plan"
git branch main docs/design
git switch -c feat/m1-foundation docs/design
git branch
```
Expected: `docs/design`, `* feat/m1-foundation`, `main`, all at the same commit. Do not push (the owner pushes later).

- [ ] **Step 2: Copy the files that need no change**

Copy verbatim from hop: `LICENSE`, `CLAUDE.md` (`@AGENTS.md`), `.claude/settings.json`, `.cursor/rules/`, `.windsurf/rules/`, `.clinerules/`, `.opencode/AGENTS.md`, `.github/copilot-instructions.md`, `.changeset/config.json`, `.changeset/README.md`. None mentions hop (checked with `grep -il hop`).

- [ ] **Step 3: Copy `.gitignore`, then drop `site/dist/` and `.astro/`**

No docs site in cue. Keep every other line (root `target/` covers the workspace target dir).

- [ ] **Step 4: Copy `AGENTS.md`, then change three things**

1. Replace the `## Stack` body with: "Tauri 2 with a Rust Cargo workspace (`crates/cue-core`, `crates/cue-coreaudio` from M3, `src-tauri`) and React 19 + Vite, plain CSS using CSS variables. The Settings and What's new windows use native HTML controls styled to look like macOS; no component library, no ESLint."
2. Add `## Size limits`: non-test `.rs`, `.ts` and `.tsx` files stay at 300 lines or fewer (`pnpm check:size`; what counts as test is in ADR 0005); functions stay at 40 lines or fewer (`clippy::too_many_lines`, threshold in `clippy.toml`). Split by responsibility before a file reaches the limit. `unsafe` only in `crates/cue-coreaudio`.
3. Add `## Decisions`: the spec is `docs/superpowers/specs/2026-11-23-cue-design.md`. Any decision that departs from it gets a new ADR in `docs/adr/` (template and index in `docs/adr/README.md`) in the same PR.

- [ ] **Step 5: Copy `CONTRIBUTING.md`, then adapt it**

- Title `# Contributing to cue`; every `hop` becomes `cue`.
- Quality gate row 1 becomes: `pnpm build`, `pnpm test`, `pnpm check:size`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` all green; new logic has a test.
- Row 2: `pnpm build:app` (or at least `cargo build --release`) when `src-tauri/`, `crates/` or `tauri.conf.json` changed.
- Delete the docs-site paragraph. Keep "quit the installed cue before `pnpm tauri dev`".
- Replace the bodies of "Testing updates against a local feed" and "Releases" with one line each: "Added in M5 with the updater and release workflows."

- [ ] **Step 6: Write the `README.md` stub**

Sections: one-line description (spec §1 first sentence), "Status: in development, no release yet", "Build from source" with `pnpm install`, `pnpm build`, `pnpm tauri dev`, and "Logs: `~/Library/Logs/com.cef62.cue/cue.log`; `CUE_LOG=debug` adds detail".

- [ ] **Step 7: Verify the copies**

Run: `diff -r /Volumes/Dev/matteo/hop/.cursor .cursor && diff /Volumes/Dev/matteo/hop/LICENSE LICENSE && grep -rn "hop" AGENTS.md CONTRIBUTING.md README.md .gitignore`
Expected: no diff output; the grep prints nothing.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "chore: repo skeleton and agent config from hop"
```

---

### Task 2: Node tooling and the size check

**Files:**
- Create: `package.json`, `pnpm-workspace.yaml`, `tsconfig.json`, `vite.config.ts`, `vitest.config.ts`, `index.html`, `src/main.ts`, `scripts/sync-version.mjs`, `scripts/check-size.mjs`
- Test: `scripts/check-size.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `pnpm build` (emits `dist/`, which `tauri::generate_context!` needs at compile time), `pnpm test`, `pnpm check:size`. From `scripts/check-size.mjs`:
  - `export const LIMIT = 300`
  - `export function isTestFile(path: string): boolean`
  - `export function countLines(path: string, text: string): number`
  - `export function findOversized(files: {path: string, text: string}[], limit = LIMIT): {path: string, lines: number}[]`
  - `export function listFiles(cwd: string): string[]`

- [ ] **Step 1: Copy `package.json` from hop, then change it**

- `name` `cue`, `version` `0.0.0` (the first changeset makes it 0.1.0).
- Scripts: keep `dev`, `build`, `preview`, `test`, `changeset`, `version-packages`, `tauri`. Add `"check:size": "node scripts/check-size.mjs"` and `"build:app": "tauri build --target aarch64-apple-darwin --bundles app"` (M5 adds the signing env vars). Drop `release:tag` (M5) and every `site:*`/`docs:*` script.
- devDependencies: keep `@changesets/cli`, `@tauri-apps/cli`, `@types/node`, `typescript`, `vite`, `vitest`. Drop `astro`, `@astrojs/markdown-remark`, `@vitejs/plugin-react`, `@types/react*`.
- dependencies: `@tauri-apps/api` ^2.12.4, `@tauri-apps/plugin-log` ^2.10.2. React, react-dom and the React plugin come back in M4 with the first window (no unused dependencies, gate row 4).

- [ ] **Step 2: Copy `pnpm-workspace.yaml`, `tsconfig.json`, `vitest.config.ts` as-is; copy `vite.config.ts` without the React plugin and without `build.rollupOptions` (one page)**

- [ ] **Step 3: Write `index.html` (copy hop's, title `cue`, no stylesheet, script `/src/main.ts`) and `src/main.ts` as one comment line: `// The panel arrives in M4; Task 5 adds console forwarding here.`**

- [ ] **Step 4: Copy `scripts/sync-version.mjs` as-is** (no hop names in it).

- [ ] **Step 5: Install and build**

Run: `pnpm install && pnpm build`
Expected: `pnpm-lock.yaml` written; `dist/index.html` exists; exit 0.

- [ ] **Step 6: Write the failing tests in `scripts/check-size.test.ts`**

Import from `./check-size.mjs`. Helper `lines(n)` returns `n` lines joined by `\n` plus a trailing `\n`.

```ts
test("a 300-line file passes and 301 fails", () => {
  expect(findOversized([{ path: "src/a.ts", text: lines(300) }])).toEqual([]);
  expect(findOversized([{ path: "src/a.ts", text: lines(301) }])).toEqual([{ path: "src/a.ts", lines: 301 }]);
});
test("trailing newline and CRLF do not change the count", () => {
  expect(countLines("a.ts", "x\ny")).toBe(2);
  expect(countLines("a.ts", "x\ny\n")).toBe(2);
  expect(countLines("a.ts", "x\r\ny\r\n")).toBe(2);
});
test("test files count as zero", () => {
  for (const p of ["src/a.test.ts", "src/A.test.tsx", "tests/x.rs", "crates/cue-core/tests/s.rs", "src-tauri/tests/t.ts"])
    expect(isTestFile(p), p).toBe(true);
  for (const p of ["src/a.ts", "src/contest.ts", "src/testsuite.rs", "src-tauri/src/lib.rs"])
    expect(isTestFile(p), p).toBe(false);
});
test("Rust counts only the lines before the first #[cfg(test)] line", () => {
  expect(countLines("x.rs", `${lines(10)}#[cfg(test)]\n${lines(500)}`)).toBe(10);
  expect(countLines("x.rs", `${lines(10)}    #[cfg(test)]\n${lines(5)}`)).toBe(10);
});
test("#[cfg(test)] inside a string or followed by code is not the cut", () => {
  expect(countLines("x.rs", `let s = "#[cfg(test)]";\n${lines(4)}`)).toBe(5);
  expect(countLines("x.rs", `#[cfg(test)] use foo;\n${lines(4)}`)).toBe(5);
});
test("only .rs, .ts and .tsx are checked", () => {
  for (const p of ["a.mjs", "a.css", "a.json", "a.md"])
    expect(findOversized([{ path: p, text: lines(999) }])).toEqual([]);
});
test("lists untracked files and skips gitignored ones", () => {
  // tmp dir: git init, write .gitignore "target/\n", target/big.rs, src/new.rs (untracked, not added)
  expect(files).toContain("src/new.rs");
  expect(files).not.toContain("target/big.rs");
});
```

- [ ] **Step 7: Run them to see them fail**

Run: `pnpm test scripts/check-size.test.ts`
Expected: FAIL, "Failed to load url ./check-size.mjs" (or "does not provide an export").

- [ ] **Step 8: Implement `scripts/check-size.mjs`**

- Lines: `text.split(/\r?\n/)`, drop one trailing empty element.
- `isTestFile`: a path segment equal to `tests`, or a name ending `.test.ts` / `.test.tsx`.
- Rust cut: the first line matching `/^\s*#\[cfg\(test\)\]\s*$/`. Add `// shortcut: everything after the first #[cfg(test)] line is uncounted, even non-test code; upgrade to a real parser if that is abused.`
- `listFiles(cwd)`: `git ls-files --cached --others --exclude-standard` run in `cwd`, split on newlines, drop empties.
- CLI (same `invokedDirectly` guard as `sync-version.mjs`): read every listed `.rs/.ts/.tsx` file, print each offender as `<path>: <n> lines (limit 300)` and exit 1; otherwise print `check:size ok (<k> files)`.

- [ ] **Step 9: Run tests and the CLI**

Run: `pnpm test && pnpm check:size`
Expected: `Test Files  1 passed`, 7 tests passed; `check:size ok (` followed by a file count.

- [ ] **Step 10: Commit**

```bash
git add package.json pnpm-workspace.yaml pnpm-lock.yaml tsconfig.json vite.config.ts vitest.config.ts index.html src scripts
git commit -m "build: node tooling and size check"
```

---

### Task 3: Cargo workspace and cue-core

**Files:**
- Create: `Cargo.toml`, `clippy.toml`, `crates/cue-core/Cargo.toml`, `crates/cue-core/src/lib.rs`
- Copy: `hop:src-tauri/Cargo.lock` to `Cargo.lock` (seed, see Step 2)

**Interfaces:**
- Consumes: nothing.
- Produces: workspace with `[workspace.package]` (`version = "0.1.0"`, `edition = "2024"`, `license = "MIT"`) and `[workspace.lints]`; every member sets `[lints] workspace = true` and inherits package fields with `.workspace = true`. Crate `cue_core` with no public items yet.

- [ ] **Step 1: Write the root `Cargo.toml`**

- `[workspace]` `resolver = "3"`, `members = ["crates/cue-core", "src-tauri"]`.
- `[workspace.lints.clippy]` `too_many_lines = "warn"`. It is a pedantic lint, so the threshold in `clippy.toml` does nothing unless it is enabled here; `-D warnings` turns it into an error.
- Move hop's `[profile.release]` block here verbatim. Cargo ignores profiles in member manifests.
- `clippy.toml`: `too-many-lines-threshold = 40`. No `rustfmt.toml` (defaults).

- [ ] **Step 2: Seed `Cargo.lock` from hop**

`cp /Volumes/Dev/matteo/hop/src-tauri/Cargo.lock Cargo.lock`. Cargo keeps the versions already locked (tauri 2.12.7, tauri-build 2.7.6, log 0.4.35, tempfile 3.27.0, serde_json 1.0.154), adds new ones and prunes unused ones on the next build.

- [ ] **Step 3: Write `crates/cue-core`**

`Cargo.toml`: `name = "cue-core"`, inherited version/edition/license, `description = "cue's pure domain: priority lists, reconcile, identity"`, no dependencies (serde arrives in M2), `[lints] workspace = true`.

`src/lib.rs`: a `//!` doc line ("Pure domain for cue; no I/O, no clock. Filled in M2."), `#![forbid(unsafe_code)]`, and the smoke test:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds_as_a_workspace_member() {
        assert_eq!(env!("CARGO_PKG_NAME"), "cue-core");
    }
}
```

- [ ] **Step 4: Temporarily build only the core**

`src-tauri` does not exist yet, so run: `cargo test -p cue-core` after commenting `"src-tauri"` out of `members`.
Expected: `test tests::crate_builds_as_a_workspace_member ... ok`, `test result: ok. 1 passed`.

- [ ] **Step 5: Lint and format**

Run: `cargo clippy -p cue-core --all-targets -- -D warnings && cargo fmt --all --check`
Expected: `Finished`, no warnings; fmt prints nothing, exit 0.

- [ ] **Step 6: Commit (with `"src-tauri"` still commented out; Task 4 restores it)**

```bash
git add Cargo.toml Cargo.lock clippy.toml crates
git commit -m "build: cargo workspace with empty cue-core"
```

---

### Task 4: Tauri app shell (agent app, tray, single instance)

**Files:**
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/Info.plist`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/instance.rs`, `src-tauri/src/tray.rs`, `src-tauri/icons/tray-template.svg`, `src-tauri/icons/tray-template.png`, `src-tauri/icons/{32x32.png,128x128.png,128x128@2x.png,icon.icns,icon.png}`
- Modify: `Cargo.toml` (restore `"src-tauri"` in `members`)

**Interfaces:**
- Consumes: workspace from Task 3; `dist/` from Task 2 (`pnpm build` before any cargo command).
- Produces:
  - `cue_lib::run()` (called by `main.rs`).
  - `instance::hold_instance_lock(dir: &Path) -> bool`: true when this process holds `dir/instance.lock`.
  - `tray::build(app: &tauri::AppHandle) -> tauri::Result<()>`: tray id `"main"`, tooltip `"cue"`, one item id `"quit"` text `"Quit cue"`.
  - `run()` has a `.setup` closure that Task 5 extends; keep `run` at 40 lines or fewer by putting the setup body in `fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>>`.

- [ ] **Step 1: Write `src-tauri/Cargo.toml`** (copy hop's, then change)

- `name = "cue"`, inherited version/edition/license, description from the README line. `[lib] name = "cue_lib"`, same `crate-type`.
- `[build-dependencies]` `tauri-build = "2.7.6"`.
- `[dependencies]` `tauri = { version = "2.12.7", features = ["tray-icon", "image-png"] }`, `tauri-plugin-log = "2.10.2"`, `log = "0.4.35"`. Drop every other hop dependency, the objc2 block and `macos-private-api` (M4 adds the panel helpers).
- `[dev-dependencies]` `tempfile = "3.27.0"`, `serde_json = "1.0.154"`.
- Drop `[profile.release]` (now in the root). Add `[lints] workspace = true`.

- [ ] **Step 2: Copy `build.rs`, `Info.plist` (comment: "hop" becomes "cue") and `main.rs` (`hop_lib` becomes `cue_lib`; add `#![forbid(unsafe_code)]`)**

- [ ] **Step 3: Write `tauri.conf.json`** (copy hop's, then change)

- `productName` `cue`, `version` `0.0.0`, `identifier` `com.cef62.cue`. Keep `build`.
- `app.windows`: `[]`. Tauri 2 runs with no windows; a tray app stays alive. Drop `macOSPrivateApi`.
- CSP: hop's without `icon:`.
- `bundle`: drop `createUpdaterArtifacts`; keep `targets`, `icon`; `macOS`: `{ "signingIdentity": "-", "minimumSystemVersion": "14.0" }`.
- Drop the whole `plugins.updater` block (M5).

- [ ] **Step 4: Make the icons**

- App icons: copy hop's five files from `hop:src-tauri/icons/` as placeholders (a cue icon comes before the first release, M5).
- Tray glyph (spec §7.2 "on" speaker, placeholder): write `src-tauri/icons/tray-template.svg`:

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="44" height="36" viewBox="0 0 22 18"><path d="M2 6h4l5-4v14l-5-4H2z" fill="#000"/><path d="M14 5.5a5 5 0 0 1 0 7M17 3a8.5 8.5 0 0 1 0 12" fill="none" stroke="#000" stroke-width="1.8" stroke-linecap="round"/></svg>
```

Run: `sips -s format png src-tauri/icons/tray-template.svg --out src-tauri/icons/tray-template.png && file src-tauri/icons/tray-template.png`
Expected: `PNG image data, 44 x 36, 8-bit/color RGBA`.

- [ ] **Step 5: Write the failing tests**

In `src-tauri/src/instance.rs` (port of `hop:src-tauri/src/login.rs:195-201`, without the wait):

```rust
#[test]
fn a_second_holder_of_the_instance_lock_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(hold_instance_lock(tmp.path()));
    assert!(!hold_instance_lock(tmp.path()));
}
#[test]
fn a_missing_dir_is_created() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(hold_instance_lock(&tmp.path().join("a/b")));
}
```

In `src-tauri/src/lib.rs`:

```rust
#[test]
fn bundle_config_pins_identity_and_platform() {
    let c: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert_eq!(c["identifier"], "com.cef62.cue");
    assert_eq!(c["productName"], "cue");
    assert_eq!(c["bundle"]["macOS"]["minimumSystemVersion"], "14.0");
    assert_eq!(c["bundle"]["macOS"]["signingIdentity"], "-");
    assert!(c["plugins"]["updater"].is_null(), "updater is M5");
}
```

- [ ] **Step 6: Run them to see them fail**

Run: `cargo test -p cue`
Expected: compile error `cannot find function hold_instance_lock`.

- [ ] **Step 7: Implement `instance.rs`**

Copy `hold_instance_lock` and its doc comment from `hop:src-tauri/src/login.rs:16-41`. Drop the `wait` parameter and the retry loop (the restart mark is M5 with the updater): one `try_lock`, `WouldBlock` returns false, any other error logs `log::warn!` and returns true. Keep `std::mem::forget(f)`. Single-instance uses this lock, not `tauri-plugin-single-instance`; the reason is in the copied doc comment and in ADR 0005.

- [ ] **Step 8: Implement `tray.rs` and `lib.rs`**

- `tray::build`: adapt `hop:src-tauri/src/lib.rs:50-78`. Menu has only `MenuItem::with_id(app, "quit", "Quit cue", true, None::<&str>)`; on `"quit"` call `log::info!("quit from tray")` then `app.exit(0)`. Keep `include_bytes!("../icons/tray-template.png")`, `icon_as_template(true)`.
- `lib.rs`: `#![forbid(unsafe_code)]`, `mod instance; mod tray;`. `run()` follows `hop:src-tauri/src/lib.rs:80-158` minus everything cue lacks: builder with `.setup(setup)`, `.build(tauri::generate_context!()).expect("error while running cue")`, `set_activation_policy(Accessory)` before `run` (keep hop's comment), `app.run(|_, _| {})`.
- `setup`: first, `app.path().app_data_dir()` and `hold_instance_lock`; when false, print `cue: another cue is running; this one exits` to stderr and `std::process::exit(0)`. Then `tray::build`; on error print `cue: no menu-bar icon (<e>)` and continue.
- Restore `"src-tauri"` in the root `members`.

- [ ] **Step 9: Run tests, lint, format**

Run: `pnpm build && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`
Expected: 4 tests pass (`cue-core` 1, `cue` 3); clippy `Finished` with no warnings; fmt silent.
If `#![forbid(unsafe_code)]` fails inside `tauri::generate_context!` or `generate_handler!` expansion, switch `src-tauri` to `#![deny(unsafe_code)]` and record it in ADR 0005 (Task 7).

- [ ] **Step 10: Launch by hand**

Run: `pnpm tauri dev`
Expected: a speaker glyph in the menu bar, tinted for light and dark; no Dock icon; not in ⌘Tab; menu shows only "Quit cue", which ends the process.

- [ ] **Step 11: Commit**

```bash
git add Cargo.toml Cargo.lock src-tauri
git commit -m "feat: tray-only agent app with single-instance lock"
```

---

### Task 5: Logging and webview console forwarding

**Files:**
- Create: `src-tauri/src/logging.rs`, `src/log.ts`, `src-tauri/capabilities/default.json`
- Modify: `src-tauri/src/lib.rs` (plugin, setup), `src-tauri/src/instance.rs` and `tray.rs` (stderr prints become `log::` calls), `src/main.ts`
- Test: inline in `logging.rs`; `src/log.test.ts`

**Interfaces:**
- Consumes: `run()` / `setup` from Task 4.
- Produces (M4 calls `apply` when the Debug toggle changes):
  - `logging::log_level(env: Option<&str>, debug_build: bool, debug_setting: bool) -> log::LevelFilter`
  - `logging::allowed(target: &str, level: log::Level, max: log::LevelFilter) -> bool`
  - `logging::plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R>`
  - `logging::apply(debug_setting: bool) -> log::LevelFilter`: reads `CUE_LOG`, calls `log_level` with `cfg!(debug_assertions)`, then `log::set_max_level`; returns the level.
  - `const CUE_TARGETS: &[&str] = &["cue", "cue_lib", "cue_core", "cue_coreaudio", tauri_plugin_log::WEBVIEW_TARGET]`
  - TS: `export type LogSink = Record<"trace" | "debug" | "info" | "warn" | "error", (msg: string) => Promise<void>>`, `export function format(args: unknown[]): string`, `export function forwardConsole(c: Pick<Console, "log" | "debug" | "info" | "warn" | "error">, sink: LogSink): void`

Plugin facts (read in the 2.10.2 crate source):
- `RotationStrategy::KeepSome(n)` keeps `n` rotated files plus the active one. Spec "keeps 5 files" means 5 on disk, so use `KeepSome(4)`: at most 5 files, 5 MB.
- `TargetKind::LogDir` resolves to `app_log_dir()`, which is `~/Library/Logs/com.cef62.cue/` on macOS; `file_name: Some("cue")` gives `cue.log`.
- `build()` calls `log::set_max_level` with the dispatch's own maximum during plugin setup, which runs before the app's `setup`. `apply` runs in `setup`, so it wins.
- The webview's `plugin:log|log` command logs with a target that starts with `WEBVIEW_TARGET` (`"webview"`).

- [ ] **Step 1: Write the failing Rust tests in `logging.rs`**

```rust
use log::{Level, LevelFilter as F};
#[test]
fn level_defaults_to_info_in_release_without_setting() {
    assert_eq!(log_level(None, false, false), F::Info);
}
#[test]
fn debug_build_or_setting_turns_debug_on() {
    assert_eq!(log_level(None, true, false), F::Debug);
    assert_eq!(log_level(None, false, true), F::Debug);
}
#[test]
fn cue_log_wins_and_is_parsed_loosely() {
    assert_eq!(log_level(Some("debug"), false, false), F::Debug);
    assert_eq!(log_level(Some(" DEBUG "), false, false), F::Debug);
    assert_eq!(log_level(Some("trace"), false, false), F::Trace);
    assert_eq!(log_level(Some("warn"), true, true), F::Warn);
}
#[test]
fn unknown_or_empty_cue_log_falls_back() {
    assert_eq!(log_level(Some(""), false, false), F::Info);
    assert_eq!(log_level(Some("verbose"), true, false), F::Debug);
}
#[test]
fn debug_detail_is_for_cue_crates_only() {
    for t in ["cue", "cue_lib::tray", "cue_core", "cue_coreaudio::sys", "webview:http://localhost"] {
        assert!(allowed(t, Level::Debug, F::Debug), "{t}");
    }
    for t in ["tao::platform", "wry", "cue_libx", "cuex", "tauri"] {
        assert!(!allowed(t, Level::Debug, F::Debug), "{t}");
        assert!(allowed(t, Level::Info, F::Debug), "{t}");
    }
}
#[test]
fn max_level_caps_everyone() {
    assert!(!allowed("cue_lib", Level::Debug, F::Info));
    assert!(!allowed("tao", Level::Info, F::Warn));
    assert!(allowed("cue_lib", Level::Error, F::Error));
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p cue logging`
Expected: compile error `cannot find function log_level`.

- [ ] **Step 3: Implement `logging.rs`**

- `log_level`: `env` trimmed, parsed case-insensitively with `LevelFilter::from_str`; a parse error or empty string is ignored. Otherwise `Debug` if `debug_build || debug_setting`, else `Info`.
- `allowed`: a target is a cue target when it equals an entry of `CUE_TARGETS` or starts with `<entry>::` or `<entry>:` (the webview target uses a single colon). Cue targets pass when `level <= max`; others when `level <= min(max, Info)`.
- `plugin()`: `tauri_plugin_log::Builder::new()` with `.clear_targets()`, targets `Stdout` and `LogDir { file_name: Some("cue".into()) }`, `.max_file_size(1_000_000)`, `.rotation_strategy(RotationStrategy::KeepSome(4))` (comment: KeepSome counts rotated files only; 4 + active = the spec's 5), `.timezone_strategy(TimezoneStrategy::UseLocal)`, `.filter(|m| allowed(m.target(), m.level(), log::max_level()))`, `.build()`. Leave `.level` at its default (Trace) so `log::set_max_level` alone controls the level at runtime.
- `apply`: as in Interfaces; ends with `log::info!("log level {level}")` so every toggle is logged (spec §5.11).

- [ ] **Step 4: Wire it in `lib.rs`**

- `.plugin(logging::plugin())` first on the builder.
- `setup` order: `logging::apply(false)` (comment: M4 passes the persisted Debug setting), then `log::info!("cue {} starting", app.package_info().version)`, then the instance lock, then the tray.
- The "another cue is running" and "no menu-bar icon" messages become `log::warn!`. In `instance.rs` the unexpected lock error stays `log::warn!`. Add `log::debug!("instance lock held in {}", dir.display())` after a successful lock (a debug line to check the level by hand).

- [ ] **Step 5: Run the Rust tests**

Run: `cargo test --workspace`
Expected: `test result: ok` for both crates; 9 tests in `cue`.

- [ ] **Step 6: Write the failing TS tests in `src/log.test.ts`**

```ts
test("format joins args and shows errors and objects readably", () => {
  expect(format(["a", 1, true])).toBe("a 1 true");
  expect(format([{ x: 1 }])).toBe('{"x":1}');
  const e = new Error("boom");
  expect(format([e])).toBe(e.stack);
  const cyclic: Record<string, unknown> = {}; cyclic.self = cyclic;
  expect(format([cyclic])).toBe("[object Object]");
});
test("forwardConsole maps each method to its level and still prints", () => {
  // fake console whose methods record calls; fake sink whose methods record (level, msg) and resolve
  forwardConsole(fake, sink);
  fake.log("l"); fake.debug("d"); fake.info("i"); fake.warn("w"); fake.error("e");
  expect(sent).toEqual([["info", "l"], ["debug", "d"], ["info", "i"], ["warn", "w"], ["error", "e"]]);
  expect(printed).toEqual(["l", "d", "i", "w", "e"]);
});
test("a failing sink never throws into the caller", () => {
  // sink methods return Promise.reject(new Error("ipc down"))
  expect(() => fake.error("x")).not.toThrow();
});
```

- [ ] **Step 7: Run them to see them fail**

Run: `pnpm test src/log.test.ts`
Expected: FAIL, cannot resolve `./log`.

- [ ] **Step 8: Implement `src/log.ts`, `src/main.ts` and the capability**

- `format`: strings as-is; `Error` as `stack ?? message`; other objects with `JSON.stringify`, falling back to `String(v)` when it throws; join with one space.
- `forwardConsole`: for each method keep the original, replace it with a function that calls the original then `sink[level](format(args)).catch(() => {})`; `log` maps to `info`.
- `src/main.ts`: `import * as sink from "@tauri-apps/plugin-log"; import { forwardConsole } from "./log"; forwardConsole(console, sink);`
- `src-tauri/capabilities/default.json`: shape of `hop:src-tauri/capabilities/default.json`, `"windows": ["panel"]`, `"permissions": ["log:default"]`, description "Panel window (M4): forwards its console to the log file". M4 must create its window with label `panel` or change this file.

- [ ] **Step 9: Run the whole gate**

Run: `pnpm build && pnpm test && pnpm check:size && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`
Expected: all exit 0; Vitest `Test Files  2 passed`.

- [ ] **Step 10: Check the log by hand**

```bash
pnpm tauri dev            # quit from the tray after it starts
tail -n 5 ~/Library/Logs/com.cef62.cue/cue.log
CUE_LOG=info pnpm tauri dev   # quit again
tail -n 5 ~/Library/Logs/com.cef62.cue/cue.log
```
Expected: first run has `log level DEBUG`, `cue 0.0.0 starting` and `instance lock held in …`; second run has `log level INFO`, the start line, and no `instance lock held` line.

- [ ] **Step 11: Commit**

```bash
git add src src-tauri package.json pnpm-lock.yaml Cargo.lock
git commit -m "feat: file logging with CUE_LOG and debug-build levels"
```

---

### Task 6: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: the gate commands from Global Constraints.
- Produces: workflow `CI`, job `verify`, which M5's `release.yml` dispatches by name.

- [ ] **Step 1: Copy `hop:.github/workflows/ci.yml`, then change it**

- Keep triggers, concurrency, permissions, the pinned action SHAs and their comments, node 26.
- `dtolnay/rust-toolchain` step: add `with: components: clippy, rustfmt`.
- `Swatinem/rust-cache`: remove `with: workspaces: src-tauri` (the workspace target dir is the root default).
- Steps after `pnpm install --frozen-lockfile`, in this order: `pnpm build` (makes `dist/`, needed by the Rust build), `pnpm test`, `pnpm check:size`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. No `working-directory`.

- [ ] **Step 2: Validate the YAML**

Run: `grep -c "run: " .github/workflows/ci.yml`
Expected: `7` (install plus six gate commands). If `actionlint` is installed, `actionlint .github/workflows/ci.yml` prints nothing.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: quality gate with clippy, fmt and size check"
```

CI runs for real once the owner creates `cef62/cue` and pushes (M5 prerequisite list).

---

### Task 7: ADR 0005, changeset, final gate

**Files:**
- Create: `docs/adr/0005-cargo-workspace-and-size-limits.md`, `.changeset/<generated>.md`
- Modify: `docs/adr/README.md` (index row)

**Interfaces:**
- Consumes: decisions made in Tasks 2 to 5.
- Produces: the rules that `AGENTS.md` points to.

- [ ] **Step 1: Write ADR 0005 "Cargo workspace layout and size limits"** (template in `docs/adr/README.md`, Status Accepted, Date 2026-11-23)

- Context: spec §6 and §8 size limits; hop had no lint or size check (`docs/audit/hop-reuse.md` §6).
- Options: single crate vs workspace; size check by an external tool (tokei, plus a wrapper) vs a small script; function length by clippy vs the script.
- Decision:
  - Workspace members and lib names from the roadmap; release profile and lints at the root; `too_many_lines` enabled in `[workspace.lints.clippy]` because it is pedantic.
  - `#![forbid(unsafe_code)]` per crate, not as a workspace lint, because `cue-coreaudio` must allow it (or `deny` in `src-tauri` if Task 4 Step 9 hit the macro case: say which).
  - What counts as test for `check:size`: files under a `tests/` directory, `*.test.ts(x)`, and in `.rs` everything from the first line that is only `#[cfg(test)]`. Files come from `git ls-files --cached --others --exclude-standard`.
  - Function length applies to test functions too (clippy cannot skip them); a long test gets `#[allow(clippy::too_many_lines)]` with a reason.
  - Single instance uses hop's `flock` lock, not `tauri-plugin-single-instance` (a socket in shared `/tmp` can be squatted).
- Consequences: the `#[cfg(test)]` cut is a known loophole for code placed after it; review catches it.

Add the row `| [0005](0005-cargo-workspace-and-size-limits.md) | Cargo workspace layout and size limits | Accepted |` to the index.

- [ ] **Step 2: Add the changeset**

Run: `pnpm changeset` (choose `minor`). Text: "cue starts in the menu bar with a Quit item and writes a log to ~/Library/Logs/com.cef62.cue/cue.log."
Expected: one new `.changeset/*.md` file.

- [ ] **Step 3: Run the quality gate and report it as a table**

Run: `pnpm build && pnpm test && pnpm check:size && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check && pnpm build:app`
Expected: every command exits 0; `target/aarch64-apple-darwin/release/bundle/macos/cue.app` exists.

- [ ] **Step 4: Check the bundle**

```bash
plutil -p target/aarch64-apple-darwin/release/bundle/macos/cue.app/Contents/Info.plist | grep -E "LSUIElement|LSMinimumSystemVersion|CFBundleIdentifier"
codesign -dv target/aarch64-apple-darwin/release/bundle/macos/cue.app 2>&1 | grep Signature
open target/aarch64-apple-darwin/release/bundle/macos/cue.app; open -n target/aarch64-apple-darwin/release/bundle/macos/cue.app
grep "another cue is running" ~/Library/Logs/com.cef62.cue/cue.log
```
Expected: `"LSUIElement" => true`, `"LSMinimumSystemVersion" => "14.0"`, `"CFBundleIdentifier" => "com.cef62.cue"`; `Signature=adhoc`; one tray icon only; the grep finds the second launch's line. Quit from the tray.

- [ ] **Step 5: Commit**

```bash
git add docs/adr .changeset
git commit -m "docs: ADR 0005 workspace layout and size limits"
```

---

## Self-review

1. **Spec coverage:** §3 platform (Tasks 4, 7), §5.11 logging (Task 5; Settings toggle, badge and "Show log in Finder" are M4), §6 workspace and `forbid` (Tasks 3, 4), §8 repo setup (Tasks 1, 2, 6; `release-notes.mjs`, `release.yml`, `build-release.yml`, updater and owner prerequisites are M5). M1 roadmap deliverable is fully covered.
2. **Step scan:** every code step names its file, signature and pinned values; bodies appear only for the tray glyph (exact asset) and tests.
3. **Type consistency:** `hold_instance_lock(dir: &Path) -> bool`, `log_level`, `allowed`, `apply`, `plugin`, `forwardConsole`, `format`, `LogSink` are used with the same names in Tasks 4, 5 and 7.
4. **Review Focus:** each of the five lines has its test in Task 2, 4 or 5.
5. **Proportion:** the plan is shorter than the spec; code blocks are tests and one asset.
