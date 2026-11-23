mod common;
use common::{T0, dev, key, order, snap};
use std::collections::BTreeSet;
use usher_core::{Role, State, Timestamp, Transport, remember};

fn run(state: &mut State, devs: Vec<usher_core::DeviceSnapshot>) {
    remember(state, &snap(devs, None, None), None, T0);
}

#[test]
fn new_device_goes_to_the_bottom() {
    let mut s = State::default();
    s.output.order.push(key("a"));
    run(&mut s, vec![dev("b", 2, 0), dev("a", 2, 0)]);
    assert_eq!(order(&s, Role::Output), ["a", "b"]);
}

#[test]
fn new_devices_keep_discovery_order() {
    let mut s = State::default();
    run(&mut s, vec![dev("c", 2, 0), dev("a", 2, 0), dev("b", 2, 0)]);
    assert_eq!(order(&s, Role::Output), ["c", "a", "b"]);
}

#[test]
fn full_duplex_device_joins_both_lists() {
    let mut s = State::default();
    run(&mut s, vec![dev("h", 2, 1)]);
    assert_eq!(order(&s, Role::Output), ["h"]);
    assert_eq!(order(&s, Role::Input), ["h"]);
    let want: BTreeSet<Role> = [Role::Output, Role::Input].into();
    assert_eq!(s.devices[&key("h")].roles, want);
}

#[test]
fn virtual_and_aggregate_start_never_use() {
    let mut s = State::default();
    let mut bh = dev("bh", 2, 2);
    bh.transport = Transport::Virtual;
    let mut agg = dev("agg", 2, 1);
    agg.transport = Transport::Aggregate;
    run(&mut s, vec![bh, agg]);
    for role in Role::ALL {
        assert!(s.list(role).never_use.contains(&key("bh")));
        assert!(s.list(role).never_use.contains(&key("agg")));
        assert_eq!(order(&s, role), ["bh", "agg"]);
    }
}

#[test]
fn opting_a_virtual_device_back_in_sticks() {
    let mut s = State::default();
    let mut bh = dev("bh", 2, 0);
    bh.transport = Transport::Virtual;
    run(&mut s, vec![bh.clone()]);
    s.output.never_use.remove(&key("bh"));
    run(&mut s, vec![bh]);
    assert!(!s.output.never_use.contains(&key("bh")));
}

#[test]
fn hidden_and_dead_devices_are_not_added() {
    let mut s = State::default();
    let mut h = dev("h", 2, 1);
    h.hidden = true;
    let mut d = dev("d", 2, 1);
    d.alive = false;
    run(&mut s, vec![h, d]);
    assert!(order(&s, Role::Output).is_empty() && order(&s, Role::Input).is_empty());
    assert!(s.devices.is_empty());
}

#[test]
fn last_seen_stamped_at_launch_and_on_disconnect() {
    let mut s = State::default();
    run(&mut s, vec![dev("a", 2, 0), dev("b", 2, 0)]);
    assert_eq!(s.devices[&key("a")].last_seen, Some(T0));
    let prev: BTreeSet<_> = [key("a"), key("b")].into();
    let t5 = Timestamp(T0.0 + 5);
    remember(
        &mut s,
        &snap(vec![dev("b", 2, 0)], None, None),
        Some(&prev),
        t5,
    );
    assert_eq!(s.devices[&key("a")].last_seen, Some(t5));
    assert_eq!(s.devices[&key("b")].last_seen, Some(T0));
}

#[test]
fn verified_family_is_recorded_and_never_cleared() {
    let uid = "AppleUSBAudioEngine:Shure:MV7:14200000:1,2";
    let mut s = State::default();
    let mut d = dev(uid, 0, 1);
    d.transport = Transport::Usb;
    d.serialless_usb = true;
    run(&mut s, vec![d.clone()]);
    let fam = Some("AppleUSBAudioEngine:Shure:MV7".to_string());
    assert_eq!(s.devices[&key(uid)].family, fam);
    d.serialless_usb = false;
    run(&mut s, vec![d]);
    assert_eq!(s.devices[&key(uid)].family, fam);
}
