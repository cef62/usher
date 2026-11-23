mod common;
use common::{T0, dev, key, order, snap};
use usher_core::{DeviceSnapshot, Merge, Role, State, Transport, merge_aliases, remember};

const FAM: &str = "AppleUSBAudioEngine:Shure:MV7";
const UUID: &str = "37D8832A-2D66-02CA-B9F7-8F30A301B230";

fn usb_ep(port: &str, ep: &str, serialless: bool) -> DeviceSnapshot {
    let mut d = dev(&format!("{FAM}:{port}:{ep}"), 1, 1);
    d.transport = Transport::Usb;
    d.serialless_usb = serialless;
    d
}

fn usb(port: &str, serialless: bool) -> DeviceSnapshot {
    usb_ep(port, "1", serialless)
}

fn uid(port: &str) -> String {
    format!("{FAM}:{port}:1")
}

fn display(uid: &str) -> DeviceSnapshot {
    let mut d = dev(uid, 2, 0);
    d.transport = Transport::Hdmi;
    d
}

/// Mirrors reconcile: merge, then remember.
fn step(s: &mut State, devs: Vec<DeviceSnapshot>) -> Vec<Merge> {
    let sn = snap(devs, None, None);
    let merges = merge_aliases(s, &sn);
    remember(s, &sn, None, T0);
    merges
}

fn m(from: &str, to: &str) -> Merge {
    Merge {
        from: key(from),
        to: key(to),
    }
}

const P1: &str = "14200000";
const P2: &str = "14300000";

#[test]
fn usb_port_move_keeps_rank_and_never_use_per_role() {
    let mut s = State::default();
    step(&mut s, vec![usb(P1, true), dev("other", 1, 1)]);
    s.input.order = vec![key(&uid(P1)), key("other")];
    s.output.order = vec![key("other"), key(&uid(P1))];
    s.output.never_use.insert(key(&uid(P1)));
    let merges = step(&mut s, vec![usb(P2, true)]);
    assert_eq!(merges, [m(&uid(P1), &uid(P2))]);
    assert_eq!(order(&s, Role::Input), [uid(P2).as_str(), "other"]);
    assert_eq!(order(&s, Role::Output), ["other", uid(P2).as_str()]);
    assert!(s.output.never_use.contains(&key(&uid(P2))));
    assert!(!s.input.never_use.contains(&key(&uid(P2))));
    assert!(!usher_core::to_json(&s).contains(&uid(P1)));
}

#[test]
fn display_reconnect_keeps_rank() {
    let (a, b) = (format!("{UUID}_00000001"), format!("{UUID}_00000002"));
    let mut s = State::default();
    step(&mut s, vec![dev("other", 1, 0), display(&a)]);
    s.output.order = vec![key(&a), key("other")];
    let merges = step(&mut s, vec![display(&b)]);
    assert_eq!(merges, [m(&a, &b)]);
    assert_eq!(order(&s, Role::Output), [b.as_str(), "other"]);
}

#[test]
fn identical_devices_seen_together_latch() {
    let mut s = State::default();
    let merges = step(&mut s, vec![usb(P1, true), usb(P2, true)]);
    assert!(merges.is_empty());
    assert!(s.ambiguous_families.contains(FAM));
    let merges = step(&mut s, vec![usb("14400000", true)]);
    assert!(merges.is_empty());
    assert_eq!(order(&s, Role::Output).len(), 3);
}

#[test]
fn serial_usb_keeps_its_own_identity() {
    let mut s = State::default();
    step(&mut s, vec![usb(P1, true)]);
    assert!(step(&mut s, vec![usb(P2, false)]).is_empty());
    assert_eq!(
        order(&s, Role::Output),
        [uid(P1).as_str(), uid(P2).as_str()]
    );
}

#[test]
fn live_device_with_location_shaped_serial_keeps_its_settings() {
    let mut s = State::default();
    step(&mut s, vec![usb(P1, false)]);
    assert!(step(&mut s, vec![usb(P1, false), usb(P2, true)]).is_empty());
    assert_eq!(
        order(&s, Role::Output),
        [uid(P1).as_str(), uid(P2).as_str()]
    );
}

#[test]
fn unverified_offline_record_cannot_donate() {
    let mut s = State::default();
    step(&mut s, vec![usb(P1, false)]);
    assert!(s.devices[&key(&uid(P1))].family.is_none());
    assert!(step(&mut s, vec![usb(P2, true)]).is_empty());
    assert_eq!(
        order(&s, Role::Output),
        [uid(P1).as_str(), uid(P2).as_str()]
    );
}

#[test]
fn late_usb_descriptor_does_not_strand_settings() {
    let mut s = State::default();
    step(&mut s, vec![usb(P1, true)]);
    step(&mut s, vec![usb(P2, false)]);
    assert_eq!(
        order(&s, Role::Output),
        [uid(P1).as_str(), uid(P2).as_str()]
    );
    let merges = step(&mut s, vec![usb(P2, true)]);
    assert_eq!(merges, [m(&uid(P1), &uid(P2))]);
    assert_eq!(order(&s, Role::Output), [uid(P2).as_str()]);
}

#[test]
fn zero_uuid_keeps_exact_uids() {
    let z = "00000000-0000-0000-0000-000000000000";
    let mut s = State::default();
    step(&mut s, vec![display(&format!("{z}_00000001"))]);
    assert!(step(&mut s, vec![display(&format!("{z}_00000002"))]).is_empty());
    assert_eq!(order(&s, Role::Output).len(), 2);
}

#[test]
fn unrelated_displays_keep_identity() {
    let other = "47D8832A-2D66-02CA-B9F7-8F30A301B230";
    let mut s = State::default();
    step(&mut s, vec![display(&format!("{UUID}_00000001"))]);
    assert!(step(&mut s, vec![display(&format!("{other}_00000001"))]).is_empty());
    assert_eq!(order(&s, Role::Output).len(), 2);
}

#[test]
fn different_endpoint_is_not_merged() {
    let mut s = State::default();
    step(&mut s, vec![usb_ep(P1, "1", true)]);
    assert!(step(&mut s, vec![usb_ep(P2, "2", true)]).is_empty());
    assert_eq!(order(&s, Role::Output).len(), 2);
}
