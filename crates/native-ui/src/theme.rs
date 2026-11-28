// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Runtime theme: palette, accent presets, density and font settings.
//!
//! The palette fields deliberately mirror the player's historical
//! `src/theme.rs` palette one-to-one so the main window can switch to this
//! crate without touching its page painters.

use crate::color::Color;

pub use bamboo_native_ui_layout_core::radius;

/// Accent presets offered by settings pages. The first entry is the historical
/// Bamboo green.
pub const ACCENT_PRESETS: [(&str, [u8; 3]); 6] = [
    ("竹绿", [127, 211, 168]),
    ("海蓝", [104, 140, 255]),
    ("紫罗兰", [167, 139, 250]),
    ("珊瑚", [251, 146, 120]),
    ("琥珀", [245, 190, 90]),
    ("青碧", [120, 200, 210]),
];

/// Labels for the segmented accent picker (same order as [`ACCENT_PRESETS`]).
pub const ACCENT_NAMES: [&str; 6] = ["竹绿", "海蓝", "紫罗兰", "珊瑚", "琥珀", "青碧"];

/// List row density.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListDensity {
    Compact,
    #[default]
    Comfortable,
    Spacious,
}

/// Theme inputs the application can change at runtime.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeSettings {
    pub accent: [u8; 3],
    pub font_family: Option<String>,
    /// UI base size in DIPs; 13.5 is the reference.
    pub font_size: f32,
    pub list_density: ListDensity,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            accent: ACCENT_PRESETS[0].1,
            font_family: Some("Microsoft YaHei UI".to_string()),
            font_size: 13.5,
            list_density: ListDensity::Comfortable,
        }
    }
}

impl ThemeSettings {
    /// Font scale relative to the 13.5 DIP reference, clamped like the player.
    pub fn font_scale(&self) -> f32 {
        (self.font_size / 13.5).clamp(0.85, 1.4)
    }

    /// List row height in DIPs for the current density.
    pub fn row_height(&self) -> f32 {
        match self.list_density {
            ListDensity::Compact => 34.0,
            ListDensity::Comfortable => 40.0,
            ListDensity::Spacious => 48.0,
        }
    }
}

/// Every color a control may use. Mirrors the player palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub dark: bool,
    pub background: Color,
    pub sidebar: Color,
    pub player: Color,
    pub cover: Color,
    pub nav_active: Color,
    pub accent: Color,
    pub text: Color,
    pub muted: Color,
    pub divider: Color,
    pub row_selected: Color,
    pub scroll_track: Color,
    pub scroll_thumb: Color,
    pub accent_text: Color,
    pub search_bg: Color,
    pub button_bg: Color,
    pub button_hover: Color,
    pub button_pressed: Color,
    pub button_active: Color,
    pub button_active_hover: Color,
    pub explicit_border: Color,
    pub nav_hover: Color,
    pub row_hover: Color,
    pub nav_pressed: Color,
    pub row_pressed: Color,
    pub scroll_thumb_hover: Color,
    pub card: Color,
    pub panel: Color,
    pub button_disabled: Color,
    pub text_disabled: Color,
    pub play_button: Color,
    pub play_hover: Color,
}

impl Palette {
    /// Build the palette for `settings`; `dark` is the resolved light/dark
    /// decision (`follow system` is resolved by the caller).
    pub fn new(settings: &ThemeSettings, dark: bool) -> Self {
        let accent = Color::rgb(settings.accent);
        let mix = |base: Color, t: f32| base.mix(accent, t);
        if dark {
            let background = Color::new(27, 28, 31);
            let sidebar = Color::new(34, 36, 40);
            Self {
                dark,
                background,
                sidebar,
                player: Color::new(37, 39, 43),
                cover: Color::new(48, 51, 56),
                nav_active: mix(sidebar, 0.28),
                accent,
                text: Color::new(232, 235, 238),
                muted: Color::new(158, 166, 176),
                divider: Color::new(51, 54, 59),
                row_selected: mix(background, 0.17),
                scroll_track: Color::new(41, 43, 48),
                scroll_thumb: Color::new(92, 98, 106),
                accent_text: accent.lighten(0.22),
                search_bg: Color::new(22, 23, 26),
                button_bg: Color::new(45, 48, 53),
                button_hover: Color::new(58, 62, 69),
                button_pressed: mix(background, 0.33),
                button_active: mix(background, 0.24),
                button_active_hover: mix(background, 0.38),
                explicit_border: Color::new(70, 75, 83),
                nav_hover: Color::new(40, 43, 48),
                row_hover: Color::new(37, 40, 45),
                nav_pressed: mix(background, 0.30),
                row_pressed: mix(background, 0.26),
                scroll_thumb_hover: Color::new(126, 134, 144),
                card: Color::new(33, 35, 40),
                panel: Color::new(29, 31, 35),
                button_disabled: Color::new(38, 40, 44),
                text_disabled: Color::new(108, 114, 122),
                play_button: mix(background, 0.21),
                play_hover: mix(background, 0.33),
            }
        } else {
            let background = Color::new(244, 245, 247);
            let sidebar = Color::new(235, 237, 240);
            Self {
                dark,
                background,
                sidebar,
                player: Color::new(250, 250, 252),
                cover: Color::new(224, 227, 232),
                nav_active: mix(sidebar, 0.26),
                accent,
                text: Color::new(32, 35, 40),
                muted: Color::new(98, 106, 116),
                divider: Color::new(206, 210, 217),
                row_selected: Color::new(255, 255, 255).mix(accent, 0.18),
                scroll_track: Color::new(229, 231, 236),
                scroll_thumb: Color::new(168, 175, 184),
                accent_text: accent.darken(0.35),
                search_bg: Color::new(255, 255, 255),
                button_bg: Color::new(232, 235, 239),
                button_hover: Color::new(220, 224, 230),
                button_pressed: Color::new(255, 255, 255).mix(accent, 0.34),
                button_active: Color::new(255, 255, 255).mix(accent, 0.24),
                button_active_hover: Color::new(255, 255, 255).mix(accent, 0.36),
                explicit_border: Color::new(196, 201, 209),
                nav_hover: Color::new(226, 230, 235),
                row_hover: Color::new(233, 236, 240),
                nav_pressed: Color::new(255, 255, 255).mix(accent, 0.30),
                row_pressed: Color::new(255, 255, 255).mix(accent, 0.26),
                scroll_thumb_hover: Color::new(120, 128, 138),
                card: Color::new(255, 255, 255),
                panel: Color::new(248, 249, 251),
                button_disabled: Color::new(238, 240, 243),
                text_disabled: Color::new(158, 164, 173),
                play_button: Color::new(255, 255, 255).mix(accent, 0.22),
                play_hover: Color::new(255, 255, 255).mix(accent, 0.34),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_presets_include_the_default_green() {
        assert_eq!(ACCENT_PRESETS[0].1, ThemeSettings::default().accent);
    }

    #[test]
    fn light_palette_has_dark_text_and_light_background() {
        let settings = ThemeSettings {
            accent: ACCENT_PRESETS[1].1,
            ..ThemeSettings::default()
        };
        let light = Palette::new(&settings, false);
        let dark = Palette::new(&settings, true);
        assert!(light.background.r > 0xc0);
        assert!(light.text.r < 0x60);
        assert!(dark.background.r < 0x40);
        assert!(dark.text.r > 0xc0);
        assert_ne!(light.accent_text, dark.accent_text);
    }

    #[test]
    fn dark_and_light_palettes_agree_on_structure() {
        let settings = ThemeSettings::default();
        let dark = Palette::new(&settings, true);
        let light = Palette::new(&settings, false);
        assert!(dark.dark);
        assert!(!light.dark);
        assert_eq!(dark.accent, light.accent);
    }

    #[test]
    fn row_height_tracks_density() {
        let mut settings = ThemeSettings::default();
        assert_eq!(settings.row_height(), 40.0);
        settings.list_density = ListDensity::Compact;
        assert_eq!(settings.row_height(), 34.0);
        settings.list_density = ListDensity::Spacious;
        assert_eq!(settings.row_height(), 48.0);
    }
}
