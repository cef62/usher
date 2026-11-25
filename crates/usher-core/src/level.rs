//! Volume and mute: the value a role's active device reports, and the panel's write requests.
use serde::{Deserialize, Serialize};

use crate::{DeviceKey, Role};

/// `None` means the device has no settable control.
#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Level {
    pub volume: Option<f32>,
    pub muted: Option<bool>,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LevelRequest {
    Volume {
        role: Role,
        key: DeviceKey,
        value: f32,
    },
    Mute {
        role: Role,
        key: DeviceKey,
        on: bool,
    },
}

impl LevelRequest {
    pub fn role(&self) -> Role {
        match self {
            LevelRequest::Volume { role, .. } | LevelRequest::Mute { role, .. } => *role,
        }
    }

    pub fn key(&self) -> &DeviceKey {
        match self {
            LevelRequest::Volume { key, .. } | LevelRequest::Mute { key, .. } => key,
        }
    }

    /// False only for a non-finite volume.
    pub fn valid(&self) -> bool {
        !matches!(self, LevelRequest::Volume { value, .. } if !value.is_finite())
    }
}

pub fn clamp_volume(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_serializes_camel_case_with_nulls() {
        assert_eq!(
            serde_json::to_string(&Level {
                volume: Some(0.5),
                muted: None
            })
            .unwrap(),
            r#"{"volume":0.5,"muted":null}"#
        );
    }
    #[test]
    fn requests_parse_from_the_panel_shape() {
        let v: LevelRequest =
            serde_json::from_str(r#"{"kind":"volume","role":"output","key":"k","value":0.25}"#)
                .unwrap();
        assert_eq!(
            v,
            LevelRequest::Volume {
                role: Role::Output,
                key: "k".into(),
                value: 0.25
            }
        );
        let m: LevelRequest =
            serde_json::from_str(r#"{"kind":"mute","role":"input","key":"k","on":true}"#).unwrap();
        assert_eq!((m.role(), m.key().as_str()), (Role::Input, "k"));
    }
    #[test]
    fn clamp_and_validity() {
        assert_eq!(
            (clamp_volume(-0.2), clamp_volume(1.5), clamp_volume(0.4)),
            (0.0, 1.0, 0.4)
        );
        let vol = |value| LevelRequest::Volume {
            role: Role::Output,
            key: "k".into(),
            value,
        };
        assert!(vol(0.5).valid());
        assert!(!vol(f32::NAN).valid());
        assert!(!vol(f32::INFINITY).valid());
    }
}
