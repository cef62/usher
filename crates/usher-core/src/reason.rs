use crate::{DeviceKey, Role};
use serde::Serialize;
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Skip {
    Disconnected,
    NotAlive,
    Hidden,
    NoStreams,
    NeverUse,
}

impl fmt::Display for Skip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Skip::Disconnected => "disconnected",
            Skip::NotAlive => "not alive",
            Skip::Hidden => "hidden",
            Skip::NoStreams => "no streams",
            Skip::NeverUse => "never use",
        })
    }
}

#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Skipped {
    pub key: DeviceKey,
    pub why: Skip,
}

#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Reason {
    pub role: Role,
    pub desired: Option<DeviceKey>,
    pub current: Option<DeviceKey>,
    pub skipped: Vec<Skipped>,
}

fn key_or_none(k: &Option<DeviceKey>) -> &str {
    k.as_ref().map_or("none", DeviceKey::as_str)
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let skipped: Vec<String> = self
            .skipped
            .iter()
            .map(|s| format!("{}: {}", s.key, s.why))
            .collect();
        write!(
            f,
            "{}: desired={} current={} skipped=[{}]",
            self.role,
            key_or_none(&self.desired),
            key_or_none(&self.current),
            skipped.join(", ")
        )
    }
}
