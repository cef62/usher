#![forbid(unsafe_code)]

mod instance;
mod logging;
pub mod store;
mod tray;

use tauri::Manager;

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("usher {} starting", app.package_info().version);
    // a second copy (login item beside a running usher, or a dev build) leaves before it registers anything
    match app.path().app_data_dir() {
        Ok(dir) => {
            if !instance::hold_instance_lock(&dir) {
                log::warn!("another usher is running; this one exits");
                std::process::exit(0);
            }
            log::debug!("instance lock held in {}", dir.display());
        }
        Err(e) => log::warn!("no app data dir, instance lock skipped ({e})"),
    }
    if let Err(e) = tray::build(app.handle()) {
        // an agent app without a tray has no UI and no way to quit
        log::error!("no menu-bar icon ({e})");
        std::process::exit(1);
    }
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
        .setup(setup)
        .build(tauri::generate_context!())
        .expect("error while running usher");
    // right after build, not in setup: tao logs TRACE from applicationDidFinishLaunching before setup runs, and the
    // plugin leaves the max level at Trace. M4 passes the persisted Debug setting.
    logging::apply(false);
    // no Dock icon, not in ⌘Tab. Set before run: tao applies its policy (Regular by default) in
    // applicationDidFinishLaunching, which overrides Info.plist's LSUIElement; from setup it lands ~60 ms too late
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    app.run(|_, _| {});
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
}
