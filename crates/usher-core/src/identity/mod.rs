//! Port identity parsing: the only two UID shapes that may be aliased (audit §3.3).
use crate::{DeviceSnapshot, Transport};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PortKind {
    Usb,
    Display,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PortIdentity {
    pub kind: PortKind,
    pub family: String,
    pub port: String,
    pub endpoint: String,
}

pub fn parse_port(uid: &str) -> Option<PortIdentity> {
    parse_usb(uid).or_else(|| parse_display(uid))
}

fn parse_usb(uid: &str) -> Option<PortIdentity> {
    let p: Vec<&str> = uid.split(':').collect();
    if p.len() != 5 || p[0] != "AppleUSBAudioEngine" || p[1].is_empty() || p[2].is_empty() {
        return None;
    }
    // from_str_radix alone accepts a leading '+', so check the digits first.
    if !(1..=8).contains(&p[3].len()) || !p[3].bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let loc = u32::from_str_radix(p[3], 16).ok()?;
    // Our reading of "comma-separated integers": non-empty, digits-only segments.
    let endpoints_ok = p[4]
        .split(',')
        .all(|e| !e.is_empty() && e.bytes().all(|b| b.is_ascii_digit()));
    if loc == 0 || !endpoints_ok {
        return None;
    }
    Some(PortIdentity {
        kind: PortKind::Usb,
        family: format!("{}:{}:{}", p[0], p[1], p[2]),
        port: p[3].to_string(),
        endpoint: p[4].to_string(),
    })
}

fn parse_display(uid: &str) -> Option<PortIdentity> {
    let (uuid, suffix) = match uid.split_once('_') {
        Some((u, s)) => (u, Some(s)),
        None => (uid, None),
    };
    if suffix.is_some_and(|s| s.len() != 8 || !s.bytes().all(|b| b.is_ascii_hexdigit())) {
        return None;
    }
    let shape_ok = uuid.len() == 36
        && uuid.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        });
    if !shape_ok || uuid.bytes().all(|b| b == b'0' || b == b'-') {
        return None;
    }
    Some(PortIdentity {
        kind: PortKind::Display,
        family: uuid.to_string(),
        port: uid.to_string(),
        endpoint: String::new(),
    })
}

pub fn alias_identity(d: &DeviceSnapshot) -> Option<PortIdentity> {
    let id = parse_port(d.uid.as_str())?;
    let ok = match id.kind {
        PortKind::Usb => d.transport == Transport::Usb && d.serialless_usb,
        PortKind::Display => d.transport.is_display(),
    };
    ok.then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISP: &str = "37D8832A-2D66-02CA-B9F7-8F30A301B230";

    fn dev(uid: &str, transport: Transport, serialless: bool) -> DeviceSnapshot {
        DeviceSnapshot {
            uid: uid.into(),
            name: "d".into(),
            transport,
            alive: true,
            hidden: false,
            input_streams: 0,
            output_streams: 1,
            serialless_usb: serialless,
        }
    }

    #[test]
    fn usb_uid_parses() {
        let id = parse_port("AppleUSBAudioEngine:Shure:MV7:14200000:1,2").unwrap();
        assert_eq!(id.kind, PortKind::Usb);
        assert_eq!(id.family, "AppleUSBAudioEngine:Shure:MV7");
        assert_eq!(id.port, "14200000");
        assert_eq!(id.endpoint, "1,2");
    }

    #[test]
    fn usb_rejects() {
        for uid in [
            "AppleUSBAudioEngine:Shure:MV7:14200000",
            "Other:Shure:MV7:14200000:1,2",
            "AppleUSBAudioEngine::MV7:14200000:1,2",
            "AppleUSBAudioEngine:Shure:MV7:0:1,2",
            "AppleUSBAudioEngine:Shure:MV7:123456789:1,2",
            "AppleUSBAudioEngine:Shure:MV7:zz:1,2",
            "AppleUSBAudioEngine:Shure:MV7:14200000:a",
            "AppleUSBAudioEngine:V:P:+1:1",
            "AppleUSBAudioEngine:V:P:012345678:1",
            "AppleUSBAudioEngine:V:P:00000000:1",
            "AppleUSBAudioEngine:V:P:14200000:1,",
            "AppleUSBAudioEngine:V:P:14200000:1,,2",
            "AppleUSBAudioEngine:V:P:14200000:",
        ] {
            assert_eq!(parse_port(uid), None, "{uid}");
        }
    }

    #[test]
    fn display_uid_parses() {
        let uid = format!("{DISP}_00000002");
        let id = parse_port(&uid).unwrap();
        assert_eq!(id.kind, PortKind::Display);
        assert_eq!(id.family, DISP);
        assert_eq!(id.port, uid);
        assert_eq!(id.endpoint, "");
        assert_eq!(parse_port(DISP).unwrap().port, DISP);
    }

    #[test]
    fn lowercase_forms_parse_as_given() {
        let id = parse_port("AppleUSBAudioEngine:V:P:142a0000:1").unwrap();
        assert_eq!(id.port, "142a0000");
        let lower = DISP.to_lowercase();
        assert_eq!(parse_port(&lower).unwrap().family, lower);
    }

    #[test]
    fn zero_uuid_is_never_aliased() {
        let z = "00000000-0000-0000-0000-000000000000";
        assert_eq!(parse_port(z), None);
        assert_eq!(parse_port(&format!("{z}_00000001")), None);
    }

    #[test]
    fn display_rejects() {
        let short = &DISP[..35];
        assert_eq!(short.len(), 35);
        assert_eq!(parse_port(short), None);
        assert_eq!(parse_port(&format!("{DISP}_123")), None);
        assert_eq!(parse_port(&format!("{DISP}_0000000g")), None);
        assert_eq!(parse_port(&format!("{DISP}_0000000")), None);
        assert_eq!(parse_port(&format!("{DISP}_000000000")), None);
        assert_eq!(parse_port("37D8832G-2D66-02CA-B9F7-8F30A301B230"), None);
    }

    #[test]
    fn alias_identity_needs_live_verdict() {
        let usb = "AppleUSBAudioEngine:Shure:MV7:14200000:1,2";
        let disp = format!("{DISP}_00000002");
        assert!(alias_identity(&dev(usb, Transport::Usb, false)).is_none());
        assert!(alias_identity(&dev(usb, Transport::Usb, true)).is_some());
        assert!(alias_identity(&dev(usb, Transport::Bluetooth, true)).is_none());
        assert!(alias_identity(&dev(&disp, Transport::Hdmi, false)).is_some());
        assert!(alias_identity(&dev(&disp, Transport::Usb, true)).is_none());
    }
}
