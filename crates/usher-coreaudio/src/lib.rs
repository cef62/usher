//! Safe CoreAudio binding for usher: the `AudioSystem` seam the engine drives.
#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]

use std::fmt;
use std::sync::Arc;

// dead_code until CoreAudioSystem (Task 4) calls these.
#[allow(dead_code)]
mod props;
#[allow(dead_code)]
mod scan;
#[allow(dead_code)]
mod usb;

use usher_core::{DeviceKey, Role, Snapshot};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AudioEvent {
    DevicesChanged,
    DefaultChanged(Role),
    ServiceRestarted,
}

#[derive(Clone, PartialEq, Debug)]
pub enum AudioError {
    NotFound(DeviceKey),
    Status { call: &'static str, status: i32 },
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioError::NotFound(key) => write!(f, "device {} is gone", key.as_str()),
            AudioError::Status { call, status } => write!(f, "{call} failed (OSStatus {status})"),
        }
    }
}

impl std::error::Error for AudioError {}

pub type Notify = Arc<dyn Fn(AudioEvent) + Send + Sync>;

pub struct Scan {
    pub snapshot: Snapshot,
    pub usb_unresolved: bool,
}

pub trait AudioSystem {
    fn scan(&self) -> Result<Scan, AudioError>;
    fn set_default(&self, role: Role, key: &DeviceKey) -> Result<(), AudioError>;
    fn subscribe(&mut self, notify: Notify) -> Result<(), AudioError>;
    fn unsubscribe(&mut self);
    /// unsubscribe, then subscribe again with the last `Notify`
    fn rebind(&mut self) -> Result<(), AudioError>;
}

pub(crate) fn check(call: &'static str, status: i32) -> Result<(), AudioError> {
    if status == 0 {
        Ok(())
    } else {
        Err(AudioError::Status { call, status })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_status_is_ok() {
        assert_eq!(check("set default", 0), Ok(()));
    }
    #[test]
    fn nonzero_status_is_error_with_code() {
        assert_eq!(
            check("set default", 560227702),
            Err(AudioError::Status {
                call: "set default",
                status: 560227702
            })
        );
        assert_eq!(
            AudioError::Status {
                call: "set default",
                status: -50
            }
            .to_string(),
            "set default failed (OSStatus -50)"
        );
    }
    #[test]
    fn not_found_names_the_uid() {
        assert_eq!(
            AudioError::NotFound("BuiltInSpeakerDevice".into()).to_string(),
            "device BuiltInSpeakerDevice is gone"
        );
    }
}
