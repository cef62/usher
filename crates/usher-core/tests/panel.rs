mod common;
use common::{T0, dev, key, snap};
use usher_core::{DeviceRecord, Role, State, Transport, panel_state};

fn record(name: &str) -> DeviceRecord {
    DeviceRecord {
        name: name.into(),
        transport: Transport::BuiltIn,
        roles: [Role::Output].into(),
        last_seen: Some(T0),
        family: None,
    }
}

fn state(order: &[&str]) -> State {
    let mut s = State::default();
    s.output.order = order.iter().map(|k| key(k)).collect();
    s
}

#[test]
fn rows_follow_order_with_ranks() {
    let s = state(&["a", "b"]);
    let p = panel_state(&s, &snap(vec![dev("a", 1, 0), dev("b", 1, 0)], None, None));
    let ranks: Vec<_> = p.output.iter().map(|r| r.rank).collect();
    assert_eq!(ranks, [Some(1), Some(2)]);
    assert_eq!(p.output[1].key, key("b"));
    assert!(p.input.is_empty());
}

#[test]
fn disconnected_row_uses_record_name_and_last_seen() {
    let mut s = state(&["gone", "nameless"]);
    s.devices.insert(key("gone"), record("Old Speakers"));
    let p = panel_state(&s, &snap(vec![], None, None));
    let r = &p.output[0];
    assert_eq!(r.name, "Old Speakers");
    assert!(!r.connected && r.can_forget);
    assert_eq!(r.last_seen, Some(T0));
    assert_eq!(p.output[1].name, "nameless");
    assert_eq!(p.output[1].last_seen, None);
}

#[test]
fn connected_row_prefers_snapshot_name() {
    let mut s = state(&["a"]);
    s.devices.insert(key("a"), record("Stale"));
    let mut d = dev("a", 1, 0);
    d.name = "Fresh".into();
    let r = &panel_state(&s, &snap(vec![d], None, None)).output[0];
    assert_eq!(r.name, "Fresh");
    assert!(r.connected && !r.can_forget);
}

#[test]
fn active_row_matches_current_default() {
    let s = state(&["a", "b"]);
    let p = panel_state(
        &s,
        &snap(vec![dev("a", 1, 0), dev("b", 1, 0)], Some("b"), None),
    );
    let active: Vec<_> = p.output.iter().map(|r| r.active).collect();
    assert_eq!(active, [false, true]);
}

#[test]
fn never_use_row_is_flagged() {
    let mut s = state(&["a", "b"]);
    s.output.never_use.insert(key("b"));
    let p = panel_state(&s, &snap(vec![dev("a", 1, 0), dev("b", 1, 0)], None, None));
    assert!(!p.output[0].never_use && p.output[1].never_use);
}

#[test]
fn hidden_current_default_is_shown_unranked() {
    let s = state(&["a"]);
    let mut h = dev("h", 1, 0);
    h.hidden = true;
    h.name = "Hidden".into();
    let p = panel_state(&s, &snap(vec![dev("a", 1, 0), h], Some("h"), None));
    assert_eq!(p.output.len(), 2);
    let r = &p.output[1];
    assert_eq!((r.key.clone(), r.rank, r.active), (key("h"), None, true));
    assert_eq!(r.name, "Hidden");
    assert!(r.connected && !r.can_forget && !r.never_use);
}

#[test]
fn current_default_in_order_is_not_duplicated() {
    let s = state(&["a"]);
    let p = panel_state(&s, &snap(vec![dev("a", 1, 0)], Some("a"), None));
    assert_eq!(p.output.len(), 1);
}

#[test]
fn flags_and_pause_are_copied() {
    let s = State {
        auto_switch: false,
        debug: true,
        paused_reason: Some(usher_core::PausedReason {
            role: Role::Input,
            key: key("m"),
            name: "Mic".into(),
        }),
        ..State::default()
    };
    let p = panel_state(&s, &snap(vec![], None, None));
    assert!(!p.auto_switch && p.debug);
    assert_eq!(p.paused, s.paused_reason);
}

#[test]
fn serialises_camel_case() {
    let mut s = state(&["a"]);
    s.devices.insert(key("a"), record("A"));
    let p = panel_state(&s, &snap(vec![], None, None));
    let v = serde_json::to_value(&p).unwrap();
    assert!(v["output"][0].get("canForget").is_some() && v.get("autoSwitch").is_some());
    assert_eq!(v["output"][0]["lastSeen"], 1_700_000_000_000u64);
    assert_eq!(v["output"][0]["key"], "a");
    assert!(v["paused"].is_null());
}
