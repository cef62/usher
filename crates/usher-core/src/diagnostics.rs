use crate::desired::skip_cause;
use crate::{
    DeviceKey, Redactor, Role, Skip, Snapshot, State, Transport, decide, strip_owner, tilde,
};

#[derive(Clone, Default, PartialEq, Debug)]
pub struct AppInfo {
    pub version: String,
    pub macos: String,
    pub log_path: String,
    pub owner_names: Vec<String>,
}

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct EngineStatus {
    pub scanned: bool,
    pub subscribed: bool,
    /// Attempt number while a rebind is pending.
    pub rebinding: Option<u32>,
    pub read_only: bool,
    pub usb_unresolved: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub struct RankedReport {
    pub key: DeviceKey,
    pub name: String,
    pub skip: Option<Skip>,
    pub serialless: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub struct RoleReport {
    pub role: Role,
    pub desired: Option<DeviceKey>,
    pub current: Option<DeviceKey>,
    pub ranked: Vec<RankedReport>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct DeviceReport {
    pub key: DeviceKey,
    pub name: String,
    pub transport: Transport,
    pub alive: bool,
    pub hidden: bool,
    pub serialless_usb: bool,
    pub family: Option<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Diagnostics {
    pub app: AppInfo,
    pub engine: EngineStatus,
    pub roles: Vec<RoleReport>,
    pub devices: Vec<DeviceReport>,
    pub ambiguous_families: Vec<String>,
}

fn role_report(state: &State, snap: &Snapshot, role: Role) -> RoleReport {
    let ranked = state
        .list(role)
        .order
        .iter()
        .map(|key| {
            let live = snap.device(key);
            let rec = state.devices.get(key);
            RankedReport {
                key: key.clone(),
                // Never fall back to the key: it can hold an address or serial.
                name: live
                    .map(|d| d.name.clone())
                    .or_else(|| rec.map(|r| r.name.clone()))
                    .unwrap_or_else(|| "unknown device".into()),
                skip: skip_cause(state, snap, role, key),
                serialless: live.is_some_and(|d| d.serialless_usb)
                    || rec.is_some_and(|r| r.family.is_some()),
            }
        })
        .collect();
    RoleReport {
        role,
        desired: decide(state, snap, role).desired,
        current: snap.current.get(role).clone(),
        ranked,
    }
}

pub fn diagnose(state: &State, snap: &Snapshot, engine: EngineStatus) -> Diagnostics {
    Diagnostics {
        app: AppInfo::default(),
        engine,
        roles: Role::ALL
            .iter()
            .map(|&r| role_report(state, snap, r))
            .collect(),
        devices: snap
            .devices
            .iter()
            .map(|d| DeviceReport {
                key: d.uid.clone(),
                name: d.name.clone(),
                transport: d.transport,
                alive: d.alive,
                hidden: d.hidden,
                serialless_usb: d.serialless_usb,
                family: state.devices.get(&d.uid).and_then(|r| r.family.clone()),
            })
            .collect(),
        ambiguous_families: state.ambiguous_families.iter().cloned().collect(),
    }
}

fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

fn transport_name(t: Transport) -> String {
    serde_json::to_value(t)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "other".into())
}

fn header(d: &Diagnostics, home: &str) -> String {
    let (a, e) = (&d.app, &d.engine);
    let rebinding = e
        .rebinding
        .map_or("no".to_string(), |n| format!("attempt {n}"));
    format!(
        "usher {} · macOS {} · log {}\nengine: {} · rebinding: {rebinding} · read-only store: {} · USB verdict unresolved: {}\n",
        a.version,
        a.macos,
        tilde(&a.log_path, home),
        if e.subscribed {
            "subscribed"
        } else {
            "not subscribed"
        },
        yes_no(e.read_only),
        yes_no(e.usb_unresolved),
    )
}

fn role_section(d: &Diagnostics, r: &RoleReport, red: &mut Redactor) -> String {
    let owners = &d.app.owner_names;
    let mut describe = |key: &Option<DeviceKey>| match key {
        None => "none".to_string(),
        Some(k) => {
            let known = r
                .ranked
                .iter()
                .find(|x| &x.key == k)
                .map(|x| (&x.name, x.serialless));
            let known = known.or_else(|| {
                d.devices
                    .iter()
                    .find(|x| &x.key == k)
                    .map(|x| (&x.name, x.serialless_usb))
            });
            let (name, serialless) =
                known.map_or(("unknown device", false), |(n, s)| (n.as_str(), s));
            format!(
                "{} ({})",
                strip_owner(name, owners),
                red.uid(k.as_str(), serialless)
            )
        }
    };
    let mut out = format!(
        "{}: desired {}, current {}\n",
        r.role,
        describe(&r.desired),
        describe(&r.current)
    );
    for (i, x) in r.ranked.iter().enumerate() {
        let why = x.skip.map_or(String::new(), |s| format!(" — {s}"));
        out += &format!(
            "  {}. {} ({}){why}\n",
            i + 1,
            strip_owner(&x.name, owners),
            red.uid(x.key.as_str(), x.serialless)
        );
    }
    out
}

fn device_line(d: &Diagnostics, x: &DeviceReport, red: &mut Redactor) -> String {
    let alias = x
        .family
        .as_deref()
        .map_or("none".to_string(), |f| red.family(f, x.key.as_str()));
    format!(
        "  {} · {} · {} · {} · {} · serial-less USB: {} · alias: {alias}\n",
        strip_owner(&x.name, &d.app.owner_names),
        red.uid(x.key.as_str(), x.serialless_usb),
        transport_name(x.transport),
        if x.alive { "alive" } else { "not alive" },
        if x.hidden { "hidden" } else { "visible" },
        yes_no(x.serialless_usb),
    )
}

/// Plain-text report for a public issue. Ambiguous families have no device key, so each
/// renders through `Redactor::family(f, f)`.
pub fn render_diagnostics(d: &Diagnostics, home: &str) -> String {
    let mut red = Redactor::default();
    let mut out = header(d, home);
    out.push('\n');
    if d.engine.scanned {
        for r in &d.roles {
            out += &role_section(d, r, &mut red);
        }
        out += "\ndevices in the last scan:\n";
        for x in &d.devices {
            out += &device_line(d, x, &mut red);
        }
    } else {
        out += "no scan yet\n";
    }
    let amb: Vec<String> = d
        .ambiguous_families
        .iter()
        .map(|f| red.family(f, f))
        .collect();
    out += &format!(
        "ambiguous aliases: {}\n",
        if amb.is_empty() {
            "none".into()
        } else {
            amb.join(", ")
        }
    );
    out
}
