use crate::{Role, Snapshot, State, Timestamp, remember};

/// First-launch state: the current default per role ranks first, even if virtual.
/// The engine must save the seeded state immediately.
pub fn seed(snap: &Snapshot, now: Timestamp) -> State {
    let mut state = State::default();
    for role in Role::ALL {
        let current = snap.current.get(role).as_ref();
        let usable = current
            .and_then(|k| snap.device(k))
            .filter(|d| d.listable() && d.has_role(role));
        if let Some(d) = usable {
            state.list_mut(role).order.push(d.uid.clone());
        }
    }
    remember(&mut state, snap, None, now);
    state
}
