//! Volume and mute: live levels of each role's active device, and the engine-owned writes. Nothing here touches the
//! store, the reconcile schedule, the Activity log or the notices.

use log::{debug, warn};
use usher_core::{DeviceKey, Level, LevelRequest, PerRole, Role, clamp_volume};
use usher_coreaudio::{AudioError, AudioSystem};

use super::{Engine, Refusal, Shell};

#[derive(Default)]
pub(super) struct Levels {
    /// The role's active device and its last good read (`None` when the read failed).
    cur: PerRole<Option<(DeviceKey, Option<Level>)>>,
    errors: PerRole<Option<String>>,
    refused: Option<Refusal>,
}

impl Levels {
    pub(super) fn levels(&self) -> PerRole<Option<Level>> {
        let of = |s: &Option<(DeviceKey, Option<Level>)>| s.as_ref().and_then(|(_, l)| *l);
        PerRole {
            output: of(&self.cur.output),
            input: of(&self.cur.input),
        }
    }

    pub(super) fn errors(&self) -> PerRole<Option<String>> {
        self.errors.clone()
    }
}

impl<A: AudioSystem, S: Shell> Engine<A, S> {
    /// After every reconcile: move each role's watch to its current device, then read the levels.
    pub(super) fn sync_levels(&mut self) {
        for role in Role::ALL {
            let key = self.active_key(role);
            if let Err(e) = self.audio.watch_levels(role, key.as_ref()) {
                warn!("couldn't watch {role} levels: {e}");
            }
            self.read_levels(role);
        }
    }

    pub(super) fn on_level_changed(&mut self, role: Role) {
        if self.snap.is_none() || self.rebinding() {
            return;
        }
        if self.read_levels(role) {
            self.emit();
        }
    }

    pub(super) fn on_level(&mut self, req: LevelRequest) -> Result<(), Refusal> {
        let r = self.write_checked(&req);
        match &r {
            Err(why) if self.levels.refused.as_ref() != Some(why) => {
                debug!("level request refused: {why}");
                self.levels.refused = Some(why.clone());
            }
            _ => {}
        }
        r
    }

    fn active_key(&self, role: Role) -> Option<DeviceKey> {
        self.snap.as_ref()?.current.get(role).clone()
    }

    /// Re-read the role's levels. `true` when what the panel shows changed.
    fn read_levels(&mut self, role: Role) -> bool {
        let key = self.active_key(role);
        let read = key.map(|k| {
            let l = self.audio.levels(role, &k);
            if let Err(e) = &l {
                debug!("couldn't read {role} levels of {k}: {e}");
            }
            (k, l.ok())
        });
        let slot = self.levels.cur.get_mut(role);
        if *slot == read {
            return false;
        }
        if slot.as_ref().map(|s| &s.0) != read.as_ref().map(|r| &r.0) {
            *self.levels.errors.get_mut(role) = None;
        }
        *slot = read;
        true
    }

    fn write_checked(&mut self, req: &LevelRequest) -> Result<(), Refusal> {
        let Some(snap) = &self.snap else {
            return Err(Refusal::NotReady);
        };
        if self.rebinding() {
            return Err(Refusal::Rebinding);
        }
        let (role, key) = (req.role(), req.key());
        // the live default, not the last scan: a switch may not have reached the engine yet
        if !matches!(self.audio.default_key(role), Ok(Some(k)) if &k == key) {
            return Err(Refusal::Stale);
        }
        let name = self.name_of(snap, key);
        let (what, done) = match req {
            LevelRequest::Volume { value, .. } => {
                let v = clamp_volume(*value);
                debug!("set {role} volume to {}% on {name}", (v * 100.0).round());
                ("volume", self.audio.set_volume(role, key, v))
            }
            LevelRequest::Mute { on, .. } => {
                debug!(
                    "set {role} mute {} on {name}",
                    if *on { "on" } else { "off" }
                );
                ("mute", self.audio.set_mute(role, key, *on))
            }
        };
        let error = done
            .err()
            .map(|e| self.write_failed(role, what, &name, key, &e));
        if error.is_none() {
            self.levels.refused = None;
        }
        let was = std::mem::replace(self.levels.errors.get_mut(role), error);
        if self.read_levels(role) | (was != *self.levels.errors.get(role)) {
            self.emit();
        }
        Ok(())
    }

    fn write_failed(
        &self,
        role: Role,
        what: &str,
        name: &str,
        key: &DeviceKey,
        e: &AudioError,
    ) -> String {
        warn!("couldn't set {role} {what} on {name} ({key}): {e}");
        format!("Couldn't set {what}: {e}")
    }
}
