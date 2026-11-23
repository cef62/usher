use log::warn;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Manager};

use crate::engine::{EngineHandle, Event};

const ICON_ON: &[u8] = include_bytes!("../icons/tray-template.png");
const ICON_OFF: &[u8] = include_bytes!("../icons/tray-off-template.png");

pub fn tooltip(on: bool) -> &'static str {
    if on {
        "usher"
    } else {
        "usher: auto-switch off"
    }
}

/// The menu-bar icon: Auto-switch toggle and a Quit item.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let auto = CheckMenuItem::with_id(app, "auto", "Auto-switch", true, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &auto,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit usher", true, None::<&str>)?,
        ],
    )?;
    tauri::tray::TrayIconBuilder::with_id("main")
        .tooltip(tooltip(true))
        // a black glyph the menu bar tints for light and dark (icons/tray-template.svg is the source)
        .icon(tauri::image::Image::from_bytes(ICON_ON)?)
        .icon_as_template(true)
        .menu(&menu)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "auto" => {
                // muda flipped the check; with no engine the setting cannot change, so flip it back to "on"
                if !app.state::<EngineHandle>().send(Event::ToggleAutoSwitch) {
                    set_auto(app, true);
                }
            }
            "quit" => {
                log::info!("quit from tray");
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    app.manage(auto);
    Ok(())
}

/// Icon, check mark and tooltip follow the engine's value (muda already flipped the check on click). Runs on the
/// main thread without waiting: the tray setters block on it, and the main thread may be waiting on the engine
/// in `RunEvent::Exit`.
pub fn set_auto(app: &AppHandle, on: bool) {
    let handle = app.clone();
    let queued = app.run_on_main_thread(move || {
        if let Some(item) = handle.try_state::<CheckMenuItem<tauri::Wry>>()
            && let Err(e) = item.set_checked(on)
        {
            warn!("auto-switch check mark not set ({e})");
        }
        let Some(tray) = handle.tray_by_id("main") else {
            return;
        };
        let bytes = if on { ICON_ON } else { ICON_OFF };
        match tauri::image::Image::from_bytes(bytes) {
            Ok(img) => {
                if let Err(e) = tray.set_icon(Some(img)) {
                    warn!("tray icon not set ({e})");
                }
                // set_icon resets the template flag
                if let Err(e) = tray.set_icon_as_template(true) {
                    warn!("tray template flag not set ({e})");
                }
            }
            Err(e) => warn!("tray glyph did not decode ({e})"),
        }
        if let Err(e) = tray.set_tooltip(Some(tooltip(on))) {
            warn!("tray tooltip not set ({e})");
        }
    });
    if let Err(e) = queued {
        warn!("tray update not queued ({e})");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_tray_glyphs_are_44x36() {
        for bytes in [ICON_ON, ICON_OFF] {
            let i = tauri::image::Image::from_bytes(bytes).unwrap();
            assert_eq!((i.width(), i.height()), (44, 36));
        }
    }
    #[test]
    fn tooltip_says_when_auto_switch_is_off() {
        assert_eq!(tooltip(true), "usher");
        assert_eq!(tooltip(false), "usher: auto-switch off");
    }
}
