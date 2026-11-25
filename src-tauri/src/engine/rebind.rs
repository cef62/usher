//! coreaudiod restart and launch failures: retry `subscribe`/`rebind` plus a scan on a 1, 2, 4, 8, 16 s then
//! 60 s schedule, forever.

use std::time::{Duration, Instant};

use log::{debug, info, warn};
use usher_core::Trigger;
use usher_coreaudio::AudioSystem;

use super::{Engine, Shell};

pub fn retry_delay(attempt: u32) -> Duration {
    Duration::from_secs(match attempt {
        0..=4 => 1 << attempt,
        _ => 60,
    })
}

/// The log line for a failed bind attempt: launch failures and rebinds read differently.
pub(super) fn retry_line(restarted: bool, why: &str, d: Duration) -> String {
    let what = if restarted { "rebind" } else { "audio setup" };
    format!("{what} failed: {why}; next try in {}s", d.as_secs())
}

impl<A: AudioSystem, S: Shell> Engine<A, S> {
    /// True while a rebind deadline is pending.
    pub fn rebinding(&self) -> bool {
        self.rebind_due.is_some()
    }

    pub(super) fn begin_rebind(&mut self, now: Instant) {
        info!("coreaudiod restarted; rebinding");
        self.rebind_attempt = 0;
        self.restarted = true;
        self.rebind_due = Some(now);
        self.trigger = Some(
            self.trigger
                .map_or(Trigger::Rebind, |t| t.merge(Trigger::Rebind)),
        );
        self.reconcile_due = None;
        // devices may have changed during the restart; the post-rebind scan may schedule a fresh recheck
        self.usb_due = None;
        self.usb_rechecked = false;
    }

    /// A reconcile from `tick`. A failed scan retries through the bind backoff: the decision it carried (a launch,
    /// seed or rebind reconcile deferred to the USB recheck) would otherwise wait for the next device event.
    pub(super) fn reconcile_or_retry(&mut self, now: Instant) {
        if !self.run_reconcile(now) {
            // events are dropped until the retry: a device plugged in meanwhile still gets its USB recheck
            self.usb_rechecked = false;
            let d = retry_delay(self.rebind_attempt);
            warn!("scan retry in {} s", d.as_secs());
            self.rebind_due = Some(now + d);
            self.rebind_attempt += 1;
        }
    }

    /// One bind attempt (subscribe, or rebind when already subscribed) followed by a scan.
    pub(super) fn attempt_bind(&mut self, now: Instant) {
        let n = self.rebind_attempt;
        let rebinding = self.subscribed;
        if rebinding || n > 0 {
            info!("rebind attempt {}", n + 1);
        }
        let bound = if rebinding {
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
            if self.restarted {
                self.notices.quiet_from(self.clock.wall(now));
            }
            self.restarted = false;
            return;
        }
        let d = retry_delay(n);
        let why = result.err().unwrap_or_else(|| "scan failed".into());
        warn!("{}", retry_line(self.restarted, &why, d));
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

    #[test]
    fn retry_line_names_launch_or_rebind() {
        assert_eq!(
            retry_line(
                false,
                "subscribe failed (OSStatus -1)",
                Duration::from_secs(1)
            ),
            "audio setup failed: subscribe failed (OSStatus -1); next try in 1s"
        );
        assert_eq!(
            retry_line(true, "scan failed", Duration::from_secs(60)),
            "rebind failed: scan failed; next try in 60s"
        );
    }
}
