//! In-app updates: a check 30 s after launch, then daily while the daily check is on; a manual check
//! from Settings and the menu-bar icon; install on request from the What's new window. Delivery goes through
//! tauri-plugin-updater, which verifies the package against the minisign public key in `tauri.conf.json` (with
//! `requireSignedVersion`). Nothing pops up by itself: a found update shows in the panel view and as the menu item.

pub mod install;

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::engine::PanelView;

/// Let startup settle before touching GitHub.
pub const FIRST_CHECK_DELAY: Duration = Duration::from_secs(30);
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 3600);
/// How often the checker thread wakes to re-read the clock and the setting: turning the daily check back on takes
/// effect within this. One wake per ten minutes keeps idle CPU at zero.
const WAKE_POLL: Duration = Duration::from_secs(600);
const _: () = assert!(FIRST_CHECK_DELAY.as_secs() < CHECK_INTERVAL.as_secs());

/// Idle time between bytes before a check or download is abandoned. Without it a half-open socket leaves `busy` set
/// until usher restarts.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

pub const DEBUG_BUILD: &str = "Updates are off in development builds.";
pub const BUSY: &str = "An update check or install is already running.";
pub const NO_UPDATE: &str = "No update to install.";
pub const NEWER: &str = "usher found a newer version since this window opened.";

/// What the last check found. The plugin's `Update` handle is kept so "Install" needs no second round-trip to the feed.
#[derive(Default)]
pub struct UpdateState {
    pub available: Option<Update>,
    /// A check or an install is running; a second request is refused.
    pub busy: bool,
    /// The install as the What's new window shows it, and that window's channel.
    pub phase: Phase,
    pub watcher: Option<Channel<Phase>>,
}

/// Where an install stands, for the What's new window. After `Failed` the update is kept: the window offers a retry.
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Idle,
    Downloading {
        downloaded: u64,
        total: Option<u64>,
    },
    Installing,
    Failed {
        message: String,
    },
}

/// Records `phase` and sends it to the What's new window, if one listens. Its channel reaches only that webview.
pub(crate) fn set_phase(st: &mut UpdateState, phase: Phase) {
    if let Some(ch) = &st.watcher
        && ch.send(phase.clone()).is_err()
    {
        st.watcher = None;
    }
    st.phase = phase;
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct UpdateInfo {
    pub version: String,
    /// The running version, as the updater compared it.
    pub current_version: String,
    /// The release notes from the feed (`latest.json` `notes`), empty if none. Untrusted text: the What's new window
    /// renders it without HTML.
    pub notes: String,
}

impl From<&Update> for UpdateInfo {
    fn from(u: &Update) -> Self {
        Self {
            version: u.version.clone(),
            current_version: u.current_version.clone(),
            notes: u.body.clone().unwrap_or_default(),
        }
    }
}

/// Which `n`th of the download `done` is in (`n` = 100: percent), or 256 KiB steps when the size is unknown. Progress is
/// sent when the bucket changes, not on every chunk.
fn bucket(done: u64, total: Option<u64>, n: u64) -> u64 {
    match total {
        Some(t) if t > 0 => done.min(t) * n / t,
        _ => done >> 18,
    }
}

/// Time since the last check: by the wall clock (None when the clock was set back past it) and awake (monotonic, which
/// stops while the Mac sleeps).
#[derive(Clone, Copy)]
pub struct Since {
    pub wall: Option<Duration>,
    pub awake: Duration,
}

/// Whether the checker runs a check at this wake. `last`: None until the first check (the thread has already slept
/// `FIRST_CHECK_DELAY`). The wall clock makes a laptop that sleeps most of the day still check daily; the awake bound
/// ends the wait if the clock was set back.
pub fn due(enabled: bool, last: Option<Since>) -> bool {
    enabled
        && last.is_none_or(|s| {
            s.wall.is_some_and(|w| w >= CHECK_INTERVAL) || s.awake >= CHECK_INTERVAL
        })
}

/// The daily check follows the engine's last view. Before the first view it is on: an update may be the fix.
pub fn daily_check_on(view: Option<&PanelView>) -> bool {
    view.is_none_or(|v| v.panel.update_check)
}

pub(crate) fn state(app: &AppHandle) -> MutexGuard<'_, UpdateState> {
    app.state::<Mutex<UpdateState>>()
        .inner()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// The version waiting to install, if any: the panel view and the menu-bar item show it.
pub fn pending(app: &AppHandle) -> Option<String> {
    app.try_state::<Mutex<UpdateState>>()?
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .available
        .as_ref()
        .map(|u| u.version.clone())
}

/// Claims the busy flag; `false` means another check or install is already running.
fn begin(st: &mut UpdateState) -> bool {
    !std::mem::replace(&mut st.busy, true)
}

pub fn setup(app: &AppHandle) {
    app.manage(Mutex::new(UpdateState::default()));
    spawn_checker(app.clone());
}

fn daily_on(app: &AppHandle) -> bool {
    let Some(last) = app.try_state::<crate::shell::LastView>() else {
        return daily_check_on(None);
    };
    let view = last.0.lock().unwrap_or_else(|p| p.into_inner());
    daily_check_on(view.as_ref())
}

/// Release builds only, so `tauri dev` never hits the feed or replaces its own `target/debug` binary.
fn spawn_checker(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK_DELAY);
        let mut last: Option<(SystemTime, Instant)> = None;
        loop {
            let since = last.map(|(w, a)| Since {
                wall: w.elapsed().ok(),
                awake: a.elapsed(),
            });
            if due(daily_on(&app), since) {
                // a background failure is never shown (offline, GitHub down, a manual check running): one log line,
                // and the next wake tries again instead of waiting a day
                if tauri::async_runtime::block_on(check(&app)).is_ok() {
                    last = Some((SystemTime::now(), Instant::now())); // a failure was logged by check()
                }
            }
            std::thread::sleep(WAKE_POLL);
        }
    });
}

/// Asks the feed for a newer version. `None`: up to date. A second check while one runs is refused with `BUSY`.
pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    if cfg!(debug_assertions) {
        return Err(DEBUG_BUILD.into());
    }
    if !begin(&mut state(app)) {
        return Err(BUSY.into());
    }
    // the lock is released here; the network call runs without it
    let result = async {
        app.updater_builder()
            .configure_client(|c| c.read_timeout(READ_TIMEOUT))
            .build()?
            .check()
            .await
    }
    .await;
    let mut st = state(app);
    st.busy = false;
    let update = result.map_err(|e| {
        log::warn!("update check failed: {e}");
        e.to_string()
    })?;
    match &update {
        Some(u) => log::info!("update check: usher {} available", u.version),
        None => log::info!("update check: up to date"),
    }
    let info = update.as_ref().map(UpdateInfo::from);
    if info.is_some() {
        set_phase(&mut st, Phase::Idle); // a fresh update: an earlier failure is over
    }
    st.available = update;
    drop(st);
    crate::shell::republish(app);
    install::refresh_tray(app);
    Ok(info)
}

/// Settings › Check now. The answer shows inline in Settings.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    check(&app).await
}

#[cfg(test)]
mod tests {
    use super::install::{stale, tray_text};
    use super::*;
    use usher_core::PerRole;

    fn view_fixture() -> PanelView {
        PanelView {
            panel: usher_core::panel_state(
                &usher_core::State::default(),
                &usher_core::Snapshot {
                    devices: vec![],
                    current: PerRole::default(),
                },
            ),
            errors: PerRole::default(),
            read_only: false,
            update: None,
        }
    }

    #[test]
    fn checks_first_then_daily_and_only_while_on() {
        let h = |n: u64| Duration::from_secs(n * 3600);
        assert!(due(true, None), "the first check, after the startup delay");
        assert!(!due(false, None), "the daily check is off");
        let since = |wall: Option<Duration>, awake| Some(Since { wall, awake });
        assert!(!due(true, since(Some(h(23)), h(23))));
        assert!(
            due(true, since(Some(h(24)), h(2))),
            "a laptop asleep most of the day still checks daily"
        );
        assert!(
            due(true, since(None, h(24))),
            "a clock set back: the awake time ends the wait"
        );
        assert!(!due(true, since(None, h(1))));
        assert!(
            !due(false, since(Some(h(48)), h(48))),
            "turned off: no check however long"
        );
    }

    #[test]
    fn daily_check_follows_the_last_view_and_defaults_on() {
        assert!(
            daily_check_on(None),
            "no view yet: an update may be the fix"
        );
        let mut v = view_fixture();
        assert!(daily_check_on(Some(&v)));
        v.panel.update_check = false;
        assert!(!daily_check_on(Some(&v)));
    }

    #[test]
    fn a_second_check_or_install_is_refused_while_one_runs() {
        let mut st = UpdateState::default();
        assert!(begin(&mut st));
        assert!(!begin(&mut st));
        st.busy = false;
        assert!(begin(&mut st));
    }

    #[test]
    fn the_whatsnew_window_knows_the_stale_install_errors() {
        let api = include_str!("../../../src/whatsnew/api.ts");
        assert!(api.contains(&format!("\"{NO_UPDATE}\"")) && api.contains(&format!("\"{NEWER}\"")));
    }

    #[test]
    fn install_refuses_a_stale_or_missing_update() {
        assert_eq!(stale(None, "0.2.0"), Some(NO_UPDATE));
        assert_eq!(stale(Some("0.3.0"), "0.2.0"), Some(NEWER));
        assert_eq!(stale(Some("0.2.0"), "0.2.0"), None);
    }

    /// The shape the What's new window reads.
    #[test]
    fn phase_wire_format() {
        let j = |p: Phase| serde_json::to_value(p).unwrap();
        assert_eq!(j(Phase::Idle), serde_json::json!({ "kind": "idle" }));
        assert_eq!(
            j(Phase::Downloading {
                downloaded: 5,
                total: None
            }),
            serde_json::json!({ "kind": "downloading", "downloaded": 5, "total": null })
        );
        assert_eq!(
            j(Phase::Installing),
            serde_json::json!({ "kind": "installing" })
        );
        assert_eq!(
            j(Phase::Failed {
                message: "x".into()
            }),
            serde_json::json!({ "kind": "failed", "message": "x" })
        );
    }

    #[test]
    fn progress_buckets() {
        assert_eq!(bucket(0, Some(1000), 100), 0);
        assert_eq!(bucket(499, Some(1000), 4), 1);
        assert_eq!(bucket(1000, Some(1000), 4), 4);
        assert_eq!(bucket(2000, Some(1000), 100), 100); // a server that lied about the size
        assert_eq!(bucket(300 * 1024, None, 100), 1); // unknown size: 256 KiB steps
        assert_eq!(bucket(10, Some(0), 100), 0);
    }

    #[test]
    fn tray_item_names_the_pending_version() {
        assert_eq!(tray_text(None), "Check for Updates…");
        assert_eq!(tray_text(Some("0.2.0")), "Update to usher 0.2.0…");
    }

    #[test]
    fn install_restarts_through_the_exit_path() {
        // request_restart runs RunEvent::Exit, which flushes state.json; restart() from the main thread would skip it
        let src = include_str!("install.rs");
        assert!(src.contains("request_restart()") && !src.contains(".restart();"));
    }

    /// The feed and key the installed app trusts: a release must not ship with a local test feed or plain http.
    #[test]
    fn updater_config_is_the_signed_github_feed() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        let u = &conf["plugins"]["updater"];
        assert_eq!(
            u["endpoints"],
            serde_json::json!([
                "https://github.com/cef62/usher/releases/latest/download/latest.json"
            ])
        );
        assert_eq!(u["requireSignedVersion"], true);
        assert!(u["pubkey"].as_str().is_some_and(|k| !k.is_empty()));
        assert!(u.get("dangerousInsecureTransportProtocol").is_none());
        assert_eq!(conf["bundle"]["createUpdaterArtifacts"], true);
    }
}
