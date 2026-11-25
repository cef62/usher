//! IPC commands of the panel and the per-window guard that makes capabilities.json's promise true.

use log::info;
use tauri::ipc::Invoke;
use tauri::{AppHandle, Runtime, State};
use usher_core::{Command, LevelRequest};

use crate::engine::{EngineHandle, PanelView, REQUEST_TIMEOUT};
use crate::{activity, panel, settings, shell::LastView, whatsnew};

pub const PANEL_COMMANDS: &[&str] = &[
    "get_view",
    "run_command",
    "set_level",
    "hide_panel",
    "open_settings",
    "open_whatsnew",
    "open_activity",
    "quit",
];
pub const SETTINGS_COMMANDS: &[&str] = &[
    "get_settings",
    "set_login",
    "set_debug",
    "reveal_log",
    "set_update_check",
    "set_notify",
    "check_update",
    "open_whatsnew",
    "open_activity",
    "close_window",
];
pub const WHATSNEW_COMMANDS: &[&str] = &["whatsnew_info", "whatsnew_install", "close_window"];
pub const ACTIVITY_COMMANDS: &[&str] = &["get_activity", "get_diagnostics", "close_window"];

/// Custom commands are open to every window (no app ACL manifest in build.rs, so capabilities do not restrict them):
/// this is the boundary. A command missing from both lists is denied everywhere.
pub fn allowed(label: &str, command: &str) -> bool {
    match label {
        panel::LABEL => PANEL_COMMANDS.contains(&command),
        settings::LABEL => SETTINGS_COMMANDS.contains(&command),
        whatsnew::LABEL => WHATSNEW_COMMANDS.contains(&command),
        activity::LABEL => ACTIVITY_COMMANDS.contains(&command),
        _ => false,
    }
}

pub fn with_window_guard<R: Runtime>(
    h: impl Fn(Invoke<R>) -> bool + Send + Sync + 'static,
) -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    move |invoke| {
        if !allowed(
            invoke.message.webview_ref().label(),
            invoke.message.command(),
        ) {
            invoke.resolver.reject("not allowed from this window");
            return true;
        }
        h(invoke)
    }
}

/// Waits up to `REQUEST_TIMEOUT` for the engine, off the main thread.
pub async fn request(engine: &EngineHandle, cmd: Command) -> Result<(), String> {
    let engine = engine.clone();
    tauri::async_runtime::spawn_blocking(move || engine.request(cmd, REQUEST_TIMEOUT))
        .await
        .map_err(|e| format!("usher could not run the command ({e})"))?
        .map_err(|r| r.to_string())
}

#[tauri::command]
pub fn get_view(view: State<LastView>) -> Option<PanelView> {
    view.0.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

#[tauri::command]
pub async fn run_command(engine: State<'_, EngineHandle>, cmd: Command) -> Result<(), String> {
    request(&engine, cmd).await
}

#[tauri::command]
pub async fn set_level(engine: State<'_, EngineHandle>, req: LevelRequest) -> Result<(), String> {
    if !req.valid() {
        return Err("invalid volume".into());
    }
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.level(req, REQUEST_TIMEOUT))
        .await
        .map_err(|e| format!("usher could not run the command ({e})"))?
        .map_err(|r| r.to_string())
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    settings::open(&app);
}

/// Esc in Settings, Activity or What's new: closes the calling window only (no window may close another).
#[tauri::command]
pub fn close_window(window: tauri::WebviewWindow) {
    window.close().ok();
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    info!("quit from panel");
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_window_calls_only_its_own_commands() {
        let lists = [
            ("panel", PANEL_COMMANDS),
            ("settings", SETTINGS_COMMANDS),
            ("whatsnew", WHATSNEW_COMMANDS),
            ("activity", ACTIVITY_COMMANDS),
        ];
        for (label, list) in lists {
            for c in list {
                assert!(allowed(label, c));
                for (other, _) in lists {
                    if other != label {
                        // the open_* commands are shared by the panel and Settings; close_window by every window but the panel
                        let shared = ((*c == "open_whatsnew" || *c == "open_activity")
                            && (other == "panel" || other == "settings"))
                            || (*c == "close_window" && other != "panel");
                        assert_eq!(allowed(other, c), shared, "{other} {c}");
                    }
                }
            }
        }
        assert!(!allowed("whatsnew", "get_view") && !allowed("whatsnew", "open_whatsnew"));
        assert!(!allowed("activity", "get_view") && !allowed("panel", "get_diagnostics"));
        assert!(!allowed("activity", "open_activity") && !allowed("settings", "get_activity"));
        assert!(!allowed("panel", "whatsnew_install") && !allowed("", "quit"));
        assert!(!allowed("panel", "close_window") && allowed("whatsnew", "close_window"));
        assert!(!allowed("panel", "plugin:log|log"));
        assert!(allowed("panel", "set_level"));
        for other in ["settings", "whatsnew", "activity", ""] {
            assert!(!allowed(other, "set_level"), "{other}");
        }
    }
}
