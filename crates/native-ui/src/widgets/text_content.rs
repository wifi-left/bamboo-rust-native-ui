// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Text widgets that need more than one painted line: copyable text, a
//! scrolling marquee and rich text with inline links.
//!
//! All three are display-only by default. `CopyableText` and `RichText` become
//! pointer-interactive only when the application attaches a handler (a copy
//! message or a link callback); the marquee never takes input.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use crate::a11y::{A11yInfo, A11yRole};
use crate::backend::{FontWeight, TextAlign, TextStyle};
use crate::color::Color;
use crate::event::{Cursor, KeyCode};
use crate::geometry::{Point, Rect};
use crate::skin::{SkinAttrDecl, SkinElement, SkinValue, SkinValueKind};
use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

use super::content::{label_style_from_name, LabelStyle};

static MARQUEE_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("offset", SkinValueKind::Number),
    SkinAttrDecl::new("color", SkinValueKind::Color),
    SkinAttrDecl::new("text-style", SkinValueKind::Text),
];

static COPYABLE_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("color", SkinValueKind::Color),
    SkinAttrDecl::new("text-style", SkinValueKind::Text),
];

/// Read-only text with a copy affordance. `.copyable(|| Msg)` paints a copy
/// glyph and emits the message on a click, Enter/Space or Ctrl+C while
/// focused; the application writes the clipboard (for example with
/// `WindowHandle::copy_text`), so the widget never touches the platform.
pub struct CopyableText<M> {
    id: WidgetId,
    rect: Rect,
    text: String,
    glyph: char,
    style: LabelStyle,
    color: Option<Color>,
    on_copy: Option<Box<dyn Fn() -> M>>,
    hover: Cell<bool>,
}

impl<M> CopyableText<M> {
    pub fn new(id: &'static str, text: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 180.0, 22.0),
            text: text.into(),
            glyph: '\u{E8C8}',
            style: LabelStyle::Small,
            color: None,
            on_copy: None,
            hover: Cell::new(false),
        }
    }

    pub fn copyable(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.on_copy = Some(Box::new(handler));
        self
    }

    pub fn style(mut self, style: LabelStyle) -> Self {
        self.style = style;
        self
    }

    /// Replace the explicit text colour silently (the skin's `color`); `None`
    /// uses the palette's text colour.
    pub fn set_color(&mut self, color: Option<Color>) {
        self.color = color;
    }

    pub fn color_value(&self) -> Option<Color> {
        self.color
    }

    /// Replace the text role silently (the skin's `text-style`).
    pub fn set_text_style(&mut self, style: LabelStyle) {
        self.style = style;
    }

    pub fn text_style(&self) -> LabelStyle {
        self.style
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    fn glyph_rect(&self) -> Rect {
        let width = self.rect.height().min(18.0);
        Rect::new(
            self.rect.right - width,
            self.rect.top,
            self.rect.right,
            self.rect.bottom,
        )
    }
}

impl<M: 'static> Widget<M> for CopyableText<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::TEXT
            .union(crate::props::ContentCaps::LABEL)
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        COPYABLE_SKIN_ATTRS
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
        22.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn interactive(&self) -> bool {
        self.on_copy.is_some()
    }

    fn cursor(&self) -> Option<Cursor> {
        self.on_copy.as_ref().map(|_| Cursor::Hand)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let glyph = self.glyph_rect();
        let text = Rect::new(
            self.rect.left,
            self.rect.top,
            glyph.left - 4.0,
            self.rect.bottom,
        );
        let color = self.color.unwrap_or(ctx.palette.text);
        let role = match self.style {
            LabelStyle::Body => TextStyleRole::Body,
            LabelStyle::Small => TextStyleRole::Small,
            LabelStyle::Title => TextStyleRole::Title,
        };
        ctx.text(text, &self.text, role, TextAlign::Left, color);
        if self.on_copy.is_some() {
            let mut buf = [0u8; 4];
            let glyph_text = self.glyph.encode_utf8(&mut buf);
            let tint = if self.hover.get() {
                ctx.palette.accent
            } else {
                ctx.palette.muted
            };
            ctx.text(glyph, glyph_text, TextStyleRole::Small, TextAlign::Center, tint);
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if self.on_copy.is_none() {
            return None;
        }
        match input {
            WidgetInput::HoverAt(_) => {
                self.hover.set(true);
                None
            }
            WidgetInput::HoverLeave => {
                self.hover.set(false);
                None
            }
            WidgetInput::ClickAt(_) | WidgetInput::Activate => {
                self.on_copy.as_ref().map(|handler| handler())
            }
            WidgetInput::Key(key)
                if key.ctrl && matches!(key.code, KeyCode::Letter(2)) =>
            {
                self.on_copy.as_ref().map(|handler| handler())
            }
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(A11yInfo::new(A11yRole::Text, self.text.clone()).display_only())
    }

    fn set_label(&mut self, label: String) {
        self.text = label;
    }

    fn set_text(&mut self, text: String) {
        self.text = text;
    }
}

/// A single-line title that scrolls when it does not fit. The widget owns no
/// timer: the application advances it from its animation tick with
/// `Ui::edit_widget::<MarqueeText>(id, |m| m.advance(dt))` and paints
/// only when the offset changed.
pub struct MarqueeText {
    id: WidgetId,
    rect: Rect,
    text: String,
    style: LabelStyle,
    color: Option<Color>,
    /// Scroll speed in DIPs per second.
    speed: f32,
    /// Blank space between the end and the repeated start, in DIPs.
    gap: f32,
    /// Hold at each end, in seconds.
    pause: f32,
    elapsed: f32,
    offset: f32,
    /// Measured text width from the last paint; 0 until painted.
    span: Cell<f32>,
}

impl MarqueeText {
    pub fn new(id: &'static str, text: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 200.0, 22.0),
            text: text.into(),
            style: LabelStyle::Body,
            color: None,
            speed: 40.0,
            gap: 48.0,
            pause: 1.5,
            elapsed: 0.0,
            offset: 0.0,
            span: Cell::new(0.0),
        }
    }

    pub fn speed(mut self, dips_per_second: f32) -> Self {
        self.speed = dips_per_second.max(1.0);
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    /// Hold at the start and the end of each cycle.
    pub fn pause(mut self, seconds: f32) -> Self {
        self.pause = seconds.max(0.0);
        self
    }

    pub fn style(mut self, style: LabelStyle) -> Self {
        self.style = style;
        self
    }

    /// Replace the explicit text colour silently (the skin's `color`); `None`
    /// uses the palette's text colour.
    pub fn set_color(&mut self, color: Option<Color>) {
        self.color = color;
    }

    pub fn color_value(&self) -> Option<Color> {
        self.color
    }

    /// Replace the text role silently (the skin's `text-style`).
    pub fn set_text_style(&mut self, style: LabelStyle) {
        self.style = style;
    }

    pub fn text_style(&self) -> LabelStyle {
        self.style
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// Jump to a painted offset in DIPs (the skin's `offset` attribute). The
    /// application's next `advance` continues from the elapsed time it keeps,
    /// so this is an initial value, not a seek.
    pub fn set_offset(&mut self, offset: f32) {
        self.offset = offset.max(0.0);
    }

    pub fn is_scrolling(&self) -> bool {
        self.span.get() > self.rect.width() + 0.01
    }

    fn replace_text(&mut self, text: String) {
        if text != self.text {
            self.text = text;
            self.elapsed = 0.0;
            self.offset = 0.0;
            self.span.set(0.0);
        }
    }

    /// Advance the animation by `dt`. Returns true when the painted offset
    /// changed, i.e. when the caller should invalidate the widget.
    pub fn advance(&mut self, dt: Duration) -> bool {
        let span = self.span.get();
        if span <= self.rect.width() + 0.01 {
            if self.offset == 0.0 {
                return false;
            }
            self.offset = 0.0;
            return true;
        }
        let travel = span + self.gap;
        let scrolling = travel / self.speed;
        let cycle = scrolling + 2.0 * self.pause;
        self.elapsed = (self.elapsed + dt.as_secs_f32()) % cycle;
        let offset = if self.elapsed < self.pause {
            0.0
        } else if self.elapsed < self.pause + scrolling {
            (self.elapsed - self.pause) * self.speed
        } else {
            travel
        };
        let changed = (offset - self.offset).abs() > 0.01;
        self.offset = offset;
        changed
    }

    fn resolved_style(&self, ctx: &WidgetPaint<'_>) -> TextStyle {
        self.style.resolved(ctx)
    }
}

impl<M> Widget<M> for MarqueeText {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::TEXT
            .union(crate::props::ContentCaps::LABEL)
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        MARQUEE_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("offset", SkinValue::Number(value)) => *value >= 0.0,
            ("color", SkinValue::Color(_)) => true,
            ("text-style", SkinValue::Text(style)) => label_style_from_name(style).is_some(),
            _ => false,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("offset", SkinValue::Number(offset)) => self.set_offset(*offset as f32),
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
        22.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let style = self.resolved_style(ctx);
        let span = ctx.canvas.measure_text(&self.text, &style).width;
        self.span.set(span);
        let color = self.color.unwrap_or(ctx.palette.text);
        if span <= self.rect.width() + 0.01 {
            let text = Rect::new(
                self.rect.left,
                self.rect.top,
                self.rect.right,
                self.rect.bottom,
            );
            ctx.canvas.text(text, &self.text, &style, TextAlign::Left, color);
            return;
        }
        ctx.canvas.push_clip(self.rect);
        let span_rect = span.max(1.0);
        let mut x = self.rect.left - self.offset;
        for _ in 0..2 {
            if x + span_rect > self.rect.left && x < self.rect.right {
                ctx.canvas.text(
                    Rect::new(x, self.rect.top, x + span_rect, self.rect.bottom),
                    &self.text,
                    &style,
                    TextAlign::Left,
                    color,
                );
            }
            x += span_rect + self.gap;
        }
        ctx.canvas.pop_clip();
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(A11yInfo::new(A11yRole::Text, self.text.clone()).display_only())
    }

    fn set_label(&mut self, label: String) {
        self.replace_text(label);
    }

    fn set_text(&mut self, text: String) {
        self.replace_text(text);
    }
}

/// Inline emphasis of a [`RichRun`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RunStyle {
    #[default]
    Normal,
    Strong,
    Muted,
    /// Painted in the accent color with an underline; clicking it runs the
    /// widget's link handler with [`RichRun::link`].
    Link,
}

/// One inline run of a [`RichText`].
#[derive(Clone, Debug)]
pub struct RichRun {
    pub text: String,
    pub style: RunStyle,
    /// Opaque id passed to the link handler; ignored for other styles.
    pub link: u32,
}

impl RichRun {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: RunStyle::Normal,
            link: 0,
        }
    }

    pub fn style(mut self, style: RunStyle) -> Self {
        self.style = style;
        self
    }
}

#[derive(Clone, Copy)]
struct Glyph {
    ch: char,
    run: usize,
    start: usize,
    end: usize,
}

#[derive(Clone)]
struct RichSeg {
    run: usize,
    start: usize,
    end: usize,
    x: f32,
    width: f32,
}

#[derive(Clone)]
struct RichLine {
    segs: Vec<RichSeg>,
    width: f32,
}

#[derive(Default)]
struct RichCache {
    width: f32,
    font_size: f32,
    revision: u64,
    lines: Vec<RichLine>,
    /// Link hit rectangles, rebuilt on every paint because they follow the
    /// alignment offset.
    links: Vec<(u32, Rect)>,
}

static RICH_SKIN_CHILDREN: &[&str] = &["run"];
static RICH_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("wrap", SkinValueKind::Bool),
    SkinAttrDecl::new("align", SkinValueKind::Text),
    SkinAttrDecl::new("text-style", SkinValueKind::Text),
    SkinAttrDecl::new("color", SkinValueKind::Color),
];
const RICH_RUN_ATTRS: &[&str] = &["text", "style", "link"];

fn rich_run_style(name: &str) -> Option<RunStyle> {
    match name {
        "normal" => Some(RunStyle::Normal),
        "strong" => Some(RunStyle::Strong),
        "muted" => Some(RunStyle::Muted),
        "link" => Some(RunStyle::Link),
        _ => None,
    }
}

/// One `<run text style link>` child: `text` is required, `style` names one of
/// the four emphasis levels, and `link` is an opaque `u32` id that is only
/// legal on a link run.
fn run_from_skin(child: &SkinElement) -> Option<RichRun> {
    let mut run = RichRun::new(child.attr("text")?);
    run.style = child
        .attr("style")
        .and_then(rich_run_style)
        .unwrap_or_default();
    run.link = child
        .attr("link")
        .and_then(|link| link.trim().parse().ok())
        .unwrap_or(0);
    Some(run)
}

fn valid_skin_run(child: &SkinElement) -> bool {
    let style = child.attr("style");
    child.name == "run"
        && child
            .attr("text")
            .is_some_and(|text| !text.trim().is_empty())
        && child.attrs_are(RICH_RUN_ATTRS)
        && style.is_none_or(|style| rich_run_style(style).is_some())
        && child
            .attr("link")
            .is_none_or(|link| style == Some("link") && link.trim().parse::<u32>().is_ok())
        && child.children.is_empty()
}

/// Text with inline styles and links. Runs are laid out together (word wrap
/// crosses run boundaries), links are underlined and clickable, and the whole
/// string is the accessible name.
pub struct RichText<M> {
    id: WidgetId,
    rect: Rect,
    runs: Vec<RichRun>,
    wrap: bool,
    align: TextAlign,
    style: LabelStyle,
    color: Option<Color>,
    on_link: Option<Box<dyn Fn(u32) -> M>>,
    cache: RefCell<RichCache>,
    revision: u64,
}

impl<M> RichText<M> {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 40.0),
            runs: Vec::new(),
            wrap: true,
            align: TextAlign::Left,
            style: LabelStyle::Body,
            color: None,
            on_link: None,
            cache: RefCell::new(RichCache::default()),
            revision: 0,
        }
    }

    pub fn run(mut self, text: impl Into<String>) -> Self {
        self.runs.push(RichRun::new(text));
        self
    }

    pub fn strong(mut self, text: impl Into<String>) -> Self {
        self.runs.push(RichRun::new(text).style(RunStyle::Strong));
        self
    }

    pub fn muted(mut self, text: impl Into<String>) -> Self {
        self.runs.push(RichRun::new(text).style(RunStyle::Muted));
        self
    }

    /// A link run. `id` is passed to the handler installed with
    /// [`RichText::on_link`].
    pub fn link(mut self, id: u32, text: impl Into<String>) -> Self {
        self.runs
            .push(RichRun::new(text).style(RunStyle::Link).link_id(id));
        self
    }

    pub fn runs(mut self, runs: Vec<RichRun>) -> Self {
        self.runs = runs;
        self.revision += 1;
        self
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
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

    // --- runtime appearance (skin locals) ---------------------------------

    fn refresh_style(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.cache.borrow_mut().revision = self.revision;
    }

    /// Wrap long lines at word boundaries. Returns whether it changed.
    pub fn set_wrap(&mut self, wrap: bool) -> bool {
        if self.wrap == wrap {
            return false;
        }
        self.wrap = wrap;
        self.refresh_style();
        true
    }

    pub fn wrap_enabled(&self) -> bool {
        self.wrap
    }

    /// Horizontal alignment of the laid-out lines. Returns whether it changed.
    pub fn set_align(&mut self, align: TextAlign) -> bool {
        if self.align == align {
            return false;
        }
        self.align = align;
        true
    }

    pub fn alignment(&self) -> TextAlign {
        self.align
    }

    /// Theme text role (body/small/title). Returns whether it changed.
    pub fn set_text_style(&mut self, style: LabelStyle) -> bool {
        if self.style == style {
            return false;
        }
        self.style = style;
        self.refresh_style();
        true
    }

    pub fn text_style(&self) -> LabelStyle {
        self.style
    }

    /// Replace the explicit colour of the normal/strong runs silently (the
    /// skin's `color`); muted and link runs keep their palette roles and `None`
    /// restores the palette's text colour.
    pub fn set_color(&mut self, color: Option<Color>) {
        self.color = color;
    }

    pub fn color_value(&self) -> Option<Color> {
        self.color
    }

    pub fn on_link(mut self, handler: impl Fn(u32) -> M + 'static) -> Self {
        self.on_link = Some(Box::new(handler));
        self
    }

    /// Plain concatenation of the runs (accessible name, copy support).
    pub fn plain_text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }

    pub fn set_runs(&mut self, runs: Vec<RichRun>) {
        self.runs = runs;
        self.revision += 1;
        self.cache.borrow_mut().revision = self.revision;
    }

    /// The current runs (typed access).
    pub fn runs_ref(&self) -> &[RichRun] {
        &self.runs
    }

    fn resolved_style(&self, ctx: &WidgetPaint<'_>) -> TextStyle {
        self.style.resolved(ctx)
    }

    fn run_style(&self, ctx: &WidgetPaint<'_>, run: usize) -> (TextStyle, Color) {
        let base = self.resolved_style(ctx);
        let style = &self.runs[run].style;
        let mut style = match style {
            RunStyle::Strong => base.with_weight(FontWeight::Semibold),
            _ => base,
        };
        style.italic = false;
        let color = match self.runs[run].style {
            RunStyle::Normal | RunStyle::Strong => {
                self.color.unwrap_or(ctx.palette.text)
            }
            RunStyle::Muted => ctx.palette.muted,
            RunStyle::Link => ctx.palette.accent_text,
        };
        (style, color)
    }
}

impl<M: 'static> Widget<M> for RichText<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::LABEL
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        RICH_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("wrap", SkinValue::Bool(_)) => true,
            ("align", SkinValue::Text(align)) => matches!(*align, "left" | "center" | "right"),
            ("text-style", SkinValue::Text(style)) => label_style_from_name(style).is_some(),
            ("color", SkinValue::Color(_)) => true,
            _ => false,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("wrap", SkinValue::Bool(wrap)) => {
                self.set_wrap(*wrap);
            }
            ("align", SkinValue::Text(align)) => {
                self.set_align(match *align {
                    "center" => TextAlign::Center,
                    "right" => TextAlign::Right,
                    _ => TextAlign::Left,
                });
            }
            ("text-style", SkinValue::Text(style)) => {
                if let Some(style) = label_style_from_name(style) {
                    self.set_text_style(style);
                }
            }
            ("color", SkinValue::Color(_)) => {
                if let Some(color) = value.as_color() {
                    self.set_color(Some(color));
                }
            }
            _ => {}
        }
    }

    fn skin_children(&self) -> &'static [&'static str] {
        RICH_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(valid_skin_run)
    }

    /// `<run>` elements replace the inline runs. A shared `label` on the same
    /// entry still applies afterwards (content is written after children) and
    /// replaces them with one plain run.
    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        let runs = children.iter().filter_map(run_from_skin).collect();
        self.set_runs(runs);
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

    fn interactive(&self) -> bool {
        self.on_link.is_some()
    }

    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        let cache = self.cache.borrow();
        let over = cache
            .links
            .iter()
            .any(|(_, rect)| rect.contains_point(pos));
        (self.on_link.is_some() && over).then_some(Cursor::Hand)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let (measure_style, _) = self.run_style(ctx, 0);
        let mut cache = self.cache.borrow_mut();
        if cache.width != self.rect.width()
            || cache.font_size != measure_style.size
            || cache.revision != self.revision
        {
            cache.lines = build_rich_lines(
                &self.runs,
                self.rect.width(),
                self.wrap,
                &mut |text| ctx.canvas.measure_text(text, &measure_style).width,
            );
            cache.width = self.rect.width();
            cache.font_size = measure_style.size;
            cache.revision = self.revision;
        }
        let line_height = ctx
            .canvas
            .measure_text("Ag", &measure_style)
            .height
            .max(measure_style.size);
        let mut links = Vec::new();
        for (line_index, line) in cache.lines.iter().enumerate() {
            let top = self.rect.top + line_index as f32 * line_height;
            if top + line_height > self.rect.bottom {
                break;
            }
            let start_x = match self.align {
                TextAlign::Left => self.rect.left,
                TextAlign::Center => {
                    self.rect.left + (self.rect.width() - line.width).max(0.0) / 2.0
                }
                TextAlign::Right => self.rect.right - line.width,
            };
            for seg in &line.segs {
                let (style, color) = self.run_style(ctx, seg.run);
                let rect = Rect::new(
                    start_x + seg.x,
                    top,
                    start_x + seg.x + seg.width.max(1.0),
                    top + line_height,
                );
                let text = &self.runs[seg.run].text[seg.start..seg.end];
                ctx.canvas.text(rect, text, &style, TextAlign::Left, color);
                if self.runs[seg.run].style == RunStyle::Link {
                    ctx.canvas.fill_rect(
                        Rect::new(rect.left, rect.bottom - 3.0, rect.right, rect.bottom - 2.0),
                        color,
                    );
                    links.push((self.runs[seg.run].link, rect));
                }
            }
        }
        cache.links = links;
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        let pos = match input {
            WidgetInput::ClickAt(pos) => pos,
            _ => return None,
        };
        let handler = self.on_link.as_ref()?;
        let cache = self.cache.borrow();
        let link = cache
            .links
            .iter()
            .find(|(_, rect)| rect.contains_point(pos))
            .map(|(link, _)| *link)?;
        Some(handler(link))
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(A11yInfo::new(A11yRole::Text, self.plain_text()).display_only())
    }

    fn set_label(&mut self, label: String) {
        self.runs = vec![RichRun::new(label)];
        self.revision += 1;
        self.cache.borrow_mut().revision = self.revision;
    }
}

impl RichRun {
    fn link_id(mut self, id: u32) -> Self {
        self.link = id;
        self
    }
}

/// Greedy wrap over the glyph stream. Returns one entry per painted line; each
/// line carries segments with run-relative byte ranges and DIP offsets.
fn build_rich_lines(
    runs: &[RichRun],
    max_width: f32,
    wrap: bool,
    measure: &mut dyn FnMut(&str) -> f32,
) -> Vec<RichLine> {
    let mut glyphs: Vec<Glyph> = Vec::new();
    for (index, run) in runs.iter().enumerate() {
        for (offset, ch) in run.text.char_indices() {
            glyphs.push(Glyph {
                ch,
                run: index,
                start: offset,
                end: offset + ch.len_utf8(),
            });
        }
    }
    if glyphs.is_empty() {
        return vec![RichLine {
            segs: Vec::new(),
            width: 0.0,
        }];
    }
    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut skip_leading = false;
    while start < glyphs.len() {
        let mut width = 0.0f32;
        let mut last_space: Option<usize> = None;
        let mut end = start;
        let mut next = start;
        let mut hard = false;
        while next < glyphs.len() {
            let glyph = glyphs[next];
            if glyph.ch == '\n' {
                end = next;
                next += 1;
                hard = true;
                break;
            }
            let char_width = measure(&runs[glyph.run].text[glyph.start..glyph.end]);
            if wrap && width + char_width > max_width && next > start {
                let cut = last_space.unwrap_or(next);
                end = cut;
                next = cut;
                break;
            }
            width += char_width;
            end = next + 1;
            next += 1;
            if glyphs[next - 1].ch.is_whitespace() {
                last_space = Some(next);
            }
        }
        // Trim trailing spaces and, after a wrap (not a hard newline), skip the
        // spaces the previous line broke at.
        let mut trimmed_end = end;
        while trimmed_end > start && glyphs[trimmed_end - 1].ch.is_whitespace() {
            trimmed_end -= 1;
        }
        let mut trimmed_start = start;
        if skip_leading {
            while trimmed_start < trimmed_end && glyphs[trimmed_start].ch == ' ' {
                trimmed_start += 1;
            }
        }
        let line = segments(runs, &glyphs, trimmed_start, trimmed_end, measure);
        lines.push(line);
        skip_leading = !hard && next < glyphs.len();
        if hard {
            start = next;
        } else if next <= start {
            // No progress (a single character wider than the line): consume it.
            start = trimmed_end.max(start + 1);
        } else {
            start = next;
        }
        if !wrap && !hard {
            break;
        }
    }
    lines
}

fn segments(
    runs: &[RichRun],
    glyphs: &[Glyph],
    start: usize,
    end: usize,
    measure: &mut dyn FnMut(&str) -> f32,
) -> RichLine {
    let mut segs: Vec<RichSeg> = Vec::new();
    let mut x = 0.0f32;
    let mut index = start;
    while index < end {
        let run = glyphs[index].run;
        let mut last = index;
        while last + 1 < end && glyphs[last + 1].run == run {
            last += 1;
        }
        let byte_start = glyphs[index].start;
        let byte_end = glyphs[last].end;
        let text = &runs[run].text[byte_start..byte_end];
        let width = measure(text);
        segs.push(RichSeg {
            run,
            start: byte_start,
            end: byte_end,
            x,
            width,
        });
        x += width;
        index = last + 1;
    }
    RichLine { segs, width: x }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Canvas;
    use crate::image::ImageStore;
    use crate::painter::Painter;
    use crate::widget::WidgetPaint;

    #[derive(Default)]
    struct Recorder {
        texts: Vec<(Rect, String, f32)>,
        clips: usize,
    }

    impl Canvas for Recorder {
        fn scale(&self) -> f32 {
            1.0
        }
        fn dpi(&self) -> f32 {
            96.0
        }
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
        fn text(
            &mut self,
            rect: Rect,
            text: &str,
            style: &TextStyle,
            _align: TextAlign,
            _color: Color,
        ) {
            self.texts.push((rect, text.to_string(), style.size));
        }
        fn measure_text(&mut self, text: &str, _style: &TextStyle) -> crate::geometry::Size {
            crate::geometry::Size::new(text.chars().count() as f32 * 10.0, 16.0)
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
        fn push_clip(&mut self, _rect: Rect) {
            self.clips += 1;
        }
        fn pop_clip(&mut self) {
            self.clips = self.clips.saturating_sub(1);
        }
    }

    fn paint<M: 'static>(widget: &dyn Widget<M>, canvas: &mut Recorder) {
        let painter = Painter::new();
        let images = ImageStore::new();
        let mut paint = WidgetPaint {
            painter: &painter,
            canvas,
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

    fn place<M>(widget: &mut dyn Widget<M>, rect: Rect) {
        widget.set_rect(rect);
    }

    #[test]
    fn copyable_text_emits_on_click_and_ctrl_c_only_with_a_handler() {
        let mut plain: CopyableText<u32> = CopyableText::new("c", "text");
        assert_eq!(plain.input(WidgetInput::ClickAt(Point::new(1.0, 1.0))), None);

        let mut copyable: CopyableText<u32> = CopyableText::new("c", "text").copyable(|| 9);
        assert_eq!(
            copyable.input(WidgetInput::ClickAt(Point::new(1.0, 1.0))),
            Some(9)
        );
        assert_eq!(copyable.input(WidgetInput::Activate), Some(9));
        let ctrl_c = WidgetInput::Key(crate::event::KeyEvent {
            code: KeyCode::Letter(2),
            vk: 0x43,
            ctrl: true,
            shift: false,
            alt: false,
            repeat: false,
        });
        assert_eq!(copyable.input(ctrl_c), Some(9));
        let escape = WidgetInput::Key(crate::event::KeyEvent {
            code: KeyCode::Escape,
            vk: 0x1B,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        });
        assert_eq!(copyable.input(escape), None);
    }

    #[test]
    fn marquee_does_not_scroll_when_the_text_fits() {
        let mut marquee = MarqueeText::new("m", "short");
        let mut canvas = Recorder::default();
        paint::<()>(&marquee, &mut canvas);
        // 5 chars * 10 = 50 fits the 200 DIP rect.
        assert!(!marquee.is_scrolling());
        assert!(!marquee.advance(Duration::from_secs(1)));
        assert_eq!(marquee.offset(), 0.0);
        assert_eq!(canvas.clips, 0, "a fitting label is not clipped");
    }

    #[test]
    fn marquee_scrolls_with_pauses_and_wraps() {
        let mut marquee = MarqueeText::new("m", "0123456789abcde")
            .speed(100.0)
            .gap(20.0)
            .pause(1.0);
        place::<()>(&mut marquee, Rect::from_xywh(0.0, 0.0, 50.0, 22.0));
        let mut canvas = Recorder::default();
        paint::<()>(&marquee, &mut canvas);
        assert!(marquee.is_scrolling());
        assert!(canvas.clips == 0, "clip is balanced after paint");
        // 1 s pause, then 150 DIPs of travel at 100 DIP/s.
        assert!(!marquee.advance(Duration::from_millis(500)));
        assert_eq!(marquee.offset(), 0.0);
        assert!(marquee.advance(Duration::from_millis(1000)));
        assert!((marquee.offset() - 50.0).abs() < 0.01);
        // Reach the end (span 150 + gap 20) and hold there for the pause.
        assert!(marquee.advance(Duration::from_millis(1200)));
        assert!((marquee.offset() - 170.0).abs() < 0.01);
        assert!(!marquee.advance(Duration::from_millis(500)));
        // A full cycle wraps back to the start.
        assert!(marquee.advance(Duration::from_millis(600)));
        assert!(marquee.offset() < 1.0);
    }

    #[test]
    fn rich_text_wraps_across_runs_and_keeps_the_plain_text() {
        let mut rich: RichText<u32> = RichText::new("r")
            .run("hello ")
            .strong("brave ")
            .link(7, "world");
        place::<u32>(&mut rich, Rect::from_xywh(0.0, 0.0, 100.0, 60.0));
        assert_eq!(rich.plain_text(), "hello brave world");
        let mut canvas = Recorder::default();
        paint::<u32>(&rich, &mut canvas);
        let joined: String = canvas
            .texts
            .iter()
            .map(|(_, text, _)| text.as_str())
            .collect();
        // Spaces at a wrap point are not painted, so the joined lines have
        // no gap where the break happened.
        assert_eq!(joined, "hellobraveworld");
        assert!(canvas.texts.len() >= 2, "10 chars per line must wrap");
    }

    #[test]
    fn rich_text_click_finds_the_link_run() {
        let mut rich: RichText<u32> = RichText::new("r")
            .run("open ")
            .link(7, "details")
            .on_link(|id| id);
        place::<u32>(&mut rich, Rect::from_xywh(0.0, 0.0, 240.0, 24.0));
        let mut canvas = Recorder::default();
        paint::<u32>(&rich, &mut canvas);
        // "details" starts at x = 5 chars * 10 = 50.
        assert_eq!(
            rich.input(WidgetInput::ClickAt(Point::new(55.0, 10.0))),
            Some(7)
        );
        assert_eq!(
            rich.input(WidgetInput::ClickAt(Point::new(5.0, 10.0))),
            None
        );
        assert_eq!(rich.cursor_at(Point::new(55.0, 10.0)), Some(Cursor::Hand));
        assert_eq!(rich.cursor_at(Point::new(5.0, 10.0)), None);
    }

    #[test]
    fn rich_text_without_a_handler_is_not_interactive() {
        let rich: RichText<u32> = RichText::new("r").run("plain");
        assert!(!rich.interactive());
        let rich: RichText<u32> = RichText::new("r").link(1, "link");
        assert!(!rich.interactive());
        let rich: RichText<u32> = RichText::new("r").link(1, "link").on_link(|_| 1);
        assert!(rich.interactive());
    }

    #[test]
    fn rich_text_set_runs_replaces_the_text() {
        let mut rich: RichText<u32> = RichText::new("r").run("old");
        rich.set_runs(vec![RichRun::new("new")]);
        assert_eq!(rich.plain_text(), "new");
        rich.set_label("label".to_string());
        assert_eq!(rich.plain_text(), "label");
    }

    #[test]
    fn the_run_child_list_replaces_the_inline_runs() {
        let mut text: RichText<()> = RichText::new("rich");
        let mut plain = SkinElement::new("run", 2, 3);
        plain.push_attr("text", "重点：");
        let mut strong = SkinElement::new("run", 3, 3);
        strong.push_attr("text", "夜曲");
        strong.push_attr("style", "strong");
        let mut link = SkinElement::new("run", 4, 3);
        link.push_attr("text", "查看");
        link.push_attr("style", "link");
        link.push_attr("link", "7");
        let runs = [plain, strong, link];
        assert!(<RichText<()> as Widget<()>>::validate_skin_children(
            &text, &runs
        ));
        <RichText<()> as Widget<()>>::apply_skin_children(&mut text, &runs);
        let runs = text.runs_ref();
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].text, "重点：");
        assert_eq!(runs[0].style, RunStyle::Normal);
        assert_eq!(runs[1].style, RunStyle::Strong);
        assert_eq!(runs[2].style, RunStyle::Link);
        assert_eq!(runs[2].link, 7);
        assert_eq!(text.plain_text(), "重点：夜曲查看");

        // `link` on a non-link run, an unknown style and an empty text reject.
        let mut stray = SkinElement::new("run", 5, 3);
        stray.push_attr("text", "x");
        stray.push_attr("link", "1");
        assert!(!<RichText<()> as Widget<()>>::validate_skin_children(
            &text,
            &[stray]
        ));
        let mut bad_style = SkinElement::new("run", 6, 3);
        bad_style.push_attr("text", "x");
        bad_style.push_attr("style", "bold");
        assert!(!<RichText<()> as Widget<()>>::validate_skin_children(
            &text,
            &[bad_style]
        ));
        let mut empty = SkinElement::new("run", 7, 3);
        empty.push_attr("text", "");
        assert!(!<RichText<()> as Widget<()>>::validate_skin_children(
            &text,
            &[empty]
        ));
    }
}
