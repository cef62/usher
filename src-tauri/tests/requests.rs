mod common;
use common::*;
use std::sync::mpsc;
use std::time::Duration;
use usher_core::{Command, CommandError, DeviceKey, Role, State};
use usher_coreaudio::AudioEvent;
use usher_lib::engine::{Engine, EngineHandle, Event, REQUEST_TIMEOUT, Refusal, run};

fn trio() -> Fake {
    Fake::with(
        vec![spk(), pods(), mic()],
        "BuiltInSpeakerDevice",
        "BuiltInMicrophoneDevice",
    )
}

fn ask(e: &mut Engine<Fake, Recorder>, cmd: Command, t: u64) -> Result<(), Refusal> {
    let (tx, rx) = mpsc::channel();
    e.handle(Event::Command(cmd, Some(tx)), at(t));
    rx.try_recv().unwrap()
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
    let r = ask(&mut e, Command::Forget { key: key.into() }, 10);
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
