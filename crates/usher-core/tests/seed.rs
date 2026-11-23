mod common;
use common::{T0, dev, order, snap};
use usher_core::{Action, Role, Runtime, Transport, reconcile, seed};

fn virt(uid: &str) -> usher_core::DeviceSnapshot {
    let mut d = dev(uid, 1, 0);
    d.transport = Transport::Virtual;
    d
}

#[test]
fn current_default_is_ranked_first() {
    let s = seed(
        &snap(vec![dev("a", 1, 0), dev("b", 1, 0)], Some("b"), None),
        T0,
    );
    assert_eq!(order(&s, Role::Output), ["b", "a"]);
}

#[test]
fn virtual_devices_are_flagged_after_current() {
    let s = seed(
        &snap(vec![dev("spk", 1, 0), virt("bh")], Some("spk"), None),
        T0,
    );
    assert_eq!(order(&s, Role::Output), ["spk", "bh"]);
    assert!(s.output.never_use.contains(&"bh".into()));
    assert!(!s.output.never_use.contains(&"spk".into()));
}

#[test]
fn virtual_current_default_is_not_flagged_at_seed() {
    let s = seed(
        &snap(vec![dev("spk", 1, 0), virt("bh")], Some("bh"), None),
        T0,
    );
    assert_eq!(order(&s, Role::Output), ["bh", "spk"]);
    assert!(s.output.never_use.is_empty());
}

#[test]
fn first_reconcile_after_seed_does_nothing() {
    for cur in ["spk", "bh"] {
        let sn = snap(
            vec![dev("spk", 1, 1), virt("bh"), dev("mic", 0, 1)],
            Some(cur),
            Some("mic"),
        );
        let mut s = seed(&sn, T0);
        let out = reconcile(&mut s, &mut Runtime::default(), &sn, T0);
        assert!(
            !out.actions
                .iter()
                .any(|a| matches!(a, Action::SetDefault { .. } | Action::PauseAuto { .. })),
            "{cur}: {:?}",
            out.actions
        );
    }
}

#[test]
fn seed_starts_with_auto_switch_on() {
    assert!(seed(&snap(vec![], None, None), T0).auto_switch);
}
