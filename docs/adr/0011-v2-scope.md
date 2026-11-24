# 0011. v2 scope

- Status: Accepted
- Date: 2026-11-24
- Departs from: the v1 non-goals in `docs/superpowers/specs/2026-11-23-usher-design.md` §2
- Spec: `docs/superpowers/specs/2026-11-24-usher-v2-design.md` §2, §5, §9

## Context

v1 shipped (0.1.0 to 0.1.3) with these non-goals: switch notifications and history, a diagnostics
UI, volume and mute, Bluetooth connect and battery. The owner asked for them in v2, in three
milestones: M6 Activity, M7 Volume and mute, M8 Bluetooth.

## Options

1. **Keep the v1 non-goals.** usher stays a silent switcher; bug reports need the log file.
2. **Lift them one milestone at a time, each with its own ADR where the design departs.** Chosen.
3. **Lift them all with one ADR.** Hides the M8 trade-offs (IOBluetooth, `unsafe` scope) in a scope note.

## Decision

Option 2. This ADR lifts the v1 non-goals for:

- **Switch notices and a session history** (M6, built).
- **A diagnostics UI** (M6, built): the Activity window and **Copy diagnostics**, redacted for a public issue.
- **Volume and mute** (M7, to come) and **Bluetooth connect and battery** (M8, to come). ADR 0012
  (Bluetooth) and ADR 0013 (level listeners) will record their design.

Non-goals that stay (v2 spec §2): volume or mute shortcuts, option-click or scroll on the icon,
per-row volume sliders, a remembered volume per device, a persisted history (the log file is the
permanent record), selecting a device on Connect, and profiles, Focus modes, separate
speaker and headphone lists, vendor HID and Intel builds. All v1 anti-goals still apply, plus: no
notice for a change the user made in usher, no CoreAudio write outside the engine thread, nothing
unsafe to paste in a public issue in Copy diagnostics.

M6 departs from the spec text in three places (plan `2026-11-24-usher-m6-activity.md`):

- **Decision 4, trigger merge order.** The spec's precedence is extended to
  `Command > Rebind > Launch > DefaultChanged > DevicesChanged`. `Launch` and `Rebind` are history
  only, so they must outrank the event triggers that would otherwise make a notice at start or
  after a coreaudiod restart.
- **Decision 9, history lines are rendered in Rust.** The shell keeps `Line { seq, at, text, why }`;
  the page only lays them out, so the text exists once (log, window, notice). There is no role
  column: the role is the first word of the text.
- **Decision 11, owner-name stripping goes beyond the spec's `'s` rule.** Besides a leading
  `<name>'s `, any whole word, case-insensitive, of the Mac account's full name or short name is
  removed together with a connector word before it (`di`, `de`, `da`, `du`, `del`, `von`, `van`,
  `of`). A non-ASCII name (`山田`) also matches inside a word, as CJK names sit glued to particles. Device names carry the owner in any language ("AirPods Pro di Ada" on an Italian Mac), and the
  text is meant for public issues.
- **Clarifications, not departures:** Decisions 1 to 3, 5, 6, 10: the cause comes from the snapshot diff, not the trigger; at most
  one notice per role per 10 s (history keeps every entry); `Shell::notice` sits beside
  `Shell::activity`; entries hold names, not keys; diagnostics answer before the first scan; the
  window shows the redacted text Copy puts on the clipboard.
  The spec §5.2 line "notifications are not allowed" cannot be observed: the notification plugin on
  desktop always reports permission as granted, so the line is never logged.

## Consequences

- **Known limit.** An unplug within about 150 ms of usher's own write can read as
  "`<name>` took output; usher put it back" instead of "disconnected": the previous snapshot is the
  pre-write scan, so it still lists the device as current. The device switch itself is right; only the history
  cause (and the notice it triggers) is off.
- Diagnostics redaction is allow-list based: an unknown UID shape renders as `uid#<n>`. An owner name the
  account does not carry (a device renamed to someone else's name) is not caught; the on-screen preview
  is the backstop.
- The Usage page, README and screenshots move with the UI (ADR 0010).
