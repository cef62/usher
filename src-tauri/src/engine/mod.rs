//! The engine: one thread's worth of state. Audio events coalesce, a scan feeds `usher_core::reconcile`, the
//! actions run against the `AudioSystem`, and a `Shell` hears about every visible change. No clock of its own:
//! every entry point takes `now`, so tests drive time by hand.

mod exec;

use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use log::{debug, error, info, warn};
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
        };
        e.launch(notify, now);
        e.shell.auto_switch_changed(e.state.auto_switch);
        e.emit();
        e
    }

    /// Task 7 hook: backoff on a failed subscribe or scan goes here.
    fn launch(&mut self, notify: Notify, now: Instant) {
        match self.audio.subscribe(notify) {
            Ok(()) => self.run_reconcile(now),
            Err(e) => error!("could not subscribe to audio events: {e}"),
        }
    }

    pub fn handle(&mut self, ev: Event, now: Instant) -> Flow {
        match ev {
            Event::Audio(a) => self.on_audio(a, now),
            Event::Command(c) => self.on_command(c, now),
            Event::ToggleAutoSwitch => {
                let on = !self.state.auto_switch;
                self.on_command(Command::SetAutoSwitch(on), now);
            }
            Event::Quit(done) => {
                self.audio.unsubscribe();
                self.store.flush(&self.state);
                let _ = done.send(());
                return Flow::Stop;
            }
        }
        Flow::Continue
    }

    pub fn tick(&mut self, now: Instant) {
        if self.reconcile_due.is_some_and(|d| d <= now) {
            self.reconcile_due = None;
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
        [self.reconcile_due, self.usb_due, self.store.due()]
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
        self.reconcile_due.get_or_insert(now + COALESCE);
    }

    fn on_audio(&mut self, ev: AudioEvent, now: Instant) {
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
                    self.shell.auto_switch_changed(on);
                }
                self.emit();
                self.schedule_reconcile(now);
            }
            Ok(false) => {}
            Err(e) => warn!("command rejected: {e}"),
        }
    }
}
