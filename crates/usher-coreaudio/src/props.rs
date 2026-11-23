//! Typed `AudioObject` property reads/writes. `id` is an `AudioObjectID` valid only for the call.
use std::ffi::c_void;
use std::mem::{MaybeUninit, size_of};
use std::ptr::NonNull;

use objc2_core_audio::{
    AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectPropertyAddress,
    AudioObjectSetPropertyData, kAudioDevicePropertyStreams, kAudioObjectPropertyElementMain,
};
use objc2_core_foundation::{CFRetained, CFString};

use crate::{AudioError, check};

fn addr(selector: u32, scope: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: kAudioObjectPropertyElementMain,
    }
}

fn size(id: u32, a: &mut AudioObjectPropertyAddress) -> Result<u32, AudioError> {
    let mut n = 0u32;
    // SAFETY: `a` and `n` are live stack values; no qualifier is passed.
    let st = unsafe {
        AudioObjectGetPropertyDataSize(
            id,
            NonNull::from(a),
            0,
            std::ptr::null(),
            NonNull::from(&mut n),
        )
    };
    check("AudioObjectGetPropertyDataSize", st)?;
    Ok(n)
}

/// Read a fixed-size `T`.
fn get<T: Copy>(id: u32, selector: u32, scope: u32) -> Result<T, AudioError> {
    let mut a = addr(selector, scope);
    let mut n = size_of::<T>() as u32;
    let mut out = MaybeUninit::<T>::uninit();
    // SAFETY: `a` is a stack address; `n` equals the size of `out`, which is written on success.
    let st = unsafe {
        AudioObjectGetPropertyData(
            id,
            NonNull::from(&mut a),
            0,
            std::ptr::null(),
            NonNull::from(&mut n),
            NonNull::from(&mut out).cast::<c_void>(),
        )
    };
    check("AudioObjectGetPropertyData", st)?;
    // SAFETY: status 0 means CoreAudio filled all `size_of::<T>()` bytes.
    Ok(unsafe { out.assume_init() })
}

pub(crate) fn get_u32(id: u32, selector: u32, scope: u32) -> Result<u32, AudioError> {
    get(id, selector, scope)
}

pub(crate) fn get_ids(id: u32, selector: u32) -> Result<Vec<u32>, AudioError> {
    let mut a = addr(selector, objc2_core_audio::kAudioObjectPropertyScopeGlobal);
    let mut n = size(id, &mut a)?;
    let mut ids = vec![0u32; n as usize / size_of::<u32>()];
    n = (ids.len() * size_of::<u32>()) as u32;
    // SAFETY: `a` is a stack address; `n` is the byte length of `ids`.
    let st = unsafe {
        AudioObjectGetPropertyData(
            id,
            NonNull::from(&mut a),
            0,
            std::ptr::null(),
            NonNull::from(&mut n),
            NonNull::new(ids.as_mut_ptr().cast::<c_void>()).unwrap_or(NonNull::dangling()),
        )
    };
    check("AudioObjectGetPropertyData", st)?;
    ids.truncate(n as usize / size_of::<u32>());
    Ok(ids)
}

pub(crate) fn get_string(id: u32, selector: u32) -> Result<String, AudioError> {
    let raw: *mut CFString = get(
        id,
        selector,
        objc2_core_audio::kAudioObjectPropertyScopeGlobal,
    )?;
    let nn = NonNull::new(raw).ok_or(AudioError::Status {
        call: "AudioObjectGetPropertyData",
        status: -1,
    })?;
    // SAFETY: CoreAudio returns CFString properties at +1; we take ownership of that retain.
    let s = unsafe { CFRetained::from_raw(nn) };
    Ok(s.to_string())
}

pub(crate) fn stream_count(id: u32, scope: u32) -> Result<u32, AudioError> {
    let mut a = addr(kAudioDevicePropertyStreams, scope);
    Ok(size(id, &mut a)? / size_of::<u32>() as u32)
}

pub(crate) fn set_u32(id: u32, selector: u32, value: u32) -> Result<(), AudioError> {
    let mut a = addr(selector, objc2_core_audio::kAudioObjectPropertyScopeGlobal);
    let mut v = value;
    // SAFETY: `a` and `v` are live stack values; the size passed is that of `v`.
    let st = unsafe {
        AudioObjectSetPropertyData(
            id,
            NonNull::from(&mut a),
            0,
            std::ptr::null(),
            size_of::<u32>() as u32,
            NonNull::from(&mut v).cast::<c_void>(),
        )
    };
    check("AudioObjectSetPropertyData", st)
}
