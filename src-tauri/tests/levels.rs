mod common;
use common::*;
use usher_core::{Command, Level, LevelRequest, Role};
use usher_coreaudio::{AudioError, AudioEvent};
use usher_lib::engine::{Engine, Event, PanelView, Refusal};

const SPK: &str = "BuiltInSpeakerDevice";

fn lv(volume: f32) -> Level {
    Level {
        volume: Some(volume),
        muted: Some(false),
    }
}

fn world() -> Fake {
    let f = Fake::with(vec![spk(), pods(), mic()], SPK, "BuiltInMicrophoneDevice");
    f.world().levels.insert(SPK.into(), lv(0.5));
    f
}

fn vol(value: f32) -> LevelRequest {
    LevelRequest::Volume {
        role: Role::Output,
        key: SPK.into(),
        value,
    }
}

fn level(e: &mut Engine<Fake, Recorder>, req: LevelRequest, t: u64) -> Result<(), Refusal> {
    let (tx, rx) = std::sync::mpsc::channel();
    e.handle(Event::Level(req, Some(tx)), at(t));
    rx.try_recv().unwrap()
}

fn last_view(rec: &Recorder) -> PanelView {
    rec.calls()
        .into_iter()
        .rev()
        .find_map(|c| match c {
            Call::View(v) => Some(v),
            _ => None,
        })
        .unwrap()
}

fn views(rec: &Recorder) -> usize {
    rec.calls()
        .iter()
        .filter(|c| matches!(c, Call::View(_)))
        .count()
}

#[test]
fn level_changed_emits_without_a_scan() {
    let d = tempfile::tempdir().unwrap();
    let fake = world();
    let (mut e, rec) = boot(fake.clone(), d.path());
    assert_eq!(last_view(&rec).levels.output, Some(lv(0.5)));
    let scans = fake.world().scans;
    fake.world().levels.insert(SPK.into(), lv(0.8));
    e.handle(Event::Audio(AudioEvent::LevelChanged(Role::Output)), at(10));
    assert_eq!(fake.world().scans, scans);
    assert_eq!(e.next_due(), None);
    assert_eq!(last_view(&rec).levels.output, Some(lv(0.8)));
}

#[test]
fn an_unchanged_level_emits_nothing() {
    let d = tempfile::tempdir().unwrap();
    let (mut e, rec) = boot(world(), d.path());
    let n = views(&rec);
    e.handle(Event::Audio(AudioEvent::LevelChanged(Role::Output)), at(10));
    assert_eq!(views(&rec), n);
}

#[test]
fn a_volume_request_writes_the_active_device_only() {
    let d = tempfile::tempdir().unwrap();
    let fake = world();
    let (mut e, rec) = boot(fake.clone(), d.path());
    let (saves, scans) = (e.saves(), fake.world().scans);
    assert_eq!(level(&mut e, vol(0.4), 10), Ok(()));
    assert_eq!(fake.world().level_writes, vec![vol(0.4)]);
    assert_eq!(last_view(&rec).levels.output, Some(lv(0.4)));
    assert_eq!((e.saves(), fake.world().scans), (saves, scans));
    assert!(rec.entries().is_empty() && rec.notices().is_empty());
    assert_eq!(e.next_due(), None);
}

#[test]
fn values_are_clamped() {
    let d = tempfile::tempdir().unwrap();
    let fake = world();
    let (mut e, _) = boot(fake.clone(), d.path());
    assert_eq!(level(&mut e, vol(1.5), 10), Ok(()));
    assert_eq!(fake.world().level_writes, vec![vol(1.0)]);
}

#[test]
fn a_request_for_a_device_that_is_no_longer_default_is_stale() {
    let d = tempfile::tempdir().unwrap();
    let fake = world();
    let (mut e, _) = boot(fake.clone(), d.path());
    fake.world().current.output = Some("AirPods-uid".into());
    assert_eq!(level(&mut e, vol(0.3), 10), Err(Refusal::Stale));
    fake.world().current.output = Some(SPK.into());
    fake.world().fail_default_read = true;
    assert_eq!(level(&mut e, vol(0.3), 20), Err(Refusal::Stale));
    assert!(fake.world().level_writes.is_empty());
}

#[test]
fn requests_before_the_first_scan_or_during_a_rebind_are_refused() {
    let d = tempfile::tempdir().unwrap();
    let fake = world();
    fake.world().fail_scan = 1;
    let (mut e, _) = boot(fake.clone(), d.path());
    assert_eq!(level(&mut e, vol(0.3), 10), Err(Refusal::NotReady));

    let d = tempfile::tempdir().unwrap();
    let fake = world();
    let (mut e, _) = boot(fake.clone(), d.path());
    e.handle(Event::Audio(AudioEvent::ServiceRestarted), at(10));
    assert_eq!(level(&mut e, vol(0.3), 11), Err(Refusal::Rebinding));
    assert!(fake.world().level_writes.is_empty());
}

#[test]
fn a_failed_write_shows_the_error_until_the_next_success() {
    let d = tempfile::tempdir().unwrap();
    let fake = world();
    let (mut e, rec) = boot(fake.clone(), d.path());
    fake.world().fail_level = Some(AudioError::Status {
        call: "set volume",
        status: -50,
    });
    fake.world().levels.insert(SPK.into(), lv(0.55));
    assert_eq!(level(&mut e, vol(0.3), 10), Ok(()));
    let v = last_view(&rec);
    assert_eq!(
        v.level_errors.output.as_deref(),
        Some("Couldn't set volume: set volume failed (OSStatus -50)")
    );
    assert_eq!(v.levels.output, Some(lv(0.55)));
    fake.world().fail_level = None;
    assert_eq!(level(&mut e, vol(0.3), 20), Ok(()));
    assert_eq!(last_view(&rec).level_errors.output, None);
}

#[test]
fn a_mute_without_a_control_reports_the_error() {
    let d = tempfile::tempdir().unwrap();
    let fake = world();
    fake.world().levels.insert(
        SPK.into(),
        Level {
            volume: Some(0.5),
            muted: None,
        },
    );
    let (mut e, rec) = boot(fake, d.path());
    let req = LevelRequest::Mute {
        role: Role::Output,
        key: SPK.into(),
        on: true,
    };
    assert_eq!(level(&mut e, req, 10), Ok(()));
    let err = last_view(&rec).level_errors.output.unwrap();
    assert!(err.starts_with("Couldn't set mute: "), "{err}");
}

#[test]
fn watches_follow_every_reconcile() {
    let d = tempfile::tempdir().unwrap();
    let fake = world();
    let (mut e, _) = boot(fake.clone(), d.path());
    let last = |f: &Fake, r: Role| {
        f.world()
            .watches
            .iter()
            .rev()
            .find(|(x, _)| *x == r)
            .cloned()
            .unwrap()
            .1
    };
    assert_eq!(last(&fake, Role::Output), Some(SPK.into()));
    fake.world().current.output = Some("AirPods-uid".into());
    e.handle(
        Event::Audio(AudioEvent::DefaultChanged(Role::Output)),
        at(10),
    );
    e.tick(at(1000));
    assert_eq!(last(&fake, Role::Output), Some("AirPods-uid".into()));

    let n = fake.world().watches.len();
    e.handle(Event::Audio(AudioEvent::ServiceRestarted), at(2000));
    e.tick(at(2000));
    assert_eq!(fake.world().rebinds, 1);
    assert!(fake.world().watches.len() > n);

    fake.world()
        .devices
        .retain(|d| d.uid.as_str() != "AirPods-uid");
    fake.world().current.output = None;
    e.handle(Event::Audio(AudioEvent::DevicesChanged), at(3000));
    e.tick(at(4000));
    assert_eq!(last(&fake, Role::Output), None);
}

#[test]
fn auto_switch_off_paused_or_read_only_still_sets_levels() {
    let d = tempfile::tempdir().unwrap();
    let fake = world();
    let (mut e, _) = boot(fake.clone(), d.path());
    assert_eq!(ask(&mut e, Command::SetAutoSwitch(false), 10), Ok(()));
    assert_eq!(level(&mut e, vol(0.2), 20), Ok(()));
    assert_eq!(fake.world().level_writes, vec![vol(0.2)]);

    // read-only store
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("state.json"), r#"{"schema_version":2}"#).unwrap();
    let fake = world();
    let (mut e, _) = boot(fake.clone(), d.path());
    assert!(e.view().read_only);
    assert_eq!(level(&mut e, vol(0.3), 10), Ok(()));
    assert_eq!(fake.world().level_writes, vec![vol(0.3)]);

    // paused by the loop guard
    let d = tempfile::tempdir().unwrap();
    state_file(d.path(), &ranked(&[SPK, "AirPods-uid"]));
    let fake = world();
    fake.world().current.output = Some("AirPods-uid".into());
    fake.world().sticky.output = Some("AirPods-uid".into());
    fake.world().levels.insert("AirPods-uid".into(), lv(0.6));
    let (mut e, rec) = boot(fake.clone(), d.path());
    for t in [200, 500] {
        e.handle(
            Event::Audio(AudioEvent::DefaultChanged(Role::Output)),
            at(t),
        );
        e.tick(at(t + 150));
    }
    assert!(rec.last_paused().is_some());
    let req = LevelRequest::Volume {
        role: Role::Output,
        key: "AirPods-uid".into(),
        value: 0.7,
    };
    assert_eq!(level(&mut e, req.clone(), 1_000), Ok(()));
    assert_eq!(fake.world().level_writes, vec![req]);
}

#[test]
fn level_changed_before_the_first_scan_or_during_a_rebind_is_ignored() {
    let d = tempfile::tempdir().unwrap();
    let fake = world();
    fake.world().fail_scan = 1;
    let (mut e, rec) = boot(fake.clone(), d.path());
    let (n, scans) = (views(&rec), fake.world().scans);
    e.handle(Event::Audio(AudioEvent::LevelChanged(Role::Output)), at(10));
    assert_eq!((views(&rec), fake.world().scans), (n, scans));

    let d = tempfile::tempdir().unwrap();
    let fake = world();
    let (mut e, rec) = boot(fake.clone(), d.path());
    e.handle(Event::Audio(AudioEvent::ServiceRestarted), at(10));
    fake.world().levels.insert(SPK.into(), lv(0.9));
    let (n, scans) = (views(&rec), fake.world().scans);
    e.handle(Event::Audio(AudioEvent::LevelChanged(Role::Output)), at(11));
    assert_eq!((views(&rec), fake.world().scans), (n, scans));
}

#[test]
fn a_new_current_device_clears_the_level_error() {
    let d = tempfile::tempdir().unwrap();
    let fake = world();
    let (mut e, rec) = boot(fake.clone(), d.path());
    fake.world().fail_level = Some(AudioError::Status {
        call: "set volume",
        status: -50,
    });
    assert_eq!(level(&mut e, vol(0.3), 10), Ok(()));
    assert!(last_view(&rec).level_errors.output.is_some());
    fake.world().current.output = Some("AirPods-uid".into());
    fake.world().levels.insert("AirPods-uid".into(), lv(0.6));
    e.handle(
        Event::Audio(AudioEvent::DefaultChanged(Role::Output)),
        at(20),
    );
    e.tick(at(200));
    let v = last_view(&rec);
    assert_eq!(v.level_errors.output, None);
    assert_eq!(v.levels.output, Some(lv(0.6)));
}
