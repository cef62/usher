mod common;
use common::*;
use usher_core::*;

#[test]
fn known_uid_shapes_render_safely() {
    let mut r = Redactor::default();
    assert_eq!(r.uid("BuiltInSpeakerDevice", false), "BuiltInSpeakerDevice");
    assert_eq!(
        r.uid("00-11-22-33-CF-9D:output", false),
        "xx-xx-xx-xx-CF-9D:output"
    );
    assert_eq!(
        r.uid("00-11-22-33-cf-9d:input", false),
        "xx-xx-xx-xx-cf-9d:input"
    );
    assert_eq!(
        r.uid(
            "AppleUSBAudioEngine:Focusrite:Scarlett 2i2 USB:Y8ABC123:1",
            false
        ),
        "AppleUSBAudioEngine:Focusrite:Scarlett 2i2 USB:<serial>:1"
    );
    assert_eq!(
        r.uid("AppleUSBAudioEngine:Generic:USB Audio:14200000:1", true),
        "AppleUSBAudioEngine:Generic:USB Audio:14200000:1"
    );
}

#[test]
fn unknown_uid_shapes_become_indexed_placeholders() {
    let mut r = Redactor::default();
    let disp = "37D8832A-2D66-02CA-B9F7-8F30A301B230";
    assert_eq!(r.uid(disp, false), "uid#1");
    assert_eq!(r.uid("com.vendor.driver:Ada-Mac:SN12345", false), "uid#2");
    assert_eq!(r.uid(disp, true), "uid#1", "same key, same index");
    assert_eq!(
        r.uid("AppleUSBAudioEngine:a:b:c", false),
        "uid#3",
        "4 fields is not the USB shape"
    );
    assert_eq!(r.uid("00-11-22-33-CF-9D:headset", false), "uid#4");
    assert_eq!(
        r.family("37D8832A", disp),
        "uid#1",
        "an alias renders as its key"
    );
    assert_eq!(
        r.family(
            "AppleUSBAudioEngine:Shure:MV7",
            "AppleUSBAudioEngine:Shure:MV7:14200000:1"
        ),
        "AppleUSBAudioEngine:Shure:MV7"
    );
    assert_eq!(
        r.family("Ada-Mac-Alias", "Ada-Mac-Alias"),
        "uid#5",
        "ambiguous family has no key"
    );
}

#[test]
fn owner_names_and_home_are_dropped() {
    let ada = vec!["Ada".to_string(), "Lovelace".to_string(), "ada".to_string()];
    assert_eq!(strip_owner("Ada's AirPods Pro", &ada), "AirPods Pro");
    assert_eq!(strip_owner("Ada’s AirPods Pro", &ada), "AirPods Pro");
    assert_eq!(strip_owner("AirPods Pro di Ada", &ada), "AirPods Pro");
    assert_eq!(strip_owner("AirPods von Ada Lovelace", &ada), "AirPods");
    assert_eq!(
        strip_owner("Adapter Speakers", &ada),
        "Adapter Speakers",
        "whole words only"
    );
    assert_eq!(
        strip_owner("Studio Display Speakers", &[]),
        "Studio Display Speakers"
    );
    assert_eq!(
        tilde(
            "/Users/ada/Library/Logs/com.cef62.usher/usher.log",
            "/Users/ada"
        ),
        "~/Library/Logs/com.cef62.usher/usher.log"
    );
    assert_eq!(tilde("/Users/ada2/x", "/Users/ada"), "/Users/ada2/x");
}

fn named(uid: &str, name: &str, t: Transport) -> DeviceSnapshot {
    DeviceSnapshot {
        name: name.into(),
        transport: t,
        ..dev(uid, 1, 1)
    }
}

#[test]
fn diagnose_reports_every_ranked_skip() {
    let mut state = State::default();
    state.output.order = vec![key("pods"), key("disp"), key("spk")];
    state.output.never_use.insert(key("spk"));
    let snap = snap(
        vec![dev("disp", 1, 0), dev("spk", 1, 0)],
        Some("disp"),
        None,
    );
    let d = diagnose(
        &state,
        &snap,
        EngineStatus {
            scanned: true,
            ..Default::default()
        },
    );
    let skips: Vec<_> = d.roles[0].ranked.iter().map(|r| r.skip).collect();
    assert_eq!(
        skips,
        vec![Some(Skip::Disconnected), None, Some(Skip::NeverUse)]
    );
    assert_eq!(d.roles[0].desired, Some(key("disp")));
}

fn leaky() -> Diagnostics {
    let pods = "00-11-22-33-CF-9D:output";
    let scar = "AppleUSBAudioEngine:Focusrite:Scarlett 2i2 USB:Y8ABC123:1";
    let disp = "37D8832A-2D66-02CA-B9F7-8F30A301B230";
    let mut state = State::default();
    state.output.order = vec![key(disp), key(pods), key(scar)];
    state.ambiguous_families.insert("Ada-Mac-Alias".into());
    let snap = snap(
        vec![
            named(pods, "Ada's AirPods Pro", Transport::Bluetooth),
            named(scar, "Scarlett 2i2 USB", Transport::Usb),
            named(disp, "Studio Display", Transport::DisplayPort),
        ],
        Some(pods),
        None,
    );
    let mut d = diagnose(
        &state,
        &snap,
        EngineStatus {
            scanned: true,
            subscribed: true,
            ..Default::default()
        },
    );
    d.app = AppInfo {
        version: "0.2.0".into(),
        macos: "26.1".into(),
        log_path: "/Users/ada/Library/Logs/com.cef62.usher/usher.log".into(),
        owner_names: vec!["Ada".into(), "Lovelace".into()],
    };
    d
}

#[test]
fn rendered_report_leaks_no_address_serial_owner_or_home() {
    let text = render_diagnostics(&leaky(), "/Users/ada");
    for leak in ["00-11-22-33", "Y8ABC123", "Ada", "/Users/ada", "37D8832A"] {
        assert!(!text.contains(leak), "{leak} in\n{text}");
    }
    assert!(
        text.contains("xx-xx-xx-xx-CF-9D:output")
            && text.contains("<serial>")
            && text.contains("uid#1")
    );
    assert!(text.contains("ambiguous aliases: uid#"));
    assert!(
        text.starts_with(
            "usher 0.2.0 · macOS 26.1 · log ~/Library/Logs/com.cef62.usher/usher.log\n"
        )
    );
    assert!(text.contains(
        "output: desired Studio Display (uid#1), current AirPods Pro (xx-xx-xx-xx-CF-9D:output)"
    ));
}

#[test]
fn report_before_the_first_scan_says_so() {
    let empty = snap(vec![], None, None);
    let text = render_diagnostics(
        &diagnose(&State::default(), &empty, EngineStatus::default()),
        "/Users/u",
    );
    assert!(text.contains("no scan yet") && text.contains("engine: not subscribed"));
}

#[test]
fn rebinding_shows_the_attempt() {
    let mut d = leaky();
    d.engine.rebinding = Some(3);
    assert!(render_diagnostics(&d, "/h").contains("rebinding: attempt 3"));
}

#[test]
fn owner_names_are_stripped_in_every_position() {
    let o: Vec<String> = ["Ada", "Lovelace", "Zoë"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for (input, want) in [
        ("AirPods Pro (Ada)", "AirPods Pro"),
        ("Ada, AirPods", "AirPods"),
        ("AirPods di ADA.", "AirPods"),
        ("AirPods Pro Ada's", "AirPods Pro"),
        ("AirPods-Ada", "unknown device"),
        ("Ada's", "unknown device"),
        ("ZOË AirPods", "AirPods"),
        ("AirPods Ada's Lovelace", "AirPods"),
    ] {
        assert_eq!(strip_owner(input, &o), want, "{input}");
    }
    assert_eq!(tilde("/Users/ada/Library/x", "/Users/ada/"), "~/Library/x");
}

#[test]
fn one_key_renders_the_same_in_ranked_and_devices_lines() {
    let k = "AppleUSBAudioEngine:Generic:USB Audio:14200000:1";
    let mut state = State::default();
    state.output.order = vec![key(k)];
    state.devices.insert(
        key(k),
        DeviceRecord {
            name: "USB Audio".into(),
            transport: Transport::Usb,
            roles: Default::default(),
            last_seen: None,
            family: Some("AppleUSBAudioEngine:Generic:USB Audio".into()),
        },
    );
    let snap = snap(vec![named(k, "USB Audio", Transport::Usb)], Some(k), None);
    let d = diagnose(
        &state,
        &snap,
        EngineStatus {
            scanned: true,
            ..Default::default()
        },
    );
    let text = render_diagnostics(&d, "/h");
    assert_eq!(
        text.matches(k).count(),
        4,
        "desired, ranked, device: {text}"
    );
    assert!(!text.contains("<serial>"), "{text}");
}

#[test]
fn current_key_outside_the_order_renders_like_its_devices_line() {
    let k = "AppleUSBAudioEngine:Generic:USB Audio:14200000:1";
    let mut state = State::default();
    state.devices.insert(
        key(k),
        DeviceRecord {
            name: "USB Audio".into(),
            transport: Transport::Usb,
            roles: Default::default(),
            last_seen: None,
            family: Some("AppleUSBAudioEngine:Generic:USB Audio".into()),
        },
    );
    let snap = snap(vec![named(k, "USB Audio", Transport::Usb)], Some(k), None);
    let d = diagnose(
        &state,
        &snap,
        EngineStatus {
            scanned: true,
            ..Default::default()
        },
    );
    let text = render_diagnostics(&d, "/h");
    assert!(text.contains(&format!("current USB Audio ({k})")), "{text}");
    assert!(text.contains(&format!("USB Audio · {k} ·")), "{text}");
    assert!(!text.contains("<serial>"), "{text}");
}
