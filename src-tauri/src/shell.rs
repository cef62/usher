//! The engine's window onto the app: panel events, tray state, loop-guard notification.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::Receiver;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use log::{error, info, warn};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::{NotificationExt, PermissionState};
use usher_core::{Entry, PausedReason, Timestamp};
use usher_coreaudio::{CoreAudioSystem, Notify};

use crate::activity::{ActivityLog, LABEL};
use crate::engine::{Clock, Engine, Event, PanelView, Shell, run};
use crate::store::Store;
use crate::{logging, tray};

/// The engine's last view, for a window that opens after the event (`get_view`, `get_settings`).
pub struct LastView(pub Mutex<Option<PanelView>>);

/// Sets the slot to `new` (when given), stamps it with `pending` and returns it. `None`: nothing was ever published.
fn apply(
    slot: &mut Option<PanelView>,
    new: Option<PanelView>,
    pending: Option<String>,
) -> Option<&PanelView> {
    if new.is_some() {
        *slot = new;
    }
    let view = slot.as_mut()?;
    view.update = pending;
    Some(view)
}

/// Stores and sends `view` (or, with `None`, the last view again) stamped with the pending update. The `LastView` lock is
/// held from reading the pending update to the emit, so the engine thread and a finished check cannot overwrite each
/// other with a stale view. Lock order: `LastView`, then `UpdateState`.
fn send(app: &AppHandle, new: Option<PanelView>) {
    let Some(last) = app.try_state::<LastView>() else {
        return;
    };
    let mut slot = last.0.lock().unwrap_or_else(|p| p.into_inner());
    let pending = crate::update::pending(app);
    if let Some(view) = apply(&mut slot, new, pending)
        && let Err(e) = app.emit("state-changed", view)
    {
        warn!("state-changed emit failed ({e})");
    }
}

/// Adds the pending update to `view`, remembers it as the last view and sends it to the windows.
pub fn publish(app: &AppHandle, view: PanelView) {
    send(app, Some(view));
}

/// Publishes the last view again: the pending update changed, the engine's state did not.
pub fn republish(app: &AppHandle) {
    send(app, None);
}

pub struct TauriShell(AppHandle);

impl Shell for TauriShell {
    fn state_changed(&self, view: &PanelView) {
        publish(&self.0, view.clone());
    }

    fn auto_switch_changed(&self, on: bool) {
        tray::set_auto(&self.0, on);
    }

    fn debug_changed(&self, on: bool) {
        logging::apply(on);
    }

    fn activity(&self, entry: &Entry) {
        let wall = wall_ms();
        let Some(log) = self.0.try_state::<ActivityLog>() else {
            return;
        };
        let line = log
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(wall, entry);
        if let Err(e) = self.0.emit_to(LABEL, "activity", &line) {
            warn!("activity emit failed ({e})");
        }
    }

    fn paused(&self, reason: &PausedReason) {
        show(self.0.clone(), reason.message());
    }

    fn notice(&self, body: &str) {
        show(self.0.clone(), body.into());
    }
}

fn wall_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Shows a system notification. Permission calls can block on a system prompt: never on the engine thread.
fn show(app: AppHandle, body: String) {
    std::thread::spawn(move || {
        let n = app.notification();
        let granted = match n.permission_state() {
            Ok(PermissionState::Granted) => true,
            _ => matches!(n.request_permission(), Ok(PermissionState::Granted)),
        };
        if !granted {
            info!("notifications are not allowed; notice not shown");
        } else if let Err(e) = n.builder().title("usher").body(body).show() {
            warn!("notification failed ({e})");
        }
    });
}

/// Opens the store and starts the engine thread. `CoreAudioSystem` is !Send, so it is built inside the thread.
/// `false` when the thread did not start.
pub fn spawn(app: &AppHandle, rx: Receiver<Event>, notify: Notify, dir: PathBuf) -> bool {
    let (store, state) = Store::open(dir.join("state.json"));
    logging::apply(state.as_ref().is_some_and(|s| s.debug));
    let shell = TauriShell(app.clone());
    let wall = wall_ms();
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
    if let Err(e) = &spawned {
        error!("engine thread did not start ({e})");
    }
    spawned.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use usher_core::PerRole;

    fn view(auto: bool) -> PanelView {
        let mut panel = usher_core::panel_state(
            &usher_core::State::default(),
            &usher_core::Snapshot {
                devices: vec![],
                current: PerRole::default(),
            },
        );
        panel.auto_switch = auto;
        PanelView {
            panel,
            errors: PerRole::default(),
            read_only: false,
            update: None,
        }
    }

    #[test]
    fn republish_stamps_the_stored_view_and_publish_keeps_the_pending_update() {
        let mut slot = None;
        assert!(
            apply(&mut slot, None, Some("1".into())).is_none(),
            "nothing published yet"
        );
        apply(&mut slot, Some(view(true)), None);
        let v = apply(&mut slot, None, Some("0.2.0".into())).unwrap();
        assert_eq!(v.update.as_deref(), Some("0.2.0"));
        // a newer engine view still carries the update the check found
        let v = apply(&mut slot, Some(view(false)), Some("0.2.0".into())).unwrap();
        assert!(!v.panel.auto_switch && v.update.as_deref() == Some("0.2.0"));
        // republish must not roll the engine's view back
        assert!(!apply(&mut slot, None, None).unwrap().panel.auto_switch);
    }
}
