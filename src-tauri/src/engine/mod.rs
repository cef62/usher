//! The engine: one thread's worth of state. Audio events coalesce, a scan feeds `usher_core::reconcile`, the
//! actions run against the `AudioSystem`, and a `Shell` hears about every visible change. No clock of its own:
//! every entry point takes `now`, so tests drive time by hand.

mod exec;
mod rebind;
mod thread;

pub use rebind::retry_delay;
pub use thread::{EngineHandle, run};

use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use log::{debug, info, warn};
use serde::Serialize;
use usher_core::{
    Command, PanelState, PausedReason, PerRole, Runtime, Snapshot, State, Timestamp, apply,
    panel_state,
};
use usher_coreaudio::{AudioEvent, AudioSystem, Notify};

use crate::store::Store;

pub const COALESCE: Duration = Duration::from_millis(150);
pub const USB_RECHECK: Duration = Duration::from_millis(500);

pub enum Event {
    Audio(AudioEvent),
    Command(Command),
    ToggleAutoSwitch,
    Quit(Sender<()>),
}

#[derive(Debug, PartialEq)]
pub enum Flow {
    Continue,
    Stop,
}

pub trait Shell {
    fn state_changed(&self, view: &PanelView);
    fn auto_switch_changed(&self, on: bool);
    fn paused(&self, reason: &PausedReason);
}

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PanelView {
    #[serde(flatten)]
    pub panel: PanelState,
    pub errors: PerRole<Option<String>>,
}

pub struct Clock {
    pub start: Instant,
    pub wall_at_start: Timestamp,
}

impl Clock {
    pub fn wall(&self, now: Instant) -> Timestamp {
        let ms = now.saturating_duration_since(self.start).as_millis();
        Timestamp(self.wall_at_start.0.saturating_add(ms as u64))
    }
}

pub struct Engine<A: AudioSystem, S: Shell> {
    audio: A,
    shell: S,
    store: Store,
    clock: Clock,
    state: State,
    rt: Runtime,
    /// Last good scan; `None` until one succeeds. Commands wait for it.
    snap: Option<Snapshot>,
    seed_pending: bool,
    errors: PerRole<Option<String>>,
    reconcile_due: Option<Instant>,
    usb_due: Option<Instant>,
    /// A recheck ran since the last device event: do not schedule another.
    usb_rechecked: bool,
    notify: Notify,
    subscribed: bool,
    rebind_due: Option<Instant>,
    rebind_attempt: u32,
}

impl<A: AudioSystem, S: Shell> Engine<A, S> {
    pub fn start(
        audio: A,
        shell: S,
        store: Store,
        state: Option<State>,
        notify: Notify,
        clock: Clock,
        now: Instant,
    ) -> Self {
        let seed_pending = state.is_none();
        let mut e = Engine {
            audio,
            shell,
            store,
            clock,
            state: state.unwrap_or_default(),
            rt: Runtime::default(),
            snap: None,
            seed_pending,
            errors: PerRole::default(),
            reconcile_due: None,
            usb_due: None,
            usb_rechecked: false,
            notify,
            subscribed: false,
            rebind_due: None,
            rebind_attempt: 0,
        };
        e.attempt_bind(now);
        e.shell.auto_switch_changed(e.state.auto_switch);
        e.emit();
        e
    }

    pub fn handle(&mut self, ev: Event, now: Instant) -> Flow {
        match ev {
            Event::Audio(a) => self.on_audio(a, now),
            Event::Command(c) => {
                self.on_command(c, now);
                self.shell.auto_switch_changed(self.state.auto_switch);
            }
            Event::ToggleAutoSwitch => {
                let on = !self.state.auto_switch;
                self.on_command(Command::SetAutoSwitch(on), now);
                // the tray flipped its own check on click: say the truth on every path (dropped, no-op, rejected)
                self.shell.auto_switch_changed(self.state.auto_switch);
            }
            Event::Quit(done) => {
                self.shutdown();
                let _ = done.send(());
                return Flow::Stop;
            }
        }
        Flow::Continue
    }

    pub fn tick(&mut self, now: Instant) {
        if self.rebind_due.is_some_and(|d| d <= now) {
            self.rebind_due = None;
            self.attempt_bind(now);
        }
        if self.reconcile_due.is_some_and(|d| d <= now) {
            self.reconcile_due = None;
            // a recheck that is already due would scan twice back to back
            if self.usb_due.is_some_and(|d| d <= now) {
                self.usb_due = None;
            }
            self.run_reconcile(now);
        }
        if self.usb_due.is_some_and(|d| d <= now) {
            self.usb_due = None;
            self.usb_rechecked = true;
            self.run_reconcile(now);
        }
        self.store.save_if_due(&self.state, now);
    }

    pub fn next_due(&self) -> Option<Instant> {
        [
            self.reconcile_due,
            self.usb_due,
            self.rebind_due,
            self.store.due(),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn saves(&self) -> u32 {
        self.store.saves()
    }

    pub fn view(&self) -> PanelView {
        let empty;
        let snap = match &self.snap {
            Some(s) => s,
            None => {
                empty = Snapshot {
                    devices: Vec::new(),
                    current: PerRole::default(),
                };
                &empty
            }
        };
        PanelView {
            panel: panel_state(&self.state, snap),
            errors: self.errors.clone(),
        }
    }

    fn emit(&self) {
        self.shell.state_changed(&self.view());
    }

    fn schedule_reconcile(&mut self, now: Instant) {
        if self.rebind_due.is_some() {
            return;
        }
        self.reconcile_due.get_or_insert(now + COALESCE);
    }

    fn on_audio(&mut self, ev: AudioEvent, now: Instant) {
        if ev == AudioEvent::ServiceRestarted {
            self.begin_rebind(now);
            return;
        }
        if self.rebind_due.is_some() {
            debug!("ignored {ev:?}: rebind pending");
            return;
        }
        if self.reconcile_due.is_some() {
            debug!("coalesced {ev:?}");
        } else {
            debug!("event {ev:?}");
        }
        if ev == AudioEvent::DevicesChanged {
            self.usb_rechecked = false;
        }
        self.schedule_reconcile(now);
    }

    fn on_command(&mut self, cmd: Command, now: Instant) {
        let Some(snap) = &self.snap else {
            warn!("command {cmd:?} dropped: no scan yet");
            return;
        };
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
            }
            Ok(false) => {}
            Err(e) => warn!("command rejected: {e}"),
        }
    }
}
