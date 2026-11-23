//! Set the default devices by UID. The `AudioObjectID` lives only inside one call.
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::NonNull;

use objc2_core_audio::{
    AudioObjectGetPropertyData, AudioObjectPropertyAddress,
    kAudioHardwarePropertyDefaultInputDevice, kAudioHardwarePropertyDefaultOutputDevice,
    kAudioHardwarePropertyDefaultSystemOutputDevice, kAudioHardwarePropertyTranslateUIDToDevice,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject,
};
use objc2_core_foundation::CFString;
use usher_core::{DeviceKey, Role};

use crate::props::set_u32;
use crate::{AudioError, check};

const SYSTEM: u32 = kAudioObjectSystemObject as u32;

/// `kAudioObjectUnknown` (0) means CoreAudio has no device for the UID.
pub(crate) fn found(id: u32, key: &DeviceKey) -> Result<u32, AudioError> {
    if id == 0 {
        Err(AudioError::NotFound(key.clone()))
    } else {
        Ok(id)
    }
}

fn translate(key: &DeviceKey) -> Result<u32, AudioError> {
    let uid = CFString::from_str(key.as_str());
    let qualifier: *const CFString = &*uid;
    let mut a = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyTranslateUIDToDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    let mut id = 0u32;
    let mut n = size_of::<u32>() as u32;
    // SAFETY: the qualifier is a pointer-sized `CFStringRef`, so we pass the address of the
    // `qualifier` local with size `size_of::<*const CFString>()`; CoreAudio only reads the string,
    // and `uid` keeps it alive until after the call. `a`, `n` and `id` are live stack values and
    // `n` equals the size of `id`.
    let st = unsafe {
        AudioObjectGetPropertyData(
            SYSTEM,
            NonNull::from(&mut a),
            size_of::<*const CFString>() as u32,
            (&raw const qualifier).cast::<c_void>(),
            NonNull::from(&mut n),
            NonNull::from(&mut id).cast::<c_void>(),
        )
    };
    check("AudioObjectGetPropertyData", st)?;
    found(id, key)
}

pub(crate) fn set_default(role: Role, key: &DeviceKey) -> Result<(), AudioError> {
    let id = translate(key)?;
    let main = match role {
        Role::Output => kAudioHardwarePropertyDefaultOutputDevice,
        Role::Input => kAudioHardwarePropertyDefaultInputDevice,
    };
    set_u32(SYSTEM, main, id)?;
    if role == Role::Output
        && let Err(e) = set_u32(SYSTEM, kAudioHardwarePropertyDefaultSystemOutputDevice, id)
    {
        log::warn!("system output not moved to {key}: {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AudioError;
    use usher_core::DeviceKey;

    #[test]
    fn unknown_object_is_not_found() {
        let k = DeviceKey::from("gone-uid");
        assert_eq!(found(0, &k), Err(AudioError::NotFound(k.clone())));
        assert_eq!(found(73, &k), Ok(73));
    }
}
