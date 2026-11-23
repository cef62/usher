use tauri::menu::{Menu, MenuItem};

/// The menu-bar icon: tooltip and a Quit item.
pub fn build(app: &tauri::AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[&MenuItem::with_id(
            app,
            "quit",
            "Quit usher",
            true,
            None::<&str>,
        )?],
    )?;
    tauri::tray::TrayIconBuilder::with_id("main")
        .tooltip("usher")
        // a black glyph the menu bar tints for light and dark (icons/tray-template.svg is the source)
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray-template.png"
        ))?)
        .icon_as_template(true)
        .menu(&menu)
        .on_menu_event(|app, ev| {
            if ev.id().as_ref() == "quit" {
                log::info!("quit from tray");
                app.exit(0);
            }
        })
        .build(app)?;
    Ok(())
}
