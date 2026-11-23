//! Per-role sliding-window loop guard: stops auto-switch fighting a device that keeps retaking a role.
use crate::{PerRole, Role, Timestamp};

pub const GUARD_WRITES: usize = 3;
pub const GUARD_WINDOW_MS: u64 = 10_000;

#[derive(Default, Debug)]
pub struct LoopGuard {
    writes: PerRole<Vec<Timestamp>>,
}

impl LoopGuard {
    /// Records a corrective write; returns true when the guard trips.
    /// Keeps returning true after a trip until `reset`/`reset_role`.
    pub fn record(&mut self, role: Role, now: Timestamp) -> bool {
        let w = self.writes.get_mut(role);
        w.retain(|&t| now.since(t) < GUARD_WINDOW_MS);
        w.push(now);
        w.len() >= GUARD_WRITES
    }

    pub fn reset_role(&mut self, role: Role) {
        self.writes.get_mut(role).clear();
    }

    pub fn reset(&mut self) {
        self.writes = PerRole::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(ms: u64) -> Timestamp {
        Timestamp(ms)
    }

    #[test]
    fn third_write_inside_window_trips() {
        let mut g = LoopGuard::default();
        assert!(!g.record(Role::Output, t(0)));
        assert!(!g.record(Role::Output, t(4_000)));
        assert!(g.record(Role::Output, t(9_999)));
    }

    #[test]
    fn window_slides() {
        let mut g = LoopGuard::default();
        assert!(!g.record(Role::Output, t(0)));
        assert!(!g.record(Role::Output, t(5_000)));
        assert!(!g.record(Role::Output, t(10_000)));
        assert!(g.record(Role::Output, t(14_999)));
    }

    #[test]
    fn roles_count_separately() {
        let mut g = LoopGuard::default();
        assert!(!g.record(Role::Output, t(0)));
        assert!(!g.record(Role::Input, t(1)));
        assert!(!g.record(Role::Output, t(2)));
        assert!(!g.record(Role::Input, t(3)));
    }

    #[test]
    fn reset_clears_counts() {
        let mut g = LoopGuard::default();
        g.record(Role::Output, t(0));
        g.record(Role::Output, t(1));
        g.reset();
        assert!(!g.record(Role::Output, t(2)));
    }

    #[test]
    fn clock_step_back_does_not_panic() {
        let mut g = LoopGuard::default();
        g.record(Role::Output, t(10_000));
        g.record(Role::Output, t(0));
    }
}
