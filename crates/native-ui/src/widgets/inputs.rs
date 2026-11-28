// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Selection and search inputs: checkbox, radio button / radio group, search
//! box, and named aliases of the controls they wrap.
//!
//! The aliases exist because the player's vocabulary differs from the
//! framework's: a switch is a toggle, a number stepper is a number field with
//! the stepper chrome, and a password box is a text field with masking. They
//! are the same widgets with the same behavior, so there is no second
//! implementation to keep in sync.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use crate::a11y::{A11yChild, A11yInfo, A11yRole};
use crate::backend::TextAlign;
use crate::color::Color;
use crate::event::Cursor;
use crate::backend::TextStyle;
use crate::geometry::{Point, Rect};
use crate::theme::Palette;
use crate::geometry::Insets;
use crate::menu::Menu;
use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

use super::controls::Toggle;
use super::observers::{Observers, observer_builders, observer_methods};
use crate::skin::{SkinAttrDecl, SkinValue, SkinValueKind};
use crate::surface::{SurfaceStyle, surface_builders};
use super::edit::{NumberInput, TextEdit};

static SEARCH_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("cue", SkinValueKind::Text),
    SkinAttrDecl::new("color", SkinValueKind::Color),
];
use super::inline::PaintedField;

/// A toggle rendered as an on/off switch (same widget, player vocabulary).
pub type Switch<M> = Toggle<M>;
/// A number field with clickable up/down zones.
pub type NumberStepper<M> = NumberInput<M>;
/// A masked text field (call `.password()` when building it).
pub type PasswordBox<M> = TextEdit<M>;
/// A multiline text field (call `.multiline()` when building it). Enter
/// inserts a newline; Tab/Escape/focus loss commit or cancel like a single-line
/// field, and the native editor wraps and scrolls vertically.
pub type TextArea<M> = TextEdit<M>;

const BOX: f32 = 18.0;

fn check_colors(palette: &Palette, enabled: bool, active: bool, hovered: bool, pressed: bool) -> (Color, Color) {
    let border = if !enabled {
        palette.button_disabled
    } else if active {
        palette.accent
    } else if hovered {
        palette.accent.mix(palette.muted, 0.4)
    } else {
        palette.explicit_border
    };
    let background = if !enabled {
        palette.button_disabled
    } else if active {
        palette.accent
    } else if pressed {
        palette.button_pressed
    } else if hovered {
        palette.button_hover
    } else {
        palette.search_bg
    };
    (border, background)
}

/// Readable ink on an accent fill: light on a dark accent, near-black on a
/// light one (the player's default 竹绿 is light).
pub(crate) fn on_accent(color: Color) -> Color {
    if color.luminance() < 0.6 {
        Color::new(250, 250, 250)
    } else {
        Color::new(24, 26, 30)
    }
}

/// Draw the checkbox body and, when checked, a mark in the MDL2 glyph face.
/// The mark is a real glyph (U+E73E by default), not a UI-font character: the
/// UI face has no private-use glyphs, which is why an unchecked-looking filled
/// square was possible.
///
/// `radius` is the outer corner radius in DIPs; `0` paints a sharp square
/// (two plain fills, no corner ellipses) and larger values are clamped to half
/// the box. The inner fill keeps the border one DIP wide by using a radius
/// one DIP smaller.
pub(crate) fn paint_check(
    ctx: &mut WidgetPaint<'_>,
    rect: Rect,
    checked: bool,
    state: &WidgetState,
    glyph: char,
    radius: f32,
) {
    let (border, background) =
        check_colors(&ctx.palette, state.enabled, checked, state.hovered, state.pressed);
    let radius = radius.max(0.0).min(rect.width().min(rect.height()) / 2.0);
    let painter = ctx.painter;
    painter.round_rect(&mut *ctx.canvas, rect, radius, border);
    painter.round_rect(
        &mut *ctx.canvas,
        rect.inset(1.0, 1.0),
        (radius - 1.0).max(0.0),
        background,
    );
    if checked {
        let mut buf = [0u8; 4];
        let mark = glyph.encode_utf8(&mut buf);
        let size = (rect.height() * 0.74).max(9.0);
        let code = glyph as u32;
        // A private-use codepoint is an MDL2 glyph (the UI face has no PUA
        // coverage); a text character (e.g. an × from U+2715) belongs to the
        // UI face.
        let style = if (0xE000..=0xF8FF).contains(&code) {
            TextStyle::glyph(size)
        } else {
            let base = ctx.painter.body().clone();
            let mut style = ctx.style(&base);
            style.size = size.min(style.size + 1.0);
            style
        };
        let color = on_accent(background);
        ctx.canvas.text(rect, mark, &style, TextAlign::Center, color);
    }
}

fn paint_radio(ctx: &mut WidgetPaint<'_>, rect: Rect, selected: bool, state: &WidgetState) {
    let palette = ctx.palette;
    let accent = palette.accent;
    let border = if !state.enabled {
        palette.button_disabled
    } else if selected {
        accent
    } else if state.hovered {
        accent.mix(palette.muted, 0.4)
    } else {
        palette.explicit_border
    };
    let fill = if state.enabled {
        palette.search_bg
    } else {
        palette.button_disabled
    };
    ctx.canvas.fill_circle(rect, border);
    ctx.canvas.fill_circle(rect.inset(1.5, 1.5), fill);
    if selected {
        let dot = if state.enabled {
            accent
        } else {
            palette.text_disabled
        };
        ctx.canvas.fill_circle(rect.inset(4.5, 4.5), dot);
    }
}

/// A labelled checkbox. Emits the new checked state through `on_change`.
pub struct Checkbox<M> {
    id: WidgetId,
    rect: Rect,
    label: String,
    checked: bool,
    enabled: bool,
    /// Checked-state mark; defaults to the MDL2 check (U+E73E).
    check: char,
    /// Outer box corner radius in DIPs (`0` = square). Clamped to half the box
    /// at paint time.
    corner: f32,
    on_change: Box<dyn Fn(bool) -> M>,
    observers: Observers<M>,
}

impl<M> Checkbox<M> {
    pub fn new(
        id: &'static str,
        label: impl Into<String>,
        checked: bool,
        on_change: impl Fn(bool) -> M + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 180.0, 28.0),
            label: label.into(),
            checked,
            enabled: true,
            check: '\u{E73E}',
            corner: crate::theme::radius::SMALL,
            on_change: Box::new(on_change),
            observers: Observers::new(),
        }
    }

    /// Custom checked-state mark (e.g. `\u{2715}` for an ×, or another MDL2
    /// glyph). Private-use glyphs are drawn with the glyph face.
    pub fn check(mut self, glyph: char) -> Self {
        self.check = glyph;
        self
    }

    pub fn check_glyph(&self) -> char {
        self.check
    }

    /// Paint the box with sharp corners (radius `0`).
    pub fn square(mut self) -> Self {
        self.corner = 0.0;
        self
    }

    /// Outer box corner radius in DIPs. `0` is square; the value is clamped to
    /// half the box when painting, so a pill radius simply rounds as far as
    /// the box allows.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner = radius.max(0.0);
        self
    }

    /// Runtime setter for the configured radius (not yet clamped).
    pub fn set_corner_radius(&mut self, radius: f32) {
        self.corner = radius.max(0.0);
    }

    /// The configured corner radius in DIPs (`0` = square).
    pub fn corner_radius_value(&self) -> f32 {
        self.corner
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn checked(&self) -> bool {
        self.checked
    }

    fn box_rect(&self) -> Rect {
        let center = self.rect.center().y;
        Rect::new(
            self.rect.left + 2.0,
            center - BOX / 2.0,
            self.rect.left + 2.0 + BOX,
            center + BOX / 2.0,
        )
    }

    fn label_rect(&self, box_rect: Rect) -> Rect {
        Rect::new(
            box_rect.right + 8.0,
            self.rect.top,
            self.rect.right,
            self.rect.bottom,
        )
    }
}

impl<M: 'static> Widget<M> for Checkbox<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::LABEL
            .union(crate::props::ContentCaps::TOGGLED)
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
        28.0
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let box_rect = self.box_rect();
        paint_check(ctx, box_rect, self.checked, state, self.check, self.corner);
        let label = self.label_rect(box_rect);
        let color = if self.enabled {
            ctx.palette.text
        } else {
            ctx.palette.text_disabled
        };
        ctx.text(label, &self.label, TextStyleRole::Body, TextAlign::Left, color);
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::ClickAt(_) | WidgetInput::Activate => {
                self.checked = !self.checked;
                Some((self.on_change)(self.checked))
            }
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Toggle, self.label.clone())
                .checked(self.checked)
                .read_only(!self.enabled),
        )
    }

    fn set_toggle(&mut self, on: bool) {
        self.checked = on;
    }

    fn set_label(&mut self, label: String) {
        self.label = label;
    }
}

observer_builders!(Checkbox);

/// A standalone radio button. It only emits when it becomes selected; making
/// another button in the group selected is the application's job (or use
/// [`RadioGroup`]).
pub struct RadioButton<M> {
    id: WidgetId,
    rect: Rect,
    label: String,
    selected: bool,
    enabled: bool,
    on_select: Box<dyn Fn() -> M>,
    observers: Observers<M>,
}

impl<M> RadioButton<M> {
    pub fn new(
        id: &'static str,
        label: impl Into<String>,
        selected: bool,
        on_select: impl Fn() -> M + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 180.0, 28.0),
            label: label.into(),
            selected,
            enabled: true,
            on_select: Box::new(on_select),
            observers: Observers::new(),
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn selected(&self) -> bool {
        self.selected
    }

    /// Select or clear this button directly; the group owner usually does
    /// this when another button takes the selection.
    pub fn set_checked(&mut self, selected: bool) {
        self.selected = selected;
    }

    fn dot_rect(&self) -> Rect {
        let center = self.rect.center().y;
        Rect::new(
            self.rect.left + 2.0,
            center - BOX / 2.0,
            self.rect.left + 2.0 + BOX,
            center + BOX / 2.0,
        )
    }
}

impl<M: 'static> Widget<M> for RadioButton<M> {
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

    observer_methods!();

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
    }

    /// A standalone radio's visual is its dot, so the ring hugs the dot
    /// instead of the label (a `RadioGroup` paints its own selection).
    fn focus_ring_rect(&self) -> Rect {
        self.dot_rect().inflate(2.0, 2.0)
    }

    fn focus_ring_radius(&self) -> Option<f32> {
        let ring = self.focus_ring_rect();
        Some(ring.width().min(ring.height()) / 2.0)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let dot = self.dot_rect();
        paint_radio(ctx, dot, self.selected, state);
        let label = Rect::new(
            dot.right + 8.0,
            self.rect.top,
            self.rect.right,
            self.rect.bottom,
        );
        let color = if self.enabled {
            ctx.palette.text
        } else {
            ctx.palette.text_disabled
        };
        ctx.text(label, &self.label, TextStyleRole::Body, TextAlign::Left, color);
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled || self.selected {
            return None;
        }
        match input {
            WidgetInput::ClickAt(_) | WidgetInput::Activate => {
                self.selected = true;
                Some((self.on_select)())
            }
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let mut info = A11yInfo::new(A11yRole::RadioItem, self.label.clone());
        info.selected = Some(self.selected);
        info.read_only = !self.enabled;
        Some(info)
    }

    fn set_label(&mut self, label: String) {
        self.label = label;
    }
}

observer_builders!(RadioButton);

/// A vertical radio group: one widget that paints and manages its options,
/// like a native radio group. Arrows move (and select) the option in adjust
/// mode; disabled options are skipped by pointer, keyboard and UIA.
pub struct RadioGroup<M> {
    id: WidgetId,
    rect: Rect,
    options: Vec<String>,
    disabled: Vec<bool>,
    selected: usize,
    row_height: f32,
    gap: f32,
    enabled: bool,
    on_select: Box<dyn Fn(usize) -> M>,
    hover: Cell<Option<usize>>,
    observers: Observers<M>,
}

impl<M> RadioGroup<M> {
    pub fn new(
        id: &'static str,
        options: &[&str],
        selected: usize,
        on_select: impl Fn(usize) -> M + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 200.0, 96.0),
            options: options.iter().map(|option| option.to_string()).collect(),
            disabled: vec![false; options.len()],
            selected,
            row_height: 28.0,
            gap: 4.0,
            enabled: true,
            on_select: Box::new(on_select),
            hover: Cell::new(None),
            observers: Observers::new(),
        }
    }

    pub fn disable(mut self, index: usize) -> Self {
        if index < self.disabled.len() {
            self.disabled[index] = true;
        }
        self
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn option_enabled(&self, index: usize) -> bool {
        self.disabled.get(index).is_some_and(|disabled| !*disabled)
    }

    pub fn set_option_disabled(&mut self, index: usize, disabled: bool) {
        if index < self.disabled.len() {
            self.disabled[index] = disabled;
        }
    }

    fn row_rect(&self, index: usize) -> Rect {
        let top = self.rect.top + index as f32 * (self.row_height + self.gap);
        Rect::new(
            self.rect.left,
            top,
            self.rect.right,
            top + self.row_height,
        )
    }

    fn row_at(&self, pos: Point) -> Option<usize> {
        (0..self.options.len()).find(|index| self.row_rect(*index).contains_point(pos))
    }

    fn select_index(&mut self, index: usize) -> Option<M> {
        if index >= self.options.len() || !self.option_enabled(index) || index == self.selected {
            return None;
        }
        self.selected = index;
        Some((self.on_select)(index))
    }

    fn step(&mut self, delta: i32) -> Option<M> {
        let count = self.options.len();
        if count == 0 {
            return None;
        }
        let mut index = self.selected as i32;
        for _ in 0..count {
            index = (index + delta).rem_euclid(count as i32);
            if self.option_enabled(index as usize) {
                return self.select_index(index as usize);
            }
        }
        None
    }
}

impl<M: 'static> Widget<M> for RadioGroup<M> {
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
        (self.options.len() as f32 * (self.row_height + self.gap) - self.gap).max(0.0)
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover.set(None);
        }
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    fn adjustable(&self) -> bool {
        true
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let hovered = self.hover.get();
        for index in 0..self.options.len() {
            let row = self.row_rect(index);
            let enabled = self.option_enabled(index);
            if Some(index) == hovered && enabled && index != self.selected {
                ctx.canvas.fill_rect(row.inset(0.0, 1.0), ctx.palette.row_hover);
            }
            let dot = Rect::new(
                row.left + 2.0,
                row.center().y - BOX / 2.0,
                row.left + 2.0 + BOX,
                row.center().y + BOX / 2.0,
            );
            let state = WidgetState {
                hovered: Some(index) == hovered,
                enabled: self.enabled && enabled,
                ..Default::default()
            };
            paint_radio(ctx, dot, index == self.selected, &state);
            let label = Rect::new(
                dot.right + 8.0,
                row.top,
                row.right,
                row.bottom,
            );
            let color = if self.enabled && enabled {
                ctx.palette.text
            } else {
                ctx.palette.text_disabled
            };
            ctx.text(
                label,
                &self.options[index],
                TextStyleRole::Body,
                TextAlign::Left,
                color,
            );
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::HoverAt(pos) => {
                self.hover.set(self.row_at(pos));
                None
            }
            WidgetInput::HoverLeave => {
                self.hover.set(None);
                None
            }
            WidgetInput::ClickAt(pos) => {
                let index = self.row_at(pos)?;
                self.select_index(index)
            }
            WidgetInput::Activate => self.select_index(self.selected),
            WidgetInput::Key(key) => match key.code {
                crate::event::KeyCode::Up | crate::event::KeyCode::Left => self.step(-1),
                crate::event::KeyCode::Down | crate::event::KeyCode::Right => self.step(1),
                crate::event::KeyCode::Home => self.select_index(0),
                crate::event::KeyCode::End => self.select_index(self.options.len().saturating_sub(1)),
                _ => None,
            },
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let children = self
            .options
            .iter()
            .enumerate()
            .map(|(index, label)| {
                A11yChild::new(index, label.clone(), self.row_rect(index))
                    .selected(index == self.selected)
                    .enabled(self.option_enabled(index))
            })
            .collect();
        Some(
            A11yInfo::new(
                A11yRole::RadioGroup,
                self.options
                    .get(self.selected)
                    .cloned()
                    .unwrap_or_else(|| self.id.0.to_string()),
            )
            .children(children),
        )
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        self.select_index(index)
    }

    fn set_selected(&mut self, index: usize) {
        if index < self.options.len() {
            self.selected = index;
        }
    }

    fn tooltip_rect(&self) -> Rect {
        self.row_rect(self.selected.min(self.options.len().saturating_sub(1)))
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.row_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        Some(self.selected)
    }
}

observer_builders!(RadioGroup);

/// A search field with a leading glyph and a clear button. Commits on
/// Enter/Tab/focus loss like a text field; the application can schedule its
/// own debounce using [`SearchBox::debounce`].
pub struct SearchBox<M> {
    id: WidgetId,
    rect: Rect,
    text: String,
    cue: String,
    color: Option<Color>,
    enabled: bool,
    /// Locks the value without disabling the box: the session still selects
    /// and copies, only mutations are refused.
    read_only: bool,
    max_length: u32,
    debounce: Option<Duration>,
    on_commit: Box<dyn Fn(String) -> M>,
    clear_hover: Cell<bool>,
    /// The embedded painted field, open while the box is being edited.
    field: RefCell<PaintedField<M>>,
    editing: bool,
    surface: SurfaceStyle,
}

impl<M> SearchBox<M> {
    pub fn new(
        id: &'static str,
        text: impl Into<String>,
        on_commit: impl Fn(String) -> M + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 34.0),
            text: text.into(),
            cue: "搜索".to_string(),
            color: None,
            enabled: true,
            read_only: false,
            max_length: 0,
            debounce: None,
            on_commit: Box::new(on_commit),
            clear_hover: Cell::new(false),
            field: RefCell::new(PaintedField::new()),
            editing: false,
            surface: SurfaceStyle::new(),
        }
    }

    pub fn cue(mut self, cue: impl Into<String>) -> Self {
        self.cue = cue.into();
        self
    }

    pub fn cue_text(&self) -> &str {
        &self.cue
    }

    /// Replace the cue (placeholder) silently (the skin's `cue` attribute).
    pub fn set_cue(&mut self, cue: impl Into<String>) {
        self.cue = cue.into();
    }

    /// Replace the field text colour silently (the skin's `color`); `None`
    /// restores the palette's text colour.
    pub fn set_color(&mut self, color: Option<Color>) {
        self.color = color;
        self.field.get_mut().set_text_color(color);
    }

    pub fn color_value(&self) -> Option<Color> {
        self.color
    }

    /// Lock the value like `TextEdit::read_only`: focus, selection and copy
    /// stay available, mutations (typing, paste, cut, delete, undo, the clear
    /// button) are refused, and the UIA value reports read-only. Distinct from
    /// `enabled(false)`, which also removes focus/mouse policy and greys the
    /// box.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn max_length(mut self, length: u32) -> Self {
        self.max_length = length;
        self
    }

    /// Debounce hint for the application's own timer; the widget emits one
    /// message per commit and never starts a timer itself.
    pub fn debounce(mut self, duration: Duration) -> Self {
        self.debounce = Some(duration);
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn debounce_value(&self) -> Option<Duration> {
        self.debounce
    }

    /// Whether a painted editing session is open.
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    fn clear_rect(&self) -> Rect {
        let size = 22.0;
        Rect::new(
            self.rect.right - size - 4.0,
            self.rect.center().y - size / 2.0,
            self.rect.right - 4.0,
            self.rect.center().y + size / 2.0,
        )
    }

    fn glyph_rect(&self) -> Rect {
        Rect::new(
            self.rect.left + 8.0,
            self.rect.top,
            self.rect.left + 26.0,
            self.rect.bottom,
        )
    }

    /// Text area before the clear button. The editing field always reserves
    /// the clear button's room, so typed text never runs under it.
    fn text_rect(&self) -> Rect {
        let left = self.glyph_rect().right + 4.0;
        Rect::new(
            left,
            self.rect.top,
            (self.clear_rect().left - 2.0).max(left),
            self.rect.bottom,
        )
    }

    fn clear_hit(&self, pos: Point) -> bool {
        let has_text = if self.editing {
            !self.field.borrow().text().is_empty()
        } else {
            !self.text.is_empty()
        };
        has_text && self.clear_rect().contains_point(pos)
    }

    /// Open (or re-position) the painted session. `None` selects the whole
    /// value so typing replaces it; a pointer press places the caret.
    fn begin_edit(&mut self, position: Option<Point>) -> bool
    where
        M: 'static,
    {
        if !self.enabled {
            return false;
        }
        if self.editing {
            return match position {
                Some(pos) => self.field.get_mut().place_at(pos),
                None => true,
            };
        }
        let rect = self.text_rect();
        self.field.get_mut().set_read_only(self.read_only);
        self.field.get_mut().set_text_color(self.color);
        let opened = self.field.get_mut().begin_styled(
            rect,
            &self.text,
            position,
            Insets::xy(0.0, 2.0),
            TextAlign::Left,
            false,
            self.max_length,
        );
        if opened {
            self.editing = true;
        }
        opened
    }

    /// Close the session. `commit` applies the typed text and reports it
    /// through `on_commit` when it changed.
    fn finish_edit(&mut self, commit: bool) -> Option<M> {
        if !self.editing {
            return None;
        }
        self.editing = false;
        self.clear_hover.set(false);
        let text = self.field.get_mut().finish(commit)?;
        if text == self.text {
            return None;
        }
        self.text = text;
        Some((self.on_commit)(self.text.clone()))
    }
}

impl<M: 'static> Widget<M> for SearchBox<M> {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
    }

    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::TEXT
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        SEARCH_SKIN_ATTRS
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("cue", SkinValue::Text(cue)) => self.cue = (*cue).to_string(),
            ("color", SkinValue::Color(_)) => {
                if let Some(color) = value.as_color() {
                    self.set_color(Some(color));
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
        34.0
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.finish_edit(false);
            self.clear_hover.set(false);
        }
    }

    // --- painted session ---------------------------------------------------

    fn inline_edit(&self) -> bool {
        self.enabled
    }

    fn inline_edit_at(&self, pos: Point) -> bool {
        self.enabled && self.rect.contains_point(pos) && !self.clear_hit(pos)
    }

    fn editing(&self) -> bool {
        self.editing
    }

    fn begin_inline_edit(&mut self, position: Option<Point>) -> bool {
        self.begin_edit(position)
    }

    fn finish_edit(&mut self, commit: bool) -> Option<M> {
        SearchBox::finish_edit(self, commit)
    }

    fn insert_text(&mut self, text: &str) -> bool {
        if !self.editing {
            return false;
        }
        self.field.get_mut().insert_text(text)
    }

    fn set_composition(&mut self, text: &str, cursor: usize) -> bool {
        if !self.editing {
            return false;
        }
        self.field.get_mut().set_composition(text, cursor)
    }

    fn caret_rect(&self) -> Option<Rect> {
        if !self.editing {
            return None;
        }
        self.field.borrow().caret_rect()
    }

    fn edit_menu(&self) -> Option<Menu> {
        self.field.borrow().edit_menu()
    }

    fn edit_command(&mut self, command: u32) -> bool {
        self.field.get_mut().edit_command(command)
    }

    fn take_clipboard(&mut self) -> Option<String> {
        self.field.get_mut().take_clipboard()
    }

    fn focus_ring_radius(&self) -> Option<f32> {
        self.surface.focus_radius()
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        let border = if !self.enabled {
            palette.button_disabled
        } else if state.focused {
            palette.accent
        } else {
            palette.explicit_border
        };
        self.surface.frame(ctx.canvas, self.rect, border, palette.search_bg);
        let glyph = self.glyph_rect();
        ctx.text(
            glyph,
            "\u{E721}",
            TextStyleRole::Small,
            TextAlign::Center,
            palette.muted,
        );
        let shown = if self.editing {
            self.field.borrow().text().to_string()
        } else {
            self.text.clone()
        };
        let text = self.text_rect();
        let body = ctx.painter.body().clone();
        let style = ctx.style(&body);
        if self.editing {
            // The field paints the live text, selection, IME strip and caret;
            // its own frame stays off because the box has already drawn one.
            let mut field = self.field.borrow_mut();
            if field.rect() != text {
                field.set_rect(text);
            }
            field.paint(ctx, state);
        } else if shown.is_empty() {
            ctx.canvas
                .text(text, &self.cue, &style, TextAlign::Left, palette.muted);
        } else {
            let color = if self.enabled {
                self.color.unwrap_or(palette.text)
            } else {
                palette.text_disabled
            };
            ctx.canvas
                .text(text, &shown, &style, TextAlign::Left, color);
        }
        if !shown.is_empty() {
            let clear = self.clear_rect();
            let tint = if self.clear_hover.get() {
                palette.text
            } else {
                palette.muted
            };
            ctx.text(
                clear,
                "\u{00D7}",
                TextStyleRole::Small,
                TextAlign::Center,
                tint,
            );
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        // Pointer work inside an open session belongs to the field (caret,
        // selection drag, word select); the clear button lives outside it.
        if self.editing {
            // The open session owns the keyboard (arrows, Home/End, Delete,
            // Backspace, undo/redo, copy/cut) and the pointer (caret,
            // selection drag, word select). Tab/Escape/Enter and the paste
            // chord are intercepted by the container before they get here.
            match input {
                WidgetInput::PressAt(_)
                | WidgetInput::DragTo(_)
                | WidgetInput::ReleaseAt(_)
                | WidgetInput::DoubleClickAt(_)
                | WidgetInput::RightClickAt(_)
                | WidgetInput::Wheel(_)
                | WidgetInput::Key(_) => {
                    self.field.get_mut().input(input);
                    return None;
                }
                _ => {}
            }
        }
        match input {
            WidgetInput::HoverAt(pos) => {
                self.clear_hover.set(!self.read_only && self.clear_hit(pos));
                None
            }
            WidgetInput::HoverLeave => {
                self.clear_hover.set(false);
                None
            }
            WidgetInput::ClickAt(pos) => {
                if !self.read_only && self.clear_hit(pos) {
                    self.field.get_mut().finish(false);
                    self.editing = false;
                    self.text.clear();
                    self.clear_hover.set(false);
                    return Some((self.on_commit)(String::new()));
                }
                None
            }
            _ => None,
        }
    }

    fn repaint_on_move(&self) -> bool {
        self.editing || !self.text.is_empty()
    }

    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        if self.clear_hit(pos) {
            Some(Cursor::Arrow)
        } else {
            Some(Cursor::IBeam)
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let name = if self.cue.is_empty() {
            self.id.0.to_string()
        } else {
            self.cue.clone()
        };
        let value = if self.editing {
            self.field.borrow().text().to_string()
        } else {
            self.text.clone()
        };
        Some(
            A11yInfo::new(A11yRole::TextField, name)
                .value(value)
                .read_only(self.read_only || !self.enabled),
        )
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        self.field.get_mut().set_read_only(read_only);
        if read_only {
            self.clear_hover.set(false);
        }
    }

    fn commit_text(&mut self, text: String) -> Option<M> {
        if self.read_only {
            if self.editing {
                self.field.get_mut().finish(false);
                self.editing = false;
            }
            return None;
        }
        if self.editing {
            self.field.get_mut().finish(false);
            self.editing = false;
        }
        if text == self.text {
            return None;
        }
        self.text = text;
        self.clear_hover.set(false);
        Some((self.on_commit)(self.text.clone()))
    }

    fn set_text(&mut self, text: String) {
        if self.editing {
            self.field.get_mut().finish(false);
            self.editing = false;
        }
        self.text = text;
        self.clear_hover.set(false);
    }
}

surface_builders!(SearchBox);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Canvas;

    #[test]
    fn a_read_only_search_box_refuses_mutations_but_keeps_the_session() {
        let mut field: SearchBox<&'static str> =
            SearchBox::new("s", "abc", |_| "committed").read_only(true);
        assert!(field.is_read_only());
        // The session opens for selection/copy; mutations are refused.
        assert!(field.begin_inline_edit(None));
        assert!(field.editing());
        assert!(!field.insert_text("x"));
        assert_eq!(field.finish_edit(true), None);
        assert_eq!(field.text(), "abc");
        // The clear button is a mutation too.
        let clear = field.clear_rect().center();
        assert_eq!(field.input(WidgetInput::ClickAt(clear)), None);
        assert_eq!(field.text(), "abc");
        // The container's property reaches the widget and UIA reports it;
        // read-only is not disabled (the field stays enabled).
        assert!(<SearchBox<&'static str> as Widget<&'static str>>::read_only(&field));
        assert!(field.enabled());
        assert!(field.a11y().expect("a11y").read_only);
        // A programmatic setter still changes the value.
        field.set_text("new".to_string());
        assert_eq!(field.text(), "new");
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

    /// Records the body shape: corner ellipses only appear for a rounded box.
    #[derive(Default)]
    struct BoxRecorder {
        fills: Vec<Rect>,
        ellipses: Vec<Rect>,
    }

    impl Canvas for BoxRecorder {
        fn scale(&self) -> f32 {
            1.0
        }
        fn dpi(&self) -> f32 {
            96.0
        }
        fn fill_rect(&mut self, rect: Rect, _color: Color) {
            self.fills.push(rect);
        }
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

    fn record_checkbox(check: &Checkbox<()>) -> BoxRecorder {
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
                viewport: Rect::from_xywh(0.0, 0.0, 400.0, 300.0),
                dirty: &[],
            };
            <Checkbox<()> as Widget<()>>::paint(
                check,
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
    fn checkbox_toggles_and_reports_the_new_state() {
        let mut checkbox: Checkbox<bool> =
            Checkbox::new("c", "随机播放", false, |on| on);
        assert_eq!(
            checkbox.input(WidgetInput::ClickAt(Point::new(10.0, 10.0))),
            Some(true)
        );
        assert!(checkbox.checked());
        assert_eq!(checkbox.input(WidgetInput::Activate), Some(false));
        assert!(!checkbox.checked());
        checkbox.set_toggle(true);
        assert!(checkbox.checked());
        // Disabled instances ignore input.
        checkbox.set_enabled(false);
        assert_eq!(checkbox.input(WidgetInput::Activate), None);
    }

    #[test]
    fn checkbox_uses_a_glyph_check_and_allows_a_custom_one() {
        let checkbox: Checkbox<bool> = Checkbox::new("c", "随机", false, |on| on);
        assert_eq!(checkbox.check_glyph(), '\u{E73E}');
        let custom = checkbox.check('\u{2715}');
        assert_eq!(custom.check_glyph(), '\u{2715}');
    }

    #[test]
    fn checkbox_box_is_rounded_by_default_and_square_on_request() {
        let rounded: Checkbox<()> = Checkbox::new("c", "随机", false, |_| ());
        assert_eq!(rounded.corner_radius_value(), 4.0);
        let painted = record_checkbox(&rounded);
        // Two rounded fills (border + inset background), four corner ellipses
        // each: the current composition, but the shape is real.
        assert_eq!(painted.fills.len(), 6);
        assert_eq!(painted.ellipses.len(), 8);

        let square = rounded.square();
        assert_eq!(square.corner_radius_value(), 0.0);
        let painted = record_checkbox(&square);
        assert!(painted.ellipses.is_empty());
        assert_eq!(painted.fills.len(), 2);
    }

    #[test]
    fn checkbox_corner_radius_clamps_to_half_the_box() {
        // Boxes are 18 DIPs; a huge radius rounds as far as the box allows.
        let checkbox: Checkbox<()> =
            Checkbox::new("c", "随机", false, |_| ()).corner_radius(999.0);
        let painted = record_checkbox(&checkbox);
        assert_eq!(painted.ellipses.len(), 8);
        for ellipse in &painted.ellipses {
            assert!(ellipse.width() <= 18.0 + 0.01);
            assert!(ellipse.height() <= 18.0 + 0.01);
        }
    }

    #[test]
    fn checkbox_a11y_is_a_checked_toggle() {
        let mut checkbox: Checkbox<bool> = Checkbox::new("c", "随机", true, |on| on);
        let info = checkbox.a11y().unwrap();
        assert_eq!(info.role, A11yRole::Toggle);
        assert_eq!(info.name, "随机");
        assert_eq!(info.checked, Some(true));
        checkbox.set_toggle(false);
        assert_eq!(checkbox.a11y().unwrap().checked, Some(false));
    }

    #[test]
    fn radio_button_emits_only_when_it_becomes_selected() {
        let mut radio: RadioButton<u32> =
            RadioButton::new("r", "单曲循环", false, || 5);
        assert_eq!(radio.input(WidgetInput::Activate), Some(5));
        assert!(radio.selected());
        assert_eq!(radio.input(WidgetInput::Activate), None);
        assert_eq!(radio.a11y().unwrap().selected, Some(true));
    }

    #[test]
    fn radio_group_arrows_skip_disabled_options() {
        let mut group: RadioGroup<usize> =
            RadioGroup::new("g", &["a", "b", "c"], 0, |index| index).disable(1);
        assert!(group.adjustable());
        assert_eq!(group.input(key(crate::event::KeyCode::Down)), Some(2));
        assert_eq!(group.selected(), 2);
        assert_eq!(group.input(key(crate::event::KeyCode::Down)), Some(0));
        assert_eq!(group.input(key(crate::event::KeyCode::Home)), None);
        assert_eq!(group.input(key(crate::event::KeyCode::End)), Some(2));
    }

    #[test]
    fn radio_group_click_selects_its_row_and_reports_children() {
        let mut group: RadioGroup<usize> =
            RadioGroup::new("g", &["a", "b", "c"], 0, |index| index);
        group.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 96.0));
        // Second row: 28 high + 4 gap.
        assert_eq!(
            group.input(WidgetInput::ClickAt(Point::new(10.0, 40.0))),
            Some(1)
        );
        let info = group.a11y().unwrap();
        assert_eq!(info.role, A11yRole::RadioGroup);
        assert_eq!(info.children.len(), 3);
        assert!(info.children[0].selected == false);
        assert!(info.children[1].selected);
        assert_eq!(group.a11y_select_child(2), Some(2));
        assert_eq!(group.selected(), 2);
    }

    #[test]
    fn search_box_edits_in_place_through_the_painted_field() {
        let mut search: SearchBox<String> = SearchBox::new("s", "abc", |text| text);
        search.set_rect(Rect::from_xywh(0.0, 0.0, 240.0, 34.0));
        // A click in the text area opens the session with the caret placed
        // (the first session has no layout yet, so it lands at the end).
        assert!(search.inline_edit_at(Point::new(60.0, 17.0)));
        assert!(search.begin_inline_edit(Some(Point::new(60.0, 17.0))));
        assert!(search.is_editing());
        assert!(search.insert_text("x"));
        assert_eq!(search.finish_edit(true), Some("abcx".to_string()));
        assert_eq!(search.text(), "abcx");
        // A keyboard session starts selected, so typing replaces it; cancel
        // keeps the committed value.
        assert!(search.begin_inline_edit(None));
        assert!(search.insert_text("新"));
        assert_eq!(search.finish_edit(false), None);
        assert_eq!(search.text(), "abcx");
        assert!(search.begin_inline_edit(None));
        assert!(search.insert_text("新"));
        assert_eq!(search.finish_edit(true), Some("新".to_string()));
        assert_eq!(search.text(), "新");
        // The clear target is outside the session and reports the empty value.
        assert!(!search.inline_edit_at(Point::new(225.0, 17.0)));
        assert_eq!(search.a11y().unwrap().value.as_deref(), Some("新"));
    }

    #[test]
    fn the_search_box_session_owns_the_editing_keys() {
        let mut search: SearchBox<String> = SearchBox::new("s", "", |text| text);
        search.set_rect(Rect::from_xywh(0.0, 0.0, 240.0, 34.0));
        assert!(search.begin_inline_edit(None));
        assert!(search.insert_text("abcd"));
        let key = |code, ctrl| {
            WidgetInput::Key(crate::event::KeyEvent {
                code,
                vk: 0,
                ctrl,
                shift: false,
                alt: false,
                repeat: false,
            })
        };
        // Caret motion and deletion edit the live text, not just the field's
        // internal buffer.
        search.input(key(crate::event::KeyCode::Left, false));
        search.input(key(crate::event::KeyCode::Backspace, false));
        search.input(key(crate::event::KeyCode::Delete, false));
        assert_eq!(search.field.borrow().text(), "ab");
        // Ctrl+A then Ctrl+X selects all and reports it through the clipboard
        // seam; the session keeps running on the emptied text.
        search.input(key(crate::event::KeyCode::Letter(0), true));
        search.input(key(crate::event::KeyCode::Letter(23), true));
        assert_eq!(search.take_clipboard().as_deref(), Some("ab"));
        assert_eq!(search.field.borrow().text(), "");
        // The emptied value equals the opened one, so the commit reports
        // nothing; a new session pushes its text.
        assert_eq!(search.finish_edit(true), None);
        assert!(search.begin_inline_edit(None));
        assert!(search.insert_text("xy"));
        assert_eq!(search.finish_edit(true), Some("xy".to_string()));
    }

    #[test]
    fn search_box_clear_target_clears_and_commits() {
        let mut search: SearchBox<String> =
            SearchBox::new("s", "夜曲", |text| text);
        search.set_rect(Rect::from_xywh(0.0, 0.0, 240.0, 34.0));
        assert!(!search.inline_edit_at(Point::new(225.0, 17.0)));
        assert_eq!(
            search.input(WidgetInput::ClickAt(Point::new(225.0, 17.0))),
            Some(String::new())
        );
        assert_eq!(search.text(), "");
        assert_eq!(
            search.a11y().unwrap().value.as_deref(),
            Some(""),
            "the cleared value reaches the tree"
        );
    }

    #[test]
    fn search_box_commit_stores_text_and_emits() {
        let mut search: SearchBox<String> =
            SearchBox::new("s", "", |text| text);
        assert_eq!(search.commit_text("竹韵".to_string()), Some("竹韵".to_string()));
        assert_eq!(search.text(), "竹韵");
        assert_eq!(search.commit_text("竹韵".to_string()), None);
    }

    #[test]
    fn aliases_have_the_underlying_widget_defaults() {
        // The aliases are type aliases, so a compile-time check is enough:
        // these lines would not type-check if they diverged.
        let _switch: Switch<bool> = Toggle::new("s", "开关", false, |on| on);
        let _stepper: NumberStepper<i64> = NumberInput::new("n", 0, 0..=10, |value| value);
        let _password: PasswordBox<String> = TextEdit::new("p", "", |text| text).password();
    }
}
