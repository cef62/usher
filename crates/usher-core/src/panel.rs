use crate::{DeviceKey, PausedReason, Role, Snapshot, State, Timestamp};
use serde::Serialize;

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PanelRow {
    pub key: DeviceKey,
    pub name: String,
    pub rank: Option<u32>,
    pub connected: bool,
    pub active: bool,
    pub never_use: bool,
    pub last_seen: Option<Timestamp>,
    pub can_forget: bool,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PanelState {
    pub auto_switch: bool,
    pub debug: bool,
    pub update_check: bool,
    pub paused: Option<PausedReason>,
    pub output: Vec<PanelRow>,
    pub input: Vec<PanelRow>,
}

pub fn panel_state(state: &State, snap: &Snapshot) -> PanelState {
    PanelState {
        auto_switch: state.auto_switch,
        debug: state.debug,
        update_check: state.update_check,
        paused: state.paused_reason.clone(),
        output: rows(state, snap, Role::Output),
        input: rows(state, snap, Role::Input),
    }
}

fn rows(state: &State, snap: &Snapshot, role: Role) -> Vec<PanelRow> {
    let list = state.list(role);
    let current = snap.current.get(role).as_ref();
    let row = |key: &DeviceKey, rank: Option<u32>| {
        let record = state.devices.get(key);
        let connected = snap.is_connected(key);
        PanelRow {
            key: key.clone(),
            name: snap
                .device(key)
                .map(|d| d.name.clone())
                .or_else(|| record.map(|r| r.name.clone()))
                .unwrap_or_else(|| key.0.clone()),
            rank,
            connected,
            active: current == Some(key),
            never_use: list.never_use.contains(key),
            last_seen: record.and_then(|r| r.last_seen),
            can_forget: !connected && rank.is_some(),
        }
    };
    let mut out: Vec<PanelRow> = (1..)
        .zip(&list.order)
        .map(|(rank, key)| row(key, Some(rank)))
        .collect();
    if let Some(cur) = current.filter(|c| !list.order.contains(c)) {
        out.push(row(cur, None));
    }
    out
}
