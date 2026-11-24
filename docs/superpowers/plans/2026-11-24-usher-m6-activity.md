# usher M6: Activity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** usher tells you when it switches a device (a macOS notification, on by default, with a setting), keeps a session history of what it did and why, and shows a diagnostics report that **Copy diagnostics** puts on the clipboard, redacted for a public GitHub issue. Ships as 0.2.0.

**Architecture:** A pure `explain` in usher-core turns each reconcile's per-role outcome into an `Entry`, reading the cause from the snapshot diff; the merged event trigger only tells `Command`, `Launch` and `Rebind` apart (ADR 0003: no guessing from event order). The engine tracks the trigger, calls the new `Shell::activity(&Entry)` per entry and `Shell::notice(&str)` at most once per reconcile, through a per-role 10 s `NoticeGate`. The shell keeps the last 200 entries in an `ActivityLog` next to `LastView` and emits each one to the Activity window. Diagnostics are a new engine query (`Event::Diagnostics` with its own reply channel); a pure renderer in usher-core redacts the report, and the Activity window shows exactly the text that Copy puts on the clipboard.

**Tech Stack:** existing only: `tauri-plugin-notification` (already used for the pause), `navigator.clipboard` (no clipboard plugin), React 19, Vitest with happy-dom. No new dependency.

**Spec:** `docs/superpowers/specs/2026-11-24-usher-v2-design.md` (§2 goals, non-goals, anti-goals; §3; §5 M6; §8; §9 ADR 0011; §10; §11 release 0.2.0). The v1 spec `docs/superpowers/specs/2026-11-23-usher-design.md` still holds (§5.3 error rule, §5.4 loop guard and pause notification, §5.11 logging and privacy). This plan applies the PR #16 review corrections to §5.1, §5.2 and §5.4 (listed in Decisions 1–4). PR #16 merged those corrections into the spec.

**Branch:** `feat/m6-activity` (exists). One PR.

## Global Constraints

- Non-test `.rs`, `.ts`, `.tsx` files ≤300 lines; functions ≤40 lines (`clippy::too_many_lines`). `src-tauri/src/engine/mod.rs` is at 248 lines and `panel/mod.rs` at 226 non-test lines: new engine logic goes in `engine/activity.rs`, not `mod.rs`. `Engine::run_reconcile` is near 40 lines: split it before adding to it (Task 3).
- `#![forbid(unsafe_code)]` stays in `usher-core` and `src-tauri`. `usher-core` stays pure (serde only, no clock, no I/O, no logging).
- Only the engine thread writes CoreAudio state (ADR 0006). Nothing in M6 writes CoreAudio.
- Notification permission calls never run on the engine thread (they can block on a system prompt).
- `State.notify_switches: bool`, serde default `true`, no `SCHEMA_VERSION` bump (as `update_check`, ADR 0008).
- History: last **200** entries, newest first, memory only. No persisted history.
- Labels and lists: window label `activity`, page `activity.html`, event `activity`, `ACTIVITY_COMMANDS = ["get_activity", "get_diagnostics"]`.
- Copy (verbatim): `Output: <from> → <to>` (`Input: …`); `Couldn't switch output to <to>: <error>` (`input`); `Auto-switch on`; the v1 pause text `usher paused auto-switch: <name> kept taking <role>`; notification title `usher`; Settings row `Notify when usher switches` (General) and button `Open Activity…` (Troubleshooting); panel footer and tray item `Activity…`; button `Copy diagnostics`.
- Log lines: `info!("activity: {text}")` or `info!("activity: {text} ({why})")`, one per entry; `info!("switch notices {on|off}")`; `info!("notifications are not allowed; notice not shown")`.
- Window labels after M6: `panel`, `settings`, `whatsnew`, `activity`. Each window invokes only its own list (`commands::allowed`).
- Quality gate (CONTRIBUTING): `pnpm build`, `pnpm test`, `pnpm check:size`, `pnpm docs:build`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`; app build `pnpm build:app`. Report it as a table.
- One `minor` changeset (Task 8). Site Usage page, README and screenshots move with the UI (Task 8).

## Owner prerequisites

None. Release 0.2.0 follows the usual Version Packages PR after the M6 merge, on the owner's word.

## Review Focus

1. **An unplug fires devices-changed and default-changed in one window.** Expected: the entry reads `<name> disconnected`, never `took output; usher put it back`. Pinned in Task 1 (`unplug_with_a_fallback_default_is_disconnected`) and Task 3 (`unplug_reads_as_disconnected_not_corrected`).
2. **A flapping Bluetooth headset** connects and drops every few seconds. Expected: at most one notice per role per 10 s; history keeps every entry; the loop guard still trips as in v1. Pinned in Task 1 (`gate_allows_one_notice_per_role_per_10_s`) and Task 4 (`notices_are_capped_per_role`).
3. **A UID shape usher has never seen** (a display, AirPlay, a virtual driver, a vendor UID with a serial) lands in Copy diagnostics. Expected: it renders as `uid#<n>`, never raw. Pinned in Task 2 (`unknown_uid_shapes_become_indexed_placeholders`).
4. **The Activity window opens while entries arrive.** Expected: no line shows twice, none is missing, newest stays on top. Pinned in Task 7 (`merge drops duplicates by seq and keeps 200`).
5. **The panel footer with Debug, Update to usher X…, Activity…, Settings… and Quit** at 360 pt. Expected: one row, nothing clipped. Pinned in Task 7 (`footer order` test) and QA row 41 (CSS cannot be measured in happy-dom).

## Decisions the spec leaves open

1. **`why` precedence (PR #16 correction to §5.1).** `explain` derives `why` in this order, first match wins:
   1. trigger `Launch` or `Rebind`, or `prev` is `None` → `Why::Launch` (`Why::Rebind` for `Rebind`);
   2. the role's previous current (`prev.current`) is no longer connected in `snap` → `Disconnected(prev current)`;
   3. `desired` was not connected in `prev` → `Connected(desired)`;
   4. trigger `Command` → `Ranked`;
   5. `snap.current` is `Some(taker)` → `Corrected(taker)`;
   6. `snap.current` is `None` (macOS has no default, nothing above explains it) → `Disconnected(prev current)` when `prev.current` is `Some`, else `Connected(desired)`.
2. **Who notifies (§5.2).** A notice line exists only when the trigger is `DevicesChanged` or `DefaultChanged` (`Trigger::notifies`) and the entry is `Failed`, or `Switched` with `why` `Connected`, `Disconnected` or `Corrected`. `Launch`, `Rebind` and `Command` reconciles are history only. The engine asks the gate only when `state.notify_switches && state.auto_switch` after the reconcile, so a reconcile that tripped the guard makes no switch notice.
3. **Notice cap (PR #16 correction).** `NoticeGate` allows one notice line per role per `NOTICE_GAP_MS = 10_000` (wall clock); a capped line goes to history only. `Paused` keeps its own path, `Shell::paused` (v1), and always notifies.
4. **Trigger merge (§5.1 precedence, extended).** `Command > Rebind > Launch > DefaultChanged > DevicesChanged`. Two `DefaultChanged` keep the first. `Engine::start` sets `Launch`; `begin_rebind` sets `Rebind`; `schedule_reconcile` merges after its rebind early-return. The trigger is taken only when a scan is acted on (not on a USB deferral or a failed scan); a reconcile with none (a USB recheck) uses `DevicesChanged`.
5. **Two shell methods, not one.** §5.3 names `Shell::activity(&Entry)`; one notice per reconcile needs the whole reconcile. Kept `activity(&Entry)` (history, one call per entry) and added `notice(&str)` (one call per reconcile, body already joined). `Shell::paused` stays as the pause's notification.
6. **Names, not keys, in `Entry` and `Why`.** Text needs names and the history outlives the devices, so `Why::Connected(String)` etc. hold the display name (snapshot, then previous snapshot, then the key). `from` is `snap.current` (the device usher moved the role away from); `None` renders `Output: <to>`.
7. **`Wrote`, the per-role outcome `explain` takes:** `Nothing | Set | Failed(String) | Paused(PausedReason)`. The core `Outcome` name is taken by `reconcile`.
8. **Copy left to the builder (§12).** `why` texts: `<name> connected`, `<name> disconnected`, `<taker> took output; usher put it back` (`input`), `changed in usher` (Ranked), `usher started` (Launch), `usher reconnected to the audio system` (Rebind). `Resumed` is produced by the engine when a command turns Auto-switch on (any off→on), not by `explain`.
9. **History lines are rendered in Rust.** `activity::Line { seq: u64, at: u64, text: String, why: Option<String> }` (`at` = wall ms). `seq` lets the page drop a live line it also fetched. The role is the text's first word, so there is no separate role column (layout detail, §12).
10. **Diagnostics query (PR #16 correction).** `Event::Diagnostics(Sender<Diagnostics>)`, its own reply channel (`Reply` carries no payload). It answers before the first scan too (`engine.scanned = false`, no devices): a broken engine is when diagnostics matter. `get_diagnostics` adds `AppInfo` (version, `sw_vers -productVersion` or `unknown`, log path) and returns the rendered text.
11. **Redaction (PR #16 correction to §5.4).** UIDs render only through known-safe shapes: built-in (`BuiltIn` + ASCII alphanumerics) as is; `AppleUSBAudioEngine:<vendor>:<product>:<field4>:<n>` (exactly 5 fields) with field 4 kept only when the device is confirmed serial-less (`serialless_usb`, or a stored `family`), else `<serial>`; a Bluetooth address shape (`XX-XX-XX-XX-XX-XX:output|:input`, hex any case) as `xx-xx-xx-xx-CF-9D:output`. Any other UID is `uid#<n>`, numbered per report in order of first appearance; a family (alias) renders as `AppleUSBAudioEngine:<vendor>:<product>` when it has that 3-field shape, else as its device key's `uid#<n>`. Names drop a leading `<name>'s ` / `<name>’s `, and every whole-word, case-insensitive occurrence of the Mac account's own names (the words of the full name from `id -F`, and the short user name; words under 3 letters ignored), together with a connector word just before it (`di`, `de`, `da`, `du`, `del`, `von`, `van`, `of`): "AirPods Pro di Matteo" → "AirPods Pro" on an Italian Mac (controller call, owner wants public-safe text). The home folder becomes `~` only at a path-segment boundary. The Activity window shows the same redacted text Copy puts on the clipboard.
12. **Activity window.** Built like Settings: `activity.html`, title `usher Activity`, 480×560, min 360×320, resizable, not minimizable or maximizable, centred, Liquid Glass (`glass::apply`, `WINDOW_RADIUS`), overlay title bar with the drag strip. `panel::KEEP_FOCUS` gains `activity`. The `activity` event goes only to this window (`emit_to`). Diagnostics sit in a native `<details>` (collapsed); they load when it opens and on **Refresh**. Copy uses `navigator.clipboard.writeText` on the click; a failure shows `Couldn't copy: <e>`.
13. **IPC lists.** `panel` and `settings` gain `open_activity`; `settings` gains `set_notify`; `activity` gets `ACTIVITY_COMMANDS`. Shared: `open_whatsnew`, `open_activity` (panel and Settings only).
14. **Settings.** `SettingsView.notify_switches` (`notifySwitches`) reads `LastView`, default `true` with no view. `PanelState.notify_switches` carries it (as `update_check`).

---

## File structure

| File | Responsibility | Budget |
|---|---|---|
| `crates/usher-core/src/activity.rs` | `Trigger`, `Why`, `Entry`, `Wrote`, `explain`, entry texts, `NoticeGate` | 170 |
| `crates/usher-core/src/diagnostics.rs` | `Diagnostics` types, `diagnose`, `render_diagnostics` | 150 |
| `crates/usher-core/src/redact.rs` | `Redactor`, `strip_owner`, `tilde` | 110 |
| `crates/usher-core/src/{lib,state,commands,panel,desired}.rs` | modules, `notify_switches`, `SetNotify`, `skip_cause` made `pub(crate)` | +20 |
| `crates/usher-core/tests/{activity,diagnostics}.rs` | scenarios | tests |
| `src-tauri/src/engine/activity.rs` | trigger tracking, `report`, `diagnostics()` | 110 |
| `src-tauri/src/engine/{mod,exec,command,rebind,thread}.rs` | fields, `Event::Diagnostics`, `Wrote` per role, `Resumed`, `EngineHandle::diagnostics` | +40 |
| `src-tauri/src/activity.rs` | `LABEL`, `ActivityLog`, `Line`, `open`, `open_activity`, `get_activity`, `get_diagnostics` | 150 |
| `src-tauri/src/shell.rs` | `activity`, `notice`, shared `show` | +30 |
| `src-tauri/src/{settings,commands,tray,lib}.rs`, `panel/place.rs` | `set_notify`, lists, tray item, handlers, `KEEP_FOCUS` | +30 |
| `src-tauri/capabilities/activity.json` | capability | |
| `src-tauri/tests/{activity,common/mod}.rs` | engine scenarios, `Recorder` calls, `disp()` | tests |
| `activity.html`, `vite.config.ts` | fourth page | |
| `src/activity/{api,lines,Activity,ActivityApp,main}.ts(x)`, `activity.css` + tests | the page | ≤130 each |
| `src/{types,api}.ts`, `src/panel/PanelView.tsx`, `src/settings/Settings.tsx` | `notifySwitches`, calls, footer button, two rows | +20 |
| `site/src/pages/usage.astro`, `site/scripts/screens/{mock.js,capture.mjs}`, `site/src/assets/screens/*` | docs site | |
| `docs/adr/0011-v2-scope.md`, `docs/adr/README.md`, `README.md`, `docs/qa.md`, `docs/architecture/overview.md`, roadmap, `.changeset/*.md` | docs | |

---

### Task 1: Activity entries in the core

**Files:**
- Create: `crates/usher-core/src/activity.rs`, `crates/usher-core/tests/activity.rs`
- Modify: `crates/usher-core/src/lib.rs` (`mod activity; pub use activity::*;`)

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Trigger { Launch, DevicesChanged, DefaultChanged(Role), Command, Rebind }`; `impl Trigger { pub fn merge(self, other: Trigger) -> Trigger; pub fn notifies(self) -> bool }` (Decisions 2, 4).
  - `#[derive(Clone, PartialEq, Debug)] pub enum Why { Connected(String), Disconnected(String), Corrected(String), Ranked, Launch, Rebind }`.
  - `#[derive(Clone, PartialEq, Debug)] pub enum Entry { Switched { role: Role, from: Option<String>, to: String, why: Why }, Failed { role: Role, to: String, error: String }, Paused { reason: PausedReason }, Resumed }`; `impl Entry { pub fn text(&self) -> String; pub fn why_text(&self) -> Option<String>; pub fn role(&self) -> Option<Role> }`.
  - `#[derive(Clone, PartialEq, Debug, Default)] pub enum Wrote { #[default] Nothing, Set, Failed(String), Paused(PausedReason) }`.
  - `pub fn explain(trigger: Trigger, prev: Option<&Snapshot>, snap: &Snapshot, reason: &Reason, wrote: &Wrote) -> Option<Entry>` (Decision 1; `Nothing` → `None`; `to` = name of `reason.desired`).
  - `pub const NOTICE_GAP_MS: u64 = 10_000;` `#[derive(Default, Debug)] pub struct NoticeGate { last: PerRole<Option<Timestamp>> }`; `impl NoticeGate { pub fn body(&mut self, trigger: Trigger, entries: &[Entry], now: Timestamp) -> Option<String> }` — eligible lines (Decision 2) of roles not noticed in the last 10 s, joined with `\n`; records `now` for each role it includes; `None` when no line.

- [ ] **Step 1: Write the failing tests** (`tests/activity.rs`; fixtures `spk()`, `pods()`, `disp()` snapshots built locally, names `MacBook Pro Speakers`, `AirPods`, `Studio Display`)

```rust
#[test] fn triggers_merge_by_precedence() {
    use Trigger::*;
    assert_eq!(DevicesChanged.merge(DefaultChanged(Role::Input)), DefaultChanged(Role::Input));
    assert_eq!(DefaultChanged(Role::Output).merge(DevicesChanged), DefaultChanged(Role::Output));
    assert_eq!(DefaultChanged(Role::Output).merge(DefaultChanged(Role::Input)), DefaultChanged(Role::Output));
    assert_eq!(Launch.merge(DevicesChanged), Launch);
    assert_eq!(Rebind.merge(Launch), Rebind);
    assert_eq!(Rebind.merge(Command), Command);
    assert!(DevicesChanged.notifies() && DefaultChanged(Role::Output).notifies());
    assert!(!Command.notifies() && !Launch.notifies() && !Rebind.notifies());
}
#[test] fn no_write_makes_no_entry() {
    assert_eq!(explain(Trigger::DevicesChanged, Some(&prev), &snap, &reason, &Wrote::Nothing), None);
}
#[test] fn first_reconcile_is_launch_whatever_the_trigger() {
    // prev None, current spk, desired pods
    let e = explain(Trigger::DevicesChanged, None, &snap, &reason, &Wrote::Set).unwrap();
    assert_eq!(e, Entry::Switched { role: Role::Output, from: Some("MacBook Pro Speakers".into()), to: "AirPods".into(), why: Why::Launch });
    assert_eq!(e.text(), "Output: MacBook Pro Speakers → AirPods");
    assert_eq!(e.why_text().as_deref(), Some("usher started"));
}
#[test] fn rebind_wins_over_the_diff() {
    // prev had pods current; snap lost pods: still Rebind
    assert!(matches!(explain(Trigger::Rebind, Some(&prev), &snap, &reason, &Wrote::Set), Some(Entry::Switched { why: Why::Rebind, .. })));
}
#[test] fn unplug_with_a_fallback_default_is_disconnected() {
    // prev: pods current; snap: pods gone, macOS fell back to spk, desired disp; trigger DefaultChanged(Output)
    let e = explain(Trigger::DefaultChanged(Role::Output), Some(&prev), &snap, &reason, &Wrote::Set).unwrap();
    assert_eq!(e.why_text().as_deref(), Some("AirPods disconnected"));
    assert_eq!(e.text(), "Output: MacBook Pro Speakers → Studio Display");
}
#[test] fn new_desired_device_is_connected() {
    // prev without pods; snap with pods ranked first
    assert_eq!(explain(Trigger::DevicesChanged, Some(&prev), &snap, &reason, &Wrote::Set).unwrap().why_text().as_deref(), Some("AirPods connected"));
}
#[test] fn a_command_reconcile_is_ranked() {
    // same devices before and after, current spk, desired pods
    let e = explain(Trigger::Command, Some(&prev), &snap, &reason, &Wrote::Set).unwrap();
    assert_eq!(e.why_text().as_deref(), Some("changed in usher"));
}
#[test] fn a_taker_is_corrected() {
    // same devices, pods took the input, desired mic
    let e = explain(Trigger::DefaultChanged(Role::Input), Some(&prev), &snap, &reason, &Wrote::Set).unwrap();
    assert_eq!(e.why_text().as_deref(), Some("AirPods took input; usher put it back"));
}
#[test] fn no_current_default_falls_back_to_disconnected_or_connected() { /* Decision 1 step 6, both branches */ }
#[test] fn failed_and_paused_entries() {
    let f = explain(Trigger::DevicesChanged, Some(&prev), &snap, &reason, &Wrote::Failed("set default failed (OSStatus -50)".into())).unwrap();
    assert_eq!(f.text(), "Couldn't switch output to AirPods: set default failed (OSStatus -50)");
    let p = PausedReason { role: Role::Output, key: "AirPods-uid".into(), name: "AirPods".into() };
    assert_eq!(explain(Trigger::DefaultChanged(Role::Output), Some(&prev), &snap, &reason, &Wrote::Paused(p.clone())), Some(Entry::Paused { reason: p.clone() }));
    assert_eq!(Entry::Paused { reason: p }.text(), "usher paused auto-switch: AirPods kept taking output");
    assert_eq!(Entry::Resumed.text(), "Auto-switch on");
    assert_eq!(Entry::Resumed.why_text(), None);
}
#[test] fn no_previous_default_renders_without_from() {
    assert_eq!(Entry::Switched { role: Role::Input, from: None, to: "AirPods".into(), why: Why::Ranked }.text(), "Input: AirPods");
}
#[test] fn one_body_for_both_roles_and_none_for_quiet_entries() {
    let mut g = NoticeGate::default();
    let (out, inp) = (switched(Role::Output, Why::Connected("AirPods".into())), switched(Role::Input, Why::Connected("AirPods".into())));
    assert_eq!(g.body(Trigger::DevicesChanged, &[out, inp], Timestamp(0)).as_deref(), Some("Output: MacBook Pro Speakers → AirPods\nInput: MacBook Pro Microphone → AirPods"));
    let mut g = NoticeGate::default();
    assert_eq!(g.body(Trigger::Command, &[failed(Role::Output)], Timestamp(0)), None, "a user command never notifies");
    assert_eq!(g.body(Trigger::DevicesChanged, &[switched(Role::Output, Why::Ranked), Entry::Resumed], Timestamp(0)), None);
    assert!(g.body(Trigger::DevicesChanged, &[Entry::Paused { reason: p() }], Timestamp(0)).is_none(), "Paused notifies through Shell::paused");
}
#[test] fn gate_allows_one_notice_per_role_per_10_s() {
    let mut g = NoticeGate::default();
    let out = switched(Role::Output, Why::Corrected("AirPods".into()));
    assert!(g.body(Trigger::DefaultChanged(Role::Output), &[out.clone()], Timestamp(1_000)).is_some());
    assert!(g.body(Trigger::DefaultChanged(Role::Output), &[out.clone()], Timestamp(10_999)).is_none());
    assert!(g.body(Trigger::DevicesChanged, &[switched(Role::Input, Why::Connected("AirPods".into()))], Timestamp(10_999)).is_some(), "roles count apart");
    assert!(g.body(Trigger::DefaultChanged(Role::Output), &[out], Timestamp(11_000)).is_some());
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p usher-core --test activity`
Expected: compile errors (`Trigger`, `explain` unknown).

- [ ] **Step 3: Implement `activity.rs`** with the Interfaces above. Name lookup: `snap.device(k)`, then `prev.device(k)`, then the key.

- [ ] **Step 4: Run** `cargo test -p usher-core && cargo clippy -p usher-core --all-targets -- -D warnings`. Expected: pass.

- [ ] **Step 5: Commit** `feat(core): activity entries and the notice gate`

---

### Task 2: Diagnostics and redaction in the core

**Files:**
- Create: `crates/usher-core/src/diagnostics.rs`, `crates/usher-core/src/redact.rs`, `crates/usher-core/tests/diagnostics.rs`
- Modify: `crates/usher-core/src/lib.rs`, `crates/usher-core/src/desired.rs` (`skip_cause` → `pub(crate)`)

**Interfaces:**
- Produces:
  - `#[derive(Clone, Default, PartialEq, Debug)] pub struct AppInfo { pub version: String, pub macos: String, pub log_path: String, pub owner_names: Vec<String> }`.
  - `#[derive(Clone, Copy, Default, PartialEq, Debug)] pub struct EngineStatus { pub scanned: bool, pub subscribed: bool, pub rebinding: Option<u32>, pub read_only: bool, pub usb_unresolved: bool }` (`rebinding` = attempt number while a rebind is pending).
  - `pub struct RankedReport { pub key: DeviceKey, pub name: String, pub skip: Option<Skip>, pub serialless: bool }`; `pub struct RoleReport { pub role: Role, pub desired: Option<DeviceKey>, pub current: Option<DeviceKey>, pub ranked: Vec<RankedReport> }`; `pub struct DeviceReport { pub key: DeviceKey, pub name: String, pub transport: Transport, pub alive: bool, pub hidden: bool, pub serialless_usb: bool, pub family: Option<String> }`; `pub struct Diagnostics { pub app: AppInfo, pub engine: EngineStatus, pub roles: Vec<RoleReport>, pub devices: Vec<DeviceReport>, pub ambiguous_families: Vec<String> }` (all `Clone, PartialEq, Debug`).
  - `pub fn diagnose(state: &State, snap: &Snapshot, engine: EngineStatus) -> Diagnostics` — `app` default; every ranked key with `skip_cause` (not only those above `desired`); `serialless` = snapshot `serialless_usb` or a stored `family`; `devices` = the snapshot's, in enumeration order, with the stored `family`.
  - `#[derive(Default)] pub struct Redactor { .. }`; `impl Redactor { pub fn uid(&mut self, uid: &str, serialless: bool) -> String; pub fn family(&mut self, family: &str, key: &str) -> String }` (Decision 11).
  - `pub fn strip_owner(name: &str, owner_names: &[String]) -> String`; `pub fn tilde(path: &str, home: &str) -> String`.
  - `pub fn render_diagnostics(d: &Diagnostics, home: &str) -> String` — plain text, every name through `strip_owner` with `d.app.owner_names`, every key and family through one `Redactor`, the log path through `tilde`. Shape:

```text
usher 0.2.0 · macOS 26.1 · log ~/Library/Logs/com.cef62.usher/usher.log
engine: subscribed · rebinding: no · read-only store: no · USB verdict unresolved: no

output: desired Studio Display (uid#1), current AirPods Pro (xx-xx-xx-xx-CF-9D:output)
  1. Studio Display (uid#1)
  2. AirPods Pro (xx-xx-xx-xx-CF-9D:output)
  3. Scarlett 2i2 USB (AppleUSBAudioEngine:Focusrite:Scarlett 2i2 USB:<serial>:1) — disconnected
input: …

devices in the last scan:
  AirPods Pro · xx-xx-xx-xx-CF-9D:output · bluetooth · alive · visible · serial-less USB: no · alias: none
ambiguous aliases: none
```

  `rebinding: attempt 3` while one is pending; `no scan yet` replaces the role and device sections when `scanned` is false. Transport uses its `state.json` spelling.

- [ ] **Step 1: Write the failing tests** (`tests/diagnostics.rs`)

```rust
#[test] fn known_uid_shapes_render_safely() {
    let mut r = Redactor::default();
    assert_eq!(r.uid("BuiltInSpeakerDevice", false), "BuiltInSpeakerDevice");
    assert_eq!(r.uid("00-1A-7D-DA-CF-9D:output", false), "xx-xx-xx-xx-CF-9D:output");
    assert_eq!(r.uid("00-1a-7d-da-cf-9d:input", false), "xx-xx-xx-xx-cf-9d:input");
    assert_eq!(r.uid("AppleUSBAudioEngine:Focusrite:Scarlett 2i2 USB:Y8ABC123:1", false),
               "AppleUSBAudioEngine:Focusrite:Scarlett 2i2 USB:<serial>:1");
    assert_eq!(r.uid("AppleUSBAudioEngine:Generic:USB Audio:14200000:1", true),
               "AppleUSBAudioEngine:Generic:USB Audio:14200000:1");
}
#[test] fn unknown_uid_shapes_become_indexed_placeholders() {
    let mut r = Redactor::default();
    let disp = "37D8832A-2D66-02CA-B9F7-8F30A301B230";
    assert_eq!(r.uid(disp, false), "uid#1");
    assert_eq!(r.uid("com.vendor.driver:Matteo-Mac:SN12345", false), "uid#2");
    assert_eq!(r.uid(disp, true), "uid#1", "same key, same index");
    assert_eq!(r.uid("AppleUSBAudioEngine:a:b:c", false), "uid#3", "4 fields is not the USB shape");
    assert_eq!(r.uid("00-1A-7D-DA-CF-9D:headset", false), "uid#4");
    assert_eq!(r.family("37D8832A", disp), "uid#1", "an alias renders as its key");
    assert_eq!(r.family("AppleUSBAudioEngine:Shure:MV7", "AppleUSBAudioEngine:Shure:MV7:14200000:1"), "AppleUSBAudioEngine:Shure:MV7");
}
#[test] fn owner_names_and_home_are_dropped() {
    let ada = vec!["Ada".to_string(), "Lovelace".to_string(), "ada".to_string()];
    assert_eq!(strip_owner("Ada's AirPods Pro", &ada), "AirPods Pro");
    assert_eq!(strip_owner("Ada’s AirPods Pro", &ada), "AirPods Pro");
    assert_eq!(strip_owner("AirPods Pro di Ada", &ada), "AirPods Pro");
    assert_eq!(strip_owner("AirPods von Ada Lovelace", &ada), "AirPods");
    assert_eq!(strip_owner("Adapter Speakers", &ada), "Adapter Speakers", "whole words only");
    assert_eq!(strip_owner("Studio Display Speakers", &[]), "Studio Display Speakers");
    assert_eq!(tilde("/Users/matteo/Library/Logs/com.cef62.usher/usher.log", "/Users/matteo"), "~/Library/Logs/com.cef62.usher/usher.log");
    assert_eq!(tilde("/Users/matteo2/x", "/Users/matteo"), "/Users/matteo2/x");
}
#[test] fn diagnose_reports_every_ranked_skip() {
    // order [pods(disconnected), disp, spk(never use)]: desired disp, and spk still gets Some(Skip::NeverUse)
    let d = diagnose(&state, &snap, EngineStatus { scanned: true, ..Default::default() });
    let skips: Vec<_> = d.roles[0].ranked.iter().map(|r| r.skip).collect();
    assert_eq!(skips, vec![Some(Skip::Disconnected), None, Some(Skip::NeverUse)]);
}
#[test] fn rendered_report_leaks_no_address_serial_owner_or_home() {
    // devices: "Matteo's AirPods Pro" 00-1A-7D-DA-CF-9D:output, Scarlett with serial Y8ABC123, a display UID;
    // app.log_path under /Users/matteo
    let text = render_diagnostics(&d, "/Users/matteo");
    for leak in ["00-1A-7D-DA", "Y8ABC123", "Matteo", "/Users/matteo", "37D8832A"] {
        assert!(!text.contains(leak), "{leak} in\n{text}");
    }
    assert!(text.contains("xx-xx-xx-xx-CF-9D:output") && text.contains("<serial>") && text.contains("uid#1"));
    assert!(text.starts_with("usher 0.2.0 · macOS 26.1 · log ~/Library/Logs/com.cef62.usher/usher.log\n"));
}
#[test] fn report_before_the_first_scan_says_so() {
    let text = render_diagnostics(&diagnose(&State::default(), &empty, EngineStatus::default()), "/Users/u");
    assert!(text.contains("no scan yet") && text.contains("engine: not subscribed"));
}
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -p usher-core --test diagnostics` → compile errors.

- [ ] **Step 3: Implement** `redact.rs` and `diagnostics.rs` (no regex crate: match the shapes by splitting on `:` and `-`).

- [ ] **Step 4: Run** `cargo test -p usher-core && cargo clippy -p usher-core --all-targets -- -D warnings && pnpm check:size`. Expected: pass.

- [ ] **Step 5: Commit** `feat(core): diagnostics report with redaction`

---

### Task 3: Engine trigger tracking, `Shell::activity` and the history

**Files:**
- Create: `src-tauri/src/engine/activity.rs`, `src-tauri/src/activity.rs` (`ActivityLog`, `Line` only in this task), `src-tauri/tests/activity.rs`
- Modify: `src-tauri/src/engine/{mod,exec,command,rebind}.rs`, `src-tauri/src/shell.rs`, `src-tauri/src/lib.rs` (`mod activity;`, manage `ActivityLog`), `src-tauri/tests/common/mod.rs`

**Interfaces:**
- Consumes: Task 1 `Trigger`, `Wrote`, `Entry`, `explain`.
- Produces:
  - `Shell` gains `fn activity(&self, entry: &Entry);`.
  - `Engine` fields `trigger: Option<Trigger>` (start: `Some(Launch)`); `schedule_reconcile(&mut self, now: Instant, trigger: Trigger)` (Decision 4); `on_audio` maps `DevicesChanged`/`DefaultChanged(r)`; `on_command` passes `Command`; `begin_rebind` sets `Some(Rebind)`.
  - `exec.rs`: `execute` returns `PerRole<Wrote>`; `set_one` returns the role's `Wrote` (`Set` or `Failed(e.to_string())`); `PauseAuto` gives `Wrote::Paused(paused_reason)`. Split `run_reconcile` (scan and deferral) from a new `fn act(&mut self, snap: Snapshot, now: Instant)` (seed, reconcile, execute, report, store `snap`, emit), each ≤40 lines.
  - `engine/activity.rs`: `pub(super) fn report(&mut self, trigger: Trigger, prev: Option<&Snapshot>, snap: &Snapshot, reasons: &[Reason], wrote: &PerRole<Wrote>, now: Instant)` — `explain` per reason; per entry the `info!` line and `shell.activity`. (Notices are Task 4.)
  - `command.rs` `announce`: auto-switch off→on also logs `activity: Auto-switch on` and calls `shell.activity(&Entry::Resumed)`.
  - `src-tauri/src/activity.rs`: `pub const HISTORY: usize = 200;` `#[derive(Serialize, Clone, PartialEq, Debug)] #[serde(rename_all = "camelCase")] pub struct Line { pub seq: u64, pub at: u64, pub text: String, pub why: Option<String> }`; `#[derive(Default)] pub struct Log { next: u64, lines: VecDeque<Line> }` with `pub fn push(&mut self, at: u64, entry: &Entry) -> Line` (newest first, `seq` from 1) and `pub fn lines(&self) -> Vec<Line>`; `pub struct ActivityLog(pub Mutex<Log>)`.
  - `TauriShell::activity`: stamp wall ms, push, `app.emit_to(activity::LABEL, "activity", &line)` (warn on error). `LABEL = "activity"` is declared here.
  - Test shell: `Call::Activity(Entry)`; `Recorder::entries() -> Vec<Entry>`; `common::disp()` = `"DisplayUID"`, `Studio Display`, `Transport::Hdmi`, output 2.

- [ ] **Step 1: Write the failing tests** (`src-tauri/tests/activity.rs`; `ranked`/`ask` helpers as in `requests.rs`)

```rust
#[test] fn first_reconcile_with_a_lower_ranked_default_is_launch_history() {
    // state.json ranks pods first; pods connected; current spk; boot
    assert!(matches!(rec.entries()[..], [Entry::Switched { role: Role::Output, why: Why::Launch, .. }]));
}
#[test] fn rebind_that_restores_the_desired_device_is_rebind_history() {
    // pods ranked first and current; ServiceRestarted at 100; world.current.output = spk; tick(1_200)
    assert!(matches!(rec.entries().last(), Some(Entry::Switched { why: Why::Rebind, .. })));
}
#[test] fn unplug_reads_as_disconnected_not_corrected() {
    // order [pods, disp, spk], pods current; remove pods, current = spk; DevicesChanged at 100, DefaultChanged(Output) at 110; tick(300)
    let e = rec.entries().last().cloned().unwrap();
    assert_eq!(e.why_text().as_deref(), Some("AirPods disconnected"));
    assert_eq!(e.text(), "Output: MacBook Pro Speakers → Studio Display");
}
#[test] fn a_connect_is_recorded_with_its_cause() { /* pods appears, ranked first: why_text "AirPods connected" */ }
#[test] fn a_taken_default_is_corrected() { /* current.output = pods (ranked second); DefaultChanged(Output): "AirPods took output; usher put it back" */ }
#[test] fn a_reorder_is_ranked() { /* ask Reorder putting pods first: why Ranked */ }
#[test] fn a_failed_write_is_recorded() {
    // fail_set pods with Status { call: "set default", status: -50 }
    assert_eq!(rec.entries().last().unwrap().text(), "Couldn't switch output to AirPods: set default failed (OSStatus -50)");
}
#[test] fn a_trip_records_paused_and_turning_on_records_resumed() { /* 3 corrections in 10 s → Entry::Paused; ask SetAutoSwitch(true) → Entry::Resumed */ }
#[test] fn a_reconcile_that_changes_nothing_records_nothing() { /* DevicesChanged with no change: entries() unchanged */ }
#[test] fn a_deferred_launch_keeps_its_trigger() {
    // usb_unresolved at boot; DevicesChanged at 100; resolve; tick(500): the entry is Launch, not DevicesChanged
}
```

`src-tauri/src/activity.rs` (inline):

```rust
#[test] fn history_keeps_the_newest_200_newest_first() {
    let mut log = Log::default();
    for i in 0..201 { log.push(i, &Entry::Resumed); }
    let lines = log.lines();
    assert_eq!(lines.len(), 200);
    assert_eq!((lines[0].seq, lines[0].at), (201, 200));
    assert_eq!(lines[199].seq, 2);
    assert_eq!(lines[0].text, "Auto-switch on");
}
#[test] fn a_line_carries_the_why_text() { /* Switched with Connected("AirPods") → why Some("AirPods connected") */ }
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -p usher` → compile errors.

- [ ] **Step 3: Implement.** Update `Recorder` and any existing test that matches a full `rec.calls()` list (`grep -rn "calls()" src-tauri/tests`).

- [ ] **Step 4: Run** `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && pnpm check:size`. Expected: pass; `engine/mod.rs` ≤ 270 lines.

- [ ] **Step 5: Commit** `feat(engine): activity entries from every reconcile`

---

### Task 4: Switch notices and their setting

**Files:**
- Modify: `crates/usher-core/src/{state,commands,panel}.rs`, `src-tauri/src/engine/{mod,command,activity}.rs`, `src-tauri/src/shell.rs`, `src-tauri/src/settings.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src-tauri/tests/common/mod.rs`, `src/types.ts`, `src/api.ts`, `src/settings/Settings.tsx`
- Test: `crates/usher-core/tests/commands.rs`, inline `state.rs`, `src-tauri/tests/activity.rs`, `src/settings/Settings.test.tsx`

**Interfaces:**
- Consumes: Task 1 `NoticeGate`, `Trigger::notifies`; Task 3 `report`.
- Produces:
  - `State.notify_switches: bool` (default `true`, after `update_check`); `Command::SetNotify(bool)` (JSON `{"kind":"setNotify","args":false}`); `PanelState.notify_switches` (`notifySwitches`). Engine logs `switch notices {on|off}` when it changed (follow `SetUpdateCheck`: `grep -rn SetUpdateCheck crates src-tauri src`).
  - `Shell` gains `fn notice(&self, body: &str);`. `Engine` field `notices: NoticeGate`; `report` ends with `if state.notify_switches && state.auto_switch { if let Some(b) = notices.body(trigger, &entries, wall) { shell.notice(&b) } }`.
  - `shell.rs`: `fn show(app: AppHandle, body: String)` — the thread, permission and `show` code moved out of `paused`; a refused permission logs `notifications are not allowed; notice not shown`. `paused` and `notice` both call it.
  - `SettingsView.notify_switches: bool`; `#[tauri::command] pub async fn set_notify(engine, on: bool) -> Result<(), String>`; `SETTINGS_COMMANDS` + `set_notify`.
  - TS: `View.notifySwitches`, `SettingsView.notifySwitches`, `settingsApi.setNotify(on)`. Settings › General row `Notify when usher switches` (switch) with note `usher always tells you when it pauses Auto-switch.`
  - Test shell: `Call::Notice(String)`; `Recorder::notices() -> Vec<String>`.

- [ ] **Step 1: Write the failing tests**

```rust
// state.rs
#[test] fn a_file_without_notify_switches_reads_on() { /* {"schema_version":1} → true; false round-trips */ }
// tests/commands.rs
assert_eq!(p(r#"{"kind":"setNotify","args":false}"#).unwrap(), Command::SetNotify(false));
#[test] fn set_notify_reports_a_change_once() { /* Ok(true), then Ok(false); panel_state(..).notify_switches false */ }
// src-tauri/tests/activity.rs
#[test] fn a_connect_notifies_once_for_both_roles() {
    // pods appears, ranked first in output and input
    assert_eq!(rec.notices(), vec!["Output: MacBook Pro Speakers → AirPods\nInput: MacBook Pro Microphone → AirPods"]);
}
#[test] fn launch_rebind_and_commands_never_notify() { /* the Task 3 launch, rebind, reorder and promote-into-failure scenarios: notices() empty */ }
#[test] fn notices_are_capped_per_role() {
    // corrections at 200 ms, 5_000 ms, 11_000 ms (no trip: 2 writes in any 10 s window after the first expires)
    assert_eq!(rec.notices().len(), 2);
    assert_eq!(rec.entries().len(), 3);
}
#[test] fn notices_off_still_records_history() {
    assert_eq!(ask(&mut e, Command::SetNotify(false), 10), Ok(()));
    // a correction: entries() grows, notices() empty; after the debounce state.json has "notify_switches": false
}
#[test] fn the_trip_notifies_only_through_paused() {
    // third correction in 10 s: Call::Paused once, no Call::Notice from that reconcile
}
#[test] fn a_failed_switch_notifies() { /* DevicesChanged with fail_set pods: notices() == ["Couldn't switch output to AirPods: set default failed (OSStatus -50)"] */ }
```

```tsx
// Settings.test.tsx
it("turns switch notices off through the engine", () => { /* uncheck "Notify when usher switches" → setNotify(false) called once */ });
```

- [ ] **Step 2: Run to verify they fail** — `cargo test --workspace` and `pnpm test src/settings` → compile errors / failures.

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run the full gate.** Expected: pass.

- [ ] **Step 5: Commit** `feat: notify when usher switches, with a setting`

---

### Task 5: Diagnostics query

**Files:**
- Modify: `src-tauri/src/engine/{mod,thread,activity,exec}.rs`, `src-tauri/src/activity.rs`, `src-tauri/src/lib.rs` (handler)
- Test: `src-tauri/tests/activity.rs`, inline `src-tauri/src/activity.rs`

**Interfaces:**
- Consumes: Task 2 `diagnose`, `render_diagnostics`, `EngineStatus`, `AppInfo`.
- Produces:
  - `Event::Diagnostics(Sender<Diagnostics>)`; `handle` replies with `self.diagnostics()`.
  - `Engine` field `usb_unresolved: bool` (last scan's `usb_unresolved`); `pub fn diagnostics(&self) -> Diagnostics` in `engine/activity.rs` (`scanned` = `snap.is_some()`, `subscribed`, `rebinding` = `rebinding().then_some(rebind_attempt + 1)`, `read_only` = `!store.writable()`).
  - `EngineHandle::diagnostics(&self, timeout: Duration) -> Result<Diagnostics, Refusal>` (`Gone`, `NoAnswer` as `request`).
  - `activity.rs`: `pub fn macos_version() -> String` (`sw_vers -productVersion`, trimmed, `unknown` on any error, cached in a `OnceLock`); `#[tauri::command] pub async fn get_diagnostics(app: AppHandle, engine: State<'_, EngineHandle>) -> Result<String, String>` — off the main thread (`spawn_blocking`), fills `AppInfo { version: package_info().version, macos, log_path: app_log_dir()/usher.log, owner_names }` (`owner_names`: words of `id -F` plus `$USER`, cached in a `OnceLock`, empty on error), renders with `home_dir()`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn diagnostics_answer_before_the_first_scan() {
    // fail_scan = 5 at boot
    let (tx, rx) = mpsc::channel();
    e.handle(Event::Diagnostics(tx), at(10));
    let d = rx.recv().unwrap();
    assert!(!d.engine.scanned && d.devices.is_empty());
    assert_eq!(d.engine.rebinding, Some(2), "first retry pending");
}
#[test] fn diagnostics_report_the_scan_and_the_ranked_skips() {
    // trio, order [pods, spk]; pods unplugged
    assert!(d.engine.scanned && d.engine.subscribed && !d.engine.read_only);
    assert_eq!(d.roles[0].ranked[0].skip, Some(Skip::Disconnected));
    assert_eq!(d.devices.len(), 2);
}
#[test] fn diagnostics_through_the_run_loop() { /* EngineHandle::diagnostics(REQUEST_TIMEOUT) on a running thread: Ok; on a dropped receiver: Err(Refusal::Gone) */ }
// src-tauri/src/activity.rs
#[test] fn macos_version_is_a_dotted_number_or_unknown() {
    let v = macos_version();
    assert!(v == "unknown" || v.split('.').all(|p| p.parse::<u32>().is_ok()), "{v}");
}
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -p usher` → compile errors.

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run** `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && pnpm check:size`. Expected: pass.

- [ ] **Step 5: Commit** `feat(engine): diagnostics query`

---

### Task 6: Activity window, guard lists and the tray item

**Files:**
- Create: `src-tauri/capabilities/activity.json`
- Modify: `src-tauri/src/activity.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/panel/place.rs`, `src-tauri/src/tray.rs`, `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 3 `ActivityLog`; Task 5 `get_diagnostics`.
- Produces:
  - `activity::open(app: &AppHandle)` (Decision 12; mirror `settings::open`: `panel::hide_for_window`, `app.show()`, reuse an open window); `#[tauri::command] pub fn open_activity(app)`; `#[tauri::command] pub fn get_activity(log: State<ActivityLog>) -> Vec<Line>`.
  - `commands::ACTIVITY_COMMANDS: &[&str] = &["get_activity", "get_diagnostics"]`; `PANEL_COMMANDS` and `SETTINGS_COMMANDS` + `open_activity`; `allowed` gains `activity::LABEL`.
  - `panel::place::KEEP_FOCUS: [&str; 3] = [settings::LABEL, whatsnew::LABEL, activity::LABEL]`.
  - `tray.rs`: `MenuItem` id `activity`, text `Activity…`, between `Auto-switch` and `Settings…`; `"activity" => crate::activity::open(app)`.
  - `capabilities/activity.json`: `windows: ["activity"]`, `permissions: ["log:default", "core:event:allow-listen", "core:event:allow-unlisten", "core:window:allow-start-dragging", "core:window:allow-internal-toggle-maximize"]`.
  - `lib.rs` handler: `activity::open_activity`, `activity::get_activity`, `activity::get_diagnostics`, `settings::set_notify`.

- [ ] **Step 1: Write the failing tests**

```rust
// commands.rs, replacing each_window_calls_only_its_own_commands
let lists = [("panel", PANEL_COMMANDS), ("settings", SETTINGS_COMMANDS), ("whatsnew", WHATSNEW_COMMANDS), ("activity", ACTIVITY_COMMANDS)];
// shared = (c == "open_whatsnew" || c == "open_activity") && (other == "panel" || other == "settings")
assert!(!allowed("activity", "get_view") && !allowed("panel", "get_diagnostics") && !allowed("activity", "open_activity"));
// activity.rs
#[test] fn capability_grants_log_listen_and_drag() { /* windows == ["activity"], permissions == the list above */ }
// place.rs
#[test] fn panel_keeps_the_app_shown_for_the_other_windows() {
    assert_eq!(KEEP_FOCUS, ["settings", "whatsnew", "activity"]);
}
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -p usher` → failures.

- [ ] **Step 3: Implement.** `activity.html` lands in Task 7; until then the window shows a blank page.

- [ ] **Step 4: Run** `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && pnpm check:size`. Expected: pass.

- [ ] **Step 5: Commit** `feat: Activity window, tray item and IPC guard`

---

### Task 7: Activity page and the entry points

**Files:**
- Create: `activity.html` (copy `whatsnew.html`: title `usher Activity`, drag strip, `/src/activity/main.tsx`), `src/activity/{api.ts, lines.ts, lines.test.ts, Activity.tsx, Activity.test.tsx, ActivityApp.tsx, main.tsx, activity.css}`
- Modify: `vite.config.ts` (input `activity`), `src/api.ts`, `src/panel/{PanelView,Panel}.tsx`, `src/settings/Settings.tsx`
- Test: `src/panel/PanelView.test.tsx`, `src/settings/Settings.test.tsx`

**Interfaces:**
- Consumes: Task 6 commands, event `activity` with a `Line` payload.
- Produces:
  - `api.ts`: `type Line = { seq: number; at: number; text: string; why: string | null }`; `history(): Promise<Line[]>`; `onLine(cb): Promise<UnlistenFn>`; `diagnostics(): Promise<string>`.
  - `lines.ts`: `merge(lines: Line[], add: Line[]): Line[]` (by `seq` descending, no duplicate `seq`, at most 200); `clock(at: number): string` (`toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", second: "2-digit" })`).
  - `Activity.tsx`: `<h2>History</h2>`, a list (`role="list"`) of rows `time · text`, `why` in a `.note` span; empty `Nothing yet this session.`; `<details>` with `<summary>Diagnostics</summary>`, a `<pre>` of the report, buttons `Refresh` and `Copy diagnostics`, status `Copied.` or `Couldn't copy: <e>` (`role="status"`).
  - `ActivityApp.tsx`: listens first, then fetches the history and merges both; loads diagnostics on the `toggle` to open and on Refresh.
  - `main.tsx`: `forwardConsole(console, sink)`, imports `../settings/settings.css` and `./activity.css`.
  - `panelApi.openActivity()`, `settingsApi.openActivity()`; `Actions.openActivity(): void`; footer order: Debug badge, spacer, `Update to usher X…`, `Activity…`, `Settings…`, `Quit`; Settings › Troubleshooting: `Open Activity…` beside `Show log in Finder`.

- [ ] **Step 1: Write the failing tests**

```ts
// lines.test.ts
it("merge drops duplicates by seq and keeps 200", () => {
  const l = (seq: number) => ({ seq, at: seq, text: `t${seq}`, why: null });
  expect(merge([l(3), l(2)], [l(4), l(3)]).map((x) => x.seq)).toEqual([4, 3, 2]);
  expect(merge([], Array.from({ length: 205 }, (_, i) => l(i + 1))).map((x) => x.seq).slice(0, 2)).toEqual([205, 204]);
  expect(merge([], Array.from({ length: 205 }, (_, i) => l(i + 1)))).toHaveLength(200);
});
// Activity.test.tsx (happy-dom)
it("a live line that arrives before the history shows once", async () => { /* onLine fires seq 5; history resolves [5, 4]: two rows */ });
it("shows the why in dim text and the empty state", () => { /* "AirPods connected" inside .note; [] → "Nothing yet this session." */ });
it("loads diagnostics on open and Refresh, copies the shown text", async () => {
  // diagnostics resolves "usher 0.2.0 · …"; open <details>: one call, <pre> shows it; Refresh: second call;
  // Copy: navigator.clipboard.writeText called with the <pre> text; status "Copied."; a rejection → "Couldn't copy: denied"
});
// PanelView.test.tsx
it("footer order", () => { /* button names: "Update to usher 0.2.0…", "Activity…", "Settings…", "Quit"; Activity… calls openActivity */ });
// Settings.test.tsx
it("opens Activity from Troubleshooting", () => { /* "Open Activity…" → openActivity called once */ });
```

- [ ] **Step 2: Run** `pnpm test src/activity src/panel src/settings` → fail.

- [ ] **Step 3: Implement** the page, vite input and entry points.

- [ ] **Step 4: Run the full gate** and `pnpm build:app`. Expected: pass; `dist/activity.html` exists.

- [ ] **Step 5: Commit** `feat(ui): Activity window with history and Copy diagnostics`

---

### Task 8: Docs, site, ADR, QA and changeset

**Files:**
- Create: `docs/adr/0011-v2-scope.md`, `.changeset/<name>.md`, `site/src/assets/screens/activity-{light,dark}.jpg`
- Modify: `docs/adr/README.md`, `README.md` (Using usher), `site/src/pages/usage.astro`, `site/scripts/screens/{mock.js,capture.mjs}`, `site/src/assets/screens/{panel,panel-menu,panel-paused,settings}-*.jpg`, `docs/qa.md`, `docs/architecture/overview.md`, `docs/superpowers/plans/2026-11-23-usher-roadmap.md`

**Content pinned here:**
- ADR 0011 (Accepted): spec §9 (lifts the v1 non-goals for switch notices and history, a diagnostics UI, volume and mute, Bluetooth connect and battery; keeps §2's v2 non-goals), plus the M6 details that depart from the spec text: Decisions 1–3, 5, 6, 10, 11 (cause from the snapshot diff, not the trigger; notice cap; `Shell::notice` beside `Shell::activity`; names in entries; diagnostics answer before a scan; `uid#<n>` redaction; the window shows the redacted text).
- `usage.astro`: toc entry `activity` (`Activity and notices`) before `logs`; a section with the notice (what triggers one, the 10 s cap, never for your own changes or at launch), the setting, the history (200 entries, this session only, the log keeps the rest), the Diagnostics section and what Copy redacts; the right-click menu list gains **Activity…**; the footer paragraph and the keyboard Tab row gain **Activity…**; Settings list gains **Notify when usher switches** and **Open Activity…**; Logs gains one line pointing at Copy diagnostics for bug reports. `README.md` "Using usher" says the same in brief.
- `mock.js`: `get_activity` returns four made-up lines (a connect, a disconnect, a correction, `Auto-switch on`); `get_diagnostics` returns a made-up redacted report; `settings.notifySwitches: true`; `view.notifySwitches: true`. `capture.mjs`: add `['activity', 'activity.html', 480, 560, 'Activity']` to the window loop, open `<details>` before the shot. Regenerate every shot (the footer and Settings changed). `usage.astro` gets `<Shot name="activity" …>`.
- QA rows 35–42: 35 AirPods connect → one notice `Output: … → AirPods` (and Input when ranked); 36 unplug the top device → `… disconnected` notice; 37 reorder in the panel → no notice, history `changed in usher`; 38 Notify off persists across relaunch, history still fills; 39 notification permission denied → no notice, `notifications are not allowed` in the log, history fills; 40 Copy diagnostics with AirPods and a USB interface connected, paste into a text editor: no full Bluetooth address, no USB serial, no first name, `~` for the home folder; 41 panel footer with Debug on and an update pending: one row, nothing clipped; 42 Activity window: opens from the footer, the right-click menu and Settings, live lines appear while open, drags, resizes, light and dark glass.
- `overview.md`: an `ACT[Activity window + log]` node fed by the engine; core gains `ACTV[explain / diagnostics]`. Roadmap: a v2 table row `M6 Activity · 2026-11-24-usher-m6-activity.md`.
- Changeset (`minor`): `Added a notification when usher switches a device (with a setting to turn it off), an Activity window with this session's history, and diagnostics you can copy into a bug report.`

- [ ] **Step 1: Write the docs, mock and capture changes.**
- [ ] **Step 2: Regenerate the screenshots** (`pnpm dev`, then `node site/scripts/screens/capture.mjs`, per its README) and check each image by eye.
- [ ] **Step 3: Verify** the quality gate table, including `pnpm docs:build` and `pnpm build:app`.
- [ ] **Step 4: Commit** `docs: Activity usage, ADR 0011, QA rows and changeset`
- [ ] **Step 5: Open the PR** `feat: switch notices, Activity history and diagnostics (M6)` with `## Summary` and `## Test plan`; run the adversarial review (Opus: many files, a privacy boundary). The attack list enumerates every path that can make a notice (each trigger, a merged window, a guard trip, a failed write, a command, launch, rebind, a USB deferral, auto-switch off, the setting off, the 10 s cap) and every string that reaches Copy diagnostics (each UID shape, aliases, ambiguous families, device names, the log path, error strings), plus the guard lists, the capability, the live-line race and `KEEP_FOCUS`.

---

## Self-review notes

- Spec coverage: §5.1 (Tasks 1, 3; Decisions 1, 4, 6–8), §5.2 (Tasks 1, 4; Decisions 2, 3, 5), §5.3 (Task 3; Decision 9), §5.4 (Tasks 2, 5; Decisions 10, 11), §5.5 (Tasks 6, 7; Decisions 12, 13), §8 M6 parts (file table), §9 ADR 0011 (Task 8), §10 M6 bullets (Tasks 1–5; QA rows 35–42), §11 0.2.0 `minor` (Task 8). Anti-goals: no notice for user changes (Decision 2, `launch_rebind_and_commands_never_notify`), engine-only CoreAudio writes (nothing new writes), safe Copy (Task 2).
- Not built: a persisted history (v2 non-goal), Escape-to-close for Activity (it would need a third command beyond `ACTIVITY_COMMANDS`), a clipboard plugin, owner names the account does not carry (e.g. a device renamed to someone else's name): the on-screen preview is the backstop.
