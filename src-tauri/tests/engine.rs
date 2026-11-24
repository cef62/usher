mod common;
use common::*;
use usher_core::{DeviceKey, Role, State};
use usher_coreaudio::{AudioError, AudioEvent};
use usher_lib::engine::{Event, Flow};

fn ranked(out: &[&str]) -> State {
    let mut s = State::default();
    // mic ranks first so a newly seen AirPods never takes the input
    s.input.order = vec!["BuiltInMicrophoneDevice".into()];
    s.output.order = out.iter().map(|k| DeviceKey::from(*k)).collect();
    s
}

fn trio() -> Fake {
    Fake::with(
        vec![spk(), pods(), mic()],
        "BuiltInSpeakerDevice",
        "BuiltInMicrophoneDevice",
    )
}

fn failing_pods(dir: &std::path::Path) -> (usher_lib::engine::Engine<Fake, Recorder>, Fake) {
    let fake = trio();
    state_file(dir, &ranked(&["AirPods-uid", "BuiltInSpeakerDevice"]));
    fake.world().fail_set.insert(
        "AirPods-uid".into(),
        AudioError::Status {
            call: "set default",
            status: -50,
        },
    );
    let (e, _) = boot(fake.clone(), dir);
    (e, fake)
}

#[test]
fn launch_without_file_seeds_saves_and_switches_nothing() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    // a reconcile before the seed would see pods as the unranked newcomer and switch
    fake.world().current.output = Some("AirPods-uid".into());
    let (e, _) = boot(fake.clone(), d.path());
    assert_eq!(e.saves(), 1);
    assert!(fake.world().sets.is_empty());
    assert_eq!(read_state(d.path()).output.order[0].as_str(), "AirPods-uid");
}

#[test]
fn due_usb_recheck_is_absorbed_by_the_coalesced_reconcile() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().usb_unresolved = true;
    let (mut e, _) = boot(fake.clone(), d.path());
    e.handle(Event::Audio(AudioEvent::DevicesChanged), at(400));
    e.tick(at(600));
    assert_eq!(fake.world().scans, 2);
    // the absorbing scan is the recheck: it seeds and acts, and no further recheck is scheduled
    assert_eq!(e.saves(), 1);
    e.tick(at(1_200));
    assert_eq!(fake.world().scans, 2);
}

#[test]
fn launch_with_unresolved_usb_waits_for_the_recheck_before_switching() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    // AirPods hold the output, unranked: a reconcile switches to the speakers
    fake.world().current.output = Some("AirPods-uid".into());
    fake.world().usb_unresolved = true;
    state_file(d.path(), &ranked(&["BuiltInSpeakerDevice"]));
    let (mut e, _) = boot(fake.clone(), d.path());
    assert!(fake.world().sets.is_empty());
    fake.world().usb_unresolved = false;
    e.tick(at(500));
    assert_eq!(fake.world().scans, 2);
    assert_eq!(fake.world().sets.len(), 1);
}

#[test]
fn first_launch_with_unresolved_usb_seeds_at_the_recheck() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().usb_unresolved = true;
    let (mut e, _) = boot(fake.clone(), d.path());
    assert_eq!(e.saves(), 0);
    // still unresolved at the recheck: usher acts anyway, a verdict may never come (IOKit failure)
    e.tick(at(500));
    assert_eq!(e.saves(), 1);
}

#[test]
fn unresolved_usb_after_a_device_event_defers_the_switch_to_the_recheck() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    state_file(d.path(), &ranked(&["AirPods-uid", "BuiltInSpeakerDevice"]));
    let (mut e, _) = boot(fake.clone(), d.path());
    let sets = fake.world().sets.len();
    fake.world().usb_unresolved = true;
    fake.world().current.output = Some("BuiltInSpeakerDevice".into());
    e.handle(Event::Audio(AudioEvent::DevicesChanged), at(1_000));
    e.tick(at(1_150));
    assert_eq!(fake.world().sets.len(), sets);
    e.tick(at(1_650));
    assert_eq!(fake.world().sets.len(), sets + 1);
}

#[test]
fn events_inside_150ms_coalesce_into_one_scan() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    let (mut e, _) = boot(fake.clone(), d.path());
    assert_eq!(fake.world().scans, 1);
    e.handle(Event::Audio(AudioEvent::DevicesChanged), at(1_000));
    e.handle(
        Event::Audio(AudioEvent::DefaultChanged(Role::Output)),
        at(1_100),
    );
    assert_eq!(e.next_due(), Some(at(1_150)));
    e.tick(at(1_149));
    assert_eq!(fake.world().scans, 1);
    e.tick(at(1_150));
    assert_eq!(fake.world().scans, 2);
}

#[test]
fn desired_output_is_set_once() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    state_file(d.path(), &ranked(&["AirPods-uid", "BuiltInSpeakerDevice"]));
    let _ = boot(fake.clone(), d.path());
    assert_eq!(
        fake.world().sets,
        vec![(Role::Output, "AirPods-uid".into())]
    );
}

#[test]
fn failed_set_shows_an_error_and_is_not_retried_in_the_event() {
    let d = tempfile::tempdir().unwrap();
    let (e, fake) = failing_pods(d.path());
    assert_eq!(fake.world().sets.len(), 1);
    assert_eq!(
        e.view().errors.output.as_deref(),
        Some("couldn't switch output to AirPods: set default failed (OSStatus -50)")
    );
    assert_eq!(
        fake.world().current.output.as_ref().unwrap().as_str(),
        "BuiltInSpeakerDevice"
    );
}

#[test]
fn vanished_device_is_an_error_line_not_a_panic() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    state_file(d.path(), &ranked(&["AirPods-uid", "BuiltInSpeakerDevice"]));
    fake.world().vanish_on_set.insert("AirPods-uid".into());
    let (e, _) = boot(fake, d.path());
    assert_eq!(
        e.view().errors.output.as_deref(),
        Some("couldn't switch output to AirPods: device AirPods-uid is gone")
    );
}

#[test]
fn error_clears_once_desired_equals_current() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, fake) = failing_pods(d.path());
    assert!(e.view().errors.output.is_some());
    {
        let mut w = fake.world();
        w.fail_set.clear();
        w.current.output = Some("AirPods-uid".into());
    }
    e.handle(
        Event::Audio(AudioEvent::DefaultChanged(Role::Output)),
        at(1_000),
    );
    e.tick(at(1_150));
    assert_eq!(e.view().errors.output, None);
}

#[test]
fn guard_trip_pauses_and_tells_the_shell() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().current.output = Some("AirPods-uid".into());
    fake.world().sticky.output = Some("AirPods-uid".into());
    state_file(d.path(), &ranked(&["BuiltInSpeakerDevice", "AirPods-uid"]));
    let (mut e, rec) = boot(fake.clone(), d.path());
    e.handle(
        Event::Audio(AudioEvent::DefaultChanged(Role::Output)),
        at(200),
    );
    e.tick(at(350));
    e.handle(
        Event::Audio(AudioEvent::DefaultChanged(Role::Output)),
        at(500),
    );
    e.tick(at(650));
    assert_eq!(fake.world().sets.len(), 2);
    assert!(rec.calls().contains(&Call::Auto(false)));
    assert!(matches!(rec.last_paused(), Some(p) if p.name == "AirPods" && p.role == Role::Output));
    assert!(!e.state().auto_switch);
    // the trip itself must be persisted by the reconcile, not only by a later toggle
    e.tick(at(1_150));
    let saved = read_state(d.path());
    assert!(!saved.auto_switch);
    assert!(saved.paused_reason.is_some());
}

#[test]
fn toggling_auto_switch_on_reconciles_and_sets() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    let mut s = ranked(&["AirPods-uid", "BuiltInSpeakerDevice"]);
    s.auto_switch = false;
    state_file(d.path(), &s);
    let (mut e, _) = boot(fake.clone(), d.path());
    assert!(fake.world().sets.is_empty());
    e.handle(Event::ToggleAutoSwitch, at(1_000));
    e.tick(at(1_150));
    assert_eq!(
        fake.world().sets,
        vec![(Role::Output, "AirPods-uid".into())]
    );
}

#[test]
fn toggle_saves_after_500ms_not_before() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, rec) = boot(trio(), d.path());
    let base = e.saves();
    e.handle(Event::ToggleAutoSwitch, at(1_000));
    assert!(rec.calls().contains(&Call::Auto(false)));
    e.tick(at(1_499));
    assert_eq!(e.saves(), base);
    e.tick(at(1_500));
    assert_eq!(e.saves(), base + 1);
    assert!(!read_state(d.path()).auto_switch);
}

#[test]
fn change_after_a_save_is_in_the_next_save() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, _) = boot(trio(), d.path());
    e.handle(Event::ToggleAutoSwitch, at(1_000));
    e.tick(at(1_500));
    e.handle(Event::ToggleAutoSwitch, at(1_501));
    e.tick(at(2_001));
    assert!(read_state(d.path()).auto_switch);
}

#[test]
fn unresolved_usb_rescans_once_after_500ms() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().usb_unresolved = true;
    let (mut e, _) = boot(fake.clone(), d.path());
    e.tick(at(500));
    assert_eq!(fake.world().scans, 2);
    e.tick(at(1_000));
    assert_eq!(fake.world().scans, 2);
}

#[test]
fn panel_view_serialises_errors_beside_the_panel() {
    let d = tempfile::tempdir().unwrap();
    let (e, _) = boot(trio(), d.path());
    let v = serde_json::to_value(e.view()).unwrap();
    assert!(v.get("autoSwitch").is_some());
    assert!(v["errors"].get("output").is_some());
}

#[test]
fn failed_scan_at_launch_leaves_state_unseeded_and_drops_commands() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().fail_scan = 1;
    let (mut e, _) = boot(fake, d.path());
    assert_eq!(e.saves(), 0);
    e.handle(
        Event::Command(usher_core::Command::SetDebug(true), None),
        at(10),
    );
    assert!(!e.state().debug);
}

#[test]
fn toggle_before_first_scan_reasserts_the_check_mark() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().fail_scan = 1;
    let (mut e, rec) = boot(fake, d.path());
    e.handle(Event::ToggleAutoSwitch, at(10));
    assert!(e.state().auto_switch);
    let autos: Vec<_> = rec
        .calls()
        .into_iter()
        .filter(|c| matches!(c, Call::Auto(_)))
        .collect();
    assert_eq!(autos, vec![Call::Auto(true), Call::Auto(true)]); // launch, then the toggle
}

#[test]
fn quit_flushes_and_stops() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, _) = boot(trio(), d.path());
    e.handle(Event::ToggleAutoSwitch, at(1_000));
    let (tx, rx) = std::sync::mpsc::channel();
    assert!(matches!(e.handle(Event::Quit(tx), at(1_001)), Flow::Stop));
    rx.recv().unwrap();
    assert!(!read_state(d.path()).auto_switch);
}

#[test]
fn usb_plugged_after_launch_gets_its_rescan() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    let (mut e, _) = boot(fake.clone(), d.path());
    fake.world().usb_unresolved = true;
    e.handle(Event::Audio(AudioEvent::DevicesChanged), at(1_000));
    e.tick(at(1_150));
    e.tick(at(1_650));
    assert_eq!(fake.world().scans, 3);
    e.tick(at(3_000));
    assert_eq!(fake.world().scans, 3);
}

#[test]
fn a_failed_recheck_scan_at_launch_retries_and_seeds() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    fake.world().usb_unresolved = true;
    let (mut e, _) = boot(fake.clone(), d.path());
    fake.world().fail_scan = 1;
    e.tick(at(500));
    assert_eq!(e.saves(), 0);
    assert_eq!(e.next_due(), Some(at(1_500)));
    // the retry scan defers once more to its own recheck
    e.tick(at(1_500));
    assert_eq!(e.saves(), 0);
    e.tick(at(2_000));
    assert_eq!(e.saves(), 1);
}

#[test]
fn a_failed_recheck_scan_after_a_restart_retries_and_switches_back() {
    let d = tempfile::tempdir().unwrap();
    let fake = trio();
    state_file(d.path(), &ranked(&["AirPods-uid", "BuiltInSpeakerDevice"]));
    fake.world().current.output = Some("AirPods-uid".into());
    let (mut e, _) = boot(fake.clone(), d.path());
    fake.world().current.output = Some("BuiltInSpeakerDevice".into());
    fake.world().usb_unresolved = true;
    e.handle(Event::Audio(AudioEvent::ServiceRestarted), at(1_000));
    e.tick(at(1_000));
    fake.world().fail_scan = 1;
    e.tick(at(1_500));
    assert!(fake.world().sets.is_empty());
    e.tick(at(2_500));
    e.tick(at(3_000));
    assert_eq!(fake.world().sets.len(), 1);
}
