//! The activity history the engine feeds: the newest [`HISTORY`] lines, newest first.

use std::collections::VecDeque;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use usher_core::{AppInfo, Entry, render_diagnostics};

use crate::engine::{EngineHandle, REQUEST_TIMEOUT};

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

    pub fn lines(&self) -> Vec<Line> {
        self.lines.iter().cloned().collect()
    }
}

#[derive(Default)]
pub struct ActivityLog(pub Mutex<Log>);

/// First line of a command's stdout, trimmed; `None` on any error.
fn run_line(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    out.status.success().then(|| text.trim().to_string())
}

pub fn macos_version() -> String {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| {
        run_line("/usr/bin/sw_vers", &["-productVersion"])
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "unknown".into())
    })
    .clone()
}

/// The alphanumeric parts of the full name and short name worth stripping from a report: 3+ letters each, 2+ with a non-ASCII letter.
/// Parts, not words: `strip_owner` compares whole alphanumeric parts, so `john.smith` must become `john`, `smith`.
fn owner_words(full_name: &str, user: &str) -> Vec<String> {
    full_name
        .split(|c: char| !c.is_alphanumeric())
        .chain(user.split(|c: char| !c.is_alphanumeric()))
        .filter(|w| {
            let letters = w.chars().filter(|c| c.is_alphabetic()).count();
            letters >= 3 || (letters >= 2 && !w.is_ascii())
        })
        .map(str::to_string)
        .collect()
}

fn owner_names() -> Vec<String> {
    static N: OnceLock<Vec<String>> = OnceLock::new();
    N.get_or_init(|| {
        let full = run_line("/usr/bin/id", &["-F"]).unwrap_or_default();
        owner_words(&full, &std::env::var("USER").unwrap_or_default())
    })
    .clone()
}

/// The redacted report for a GitHub issue. Blocking calls run off the main thread.
#[tauri::command]
pub async fn get_diagnostics(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
) -> Result<String, String> {
    let engine = engine.inner().clone();
    let log_path = app
        .path()
        .app_log_dir()
        .map_err(|e| e.to_string())?
        .join("usher.log");
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    let version = app.package_info().version.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let mut d = engine
            .diagnostics(REQUEST_TIMEOUT)
            .map_err(|e| e.to_string())?;
        d.app = AppInfo {
            version,
            macos: macos_version(),
            log_path: log_path.display().to_string(),
            owner_names: owner_names(),
        };
        Ok(render_diagnostics(&d, &home.display().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Opens the window, or brings it to the front. The panel hides first, without handing the focus back: this window
/// takes it.
pub fn open(app: &AppHandle) {
    crate::panel::hide_for_window(app);
    app.show().ok(); // undo the panel's app.hide(), or the window stays hidden
    if let Some(w) = app.get_webview_window(LABEL) {
        w.unminimize().ok();
        w.show().ok();
        w.set_focus().ok();
        return;
    }
    let dev = cfg!(debug_assertions)
        .then(|| app.config().build.dev_url.clone())
        .flatten();
    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("activity.html".into()))
        .title("usher Activity")
        .transparent(true) // the page draws over Liquid Glass, like the panel
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .inner_size(480.0, 560.0)
        .min_inner_size(360.0, 320.0)
        .minimizable(false)
        .maximizable(false)
        .center()
        .on_navigation(move |u| crate::whatsnew::own_page(u, dev.as_ref()))
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .build();
    match built {
        Ok(w) => {
            crate::glass::apply(&w, crate::glass::WINDOW_RADIUS);
            w.set_focus().ok();
        }
        Err(e) => log::warn!("could not open Activity ({e})"),
    }
}

/// The panel's menu and Settings.
#[tauri::command]
pub fn open_activity(app: AppHandle) {
    open(&app);
}

/// The history so far, newest first; later lines arrive as `activity` events.
#[tauri::command]
pub fn get_activity(log: State<ActivityLog>) -> Vec<Line> {
    log.0.lock().unwrap_or_else(|p| p.into_inner()).lines()
}

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

    #[test]
    fn capability_grants_log_listen_and_drag() {
        let cap: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/activity.json")).unwrap();
        assert_eq!(cap["windows"], serde_json::json!([LABEL]));
        assert_eq!(
            cap["permissions"],
            serde_json::json!([
                "log:default",
                "core:event:allow-listen",
                "core:event:allow-unlisten",
                "core:window:allow-start-dragging",
                "core:window:allow-internal-toggle-maximize"
            ])
        );
    }

    #[test]
    fn macos_version_is_a_dotted_number_or_unknown() {
        let v = macos_version();
        assert!(
            v == "unknown" || v.split('.').all(|p| p.parse::<u32>().is_ok()),
            "{v}"
        );
    }

    #[test]
    fn owner_words_keep_long_words_of_name_and_user() {
        let w = owner_words("Ada Lo Lovelace", "ada");
        assert_eq!(w, ["Ada", "Lovelace", "ada"]);
        assert!(owner_words("", "").is_empty());
        assert_eq!(
            owner_words("山田 太郎", "yamada"),
            ["山田", "太郎", "yamada"]
        );
        assert_eq!(
            owner_words("Jo O'Brien", "mary-jane"),
            ["Brien", "mary", "jane"]
        );
    }
}
