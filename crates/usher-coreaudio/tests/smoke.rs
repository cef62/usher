// Re-enabled in Task 4, when `CoreAudioSystem` exists: delete the next line.
#![cfg(any())]
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
