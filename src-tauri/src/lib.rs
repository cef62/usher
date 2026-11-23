#![forbid(unsafe_code)]

mod instance;
mod tray;

use tauri::Manager;

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // first: a second copy (login item beside a running usher, or a dev build) leaves before it registers anything
    if let Ok(dir) = app.path().app_data_dir()
        && !instance::hold_instance_lock(&dir)
    {
        eprintln!("usher: another usher is running; this one exits");
        std::process::exit(0);
    }
    if let Err(e) = tray::build(app.handle()) {
        eprintln!("usher: no menu-bar icon ({e})");
    }
    Ok(())
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .setup(setup)
        .build(tauri::generate_context!())
        .expect("error while running usher");
    // no Dock icon, not in ⌘Tab. Set before run: tao applies its policy (Regular by default) in
    // applicationDidFinishLaunching, which overrides Info.plist's LSUIElement; from setup it lands ~60 ms too late
    #[cfg(target_os = "macos")]
    let mut app = app;
    #[cfg(target_os = "macos")]
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
