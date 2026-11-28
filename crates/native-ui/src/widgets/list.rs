// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Virtualized list: only visible rows are painted, and the widget owns its
//! scroll offset, wheel handling, row hover/press and an integrated scrollbar.

use crate::a11y::{A11yChild, A11yInfo, A11yRole, A11yScroll};
use crate::backend::TextAlign;
use crate::event::Cursor;
use crate::geometry::{Point, Rect};
use crate::skin::{SkinAttrDecl, SkinElement, SkinValue, SkinValueKind};
use crate::widget::{Row, TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

use super::observers::{Observers, observer_builders, observer_methods};
use super::scrollbar::{self, BarHit, ScrollBarState, ScrollBarStyle};

static LIST_SKIN_CHILDREN: &[&str] = &["item"];
static LIST_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("row-height", SkinValueKind::Number),
    SkinAttrDecl::new("alternating", SkinValueKind::Bool),
    SkinAttrDecl::new("empty-text", SkinValueKind::Text),
];

pub struct ListView<M> {
    id: WidgetId,
    rect: Rect,
    rows: Vec<Row>,
    row_height: f32,
    scroll: f32,
    hover_row: Option<usize>,
    press_row: Option<usize>,
    bar: ScrollBarState,
    style: ScrollBarStyle,
    /// Narrow repaint for the input that just ran (bar chrome only).
    repaint_hint: Option<Rect>,
    selected: usize,
    /// Centered message painted while there are no rows.
    empty_text: String,
    /// Tint every other row with the panel color.
    alternating: bool,
    on_activate: Option<Box<dyn Fn(usize) -> M>>,
    observers: Observers<M>,
}

impl<M> ListView<M> {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 320.0, 360.0),
            rows: Vec::new(),
            row_height: 44.0,
            scroll: 0.0,
            hover_row: None,
            press_row: None,
            bar: ScrollBarState::default(),
            style: ScrollBarStyle::default(),
            repaint_hint: None,
            selected: 0,
            empty_text: String::new(),
            alternating: false,
            on_activate: None,
            observers: Observers::new(),
        }
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(24.0);
        self
    }

    /// Message painted in the middle of an empty list ("暂无曲目").
    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.empty_text = text.into();
        self
    }

    /// Fill alternating rows with the panel color for easier scanning.
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

    /// Message painted while there are no rows.
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

    pub fn on_activate(mut self, handler: impl Fn(usize) -> M + 'static) -> Self {
        self.on_activate = Some(Box::new(handler));
        self
    }

    /// Scrollbar thickness, insets, minimum thumb and hover/active colors.
    pub fn scrollbar_style(mut self, style: ScrollBarStyle) -> Self {
        self.style = style;
        self
    }

    pub fn scrollbar(&self) -> &ScrollBarStyle {
        &self.style
    }

    /// Row cursor position (the typed access seam).
    pub fn selected_row(&self) -> usize {
        self.selected
    }

    /// The widget's data model (typed access; the `<item>` child list writes
    /// it through `Widget::set_rows`).
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// `(first, end)` published viewport rows, end exclusive.
    pub fn visible_rows(&self) -> (usize, usize) {
        self.visible_range()
    }

    /// Current scroll offset in DIPs.
    pub fn scroll_offset(&self) -> f32 {
        self.scroll
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

    /// Rectangle of the keyboard-selected row, clipped to the viewport. Used
    /// as the tooltip anchor so a tip on a long list points at the focused
    /// row, not at the bottom of the container. `None` when the selection is
    /// scrolled out entirely.
    fn selected_row_rect(&self) -> Option<Rect> {
        if self.rows.is_empty() {
            return None;
        }
        let index = self.selected.min(self.rows.len() - 1);
        let top = self.rect.top + index as f32 * self.row_height - self.scroll;
        let row = Rect::new(
            self.rect.left,
            top,
            self.list_rect().right,
            top + self.row_height,
        );
        row.intersect(self.list_rect())
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

    fn visible_range(&self) -> (usize, usize) {
        if self.rows.is_empty() {
            return (0, 0);
        }
        let start = (self.scroll / self.row_height).floor().max(0.0) as usize;
        let rows = (self.rect.height() / self.row_height).ceil() as usize + 1;
        let end = (start + rows).min(self.rows.len());
        (start.min(self.rows.len()), end)
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    fn scroll_to_selected(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        let selected = self.selected.min(self.rows.len() - 1);
        let top = selected as f32 * self.row_height;
        let bottom = top + self.row_height;
        if top < self.scroll {
            self.scroll = top;
        } else if bottom > self.scroll + self.rect.height() {
            self.scroll = bottom - self.rect.height();
        }
        self.clamp_scroll();
    }

    /// Rows that fit in the viewport; PageUp/PageDown step by this many.
    fn page_rows(&self) -> usize {
        ((self.rect.height() / self.row_height).floor() as usize).max(1)
    }

    /// Move the keyboard row cursor and keep it scrolled into view.
    fn select(&mut self, index: usize) {
        if self.rows.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected = index.min(self.rows.len() - 1);
        self.scroll_to_selected();
    }

    fn activate(&self) -> Option<M> {
        if self.rows.is_empty() {
            return None;
        }
        let index = self.selected.min(self.rows.len() - 1);
        self.on_activate.as_ref().map(|handler| handler(index))
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
}

impl<M: 'static> Widget<M> for ListView<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        LIST_SKIN_ATTRS
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
        LIST_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children
            .iter()
            .all(|child| child.name == "item" && Row::valid_skin_row(child))
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

    observer_methods!();

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        self.clamp_scroll();
    }

    fn set_rows(&mut self, rows: Vec<Row>) {
        self.rows = rows;
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
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

    /// Only a real hover change repaints: a different row, the bar tint, or a
    /// thumb drag (the content moves under it). Moving along one row or along
    /// an already-hovered bar repaints nothing.
    fn repaint_on_move_at(&self, pos: Point) -> bool {
        if self.bar.active {
            return true;
        }
        let over_bar = self.has_scrollbar()
            && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some();
        if over_bar {
            return !self.bar.hovered;
        }
        self.bar.hovered || self.row_at(pos) != self.hover_row
    }

    /// Bar-hover/press chrome changes repaint the scrollbar strip only.
    fn take_repaint_hint(&mut self) -> Option<Rect> {
        self.repaint_hint.take()
    }

    /// A list scrolls its rows with the wheel, so a scroll view around it leaves
    /// the notch to the list (D65).
    fn scrolls_with_wheel(&self) -> bool {
        true
    }

    /// Enter enters row navigation; arrows/PageUp/PageDown/Home/End move the
    /// row cursor, Enter again activates it, Esc/Backspace leave.
    fn adjustable(&self) -> bool {
        true
    }

    /// Ring around the rows (not the scrollbar) so the row fill sits exactly
    /// inside the focus frame.
    fn focus_ring_rect(&self) -> Rect {
        self.list_rect()
    }

    /// A scrollbar track shows the arrow (dragging a thumb is not a click).
    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        (self.has_scrollbar() && self.scrollbar_rect().contains_point(pos))
            .then_some(Cursor::Arrow)
    }

    /// Publish only the rows currently in the viewport.
    fn a11y(&self) -> Option<A11yInfo> {
        let (start, end) = self.visible_range();
        let list = self.list_rect();
        let children = (start..end)
            .map(|index| {
                let top = self.rect.top + index as f32 * self.row_height - self.scroll;
                let rect = Rect::new(self.rect.left, top, list.right, top + self.row_height);
                // `visible_range` publishes one row past the viewport so the
                // next PageDown row is already in the tree. That row (and the
                // tail of a partially scrolled one) must say it is offscreen
                // and report bounds clipped to the viewport, or a client would
                // read a rectangle that overlaps the pager below the list.
                let offscreen = !rect.intersects(list);
                let rect = rect.intersect(list).unwrap_or(rect);
                A11yChild::new(index, self.rows[index].title.clone(), rect)
                    .selected(index == self.selected)
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
        self.select(index);
        None
    }

    fn a11y_scroll(&mut self, _horizontal: Option<f32>, vertical: Option<f32>) -> bool {
        let Some(target) = vertical else {
            return false;
        };
        let before = self.scroll;
        self.scroll = target.clamp(0.0, self.max_scroll());
        (self.scroll - before).abs() > 0.01
    }

    fn tooltip_rect(&self) -> Rect {
        self.selected_row_rect().unwrap_or(self.rect)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.row_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        (!self.rows.is_empty()).then_some(self.selected.min(self.rows.len() - 1))
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let pal = ctx.palette;
        if self.rows.is_empty() {
            if !self.empty_text.is_empty() {
                ctx.text(
                    self.rect,
                    &self.empty_text,
                    TextStyleRole::Small,
                    TextAlign::Center,
                    pal.muted,
                );
            }
            return;
        }
        let (start, end) = self.visible_range();
        ctx.canvas.push_clip(self.rect);
        for index in start..end {
            let top = self.rect.top + index as f32 * self.row_height - self.scroll;
            let row_rect = Rect::new(
                self.rect.left,
                top,
                self.list_rect().right,
                top + self.row_height,
            );
            let row = &self.rows[index];
            if self.alternating && index % 2 == 1 {
                ctx.canvas.fill_rect(row_rect, pal.panel);
            }
            if row.current {
                ctx.canvas.fill_rect(row_rect, pal.row_selected);
                ctx.canvas.fill_rect(
                    Rect::new(row_rect.left, row_rect.top, row_rect.left + 3.0, row_rect.bottom),
                    pal.accent,
                );
            } else if state.pressed && self.press_row == Some(index) {
                ctx.canvas.fill_rect(row_rect, pal.row_pressed);
            } else if state.hovered && self.hover_row == Some(index) {
                ctx.canvas.fill_rect(row_rect, pal.row_hover);
            } else if state.focused && index == self.selected {
                // Keyboard row cursor (arrow/PageUp/Home navigation).
                ctx.canvas.fill_rect(row_rect, pal.row_hover);
            }
            let title_color = if row.disabled {
                pal.text_disabled
            } else {
                pal.text
            };
            if row.subtitle.is_empty() {
                ctx.text(
                    Rect::new(
                        row_rect.left + 12.0,
                        row_rect.top,
                        row_rect.right - 12.0,
                        row_rect.bottom,
                    ),
                    &row.title,
                    TextStyleRole::Body,
                    TextAlign::Left,
                    title_color,
                );
            } else {
                ctx.text(
                    Rect::new(
                        row_rect.left + 12.0,
                        row_rect.top + 3.0,
                        row_rect.right - 12.0,
                        row_rect.top + 24.0,
                    ),
                    &row.title,
                    TextStyleRole::Body,
                    TextAlign::Left,
                    title_color,
                );
                ctx.text(
                    Rect::new(
                        row_rect.left + 12.0,
                        row_rect.top + 24.0,
                        row_rect.right - 12.0,
                        row_rect.bottom - 2.0,
                    ),
                    &row.subtitle,
                    TextStyleRole::Small,
                    TextAlign::Left,
                    pal.muted,
                );
            }
            if !row.trailing.is_empty() {
                ctx.text(
                    Rect::new(
                        row_rect.right - 90.0,
                        row_rect.top,
                        row_rect.right - 12.0,
                        row_rect.bottom,
                    ),
                    &row.trailing,
                    TextStyleRole::Small,
                    TextAlign::Right,
                    pal.muted,
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
                let rows_before = self.hover_row;
                if over_bar {
                    self.bar.hovered = true;
                    self.hover_row = None;
                } else {
                    self.bar.hovered = false;
                    self.hover_row = self.row_at(pos);
                }
                if self.bar.hovered != bar_before && self.hover_row == rows_before {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::HoverLeave => {
                let bar_before = self.bar.hovered;
                let rows_before = self.hover_row;
                self.hover_row = None;
                if !self.bar.active {
                    self.bar.hovered = false;
                }
                if self.bar.hovered != bar_before && self.hover_row == rows_before {
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
                self.press_row = self.row_at(pos);
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
                let pressed = self.press_row.take();
                let hit = self.row_at(pos);
                if let (Some(pressed), Some(hit)) = (pressed, hit)
                    && pressed == hit
                    && !self.rows.get(hit).map(|row| row.disabled).unwrap_or(true)
                {
                    return self.on_activate.as_ref().map(|handler| handler(hit));
                }
                None
            }
            WidgetInput::ClickAt(pos) => {
                // Row activation already ran on the release; a click that
                // landed on the bar changes nothing and must not repaint the
                // rows underneath it.
                if self.has_scrollbar()
                    && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some()
                {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
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
                use crate::event::KeyCode;
                let page = self.page_rows();
                match key.code {
                    KeyCode::Up => self.select(self.selected.saturating_sub(1)),
                    KeyCode::Down => self.select(self.selected.saturating_add(1)),
                    KeyCode::PageUp => self.select(self.selected.saturating_sub(page)),
                    KeyCode::PageDown => self.select(self.selected.saturating_add(page)),
                    KeyCode::Home => self.select(0),
                    KeyCode::End => self.select(self.rows.len().saturating_sub(1)),
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

observer_builders!(ListView);

#[cfg(test)]
mod tests {
    use super::*;

    fn view(rows: usize) -> ListView<()> {
        let mut view = ListView::new("test").row_height(40.0);
        view.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 400.0));
        view.set_rows((0..rows).map(|i| Row::new(format!("row {i}"))).collect());
        view
    }

    #[test]
    fn visible_range_covers_the_viewport() {
        let view = view(1000);
        let (start, end) = view.visible_range();
        assert_eq!(start, 0);
        assert!(end >= 11 && end <= 12);
    }

    #[test]
    fn the_item_child_list_replaces_the_rows() {
        let mut view: ListView<()> = ListView::new("list");
        let mut first = SkinElement::new("item", 2, 3);
        first.push_attr("title", "夜曲");
        first.push_attr("subtitle", "周杰伦");
        first.push_attr("trailing", "3:58");
        first.push_attr("current", "true");
        let mut second = SkinElement::new("item", 3, 3);
        second.push_attr("title", "晴天");
        let children = [first, second];
        assert!(<ListView<()> as Widget<()>>::validate_skin_children(&view, &children));
        <ListView<()> as Widget<()>>::apply_skin_children(&mut view, &children);
        assert_eq!(view.rows().len(), 2);
        assert_eq!(view.rows()[0].title, "夜曲");
        assert_eq!(view.rows()[0].subtitle, "周杰伦");
        assert_eq!(view.rows()[0].trailing, "3:58");
        assert!(view.rows()[0].current);

        let mut missing_title = SkinElement::new("item", 4, 3);
        missing_title.push_attr("subtitle", "无标题");
        assert!(!<ListView<()> as Widget<()>>::validate_skin_children(
            &view,
            &[missing_title]
        ));
        let mut unknown_attr = SkinElement::new("item", 5, 3);
        unknown_attr.push_attr("title", "夜曲");
        unknown_attr.push_attr("colour", "red");
        assert!(!<ListView<()> as Widget<()>>::validate_skin_children(
            &view,
            &[unknown_attr]
        ));
    }

    #[test]
    fn wheel_scrolls_and_clamps() {
        let mut view = view(1000);
        view.input(WidgetInput::Wheel(-10.0));
        assert!(view.scroll > 0.0);
        view.input(WidgetInput::Wheel(-10000.0));
        assert!((view.scroll - view.max_scroll()).abs() < 0.01);
    }

    #[test]
    fn row_hit_testing_accounts_for_scroll() {
        let mut view = view(1000);
        view.input(WidgetInput::Wheel(-2.0));
        let row = view.scroll / view.row_height;
        let hit = view.row_at(Point::new(10.0, view.rect.top + 5.0));
        assert_eq!(hit, Some(row as usize));
    }

    #[test]
    fn a11y_scroll_moves_the_viewport_and_publishes_its_range() {
        let mut view = view(1000);
        let info = view.a11y().expect("list publishes an a11y node");
        let axis = info.scroll.expect("scroll state").vertical.expect("vertical");
        assert_eq!(axis.offset, 0.0);
        assert_eq!(axis.viewport, 400.0);
        assert_eq!(axis.content, 40_000.0);

        assert!(view.a11y_scroll(None, Some(1200.0)));
        assert_eq!(view.scroll, 1200.0);
        // A negative target clamps to the top; the same target is a no-op.
        assert!(view.a11y_scroll(None, Some(-40.0)));
        assert_eq!(view.scroll, 0.0);
        assert!(!view.a11y_scroll(None, Some(0.0)));
        // A list has no horizontal axis, so a horizontal request is ignored.
        assert!(!view.a11y_scroll(Some(500.0), None));
        assert_eq!(view.scroll, 0.0);

        // The published axis follows the offset.
        view.a11y_scroll(None, Some(view.max_scroll()));
        let info = view.a11y().unwrap();
        let axis = info.scroll.unwrap().vertical.unwrap();
        assert!(axis.percent().unwrap() > 99.9);
    }

    #[test]
    fn selecting_a_row_scrolls_it_into_view() {
        let mut view = view(1000);
        view.set_selected(500);
        assert!(view.scroll > 0.0);
        let top = 500.0 * view.row_height;
        assert!(view.scroll <= top);
        assert!(view.scroll + view.rect.height() >= top + view.row_height);
    }

    fn key(code: crate::event::KeyCode) -> WidgetInput {
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
    fn arrow_and_page_keys_move_the_row_cursor() {
        use crate::event::KeyCode;
        let mut view = view(100);
        view.input(key(KeyCode::Down));
        view.input(key(KeyCode::Down));
        assert_eq!(view.selected, 2);
        view.input(key(KeyCode::End));
        assert_eq!(view.selected, 99);
        view.input(key(KeyCode::Home));
        assert_eq!(view.selected, 0);
        view.input(key(KeyCode::PageDown));
        assert_eq!(view.selected, view.page_rows());
        view.input(key(KeyCode::PageUp));
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn scrolled_out_rows_are_marked_offscreen_and_clipped() {
        let view = view(1000);
        let info = view.a11y().expect("list is exposed");
        // 400px / 40px is 10 rows plus the look-ahead row.
        assert_eq!(info.children.len(), 11);
        let viewport = view.list_rect();
        let last = info.children.last().unwrap();
        assert!(last.offscreen, "look-ahead row must report offscreen");
        assert!(last.rect.intersect(viewport).is_none());
        for child in &info.children[..10] {
            assert!(!child.offscreen, "row {} is in the viewport", child.index);
            assert!(child.rect.top >= viewport.top);
            assert!(child.rect.bottom <= viewport.bottom);
        }
    }

    #[test]
    fn tooltip_anchor_is_the_selected_row() {
        let mut view = view(1000);
        view.set_selected(5);
        let anchor = view.tooltip_rect();
        let top = view.rect.top + 5.0 * view.row_height - view.scroll;
        assert!((anchor.top - top).abs() < 0.01);
        assert!((anchor.height() - view.row_height).abs() < 0.01);
        assert!(anchor.bottom <= view.rect.bottom + 0.01);
    }

    #[test]
    fn enter_and_activate_report_the_selected_row() {
        use crate::event::KeyCode;
        let mut view: ListView<usize> = ListView::new("t").on_activate(|index| index);
        view.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 400.0));
        view.set_rows((0..3).map(|i| Row::new(format!("r{i}"))).collect());
        view.input(key(KeyCode::Down));
        assert_eq!(view.input(WidgetInput::Activate), Some(1));
        assert_eq!(view.input(key(KeyCode::Enter)), Some(1));
    }

    #[test]
    fn scrollbar_thumb_hover_drag_and_track_click() {
        let mut view = view(1000);
        let track = view.scrollbar_rect();
        let thumb = view.thumb_rect().expect("rows overflow the viewport");
        let over_thumb = Point::new(track.center().x, thumb.center().y);
        assert_eq!(view.cursor_at(over_thumb), Some(Cursor::Arrow));
        assert_eq!(view.cursor_at(Point::new(10.0, 10.0)), None);
        view.input(WidgetInput::HoverAt(over_thumb));
        assert!(view.bar.hovered);
        assert!(view.hover_row.is_none(), "the bar must not highlight rows");

        view.input(WidgetInput::PressAt(over_thumb));
        assert!(view.bar.active);
        let before = view.scroll;
        view.input(WidgetInput::DragTo(Point::new(
            over_thumb.x,
            over_thumb.y + 40.0,
        )));
        assert!(view.scroll > before, "dragging the thumb scrolls down");
        view.input(WidgetInput::ReleaseAt(Point::new(
            over_thumb.x,
            over_thumb.y + 40.0,
        )));
        assert!(!view.bar.active);

        let below = Point::new(track.center().x, track.bottom - 4.0);
        let before = view.scroll;
        view.input(WidgetInput::PressAt(below));
        assert!(view.scroll > before, "a track click jumps toward the click");
        view.input(WidgetInput::ReleaseAt(below));
    }

    #[test]
    fn empty_text_and_alternating_rows_are_stored() {
        let view: ListView<()> = ListView::new("t")
            .empty_text("暂无曲目")
            .alternating(true);
        assert_eq!(view.empty_text, "暂无曲目");
        assert!(view.alternating);
        // An empty list publishes no rows and reports no scroll range.
        assert_eq!(view.visible_range(), (0, 0));
        assert!(view.a11y().unwrap().children.is_empty());
        // The defaults stay untouched.
        let plain: ListView<()> = ListView::new("t");
        assert!(plain.empty_text.is_empty());
        assert!(!plain.alternating);
    }
}
