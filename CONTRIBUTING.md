# Contributing to usher

## Making a change

1. Branch from `main` (`git checkout -b feat/short-name`, or `fix/…`, `docs/…`, `ci/…`). Never commit to `main` directly.
2. Write the failing test first when the change is logic, then the code.
3. Run the quality gate below.
4. **Add a changeset** for anything a user can notice (behaviour, settings, docs that change what the user is told to do): `pnpm changeset`, pick `patch` or `minor`, write one sentence in the past tense from the user's point of view. Commit the generated `.changeset/*.md` with your change.
5. Open a pull request against `main` and get an adversarial review (below). CI must be green.

## Quality gate

Run before every commit that finishes a task and before opening a PR. Report the results as a table (check, status, details).

| # | Check | How |
|---|---|---|
| 1 | Verify | `pnpm build`, `pnpm test`, `pnpm check:size`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` all green; new logic has a test |
| 2 | App build | when `src-tauri/`, `crates/` or `tauri.conf.json` changed: `pnpm build:app` (or at least `cargo build --release`) |
| 3 | Docs | README reflects new behaviour, settings and shortcuts; comments explain why, not what |
| 4 | Staleness | no orphaned files, no unused dependencies in `package.json` or `Cargo.toml`, no docs pointing at removed code |
| 5 | Capabilities | every file in `src-tauri/capabilities/` grants exactly what its window uses |

Dev and installed builds share the identifier and lock on macOS: quit the installed usher before `pnpm tauri dev`.

### Testing updates against a local feed

Development builds never check for updates; release builds read the feed and key from `tauri.conf.json`. To test against a local feed, override them at build time only (a test in `update/mod.rs` keeps the committed config on the signed GitHub feed). Never use `~/.tauri/usher.key` for this: make a throwaway key under `/tmp/usher-test/`.

Both `--config` overrides set `"identifier":"com.cef62.usher.updatetest"`, so the test copy has its own app data, log folder and instance lock and never touches the installed usher's `state.json`. Never toggle Open at login in the test copy: it shares the LaunchAgent name.

1. `pnpm tauri signer generate --ci -p "" -w /tmp/usher-test/test.key`
2. Build the new version, signed with that key: `TAURI_SIGNING_PRIVATE_KEY="$(cat /tmp/usher-test/test.key)" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" pnpm tauri build --bundles app --config "$CONF"` with `CONF` = `{"identifier":"com.cef62.usher.updatetest","version":"0.0.2","bundle":{"createUpdaterArtifacts":true},"plugins":{"updater":{"pubkey":"<test.key.pub>","endpoints":["http://127.0.0.1:8765/latest.json"],"dangerousInsecureTransportProtocol":true}}}`. Keep `usher.app.tar.gz` and its `.sig`.
3. Build the current version with the same `plugins` override, the same identifier and `createUpdaterArtifacts: false`, and copy `usher.app` to a folder whose path contains `/target/release/` (so it never touches the installed app's login item).
4. Serve `latest.json` (`{"version":"0.0.2","notes":"…","platforms":{"darwin-aarch64":{"signature":"<.sig contents>","url":"http://127.0.0.1:8765/usher.app.tar.gz"}}}`) and the package on port 8765, quit the installed usher, and run the copy from step 3.

## Pull requests

- PRs are squash-merged, so the title is in conventional-commit form (`feat: …`, `fix: …`, `docs: …`, `ci: …`); it becomes the commit on `main`.
- Body: `## Summary` (what changed and why, as bullets) and `## Test plan` (checkboxes: what was run, ticked; manual checks still to do after merge or release, unticked).
- Name what was not tested and any risk the reviewer must know.

## Adversarial review

Every PR gets an adversarial review before merge, on top of CI. The reviewer's job is to break the change, not to approve it.

- **Reviewer:** a fresh agent (or a human), read-only, on Opus for a complex PR (data integrity, security or encryption, concurrency, or many files) and Sonnet for a simple one (docs, copy, small isolated fixes), given the diff, the PR body and an **attack list** written for this change: the inputs, races, failure paths, platforms and upgrade paths most likely to break it. It reads the code the diff touches, not only the diff.
- **Findings** need a concrete reproduction (input or sequence → wrong result), proven by a test, a script or exact steps. No reproduction, no finding. Severity: **Critical** (data loss, security, broken release), **Major** (wrong behaviour a user will hit), **Minor** (edge case or polish).
- **Verdict:** FAIL if any Critical or Major finding is open, else PASS. The report goes on the PR as a comment.
- **FAIL:** fix each finding with a regression test, then a scoped re-review of the fixes plus another adversarial round. Add an extra round whenever a fix goes beyond its finding.
- **Strike rule:** two failed rounds is the preferred stop (continue only when the open findings are concrete, reproduced and the fix is small and local); four is the hard maximum, then stop and ask the maintainer.
- Minors may be deferred: list them in the PR with the reason.

## Releases

Versions are managed with [Changesets](https://github.com/changesets/changesets): merging a PR with a changeset makes the bot open (or update) a **Version Packages** PR; merging that PR bumps the version, writes `CHANGELOG.md`, tags `vX.Y.Z` and dispatches `build-release.yml`. Never edit versions or create tags by hand.

Releases are **immutable** once published (the repository has release immutability on): a published release's assets and its tag can't be changed, re-uploaded or deleted. `build-release.yml` therefore does all asset work on a draft and publishes only when `latest.json` lists every platform. If a platform job fails, re-run it while the release is still a draft (`gh run rerun <run-id> --failed`). If every build passed but `publish` refused because `latest.json` misses a platform, re-run the whole workflow (`gh run rerun <run-id>`, without `--failed`) so each platform writes its entry again. A broken published release is never fixed in place: ship a new patch version.

The `publish` job runs `scripts/check-feed.mjs` and refuses a feed signed by another key than the one in `tauri.conf.json`; fix the key or the secret, then re-run the workflow. The check reads key IDs only (ADR 0008).

Release builds sign the updater artifacts with the key in the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; the public key is in `src-tauri/tauri.conf.json`. Never lose or commit the private key: installed apps only accept updates signed with it. Never rotate it after the first release: every installed copy would refuse the next update.

`pnpm build:app` is unsigned and for local checks only; the bundle lands at `target/aarch64-apple-darwin/release/bundle/macos/usher.app`. Signed builds exist only in CI.
