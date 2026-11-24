//! The Settings window: Open at login, Debug logging, Show log in Finder, Version.

use std::ffi::OsString;
use std::path::Path;

use log::{info, warn};
use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use usher_core::Command;

use crate::engine::EngineHandle;
use crate::shell::LastView;
use crate::{commands, login, panel, update};

pub const LABEL: &str = "settings";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub login: bool,
    pub login_refusal: Option<String>,
    pub debug: bool,
    pub update_check: bool,
    /// The version waiting to install, if a check found one.
    pub update: Option<String>,
    pub version: String,
}

/// Opens Settings, or brings it to the front. The panel hides first.
pub fn open(app: &AppHandle) {
    panel::hide_for_window(app); // no app.hide(): the previous app must not flash active
    app.show().ok(); // an earlier panel hide left the app hidden
    if let Some(w) = app.get_webview_window(LABEL) {
        w.unminimize().ok();
        w.show().ok();
        w.set_focus().ok();
        return;
    }
    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("settings.html".into()))
        .title("usher Settings")
        .inner_size(420.0, 360.0)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .center()
        .build();
    match built {
        Ok(w) => {
            w.set_focus().ok();
        }
        Err(e) => warn!("could not open Settings ({e})"),
    }
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> SettingsView {
    let last = app.state::<LastView>();
    let last = last.0.lock().unwrap_or_else(|p| p.into_inner());
    let debug = last.as_ref().is_some_and(|v| v.panel.debug);
    let update_check = update::daily_check_on(last.as_ref());
    SettingsView {
        login: login::enabled(&app),
        login_refusal: login::refusal().map(Into::into),
        debug,
        update_check,
        update: update::pending(&app),
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub async fn set_debug(engine: State<'_, EngineHandle>, on: bool) -> Result<(), String> {
    commands::request(&engine, Command::SetDebug(on)).await
}

#[tauri::command]
pub async fn set_update_check(engine: State<'_, EngineHandle>, on: bool) -> Result<(), String> {
    commands::request(&engine, Command::SetUpdateCheck(on)).await
}

/// `open -R <log>` selects the file; a missing log opens the folder.
pub fn reveal_args(log_dir: &Path, file_exists: bool) -> Vec<OsString> {
    if file_exists {
        vec!["-R".into(), log_dir.join("usher.log").into()]
    } else {
        vec![log_dir.into()]
    }
}

#[tauri::command]
pub fn reveal_log(app: AppHandle) -> Result<(), String> {
    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    let args = reveal_args(&dir, dir.join("usher.log").exists());
    info!("show log in Finder");
    let mut child = std::process::Command::new("/usr/bin/open")
        .args(args)
        .spawn()
        .map_err(|e| format!("Could not open the log folder: {e}"))?;
    // wait on a thread so the finished `open` is reaped, not left a zombie
    std::thread::spawn(move || match child.wait() {
        Ok(s) if !s.success() => warn!("open exited with {s}"),
        Err(e) => warn!("could not wait on open ({e})"),
        _ => {}
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::Path;
    #[test]
    fn reveal_selects_the_log_or_opens_the_folder() {
        let d = Path::new("/Users/u/Library/Logs/com.cef62.usher");
        assert_eq!(
            reveal_args(d, true),
            vec![OsString::from("-R"), d.join("usher.log").into()]
        );
        assert_eq!(reveal_args(d, false), vec![OsString::from(d)]);
    }
}
