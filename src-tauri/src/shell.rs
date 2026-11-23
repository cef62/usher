//! The engine's window onto the app: panel events, tray state, loop-guard notification.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use log::{error, warn};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::{NotificationExt, PermissionState};
use usher_core::{PausedReason, Timestamp};
use usher_coreaudio::{CoreAudioSystem, Notify};

use crate::engine::{Clock, Engine, Event, PanelView, Shell, run};
use crate::store::Store;
use crate::{logging, tray};

pub struct TauriShell(AppHandle);

impl Shell for TauriShell {
    fn state_changed(&self, view: &PanelView) {
        if let Err(e) = self.0.emit("state-changed", view) {
            warn!("state-changed emit failed ({e})");
        }
    }

    fn auto_switch_changed(&self, on: bool) {
        tray::set_auto(&self.0, on);
    }

    fn paused(&self, reason: &PausedReason) {
        let app = self.0.clone();
        let body = reason.message();
        // permission calls can block on a system prompt: never on the engine thread
        std::thread::spawn(move || {
            let n = app.notification();
            let granted = match n.permission_state() {
                Ok(PermissionState::Granted) => true,
                _ => matches!(n.request_permission(), Ok(PermissionState::Granted)),
            };
            if granted && let Err(e) = n.builder().title("usher").body(body).show() {
                warn!("notification failed ({e})");
            }
        });
    }
}

/// Opens the store and starts the engine thread. `CoreAudioSystem` is !Send, so it is built inside the thread.
pub fn spawn(app: &AppHandle, rx: Receiver<Event>, notify: Notify, dir: PathBuf) {
    let (store, state) = Store::open(dir.join("state.json"));
    logging::apply(state.as_ref().is_some_and(|s| s.debug));
    let shell = TauriShell(app.clone());
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let spawned = std::thread::Builder::new()
        .name("usher-engine".into())
        .spawn(move || {
            let now = Instant::now();
            let clock = Clock {
                start: now,
                wall_at_start: Timestamp(wall),
            };
            let engine = Engine::start(
                CoreAudioSystem::new(),
                shell,
                store,
                state,
                notify,
                clock,
                now,
            );
            run(engine, rx);
        });
    if let Err(e) = spawned {
        error!("engine thread did not start ({e})");
    }
}
