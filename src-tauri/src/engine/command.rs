//! Panel and tray commands: gate, apply, answer.

use std::fmt;
use std::sync::mpsc::Sender;
use std::time::Instant;

use log::{info, warn};
use usher_core::{Command, CommandError, apply};
use usher_coreaudio::AudioSystem;

use super::{Engine, Shell};

pub type Reply = Sender<Result<(), Refusal>>;

#[derive(Clone, PartialEq, Debug)]
pub enum Refusal {
    Rejected(CommandError),
    NotReady,
    Rebinding,
    Gone,
    NoAnswer,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Rejected(e) => write!(f, "{e}"),
            Refusal::NotReady => {
                f.write_str("usher has not read the audio devices yet; try again in a moment")
            }
            Refusal::Rebinding => {
                f.write_str("usher is reconnecting to the audio system; try again in a moment")
            }
            Refusal::Gone => f.write_str("usher's audio engine stopped; quit and reopen usher"),
            Refusal::NoAnswer => f.write_str("usher did not answer in time; try again"),
        }
    }
}

impl<A: AudioSystem, S: Shell> Engine<A, S> {
    pub(super) fn on_command(&mut self, cmd: Command, now: Instant) -> Result<(), Refusal> {
        let Some(snap) = &self.snap else {
            warn!("command {cmd:?} dropped: no scan yet");
            return Err(Refusal::NotReady);
        };
        // a Forget now could race the pending rebind scan, which would re-add the device
        if matches!(cmd, Command::Forget { .. }) && self.rebinding() {
            return Err(Refusal::Rebinding);
        }
        let was_on = self.state.auto_switch;
        match apply(&mut self.state, &mut self.rt, snap, cmd) {
            Ok(true) => {
                self.store.mark_dirty(now);
                if was_on != self.state.auto_switch {
                    let on = self.state.auto_switch;
                    info!("auto-switch {}", if on { "on" } else { "off" });
                }
                self.emit();
                self.schedule_reconcile(now);
                Ok(())
            }
            Ok(false) => Ok(()),
            Err(e) => {
                warn!("command rejected: {e}");
                Err(Refusal::Rejected(e))
            }
        }
    }
}
