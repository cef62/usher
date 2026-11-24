//! Redaction for text that goes into public issues. Shapes are matched by splitting, no regex.

const USB: &str = "AppleUSBAudioEngine";

fn is_bt_address(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Renders device keys and aliases without hardware addresses or serials.
/// Unknown shapes become `uid#N`, numbered in order of first sight.
#[derive(Default)]
pub struct Redactor {
    seen: Vec<String>,
}

impl Redactor {
    fn placeholder(&mut self, uid: &str) -> String {
        let i = self.seen.iter().position(|s| s == uid).unwrap_or_else(|| {
            self.seen.push(uid.to_string());
            self.seen.len() - 1
        });
        format!("uid#{}", i + 1)
    }

    /// `serialless`: the USB key's 4th field is a port location, not a serial, so it stays.
    pub fn uid(&mut self, uid: &str, serialless: bool) -> String {
        let f: Vec<&str> = uid.split(':').collect();
        match f.as_slice() {
            [one]
                if one.starts_with("BuiltIn") && one.bytes().all(|b| b.is_ascii_alphanumeric()) =>
            {
                uid.into()
            }
            [addr, role @ ("output" | "input")] if is_bt_address(addr) => {
                format!("xx-xx-xx-xx-{}:{role}", &addr[12..])
            }
            [USB, vendor, product, serial, n]
                if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) =>
            {
                let serial = if serialless { serial } else { &"<serial>" };
                format!("{USB}:{vendor}:{product}:{serial}:{n}")
            }
            _ => self.placeholder(uid),
        }
    }

    /// A USB family (`AppleUSBAudioEngine:<vendor>:<product>`) is safe as is; any other
    /// alias is an opaque id and renders as its device key. An ambiguous family has no
    /// key: callers pass the family as `key`.
    pub fn family(&mut self, family: &str, key: &str) -> String {
        if family.split(':').count() == 3 && family.starts_with("AppleUSBAudioEngine:") {
            family.into()
        } else {
            self.uid(key, false)
        }
    }
}

const CONNECTORS: [&str; 8] = ["di", "de", "da", "du", "del", "von", "van", "of"];

/// Drops owner names from a device name: a leading `<x>'s`, and whole-word matches
/// (with one connector word before them).
pub fn strip_owner(name: &str, owner_names: &[String]) -> String {
    let mut words: Vec<&str> = name.split_whitespace().collect();
    if words.len() > 1 && (words[0].ends_with("'s") || words[0].ends_with("’s")) {
        words.remove(0);
    }
    let owners: Vec<String> = owner_names
        .iter()
        .filter(|o| o.chars().count() >= 3)
        .map(|o| o.to_lowercase())
        .collect();
    let mut out: Vec<&str> = Vec::new();
    for w in words {
        let is_owner = w
            .split(|c: char| !c.is_alphanumeric())
            .any(|part| owners.contains(&part.to_lowercase()));
        if !is_owner {
            out.push(w);
        } else if out
            .last()
            .is_some_and(|p| CONNECTORS.contains(&p.to_lowercase().as_str()))
        {
            out.pop();
        }
    }
    if out.is_empty() {
        "unknown device".into()
    } else {
        out.join(" ")
    }
}

pub fn tilde(path: &str, home: &str) -> String {
    let home = home.trim_end_matches('/');
    match path.strip_prefix(home) {
        Some(rest) if !home.is_empty() && (rest.is_empty() || rest.starts_with('/')) => {
            format!("~{rest}")
        }
        _ => path.into(),
    }
}
