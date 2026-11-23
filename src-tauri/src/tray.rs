use std::sync::atomic::{AtomicBool, Ordering};

use log::warn;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::engine::{EngineHandle, Event};

/// The auto-switch value the engine last reported; the tray falls back to it when the engine is gone.
pub struct LastAuto(pub AtomicBool);

const ICON_ON: &[u8] = include_bytes!("../icons/tray-template.png");
const ICON_OFF: &[u8] = include_bytes!("../icons/tray-off-template.png");

pub fn tooltip(on: bool) -> &'static str {
    if on {
        "usher"
    } else {
        "usher: auto-switch off"
    }
}

fn build_menu(
    app: &AppHandle,
) -> tauri::Result<(
    Menu<tauri::Wry>,
    CheckMenuItem<tauri::Wry>,
    MenuItem<tauri::Wry>,
)> {
    let auto = CheckMenuItem::with_id(app, "auto", "Auto-switch", true, true, None::<&str>)?;
    let updates = MenuItem::with_id(
        app,
        "updates",
        crate::update::install::tray_text(None),
        true,
        None::<&str>,
    )?;
    let menu = Menu::with_items(
        app,
        &[
            &auto,
            &MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?,
            &updates,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit usher", true, None::<&str>)?,
        ],
    )?;
    Ok((menu, auto, updates))
}

/// The menu-bar icon: left click opens the panel, right click shows Auto-switch, Settings… and Quit.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let (menu, auto, updates) = build_menu(app)?;
    app.manage(LastAuto(AtomicBool::new(true)));
    tauri::tray::TrayIconBuilder::with_id("main")
        .tooltip(tooltip(true))
        // a black glyph the menu bar tints for light and dark (icons/tray-template.svg is the source)
        .icon(tauri::image::Image::from_bytes(ICON_ON)?)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = ev
            {
                crate::panel::tray_click(tray.app_handle(), rect);
            }
        })
        .on_menu_event(|app, ev| on_menu(app, ev.id().as_ref()))
        .build(app)?;
    app.manage(auto);
    app.manage(crate::update::install::TrayItem(updates));
    Ok(())
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "auto" => {
            // muda flipped the check; with no engine the setting cannot change, so restore the last value
            if !app.state::<EngineHandle>().send(Event::ToggleAutoSwitch) {
                warn!("engine is gone; auto-switch unchanged");
                set_auto(app, app.state::<LastAuto>().0.load(Ordering::Relaxed));
            }
        }
        "settings" => crate::settings::open(app),
        "updates" => crate::update::install::tray_clicked(app),
        "quit" => {
            log::info!("quit from tray");
            app.exit(0);
        }
        _ => {}
    }
}

/// Icon, check mark and tooltip follow the engine's value (muda already flipped the check on click). Runs on the
/// main thread without waiting: the tray setters block on it, and the main thread may be waiting on the engine
/// in `RunEvent::Exit`.
pub fn set_auto(app: &AppHandle, on: bool) {
    if let Some(last) = app.try_state::<LastAuto>() {
        last.0.store(on, Ordering::Relaxed);
    }
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
