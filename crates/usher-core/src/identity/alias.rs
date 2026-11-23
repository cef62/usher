//! Alias merge: moves a record to its new UID when a port-dependent device reappears (audit §3.3).
use super::{PortIdentity, alias_identity, parse_port, registry};
use crate::{DeviceKey, Snapshot, State};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Merge {
    pub from: DeviceKey,
    pub to: DeviceKey,
}

/// Run before `remember`, so a donor's rank moves to the new UID before it could be appended as new.
#[doc(hidden)]
pub fn merge_aliases(state: &mut State, snap: &Snapshot) -> Vec<Merge> {
    let candidates: Vec<(DeviceKey, PortIdentity)> = snap
        .devices
        .iter()
        .filter_map(|d| Some((d.uid.clone(), alias_identity(d)?)))
        .collect();
    latch_ambiguous(state, &candidates);
    let mut merges = Vec::new();
    for (to, id) in &candidates {
        // The latch above counts every candidate; only listable devices may receive a rank.
        let listable = snap.device(to).is_some_and(|d| d.listable());
        if !listable || state.ambiguous_families.contains(&id.family) {
            continue;
        }
        for from in donors_for(state, snap, id) {
            registry::rename(state, &from, to);
            merges.push(Merge {
                from,
                to: to.clone(),
            });
        }
    }
    merges
}

/// A family seen with two distinct ports in one snapshot can never prove which copy moved.
fn latch_ambiguous(state: &mut State, candidates: &[(DeviceKey, PortIdentity)]) {
    let mut ports: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (_, id) in candidates {
        ports.entry(&id.family).or_default().insert(&id.port);
    }
    for (family, set) in ports {
        if set.len() > 1 {
            state.ambiguous_families.insert(family.to_string());
        }
    }
}

/// Disconnected, verified records of the same family and endpoint, newest first (key breaks ties).
fn donors_for(state: &State, snap: &Snapshot, id: &PortIdentity) -> Vec<DeviceKey> {
    let mut donors: Vec<_> = state
        .devices
        .iter()
        .filter(|(k, rec)| {
            !snap.is_connected(k)
                && rec.family.as_deref() == Some(&id.family)
                && parse_port(k.as_str()).is_some_and(|p| p.endpoint == id.endpoint)
        })
        .map(|(k, rec)| (rec.last_seen, k.clone()))
        .collect();
    // BTreeMap iterates keys ascending and the sort is stable, so ties stay key-ascending.
    donors.sort_by_key(|d| std::cmp::Reverse(d.0));
    donors.into_iter().map(|(_, k)| k).collect()
}
