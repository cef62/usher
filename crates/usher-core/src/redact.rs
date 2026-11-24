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

/// Hangul, CJK, kana and CJK compatibility letters: names in these scripts are short.
pub fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x1100..=0x11FF | 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF65..=0xFF9F)
}

/// Full-width Latin (`Ｔａｒｏ`) folded to ASCII.
fn fold_wide(s: &str) -> String {
    s.chars()
        .map(|c| match c as u32 {
            n @ 0xFF01..=0xFF5E => char::from_u32(n - 0xFEE0).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// Owner words match a whole part; non-ASCII ones also inside it (`山田の…`, names glue to particles),
/// ASCII ones also as a sub-part split at non-ASCII letters (`Taroの`, but never `Adapter` for `Ada`).
fn hits(owners: &[String], part: &str) -> bool {
    owners.iter().any(|o| {
        o == part
            || if o.is_ascii() {
                part.split(|c: char| !c.is_ascii_alphanumeric())
                    .any(|s| s == o)
            } else {
                part.contains(o.as_str())
            }
    })
}

/// Drops owner names from a device name: a leading `<x>'s`, and whole-word matches
/// (with one connector word before them); non-ASCII names also match inside a word, and ASCII
/// names also match next to a non-ASCII particle (`Taroの`).
pub fn strip_owner(name: &str, owner_names: &[String]) -> String {
    let mut words: Vec<&str> = name.split_whitespace().collect();
    if words.len() > 1 && (words[0].ends_with("'s") || words[0].ends_with("’s")) {
        words.remove(0);
    }
    let owners: Vec<String> = owner_names
        .iter()
        // CJK and Hangul names are short; Latin and Cyrillic ones keep the 3-letter floor.
        .filter(|o| o.chars().count() >= if o.chars().any(is_cjk) { 2 } else { 3 })
        .map(|o| fold_wide(o).to_lowercase())
        .collect();
    let mut out: Vec<&str> = Vec::new();
    for w in words {
        let is_owner = w
            .split(|c: char| !c.is_alphanumeric())
            .any(|part| hits(&owners, &fold_wide(part).to_lowercase()));
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
