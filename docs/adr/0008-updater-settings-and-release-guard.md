# 0008. Updater setting, release guard and install restart

- Status: Accepted
- Date: 2026-11-23

## Context

M5 reuses hop's update mechanism (ADR 0002). Porting it to usher raises six points where the code
departs from the spec or from hop. A published GitHub release is immutable, so a release with a
wrong feed cannot be fixed in place.

## Options

**Where the daily-check setting lives**

1. **A new settings file.** Another file to load, flush and migrate.
2. **A field in `state.json`.** One store, one atomic write path, one read-only mode.

**Local and CI builds**

1. **`build:app` signs updater artifacts everywhere.** Every local build needs the private key.
2. **`build:app` is unsigned; only CI signs.** The private key lives only in repository secrets.

**Feed check before publish**

1. **Trust the build.** A wrong secret ships a feed that no installed copy accepts, and the
   release cannot be changed.
2. **`scripts/check-feed.mjs` in the `publish` job.** It refuses a feed that names the wrong
   version, lacks the macOS arm64 entry, or carries a signature from another key.

**Restart after an install**

1. **hop's `restart` plus a palette wait.** usher has no palette. A direct `restart()` from the
   main thread skips `RunEvent::Exit`, which flushes `state.json`.
2. **`request_restart`.** It runs `RunEvent::Exit` before relaunching. A restart mark lets the new
   process wait for the old one to release the instance lock.

**Tray check result**

1. **A notification.** It needs a permission prompt and is easy to miss.
2. **`tauri-plugin-dialog`.** A native message box shows the result of a manual check.

## Decision

- **`update_check` in `state.json`.** Option 2 for the setting. This adds a field to the spec §5.7
  list. It defaults to on. Unknown keys stay preserved as before.
- **`build:app` is unsigned.** Option 2. `build:app` sets `createUpdaterArtifacts: false`. The local
  bundle lands at `target/aarch64-apple-darwin/release/bundle/macos/usher.app`. Only
  `build-release.yml` builds signed artifacts.
- **`check-feed.mjs` guard.** Option 2. The `publish` job runs it before it publishes the draft.
- **What's new capability.** The `whatsnew` window gets `log:default` only. hop grants none. The
  window forwards its console to the log, like the panel. Install progress arrives over a Channel,
  and its custom commands are guarded in `commands.rs`.
- **`request_restart`.** Option 2, instead of hop's `restart` plus palette wait. At launch, a fresh
  restart mark lengthens the wait for the instance lock. The mark is consumed once.
- **Tray check result.** Option 2. `tauri-plugin-dialog` is a new dependency, approved by the owner.

## Known limits of `check-feed`

- It checks the minisign key ID and the key and signature shape. It does not verify the
  cryptographic signature over the artifact. A wrong key is caught. A corrupt artifact signed by the
  right key is not.
- It compares against the public key in the tagged `tauri.conf.json`. If the key is rotated (a new
  key in both the config and the secrets), the check passes, but every installed copy still trusts
  the old key and refuses the update. Never rotate the key after the first release.

## Consequences

- One more field in `state.json`, and one more dependency (`tauri-plugin-dialog`).
- A release signed by a wrong key never publishes. The fix is the key or the secret, then a re-run
  of the workflow.
- No local build can produce a signed update. Testing an update needs the throwaway-key recipe in
  `CONTRIBUTING.md`.
- The update install path is proven only by hand (`docs/qa.md` rows 27 to 32), because it needs a
  real signed feed. The update-path rows run after 0.1.1 ships.
- Developer ID notarization stays deferred (ADR 0002). First launch of a downloaded release meets
  Gatekeeper.
