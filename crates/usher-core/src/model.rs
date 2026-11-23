use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Output,
    Input,
}

impl Role {
    pub const ALL: [Role; 2] = [Role::Output, Role::Input];
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Role::Output => "output",
            Role::Input => "input",
        })
    }
}

#[derive(Default, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PerRole<T> {
    pub output: T,
    pub input: T,
}

impl<T> PerRole<T> {
    pub fn get(&self, role: Role) -> &T {
        match role {
            Role::Output => &self.output,
            Role::Input => &self.input,
        }
    }

    pub fn get_mut(&mut self, role: Role) -> &mut T {
        match role {
            Role::Output => &mut self.output,
            Role::Input => &mut self.input,
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceKey(pub String);

impl DeviceKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DeviceKey {
    fn from(s: &str) -> Self {
        DeviceKey(s.to_string())
    }
}

impl fmt::Display for DeviceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Milliseconds since the Unix epoch.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(pub u64);

impl Timestamp {
    /// Saturates so a clock step back never panics.
    pub fn since(self, earlier: Timestamp) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    BuiltIn,
    Usb,
    Bluetooth,
    Hdmi,
    DisplayPort,
    Thunderbolt,
    AirPlay,
    Virtual,
    Aggregate,
    #[serde(other)]
    Other,
}

impl Transport {
    pub fn is_virtual(self) -> bool {
        matches!(self, Transport::Virtual | Transport::Aggregate)
    }

    pub fn is_display(self) -> bool {
        matches!(self, Transport::Hdmi | Transport::DisplayPort)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceSnapshot {
    pub uid: DeviceKey,
    pub name: String,
    pub transport: Transport,
    pub alive: bool,
    pub hidden: bool,
    pub input_streams: u32,
    pub output_streams: u32,
    /// True only when M3's IOKit check confirmed a serial-less USB device;
    /// false means "has a serial" or "not resolved yet".
    pub serialless_usb: bool,
}

impl DeviceSnapshot {
    pub fn streams(&self, role: Role) -> u32 {
        match role {
            Role::Output => self.output_streams,
            Role::Input => self.input_streams,
        }
    }

    pub fn has_role(&self, role: Role) -> bool {
        self.streams(role) > 0
    }

    pub fn listable(&self) -> bool {
        self.alive && !self.hidden
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Enumeration order.
    pub devices: Vec<DeviceSnapshot>,
    pub current: PerRole<Option<DeviceKey>>,
}

impl Snapshot {
    pub fn device(&self, key: &DeviceKey) -> Option<&DeviceSnapshot> {
        self.devices.iter().find(|d| &d.uid == key)
    }

    pub fn is_connected(&self, key: &DeviceKey) -> bool {
        self.device(key).is_some()
    }
}
