// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Theme-aware paint helpers over a [`Canvas`].
//!
//! The painter owns the resolved palette and the text styles for the current
//! theme. It holds no per-frame state and never allocates while painting; the
//! backend owns all caches.

use std::sync::Arc;

use crate::backend::{Canvas, FontFace, FontWeight, TextAlign, TextStyle};
use crate::color::Color;
use crate::geometry::Rect;
use crate::theme::{Palette, ThemeSettings};

pub struct Painter {
    palette: Palette,
    settings: ThemeSettings,
    /// The last dark/light decision, so a child window (a dialog) can follow
    /// the page's mode.
    dark: bool,
    body: TextStyle,
    small: TextStyle,
    title: TextStyle,
    brand: TextStyle,
    glyph: TextStyle,
    glyph_large: TextStyle,
    glyph_huge: TextStyle,
    /// Caret bar width in DIPs for framework-painted editors. The host seeds
    /// it from the system accessibility caret width; a field can override it.
    caret_width: f32,
}

impl Default for Painter {
    fn default() -> Self {
        Self::new()
    }
}

impl Painter {
    pub fn new() -> Self {
        let settings = ThemeSettings::default();
        let palette = Palette::new(&settings, true);
        let (body, small, title, brand, glyph, glyph_large, glyph_huge) = styles(&settings);
        Self {
            palette,
            settings,
            dark: true,
            body,
            small,
            title,
            brand,
            glyph,
            glyph_large,
            glyph_huge,
            caret_width: 1.0,
        }
    }

    /// Caret bar width in DIPs for painted editors (default 1.0; the host
    /// replaces it with the system accessibility setting when available).
    pub fn caret_width(&self) -> f32 {
        self.caret_width
    }

    pub fn set_caret_width(&mut self, width: f32) {
        self.caret_width = width.clamp(0.5, 6.0);
    }

    pub fn settings(&self) -> &ThemeSettings {
        &self.settings
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Whether the current palette is the dark variant.
    pub fn dark(&self) -> bool {
        self.dark
    }

    /// Install a palette without touching the text styles (owner-draw hooks and
    /// a `.buitheme` document's derived palette). The dark flag follows the
    /// palette, so the window chrome agrees with it.
    pub fn set_palette(&mut self, palette: Palette) {
        self.dark = palette.dark;
        self.palette = palette;
    }

    /// Rebuild palette and styles. Returns true when the text styles changed,
    /// so the caller can drop cached fonts.
    pub fn set_theme(&mut self, settings: &ThemeSettings, dark: bool) -> bool {
        let restyle = self.settings != *settings;
        self.dark = dark;
        self.palette = Palette::new(settings, dark);
        if restyle {
            self.settings = settings.clone();
            (self.body, self.small, self.title, self.brand, self.glyph, self.glyph_large, self.glyph_huge) =
                styles(settings);
        }
        restyle
    }

    pub fn body(&self) -> &TextStyle {
        &self.body
    }

    pub fn small(&self) -> &TextStyle {
        &self.small
    }

    pub fn title(&self) -> &TextStyle {
        &self.title
    }

    pub fn brand(&self) -> &TextStyle {
        &self.brand
    }

    pub fn glyph(&self) -> &TextStyle {
        &self.glyph
    }

    pub fn glyph_large(&self) -> &TextStyle {
        &self.glyph_large
    }

    pub fn glyph_huge(&self) -> &TextStyle {
        &self.glyph_huge
    }

    /// A one-off UI style at an explicit size (page-specific headings).
    pub fn ui_style(&self, size: f32) -> TextStyle {
        let mut style = self.body.clone();
        style.size = size * self.settings.font_scale();
        style
    }

    pub fn fill(&self, canvas: &mut dyn Canvas, rect: Rect, color: Color) {
        canvas.fill_rect(rect, color);
    }

    pub fn frame(&self, canvas: &mut dyn Canvas, rect: Rect, border: Color, background: Color) {
        canvas.frame(rect, border, background);
    }

    /// Fill a rounded rectangle. `radius` is clamped to half the shorter side,
    /// so `rect.height() / 2` paints a pill. The GDI backend draws an
    /// anti-aliased cached mask; other backends compose rects and ellipses.
    pub fn round_rect(&self, canvas: &mut dyn Canvas, rect: Rect, radius: f32, color: Color) {
        canvas.fill_round_rect(rect, radius, color);
    }

    /// A rounded fill whose corners have independent radii.
    pub fn round_rect_corners(
        &self,
        canvas: &mut dyn Canvas,
        rect: Rect,
        corners: crate::geometry::Corners,
        color: Color,
    ) {
        canvas.fill_round_rect_corners(rect, corners, color);
    }

    /// A rounded outline `thickness` DIPs wide (an anti-aliased ring on GDI).
    pub fn round_rect_stroke(
        &self,
        canvas: &mut dyn Canvas,
        rect: Rect,
        radius: f32,
        thickness: f32,
        color: Color,
    ) {
        canvas.stroke_round_rect(rect, radius, thickness, color);
    }

    /// The circle inscribed in `rect` (an anti-aliased circle on GDI).
    pub fn circle(&self, canvas: &mut dyn Canvas, rect: Rect, color: Color) {
        canvas.fill_circle(rect, color);
    }

    pub fn text(
        &self,
        canvas: &mut dyn Canvas,
        rect: Rect,
        text: &str,
        style: &TextStyle,
        align: TextAlign,
        color: Color,
    ) {
        canvas.text(rect, text, style, align, color);
    }
}

fn styles(
    settings: &ThemeSettings,
) -> (
    TextStyle,
    TextStyle,
    TextStyle,
    TextStyle,
    TextStyle,
    TextStyle,
    TextStyle,
) {
    let scale = settings.font_scale();
    let face = match settings.font_family.as_deref() {
        Some(name) if !name.trim().is_empty() => FontFace::Named(Arc::from(name)),
        _ => FontFace::Ui,
    };
    let ui = |size: f32, weight: FontWeight| TextStyle {
        size: size * scale,
        weight,
        italic: false,
        face: face.clone(),
    };
    (
        ui(13.5, FontWeight::Normal),
        ui(11.5, FontWeight::Normal),
        ui(20.0, FontWeight::Bold),
        ui(12.0, FontWeight::Semibold),
        TextStyle::glyph(15.0),
        TextStyle::glyph(18.0),
        TextStyle::glyph(24.0),
    )
}
