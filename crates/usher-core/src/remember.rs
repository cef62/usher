use crate::identity::alias_identity;
use crate::{DeviceKey, DeviceRecord, DeviceSnapshot, Role, Snapshot, State, Timestamp};
use std::collections::BTreeSet;

/// Adds newly seen devices to the bottom of the lists, refreshes their records
/// and stamps `last_seen`. Public only so `tests/` can drive it.
#[doc(hidden)]
pub fn remember(
    state: &mut State,
    snap: &Snapshot,
    prev: Option<&BTreeSet<DeviceKey>>,
    now: Timestamp,
) {
    for d in snap.devices.iter().filter(|d| d.listable()) {
        append_new(state, d);
        upsert_record(state, d);
    }
    stamp_last_seen(state, snap, prev, now);
}

fn append_new(state: &mut State, d: &DeviceSnapshot) {
    for role in Role::ALL.into_iter().filter(|r| d.has_role(*r)) {
        let list = state.list_mut(role);
        if !list.order.contains(&d.uid) {
            list.order.push(d.uid.clone());
            if d.transport.is_virtual() {
                list.never_use.insert(d.uid.clone());
            }
        }
    }
}

fn upsert_record(state: &mut State, d: &DeviceSnapshot) {
    let rec = state
        .devices
        .entry(d.uid.clone())
        .or_insert_with(|| DeviceRecord {
            name: String::new(),
            transport: d.transport,
            roles: BTreeSet::new(),
            last_seen: None,
            family: None,
        });
    rec.name = d.name.clone();
    rec.transport = d.transport;
    rec.roles
        .extend(Role::ALL.into_iter().filter(|r| d.has_role(*r)));
    // Never cleared: a lagging IOKit reply must not strand a verified record.
    if let Some(id) = alias_identity(d) {
        rec.family = Some(id.family);
    }
}

fn stamp_last_seen(
    state: &mut State,
    snap: &Snapshot,
    prev: Option<&BTreeSet<DeviceKey>>,
    now: Timestamp,
) {
    match prev {
        None => {
            for d in snap.devices.iter().filter(|d| d.listable()) {
                if let Some(r) = state.devices.get_mut(&d.uid) {
                    r.last_seen = Some(now);
                }
            }
        }
        Some(prev) => {
            let gone = prev
                .iter()
                .filter(|k| snap.devices.iter().all(|d| &d.uid != *k));
            for k in gone {
                if let Some(r) = state.devices.get_mut(k) {
                    r.last_seen = Some(now);
                }
            }
        }
    }
}
