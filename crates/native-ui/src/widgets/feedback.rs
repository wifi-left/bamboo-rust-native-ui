// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Overlays and feedback: spinner, progress ring, banner, tooltip card and a
//! modal scrim.
//!
//! Spinners and rings are app-driven like the marquee: the application
//! advances the spinner from its animation tick (or sets the ring value), so
//! an idle page still has no periodic work. The banner and tooltip card are
//! display widgets that opt into interaction only when the instance attaches a
//! handler. The scrim is the backdrop of an in-window modal shell: it is
//! opaque input-wise (it swallows every press) and translucent visually.

use std::cell::Cell;
use std::time::Duration;

use crate::a11y::{A11yInfo, A11yRole};
use crate::backend::TextAlign;
use crate::color::Color;
use crate::event::Cursor;
use crate::geometry::{Point, Rect};
use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

use super::display::Toast;
use crate::skin::{SkinAttrDecl, SkinValue, SkinValueKind};
use crate::surface::{SurfaceStyle, surface_builders};

static SPINNER_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("phase", SkinValueKind::Number),
    SkinAttrDecl::new("animating", SkinValueKind::Bool),
];

static BANNER_SKIN_ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("body", SkinValueKind::Text)];

/// A snackbar is a toast with transient-status semantics (player vocabulary).
pub type Snackbar = Toast;

/// A ring of dots that rotates. The application owns the tick:
/// `Ui::edit_widget::<Spinner>(id, |spinner| spinner.advance(dt))`.
pub struct Spinner {
    id: WidgetId,
    rect: Rect,
    phase: f32,
    /// Degrees per second.
    speed: f32,
    color: Option<Color>,
    /// When false, `advance` leaves the phase alone (a paused spinner).
    animating: bool,
}

impl Spinner {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 24.0, 24.0),
            phase: 0.0,
            speed: 300.0,
            color: None,
            animating: true,
        }
    }

    pub fn speed(mut self, degrees_per_second: f32) -> Self {
        self.speed = degrees_per_second.max(1.0);
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn phase(&self) -> f32 {
        self.phase
    }

    pub fn set_phase(&mut self, degrees: f32) {
        self.phase = degrees.rem_euclid(360.0);
    }

    /// Whether the spinner advances. A paused spinner keeps its painted phase.
    pub fn is_animating(&self) -> bool {
        self.animating
    }

    pub fn set_animating(&mut self, animating: bool) {
        self.animating = animating;
    }

    /// Advance by `dt`; returns whether the painted phase changed.
    pub fn advance(&mut self, dt: Duration) -> bool {
        if !self.animating {
            return false;
        }
        let step = self.speed * dt.as_secs_f32();
        self.phase = (self.phase + step).rem_euclid(360.0);
        step > 0.001
    }
}

impl<M> Widget<M> for Spinner {
    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        SPINNER_SKIN_ATTRS
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("phase", SkinValue::Number(degrees)) => self.set_phase(*degrees as f32),
            ("animating", SkinValue::Bool(animating)) => self.animating = *animating,
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
        24.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        let color = self.color.unwrap_or(palette.accent);
        let center = self.rect.center();
        let radius = (self.rect.width().min(self.rect.height()) / 2.0 - 2.5).max(2.0);
        let dot = (radius * 0.36).clamp(1.5, 3.0);
        let head = ((self.phase / 30.0) as i32).rem_euclid(12);
        for index in 0..12i32 {
            let angle = (self.phase + index as f32 * 30.0 - 90.0).to_radians();
            let x = center.x + radius * angle.cos();
            let y = center.y + radius * angle.sin();
            let age = (head - index).rem_euclid(12) as f32 / 12.0;
            let tint = color.mix(palette.background, 0.25 + 0.7 * age);
            ctx.canvas.fill_circle(
                Rect::new(x - dot, y - dot, x + dot, y + dot),
                tint,
            );
        }
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(A11yInfo::new(A11yRole::Text, self.id.0.to_string()).display_only())
    }
}

/// A circular determinate progress indicator. The ring is drawn from ~60
/// dots so it needs no arc primitive and no assumption about the page color.
pub struct ProgressRing {
    id: WidgetId,
    rect: Rect,
    value: f32,
    thickness: f32,
    show_value: bool,
}

impl ProgressRing {
    pub fn new(id: &'static str, value: f32) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 56.0, 56.0),
            value: value.clamp(0.0, 1.0),
            thickness: 5.0,
            show_value: false,
        }
    }

    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(0.0, 1.0);
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness.clamp(1.0, 12.0);
        self
    }

    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    pub fn value_now(&self) -> f32 {
        self.value
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
    }
}

/// Dots overlap by this fraction of their diameter, so the arc stays smooth
/// even when the step is derived from the radius.
const RING_DOT_OVERLAP: f32 = 0.85;

/// Draw an arc as evenly spaced dots. `from` and `to` are degrees (0 = three
/// o'clock, the ring starts at -90°); the step count follows the sweep, so a
/// short value arc costs proportionally fewer draws.
pub(crate) fn ring_dots(
    ctx: &mut WidgetPaint<'_>,
    rect: Rect,
    from: f32,
    to: f32,
    dot: f32,
    color: Color,
) {
    let radius = (rect.width().min(rect.height()) - dot * 2.0) / 2.0;
    // Space the dots by roughly one dot diameter along the arc, derived from
    // the radius so a large ring keeps its density without ever approaching
    // the thousands of overlapping dots the old degree/fraction mix produced.
    let arc = (dot * RING_DOT_OVERLAP).max(1.0);
    let step_degrees = (arc / radius.max(1.0))
        .to_degrees()
        .clamp(1.5, 12.0);
    let steps = ((to - from).abs() / step_degrees).ceil().clamp(1.0, 360.0) as i32;
    let center = rect.center();
    for index in 0..=steps {
        let t = index as f32 / steps as f32;
        let angle = (from + (to - from) * t - 90.0).to_radians();
        let x = center.x + radius * angle.cos();
        let y = center.y + radius * angle.sin();
        ctx.canvas
            .fill_circle(Rect::new(x - dot / 2.0, y - dot / 2.0, x + dot / 2.0, y + dot / 2.0), color);
    }
}

impl<M> Widget<M> for ProgressRing {
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
        56.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        ring_dots(
            ctx,
            self.rect,
            0.0,
            360.0,
            self.thickness,
            ctx.palette.scroll_track,
        );
        if self.value > 0.0 {
            ring_dots(
                ctx,
                self.rect,
                0.0,
                360.0 * self.value,
                self.thickness,
                ctx.palette.accent,
            );
        }
        if self.show_value {
            ctx.text(
                self.rect,
                &format!("{:.0}%", self.value * 100.0),
                TextStyleRole::Small,
                TextAlign::Center,
                ctx.palette.text,
            );
        }
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::ProgressBar, self.id.0.to_string())
                .range(0.0, 1.0, self.value as f64)
                .value(format!("{:.0}%", self.value * 100.0)),
        )
    }
}

/// Banner emphasis. Colors are fixed per kind (like the danger button) so a
/// warning stays yellow independently of the accent setting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BannerKind {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl BannerKind {
    fn color(self) -> Color {
        match self {
            BannerKind::Info => Color::new(84, 132, 220),
            BannerKind::Success => Color::new(52, 132, 88),
            BannerKind::Warning => Color::new(176, 136, 34),
            BannerKind::Error => Color::new(178, 42, 42),
        }
    }

    fn glyph(self) -> char {
        match self {
            BannerKind::Info => '\u{E946}',
            BannerKind::Success => '\u{E930}',
            BannerKind::Warning => '\u{E7BA}',
            BannerKind::Error => '\u{EA39}',
        }
    }
}

/// An inline notification with an optional action and close target. It stays
/// display-only unless one of those handlers is attached.
pub struct Banner<M> {
    id: WidgetId,
    rect: Rect,
    kind: BannerKind,
    title: String,
    body: String,
    action: Option<(String, Box<dyn Fn() -> M>)>,
    close: Option<Box<dyn Fn() -> M>>,
    hover_action: Cell<bool>,
    hover_close: Cell<bool>,
    /// Painted action rectangle, so hover and click agree with the measured
    /// text width instead of estimating it.
    action_rect: Cell<Rect>,
    surface: SurfaceStyle,
}

impl<M> Banner<M> {
    pub fn new(id: &'static str, kind: BannerKind, title: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 420.0, 60.0),
            kind,
            title: title.into(),
            body: String::new(),
            action: None,
            close: None,
            hover_action: Cell::new(false),
            hover_close: Cell::new(false),
            action_rect: Cell::new(Rect::default()),
            surface: SurfaceStyle::new().with_radius(crate::theme::radius::MEDIUM),
        }
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn body_text(&self) -> &str {
        &self.body
    }

    /// Replace the body text silently (the skin's `body` attribute).
    pub fn set_body(&mut self, body: impl Into<String>) {
        self.body = body.into();
    }

    pub fn action(mut self, label: impl Into<String>, handler: impl Fn() -> M + 'static) -> Self {
        self.action = Some((label.into(), Box::new(handler)));
        self
    }

    pub fn closable(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.close = Some(Box::new(handler));
        self
    }

    pub fn kind(&self) -> BannerKind {
        self.kind
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    fn close_rect(&self) -> Option<Rect> {
        self.close.as_ref()?;
        let size = 20.0;
        Some(Rect::new(
            self.rect.right - size - 8.0,
            self.rect.top + 8.0,
            self.rect.right - 8.0,
            self.rect.top + 8.0 + size,
        ))
    }

    fn action_rect(&self, ctx: &mut WidgetPaint<'_>, before: Rect) -> Option<Rect> {
        let (label, _) = self.action.as_ref()?;
        let width = ctx.measure_text(label, TextStyleRole::Small).width + 16.0;
        Some(Rect::new(
            before.left - width,
            self.rect.top + 12.0,
            before.left,
            self.rect.bottom - 12.0,
        ))
    }
}

impl<M: 'static> Widget<M> for Banner<M> {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        BANNER_SKIN_ATTRS
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        if let ("body", SkinValue::Text(body)) = (name, value) {
            self.body = (*body).to_string();
        }
    }

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
        if self.body.is_empty() { 44.0 } else { 64.0 }
    }

    fn focusable(&self) -> bool {
        false
    }

    fn interactive(&self) -> bool {
        self.action.is_some() || self.close.is_some()
    }

    fn repaint_on_move(&self) -> bool {
        self.interactive()
    }

    fn cursor(&self) -> Option<Cursor> {
        self.interactive().then_some(Cursor::Hand)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        let accent = self.kind.color();
        let background = palette.panel.mix(accent, 0.14);
        let border = accent.mix(palette.panel, 0.35);
        self.surface.frame(ctx.canvas, self.rect, border, background);
        let icon = Rect::new(
            self.rect.left + 12.0,
            self.rect.top,
            self.rect.left + 34.0,
            self.rect.bottom,
        );
        let mut buf = [0u8; 4];
        let glyph = self.kind.glyph().encode_utf8(&mut buf);
        ctx.text(icon, glyph, TextStyleRole::Body, TextAlign::Center, accent);
        let close = self.close_rect();
        let mut right = self.rect.right - 12.0;
        if let Some(close) = close {
            let tint = if self.hover_close.get() {
                palette.text
            } else {
                palette.muted
            };
            ctx.text(close, "\u{00D7}", TextStyleRole::Small, TextAlign::Center, tint);
            right = close.left - 6.0;
        }
        let mut text_right = right;
        self.action_rect.set(Rect::default());
        if self.action.is_some() {
            let action_rect = self.action_rect(
                ctx,
                Rect::new(right, self.rect.top, right, self.rect.bottom),
            );
            if let Some(action_rect) = action_rect {
                let (label, _) = self.action.as_ref().unwrap();
                let tint = if self.hover_action.get() {
                    palette.accent_text.lighten(0.15)
                } else {
                    palette.accent_text
                };
                ctx.text(
                    action_rect,
                    label,
                    TextStyleRole::Small,
                    TextAlign::Center,
                    tint,
                );
                self.action_rect.set(action_rect);
                text_right = action_rect.left - 8.0;
            }
        }
        let text = Rect::new(icon.right + 6.0, self.rect.top, text_right.max(icon.right + 6.0), self.rect.bottom);
        if self.body.is_empty() {
            ctx.text(
                text,
                &self.title,
                TextStyleRole::Body,
                TextAlign::Left,
                palette.text,
            );
        } else {
            let title = Rect::new(text.left, text.top + 6.0, text.right, text.top + 30.0);
            let body = Rect::new(text.left, text.top + 30.0, text.right, text.bottom - 6.0);
            ctx.text(title, &self.title, TextStyleRole::Body, TextAlign::Left, palette.text);
            ctx.text(body, &self.body, TextStyleRole::Small, TextAlign::Left, palette.muted);
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        let close = self.close_rect();
        match input {
            WidgetInput::HoverAt(pos) => {
                self.hover_close
                    .set(close.is_some_and(|rect| rect.contains_point(pos)));
                self.hover_action.set(self.action_rect.get().contains_point(pos));
                None
            }
            WidgetInput::HoverLeave => {
                self.hover_action.set(false);
                self.hover_close.set(false);
                None
            }
            WidgetInput::ClickAt(pos) => {
                if close.is_some_and(|rect| rect.contains_point(pos)) {
                    return self.close.as_ref().map(|handler| handler());
                }
                if let Some((_, handler)) = self.action.as_ref()
                    && self.action_rect.get().contains_point(pos)
                {
                    return Some(handler());
                }
                None
            }
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Group, self.title.clone())
                .description(self.body.clone())
                .display_only(),
        )
    }

    fn set_label(&mut self, label: String) {
        self.title = label;
    }
}

surface_builders!(Banner);

/// A styled tooltip panel for in-page hints (the M4 host tooltip shares the
/// same palette and rounded shape but is a separate window).
pub struct TooltipCard {
    id: WidgetId,
    rect: Rect,
    title: String,
    body: String,
}

impl TooltipCard {
    pub fn new(id: &'static str, title: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 56.0),
            title: title.into(),
            body: String::new(),
        }
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }
}

impl<M> Widget<M> for TooltipCard {
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
        56.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        let painter = ctx.painter;
        painter.round_rect(&mut *ctx.canvas, self.rect, crate::theme::radius::MEDIUM, palette.explicit_border);
        painter.round_rect(
            &mut *ctx.canvas,
            self.rect.inset(1.0, 1.0),
            crate::theme::radius::MEDIUM - 1.0,
            palette.panel,
        );
        let title = Rect::new(
            self.rect.left + 10.0,
            self.rect.top + 5.0,
            self.rect.right - 10.0,
            self.rect.top + 27.0,
        );
        ctx.text(
            title,
            &self.title,
            TextStyleRole::Body,
            TextAlign::Left,
            palette.text,
        );
        if !self.body.is_empty() {
            let body = Rect::new(
                self.rect.left + 10.0,
                self.rect.top + 25.0,
                self.rect.right - 10.0,
                self.rect.bottom - 4.0,
            );
            ctx.text(
                body,
                &self.body,
                TextStyleRole::Small,
                TextAlign::Left,
                palette.muted,
            );
        }
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Text, self.title.clone())
                .description(self.body.clone())
                .display_only(),
        )
    }
}

/// The dimmed, input-blocking backdrop of an in-window modal shell. Add it
/// after the page's widgets and before the modal panel; showing it is a normal
/// visibility toggle, and `Ui::set_focus_scope` traps Tab inside the panel.
pub struct Scrim {
    id: WidgetId,
    rect: Rect,
    alpha: f32,
}

impl Scrim {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::default(),
            alpha: 0.45,
        }
    }

    pub fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha.clamp(0.0, 1.0);
        self
    }
}

impl<M> Widget<M> for Scrim {
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
        0.0
    }

    fn focusable(&self) -> bool {
        false
    }

    /// Blocks every press meant for the page behind a modal panel.
    fn interactive(&self) -> bool {
        true
    }

    fn hit_test(&self, pos: Point) -> bool {
        self.rect.contains_point(pos)
    }

    /// The scrim looks the same hovered, pressed or idle; invalidating its
    /// (full-window) rectangle for every pointer transition forced a complete
    /// repaint under the modal.
    fn paints_pointer_state(&self) -> bool {
        false
    }

    /// The page/overlay split: the container composites this scrim and the
    /// widgets added after it over the cached page (D87).
    fn is_backdrop(&self) -> bool {
        true
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let color = Color::new(0, 0, 0);
        if ctx.dirty.is_empty() {
            ctx.canvas.fill_translucent(self.rect, color, self.alpha);
            return;
        }
        // Only the dirty pieces are blended. The back buffer already holds the
        // dimmed pixels elsewhere; blending the whole window again on every
        // hover/click in a modal was both slow and (when dirty pieces
        // overlapped) darkened twice.
        for dirty in ctx.dirty {
            if let Some(piece) = dirty.intersect(self.rect) {
                ctx.canvas.fill_translucent(piece, color, self.alpha);
            }
        }
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Arrow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Canvas;

    /// Counts ellipse calls: a ring must stay a bounded number of dots.
    struct Counting {
        ellipses: usize,
    }

    impl Canvas for Counting {
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
            _style: &crate::backend::TextStyle,
            _align: TextAlign,
            _color: Color,
        ) {
        }
        fn measure_text(
            &mut self,
            text: &str,
            _style: &crate::backend::TextStyle,
        ) -> crate::geometry::Size {
            crate::geometry::Size::new(text.chars().count() as f32 * 8.0, 16.0)
        }
        fn ellipse(&mut self, _rect: Rect, _color: Color) {
            self.ellipses += 1;
        }
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

    /// Measures 8 DIPs per character and paints nothing.
    struct Stub;

    impl Canvas for Stub {
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
            _style: &crate::backend::TextStyle,
            _align: TextAlign,
            _color: Color,
        ) {
        }
        fn measure_text(
            &mut self,
            text: &str,
            _style: &crate::backend::TextStyle,
        ) -> crate::geometry::Size {
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

    fn place(widget: &mut dyn Widget<()>, rect: Rect) {
        widget.set_rect(rect);
    }

    fn info(widget: &dyn Widget<()>) -> Option<A11yInfo> {
        widget.a11y()
    }

    fn click(widget: &mut dyn Widget<()>, pos: Point) -> Option<()> {
        widget.input(WidgetInput::ClickAt(pos))
    }

    fn interactive(widget: &dyn Widget<()>) -> bool {
        widget.interactive()
    }

    fn hit(widget: &dyn Widget<()>, pos: Point) -> bool {
        widget.hit_test(pos)
    }

    #[test]
    fn spinner_phase_advances_and_wraps() {
        let mut spinner = Spinner::new("s").speed(360.0);
        assert!(spinner.advance(Duration::from_millis(500)));
        assert!((spinner.phase() - 180.0).abs() < 0.01);
        assert!(spinner.advance(Duration::from_millis(500)));
        assert!(spinner.phase() < 0.01);
        spinner.set_phase(-30.0);
        assert!((spinner.phase() - 330.0).abs() < 0.01);
    }

    #[test]
    fn progress_ring_clamps_and_exposes_a_range() {
        let mut ring = ProgressRing::new("r", 1.5).show_value(true);
        assert_eq!(ring.value_now(), 1.0);
        ring.set_value(-0.5);
        assert_eq!(ring.value_now(), 0.0);
        ring.set_value(0.4);
        let info = info(&ring).unwrap();
        assert_eq!(info.role, A11yRole::ProgressBar);
        assert!(info.range.is_some_and(|(min, max, value)| {
            min == 0.0 && max == 1.0 && (value - 0.4).abs() < 1e-5
        }));
        assert_eq!(info.value.as_deref(), Some("40%"));
    }

    #[test]
    fn banner_without_handlers_is_decoration() {
        let banner: Banner<u32> = Banner::new("b", BannerKind::Warning, "注意").body("详情");
        assert!(!banner.interactive());
        let info = banner.a11y().unwrap();
        assert_eq!(info.role, A11yRole::Group);
        assert_eq!(info.name, "注意");
        assert_eq!(info.description.as_deref(), Some("详情"));
        assert!(info.display_only);
    }

    #[test]
    fn banner_action_and_close_emit_their_messages() {
        let mut banner: Banner<u32> = Banner::new("b", BannerKind::Info, "更新")
            .body("有新版本")
            .action("重试", || 1)
            .closable(|| 2);
        assert!(banner.interactive());
        banner.set_rect(Rect::from_xywh(0.0, 0.0, 420.0, 64.0));
        // Close target is the square at the top-right.
        assert_eq!(
            banner.input(WidgetInput::ClickAt(Point::new(404.0, 18.0))),
            Some(2)
        );
        // Paint once so the measured action rectangle is known, then click
        // inside it (the action sits left of the close target).
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = Stub;
        let mut paint = WidgetPaint {
            painter: &painter,
            canvas: &mut canvas,
            palette: *painter.palette(),
            font: None,
            images: &images,
            viewport: Rect::from_xywh(0.0, 0.0, 400.0, 300.0),
            dirty: &[],
        };
        banner.paint(
            &mut paint,
            &WidgetState {
                enabled: true,
                ..Default::default()
            },
        );
        let action = banner.action_rect.get();
        assert_eq!(
            banner.input(WidgetInput::ClickAt(action.center())),
            Some(1)
        );
        assert_eq!(
            banner.input(WidgetInput::ClickAt(Point::new(60.0, 32.0))),
            None
        );
    }

    #[test]
    fn tooltip_card_carries_a_title_and_body() {
        let card = TooltipCard::new("t", "提示").body("这是说明");
        let info = info(&card).unwrap();
        assert_eq!(info.name, "提示");
        assert_eq!(info.description.as_deref(), Some("这是说明"));
        assert!(info.display_only);
    }

    #[test]
    fn scrim_takes_the_pointer_but_emits_nothing() {
        let mut scrim: Scrim = Scrim::new("s");
        place(&mut scrim, Rect::from_xywh(0.0, 0.0, 100.0, 60.0));
        assert!(interactive(&scrim));
        assert!(hit(&scrim, Point::new(50.0, 30.0)));
        assert!(!hit(&scrim, Point::new(150.0, 30.0)));
        assert_eq!(click(&mut scrim, Point::new(50.0, 30.0)), None);
    }

    /// Records translucent fills; everything else is a no-op.
    #[derive(Default)]
    struct FillRecorder {
        translucent: Vec<(Rect, f32)>,
    }

    impl Canvas for FillRecorder {
        fn scale(&self) -> f32 {
            1.0
        }
        fn dpi(&self) -> f32 {
            96.0
        }
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
        fn fill_translucent(&mut self, rect: Rect, _color: Color, alpha: f32) {
            self.translucent.push((rect, alpha));
        }
        fn text(
            &mut self,
            _rect: Rect,
            _text: &str,
            _style: &crate::backend::TextStyle,
            _align: TextAlign,
            _color: Color,
        ) {
        }
        fn measure_text(
            &mut self,
            text: &str,
            _style: &crate::backend::TextStyle,
        ) -> crate::geometry::Size {
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

    #[test]
    fn scrim_blends_only_the_dirty_pieces() {
        let mut scrim: Scrim = Scrim::new("s");
        place(&mut scrim, Rect::from_xywh(0.0, 0.0, 100.0, 60.0));
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let dirty = [
            Rect::new(0.0, 0.0, 20.0, 20.0),
            Rect::new(80.0, 40.0, 140.0, 80.0),
            Rect::new(200.0, 0.0, 220.0, 20.0),
        ];
        let mut canvas = FillRecorder::default();
        let mut paint = WidgetPaint {
            painter: &painter,
            canvas: &mut canvas,
            palette: *painter.palette(),
            font: None,
            images: &images,
            viewport: Rect::from_xywh(0.0, 0.0, 200.0, 100.0),
            dirty: &dirty,
        };
        <Scrim as Widget<()>>::paint(
            &scrim,
            &mut paint,
            &WidgetState {
                enabled: true,
                ..Default::default()
            },
        );
        // Two dirty rectangles intersect the scrim, the third does not; each
        // is blended exactly once over its intersection.
        assert_eq!(canvas.translucent.len(), 2);
        assert_eq!(canvas.translucent[0].0, Rect::new(0.0, 0.0, 20.0, 20.0));
        assert_eq!(canvas.translucent[1].0, Rect::new(80.0, 40.0, 100.0, 60.0));
        assert!((canvas.translucent[0].1 - 0.45).abs() < 0.001);
    }

    #[test]
    fn banner_kinds_use_distinct_colors() {
        let colors = [
            BannerKind::Info.color(),
            BannerKind::Success.color(),
            BannerKind::Warning.color(),
            BannerKind::Error.color(),
        ];
        assert_eq!(colors.len(), 4);
        assert!(colors.iter().all(|color| color.luminance() > 0.1));
    }

    #[test]
    fn a_ring_is_a_bounded_number_of_dots() {
        // A regression guard: the dot spacing is in degrees, so a full ring is
        // ~60 dots. Treating the degree sweep as a fraction once drew 21,600
        // ellipses per frame (tens of milliseconds per widget).
        let mut ring = ProgressRing::new("ring", 0.8);
        <ProgressRing as Widget<u32>>::set_rect(
            &mut ring,
            Rect::from_xywh(0.0, 0.0, 80.0, 80.0),
        );
        let painter = crate::painter::Painter::new();
        let mut canvas = Counting { ellipses: 0 };
        let mut paint = WidgetPaint {
            painter: &painter,
            canvas: &mut canvas,
            palette: *painter.palette(),
            font: None,
            images: &crate::image::ImageStore::new(),
            viewport: Rect::from_xywh(0.0, 0.0, 200.0, 200.0),
            dirty: &[],
        };
        <ProgressRing as Widget<u32>>::paint(&ring, &mut paint, &WidgetState::default());
        assert!(
            canvas.ellipses < 200,
            "ring drew {} ellipses",
            canvas.ellipses
        );
        assert!(canvas.ellipses > 10, "ring must still be a dot arc");
    }
}
