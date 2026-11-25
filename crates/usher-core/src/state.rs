use crate::{DeviceKey, Role, Timestamp, Transport};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Default, Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct RoleList {
    pub order: Vec<DeviceKey>,
    pub never_use: BTreeSet<DeviceKey>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct DeviceRecord {
    pub name: String,
    pub transport: Transport,
    pub roles: BTreeSet<Role>,
    pub last_seen: Option<Timestamp>,
    /// Set only once the device was verified port-dependent (serial-less USB or display).
    pub family: Option<String>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PausedReason {
    pub role: Role,
    pub key: DeviceKey,
    pub name: String,
}

impl PausedReason {
    pub fn message(&self) -> String {
        format!(
            "usher paused auto-switch: {} kept taking {}",
            self.name, self.role
        )
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub schema_version: u32,
    pub auto_switch: bool,
    pub debug: bool,
    pub update_check: bool,
    pub notify_switches: bool,
    pub paused_reason: Option<PausedReason>,
    pub output: RoleList,
    pub input: RoleList,
    pub devices: BTreeMap<DeviceKey, DeviceRecord>,
    pub ambiguous_families: BTreeSet<String>,
    /// Unknown top-level keys, preserved at the top level only (nested structs drop unknown keys).
    /// Must stay the last field.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for State {
    fn default() -> Self {
        State {
            schema_version: SCHEMA_VERSION,
            auto_switch: true,
            debug: false,
            update_check: true,
            notify_switches: true,
            paused_reason: None,
            output: RoleList::default(),
            input: RoleList::default(),
            devices: BTreeMap::new(),
            ambiguous_families: BTreeSet::new(),
            extra: Map::new(),
        }
    }
}

impl State {
    pub fn list(&self, role: Role) -> &RoleList {
        match role {
            Role::Output => &self.output,
            Role::Input => &self.input,
        }
    }

    pub fn list_mut(&mut self, role: Role) -> &mut RoleList {
        match role {
            Role::Output => &mut self.output,
            Role::Input => &mut self.input,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum StateError {
    Corrupt(String),
    UnsupportedVersion(u32),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::Corrupt(e) => write!(f, "state file is corrupt: {e}"),
            StateError::UnsupportedVersion(v) => write!(f, "unsupported state schema version {v}"),
        }
    }
}

impl std::error::Error for StateError {}

pub fn from_json(text: &str) -> Result<State, StateError> {
    let mut state: State =
        serde_json::from_str(text).map_err(|e| StateError::Corrupt(e.to_string()))?;
    if state.schema_version > SCHEMA_VERSION {
        return Err(StateError::UnsupportedVersion(state.schema_version));
    }
    state.schema_version = SCHEMA_VERSION;
    Ok(state)
}

pub fn to_json(state: &State) -> String {
    serde_json::to_string_pretty(state).expect("State only holds string-keyed, serialisable data")
}
