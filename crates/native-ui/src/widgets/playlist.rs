// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Playlist: a virtualized, reorderable track list for the player.
//!
//! Rows reuse [`Row`]: title (track), subtitle (artist), trailing (duration)
//! and `current` (the track that is playing). A click selects, a double click
//! or Enter plays ([`PlaylistView::on_activate`]), and pressing a row and
//! dragging it moves the track: the widget paints an insertion line,
//! auto-scrolls near the viewport edges and reports the move through
//! [`PlaylistView::on_reorder`]. The application owns the data and applies the
//! move itself; the widget never reorders its copy, so the two cannot drift.
//!
//! The keyboard mirrors the pointer: arrows move the cursor, `Alt+Up/Down`
//! moves the selected track (keyboard reorder), Enter plays. Wheel, focus
//! reveal, the shared themed scrollbar and the UIA list/selection/scroll
//! patterns match the other scrollables.

use crate::a11y::{A11yChild, A11yInfo, A11yRole, A11yScroll};
use crate::backend::{TextAlign, TextOverflow};
use crate::event::{Cursor, KeyCode};
use crate::geometry::{Point, Rect};
use crate::skin::{SkinAttrDecl, SkinElement, SkinValue, SkinValueKind};
use crate::widget::{Row, TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

use super::scrollbar::{self, BarHit, ScrollBarState, ScrollBarStyle};

static PLAYLIST_SKIN_CHILDREN: &[&str] = &["track"];
static PLAYLIST_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("row-height", SkinValueKind::Number),
    SkinAttrDecl::new("alternating", SkinValueKind::Bool),
    SkinAttrDecl::new("empty-text", SkinValueKind::Text),
];

/// An in-flight drag: the track being moved and the insertion slot
/// (`0..=rows.len()` in *pre-removal* index space).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Drag {
    from: usize,
    insert: usize,
}

pub struct PlaylistView<M> {
    id: WidgetId,
    rect: Rect,
    rows: Vec<Row>,
    row_height: f32,
    scroll: f32,
    hover: Option<usize>,
    press: Option<usize>,
    drag: Option<Drag>,
    bar: ScrollBarState,
    style: ScrollBarStyle,
    /// Narrow repaint for the input that just ran (bar chrome only).
    repaint_hint: Option<Rect>,
    selected: usize,
    empty_text: String,
    alternating: bool,
    on_select: Option<Box<dyn Fn(usize) -> M>>,
    on_activate: Option<Box<dyn Fn(usize) -> M>>,
    on_reorder: Option<Box<dyn Fn(usize, usize) -> M>>,
}

impl<M> PlaylistView<M> {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 320.0, 360.0),
            rows: Vec::new(),
            row_height: 44.0,
            scroll: 0.0,
            hover: None,
            press: None,
            drag: None,
            bar: ScrollBarState::default(),
            style: ScrollBarStyle::default(),
            repaint_hint: None,
            selected: 0,
            empty_text: String::new(),
            alternating: false,
            on_select: None,
            on_activate: None,
            on_reorder: None,
        }
    }

    /// Replace the tracks (the typed-access seam also works:
    /// `Ui::edit_widget::<PlaylistView<_>>(id, |list| list.set_rows(..))`).
    pub fn rows(mut self, rows: Vec<Row>) -> Self {
        self.set_rows(rows);
        self
    }

    pub fn set_rows(&mut self, rows: Vec<Row>) {
        self.rows = rows;
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
        self.hover = None;
        self.press = None;
        self.drag = None;
        self.clamp_scroll();
        self.scroll_to_selected();
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(24.0);
        self
    }

    /// Message painted in the middle of an empty list ("队列为空").
    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.empty_text = text.into();
        self
    }

    /// Tint every other row with the panel color for easier scanning.
    pub fn alternating(mut self, alternating: bool) -> Self {
        self.alternating = alternating;
        self
    }

    // --- runtime appearance (skin locals) ---------------------------------

    /// Row height in DIPs (24 minimum); scroll and selection follow. Returns
    /// whether it changed.
    pub fn set_row_height(&mut self, height: f32) -> bool {
        let height = height.max(24.0);
        if (self.row_height - height).abs() < 0.01 {
            return false;
        }
        self.row_height = height;
        self.clamp_scroll();
        self.scroll_to_selected();
        true
    }

    pub fn row_height_value(&self) -> f32 {
        self.row_height
    }

    /// Fill alternating rows with the panel color. Returns whether it changed.
    pub fn set_alternating(&mut self, alternating: bool) -> bool {
        if self.alternating == alternating {
            return false;
        }
        self.alternating = alternating;
        true
    }

    pub fn is_alternating(&self) -> bool {
        self.alternating
    }

    /// Message painted while there are no tracks.
    pub fn set_empty_text(&mut self, text: impl Into<String>) -> bool {
        let text = text.into();
        if self.empty_text == text {
            return false;
        }
        self.empty_text = text;
        true
    }

    pub fn empty_text_ref(&self) -> &str {
        &self.empty_text
    }

    /// The current track changed by a click. The widget's cursor already moved;
    /// the message is for the application's "now browsing" state.
    pub fn on_select(mut self, handler: impl Fn(usize) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    /// A track was played (double click, Enter or `Activate`).
    pub fn on_activate(mut self, handler: impl Fn(usize) -> M + 'static) -> Self {
        self.on_activate = Some(Box::new(handler));
        self
    }

    /// A track was moved. `to` is the final index in the list *after* the
    /// removal, so the application applies it as
    /// `let track = rows.remove(from); rows.insert(to, track);` and pushes the
    /// reordered rows back with `set_rows`. The widget moves its cursor to
    /// `to`; a `from == to` drop emits nothing.
    pub fn on_reorder(mut self, handler: impl Fn(usize, usize) -> M + 'static) -> Self {
        self.on_reorder = Some(Box::new(handler));
        self
    }

    pub fn scrollbar_style(mut self, style: ScrollBarStyle) -> Self {
        self.style = style;
        self
    }

    pub fn scrollbar(&self) -> &ScrollBarStyle {
        &self.style
    }

    pub fn rows_ref(&self) -> &[Row] {
        &self.rows
    }

    /// Cursor index (`0` for an empty list).
    pub fn selected_index(&self) -> usize {
        self.selected.min(self.rows.len().saturating_sub(1))
    }

    /// Current scroll offset in DIPs.
    pub fn scroll_offset(&self) -> f32 {
        self.scroll
    }

    /// `(from, insertion slot)` while a drag is in flight. The slot is in
    /// pre-removal index space (`0..=len`); a drop there reports
    /// `to = slot - 1` when the slot is below `from`.
    pub fn drag_state(&self) -> Option<(usize, usize)> {
        self.drag.map(|drag| (drag.from, drag.insert))
    }

    /// `(first, end)` published viewport rows, end exclusive.
    pub fn visible_range(&self) -> (usize, usize) {
        if self.rows.is_empty() {
            return (0, 0);
        }
        let start = (self.scroll / self.row_height).floor().max(0.0) as usize;
        let rows = (self.rect.height() / self.row_height).ceil() as usize + 1;
        let end = (start + rows).min(self.rows.len());
        (start.min(self.rows.len()), end)
    }

    /// Rectangle of `index` in client coordinates (clipped to the list area).
    pub fn row_rect(&self, index: usize) -> Option<Rect> {
        (index < self.rows.len()).then(|| self.row_rect_at(index))
    }

    /// Scroll so `row` is at the top of the viewport (clamped).
    pub fn scroll_to_row(&mut self, row: usize) -> bool {
        if self.rows.is_empty() {
            return false;
        }
        let row = row.min(self.rows.len() - 1);
        let target = (row as f32 * self.row_height).clamp(0.0, self.max_scroll());
        if (self.scroll - target).abs() < 0.01 {
            return false;
        }
        self.scroll = target;
        true
    }

    /// Scroll the minimum distance that makes `row` fully visible.
    pub fn ensure_row_visible(&mut self, row: usize) -> bool {
        if self.rows.is_empty() {
            return false;
        }
        let row = row.min(self.rows.len() - 1);
        let before = self.scroll;
        let top = row as f32 * self.row_height;
        let bottom = top + self.row_height;
        if top < self.scroll {
            self.scroll = top;
        } else if bottom > self.scroll + self.rect.height() {
            self.scroll = bottom - self.rect.height();
        }
        self.clamp_scroll();
        (self.scroll - before).abs() > 0.01
    }

    fn row_rect_at(&self, index: usize) -> Rect {
        let top = self.rect.top + index as f32 * self.row_height - self.scroll;
        Rect::new(self.rect.left, top, self.list_rect().right, top + self.row_height)
    }

    fn content_height(&self) -> f32 {
        self.rows.len() as f32 * self.row_height
    }

    fn max_scroll(&self) -> f32 {
        (self.content_height() - self.rect.height()).max(0.0)
    }

    fn has_scrollbar(&self) -> bool {
        self.max_scroll() > 0.5
    }

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

    fn row_at(&self, pos: Point) -> Option<usize> {
        if self.rows.is_empty() || !self.list_rect().contains_point(pos) {
            return None;
        }
        let y = pos.y - self.rect.top + self.scroll;
        if y < 0.0 {
            return None;
        }
        let index = (y / self.row_height).floor() as usize;
        (index < self.rows.len()).then_some(index)
    }

    /// Insertion slot (`0..=len`) for a pointer position: the nearest gap
    /// between rows, clamped to the ends. Positions outside the widget (a drag
    /// above/below it) clamp to the first/last slot.
    fn insertion_at(&self, pos: Point) -> usize {
        let y = pos.y - self.rect.top + self.scroll;
        let slot = (y / self.row_height).round();
        slot.clamp(0.0, self.rows.len() as f32) as usize
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    fn page_rows(&self) -> usize {
        ((self.rect.height() / self.row_height).floor() as usize).max(1)
    }

    fn scroll_to_selected(&mut self) {
        if !self.rows.is_empty() {
            self.ensure_row_visible(self.selected_index());
        }
    }

    fn select(&mut self, index: usize, reveal: bool) -> Option<M> {
        if self.rows.is_empty() {
            return None;
        }
        self.selected = index.min(self.rows.len() - 1);
        if reveal {
            self.ensure_row_visible(self.selected);
        }
        if self.rows[self.selected].disabled {
            return None;
        }
        self.on_select.as_ref().map(|handler| handler(self.selected))
    }

    fn activate(&self) -> Option<M> {
        if self.rows.is_empty() {
            return None;
        }
        let index = self.selected_index();
        if self.rows[index].disabled {
            return None;
        }
        self.on_activate.as_ref().map(|handler| handler(index))
    }

    /// Move the cursor and its track in the list (keyboard reorder).
    fn reorder_selected(&mut self, delta: isize) -> Option<M> {
        if self.rows.is_empty() || self.drag.is_some() {
            return None;
        }
        let from = self.selected_index();
        if self.rows[from].disabled {
            return None;
        }
        let to = from as isize + delta;
        if to < 0 || to >= self.rows.len() as isize {
            return None;
        }
        let to = to as usize;
        self.selected = to;
        self.ensure_row_visible(to);
        self.on_reorder.as_ref().map(|handler| handler(from, to))
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

    /// Update the insertion slot for a drag. Holding the pointer near the top
    /// or bottom edge scrolls the list by half a row per move, so a long
    /// playlist can be reordered past the fold.
    fn drag_to(&mut self, pos: Point) {
        if self.drag.is_none() {
            return;
        }
        let edge = 24.0f32.min(self.row_height);
        if pos.y < self.rect.top + edge {
            self.scroll -= self.row_height * 0.5;
        } else if pos.y > self.rect.bottom - edge {
            self.scroll += self.row_height * 0.5;
        }
        self.clamp_scroll();
        let insert = self.insertion_at(pos);
        if let Some(drag) = self.drag.as_mut() {
            drag.insert = insert;
        }
    }

    fn selected_row_rect(&self) -> Option<Rect> {
        if self.rows.is_empty() {
            return None;
        }
        self.row_rect_at(self.selected_index())
            .intersect(self.list_rect())
    }
}

impl<M: 'static> Widget<M> for PlaylistView<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        PLAYLIST_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("row-height", SkinValue::Number(height)) => *height >= 24.0,
            ("alternating", SkinValue::Bool(_)) => true,
            ("empty-text", SkinValue::Text(_)) => true,
            _ => false,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("row-height", SkinValue::Number(height)) => {
                self.set_row_height(*height as f32);
            }
            ("alternating", SkinValue::Bool(alternating)) => {
                self.set_alternating(*alternating);
            }
            ("empty-text", SkinValue::Text(text)) => {
                self.set_empty_text(*text);
            }
            _ => {}
        }
    }

    fn skin_children(&self) -> &'static [&'static str] {
        PLAYLIST_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children
            .iter()
            .all(|child| child.name == "track" && Row::valid_skin_row(child))
    }

    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        let rows = children.iter().filter_map(Row::from_skin).collect();
        self.set_rows(rows);
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
        260.0
    }

    fn set_rows(&mut self, rows: Vec<Row>) {
        self.rows = rows;
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
        self.hover = None;
        self.press = None;
        self.drag = None;
        self.clamp_scroll();
        self.scroll_to_selected();
    }

    fn set_selected(&mut self, index: usize) {
        self.selected = index;
        self.scroll_to_selected();
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    /// A drag repaints on every move; otherwise only a real hover change
    /// (a different row, the bar tint) does.
    fn repaint_on_move_at(&self, pos: Point) -> bool {
        if self.drag.is_some() || self.bar.active {
            return true;
        }
        let over_bar = self.has_scrollbar()
            && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some();
        if over_bar {
            return !self.bar.hovered;
        }
        self.bar.hovered || self.row_at(pos) != self.hover
    }

    /// Bar-hover/press chrome changes repaint the scrollbar strip only.
    fn take_repaint_hint(&mut self) -> Option<Rect> {
        self.repaint_hint.take()
    }

    /// The list scrolls its own rows with the wheel, so a scroll view around it
    /// leaves the notch to the list (D65).
    fn scrolls_with_wheel(&self) -> bool {
        true
    }

    /// Enter enters row navigation; arrows/PageUp/PageDown/Home/End move the
    /// cursor, Enter again plays, `Alt+Up/Down` moves the track.
    fn adjustable(&self) -> bool {
        true
    }

    fn focus_ring_rect(&self) -> Rect {
        self.list_rect()
    }

    /// A scrollbar track shows the arrow (a bar press owns the pointer).
    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        (self.has_scrollbar() && self.scrollbar_rect().contains_point(pos))
            .then_some(Cursor::Arrow)
    }

    fn tooltip_rect(&self) -> Rect {
        self.selected_row_rect().unwrap_or(self.rect)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.row_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        (!self.rows.is_empty()).then_some(self.selected_index())
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let (start, end) = self.visible_range();
        let list = self.list_rect();
        let children = (start..end)
            .map(|index| {
                let row = &self.rows[index];
                let name = if row.subtitle.is_empty() {
                    row.title.clone()
                } else {
                    format!("{} — {}", row.title, row.subtitle)
                };
                let rect = self.row_rect_at(index);
                let offscreen = !rect.intersects(list);
                let rect = rect.intersect(list).unwrap_or(rect);
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
        self.select(index, true)
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
        if self.rows.is_empty() {
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
        let list = self.list_rect();
        ctx.canvas.push_clip(list);
        for index in start..end {
            let row_rect = self.row_rect_at(index);
            if !row_rect.intersects(list) {
                continue;
            }
            let row = &self.rows[index];
            let dragging = self.drag.is_some_and(|drag| drag.from == index);
            if self.alternating && index % 2 == 1 {
                ctx.canvas.fill_rect(row_rect, palette.panel);
            }
            if row.current {
                ctx.canvas.fill_rect(row_rect, palette.row_selected);
                ctx.canvas.fill_rect(
                    Rect::new(
                        row_rect.left,
                        row_rect.top,
                        row_rect.left + 3.0,
                        row_rect.bottom,
                    ),
                    palette.accent,
                );
            } else if !dragging && state.pressed && self.press == Some(index) {
                ctx.canvas.fill_rect(row_rect, palette.row_pressed);
            } else if !dragging && state.hovered && self.hover == Some(index) {
                ctx.canvas.fill_rect(row_rect, palette.row_hover);
            } else if !dragging && state.focused && index == self.selected_index() {
                // Keyboard row cursor (arrow/PageUp/Home navigation).
                ctx.canvas.fill_rect(row_rect, palette.row_hover);
            }
            let title_color = if row.disabled || dragging {
                palette.text_disabled
            } else if row.current {
                palette.accent
            } else {
                palette.text
            };
            let text_left = row_rect.left + 12.0;
            let text_right = row_rect.right - 12.0;
            if row.subtitle.is_empty() {
                ctx.text_overflow(
                    Rect::new(text_left, row_rect.top, text_right, row_rect.bottom),
                    &row.title,
                    TextStyleRole::Body,
                    TextAlign::Left,
                    title_color,
                    TextOverflow::Ellipsis,
                );
            } else {
                ctx.text_overflow(
                    Rect::new(text_left, row_rect.top + 3.0, text_right, row_rect.top + 24.0),
                    &row.title,
                    TextStyleRole::Body,
                    TextAlign::Left,
                    title_color,
                    TextOverflow::Ellipsis,
                );
                ctx.text_overflow(
                    Rect::new(
                        text_left,
                        row_rect.top + 24.0,
                        text_right,
                        row_rect.bottom - 2.0,
                    ),
                    &row.subtitle,
                    TextStyleRole::Small,
                    TextAlign::Left,
                    palette.muted,
                    TextOverflow::Ellipsis,
                );
            }
            if !row.trailing.is_empty() {
                ctx.text(
                    Rect::new(
                        row_rect.right - 76.0,
                        row_rect.top,
                        row_rect.right - 12.0,
                        row_rect.bottom,
                    ),
                    &row.trailing,
                    TextStyleRole::Small,
                    TextAlign::Right,
                    palette.muted,
                );
            }
        }
        // Insertion line: where the dragged track lands. Drawn over the rows
        // and clipped to the viewport, so it stays visible while scrolling.
        if let Some(drag) = self.drag {
            let y = (self.rect.top + drag.insert as f32 * self.row_height - self.scroll)
                .clamp(list.top, list.bottom - 2.0);
            ctx.canvas
                .fill_rect(Rect::new(list.left, y, list.right, y + 2.0), palette.accent);
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
                if self.drag.is_some() {
                    return None;
                }
                let over_bar = self.has_scrollbar()
                    && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some();
                let bar_before = self.bar.hovered;
                let rows_before = self.hover;
                if over_bar {
                    self.bar.hovered = true;
                    self.hover = None;
                } else {
                    self.bar.hovered = false;
                    self.hover = self.row_at(pos);
                }
                if self.bar.hovered != bar_before && self.hover == rows_before {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::HoverLeave => {
                let bar_before = self.bar.hovered;
                let rows_before = self.hover;
                self.hover = None;
                if !self.bar.active {
                    self.bar.hovered = false;
                }
                if self.bar.hovered != bar_before && self.hover == rows_before {
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
                self.press = self.row_at(pos);
                None
            }
            WidgetInput::DragStart(_) => {
                if let Some(from) = self.press {
                    if !self.rows[from].disabled {
                        self.drag = Some(Drag {
                            from,
                            insert: from,
                        });
                    }
                }
                None
            }
            WidgetInput::DragTo(pos) => {
                if self.bar.active {
                    self.drag_thumb(pos, self.bar.grab);
                } else {
                    self.drag_to(pos);
                }
                None
            }
            WidgetInput::ReleaseAt(pos) => {
                if self.bar.active {
                    self.bar.active = false;
                    self.repaint_hint = Some(self.scrollbar_rect());
                    return None;
                }
                if let Some(drag) = self.drag.take() {
                    self.press = None;
                    // The slot is in pre-removal space: dropping below the
                    // source shifts every later index down by one.
                    let to = if drag.insert > drag.from {
                        drag.insert - 1
                    } else {
                        drag.insert
                    };
                    if to == drag.from {
                        return None;
                    }
                    self.selected = to;
                    self.ensure_row_visible(to);
                    return self.on_reorder.as_ref().map(|handler| handler(drag.from, to));
                }
                let pressed = self.press.take();
                let hit = self.row_at(pos);
                if let (Some(pressed), Some(hit)) = (pressed, hit)
                    && pressed == hit
                    && !self.rows[hit].disabled
                {
                    return self.select(hit, false);
                }
                None
            }
            WidgetInput::ClickAt(pos) => {
                // Selection/play already ran on the release; a bar click
                // changes nothing and must not repaint the rows.
                if self.has_scrollbar()
                    && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some()
                {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::DoubleClickAt(pos) => {
                let index = self.row_at(pos)?;
                if self.rows[index].disabled {
                    return None;
                }
                self.selected = index;
                self.ensure_row_visible(index);
                self.on_activate.as_ref().map(|handler| handler(index))
            }
            WidgetInput::Wheel(delta) => {
                if self.max_scroll() <= 0.0 {
                    return None;
                }
                let step = self.row_height * scrollbar::wheel_lines();
                self.scroll -= delta * step;
                self.clamp_scroll();
                None
            }
            WidgetInput::Key(key) => {
                if key.alt {
                    return match key.code {
                        KeyCode::Up => self.reorder_selected(-1),
                        KeyCode::Down => self.reorder_selected(1),
                        _ => None,
                    };
                }
                let page = self.page_rows();
                match key.code {
                    KeyCode::Up => {
                        let target = self.selected_index().saturating_sub(1);
                        self.select(target, true);
                    }
                    KeyCode::Down => {
                        let target = self.selected_index().saturating_add(1);
                        self.select(target, true);
                    }
                    KeyCode::PageUp => {
                        let target = self.selected_index().saturating_sub(page);
                        self.select(target, true);
                    }
                    KeyCode::PageDown => {
                        let target = self.selected_index().saturating_add(page);
                        self.select(target, true);
                    }
                    KeyCode::Home => {
                        self.select(0, true);
                    }
                    KeyCode::End => {
                        let last = self.rows.len().saturating_sub(1);
                        self.select(last, true);
                    }
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

    #[derive(Debug, PartialEq)]
    enum Msg {
        Select(usize),
        Open(usize),
        Reorder(usize, usize),
    }

    fn view(rows: usize) -> PlaylistView<Msg> {
        let mut view = PlaylistView::new("test")
            .row_height(40.0)
            .on_select(Msg::Select)
            .on_activate(Msg::Open)
            .on_reorder(Msg::Reorder);
        view.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 400.0));
        view.set_rows((0..rows).map(|i| Row::new(format!("track {i}"))).collect());
        view
    }

    fn key(code: KeyCode, alt: bool) -> WidgetInput {
        WidgetInput::Key(crate::event::KeyEvent {
            code,
            vk: 0,
            ctrl: false,
            shift: false,
            alt,
            repeat: false,
        })
    }

    #[test]
    fn visible_range_covers_the_viewport() {
        let view = view(1000);
        let (start, end) = view.visible_range();
        assert_eq!(start, 0);
        assert!(end >= 11 && end <= 12);
    }

    #[test]
    fn click_selects_and_double_click_opens() {
        let mut view = view(5);
        let pos = view.row_rect(2).unwrap().center();
        view.input(WidgetInput::PressAt(pos));
        assert_eq!(view.input(WidgetInput::ReleaseAt(pos)), Some(Msg::Select(2)));
        assert_eq!(view.selected_index(), 2);
        assert_eq!(view.input(WidgetInput::DoubleClickAt(pos)), Some(Msg::Open(2)));
        assert_eq!(view.input(WidgetInput::Activate), Some(Msg::Open(2)));
    }

    #[test]
    fn arrow_keys_move_the_cursor_and_reveal_it() {
        let mut view = view(100);
        view.input(key(KeyCode::Down, false));
        view.input(key(KeyCode::Down, false));
        assert_eq!(view.selected_index(), 2);
        view.input(key(KeyCode::End, false));
        assert_eq!(view.selected_index(), 99);
        assert!(view.scroll_offset() > 0.0);
        view.input(key(KeyCode::Home, false));
        assert_eq!(view.selected_index(), 0);
        assert_eq!(view.scroll_offset(), 0.0);
        view.input(key(KeyCode::PageDown, false));
        assert_eq!(view.selected_index(), view.page_rows());
    }

    #[test]
    fn keyboard_reorder_reports_from_and_to() {
        let mut view = view(5);
        view.input(key(KeyCode::Down, false));
        assert_eq!(
            view.input(key(KeyCode::Down, true)),
            Some(Msg::Reorder(1, 2))
        );
        assert_eq!(view.selected_index(), 2);
        assert_eq!(view.input(key(KeyCode::Up, true)), Some(Msg::Reorder(2, 1)));
        // The ends do not wrap.
        view.input(key(KeyCode::Home, false));
        assert_eq!(view.input(key(KeyCode::Up, true)), None);
    }

    #[test]
    fn drag_down_moves_a_track_past_its_neighbour() {
        let mut view = view(5);
        let start = view.row_rect(0).unwrap().center();
        view.input(WidgetInput::PressAt(start));
        view.input(WidgetInput::DragStart(start));
        assert_eq!(view.drag_state(), Some((0, 0)));
        // In the lower half of row 2 the insertion slot is 3 (between rows 2
        // and 3), which becomes destination index 2 after the removal.
        let target = Point::new(view.rect.center().x, view.row_rect(2).unwrap().top + 38.0);
        view.input(WidgetInput::DragTo(target));
        assert_eq!(view.drag_state(), Some((0, 3)));
        assert_eq!(
            view.input(WidgetInput::ReleaseAt(target)),
            Some(Msg::Reorder(0, 2))
        );
        assert_eq!(view.drag_state(), None);
        assert_eq!(view.selected_index(), 2);
    }

    #[test]
    fn drag_up_moves_a_track_before_itself() {
        let mut view = view(5);
        let start = view.row_rect(3).unwrap().center();
        view.input(WidgetInput::PressAt(start));
        view.input(WidgetInput::DragStart(start));
        let target = Point::new(view.rect.center().x, view.row_rect(1).unwrap().top);
        view.input(WidgetInput::DragTo(target));
        assert_eq!(
            view.input(WidgetInput::ReleaseAt(target)),
            Some(Msg::Reorder(3, 1))
        );
    }

    #[test]
    fn dropping_in_place_emits_nothing() {
        let mut view = view(5);
        let start = view.row_rect(2).unwrap().center();
        view.input(WidgetInput::PressAt(start));
        view.input(WidgetInput::DragStart(start));
        assert_eq!(view.input(WidgetInput::ReleaseAt(start)), None);
        assert_eq!(view.drag_state(), None);
        assert_eq!(view.selected_index(), 0);
    }

    #[test]
    fn dragging_near_the_bottom_edge_scrolls_the_list() {
        let mut view = view(100);
        let start = view.row_rect(0).unwrap().center();
        view.input(WidgetInput::PressAt(start));
        view.input(WidgetInput::DragStart(start));
        let bottom = Point::new(view.rect.center().x, view.rect.bottom - 4.0);
        view.input(WidgetInput::DragTo(bottom));
        assert!(view.scroll_offset() > 0.0);
        assert_eq!(view.drag_state().map(|(from, _)| from), Some(0));
    }

    #[test]
    fn a_drag_release_does_not_select_the_landing_row() {
        let mut view = view(5);
        let start = view.row_rect(0).unwrap().center();
        view.input(WidgetInput::PressAt(start));
        view.input(WidgetInput::DragStart(start));
        let target = view.row_rect(3).unwrap().center();
        view.input(WidgetInput::DragTo(target));
        // The container sends ReleaseAt, then DragEnd; neither is a click.
        assert_eq!(
            view.input(WidgetInput::ReleaseAt(target)),
            Some(Msg::Reorder(0, 3))
        );
        assert_eq!(view.input(WidgetInput::DragEnd(target)), None);
        assert_eq!(view.selected_index(), 3);
    }

    #[test]
    fn wheel_scrolls_and_clamps() {
        let mut view = view(1000);
        view.input(WidgetInput::Wheel(-10.0));
        assert!(view.scroll_offset() > 0.0);
        view.input(WidgetInput::Wheel(-10_000.0));
        assert!((view.scroll_offset() - view.max_scroll()).abs() < 0.01);
    }

    #[test]
    fn a11y_publishes_visible_tracks_and_the_selected_one() {
        let mut view = view(100);
        view.set_selected(4);
        let info = view.a11y().expect("the playlist is exposed");
        assert_eq!(info.role, A11yRole::List);
        let selected: Vec<usize> = info
            .children
            .iter()
            .filter(|child| child.selected)
            .map(|child| child.index)
            .collect();
        assert_eq!(selected, vec![4]);
        let axis = info.scroll.expect("scroll state").vertical.expect("vertical");
        assert_eq!(axis.viewport, 400.0);
        assert_eq!(axis.content, 4000.0);
        assert!(info.children.iter().any(|child| child.offscreen));
        assert_eq!(view.a11y_select_child(9), Some(Msg::Select(9)));
        assert_eq!(view.selected_index(), 9);
    }

    #[test]
    fn bar_hover_and_press_repaint_only_the_bar() {
        let mut view = view(100);
        let track = view.scrollbar_rect();
        let thumb = view.thumb_rect().expect("overflow");
        let over = Point::new(track.center().x, thumb.center().y);
        view.input(WidgetInput::HoverAt(over));
        assert!(view.bar.hovered);
        assert!(view.take_repaint_hint().is_some());
        view.input(WidgetInput::PressAt(over));
        assert!(view.bar.active);
        assert_eq!(view.take_repaint_hint(), Some(track));
        let before = view.scroll_offset();
        view.input(WidgetInput::DragTo(Point::new(over.x, over.y + 40.0)));
        assert!(view.scroll_offset() > before);
        view.input(WidgetInput::ReleaseAt(Point::new(over.x, over.y + 40.0)));
        assert!(!view.bar.active);
    }

    #[test]
    fn set_rows_clamps_the_cursor_and_clears_a_drag() {
        let mut view = view(10);
        view.set_selected(9);
        let start = view.row_rect(9).unwrap().center();
        view.input(WidgetInput::PressAt(start));
        view.input(WidgetInput::DragStart(start));
        view.set_rows(vec![Row::new("only")]);
        assert_eq!(view.selected_index(), 0);
        assert_eq!(view.drag_state(), None);
        assert!(!view.has_scrollbar());
    }

    #[test]
    fn disabled_tracks_cannot_be_selected_played_or_dragged() {
        let mut view = view(3);
        view.set_rows(vec![
            Row::new("a"),
            Row::new("b").disabled(true),
            Row::new("c"),
        ]);
        let pos = view.row_rect(1).unwrap().center();
        view.input(WidgetInput::PressAt(pos));
        assert_eq!(view.input(WidgetInput::ReleaseAt(pos)), None);
        assert_eq!(view.selected_index(), 0);
        assert_eq!(view.input(WidgetInput::DoubleClickAt(pos)), None);
        view.input(WidgetInput::PressAt(pos));
        view.input(WidgetInput::DragStart(pos));
        assert_eq!(view.drag_state(), None);
    }
}
