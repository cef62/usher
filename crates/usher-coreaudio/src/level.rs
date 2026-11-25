//! Volume and mute reads and writes on a device's main element.
use objc2_core_audio::{
    kAudioDevicePropertyMute, kAudioDevicePropertyScopeInput, kAudioDevicePropertyScopeOutput,
    kAudioDevicePropertyVolumeScalar, kAudioHardwareUnknownPropertyError,
};
use usher_core::{DeviceKey, Level, Role, clamp_volume};

use crate::AudioError;
use crate::props::{get_f32, get_u32, has, set_f32, set_u32, settable};
use crate::write::translate;

/// `kAudioHardwareServiceDeviceProperty_VirtualMainVolume` ('vmvc'). objc2-core-audio omits it.
/// A read-only probe (macOS 27.2, built-in speaker and mic) found it present and settable on the
/// main element, with working get and listener add/remove, in both scopes: no AudioToolbox needed.
pub(crate) const VIRTUAL_MAIN_VOLUME: u32 = 0x766d7663;

pub(crate) fn scope(role: Role) -> u32 {
    match role {
        Role::Output => kAudioDevicePropertyScopeOutput,
        Role::Input => kAudioDevicePropertyScopeInput,
    }
}

/// Each pair is (present, settable). A control needs both. Virtual main volume wins.
pub(crate) fn volume_selector(vmvc: (bool, bool), scalar: (bool, bool)) -> Option<u32> {
    if vmvc.0 && vmvc.1 {
        Some(VIRTUAL_MAIN_VOLUME)
    } else if scalar.0 && scalar.1 {
        Some(kAudioDevicePropertyVolumeScalar)
    } else {
        None
    }
}

fn probe(id: u32, selector: u32, sc: u32) -> (bool, bool) {
    let present = has(id, selector, sc);
    (present, present && settable(id, selector, sc))
}

fn pick_volume(id: u32, sc: u32) -> Option<u32> {
    volume_selector(
        probe(id, VIRTUAL_MAIN_VOLUME, sc),
        probe(id, kAudioDevicePropertyVolumeScalar, sc),
    )
}

fn has_mute(id: u32, sc: u32) -> bool {
    probe(id, kAudioDevicePropertyMute, sc) == (true, true)
}

fn no_control(call: &'static str) -> AudioError {
    // no honest OSStatus exists for "no control": report the unknown-property error
    AudioError::Status {
        call,
        status: kAudioHardwareUnknownPropertyError,
    }
}

pub(crate) fn levels(role: Role, key: &DeviceKey) -> Result<Level, AudioError> {
    let id = translate(key)?;
    let sc = scope(role);
    let volume = match pick_volume(id, sc) {
        Some(sel) => Some(get_f32(id, sel, sc)?),
        None => None,
    };
    let muted = if has_mute(id, sc) {
        Some(get_u32(id, kAudioDevicePropertyMute, sc)? != 0)
    } else {
        None
    };
    Ok(Level { volume, muted })
}

pub(crate) fn set_volume(role: Role, key: &DeviceKey, value: f32) -> Result<(), AudioError> {
    let id = translate(key)?;
    let sc = scope(role);
    let sel = pick_volume(id, sc).ok_or_else(|| no_control("set volume"))?;
    set_f32(id, sel, sc, clamp_volume(value))
}

pub(crate) fn set_mute(role: Role, key: &DeviceKey, on: bool) -> Result<(), AudioError> {
    let id = translate(key)?;
    let sc = scope(role);
    if !has_mute(id, sc) {
        return Err(no_control("set mute"));
    }
    set_u32(id, kAudioDevicePropertyMute, sc, u32::from(on))
}

/// Selectors worth a listener on this device: only the controls that exist.
pub(crate) fn watched(id: u32, role: Role) -> Vec<u32> {
    let sc = scope(role);
    let mut v: Vec<u32> = pick_volume(id, sc).into_iter().collect();
    if has_mute(id, sc) {
        v.push(kAudioDevicePropertyMute);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_selector_prefers_virtual_main_then_scalar() {
        assert_eq!(
            volume_selector((true, true), (true, true)),
            Some(VIRTUAL_MAIN_VOLUME)
        );
        assert_eq!(
            volume_selector((true, false), (true, true)),
            Some(kAudioDevicePropertyVolumeScalar)
        );
        assert_eq!(volume_selector((false, false), (true, false)), None); // readable but not settable: no control
    }
}
