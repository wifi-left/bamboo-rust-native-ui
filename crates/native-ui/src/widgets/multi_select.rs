// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Multi-selection list: checkable rows, independent of each other.
//!
//! It is a virtualized row painter like `ListView` (only the viewport rows are
//! painted and published) but selection is a per-row checked flag rather than
//! one current row. UIA sees `MultiSelectList` (Selection + `CanSelectMultiple`
//! true, no required selection) with `ListItem` children.

use crate::a11y::{A11yChild, A11yInfo, A11yRole, A11yScroll};
use crate::backend::TextAlign;
use crate::event::{Cursor, KeyCode, PointerModifiers};
use crate::geometry::{Point, Rect};
use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

use super::inputs::paint_check;
use super::observers::{Observers, observer_builders, observer_methods};
use super::scrollbar::{self, BarHit, ScrollBarState, ScrollBarStyle};

pub struct MultiSelectList<M> {
    id: WidgetId,
    rect: Rect,
    rows: Vec<String>,
    checked: Vec<bool>,
    cursor: usize,
    scroll: f32,
    row_height: f32,
    enabled: bool,
    check: char,
    /// Corner radius of the per-row check box in DIPs (`0` = square).
    corner: f32,
    bar: ScrollBarState,
    bar_style: ScrollBarStyle,
    /// Narrow repaint for the input that just ran (bar chrome only).
    repaint_hint: Option<Rect>,
    on_change: Box<dyn Fn(Vec<usize>) -> M>,
    hover: Option<usize>,
    /// Row a Shift+click extends from: the last row toggled/selected without
    /// Shift. `None` until the user clicks a row.
    anchor: Option<usize>,
    observers: Observers<M>,
}

impl<M> MultiSelectList<M> {
    pub fn new(
        id: &'static str,
        rows: &[&str],
        on_change: impl Fn(Vec<usize>) -> M + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 220.0, 160.0),
            rows: rows.iter().map(|row| row.to_string()).collect(),
            checked: vec![false; rows.len()],
            cursor: 0,
            scroll: 0.0,
            row_height: 28.0,
            enabled: true,
            check: '\u{E73E}',
            corner: 4.0,
            bar: ScrollBarState::default(),
            bar_style: ScrollBarStyle::default(),
            repaint_hint: None,
            on_change: Box::new(on_change),
            hover: None,
            anchor: None,
            observers: Observers::new(),
        }
    }

    /// Scrollbar thickness, insets, minimum thumb and hover/active colors.
    pub fn scrollbar_style(mut self, style: ScrollBarStyle) -> Self {
        self.bar_style = style;
        self
    }

    pub fn scrollbar(&self) -> &ScrollBarStyle {
        &self.bar_style
    }

    /// Custom checked-state mark (same contract as `Checkbox::check`).
    pub fn check(mut self, glyph: char) -> Self {
        self.check = glyph;
        self
    }

    /// Paint the row check boxes with sharp corners (radius `0`).
    pub fn square(mut self) -> Self {
        self.corner = 0.0;
        self
    }

    /// Corner radius of the row check boxes in DIPs; `0` is square and the
    /// value is clamped to half the box when painting.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner = radius.max(0.0);
        self
    }

    /// Runtime setter for the configured radius (not yet clamped).
    pub fn set_corner_radius(&mut self, radius: f32) {
        self.corner = radius.max(0.0);
    }

    /// The configured check-box corner radius in DIPs (`0` = square).
    pub fn corner_radius_value(&self) -> f32 {
        self.corner
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(18.0);
        self
    }

    /// Mark rows checked at creation (no message).
    pub fn checked(mut self, indexes: &[usize]) -> Self {
        for &index in indexes {
            if index < self.checked.len() {
                self.checked[index] = true;
            }
        }
        self
    }

    pub fn selected(&self) -> Vec<usize> {
        self.checked
            .iter()
            .enumerate()
            .filter_map(|(index, checked)| checked.then_some(index))
            .collect()
    }

    pub fn is_checked(&self, index: usize) -> bool {
        self.checked.get(index).copied().unwrap_or(false)
    }

    pub fn set_checked(&mut self, index: usize, checked: bool) {
        if index < self.checked.len() {
            self.checked[index] = checked;
        }
    }

    pub fn set_selection(&mut self, indexes: &[usize]) {
        self.checked.iter_mut().for_each(|checked| *checked = false);
        for &index in indexes {
            self.set_checked(index, true);
        }
    }

    pub fn set_rows(&mut self, rows: Vec<String>) {
        self.checked.resize(rows.len(), false);
        self.rows = rows;
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
        self.anchor = self
            .anchor
            .map(|anchor| anchor.min(self.rows.len().saturating_sub(1)));
        self.clamp_scroll();
        self.scroll_into_view();
    }

    /// Rows intersecting the viewport; the first and last may be partial (the
    /// offset is fractional, so the wheel scrolls smoothly instead of jumping
    /// a whole row).
    fn row_range(&self) -> (usize, usize) {
        if self.rows.is_empty() {
            return (0, 0);
        }
        let first = (self.scroll / self.row_height).floor().max(0.0) as usize;
        let last = ((self.scroll + self.rect.height()) / self.row_height)
            .ceil()
            .max(0.0) as usize;
        let first = first.min(self.rows.len());
        (first, last.clamp(first, self.rows.len()))
    }

    fn content_height(&self) -> f32 {
        self.rows.len() as f32 * self.row_height
    }

    fn max_scroll(&self) -> f32 {
        (self.content_height() - self.rect.height()).max(0.0)
    }

    fn has_scrollbar(&self) -> bool {
        self.max_scroll() > 0.5 && self.rect.height() > 0.0
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    fn scrollbar_rect(&self) -> Rect {
        scrollbar::track_rect(self.rect, true, &self.bar_style)
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
            &self.bar_style,
        )
    }

    fn bar_at(&self, pos: Point) -> Option<BarHit> {
        if !self.has_scrollbar() {
            return None;
        }
        scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos)
    }

    /// Set the scroll offset in DIPs (fractional: a wheel notch or a thumb
    /// drag moves the rows smoothly, not a whole row at a time).
    fn set_scroll_offset(&mut self, offset: f32) {
        self.scroll = offset;
        self.clamp_scroll();
    }

    fn row_rect(&self, index: usize) -> Rect {
        let top = self.rect.top + index as f32 * self.row_height - self.scroll;
        Rect::new(
            self.rect.left,
            top,
            self.rect.right,
            top + self.row_height,
        )
    }

    fn index_at(&self, pos: Point) -> Option<usize> {
        if !self.rect.contains_point(pos) {
            return None;
        }
        if self.has_scrollbar() && self.scrollbar_rect().contains_point(pos) {
            return None;
        }
        let y = pos.y - self.rect.top + self.scroll;
        if y < 0.0 {
            return None;
        }
        let index = (y / self.row_height).floor() as usize;
        (index < self.rows.len()).then_some(index)
    }

    fn scroll_into_view(&mut self) {
        let top = self.cursor as f32 * self.row_height;
        let bottom = top + self.row_height;
        if top < self.scroll {
            self.scroll = top;
        } else if bottom > self.scroll + self.rect.height() {
            self.scroll = bottom - self.rect.height();
        }
        self.clamp_scroll();
    }

    fn toggle(&mut self, index: usize) -> Option<M> {
        if index >= self.checked.len() {
            return None;
        }
        self.checked[index] = !self.checked[index];
        self.anchor = Some(index);
        self.cursor = index;
        self.scroll_into_view();
        Some((self.on_change)(self.selected()))
    }

    /// Ctrl/Shift click. Ctrl toggles one row and moves the anchor; Shift
    /// selects the inclusive range from the anchor and keeps it. Like a native
    /// checked list, a plain click toggles. See D66.
    fn select_with(&mut self, index: usize, modifiers: PointerModifiers) -> Option<M> {
        if index >= self.checked.len() {
            return None;
        }
        let Some(anchor) = self.anchor.filter(|_| modifiers.shift) else {
            return self.toggle(index);
        };
        let (first, last) = if anchor <= index {
            (anchor, index)
        } else {
            (index, anchor)
        };
        let mut changed = false;
        for (row, checked) in self.checked.iter_mut().enumerate() {
            let want = row >= first && row <= last;
            if *checked != want {
                *checked = want;
                changed = true;
            }
        }
        self.cursor = index;
        self.scroll_into_view();
        changed.then(|| (self.on_change)(self.selected()))
    }
}

impl<M: 'static> Widget<M> for MultiSelectList<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
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
        160.0
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover = None;
        }
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    /// Only a real hover change repaints (a different row, the bar tint, or a
    /// thumb drag, which moves the rows).
    fn repaint_on_move_at(&self, pos: Point) -> bool {
        if self.bar.active {
            return true;
        }
        if self.bar_at(pos).is_some() {
            return !self.bar.hovered;
        }
        self.bar.hovered || self.index_at(pos) != self.hover
    }

    /// Bar-hover/press chrome changes repaint the scrollbar strip only.
    fn take_repaint_hint(&mut self) -> Option<Rect> {
        self.repaint_hint.take()
    }

    /// The list scrolls its rows with the wheel (D65).
    fn scrolls_with_wheel(&self) -> bool {
        true
    }

    /// Ctrl/Shift clicks arrive as `SelectWith` (Ctrl toggles, Shift extends
    /// the range from the anchor). See D66.
    fn multi_selectable(&self) -> bool {
        true
    }

    fn adjustable(&self) -> bool {
        true
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        let (first, last) = self.row_range();
        ctx.canvas.push_clip(self.rect);
        for index in first..last {
            let row = self.row_rect(index);
            let checked = self.checked[index];
            if checked {
                ctx.canvas.fill_rect(row, palette.row_selected);
            } else if Some(index) == self.hover
                || (state.focused && index == self.cursor)
            {
                ctx.canvas.fill_rect(row, palette.row_hover);
            }
            let size = 16.0;
            let box_rect = Rect::new(
                row.left + 6.0,
                row.center().y - size / 2.0,
                row.left + 6.0 + size,
                row.center().y + size / 2.0,
            );
            paint_check(
                ctx,
                box_rect,
                checked,
                &WidgetState {
                    hovered: Some(index) == self.hover,
                    enabled: self.enabled,
                    ..Default::default()
                },
                self.check,
                self.corner,
            );
            let label = Rect::new(
                box_rect.right + 8.0,
                row.top,
                if self.has_scrollbar() {
                    self.scrollbar_rect().left - 4.0
                } else {
                    row.right - 6.0
                },
                row.bottom,
            );
            let color = if self.enabled {
                palette.text
            } else {
                palette.text_disabled
            };
            ctx.text(
                label,
                &self.rows[index],
                TextStyleRole::Body,
                TextAlign::Left,
                color,
            );
        }
        ctx.canvas.pop_clip();
        if self.has_scrollbar() {
            scrollbar::paint(
                ctx,
                self.scrollbar_rect(),
                self.thumb_rect(),
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
                let over_bar = self.bar_at(pos).is_some();
                let bar_before = self.bar.hovered;
                let rows_before = self.hover;
                if over_bar {
                    self.bar.hovered = true;
                    self.hover = None;
                } else {
                    self.bar.hovered = false;
                    self.hover = self.index_at(pos);
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
                let track = self.scrollbar_rect();
                let thumb = self.thumb_rect();
                match self.bar_at(pos) {
                    Some(BarHit::Thumb) => {
                        if let Some(thumb) = thumb {
                            self.bar.active = true;
                            self.bar.hovered = true;
                            self.bar.grab = pos.y - thumb.top;
                            self.repaint_hint = Some(self.scrollbar_rect());
                        }
                    }
                    Some(BarHit::Track) => {
                        if let Some(thumb) = thumb {
                            let offset = scrollbar::track_click_offset(
                                pos,
                                track,
                                thumb,
                                self.content_height(),
                                self.rect.height(),
                                self.scroll,
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
                None
            }
            WidgetInput::DragTo(pos) => {
                if self.bar.active
                    && let Some(thumb) = self.thumb_rect()
                {
                    let offset = scrollbar::drag_offset(
                        pos,
                        self.scrollbar_rect(),
                        thumb,
                        self.bar.grab,
                        self.content_height(),
                        self.rect.height(),
                        true,
                    );
                    self.set_scroll_offset(offset);
                }
                None
            }
            WidgetInput::ReleaseAt(_) => {
                if self.bar.active {
                    self.bar.active = false;
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::ClickAt(pos) => {
                if self.bar_at(pos).is_some() {
                    self.repaint_hint = Some(self.scrollbar_rect());
                    return None;
                }
                let index = self.index_at(pos)?;
                self.toggle(index)
            }
            WidgetInput::SelectWith(pos, modifiers) => {
                if self.bar_at(pos).is_some() {
                    self.repaint_hint = Some(self.scrollbar_rect());
                    return None;
                }
                let index = self.index_at(pos)?;
                self.select_with(index, modifiers)
            }
            WidgetInput::Activate => self.toggle(self.cursor),
            WidgetInput::Wheel(delta) => {
                // The system's lines per notch, in DIPs; the offset stays
                // fractional so the rows and the thumb move together.
                let step = self.row_height * scrollbar::wheel_lines();
                self.scroll -= delta * step;
                self.clamp_scroll();
                None
            }
            WidgetInput::Key(key) => match key.code {
                KeyCode::Up => {
                    self.cursor = self.cursor.saturating_sub(1);
                    self.scroll_into_view();
                    None
                }
                KeyCode::Down => {
                    self.cursor = (self.cursor + 1).min(self.rows.len().saturating_sub(1));
                    self.scroll_into_view();
                    None
                }
                KeyCode::Home => {
                    self.cursor = 0;
                    self.scroll_into_view();
                    None
                }
                KeyCode::End => {
                    self.cursor = self.rows.len().saturating_sub(1);
                    self.scroll_into_view();
                    None
                }
                KeyCode::Space | KeyCode::Enter => self.toggle(self.cursor),
                _ => None,
            },
            _ => None,
        }
    }

    /// A scrollbar track shows the arrow (dragging a thumb is not a click).
    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        (self.has_scrollbar() && self.scrollbar_rect().contains_point(pos))
            .then_some(Cursor::Arrow)
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let (first, last) = self.row_range();
        let children = (first..last)
            .map(|index| {
                A11yChild::new(index, self.rows[index].clone(), self.row_rect(index))
                    .selected(self.checked[index])
            })
            .collect();
        Some(
            A11yInfo::new(A11yRole::MultiSelectList, self.id.0.to_string())
                .children(children)
                .scroll(A11yScroll::new().vertical(
                    self.scroll,
                    self.rect.height(),
                    self.content_height(),
                )),
        )
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        self.toggle(index)
    }

    fn a11y_scroll(&mut self, _horizontal: Option<f32>, vertical: Option<f32>) -> bool {
        let Some(target) = vertical else {
            return false;
        };
        let before = self.scroll;
        self.scroll = target.clamp(0.0, self.max_scroll());
        (self.scroll - before).abs() > 0.01
    }

    fn set_selected(&mut self, index: usize) {
        self.cursor = index.min(self.rows.len().saturating_sub(1));
        self.scroll_into_view();
    }

    fn tooltip_rect(&self) -> Rect {
        self.row_rect(self.cursor)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.index_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        Some(self.cursor)
    }
}

observer_builders!(MultiSelectList);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Canvas, ImageData, ImageId, TextStyle};
    use crate::color::Color;

    #[derive(Default)]
    struct BoxRecorder {
        ellipses: Vec<Rect>,
    }

    impl Canvas for BoxRecorder {
        fn scale(&self) -> f32 {
            1.0
        }
        fn dpi(&self) -> f32 {
            96.0
        }
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
        fn text(
            &mut self,
            _rect: Rect,
            _text: &str,
            _style: &TextStyle,
            _align: TextAlign,
            _color: Color,
        ) {
        }
        fn measure_text(&mut self, text: &str, _style: &TextStyle) -> crate::geometry::Size {
            crate::geometry::Size::new(text.chars().count() as f32 * 8.0, 16.0)
        }
        fn ellipse(&mut self, rect: Rect, _color: Color) {
            self.ellipses.push(rect);
        }
        fn vertical_gradient(&mut self, _rect: Rect, _top: Color, _bottom: Color) {}
        fn image(&mut self, _rect: Rect, _crop: bool, _id: ImageId, _data: ImageData<'_>) {}
        fn push_clip(&mut self, _rect: Rect) {}
        fn pop_clip(&mut self) {}
    }

    fn record(list: &MultiSelectList<()>) -> BoxRecorder {
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = BoxRecorder::default();
        {
            let mut paint = WidgetPaint {
                painter: &painter,
                canvas: &mut canvas,
                palette: *painter.palette(),
                font: None,
                images: &images,
                viewport: Rect::from_xywh(0.0, 0.0, 220.0, 160.0),
                dirty: &[],
            };
            <MultiSelectList<()> as Widget<()>>::paint(
                list,
                &mut paint,
                &WidgetState {
                    enabled: true,
                    ..Default::default()
                },
            );
        }
        canvas
    }

    #[test]
    fn scrollbar_hover_drag_and_click_guard() {
        let rows: Vec<String> = (0..20).map(|index| format!("行 {index}")).collect();
        let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
        let mut list: MultiSelectList<Vec<usize>> =
            MultiSelectList::new("m", &refs, |rows| rows);
        list.set_rect(Rect::from_xywh(0.0, 0.0, 220.0, 140.0));
        assert!(list.has_scrollbar());
        let track = list.scrollbar_rect();
        let thumb = list.thumb_rect().expect("20 rows overflow 5 rows");
        let over = Point::new(track.center().x, thumb.center().y);
        assert_eq!(list.cursor_at(over), Some(Cursor::Arrow));
        assert_eq!(list.cursor_at(Point::new(10.0, 10.0)), None);
        list.input(WidgetInput::HoverAt(over));
        assert!(list.bar.hovered);
        assert!(list.hover.is_none(), "the bar must not hover a row");

        // Clicking the bar is not a row toggle.
        assert_eq!(list.input(WidgetInput::ClickAt(over)), None);
        assert!(list.selected().is_empty());

        list.input(WidgetInput::PressAt(over));
        assert!(list.bar.active);
        list.input(WidgetInput::DragTo(Point::new(over.x, over.y + 70.0)));
        assert!(list.scroll > 0.0, "dragging the thumb scrolls the rows");
        list.input(WidgetInput::ReleaseAt(Point::new(over.x, over.y + 70.0)));
        assert!(!list.bar.active);
    }

    #[test]
    fn row_boxes_are_rounded_by_default_and_square_on_request() {
        let rounded: MultiSelectList<()> = MultiSelectList::new("m", &["a", "b"], |_| ());
        assert_eq!(rounded.corner_radius_value(), 4.0);
        assert!(!record(&rounded).ellipses.is_empty());

        let square = rounded.square();
        assert_eq!(square.corner_radius_value(), 0.0);
        assert!(record(&square).ellipses.is_empty());
    }

    #[test]
    fn clicking_rows_toggles_independently() {
        let mut list: MultiSelectList<Vec<usize>> =
            MultiSelectList::new("m", &["a", "b", "c"], |rows| rows);
        list.set_rect(Rect::from_xywh(0.0, 0.0, 220.0, 160.0));
        assert_eq!(
            list.input(WidgetInput::ClickAt(Point::new(20.0, 10.0))),
            Some(vec![0])
        );
        assert_eq!(
            list.input(WidgetInput::ClickAt(Point::new(20.0, 40.0))),
            Some(vec![0, 1])
        );
        // Toggling the first row off keeps the second.
        assert_eq!(
            list.input(WidgetInput::ClickAt(Point::new(20.0, 10.0))),
            Some(vec![1])
        );
        assert_eq!(list.selected(), vec![1]);
    }

    #[test]
    fn the_wheel_scrolls_in_dips_not_whole_rows() {
        let rows: Vec<String> = (0..20).map(|index| format!("行 {index}")).collect();
        let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
        let mut list: MultiSelectList<usize> =
            MultiSelectList::new("m", &refs, |rows| rows.len());
        list.set_rect(Rect::from_xywh(0.0, 0.0, 220.0, 112.0)); // four 28 DIP rows
        // The notch is the system's line count in DIPs (three rows by
        // default), kept fractional.
        let notch = 28.0 * scrollbar::wheel_lines();
        list.input(WidgetInput::Wheel(-1.0));
        let expected = notch.min(list.content_height() - list.rect.height());
        assert!((list.scroll - expected).abs() < 0.01, "offset {}", list.scroll);
        // A fractional offset shows the boundary rows partially.
        list.set_scroll_offset(14.0);
        assert_eq!(list.row_range(), (0, 5));
        assert_eq!(list.row_rect(1).top, 14.0);
    }

    #[test]
    fn the_thumb_follows_the_offset_and_the_wheel_steps_one_row_for_a_short_list() {
        // The gallery shape: eight rows in a four-row viewport.
        let list_rows = [
            "无损音质",
            "本地曲库",
            "自动下载封面",
            "按专辑分组",
            "按文件夹分组",
            "显示文件扩展名",
            "记住播放位置",
            "自动匹配歌词",
        ];
        let mut list: MultiSelectList<usize> = MultiSelectList::new("m", &list_rows, |rows| rows.len());
        list.set_rect(Rect::from_xywh(0.0, 0.0, 220.0, 112.0));
        let track = list.scrollbar_rect();
        let thumb_at = |list: &MultiSelectList<usize>| list.thumb_rect().expect("thumb");
        // At the top the thumb starts at the track's top; at the end it ends at
        // the track's bottom; halfway it is halfway.
        let top = thumb_at(&list);
        assert!((top.top - track.top).abs() < 0.01);
        let max = list.content_height() - list.rect.height();
        list.set_scroll_offset(max);
        let bottom = thumb_at(&list);
        assert!((bottom.bottom - track.bottom).abs() < 0.01);
        list.set_scroll_offset(max / 2.0);
        let middle = thumb_at(&list);
        let start = |thumb: Rect| thumb.top - track.top;
        assert!(
            ((start(middle) - (start(top) + start(bottom)) / 2.0).abs()) < 0.5,
            "top {} middle {} bottom {}",
            start(top),
            start(middle),
            start(bottom)
        );
        // One notch is the system's line count (three by default), in DIPs.
        list.set_scroll_offset(0.0);
        list.input(WidgetInput::Wheel(-1.0));
        let notch = 28.0 * scrollbar::wheel_lines();
        let expected = notch.min(max);
        assert!((list.scroll - expected).abs() < 0.01, "offset {}", list.scroll);
        let moved = thumb_at(&list);
        assert!(start(moved) > 0.0 && start(moved) <= start(bottom));
    }

    #[test]
    fn shift_click_selects_a_range_and_ctrl_click_toggles() {
        let mut list: MultiSelectList<Vec<usize>> =
            MultiSelectList::new("m", &["a", "b", "c", "d", "e"], |rows| rows);
        list.set_rect(Rect::from_xywh(0.0, 0.0, 220.0, 160.0));
        // A plain click sets the anchor.
        assert_eq!(
            list.input(WidgetInput::ClickAt(Point::new(20.0, 10.0))),
            Some(vec![0])
        );
        // Shift+click on row 3 selects the inclusive range and keeps the
        // anchor: Shift+click on row 2 then replaces the range with 0..=1.
        assert_eq!(
            list.input(WidgetInput::SelectWith(
                Point::new(20.0, 80.0),
                PointerModifiers::shift(true)
            )),
            Some(vec![0, 1, 2])
        );
        assert_eq!(
            list.input(WidgetInput::SelectWith(
                Point::new(20.0, 40.0),
                PointerModifiers::shift(true)
            )),
            Some(vec![0, 1])
        );
        // Ctrl+click toggles one row and moves the anchor there; the next
        // Shift+click extends from it.
        assert_eq!(
            list.input(WidgetInput::SelectWith(
                Point::new(20.0, 110.0),
                PointerModifiers::ctrl(true)
            )),
            Some(vec![0, 1, 3])
        );
        // The Ctrl+click moved the anchor to row 3, so the range is 2..=3.
        assert_eq!(
            list.input(WidgetInput::SelectWith(
                Point::new(20.0, 60.0),
                PointerModifiers::shift(true)
            )),
            Some(vec![2, 3])
        );
        // A modifier release without a previous click is a plain toggle.
        let mut fresh: MultiSelectList<Vec<usize>> =
            MultiSelectList::new("m", &["a", "b"], |rows| rows);
        fresh.set_rect(Rect::from_xywh(0.0, 0.0, 220.0, 160.0));
        assert_eq!(
            fresh.input(WidgetInput::SelectWith(
                Point::new(20.0, 10.0),
                PointerModifiers::shift(true)
            )),
            Some(vec![0])
        );
        assert!(<MultiSelectList<Vec<usize>> as Widget<Vec<usize>>>::multi_selectable(&fresh));
    }

    #[test]
    fn checked_rows_start_selected_and_a11y_reports_them() {
        let mut list: MultiSelectList<Vec<usize>> =
            MultiSelectList::new("m", &["a", "b", "c"], |rows| rows).checked(&[0, 2]);
        list.set_rect(Rect::from_xywh(0.0, 0.0, 220.0, 160.0));
        assert_eq!(list.selected(), vec![0, 2]);
        let info = list.a11y().unwrap();
        assert_eq!(info.role, A11yRole::MultiSelectList);
        assert_eq!(info.children.len(), 3);
        assert!(info.children[0].selected);
        assert!(!info.children[1].selected);
        assert!(info.children[2].selected);
        assert_eq!(list.a11y_select_child(1), Some(vec![0, 1, 2]));
    }

    #[test]
    fn wheel_scrolls_and_keyboard_keeps_the_cursor_visible() {
        let rows: Vec<String> = (0..20).map(|index| format!("行 {index}")).collect();
        let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
        let mut list: MultiSelectList<usize> =
            MultiSelectList::new("m", &refs, |rows| rows.len());
        list.set_rect(Rect::from_xywh(0.0, 0.0, 220.0, 140.0)); // 5 rows
        list.input(WidgetInput::Wheel(-1.0));
        list.input(WidgetInput::Wheel(-1.0));
        // End moves the cursor to the last row and scrolls to show it.
        let key = |code| {
            WidgetInput::Key(crate::event::KeyEvent {
                code,
                vk: 0,
                ctrl: false,
                shift: false,
                alt: false,
                repeat: false,
            })
        };
        list.input(key(KeyCode::End));
        assert_eq!(list.a11y().unwrap().children.last().unwrap().index, 19);
        assert_eq!(list.input(key(KeyCode::Space)), Some(1));
        assert_eq!(list.selected(), vec![19]);
    }
}
