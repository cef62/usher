//! System-object listeners. Every block is gated by a generation counter, so a callback
//! already in flight when we unsubscribe becomes a no-op.
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use block2::RcBlock;
use objc2_core_audio::{
    AudioObjectAddPropertyListenerBlock, AudioObjectPropertyAddress,
    AudioObjectRemovePropertyListenerBlock, kAudioHardwarePropertyDefaultInputDevice,
    kAudioHardwarePropertyDefaultOutputDevice, kAudioHardwarePropertyDevices,
    kAudioHardwarePropertyServiceRestarted, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject,
};
use usher_core::Role;

use crate::{AudioError, AudioEvent, Notify, check};

type Listener = RcBlock<dyn Fn(u32, NonNull<AudioObjectPropertyAddress>)>;

#[derive(Clone, Default)]
pub(crate) struct Gate {
    generation: Arc<AtomicU64>,
}

impl Gate {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Forwards `event` only while the generation is the one current at creation.
    pub(crate) fn callback(
        &self,
        notify: Notify,
        event: AudioEvent,
    ) -> impl Fn() + Send + Sync + 'static {
        let live = self.generation.clone();
        let mine = live.load(Ordering::SeqCst);
        move || {
            if live.load(Ordering::SeqCst) == mine {
                notify(event);
            }
        }
    }

    pub(crate) fn bump(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
}

fn addr(selector: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    }
}

const EVENTS: [(u32, AudioEvent); 4] = [
    (kAudioHardwarePropertyDevices, AudioEvent::DevicesChanged),
    (
        kAudioHardwarePropertyDefaultOutputDevice,
        AudioEvent::DefaultChanged(Role::Output),
    ),
    (
        kAudioHardwarePropertyDefaultInputDevice,
        AudioEvent::DefaultChanged(Role::Input),
    ),
    (
        kAudioHardwarePropertyServiceRestarted,
        AudioEvent::ServiceRestarted,
    ),
];

#[derive(Default)]
pub(crate) struct Listeners {
    gate: Gate,
    notify: Option<Notify>,
    added: Vec<(u32, Listener)>,
}

fn remove(selector: u32, block: &Listener) -> i32 {
    let mut a = addr(selector);
    // SAFETY: `a` is a live stack address; `block` is the very block passed to Add for this selector.
    unsafe {
        AudioObjectRemovePropertyListenerBlock(
            kAudioObjectSystemObject as u32,
            NonNull::from(&mut a),
            None,
            RcBlock::as_ptr(block).cast(),
        )
    }
}

impl Listeners {
    pub(crate) fn subscribe(&mut self, notify: Notify) -> Result<(), AudioError> {
        self.unsubscribe();
        self.notify = Some(notify.clone());
        for (selector, event) in EVENTS {
            let gated = self.gate.callback(notify.clone(), event);
            let block: Listener =
                RcBlock::new(move |_n: u32, _a: NonNull<AudioObjectPropertyAddress>| gated());
            let mut a = addr(selector);
            // SAFETY: `a` is a live stack address; CoreAudio copies the block and keeps it until
            // the matching Remove, and we keep our `RcBlock` so the same pointer can be removed.
            let st = unsafe {
                AudioObjectAddPropertyListenerBlock(
                    kAudioObjectSystemObject as u32,
                    NonNull::from(&mut a),
                    None,
                    RcBlock::as_ptr(&block).cast(),
                )
            };
            if let Err(e) = check("AudioObjectAddPropertyListenerBlock", st) {
                for (s, b) in self.added.drain(..).rev() {
                    remove(s, &b);
                }
                return Err(e);
            }
            self.added.push((selector, block));
        }
        Ok(())
    }

    /// Idempotent.
    pub(crate) fn unsubscribe(&mut self) {
        self.gate.bump();
        for (s, b) in self.added.drain(..).rev() {
            let st = remove(s, &b);
            if st != 0 {
                log::warn!("AudioObjectRemovePropertyListenerBlock failed (OSStatus {st})");
            }
        }
    }

    pub(crate) fn rebind(&mut self) -> Result<(), AudioError> {
        self.unsubscribe();
        match self.notify.clone() {
            Some(n) => self.subscribe(n),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn counter() -> (Notify, Arc<Mutex<Vec<AudioEvent>>>) {
        let got = Arc::new(Mutex::new(Vec::new()));
        let g = got.clone();
        (Arc::new(move |e| g.lock().unwrap().push(e)), got)
    }

    #[test]
    fn live_generation_forwards_the_event() {
        let (n, got) = counter();
        let g = Gate::new();
        g.callback(n, AudioEvent::DefaultChanged(Role::Input))();
        assert_eq!(
            *got.lock().unwrap(),
            vec![AudioEvent::DefaultChanged(Role::Input)]
        );
    }

    #[test]
    fn stale_generation_is_dropped() {
        let (n, got) = counter();
        let g = Gate::new();
        let old = g.callback(n, AudioEvent::DevicesChanged);
        g.bump();
        old();
        assert!(got.lock().unwrap().is_empty());
    }
}
