// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Text and content widgets: labels, icons, badges, dividers, avatars and
//! shortcut hints.
//!
//! These are decoration by default (`display_only`), the same contract as
//! titles and cards: they stay out of Tab and the accessibility tree until the
//! instance opts in through [`crate::WidgetProps`] (`focusable`, `mouse`,
//! `expose`, narration or a role override). A link label is the exception: it
//! is interactive so pointer hit-testing and a `Click` binding reach it, and a
//! removable badge is interactive because it owns a close target.

use std::cell::RefCell;

use crate::a11y::{A11yInfo, A11yRole};
use crate::backend::{Canvas, TextAlign, TextStyle, TextOverflow};
use crate::color::Color;
use crate::event::{Cursor, KeyCode};
use crate::geometry::{Point, Rect};
use crate::skin::{SkinAttrDecl, SkinValue, SkinValueKind};
use crate::text::wrap_ranges;
use crate::theme::Palette;
use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};
use crate::surface::{SurfaceStyle, surface_builders_plain};

static ICON_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("glyph", SkinValueKind::Text),
    SkinAttrDecl::new("tint", SkinValueKind::Color),
];

static BADGE_SKIN_ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("tone", SkinValueKind::Text)];

static AVATAR_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("initials", SkinValueKind::Text),
    SkinAttrDecl::new("tint", SkinValueKind::Color),
    SkinAttrDecl::new("ring", SkinValueKind::Bool),
    // The numeric `ImageId`, the same convention as a `<cover image>` (D147):
    // a page has no image-name registry, and the application owns the ids.
    SkinAttrDecl::new("image", SkinValueKind::Number),
];

static KBD_SKIN_ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("shortcut", SkinValueKind::Text)];

static LABEL_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("color", SkinValueKind::Color),
    SkinAttrDecl::new("text-style", SkinValueKind::Text),
];

/// Theme text role a [`Label`] paints with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LabelStyle {
    #[default]
    Body,
    Small,
    Title,
}

pub(crate) fn label_style_from_name(name: &str) -> Option<LabelStyle> {
    match name {
        "body" => Some(LabelStyle::Body),
        "small" => Some(LabelStyle::Small),
        "title" => Some(LabelStyle::Title),
        _ => None,
    }
}

impl LabelStyle {
    pub(crate) fn resolved(self, ctx: &WidgetPaint<'_>) -> TextStyle {
        let base = match self {
            LabelStyle::Body => ctx.painter.body(),
            LabelStyle::Small => ctx.painter.small(),
            LabelStyle::Title => ctx.painter.title(),
        };
        ctx.style(base)
    }
}

/// Reused line ranges for a wrapping label. Recomputed only when the text, the
/// width or the effective font size changes, so an idle repaint allocates
/// nothing.
#[derive(Default)]
struct WrapCache {
    width: f32,
    font_size: f32,
    text_len: usize,
    ranges: Vec<(usize, usize)>,
    /// The text changed since the last computation.
    stale: bool,
}

pub struct Label {
    id: WidgetId,
    rect: Rect,
    text: String,
    wrap: bool,
    ellipsis: bool,
    align: TextAlign,
    style: LabelStyle,
    link: bool,
    color: Option<Color>,
    cache: RefCell<WrapCache>,
    surface: SurfaceStyle,
}

impl Label {
    pub fn new(id: &'static str, text: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 24.0),
            text: text.into(),
            wrap: false,
            ellipsis: true,
            align: TextAlign::Left,
            style: LabelStyle::Body,
            link: false,
            color: None,
            cache: RefCell::new(WrapCache::default()),
            surface: SurfaceStyle::new(),
        }
    }

    /// Wrap onto as many lines as the rectangle height holds. A hard `\n`
    /// always starts a new line.
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Single-line edge behavior: an ellipsis (default) or a hard clip.
    pub fn ellipsis(mut self, ellipsis: bool) -> Self {
        self.ellipsis = ellipsis;
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn style(mut self, style: LabelStyle) -> Self {
        self.style = style;
        self
    }

    /// Paint in the link color with an underline, take pointer input (so a
    /// `Click` binding fires) and show the hand cursor.
    pub fn link(mut self, link: bool) -> Self {
        self.link = link;
        self
    }

    /// Explicit text color; the default is the palette text (or the accent
    /// text for a link).
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replace the explicit text colour silently (the skin's `color`); `None`
    /// restores the palette default.
    pub fn set_color(&mut self, color: Option<Color>) {
        self.color = color;
    }

    /// The explicit text colour, when one is set.
    pub fn color_value(&self) -> Option<Color> {
        self.color
    }

    /// Replace the text role silently (the skin's `text-style`).
    pub fn set_text_style(&mut self, style: LabelStyle) {
        if style != self.style {
            self.style = style;
            self.cache.borrow_mut().stale = true;
        }
    }

    pub fn text_style(&self) -> LabelStyle {
        self.style
    }

    fn text_color(&self, palette: &Palette) -> Color {
        self.color.unwrap_or(if self.link {
            palette.accent_text
        } else {
            palette.text
        })
    }
}

/// Draw a 1 DIP underline below one line, following its alignment.
fn underline(
    canvas: &mut dyn Canvas,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    align: TextAlign,
    color: Color,
) {
    let width = canvas.measure_text(text, style).width.min(rect.width());
    if width <= 0.0 {
        return;
    }
    let left = match align {
        TextAlign::Left => rect.left,
        TextAlign::Center => rect.left + (rect.width() - width) / 2.0,
        TextAlign::Right => rect.right - width,
    };
    canvas.fill_rect(
        Rect::new(left, rect.bottom - 3.0, left + width, rect.bottom - 2.0),
        color,
    );
}

impl<M> Widget<M> for Label {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
    }

    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::LABEL
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        LABEL_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("color", SkinValue::Color(_)) => true,
            ("text-style", SkinValue::Text(style)) => label_style_from_name(style).is_some(),
            _ => false,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("color", SkinValue::Color(_)) => {
                if let Some(color) = value.as_color() {
                    self.set_color(Some(color));
                }
            }
            ("text-style", SkinValue::Text(style)) => {
                if let Some(style) = label_style_from_name(style) {
                    self.set_text_style(style);
                }
            }
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
        match self.style {
            LabelStyle::Small => 20.0,
            LabelStyle::Body => 24.0,
            LabelStyle::Title => 30.0,
        }
    }

    fn focusable(&self) -> bool {
        false
    }

    /// A link is clickable even though it is not a Tab stop; the application
    /// makes it focusable when keyboard activation matters.
    fn interactive(&self) -> bool {
        self.link
    }

    fn cursor(&self) -> Option<Cursor> {
        self.link.then_some(Cursor::Hand)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let style = self.style.resolved(ctx);
        let color = self.text_color(&ctx.palette);
        // An explicit background (a chip-style label) paints behind the text
        // with the configured corner radius; a plain label has no fill.
        if let Some(background) = self.surface.background {
            self.surface.shape(ctx.canvas, self.rect, background);
        }
        if !self.wrap {
            ctx.text_overflow(
                self.rect,
                &self.text,
                match self.style {
                    LabelStyle::Body => TextStyleRole::Body,
                    LabelStyle::Small => TextStyleRole::Small,
                    LabelStyle::Title => TextStyleRole::Title,
                },
                self.align,
                color,
                if self.ellipsis {
                    TextOverflow::Ellipsis
                } else {
                    TextOverflow::Clip
                },
            );
            if self.link {
                underline(ctx.canvas, self.rect, &self.text, &style, self.align, color);
            }
            return;
        }

        let width = self.rect.width();
        let mut cache = self.cache.borrow_mut();
        if cache.stale
            || cache.width != width
            || cache.font_size != style.size
            || cache.text_len != self.text.len()
        {
            cache.ranges = wrap_ranges(&self.text, width, &mut |slice| {
                ctx.canvas.measure_text(slice, &style).width
            });
            cache.width = width;
            cache.font_size = style.size;
            cache.text_len = self.text.len();
            cache.stale = false;
        }
        let line_height = ctx.canvas.measure_text("Ag", &style).height.max(style.size);
        for (index, &(start, end)) in cache.ranges.iter().enumerate() {
            let top = self.rect.top + index as f32 * line_height;
            // A line that does not fully fit is not painted: a clipped half
            // line reads as a rendering glitch.
            if top + line_height > self.rect.bottom {
                break;
            }
            let line = Rect::new(self.rect.left, top, self.rect.right, top + line_height);
            let slice = &self.text[start..end];
            ctx.canvas.text(line, slice, &style, self.align, color);
            if self.link {
                underline(ctx.canvas, line, slice, &style, self.align, color);
            }
        }
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(A11yInfo::new(A11yRole::Text, self.text.clone()).display_only())
    }

    fn set_label(&mut self, label: String) {
        if label != self.text {
            self.text = label;
            self.cache.borrow_mut().stale = true;
        }
    }
}

surface_builders_plain!(Label);

pub struct Icon {
    id: WidgetId,
    rect: Rect,
    glyph: char,
    size: Option<f32>,
    tint: Option<Color>,
}

impl Icon {
    pub fn new(id: &'static str, glyph: char) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 24.0, 24.0),
            glyph,
            size: None,
            tint: None,
        }
    }

    /// Glyph size in DIPs; the default is 80% of the cell height.
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size.max(1.0));
        self
    }

    pub fn tint(mut self, color: Color) -> Self {
        self.tint = Some(color);
        self
    }

    pub fn set_glyph(&mut self, glyph: char) {
        self.glyph = glyph;
    }

    pub fn glyph(&self) -> char {
        self.glyph
    }

    pub fn tint_color(&self) -> Option<Color> {
        self.tint
    }

    pub fn set_tint(&mut self, tint: Option<Color>) {
        self.tint = tint;
    }
}

impl<M> Widget<M> for Icon {
    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        ICON_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("glyph", SkinValue::Text(text)) => text.chars().count() == 1,
            _ => true,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("glyph", SkinValue::Text(text)) => {
                if let Some(glyph) = text.chars().next() {
                    self.glyph = glyph;
                }
            }
            // A tint has no alpha; an RGBA value drops it silently.
            ("tint", value) => {
                if let Some(color) = value.as_color() {
                    self.tint = Some(color);
                }
            }
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
        let size = self.size.unwrap_or(self.rect.height() * 0.8).max(1.0);
        let style = TextStyle::glyph(size);
        let color = self.tint.unwrap_or(ctx.palette.muted);
        let mut buf = [0u8; 4];
        let text = self.glyph.encode_utf8(&mut buf);
        ctx.canvas.text(self.rect, text, &style, TextAlign::Center, color);
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(A11yInfo::new(A11yRole::Text, String::new()).display_only())
    }
}

/// Badge emphasis. Tones other than `Neutral`/`Accent` use fixed colors, like
/// the destructive button, so they do not follow the accent setting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BadgeTone {
    #[default]
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
}

impl BadgeTone {
    /// Parse the skin's `tone` text (`neutral`, `accent`, `success`,
    /// `warning`, `danger`).
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "neutral" => Some(Self::Neutral),
            "accent" => Some(Self::Accent),
            "success" => Some(Self::Success),
            "warning" => Some(Self::Warning),
            "danger" => Some(Self::Danger),
            _ => None,
        }
    }

    fn colors(self, palette: &Palette) -> (Color, Color) {
        match self {
            BadgeTone::Neutral => (palette.button_bg, palette.text),
            BadgeTone::Accent => (palette.button_active, palette.accent_text),
            BadgeTone::Success => {
                let bg = if palette.dark {
                    Color::new(58, 124, 86)
                } else {
                    Color::new(52, 132, 88)
                };
                (bg, Color::new(242, 250, 245))
            }
            BadgeTone::Warning => {
                let bg = if palette.dark {
                    Color::new(158, 118, 34)
                } else {
                    Color::new(176, 136, 34)
                };
                (bg, Color::new(32, 25, 10))
            }
            BadgeTone::Danger => {
                let bg = if palette.dark {
                    Color::new(188, 66, 66)
                } else {
                    Color::new(178, 42, 42)
                };
                (bg, Color::new(252, 246, 246))
            }
        }
    }
}

/// A small status chip. `.removable(handler)` paints a close target; clicking
/// it (or Delete/Backspace while the instance is focused) emits the message.
pub struct Badge<M> {
    id: WidgetId,
    rect: Rect,
    text: String,
    tone: BadgeTone,
    on_remove: Option<Box<dyn Fn() -> M>>,
    /// Pointer is over the close target (not merely the chip).
    hover_close: std::cell::Cell<bool>,
}

impl<M> Badge<M> {
    pub fn new(id: &'static str, text: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 64.0, 22.0),
            text: text.into(),
            tone: BadgeTone::Neutral,
            on_remove: None,
            hover_close: std::cell::Cell::new(false),
        }
    }

    pub fn tone(mut self, tone: BadgeTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn tone_value(&self) -> BadgeTone {
        self.tone
    }

    /// Replace the tone silently (the skin's `tone` attribute).
    pub fn set_tone(&mut self, tone: BadgeTone) {
        self.tone = tone;
    }

    pub fn removable(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.on_remove = Some(Box::new(handler));
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_removable(&self) -> bool {
        self.on_remove.is_some()
    }

    fn close_rect(&self) -> Option<Rect> {
        self.on_remove.as_ref()?;
        let width = self.rect.height().min(24.0);
        Some(Rect::new(
            self.rect.right - width,
            self.rect.top,
            self.rect.right,
            self.rect.bottom,
        ))
    }
}

impl<M: 'static> Widget<M> for Badge<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::LABEL
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        BADGE_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("tone", SkinValue::Text(text)) => BadgeTone::parse(text).is_some(),
            _ => true,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        if let ("tone", SkinValue::Text(text)) = (name, value)
            && let Some(tone) = BadgeTone::parse(text)
        {
            self.tone = tone;
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
        22.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn interactive(&self) -> bool {
        self.on_remove.is_some()
    }

    fn repaint_on_move(&self) -> bool {
        self.on_remove.is_some()
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let (background, foreground) = self.tone.colors(&ctx.palette);
        let border = if self.tone == BadgeTone::Neutral {
            ctx.palette.explicit_border
        } else {
            background
        };
        let radius = self.rect.height() / 2.0;
        let painter = ctx.painter;
        painter.round_rect(&mut *ctx.canvas, self.rect, radius, border);
        painter.round_rect(
            &mut *ctx.canvas,
            self.rect.inset(1.0, 1.0),
            (radius - 1.0).max(0.0),
            background,
        );
        let close = self.close_rect();
        let padding = 8.0;
        let text_right = close.map_or(self.rect.right - padding, |close| close.left - 2.0);
        let text = Rect::new(
            self.rect.left + padding,
            self.rect.top,
            text_right.max(self.rect.left + padding),
            self.rect.bottom,
        );
        ctx.text(text, &self.text, TextStyleRole::Small, TextAlign::Center, foreground);
        if let Some(close) = close {
            let color = if self.hover_close.get() {
                foreground
            } else {
                foreground.mix(background, 0.35)
            };
            ctx.text(
                close,
                "\u{00D7}",
                TextStyleRole::Small,
                TextAlign::Center,
                color,
            );
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        let close = self.close_rect();
        match input {
            WidgetInput::HoverAt(pos) => {
                self.hover_close
                    .set(close.is_some_and(|close| close.contains_point(pos)));
                None
            }
            WidgetInput::HoverLeave => {
                self.hover_close.set(false);
                None
            }
            WidgetInput::ClickAt(pos) => {
                if close.is_some_and(|close| close.contains_point(pos)) {
                    return self.on_remove.as_ref().map(|handler| handler());
                }
                None
            }
            WidgetInput::Key(key) => match key.code {
                KeyCode::Delete | KeyCode::Backspace => {
                    self.on_remove.as_ref().map(|handler| handler())
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(A11yInfo::new(A11yRole::Text, self.text.clone()).display_only())
    }

    fn set_label(&mut self, label: String) {
        self.text = label;
    }
}

/// A horizontal or vertical rule. A horizontal rule can carry a centered
/// label; a vertical one is a plain line.
pub struct Divider {
    id: WidgetId,
    rect: Rect,
    vertical: bool,
    label: Option<String>,
    color: Option<Color>,
}

impl Divider {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 1.0),
            vertical: false,
            label: None,
            color: None,
        }
    }

    pub fn vertical(mut self, vertical: bool) -> Self {
        self.vertical = vertical;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

impl<M> Widget<M> for Divider {
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
        if self.vertical { 40.0 } else { 16.0 }
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let color = self.color.unwrap_or(ctx.palette.divider);
        if self.vertical {
            let x = self.rect.center().x;
            ctx.canvas
                .fill_rect(Rect::new(x, self.rect.top, x + 1.0, self.rect.bottom), color);
            return;
        }
        let y = self.rect.center().y;
        let Some(label) = self.label.as_deref() else {
            ctx.canvas
                .fill_rect(Rect::new(self.rect.left, y, self.rect.right, y + 1.0), color);
            return;
        };
        let width = ctx.measure_text(label, TextStyleRole::Small).width;
        let text = Rect::new(
            self.rect.center().x - width / 2.0,
            y - 8.0,
            self.rect.center().x + width / 2.0,
            y + 8.0,
        );
        let gap = 8.0;
        ctx.canvas.fill_rect(
            Rect::new(self.rect.left, y, (text.left - gap).max(self.rect.left), y + 1.0),
            color,
        );
        ctx.canvas.fill_rect(
            Rect::new((text.right + gap).min(self.rect.right), y, self.rect.right, y + 1.0),
            color,
        );
        ctx.text(
            text,
            label,
            TextStyleRole::Small,
            TextAlign::Center,
            ctx.palette.muted,
        );
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn a11y(&self) -> Option<A11yInfo> {
        self.label
            .as_ref()
            .map(|label| A11yInfo::new(A11yRole::Text, label.clone()).display_only())
    }

    fn set_label(&mut self, label: String) {
        self.label = Some(label);
    }
}

/// Fixed avatar tints; picked deterministically from the name so an artist
/// keeps one color across pages.
const AVATAR_TINTS: [Color; 6] = [
    Color::new(96, 134, 208),
    Color::new(120, 164, 104),
    Color::new(184, 128, 72),
    Color::new(156, 108, 176),
    Color::new(88, 164, 164),
    Color::new(192, 96, 112),
];

/// A round avatar with initials, or a square-cropped image when one is
/// registered with [`Avatar::image`].
pub struct Avatar {
    id: WidgetId,
    rect: Rect,
    name: String,
    initials: String,
    tint: Option<Color>,
    ring: bool,
    image: Option<crate::backend::ImageId>,
}

impl Avatar {
    pub fn new(id: &'static str, name: impl Into<String>) -> Self {
        let name = name.into();
        let initials = derive_initials(&name);
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 40.0, 40.0),
            name,
            initials,
            tint: None,
            ring: false,
            image: None,
        }
    }

    /// Explicit circle text (overrides the derived initials).
    pub fn initials(mut self, initials: impl Into<String>) -> Self {
        self.initials = initials.into();
        self
    }

    /// Show an image registered with [`crate::Ui::set_image`] instead of the
    /// initials. The source is center-cropped to a square and clipped to the
    /// avatar's circle (anti-aliased); the ring follows the circle. An
    /// unregistered id falls back to initials.
    pub fn image(mut self, id: crate::backend::ImageId) -> Self {
        self.image = Some(id);
        self
    }

    pub fn tint(mut self, color: Color) -> Self {
        self.tint = Some(color);
        self
    }

    /// Draw a 1 DIP ring around the circle (overlapping avatars).
    pub fn ring(mut self, ring: bool) -> Self {
        self.ring = ring;
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn initials_text(&self) -> &str {
        &self.initials
    }

    /// Replace the circle text silently (the skin's `initials` attribute).
    pub fn set_initials(&mut self, initials: impl Into<String>) {
        self.initials = initials.into();
    }

    pub fn tint_color(&self) -> Option<Color> {
        self.tint
    }

    pub fn set_tint(&mut self, tint: Option<Color>) {
        self.tint = tint;
    }

    pub fn has_ring(&self) -> bool {
        self.ring
    }

    pub fn set_ring(&mut self, ring: bool) {
        self.ring = ring;
    }

    pub fn image_id(&self) -> Option<crate::backend::ImageId> {
        self.image
    }

    /// Show a registered page image instead of the initials, or go back to the
    /// initials with `None` (the skin's `image` attribute; silent).
    pub fn set_image_id(&mut self, image: Option<crate::backend::ImageId>) {
        self.image = image;
    }

    fn circle_color(&self) -> Color {
        if let Some(tint) = self.tint {
            return tint;
        }
        let mut hash: u32 = 2_166_136_261;
        for byte in self.name.as_bytes() {
            hash ^= *byte as u32;
            hash = hash.wrapping_mul(16_777_619);
        }
        AVATAR_TINTS[(hash as usize) % AVATAR_TINTS.len()]
    }
}

fn derive_initials(name: &str) -> String {
    let mut words = name.split_whitespace();
    let first = words.next().unwrap_or("");
    let second = words.next();
    let mut initials = String::new();
    match second {
        Some(second) => {
            for word in [first, second] {
                if let Some(ch) = word.chars().next() {
                    initials.extend(ch.to_uppercase());
                }
            }
        }
        None => {
            let ascii = first.chars().all(|ch| ch.is_ascii_alphanumeric());
            for ch in first.chars().take(if ascii { 2 } else { 1 }) {
                initials.extend(ch.to_uppercase());
            }
        }
    }
    initials
}

impl<M> Widget<M> for Avatar {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::LABEL
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        AVATAR_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            // An `ImageId` is a whole, non-negative number.
            ("image", value) => value.as_number().is_some_and(|image| {
                image >= 0.0 && image.fract() == 0.0 && image <= u64::MAX as f64
            }),
            _ => true,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("initials", SkinValue::Text(initials)) => self.initials = (*initials).to_string(),
            // A tint has no alpha; an RGBA value drops it silently.
            ("tint", value) => {
                if let Some(color) = value.as_color() {
                    self.tint = Some(color);
                }
            }
            ("ring", SkinValue::Bool(ring)) => self.ring = *ring,
            ("image", SkinValue::Number(image)) => {
                self.image = Some(crate::backend::ImageId(*image as u64));
            }
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
        40.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn hit_test(&self, pos: Point) -> bool {
        let center = self.rect.center();
        let rx = (self.rect.width() / 2.0).max(1.0);
        let ry = (self.rect.height() / 2.0).max(1.0);
        let dx = (pos.x - center.x) / rx;
        let dy = (pos.y - center.y) / ry;
        dx * dx + dy * dy <= 1.0
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        if let Some(id) = self.image
            && ctx.image_circle(self.rect, true, id)
        {
            if self.ring {
                let radius = self.rect.width().min(self.rect.height()) / 2.0;
                ctx.canvas.stroke_round_rect(
                    self.rect,
                    radius,
                    1.0,
                    ctx.palette.explicit_border,
                );
            }
            return;
        }
        let color = self.circle_color();
        let circle = if self.ring {
            ctx.canvas.fill_circle(self.rect, ctx.palette.explicit_border);
            self.rect.inset(1.0, 1.0)
        } else {
            self.rect
        };
        ctx.canvas.fill_circle(circle, color);
        let foreground = if color.luminance() < 0.62 {
            Color::new(250, 250, 250)
        } else {
            Color::new(26, 28, 32)
        };
        ctx.text(
            circle,
            &self.initials,
            TextStyleRole::Body,
            TextAlign::Center,
            foreground,
        );
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let name = if self.name.is_empty() {
            self.initials.clone()
        } else {
            self.name.clone()
        };
        Some(A11yInfo::new(A11yRole::Text, name).display_only())
    }

    fn set_label(&mut self, label: String) {
        self.initials = derive_initials(&label);
        self.name = label;
    }
}

/// A keyboard shortcut hint: text in a small bordered chip (`Ctrl+K`).
pub struct Kbd {
    id: WidgetId,
    rect: Rect,
    text: String,
}

impl Kbd {
    pub fn new(id: &'static str, text: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 56.0, 22.0),
            text: text.into(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replace the shortcut text silently (the skin's `shortcut` attribute).
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
}

impl<M> Widget<M> for Kbd {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::LABEL
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        KBD_SKIN_ATTRS
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        if let ("shortcut", SkinValue::Text(text)) = (name, value) {
            self.text = (*text).to_string();
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
        22.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        let painter = ctx.painter;
        painter.round_rect(&mut *ctx.canvas, self.rect, crate::theme::radius::SMALL, palette.explicit_border);
        painter.round_rect(
            &mut *ctx.canvas,
            self.rect.inset(1.0, 1.0),
            crate::theme::radius::SMALL - 1.0,
            palette.button_bg,
        );
        ctx.text(
            self.rect,
            &self.text,
            TextStyleRole::Small,
            TextAlign::Center,
            palette.muted,
        );
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(A11yInfo::new(A11yRole::Text, self.text.clone()).display_only())
    }

    fn set_label(&mut self, label: String) {
        self.text = label;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{ImageData, ImageId, TextAlign, TextStyle};
    use crate::geometry::Size;
    use crate::painter::Painter;
    use crate::widget::WidgetPaint;

    /// Records what a widget paints. A character is 10 units wide and a line
    /// 16 units tall, so wrapping is predictable without a real font.
    #[derive(Default)]
    struct Recorder {
        fills: Vec<(Rect, Color)>,
        ellipses: Vec<(Rect, Color)>,
        texts: Vec<(Rect, String, f32, TextAlign)>,
        overflows: Vec<TextOverflow>,
    }

    impl Canvas for Recorder {
        fn scale(&self) -> f32 {
            1.0
        }
        fn dpi(&self) -> f32 {
            96.0
        }
        fn fill_rect(&mut self, rect: Rect, color: Color) {
            self.fills.push((rect, color));
        }
        fn text(
            &mut self,
            rect: Rect,
            text: &str,
            style: &TextStyle,
            align: TextAlign,
            _color: Color,
        ) {
            self.texts
                .push((rect, text.to_string(), style.size, align));
        }
        fn text_overflow(
            &mut self,
            rect: Rect,
            text: &str,
            style: &TextStyle,
            align: TextAlign,
            color: Color,
            overflow: TextOverflow,
        ) {
            self.overflows.push(overflow);
            self.text(rect, text, style, align, color);
        }
        fn measure_text(&mut self, text: &str, _style: &TextStyle) -> Size {
            Size::new(text.chars().count() as f32 * 10.0, 16.0)
        }
        fn ellipse(&mut self, rect: Rect, color: Color) {
            self.ellipses.push((rect, color));
        }
        fn vertical_gradient(&mut self, _rect: Rect, _top: Color, _bottom: Color) {}
        fn image(&mut self, _rect: Rect, _crop: bool, _id: ImageId, _data: ImageData<'_>) {}
        fn push_clip(&mut self, _rect: Rect) {}
        fn pop_clip(&mut self) {}
    }

    fn paint(widget: &dyn Widget<()>, canvas: &mut Recorder) {
        let painter = Painter::new();
        let palette = *painter.palette();
        let images = crate::image::ImageStore::new();
        let mut paint = WidgetPaint {
            painter: &painter,
            canvas,
            palette,
            font: None,
            images: &images,
            viewport: Rect::from_xywh(0.0, 0.0, 400.0, 300.0),
        dirty: &[],
        };
        widget.paint(&mut paint, &WidgetState { enabled: true, ..Default::default() });
    }

    /// Position a non-generic widget; the `Widget<M>` impl is generic over the
    /// message type, so the test pins it to `()`.
    fn place(widget: &mut dyn Widget<()>, rect: Rect) {
        widget.set_rect(rect);
    }

    fn info(widget: &dyn Widget<()>) -> Option<A11yInfo> {
        widget.a11y()
    }

    fn hit(widget: &dyn Widget<()>, pos: Point) -> bool {
        widget.hit_test(pos)
    }

    fn interactive(widget: &dyn Widget<()>) -> bool {
        widget.interactive()
    }

    fn cursor(widget: &dyn Widget<()>) -> Option<Cursor> {
        widget.cursor()
    }

    fn rename(widget: &mut dyn Widget<()>, text: &str) {
        widget.set_label(text.to_string());
    }

    #[test]
    fn label_wraps_and_draws_one_text_call_per_line() {
        let mut label = Label::new("l", "abcdefghij klmnopqrst").wrap(true);
        place(&mut label, Rect::from_xywh(0.0, 0.0, 100.0, 60.0));
        let mut canvas = Recorder::default();
        paint(&label, &mut canvas);
        let lines: Vec<&str> = canvas.texts.iter().map(|entry| entry.1.as_str()).collect();
        assert_eq!(lines, vec!["abcdefghij", "klmnopqrst"]);
        // The second line starts one line height below the first.
        assert!((canvas.texts[1].0.top - 16.0).abs() < 0.01);
    }

    #[test]
    fn a_label_chip_paints_its_background_with_the_configured_corners() {
        use crate::geometry::Corners;
        let chip = Color::new(47, 111, 235);
        let rect = Rect::from_xywh(0.0, 0.0, 100.0, 26.0);

        // A plain label has no fill: the page shows through.
        let mut plain = Label::new("plain", "文本");
        place(&mut plain, rect);
        let mut canvas = Recorder::default();
        paint(&plain, &mut canvas);
        assert!(canvas.fills.is_empty());

        // A chip paints its background; square is one rectangle.
        let mut square = Label::new("square", "芯片").background(chip);
        assert_eq!(square.surface().background, Some(chip));
        place(&mut square, rect);
        let mut canvas = Recorder::default();
        paint(&square, &mut canvas);
        assert_eq!(canvas.fills.len(), 1);
        assert_eq!(canvas.fills[0], (rect, chip));
        assert!(canvas.ellipses.is_empty());

        // A radius composes the fill from bands and four corner arcs.
        let mut rounded = Label::new("rounded", "芯片")
            .background(chip)
            .corner_radius(13.0);
        assert_eq!(rounded.surface().corners, Corners::uniform(13.0));
        place(&mut rounded, rect);
        let mut canvas = Recorder::default();
        paint(&rounded, &mut canvas);
        assert_eq!(canvas.fills.len(), 3, "bands around the corner arcs");
        assert_eq!(canvas.ellipses.len(), 4, "one arc per corner");
        assert!(canvas.fills.iter().all(|(_, color)| *color == chip));
        assert!(canvas.ellipses.iter().all(|(_, color)| *color == chip));
    }

    #[test]
    fn label_stops_at_the_rectangle_height() {
        let mut label = Label::new("l", "aa bb cc dd ee ff").wrap(true);
        place(&mut label, Rect::from_xywh(0.0, 0.0, 50.0, 20.0));
        let mut canvas = Recorder::default();
        paint(&label, &mut canvas);
        // Only one 16-unit line fits in a 20-unit rectangle.
        assert_eq!(canvas.texts.len(), 1);
        assert_eq!(canvas.texts[0].1, "aa");
    }

    #[test]
    fn label_single_line_chooses_ellipsis_or_clip() {
        let mut label = Label::new("l", "a very long single line");
        place(&mut label, Rect::from_xywh(0.0, 0.0, 60.0, 20.0));
        let mut canvas = Recorder::default();
        paint(&label, &mut canvas);
        assert_eq!(canvas.overflows, vec![TextOverflow::Ellipsis]);

        let mut clipped = Label::new("c", "a very long single line").ellipsis(false);
        place(&mut clipped, Rect::from_xywh(0.0, 0.0, 60.0, 20.0));
        let mut canvas = Recorder::default();
        paint(&clipped, &mut canvas);
        assert_eq!(canvas.overflows, vec![TextOverflow::Clip]);
    }

    #[test]
    fn label_repaints_new_text_after_set_label() {
        let mut label = Label::new("l", "first text").wrap(true);
        place(&mut label, Rect::from_xywh(0.0, 0.0, 140.0, 40.0));
        let mut canvas = Recorder::default();
        paint(&label, &mut canvas);
        assert_eq!(canvas.texts[0].1, "first text");
        rename(&mut label, "second text");
        let mut canvas = Recorder::default();
        paint(&label, &mut canvas);
        assert_eq!(canvas.texts[0].1, "second text");
    }

    #[test]
    fn link_labels_take_pointer_input_and_the_hand_cursor() {
        let plain = Label::new("plain", "text");
        assert!(!interactive(&plain));
        assert_eq!(cursor(&plain), None);
        let link = Label::new("link", "打开").link(true);
        assert!(interactive(&link));
        assert_eq!(cursor(&link), Some(Cursor::Hand));
    }

    #[test]
    fn badge_close_target_emits_only_inside_its_zone() {
        let mut badge: Badge<u32> =
            Badge::new("tag", "无损").tone(BadgeTone::Success).removable(|| 7);
        badge.set_rect(Rect::from_xywh(0.0, 0.0, 80.0, 22.0));
        assert!(badge.is_removable());
        assert!(badge.interactive());
        // Left side: no message. The close zone is the last 22 DIPs.
        assert_eq!(badge.input(WidgetInput::ClickAt(Point::new(20.0, 11.0))), None);
        assert_eq!(
            badge.input(WidgetInput::ClickAt(Point::new(70.0, 11.0))),
            Some(7)
        );
        let key = WidgetInput::Key(crate::event::KeyEvent {
            code: KeyCode::Delete,
            vk: 0,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        });
        assert_eq!(badge.input(key), Some(7));

        // Hover feedback belongs to the close target, not the whole chip.
        assert!(badge.repaint_on_move());
        badge.input(WidgetInput::HoverAt(Point::new(20.0, 11.0)));
        assert!(!badge.hover_close.get());
        badge.input(WidgetInput::HoverAt(Point::new(70.0, 11.0)));
        assert!(badge.hover_close.get());
        badge.input(WidgetInput::HoverLeave);
        assert!(!badge.hover_close.get());

        let mut plain: Badge<u32> = Badge::new("tag2", "FLAC");
        assert!(!plain.is_removable());
        assert!(!plain.interactive());
        assert!(!plain.repaint_on_move());
        assert_eq!(plain.input(WidgetInput::ClickAt(Point::new(70.0, 11.0))), None);
    }

    #[test]
    fn divider_is_decoration_unless_it_carries_a_label() {
        let mut divider = Divider::new("d");
        place(&mut divider, Rect::from_xywh(0.0, 0.0, 200.0, 16.0));
        assert!(info(&divider).is_none());
        let mut canvas = Recorder::default();
        paint(&divider, &mut canvas);
        // One 1 DIP line, no text.
        assert_eq!(canvas.fills.len(), 1);
        assert!((canvas.fills[0].0.height() - 1.0).abs() < 0.01);
        assert!(canvas.texts.is_empty());

        let mut labeled = Divider::new("d2").label("组件");
        place(&mut labeled, Rect::from_xywh(0.0, 0.0, 200.0, 16.0));
        let info = info(&labeled).expect("a labeled divider is a text node");
        assert_eq!(info.name, "组件");
        assert!(info.display_only);
        let mut canvas = Recorder::default();
        paint(&labeled, &mut canvas);
        // Two line segments around the centered label.
        assert_eq!(canvas.fills.len(), 2);
        assert_eq!(canvas.texts[0].1, "组件");
    }

    #[test]
    fn vertical_divider_draws_a_one_pixel_column() {
        let mut divider = Divider::new("d").vertical(true).label("ignored");
        place(&mut divider, Rect::from_xywh(0.0, 0.0, 20.0, 100.0));
        let mut canvas = Recorder::default();
        paint(&divider, &mut canvas);
        assert_eq!(canvas.fills.len(), 1);
        let (line, _) = canvas.fills[0];
        assert!((line.width() - 1.0).abs() < 0.01);
        assert!((line.height() - 100.0).abs() < 0.01);
        assert!(canvas.texts.is_empty());
    }

    #[test]
    fn avatar_initials_cover_ascii_and_cjk_names() {
        assert_eq!(Avatar::new("a", "Annie Lennox").initials_text(), "AL");
        assert_eq!(Avatar::new("a", "bamboo").initials_text(), "BA");
        assert_eq!(Avatar::new("a", "竹韵").initials_text(), "竹");
        assert_eq!(Avatar::new("a", "  ").initials_text(), "");
        let explicit = Avatar::new("a", "Annie Lennox").initials("Z");
        assert_eq!(explicit.initials_text(), "Z");
    }

    #[test]
    fn avatar_set_label_rederives_the_initials() {
        let mut avatar = Avatar::new("a", "bamboo");
        rename(&mut avatar, "Annie Lennox");
        assert_eq!(avatar.initials_text(), "AL");
        assert_eq!(avatar.name(), "Annie Lennox");
    }

    #[test]
    fn avatar_hit_test_is_a_circle() {
        let mut avatar = Avatar::new("a", "竹韵");
        place(&mut avatar, Rect::from_xywh(0.0, 0.0, 40.0, 40.0));
        assert!(hit(&avatar, Point::new(20.0, 20.0)));
        // The corner of the cell is outside the circle.
        assert!(!hit(&avatar, Point::new(1.0, 1.0)));
    }

    #[test]
    fn avatar_uses_its_tint_or_a_stable_name_color() {
        let a = Avatar::new("a", "bamboo");
        let b = Avatar::new("b", "bamboo");
        assert_eq!(a.circle_color(), b.circle_color());
        let tint = Color::new(1, 2, 3);
        assert_eq!(Avatar::new("a", "bamboo").tint(tint).circle_color(), tint);
    }

    #[test]
    fn icon_paints_the_glyph_at_the_requested_size() {
        let mut icon = Icon::new("i", '\u{E768}').size(18.0).tint(Color::new(1, 2, 3));
        place(&mut icon, Rect::from_xywh(0.0, 0.0, 24.0, 24.0));
        let mut canvas = Recorder::default();
        paint(&icon, &mut canvas);
        assert_eq!(canvas.texts.len(), 1);
        assert_eq!(canvas.texts[0].1, "\u{E768}");
        assert!((canvas.texts[0].2 - 18.0).abs() < 0.01);
        assert_eq!(canvas.texts[0].3, TextAlign::Center);
    }

    #[test]
    fn kbd_paints_a_bordered_chip_with_its_label() {
        let mut kbd = Kbd::new("k", "Ctrl+K");
        place(&mut kbd, Rect::from_xywh(0.0, 0.0, 60.0, 22.0));
        let mut canvas = Recorder::default();
        paint(&kbd, &mut canvas);
        assert!(canvas.fills.len() >= 3, "a chip needs fill and border rects");
        assert!(!canvas.ellipses.is_empty(), "the chip corners are round");
        assert_eq!(canvas.texts[0].1, "Ctrl+K");
        let info = info(&kbd).expect("a shortcut hint is text");
        assert_eq!(info.name, "Ctrl+K");
        assert!(info.display_only);
    }
}
