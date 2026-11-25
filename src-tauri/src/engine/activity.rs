//! Activity entries: what one reconcile did, as history lines for the shell.

use std::time::Instant;

use log::info;
use usher_core::{
    Diagnostics, EngineStatus, Entry, PerRole, Reason, Snapshot, Trigger, Wrote, diagnose, explain,
};
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
        now: Instant,
    ) {
        let entries: Vec<Entry> = reasons
            .iter()
            .filter_map(|r| explain(trigger, prev, snap, r, wrote.get(r.role)))
            .collect();
        for e in &entries {
            self.record(e);
        }
        // paused keeps its own always-on notice, so a tripped reconcile adds none here
        if self.state.notify_switches
            && self.state.auto_switch
            && let Some(body) = self.notices.body(trigger, &entries, self.clock.wall(now))
        {
            self.shell.notice(&body);
        }
    }

    /// What a bug report needs; `app` stays empty, the shell fills it.
    pub fn diagnostics(&self) -> Diagnostics {
        let status = EngineStatus {
            scanned: self.snap.is_some(),
            subscribed: self.subscribed,
            rebinding: self.rebinding().then_some(self.rebind_attempt + 1),
            read_only: !self.store.writable(),
            usb_unresolved: self.usb_unresolved,
        };
        let empty = Snapshot {
            devices: Vec::new(),
            current: PerRole::default(),
        };
        diagnose(&self.state, self.snap.as_ref().unwrap_or(&empty), status)
    }
}
