//! coreaudiod restart and launch failures: retry `subscribe`/`rebind` plus a scan on a 1, 2, 4, 8, 16 s then
//! 60 s schedule, forever.

use std::time::{Duration, Instant};

use log::{debug, info, warn};
use usher_coreaudio::AudioSystem;

use super::{Engine, Shell};

pub fn retry_delay(attempt: u32) -> Duration {
    Duration::from_secs(match attempt {
        0..=4 => 1 << attempt,
        _ => 60,
    })
}

impl<A: AudioSystem, S: Shell> Engine<A, S> {
    /// True while a rebind deadline is pending.
    pub fn rebinding(&self) -> bool {
        self.rebind_due.is_some()
    }

    pub(super) fn begin_rebind(&mut self, now: Instant) {
        info!("coreaudiod restarted; rebinding");
        self.rebind_attempt = 0;
        self.rebind_due = Some(now);
        self.reconcile_due = None;
    }

    /// One bind attempt (subscribe, or rebind when already subscribed) followed by a scan.
    pub(super) fn attempt_bind(&mut self, now: Instant) {
        let n = self.rebind_attempt;
        let rebinding = self.subscribed;
        let bound = if rebinding {
            info!("rebind attempt {}", n + 1);
            self.audio.rebind()
        } else {
            self.audio.subscribe(self.notify.clone())
        };
        let result = bound
            .map(|()| self.subscribed = true)
            .map_err(|e| e.to_string());
        if result.is_ok() && self.run_reconcile(now) {
            if rebinding || n > 0 {
                info!("rebound after {} attempt(s)", n + 1);
            }
            self.rebind_attempt = 0;
            return;
        }
        let d = retry_delay(n);
        let why = result.err().unwrap_or_else(|| "scan failed".into());
        warn!("rebind failed: {why}; next try in {d:?}");
        self.rebind_due = Some(now + d);
        self.rebind_attempt = n + 1;
    }

    /// Flush the state and drop the listeners.
    pub(super) fn shutdown(&mut self) {
        self.store.flush(&self.state);
        self.audio.unsubscribe();
        debug!("engine shut down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_schedule_is_1_2_4_8_16_then_60_forever() {
        let secs: Vec<u64> = (0..8).map(|a| retry_delay(a).as_secs()).collect();
        assert_eq!(secs, vec![1, 2, 4, 8, 16, 60, 60, 60]);
        assert_eq!(retry_delay(u32::MAX), Duration::from_secs(60));
    }
}
