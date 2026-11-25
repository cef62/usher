use std::sync::Arc;

use usher_core::Role;
use usher_coreaudio::{AudioSystem, CoreAudioSystem};

#[test]
#[ignore = "needs real audio hardware"]
fn enumerates_real_devices() {
    let scan = CoreAudioSystem::new().scan().unwrap();
    assert!(scan.snapshot.devices.iter().any(|d| d.output_streams > 0));
    assert!(
        scan.snapshot
            .devices
            .iter()
            .all(|d| !d.uid.as_str().is_empty())
    );
    let out = scan
        .snapshot
        .current
        .output
        .clone()
        .expect("a default output");
    assert!(scan.snapshot.is_connected(&out));
}

fn current_output(sys: &CoreAudioSystem) -> usher_core::DeviceKey {
    sys.scan()
        .unwrap()
        .snapshot
        .current
        .output
        .expect("a default output")
}

#[test]
#[ignore = "needs real audio hardware"]
fn set_default_to_current_output_is_ok() {
    let sys = CoreAudioSystem::new();
    let cur = current_output(&sys);
    assert_eq!(sys.set_default(Role::Output, &cur), Ok(()));
}

#[test]
#[ignore = "needs real audio hardware"]
fn subscribe_then_unsubscribe_is_clean() {
    let mut sys = CoreAudioSystem::new();
    assert_eq!(sys.subscribe(Arc::new(|_| {})), Ok(()));
    sys.unsubscribe();
    sys.unsubscribe();
}

#[test]
#[ignore = "needs real audio hardware"]
fn set_default_after_rebind_is_ok() {
    let mut sys = CoreAudioSystem::new();
    sys.subscribe(Arc::new(|_| {})).unwrap();
    sys.rebind().unwrap();
    let cur = current_output(&sys);
    assert_eq!(sys.set_default(Role::Output, &cur), Ok(()));
}

#[test]
#[ignore = "needs real audio hardware"]
fn reads_levels_of_the_current_output() {
    let sys = CoreAudioSystem::new();
    let cur = current_output(&sys);
    assert_eq!(sys.default_key(Role::Output), Ok(Some(cur.clone())));
    let level = sys.levels(Role::Output, &cur).unwrap();
    if let Some(v) = level.volume {
        assert!((0.0..=1.0).contains(&v));
    }
}
