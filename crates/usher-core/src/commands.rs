//! Panel actions as pure state transitions. The engine runs `reconcile` after every `Ok`.
use crate::identity::registry;
use crate::{DeviceKey, Role, Runtime, Snapshot, State};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, PartialEq, Debug)]
pub enum Command {
    Reorder {
        role: Role,
        order: Vec<DeviceKey>,
    },
    Promote {
        role: Role,
        key: DeviceKey,
    },
    SetNeverUse {
        role: Role,
        key: DeviceKey,
        on: bool,
    },
    Forget {
        key: DeviceKey,
    },
    SetAutoSwitch(bool),
    SetDebug(bool),
}

#[derive(Clone, PartialEq, Debug)]
pub enum CommandError {
    UnknownDevice(DeviceKey),
    StillConnected(DeviceKey),
    StaleOrder(Role),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::UnknownDevice(k) => write!(f, "unknown device {}", k.as_str()),
            CommandError::StillConnected(k) => {
                write!(f, "device {} is still connected", k.as_str())
            }
            CommandError::StaleOrder(r) => write!(f, "the {r} list changed; reload and retry"),
        }
    }
}

impl std::error::Error for CommandError {}

/// Every arm validates before it mutates, so an `Err` leaves `state` untouched.
pub fn apply(
    state: &mut State,
    rt: &mut Runtime,
    snap: &Snapshot,
    cmd: Command,
) -> Result<(), CommandError> {
    match cmd {
        Command::Reorder { role, order } => reorder(state, role, order),
        Command::Promote { role, key } => promote(state, role, &key),
        Command::SetNeverUse { role, key, on } => set_never_use(state, role, key, on),
        Command::Forget { key } => forget(state, snap, &key),
        Command::SetAutoSwitch(on) => {
            set_auto_switch(state, rt, on);
            Ok(())
        }
        Command::SetDebug(on) => {
            state.debug = on;
            Ok(())
        }
    }
}

fn reorder(state: &mut State, role: Role, order: Vec<DeviceKey>) -> Result<(), CommandError> {
    let stored = &state.list(role).order;
    let same: BTreeSet<_> = order.iter().collect();
    if order.len() != stored.len() || same != stored.iter().collect() {
        return Err(CommandError::StaleOrder(role));
    }
    state.list_mut(role).order = order;
    Ok(())
}

fn position(state: &State, role: Role, key: &DeviceKey) -> Result<usize, CommandError> {
    let pos = state.list(role).order.iter().position(|k| k == key);
    pos.ok_or_else(|| CommandError::UnknownDevice(key.clone()))
}

fn promote(state: &mut State, role: Role, key: &DeviceKey) -> Result<(), CommandError> {
    let pos = position(state, role, key)?;
    state.list_mut(role).order[..=pos].rotate_right(1);
    Ok(())
}

fn set_never_use(
    state: &mut State,
    role: Role,
    key: DeviceKey,
    on: bool,
) -> Result<(), CommandError> {
    position(state, role, &key)?;
    let never = &mut state.list_mut(role).never_use;
    if on {
        never.insert(key);
    } else {
        never.remove(&key);
    }
    Ok(())
}

fn forget(state: &mut State, snap: &Snapshot, key: &DeviceKey) -> Result<(), CommandError> {
    let listed = state.devices.contains_key(key)
        || Role::ALL.iter().any(|&r| state.list(r).order.contains(key));
    if !listed {
        return Err(CommandError::UnknownDevice(key.clone()));
    }
    if snap.is_connected(key) {
        return Err(CommandError::StillConnected(key.clone()));
    }
    registry::remove(state, key);
    Ok(())
}

fn set_auto_switch(state: &mut State, rt: &mut Runtime, on: bool) {
    state.auto_switch = on;
    if on {
        state.paused_reason = None;
        rt.guard.reset();
    }
}
