#![allow(dead_code)]
use usher_core::{DeviceKey, DeviceSnapshot, PerRole, Snapshot, Timestamp, Transport};

pub const T0: Timestamp = Timestamp(1_700_000_000_000);

pub fn key(s: &str) -> DeviceKey {
    DeviceKey::from(s)
}

pub fn dev(uid: &str, outputs: u32, inputs: u32) -> DeviceSnapshot {
    DeviceSnapshot {
        uid: key(uid),
        name: uid.to_string(),
        transport: Transport::BuiltIn,
        alive: true,
        hidden: false,
        input_streams: inputs,
        output_streams: outputs,
        serialless_usb: false,
    }
}

pub fn snap(devices: Vec<DeviceSnapshot>, output: Option<&str>, input: Option<&str>) -> Snapshot {
    Snapshot {
        devices,
        current: PerRole {
            output: output.map(key),
            input: input.map(key),
        },
    }
}

pub fn order(state: &usher_core::State, role: usher_core::Role) -> Vec<&str> {
    state
        .list(role)
        .order
        .iter()
        .map(|k| k.0.as_str())
        .collect()
}
