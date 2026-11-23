use std::path::Path;

/// Takes the exclusive lock on `dir/instance.lock` for the rest of the process (the OS drops it at exit or crash, so no
/// stale lock is left). False only when another process holds it; a file system that cannot lock lets usher run.
///
/// Why not the single-instance plugin: its socket in shared /tmp could be squatted to block launch. A Finder, Dock or
/// Spotlight launch already reaches the running usher (LaunchServices keeps one instance), but the login item execs
/// the binary outside LaunchServices: this lock makes that copy exit.
pub fn hold_instance_lock(dir: &Path) -> bool {
    std::fs::create_dir_all(dir).ok();
    let Ok(f) = std::fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("instance.lock"))
    else {
        return true;
    };
    match f.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return false,
        Err(std::fs::TryLockError::Error(e)) => log::warn!("instance lock not taken ({e})"),
    }
    std::mem::forget(f); // held until the process ends
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_holder_of_the_instance_lock_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(hold_instance_lock(tmp.path()));
        assert!(!hold_instance_lock(tmp.path()));
    }

    #[test]
    fn a_missing_dir_is_created() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(hold_instance_lock(&tmp.path().join("a/b")));
    }
}
