// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Album cover wall: a virtualized grid of square covers with a caption.
//!
//! The wall is the player's browse surface: covers fill the width at the
//! largest tile width that still fits the target tile size, the grid scrolls
//! vertically with the shared themed scrollbar, and one cover is the current
//! (selected) one. A click selects; a double click or Enter opens
//! (`on_open`), so a single click never starts playback by accident.
//!
//! Covers reference page images registered with [`crate::Ui::set_image`];
//! an unregistered or missing image paints a deterministic gradient
//! placeholder with the title's first character, so the wall looks complete
//! before the artwork pipeline exists.

use crate::a11y::{A11yChild, A11yInfo, A11yRole, A11yScroll};
use crate::backend::{ImageId, TextAlign, TextOverflow};
use crate::color::Color;
use crate::event::{Cursor, KeyCode};
use crate::geometry::{Point, Rect};
use crate::skin::{SkinAttrDecl, SkinElement, SkinValue, SkinValueKind};
use crate::widget::{
    TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState,
};

use super::scrollbar::{self, BarHit, ScrollBarState, ScrollBarStyle};

static COVER_SKIN_CHILDREN: &[&str] = &["cover"];
static COVER_WALL_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("gap", SkinValueKind::Number),
    SkinAttrDecl::new("corner-radius", SkinValueKind::Number),
    SkinAttrDecl::new("caption-height", SkinValueKind::Number),
    SkinAttrDecl::new("empty-text", SkinValueKind::Text),
];
/// The child vocabulary. `image` is the raw page `ImageId` (a page references
/// pixels by id, not by name — there is no image-name registry, D42).
const COVER_SKIN_ATTRS: &[&str] = &["title", "subtitle", "image", "corner-radius"];

/// One tile of the wall.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cover {
    pub title: String,
    pub subtitle: String,
    /// Artwork registered with [`crate::Ui::set_image`]. `None` (or an id the
    /// page does not hold) paints the gradient placeholder.
    pub image: Option<ImageId>,
    /// Corner radius of this tile's artwork; `None` follows the wall's
    /// [`CoverWall::corner_radius`], `Some(0.0)` is square.
    pub corner_radius: Option<f32>,
}

impl Cover {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: String::new(),
            image: None,
            corner_radius: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    pub fn image(mut self, image: ImageId) -> Self {
        self.image = Some(image);
        self
    }

    /// Override the artwork corner radius for this tile (the wall sets the
    /// default). `0.0` is a square crop.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = Some(radius.max(0.0));
        self
    }
}

pub struct CoverWall<M> {
    id: WidgetId,
    rect: Rect,
    covers: Vec<Cover>,
    /// Target tile width in DIPs; the actual cell grows to fill the row.
    tile: f32,
    gap: f32,
    /// Space reserved under the artwork for the title and subtitle.
    caption_height: f32,
    /// Default artwork corner radius in DIPs (0 = square). A cover can
    /// override it; both the real image and the gradient placeholder follow.
    corner_radius: f32,
    scroll: f32,
    hover: Option<usize>,
    press: Option<usize>,
    bar: ScrollBarState,
    style: ScrollBarStyle,
    /// Narrow repaint for the input that just ran (bar chrome only).
    repaint_hint: Option<Rect>,
    selected: usize,
    empty_text: String,
    on_open: Option<Box<dyn Fn(usize) -> M>>,
    on_select: Option<Box<dyn Fn(usize) -> M>>,
}

/// Thickness of the selection frame in DIPs. Every tile reserves this band
/// around its artwork, so the selected frame never resizes the art.
const SELECTION_FRAME: f32 = 2.0;

/// Placeholder tints, picked by the title hash so a wall without artwork still
/// reads as a collection instead of one repeated block.
const COVER_TINTS: [Color; 8] = [
    Color::from_hex(0x4C6EF5),
    Color::from_hex(0x12B886),
    Color::from_hex(0xE8590C),
    Color::from_hex(0x9C36B5),
    Color::from_hex(0x1C7ED6),
    Color::from_hex(0xC2255C),
    Color::from_hex(0x2F9E44),
    Color::from_hex(0x5F3DC4),
];

impl<M> CoverWall<M> {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 520.0, 320.0),
            covers: Vec::new(),
            tile: 132.0,
            gap: 14.0,
            caption_height: 42.0,
            corner_radius: 0.0,
            scroll: 0.0,
            hover: None,
            press: None,
            bar: ScrollBarState::default(),
            style: ScrollBarStyle::default(),
            repaint_hint: None,
            selected: 0,
            empty_text: String::new(),
            on_open: None,
            on_select: None,
        }
    }

    /// Replace the wall's contents (the typed-access seam also works:
    /// `Ui::edit_widget::<CoverWall<_>>(id, |wall| wall.set_covers(..))`).
    pub fn covers(mut self, covers: Vec<Cover>) -> Self {
        self.set_covers(covers);
        self
    }

    pub fn set_covers(&mut self, covers: Vec<Cover>) {
        self.covers = covers;
        self.selected = self.selected.min(self.covers.len().saturating_sub(1));
        self.hover = None;
        self.press = None;
        self.clamp_scroll();
        self.scroll_to_selected();
    }

    /// Target tile width in DIPs; the row fills the width and the tiles share
    /// the leftover space.
    pub fn tile(mut self, width: f32) -> Self {
        self.tile = width.max(48.0);
        self
    }

    /// Default corner radius of every tile's artwork and placeholder.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.set_corner_radius(radius);
        self
    }

    pub fn set_corner_radius(&mut self, radius: f32) {
        self.corner_radius = radius.max(0.0);
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    pub fn caption_height(mut self, height: f32) -> Self {
        self.caption_height = height.max(0.0);
        self
    }

    /// Message painted in the middle when the wall has no covers.
    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.empty_text = text.into();
        self
    }

    // --- runtime appearance (skin locals) ---------------------------------

    /// Gap between tiles in DIPs. Returns whether it changed.
    pub fn set_gap(&mut self, gap: f32) -> bool {
        let gap = gap.max(0.0);
        if (self.gap - gap).abs() < 0.01 {
            return false;
        }
        self.gap = gap;
        true
    }

    pub fn gap_value(&self) -> f32 {
        self.gap
    }

    pub fn corner_radius_value(&self) -> f32 {
        self.corner_radius
    }

    /// Caption band height under each tile. Returns whether it changed.
    pub fn set_caption_height(&mut self, height: f32) -> bool {
        let height = height.max(0.0);
        if (self.caption_height - height).abs() < 0.01 {
            return false;
        }
        self.caption_height = height;
        true
    }

    pub fn caption_height_value(&self) -> f32 {
        self.caption_height
    }

    /// Message painted when the wall has no covers.
    pub fn set_empty_text(&mut self, text: impl Into<String>) -> bool {
        let text = text.into();
        if self.empty_text == text {
            return false;
        }
        self.empty_text = text;
        true
    }

    pub fn empty_text_value(&self) -> &str {
        &self.empty_text
    }

    /// A cover was opened (double click or Enter/Activate).
    pub fn on_open(mut self, handler: impl Fn(usize) -> M + 'static) -> Self {
        self.on_open = Some(Box::new(handler));
        self
    }

    /// The current cover changed from a click. The widget's own selection
    /// already moved; the message is for the application's "now browsing"
    /// state.
    pub fn on_select(mut self, handler: impl Fn(usize) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    pub fn scrollbar_style(mut self, style: ScrollBarStyle) -> Self {
        self.style = style;
        self
    }

    pub fn scrollbar(&self) -> &ScrollBarStyle {
        &self.style
    }

    pub fn covers_ref(&self) -> &[Cover] {
        &self.covers
    }

    pub fn selected_index(&self) -> usize {
        self.selected.min(self.covers.len().saturating_sub(1))
    }

    pub fn scroll_offset(&self) -> f32 {
        self.scroll
    }

    /// Columns currently in a row (at least one).
    pub fn columns(&self) -> usize {
        self.metrics().0
    }

    /// `(first, end)` cover indices painted in the viewport, end exclusive.
    pub fn visible_range(&self) -> (usize, usize) {
        if self.covers.is_empty() {
            return (0, 0);
        }
        let (cols, _) = self.metrics();
        let row_step = self.row_height() + self.gap;
        let first = (self.scroll / row_step).floor().max(0.0) as usize;
        let rows = (self.rect.height() / row_step).ceil() as usize + 1;
        let end_row = (first + rows).min(self.rows(cols));
        (
            (first * cols).min(self.covers.len()),
            (end_row * cols).min(self.covers.len()),
        )
    }

    /// Scroll so `index` is the top-left-visible row (clamped).
    pub fn scroll_to_index(&mut self, index: usize) -> bool {
        if self.covers.is_empty() {
            return false;
        }
        let (cols, _) = self.metrics();
        let row = index.min(self.covers.len() - 1) / cols;
        let target = (row as f32 * (self.row_height() + self.gap)).clamp(0.0, self.max_scroll());
        if (self.scroll - target).abs() < 0.01 {
            return false;
        }
        self.scroll = target;
        true
    }

    /// Rectangle of `index` in client coordinates (includes the caption; the
    /// artwork is the top square of it).
    pub fn cell_rect(&self, index: usize) -> Option<Rect> {
        (index < self.covers.len()).then(|| self.cell_rect_at(index))
    }

    fn rows(&self, cols: usize) -> usize {
        if self.covers.is_empty() {
            return 0;
        }
        self.covers.len().div_ceil(cols.max(1))
    }

    /// `(columns, cell width)` for the current width. A shown bar takes its
    /// strip out of the row width; because fewer columns only make the content
    /// taller, the two-step decision is stable (never oscillates).
    fn metrics(&self) -> (usize, f32) {
        let (cols, cell) = Self::split(self.rect.width(), self.tile, self.gap);
        if self.content_height_with(cols, cell) > self.rect.height() + 0.5 {
            Self::split(
                (self.rect.width() - self.style.thickness).max(1.0),
                self.tile,
                self.gap,
            )
        } else {
            (cols, cell)
        }
    }

    fn split(available: f32, tile: f32, gap: f32) -> (usize, f32) {
        let tile = tile.max(48.0);
        let cols = (((available + gap) / (tile + gap)).floor() as usize).max(1);
        let cell = ((available - gap * (cols as f32 - 1.0)) / cols as f32).max(1.0);
        (cols, cell)
    }

    fn row_height(&self) -> f32 {
        let (_, cell) = self.metrics();
        cell + self.caption_height
    }

    fn content_height_with(&self, cols: usize, cell: f32) -> f32 {
        let rows = self.rows(cols);
        if rows == 0 {
            return 0.0;
        }
        let row_height = cell + self.caption_height;
        rows as f32 * row_height + (rows.saturating_sub(1)) as f32 * self.gap
    }

    fn content_height(&self) -> f32 {
        let (cols, cell) = self.metrics();
        self.content_height_with(cols, cell)
    }

    fn max_scroll(&self) -> f32 {
        (self.content_height() - self.rect.height()).max(0.0)
    }

    fn has_scrollbar(&self) -> bool {
        self.max_scroll() > 0.5
    }

    /// Area the covers occupy: the widget minus the scrollbar strip.
    fn list_rect(&self) -> Rect {
        if self.has_scrollbar() {
            let track = self.scrollbar_rect();
            Rect::new(self.rect.left, self.rect.top, track.left, self.rect.bottom)
        } else {
            self.rect
        }
    }

    fn scrollbar_rect(&self) -> Rect {
        scrollbar::track_rect(self.rect, true, &self.style)
    }

    fn thumb_rect(&self) -> Option<Rect> {
        if !self.has_scrollbar() {
            return None;
        }
        scrollbar::thumb_rect(
            self.scrollbar_rect(),
            self.content_height(),
            self.rect.height(),
            self.scroll,
            true,
            &self.style,
        )
    }

    fn cell_rect_at(&self, index: usize) -> Rect {
        let (cols, cell) = self.metrics();
        let col = index % cols;
        let row = index / cols;
        let left = self.rect.left + col as f32 * (cell + self.gap);
        let top = self.rect.top + row as f32 * (cell + self.caption_height + self.gap) - self.scroll;
        Rect::from_xywh(left, top, cell, cell + self.caption_height)
    }

    fn art_rect(&self, index: usize) -> Rect {
        let cell = self.cell_rect_at(index);
        Rect::from_xywh(cell.left, cell.top, cell.width(), cell.width())
    }

    fn index_at(&self, pos: Point) -> Option<usize> {
        if self.covers.is_empty() || !self.list_rect().contains_point(pos) {
            return None;
        }
        let (cols, cell) = self.metrics();
        let x = pos.x - self.rect.left;
        let y = pos.y - self.rect.top + self.scroll;
        if x < 0.0 || y < 0.0 {
            return None;
        }
        let col = (x / (cell + self.gap)).floor() as usize;
        let row = (y / (cell + self.caption_height + self.gap)).floor() as usize;
        if col >= cols {
            return None;
        }
        // The gap between cells is not a hit: a click there hits nothing.
        let in_col = x - col as f32 * (cell + self.gap);
        let in_row = y - row as f32 * (cell + self.caption_height + self.gap);
        if in_col > cell || in_row > cell + self.caption_height {
            return None;
        }
        let index = row * cols + col;
        (index < self.covers.len()).then_some(index)
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    fn ensure_visible(&mut self, index: usize) {
        if self.covers.is_empty() {
            return;
        }
        let (cols, _) = self.metrics();
        let row = index.min(self.covers.len() - 1) / cols;
        let row_step = self.row_height() + self.gap;
        let top = row as f32 * row_step;
        let bottom = top + self.row_height();
        if top < self.scroll {
            self.scroll = top;
        } else if bottom > self.scroll + self.rect.height() {
            self.scroll = bottom - self.rect.height();
        }
        self.clamp_scroll();
    }

    fn scroll_to_selected(&mut self) {
        if !self.covers.is_empty() {
            self.ensure_visible(self.selected_index());
        }
    }

    fn select(&mut self, index: usize) -> Option<M> {
        if self.covers.is_empty() {
            return None;
        }
        self.selected = index.min(self.covers.len() - 1);
        self.ensure_visible(self.selected);
        self.on_select
            .as_ref()
            .map(|handler| handler(self.selected))
    }

    /// Select without scrolling. A pointer click lands on a tile the user can
    /// already see; scrolling by the few DIPs that finish a partially visible
    /// row would clip the row above it and read as the grid jumping.
    fn select_visible(&mut self, index: usize) -> Option<M> {
        if self.covers.is_empty() {
            return None;
        }
        self.selected = index.min(self.covers.len() - 1);
        self.on_select
            .as_ref()
            .map(|handler| handler(self.selected))
    }

    fn open(&self, index: usize) -> Option<M> {
        if index >= self.covers.len() {
            return None;
        }
        self.on_open.as_ref().map(|handler| handler(index))
    }

    fn activate(&self) -> Option<M> {
        if self.covers.is_empty() {
            return None;
        }
        self.open(self.selected_index())
    }

    /// Rows that fit the viewport; PageUp/PageDown step by this many.
    fn page_rows(&self) -> usize {
        ((self.rect.height() / (self.row_height() + self.gap)).floor() as usize).max(1)
    }

    fn move_cursor(&mut self, delta: isize) {
        if self.covers.is_empty() {
            return;
        }
        let last = self.covers.len() - 1;
        let next = (self.selected as isize + delta).clamp(0, last as isize) as usize;
        self.selected = next;
        self.ensure_visible(next);
    }

    fn drag_thumb(&mut self, pos: Point, grab: f32) {
        let Some(thumb) = self.thumb_rect() else {
            return;
        };
        self.scroll = scrollbar::drag_offset(
            pos,
            self.scrollbar_rect(),
            thumb,
            grab,
            self.content_height(),
            self.rect.height(),
            true,
        );
        self.clamp_scroll();
    }

    /// Wheel travel for one notch: one row at the default system line count.
    fn wheel_step(&self) -> f32 {
        (self.row_height() + self.gap) * scrollbar::wheel_lines() / 3.0
    }

    fn tint(&self, index: usize) -> Color {
        let mut hash: u32 = 2_166_136_261;
        let title = self.covers.get(index).map(|cover| cover.title.as_str()).unwrap_or("");
        for byte in title.as_bytes() {
            hash ^= *byte as u32;
            hash = hash.wrapping_mul(16_777_619);
        }
        COVER_TINTS[(hash as usize) % COVER_TINTS.len()]
    }

    fn selected_cell_rect(&self) -> Option<Rect> {
        if self.covers.is_empty() {
            return None;
        }
        self.cell_rect_at(self.selected_index())
            .intersect(self.list_rect())
    }
}

impl<M: 'static> Widget<M> for CoverWall<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        COVER_WALL_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("gap" | "corner-radius" | "caption-height", SkinValue::Number(value)) => *value >= 0.0,
            ("empty-text", SkinValue::Text(_)) => true,
            _ => false,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("gap", SkinValue::Number(gap)) => {
                self.set_gap(*gap as f32);
            }
            ("corner-radius", SkinValue::Number(radius)) => {
                self.set_corner_radius(*radius as f32);
            }
            ("caption-height", SkinValue::Number(height)) => {
                self.set_caption_height(*height as f32);
            }
            ("empty-text", SkinValue::Text(text)) => {
                self.set_empty_text(*text);
            }
            _ => {}
        }
    }

    fn skin_children(&self) -> &'static [&'static str] {
        COVER_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(|child| {
            child.name == "cover"
                && child
                    .attr("title")
                    .is_some_and(|title| !title.trim().is_empty())
                && child.attrs_are(COVER_SKIN_ATTRS)
                && child.attr_number_ok("image")
                && child.attr_number_ok("corner-radius")
        })
    }

    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        let covers = children
            .iter()
            .filter_map(|child| {
                let title = child.attr("title")?.to_string();
                let mut cover = Cover::new(title);
                if let Some(subtitle) = child.attr("subtitle") {
                    cover = cover.subtitle(subtitle);
                }
                if let Some(image) = child.attr_number("image")
                    && image >= 0.0
                {
                    cover.image = Some(ImageId(image as u64));
                }
                if let Some(radius) = child.attr_number("corner-radius") {
                    cover.corner_radius = Some(radius.max(0.0) as f32);
                }
                Some(cover)
            })
            .collect();
        self.set_covers(covers);
    }

    fn id(&self) -> WidgetId {
        self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        self.clamp_scroll();
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        320.0
    }

    fn set_selected(&mut self, index: usize) {
        self.selected = index;
        self.scroll_to_selected();
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    /// Only a real hover change repaints (a different cover, the bar tint, or
    /// a thumb drag, which moves the grid).
    fn repaint_on_move_at(&self, pos: Point) -> bool {
        if self.bar.active {
            return true;
        }
        if self.has_scrollbar()
            && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some()
        {
            return !self.bar.hovered;
        }
        self.bar.hovered || self.index_at(pos) != self.hover
    }

    /// Bar-hover/press chrome changes repaint the scrollbar strip only.
    fn take_repaint_hint(&mut self) -> Option<Rect> {
        self.repaint_hint.take()
    }

    fn scrolls_with_wheel(&self) -> bool {
        true
    }

    fn adjustable(&self) -> bool {
        true
    }

    fn focus_ring_rect(&self) -> Rect {
        self.list_rect()
    }

    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        (self.has_scrollbar() && self.scrollbar_rect().contains_point(pos))
            .then_some(Cursor::Arrow)
    }

    fn tooltip_rect(&self) -> Rect {
        self.selected_cell_rect().unwrap_or(self.rect)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.index_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        (!self.covers.is_empty()).then_some(self.selected_index())
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let (start, end) = self.visible_range();
        let list = self.list_rect();
        let children = (start..end)
            .map(|index| {
                let cover = &self.covers[index];
                let name = if cover.subtitle.is_empty() {
                    cover.title.clone()
                } else {
                    format!("{} — {}", cover.title, cover.subtitle)
                };
                let mut rect = self.cell_rect_at(index);
                let offscreen = !rect.intersects(list);
                rect = rect.intersect(list).unwrap_or(rect);
                A11yChild::new(index, name, rect)
                    .selected(index == self.selected_index())
                    .offscreen(offscreen)
            })
            .collect();
        Some(
            A11yInfo::new(A11yRole::List, self.id.0)
                .children(children)
                .scroll(A11yScroll::new().vertical(
                    self.scroll,
                    self.rect.height(),
                    self.content_height(),
                )),
        )
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        self.select(index)
    }

    fn a11y_scroll(&mut self, _horizontal: Option<f32>, vertical: Option<f32>) -> bool {
        let Some(target) = vertical else {
            return false;
        };
        let before = self.scroll;
        self.scroll = target.clamp(0.0, self.max_scroll());
        (self.scroll - before).abs() > 0.01
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        if self.covers.is_empty() {
            if !self.empty_text.is_empty() {
                ctx.text(
                    self.rect,
                    &self.empty_text,
                    TextStyleRole::Small,
                    TextAlign::Center,
                    palette.muted,
                );
            }
            return;
        }
        let (start, end) = self.visible_range();
        ctx.canvas.push_clip(self.rect);
        for index in start..end {
            let cell = self.cell_rect_at(index);
            if !cell.intersects(self.rect) {
                continue;
            }
            // The selection frame lives in a band reserved on *every* tile, so
            // selecting a cover never changes the artwork's size (a frame
            // painted over the image would shrink it and read as the row
            // growing).
            let art = self.art_rect(index);
            let inner = art.inset(SELECTION_FRAME, SELECTION_FRAME);
            let cover = &self.covers[index];
            let selected = index == self.selected_index();
            let hovered = state.hovered && self.hover == Some(index) && !selected;
            let pressed = state.pressed && self.press == Some(index);

            let radius = cover.corner_radius.unwrap_or(self.corner_radius);
            let painted =
                cover
                    .image
                    .is_some_and(|id| ctx.image_round(inner, radius, true, id));
            if !painted {
                let tint = self.tint(index);
                ctx.canvas.vertical_gradient_round(
                    inner,
                    radius,
                    tint.lighten(0.10),
                    tint.darken(0.35),
                );
                if let Some(ch) = cover.title.chars().next() {
                    let glyph = Rect::new(
                        inner.left,
                        inner.top + inner.height() * 0.28,
                        inner.right,
                        inner.bottom - inner.height() * 0.18,
                    );
                    let ink = Color::new(250, 250, 252);
                    ctx.text(
                        glyph,
                        &ch.to_uppercase().to_string(),
                        TextStyleRole::Title,
                        TextAlign::Center,
                        ink,
                    );
                }
            }
            if pressed {
                ctx.canvas.fill_translucent(inner, palette.accent, 0.22);
            } else if hovered {
                ctx.canvas.fill_translucent(inner, palette.accent, 0.10);
            }
            if selected {
                // A 2 DIP accent frame marks the current cover even when the
                // ring is not drawn (pointer focus). It occupies the reserved
                // band, never the artwork.
                let (l, t, r, b) = (art.left, art.top, art.right, art.bottom);
                let f = SELECTION_FRAME;
                ctx.canvas.fill_rect(Rect::new(l, t, r, t + f), palette.accent);
                ctx.canvas.fill_rect(Rect::new(l, b - f, r, b), palette.accent);
                ctx.canvas.fill_rect(Rect::new(l, t, l + f, b), palette.accent);
                ctx.canvas.fill_rect(Rect::new(r - f, t, r, b), palette.accent);
            }

            let title_rect = Rect::new(
                cell.left + 2.0,
                art.bottom + 4.0,
                cell.right - 2.0,
                (art.bottom + 24.0).min(cell.bottom),
            );
            ctx.text_overflow(
                title_rect,
                &cover.title,
                TextStyleRole::Body,
                TextAlign::Left,
                if selected { palette.accent } else { palette.text },
                TextOverflow::Ellipsis,
            );
            if !cover.subtitle.is_empty() && self.caption_height >= 34.0 {
                let subtitle_rect = Rect::new(
                    cell.left + 2.0,
                    title_rect.bottom,
                    cell.right - 2.0,
                    cell.bottom - 2.0,
                );
                ctx.text_overflow(
                    subtitle_rect,
                    &cover.subtitle,
                    TextStyleRole::Small,
                    TextAlign::Left,
                    palette.muted,
                    TextOverflow::Ellipsis,
                );
            }
        }
        ctx.canvas.pop_clip();

        if self.has_scrollbar() {
            scrollbar::paint(
                ctx,
                self.scrollbar_rect(),
                self.thumb_rect(),
                &self.bar,
                &self.style,
            );
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        match input {
            WidgetInput::HoverAt(pos) => {
                let over_bar = self.has_scrollbar()
                    && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some();
                let bar_before = self.bar.hovered;
                let cells_before = self.hover;
                if over_bar {
                    self.bar.hovered = true;
                    self.hover = None;
                } else {
                    self.bar.hovered = false;
                    self.hover = self.index_at(pos);
                }
                if self.bar.hovered != bar_before && self.hover == cells_before {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::HoverLeave => {
                let bar_before = self.bar.hovered;
                let cells_before = self.hover;
                self.hover = None;
                if !self.bar.active {
                    self.bar.hovered = false;
                }
                if self.bar.hovered != bar_before && self.hover == cells_before {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::PressAt(pos) => {
                if self.has_scrollbar() {
                    let track = self.scrollbar_rect();
                    let thumb = self.thumb_rect();
                    match scrollbar::hit(track, thumb, pos) {
                        Some(BarHit::Thumb) => {
                            if let Some(thumb) = thumb {
                                self.bar.active = true;
                                self.bar.hovered = true;
                                self.bar.grab = pos.y - thumb.top;
                                self.repaint_hint = Some(self.scrollbar_rect());
                                return None;
                            }
                        }
                        Some(BarHit::Track) => {
                            if let Some(thumb) = thumb {
                                self.scroll = scrollbar::track_click_offset(
                                    pos,
                                    track,
                                    thumb,
                                    self.content_height(),
                                    self.rect.height(),
                                    self.scroll,
                                    true,
                                    &self.style,
                                );
                                self.clamp_scroll();
                                self.bar.active = true;
                                self.bar.hovered = true;
                                self.bar.grab = thumb.height() * 0.5;
                                return None;
                            }
                        }
                        None => {}
                    }
                }
                self.press = self.index_at(pos);
                None
            }
            WidgetInput::DragTo(pos) => {
                if self.bar.active {
                    self.drag_thumb(pos, self.bar.grab);
                }
                None
            }
            WidgetInput::ReleaseAt(pos) => {
                if self.bar.active {
                    self.bar.active = false;
                    self.repaint_hint = Some(self.scrollbar_rect());
                    return None;
                }
                let pressed = self.press.take();
                let hit = self.index_at(pos);
                if let (Some(pressed), Some(hit)) = (pressed, hit)
                    && pressed == hit
                {
                    return self.select_visible(hit);
                }
                None
            }
            WidgetInput::ClickAt(pos) => {
                // Selection already ran on the release; a bar click changes
                // nothing and must not repaint the grid underneath it.
                if self.has_scrollbar()
                    && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some()
                {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::DoubleClickAt(pos) => {
                let index = self.index_at(pos)?;
                self.selected = index;
                self.open(index)
            }
            WidgetInput::Wheel(delta) => {
                if self.max_scroll() <= 0.0 {
                    return None;
                }
                self.scroll -= delta * self.wheel_step();
                self.clamp_scroll();
                None
            }
            WidgetInput::Key(key) => {
                let (cols, _) = self.metrics();
                let page = self.page_rows() * cols;
                match key.code {
                    KeyCode::Left => self.move_cursor(-1),
                    KeyCode::Right => self.move_cursor(1),
                    KeyCode::Up => self.move_cursor(-(cols as isize)),
                    KeyCode::Down => self.move_cursor(cols as isize),
                    KeyCode::PageUp => self.move_cursor(-(page as isize)),
                    KeyCode::PageDown => self.move_cursor(page as isize),
                    KeyCode::Home => self.move_cursor(-(self.covers.len() as isize)),
                    KeyCode::End => self.move_cursor(self.covers.len() as isize),
                    KeyCode::Enter => return self.activate(),
                    _ => {}
                }
                None
            }
            WidgetInput::Activate => self.activate(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall(count: usize) -> CoverWall<usize> {
        let mut wall: CoverWall<usize> = CoverWall::new("wall");
        wall.set_rect(Rect::from_xywh(0.0, 0.0, 520.0, 300.0));
        wall.set_covers(
            (0..count)
                .map(|index| Cover::new(format!("专辑 {index}")).subtitle("歌手"))
                .collect(),
        );
        wall
    }

    fn key(code: KeyCode) -> WidgetInput {
        WidgetInput::Key(crate::event::KeyEvent {
            code,
            vk: 0,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        })
    }

    #[test]
    fn tiles_fill_the_row_and_the_bar_takes_its_strip() {
        // 520 wide, target 132, gap 14: (520+14)/146 = 3.65 → 3 columns.
        let wall = wall(12);
        assert_eq!(wall.columns(), 3);
        // The row shrinks by the 10 DIP bar and still fits three tiles.
        let (cols, cell) = wall.metrics();
        assert_eq!(cols, 3);
        assert!((cell * 3.0 + 28.0 - (520.0 - 10.0)).abs() < 0.01);
        // Twelve covers make four rows; the content overflows the 300 DIP
        // viewport, so the bar is shown.
        assert!(wall.has_scrollbar());
        assert!(wall.max_scroll() > 0.0);
    }

    #[test]
    fn a_short_wall_has_no_bar_and_keeps_the_full_width() {
        let wall = wall(2);
        assert!(!wall.has_scrollbar());
        assert_eq!(wall.list_rect().width(), 520.0);
        assert!((wall.max_scroll() - 0.0).abs() < 0.01);
    }

    #[test]
    fn hit_testing_maps_the_grid_and_skips_the_gaps() {
        let wall = wall(12);
        let cell = wall.cell_rect_at(0);
        assert_eq!(wall.index_at(cell.center()), Some(0));
        // A point in the gap between cell 0 and cell 1 hits nothing.
        let gap = Point::new(cell.right + 2.0, cell.top + 10.0);
        assert_eq!(wall.index_at(gap), None);
        // The 1-2 cell is the second column of the first row.
        let second = wall.cell_rect_at(1);
        assert_eq!(wall.index_at(second.center()), Some(1));
        // Below the caption band is the gap between rows.
        let below = Point::new(cell.center().x, cell.bottom + 2.0);
        assert_eq!(wall.index_at(below), None);
    }

    #[test]
    fn arrows_move_the_cursor_by_a_row_and_keep_it_visible() {
        let mut wall = wall(50);
        assert_eq!(wall.selected_index(), 0);
        wall.input(key(KeyCode::Right));
        assert_eq!(wall.selected_index(), 1);
        wall.input(key(KeyCode::Down));
        assert_eq!(wall.selected_index(), 1 + wall.columns());
        wall.input(key(KeyCode::Left));
        wall.input(key(KeyCode::Left));
        assert_eq!(wall.selected_index(), wall.columns() - 1);
        // Up from the first cell of the second row clamps at the first cover
        // (no wrap into the previous row's last cell).
        wall.input(key(KeyCode::Up));
        assert_eq!(wall.selected_index(), 0);
        wall.input(key(KeyCode::Home));
        assert_eq!(wall.selected_index(), 0);
        assert_eq!(wall.scroll_offset(), 0.0);
        // End scrolls the last row into view.
        wall.input(key(KeyCode::End));
        assert_eq!(wall.selected_index(), 49);
        assert!(wall.scroll_offset() > 0.0);
        assert!(wall.scroll_offset() <= wall.max_scroll() + 0.01);
    }

    #[test]
    fn clicking_selects_and_double_clicking_opens() {
        let mut wall: CoverWall<&'static str> =
            CoverWall::new("wall").on_open(|index| if index == 1 { "open 1" } else { "open" });
        wall.set_rect(Rect::from_xywh(0.0, 0.0, 520.0, 300.0));
        wall.set_covers((0..12).map(|i| Cover::new(format!("c{i}"))).collect());
        let cell = wall.cell_rect_at(1).center();
        wall.input(WidgetInput::PressAt(cell));
        assert_eq!(wall.input(WidgetInput::ReleaseAt(cell)), None);
        assert_eq!(wall.selected_index(), 1);
        assert_eq!(wall.input(WidgetInput::DoubleClickAt(cell)), Some("open 1"));
        assert_eq!(wall.input(WidgetInput::Activate), Some("open 1"));
    }

    #[test]
    fn wheel_scrolls_one_row_per_notch_and_clamps() {
        let mut wall = wall(50);
        wall.input(WidgetInput::Wheel(-1.0));
        let row = wall.row_height() + wall.gap;
        assert!((wall.scroll_offset() - row).abs() < 0.01);
        wall.input(WidgetInput::Wheel(-10_000.0));
        assert!((wall.scroll_offset() - wall.max_scroll()).abs() < 0.01);
        wall.input(WidgetInput::Wheel(10_000.0));
        assert_eq!(wall.scroll_offset(), 0.0);
    }

    #[test]
    fn a11y_publishes_visible_covers_with_the_current_one_selected() {
        let mut wall = wall(50);
        wall.set_selected(4);
        let info = wall.a11y().expect("the wall is exposed");
        assert_eq!(info.role, A11yRole::List);
        assert!(!info.children.is_empty());
        let axis = info.scroll.expect("scroll state").vertical.expect("vertical");
        assert_eq!(axis.viewport, 300.0);
        assert!(axis.content > 300.0);
        let selected: Vec<usize> = info
            .children
            .iter()
            .filter(|child| child.selected)
            .map(|child| child.index)
            .collect();
        assert_eq!(selected, vec![4]);
        // The first cover is in the viewport; the look-ahead row (published so
        // the next PageDown row is already in the tree) says it is offscreen.
        assert!(!info.children[0].offscreen);
        assert!(info.children.iter().any(|child| child.offscreen));
        // A UIA selection request moves the cursor.
        assert_eq!(wall.a11y_select_child(7), None);
        assert_eq!(wall.selected_index(), 7);
    }

    #[test]
    fn selection_never_changes_the_tile_geometry() {
        let mut wall = wall(50);
        let before: Vec<(f32, f32)> = (0..9)
            .map(|index| {
                let cell = wall.cell_rect(index).unwrap();
                (cell.width(), cell.height())
            })
            .collect();
        let content = wall.content_height();
        let columns = wall.columns();
        wall.set_selected(4);
        for (index, (width, height)) in before.iter().enumerate() {
            let cell = wall.cell_rect(index).unwrap();
            assert!(
                (cell.width() - width).abs() < 0.01 && (cell.height() - height).abs() < 0.01,
                "cell {index} resized with its state: {cell:?}"
            );
        }
        assert_eq!(wall.content_height(), content);
        assert_eq!(wall.columns(), columns);
    }

    #[test]
    fn a_pointer_click_does_not_scroll_a_partially_visible_row() {
        // 300 DIP tall: the second row is partially visible. Clicking it must
        // not scroll the few DIPs that finish the row (that clipped the row
        // above and read as the grid jumping); keyboard/typed selection still
        // reveals it.
        let mut wall = wall(50);
        assert_eq!(wall.scroll_offset(), 0.0);
        let row_two = wall.cell_rect(3).unwrap();
        let point = Point::new(row_two.center().x, row_two.top + 10.0);
        wall.input(WidgetInput::PressAt(point));
        assert_eq!(wall.input(WidgetInput::ReleaseAt(point)), None);
        assert_eq!(wall.selected_index(), 3);
        assert_eq!(wall.scroll_offset(), 0.0, "a click never reveals-scrolls");
        wall.set_selected(3);
        assert!(wall.scroll_offset() > 0.0, "typed selection reveals the row");
    }

    #[test]
    fn bar_hover_and_press_repaint_only_the_bar() {
        let mut wall = wall(50);
        let track = wall.scrollbar_rect();
        let thumb = wall.thumb_rect().expect("overflow");
        let over = Point::new(track.center().x, thumb.center().y);
        wall.input(WidgetInput::HoverAt(over));
        assert!(wall.bar.hovered);
        assert!(wall.take_repaint_hint().is_some());
        wall.input(WidgetInput::PressAt(over));
        assert!(wall.bar.active);
        let hint = wall.take_repaint_hint().expect("bar-only repaint");
        assert_eq!(hint, track);
        let before = wall.scroll_offset();
        wall.input(WidgetInput::DragTo(Point::new(over.x, over.y + 40.0)));
        assert!(wall.scroll_offset() > before, "dragging the thumb scrolls");
        wall.input(WidgetInput::ReleaseAt(Point::new(over.x, over.y + 40.0)));
        assert!(!wall.bar.active);
    }

    #[test]
    fn corner_radius_is_a_wall_default_with_a_per_cover_override() {
        let mut wall = wall(5);
        assert_eq!(wall.corner_radius, 0.0, "square by default");
        wall.set_corner_radius(6.0);
        let cover = Cover::new("rounded").corner_radius(2.0);
        let square = Cover::new("square").corner_radius(0.0);
        assert_eq!(cover.corner_radius, Some(2.0));
        assert_eq!(square.corner_radius, Some(0.0));
        let plain = Cover::new("plain");
        assert_eq!(plain.corner_radius, None, "an unset cover follows the wall");
        wall.set_corner_radius(-4.0);
        assert_eq!(wall.corner_radius, 0.0, "a negative radius clamps to square");
    }

    #[test]
    fn set_covers_clamps_the_cursor_and_images_are_optional() {
        let mut wall = wall(5);
        wall.set_selected(4);
        wall.set_covers(vec![Cover::new("only").image(ImageId(9))]);
        assert_eq!(wall.selected_index(), 0);
        assert_eq!(wall.covers_ref()[0].image, Some(ImageId(9)));
        assert!(!wall.has_scrollbar());
    }
}
