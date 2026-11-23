mod common;
use common::{T0, dev, key, order, snap};
use usher_core::{
    Command, CommandError, DeviceRecord, PausedReason, Role, Runtime, State, Transport, apply,
    to_json,
};

fn with_order(keys: &[&str]) -> State {
    let mut s = State::default();
    s.output.order = keys.iter().map(|k| key(k)).collect();
    s
}

fn run(s: &mut State, snap_devs: &[&str], cmd: Command) -> Result<(), CommandError> {
    let devs = snap_devs.iter().map(|k| dev(k, 2, 2)).collect();
    apply(s, &mut Runtime::default(), &snap(devs, None, None), cmd)
}

fn reorder(keys: &[&str]) -> Command {
    Command::Reorder {
        role: Role::Output,
        order: keys.iter().map(|k| key(k)).collect(),
    }
}

/// Run `cmd`, expect `err`, and prove state is byte-identical afterwards.
fn refused(s: &mut State, snap_devs: &[&str], cmd: Command, err: CommandError) {
    let before = to_json(s);
    assert_eq!(run(s, snap_devs, cmd), Err(err));
    assert_eq!(to_json(s), before);
}

#[test]
fn reorder_rewrites_the_full_list() {
    let mut s = with_order(&["a", "b", "c"]);
    run(&mut s, &[], reorder(&["c", "a", "b"])).unwrap();
    assert_eq!(order(&s, Role::Output), ["c", "a", "b"]);
}

#[test]
fn reorder_with_stale_list_is_refused() {
    let mut s = with_order(&["a", "b", "c"]);
    refused(
        &mut s,
        &[],
        reorder(&["b", "a"]),
        CommandError::StaleOrder(Role::Output),
    );
    assert_eq!(order(&s, Role::Output), ["a", "b", "c"]);
    refused(
        &mut s,
        &[],
        reorder(&["a", "b", "c", "d"]),
        CommandError::StaleOrder(Role::Output),
    );
    refused(
        &mut s,
        &[],
        reorder(&["a", "b", "z"]),
        CommandError::StaleOrder(Role::Output),
    );
}

#[test]
fn reorder_with_duplicate_is_refused() {
    let mut s = with_order(&["a", "b", "c"]);
    refused(
        &mut s,
        &[],
        reorder(&["a", "a", "b"]),
        CommandError::StaleOrder(Role::Output),
    );
}

#[test]
fn promote_moves_to_top() {
    let mut s = with_order(&["a", "b", "c"]);
    let promote = |k: &str| Command::Promote {
        role: Role::Output,
        key: key(k),
    };
    run(&mut s, &[], promote("c")).unwrap();
    assert_eq!(order(&s, Role::Output), ["c", "a", "b"]);
    run(&mut s, &[], promote("c")).unwrap();
    assert_eq!(order(&s, Role::Output), ["c", "a", "b"]);
}

#[test]
fn promote_unknown_is_refused() {
    let mut s = with_order(&["a", "b", "c"]);
    let cmd = Command::Promote {
        role: Role::Output,
        key: key("z"),
    };
    refused(&mut s, &[], cmd, CommandError::UnknownDevice(key("z")));
}

#[test]
fn set_never_use_toggles_one_role() {
    let mut s = with_order(&["a"]);
    s.input.order = vec![key("a")];
    let set = |on| Command::SetNeverUse {
        role: Role::Output,
        key: key("a"),
        on,
    };
    run(&mut s, &[], set(true)).unwrap();
    assert!(s.output.never_use.contains(&key("a")));
    assert!(s.input.never_use.is_empty());
    run(&mut s, &[], set(false)).unwrap();
    assert!(s.output.never_use.is_empty());
    let cmd = Command::SetNeverUse {
        role: Role::Input,
        key: key("z"),
        on: true,
    };
    refused(&mut s, &[], cmd, CommandError::UnknownDevice(key("z")));
}

fn with_headset() -> State {
    let mut s = with_order(&["h", "a"]);
    s.input.order = vec![key("h")];
    s.output.never_use.insert(key("h"));
    s.devices.insert(
        key("h"),
        DeviceRecord {
            name: "Headset".into(),
            transport: Transport::Usb,
            roles: [Role::Output, Role::Input].into(),
            last_seen: Some(T0),
            family: None,
        },
    );
    s
}

#[test]
fn forget_removes_key_everywhere() {
    let mut s = with_headset();
    run(&mut s, &["a"], Command::Forget { key: key("h") }).unwrap();
    assert!(!to_json(&s).contains("\"h\""));
    assert_eq!(order(&s, Role::Output), ["a"]);
}

#[test]
fn forget_connected_is_refused() {
    let mut s = with_headset();
    let cmd = Command::Forget { key: key("h") };
    refused(&mut s, &["h"], cmd, CommandError::StillConnected(key("h")));
    assert_eq!(order(&s, Role::Output), ["h", "a"]);
    let cmd = Command::Forget { key: key("z") };
    refused(&mut s, &[], cmd, CommandError::UnknownDevice(key("z")));
}

fn paused() -> State {
    State {
        auto_switch: false,
        paused_reason: Some(PausedReason {
            role: Role::Output,
            key: key("h"),
            name: "H".into(),
        }),
        ..State::default()
    }
}

#[test]
fn enabling_auto_switch_clears_pause_and_guard() {
    let mut s = paused();
    let mut rt = Runtime::default();
    rt.guard.record(Role::Output, T0);
    rt.guard.record(Role::Output, T0);
    apply(
        &mut s,
        &mut rt,
        &snap(vec![], None, None),
        Command::SetAutoSwitch(true),
    )
    .unwrap();
    assert!(s.auto_switch);
    assert!(s.paused_reason.is_none());
    assert!(!rt.guard.record(Role::Output, T0));
    assert!(!rt.guard.record(Role::Output, T0));
}

#[test]
fn disabling_auto_switch_keeps_pause_reason() {
    let mut s = paused();
    s.auto_switch = true;
    run(&mut s, &[], Command::SetAutoSwitch(false)).unwrap();
    assert!(!s.auto_switch);
    assert!(s.paused_reason.is_some());
}

#[test]
fn set_debug_persists_flag() {
    let mut s = State::default();
    run(&mut s, &[], Command::SetDebug(true)).unwrap();
    assert!(usher_core::from_json(&to_json(&s)).unwrap().debug);
    run(&mut s, &[], Command::SetDebug(false)).unwrap();
    assert!(!s.debug);
}
