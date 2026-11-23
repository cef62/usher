//! IPC commands of the panel and the per-window guard that makes capabilities.json's promise true.

use log::info;
use tauri::ipc::Invoke;
use tauri::{AppHandle, Runtime, State};
use usher_core::Command;

use crate::engine::{EngineHandle, PanelView, REQUEST_TIMEOUT};
use crate::{panel, settings, shell::LastView};

pub const PANEL_COMMANDS: &[&str] = &[
    "get_view",
    "run_command",
    "hide_panel",
    "open_settings",
    "quit",
];
pub const SETTINGS_COMMANDS: &[&str] = &["get_settings", "set_login", "set_debug", "reveal_log"];

/// Custom commands are open to every window (no app ACL manifest in build.rs, so capabilities do not restrict them):
/// this is the boundary. A command missing from both lists is denied everywhere.
pub fn allowed(label: &str, command: &str) -> bool {
    match label {
        panel::LABEL => PANEL_COMMANDS.contains(&command),
        settings::LABEL => SETTINGS_COMMANDS.contains(&command),
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
pub fn open_settings(app: AppHandle) {
    settings::open(&app);
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
        for c in PANEL_COMMANDS {
            assert!(allowed("panel", c));
            assert!(!allowed("settings", c), "{c}");
        }
        for c in SETTINGS_COMMANDS {
            assert!(allowed("settings", c));
            assert!(!allowed("panel", c), "{c}");
        }
        assert!(!allowed("whatsnew", "get_view"));
        assert!(!allowed("panel", "plugin:log|log"));
    }
}
