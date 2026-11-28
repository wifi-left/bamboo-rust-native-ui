// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Rendering backend abstraction.
//!
//! Everything platform-specific sits behind these traits. The public API only
//! uses the neutral types re-exported here, so adding a software-raster or GPU
//! backend later does not change a single call site.
//!
//! Dispatch happens once per draw call (tens to a few hundred per frame), not
//! per widget, so the `dyn` indirection is negligible. If a future backend
//! needs monomorphization, `Painter` can be parameterized over `Canvas`.

use std::sync::Arc;

use crate::color::Color;
use crate::geometry::{Corners, Rect, Size};

#[cfg(all(windows, feature = "gdi"))]
pub mod d2d;
#[cfg(all(windows, feature = "gdi"))]
pub mod gdi;

/// Which renderer paints the window's back buffer.
///
/// The window keeps one persistent back buffer and blits the dirty bounding
/// box in every mode; the renderer only changes how a frame is drawn into that
/// buffer. `Gdi` is the software fallback and the default; the Direct2D modes
/// draw through `ID2D1DCRenderTarget` (hardware when the driver offers one,
/// Direct2D's own software fallback otherwise). Select with
/// `WindowBuilder::renderer` or the `BAMBOO_UI_RENDERER` environment variable
/// (`gdi`, `d2d-software`, `d2d-hardware`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Renderer {
    #[default]
    Gdi,
    D2dSoftware,
    D2dHardware,
}

impl Renderer {
    /// Parse a selector name (`gdi`, `d2d-software`, `d2d-hardware`).
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "" | "gdi" | "gdi-software" => Some(Renderer::Gdi),
            "d2d" | "d2d-hardware" | "hardware" => Some(Renderer::D2dHardware),
            "d2d-software" | "software" => Some(Renderer::D2dSoftware),
            _ => None,
        }
    }

    /// The environment selection (`BAMBOO_UI_RENDERER`), if set and valid.
    pub fn from_env() -> Option<Self> {
        std::env::var("BAMBOO_UI_RENDERER")
            .ok()
            .and_then(|value| Self::parse(&value))
    }

    pub fn name(self) -> &'static str {
        match self {
            Renderer::Gdi => "gdi",
            Renderer::D2dSoftware => "d2d-software",
            Renderer::D2dHardware => "d2d-hardware",
        }
    }
}

/// Font weight, mapped to platform-specific numeric weights by the backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontWeight {
    Normal,
    Medium,
    Semibold,
    Bold,
}

impl FontWeight {
    /// Win32 logical weight.
    pub const fn to_windows(self) -> i32 {
        match self {
            FontWeight::Normal => 400,
            FontWeight::Medium => 500,
            FontWeight::Semibold => 600,
            FontWeight::Bold => 700,
        }
    }
}

/// The portable rounded-rectangle composition (three bands plus four corner
/// ellipses). Used by the default [`Canvas::fill_round_rect`] and as the GDI
/// fallback for shapes too large to mask.
pub(crate) fn composite_round_rect<C: Canvas + ?Sized>(
    canvas: &mut C,
    rect: Rect,
    radius: f32,
    color: Color,
) {
    let radius = radius.max(0.0).min(rect.width().min(rect.height()) / 2.0);
    if radius <= 0.0 {
        canvas.fill_rect(rect, color);
        return;
    }
    let (left, top, right, bottom) = (rect.left, rect.top, rect.right, rect.bottom);
    canvas.fill_rect(Rect::new(left + radius, top, right - radius, bottom), color);
    canvas.fill_rect(Rect::new(left, top + radius, left + radius, bottom - radius), color);
    canvas.fill_rect(
        Rect::new(right - radius, top + radius, right, bottom - radius),
        color,
    );
    let d = radius * 2.0;
    canvas.ellipse(Rect::new(left, top, left + d, top + d), color);
    canvas.ellipse(Rect::new(right - d, top, right, top + d), color);
    canvas.ellipse(Rect::new(left, bottom - d, left + d, bottom), color);
    canvas.ellipse(Rect::new(right - d, bottom - d, right, bottom), color);
}

/// Portable rounded-outline fallback: four straight bands plus the corner
/// quarter-arcs approximated by rounded fills.
pub(crate) fn composite_round_stroke<C: Canvas + ?Sized>(
    canvas: &mut C,
    rect: Rect,
    radius: f32,
    thickness: f32,
    color: Color,
) {
    let thickness = thickness.max(0.0).min(rect.width().min(rect.height()) / 2.0);
    if thickness <= 0.0 || rect.is_empty() {
        return;
    }
    let inner = rect.inset(thickness, thickness);
    if inner.is_empty() {
        canvas.fill_round_rect(rect, radius, color);
        return;
    }
    let r = radius.max(0.0).min(rect.width().min(rect.height()) / 2.0);
    if r <= 0.5 {
        canvas.fill_rect(
            Rect::new(rect.left, rect.top, rect.right, rect.top + thickness),
            color,
        );
        canvas.fill_rect(
            Rect::new(rect.left, rect.bottom - thickness, rect.right, rect.bottom),
            color,
        );
        canvas.fill_rect(
            Rect::new(rect.left, inner.top, rect.left + thickness, inner.bottom),
            color,
        );
        canvas.fill_rect(
            Rect::new(rect.right - thickness, inner.top, rect.right, inner.bottom),
            color,
        );
        return;
    }
    canvas.fill_rect(
        Rect::new(rect.left + r, rect.top, rect.right - r, rect.top + thickness),
        color,
    );
    canvas.fill_rect(
        Rect::new(rect.left + r, rect.bottom - thickness, rect.right - r, rect.bottom),
        color,
    );
    canvas.fill_rect(
        Rect::new(rect.left, rect.top + r, rect.left + thickness, rect.bottom - r),
        color,
    );
    canvas.fill_rect(
        Rect::new(rect.right - thickness, rect.top + r, rect.right, rect.bottom - r),
        color,
    );
    // The corner arcs. Each is a quarter ring, so a corner must not be painted
    // as a filled disc — the inner quarter stays untouched. Walking one device
    // row at a time is exact and needs no mask (this fallback exists precisely
    // for shapes too large to mask and for backends without masks).
    let row = 1.0 / canvas.scale().max(0.05);
    let inner_radius = (r - thickness).max(0.0);
    let rows = ((r * canvas.scale()).round() as usize).max(1);
    for (corner, sx, sy) in [
        ((rect.left, rect.top), 1.0, 1.0),
        ((rect.right, rect.top), -1.0, 1.0),
        ((rect.right, rect.bottom), -1.0, -1.0),
        ((rect.left, rect.bottom), 1.0, -1.0),
    ] {
        let (cx, cy) = corner;
        for index in 0..rows {
            let y0 = index as f32 * row;
            let y1 = y0 + row;
            // Distance from the arc centre on this scanline.
            let dy = r - (y0 + y1) * 0.5;
            if dy <= 0.0 {
                break;
            }
            let outer_x = r - (r * r - dy * dy).max(0.0).sqrt();
            let inner_x = if dy < inner_radius {
                r - (inner_radius * inner_radius - dy * dy).max(0.0).sqrt()
            } else {
                r
            };
            if inner_x <= outer_x {
                continue;
            }
            let (x0, x1) = if sx > 0.0 {
                (cx + outer_x, cx + inner_x)
            } else {
                (cx - inner_x, cx - outer_x)
            };
            let (ya, yb) = if sy > 0.0 {
                (cy + y0, cy + y1)
            } else {
                (cy - y1, cy - y0)
            };
            canvas.fill_rect(Rect::new(x0, ya, x1, yb), color);
        }
    }
}

/// Which typeface a text run uses.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFace {
    /// The theme's UI family (falls back to Microsoft YaHei UI).
    Ui,
    /// Segoe MDL2 Assets, for monochrome icon glyphs.
    Glyph,
    /// An explicit family chosen by the application.
    Named(Arc<str>),
}

/// A text style. Constructed once per theme, borrowed by every draw call.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// Font size in DIPs (already scaled by the theme).
    pub size: f32,
    pub weight: FontWeight,
    pub italic: bool,
    pub face: FontFace,
}

impl TextStyle {
    pub fn ui(size: f32) -> Self {
        Self {
            size,
            weight: FontWeight::Normal,
            italic: false,
            face: FontFace::Ui,
        }
    }

    pub fn glyph(size: f32) -> Self {
        Self {
            size,
            weight: FontWeight::Normal,
            italic: false,
            face: FontFace::Glyph,
        }
    }

    pub fn with_weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }
}

/// Horizontal text alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// What a single-line text run does when it is wider than its rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextOverflow {
    /// Stop at the rectangle edge.
    Clip,
    /// Stop at the rectangle edge behind an ellipsis.
    #[default]
    Ellipsis,
}

/// Stable identity of an image in the backend cache (e.g. a cover revision).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u64);

/// Borrowed RGBA pixels. The backend converts and caches them by [`ImageId`];
/// the data is only read on a cache miss.
#[derive(Clone, Copy)]
pub struct ImageData<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
}

/// A drawing surface for one paint pass.
///
/// All coordinates are DIPs. The backend owns brush/font/image caches, so the
/// implementation must live for the lifetime of the window, not one frame.
pub trait Canvas {
    /// Device scale (DPI / 96).
    fn scale(&self) -> f32;

    /// DPI value (96 at 100%).
    fn dpi(&self) -> f32;

    fn fill_rect(&mut self, rect: Rect, color: Color);

    /// Fill with a translucent color (`alpha` 0..=1). The default paints the
    /// color opaquely, which is the honest fallback for a backend without
    /// alpha blending; the GDI backend uses `AlphaBlend`. Scrims and tinted
    /// overlays use this.
    fn fill_translucent(&mut self, rect: Rect, color: Color, alpha: f32) {
        let _ = alpha;
        self.fill_rect(rect, color);
    }

    /// A 1 DIP border with an inner background fill.
    fn frame(&mut self, rect: Rect, border: Color, background: Color) {
        self.fill_rect(rect, border);
        self.fill_rect(rect.inset(1.0, 1.0), background);
    }

    /// Fill a rectangle with rounded corners. The radius is clamped to half
    /// the shorter side (a pill). The default composes three rectangles and
    /// four ellipses, which is not anti-aliased; a backend with a mask or AA
    /// path (the GDI backend caches a coverage mask) overrides it.
    fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        composite_round_rect(self, rect, radius, color);
    }

    /// Fill a rounded rectangle with independent corner radii. A backend
    /// without per-corner masks paints with the largest radius; the GDI
    /// backend caches one coverage map per corner set.
    fn fill_round_rect_corners(&mut self, rect: Rect, corners: Corners, color: Color) {
        self.fill_round_rect(rect, corners.max(), color);
    }

    /// Stroke a rounded outline `thickness` DIPs wide (a ring). The default
    /// paints four straight edge bands and leaves the corner arcs to the
    /// backend's rounded fill; the GDI backend uses an anti-aliased ring mask.
    fn stroke_round_rect(&mut self, rect: Rect, radius: f32, thickness: f32, color: Color) {
        composite_round_stroke(self, rect, radius, thickness, color);
    }

    /// Stroke a rounded outline with independent corner radii. The default
    /// uses the largest radius; the GDI backend builds a per-corner ring mask
    /// and Direct2D an outer-minus-inner path, so a transparent surface with
    /// cut corners keeps its exact outline.
    fn stroke_round_rect_corners(
        &mut self,
        rect: Rect,
        corners: Corners,
        thickness: f32,
        color: Color,
    ) {
        self.stroke_round_rect(rect, corners.max(), thickness, color);
    }

    /// A rounded 1 DIP (or `thickness`) border plus an inner fill in one shape.
    /// The default draws the inner fill and the outline separately; the GDI
    /// backend bakes both colors into one cached mask, so a frame costs one
    /// blend instead of a full-shape border fill covered by an inner fill.
    fn fill_round_frame(
        &mut self,
        rect: Rect,
        corners: Corners,
        thickness: f32,
        border: Color,
        fill: Color,
    ) {
        let thickness = thickness.max(0.0);
        let limit = rect.width().min(rect.height()) / 2.0;
        let outer = corners.map(|radius| radius.clamp(0.0, limit));
        let inner = rect.inset(thickness, thickness);
        let inner_limit = inner.width().min(inner.height()) / 2.0;
        let inner_corners = outer.map(|radius| (radius - thickness).max(0.0).clamp(0.0, inner_limit));
        if !inner.is_empty() {
            self.fill_round_rect_corners(inner, inner_corners, fill);
        }
        self.stroke_round_rect_corners(rect, outer, thickness, border);
    }

    /// Fill the circle inscribed in `rect` (an ellipse when the rectangle is
    /// not square), anti-aliased on the GDI backend.
    fn fill_circle(&mut self, rect: Rect, color: Color) {
        self.ellipse(rect, color);
    }

    fn text(
        &mut self,
        rect: Rect,
        text: &str,
        style: &TextStyle,
        align: TextAlign,
        color: Color,
    );

    /// Draw one line and choose what happens at the rectangle edge. A backend
    /// that cannot distinguish the two still paints the text; the default
    /// keeps [`Canvas::text`] behavior (an ellipsis).
    fn text_overflow(
        &mut self,
        rect: Rect,
        text: &str,
        style: &TextStyle,
        align: TextAlign,
        color: Color,
        overflow: TextOverflow,
    ) {
        let _ = overflow;
        self.text(rect, text, style, align, color);
    }

    /// Measured size of a single line in DIPs.
    fn measure_text(&mut self, text: &str, style: &TextStyle) -> Size;

    fn ellipse(&mut self, rect: Rect, color: Color);

    fn vertical_gradient(&mut self, rect: Rect, top: Color, bottom: Color);

    /// A vertical gradient clipped to a rounded rectangle (a cover
    /// placeholder). The default paints the square gradient.
    fn vertical_gradient_round(&mut self, rect: Rect, radius: f32, top: Color, bottom: Color) {
        let _ = radius;
        self.vertical_gradient(rect, top, bottom);
    }

    /// Draw an image scaled into `rect`. With `crop_square`, the source is
    /// center-cropped to a square first (covers and thumbnails).
    fn image(&mut self, rect: Rect, crop_square: bool, id: ImageId, data: ImageData<'_>);

    /// Draw an image clipped to the circle inscribed in `rect` (an avatar).
    /// The default paints the square image: an honest fallback for a backend
    /// without a mask; the GDI backend anti-aliases the circular edge.
    fn image_circle(&mut self, rect: Rect, crop_square: bool, id: ImageId, data: ImageData<'_>) {
        self.image(rect, crop_square, id, data);
    }

    /// Draw an image clipped to a rectangle whose corners have `radius` DIPs
    /// (rounded cover art). The radius is clamped to half the shorter side, so
    /// `rect.height() / 2` is a capsule. The default paints the square image:
    /// an honest fallback for a backend without a mask; the GDI backend
    /// anti-aliases the corners.
    fn image_round(
        &mut self,
        rect: Rect,
        radius: f32,
        crop_square: bool,
        id: ImageId,
        data: ImageData<'_>,
    ) {
        let _ = radius;
        self.image(rect, crop_square, id, data);
    }

    fn push_clip(&mut self, rect: Rect);

    fn pop_clip(&mut self);
}
