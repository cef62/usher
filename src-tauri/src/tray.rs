use std::sync::atomic::{AtomicBool, Ordering};

use log::warn;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::engine::{EngineHandle, Event};

/// The auto-switch value the engine last reported; the tray falls back to it when the engine is gone.
pub struct LastAuto(pub AtomicBool);

/// Whether the active input is muted; the tray draws a dot while it is.
pub struct MicMuted(pub AtomicBool);

const ICON_ON: &[u8] = include_bytes!("../icons/tray-template.png");
const ICON_OFF: &[u8] = include_bytes!("../icons/tray-off-template.png");
const ICON_ON_MUTED: &[u8] = include_bytes!("../icons/tray-muted-template.png");
const ICON_OFF_MUTED: &[u8] = include_bytes!("../icons/tray-off-muted-template.png");

pub fn icon(auto: bool, muted: bool) -> &'static [u8] {
    match (auto, muted) {
        (true, false) => ICON_ON,
        (false, false) => ICON_OFF,
        (true, true) => ICON_ON_MUTED,
        (false, true) => ICON_OFF_MUTED,
    }
}

pub fn tooltip(auto: bool, muted: bool) -> &'static str {
    match (auto, muted) {
        (true, false) => "usher",
        (false, false) => "usher: auto-switch off",
        (true, true) => "usher: microphone muted",
        (false, true) => "usher: auto-switch off, microphone muted",
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
            &MenuItem::with_id(app, "activity", "Activity…", true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?,
            &updates,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit usher", true, None::<&str>)?,
        ],
    )?;
    Ok((menu, auto, updates))
}

/// The menu-bar icon: left click opens the panel, right click shows Auto-switch, Activity…, Settings…, Check for Updates… and Quit.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let (menu, auto, updates) = build_menu(app)?;
    app.manage(LastAuto(AtomicBool::new(true)));
    app.manage(MicMuted(AtomicBool::new(false)));
    tauri::tray::TrayIconBuilder::with_id("main")
        .tooltip(tooltip(true, false))
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
        "activity" => crate::activity::open(app),
        "settings" => crate::settings::open(app),
        "updates" => crate::update::install::tray_clicked(app),
        "quit" => {
            log::info!("quit from tray");
            app.exit(0);
        }
        _ => {}
    }
}

/// Icon, check mark and tooltip follow the engine's value (muda already flipped the check on click).
pub fn set_auto(app: &AppHandle, on: bool) {
    if let Some(last) = app.try_state::<LastAuto>() {
        last.0.store(on, Ordering::Relaxed);
    }
    redraw(app);
}

/// Dot on the icon while the active input is muted; does nothing unless the bit flips.
pub fn set_mic_muted(app: &AppHandle, muted: bool) {
    if let Some(m) = app.try_state::<MicMuted>()
        && m.0.swap(muted, Ordering::Relaxed) != muted
    {
        redraw(app);
    }
}

/// Runs on the main thread without waiting: the tray setters block on it, and the main thread may be waiting on
/// the engine in `RunEvent::Exit`. Reads both bits when it runs, so an auto toggle keeps the dot and a mute
/// change keeps the off glyph.
fn redraw(app: &AppHandle) {
    let handle = app.clone();
    let queued = app.run_on_main_thread(move || {
        let on = handle
            .try_state::<LastAuto>()
            .is_none_or(|s| s.0.load(Ordering::Relaxed));
        let muted = handle
            .try_state::<MicMuted>()
            .is_some_and(|s| s.0.load(Ordering::Relaxed));
        if let Some(item) = handle.try_state::<CheckMenuItem<tauri::Wry>>()
            && let Err(e) = item.set_checked(on)
        {
            warn!("auto-switch check mark not set ({e})");
        }
        let Some(tray) = handle.tray_by_id("main") else {
            return;
        };
        match tauri::image::Image::from_bytes(icon(on, muted)) {
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
        if let Err(e) = tray.set_tooltip(Some(tooltip(on, muted))) {
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
    const PAIRS: [(bool, bool); 4] = [(true, false), (false, false), (true, true), (false, true)];
    #[test]
    fn all_four_tray_glyphs_are_44x36() {
        for (a, m) in PAIRS {
            let i = tauri::image::Image::from_bytes(icon(a, m)).unwrap();
            assert_eq!((i.width(), i.height()), (44, 36));
        }
    }
    #[test]
    fn tooltip_names_each_state() {
        assert_eq!(tooltip(true, false), "usher");
        assert_eq!(tooltip(false, false), "usher: auto-switch off");
        assert_eq!(tooltip(true, true), "usher: microphone muted");
        assert_eq!(
            tooltip(false, true),
            "usher: auto-switch off, microphone muted"
        );
    }
    #[test]
    fn each_state_has_its_own_glyph() {
        for (i, a) in PAIRS.iter().enumerate() {
            for b in &PAIRS[i + 1..] {
                assert_ne!(icon(a.0, a.1), icon(b.0, b.1));
            }
        }
    }
}
