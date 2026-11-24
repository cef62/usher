//! The activity history the engine feeds: the newest [`HISTORY`] lines, newest first.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde::Serialize;
use usher_core::Entry;

pub const HISTORY: usize = 200;
/// The Activity window's label; lines are emitted to it only.
pub const LABEL: &str = "activity";

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub seq: u64,
    pub at: u64,
    pub text: String,
    pub why: Option<String>,
}

#[derive(Default)]
pub struct Log {
    next: u64,
    lines: VecDeque<Line>,
}

impl Log {
    pub fn push(&mut self, at: u64, entry: &Entry) -> Line {
        self.next += 1;
        let line = Line {
            seq: self.next,
            at,
            text: entry.text(),
            why: entry.why_text(),
        };
        self.lines.push_front(line.clone());
        self.lines.truncate(HISTORY);
        line
    }

    #[allow(dead_code)] // read by `get_activity` (M6 Task 6)
    pub fn lines(&self) -> Vec<Line> {
        self.lines.iter().cloned().collect()
    }
}

#[derive(Default)]
pub struct ActivityLog(pub Mutex<Log>);

#[cfg(test)]
mod tests {
    use super::*;
    use usher_core::{Role, Why};

    #[test]
    fn history_keeps_the_newest_200_newest_first() {
        let mut log = Log::default();
        for i in 0..201 {
            log.push(i, &Entry::Resumed);
        }
        let lines = log.lines();
        assert_eq!(lines.len(), 200);
        assert_eq!((lines[0].seq, lines[0].at), (201, 200));
        assert_eq!(lines[199].seq, 2);
        assert_eq!(lines[0].text, "Auto-switch on");
    }

    #[test]
    fn a_line_carries_the_why_text() {
        let e = Entry::Switched {
            role: Role::Output,
            from: None,
            to: "Studio Display".into(),
            why: Why::Connected("AirPods".into()),
        };
        let line = Log::default().push(5, &e);
        assert_eq!(line.why.as_deref(), Some("AirPods connected"));
    }
}
