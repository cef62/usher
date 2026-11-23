# 0002. Reuse hop's update mechanism; defer Developer ID notarization

- Status: Accepted (Developer ID part: Deferred)
- Date: 2026-11-23

## Context

The owner asked for the same update mechanism as velo and hop, unless a better one exists.

**Hop's mechanism:**

- Uses `tauri-plugin-updater` with a minisign key pair. The public key is in `tauri.conf.json`. The
  private key is stored in the `TAURI_SIGNING_PRIVATE_KEY` secret.
- The update manifest is `latest.json` on the latest GitHub Release, with `requireSignedVersion: true`.
- It checks 30 seconds after launch, then every 24 hours. Users can turn the check off in Settings.
- It never installs silently. A pending update shows in the panel and in the tray menu. The user
  installs it from the What's-new window, which displays the CHANGELOG notes carried in `latest.json`.
- The app is ad-hoc signed (`signingIdentity: "-"`) and not notarized. On first launch, users clear
  quarantine or click "Open Anyway".

**The fork's mechanism:**

- Uses Sparkle with EdDSA signing and `SUAutomaticallyUpdate=true`, so updates install silently.
- It is also self-signed and not notarized.

## Options

1. **Hop's mechanism as-is.**
   - Updates are signed, and installs need user consent.
   - The release pipeline is shared with the owner's other apps.
2. **Hop's mechanism plus an Apple Developer ID with notarization.**
   - Removes the Gatekeeper workaround.
   - Adds a second trust anchor (Apple) on top of the minisign key.
   - Costs $99 a year and needs certificate and notarization secrets in CI.
3. **Silent auto-install, like the fork.**
   - Rejected. A compromised GitHub account or key would push code to users with no prompt.

## Decision

Option 1 now. Option 2 is deferred until the app is distributed beyond the owner. Revisit it at the
same time for velo and hop, so all three apps share one signing setup.

## Consequences

- We copy `update.rs`, `whatsnew.rs`, the What's-new window, `scripts/release-notes.mjs` and the
  release workflows from hop. We rename the endpoint and key file names.
- The app gets its own minisign key pair (`~/.tauri/<app>.key`) and its own repository secrets.
- The test that pins the endpoint, `requireSignedVersion` and secure transport is carried over.
