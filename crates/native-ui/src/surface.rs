// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Per-widget background surface: corner radius, fill override and
//! transparency, shared by the painted components (D110).
//!
//! A widget keeps one [`SurfaceStyle`] field. Its paint path fills through
//! [`SurfaceStyle::fill`] instead of `Canvas::fill_rect`, so the same rounded
//! shape (and the same override/transparency rule) applies to the idle,
//! hovered, pressed and focus fills. Hit-testing stays rectangular by design:
//! the layout rectangle is the click target even when the paint rounds it.

use crate::backend::Canvas;
use crate::color::Color;
use crate::geometry::{Corners, Rect};

/// A widget's own background: radius, fill color override and whether the
/// widget paints a fill at all.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceStyle {
    /// Corner radii in DIPs; `ZERO` is a sharp rectangle.
    pub corners: Corners,
    /// `None` uses the widget's theme fill, `Some(color)` overrides it.
    pub background: Option<Color>,
    /// Suppress the widget's own fill so the parent shows through.
    pub transparent: bool,
}

impl SurfaceStyle {
    pub const fn new() -> Self {
        Self {
            corners: Corners::ZERO,
            background: None,
            transparent: false,
        }
    }

    /// Uniform corner radius in DIPs (clamped into `Corners` at paint time).
    pub fn with_radius(mut self, radius: f32) -> Self {
        self.corners = Corners::uniform(radius.max(0.0));
        self
    }

    /// Per-corner radii (a card with one square corner, for example).
    pub fn with_corners(mut self, corners: Corners) -> Self {
        self.corners = corners;
        self
    }

    /// Override the theme fill.
    pub fn with_background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// `None` returns to the theme fill.
    pub fn with_background_option(mut self, color: Option<Color>) -> Self {
        self.background = color;
        self
    }

    pub fn with_transparent(mut self, transparent: bool) -> Self {
        self.transparent = transparent;
        self
    }

    /// The color a fill would use for `default`, or `None` when transparent.
    pub fn color(&self, default: Color) -> Option<Color> {
        if self.transparent {
            None
        } else {
            Some(self.background.unwrap_or(default))
        }
    }

    /// Fill `rect` with the override (or `default`), rounded by `corners`.
    /// A no-op when the surface is transparent or the rectangle is empty.
    pub fn fill(&self, canvas: &mut dyn Canvas, rect: Rect, default: Color) {
        if let Some(color) = self.color(default) {
            self.shape(canvas, rect, color);
        }
    }

    /// Fill `rect` with `color` using the configured corners, ignoring the
    /// transparency/override rule. For a sub-shape that always shows (a
    /// progress fill, a knob) but follows the widget's rounding.
    pub fn shape(&self, canvas: &mut dyn Canvas, rect: Rect, color: Color) {
        fill_corners(canvas, rect, self.corners, color);
    }

    /// A 1 DIP border plus an inner fill, both following the corners. The
    /// border always paints (a focus affordance); `transparent_background`
    /// only suppresses the inner fill. A transparent surface is an outline,
    /// never a filled shape: the old outer-fill-then-inner-fill pass painted
    /// the whole rounded rectangle in the border color when there was no fill,
    /// which is what made a transparent rounded field look like a solid pill.
    pub fn frame(&self, canvas: &mut dyn Canvas, rect: Rect, border: Color, background: Color) {
        match self.color(background) {
            Some(fill) => {
                if self.corners.max() <= 0.0 {
                    canvas.frame(rect, border, fill);
                } else {
                    canvas.fill_round_frame(rect, self.corners, 1.0, border, fill);
                }
            }
            None => stroke_corners(canvas, rect, self.corners, 1.0, border),
        }
    }

    /// The corner radius a focus ring should follow, or `None` for a sharp
    /// rectangle. Only a uniform radius maps to the ring's single-radius
    /// stroke; mixed corners keep the rectangular ring.
    pub fn focus_radius(&self) -> Option<f32> {
        let radius = self.corners.top_left;
        (radius > 0.0 && self.is_uniform()).then_some(radius)
    }

    fn is_uniform(&self) -> bool {
        let c = self.corners;
        c.top_left == c.top_right && c.top_right == c.bottom_right && c.bottom_right == c.bottom_left
    }
}

/// Stroke a rectangle outline `thickness` DIPs wide without a fill (the
/// transparent frame case).
fn stroke_corners(
    canvas: &mut dyn Canvas,
    rect: Rect,
    corners: Corners,
    thickness: f32,
    color: Color,
) {
    if rect.is_empty() {
        return;
    }
    let thickness = thickness.max(0.0).min(rect.width().min(rect.height()) / 2.0);
    if thickness <= 0.0 {
        return;
    }
    if corners.max() <= 0.0 {
        let inner = rect.inset(thickness, thickness);
        canvas.fill_rect(
            Rect::new(rect.left, rect.top, rect.right, rect.top + thickness),
            color,
        );
        canvas.fill_rect(
            Rect::new(rect.left, rect.bottom - thickness, rect.right, rect.bottom),
            color,
        );
        if !inner.is_empty() {
            canvas.fill_rect(
                Rect::new(rect.left, inner.top, rect.left + thickness, inner.bottom),
                color,
            );
            canvas.fill_rect(
                Rect::new(rect.right - thickness, inner.top, rect.right, inner.bottom),
                color,
            );
        }
        return;
    }
    let uniform = corners.top_left == corners.top_right
        && corners.top_right == corners.bottom_right
        && corners.bottom_right == corners.bottom_left;
    if uniform {
        canvas.stroke_round_rect(rect, corners.top_left, thickness, color);
    } else {
        canvas.stroke_round_rect_corners(rect, corners, thickness, color);
    }
}

/// Fill `rect` with `color`, rounding by `corners` (the shared paint helper so
/// `fill`, `shape` and `frame` agree on the shape).
fn fill_corners(canvas: &mut dyn Canvas, rect: Rect, corners: Corners, color: Color) {
    if rect.is_empty() {
        return;
    }
    if corners.max() <= 0.0 {
        canvas.fill_rect(rect, color);
    } else if corners.top_left == corners.top_right
        && corners.top_right == corners.bottom_right
        && corners.bottom_right == corners.bottom_left
    {
        canvas.fill_round_rect(rect, corners.top_left, color);
    } else {
        canvas.fill_round_rect_corners(rect, corners, color);
    }
}

/// Generate the fluent surface builders for a widget whose `surface:
/// SurfaceStyle` field it already has. Invoke at module scope with the type
/// name. Do not use it where the widget already defines one of these names
/// (the checkbox and multi-select row corner radius are their own).
///
/// Public so an application can give its own widget the same builders:
/// `surface_builders!(MyWidget);` requires a `surface: SurfaceStyle` field.
#[macro_export]
macro_rules! surface_builders {
    ($ty:ident) => {
        impl<M> $ty<M> {
            /// Uniform corner radius of the fill and (when uniform) the focus
            /// ring, in DIPs.
            pub fn corner_radius(mut self, radius: f32) -> Self {
                self.surface = self.surface.with_radius(radius);
                self
            }

            /// Per-corner radii (a card with one square corner, a notch).
            pub fn corners(mut self, corners: $crate::geometry::Corners) -> Self {
                self.surface = self.surface.with_corners(corners);
                self
            }

            /// Override the theme fill color.
            pub fn background(mut self, color: $crate::color::Color) -> Self {
                self.surface = self.surface.with_background(color);
                self
            }

            /// `None` returns to the theme fill.
            pub fn background_option(mut self, color: Option<$crate::color::Color>) -> Self {
                self.surface = self.surface.with_background_option(color);
                self
            }

            /// Suppress the widget's own fill so the parent shows through.
            pub fn transparent_background(mut self, transparent: bool) -> Self {
                self.surface = self.surface.with_transparent(transparent);
                self
            }

            /// The configured surface (typed access).
            pub fn surface(&self) -> &$crate::surface::SurfaceStyle {
                &self.surface
            }
        }
    };
}

/// The same builders for a widget type that is not generic over `M`.
#[macro_export]
macro_rules! surface_builders_plain {
    ($ty:ident) => {
        impl $ty {
            pub fn corner_radius(mut self, radius: f32) -> Self {
                self.surface = self.surface.with_radius(radius);
                self
            }

            pub fn corners(mut self, corners: $crate::geometry::Corners) -> Self {
                self.surface = self.surface.with_corners(corners);
                self
            }

            pub fn background(mut self, color: $crate::color::Color) -> Self {
                self.surface = self.surface.with_background(color);
                self
            }

            pub fn background_option(mut self, color: Option<$crate::color::Color>) -> Self {
                self.surface = self.surface.with_background_option(color);
                self
            }

            pub fn transparent_background(mut self, transparent: bool) -> Self {
                self.surface = self.surface.with_transparent(transparent);
                self
            }

            pub fn surface(&self) -> &$crate::surface::SurfaceStyle {
                &self.surface
            }
        }
    };
}

pub use surface_builders;
pub use surface_builders_plain;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_override_and_transparency_resolve_the_fill() {
        let theme = Color::new(10, 20, 30);
        let custom = Color::new(200, 100, 50);
        let plain = SurfaceStyle::new();
        assert_eq!(plain.color(theme), Some(theme));
        assert_eq!(plain.with_background(custom).color(theme), Some(custom));
        // Returning to the theme fill and suppressing it entirely.
        assert_eq!(
            plain
                .with_background(custom)
                .with_background_option(None)
                .color(theme),
            Some(theme)
        );
        assert_eq!(plain.with_transparent(true).color(theme), None);
        // A transparent surface wins even over an explicit background.
        assert_eq!(
            plain.with_background(custom).with_transparent(true).color(theme),
            None
        );
    }

    #[test]
    fn focus_radius_follows_only_a_uniform_round() {
        assert_eq!(SurfaceStyle::new().focus_radius(), None);
        assert_eq!(SurfaceStyle::new().with_radius(6.0).focus_radius(), Some(6.0));
        // Mixed corners keep the rectangular ring.
        let mixed = SurfaceStyle::new().with_corners(Corners::new(6.0, 6.0, 0.0, 0.0));
        assert_eq!(mixed.focus_radius(), None);
        // A negative radius clamps to square.
        assert_eq!(SurfaceStyle::new().with_radius(-3.0).focus_radius(), None);
    }

    #[test]
    fn corners_are_preserved_for_the_paint_path() {
        let corners = Corners::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(SurfaceStyle::new().with_corners(corners).corners, corners);
        assert_eq!(SurfaceStyle::new().with_radius(5.0).corners, Corners::uniform(5.0));
    }
}
