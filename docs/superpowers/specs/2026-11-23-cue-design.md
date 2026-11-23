# cue: design

- Date: 2026-11-23
- Status: Draft, awaiting owner approval
- Repo: `cef62/cue` · bundle id `com.cef62.cue` · working dir `/Volumes/Dev/matteo/macos-audio-priority`

Marks used below:

- **(owner)**: a decision the owner made.
- **(call)**: a decision made while writing this design. The owner can override it.

## 1. Intent

cue is a macOS menu-bar app. It keeps the default output and input audio devices on the owner's
preferred device: the highest-ranked device in a priority list that is currently connected.

The owner uses camguillory/Audio-Priority-Bar day to day (`docs/audit/`). That app works, but it has
two problems:

- It has grown to about 20k LOC, with a 2,200-line app class.
- It relies on fragile heuristics to guess who changed a device.

cue is a from-scratch rewrite with three aims:

- Do the core job only.
- Use a strict, predictable switching rule.
- Keep a codebase of small, well-tested units that follows the owner's other apps (velo, hop).

## 2. Goals, non-goals, anti-goals

**Goals (v1, the "keep" set) (owner)**

1. One priority list for output and one for input. Auto-switch to the top connected device in each.
2. Device identity that survives UID changes on USB port moves and display reconnects.
3. A persistent store of remembered devices. Disconnected devices stay listed, and can be forgotten.
4. Drag to reorder, with Move Up and Move Down as the keyboard and accessibility alternative.
5. An auto-switch on/off toggle. The off state is visible on the menu-bar icon by default.
6. Re-attach the CoreAudio listeners after coreaudiod restarts.
7. Keep the chosen mic when macOS or AirPods take the default input. Under the strict rule this
   happens without extra code (§5.3).
8. A tray-anchored panel that does not take focus.
9. Launch at login.
10. Same repo setup, agent config, CI, release and updater as hop (owner, ADR 0001, ADR 0002).
11. Hidden-device awareness: the active device is always shown, even if it is hidden or flagged
    "never use".

**Non-goals (v1):**

- Volume and mute controls.
- Separate speaker and headphone lists.
- Profiles and Focus modes.
- Shortcuts, URL scheme and global hotkeys.
- Bluetooth connect actions and battery levels.
- Jabra and other vendor HID features.
- Switch notices and history.
- A diagnostics UI.
- A docs site.
- Intel builds.

**Anti-goals (failure even if it works):**

- Any source file over 300 lines, or any function over 40 lines, outside tests.
- Logic that can only be tested by running the app.
- Heuristics that guess whether the user made a change (ADR 0003).
- Silent update installs (ADR 0002).
- Feature creep beyond the keep set without a new ADR.

## 3. Constraints

- macOS on Apple Silicon only (`aarch64-apple-darwin`), like hop. Minimum macOS 14 **(call)**.
- The app is ad-hoc signed and not notarized (ADR 0002).
- CoreAudio does not report who made a change (ADR 0003).
- `AudioObjectID`s are recycled, so nothing is keyed or persisted by them.

## 4. Approaches

The stack was chosen between Tauri 2 (picked), Swift with Sparkle, and pure Rust (ADR 0001).

The switching rule was chosen between strict (picked), kept pick, and topology-only (ADR 0003).

## 5. Behaviour

### 5.1 Roles and lists

- There are two roles, `Output` and `Input`.
- A device belongs to a role when it has at least one stream in that scope. A headset therefore
  appears in both lists, and each list keeps its own entry and rank.
- Each role has an ordered list of `DeviceKey`s and a set of keys flagged "never use".

### 5.2 Desired default (the whole policy)

```
desired(role) = first key in order(role) that is
                connected ∧ alive ∧ has streams in role ∧ ¬never_use(role)
```

Rules for applying it:

- If no device qualifies, cue does nothing and leaves the macOS choice in place.
- When auto-switch is on and `desired != current`, cue sets the default.
- For `Output`, cue also sets `kAudioHardwarePropertyDefaultSystemOutputDevice`, so alerts follow
  the output **(call; fixes fork bug 11)**.
- cue checks the status of `AudioObjectSetPropertyData`. A failure is logged and shown as an error
  line in the panel. cue does not retry within the same event.

### 5.3 When it runs

cue runs `reconcile()` after any of these events, coalesced over 150 ms **(call)**:

- launch;
- device list changed;
- default input or output changed;
- coreaudiod restarted (after rebind);
- prefs changed (reorder, promote, toggle, never use, forget).

Our own write fires a default-changed event. The next reconcile finds `desired == current` and does
nothing, so no echo tracking is needed.

When AirPods take the mic on connect, the event fires and cue reverts it to the top-ranked mic. That
is goal 7.

### 5.4 Loop guard (ADR 0003)

- cue counts its own corrective writes per role in a sliding 10 s window, using an injected clock.
- On the 3rd write in that window it does three things:
  - turns auto-switch off and persists that;
  - switches the icon to its off state;
  - posts a notification: "cue paused auto-switch: <device> kept taking <role>".
- The count resets when the user turns auto-switch back on.
- The state also stores `paused_reason`, which the panel shows as a persistent line until the user
  turns auto-switch back on. This way the reason is visible even if notifications are denied
  **(call)**.
- Notifications use `tauri-plugin-notification`. Permission is requested on the first trip, not at
  launch. If permission is denied, the icon and the panel line are the only signals **(call)**.

### 5.5 New devices

- An unseen device is appended at the bottom of each list for its roles.
- If its transport type is virtual or aggregate, it is flagged "never use" for those roles.
- Devices that macOS marks hidden (`kAudioDevicePropertyIsHidden`) or not alive are not added. They
  are shown only while they are the current default (goal 11).

### 5.6 Identity and aliasing

The key is `DeviceKey = CoreAudio device UID`. It is persisted per role.

An alias layer follows the fork's verified algorithm (`docs/audit/core-audio-and-switching.md` §3.3):

- **Which UIDs get aliased:** serial-less USB devices, confirmed by an IOKit check (not by parsing
  the UID), and displays, keyed by display UUID. The all-zero UUID is never aliased.
- **When:** if a new UID matches a stored family and that family's old UID is absent, the old UID is
  rewritten to the new one in every UID-bearing structure. One registry lists those structures.
- **Ambiguity latch:** once two copies of a family have been seen connected at the same time, that
  family is never merged again.
- **Bluetooth:** exact UID only in v1. There is no precedent for anything else, and it is noted as a
  known limit.

### 5.7 Persistence

- One JSON file at `~/Library/Application Support/com.cef62.cue/state.json` **(call)**, with
  `schema_version: 1`.
- The file holds:
  - `auto_switch`;
  - per role: `order` and `never_use`;
  - `devices` (key → name, transport, roles, last_seen, alias family);
  - `ambiguous_families`.
- Writes are atomic: a temp file followed by a rename. A corrupt file is moved aside and cue starts
  fresh. Unknown keys are preserved. All of this follows hop's `settings.rs` pattern.
- Writes are debounced at 500 ms and flushed on quit. `last_seen` updates only on disconnect, and
  at launch for connected devices.
- Reordering rewrites the full order list from the panel. The panel always shows the full list,
  disconnected devices included, so upstream bug #25 cannot happen.

### 5.8 Forget

- Only disconnected devices can be forgotten. The UI blocks forgetting a connected one.
- Forget removes the key from every UID-bearing structure, using the same registry as aliasing.
- The action is confirmed in the panel.

### 5.9 coreaudiod restart

- cue listens for `kAudioHardwarePropertyServiceRestarted`.
- On that event it removes all listeners, re-adds them and re-enumerates.
- If a step fails, it retries after 1, 2, 4, 8 and 16 s, then every 60 s forever (fixes fork bug 2).

### 5.10 First launch

When no state file exists **(call)**, cue seeds the lists as follows:

- The **current macOS default** for each role goes first.
- The other devices follow in enumeration order. Virtual and aggregate devices are flagged
  "never use".
- Auto-switch starts **on**.

Because the current default is ranked first, the first reconcile does nothing. cue never switches
your audio just because it was launched.

## 6. Architecture

See `docs/architecture/overview.md` for the diagrams. The Cargo workspace has three crates and a
React frontend **(call)**:

| Unit | Contents | Depends on |
|---|---|---|
| `crates/cue-core` | Pure domain: `Device`, `Role`, `DeviceKey`, `Prefs`, `desired()`, `reconcile()` returning `Vec<Action>`, the loop guard, the alias and forget registry, state (de)serialisation. No I/O, no clock (injected). | serde |
| `crates/cue-coreaudio` | The only `unsafe`. Implements the `AudioSystem` trait: enumerate, read default, set default, subscribe to events, IOKit serial check. Converts CoreAudio callbacks into `Event`s on an mpsc channel. | objc2-core-audio 0.3.x, objc2-io-kit |
| `src-tauri` (app `cue`) | The engine thread owns state: it receives `Event`s, debounces, calls core, applies `Action`s through `AudioSystem`, persists, and emits `state-changed` to the UI. Also the tray, the panel window, settings, login, the updater and What's new (adapted from hop). | tauri 2, core, coreaudio |
| `src/` (React 19) | Panel UI: two lists, drag (dnd-kit **(call)**), row menu, toggle. Settings and What's new windows. Plain CSS variables, as in hop. | |

Rules:

- Only `cue-coreaudio` may contain `unsafe`. The other crates set `#![forbid(unsafe_code)]`.
- `cue-core` does not depend on tauri or objc2.
- One engine thread holds all mutable state, so there are no locks in the domain code. CoreAudio
  callbacks only send `Event`s through the channel.
- The UI never mutates state directly. It sends commands (`reorder`, `promote`, `set_never_use`,
  `forget`, `set_auto_switch`), and the engine replies with a full `PanelState` snapshot.

## 7. UI

### 7.1 Panel

The panel is 360 pt wide. It sits under the tray icon, uses a non-activating window, and closes on
an outside click, Escape, or a second click on the icon.

```
+----------------------------------------------+
| Auto-switch                          (o===)  |
|  Keeps the top connected device active.      |
|----------------------------------------------|
| Output                                       |
|  1  ● MacBook Pro Speakers         [active]  |  click row = move to top
|  2  ● Studio Display               ...       |  number -> drag handle on hover
|  3  ○ AirPods Pro    last seen 2 d ...       |  dimmed = disconnected
|  4  ○ BlackHole 2ch  never use     ...       |  struck through
|----------------------------------------------|
| Input                                        |
|  1  ● Shure MV7                    [active]  |
|  2  ● MacBook Pro Microphone       ...       |
|----------------------------------------------|
| ⚠ Could not set output: <reason>              |  only on error
| Update to cue 1.2.0…                          |  only when pending (hop)
| Settings…                              Quit  |
+----------------------------------------------+
```

The `...` menu holds Move up, Move down, Never use / Use, and Forget (disconnected rows only).

**Focus:** "non-activating" means the panel does not activate the app or take focus from the
frontmost app's windows. While open, the panel becomes the key window so that it receives keys.
On close it gives focus back, following hop's `give_back` **(call)**.

**Keyboard:**

- Up and Down move focus between rows.
- Return promotes the focused row.
- Alt+Up and Alt+Down move the focused row.
- Escape closes the panel.

### 7.2 Icon

The icon has two template glyphs **(call)**: on (speaker) and off (speaker with a slash). There are
no other states in v1.

### 7.3 Settings window

The settings window has one pane:

- Open at login, showing the approval state.
- Check for updates daily, plus a "Check now" button.
- Version.

## 8. Repo setup (mirrors hop)

**Copied from hop, with names changed:**

- `AGENTS.md`, `CLAUDE.md` (`@AGENTS.md`) and `CONTRIBUTING.md` (quality gate, adversarial review,
  merge rules).
- `.claude/settings.json`.
- The agent rule dirs: `.cursor`, `.windsurf`, `.clinerules`, `.opencode`, and
  `.github/copilot-instructions.md`.
- `.changeset/`, `scripts/sync-version.mjs` and `scripts/release-notes.mjs`, with its test.
- The workflows `ci.yml`, `release.yml` and `build-release.yml`. The pages dispatch is dropped
  because there is no site.
- The MIT LICENSE and the `.gitignore`.

**Release prerequisites, run by the owner (call):**

The plan includes these steps, written as commands for the owner to run:

- create the GitHub repo `cef62/cue`;
- generate the minisign key pair (`~/.tauri/cue.key` and `cue.password`);
- add the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets.

Until those steps are done:

- `tauri.conf.json` holds a placeholder pubkey;
- the pinned-endpoint test uses `https://github.com/cef62/cue/releases/latest/download/latest.json`;
- the release workflows are built and run last.

**Additions beyond hop (owner):**

- `docs/adr/`, `docs/architecture/` (Mermaid) and `docs/audit/`.
- Size limits, enforced in CI and in the quality gate:
  - `clippy.toml` with `too-many-lines-threshold = 40`, and `cargo clippy --all-targets -- -D warnings`;
  - `cargo fmt --check`;
  - `scripts/check-size.mjs`, which fails on any non-test `.rs`, `.ts` or `.tsx` file over 300 lines.
    Its test is `scripts/check-size.test.ts`.
  - TypeScript runs in `strict` mode. No ESLint, matching hop **(call)**.

## 9. Testing

- **cue-core:** unit tests for every rule above, using a fake device set and a fake clock. The
  scenarios are ported from the fork (`docs/audit/core-audio-and-switching.md` §5):
  - the top device connects or leaves;
  - AirPods take the input on connect;
  - an echo of our own write;
  - the loop guard trips and resets;
  - new device placement and virtual devices flagged "never use";
  - alias merge, the ambiguity latch and the zero UUID;
  - forget across all structures;
  - state round-trip, corrupt-file recovery and unknown-key preservation.
- **Engine (`src-tauri`):** tested against a `FakeAudioSystem` that implements the trait:
  - event coalescing;
  - applying actions;
  - a failed set leaves an error and no state change;
  - the rebind backoff schedule.
- **cue-coreaudio:** thin. One `#[ignore]` smoke test enumerates real devices, run by hand on a Mac.
- **Frontend:** Vitest for list ordering, drag-to-command mapping, keyboard handling and the
  release-notes parser (from hop).
- **Manual QA checklist** in `docs/qa.md`: real AirPods, a USB interface on two ports, and a display
  reconnect.

## 10. Left to the builder

- Exact module split inside each crate, provided it stays within the size limits.
- Whether the tray-anchored placement goes in a new `tray_panel.rs` or adapts `panel.rs`.
- dnd-kit or another maintained sortable library, if dnd-kit turns out to be a poor fit.
- The wording of captions and notifications.
- Settings window styling (follow hop).
