//! The What's new window: what the stored update brings (its release notes), then "Install and restart". Opened on
//! request only (the panel's update row, the menu-bar item, Settings); one at a time.
//!
//! Boundary. The window invokes only `commands::WHATSNEW_COMMANDS`. Its permissions (capabilities/whatsnew.json) are
//! `log:default` and dragging by its title strip: install progress reaches it over a `Channel` given to `whatsnew_info`, scoped to its webview.

use tauri::ipc::Channel;
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder};

use crate::update::{self, Phase, UpdateInfo};

pub const LABEL: &str = "whatsnew";

/// The bundled page or, in `tauri dev`, the dev server. Anything else (a dropped URL) must not load in this window.
/// Scheme, host and port must match exactly, with no userinfo.
fn own_page(u: &Url, dev: Option<&Url>) -> bool {
    let plain = u.username().is_empty() && u.password().is_none();
    let bundled = u.scheme() == "tauri" && u.host_str() == Some("localhost") && u.port().is_none();
    plain && (bundled || dev.is_some_and(|d| d.origin() == u.origin()))
}

/// Opens the window, or brings it to the front (reloaded: a newer version may have been found since it opened). The
/// panel hides first, without handing the focus back: this window takes it.
pub fn open(app: &AppHandle) {
    crate::panel::hide_for_window(app);
    app.show().ok(); // undo the panel's app.hide(), or the window stays hidden
    if let Some(w) = app.get_webview_window(LABEL) {
        w.reload().ok();
        w.unminimize().ok();
        w.show().ok();
        w.set_focus().ok();
        return;
    }
    let dev = cfg!(debug_assertions)
        .then(|| app.config().build.dev_url.clone())
        .flatten();
    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("whatsnew.html".into()))
        .title("What's new in usher")
        .transparent(true) // the page draws over Liquid Glass, like the panel
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .inner_size(520.0, 560.0)
        .min_inner_size(420.0, 360.0)
        .minimizable(false)
        .maximizable(false)
        .center()
        .on_navigation(move |u| own_page(u, dev.as_ref()))
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .build();
    match built {
        Ok(w) => {
            crate::glass::apply(&w, crate::glass::WINDOW_RADIUS);
            w.set_focus().ok();
        }
        Err(e) => log::warn!("could not open What's new ({e})"),
    }
}

/// The panel's update row and Settings.
#[tauri::command]
pub fn open_whatsnew(app: AppHandle) {
    open(&app);
}

/// The stored update and where its install stands.
#[derive(serde::Serialize)]
pub struct WhatsNew {
    update: Option<UpdateInfo>,
    phase: Phase,
}

/// The window's data, asked on mount. `on_phase` receives every later change of the install; the reply carries the
/// current one, so nothing sent before the window loaded is missed.
#[tauri::command]
pub fn whatsnew_info(app: AppHandle, on_phase: Channel<Phase>) -> WhatsNew {
    let mut st = update::state(&app);
    st.watcher = Some(on_phase);
    WhatsNew {
        update: st.available.as_ref().map(UpdateInfo::from),
        phase: st.phase.clone(),
    }
}

/// "Install and restart" for `version`, the one the window shows. Returns only on failure: success relaunches usher.
#[tauri::command]
pub async fn whatsnew_install(app: AppHandle, version: String) -> Result<(), String> {
    update::install::install(&app, &version).await
}

/// Esc or Later. Closes this window only.
#[tauri::command]
pub fn whatsnew_close(app: AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        w.close().ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_bundled_page_loads() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert!(own_page(&u("tauri://localhost/whatsnew.html"), None));
        let dev = u("http://localhost:1420");
        assert!(own_page(
            &u("http://localhost:1420/whatsnew.html"),
            Some(&dev)
        ));
        for bad in [
            "https://github.com/cef62/usher",
            "http://localhost:1421/",
            "tauri://evil/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "tauri://localhost:9/",
            "tauri://localhost.evil/",
            "tauri://LOCALHOST/", // not lowercased for a custom scheme: not the app's origin
            "tauri://user:pw@localhost/",
            "http://localhost:1420@evil.com/",
        ] {
            assert!(!own_page(&u(bad), Some(&dev)), "{bad}");
        }
        assert!(!own_page(&u("http://localhost:1420/"), None)); // release: no dev server
    }

    #[test]
    fn capability_grants_only_the_log_and_drag() {
        let cap: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/whatsnew.json")).unwrap();
        assert_eq!(cap["windows"], serde_json::json!([LABEL]));
        assert_eq!(
            cap["permissions"],
            serde_json::json!([
                "log:default",
                "core:window:allow-start-dragging",
                "core:window:allow-internal-toggle-maximize"
            ])
        );
    }
}
