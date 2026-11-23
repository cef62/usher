mod common;
use common::*;
use std::sync::mpsc;
use std::time::Duration;
use usher_core::{DeviceKey, State};
use usher_coreaudio::{AudioError, AudioEvent};
use usher_lib::engine::{Engine, EngineHandle, Event, Flow, run};

fn trio() -> Fake {
    Fake::with(
        vec![spk(), pods(), mic()],
        "BuiltInSpeakerDevice",
        "BuiltInMicrophoneDevice",
    )
}

fn restart(e: &mut Engine<Fake, Recorder>, ms: u64) {
    e.handle(Event::Audio(AudioEvent::ServiceRestarted), at(ms));
}

#[test]
fn service_restarted_rebinds_then_reconciles() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    let (mut e, _) = boot(fake.clone(), d.path());
    restart(&mut e, 1_000);
    e.tick(at(1_000));
    assert_eq!(fake.world().rebinds, 1);
    assert_eq!(fake.world().scans, 2);
}

#[test]
fn failed_rebinds_back_off_1_2_4_8_16_then_60() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    let (mut e, _) = boot(fake.clone(), d.path());
    fake.world().fail_rebind = 6;
    restart(&mut e, 0);
    let steps = [
        (0, 1),
        (999, 1),
        (1_000, 2),
        (3_000, 3),
        (7_000, 4),
        (15_000, 5),
        (31_000, 6),
        (90_999, 6),
        (91_000, 7),
    ];
    for (t, n) in steps {
        e.tick(at(t));
        assert_eq!(fake.world().rebinds, n, "at {t}");
    }
    assert!(!e.rebinding());
}

#[test]
fn restart_mid_event_recovers_after_rebind() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    let mut s = State::default();
    s.input.order = vec!["BuiltInMicrophoneDevice".into()];
    s.output.order = vec!["AirPods-uid".into(), "BuiltInSpeakerDevice".into()];
    state_file(d.path(), &s);
    fake.world().fail_set.insert(
        DeviceKey::from("AirPods-uid"),
        AudioError::Status {
            call: "set default",
            status: 560947818,
        },
    );
    let (mut e, _) = boot(fake.clone(), d.path());
    assert!(e.view().errors.output.is_some());
    restart(&mut e, 1_000);
    fake.world().fail_set.clear();
    e.tick(at(1_000));
    assert_eq!(fake.world().rebinds, 1);
    assert_eq!(fake.world().sets.len(), 2);
    assert_eq!(e.view().errors.output, None);
}

#[test]
fn events_during_backoff_do_not_scan() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    let (mut e, _) = boot(fake.clone(), d.path());
    fake.world().fail_rebind = 1;
    restart(&mut e, 0);
    e.tick(at(0));
    e.handle(Event::Audio(AudioEvent::DevicesChanged), at(100));
    e.tick(at(250));
    assert_eq!(fake.world().scans, 1);
}

#[test]
fn start_failure_enters_backoff() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().fail_subscribe = 1;
    let (mut e, _) = boot(fake.clone(), d.path());
    assert_eq!(fake.world().scans, 0);
    assert!(e.rebinding());
    e.tick(at(999));
    assert!(!fake.world().subscribed);
    e.tick(at(1_000));
    assert!(fake.world().subscribed);
    assert_eq!(fake.world().scans, 1);
    assert!(!e.rebinding());
}

#[test]
fn failed_launch_scan_retries_by_rebind() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().fail_scan = 1;
    let (mut e, _) = boot(fake.clone(), d.path());
    assert!(e.rebinding());
    e.tick(at(1_000));
    assert_eq!(fake.world().rebinds, 1);
    assert_eq!(fake.world().scans, 2);
    assert!(!e.rebinding());
    assert_eq!(e.saves(), 1);
}

#[test]
fn quit_inside_debounce_window_flushes() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    let (mut e, _) = boot(fake.clone(), d.path());
    let (tx, rx) = mpsc::channel();
    e.handle(Event::ToggleAutoSwitch, at(1_000));
    assert!(matches!(e.handle(Event::Quit(tx), at(1_100)), Flow::Stop));
    assert!(!read_state(d.path()).auto_switch);
    assert!(rx.try_recv().is_ok());
    assert!(!fake.world().subscribed);
}

#[test]
fn notify_after_engine_stopped_is_silent() {
    let (h, rx) = EngineHandle::new();
    let n = h.notify();
    drop(rx);
    n(AudioEvent::DevicesChanged);
}

#[test]
fn send_after_quit_returns_false() {
    let (h, rx) = EngineHandle::new();
    drop(rx);
    assert!(!h.send(Event::ToggleAutoSwitch));
    assert!(!h.quit(Duration::from_millis(10)));
}

#[test]
fn run_loop_flushes_and_stops_when_senders_drop() {
    let d = tempfile::tempdir().unwrap();
    let (e, _) = boot(trio(), d.path());
    let (h, rx) = EngineHandle::new();
    let t = std::thread::spawn(move || run(e, rx));
    assert!(h.send(Event::ToggleAutoSwitch));
    drop(h);
    t.join().unwrap();
    assert!(!read_state(d.path()).auto_switch);
}

#[test]
fn run_loop_quit_acks_and_unsubscribes() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    let (e, _) = boot(fake.clone(), d.path());
    let (h, rx) = EngineHandle::new();
    let t = std::thread::spawn(move || run(e, rx));
    assert!(h.quit(Duration::from_secs(5)));
    t.join().unwrap();
    assert!(!fake.world().subscribed);
}

#[test]
fn restart_cancels_pending_usb_recheck() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().usb_unresolved = true;
    let (mut e, _) = boot(fake.clone(), d.path());
    fake.world().fail_rebind = 5;
    restart(&mut e, 100);
    e.tick(at(100));
    e.tick(at(500));
    assert_eq!(fake.world().scans, 1);
}
