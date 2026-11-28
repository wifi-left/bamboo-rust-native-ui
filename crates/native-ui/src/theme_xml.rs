// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! `.buitheme` theme documents (M7.8, D150), feature `theme-xml`.
//!
//! The model, reader and canonical writer live in
//! `bamboo_native_ui_layout_core::theme` (no Win32); this module is the bridge
//! to the window crate's [`Palette`]. Applying a document is the application
//! swapping its palette and repainting — the layout schema's colour tokens then
//! resolve against the new palette:
//!
//! ```ignore
//! let document = ThemeDocument::parse(text)?;
//! let palette = document.palette(&settings);      // ThemeDocumentExt
//! window.set_theme(settings.clone(), palette.dark);
//! window.set_palette(palette);
//! ui.apply_layout(&layout, &palette, client, sink)?;
//! ```
//!
//! Derivation is exactly [`Palette::new`] followed by the document's overrides,
//! so a theme may change one token or the whole table. The optional `accent`
//! feeds [`Palette::new`] (the accent-derived tokens follow it); a
//! `<color name="accent">` then sets that one field explicitly.

use bamboo_native_ui_layout_core::color::Color;

use crate::theme::{Palette, ThemeSettings};

pub use bamboo_native_ui_layout_core::theme::{
    ThemeBase, ThemeColor, ThemeDiagnostics, ThemeDocument,
};

/// Map a theme document onto the window crate's palette.
pub trait ThemeDocumentExt {
    /// Build the palette for `settings`: the document's `base` decides
    /// light/dark, its `accent` (if any) replaces the settings accent before
    /// derivation, and every `<color>` override is applied in declaration
    /// order.
    fn palette(&self, settings: &ThemeSettings) -> Palette;
}

impl ThemeDocumentExt for ThemeDocument {
    fn palette(&self, settings: &ThemeSettings) -> Palette {
        let mut settings = settings.clone();
        if let Some(accent) = self.accent() {
            settings.accent = accent.to_rgb();
        }
        let mut palette = Palette::new(&settings, self.base().is_dark());
        for override_color in self.colors() {
            set_palette_token(&mut palette, override_color.name(), override_color.color());
        }
        palette
    }
}

/// Write one palette token by its kebab-case name (the `background` token
/// vocabulary). Returns whether the name is a token. Unknown names cannot come
/// from a parsed document (`theme.color.unknown`), but the function is total so
/// an application can also use it directly.
pub fn set_palette_token(palette: &mut Palette, name: &str, color: Color) -> bool {
    let slot = match name {
        "background" => &mut palette.background,
        "sidebar" => &mut palette.sidebar,
        "player" => &mut palette.player,
        "cover" => &mut palette.cover,
        "nav-active" => &mut palette.nav_active,
        "accent" => &mut palette.accent,
        "text" => &mut palette.text,
        "muted" => &mut palette.muted,
        "divider" => &mut palette.divider,
        "row-selected" => &mut palette.row_selected,
        "scroll-track" => &mut palette.scroll_track,
        "scroll-thumb" => &mut palette.scroll_thumb,
        "accent-text" => &mut palette.accent_text,
        "search-bg" => &mut palette.search_bg,
        "button-bg" => &mut palette.button_bg,
        "button-hover" => &mut palette.button_hover,
        "button-pressed" => &mut palette.button_pressed,
        "button-active" => &mut palette.button_active,
        "button-active-hover" => &mut palette.button_active_hover,
        "explicit-border" => &mut palette.explicit_border,
        "nav-hover" => &mut palette.nav_hover,
        "row-hover" => &mut palette.row_hover,
        "nav-pressed" => &mut palette.nav_pressed,
        "row-pressed" => &mut palette.row_pressed,
        "scroll-thumb-hover" => &mut palette.scroll_thumb_hover,
        "card" => &mut palette.card,
        "panel" => &mut palette.panel,
        "button-disabled" => &mut palette.button_disabled,
        "text-disabled" => &mut palette.text_disabled,
        "play-button" => &mut palette.play_button,
        "play-hover" => &mut palette.play_hover,
        _ => return false,
    };
    *slot = color;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const NIGHT: &str = r##"<theme version="1" name="夜蓝" base="dark" accent="#7aa2f7">
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
</theme>"##;

    fn settings() -> ThemeSettings {
        ThemeSettings::default()
    }

    #[test]
    fn the_base_decides_the_mode_and_the_defaults_derive_it() {
        let dark = ThemeDocument::parse(NIGHT).expect("parses");
        let light = ThemeDocument::parse(r#"<theme version="1" base="light"/>"#).expect("parses");
        let palette = dark.palette(&settings());
        assert!(palette.dark);
        assert!(!light.palette(&settings()).dark);
        // The document's base wins over the settings' own mode (there is none;
        // this is the whole point).
        assert_eq!(palette.background, Color::new(0x16, 0x18, 0x1d));
    }

    #[test]
    fn the_accent_feeds_derivation_and_overrides_win() {
        let document = ThemeDocument::parse(NIGHT).expect("parses");
        let palette = document.palette(&settings());
        assert_eq!(palette.accent, Color::new(0x7a, 0xa2, 0xf7));
        // accent_text is derived from the accent by Palette::new.
        assert_eq!(palette.accent_text, Palette::new(&accent_settings(&document), true).accent_text);
        assert_eq!(palette.text, Color::new(0xe8, 0xeb, 0xee));
        // A token that the document does not override keeps the derived value.
        assert_eq!(palette.sidebar, Palette::new(&accent_settings(&document), true).sidebar);

        // An explicit <color name="accent"> sets only that field, after
        // derivation.
        let explicit = ThemeDocument::parse(
            r##"<theme version="1" base="dark"><color name="accent" value="#ff0000"/></theme>"##,
        )
        .expect("parses");
        let palette = explicit.palette(&settings());
        assert_eq!(palette.accent, Color::new(255, 0, 0));
        assert_eq!(palette.accent_text, Palette::new(&settings(), true).accent_text);
    }

    fn accent_settings(document: &ThemeDocument) -> ThemeSettings {
        let mut settings = settings();
        if let Some(accent) = document.accent() {
            settings.accent = accent.to_rgb();
        }
        settings
    }

    #[test]
    fn set_palette_token_covers_every_token_and_rejects_unknown_names() {
        let document = ThemeDocument::parse(NIGHT).expect("parses");
        let mut palette = document.palette(&settings());
        for token in bamboo_native_ui_layout_core::PALETTE_TOKENS {
            assert!(
                set_palette_token(&mut palette, token, Color::new(1, 2, 3)),
                "`{token}` is a palette token"
            );
        }
        assert!(!set_palette_token(&mut palette, "nope", Color::new(1, 2, 3)));
    }
}
