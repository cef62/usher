//! The heart of the core: one snapshot in, the writes the engine must make out.
use crate::{
    DeviceKey, LoopGuard, PausedReason, Reason, Role, Snapshot, State, Timestamp, decide,
    merge_aliases, remember,
};
use std::collections::BTreeSet;

#[derive(Clone, PartialEq, Debug)]
pub enum Action {
    /// The engine sets the role's default; for `Output` it also sets the system output.
    SetDefault { role: Role, key: DeviceKey },
    /// The guard tripped: `auto_switch` is already false and `paused_reason` set.
    /// `key` is the device that kept taking the role.
    PauseAuto { role: Role, key: DeviceKey },
    /// Info log only.
    AliasMerged { from: DeviceKey, to: DeviceKey },
    /// `State` changed; the engine schedules the debounced save.
    Persist,
}

/// In memory only.
#[derive(Default)]
pub struct Runtime {
    pub guard: LoopGuard,
    connected: Option<BTreeSet<DeviceKey>>,
}

pub struct Outcome {
    pub actions: Vec<Action>,
    pub reasons: Vec<Reason>,
}

pub fn reconcile(state: &mut State, rt: &mut Runtime, snap: &Snapshot, now: Timestamp) -> Outcome {
    let before = state.clone();
    let mut actions: Vec<Action> = merge_aliases(state, snap)
        .into_iter()
        .map(|m| Action::AliasMerged {
            from: m.from,
            to: m.to,
        })
        .collect();
    remember(state, snap, rt.connected.as_ref(), now);
    rt.connected = Some(snap.devices.iter().map(|d| d.uid.clone()).collect());
    let mut reasons = Vec::new();
    for role in Role::ALL {
        let reason = decide(state, snap, role);
        actions.extend(correct(state, rt, snap, &reason, now));
        reasons.push(reason);
    }
    if *state != before {
        actions.push(Action::Persist);
    }
    Outcome { actions, reasons }
}

/// A missing default (`current` None) is written without the guard: nothing is fighting us.
fn correct(
    state: &mut State,
    rt: &mut Runtime,
    snap: &Snapshot,
    reason: &Reason,
    now: Timestamp,
) -> Option<Action> {
    let role = reason.role;
    let desired = reason.desired.clone()?;
    if !state.auto_switch || reason.current.as_ref() == Some(&desired) {
        return None;
    }
    let Some(taker) = reason.current.clone() else {
        return Some(Action::SetDefault { role, key: desired });
    };
    if !rt.guard.record(role, now) {
        return Some(Action::SetDefault { role, key: desired });
    }
    let name = snap
        .device(&taker)
        .map(|d| d.name.clone())
        .or_else(|| state.devices.get(&taker).map(|r| r.name.clone()))
        .unwrap_or_else(|| taker.0.clone());
    state.auto_switch = false;
    state.paused_reason = Some(PausedReason {
        role,
        key: taker.clone(),
        name,
    });
    Some(Action::PauseAuto { role, key: taker })
}
