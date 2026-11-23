//! Open at login: a LaunchAgent through tauri-plugin-autostart. The LaunchAgent is the truth; usher keeps no copy.

use std::path::Path;

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// The LaunchAgent's label and file name: the bundle identifier. A test pins it to `tauri.conf.json`.
pub const AUTOSTART_NAME: &str = "com.cef62.usher";

/// Why a login item must not point at this executable, if so. The crate writes the path unquoted and unescaped: an App
/// Translocation path is gone after a reboot, and `& < >` make the plist invalid. A development build, or a release
/// build run from cargo's `target` folder, is not the installed app that shares the item's name.
pub fn login_item_refusal(exe: &str, debug_build: bool) -> Option<&'static str> {
    if debug_build {
        Some("Open at login is not available in a development build.")
    } else if exe.find("/target/").is_some_and(|i| {
        ["/release/", "/debug/"]
            .iter()
            .any(|d| exe[i..].contains(d))
    }) {
        Some(
            "This usher runs from a build folder. Open at login is only for an usher installed in Applications.",
        )
    } else if exe.contains("/AppTranslocation/") {
        Some(
            "macOS is running usher from a temporary location. Move usher to Applications, open it from there, then turn Open at login on.",
        )
    } else if exe.contains(['&', '<', '>']) {
        Some(
            "usher's folder name contains & < or >, which a login item cannot hold. Move usher to Applications, then turn Open at login on.",
        )
    } else {
        None
    }
}

fn exe() -> String {
    std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

pub fn refusal() -> Option<&'static str> {
    login_item_refusal(&exe(), cfg!(debug_assertions))
}

/// `ProgramArguments[0]` of a LaunchAgent plist as the crate writes it.
fn plist_program(plist: &str) -> Option<&str> {
    plist
        .split("<key>ProgramArguments</key>")
        .nth(1)?
        .split("<string>")
        .nth(1)?
        .split("</string>")
        .next()
}

/// Rewrite the item only when it is on, not refused, and its recorded path differs: a normal start writes nothing
/// (no macOS "Background Items Added" notice each time).
fn should_refresh(enabled: bool, refused: bool, recorded: Option<&str>, exe: &str) -> bool {
    enabled && !refused && recorded != Some(exe)
}

/// The OS's own answer: whether the plist exists.
pub fn enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// At startup: an enabled item keeps pointing at this copy after the app moved.
pub fn refresh(app: &AppHandle) {
    let launcher = app.autolaunch();
    let exe = exe();
    let plist = std::env::var_os("HOME").and_then(|h| {
        std::fs::read_to_string(
            Path::new(&h).join(format!("Library/LaunchAgents/{AUTOSTART_NAME}.plist")),
        )
        .ok()
    });
    if should_refresh(
        launcher.is_enabled().unwrap_or(false),
        refusal().is_some(),
        plist.as_deref().and_then(plist_program),
        &exe,
    ) && let Err(e) = launcher.enable()
    {
        log::warn!("could not refresh the Open at login item ({e})");
    }
}

/// Flips the login item, then answers what the OS reports. Turning it off is never refused.
#[tauri::command(async)]
pub fn set_login(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let verb = if enabled { "turn on" } else { "turn off" };
    if let Some(why) = refusal().filter(|_| enabled) {
        return Err(why.into());
    }
    let launcher = app.autolaunch();
    if let Err(e) = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    } {
        log::warn!("could not {verb} Open at login ({e})");
        return Err(format!("Could not {verb} Open at login: {e}"));
    }
    let now = launcher.is_enabled().map_err(|e| e.to_string())?;
    if now != enabled {
        return Err(format!(
            "Could not {verb} Open at login: the system kept the previous state."
        ));
    }
    Ok(now)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_dev_builds_translocated_and_unwritable_paths() {
        let ok = "/Applications/usher.app/Contents/MacOS/usher";
        assert_eq!(login_item_refusal(ok, false), None);
        assert_eq!(
            login_item_refusal(ok, true),
            Some("Open at login is not available in a development build.")
        );
        let built = "/Volumes/Dev/usher/target/release/bundle/macos/usher.app/Contents/MacOS/usher";
        assert_eq!(
            login_item_refusal(built, false),
            Some(
                "This usher runs from a build folder. Open at login is only for an usher installed in Applications."
            )
        );
        let cross = "/Users/me/usher/target/aarch64-apple-darwin/release/bundle/macos/usher.app/Contents/MacOS/usher";
        assert!(
            login_item_refusal(cross, false)
                .unwrap()
                .contains("build folder")
        );
        assert_eq!(
            login_item_refusal(
                "/Applications/target practice/usher.app/Contents/MacOS/usher",
                false
            ),
            None
        );
    }

    #[test]
    fn refuses_translocated_and_unwritable_paths() {
        let moved =
            "/private/var/folders/x/T/AppTranslocation/1234/d/usher.app/Contents/MacOS/usher";
        assert_eq!(
            login_item_refusal(moved, false),
            Some(
                "macOS is running usher from a temporary location. Move usher to Applications, open it from there, then turn Open at login on."
            )
        );
        assert_eq!(
            login_item_refusal(
                "/Users/me/Apps & Tools/usher.app/Contents/MacOS/usher",
                false
            ),
            Some(
                "usher's folder name contains & < or >, which a login item cannot hold. Move usher to Applications, then turn Open at login on."
            )
        );
    }

    #[test]
    fn refreshes_only_an_enabled_item_whose_path_moved() {
        let plist = "<dict><key>Label</key><string>com.cef62.usher</string><key>ProgramArguments</key><array><string>/Old/usher.app/Contents/MacOS/usher</string></array></dict>";
        let rec = plist_program(plist);
        assert_eq!(rec, Some("/Old/usher.app/Contents/MacOS/usher"));
        assert!(should_refresh(
            true,
            false,
            rec,
            "/Applications/usher.app/Contents/MacOS/usher"
        ));
        assert!(!should_refresh(
            true,
            false,
            rec,
            "/Old/usher.app/Contents/MacOS/usher"
        ));
        assert!(!should_refresh(false, false, rec, "/New"));
        assert!(!should_refresh(true, true, rec, "/New"));
        assert_eq!(plist_program("<dict></dict>"), None);
    }

    #[test]
    fn autostart_name_is_the_bundle_identifier() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], AUTOSTART_NAME);
    }
}
