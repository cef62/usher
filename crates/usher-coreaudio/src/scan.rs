//! Enumerate devices and defaults into a `Snapshot`.
// the binding's FourCC constants are camel-case and used as match patterns
#![allow(non_upper_case_globals)]
use objc2_core_audio::*;
use usher_core::{DeviceKey, DeviceSnapshot, PerRole, Snapshot, Transport};

use crate::props::{get_ids, get_string, get_u32, stream_count};
use crate::usb::{serialless, usb_nodes};
use crate::{AudioError, Scan};

const SYSTEM: u32 = kAudioObjectSystemObject as u32;
const GLOBAL: u32 = kAudioObjectPropertyScopeGlobal;

pub(crate) fn transport_from(fourcc: u32) -> Transport {
    match fourcc {
        kAudioDeviceTransportTypeBuiltIn => Transport::BuiltIn,
        kAudioDeviceTransportTypeUSB => Transport::Usb,
        kAudioDeviceTransportTypeBluetooth | kAudioDeviceTransportTypeBluetoothLE => {
            Transport::Bluetooth
        }
        kAudioDeviceTransportTypeHDMI => Transport::Hdmi,
        kAudioDeviceTransportTypeDisplayPort => Transport::DisplayPort,
        kAudioDeviceTransportTypeThunderbolt => Transport::Thunderbolt,
        kAudioDeviceTransportTypeAirPlay => Transport::AirPlay,
        kAudioDeviceTransportTypeVirtual => Transport::Virtual,
        kAudioDeviceTransportTypeAggregate | kAudioDeviceTransportTypeAutoAggregate => {
            Transport::Aggregate
        }
        _ => Transport::Other,
    }
}

fn uid_of(id: u32) -> Result<DeviceKey, AudioError> {
    Ok(DeviceKey::from(
        get_string(id, kAudioDevicePropertyDeviceUID)?.as_str(),
    ))
}

fn device(id: u32) -> Result<DeviceSnapshot, AudioError> {
    Ok(DeviceSnapshot {
        uid: uid_of(id)?,
        name: get_string(id, kAudioObjectPropertyName)?,
        transport: transport_from(get_u32(id, kAudioDevicePropertyTransportType, GLOBAL)?),
        alive: get_u32(id, kAudioDevicePropertyDeviceIsAlive, GLOBAL)? != 0,
        hidden: get_u32(id, kAudioDevicePropertyIsHidden, GLOBAL)? != 0,
        input_streams: stream_count(id, kAudioObjectPropertyScopeInput)?,
        output_streams: stream_count(id, kAudioObjectPropertyScopeOutput)?,
        serialless_usb: false,
    })
}

fn default_uid(selector: u32) -> Result<Option<DeviceKey>, AudioError> {
    match get_u32(SYSTEM, selector, GLOBAL)? {
        0 => Ok(None),
        id => Ok(Some(uid_of(id)?)),
    }
}

/// Marks serial-less USB devices; true when any USB verdict is unresolved.
fn resolve_usb(devices: &mut [DeviceSnapshot]) -> bool {
    if !devices.iter().any(|d| d.transport == Transport::Usb) {
        return false;
    }
    let nodes = usb_nodes();
    let mut unresolved = false;
    for d in devices.iter_mut().filter(|d| d.transport == Transport::Usb) {
        let verdict = serialless(d.uid.as_str(), &nodes);
        unresolved |= verdict.is_none();
        d.serialless_usb = verdict == Some(true);
    }
    unresolved
}

fn log_device(d: &DeviceSnapshot) {
    log::debug!(
        "device {} {:?} {:?} alive={} hidden={} in={} out={} serialless={}",
        d.uid.as_str(),
        d.name,
        d.transport,
        d.alive,
        d.hidden,
        d.input_streams,
        d.output_streams,
        d.serialless_usb
    );
}

pub(crate) fn scan() -> Result<Scan, AudioError> {
    let mut devices = Vec::new();
    for id in get_ids(SYSTEM, kAudioHardwarePropertyDevices)? {
        match device(id) {
            Ok(d) => devices.push(d),
            Err(e) => log::debug!("skipping vanished device: {e}"),
        }
    }
    let usb_unresolved = resolve_usb(&mut devices);
    devices.iter().for_each(log_device);
    let current = PerRole {
        output: default_uid(kAudioHardwarePropertyDefaultOutputDevice)?,
        input: default_uid(kAudioHardwarePropertyDefaultInputDevice)?,
    };
    Ok(Scan {
        snapshot: Snapshot { devices, current },
        usb_unresolved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_fourccs_map_to_core_transports() {
        let table = [
            (kAudioDeviceTransportTypeBuiltIn, Transport::BuiltIn),
            (kAudioDeviceTransportTypeUSB, Transport::Usb),
            (kAudioDeviceTransportTypeBluetooth, Transport::Bluetooth),
            (kAudioDeviceTransportTypeBluetoothLE, Transport::Bluetooth),
            (kAudioDeviceTransportTypeHDMI, Transport::Hdmi),
            (kAudioDeviceTransportTypeDisplayPort, Transport::DisplayPort),
            (kAudioDeviceTransportTypeThunderbolt, Transport::Thunderbolt),
            (kAudioDeviceTransportTypeAirPlay, Transport::AirPlay),
            (kAudioDeviceTransportTypeAggregate, Transport::Aggregate),
            (kAudioDeviceTransportTypeAutoAggregate, Transport::Aggregate),
            (kAudioDeviceTransportTypeVirtual, Transport::Virtual),
            (
                kAudioDeviceTransportTypeContinuityCaptureWired,
                Transport::Other,
            ),
            (0, Transport::Other),
        ];
        for (fourcc, want) in table {
            assert_eq!(transport_from(fourcc), want, "{fourcc:#x}");
        }
    }
}
