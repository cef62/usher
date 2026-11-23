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

/// `monitors` are physical frames with their scale. Returns the icon and its screen in points. The screen is the
/// first monitor whose point frame holds the icon's centre at that monitor's scale, else the first; `None` for no
/// monitors. tao on macOS reports the tray rect and each monitor frame in pixels of the monitor they sit on, so the
/// comparison happens per monitor, in points.
pub fn to_points(icon_px: Frame, monitors: &[(Frame, f64)]) -> Option<(Frame, Frame)> {
    let holds = |&&(m, k): &&(Frame, f64)| {
        let (s, i) = (scaled(m, k), scaled(icon_px, k));
        let (cx, cy) = (i.x + i.w / 2.0, i.y + i.h / 2.0);
        cx >= s.x && cx < s.x + s.w && cy >= s.y && cy < s.y + s.h
    };
    let &(m, k) = monitors.iter().find(holds).or(monitors.first())?;
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
        let (icon, screen) =
            to_points(f(2000., 0., 60., 48.), &[(f(0., 0., 3024., 1964.), 2.)]).unwrap();
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
        let (icon, screen) = to_points(f(3400., 0., 30., 24.), &ms).unwrap();
        assert_eq!(screen, f(1512., 0., 1920., 1080.));
        assert_eq!(icon.x, 3400.);
    }
    #[test]
    fn no_monitor_is_none_and_off_screen_falls_back_to_first() {
        assert!(to_points(f(0., 0., 1., 1.), &[]).is_none());
        assert_eq!(
            to_points(f(-9000., 0., 30., 24.), &[(f(0., 0., 3024., 1964.), 2.)])
                .unwrap()
                .1,
            f(0., 0., 1512., 982.)
        );
    }
}
