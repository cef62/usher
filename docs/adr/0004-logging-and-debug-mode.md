# 0004. Logging and debug mode from day one

- Status: Accepted
- Date: 2026-11-23

## Context

The owner wants a log system and a debug mode from the first commit. Audio switching bugs depend on
the user's hardware and on timing, so they cannot be reproduced on a developer's machine. The only
evidence is what the app recorded. The fork added debug logging late (`Redactor.swift`, Copy
Diagnostics), after users had already filed reports with no data in them.

## Options

1. **`log` facade with `tauri-plugin-log`.**
   - One dependency, which also forwards the web view's console.
   - File rotation, and the level can change at runtime.
2. **`tracing` with a file appender.**
   - Gives richer spans.
   - Needs more setup, and the web view console is not forwarded for us.
3. **Apple unified logging (`os_log`).**
   - Native, and visible in Console.app.
   - Needs FFI, and it is harder for users to export and attach to a report.

## Decision

Option 1. The format and levels are in design §5.11.

The pure core logs nothing. `reconcile()` returns structured `Reason`s, which the engine logs. This
keeps the core free of I/O and makes "why did it pick X" testable.

Debug mode is a persisted Settings toggle. It also turns on with `USHER_LOG=debug`, and debug builds
default to it. While it is on, the panel shows a badge.

## Consequences

- Every decision shows up in the log with its reasons.
- `Reason` is part of the core API, and it has its own tests.
- Logs may contain device names. They stay local, and redacted diagnostics are deferred.
