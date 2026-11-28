// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Drop-down list (combo box).
//!
//! The open list is painted by the widget itself, above its siblings: the
//! container's overlay pass paints it last and hit-tests it first, and
//! `paint_bounds` makes the invalidation cover the list too. The list is
//! capped at `max_visible` rows and scrolls with the wheel or by following the
//! keyboard highlight, so a long list never grows past the placement the
//! application chose.

use std::cell::Cell;

use crate::a11y::{A11yInfo, A11yRole};
use crate::backend::{TextAlign, TextStyle};
use crate::event::{Cursor, KeyCode};
use crate::geometry::{Point, Rect};
use crate::skin::SkinElement;
use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

use super::observers::{Observers, observer_builders, observer_methods};
use super::scrollbar::{self, BarHit, ScrollBarState, ScrollBarStyle};

const DEFAULT_ROW: f32 = 26.0;
const GAP: f32 = 2.0;

static COMBO_SKIN_CHILDREN: &[&str] = &["option"];

pub struct ComboBox<M> {
    id: WidgetId,
    rect: Rect,
    items: Vec<String>,
    selected: usize,
    open: bool,
    highlighted: usize,
    scroll: usize,
    max_visible: usize,
    row_height: f32,
    drop_up: bool,
    enabled: bool,
    label: String,
    /// Closed-state affordance, drawn with the UI face for a text triangle and
    /// with Segoe MDL2 for a private-use glyph.
    arrow: char,
    arrow_open: char,
    /// Effective direction from the last paint (auto-flip near the window
    /// edge). Hit-testing and invalidation use it too.
    flip_up: Cell<bool>,
    bar: ScrollBarState,
    bar_style: ScrollBarStyle,
    /// Narrow repaint for the input that just ran (bar chrome only).
    repaint_hint: Option<Rect>,
    on_select: Box<dyn Fn(usize) -> M>,
    hover: Option<usize>,
    observers: Observers<M>,
}

impl<M> ComboBox<M> {
    pub fn new(
        id: &'static str,
        items: &[&str],
        selected: usize,
        on_select: impl Fn(usize) -> M + 'static,
    ) -> Self {
        let selected = selected.min(items.len().saturating_sub(1));
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 200.0, 32.0),
            items: items.iter().map(|item| item.to_string()).collect(),
            selected,
            open: false,
            highlighted: selected,
            scroll: 0,
            max_visible: 8,
            row_height: DEFAULT_ROW,
            drop_up: false,
            enabled: true,
            label: String::new(),
            arrow: '\u{25BE}',
            arrow_open: '\u{25B4}',
            flip_up: Cell::new(false),
            bar: ScrollBarState::default(),
            bar_style: ScrollBarStyle {
                thickness: 6.0,
                inset: 1.0,
                thumb_inset: 1.0,
                min_thumb: 12.0,
                page_on_track_click: false,
                colors: None,
            },
            repaint_hint: None,
            on_select: Box::new(on_select),
            hover: None,
            observers: Observers::new(),
        }
    }

    /// Scrollbar thickness/insets/minimum thumb and hover/active colors for
    /// the open drop-down.
    pub fn scrollbar_style(mut self, style: ScrollBarStyle) -> Self {
        self.bar_style = style;
        self
    }

    pub fn scrollbar(&self) -> &ScrollBarStyle {
        &self.bar_style
    }

    fn bar_track(&self) -> Rect {
        scrollbar::track_rect(self.dropdown_rect(), true, &self.bar_style)
    }

    fn bar_thumb(&self) -> Option<Rect> {
        if !self.overflow() {
            return None;
        }
        scrollbar::thumb_rect(
            self.bar_track(),
            self.items.len() as f32,
            self.visible_rows() as f32,
            self.scroll as f32,
            true,
            &self.bar_style,
        )
    }

    fn set_scroll_offset(&mut self, offset: f32) {
        let row = offset.round().max(0.0) as usize;
        self.scroll = row.min(self.items.len().saturating_sub(self.visible_rows()));
    }

    /// Closed-state drop-down affordance. The default is a small solid
    /// triangle; a private-use character (U+E000..U+F8FF, e.g. an MDL2
    /// chevron) is drawn with the glyph face, any other character with the UI
    /// face. Both characters are set here; the open state defaults to the
    /// matching upward triangle.
    pub fn arrow(mut self, closed: char, open: char) -> Self {
        self.arrow = closed;
        self.arrow_open = open;
        self
    }

    /// Just the closed-state character (the open state keeps its default).
    pub fn arrow_closed(mut self, closed: char) -> Self {
        self.arrow = closed;
        self
    }

    pub fn arrow_chars(&self) -> (char, char) {
        (self.arrow, self.arrow_open)
    }

    /// Accessible name for the list (the closed box only paints the value).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Rows visible at once before the list scrolls (default 8).
    pub fn max_visible(mut self, rows: usize) -> Self {
        self.max_visible = rows.max(1);
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(16.0);
        self
    }

    /// Open the list above the box instead of below it (near the window edge).
    pub fn drop_up(mut self, up: bool) -> Self {
        self.drop_up = up;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The option list (typed access; the `<option>` child list writes it
    /// through `set_items`).
    pub fn items(&self) -> &[String] {
        &self.items
    }

    pub fn selected_text(&self) -> Option<&str> {
        self.items.get(self.selected).map(String::as_str)
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Set the selection without emitting (typed access / app sync).
    pub fn set_selected(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected = index;
            self.highlighted = index;
            self.scroll_into_view();
        }
    }

    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        if self.selected >= self.items.len() {
            self.selected = self.items.len().saturating_sub(1);
        }
        self.highlighted = self.selected;
        self.scroll_into_view();
    }

    fn visible_rows(&self) -> usize {
        self.items.len().min(self.max_visible).max(1)
    }

    fn dropdown_height(&self) -> f32 {
        self.visible_rows() as f32 * self.row_height + 2.0
    }

    fn overflow(&self) -> bool {
        self.items.len() > self.visible_rows()
    }

    fn dropdown_rect(&self) -> Rect {
        let height = self.dropdown_height();
        let up = self.drop_up || self.flip_up.get();
        if up {
            Rect::new(
                self.rect.left,
                self.rect.top - GAP - height,
                self.rect.right,
                self.rect.top - GAP,
            )
        } else {
            Rect::new(
                self.rect.left,
                self.rect.bottom + GAP,
                self.rect.right,
                self.rect.bottom + GAP + height,
            )
        }
    }

    fn row_rect(&self, index: usize) -> Rect {
        let base = self.dropdown_rect();
        let top = base.top + 1.0 + index.saturating_sub(self.scroll) as f32 * self.row_height;
        Rect::new(
            base.left + 1.0,
            top,
            base.right - 1.0,
            top + self.row_height,
        )
    }

    fn visible_row_at(&self, pos: Point) -> Option<usize> {
        let list = self.dropdown_rect();
        if !self.open || !list.contains_point(pos) {
            return None;
        }
        if self.overflow() && self.bar_track().contains_point(pos) {
            return None;
        }
        let top = list.top + 1.0;
        let row = ((pos.y - top) / self.row_height).floor();
        if row < 0.0 {
            return None;
        }
        let index = self.scroll + row as usize;
        (index < self.items.len() && index < self.scroll + self.visible_rows())
            .then_some(index)
    }

    /// Pick the direction for this paint: the application's `drop_up`, or an
    /// automatic flip when the list would leave the viewport and there is room
    /// above.
    fn resolve_direction(&self, viewport: Rect) {
        let height = self.dropdown_height();
        let below = self.rect.bottom + GAP + height;
        let above = self.rect.top - GAP - height;
        let fits_below = viewport.is_empty() || below <= viewport.bottom;
        let can_above = viewport.is_empty() || above >= viewport.top;
        self.flip_up
            .set(self.drop_up || (!fits_below && can_above));
    }

    fn arrow_style(&self, ctx: &WidgetPaint<'_>, size: f32) -> TextStyle {
        let ch = if self.open { self.arrow_open } else { self.arrow };
        let code = ch as u32;
        if (0xE000..=0xF8FF).contains(&code) {
            TextStyle::glyph(size)
        } else {
            let base = ctx.painter.small().clone();
            ctx.style(&base)
        }
    }

    fn scroll_into_view(&mut self) {
        let visible = self.visible_rows();
        if self.highlighted < self.scroll {
            self.scroll = self.highlighted;
        } else if self.highlighted >= self.scroll + visible {
            self.scroll = self.highlighted + 1 - visible;
        }
        let max_scroll = self.items.len().saturating_sub(visible);
        self.scroll = self.scroll.min(max_scroll);
    }

    fn open_list(&mut self) {
        self.open = true;
        self.highlighted = self.selected;
        self.scroll_into_view();
    }

    fn close_list(&mut self) {
        self.open = false;
        self.hover = None;
        self.bar.reset();
    }

    fn select(&mut self, index: usize) -> Option<M> {
        if index >= self.items.len() {
            return None;
        }
        self.selected = index;
        self.highlighted = index;
        self.close_list();
        Some((self.on_select)(index))
    }

    fn step(&mut self, delta: i32) {
        if self.items.is_empty() {
            return;
        }
        let count = self.items.len() as i32;
        self.highlighted = (self.highlighted as i32 + delta).rem_euclid(count) as usize;
        self.scroll_into_view();
    }
}

impl<M: 'static> Widget<M> for ComboBox<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
    }

    fn skin_children(&self) -> &'static [&'static str] {
        COMBO_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(|child| {
            child.name == "option"
                && child
                    .attr("label")
                    .is_some_and(|label| !label.trim().is_empty())
                && child.attrs_are(&["label"])
        })
    }

    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        let items = children
            .iter()
            .filter_map(|child| child.attr("label").map(str::to_string))
            .collect();
        self.set_items(items);
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
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        32.0
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.close_list();
        }
    }

    fn paint_bounds(&self) -> Rect {
        if self.open {
            self.rect.union(self.dropdown_rect())
        } else {
            self.rect
        }
    }

    fn overlay(&self) -> bool {
        self.open
    }

    fn hit_test(&self, pos: Point) -> bool {
        self.rect.contains_point(pos)
            || (self.open && self.dropdown_rect().contains_point(pos))
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Hand)
    }

    /// The open drop-down's scrollbar is a drag target, not a hand target.
    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        if self.open && self.bar_track().contains_point(pos) {
            Some(Cursor::Arrow)
        } else {
            Some(Cursor::Hand)
        }
    }

    fn repaint_on_move(&self) -> bool {
        self.open
    }

    /// Only a real hover change in the open list repaints.
    fn repaint_on_move_at(&self, pos: Point) -> bool {
        if !self.open {
            return false;
        }
        if self.bar.active {
            return true;
        }
        if self.overflow()
            && scrollbar::hit(self.bar_track(), self.bar_thumb(), pos).is_some()
        {
            return !self.bar.hovered;
        }
        self.bar.hovered || self.visible_row_at(pos) != self.hover
    }

    /// Bar-hover/press chrome changes repaint the scrollbar strip only.
    fn take_repaint_hint(&mut self) -> Option<Rect> {
        self.repaint_hint.take()
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        self.resolve_direction(ctx.viewport);
        let palette = ctx.palette;
        let border = if !self.enabled {
            palette.button_disabled
        } else if state.focused || self.open {
            palette.accent
        } else {
            palette.explicit_border
        };
        ctx.canvas.frame(self.rect, border, palette.button_bg);
        let text = Rect::new(
            self.rect.left + 9.0,
            self.rect.top,
            self.rect.right - 24.0,
            self.rect.bottom,
        );
        let color = if self.enabled {
            palette.text
        } else {
            palette.text_disabled
        };
        ctx.text(
            text,
            self.selected_text().unwrap_or(""),
            TextStyleRole::Body,
            TextAlign::Left,
            color,
        );
        let chevron = Rect::new(
            self.rect.right - 22.0,
            self.rect.top,
            self.rect.right - 6.0,
            self.rect.bottom,
        );
        let arrow_char = if self.open { self.arrow_open } else { self.arrow };
        let mut buf = [0u8; 4];
        let arrow = arrow_char.encode_utf8(&mut buf);
        let arrow_style = self.arrow_style(ctx, 12.0);
        let arrow_color = if self.enabled {
            palette.muted
        } else {
            palette.text_disabled
        };
        ctx.canvas.text(
            chevron,
            arrow,
            &arrow_style,
            TextAlign::Center,
            arrow_color,
        );
        if !self.open {
            return;
        }
        let list = self.dropdown_rect();
        ctx.canvas.frame(list, palette.explicit_border, palette.panel);
        let overflow = self.overflow();
        let first = self.scroll;
        let last = (self.scroll + self.visible_rows()).min(self.items.len());
        for index in first..last {
            let row = self.row_rect(index);
            if index == self.selected {
                ctx.canvas.fill_rect(row, palette.row_selected);
            } else if Some(index) == self.hover || index == self.highlighted {
                ctx.canvas.fill_rect(row, palette.row_hover);
            }
            let label = Rect::new(
                row.left + 9.0,
                row.top,
                row.right - if overflow { 14.0 } else { 6.0 },
                row.bottom,
            );
            let color = if index == self.selected {
                palette.accent_text
            } else {
                palette.text
            };
            ctx.text(
                label,
                &self.items[index],
                TextStyleRole::Body,
                TextAlign::Left,
                color,
            );
        }
        if overflow {
            // The wheel, the keyboard and a draggable themed bar all scroll
            // the open list.
            scrollbar::paint(
                ctx,
                self.bar_track(),
                self.bar_thumb(),
                &self.bar,
                &self.bar_style,
            );
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::HoverAt(pos) => {
                let bar_before = self.bar.hovered;
                let rows_before = self.hover;
                if self.open && self.overflow() {
                    let over_bar =
                        scrollbar::hit(self.bar_track(), self.bar_thumb(), pos).is_some();
                    self.bar.hovered = over_bar;
                    self.hover = if over_bar {
                        None
                    } else {
                        self.visible_row_at(pos)
                    };
                } else {
                    self.bar.hovered = false;
                    self.hover = self.visible_row_at(pos);
                }
                if self.bar.hovered != bar_before && self.hover == rows_before {
                    self.repaint_hint = Some(self.bar_track());
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
                    self.repaint_hint = Some(self.bar_track());
                }
                None
            }
            WidgetInput::PressAt(pos) => {
                if self.open && self.overflow() {
                    let track = self.bar_track();
                    let thumb = self.bar_thumb();
                    match scrollbar::hit(track, thumb, pos) {
                        Some(BarHit::Thumb) => {
                            if let Some(thumb) = thumb {
                                self.bar.active = true;
                                self.bar.hovered = true;
                                self.bar.grab = pos.y - thumb.top;
                                self.repaint_hint = Some(self.bar_track());
                            }
                        }
                        Some(BarHit::Track) => {
                            if let Some(thumb) = thumb {
                                let offset = scrollbar::track_click_offset(
                                    pos,
                                    track,
                                    thumb,
                                    self.items.len() as f32,
                                    self.visible_rows() as f32,
                                    self.scroll as f32,
                                    true,
                                    &self.bar_style,
                                );
                                self.set_scroll_offset(offset);
                                self.bar.active = true;
                                self.bar.hovered = true;
                                self.bar.grab = thumb.height() * 0.5;
                            }
                        }
                        None => {}
                    }
                }
                None
            }
            WidgetInput::DragTo(pos) => {
                if self.bar.active
                    && let Some(thumb) = self.bar_thumb()
                {
                    let offset = scrollbar::drag_offset(
                        pos,
                        self.bar_track(),
                        thumb,
                        self.bar.grab,
                        self.items.len() as f32,
                        self.visible_rows() as f32,
                        true,
                    );
                    self.set_scroll_offset(offset);
                }
                None
            }
            WidgetInput::ReleaseAt(_) => {
                if self.bar.active {
                    self.bar.active = false;
                    if self.open && self.overflow() {
                        self.repaint_hint = Some(self.bar_track());
                    }
                }
                None
            }
            WidgetInput::ClickAt(pos) => {
                if self.open {
                    if self.overflow() && self.bar_track().contains_point(pos) {
                        self.repaint_hint = Some(self.bar_track());
                        return None;
                    }
                    if let Some(index) = self.visible_row_at(pos) {
                        return self.select(index);
                    }
                    if self.rect.contains_point(pos) {
                        self.close_list();
                    }
                    None
                } else {
                    self.open_list();
                    None
                }
            }
            WidgetInput::Activate => {
                if self.open {
                    self.select(self.highlighted)
                } else {
                    self.open_list();
                    None
                }
            }
            WidgetInput::Wheel(delta) => {
                if self.open {
                    let visible = self.visible_rows();
                    let max_scroll = self.items.len().saturating_sub(visible);
                    let step = if delta > 0.0 { -1 } else { 1 };
                    let next = (self.scroll as i32 + step).clamp(0, max_scroll as i32) as usize;
                    self.scroll = next;
                }
                None
            }
            WidgetInput::Key(key) => match key.code {
                KeyCode::Down => {
                    if !self.open {
                        self.open_list();
                    } else {
                        self.step(1);
                    }
                    None
                }
                KeyCode::Up => {
                    if !self.open {
                        self.open_list();
                    } else {
                        self.step(-1);
                    }
                    None
                }
                KeyCode::Home if self.open => {
                    self.highlighted = 0;
                    self.scroll_into_view();
                    None
                }
                KeyCode::End if self.open => {
                    self.highlighted = self.items.len().saturating_sub(1);
                    self.scroll_into_view();
                    None
                }
                KeyCode::PageUp if self.open => {
                    let step = self.visible_rows().saturating_sub(1).max(1) as i32;
                    self.highlighted = (self.highlighted as i32 - step).max(0) as usize;
                    self.scroll_into_view();
                    None
                }
                KeyCode::PageDown if self.open => {
                    let step = self.visible_rows().saturating_sub(1).max(1) as i32;
                    self.highlighted =
                        (self.highlighted as i32 + step).min(self.items.len() as i32 - 1) as usize;
                    self.scroll_into_view();
                    None
                }
                KeyCode::Enter | KeyCode::Space if self.open => self.select(self.highlighted),
                _ => None,
            },
            _ => None,
        }
    }

    fn dismiss(&mut self) -> bool {
        if self.open {
            self.close_list();
            true
        } else {
            false
        }
    }

    fn owns_navigation_keys(&self) -> bool {
        self.open
    }

    /// Only an open drop-down scrolls with the wheel; a closed box leaves the
    /// notch to a scroll view around it (D65).
    fn scrolls_with_wheel(&self) -> bool {
        self.open
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let value = self.selected_text().unwrap_or("");
        let name = if self.label.is_empty() {
            value.to_string()
        } else {
            self.label.clone()
        };
        Some(
            A11yInfo::new(A11yRole::ComboBox, name)
                .value(value)
                .read_only(!self.enabled),
        )
    }

    fn a11y_set_value(&mut self, text: &str) -> Option<M> {
        let index = self.items.iter().position(|item| item == text)?;
        if index == self.selected {
            return None;
        }
        self.set_selected(index);
        Some((self.on_select)(index))
    }

    fn set_selected(&mut self, index: usize) {
        ComboBox::set_selected(self, index);
    }
}

observer_builders!(ComboBox);

#[cfg(test)]
mod tests {
    use super::*;

    fn place<M: 'static>(combo: &mut ComboBox<M>, rect: Rect) {
        combo.set_rect(rect);
    }

    #[test]
    fn click_opens_and_a_row_click_selects_and_closes() {
        let mut combo: ComboBox<usize> =
            ComboBox::new("c", &["一", "二", "三"], 0, |index| index);
        place(&mut combo, Rect::from_xywh(0.0, 0.0, 200.0, 32.0));
        assert!(!combo.is_open());
        assert_eq!(combo.input(WidgetInput::ClickAt(Point::new(100.0, 16.0))), None);
        assert!(combo.is_open());
        assert!(combo.overlay());
        assert!(combo.paint_bounds().bottom > combo.rect().bottom);
        // Second row sits one row below the list's top edge.
        let row = Point::new(100.0, 32.0 + 2.0 + 1.0 + 26.0 + 5.0);
        assert_eq!(combo.input(WidgetInput::ClickAt(row)), Some(1));
        assert!(!combo.is_open());
        assert_eq!(combo.selected(), 1);
    }

    #[test]
    fn the_open_dropdown_bar_shows_the_arrow_cursor() {
        let mut combo: ComboBox<usize> =
            ComboBox::new("c", &["一", "二", "三", "四", "五"], 0, |index| index).max_visible(2);
        place(&mut combo, Rect::from_xywh(0.0, 0.0, 200.0, 32.0));
        assert_eq!(combo.cursor_at(Point::new(100.0, 16.0)), Some(Cursor::Hand));
        combo.input(WidgetInput::ClickAt(Point::new(100.0, 16.0)));
        assert!(combo.overflow());
        let bar = combo.bar_track().center();
        assert_eq!(combo.cursor_at(bar), Some(Cursor::Arrow));
    }

    #[test]
    fn dismiss_closes_without_selecting() {
        let mut combo: ComboBox<usize> =
            ComboBox::new("c", &["一", "二"], 1, |index| index);
        place(&mut combo, Rect::from_xywh(0.0, 0.0, 200.0, 32.0));
        combo.input(WidgetInput::ClickAt(Point::new(10.0, 10.0)));
        assert!(combo.dismiss());
        assert!(!combo.is_open());
        assert_eq!(combo.selected(), 1);
        assert!(!combo.dismiss());
    }

    #[test]
    fn only_the_open_list_owns_the_navigation_keys() {
        let mut combo: ComboBox<usize> =
            ComboBox::new("c", &["一", "二"], 0, |index| index);
        place(&mut combo, Rect::from_xywh(0.0, 0.0, 200.0, 32.0));
        assert!(!combo.owns_navigation_keys());
        combo.input(WidgetInput::ClickAt(Point::new(10.0, 10.0)));
        assert!(combo.owns_navigation_keys());
        combo.dismiss();
        assert!(!combo.owns_navigation_keys());
    }

    #[test]
    fn long_lists_scroll_to_keep_the_selection_visible() {
        let items: Vec<String> = (0..12).map(|index| format!("项目 {index}")).collect();
        let refs: Vec<&str> = items.iter().map(String::as_str).collect();
        let mut combo: ComboBox<usize> =
            ComboBox::new("c", &refs, 10, |index| index).max_visible(4);
        place(&mut combo, Rect::from_xywh(0.0, 0.0, 200.0, 32.0));
        combo.input(WidgetInput::ClickAt(Point::new(10.0, 10.0)));
        // Opening brings row 10 into a 4-row window: scroll = 7, so row 10 is
        // the fourth visible row.
        let bounds = combo.paint_bounds();
        assert!(bounds.height() > 32.0);
        let row = 32.0 + 2.0 + 1.0 + 3.0 * 26.0 + 5.0;
        assert_eq!(
            combo.input(WidgetInput::ClickAt(Point::new(10.0, row))),
            Some(10)
        );
    }

    #[test]
    fn an_open_list_scrollbar_hovers_and_drags() {
        let items: Vec<String> = (0..12).map(|index| format!("项目 {index}")).collect();
        let refs: Vec<&str> = items.iter().map(String::as_str).collect();
        let mut combo: ComboBox<usize> =
            ComboBox::new("c", &refs, 0, |index| index).max_visible(4);
        place(&mut combo, Rect::from_xywh(0.0, 0.0, 200.0, 32.0));
        combo.open_list();
        let track = combo.bar_track();
        let thumb = combo.bar_thumb().expect("12 items overflow 4 rows");
        let over = Point::new(track.center().x, thumb.center().y);

        combo.input(WidgetInput::HoverAt(over));
        assert!(combo.bar.hovered);
        assert_eq!(combo.hover, None, "the bar does not hover a row");

        // A click on the bar is neither a row selection nor a close.
        assert_eq!(combo.input(WidgetInput::ClickAt(over)), None);
        assert!(combo.is_open());
        assert_eq!(combo.selected(), 0);

        combo.input(WidgetInput::PressAt(over));
        assert!(combo.bar.active);
        combo.input(WidgetInput::DragTo(Point::new(over.x, over.y + 30.0)));
        assert!(combo.scroll > 0, "dragging the thumb scrolls the list");
        combo.input(WidgetInput::ReleaseAt(Point::new(over.x, over.y + 30.0)));
        assert!(!combo.bar.active);
    }

    #[test]
    fn keyboard_highlight_and_activate_select() {
        let mut combo: ComboBox<usize> =
            ComboBox::new("c", &["a", "b", "c"], 0, |index| index);
        place(&mut combo, Rect::from_xywh(0.0, 0.0, 200.0, 32.0));
        let down = WidgetInput::Key(crate::event::KeyEvent {
            code: KeyCode::Down,
            vk: 0x28,
            ctrl: false,
            shift: false,
            alt: true,
            repeat: false,
        });
        assert_eq!(combo.input(down), None);
        assert!(combo.is_open());
        assert_eq!(combo.input(down), None);
        assert_eq!(combo.input(WidgetInput::Activate), Some(1));
        assert_eq!(combo.selected(), 1);
        assert!(!combo.is_open());
    }

    #[test]
    fn a11y_publishes_the_value_and_selects_by_text() {
        let mut combo: ComboBox<usize> =
            ComboBox::new("c", &["顺序", "随机"], 0, |index| index).label("循环方式");
        let info = combo.a11y().unwrap();
        assert_eq!(info.role, A11yRole::ComboBox);
        assert_eq!(info.name, "循环方式");
        assert_eq!(info.value.as_deref(), Some("顺序"));
        assert_eq!(combo.a11y_set_value("随机"), Some(1));
        assert_eq!(combo.selected(), 1);
        assert_eq!(combo.a11y_set_value("随机"), None);
        assert_eq!(combo.a11y_set_value("不存在"), None);
    }

    #[test]
    fn arrow_defaults_to_triangles_and_can_be_customized() {
        let combo: ComboBox<usize> = ComboBox::new("c", &["a"], 0, |index| index);
        assert_eq!(combo.arrow_chars(), ('\u{25BE}', '\u{25B4}'));
        let custom = combo.arrow('\u{E70D}', '\u{E70E}');
        assert_eq!(custom.arrow_chars(), ('\u{E70D}', '\u{E70E}'));
    }

    #[test]
    fn a_list_that_does_not_fit_below_flips_up() {
        struct Stub;
        impl crate::backend::Canvas for Stub {
            fn scale(&self) -> f32 { 1.0 }
            fn dpi(&self) -> f32 { 96.0 }
            fn fill_rect(&mut self, _rect: Rect, _color: crate::Color) {}
            fn text(&mut self, _rect: Rect, _text: &str, _style: &crate::backend::TextStyle, _align: crate::backend::TextAlign, _color: crate::Color) {}
            fn measure_text(&mut self, text: &str, _style: &crate::backend::TextStyle) -> crate::geometry::Size {
                crate::geometry::Size::new(text.chars().count() as f32 * 8.0, 16.0)
            }
            fn ellipse(&mut self, _rect: Rect, _color: crate::Color) {}
            fn vertical_gradient(&mut self, _rect: Rect, _top: crate::Color, _bottom: crate::Color) {}
            fn image(&mut self, _rect: Rect, _crop: bool, _id: crate::backend::ImageId, _data: crate::backend::ImageData<'_>) {}
            fn push_clip(&mut self, _rect: Rect) {}
            fn pop_clip(&mut self) {}
        }
        let mut combo: ComboBox<usize> =
            ComboBox::new("c", &["a", "b", "c", "d"], 0, |index| index).max_visible(4);
        place(&mut combo, Rect::from_xywh(0.0, 250.0, 200.0, 32.0));
        combo.open_list();
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = Stub;
        let mut paint = WidgetPaint {
            painter: &painter,
            canvas: &mut canvas,
            palette: *painter.palette(),
            font: None,
            images: &images,
            viewport: Rect::from_xywh(0.0, 0.0, 200.0, 300.0),
        dirty: &[],
        };
        combo.paint(&mut paint, &WidgetState { enabled: true, ..Default::default() });
        assert!(combo.flip_up.get(), "the list must open above the box");
        assert!(combo.paint_bounds().top < combo.rect().top);
    }
}
