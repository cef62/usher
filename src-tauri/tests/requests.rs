mod common;
use common::*;
use std::time::Duration;
use usher_core::{Command, CommandError, DeviceKey, PerRole, Role, State};
use usher_coreaudio::{AudioError, AudioEvent};
use usher_lib::engine::{EngineHandle, Event, REQUEST_TIMEOUT, Refusal, run};

fn trio() -> Fake {
    Fake::with(
        vec![spk(), pods(), mic()],
        "BuiltInSpeakerDevice",
        "BuiltInMicrophoneDevice",
    )
}

fn autos(rec: &Recorder) -> usize {
    rec.calls()
        .iter()
        .filter(|c| matches!(c, Call::Auto(_)))
        .count()
}

fn fail(fake: &Fake, uid: &str) {
    fake.world().fail_set.insert(
        uid.into(),
        AudioError::Status {
            call: "set default",
            status: -50,
        },
    );
}

#[test]
fn request_replies_ok_after_apply() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, _) = boot(trio(), d.path());
    let cmd = Command::SetNeverUse {
        role: Role::Output,
        key: "AirPods-uid".into(),
        on: true,
    };
    assert_eq!(ask(&mut e, cmd, 10), Ok(()));
    assert!(e.state().output.never_use.contains(&"AirPods-uid".into()));
}

#[test]
fn a_no_op_still_replies_ok() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, _) = boot(trio(), d.path());
    assert_eq!(ask(&mut e, Command::SetAutoSwitch(true), 10), Ok(()));
}

#[test]
fn request_before_first_scan_is_not_ready() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().fail_scan = 1;
    let (mut e, _) = boot(fake, d.path());
    assert_eq!(
        ask(&mut e, Command::SetDebug(true), 10),
        Err(Refusal::NotReady)
    );
    assert!(!e.state().debug);
}

#[test]
fn rejected_command_replies_with_the_core_error() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, _) = boot(trio(), d.path());
    let key = "BuiltInSpeakerDevice";
    let before = e.state().clone();
    let r = ask(&mut e, Command::Forget { key: key.into() }, 10);
    assert_eq!(*e.state(), before);
    assert_eq!(
        r,
        Err(Refusal::Rejected(CommandError::StillConnected(key.into())))
    );
    assert_eq!(
        r.unwrap_err().to_string(),
        "device BuiltInSpeakerDevice is still connected"
    );
}

#[test]
fn forget_is_refused_while_rebinding() {
    let d = tempfile::tempdir().unwrap();
    let fake = Fake::with(
        vec![spk(), mic()],
        "BuiltInSpeakerDevice",
        "BuiltInMicrophoneDevice",
    );
    let mut s = State::default();
    s.input.order = vec!["BuiltInMicrophoneDevice".into()];
    s.output.order = ["BuiltInSpeakerDevice", "AirPods-uid"]
        .iter()
        .map(|k| DeviceKey::from(*k))
        .collect();
    state_file(d.path(), &s);
    let (mut e, _) = boot(fake.clone(), d.path());
    fake.world().fail_rebind = 1;
    e.handle(Event::Audio(AudioEvent::ServiceRestarted), at(0));
    e.tick(at(0));
    assert!(e.rebinding());
    let forget = Command::Forget {
        key: "AirPods-uid".into(),
    };
    assert_eq!(ask(&mut e, forget, 10), Err(Refusal::Rebinding));
    assert!(e.state().output.order.contains(&"AirPods-uid".into()));
    let promote = Command::Promote {
        role: Role::Output,
        key: "AirPods-uid".into(),
    };
    assert_eq!(ask(&mut e, promote, 20), Ok(()));
}

#[test]
fn read_only_store_shows_in_the_view() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("state.json"), r#"{"schema_version":2}"#).unwrap();
    let (e, _) = boot(trio(), d.path());
    assert!(e.view().read_only);
    assert_eq!(serde_json::to_value(e.view()).unwrap()["readOnly"], true);
}

#[test]
fn writable_store_is_not_read_only() {
    let d = tempfile::tempdir().unwrap();
    let (e, _) = boot(trio(), d.path());
    assert!(!e.view().read_only);
}

#[test]
fn request_to_a_gone_engine_is_gone() {
    let (h, rx) = EngineHandle::new();
    drop(rx);
    assert_eq!(
        h.request(Command::SetDebug(true), Duration::from_millis(10)),
        Err(Refusal::Gone)
    );
}

#[test]
fn request_without_answer_times_out() {
    let (h, _rx) = EngineHandle::new();
    assert_eq!(
        h.request(Command::SetDebug(true), Duration::from_millis(10)),
        Err(Refusal::NoAnswer)
    );
}

#[test]
fn request_through_the_run_loop_gets_the_engine_answer() {
    let d = tempfile::tempdir().unwrap();
    let (e, _) = boot(trio(), d.path());
    let (h, rx) = EngineHandle::new();
    let t = std::thread::spawn(move || run(e, rx));
    assert_eq!(h.request(Command::SetDebug(true), REQUEST_TIMEOUT), Ok(()));
    drop(h);
    t.join().unwrap();
}

#[test]
fn refusal_messages_are_the_spec_copy() {
    assert_eq!(
        Refusal::NotReady.to_string(),
        "usher has not read the audio devices yet; try again in a moment"
    );
    assert_eq!(
        Refusal::Rebinding.to_string(),
        "usher is reconnecting to the audio system; try again in a moment"
    );
    assert_eq!(
        Refusal::Gone.to_string(),
        "usher's audio engine stopped; quit and reopen usher"
    );
    assert_eq!(
        Refusal::NoAnswer.to_string(),
        "usher did not answer in time; try again"
    );
}

#[test]
fn set_debug_tells_the_shell_once() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, rec) = boot(trio(), d.path());
    ask(&mut e, Command::SetDebug(true), 10).unwrap();
    ask(&mut e, Command::SetDebug(true), 20).unwrap();
    let n = rec
        .calls()
        .iter()
        .filter(|c| **c == Call::Debug(true))
        .count();
    assert_eq!(n, 1);
}

#[test]
fn promote_does_not_touch_the_tray() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, rec) = boot(trio(), d.path());
    let before = autos(&rec);
    let promote = Command::Promote {
        role: Role::Output,
        key: "AirPods-uid".into(),
    };
    ask(&mut e, promote, 10).unwrap();
    assert_eq!(autos(&rec), before);
}

#[test]
fn panel_toggle_updates_the_tray_once() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, rec) = boot(trio(), d.path());
    let before = autos(&rec);
    ask(&mut e, Command::SetAutoSwitch(false), 10).unwrap();
    assert_eq!(autos(&rec), before + 1);
    assert_eq!(
        rec.calls()
            .iter()
            .rev()
            .find(|c| matches!(c, Call::Auto(_))),
        Some(&Call::Auto(false))
    );
}

#[test]
fn turning_auto_switch_off_clears_error_lines() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    state_file(d.path(), &ranked(&["AirPods-uid", "BuiltInSpeakerDevice"]));
    fail(&fake, "AirPods-uid");
    let (mut e, _) = boot(fake, d.path());
    assert!(e.view().errors.output.is_some());
    ask(&mut e, Command::SetAutoSwitch(false), 10).unwrap();
    assert_eq!(e.view().errors, PerRole::default());
    e.handle(Event::Audio(AudioEvent::DevicesChanged), at(20));
    e.tick(at(170));
    assert_eq!(e.view().errors, PerRole::default());
}

#[test]
fn guard_trip_clears_error_lines() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().current.output = Some("AirPods-uid".into());
    state_file(d.path(), &ranked(&["BuiltInSpeakerDevice", "AirPods-uid"]));
    fail(&fake, "BuiltInSpeakerDevice");
    let (mut e, _) = boot(fake, d.path());
    assert!(e.view().errors.output.is_some());
    for (ev, tick) in [(200, 350), (500, 650), (800, 950)] {
        e.handle(
            Event::Audio(AudioEvent::DefaultChanged(Role::Output)),
            at(ev),
        );
        e.tick(at(tick));
    }
    assert!(!e.state().auto_switch);
    assert_eq!(e.view().errors.output, None);
}

#[test]
fn update_check_off_is_saved_and_in_the_view() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, _) = boot(trio(), d.path());
    assert_eq!(ask(&mut e, Command::SetUpdateCheck(false), 10), Ok(()));
    assert!(!e.view().panel.update_check);
    assert_eq!(
        e.view().update,
        None,
        "the shell sets the pending update, not the engine"
    );
    e.tick(at(2_000));
    let saved = std::fs::read_to_string(d.path().join("state.json")).unwrap();
    assert!(saved.contains("\"update_check\": false"), "{saved}");
}

#[test]
fn diagnostics_through_the_run_loop() {
    let d = tempfile::tempdir().unwrap();
    let (e, _) = boot(trio(), d.path());
    let (h, rx) = EngineHandle::new();
    let t = std::thread::spawn(move || run(e, rx));
    assert!(h.diagnostics(REQUEST_TIMEOUT).unwrap().engine.subscribed);
    assert!(h.quit(REQUEST_TIMEOUT));
    t.join().unwrap();
    assert_eq!(h.diagnostics(REQUEST_TIMEOUT), Err(Refusal::Gone));
}
