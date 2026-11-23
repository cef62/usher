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
