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
    assert_eq!(last.text(), "Output: MacBook Pro Speakers → Studio Display");
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
