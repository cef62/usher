mod common;
use common::{key, order};
use usher_core::{PausedReason, Role, State, StateError, from_json, to_json};

const FILE: &str = r#"{"schema_version":1,"auto_switch":false,"debug":true,
 "paused_reason":{"role":"input","key":"airpods","name":"AirPods"},
 "output":{"order":["spk","bh"],"never_use":["bh"]},"input":{"order":["mic"],"never_use":[]},
 "devices":{"spk":{"name":"Speakers","transport":"built_in","roles":["output"],"last_seen":1700000000000,"family":null}},
 "ambiguous_families":["AppleUSBAudioEngine:Acme:Mic"],"future_key":{"x":1}}"#;

#[test]
fn round_trip_keeps_every_field() {
    let s = from_json(FILE).unwrap();
    assert_eq!(order(&s, Role::Output), ["spk", "bh"]);
    assert!(s.output.never_use.contains(&key("bh")) && !s.auto_switch && s.debug);
    assert_eq!(from_json(&to_json(&s)).unwrap(), s);
}

#[test]
fn unknown_keys_survive_a_round_trip() {
    let v: serde_json::Value = serde_json::from_str(&to_json(&from_json(FILE).unwrap())).unwrap();
    assert_eq!(v["future_key"]["x"], 1);
}

#[test]
fn missing_keys_take_defaults() {
    let s = from_json("{}").unwrap();
    assert_eq!(s, State::default());
    assert!(s.auto_switch && s.schema_version == 1);
}

#[test]
fn corrupt_json_is_an_error() {
    assert!(matches!(
        from_json("{\"output\":"),
        Err(StateError::Corrupt(_))
    ));
    assert!(matches!(
        from_json("{\"auto_switch\":\"yes\"}"),
        Err(StateError::Corrupt(_))
    ));
}

#[test]
fn newer_schema_is_rejected() {
    assert!(matches!(
        from_json("{\"schema_version\":2}"),
        Err(StateError::UnsupportedVersion(2))
    ));
}

#[test]
fn paused_message_copy() {
    let p = PausedReason {
        role: Role::Input,
        key: key("a"),
        name: "AirPods".into(),
    };
    assert_eq!(
        p.message(),
        "usher paused auto-switch: AirPods kept taking input"
    );
}
