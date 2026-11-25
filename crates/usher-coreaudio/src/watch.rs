//! Per-role level listeners on the active device. The one place an `AudioObjectID` outlives a
//! call (ADR 0013): `Holding` keeps it from registration to removal.
use std::ptr::NonNull;

use block2::RcBlock;
use objc2_core_audio::{
    AudioObjectAddPropertyListenerBlock, AudioObjectRemovePropertyListenerBlock,
};
use usher_core::{DeviceKey, PerRole, Role};

use crate::AudioError;
use crate::level::{scope, watched};
use crate::listen::{Gate, Listener, listener};
use crate::props::addr;
use crate::write::translate;
use crate::{AudioEvent, Notify, check};

pub(crate) struct Holding {
    id: u32,
    key: DeviceKey,
    added: Vec<(u32, Listener)>,
}

/// A new registration is needed unless the same `AudioObjectID` is already watched.
fn needs(held: Option<&Holding>, id: u32) -> bool {
    held.is_none_or(|h| h.id != id)
}

#[derive(Default)]
pub(crate) struct LevelWatch {
    gates: PerRole<Gate>,
    held: PerRole<Option<Holding>>,
}

fn remove(id: u32, role: Role, selector: u32, block: &Listener) -> i32 {
    let mut a = addr(selector, scope(role));
    // SAFETY: `a` is a live stack address; `block` is the very block passed to Add for this
    // device, selector and scope.
    unsafe {
        AudioObjectRemovePropertyListenerBlock(
            id,
            NonNull::from(&mut a),
            None,
            RcBlock::as_ptr(block).cast(),
        )
    }
}

fn add(id: u32, role: Role, selector: u32, block: &Listener) -> Result<(), AudioError> {
    let mut a = addr(selector, scope(role));
    // SAFETY: `a` is a live stack address; CoreAudio copies the block and keeps it until the
    // matching Remove, and the caller keeps the `RcBlock` so the same pointer can be removed.
    let st = unsafe {
        AudioObjectAddPropertyListenerBlock(
            id,
            NonNull::from(&mut a),
            None,
            RcBlock::as_ptr(block).cast(),
        )
    };
    check("AudioObjectAddPropertyListenerBlock", st)
}

impl LevelWatch {
    /// Move `role`'s listeners to `key`'s device; `None` removes them.
    pub(crate) fn watch(
        &mut self,
        role: Role,
        key: Option<&DeviceKey>,
        notify: &Notify,
    ) -> Result<(), AudioError> {
        let Some(key) = key else {
            self.clear(role);
            return Ok(());
        };
        let id = match translate(key) {
            Ok(id) => id,
            Err(e) => {
                self.clear(role);
                return Err(e);
            }
        };
        if !needs(self.held.get(role).as_ref(), id) {
            return Ok(());
        }
        self.clear(role);
        let mut held = Holding {
            id,
            key: key.clone(),
            added: Vec::new(),
        };
        for selector in watched(id, role) {
            let block = listener(
                self.gates
                    .get(role)
                    .callback(notify.clone(), AudioEvent::LevelChanged(role)),
            );
            if let Err(e) = add(id, role, selector, &block) {
                *self.held.get_mut(role) = Some(held);
                self.clear(role);
                return Err(e);
            }
            held.added.push((selector, block));
        }
        *self.held.get_mut(role) = Some(held);
        Ok(())
    }

    /// Idempotent. A failed remove (the device vanished) is expected and ignored.
    pub(crate) fn clear(&mut self, role: Role) {
        self.gates.get(role).bump();
        if let Some(h) = self.held.get_mut(role).take() {
            for (selector, block) in h.added.into_iter().rev() {
                let st = remove(h.id, role, selector, &block);
                if st != 0 {
                    log::debug!("level listener on {} not removed (OSStatus {st})", h.key);
                }
            }
        }
    }

    pub(crate) fn clear_all(&mut self) {
        self.clear(Role::Output);
        self.clear(Role::Input);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(id: u32) -> Holding {
        Holding {
            id,
            key: "k".into(),
            added: Vec::new(),
        }
    }

    #[test]
    fn holding_needs_a_new_registration_only_for_a_new_id() {
        assert!(needs(None, 7));
        assert!(!needs(Some(&held(7)), 7));
        // same key, new id (replug inside one coalescing window)
        assert!(needs(Some(&held(7)), 8));
    }
}
