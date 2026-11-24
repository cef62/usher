mod common;
use common::*;
use usher_core::{Command, Entry, Role, Why};
use usher_coreaudio::{AudioError, AudioEvent};
use usher_lib::engine::Event;

const SPK: &str = "BuiltInSpeakerDevice";
const PODS: &str = "AirPods-uid";

fn world(devices: Vec<usher_core::DeviceSnapshot>, out: &str) -> Fake {
    Fake::with(devices, out, "BuiltInMicrophoneDevice")
}

fn why(e: &Entry) -> Option<String> {
    e.why_text()
}

fn dev_event(e: &mut Engine, ev: AudioEvent, t: u64) {
    e.handle(Event::Audio(ev), at(t));
}

type Engine = usher_lib::engine::Engine<Fake, Recorder>;

fn fail_pods(fake: &Fake) {
    fake.world().fail_set.insert(
        PODS.into(),
        AudioError::Status {
            call: "set default",
            status: -50,
        },
    );
}

#[test]
fn first_reconcile_with_a_lower_ranked_default_is_launch_history() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[PODS, SPK]));
    let (_e, rec) = boot(world(vec![spk(), pods(), mic()], SPK), d.path());
    assert!(matches!(
        rec.entries()[..],
        [Entry::Switched {
            role: Role::Output,
            why: Why::Launch,
            ..
        }]
    ));
}

#[test]
fn rebind_that_restores_the_desired_device_is_rebind_history() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[PODS, SPK]));
    let fake = world(vec![spk(), pods(), mic()], PODS);
    let (mut e, rec) = boot(fake.clone(), d.path());
    dev_event(&mut e, AudioEvent::ServiceRestarted, 100);
    fake.world().current.output = Some(SPK.into());
    e.tick(at(1_200));
    assert!(matches!(
        rec.entries().last(),
        Some(Entry::Switched {
            why: Why::Rebind,
            ..
        })
    ));
}

#[test]
fn unplug_reads_as_disconnected_not_corrected() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[PODS, "DisplayUID", SPK]));
    let fake = world(vec![spk(), pods(), disp(), mic()], PODS);
    let (mut e, rec) = boot(fake.clone(), d.path());
    fake.world().devices.retain(|x| x.uid != PODS.into());
    fake.world().current.output = Some(SPK.into());
    dev_event(&mut e, AudioEvent::DevicesChanged, 100);
    dev_event(&mut e, AudioEvent::DefaultChanged(Role::Output), 110);
    e.tick(at(300));
    let last = rec.entries().last().cloned().unwrap();
    assert_eq!(why(&last).as_deref(), Some("AirPods disconnected"));
    assert_eq!(last.text(), "Output: AirPods → Studio Display");
}

#[test]
fn a_connect_is_recorded_with_its_cause() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[PODS, SPK]));
    let fake = world(vec![spk(), mic()], SPK);
    let (mut e, rec) = boot(fake.clone(), d.path());
    fake.world().devices.push(pods());
    dev_event(&mut e, AudioEvent::DevicesChanged, 100);
    e.tick(at(300));
    let last = rec.entries().last().cloned().unwrap();
    assert_eq!(why(&last).as_deref(), Some("AirPods connected"));
}

#[test]
fn a_taken_default_is_corrected() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, PODS]));
    let fake = world(vec![spk(), pods(), mic()], SPK);
    let (mut e, rec) = boot(fake.clone(), d.path());
    fake.world().current.output = Some(PODS.into());
    dev_event(&mut e, AudioEvent::DefaultChanged(Role::Output), 100);
    e.tick(at(300));
    let last = rec.entries().last().cloned().unwrap();
    assert_eq!(
        why(&last).as_deref(),
        Some("AirPods took output; usher put it back")
    );
}

fn pods_first() -> Vec<usher_core::DeviceKey> {
    vec![PODS.into(), SPK.into()]
}

#[test]
fn a_reorder_is_ranked() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, PODS]));
    let (mut e, rec) = boot(world(vec![spk(), pods(), mic()], SPK), d.path());
    let order = pods_first();
    let cmd = Command::Reorder {
        role: Role::Output,
        order,
    };
    assert_eq!(ask(&mut e, cmd, 100), Ok(()));
    e.tick(at(300));
    assert!(matches!(
        rec.entries().last(),
        Some(Entry::Switched {
            why: Why::Ranked,
            ..
        })
    ));
}

#[test]
fn a_failed_write_is_recorded() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[PODS, SPK]));
    let fake = world(vec![spk(), pods(), mic()], SPK);
    fail_pods(&fake);
    let (_e, rec) = boot(fake, d.path());
    assert_eq!(
        rec.entries().last().unwrap().text(),
        "Couldn't switch output to AirPods: set default failed (OSStatus -50)"
    );
}

#[test]
fn a_failed_command_write_is_history_only() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, PODS]));
    let fake = world(vec![spk(), pods(), mic()], SPK);
    fail_pods(&fake);
    let (mut e, rec) = boot(fake, d.path());
    let cmd = Command::Promote {
        role: Role::Output,
        key: PODS.into(),
    };
    assert_eq!(ask(&mut e, cmd, 100), Ok(()));
    e.tick(at(300));
    assert!(matches!(
        rec.entries().last(),
        Some(Entry::Failed {
            role: Role::Output,
            ..
        })
    ));
}

#[test]
fn a_trip_records_paused_and_turning_on_records_resumed() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, PODS]));
    let fake = world(vec![spk(), pods(), mic()], PODS);
    fake.world().sticky.output = Some(PODS.into());
    let (mut e, rec) = boot(fake, d.path());
    for t in [200, 500] {
        dev_event(&mut e, AudioEvent::DefaultChanged(Role::Output), t);
        e.tick(at(t + 150));
    }
    assert!(
        rec.entries()
            .iter()
            .any(|x| matches!(x, Entry::Paused { .. }))
    );
    assert_eq!(ask(&mut e, Command::SetAutoSwitch(true), 700), Ok(()));
    assert_eq!(rec.entries().last(), Some(&Entry::Resumed));
}

#[test]
fn a_reconcile_that_changes_nothing_records_nothing() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, PODS]));
    let (mut e, rec) = boot(world(vec![spk(), pods(), mic()], SPK), d.path());
    dev_event(&mut e, AudioEvent::DevicesChanged, 100);
    e.tick(at(300));
    assert!(rec.entries().is_empty());
}

#[test]
fn a_pending_command_outranks_a_rebind() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, PODS]));
    let (mut e, rec) = boot(world(vec![spk(), pods(), mic()], SPK), d.path());
    let promote = Command::Promote {
        role: Role::Output,
        key: PODS.into(),
    };
    assert_eq!(ask(&mut e, promote, 10), Ok(()));
    dev_event(&mut e, AudioEvent::ServiceRestarted, 20);
    e.tick(at(1_200));
    assert!(matches!(
        rec.entries().last(),
        Some(Entry::Switched {
            why: Why::Ranked,
            ..
        })
    ));
}

#[test]
fn a_deferred_command_keeps_its_trigger() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, PODS]));
    let fake = world(vec![spk(), pods(), mic()], SPK);
    let (mut e, rec) = boot(fake.clone(), d.path());
    fake.world().usb_unresolved = true;
    let cmd = Command::Reorder {
        role: Role::Output,
        order: pods_first(),
    };
    assert_eq!(ask(&mut e, cmd, 100), Ok(()));
    e.tick(at(300));
    assert!(rec.entries().is_empty());
    fake.world().usb_unresolved = false;
    e.tick(at(800));
    assert!(matches!(
        rec.entries().last(),
        Some(Entry::Switched {
            why: Why::Ranked,
            ..
        })
    ));
}

fn correct(e: &mut Engine, fake: &Fake, t: u64) {
    fake.world().current.output = Some(PODS.into());
    dev_event(e, AudioEvent::DefaultChanged(Role::Output), t);
    e.tick(at(t + 150));
}

#[test]
fn a_connect_notifies_once_for_both_roles() {
    let d = tempfile::tempdir().unwrap();
    let mut s = ranked(&[PODS, SPK]);
    s.input.order = vec![PODS.into(), "BuiltInMicrophoneDevice".into()];
    state_file(d.path(), &s);
    let fake = world(vec![spk(), mic()], SPK);
    let (mut e, rec) = boot(fake.clone(), d.path());
    fake.world().devices.push(pods());
    dev_event(&mut e, AudioEvent::DevicesChanged, 10_100);
    e.tick(at(10_300));
    assert_eq!(
        rec.notices(),
        vec!["Output: MacBook Pro Speakers → AirPods\nInput: MacBook Pro Microphone → AirPods"]
    );
}

#[test]
fn launch_rebind_and_commands_never_notify() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[PODS, SPK]));
    let fake = world(vec![spk(), pods(), mic()], SPK);
    let (mut e, rec) = boot(fake.clone(), d.path());
    dev_event(&mut e, AudioEvent::ServiceRestarted, 100);
    fake.world().current.output = Some(SPK.into());
    e.tick(at(1_200));
    let order = vec![SPK.into(), PODS.into()];
    let cmd = Command::Reorder {
        role: Role::Output,
        order,
    };
    assert_eq!(ask(&mut e, cmd, 2_000), Ok(()));
    e.tick(at(2_300));
    assert!(rec.entries().len() >= 3, "{:?}", rec.entries());
    assert_eq!(rec.notices(), Vec::<String>::new());

    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, PODS]));
    let fake = world(vec![spk(), pods(), mic()], SPK);
    fail_pods(&fake);
    let (mut e, rec) = boot(fake, d.path());
    let cmd = Command::Promote {
        role: Role::Output,
        key: PODS.into(),
    };
    assert_eq!(ask(&mut e, cmd, 100), Ok(()));
    e.tick(at(300));
    assert!(matches!(rec.entries().last(), Some(Entry::Failed { .. })));
    assert_eq!(rec.notices(), Vec::<String>::new());
}

#[test]
fn notices_are_capped_per_role() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, PODS]));
    let fake = world(vec![spk(), pods(), mic()], SPK);
    let (mut e, rec) = boot(fake.clone(), d.path());
    for t in [10_200, 15_000, 21_000] {
        correct(&mut e, &fake, t);
    }
    assert_eq!(rec.notices().len(), 2);
    assert_eq!(rec.entries().len(), 3);
}

fn quiet_world(d: &std::path::Path) -> (Engine, Recorder, Fake) {
    state_file(d, &ranked(&[SPK, PODS]));
    let fake = world(vec![spk(), pods(), mic()], SPK);
    let (e, rec) = boot(fake.clone(), d);
    (e, rec, fake)
}

#[test]
fn no_notice_in_the_first_10_s_after_launch() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, rec, fake) = quiet_world(d.path());
    correct(&mut e, &fake, 5_000);
    assert_eq!(rec.entries().len(), 1, "history still gets the entry");
    assert!(rec.notices().is_empty());
    correct(&mut e, &fake, 11_000);
    assert_eq!(rec.notices().len(), 1);
}

#[test]
fn no_notice_in_the_first_10_s_after_a_rebind() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, rec, fake) = quiet_world(d.path());
    dev_event(&mut e, AudioEvent::ServiceRestarted, 20_000);
    e.tick(at(21_200));
    correct(&mut e, &fake, 26_200);
    assert!(rec.notices().is_empty());
    assert_eq!(rec.entries().len(), 1, "history still gets the entry");
    correct(&mut e, &fake, 32_000);
    assert_eq!(rec.notices().len(), 1);
}

#[test]
fn paused_inside_the_quiet_period_still_notifies() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, PODS]));
    let fake = world(vec![spk(), pods(), mic()], PODS);
    fake.world().sticky.output = Some(PODS.into());
    let (mut e, rec) = boot(fake, d.path());
    dev_event(&mut e, AudioEvent::DefaultChanged(Role::Output), 200);
    e.tick(at(350));
    dev_event(&mut e, AudioEvent::DefaultChanged(Role::Output), 500);
    e.tick(at(650));
    assert!(rec.calls().iter().any(|c| matches!(c, Call::Paused(_))));
}

#[test]
fn notices_off_still_records_history() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, PODS]));
    let fake = world(vec![spk(), pods(), mic()], SPK);
    let (mut e, rec) = boot(fake.clone(), d.path());
    assert_eq!(ask(&mut e, Command::SetNotify(false), 10), Ok(()));
    correct(&mut e, &fake, 200);
    assert_eq!(rec.entries().len(), 1);
    assert!(rec.notices().is_empty());
    e.tick(at(3_000));
    assert!(!read_state(d.path()).notify_switches);
}

#[test]
fn the_trip_notifies_only_through_paused() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, PODS]));
    let fake = world(vec![spk(), pods(), mic()], PODS);
    fake.world().sticky.output = Some(PODS.into());
    let (mut e, rec) = boot(fake, d.path());
    dev_event(&mut e, AudioEvent::DefaultChanged(Role::Output), 200);
    e.tick(at(350));
    let before = rec.notices();
    dev_event(&mut e, AudioEvent::DefaultChanged(Role::Output), 500);
    e.tick(at(650));
    let paused = rec
        .calls()
        .iter()
        .filter(|c| matches!(c, Call::Paused(_)))
        .count();
    assert_eq!(paused, 1);
    assert_eq!(
        rec.notices(),
        before,
        "the tripping reconcile adds no notice"
    );
}

#[test]
fn a_failed_switch_notifies() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[PODS, SPK]));
    let fake = world(vec![spk(), mic()], SPK);
    let (mut e, rec) = boot(fake.clone(), d.path());
    fake.world().devices.push(pods());
    fail_pods(&fake);
    dev_event(&mut e, AudioEvent::DevicesChanged, 10_100);
    e.tick(at(10_300));
    assert_eq!(
        rec.notices(),
        vec!["Couldn't switch output to AirPods: set default failed (OSStatus -50)"]
    );
}

fn trio() -> Fake {
    Fake::with(vec![spk(), pods(), mic()], SPK, "BuiltInMicrophoneDevice")
}

fn diagnose(e: &mut Engine, t: u64) -> usher_core::Diagnostics {
    let (tx, rx) = std::sync::mpsc::channel();
    e.handle(Event::Diagnostics(tx), at(t));
    rx.recv().unwrap()
}

#[test]
fn diagnostics_answer_before_the_first_scan() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().fail_scan = 5;
    let (mut e, _) = boot(fake, d.path());
    let d = diagnose(&mut e, 10);
    assert!(!d.engine.scanned && d.devices.is_empty());
    assert_eq!(d.engine.rebinding, Some(2), "first retry pending");
}

#[test]
fn diagnostics_report_the_scan_and_the_ranked_skips() {
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[PODS, SPK]));
    let fake = trio();
    fake.world().devices.retain(|d| d.uid.0 != PODS);
    let (mut e, _) = boot(fake, d.path());
    e.tick(at(1_000));
    let d = diagnose(&mut e, 1_000);
    assert!(d.engine.scanned && d.engine.subscribed && !d.engine.read_only);
    assert_eq!(
        d.roles[0].ranked[0].skip,
        Some(usher_core::Skip::Disconnected)
    );
    assert_eq!(d.devices.len(), 2);
}
