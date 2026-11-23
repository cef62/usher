//! The one registry of UID-bearing structures in `State`.
use crate::{DeviceKey, State};
use std::collections::{BTreeMap, BTreeSet};

enum Edit {
    Keep,
    Rename(DeviceKey),
    Drop,
}

/// Apply `f` to every UID in `State`. This is the only place that lists the
/// UID-bearing fields; a new UID-bearing field must be added here.
fn edit_keys(state: &mut State, f: impl Fn(&DeviceKey) -> Edit) {
    for list in [&mut state.output, &mut state.input] {
        let mut seen = BTreeSet::new();
        let order = std::mem::take(&mut list.order);
        // Keep the first occurrence, so a renamed key lands at its better rank.
        list.order = order
            .into_iter()
            .filter_map(|key| mapped(&f, key))
            .filter(|key| seen.insert(key.clone()))
            .collect();
        let never = std::mem::take(&mut list.never_use);
        list.never_use = never
            .into_iter()
            .filter_map(|key| mapped(&f, key))
            .collect();
    }
    let devices = std::mem::take(&mut state.devices);
    let (mut kept, mut moved) = (BTreeMap::new(), Vec::new());
    for (key, rec) in devices {
        match f(&key) {
            Edit::Keep => {
                kept.insert(key, rec);
            }
            Edit::Rename(to) => moved.push((to, rec)),
            Edit::Drop => {}
        }
    }
    // An existing record at the target wins over the moved one.
    for (to, rec) in moved {
        kept.entry(to).or_insert(rec);
    }
    state.devices = kept;
    if let Some(p) = state.paused_reason.as_mut() {
        match f(&p.key) {
            Edit::Keep => {}
            Edit::Rename(to) => p.key = to,
            Edit::Drop => state.paused_reason = None,
        }
    }
}

fn mapped(f: &impl Fn(&DeviceKey) -> Edit, key: DeviceKey) -> Option<DeviceKey> {
    match f(&key) {
        Edit::Keep => Some(key),
        Edit::Rename(to) => Some(to),
        Edit::Drop => None,
    }
}

/// Rewrite `from` to `to` everywhere; on a collision the better order rank,
/// the never-use flag of either key and `to`'s device record survive.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn rename(state: &mut State, from: &DeviceKey, to: &DeviceKey) {
    edit_keys(state, |k| {
        if k == from && from != to {
            Edit::Rename(to.clone())
        } else {
            Edit::Keep
        }
    });
}

/// Drop `key` from every structure, clearing `paused_reason` if it names it.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn remove(state: &mut State, key: &DeviceKey) {
    edit_keys(state, |k| if k == key { Edit::Drop } else { Edit::Keep });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeviceRecord, PausedReason, Role, Transport, to_json};

    fn k(s: &str) -> DeviceKey {
        s.into()
    }

    fn rec(name: &str) -> DeviceRecord {
        DeviceRecord {
            name: name.into(),
            transport: Transport::Usb,
            roles: [Role::Output].into(),
            last_seen: None,
            family: None,
        }
    }

    /// Key "old" in every UID-bearing structure.
    fn fixture() -> State {
        let mut s = State::default();
        for role in Role::ALL {
            let l = s.list_mut(role);
            l.order = vec![k("old")];
            l.never_use.insert(k("old"));
        }
        s.devices.insert(k("old"), rec("Dev"));
        s.paused_reason = Some(PausedReason {
            role: Role::Output,
            key: k("old"),
            name: "Dev".into(),
        });
        s
    }

    #[test]
    fn remove_leaves_no_trace() {
        let mut s = fixture();
        remove(&mut s, &k("old"));
        assert!(!to_json(&s).contains("\"old\""));
        assert!(s.paused_reason.is_none());
    }

    #[test]
    fn remove_keeps_other_keys() {
        let mut s = fixture();
        s.output.order.push(k("x"));
        s.devices.insert(k("x"), rec("X"));
        s.paused_reason.as_mut().unwrap().key = k("x");
        remove(&mut s, &k("old"));
        assert_eq!(s.output.order, [k("x")]);
        assert!(s.devices.contains_key(&k("x")));
        assert!(s.paused_reason.is_some());
    }

    #[test]
    fn rename_leaves_no_trace_of_the_old_key() {
        let mut s = fixture();
        rename(&mut s, &k("old"), &k("new"));
        assert!(!to_json(&s).contains("\"old\""));
        assert_eq!(s.output.order, [k("new")]);
        assert_eq!(s.input.order, [k("new")]);
        assert_eq!(s.paused_reason.unwrap().key, k("new"));
        assert!(s.devices.contains_key(&k("new")));
    }

    #[test]
    fn rename_keeps_the_better_rank() {
        let mut s = fixture();
        s.output.order = vec![k("old"), k("x"), k("new")];
        rename(&mut s, &k("old"), &k("new"));
        assert_eq!(s.output.order, [k("new"), k("x")]);
        s.input.order = vec![k("new"), k("x"), k("old")];
        rename(&mut s, &k("old"), &k("new"));
        assert_eq!(s.input.order, [k("new"), k("x")]);
    }

    #[test]
    fn rename_unions_never_use() {
        let mut s = fixture();
        s.input.never_use.clear();
        s.input.never_use.insert(k("new"));
        s.output.never_use.insert(k("new"));
        s.output.never_use.remove(&k("old"));
        rename(&mut s, &k("old"), &k("new"));
        assert!(s.input.never_use.contains(&k("new")));
        assert_eq!(s.input.never_use.len(), 1);
        assert!(s.output.never_use.contains(&k("new")));
        let mut s = fixture();
        rename(&mut s, &k("old"), &k("new"));
        assert!(s.output.never_use.contains(&k("new")));
    }

    #[test]
    fn rename_keeps_existing_target_record() {
        let mut s = fixture();
        s.devices.insert(k("new"), rec("Target"));
        rename(&mut s, &k("old"), &k("new"));
        assert_eq!(s.devices.len(), 1);
        assert_eq!(s.devices[&k("new")].name, "Target");
        let mut s = fixture();
        rename(&mut s, &k("old"), &k("new"));
        assert_eq!(s.devices[&k("new")].name, "Dev");
    }

    #[test]
    fn rename_to_self_is_a_no_op() {
        let mut s = fixture();
        let before = s.clone();
        rename(&mut s, &k("old"), &k("old"));
        assert_eq!(s, before);
    }
}
