mod common;
use common::{T0, dev, key, order, snap};
use usher_core::{Action, DeviceSnapshot, Role, Runtime, State, Timestamp, Transport, reconcile};

fn at(ms: u64) -> Timestamp {
    Timestamp(T0.0 + ms)
}

fn set(role: Role, k: &str) -> Action {
    Action::SetDefault { role, key: key(k) }
}

fn with_order(out: &[&str], inp: &[&str]) -> State {
    let mut s = State::default();
    s.output.order = out.iter().map(|k| key(k)).collect();
    s.input.order = inp.iter().map(|k| key(k)).collect();
    s
}

fn writes(actions: &[Action]) -> usize {
    actions
        .iter()
        .filter(|a| matches!(a, Action::SetDefault { .. }))
        .count()
}

fn usb(port: &str) -> DeviceSnapshot {
    let mut d = dev(&format!("AppleUSBAudioEngine:Shure:MV7:{port}:1"), 1, 1);
    d.transport = Transport::Usb;
    d.serialless_usb = true;
    d
}

#[test]
fn top_device_connecting_takes_default() {
    let mut s = with_order(&["hp", "spk"], &[]);
    let mut rt = Runtime::default();
    let o = reconcile(
        &mut s,
        &mut rt,
        &snap(vec![dev("spk", 1, 0)], Some("spk"), None),
        T0,
    );
    assert_eq!(writes(&o.actions), 0);
    let sn = snap(vec![dev("spk", 1, 0), dev("hp", 1, 0)], Some("spk"), None);
    let o = reconcile(&mut s, &mut rt, &sn, at(1_000));
    assert!(o.actions.contains(&set(Role::Output, "hp")));
}

#[test]
fn echo_of_own_write_is_a_noop() {
    let mut s = with_order(&["spk"], &[]);
    let mut rt = Runtime::default();
    let sn = snap(vec![dev("spk", 1, 0)], Some("spk"), None);
    for i in 0..3 {
        let o = reconcile(&mut s, &mut rt, &sn, at(i));
        assert_eq!(writes(&o.actions), 0);
    }
    // three echoes must not have counted toward the guard
    let bad = snap(vec![dev("spk", 1, 0), dev("x", 1, 0)], Some("x"), None);
    let o = reconcile(&mut s, &mut rt, &bad, at(10));
    assert_eq!(writes(&o.actions), 1);
    assert!(s.auto_switch);
}

#[test]
fn auto_switch_off_never_writes() {
    let mut s = with_order(&["spk"], &[]);
    s.auto_switch = false;
    let sn = snap(vec![dev("spk", 1, 0), dev("x", 1, 0)], Some("x"), None);
    let o = reconcile(&mut s, &mut Runtime::default(), &sn, T0);
    assert_eq!(writes(&o.actions), 0);
    assert_eq!(o.reasons.len(), 2);
}

#[test]
fn airpods_taking_both_roles_on_connect_are_reverted() {
    let mut s = with_order(&["spk"], &["mic"]);
    let sn = snap(
        vec![dev("spk", 1, 0), dev("mic", 0, 1), dev("airpods", 1, 1)],
        Some("airpods"),
        Some("airpods"),
    );
    let o = reconcile(&mut s, &mut Runtime::default(), &sn, T0);
    assert_eq!(
        o.actions,
        [
            set(Role::Output, "spk"),
            set(Role::Input, "mic"),
            Action::Persist
        ]
    );
    assert_eq!(order(&s, Role::Output), ["spk", "airpods"]);
    assert_eq!(order(&s, Role::Input), ["mic", "airpods"]);
}

#[test]
fn outside_change_is_reverted_and_auto_stays_on() {
    let mut s = with_order(&["spk"], &[]);
    let mut rt = Runtime::default();
    let sn = snap(vec![dev("spk", 1, 0), dev("x", 1, 0)], Some("x"), None);
    let o = reconcile(&mut s, &mut rt, &sn, T0);
    assert!(o.actions.contains(&set(Role::Output, "spk")));
    assert!(s.auto_switch);
    assert!(s.paused_reason.is_none());
}

#[test]
fn fast_fight_trips_the_guard() {
    let mut s = with_order(&["spk"], &["mic"]);
    let mut rt = Runtime::default();
    let sn = snap(
        vec![dev("spk", 1, 0), dev("mic", 0, 1), dev("airpods", 0, 1)],
        Some("spk"),
        Some("airpods"),
    );
    for t in [0, 1_000] {
        let o = reconcile(&mut s, &mut rt, &sn, at(t));
        assert!(o.actions.contains(&set(Role::Input, "mic")));
    }
    let o = reconcile(&mut s, &mut rt, &sn, at(2_000));
    assert!(o.actions.contains(&Action::PauseAuto {
        role: Role::Input,
        key: key("airpods")
    }));
    assert_eq!(writes(&o.actions), 0);
    assert!(!s.auto_switch);
    assert_eq!(
        s.paused_reason.unwrap().message(),
        "usher paused auto-switch: airpods kept taking input"
    );
}

#[test]
fn guard_trip_on_output_stops_the_input_write_too() {
    let mut s = with_order(&["spk"], &["mic"]);
    let mut rt = Runtime::default();
    let sn = snap(
        vec![dev("spk", 1, 0), dev("mic", 0, 1), dev("airpods", 1, 1)],
        Some("airpods"),
        Some("airpods"),
    );
    for t in [0, 1_000] {
        assert_eq!(writes(&reconcile(&mut s, &mut rt, &sn, at(t)).actions), 2);
    }
    let o = reconcile(&mut s, &mut rt, &sn, at(2_000));
    assert_eq!(writes(&o.actions), 0);
    assert_eq!(
        o.actions[0],
        Action::PauseAuto {
            role: Role::Output,
            key: key("airpods")
        }
    );
    assert_eq!(o.reasons.len(), 2);
}

#[test]
fn missing_default_is_written_without_guarding() {
    let mut s = with_order(&["spk"], &[]);
    let mut rt = Runtime::default();
    let sn = snap(vec![dev("spk", 1, 0)], None, None);
    for t in 0..5 {
        let o = reconcile(&mut s, &mut rt, &sn, at(t));
        assert!(o.actions.contains(&set(Role::Output, "spk")));
    }
    assert!(s.auto_switch);
}

#[test]
fn dock_connect_writes_once_and_does_not_trip() {
    let mut s = with_order(&["hp", "spk"], &[]);
    let mut rt = Runtime::default();
    let first = snap(vec![dev("spk", 1, 0), dev("hp", 1, 0)], Some("spk"), None);
    let after = snap(vec![dev("spk", 1, 0), dev("hp", 1, 0)], Some("hp"), None);
    let mut total = writes(&reconcile(&mut s, &mut rt, &first, T0).actions);
    for t in [100, 200, 300] {
        total += writes(&reconcile(&mut s, &mut rt, &after, at(t)).actions);
    }
    assert_eq!(total, 1);
    assert!(s.auto_switch);
}

#[test]
fn persist_only_when_state_changes() {
    let mut s = State::default();
    let mut rt = Runtime::default();
    let sn = snap(vec![dev("spk", 1, 0)], Some("spk"), None);
    let o = reconcile(&mut s, &mut rt, &sn, T0);
    assert_eq!(o.actions, [Action::Persist]);
    let o = reconcile(&mut s, &mut rt, &sn, at(1));
    assert!(o.actions.is_empty());
}

#[test]
fn alias_merge_is_reported() {
    let mut s = State::default();
    let mut rt = Runtime::default();
    let (p1, p2) = (usb("14200000"), usb("14300000"));
    let (u1, u2) = (p1.uid.0.clone(), p2.uid.0.clone());
    reconcile(&mut s, &mut rt, &snap(vec![p1], Some(&u1), Some(&u1)), T0);
    let sn = snap(vec![p2], Some(&u2), Some(&u2));
    let o = reconcile(&mut s, &mut rt, &sn, at(1_000));
    assert_eq!(
        o.actions,
        [
            Action::AliasMerged {
                from: key(&u1),
                to: key(&u2)
            },
            Action::Persist
        ]
    );
    assert_eq!(order(&s, Role::Output), [u2.as_str()]);
}

#[test]
fn reasons_cover_both_roles_in_order() {
    let mut s = State::default();
    let o = reconcile(
        &mut s,
        &mut Runtime::default(),
        &snap(vec![], None, None),
        T0,
    );
    assert_eq!(o.reasons.len(), 2);
    assert_eq!(o.reasons[0].role, Role::Output);
    assert_eq!(o.reasons[1].role, Role::Input);
}
