//! Panel and tray commands: gate, apply, answer.

use std::fmt;
use std::sync::mpsc::Sender;
use std::time::Instant;

use log::{info, warn};
use usher_core::{Command, CommandError, DeviceKey, Entry, PerRole, Trigger, apply};
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
            warn!("forget refused: rebinding");
            return Err(Refusal::Rebinding);
        }
        let was = (
            self.state.auto_switch,
            self.state.debug,
            self.state.update_check,
            self.state.notify_switches,
        );
        let forgot = match &cmd {
            Command::Forget { key } => Some((self.name_of(snap, key), key.clone())),
            _ => None,
        };
        match apply(&mut self.state, &mut self.rt, snap, cmd) {
            Ok(true) => {
                self.store.mark_dirty(now);
                self.announce(was, forgot);
                self.emit();
                self.schedule_reconcile(now, Trigger::Command);
                Ok(())
            }
            Ok(false) => Ok(()),
            Err(e) => {
                warn!("command rejected: {e}");
                Err(Refusal::Rejected(e))
            }
        }
    }

    /// Log and tell the shell about what the command changed.
    fn announce(&mut self, was: (bool, bool, bool, bool), forgot: Option<(String, DeviceKey)>) {
        let (was_on, was_debug, was_check, was_notify) = was;
        let word = |b: bool| if b { "on" } else { "off" };
        if was_on != self.state.auto_switch {
            let on = self.state.auto_switch;
            info!("auto-switch {}", word(on));
            self.shell.auto_switch_changed(on);
            if on {
                self.record(&Entry::Resumed);
            } else {
                self.errors = PerRole::default();
            }
        }
        if was_debug != self.state.debug {
            let on = self.state.debug;
            info!("debug logging {}", word(on));
            self.shell.debug_changed(on);
        }
        if was_check != self.state.update_check {
            info!("daily update check {}", word(self.state.update_check));
        }
        if was_notify != self.state.notify_switches {
            info!("switch notices {}", word(self.state.notify_switches));
        }
        if let Some((name, key)) = forgot {
            info!("forgot {name} ({key})");
        }
    }
}
