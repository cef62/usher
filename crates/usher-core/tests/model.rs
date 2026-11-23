mod common;
use common::{dev, key};
use usher_core::{DeviceSnapshot, Role, Timestamp, Transport};

#[test]
fn headset_has_both_roles() {
    let d = dev("h", 2, 1);
    assert!(d.has_role(Role::Output) && d.has_role(Role::Input));
    assert!(!dev("spk", 2, 0).has_role(Role::Input));
}
#[test]
fn virtual_and_aggregate_are_virtual() {
    assert!(Transport::Virtual.is_virtual() && Transport::Aggregate.is_virtual());
    assert!(!Transport::Usb.is_virtual());
    assert!(Transport::Hdmi.is_display() && Transport::DisplayPort.is_display());
}
#[test]
fn timestamp_since_saturates() {
    assert_eq!(Timestamp(5).since(Timestamp(9)), 0);
    assert_eq!(Timestamp(9).since(Timestamp(5)), 4);
}
#[test]
fn serde_names() {
    assert_eq!(serde_json::to_string(&Role::Output).unwrap(), "\"output\"");
    assert_eq!(
        serde_json::to_string(&Transport::BuiltIn).unwrap(),
        "\"built_in\""
    );
    assert_eq!(serde_json::to_string(&key("a")).unwrap(), "\"a\"");
}
#[test]
fn hidden_or_dead_is_not_listable() {
    assert!(
        !DeviceSnapshot {
            hidden: true,
            ..dev("x", 1, 0)
        }
        .listable()
    );
    assert!(
        !DeviceSnapshot {
            alive: false,
            ..dev("x", 1, 0)
        }
        .listable()
    );
}
