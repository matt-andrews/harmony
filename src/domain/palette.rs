//! Fixed colour palette for auto-assigning project colours.
//!
//! Catppuccin Macchiato accents, ordered so that consecutive projects land on
//! visually distant hues. Mirrored in `web/src/components/ProjectsView.svelte`.

pub const PALETTE: [&str; 12] = [
    "#f5a97f", // peach
    "#8aadf4", // blue
    "#a6da95", // green
    "#f5bde6", // pink
    "#eed49f", // yellow
    "#c6a0f6", // mauve
    "#8bd5ca", // teal
    "#ed8796", // red
    "#91d7e3", // sky
    "#7dc4e4", // sapphire
    "#b7bdf8", // lavender
    "#f0c6c6", // flamingo
];

/// The palette used by version 1 documents, index-for-index with [`PALETTE`].
const LEGACY_PALETTE: [&str; 12] = [
    "#f97316", // orange
    "#3b82f6", // blue
    "#22c55e", // green
    "#ec4899", // pink
    "#eab308", // yellow
    "#a855f7", // purple
    "#14b8a6", // teal
    "#ef4444", // red
    "#06b6d4", // cyan
    "#84cc16", // lime
    "#8b5cf6", // violet
    "#f43f5e", // rose
];

/// Colour for the `n`th project ever created (0-based). Cycles when exhausted.
pub fn color_for_index(n: usize) -> &'static str {
    PALETTE[n % PALETTE.len()]
}

/// The current equivalent of a version 1 palette colour. `None` for anything
/// else, so hand-picked colours survive the migration.
pub fn migrate_color(color: &str) -> Option<&'static str> {
    LEGACY_PALETTE
        .iter()
        .position(|old| old.eq_ignore_ascii_case(color))
        .map(|i| PALETTE[i])
}

/// Validates a `#rrggbb` colour string.
pub fn is_valid_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_through_palette() {
        assert_eq!(color_for_index(0), PALETTE[0]);
        assert_eq!(color_for_index(12), PALETTE[0]);
        assert_eq!(color_for_index(13), PALETTE[1]);
    }

    #[test]
    fn migrates_only_legacy_colors() {
        assert_eq!(migrate_color("#f97316"), Some("#f5a97f"));
        assert_eq!(migrate_color("#F43F5E"), Some("#f0c6c6"));
        assert_eq!(migrate_color("#123456"), None);
        assert_eq!(migrate_color(PALETTE[0]), None);
    }

    #[test]
    fn validates_hex() {
        assert!(is_valid_hex_color("#a1B2c3"));
        assert!(!is_valid_hex_color("a1b2c3"));
        assert!(!is_valid_hex_color("#a1b2c"));
        assert!(!is_valid_hex_color("#a1b2cg"));
    }
}
