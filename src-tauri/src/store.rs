//! state.json on disk: load (corrupt file moved aside, newer schema left alone), atomic temp-file + rename saves,
//! 500 ms debounce fixed from the first change.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use log::{debug, error, warn};
use usher_core::{State, StateError, from_json, to_json};

pub const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);
pub const SAVE_RETRY: Duration = Duration::from_secs(5);

pub struct Store {
    path: PathBuf,
    writable: bool,
    dirty: bool,
    due: Option<Instant>,
    saves: u32,
}

impl Store {
    /// `None` state: nothing usable on disk, seed from the first scan.
    pub fn open(path: PathBuf) -> (Store, Option<State>) {
        let mut s = Store {
            path,
            writable: true,
            dirty: false,
            due: None,
            saves: 0,
        };
        let state = match fs::read(&s.path) {
            Ok(bytes) => match String::from_utf8(bytes)
                .map_err(|e| StateError::Corrupt(e.to_string()))
                .and_then(|text| from_json(&text))
            {
                Ok(st) => Some(st),
                Err(StateError::Corrupt(e)) => {
                    s.move_aside(&e);
                    None
                }
                Err(StateError::UnsupportedVersion(v)) => {
                    error!(
                        "state.json has schema {v}, newer than this usher; running without saving"
                    );
                    s.writable = false;
                    None
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                debug!("no state.json yet");
                None
            }
            Err(e) => {
                error!("state.json unreadable ({e}); running without saving");
                s.writable = false;
                None
            }
        };
        (s, state)
    }

    fn move_aside(&mut self, why: &str) {
        match fs::rename(&self.path, self.path.with_extension("json.corrupt")) {
            Ok(()) => warn!("state.json is corrupt ({why}); moved aside to state.json.corrupt"),
            Err(e) => {
                error!("could not move state.json aside ({e}); running without saving");
                self.writable = false;
            }
        }
    }

    pub fn writable(&self) -> bool {
        self.writable
    }

    pub fn due(&self) -> Option<Instant> {
        self.due
    }

    pub fn saves(&self) -> u32 {
        self.saves
    }

    pub fn mark_dirty(&mut self, now: Instant) {
        if !self.writable {
            return;
        }
        self.dirty = true;
        self.due.get_or_insert(now + SAVE_DEBOUNCE);
    }

    pub fn save_if_due(&mut self, state: &State, now: Instant) {
        if self.due.is_some_and(|d| d <= now) {
            self.save_now(state, now);
        }
    }

    /// A failure keeps dirty and retries after `SAVE_RETRY`.
    pub fn save_now(&mut self, state: &State, now: Instant) {
        if !self.write(state) {
            self.due = Some(now + SAVE_RETRY);
        }
    }

    /// Best-effort final save; failure is logged and stays dirty.
    pub fn flush(&mut self, state: &State) {
        if self.dirty {
            self.write(state);
        }
    }

    fn write(&mut self, state: &State) -> bool {
        if !self.writable {
            return true;
        }
        match self.try_write(state) {
            Ok(()) => {
                self.dirty = false;
                self.due = None;
                self.saves += 1;
                debug!("state saved");
                true
            }
            Err(e) => {
                error!("state.json save failed ({e})");
                self.dirty = true;
                false
            }
        }
    }

    fn try_write(&self, state: &State) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let res = (|| {
            let mut f = File::create(&tmp)?;
            f.write_all(to_json(state).as_bytes())?;
            f.sync_all()?;
            fs::rename(&tmp, &self.path)
        })();
        if res.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::fs;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;
    use usher_core::State;

    fn tempdir() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    fn at(ms: u64) -> Instant {
        static BASE: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        *BASE.get_or_init(Instant::now) + Duration::from_millis(ms)
    }

    #[test]
    fn missing_file_needs_a_seed() {
        let d = tempdir();
        let (s, st) = Store::open(d.path().join("state.json"));
        assert!(st.is_none());
        assert!(s.writable());
    }

    #[test]
    fn round_trip_keeps_unknown_keys() {
        let d = tempdir();
        let p = d.path().join("state.json");
        fs::write(&p, r#"{"schema_version":1,"future_key":{"x":1}}"#).unwrap();
        let (mut s, st) = Store::open(p.clone());
        s.save_now(&st.unwrap(), at(0));
        let reread: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(reread["future_key"], json!({"x":1}));
        assert!(!d.path().join("state.json.tmp").exists());
        assert_eq!(s.saves(), 1);
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let d = tempdir();
        let p = d.path().join("state.json");
        fs::write(&p, b"{not json").unwrap();
        fs::write(d.path().join("state.json.corrupt"), b"old").unwrap();
        let (s, st) = Store::open(p);
        assert!(st.is_none());
        assert!(s.writable());
        assert_eq!(
            fs::read(d.path().join("state.json.corrupt")).unwrap(),
            b"{not json"
        );
        assert!(!d.path().join("state.json").exists());
    }

    #[test]
    fn non_utf8_file_is_moved_aside() {
        let d = tempdir();
        let p = d.path().join("state.json");
        fs::write(&p, [0xff, 0xfe, 0x00]).unwrap();
        let (s, st) = Store::open(p);
        assert!(st.is_none());
        assert!(s.writable());
        assert_eq!(
            fs::read(d.path().join("state.json.corrupt")).unwrap(),
            [0xff, 0xfe, 0x00]
        );
        assert!(!d.path().join("state.json").exists());
    }

    #[test]
    fn newer_schema_is_left_alone_and_read_only() {
        let d = tempdir();
        let p = d.path().join("state.json");
        fs::write(&p, r#"{"schema_version":2}"#).unwrap();
        let (mut s, st) = Store::open(p.clone());
        assert!(st.is_none());
        assert!(!s.writable());
        s.save_now(&State::default(), at(0));
        assert_eq!(fs::read_to_string(&p).unwrap(), r#"{"schema_version":2}"#);
        assert_eq!(s.saves(), 0);
    }

    #[test]
    fn unreadable_file_is_left_alone_and_read_only() {
        let d = tempdir();
        fs::create_dir(d.path().join("state.json")).unwrap();
        let (s, st) = Store::open(d.path().join("state.json"));
        assert!(st.is_none());
        assert!(!s.writable());
        assert!(!d.path().join("state.json.corrupt").exists());
    }

    #[test]
    fn debounce_is_fixed_from_the_first_change() {
        let d = tempdir();
        let (mut s, _) = Store::open(d.path().join("state.json"));
        let st = State::default();
        s.mark_dirty(at(0));
        s.mark_dirty(at(400));
        assert_eq!(s.due(), Some(at(500)));
        s.save_if_due(&st, at(499));
        assert_eq!(s.saves(), 0);
        s.save_if_due(&st, at(500));
        assert_eq!(s.saves(), 1);
        assert_eq!(s.due(), None);
    }

    #[test]
    fn failed_save_keeps_dirty_and_retries_after_5s() {
        let d = tempdir();
        let sub = d.path().join("sub");
        let (mut s, _) = Store::open(sub.join("state.json"));
        assert!(s.writable());
        fs::write(&sub, b"x").unwrap(); // parent becomes a regular file: create_dir_all fails
        let st = State::default();
        s.mark_dirty(at(0));
        s.save_if_due(&st, at(500));
        assert_eq!(s.saves(), 0);
        assert_eq!(s.due(), Some(at(5_500)));
        s.flush(&st); // still dirty: flush retries, fails, stays dirty
        assert_eq!(s.saves(), 0);
        assert!(s.due().is_some());
    }

    #[test]
    fn failed_save_now_schedules_a_retry() {
        let d = tempdir();
        let sub = d.path().join("sub");
        let (mut s, _) = Store::open(sub.join("state.json"));
        fs::write(&sub, b"x").unwrap();
        s.save_now(&State::default(), at(100));
        assert_eq!(s.saves(), 0);
        assert_eq!(s.due(), Some(at(5_100)));
    }

    #[test]
    fn flush_writes_only_when_dirty() {
        let d = tempdir();
        let (mut s, _) = Store::open(d.path().join("state.json"));
        let st = State::default();
        s.flush(&st);
        assert_eq!(s.saves(), 0);
        s.mark_dirty(at(0));
        s.flush(&st);
        assert_eq!(s.saves(), 1);
        assert_eq!(s.due(), None);
    }
}
