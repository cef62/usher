//! IOKit side of the serial-less USB check (audit §3.3 steps 1-5).
use objc2_core_foundation::{CFNumber, CFRetained, CFString, CFType};
use objc2_io_kit::{
    IOIteratorNext, IOObjectRelease, IORegistryEntryCreateCFProperty, IOServiceGetMatchingServices,
    IOServiceMatching, kIOMainPortDefault,
};

#[derive(Clone, Debug)]
pub(crate) struct UsbNode {
    pub location: u32,
    pub serial: Option<String>,
    pub serial_index: u8,
}

/// `Some(true)`: UID embeds the port. `Some(false)`: stable. `None`: not resolvable (yet).
pub(crate) fn serialless(uid: &str, nodes: &[UsbNode]) -> Option<bool> {
    let parts: Vec<&str> = uid.split(':').collect();
    let [_, _, _, hex, _] = parts[..] else {
        return None;
    };
    if nodes.iter().any(|n| n.serial.as_deref() == Some(hex)) {
        return Some(false);
    }
    let location = u32::from_str_radix(hex, 16).ok()?;
    let node = nodes.iter().find(|n| n.location == location)?;
    if node.serial.as_deref().is_some_and(|s| !s.is_empty()) {
        return Some(false);
    }
    Some(node.serial_index == 0)
}

fn prop(entry: u32, key: &str) -> Option<CFRetained<CFType>> {
    let key = CFString::from_str(key);
    // SAFETY: `entry` is a live registry entry owned by the caller; `key` outlives the call.
    unsafe { IORegistryEntryCreateCFProperty(entry, Some(&key), None, 0) }
}

fn num(entry: u32, key: &str) -> Option<i64> {
    prop(entry, key)?.downcast_ref::<CFNumber>()?.as_i64()
}

fn node(entry: u32) -> Option<UsbNode> {
    let serial = prop(entry, "USB Serial Number")
        .and_then(|v| v.downcast_ref::<CFString>().map(|s| s.to_string()));
    Some(UsbNode {
        location: u32::try_from(num(entry, "locationID")?).ok()?,
        serial,
        serial_index: num(entry, "iSerialNumber").map_or(0, |n| n as u8),
    })
}

/// Empty on any IOKit failure, which reads as "unresolved".
pub(crate) fn usb_nodes() -> Vec<UsbNode> {
    // SAFETY: the name is a NUL-terminated C string literal.
    let Some(matching) = (unsafe { IOServiceMatching(c"IOUSBHostDevice".as_ptr()) }) else {
        return Vec::new();
    };
    let mut iter = 0;
    // SAFETY: `iter` is a stack out-pointer; the matching dictionary is consumed by the call.
    let kr = unsafe {
        IOServiceGetMatchingServices(kIOMainPortDefault, Some((&matching).into()), &raw mut iter)
    };
    if kr != 0 {
        return Vec::new();
    }
    let mut nodes = Vec::new();
    loop {
        let entry = IOIteratorNext(iter);
        if entry == 0 {
            break;
        }
        nodes.extend(node(entry));
        IOObjectRelease(entry);
    }
    IOObjectRelease(iter);
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    const U: &str = "AppleUSBAudioEngine:Unknown Manufacturer:USB Audio:14200000:2,1";

    fn node(location: u32, serial: Option<&str>, idx: u8) -> UsbNode {
        UsbNode {
            location,
            serial: serial.map(String::from),
            serial_index: idx,
        }
    }

    #[test]
    fn non_five_part_uid_is_unresolved() {
        assert_eq!(serialless("BuiltInSpeakerDevice", &[]), None);
    }
    #[test]
    fn hex_that_is_a_real_serial_is_not_serialless() {
        assert_eq!(
            serialless(U, &[node(0x1, Some("14200000"), 3)]),
            Some(false)
        );
    }
    #[test]
    fn no_node_at_that_location_is_unresolved() {
        assert_eq!(serialless(U, &[node(0x14300000, None, 0)]), None);
    }
    #[test]
    fn node_with_a_serial_is_not_serialless() {
        assert_eq!(
            serialless(U, &[node(0x14200000, Some("ABC123"), 3)]),
            Some(false)
        );
    }
    #[test]
    fn node_without_serial_and_index_zero_is_serialless() {
        assert_eq!(serialless(U, &[node(0x14200000, None, 0)]), Some(true));
        assert_eq!(serialless(U, &[node(0x14200000, Some(""), 0)]), Some(true));
    }
    #[test]
    fn node_without_serial_but_nonzero_index_is_not() {
        assert_eq!(serialless(U, &[node(0x14200000, None, 2)]), Some(false));
    }
    #[test]
    fn non_hex_location_is_unresolved() {
        let uid = "AppleUSBAudioEngine:V:P:zzzz:2,1";
        assert_eq!(serialless(uid, &[node(0, None, 0)]), None);
    }
}
