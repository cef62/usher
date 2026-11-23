# 0005. Cargo workspace layout and size limits

- Status: Accepted
- Date: 2026-11-23

## Context

The roadmap splits usher into a pure core, a CoreAudio adapter and the Tauri shell. The spec (§6 and
§8) sets size limits for files and functions. hop had no lint or size check
(`docs/audit/hop-reuse.md` §6), so nothing stopped files from growing.

## Options

1. **Layout.**
   - A single crate is simpler, but it cannot keep `unsafe` out of the core.
   - A workspace lets each crate carry its own `unsafe` policy.
2. **Size check.**
   - An external tool (tokei, plus a wrapper) adds a dependency and still needs glue to skip tests.
   - A small script in the repo does exactly the counting we need.
3. **Function length.**
   - clippy `too_many_lines` runs in the normal lint step.
   - The script could do it too, but would need a Rust parser.

## Decision

- A workspace with the members and lib names from the roadmap. The release profile and lints live in
  the root `Cargo.toml`. `too_many_lines` is enabled in `[workspace.lints.clippy]` because it is
  pedantic and off by default.
- `#![forbid(unsafe_code)]` is set per crate, not as a workspace lint, because `usher-coreaudio`
  must allow `unsafe`. It compiled fine in `src-tauri`, so no `deny` fallback was needed.
- `check:size` counts test code separately. A test file is a file under a `tests/` directory, or
  `*.test.ts(x)`. In `.rs` files, everything from the first line that is only `#[cfg(test)]` counts
  as test. Files come from `git ls-files --cached --others --exclude-standard`.
- Function length applies to test functions too, because clippy cannot skip them. A long test gets
  `#[allow(clippy::too_many_lines)]` with a reason.
- Single instance uses hop's `flock` lock (`src-tauri/src/instance.rs`), not
  `tauri-plugin-single-instance`. A socket in shared `/tmp` can be squatted.
- Logging details that shaped the shell: `logging::apply(false)` runs in `run()` right after
  `.build()`, not in `setup`, because tao logs TRACE lines before `setup` runs and the plugin sets
  the max level to Trace during `build()`. `RotationStrategy::KeepSome(4)` keeps 4 rotated files
  plus the active one, the 5 files the spec asks for.

## Consequences

- Limits are enforced by `pnpm check:size` and clippy, with no new dependency.
- The `#[cfg(test)]` cut is a known loophole: code placed after it counts as test. Review catches it.
- `unsafe` is confined to `usher-coreaudio`, and the compiler enforces it elsewhere.
