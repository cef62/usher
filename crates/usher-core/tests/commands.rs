mod common;
use common::{T0, dev, key, order, snap};
use usher_core::{
    Command, CommandError, DeviceRecord, PausedReason, Role, Runtime, State, Timestamp, Transport,
    apply, decide, reconcile, to_json,
};

fn with_order(keys: &[&str]) -> State {
    let mut s = State::default();
    s.output.order = keys.iter().map(|k| key(k)).collect();
    s
}

fn run(s: &mut State, snap_devs: &[&str], cmd: Command) -> Result<bool, CommandError> {
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

fn promote_cmd(k: &str) -> Command {
    Command::Promote {
        role: Role::Output,
        key: key(k),
    }
}

#[test]
fn apply_reports_whether_state_changed() {
    let mut s = State::default();
    assert_eq!(run(&mut s, &[], Command::SetDebug(true)), Ok(true));
    assert_eq!(run(&mut s, &[], Command::SetDebug(true)), Ok(false));
    let mut s = with_order(&["a", "b"]);
    assert_eq!(run(&mut s, &[], promote_cmd("a")), Ok(false));
    assert_eq!(run(&mut s, &[], promote_cmd("b")), Ok(true));
    let mut s = paused();
    assert_eq!(run(&mut s, &[], Command::SetAutoSwitch(true)), Ok(true));
}

#[test]
fn three_promotes_in_ten_seconds_do_not_trip() {
    let mut s = with_order(&["a", "b", "c"]);
    let mut rt = Runtime::default();
    let devs = || vec![dev("a", 1, 0), dev("b", 1, 0), dev("c", 1, 0)];
    let sn = snap(devs(), Some("c"), None);
    for (i, k) in ["a", "b", "a", "b"].into_iter().enumerate() {
        apply(&mut s, &mut rt, &sn, promote_cmd(k)).unwrap();
        let o = reconcile(&mut s, &mut rt, &sn, Timestamp(T0.0 + i as u64 * 1_000));
        assert!(
            !o.actions
                .iter()
                .any(|a| matches!(a, usher_core::Action::PauseAuto { .. }))
        );
    }
    assert!(s.auto_switch);
}

#[test]
fn reorder_and_never_use_also_reset_the_guard() {
    let mut s = with_order(&["a", "b"]);
    let mut rt = Runtime::default();
    rt.guard.record(Role::Output, T0);
    rt.guard.record(Role::Output, T0);
    let sn = snap(vec![], None, None);
    apply(&mut s, &mut rt, &sn, reorder(&["b", "a"])).unwrap();
    assert!(!rt.guard.record(Role::Output, T0));
    rt.guard.record(Role::Output, T0);
    let cmd = Command::SetNeverUse {
        role: Role::Output,
        key: key("a"),
        on: true,
    };
    apply(&mut s, &mut rt, &sn, cmd).unwrap();
    assert!(!rt.guard.record(Role::Output, T0));
}

#[test]
fn promote_clears_never_use_for_that_role() {
    let mut s = with_order(&["a", "b"]);
    s.output.never_use.insert(key("b"));
    assert_eq!(run(&mut s, &[], promote_cmd("b")), Ok(true));
    assert!(s.output.never_use.is_empty());
    let sn = snap(vec![dev("a", 1, 0), dev("b", 1, 0)], None, None);
    assert_eq!(decide(&s, &sn, Role::Output).desired, Some(key("b")));
    // promoting a never-use device already at the top still clears the flag
    s.output.never_use.insert(key("b"));
    assert_eq!(run(&mut s, &[], promote_cmd("b")), Ok(true));
}

#[test]
fn panel_json_parses_into_commands() {
    let p = |s: &str| serde_json::from_str::<Command>(s);
    assert_eq!(
        p(r#"{"kind":"promote","args":{"role":"output","key":"AirPods-uid"}}"#).unwrap(),
        Command::Promote {
            role: Role::Output,
            key: "AirPods-uid".into()
        }
    );
    assert_eq!(
        p(r#"{"kind":"reorder","args":{"role":"input","order":["a","b"]}}"#).unwrap(),
        Command::Reorder {
            role: Role::Input,
            order: vec!["a".into(), "b".into()]
        }
    );
    assert_eq!(
        p(r#"{"kind":"setNeverUse","args":{"role":"output","key":"k","on":true}}"#).unwrap(),
        Command::SetNeverUse {
            role: Role::Output,
            key: "k".into(),
            on: true
        }
    );
    assert_eq!(
        p(r#"{"kind":"forget","args":{"key":"k"}}"#).unwrap(),
        Command::Forget { key: "k".into() }
    );
    assert_eq!(
        p(r#"{"kind":"setAutoSwitch","args":false}"#).unwrap(),
        Command::SetAutoSwitch(false)
    );
    assert!(p(r#"{"kind":"format","args":{}}"#).is_err());
    assert!(p(r#"{"kind":"promote","args":{"role":"speaker","key":"k"}}"#).is_err());
}

#[test]
fn panel_json_rejects_unknown_and_malformed_input() {
    let p = |s: &str| serde_json::from_str::<Command>(s);
    assert_eq!(
        p(r#"{"kind":"setDebug","args":true}"#).unwrap(),
        Command::SetDebug(true)
    );
    assert!(p(r#"{"kind":"forget","args":{"key":"k","x":1}}"#).is_err());
    assert!(p(r#"{"kind":"forget"}"#).is_err());
    assert!(p(r#"{"kind":"setAutoSwitch","args":"x"}"#).is_err());
}

#[test]
fn panel_json_rejects_extra_top_level_key() {
    let r = serde_json::from_str::<Command>(r#"{"kind":"forget","args":{"key":"k"},"extra":1}"#);
    assert!(r.is_err());
}
