# 0009. Reconcile waits for unresolved USB verdicts

- Status: Accepted
- Date: 2026-11-24
- Amends: [0006](0006-engine-threading-and-coreaudio-binding.md) ("The launch reconcile runs at once.")

## Context

A serial-less USB device is matched by its port-independent identity only once IOKit gives a
verdict for it. At launch, or right after a device event, its IOKit node may not be visible yet,
and the verdict is unresolved. ADR 0006 ran the reconcile on that scan anyway and scheduled one
rescan 500 ms later. When the unresolved device was the top-ranked one and had moved port while
usher was off, the first reconcile did not recognise it: usher switched to the next device, then
back after the rescan (M4 plan, Decision 24). "Unresolved" can also be permanent: a USB device whose
UID is not five-part, or an IOKit failure, never gets a verdict.

## Options

1. **Keep acting at once.** Up to 500 ms of audio on the wrong device, plus two writes that count
   toward the loop guard.
2. **Defer only the launch reconcile.** Fixes the reported case, but a hot-plug or a device event
   while a verdict is unresolved has the same flaw.
3. **Defer any reconcile whose scan has an unresolved verdict until the recheck.** The recheck acts
   whatever the verdict, because one may never come (an IOKit failure reads as unresolved).

## Decision

Option 3. A scan with an unresolved USB verdict, when no recheck has run since the last device
event, does not seed, decide or switch. It schedules the recheck 500 ms later and returns. The
recheck reconciles even if the verdict is still unresolved. A coalesced reconcile that falls due
together with the recheck is the recheck, so the delay stays bounded at about 650 ms per device
event even under event churn. A scan that fails in a recheck or coalesced reconcile retries through the
bind backoff (1, 2, 4, 8, 16, then 60 s), because the deferred decision would otherwise wait for the
next device event.

## Consequences

- No switch-away at launch for a moved serial-less USB device.
- Any device event while a USB verdict is unresolved switches about 500 ms later than before. With a
  permanently unresolved device, that is every device event.
- First-launch seeding waits for the recheck too, so it seeds from resolved identities.
- Commands sent before the first acting scan are refused as `NotReady`, as before any scan. During a
  later deferral they apply against the previous snapshot, for up to about 650 ms.
- Known gap, as before this ADR: a device event shortly before a pending recheck gets less than
  500 ms of IOKit settle time, because that recheck counts. Pushing the recheck back on each event
  would let event churn starve the reconcile.
