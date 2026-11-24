//! Activity entries: what one reconcile did, as history lines for the shell.

use std::time::Instant;

use log::info;
use usher_core::{Entry, PerRole, Reason, Snapshot, Trigger, Wrote, explain};
use usher_coreaudio::AudioSystem;

use super::{Engine, Shell};

impl<A: AudioSystem, S: Shell> Engine<A, S> {
    /// Log the entry and hand it to the shell's history.
    pub(super) fn record(&self, entry: &Entry) {
        match entry.why_text() {
            Some(why) => info!("activity: {} ({why})", entry.text()),
            None => info!("activity: {}", entry.text()),
        }
        self.shell.activity(entry);
    }

    pub(super) fn report(
        &mut self,
        trigger: Trigger,
        prev: Option<&Snapshot>,
        snap: &Snapshot,
        reasons: &[Reason],
        wrote: &PerRole<Wrote>,
        _now: Instant,
    ) {
        for r in reasons {
            if let Some(e) = explain(trigger, prev, snap, r, wrote.get(r.role)) {
                self.record(&e);
            }
        }
    }
}
