#![allow(dead_code)]
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use usher_core::{
    Command, DeviceKey, DeviceSnapshot, Entry, PausedReason, PerRole, Role, Snapshot, State,
    Timestamp, Transport, from_json, to_json,
};
use usher_coreaudio::{AudioError, AudioSystem, Notify, Scan};
use usher_lib::engine::{Clock, Engine, Event, PanelView, Refusal, Shell};
use usher_lib::store::Store;

#[derive(Default)]
pub struct World {
    pub devices: Vec<DeviceSnapshot>,
    pub current: PerRole<Option<DeviceKey>>,
    pub fail_set: BTreeMap<DeviceKey, AudioError>,
    pub vanish_on_set: BTreeSet<DeviceKey>,
    pub sticky: PerRole<Option<DeviceKey>>,
    pub fail_scan: u32,
    pub fail_subscribe: u32,
    pub fail_rebind: u32,
    pub usb_unresolved: bool,
    pub scans: u32,
    pub sets: Vec<(Role, DeviceKey)>,
    pub rebinds: u32,
    pub subscribed: bool,
}

#[derive(Clone, Default)]
pub struct Fake(Arc<Mutex<World>>);

impl Fake {
    pub fn world(&self) -> MutexGuard<'_, World> {
        self.0.lock().unwrap()
    }

    pub fn with(devices: Vec<DeviceSnapshot>, out: &str, inp: &str) -> Fake {
        let f = Fake::default();
        {
            let mut w = f.world();
            w.devices = devices;
            w.current.output = Some(out.into());
            w.current.input = Some(inp.into());
        }
        f
    }
}

fn failing(n: &mut u32, call: &'static str) -> Result<(), AudioError> {
    if *n > 0 {
        *n -= 1;
        return Err(AudioError::Status { call, status: -1 });
    }
    Ok(())
}

impl AudioSystem for Fake {
    fn scan(&self) -> Result<Scan, AudioError> {
        let mut w = self.world();
        w.scans += 1;
        failing(&mut w.fail_scan, "scan")?;
        Ok(Scan {
            snapshot: Snapshot {
                devices: w.devices.clone(),
                current: w.current.clone(),
            },
            usb_unresolved: w.usb_unresolved,
        })
    }

    fn set_default(&self, role: Role, key: &DeviceKey) -> Result<(), AudioError> {
        let mut w = self.world();
        w.sets.push((role, key.clone()));
        if w.vanish_on_set.contains(key) {
            w.devices.retain(|d| &d.uid != key);
            return Err(AudioError::NotFound(key.clone()));
        }
        if let Some(e) = w.fail_set.get(key) {
            return Err(e.clone());
        }
        *w.current.get_mut(role) = Some(key.clone());
        if let Some(s) = w.sticky.get(role).clone() {
            *w.current.get_mut(role) = Some(s);
        }
        Ok(())
    }

    fn subscribe(&mut self, _notify: Notify) -> Result<(), AudioError> {
        let mut w = self.world();
        failing(&mut w.fail_subscribe, "subscribe")?;
        w.subscribed = true;
        Ok(())
    }

    fn unsubscribe(&mut self) {
        self.world().subscribed = false;
    }

    fn rebind(&mut self) -> Result<(), AudioError> {
        let mut w = self.world();
        w.rebinds += 1;
        failing(&mut w.fail_rebind, "rebind")
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Call {
    View(PanelView),
    Auto(bool),
    Paused(PausedReason),
    Debug(bool),
    Activity(Entry),
}

#[derive(Clone, Default)]
pub struct Recorder(Arc<Mutex<Vec<Call>>>);

impl Recorder {
    pub fn calls(&self) -> Vec<Call> {
        self.0.lock().unwrap().clone()
    }

    pub fn entries(&self) -> Vec<Entry> {
        self.calls()
            .into_iter()
            .filter_map(|c| match c {
                Call::Activity(e) => Some(e),
                _ => None,
            })
            .collect()
    }

    pub fn last_paused(&self) -> Option<PausedReason> {
        self.calls().into_iter().rev().find_map(|c| match c {
            Call::Paused(p) => Some(p),
            _ => None,
        })
    }
}

impl Shell for Recorder {
    fn state_changed(&self, view: &PanelView) {
        self.0.lock().unwrap().push(Call::View(view.clone()));
    }
    fn auto_switch_changed(&self, on: bool) {
        self.0.lock().unwrap().push(Call::Auto(on));
    }
    fn debug_changed(&self, on: bool) {
        self.0.lock().unwrap().push(Call::Debug(on));
    }
    fn paused(&self, reason: &PausedReason) {
        self.0.lock().unwrap().push(Call::Paused(reason.clone()));
    }
    fn activity(&self, entry: &Entry) {
        self.0.lock().unwrap().push(Call::Activity(entry.clone()));
    }
}

fn dev(uid: &str, name: &str, t: Transport, i: u32, o: u32) -> DeviceSnapshot {
    DeviceSnapshot {
        uid: uid.into(),
        name: name.into(),
        transport: t,
        alive: true,
        hidden: false,
        input_streams: i,
        output_streams: o,
        serialless_usb: false,
    }
}

pub fn spk() -> DeviceSnapshot {
    dev(
        "BuiltInSpeakerDevice",
        "MacBook Pro Speakers",
        Transport::BuiltIn,
        0,
        2,
    )
}
pub fn pods() -> DeviceSnapshot {
    dev("AirPods-uid", "AirPods", Transport::Bluetooth, 1, 1)
}
pub fn mic() -> DeviceSnapshot {
    dev(
        "BuiltInMicrophoneDevice",
        "MacBook Pro Microphone",
        Transport::BuiltIn,
        1,
        0,
    )
}

pub fn disp() -> DeviceSnapshot {
    dev("DisplayUID", "Studio Display", Transport::Hdmi, 0, 2)
}

pub fn ranked(out: &[&str]) -> State {
    let mut s = State::default();
    // mic ranks first so a newly seen AirPods never takes the input
    s.input.order = vec!["BuiltInMicrophoneDevice".into()];
    s.output.order = out.iter().map(|k| DeviceKey::from(*k)).collect();
    s
}

pub fn ask(e: &mut Engine<Fake, Recorder>, cmd: Command, t: u64) -> Result<(), Refusal> {
    let (tx, rx) = std::sync::mpsc::channel();
    e.handle(Event::Command(cmd, Some(tx)), at(t));
    rx.try_recv().unwrap()
}

pub fn at(ms: u64) -> Instant {
    static BASE: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *BASE.get_or_init(Instant::now) + Duration::from_millis(ms)
}

pub fn state_file(dir: &Path, state: &State) {
    std::fs::write(dir.join("state.json"), to_json(state)).unwrap();
}

pub fn read_state(dir: &Path) -> State {
    from_json(&std::fs::read_to_string(dir.join("state.json")).unwrap()).unwrap()
}

pub fn boot(fake: Fake, dir: &Path) -> (Engine<Fake, Recorder>, Recorder) {
    let rec = Recorder::default();
    let (store, state) = Store::open(dir.join("state.json"));
    let clock = Clock {
        start: at(0),
        wall_at_start: Timestamp(1_000_000),
    };
    let e = Engine::start(
        fake,
        rec.clone(),
        store,
        state,
        Arc::new(|_| {}),
        clock,
        at(0),
    );
    (e, rec)
}
