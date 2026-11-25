//! The real `AudioSystem`, backed by CoreAudio.
use objc2_core_audio::{
    kAudioHardwarePropertyDefaultInputDevice, kAudioHardwarePropertyDefaultOutputDevice,
};
use usher_core::{DeviceKey, Level, Role};

use crate::listen::Listeners;
use crate::watch::LevelWatch;
use crate::{AudioError, AudioSystem, Notify, Scan, level, scan, write};

/// CoreAudio-backed `AudioSystem`. It holds `RcBlock`s, so it is `!Send`:
/// construct it on the thread that uses it.
#[derive(Default)]
pub struct CoreAudioSystem {
    listeners: Listeners,
    levels: LevelWatch,
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
        self.levels.clear_all();
        self.listeners.unsubscribe();
    }
    fn rebind(&mut self) -> Result<(), AudioError> {
        self.levels.clear_all();
        self.listeners.rebind()
    }
    fn levels(&self, role: Role, key: &DeviceKey) -> Result<Level, AudioError> {
        level::levels(role, key)
    }
    fn default_key(&self, role: Role) -> Result<Option<DeviceKey>, AudioError> {
        let selector = match role {
            Role::Output => kAudioHardwarePropertyDefaultOutputDevice,
            Role::Input => kAudioHardwarePropertyDefaultInputDevice,
        };
        scan::default_uid(selector, &[])
    }
    fn set_volume(&self, role: Role, key: &DeviceKey, value: f32) -> Result<(), AudioError> {
        level::set_volume(role, key, value)
    }
    fn set_mute(&self, role: Role, key: &DeviceKey, on: bool) -> Result<(), AudioError> {
        level::set_mute(role, key, on)
    }
    fn watch_levels(&mut self, role: Role, key: Option<&DeviceKey>) -> Result<(), AudioError> {
        // not subscribed yet: no `Notify` to forward to
        match self.listeners.notify() {
            Some(n) => self.levels.watch(role, key, n),
            None => Ok(()),
        }
    }
}

impl Drop for CoreAudioSystem {
    fn drop(&mut self) {
        self.levels.clear_all();
        self.listeners.unsubscribe();
    }
}
