mod common;
use common::{dev, key, snap};
use usher_core::{
    Entry, NoticeGate, PausedReason, Reason, Role, Snapshot, Timestamp, Trigger, Why, Wrote,
    explain,
};

fn named(uid: &str, name: &str) -> usher_core::DeviceSnapshot {
    let mut d = dev(uid, 1, 1);
    d.name = name.into();
    d
}
fn spk() -> usher_core::DeviceSnapshot {
    named("spk", "MacBook Pro Speakers")
}
fn pods() -> usher_core::DeviceSnapshot {
    named("pods", "AirPods")
}
fn disp() -> usher_core::DeviceSnapshot {
    named("disp", "Studio Display")
}
fn mic() -> usher_core::DeviceSnapshot {
    named("mic", "MacBook Pro Microphone")
}
fn reason(role: Role, desired: &str) -> Reason {
    Reason {
        role,
        desired: Some(key(desired)),
        current: None,
        skipped: vec![],
    }
}
fn p() -> PausedReason {
    PausedReason {
        role: Role::Output,
        key: "AirPods-uid".into(),
        name: "AirPods".into(),
    }
}
fn switched(role: Role, why: Why) -> Entry {
    let (from, to) = match role {
        Role::Output => ("MacBook Pro Speakers", "AirPods"),
        Role::Input => ("MacBook Pro Microphone", "AirPods"),
    };
    Entry::Switched {
        role,
        from: Some(from.into()),
        to: to.into(),
        why,
    }
}
fn failed(role: Role) -> Entry {
    Entry::Failed {
        role,
        to: "AirPods".into(),
        error: "x".into(),
    }
}
fn out_why(
    trigger: Trigger,
    prev: Option<&Snapshot>,
    s: &Snapshot,
    desired: &str,
) -> Option<Entry> {
    explain(
        trigger,
        prev,
        s,
        &reason(Role::Output, desired),
        &Wrote::Set,
    )
}
fn why(e: Option<Entry>) -> Option<String> {
    e.unwrap().why_text()
}

#[test]
fn triggers_merge_by_precedence() {
    use Trigger::*;
    assert_eq!(
        DevicesChanged.merge(DefaultChanged(Role::Input)),
        DefaultChanged(Role::Input)
    );
    assert_eq!(
        DefaultChanged(Role::Output).merge(DevicesChanged),
        DefaultChanged(Role::Output)
    );
    assert_eq!(
        DefaultChanged(Role::Output).merge(DefaultChanged(Role::Input)),
        DefaultChanged(Role::Output)
    );
    assert_eq!(Launch.merge(DevicesChanged), Launch);
    assert_eq!(Rebind.merge(Launch), Rebind);
    assert_eq!(Rebind.merge(Command), Command);
    assert!(DevicesChanged.notifies() && DefaultChanged(Role::Output).notifies());
    assert!(!Command.notifies() && !Launch.notifies() && !Rebind.notifies());
}

#[test]
fn no_write_makes_no_entry() {
    let s = snap(vec![spk(), pods()], Some("spk"), None);
    let r = reason(Role::Output, "pods");
    assert_eq!(
        explain(Trigger::DevicesChanged, Some(&s), &s, &r, &Wrote::Nothing),
        None
    );
}

#[test]
fn first_reconcile_is_launch_whatever_the_trigger() {
    let s = snap(vec![spk(), pods()], Some("spk"), None);
    let e = out_why(Trigger::DevicesChanged, None, &s, "pods").unwrap();
    assert_eq!(
        e,
        Entry::Switched {
            role: Role::Output,
            from: Some("MacBook Pro Speakers".into()),
            to: "AirPods".into(),
            why: Why::Launch
        }
    );
    assert_eq!(e.text(), "Output: MacBook Pro Speakers → AirPods");
    assert_eq!(e.why_text().as_deref(), Some("usher started"));
}

#[test]
fn rebind_wins_over_the_diff() {
    let prev = snap(vec![spk(), pods()], Some("pods"), None);
    let s = snap(vec![spk()], Some("spk"), None);
    let e = out_why(Trigger::Rebind, Some(&prev), &s, "spk");
    assert!(matches!(
        e,
        Some(Entry::Switched {
            why: Why::Rebind,
            ..
        })
    ));
    assert_eq!(
        why(e).as_deref(),
        Some("usher reconnected to the audio system")
    );
}

#[test]
fn unplug_with_a_fallback_default_is_disconnected() {
    let prev = snap(vec![spk(), pods(), disp()], Some("pods"), None);
    let s = snap(vec![spk(), disp()], Some("spk"), None);
    let e = out_why(
        Trigger::DefaultChanged(Role::Output),
        Some(&prev),
        &s,
        "disp",
    )
    .unwrap();
    assert_eq!(e.why_text().as_deref(), Some("AirPods disconnected"));
    assert_eq!(e.text(), "Output: MacBook Pro Speakers → Studio Display");
}

#[test]
fn new_desired_device_is_connected() {
    let prev = snap(vec![spk()], Some("spk"), None);
    let s = snap(vec![spk(), pods()], Some("spk"), None);
    assert_eq!(
        why(out_why(Trigger::DevicesChanged, Some(&prev), &s, "pods")).as_deref(),
        Some("AirPods connected")
    );
}

#[test]
fn a_command_reconcile_is_ranked() {
    let s = snap(vec![spk(), pods()], Some("spk"), None);
    assert_eq!(
        why(out_why(Trigger::Command, Some(&s), &s, "pods")).as_deref(),
        Some("changed in usher")
    );
}

#[test]
fn a_taker_is_corrected() {
    let prev = snap(vec![mic(), pods()], None, Some("mic"));
    let s = snap(vec![mic(), pods()], None, Some("pods"));
    let e = explain(
        Trigger::DefaultChanged(Role::Input),
        Some(&prev),
        &s,
        &reason(Role::Input, "mic"),
        &Wrote::Set,
    );
    assert_eq!(
        why(e).as_deref(),
        Some("AirPods took input; usher put it back")
    );
}

#[test]
fn no_current_default_falls_back_to_disconnected_or_connected() {
    let t = Trigger::DefaultChanged(Role::Output);
    // previous current is gone and macOS has no default
    let prev = snap(vec![spk(), pods()], Some("pods"), None);
    let s = snap(vec![spk()], None, None);
    assert_eq!(
        why(out_why(t, Some(&prev), &s, "spk")).as_deref(),
        Some("AirPods disconnected")
    );
    // previous current still connected, no current now: also Disconnected(prev name)
    let s = snap(vec![spk(), pods()], None, None);
    assert_eq!(
        why(out_why(t, Some(&prev), &s, "spk")).as_deref(),
        Some("AirPods disconnected")
    );
    // no previous current: Connected(desired)
    let prev = snap(vec![spk(), pods()], None, None);
    assert_eq!(
        why(out_why(t, Some(&prev), &s, "pods")).as_deref(),
        Some("AirPods connected")
    );
}

#[test]
fn failed_and_paused_entries() {
    let prev = snap(vec![spk(), pods()], Some("spk"), None);
    let s = prev.clone();
    let r = reason(Role::Output, "pods");
    let f = explain(
        Trigger::DevicesChanged,
        Some(&prev),
        &s,
        &r,
        &Wrote::Failed("set default failed (OSStatus -50)".into()),
    )
    .unwrap();
    assert_eq!(
        f.text(),
        "Couldn't switch output to AirPods: set default failed (OSStatus -50)"
    );
    assert_eq!(
        explain(
            Trigger::DefaultChanged(Role::Output),
            Some(&prev),
            &s,
            &r,
            &Wrote::Paused(p())
        ),
        Some(Entry::Paused { reason: p() })
    );
    assert_eq!(
        Entry::Paused { reason: p() }.text(),
        "usher paused auto-switch: AirPods kept taking output"
    );
    assert_eq!(Entry::Resumed.text(), "Auto-switch on");
    assert_eq!(Entry::Resumed.why_text(), None);
    assert_eq!(Entry::Resumed.role(), None);
    assert_eq!(f.role(), Some(Role::Output));
}

#[test]
fn no_previous_default_renders_without_from() {
    let e = Entry::Switched {
        role: Role::Input,
        from: None,
        to: "AirPods".into(),
        why: Why::Ranked,
    };
    assert_eq!(e.text(), "Input: AirPods");
}

#[test]
fn one_body_for_both_roles_and_none_for_quiet_entries() {
    let mut g = NoticeGate::default();
    let (out, inp) = (
        switched(Role::Output, Why::Connected("AirPods".into())),
        switched(Role::Input, Why::Connected("AirPods".into())),
    );
    assert_eq!(
        g.body(Trigger::DevicesChanged, &[out, inp], Timestamp(0))
            .as_deref(),
        Some("Output: MacBook Pro Speakers → AirPods\nInput: MacBook Pro Microphone → AirPods")
    );
    let mut g = NoticeGate::default();
    assert_eq!(
        g.body(Trigger::Command, &[failed(Role::Output)], Timestamp(0)),
        None,
        "a user command never notifies"
    );
    assert_eq!(
        g.body(
            Trigger::DevicesChanged,
            &[switched(Role::Output, Why::Ranked), Entry::Resumed],
            Timestamp(0)
        ),
        None
    );
    assert!(
        g.body(
            Trigger::DevicesChanged,
            &[Entry::Paused { reason: p() }],
            Timestamp(0)
        )
        .is_none(),
        "Paused notifies through Shell::paused"
    );
}

#[test]
fn gate_allows_one_notice_per_role_per_10_s() {
    let mut g = NoticeGate::default();
    let out = switched(Role::Output, Why::Corrected("AirPods".into()));
    assert!(
        g.body(
            Trigger::DefaultChanged(Role::Output),
            std::slice::from_ref(&out),
            Timestamp(1_000)
        )
        .is_some()
    );
    assert!(
        g.body(
            Trigger::DefaultChanged(Role::Output),
            std::slice::from_ref(&out),
            Timestamp(10_999)
        )
        .is_none()
    );
    assert!(
        g.body(
            Trigger::DevicesChanged,
            &[switched(Role::Input, Why::Connected("AirPods".into()))],
            Timestamp(10_999)
        )
        .is_some(),
        "roles count apart"
    );
    assert!(
        g.body(
            Trigger::DefaultChanged(Role::Output),
            &[out],
            Timestamp(11_000)
        )
        .is_some()
    );
}
