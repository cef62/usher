# Architecture Decision Records

One file per decision: `NNNN-short-title.md`, numbered in order, never renumbered.

Status values: `Proposed`, `Accepted`, `Deferred`, `Superseded by NNNN`.
A superseded ADR stays in place; the new ADR links back to it.

## Template

```markdown
# NNNN. Title

- Status: Proposed | Accepted | Deferred | Superseded by NNNN
- Date: YYYY-MM-DD

## Context
What forces the decision. Facts, constraints, evidence (link audits).

## Options
Each option with its trade-offs.

## Decision
What we chose and why.

## Consequences
What gets easier, what gets harder, what we must now do.
```

## Index

| ADR | Title | Status |
|-----|-------|--------|
| [0001](0001-tauri-stack.md) | Tauri 2 (Rust core + React panel) instead of Swift | Accepted |
| [0002](0002-update-mechanism.md) | Reuse hop's update mechanism; defer Developer ID notarization | Accepted |
| [0003](0003-strict-priority-policy.md) | Strict priority: always enforce the top connected device | Accepted |
| [0004](0004-logging-and-debug-mode.md) | Logging and debug mode from day one | Accepted |
| [0005](0005-cargo-workspace-and-size-limits.md) | Cargo workspace layout and size limits | Accepted |
| [0006](0006-engine-threading-and-coreaudio-binding.md) | Engine threading and the CoreAudio binding | Accepted |
| [0007](0007-panel-window-and-login-item.md) | Panel window, login item and command replies | Accepted |
| [0008](0008-updater-settings-and-release-guard.md) | Updater setting, release guard and install restart | Accepted |
| [0009](0009-reconcile-waits-for-usb-verdicts.md) | Reconcile waits for unresolved USB verdicts (amends 0006) | Accepted |
| [0010](0010-docs-site.md) | A docs site on GitHub Pages (departs from the v1 non-goal) | Accepted |
