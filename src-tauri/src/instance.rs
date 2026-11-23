use std::path::Path;
use std::time::Duration;

/// Takes the exclusive lock on `dir/instance.lock` for the rest of the process (the OS drops it at exit or crash, so no
/// stale lock is left). Retried for `wait`: the relaunch after an update can start before the old process has ended.
/// False only when another process holds it; a file system that cannot lock lets usher run.
///
/// Why not the single-instance plugin: its socket in shared /tmp could be squatted to block launch. A Finder, Dock or
/// Spotlight launch already reaches the running usher (LaunchServices keeps one instance), but the login item execs
/// the binary outside LaunchServices: this lock makes that copy exit.
pub fn hold_instance_lock(dir: &Path, wait: Duration) -> bool {
    std::fs::create_dir_all(dir).ok();
    let Ok(f) = std::fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("instance.lock"))
    else {
        return true;
    };
    let until = std::time::Instant::now() + wait;
    loop {
        match f.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < until => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(std::fs::TryLockError::WouldBlock) => return false,
            Err(std::fs::TryLockError::Error(e)) => {
                log::warn!("instance lock not taken ({e})");
                break;
            }
        }
    }
    std::mem::forget(f); // held until the process ends
    true
}

/// How long a start waits for another usher's instance lock: only the relaunch after an update (a fresh restart mark),
/// whose old process is still exiting. Any other start that loses exits at once.
pub fn lock_wait(fresh_mark: bool) -> Duration {
    if fresh_mark {
        Duration::from_secs(5)
    } else {
        Duration::ZERO
    }
}

const RESTART_MARK: &str = "restart-update";
const RESTART_MARK_TTL: Duration = Duration::from_secs(600);

/// Written when an update is downloaded and about to install; the relaunch reads it (`lock_wait`) and consumes it.
// first caller is the updater (M5 task 3)
#[allow(dead_code)]
pub fn mark_restart(dir: &Path) {
    std::fs::write(dir.join(RESTART_MARK), b"").ok();
}

/// A failed install: the next start is not a relaunch.
pub fn clear_restart_mark(dir: &Path) {
    std::fs::remove_file(dir.join(RESTART_MARK)).ok();
}

/// Whether a mark younger than ten minutes exists; reads only.
pub fn restart_mark_fresh(dir: &Path) -> bool {
    std::fs::metadata(dir.join(RESTART_MARK))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < RESTART_MARK_TTL)
}

/// True once for a mark younger than ten minutes; an older one (an install that never relaunched) is only deleted, so a
/// later login start never waits for the lock.
pub fn take_restart_mark(dir: &Path) -> bool {
    let fresh = restart_mark_fresh(dir);
    clear_restart_mark(dir);
    fresh
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn locked(dir: &Path) -> std::fs::File {
        let f = std::fs::File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join("instance.lock"))
            .unwrap();
        f.lock().unwrap();
        f
    }

    #[test]
    fn a_second_holder_of_the_instance_lock_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(hold_instance_lock(tmp.path(), Duration::ZERO));
        assert!(!hold_instance_lock(tmp.path(), Duration::ZERO));
    }

    #[test]
    fn a_missing_dir_is_created() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(hold_instance_lock(&tmp.path().join("a/b"), Duration::ZERO));
    }

    #[test]
    fn only_a_fresh_restart_mark_waits_and_it_is_consumed() {
        let d = tempfile::tempdir().unwrap();
        assert!(!take_restart_mark(d.path()));
        assert_eq!(lock_wait(restart_mark_fresh(d.path())), Duration::ZERO);
        mark_restart(d.path());
        assert_eq!(
            lock_wait(restart_mark_fresh(d.path())),
            Duration::from_secs(5)
        );
        assert!(take_restart_mark(d.path()));
        assert!(!take_restart_mark(d.path()), "consumed");
        mark_restart(d.path());
        clear_restart_mark(d.path());
        assert!(!take_restart_mark(d.path()), "a failed install clears it");
        mark_restart(d.path());
        let old = std::time::SystemTime::now() - RESTART_MARK_TTL - Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(d.path().join(RESTART_MARK))
            .unwrap()
            .set_modified(old)
            .unwrap();
        assert!(
            !restart_mark_fresh(d.path()),
            "a stale mark must not make a login start wait"
        );
        assert!(!take_restart_mark(d.path()));
        assert!(!d.path().join(RESTART_MARK).exists());
    }

    #[test]
    fn a_held_lock_is_taken_once_released_within_the_wait() {
        let d = tempfile::tempdir().unwrap();
        let f = locked(d.path());
        let t = Instant::now();
        let h = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            drop(f);
        });
        assert!(hold_instance_lock(d.path(), Duration::from_secs(2)));
        assert!(t.elapsed() >= Duration::from_millis(200));
        h.join().unwrap();
    }

    #[test]
    fn without_a_wait_a_held_lock_is_refused_at_once() {
        let d = tempfile::tempdir().unwrap();
        let _f = locked(d.path());
        let t = Instant::now();
        assert!(!hold_instance_lock(d.path(), Duration::ZERO));
        assert!(t.elapsed() < Duration::from_millis(50));
    }
}
