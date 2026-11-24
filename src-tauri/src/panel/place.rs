//! Panel placement: tray icon rect and monitors in, window position out. Pure math, all in points.

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Space between the icon's bottom edge and the panel.
pub const GAP: f64 = 4.0;
/// Minimum distance from the screen's left and right edges.
pub const MARGIN: f64 = 8.0;

fn scaled(f: Frame, k: f64) -> Frame {
    Frame {
        x: f.x / k,
        y: f.y / k,
        w: f.w / k,
        h: f.h / k,
    }
}

/// `monitors` are physical frames with their scale. Returns the icon and its screen in points; `None` for no monitors.
///
/// The tray rect is points times the status item's scale and each monitor frame is points times its own scale, so a
/// pixel value alone does not say which monitor holds the icon. `cursor_pt` (the click's global point: tao's cursor
/// divided by the primary scale) picks the monitor whose point frame holds it. Without a cursor, or off every
/// monitor, the first monitor whose point frame holds the icon's centre at that monitor's scale wins, else the first.
pub fn to_points(
    icon_px: Frame,
    cursor_pt: Option<(f64, f64)>,
    monitors: &[(Frame, f64)],
) -> Option<(Frame, Frame)> {
    let inside =
        |s: Frame, (x, y): (f64, f64)| x >= s.x && x < s.x + s.w && y >= s.y && y < s.y + s.h;
    let by_cursor =
        cursor_pt.and_then(|c| monitors.iter().find(|&&(m, k)| inside(scaled(m, k), c)));
    let by_icon = || {
        monitors.iter().find(|&&(m, k)| {
            let i = scaled(icon_px, k);
            inside(scaled(m, k), (i.x + i.w / 2.0, i.y + i.h / 2.0))
        })
    };
    let &(m, k) = by_cursor.or_else(by_icon).or(monitors.first())?;
    Some((scaled(icon_px, k), scaled(m, k)))
}

/// Top-left of the panel in points: centred under the icon, kept `MARGIN` inside the screen.
pub fn anchor(icon: Frame, screen: Frame, width: f64) -> (f64, f64) {
    let lo = screen.x + MARGIN;
    let hi = (screen.x + screen.w - width - MARGIN).max(lo);
    (
        (icon.x + icon.w / 2.0 - width / 2.0).clamp(lo, hi),
        icon.y + icon.h + GAP,
    )
}

/// Windows that keep the app shown when the panel hides.
pub const KEEP_FOCUS: [&str; 2] = [crate::settings::LABEL, crate::whatsnew::LABEL];

pub fn other_window_visible(visible: impl Fn(&str) -> bool) -> bool {
    KEEP_FOCUS.iter().any(|l| visible(l))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn f(x: f64, y: f64, w: f64, h: f64) -> Frame {
        Frame { x, y, w, h }
    }
    #[test]
    fn anchor_centres_under_the_icon() {
        assert_eq!(
            anchor(f(1000., 0., 30., 24.), f(0., 0., 1512., 982.), 360.),
            (835., 28.)
        );
    }
    #[test]
    fn anchor_clamps_to_the_right_edge() {
        assert_eq!(
            anchor(f(1490., 0., 20., 24.), f(0., 0., 1512., 982.), 360.),
            (1144., 28.)
        );
    }
    #[test]
    fn anchor_clamps_to_the_left_edge() {
        assert_eq!(
            anchor(f(2., 0., 20., 24.), f(0., 0., 1512., 982.), 360.).0,
            8.
        );
    }
    #[test]
    fn icon_on_a_2x_primary() {
        let (icon, screen) = to_points(
            f(2000., 0., 60., 48.),
            None,
            &[(f(0., 0., 3024., 1964.), 2.)],
        )
        .unwrap();
        assert_eq!(
            (icon, screen),
            (f(1000., 0., 30., 24.), f(0., 0., 1512., 982.))
        );
    }
    #[test]
    fn icon_on_a_1x_display_right_of_a_2x_primary() {
        let ms = [
            (f(0., 0., 3024., 1964.), 2.),
            (f(1512., 0., 1920., 1080.), 1.),
        ];
        // 3400 px is 1700 pt on the 2x primary (outside it), so only the 1x display holds it
        let (icon, screen) = to_points(f(3400., 0., 30., 24.), None, &ms).unwrap();
        assert_eq!(screen, f(1512., 0., 1920., 1080.));
        assert_eq!(icon.x, 3400.);
    }
    #[test]
    fn no_monitor_is_none_and_off_screen_falls_back_to_first() {
        assert!(to_points(f(0., 0., 1., 1.), None, &[]).is_none());
        assert_eq!(
            to_points(
                f(-9000., 0., 30., 24.),
                None,
                &[(f(0., 0., 3024., 1964.), 2.)]
            )
            .unwrap()
            .1,
            f(0., 0., 1512., 982.)
        );
    }
    #[test]
    fn cursor_picks_the_screen_where_pixels_are_ambiguous() {
        // 2900 px is 1450 pt on the 2x primary (inside it) and 2900 pt on the 1x secondary; the cursor decides
        let ms = [
            (f(0., 0., 3024., 1964.), 2.),
            (f(1512., 0., 1440., 900.), 1.),
        ];
        let (icon, screen) = to_points(f(2900., 0., 30., 24.), Some((2910., 12.)), &ms).unwrap();
        assert_eq!(screen, f(1512., 0., 1440., 900.));
        assert_eq!(icon.x, 2900.);
        // cursor on the primary: same pixels, other screen
        let (icon, screen) = to_points(f(2900., 0., 30., 24.), Some((1460., 12.)), &ms).unwrap();
        assert_eq!((icon.x, screen), (1450., f(0., 0., 1512., 982.)));
    }
    #[test]
    fn other_window_visible_counts_settings_and_whatsnew() {
        assert!(!other_window_visible(|_| false));
        assert!(other_window_visible(|l| l == "settings"));
        assert!(other_window_visible(|l| l == "whatsnew"));
        assert!(!other_window_visible(|l| l == "panel"));
    }
}
