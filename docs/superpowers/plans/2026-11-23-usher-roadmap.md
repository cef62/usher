# usher roadmap

- Spec: `docs/superpowers/specs/2026-11-23-usher-design.md` (approved 2026-11-23)
- Reference repo: `/Volumes/Dev/matteo/hop`

There is one plan per milestone. Each milestone ends with working, tested software and is merged as
one or more PRs, following the hop workflow (feature branch, quality gate, adversarial review).

| # | Milestone | Plan | Deliverable | Depends on |
|---|---|---|---|---|
| M1 | Foundation | `2026-11-23-usher-m1-foundation.md` | The repo mirrors hop. The Cargo workspace builds. The Tauri app launches as an agent app with a tray icon and a Quit item, and writes logs, with debug level from `USHER_LOG`. CI runs build, test, clippy, fmt and the size check. | none |
| M2 | Core domain | `2026-11-23-usher-m2-core.md` | `usher-core` covers everything in spec §5 and is fully unit-tested: the model, `desired`, `reconcile` with `Reason`s, the loop guard, the new-device and first-launch seeding, the alias registry, forget, and state serde. | M1 workspace |
| M3 | Audio engine | (later) | `usher-coreaudio` and the engine thread. The JSON store is wired in. The app switches devices for real with no panel: the tray menu has the auto-switch toggle. Also coreaudiod rebind and loop-guard notifications. Usable day to day. | M1, M2 |
| M4 | Panel and settings | (later) | The tray-anchored panel (React, dnd-kit, keyboard), the Settings window (login, debug toggle, show log), and the icon states. | M3 |
| M5 | Updates and release | (later) | The updater, What's new and the release workflows from hop, plus the owner prerequisites (repo, key, secrets). First release. | M4 |

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
