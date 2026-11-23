# 0003. Strict priority: always enforce the top connected device

- Status: Accepted
- Date: 2026-11-23

## Context

CoreAudio never reports who changed a default device: the user, macOS, an app, or us
(`docs/audit/core-audio-and-switching.md` §9). The fork's "keep the user's pick" semantic needs
heuristics to tell these apart:

- whether a Control Center window is open;
- whether System Settings is frontmost;
- whether a device connected within the last 5 s;
- whether a change happened within 2 s of a topology change.

Those heuristics caused most of its race fixes (#8, #17, #81, #87, #88, #102). They are tied to the
macOS version and timed on one hardware set.

## Options

- **A. Strict.**
  - For each role, the default is always the highest-ranked connected, usable device that is not
    marked "never use".
  - Any other default is reverted.
- **B. Kept pick (the fork).**
  - A user's pick stays until that device leaves or a higher-ranked device connects.
  - Needs pick detection by heuristics.
- **C. Topology-only.**
  - Re-apply priorities only when devices connect or disconnect.
  - Simpler than B, but it still misreads a user pick made at the same moment a device connects.

## Decision

A, chosen by the owner, with these mitigations:

1. **Click to promote.** Clicking a device in cue's panel moves it to the top of its list. A
   temporary switch is one click and it persists. A pick made in Control Center is reverted, by design.
2. **Loop guard.**
   - If cue reverts the same role 3 times within 10 s, it turns auto-switch off, shows the off state
     on the menu-bar icon, and posts a notification naming the device that kept taking the default.
   - The user turns auto-switch back on by hand.
3. **"Never use" flag** per device and role.
   - Devices with a virtual or aggregate transport type start out flagged.
   - New devices are appended at the bottom of their list.
4. **The auto-switch toggle** is in the panel header and on the icon.

## Consequences

- The decision core is one pure function: `(devices, prefs) -> desired default per role`. No pick
  detection, no timing windows except the loop guard, and no Control Center sniffing.
- Our own writes need no echo suppression. Re-running the decision after our write is a no-op,
  because the system already matches the desired default.
- Users who switch often in Control Center will find cue fights them. The loop guard and click to
  promote are the answer. If this proves wrong in daily use, supersede this ADR with C.
