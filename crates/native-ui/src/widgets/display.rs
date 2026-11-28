// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Display-only widgets: section titles, cards, stat blocks, empty states and
//! toasts. They never emit messages.

use crate::a11y::{A11yInfo, A11yRole};
use crate::backend::TextAlign;
use crate::color::Color;
use crate::geometry::{Point, Rect, Size};
use crate::skin::{SkinAttrDecl, SkinValue, SkinValueKind};
use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};
use crate::surface::{SurfaceStyle, surface_builders_plain};
use std::cell::Cell;
use std::time::Duration;

static CARD_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("title", SkinValueKind::Text),
    SkinAttrDecl::new("body", SkinValueKind::Text),
    SkinAttrDecl::new("emphasis", SkinValueKind::Bool),
];

static TOAST_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("message", SkinValueKind::Text),
    SkinAttrDecl::new("error", SkinValueKind::Bool),
];

static SECTION_SKIN_ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("color", SkinValueKind::Color)];

pub struct SectionTitle {
    id: WidgetId,
    rect: Rect,
    label: String,
    color: Option<Color>,
}

impl SectionTitle {
    pub fn new(id: &'static str, label: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 200.0, 24.0),
            label: label.into(),
            color: None,
        }
    }

    /// Replace the explicit text colour silently (the skin's `color`); `None`
    /// uses the palette's muted colour.
    pub fn set_color(&mut self, color: Option<Color>) {
        self.color = color;
    }

    pub fn color_value(&self) -> Option<Color> {
        self.color
    }
}

impl<M> Widget<M> for SectionTitle {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::LABEL
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        SECTION_SKIN_ATTRS
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        if let ("color", SkinValue::Color(_)) = (name, value)
            && let Some(color) = value.as_color()
        {
            self.set_color(Some(color));
        }
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
        24.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        ctx.text(
            self.rect,
            &self.label,
            TextStyleRole::Small,
            TextAlign::Left,
            self.color.unwrap_or(ctx.palette.muted),
        );
    }

    fn set_label(&mut self, label: String) {
        self.label = label;
    }

    /// A heading: excluded from the tree unless the instance opts in, then
    /// exposed as a text element at heading level 2.
    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Heading, self.label.clone())
                .display_only(),
        )
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }
}

pub struct Card {
    id: WidgetId,
    rect: Rect,
    title: String,
    body: String,
    padding: f32,
    emphasis: bool,
    surface: SurfaceStyle,
}

impl Card {
    pub fn new(id: &'static str, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 96.0),
            title: title.into(),
            body: body.into(),
            padding: 14.0,
            emphasis: false,
            surface: SurfaceStyle::new(),
        }
    }

    /// Inner padding in DIPs (default 14).
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding.max(0.0);
        self
    }

    /// Draw an accent strip on the leading edge (selected/highlighted card).
    pub fn emphasis(mut self, emphasis: bool) -> Self {
        self.emphasis = emphasis;
        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    /// Replace the card title (silent).
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Replace the card body (silent).
    pub fn set_body(&mut self, body: impl Into<String>) {
        self.body = body.into();
    }

    pub fn is_emphasis(&self) -> bool {
        self.emphasis
    }

    pub fn set_emphasis(&mut self, emphasis: bool) {
        self.emphasis = emphasis;
    }
}

impl<M> Widget<M> for Card {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        CARD_SKIN_ATTRS
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("title", SkinValue::Text(title)) => self.title = (*title).to_string(),
            ("body", SkinValue::Text(body)) => self.body = (*body).to_string(),
            ("emphasis", SkinValue::Bool(emphasis)) => self.emphasis = *emphasis,
            _ => {}
        }
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
        96.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let pal = ctx.palette;
        self.surface.frame(ctx.canvas, self.rect, pal.divider, pal.card);
        if self.emphasis {
            ctx.canvas.fill_rect(
                Rect::new(self.rect.left, self.rect.top, self.rect.left + 3.0, self.rect.bottom),
                pal.accent,
            );
        }
        let left = self.rect.left + self.padding + if self.emphasis { 3.0 } else { 0.0 };
        ctx.text(
            Rect::new(
                left,
                self.rect.top + 10.0,
                self.rect.right - self.padding,
                self.rect.top + 34.0,
            ),
            &self.title,
            TextStyleRole::Body,
            TextAlign::Left,
            pal.text,
        );
        ctx.text(
            Rect::new(
                left,
                self.rect.top + 38.0,
                self.rect.right - self.padding,
                self.rect.bottom - 10.0,
            ),
            &self.body,
            TextStyleRole::Small,
            TextAlign::Left,
            pal.muted,
        );
    }

    /// A named group whose description is the card body.
    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Group, self.title.clone())
                .description(self.body.clone())
                .display_only(),
        )
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }
}

/// A titled panel frame (fieldset). Paints a card background, a 1 DIP border,
/// a small title and a rule under it; the application places ordinary widgets
/// inside the rectangle. Decorative by default (D27): a screen reader sees it
/// only when the instance opts in, then as a named group.
pub struct GroupBox {
    id: WidgetId,
    rect: Rect,
    title: String,
}

impl GroupBox {
    pub fn new(id: &'static str, title: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 120.0),
            title: title.into(),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// Height of the title band; children are placed below it by the app.
    pub fn header_height(&self) -> f32 {
        28.0
    }
}

impl<M> Widget<M> for GroupBox {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::LABEL
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
        120.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        ctx.canvas.frame(self.rect, palette.explicit_border, palette.card);
        if self.rect.width() > 20.0 && self.rect.height() > 28.0 {
            ctx.text(
                Rect::new(
                    self.rect.left + 10.0,
                    self.rect.top + 6.0,
                    self.rect.right - 10.0,
                    self.rect.top + 24.0,
                ),
                &self.title,
                TextStyleRole::Small,
                TextAlign::Left,
                palette.text,
            );
            ctx.canvas.fill_rect(
                Rect::new(
                    self.rect.left + 1.0,
                    self.rect.top + 28.0,
                    self.rect.right - 1.0,
                    self.rect.top + 29.0,
                ),
                palette.divider,
            );
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(A11yInfo::new(A11yRole::Group, self.title.clone()).display_only())
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn set_label(&mut self, label: String) {
        self.title = label;
    }
}

pub struct StatCard {
    id: WidgetId,
    rect: Rect,
    value: String,
    label: String,
    padding: f32,
    emphasis: bool,
}

impl StatCard {
    pub fn new(id: &'static str, value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 140.0, 72.0),
            value: value.into(),
            label: label.into(),
            padding: 12.0,
            emphasis: false,
        }
    }

    /// Inner padding in DIPs (default 12).
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding.max(0.0);
        self
    }

    /// Tint the card background with the accent (a highlighted statistic).
    pub fn emphasis(mut self, emphasis: bool) -> Self {
        self.emphasis = emphasis;
        self
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
    }
}

impl<M> Widget<M> for StatCard {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::LABEL
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
        72.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let pal = ctx.palette;
        let background = if self.emphasis { pal.play_button } else { pal.card };
        ctx.canvas.frame(self.rect, pal.divider, background);
        ctx.text(
            Rect::new(
                self.rect.left + self.padding,
                self.rect.top + 8.0,
                self.rect.right - self.padding,
                self.rect.top + 44.0,
            ),
            &self.value,
            TextStyleRole::Title,
            TextAlign::Left,
            pal.accent_text,
        );
        ctx.text(
            Rect::new(
                self.rect.left + self.padding,
                self.rect.top + 46.0,
                self.rect.right - self.padding,
                self.rect.bottom - 6.0,
            ),
            &self.label,
            TextStyleRole::Small,
            TextAlign::Left,
            pal.muted,
        );
    }

    fn set_label(&mut self, value: String) {
        self.value = value;
    }

    /// Read-only text: the caption followed by the current value.
    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(
                A11yRole::Text,
                format!("{} {}", self.label, self.value),
            )
            .value(self.value.clone())
            .display_only(),
        )
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }
}

pub struct EmptyState {
    id: WidgetId,
    rect: Rect,
    title: String,
    hint: String,
}

impl EmptyState {
    pub fn new(id: &'static str, title: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 320.0, 120.0),
            title: title.into(),
            hint: hint.into(),
        }
    }
}

impl<M> Widget<M> for EmptyState {
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
        120.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let pal = ctx.palette;
        let center = self.rect.center();
        let title = Rect::new(
            self.rect.left,
            center.y - 28.0,
            self.rect.right,
            center.y - 4.0,
        );
        let hint = Rect::new(
            self.rect.left,
            center.y,
            self.rect.right,
            center.y + 24.0,
        );
        ctx.text(
            title,
            &self.title,
            TextStyleRole::Body,
            TextAlign::Center,
            pal.text,
        );
        ctx.text(
            hint,
            &self.hint,
            TextStyleRole::Small,
            TextAlign::Center,
            pal.muted,
        );
    }

    /// Read-only status block: title name, hint as the description.
    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Text, self.title.clone())
                .description(self.hint.clone())
                .display_only(),
        )
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }
}

pub struct Toast {
    id: WidgetId,
    rect: Rect,
    text: String,
    error: bool,
    visible: bool,
    /// How long the message should stay up; the application owns the timer.
    duration: Option<Duration>,
    /// Right-aligned action label ("撤销", "查看"); pair it with an
    /// `on_click` binding and `WidgetProps::mouse(true)`.
    action: String,
    /// The action label's painted rectangle, cached for `action_at`.
    action_zone: Cell<Option<Rect>>,
    surface: SurfaceStyle,
}

impl Toast {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 320.0, 44.0),
            text: String::new(),
            error: false,
            visible: false,
            duration: None,
            action: String::new(),
            action_zone: Cell::new(None),
            surface: SurfaceStyle::new(),
        }
    }

    /// Lifetime the application should use for its auto-hide timer.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn duration_secs(mut self, seconds: f32) -> Self {
        self.duration = Some(Duration::from_secs_f32(seconds.max(0.0)));
        self
    }

    pub fn duration_value(&self) -> Option<Duration> {
        self.duration
    }

    /// Action label painted on the right edge.
    pub fn action(mut self, action: impl Into<String>) -> Self {
        self.action = action.into();
        self
    }

    pub fn action_text(&self) -> &str {
        &self.action
    }

    /// The action label's rectangle from the last paint, or `None` when the
    /// toast has no action (or has never painted). Lets an application tell an
    /// action click from a body click in the trigger it already binds to the
    /// toast (`TriggerEvent::pos`).
    pub fn action_rect(&self) -> Option<Rect> {
        self.action_zone.get()
    }

    /// Whether `pos` hits the action label as it was last painted.
    pub fn action_at(&self, pos: Point) -> bool {
        self.action_zone
            .get()
            .is_some_and(|rect| rect.contains_point(pos))
    }

    pub fn show(&mut self, text: impl Into<String>, error: bool) {
        self.text = text.into();
        self.error = error;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Current toast text (typed access seam).
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_error(&self) -> bool {
        self.error
    }

    /// Replace the message without changing visibility (the skin's `message`
    /// attribute; visibility stays the application's decision).
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Replace the error flag without changing visibility (the skin's `error`
    /// attribute).
    pub fn set_error(&mut self, error: bool) {
        self.error = error;
    }
}

impl<M> Widget<M> for Toast {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        TOAST_SKIN_ATTRS
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("message", SkinValue::Text(text)) => self.text = (*text).to_string(),
            ("error", SkinValue::Bool(error)) => self.error = *error,
            _ => {}
        }
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
        44.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn set_message(&mut self, text: String, error: bool) {
        self.text = text;
        self.error = error;
        self.visible = true;
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        if !self.visible {
            return;
        }
        let pal = ctx.palette;
        self.surface.fill(ctx.canvas, self.rect, pal.card);
        ctx.canvas.fill_rect(
            Rect::new(self.rect.left, self.rect.top, self.rect.left + 3.0, self.rect.bottom),
            if self.error { pal.explicit_border } else { pal.accent },
        );
        let mut text_right = self.rect.right - 14.0;
        if !self.action.is_empty() {
            let width = ctx
                .measure_text(&self.action, TextStyleRole::Small)
                .width
                .max(28.0);
            let action = Rect::new(
                text_right - width,
                self.rect.top,
                text_right,
                self.rect.bottom,
            );
            self.action_zone.set(Some(action));
            ctx.text(
                action,
                &self.action,
                TextStyleRole::Small,
                TextAlign::Right,
                if self.error { pal.explicit_border } else { pal.accent },
            );
            text_right = (action.left - 12.0).max(self.rect.left + 14.0);
        } else {
            self.action_zone.set(None);
        }
        ctx.text(
            Rect::new(self.rect.left + 14.0, self.rect.top, text_right, self.rect.bottom),
            &self.text,
            TextStyleRole::Small,
            TextAlign::Left,
            pal.text,
        );
    }

    /// A toast is a live status message. It is narration for assistive
    /// technology, not a control: exposed only when opted in, and its text is
    /// also pushed as a UIA notification by the application
    /// (`WindowHandle::a11y_announce`).
    fn a11y(&self) -> Option<A11yInfo> {
        let info = A11yInfo::new(A11yRole::Text, self.text.clone())
            .description(if self.error { "错误提示" } else { "提示" })
            .display_only();
        Some(info)
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }
}

surface_builders_plain!(Card);
surface_builders_plain!(Toast);

/// Convenience: a toast sized to fit a text run.
pub fn toast_size(painter: &crate::painter::Painter, canvas: &mut dyn crate::backend::Canvas, text: &str) -> Size {
    let measured = canvas.measure_text(text, painter.small());
    Size::new((measured.width + 36.0).clamp(160.0, 520.0), 44.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_carries_a_duration_and_an_action_label() {
        let mut toast = Toast::new("toast")
            .duration(Duration::from_secs(3))
            .action("撤销");
        assert_eq!(toast.duration_value(), Some(Duration::from_secs(3)));
        assert_eq!(toast.action_text(), "撤销");
        toast.show("已删除", false);
        assert!(toast.is_visible());
        assert_eq!(toast.text(), "已删除");

        let toast = Toast::new("toast").duration_secs(1.5);
        assert_eq!(toast.duration_value(), Some(Duration::from_millis(1500)));
        assert!(Toast::new("t").duration_value().is_none());
    }

    #[test]
    fn group_box_is_a_decorative_named_group() {
        let mut group = GroupBox::new("g", "设置");
        assert_eq!(group.title(), "设置");
        assert_eq!(group.header_height(), 28.0);
        assert!(!<GroupBox as Widget<()>>::focusable(&group));
        let info = <GroupBox as Widget<()>>::a11y(&group).unwrap();
        assert_eq!(info.role, A11yRole::Group);
        assert_eq!(info.name, "设置");
        assert!(info.display_only);
        <GroupBox as Widget<()>>::set_label(&mut group, "播放设置".to_string());
        assert_eq!(
            <GroupBox as Widget<()>>::a11y(&group).unwrap().name,
            "播放设置"
        );
    }

    #[test]
    fn card_and_stat_padding_and_emphasis_are_configurable() {
        let card = Card::new("c", "标题", "正文").padding(20.0).emphasis(true);
        assert_eq!(card.padding, 20.0);
        assert!(card.emphasis);

        let stat = StatCard::new("s", "42", "曲目").padding(4.0).emphasis(true);
        assert_eq!(stat.padding, 4.0);
        assert!(stat.emphasis);
    }

    #[test]
    fn card_surface_rounds_uniformly_or_per_corner() {
        use crate::geometry::Corners;
        let rounded = Card::new("c", "标题", "正文").corner_radius(14.0);
        assert_eq!(rounded.surface().corners, Corners::uniform(14.0));

        let cut = Card::new("c", "标题", "正文").corners(Corners::new(16.0, 16.0, 0.0, 0.0));
        assert_eq!(cut.surface().corners, Corners::new(16.0, 16.0, 0.0, 0.0));
        assert_eq!(cut.surface().corners.max(), 16.0);
    }
}
