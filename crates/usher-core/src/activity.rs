//! What usher did and why: pure entries built from one reconcile's outcome (ADR 0003: the cause
//! comes from the snapshot diff, not from event order).
use crate::{DeviceKey, PausedReason, PerRole, Reason, Role, Snapshot, Timestamp};

/// What woke the reconcile. Precedence in [`Trigger::merge`]: Command > Rebind > Launch >
/// DefaultChanged > DevicesChanged.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trigger {
    Launch,
    DevicesChanged,
    DefaultChanged(Role),
    Command,
    Rebind,
}

impl Trigger {
    fn rank(self) -> u8 {
        match self {
            Trigger::DevicesChanged => 0,
            Trigger::DefaultChanged(_) => 1,
            Trigger::Launch => 2,
            Trigger::Rebind => 3,
            Trigger::Command => 4,
        }
    }

    /// The stronger trigger; on a tie the first one stays.
    pub fn merge(self, other: Trigger) -> Trigger {
        if other.rank() > self.rank() {
            other
        } else {
            self
        }
    }

    /// Only a device event may raise a notification.
    pub fn notifies(self) -> bool {
        matches!(self, Trigger::DevicesChanged | Trigger::DefaultChanged(_))
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Why {
    Connected(String),
    Disconnected(String),
    Corrected(String),
    Ranked,
    Launch,
    Rebind,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Entry {
    Switched {
        role: Role,
        from: Option<String>,
        to: String,
        why: Why,
    },
    Failed {
        role: Role,
        to: String,
        error: String,
    },
    Paused {
        reason: PausedReason,
    },
    Resumed,
}

fn title(role: Role) -> &'static str {
    match role {
        Role::Output => "Output",
        Role::Input => "Input",
    }
}

impl Entry {
    pub fn text(&self) -> String {
        match self {
            Entry::Switched {
                role,
                from: Some(from),
                to,
                ..
            } => {
                format!("{}: {from} → {to}", title(*role))
            }
            Entry::Switched { role, to, .. } => format!("{}: {to}", title(*role)),
            Entry::Failed { role, to, error } => {
                format!("Couldn't switch {role} to {to}: {error}")
            }
            Entry::Paused { reason } => reason.message(),
            Entry::Resumed => "Auto-switch on".into(),
        }
    }

    pub fn why_text(&self) -> Option<String> {
        let Entry::Switched { role, why, .. } = self else {
            return None;
        };
        Some(match why {
            Why::Connected(n) => format!("{n} connected"),
            Why::Disconnected(n) => format!("{n} disconnected"),
            Why::Corrected(n) => format!("{n} took {role}; usher put it back"),
            Why::Ranked => "changed in usher".into(),
            Why::Launch => "usher started".into(),
            Why::Rebind => "usher reconnected to the audio system".into(),
        })
    }

    pub fn role(&self) -> Option<Role> {
        match self {
            Entry::Switched { role, .. } | Entry::Failed { role, .. } => Some(*role),
            Entry::Paused { .. } | Entry::Resumed => None,
        }
    }
}

/// The engine's per-role outcome. (`Outcome` is taken by `reconcile`.)
#[derive(Clone, PartialEq, Debug, Default)]
pub enum Wrote {
    #[default]
    Nothing,
    Set,
    Failed(String),
    Paused(PausedReason),
}

/// Snapshot name, else previous snapshot name, else the key itself.
fn name_of(k: &DeviceKey, prev: Option<&Snapshot>, snap: &Snapshot) -> String {
    let found = snap.device(k).or_else(|| prev.and_then(|p| p.device(k)));
    found.map_or_else(|| k.to_string(), |d| d.name.clone())
}

fn why_of(trigger: Trigger, prev: Option<&Snapshot>, snap: &Snapshot, r: &Reason) -> Why {
    let name = |k| name_of(k, prev, snap);
    let Some(prev) = prev.filter(|_| !matches!(trigger, Trigger::Launch | Trigger::Rebind)) else {
        return if trigger == Trigger::Rebind {
            Why::Rebind
        } else {
            Why::Launch
        };
    };
    let prev_cur = prev.current.get(r.role).as_ref();
    if let Some(pc) = prev_cur.filter(|pc| !snap.is_connected(pc)) {
        return Why::Disconnected(name(pc));
    }
    let desired = r.desired.as_ref().expect("a write has a desired device");
    if !prev.is_connected(desired) {
        return Why::Connected(name(desired));
    }
    if trigger == Trigger::Command {
        return Why::Ranked;
    }
    match (snap.current.get(r.role), prev_cur) {
        (Some(taker), _) => Why::Corrected(name(taker)),
        (None, Some(pc)) => Why::Disconnected(name(pc)),
        (None, None) => Why::Connected(name(desired)),
    }
}

/// The entry for one role's write, or `None` when usher wrote nothing.
pub fn explain(
    trigger: Trigger,
    prev: Option<&Snapshot>,
    snap: &Snapshot,
    reason: &Reason,
    wrote: &Wrote,
) -> Option<Entry> {
    let role = reason.role;
    let to = || {
        name_of(
            reason
                .desired
                .as_ref()
                .expect("a write has a desired device"),
            prev,
            snap,
        )
    };
    Some(match wrote {
        Wrote::Nothing => return None,
        Wrote::Paused(p) => Entry::Paused { reason: p.clone() },
        Wrote::Failed(error) => Entry::Failed {
            role,
            to: to(),
            error: error.clone(),
        },
        Wrote::Set => Entry::Switched {
            role,
            from: snap
                .current
                .get(role)
                .as_ref()
                .map(|k| name_of(k, prev, snap)),
            to: to(),
            why: why_of(trigger, prev, snap, reason),
        },
    })
}

pub const NOTICE_GAP_MS: u64 = 10_000;

/// Caps notices at one line per role per [`NOTICE_GAP_MS`]; a capped line stays in history.
#[derive(Default, Debug)]
pub struct NoticeGate {
    last: PerRole<Option<Timestamp>>,
}

impl NoticeGate {
    pub fn body(&mut self, trigger: Trigger, entries: &[Entry], now: Timestamp) -> Option<String> {
        if !trigger.notifies() {
            return None;
        }
        let mut lines = Vec::new();
        for e in entries {
            let eligible = match e {
                Entry::Failed { .. } => true,
                Entry::Switched { why, .. } => {
                    matches!(
                        why,
                        Why::Connected(_) | Why::Disconnected(_) | Why::Corrected(_)
                    )
                }
                _ => false,
            };
            let Some(role) = e.role().filter(|_| eligible) else {
                continue;
            };
            let last = self.last.get_mut(role);
            if last.is_some_and(|t| now.since(t) < NOTICE_GAP_MS) {
                continue;
            }
            *last = Some(now);
            lines.push(e.text());
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }
}
