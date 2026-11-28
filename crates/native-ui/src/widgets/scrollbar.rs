// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Shared scrollbar geometry, pointer state, painting and drag math.
//!
//! A scrollable widget stores one [`ScrollBarState`] per bar and calls into
//! this module from paint/input, so every bar gets the same hover/active
//! colors, minimum thumb length, track-click behavior and drag mapping.
//! Thumb dragging keeps the press grab offset, so the thumb does not jump
//! under the pointer.

use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::theme::Palette;
use crate::widget::WidgetPaint;

/// Colors resolved for one paint. `None` on the style means "resolve from the
/// palette", so a per-widget or per-theme palette still applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollBarColors {
    pub track: Color,
    pub thumb: Color,
    pub thumb_hover: Color,
    pub thumb_active: Color,
}

impl ScrollBarColors {
    pub fn from_palette(palette: &Palette) -> Self {
        Self {
            track: palette.scroll_track,
            thumb: palette.scroll_thumb,
            thumb_hover: palette.scroll_thumb_hover,
            // The palette has no dedicated active color; a brighter hover
            // shifted toward the accent reads as "held".
            thumb_active: palette.scroll_thumb_hover.mix(palette.accent, 0.45),
        }
    }
}

/// Geometry and behavior of a scrollbar. Clone it from a theme or build one
/// per widget; it is plain data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollBarStyle {
    /// Track thickness (vertical width or horizontal height) in DIPs.
    pub thickness: f32,
    /// Gap between the track and the viewport edge.
    pub inset: f32,
    /// Inset of the thumb inside the track (both axes).
    pub thumb_inset: f32,
    /// Minimum thumb length in DIPs.
    pub min_thumb: f32,
    /// Track clicks page by ~90% of the viewport instead of jumping so the
    /// thumb centers on the click.
    pub page_on_track_click: bool,
    pub colors: Option<ScrollBarColors>,
}

impl Default for ScrollBarStyle {
    fn default() -> Self {
        Self {
            thickness: 10.0,
            inset: 0.0,
            thumb_inset: 2.0,
            min_thumb: 24.0,
            page_on_track_click: false,
            colors: None,
        }
    }
}

impl ScrollBarStyle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness.max(2.0);
        self
    }

    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = inset.max(0.0);
        self
    }

    pub fn thumb_inset(mut self, inset: f32) -> Self {
        self.thumb_inset = inset.max(0.0);
        self
    }

    pub fn min_thumb(mut self, length: f32) -> Self {
        self.min_thumb = length.max(4.0);
        self
    }

    pub fn page_on_track_click(mut self, page: bool) -> Self {
        self.page_on_track_click = page;
        self
    }

    pub fn colors(mut self, colors: ScrollBarColors) -> Self {
        self.colors = Some(colors);
        self
    }

    pub(crate) fn resolved_colors(&self, palette: &Palette) -> ScrollBarColors {
        self.colors
            .unwrap_or_else(|| ScrollBarColors::from_palette(palette))
    }
}

/// Pointer state of one bar (vertical or horizontal).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollBarState {
    pub hovered: bool,
    pub active: bool,
    /// Pointer position minus thumb start at press time (drag stability).
    pub grab: f32,
}

impl ScrollBarState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// What the pointer is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarHit {
    Thumb,
    Track,
}

/// Track rectangle for a viewport edge.
pub fn track_rect(viewport: Rect, vertical: bool, style: &ScrollBarStyle) -> Rect {
    let thickness = style.thickness;
    if vertical {
        Rect::new(
            viewport.right - style.inset - thickness,
            viewport.top + style.inset,
            viewport.right - style.inset,
            viewport.bottom - style.inset,
        )
    } else {
        Rect::new(
            viewport.left + style.inset,
            viewport.bottom - style.inset - thickness,
            viewport.right - style.inset,
            viewport.bottom - style.inset,
        )
    }
}

/// Thumb rectangle, or `None` when the content fits.
pub fn thumb_rect(
    track: Rect,
    content: f32,
    visible: f32,
    offset: f32,
    vertical: bool,
    style: &ScrollBarStyle,
) -> Option<Rect> {
    let span = if vertical { track.height() } else { track.width() };
    let max_offset = (content - visible).max(0.0);
    if span <= 2.0 || max_offset <= 0.01 {
        return None;
    }
    let length = (span * visible / content.max(0.001))
        .max(style.min_thumb.min(span))
        .min(span);
    let t = (offset / max_offset).clamp(0.0, 1.0);
    let start = if vertical { track.top } else { track.left } + (span - length) * t;
    let inset = style.thumb_inset;
    Some(if vertical {
        Rect::new(
            track.left + inset,
            start,
            track.right - inset,
            start + length,
        )
    } else {
        Rect::new(
            start,
            track.top + inset,
            start + length,
            track.bottom - inset,
        )
    })
}

/// Lines a wheel notch scrolls, from the system setting (3 by default, see
/// `platform::wheel_lines`). Widgets multiply their row/line step by this, so
/// the feel matches the rest of Windows; a "page" setting is the large marker
/// [`PAGE_LINES`], which clamps to the whole range.
pub fn wheel_lines() -> f32 {
    #[cfg(windows)]
    {
        crate::platform::wheel_lines()
    }
    #[cfg(not(windows))]
    {
        3.0
    }
}

/// The "scroll a page" marker returned by [`wheel_lines`].
pub const PAGE_LINES: f32 = 100.0;

/// What a press at `pos` grabs. The thumb owns its whole *axis span* across the
/// track, not just the inset pill: the pill is `thumb_inset` narrower than the
/// track, and a press on that edge strip used to count as a track click, which
/// jumped the content instead of starting a drag (a "the bar only drags
/// sometimes" report). The cross-axis extent confirms the bar is vertical or
/// horizontal rather than guessing.
pub fn hit(track: Rect, thumb: Option<Rect>, pos: Point) -> Option<BarHit> {
    if !track.contains_point(pos) {
        return None;
    }
    if let Some(thumb) = thumb {
        let vertical = track.height() > track.width();
        let inside = if vertical {
            pos.y >= thumb.top && pos.y < thumb.bottom
        } else {
            pos.x >= thumb.left && pos.x < thumb.right
        };
        if inside {
            return Some(BarHit::Thumb);
        }
    }
    Some(BarHit::Track)
}

fn axis(pos: Point, vertical: bool) -> f32 {
    if vertical { pos.y } else { pos.x }
}

fn range_start(rect: Rect, vertical: bool) -> f32 {
    if vertical { rect.top } else { rect.left }
}

fn range_span(rect: Rect, vertical: bool) -> f32 {
    if vertical { rect.height() } else { rect.width() }
}

/// Content offset for a thumb drag, preserving the press grab offset.
pub fn drag_offset(
    pos: Point,
    track: Rect,
    thumb: Rect,
    grab: f32,
    content: f32,
    visible: f32,
    vertical: bool,
) -> f32 {
    let span = range_span(track, vertical) - range_span(thumb, vertical);
    if span <= 0.01 {
        return 0.0;
    }
    let start = axis(pos, vertical) - grab - range_start(track, vertical);
    let t = (start / span).clamp(0.0, 1.0);
    t * (content - visible).max(0.0)
}

/// Content offset for a track click: page or jump (center the thumb).
pub fn track_click_offset(
    pos: Point,
    track: Rect,
    thumb: Rect,
    content: f32,
    visible: f32,
    offset: f32,
    vertical: bool,
    style: &ScrollBarStyle,
) -> f32 {
    let max_offset = (content - visible).max(0.0);
    if max_offset <= 0.01 {
        return 0.0;
    }
    if style.page_on_track_click {
        let thumb_start = range_start(thumb, vertical);
        let direction = if axis(pos, vertical) < thumb_start {
            -1.0
        } else {
            1.0
        };
        return (offset + direction * visible * 0.9).clamp(0.0, max_offset);
    }
    let span = range_span(track, vertical) - range_span(thumb, vertical);
    if span <= 0.01 {
        return offset.clamp(0.0, max_offset);
    }
    let start = axis(pos, vertical) - range_span(thumb, vertical) / 2.0 - range_start(track, vertical);
    (start / span).clamp(0.0, 1.0) * max_offset
}

/// Draw the track and thumb with the hover/active tint.
pub fn paint(
    ctx: &mut WidgetPaint<'_>,
    track: Rect,
    thumb: Option<Rect>,
    state: &ScrollBarState,
    style: &ScrollBarStyle,
) {
    let colors = style.resolved_colors(&ctx.palette);
    ctx.canvas.fill_rect(track, colors.track);
    if let Some(thumb) = thumb {
        let color = if state.active {
            colors.thumb_active
        } else if state.hovered {
            colors.thumb_hover
        } else {
            colors.thumb
        };
        // Pill-shaped thumb (the anti-aliased rounded fill caches its mask).
        let radius = thumb.width().min(thumb.height()) / 2.0;
        let painter = ctx.painter;
        painter.round_rect(&mut *ctx.canvas, thumb, radius, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> Rect {
        Rect::from_xywh(100.0, 0.0, 10.0, 200.0)
    }

    #[test]
    fn default_colors_follow_the_theme_and_overrides_win() {
        use crate::theme::{Palette, ThemeSettings};
        let settings = ThemeSettings::default();
        let light = Palette::new(&settings, false);
        let dark = Palette::new(&settings, true);
        let light_colors = ScrollBarStyle::default().resolved_colors(&light);
        let dark_colors = ScrollBarStyle::default().resolved_colors(&dark);
        // The theme-level default is the palette: the two themes differ and a
        // palette's own thumb color is used verbatim.
        assert_ne!(light_colors.track, dark_colors.track);
        assert_ne!(light_colors.thumb, dark_colors.thumb);
        assert_eq!(dark_colors.track, dark.scroll_track);
        assert_eq!(dark_colors.thumb, dark.scroll_thumb);
        // Per-widget colors replace the theme default regardless of theme.
        let custom = ScrollBarColors {
            track: crate::Color::new(0x20, 0x30, 0x40),
            thumb: crate::Color::new(0x60, 0x70, 0x80),
            thumb_hover: crate::Color::new(0x90, 0xA0, 0xB0),
            thumb_active: crate::Color::new(0xC0, 0xD0, 0xE0),
        };
        let style = ScrollBarStyle::default().colors(custom);
        assert_eq!(style.resolved_colors(&light), custom);
        assert_eq!(style.resolved_colors(&dark), custom);
    }

    #[test]
    fn thumb_length_is_bounded_and_position_follows_the_offset() {
        let style = ScrollBarStyle::default();
        let top = thumb_rect(track(), 1000.0, 200.0, 0.0, true, &style).unwrap();
        assert_eq!(top.height(), 40.0);
        assert_eq!(top.top, 0.0);
        let middle = thumb_rect(track(), 1000.0, 200.0, 400.0, true, &style).unwrap();
        assert!((middle.top - 80.0).abs() < 0.01);
        let bottom = thumb_rect(track(), 1000.0, 200.0, 800.0, true, &style).unwrap();
        assert!((bottom.bottom - 200.0).abs() < 0.01);
        // Fitting content has no thumb at all.
        assert!(thumb_rect(track(), 150.0, 200.0, 0.0, true, &style).is_none());
    }

    #[test]
    fn thumb_respects_the_minimum_and_insets() {
        let style = ScrollBarStyle::default();
        let thumb = thumb_rect(track(), 100_000.0, 200.0, 0.0, true, &style).unwrap();
        assert_eq!(thumb.height(), style.min_thumb);
        assert_eq!(thumb.left, 102.0);
        assert_eq!(thumb.right, 108.0);
    }

    #[test]
    fn dragging_preserves_the_grab_offset() {
        let style = ScrollBarStyle::default();
        let thumb = thumb_rect(track(), 1000.0, 200.0, 0.0, true, &style).unwrap();
        // Press 8 DIPs into the thumb (top 0..40) and drag to y=108.
        let offset = drag_offset(Point::new(105.0, 108.0), track(), thumb, 8.0, 1000.0, 200.0, true);
        // The thumb start is now 100; 100 / 160 * 800 = 500.
        assert!((offset - 500.0).abs() < 0.01);
    }

    #[test]
    fn track_click_jumps_or_pages() {
        let style = ScrollBarStyle::default();
        let thumb = thumb_rect(track(), 1000.0, 200.0, 0.0, true, &style).unwrap();
        // Click at the middle of the track: center the thumb there.
        let jumped = track_click_offset(
            Point::new(105.0, 100.0),
            track(),
            thumb,
            1000.0,
            200.0,
            0.0,
            true,
            &style,
        );
        assert!((jumped - 400.0).abs() < 0.01);
        let paging = style.page_on_track_click(true);
        let paged = track_click_offset(
            Point::new(105.0, 150.0),
            track(),
            thumb,
            1000.0,
            200.0,
            100.0,
            true,
            &paging,
        );
        assert!((paged - 280.0).abs() < 0.01);
    }

    #[test]
    fn the_wheel_step_follows_the_system_line_count() {
        // The setting is a line count (3 by default, a page marker for
        // "scroll a page"); a row list multiplies its row height by it.
        let lines = wheel_lines();
        assert!(lines >= 1.0, "lines {lines}");
        assert!(lines <= PAGE_LINES, "lines {lines}");
        let rows = 20.0 * lines;
        assert!(rows >= 20.0);
        // Three lines of a 30 DIP row is 90 DIPs; a page marker clamps later.
        let notch: f32 = 30.0 * 3.0;
        assert!((notch - 90.0).abs() < 0.01);
    }

    #[test]
    fn hit_separates_thumb_and_track() {
        let style = ScrollBarStyle::default();
        let thumb = thumb_rect(track(), 1000.0, 200.0, 0.0, true, &style);
        assert_eq!(hit(track(), thumb, Point::new(105.0, 10.0)), Some(BarHit::Thumb));
        assert_eq!(hit(track(), thumb, Point::new(105.0, 100.0)), Some(BarHit::Track));
        assert_eq!(hit(track(), thumb, Point::new(50.0, 100.0)), None);
        // The thumb's edge strip (the pill is inset) still grabs the thumb, so
        // pressing near the track edge starts a drag instead of a jump.
        assert_eq!(hit(track(), thumb, Point::new(100.5, 10.0)), Some(BarHit::Thumb));
        assert_eq!(hit(track(), thumb, Point::new(109.0, 39.0)), Some(BarHit::Thumb));
        // The same rule on a horizontal bar.
        let horizontal = Rect::from_xywh(0.0, 100.0, 200.0, 10.0);
        let thumb = thumb_rect(horizontal, 1000.0, 200.0, 0.0, false, &style).unwrap();
        assert_eq!(
            hit(horizontal, Some(thumb), Point::new(10.0, 100.5)),
            Some(BarHit::Thumb)
        );
        assert_eq!(
            hit(horizontal, Some(thumb), Point::new(150.0, 105.0)),
            Some(BarHit::Track)
        );
    }
}
