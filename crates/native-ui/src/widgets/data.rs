// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Data-display widgets: a mini bar sparkline, an open-arc gauge and a star
//! rating.
//!
//! The first two are display-only (D27) and opt into the accessibility tree
//! through the instance props; the rating is a real control (Slider role) with
//! click, hover preview and keyboard adjustment.

use crate::a11y::{A11yInfo, A11yRole};
use crate::backend::TextAlign;
use crate::color::Color;
use crate::event::{Cursor, KeyCode};
use crate::geometry::{Point, Rect};
use crate::skin::{SkinAttrDecl, SkinElement, SkinValue, SkinValueKind};
use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

use super::feedback::ring_dots;

static SPARK_SKIN_CHILDREN: &[&str] = &["point"];
static SPARK_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("color", SkinValueKind::Color),
    SkinAttrDecl::new("gap", SkinValueKind::Number),
];
const POINT_SKIN_ATTRS: &[&str] = &["value"];

/// A compact bar strip (sparkline). Bars are scaled from a zero baseline to
/// the largest value; the widget is decoration by default.
pub struct Sparkline {
    id: WidgetId,
    rect: Rect,
    data: Vec<f32>,
    color: Option<Color>,
    gap: f32,
}

impl Sparkline {
    pub fn new(id: &'static str, data: &[f32]) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 160.0, 36.0),
            data: data.to_vec(),
            color: None,
            gap: 2.0,
        }
    }

    /// Bar color (default: the accent).
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Gap between bars in DIPs (default 2).
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    pub fn data(&self) -> &[f32] {
        &self.data
    }

    /// Replace the samples (silent).
    pub fn set_data(&mut self, data: Vec<f32>) {
        self.data = data;
    }

    /// `(min, max)` of the current samples, including the zero baseline.
    pub fn range(&self) -> (f32, f32) {
        let mut max = 0.0f32;
        let mut min = 0.0f32;
        for value in &self.data {
            max = max.max(*value);
            min = min.min(*value);
        }
        (min, max)
    }

    /// Bar color (`None` restores the accent). Returns whether it changed.
    pub fn set_color(&mut self, color: Option<Color>) -> bool {
        if self.color == color {
            return false;
        }
        self.color = color;
        true
    }

    pub fn color_value(&self) -> Option<Color> {
        self.color
    }

    /// Gap between bars in DIPs. Returns whether it changed.
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
}

impl<M> Widget<M> for Sparkline {
    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        SPARK_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("color", SkinValue::Color(_)) => true,
            ("gap", SkinValue::Number(gap)) => *gap >= 0.0,
            _ => false,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("color", value) => {
                self.set_color(value.as_color());
            }
            ("gap", SkinValue::Number(gap)) => {
                self.set_gap(*gap as f32);
            }
            _ => {}
        }
    }

    fn skin_children(&self) -> &'static [&'static str] {
        SPARK_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(|child| {
            child.name == "point"
                && child.attr_number("value").is_some()
                && child.attrs_are(POINT_SKIN_ATTRS)
                && child.children.is_empty()
        })
    }

    /// `<point value>` elements replace the samples; the scale follows from
    /// the new data.
    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        self.set_data(
            children
                .iter()
                .filter_map(|child| child.attr_number("value").map(|value| value as f32))
                .collect(),
        );
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
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        36.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        if self.data.is_empty() || self.rect.height() <= 0.0 || self.rect.width() <= 0.0 {
            return;
        }
        let (min, max) = self.range();
        let span = (max - min).max(1e-6);
        let count = self.data.len() as f32;
        let total_gap = self.gap * (count - 1.0).max(0.0);
        let bar_width = (self.rect.width() - total_gap) / count;
        if bar_width < 0.5 {
            return;
        }
        let color = self.color.unwrap_or(ctx.palette.accent);
        for (index, value) in self.data.iter().enumerate() {
            let fraction = ((*value - min) / span).clamp(0.0, 1.0);
            let height = self.rect.height() * fraction;
            if height <= 0.0 {
                continue;
            }
            let left = self.rect.left + index as f32 * (bar_width + self.gap);
            ctx.canvas.fill_rect(
                Rect::new(
                    left,
                    self.rect.bottom - height,
                    (left + bar_width).min(self.rect.right),
                    self.rect.bottom,
                ),
                color,
            );
        }
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let (min, max) = self.range();
        Some(
            A11yInfo::new(A11yRole::Text, self.id.0.to_string())
                .description(format!("{} 个样本，范围 {min:.1}..{max:.1}", self.data.len()))
                .display_only(),
        )
    }
}

/// An open-arc gauge (270°) drawn as a dot ring with the value arc in the
/// accent color and an optional centered value/label. Display-only by default;
/// exposes a ProgressBar range when the instance opts in.
pub struct Gauge {
    id: WidgetId,
    rect: Rect,
    value: f32,
    thickness: f32,
    show_value: bool,
    label: Option<String>,
}

impl Gauge {
    pub fn new(id: &'static str, value: f32) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 96.0, 96.0),
            value: value.clamp(0.0, 1.0),
            thickness: 4.0,
            show_value: true,
            label: None,
        }
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness.clamp(1.0, 12.0);
        self
    }

    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    /// Text painted in the center instead of the percentage.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn value_now(&self) -> f32 {
        self.value
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
    }
}

impl<M> Widget<M> for Gauge {
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
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        96.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        // Open at the bottom: start at 135°, sweep 270° clockwise.
        let (start, sweep) = (135.0, 270.0);
        ring_dots(
            ctx,
            self.rect,
            start,
            start + sweep,
            self.thickness,
            palette.scroll_track,
        );
        if self.value > 0.0 {
            ring_dots(
                ctx,
                self.rect,
                start,
                start + sweep * self.value,
                self.thickness,
                palette.accent,
            );
        }
        if let Some(label) = &self.label {
            ctx.text(
                self.rect,
                label,
                TextStyleRole::Small,
                TextAlign::Center,
                palette.text,
            );
        } else if self.show_value {
            ctx.text(
                self.rect,
                &format!("{:.0}%", self.value * 100.0),
                TextStyleRole::Body,
                TextAlign::Center,
                palette.text,
            );
        }
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let name = self
            .label
            .clone()
            .unwrap_or_else(|| self.id.0.to_string());
        Some(
            A11yInfo::new(A11yRole::ProgressBar, name)
                .range(0.0, 1.0, self.value as f64)
                .value(format!("{:.0}%", self.value * 100.0))
                .display_only(),
        )
    }
}

/// A star rating. Click a star to set the value, hover for a preview, and use
/// the arrow keys in adjust mode (or the Slider UIA pattern) to change it.
pub struct RatingStars<M> {
    id: WidgetId,
    rect: Rect,
    stars: usize,
    value: u8,
    hover: Option<u8>,
    enabled: bool,
    read_only: bool,
    clearable: bool,
    on_rate: Box<dyn Fn(u8) -> M>,
}

impl<M> RatingStars<M> {
    pub fn new(
        id: &'static str,
        stars: usize,
        value: u8,
        on_rate: impl Fn(u8) -> M + 'static,
    ) -> Self {
        let stars = stars.max(1);
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 130.0, 28.0),
            stars,
            value: value.min(stars as u8),
            hover: None,
            enabled: true,
            read_only: false,
            clearable: false,
            on_rate: Box::new(on_rate),
        }
    }

    /// Clicking the current value clears it to zero.
    pub fn clearable(mut self, clearable: bool) -> Self {
        self.clearable = clearable;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn value(&self) -> u8 {
        self.value
    }

    /// Hovered preview value (`None` when the pointer is away).
    pub fn hover_value(&self) -> Option<u8> {
        self.hover
    }

    /// Set the rating silently (clamped).
    pub fn set_rating(&mut self, value: u8) {
        self.value = value.min(self.stars as u8);
    }

    /// Rate through the normal path and return the message.
    pub fn rate(&mut self, value: u8) -> Option<M> {
        if !self.enabled || self.read_only {
            return None;
        }
        let value = value.min(self.stars as u8);
        if value == self.value {
            return None;
        }
        self.value = value;
        self.hover = None;
        Some((self.on_rate)(value))
    }

    fn cell_width(&self) -> f32 {
        if self.stars == 0 {
            0.0
        } else {
            self.rect.width() / self.stars as f32
        }
    }

    fn star_at(&self, pos: Point) -> Option<u8> {
        if !self.rect.contains_point(pos) {
            return None;
        }
        let cell = self.cell_width();
        if cell <= 0.0 {
            return None;
        }
        let index = ((pos.x - self.rect.left) / cell).floor().max(0.0) as usize;
        Some((index.min(self.stars - 1) + 1) as u8)
    }

    fn star_rect(&self, index: usize) -> Rect {
        let cell = self.cell_width();
        let left = self.rect.left + index as f32 * cell;
        Rect::new(left, self.rect.top, left + cell, self.rect.bottom)
    }

    fn paint_stars(&self, ctx: &mut WidgetPaint<'_>, painted: u8) {
        let palette = ctx.palette;
        for index in 0..self.stars {
            let filled = index < painted as usize;
            let glyph = if filled { '\u{2605}' } else { '\u{2606}' };
            let color = if !self.enabled {
                palette.text_disabled
            } else if filled {
                palette.accent
            } else {
                palette.muted
            };
            ctx.text(
                self.star_rect(index),
                &glyph.to_string(),
                TextStyleRole::Body,
                TextAlign::Center,
                color,
            );
        }
    }
}

impl<M: 'static> Widget<M> for RatingStars<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::VALUE
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
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        28.0
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

    fn adjustable(&self) -> bool {
        true
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Hand)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let painted = if self.enabled {
            self.hover.unwrap_or(self.value)
        } else {
            self.value
        };
        self.paint_stars(ctx, painted);
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled || self.read_only {
            return None;
        }
        match input {
            WidgetInput::HoverAt(pos) => {
                self.hover = self.star_at(pos);
                None
            }
            WidgetInput::HoverLeave => {
                self.hover = None;
                None
            }
            WidgetInput::ClickAt(pos) => {
                let Some(value) = self.star_at(pos) else {
                    return None;
                };
                if self.clearable && value == self.value {
                    self.hover = None;
                    self.value = 0;
                    return Some((self.on_rate)(0));
                }
                self.rate(value)
            }
            WidgetInput::Key(key) => match key.code {
                KeyCode::Left | KeyCode::Down => {
                    let next = self.value.saturating_sub(1);
                    if next == self.value {
                        None
                    } else {
                        self.value = next;
                        Some((self.on_rate)(next))
                    }
                }
                KeyCode::Right | KeyCode::Up => {
                    let next = (self.value + 1).min(self.stars as u8);
                    if next == self.value {
                        None
                    } else {
                        self.value = next;
                        Some((self.on_rate)(next))
                    }
                }
                KeyCode::Home => {
                    if self.value == 0 {
                        None
                    } else {
                        self.value = 0;
                        Some((self.on_rate)(0))
                    }
                }
                KeyCode::End => {
                    let next = self.stars as u8;
                    if next == self.value {
                        None
                    } else {
                        self.value = next;
                        Some((self.on_rate)(next))
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Slider, self.id.0.to_string())
                .range(0.0, self.stars as f64, self.value as f64)
                .value(format!("{} / {}", self.value, self.stars))
                .read_only(!self.enabled || self.read_only),
        )
    }

    fn a11y_set_value(&mut self, text: &str) -> Option<M> {
        let value = text.trim().parse::<f64>().ok()?;
        self.rate(value.round().max(0.0) as u8)
    }

    fn set_value(&mut self, value: f32) {
        self.value = (value.round().max(0.0) as u8).min(self.stars as u8);
    }

    fn tooltip_rect(&self) -> Rect {
        self.rect
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Canvas, TextStyle};

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

    #[derive(Default)]
    struct Blank;

    impl Canvas for Blank {
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
        fn ellipse(&mut self, _rect: Rect, _color: Color) {}
        fn vertical_gradient(&mut self, _rect: Rect, _top: Color, _bottom: Color) {}
        fn image(
            &mut self,
            _rect: Rect,
            _crop: bool,
            _id: crate::backend::ImageId,
            _data: crate::backend::ImageData<'_>,
        ) {
        }
        fn push_clip(&mut self, _rect: Rect) {}
        fn pop_clip(&mut self) {}
    }

    fn paint_blank<M: 'static>(widget: &dyn Widget<M>) {
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = Blank;
        let mut paint = WidgetPaint {
            painter: &painter,
            canvas: &mut canvas,
            palette: *painter.palette(),
            font: None,
            images: &images,
            viewport: Rect::from_xywh(0.0, 0.0, 400.0, 300.0),
            dirty: &[],
        };
        widget.paint(
            &mut paint,
            &WidgetState {
                enabled: true,
                ..Default::default()
            },
        );
    }

    #[test]
    fn sparkline_scales_from_zero_and_reports_its_range() {
        let mut spark = Sparkline::new("s", &[1.0, 4.0, 2.0]);
        assert_eq!(spark.range(), (0.0, 4.0));
        paint_blank::<()>(&spark);
        spark.set_data(vec![-2.0, 6.0]);
        assert_eq!(spark.range(), (-2.0, 6.0));
        assert_eq!(spark.data(), &[-2.0, 6.0]);
        paint_blank::<()>(&spark);
        let info = <Sparkline as Widget<()>>::a11y(&spark).unwrap();
        assert!(info.display_only);
        assert_eq!(info.role, A11yRole::Text);
    }

    #[test]
    fn gauge_clamps_and_publishes_a_range() {
        let mut gauge = Gauge::new("g", 0.65).show_value(true);
        assert_eq!(gauge.value_now(), 0.65);
        let info = <Gauge as Widget<()>>::a11y(&gauge).unwrap();
        assert_eq!(info.role, A11yRole::ProgressBar);
        let (min, max, value) = info.range.unwrap();
        assert_eq!((min, max), (0.0, 1.0));
        assert!((value - 0.65).abs() < 1e-6);
        assert!(info.display_only);
        gauge.set_value(2.0);
        assert_eq!(gauge.value_now(), 1.0);
        gauge.set_value(-1.0);
        assert_eq!(gauge.value_now(), 0.0);
        let labeled = Gauge::new("g", 0.5).label("电平");
        assert_eq!(<Gauge as Widget<()>>::a11y(&labeled).unwrap().name, "电平");
        paint_blank::<()>(&labeled);
    }

    #[test]
    fn stars_click_hover_and_clear() {
        let mut stars: RatingStars<u8> =
            RatingStars::new("r", 5, 2, |value| value).clearable(true);
        stars.set_rect(Rect::from_xywh(0.0, 0.0, 130.0, 28.0));
        // Cells are 26 wide; the fourth star is x 78..104.
        assert_eq!(
            stars.input(WidgetInput::ClickAt(Point::new(90.0, 14.0))),
            Some(4)
        );
        assert_eq!(stars.value(), 4);
        assert_eq!(
            stars.input(WidgetInput::ClickAt(Point::new(90.0, 14.0))),
            Some(0),
            "clicking the current value clears a clearable rating"
        );
        assert_eq!(stars.value(), 0);
        stars.input(WidgetInput::HoverAt(Point::new(40.0, 14.0)));
        assert_eq!(stars.hover_value(), Some(2));
        stars.input(WidgetInput::HoverLeave);
        assert_eq!(stars.hover_value(), None);
    }

    #[test]
    fn stars_keyboard_and_a11y_range() {
        let mut stars: RatingStars<u8> = RatingStars::new("r", 5, 2, |value| value);
        stars.set_rect(Rect::from_xywh(0.0, 0.0, 130.0, 28.0));
        assert!(stars.adjustable());
        assert_eq!(stars.input(key(KeyCode::Right)), Some(3));
        assert_eq!(stars.input(key(KeyCode::Home)), Some(0));
        assert_eq!(stars.input(key(KeyCode::End)), Some(5));
        assert_eq!(stars.input(key(KeyCode::Left)), Some(4));
        let info = stars.a11y().unwrap();
        assert_eq!(info.role, A11yRole::Slider);
        assert_eq!(info.range, Some((0.0, 5.0, 4.0)));
        assert_eq!(stars.a11y_set_value("2"), Some(2));
        assert_eq!(stars.value(), 2);
        // A read-only instance ignores input.
        let mut locked: RatingStars<u8> =
            RatingStars::new("r", 5, 3, |value| value).read_only(true);
        locked.set_rect(Rect::from_xywh(0.0, 0.0, 130.0, 28.0));
        assert_eq!(
            locked.input(WidgetInput::ClickAt(Point::new(90.0, 14.0))),
            None
        );
    }

    #[test]
    fn the_point_child_list_replaces_the_samples() {
        let mut spark = Sparkline::new("spark", &[1.0]);
        let mut a = SkinElement::new("point", 2, 3);
        a.push_attr("value", "2.5");
        let mut b = SkinElement::new("point", 3, 3);
        b.push_attr("value", "-1");
        let points = [a, b];
        assert!(<Sparkline as Widget<()>>::validate_skin_children(
            &spark, &points
        ));
        <Sparkline as Widget<()>>::apply_skin_children(&mut spark, &points);
        assert_eq!(spark.data(), &[2.5, -1.0][..]);
        assert_eq!(spark.range(), (-1.0, 2.5));

        // `value` is required and numeric; an unknown attribute is a typo.
        let mut missing = SkinElement::new("point", 4, 3);
        missing.push_attr("v", "1");
        assert!(!<Sparkline as Widget<()>>::validate_skin_children(
            &spark,
            &[missing]
        ));
        let mut bad = SkinElement::new("point", 5, 3);
        bad.push_attr("value", "high");
        assert!(!<Sparkline as Widget<()>>::validate_skin_children(&spark, &[bad]));
    }
}
