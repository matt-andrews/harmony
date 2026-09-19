//! Where the floating compact window goes. Pure geometry in physical pixels, so
//! it can be tested without a window.

/// Logical size of the compact strip.
pub const COMPACT_SIZE: (f64, f64) = (360.0, 72.0);

/// Gap kept from the monitor's right edge, and from its bottom edge (tall
/// enough to clear a taskbar, which tao can't measure). Logical pixels.
const MARGIN_RIGHT: f64 = 24.0;
const MARGIN_BOTTOM: f64 = 72.0;

/// A monitor's bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Monitor {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

impl Monitor {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width as i32 && y < self.y + self.height as i32
    }
}

/// The remembered position, if the strip would still be reachable there (its
/// centre is on some monitor); otherwise the bottom-right corner of `current`.
pub fn place(saved: Option<(i32, i32)>, size: (u32, u32), current: Monitor, all: &[Monitor]) -> (i32, i32) {
    let (w, h) = (size.0 as i32, size.1 as i32);
    if let Some((x, y)) = saved
        && all.iter().any(|m| m.contains(x + w / 2, y + h / 2))
    {
        return (x, y);
    }
    let right = (MARGIN_RIGHT * current.scale).round() as i32;
    let bottom = (MARGIN_BOTTOM * current.scale).round() as i32;
    (
        current.x + current.width as i32 - w - right,
        current.y + current.height as i32 - h - bottom,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: Monitor = Monitor { x: 0, y: 0, width: 1920, height: 1080, scale: 1.0 };
    const LEFT: Monitor = Monitor { x: -2560, y: 0, width: 2560, height: 1440, scale: 1.5 };

    #[test]
    fn defaults_to_bottom_right_clear_of_the_taskbar() {
        assert_eq!(place(None, (360, 72), MAIN, &[MAIN]), (1920 - 360 - 24, 1080 - 72 - 72));
        // Margins scale with the monitor.
        assert_eq!(place(None, (540, 108), LEFT, &[MAIN, LEFT]), (-540 - 36, 1440 - 108 - 108));
    }

    #[test]
    fn keeps_a_saved_position_that_is_still_on_a_monitor() {
        assert_eq!(place(Some((-2000, 50)), (360, 72), MAIN, &[MAIN, LEFT]), (-2000, 50));
    }

    #[test]
    fn forgets_a_saved_position_on_an_unplugged_monitor() {
        assert_eq!(place(Some((-2000, 50)), (360, 72), MAIN, &[MAIN]), (1536, 936));
    }
}
