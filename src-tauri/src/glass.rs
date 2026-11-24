//! The translucent material behind the panel, Settings and What's new.

use tauri::WebviewWindow;

/// A titled window's corner on macOS 26.
pub const WINDOW_RADIUS: f64 = 16.0;

/// Liquid Glass on macOS 26+; the classic materials render near-opaque there. Older macOS gets sidebar vibrancy.
/// The window must be transparent. Settings and What's new use it too, with `WINDOW_RADIUS`.
pub fn apply(w: &WebviewWindow, radius: f64) {
    use window_vibrancy::{
        LiquidGlassOptions, NSGlassEffectViewStyle, NSVisualEffectMaterial, NSVisualEffectState,
        apply_liquid_glass, apply_vibrancy,
    };
    let opts = LiquidGlassOptions::new(NSGlassEffectViewStyle::Regular).radius(radius);
    if let Err(e) = apply_liquid_glass(w, opts) {
        log::debug!("liquid glass unavailable ({e}); using vibrancy");
        let state = Some(NSVisualEffectState::Active);
        if let Err(e) = apply_vibrancy(w, NSVisualEffectMaterial::Sidebar, state, Some(radius)) {
            log::warn!("vibrancy failed: {e}");
        }
    }
}
