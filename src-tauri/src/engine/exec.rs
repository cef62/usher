//! One reconcile pass: scan, seed on first launch, decide, then run the actions in order.

use std::time::Instant;

use log::{debug, error, info, warn};
use usher_core::{Action, DeviceKey, Outcome, PerRole, Reason, Role, Snapshot, reconcile, seed};
use usher_coreaudio::AudioSystem;

use super::{Engine, Shell, USB_RECHECK};

impl<A: AudioSystem, S: Shell> Engine<A, S> {
    /// `false` when the scan failed.
    pub(super) fn run_reconcile(&mut self, now: Instant) -> bool {
        let scan = match self.audio.scan() {
            Ok(s) => s,
            Err(e) => {
                warn!("scan failed: {e}");
                return false;
            }
        };
        // an unresolved verdict can hide a serial-less USB device that moved port: acting now would switch away
        // from it and back 500 ms later, so the recheck acts instead (seeding too)
        if scan.usb_unresolved && !self.usb_rechecked {
            self.usb_due.get_or_insert(now + USB_RECHECK);
            debug!("reconcile deferred to the USB recheck");
            return true;
        }
        let snap = scan.snapshot;
        let wall = self.clock.wall(now);
        if self.seed_pending {
            self.state = seed(&snap, wall);
            self.seed_pending = false;
            self.store.save_now(&self.state, now);
            info!("first launch: seeded from current defaults");
        }
        let started = Instant::now();
        let out = reconcile(&mut self.state, &mut self.rt, &snap, wall);
        for r in &out.reasons {
            debug!("{r}");
        }
        debug!("reconcile took {} ms", started.elapsed().as_millis());
        self.execute(&out, &snap, now);
        self.clear_errors(&out.reasons);
        if !self.state.auto_switch {
            self.errors = PerRole::default();
        }
        self.snap = Some(snap);
        self.emit();
        true
    }

    fn execute(&mut self, out: &Outcome, snap: &Snapshot, now: Instant) {
        for action in &out.actions {
            match action {
                Action::SetDefault { role, key } => self.set_one(*role, key, snap),
                Action::PauseAuto { .. } => {
                    if let Some(p) = self.state.paused_reason.clone() {
                        warn!("loop guard tripped: {}", p.message());
                        self.shell.auto_switch_changed(false);
                        self.shell.paused(&p);
                    }
                }
                Action::AliasMerged { from, to } => info!("alias merged {from} -> {to}"),
                Action::Persist => self.store.mark_dirty(now),
            }
        }
    }

    pub(super) fn name_of(&self, snap: &Snapshot, key: &DeviceKey) -> String {
        snap.device(key)
            .map(|d| d.name.clone())
            .or_else(|| self.state.devices.get(key).map(|r| r.name.clone()))
            .unwrap_or_else(|| key.0.clone())
    }

    fn set_one(&mut self, role: Role, key: &DeviceKey, snap: &Snapshot) {
        let name = self.name_of(snap, key);
        match self.audio.set_default(role, key) {
            Ok(()) => {
                info!("set {role} default to {name} ({key}): ok");
                *self.errors.get_mut(role) = None;
            }
            Err(e) => {
                error!("set {role} default to {name} ({key}) failed: {e}");
                *self.errors.get_mut(role) = Some(format!("couldn't switch {role} to {name}: {e}"));
            }
        }
    }

    fn clear_errors(&mut self, reasons: &[Reason]) {
        for r in reasons {
            if r.desired.is_none() || r.desired == r.current {
                *self.errors.get_mut(r.role) = None;
            }
        }
    }
}
