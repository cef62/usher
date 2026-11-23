#![forbid(unsafe_code)]

mod commands;
pub mod engine;
mod instance;
mod logging;
mod login;
mod panel;
mod settings;
mod shell;
pub mod store;
mod tray;

use std::time::Duration;

use engine::EngineHandle;
use std::sync::Mutex;
use tauri::Manager;

/// Managed only when the engine thread was spawned.
struct EngineStarted;

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("usher {} starting", app.package_info().version);
    let dir = app.path().app_data_dir();
    // a second copy (login item beside a running usher, or a dev build) leaves before it registers anything
    match &dir {
        Ok(dir) => {
            if !instance::hold_instance_lock(dir) {
                log::warn!("another usher is running; this one exits");
                std::process::exit(0);
            }
            log::debug!("instance lock held in {}", dir.display());
        }
        Err(e) => log::warn!("no app data dir, instance lock skipped ({e})"),
    }
    let (handle, rx) = EngineHandle::new();
    app.manage(handle.clone());
    app.manage(shell::LastView(Mutex::new(None)));
    panel::setup(app.handle());
    if let Err(e) = tray::build(app.handle()) {
        // an agent app without a tray has no UI and no way to quit
        log::error!("no menu-bar icon ({e})");
        std::process::exit(1);
    }
    match dir {
        Ok(dir) => {
            if shell::spawn(app.handle(), rx, handle.notify(), dir) {
                app.manage(EngineStarted);
            }
        }
        // no state file location, no engine; the tray still works and Quit still exits
        Err(e) => log::error!("engine not started: no app data dir ({e})"),
    }
    login::refresh(app.handle());
    Ok(())
}

fn log_panics() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        prev(info);
    }));
}

pub fn run() {
    log_panics();
    let mut app = tauri::Builder::default()
        .plugin(logging::plugin())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name(login::AUTOSTART_NAME)
                .build(),
        )
        .setup(setup)
        .on_window_event(|w, ev| {
            if let tauri::WindowEvent::Focused(f) = ev
                && w.label() == panel::LABEL
            {
                panel::focus_changed(w.app_handle(), *f);
            }
        })
        .invoke_handler(commands::with_window_guard(tauri::generate_handler![
            commands::get_view,
            commands::run_command,
            panel::hide_panel,
            commands::open_settings,
            commands::quit,
            settings::get_settings,
            login::set_login,
            settings::set_debug,
            settings::reveal_log
        ]))
        .build(tauri::generate_context!())
        .expect("error while running usher");
    // right after build, not in setup: tao logs TRACE from applicationDidFinishLaunching before setup runs, and the
    // plugin leaves the max level at Trace.
    logging::apply(false);
    // no Dock icon, not in ⌘Tab. Set before run: tao applies its policy (Regular by default) in
    // applicationDidFinishLaunching, which overrides Info.plist's LSUIElement; from setup it lands ~60 ms too late
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    app.run(|app, ev| {
        if let tauri::RunEvent::Exit = ev {
            // flush state.json; false when the engine never started or is gone
            let acked = app.state::<EngineHandle>().quit(Duration::from_secs(1));
            if !acked && app.try_state::<EngineStarted>().is_some() {
                log::warn!("engine did not ack quit within 1 s; last changes may be unsaved");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundle_config_pins_identity_and_platform() {
        let c: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(c["identifier"], "com.cef62.usher");
        assert_eq!(c["productName"], "usher");
        assert_eq!(c["bundle"]["macOS"]["minimumSystemVersion"], "14.0");
        assert_eq!(c["bundle"]["macOS"]["signingIdentity"], "-");
        assert!(c["plugins"]["updater"].is_null(), "updater is M5");
    }

    #[test]
    fn panel_window_and_capabilities_are_pinned() {
        let c: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let w = &c["app"]["windows"][0];
        assert_eq!(w["label"], "panel");
        assert_eq!(w["visible"], false);
        assert_eq!(w["width"], 360);
        assert_eq!(w["alwaysOnTop"], true);
        assert_eq!(w["visibleOnAllWorkspaces"], true);
        assert_eq!(w["acceptFirstMouse"], true);
        assert_eq!(w["transparent"], true);
        // the material is applied in code (panel::glass): config windowEffects cannot do Liquid Glass
        assert!(w["windowEffects"].is_null());
        assert_eq!(c["app"]["macOSPrivateApi"], true);
        assert_eq!(
            c["app"]["security"]["csp"],
            "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:"
        );
        let cap: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        assert_eq!(cap["windows"], serde_json::json!(["panel"]));
        assert_eq!(
            cap["permissions"],
            serde_json::json!([
                "log:default",
                "core:event:allow-listen",
                "core:event:allow-unlisten",
                "core:window:allow-set-size"
            ])
        );
    }

    #[test]
    fn settings_capability_grants_log_and_set_size() {
        let cap: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/settings.json")).unwrap();
        assert_eq!(cap["windows"], serde_json::json!(["settings"]));
        assert_eq!(
            cap["permissions"],
            serde_json::json!(["log:default", "core:window:allow-set-size"])
        );
    }
}
