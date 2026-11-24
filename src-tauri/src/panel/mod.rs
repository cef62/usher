//! The panel window: a 360 pt window anchored under the tray icon. `Visibility` decides, the glue performs.

pub mod place;
pub use place::other_window_visible;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use log::warn;
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, WebviewWindow};

pub const LABEL: &str = "panel";
pub const WIDTH: f64 = 360.0;
/// A tray click this soon after a blur-hide is the same click that blurred the panel: it must not reopen it.
pub const REOPEN_GUARD: Duration = Duration::from_millis(300);

#[derive(Debug, PartialEq)]
pub enum Step {
    Show,
    Hide,
    Stay,
}

/// The panel's own idea of whether it is up: `is_visible` lags behind a queued show.
#[derive(Default)]
pub struct Visibility {
    shown: bool,
    /// Focus arrived since the last show; a blur before that is stale and must not hide a panel still coming up.
    focused: bool,
    blur_hidden_at: Option<Instant>,
}

impl Visibility {
    pub fn tray_click(&mut self, now: Instant) -> Step {
        if self.shown {
            return self.hide();
        }
        if self
            .blur_hidden_at
            .is_some_and(|t| now.saturating_duration_since(t) < REOPEN_GUARD)
        {
            return Step::Stay;
        }
        *self = Visibility {
            shown: true,
            ..Visibility::default()
        };
        Step::Show
    }

    pub fn hide(&mut self) -> Step {
        if !self.shown {
            return Step::Stay;
        }
        self.shown = false;
        Step::Hide
    }

    pub fn focus(&mut self, focused: bool, now: Instant) -> Step {
        if !self.shown {
            return Step::Stay;
        }
        if focused {
            self.focused = true;
            return Step::Stay;
        }
        if !self.focused {
            return Step::Stay;
        }
        self.blur_hidden_at = Some(now);
        self.hide()
    }
}

#[derive(Debug, PartialEq)]
pub enum GiveBack {
    /// Hide the whole app: macOS refocuses the app that was active before.
    HideApp,
    /// Settings or What's new is open and hiding the app would hide it too.
    Stay,
}

/// `opening_window`: Settings or What's new takes the focus next, so the previous app must not flash active.
pub fn give_back(other_visible: bool, opening_window: bool) -> GiveBack {
    if other_visible || opening_window {
        GiveBack::Stay
    } else {
        GiveBack::HideApp
    }
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

pub fn setup(app: &AppHandle) {
    app.manage(Mutex::new(Visibility::default()));
    if let Some(w) = app.get_webview_window(LABEL) {
        crate::glass::apply(&w, RADIUS);
    }
}

const RADIUS: f64 = 10.0;

/// Physical monitor frames with their scales. The list is empty while a display sleeps: fall back to the primary.
fn monitors(app: &AppHandle) -> Option<Vec<(place::Frame, f64)>> {
    let primary = app.primary_monitor().ok().flatten();
    let ms = app
        .available_monitors()
        .ok()
        .filter(|m| !m.is_empty())
        .or(primary.map(|p| vec![p]))?;
    let ms: Vec<_> = ms
        .iter()
        .map(|m| {
            let (p, s) = (m.position(), m.size());
            (
                place::Frame {
                    x: f64::from(p.x),
                    y: f64::from(p.y),
                    w: f64::from(s.width),
                    h: f64::from(s.height),
                },
                m.scale_factor(),
            )
        })
        .collect();
    Some(ms)
}

/// Top-left in points under the tray icon; `None` without a monitor. The list is empty while a display sleeps.
fn position(app: &AppHandle, icon: tauri::Rect) -> Option<LogicalPosition<f64>> {
    let ms = monitors(app)?;
    // tao reports the cursor as points times the primary scale
    let cursor = app
        .primary_monitor()
        .ok()
        .flatten()
        .zip(app.cursor_position().ok())
        .map(|(m, c)| (c.x / m.scale_factor(), c.y / m.scale_factor()));
    let (p, s) = (
        icon.position.to_physical::<f64>(1.0),
        icon.size.to_physical::<f64>(1.0),
    );
    let (icon, screen) = place::to_points(
        place::Frame {
            x: p.x,
            y: p.y,
            w: s.width,
            h: s.height,
        },
        cursor,
        &ms,
    )?;
    let (x, y) = place::anchor(icon, screen, WIDTH);
    Some(LogicalPosition::new(x, y))
}

fn apply(
    app: &AppHandle,
    icon: Option<tauri::Rect>,
    opening_window: bool,
    step: impl FnOnce(&mut Visibility) -> Step,
) {
    let step = step(
        &mut app
            .state::<Mutex<Visibility>>()
            .lock()
            .unwrap_or_else(|p| p.into_inner()),
    );
    let Some(w) = window(app) else { return };
    match step {
        Step::Show => {
            // positioned before it is shown, so it never flashes at its last spot
            if let Some(pos) = icon.and_then(|r| position(app, r)) {
                w.set_position(pos).ok();
            }
            app.show().ok(); // undo the app.hide() of the last hide
            w.show().ok();
            w.set_focus().ok();
            w.emit("panel-shown", ()).ok();
        }
        Step::Hide => {
            w.hide().ok();
            let shown = |l: &str| {
                app.get_webview_window(l)
                    .is_some_and(|w| w.is_visible().unwrap_or(false))
            };
            if give_back(other_window_visible(shown), opening_window) == GiveBack::HideApp
                && let Err(e) = app.hide()
            {
                warn!("app not hidden ({e})");
            }
        }
        Step::Stay => {}
    }
}

/// Left click on the menu-bar icon; `icon` is the icon's rect.
pub fn tray_click(app: &AppHandle, icon: tauri::Rect) {
    apply(app, Some(icon), false, |v| v.tray_click(Instant::now()));
}

/// `WindowEvent::Focused` of the panel: clicking away hides it.
pub fn focus_changed(app: &AppHandle, focused: bool) {
    apply(app, None, false, |v| v.focus(focused, Instant::now()));
}

pub fn hide(app: &AppHandle) {
    apply(app, None, false, Visibility::hide);
}

/// Hide for Settings or What's new: the app stays active, so the previous app never flashes.
pub fn hide_for_window(app: &AppHandle) {
    apply(app, None, true, Visibility::hide);
}

#[tauri::command]
pub fn hide_panel(app: AppHandle) {
    // window calls belong on the main thread
    let a = app.clone();
    if let Err(e) = app.run_on_main_thread(move || hide(&a)) {
        warn!("panel hide not queued ({e})");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    fn at(ms: u64) -> Instant {
        static BASE: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        *BASE.get_or_init(Instant::now) + Duration::from_millis(ms)
    }
    #[test]
    fn tray_click_toggles() {
        let mut v = Visibility::default();
        assert_eq!(v.tray_click(at(0)), Step::Show);
        assert_eq!(v.tray_click(at(1_000)), Step::Hide);
    }
    #[test]
    fn blur_after_focus_hides_and_stale_blur_does_not() {
        let mut v = Visibility::default();
        v.tray_click(at(0));
        assert_eq!(v.focus(false, at(10)), Step::Stay);
        v.focus(true, at(20));
        assert_eq!(v.focus(false, at(30)), Step::Hide);
    }
    fn blur_hidden() -> Visibility {
        let mut v = Visibility::default();
        v.tray_click(at(0));
        v.focus(true, at(10));
        assert_eq!(v.focus(false, at(500)), Step::Hide);
        v
    }
    #[test]
    fn tray_click_right_after_blur_hide_stays_hidden() {
        assert_eq!(blur_hidden().tray_click(at(799)), Step::Stay);
    }
    #[test]
    fn tray_click_after_the_guard_shows_again() {
        assert_eq!(blur_hidden().tray_click(at(800)), Step::Show);
    }
    #[test]
    fn hide_is_idempotent_and_a_new_show_needs_a_new_focus() {
        let mut v = Visibility::default();
        assert_eq!(v.hide(), Step::Stay);
        v.tray_click(at(0));
        v.focus(true, at(1));
        assert_eq!(v.hide(), Step::Hide);
        assert_eq!(v.hide(), Step::Stay);
        assert_eq!(v.tray_click(at(2_000)), Step::Show);
        assert_eq!(v.focus(false, at(2_001)), Step::Stay);
    }
    #[test]
    fn give_back_leaves_focus_when_another_window_is_open() {
        assert_eq!(give_back(false, false), GiveBack::HideApp);
        assert_eq!(give_back(true, false), GiveBack::Stay);
        assert_eq!(give_back(false, true), GiveBack::Stay);
    }
}
