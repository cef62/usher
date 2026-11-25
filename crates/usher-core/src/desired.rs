use crate::{DeviceKey, Reason, Role, Skip, Skipped, Snapshot, State};

pub(crate) fn skip_cause(
    state: &State,
    snap: &Snapshot,
    role: Role,
    key: &DeviceKey,
) -> Option<Skip> {
    let Some(dev) = snap.device(key) else {
        return Some(Skip::Disconnected);
    };
    if !dev.alive {
        Some(Skip::NotAlive)
    } else if dev.hidden {
        Some(Skip::Hidden)
    } else if dev.streams(role) == 0 {
        Some(Skip::NoStreams)
    } else if state.list(role).never_use.contains(key) {
        Some(Skip::NeverUse)
    } else {
        None
    }
}

/// Strict policy: first usable key in `order(role)` wins.
pub fn decide(state: &State, snap: &Snapshot, role: Role) -> Reason {
    let mut skipped = Vec::new();
    let mut desired = None;
    for key in &state.list(role).order {
        match skip_cause(state, snap, role, key) {
            None => {
                desired = Some(key.clone());
                break;
            }
            Some(why) => skipped.push(Skipped {
                key: key.clone(),
                why,
            }),
        }
    }
    Reason {
        role,
        desired,
        current: snap.current.get(role).clone(),
        skipped,
    }
}
