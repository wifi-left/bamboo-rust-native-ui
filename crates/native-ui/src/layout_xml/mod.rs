// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! M7 — the layout skin (`.buixml`), behind `feature = "layout-xml"`.
//!
//! A skin is **not** a constructor: it assigns rectangles, runtime properties,
//! style tokens and static content initial values to widgets the Rust side
//! already created. A document can never create, remove, reorder or reparent a
//! widget, so a hot reload keeps every piece of live state it does not
//! explicitly own.
//!
//! The file uses XML syntax with a dedicated `.buixml` extension so it is never
//! confused with generic XML data files. The schema contract is
//! `docs/M7-XML.md`; the decisions are D123–D130 and D137.
//!
//! The model, parser and canonical writer live in
//! `bamboo-native-ui-layout-core` (shared with the compiled mode, D128); this
//! module re-exports them and adds the applier, which needs the live
//! [`crate::Ui`].
//!
//! - [`LayoutDocument::parse`] — pure parse and validation.
//! - [`crate::Ui::apply_layout`] — atomic apply to the live widget tree.
//! - [`crate::Ui::clear_layout`] — hand every claimed field back.

mod apply;
#[cfg(feature = "layout-compiled")]
pub mod compiled;

pub(crate) use bamboo_native_ui_layout_core::{diag, model};
#[cfg(feature = "layout-xml")]
pub(crate) use bamboo_native_ui_layout_core::expr;
pub use bamboo_native_ui_layout_core::{
    EntrySpace, LayoutConstants, LayoutDiagnostic, LayoutDocument, LayoutEntry, LayoutSeverity,
    LayoutSpace, LayoutVariant, ScrollEntry,
};

pub use apply::{AppliedEntry, AppliedLayout, ClaimFields, LayoutFailure, LayoutReport};
/// Model vocabulary the generated modules name directly (`layout-compiled`).
pub use bamboo_native_ui_layout_core::model::{
    BackgroundAttr, ContentAttr, Geometry, Horizontal, PropAttrs, RadiusAttr, StyleAttrs, Vertical,
};

/// Resolve a palette token name (the `background`/local-color preset table);
/// shared with `Ui::apply_skin_of` so both paths read one vocabulary.
pub(crate) fn palette_token(
    palette: &crate::theme::Palette,
    name: &str,
) -> Option<crate::color::Color> {
    Some(match name {
        "background" => palette.background,
        "sidebar" => palette.sidebar,
        "player" => palette.player,
        "cover" => palette.cover,
        "nav-active" => palette.nav_active,
        "accent" => palette.accent,
        "text" => palette.text,
        "muted" => palette.muted,
        "divider" => palette.divider,
        "row-selected" => palette.row_selected,
        "scroll-track" => palette.scroll_track,
        "scroll-thumb" => palette.scroll_thumb,
        "accent-text" => palette.accent_text,
        "search-bg" => palette.search_bg,
        "button-bg" => palette.button_bg,
        "button-hover" => palette.button_hover,
        "button-pressed" => palette.button_pressed,
        "button-active" => palette.button_active,
        "button-active-hover" => palette.button_active_hover,
        "explicit-border" => palette.explicit_border,
        "nav-hover" => palette.nav_hover,
        "row-hover" => palette.row_hover,
        "nav-pressed" => palette.nav_pressed,
        "row-pressed" => palette.row_pressed,
        "scroll-thumb-hover" => palette.scroll_thumb_hover,
        "card" => palette.card,
        "panel" => palette.panel,
        "button-disabled" => palette.button_disabled,
        "text-disabled" => palette.text_disabled,
        "play-button" => palette.play_button,
        "play-hover" => palette.play_hover,
        _ => return None,
    })
}
