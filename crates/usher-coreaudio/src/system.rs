//! The real `AudioSystem`, backed by CoreAudio.
use usher_core::{DeviceKey, Role};

use crate::listen::Listeners;
use crate::{AudioError, AudioSystem, Notify, Scan, scan, write};

/// CoreAudio-backed `AudioSystem`. It holds `RcBlock`s, so it is `!Send`:
/// construct it on the thread that uses it.
#[derive(Default)]
pub struct CoreAudioSystem {
    listeners: Listeners,
}

impl CoreAudioSystem {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AudioSystem for CoreAudioSystem {
    fn scan(&self) -> Result<Scan, AudioError> {
        scan::scan()
    }
    fn set_default(&self, role: Role, key: &DeviceKey) -> Result<(), AudioError> {
        write::set_default(role, key)
    }
    fn subscribe(&mut self, notify: Notify) -> Result<(), AudioError> {
        self.listeners.subscribe(notify)
    }
    fn unsubscribe(&mut self) {
        self.listeners.unsubscribe();
    }
    fn rebind(&mut self) -> Result<(), AudioError> {
        self.listeners.rebind()
    }
}

impl Drop for CoreAudioSystem {
    fn drop(&mut self) {
        self.listeners.unsubscribe();
    }
}
