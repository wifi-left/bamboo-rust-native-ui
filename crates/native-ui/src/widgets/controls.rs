// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Basic controls: buttons, toggles, segments, tabs, sliders, progress, pager,
//! color swatches.

use crate::a11y::{A11yChild, A11yInfo, A11yRole};
use crate::backend::TextAlign;
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::theme::Palette;
use crate::skin::{SkinAttrDecl, SkinElement, SkinValue, SkinValueKind};
use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};
use super::observers::{Observers, observer_builders, observer_methods};
use crate::surface::{SurfaceStyle, surface_builders};

static SWATCH_SKIN_ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("color", SkinValueKind::Color)];

/// The `<option label>` list shared by `Segmented` and `Tabs` (the combo box
/// declares the same child name in its own module). Per-option enabled or
/// badge state stays Rust-owned: the file only speaks labels.
static OPTION_SKIN_CHILDREN: &[&str] = &["option"];
const OPTION_SKIN_ATTRS: &[&str] = &["label"];
static TABS_SKIN_ATTRS: &[SkinAttrDecl] =
    &[SkinAttrDecl::new("label-align", SkinValueKind::Text)];

fn valid_skin_option(child: &SkinElement) -> bool {
    child.name == "option"
        && child
            .attr("label")
            .is_some_and(|label| !label.trim().is_empty())
        && child.attrs_are(OPTION_SKIN_ATTRS)
        && child.children.is_empty()
}

fn skin_option_labels(children: &[SkinElement]) -> Vec<String> {
    children
        .iter()
        .filter_map(|child| child.attr("label").map(str::to_string))
        .collect()
}

fn state_fill(pal: &Palette, state: &WidgetState, variant: ButtonVariant) -> Color {
    if !state.enabled {
        return pal.button_disabled;
    }
    match variant {
        ButtonVariant::Danger => {
            let base = if pal.dark {
                Color::new(188, 66, 66)
            } else {
                Color::new(178, 42, 42)
            };
            if state.pressed {
                base.darken(0.12)
            } else if state.hovered {
                base.lighten(0.10)
            } else {
                base
            }
        }
        ButtonVariant::Primary => {
            if state.pressed {
                pal.button_pressed
            } else if state.hovered {
                pal.button_active_hover
            } else {
                pal.button_active
            }
        }
        ButtonVariant::Secondary => {
            if state.pressed {
                pal.button_pressed
            } else if state.hovered {
                pal.button_hover
            } else {
                pal.button_bg
            }
        }
    }
}

fn state_text(pal: &Palette, state: &WidgetState, variant: ButtonVariant) -> Color {
    if !state.enabled {
        return pal.text_disabled;
    }
    match variant {
        ButtonVariant::Danger => Color::new(250, 250, 250),
        ButtonVariant::Primary => pal.accent_text,
        ButtonVariant::Secondary => {
            if state.pressed || state.hovered {
                pal.accent_text
            } else {
                pal.text
            }
        }
    }
}

/// Button emphasis. `Secondary` is the neutral default, `Primary` uses the
/// accent fill and `Danger` a destructive red (independent of the accent).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Secondary,
    Primary,
    Danger,
}

pub struct Button<M> {
    id: WidgetId,
    rect: Rect,
    label: String,
    glyph: Option<char>,
    glyph_gap: f32,
    variant: ButtonVariant,
    enabled: bool,
    message: M,
    observers: Observers<M>,
    surface: SurfaceStyle,
}

impl<M> Button<M> {
    pub fn new(id: &'static str, label: impl Into<String>, message: M) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 120.0, 36.0),
            label: label.into(),
            glyph: None,
            glyph_gap: 0.0,
            variant: ButtonVariant::Secondary,
            enabled: true,
            message,
            observers: Observers::new(),
            surface: SurfaceStyle::new(),
        }
    }

    pub fn glyph(mut self, glyph: char) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// Space between the glyph strip and the label, in DIPs.
    pub fn glyph_gap(mut self, gap: f32) -> Self {
        self.glyph_gap = gap.max(0.0);
        self
    }

    /// Emphasis (primary accent fill or destructive red).
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self
    }

    pub fn danger(mut self) -> Self {
        self.variant = ButtonVariant::Danger;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl<M: Clone + 'static> Widget<M> for Button<M> {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
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

    observer_methods!();

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let pal = ctx.palette;
        self.surface
            .fill(ctx.canvas, self.rect, state_fill(&pal, state, self.variant));
        let color = state_text(&pal, state, self.variant);
        match self.glyph {
            Some(glyph) => {
                let glyph_rect = Rect::new(
                    self.rect.left + 8.0,
                    self.rect.top,
                    self.rect.left + 34.0,
                    self.rect.bottom,
                );
                let mut buf = [0u16; 2];
                let text = String::from_utf16_lossy(&*glyph.encode_utf16(&mut buf));
                ctx.text(
                    glyph_rect,
                    &text,
                    TextStyleRole::Glyph,
                    TextAlign::Center,
                    color,
                );
                let label_rect = Rect::new(
                    glyph_rect.right + self.glyph_gap,
                    self.rect.top,
                    self.rect.right - 6.0,
                    self.rect.bottom,
                );
                ctx.text(
                    label_rect,
                    &self.label,
                    TextStyleRole::Small,
                    TextAlign::Left,
                    color,
                );
            }
            None => {
                ctx.text(
                    self.rect.inset(10.0, 0.0),
                    &self.label,
                    TextStyleRole::Small,
                    TextAlign::Center,
                    color,
                );
            }
        }
    }

    fn focus_ring_radius(&self) -> Option<f32> {
        self.surface.focus_radius()
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        match input {
            WidgetInput::ClickAt(_) | WidgetInput::Activate if self.enabled => {
                Some(self.message.clone())
            }
            _ => None,
        }
    }

    fn set_label(&mut self, label: String) {
        self.label = label;
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let name = if self.label.is_empty() {
            self.id.0.to_string()
        } else {
            self.label.clone()
        };
        Some(A11yInfo::new(A11yRole::Button, name))
    }
}

observer_builders!(Button);
surface_builders!(Button);

/// Which side of the switch track carries the label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LabelSide {
    #[default]
    Start,
    End,
}

pub struct Toggle<M> {
    id: WidgetId,
    rect: Rect,
    label: String,
    label_side: LabelSide,
    on_label: String,
    off_label: String,
    on: bool,
    enabled: bool,
    on_change: Box<dyn Fn(bool) -> M>,
    observers: Observers<M>,
}

impl<M> Toggle<M> {
    pub fn new(
        id: &'static str,
        label: impl Into<String>,
        on: bool,
        on_change: impl Fn(bool) -> M + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 160.0, 36.0),
            label: label.into(),
            label_side: LabelSide::Start,
            on_label: String::new(),
            off_label: String::new(),
            on,
            enabled: true,
            on_change: Box::new(on_change),
            observers: Observers::new(),
        }
    }

    /// Put the label after the track instead of before it.
    pub fn label_side(mut self, side: LabelSide) -> Self {
        self.label_side = side;
        self
    }

    /// Text painted inside the track while on (`"开"`, `"On"`).
    pub fn on_label(mut self, label: impl Into<String>) -> Self {
        self.on_label = label.into();
        self
    }

    /// Text painted inside the track while off (`"关"`, `"Off"`).
    pub fn off_label(mut self, label: impl Into<String>) -> Self {
        self.off_label = label.into();
        self
    }

    /// Switch track. Kept 2 DIP from the edge so the focus ring can sit just
    /// outside it without crossing the painted track.
    fn track_rect(&self) -> Rect {
        let track_w = 44.0;
        let track_h = 22.0;
        let center = self.rect.center().y;
        let (left, right) = match self.label_side {
            LabelSide::Start => (self.rect.right - 2.0 - track_w, self.rect.right - 2.0),
            LabelSide::End => (self.rect.left + 2.0, self.rect.left + 2.0 + track_w),
        };
        Rect::new(left, center - track_h * 0.5, right, center + track_h * 0.5)
    }

    fn label_rect(&self, track: Rect) -> Rect {
        match self.label_side {
            LabelSide::Start => Rect::new(
                self.rect.left,
                self.rect.top,
                track.left - 10.0,
                self.rect.bottom,
            ),
            LabelSide::End => Rect::new(
                track.right + 10.0,
                self.rect.top,
                self.rect.right,
                self.rect.bottom,
            ),
        }
    }
}

impl<M: 'static> Widget<M> for Toggle<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::TOGGLED
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

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn set_toggle(&mut self, on: bool) {
        self.on = on;
    }

    fn focus_inset(&self) -> f32 {
        0.0
    }

    fn focus_ring_rect(&self) -> Rect {
        self.track_rect().inflate(2.0, 2.0)
    }

    /// The switch track is a pill; the ring follows it.
    fn focus_ring_radius(&self) -> Option<f32> {
        Some(self.focus_ring_rect().height() / 2.0)
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(A11yInfo::new(A11yRole::Toggle, self.label.clone()).checked(self.on))
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let pal = ctx.palette;
        let track = self.track_rect();
        ctx.text(
            self.label_rect(track),
            &self.label,
            TextStyleRole::Small,
            TextAlign::Left,
            if self.enabled { pal.text } else { pal.text_disabled },
        );
        let track_color = if !self.enabled {
            pal.button_disabled
        } else if self.on {
            pal.accent
        } else if state.hovered {
            pal.button_hover
        } else {
            pal.button_bg
        };
        ctx.canvas.fill_rect(track, track_color);
        let inside = if self.on {
            &self.on_label
        } else {
            &self.off_label
        };
        if !inside.is_empty() {
            let label = if self.on {
                Rect::new(track.left + 2.0, track.top, track.right - 20.0, track.bottom)
            } else {
                Rect::new(track.left + 20.0, track.top, track.right - 2.0, track.bottom)
            };
            let color = if !self.enabled {
                pal.text_disabled
            } else if self.on {
                pal.background
            } else {
                pal.text
            };
            ctx.text(
                label,
                inside,
                TextStyleRole::Small,
                TextAlign::Center,
                color,
            );
        }
        let knob = Rect::from_xywh(
            if self.on {
                track.right - 20.0
            } else {
                track.left + 2.0
            },
            track.top + 2.0,
            18.0,
            18.0,
        );
        let knob_color = if self.enabled {
            if self.on { pal.background } else { pal.muted }
        } else {
            pal.text_disabled
        };
        ctx.canvas.fill_rect(knob, knob_color);
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        match input {
            WidgetInput::ClickAt(_) | WidgetInput::Activate if self.enabled => {
                self.on = !self.on;
                Some((self.on_change)(self.on))
            }
            _ => None,
        }
    }
}

observer_builders!(Toggle);

pub struct Segmented<M> {
    id: WidgetId,
    rect: Rect,
    options: Vec<String>,
    /// Per-option disabled flags; disabled options cannot be selected by
    /// pointer, keyboard or assistive technology.
    disabled: Vec<bool>,
    selected: usize,
    /// Hovered cell; per-cell feedback instead of tinting the whole control.
    hover: Option<usize>,
    /// Cells touched by the last hover change, so a crossing repaints two
    /// cells instead of the whole strip.
    repaint_pending: Option<Rect>,
    enabled: bool,
    on_select: Box<dyn Fn(usize) -> M>,
    observers: Observers<M>,
}

impl<M> Segmented<M> {
    pub fn new(
        id: &'static str,
        options: &[&'static str],
        selected: usize,
        on_select: impl Fn(usize) -> M + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 300.0, 32.0),
            options: options.iter().map(|option| (*option).to_string()).collect(),
            disabled: vec![false; options.len()],
            selected,
            hover: None,
            repaint_pending: None,
            enabled: true,
            on_select: Box::new(on_select),
            observers: Observers::new(),
        }
    }

    /// Disable one option (builder form).
    pub fn disable(mut self, index: usize) -> Self {
        self.set_option_disabled(index, true);
        self
    }

    /// Runtime per-option enabled state. Returns whether it changed.
    pub fn set_option_disabled(&mut self, index: usize, disabled: bool) -> bool {
        let Some(slot) = self.disabled.get_mut(index) else {
            return false;
        };
        if *slot == disabled {
            return false;
        }
        *slot = disabled;
        if disabled && self.hover == Some(index) {
            self.hover = None;
        }
        true
    }

    pub fn option_enabled(&self, index: usize) -> bool {
        self.disabled.get(index).map(|disabled| !disabled).unwrap_or(false)
    }

    /// Hovered cell index (typed access / diagnostics).
    pub fn hovered(&self) -> Option<usize> {
        self.hover
    }

    fn cell_at(&self, pos: Point) -> Option<usize> {
        if self.options.is_empty() || !self.rect.contains_point(pos) {
            return None;
        }
        let cell = self.rect.width() / self.options.len() as f32;
        let index = ((pos.x - self.rect.left) / cell).floor().max(0.0) as usize;
        Some(index.min(self.options.len() - 1))
    }

    fn cell_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.options.len() {
            return None;
        }
        let cell = self.rect.width() / self.options.len() as f32;
        Some(Rect::new(
            self.rect.left + index as f32 * cell,
            self.rect.top,
            self.rect.left + (index + 1) as f32 * cell,
            self.rect.bottom,
        ))
    }

    /// Remember the cells touched when the hovered option changes.
    fn note_hover_change(&mut self, next: Option<usize>) {
        if next == self.hover {
            return;
        }
        let old = self.hover.and_then(|index| self.cell_rect(index));
        let new = next.and_then(|index| self.cell_rect(index));
        self.repaint_pending = match (old, new) {
            (Some(a), Some(b)) => Some(a.union(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
    }

    /// Selected option index (typed access seam).
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The option labels (typed access).
    pub fn options(&self) -> &[String] {
        &self.options
    }

    /// Replace the option labels (silent). Per-option disabled flags keep
    /// their positions for the options that remain; the selection clamps and
    /// the hover clears. Returns whether the list changed.
    pub fn set_options(&mut self, options: Vec<String>) -> bool {
        if self.options == options {
            return false;
        }
        self.options = options;
        self.disabled.resize(self.options.len(), false);
        self.selected = self.selected.min(self.options.len().saturating_sub(1));
        self.hover = None;
        true
    }

    /// Next enabled option from `from` in `delta` direction, wrapping.
    fn next_enabled(&self, from: usize, delta: i32) -> Option<usize> {
        let len = self.options.len();
        if len == 0 {
            return None;
        }
        let mut index = from as i32;
        for _ in 0..len {
            index = (index + delta).rem_euclid(len as i32);
            if self.option_enabled(index as usize) {
                return Some(index as usize);
            }
        }
        None
    }

    fn first_enabled(&self) -> Option<usize> {
        (0..self.options.len()).find(|&index| self.option_enabled(index))
    }

    fn last_enabled(&self) -> Option<usize> {
        (0..self.options.len())
            .rev()
            .find(|&index| self.option_enabled(index))
    }

    /// Move the selection by `delta` with wrap-around (radio-group behaviour),
    /// skipping disabled options.
    fn step(&mut self, delta: i32) -> Option<M> {
        if self.options.is_empty() {
            return None;
        }
        let next = self.next_enabled(self.selected.min(self.options.len() - 1), delta)?;
        self.select_index(next)
    }

    fn select_index(&mut self, index: usize) -> Option<M> {
        if index >= self.options.len() || index == self.selected || !self.option_enabled(index) {
            return None;
        }
        self.selected = index;
        Some((self.on_select)(index))
    }

    /// Select an option through the normal path: disabled options and
    /// out-of-range indices select nothing. Returns the widget's message.
    pub fn select(&mut self, index: usize) -> Option<M> {
        self.select_index(index)
    }

    /// Cell rectangle of the selected option (keyboard tooltip anchor).
    fn selected_cell(&self) -> Option<Rect> {
        if self.options.is_empty() {
            return None;
        }
        let cell = self.rect.width() / self.options.len() as f32;
        let index = self.selected.min(self.options.len() - 1);
        Some(Rect::new(
            self.rect.left + index as f32 * cell,
            self.rect.top,
            self.rect.left + (index + 1) as f32 * cell - 1.0,
            self.rect.bottom,
        ))
    }

    fn a11y_children(&self) -> Vec<A11yChild> {
        if self.options.is_empty() {
            return Vec::new();
        }
        let cell = self.rect.width() / self.options.len() as f32;
        self.options
            .iter()
            .enumerate()
            .map(|(index, label)| {
                A11yChild::new(
                    index,
                    label.as_str(),
                    Rect::new(
                        self.rect.left + index as f32 * cell,
                        self.rect.top,
                        self.rect.left + (index + 1) as f32 * cell - 1.0,
                        self.rect.bottom,
                    ),
                )
                .selected(index == self.selected)
                .enabled(self.option_enabled(index))
            })
            .collect()
    }
}

impl<M: 'static> Widget<M> for Segmented<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
    }

    fn skin_children(&self) -> &'static [&'static str] {
        OPTION_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(valid_skin_option)
    }

    /// `<option label>` elements replace the option labels; per-option
    /// disabled state stays application-owned and `selected` applies after.
    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        self.set_options(skin_option_labels(children));
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

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover = None;
        }
    }

    fn set_selected(&mut self, index: usize) {
        self.selected = index;
    }

    fn adjustable(&self) -> bool {
        true
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    /// Repaint only when the hovered option changes; moving inside one cell
    /// used to repaint the whole segmented control on every mouse event.
    fn repaint_on_move_at(&self, pos: Point) -> bool {
        self.cell_at(pos).filter(|index| self.option_enabled(*index)) != self.hover
    }

    fn take_repaint_hint(&mut self) -> Option<Rect> {
        self.repaint_pending.take()
    }

    fn tooltip_rect(&self) -> Rect {
        self.selected_cell().unwrap_or(self.rect)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.cell_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        (!self.options.is_empty()).then_some(self.selected.min(self.options.len() - 1))
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::RadioGroup, self.id.0)
                .children(self.a11y_children()),
        )
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        if !self.enabled {
            return None;
        }
        self.select_index(index)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let pal = ctx.palette;
        if self.options.is_empty() {
            return;
        }
        let cell = self.rect.width() / self.options.len() as f32;
        for (index, label) in self.options.iter().enumerate() {
            let cell_rect = Rect::new(
                self.rect.left + index as f32 * cell,
                self.rect.top,
                self.rect.left + (index + 1) as f32 * cell - 1.0,
                self.rect.bottom,
            );
            let selected = index == self.selected;
            let usable = self.enabled && self.option_enabled(index);
            let hovered = usable && self.hover == Some(index);
            let fill = if !usable {
                pal.button_disabled
            } else if selected {
                pal.button_active
            } else if hovered {
                pal.row_hover
            } else {
                pal.background
            };
            ctx.canvas
                .frame(cell_rect, pal.divider, fill);
            let color = if !usable {
                pal.text_disabled
            } else if selected {
                pal.accent_text
            } else if hovered {
                pal.text
            } else {
                pal.muted
            };
            ctx.text(
                cell_rect,
                label,
                TextStyleRole::Small,
                TextAlign::Center,
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
                let next = self
                    .cell_at(pos)
                    .filter(|index| self.option_enabled(*index));
                self.note_hover_change(next);
                self.hover = next;
                None
            }
            WidgetInput::HoverLeave => {
                self.repaint_pending = self.hover.and_then(|index| self.cell_rect(index));
                self.hover = None;
                None
            }
            WidgetInput::ClickAt(pos) => {
                let index = self.cell_at(pos)?;
                if !self.option_enabled(index) {
                    return None;
                }
                self.selected = index;
                Some((self.on_select)(index))
            }
            WidgetInput::Activate => {
                self.option_enabled(self.selected)
                    .then(|| (self.on_select)(self.selected))
            }
            WidgetInput::Key(key) => match key.code {
                crate::event::KeyCode::Left | crate::event::KeyCode::Up => self.step(-1),
                crate::event::KeyCode::Right | crate::event::KeyCode::Down => self.step(1),
                crate::event::KeyCode::Home => self
                    .first_enabled()
                    .and_then(|index| self.select_index(index)),
                crate::event::KeyCode::End => self
                    .last_enabled()
                    .and_then(|index| self.select_index(index)),
                _ => None,
            },
            _ => None,
        }
    }
}

observer_builders!(Segmented);

pub struct Tabs<M> {
    id: WidgetId,
    rect: Rect,
    labels: Vec<String>,
    selected: usize,
    /// Hovered tab cell; per-cell feedback instead of tinting the whole strip.
    hover: Option<usize>,
    /// Cells touched by the last hover change, so a crossing repaints two
    /// cells instead of the whole strip.
    repaint_pending: Option<Rect>,
    /// Whether each tab paints a close target that emits `on_close`.
    closable: bool,
    on_close: Option<Box<dyn Fn(usize) -> M>>,
    /// Per-tab notification badge (counts, dot labels).
    badges: Vec<Option<String>>,
    /// Label alignment inside the tab cell. Centered by default; the badge
    /// and close target float on the right without moving the label.
    label_align: TextAlign,
    enabled: bool,
    on_select: Box<dyn Fn(usize) -> M>,
    observers: Observers<M>,
}

impl<M> Tabs<M> {
    pub fn new(
        id: &'static str,
        labels: &[&'static str],
        selected: usize,
        on_select: impl Fn(usize) -> M + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 360.0, 32.0),
            labels: labels.iter().map(|label| (*label).to_string()).collect(),
            selected,
            hover: None,
            repaint_pending: None,
            closable: false,
            on_close: None,
            badges: vec![None; labels.len()],
            label_align: TextAlign::Center,
            enabled: true,
            on_select: Box::new(on_select),
            observers: Observers::new(),
        }
    }

    /// Paint a close target on every tab; clicking it (or Delete on the
    /// selected tab) emits the index through `on_close` instead of selecting.
    pub fn closable(mut self, on_close: impl Fn(usize) -> M + 'static) -> Self {
        self.closable = true;
        self.on_close = Some(Box::new(on_close));
        self
    }

    /// Attach a badge (a count or short label) to one tab.
    pub fn badge(mut self, index: usize, text: impl Into<String>) -> Self {
        if self.badges.len() <= index {
            self.badges.resize(index + 1, None);
        }
        self.badges[index] = Some(text.into());
        self
    }

    /// Set or clear a tab's badge at runtime (a live notification count).
    /// Returns whether the model changed, so the caller can repaint.
    pub fn set_badge(&mut self, index: usize, text: Option<String>) -> bool {
        if index >= self.labels.len() {
            return false;
        }
        if self.badges.len() <= index {
            self.badges.resize(index + 1, None);
        }
        if self.badges[index] == text {
            return false;
        }
        self.badges[index] = text;
        true
    }

    pub fn badge_text(&self, index: usize) -> Option<&str> {
        self.badges
            .get(index)
            .and_then(|badge| badge.as_deref())
            .filter(|text| !text.is_empty())
    }

    pub fn is_closable(&self) -> bool {
        self.closable
    }

    /// Label alignment inside the tab cell (default `Center`). Centered labels
    /// stay centered when a badge or close target occupies the right edge;
    /// left/right alignment stops before that chrome.
    pub fn label_align(mut self, align: TextAlign) -> Self {
        self.label_align = align;
        self
    }

    pub fn label_alignment(&self) -> TextAlign {
        self.label_align
    }

    /// Runtime label alignment inside the tab cells. Returns whether it
    /// changed.
    pub fn set_label_align(&mut self, align: TextAlign) -> bool {
        if self.label_align == align {
            return false;
        }
        self.label_align = align;
        true
    }

    /// Text rectangle for one tab. The default centered layout uses the whole
    /// cell, so a right-side badge never pulls the label off center.
    fn label_rect(&self, cell: Rect, content_right: f32) -> Rect {
        if self.label_align == TextAlign::Center || content_right >= cell.right - 9.0 {
            cell
        } else {
            Rect::new(cell.left, cell.top, content_right, cell.bottom)
        }
    }

    /// Hovered tab cell index (typed access / diagnostics).
    pub fn hovered(&self) -> Option<usize> {
        self.hover
    }

    fn cell_at(&self, pos: Point) -> Option<usize> {
        if self.labels.is_empty() || !self.rect.contains_point(pos) {
            return None;
        }
        let cell = self.rect.width() / self.labels.len() as f32;
        let index = ((pos.x - self.rect.left) / cell).floor().max(0.0) as usize;
        Some(index.min(self.labels.len() - 1))
    }

    fn cell_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.labels.len() {
            return None;
        }
        let cell = self.rect.width() / self.labels.len() as f32;
        Some(Rect::new(
            self.rect.left + index as f32 * cell,
            self.rect.top,
            self.rect.left + (index + 1) as f32 * cell,
            self.rect.bottom,
        ))
    }

    /// Remember the cells touched when the hovered tab changes.
    fn note_hover_change(&mut self, next: Option<usize>) {
        if next == self.hover {
            return;
        }
        let old = self.hover.and_then(|index| self.cell_rect(index));
        let new = next.and_then(|index| self.cell_rect(index));
        self.repaint_pending = match (old, new) {
            (Some(a), Some(b)) => Some(a.union(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
    }

    /// Close target inside one tab; `None` when the tabs are not closable.
    fn close_rect(&self, index: usize) -> Option<Rect> {
        if !self.closable {
            return None;
        }
        let cell = self.cell_rect(index)?;
        let center = cell.center().y;
        Some(Rect::new(
            cell.right - 16.0,
            center - 6.0,
            cell.right - 4.0,
            center + 6.0,
        ))
    }

    /// Selected tab index (typed access seam).
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The tab labels (typed access).
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Replace the tab labels (silent). Badges keep their positions for the
    /// tabs that remain, the selection clamps and the hover clears. Returns
    /// whether the list changed.
    pub fn set_labels(&mut self, labels: Vec<String>) -> bool {
        if self.labels == labels {
            return false;
        }
        self.labels = labels;
        self.badges.resize(self.labels.len(), None);
        self.selected = self.selected.min(self.labels.len().saturating_sub(1));
        self.hover = None;
        true
    }

    fn step(&mut self, delta: i32) -> Option<M> {
        if self.labels.is_empty() {
            return None;
        }
        let len = self.labels.len() as i32;
        let next = (self.selected as i32 + delta).rem_euclid(len) as usize;
        self.select_index(next)
    }

    fn select_index(&mut self, index: usize) -> Option<M> {
        if index >= self.labels.len() || index == self.selected {
            return None;
        }
        self.selected = index;
        Some((self.on_select)(index))
    }

    /// Select a tab through the normal path and return the widget's message.
    pub fn select(&mut self, index: usize) -> Option<M> {
        self.select_index(index)
    }

    /// Cell rectangle of the selected tab (keyboard tooltip anchor).
    fn selected_cell(&self) -> Option<Rect> {
        if self.labels.is_empty() {
            return None;
        }
        let cell = self.rect.width() / self.labels.len() as f32;
        let index = self.selected.min(self.labels.len() - 1);
        Some(Rect::new(
            self.rect.left + index as f32 * cell,
            self.rect.top,
            self.rect.left + (index + 1) as f32 * cell,
            self.rect.bottom,
        ))
    }

    fn a11y_children(&self) -> Vec<A11yChild> {
        if self.labels.is_empty() {
            return Vec::new();
        }
        let cell = self.rect.width() / self.labels.len() as f32;
        self.labels
            .iter()
            .enumerate()
            .map(|(index, label)| {
                A11yChild::new(
                    index,
                    label.as_str(),
                    Rect::new(
                        self.rect.left + index as f32 * cell,
                        self.rect.top,
                        self.rect.left + (index + 1) as f32 * cell,
                        self.rect.bottom,
                    ),
                )
                .selected(index == self.selected)
            })
            .collect()
    }
}

impl<M: 'static> Widget<M> for Tabs<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        TABS_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("label-align", SkinValue::Text(align)) => {
                matches!(*align, "left" | "center" | "right")
            }
            _ => false,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        if let ("label-align", SkinValue::Text(align)) = (name, value) {
            self.set_label_align(match *align {
                "left" => TextAlign::Left,
                "right" => TextAlign::Right,
                _ => TextAlign::Center,
            });
        }
    }

    fn skin_children(&self) -> &'static [&'static str] {
        OPTION_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(valid_skin_option)
    }

    /// `<option label>` elements replace the tab labels; badges keep their
    /// positions for the tabs that remain and `selected` applies after.
    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        self.set_labels(skin_option_labels(children));
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

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover = None;
        }
    }

    fn set_selected(&mut self, index: usize) {
        self.selected = index;
    }

    fn adjustable(&self) -> bool {
        true
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    /// Repaint only when the hovered tab changes; moving inside one tab used
    /// to repaint the whole tab strip on every mouse event.
    fn repaint_on_move_at(&self, pos: Point) -> bool {
        self.cell_at(pos) != self.hover
    }

    fn take_repaint_hint(&mut self) -> Option<Rect> {
        self.repaint_pending.take()
    }

    fn tooltip_rect(&self) -> Rect {
        self.selected_cell().unwrap_or(self.rect)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.cell_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        (!self.labels.is_empty()).then_some(self.selected.min(self.labels.len() - 1))
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::TabList, self.id.0)
                .children(self.a11y_children()),
        )
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        if !self.enabled {
            return None;
        }
        self.select_index(index)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let pal = ctx.palette;
        if self.labels.is_empty() {
            return;
        }
        let cell = self.rect.width() / self.labels.len() as f32;
        ctx.canvas.fill_rect(
            Rect::new(self.rect.left, self.rect.bottom - 1.0, self.rect.right, self.rect.bottom),
            pal.divider,
        );
        for (index, label) in self.labels.iter().enumerate() {
            let cell_rect = Rect::new(
                self.rect.left + index as f32 * cell,
                self.rect.top,
                self.rect.left + (index + 1) as f32 * cell,
                self.rect.bottom - 2.0,
            );
            let selected = index == self.selected;
            let hovered = self.enabled && self.hover == Some(index);
            if !selected && hovered {
                ctx.canvas.fill_rect(cell_rect, pal.row_hover);
            }
            if selected {
                ctx.canvas.fill_rect(
                    Rect::new(
                        cell_rect.left + 8.0,
                        cell_rect.bottom,
                        cell_rect.right - 8.0,
                        self.rect.bottom,
                    ),
                    pal.accent,
                );
            }
            let color = if !self.enabled {
                pal.text_disabled
            } else if selected || hovered {
                pal.text
            } else {
                pal.muted
            };
            // Right-side chrome: close target, then the badge, then the label
            // in whatever space remains.
            let mut content_right = cell_rect.right - 8.0;
            if let Some(close) = self.close_rect(index) {
                let mut buf = [0u16; 1];
                let text = String::from_utf16_lossy(&*'\u{00D7}'.encode_utf16(&mut buf));
                ctx.text(
                    close,
                    &text,
                    TextStyleRole::Small,
                    TextAlign::Center,
                    if hovered { pal.text } else { pal.muted },
                );
                content_right = close.left - 2.0;
            }
            if let Some(badge) = self.badge_text(index) {
                let width = ctx.measure_text(badge, TextStyleRole::Small).width + 10.0;
                let badge_rect = Rect::new(
                    content_right - width,
                    cell_rect.top + 6.0,
                    content_right,
                    cell_rect.bottom - 8.0,
                );
                ctx.canvas.fill_rect(badge_rect, pal.accent);
                ctx.text(
                    badge_rect,
                    badge,
                    TextStyleRole::Small,
                    TextAlign::Center,
                    pal.background,
                );
                content_right = badge_rect.left - 4.0;
            }
            let label_rect = self.label_rect(cell_rect, content_right);
            ctx.text(
                label_rect,
                label,
                TextStyleRole::Small,
                self.label_align,
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
                let next = self.cell_at(pos);
                self.note_hover_change(next);
                self.hover = next;
                None
            }
            WidgetInput::HoverLeave => {
                self.repaint_pending = self.hover.and_then(|index| self.cell_rect(index));
                self.hover = None;
                None
            }
            WidgetInput::ClickAt(pos) => {
                let index = self.cell_at(pos)?;
                if let Some(close) = self.close_rect(index)
                    && close.contains_point(pos)
                {
                    return self.on_close.as_ref().map(|handler| handler(index));
                }
                self.selected = index;
                Some((self.on_select)(index))
            }
            WidgetInput::Activate => Some((self.on_select)(self.selected)),
            WidgetInput::Key(key) => match key.code {
                crate::event::KeyCode::Left | crate::event::KeyCode::Up => self.step(-1),
                crate::event::KeyCode::Right | crate::event::KeyCode::Down => self.step(1),
                crate::event::KeyCode::Home => self.select_index(0),
                crate::event::KeyCode::End => {
                    self.select_index(self.labels.len().saturating_sub(1))
                }
                crate::event::KeyCode::Delete if self.closable && !self.labels.is_empty() => {
                    self.on_close.as_ref().map(|handler| {
                        handler(self.selected.min(self.labels.len() - 1))
                    })
                }
                _ => None,
            },
            _ => None,
        }
    }
}

observer_builders!(Tabs);

pub struct Slider<M> {
    id: WidgetId,
    rect: Rect,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    /// Snap pointer drags and UIA `SetValue` to `step` (keyboard steps already
    /// use it). Off by default, so an ordinary slider stays continuous.
    snap: bool,
    /// Evenly spaced tick marks under the track (`0` hides them). Display
    /// only: the value grid is `step`/`snap`/`discrete`.
    ticks: usize,
    /// Explicit display positions for tick marks; overrides `ticks` when set.
    /// Independent of the value step.
    tick_values: Vec<f32>,
    /// Labels evenly spaced across the track below the ticks.
    tick_labels: Vec<String>,
    /// Optional formatted value label above the track (`0.6` → `"60%"`).
    value_text: Option<Box<dyn Fn(f32) -> String>>,
    enabled: bool,
    dragging: bool,
    preview: f32,
    on_change: Box<dyn Fn(f32) -> M>,
    /// Pointer-drag lifecycle observers. `on_drag_start` fires once on the
    /// press, `on_drag` on every move while the button is held (so an
    /// application can preview the value), and `on_drag_end` on the release
    /// beside the usual `on_change` report. All optional and additive.
    on_drag_start: Option<Box<dyn Fn(f32) -> M>>,
    on_drag: Option<Box<dyn Fn(f32) -> M>>,
    on_drag_end: Option<Box<dyn Fn(f32) -> M>>,
    /// Message reported beside the primary one (`on_drag_end` next to
    /// `on_change`); drained by the container after the input.
    extra: Option<M>,
    observers: Observers<M>,
    surface: SurfaceStyle,
}

impl<M> Slider<M> {
    pub fn new(
        id: &'static str,
        value: f32,
        range: std::ops::RangeInclusive<f32>,
        on_change: impl Fn(f32) -> M + 'static,
    ) -> Self {
        let (min, max) = (*range.start(), *range.end());
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 32.0),
            value: value.clamp(min, max),
            min,
            max,
            step: (max - min) / 100.0,
            snap: false,
            ticks: 0,
            tick_values: Vec::new(),
            tick_labels: Vec::new(),
            value_text: None,
            enabled: true,
            dragging: false,
            preview: value.clamp(min, max),
            on_change: Box::new(on_change),
            on_drag_start: None,
            on_drag: None,
            on_drag_end: None,
            extra: None,
            observers: Observers::new(),
            surface: SurfaceStyle::new(),
        }
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Snap pointer drags and UIA `SetValue` to `step` (keyboard adjustment
    /// already steps). Application-driven [`Slider::set_value`] stays exact.
    pub fn snap(mut self, snap: bool) -> Self {
        self.snap = snap;
        self
    }

    /// Discrete slider in one call: `step` becomes the grid and pointer values
    /// snap to it (`0..=100` with `5.0` → 0, 5, 10, …, 100).
    pub fn discrete(mut self, step: f32) -> Self {
        if step.is_finite() && step > 0.0 {
            self.step = step;
            self.snap = true;
        }
        self
    }

    pub fn is_snapped(&self) -> bool {
        self.snap
    }

    /// Runtime snap toggle (typed access).
    pub fn set_snap(&mut self, snap: bool) -> bool {
        if self.snap == snap {
            return false;
        }
        self.snap = snap;
        true
    }

    /// Current increment/grid size.
    pub fn step_size(&self) -> f32 {
        self.step
    }

    /// Draw `count` evenly spaced tick marks under the track. Display only:
    /// the value grid is set separately with [`Slider::step`] +
    /// [`Slider::snap`] (or [`Slider::discrete`]).
    pub fn ticks(mut self, count: usize) -> Self {
        self.ticks = count;
        self
    }

    /// Draw tick marks at explicit values (clamped into the range). Display
    /// only and independent of the value step, so a slider can step by 5 while
    /// showing marks every 25.
    pub fn ticks_at(mut self, values: &[f32]) -> Self {
        self.tick_values = values.to_vec();
        self
    }

    /// Draw labels evenly spaced across the track; each label marks a tick.
    /// The widget asks for a taller cell while labels are present.
    pub fn tick_labels(mut self, labels: &[&str]) -> Self {
        self.tick_labels = labels.iter().map(|label| label.to_string()).collect();
        self
    }

    /// Format the current value into a label painted above the track.
    pub fn value_text(mut self, format: impl Fn(f32) -> String + 'static) -> Self {
        self.value_text = Some(Box::new(format));
        self
    }

    pub fn tick_count(&self) -> usize {
        self.ticks
    }

    /// Explicit tick positions set with [`Slider::ticks_at`].
    pub fn tick_values(&self) -> &[f32] {
        &self.tick_values
    }

    /// The formatted value label, if one is configured.
    pub fn value_label(&self) -> Option<String> {
        self.value_text.as_ref().map(|format| format(self.value))
    }

    /// Called once when a pointer drag begins, with the value under the press.
    /// Additive: the built-in drag preview still runs.
    pub fn on_drag_start(mut self, handler: impl Fn(f32) -> M + 'static) -> Self {
        self.on_drag_start = Some(Box::new(handler));
        self
    }

    /// Called on every pointer move while the button is held, with the value
    /// under the pointer. Lets the application follow/preview a drag instead
    /// of waiting for the release. Additive: the built-in preview still runs.
    pub fn on_drag(mut self, handler: impl Fn(f32) -> M + 'static) -> Self {
        self.on_drag = Some(Box::new(handler));
        self
    }

    /// Called when the drag ends, with the committed value. Reported beside
    /// the usual `on_change` message (queued after it).
    pub fn on_drag_end(mut self, handler: impl Fn(f32) -> M + 'static) -> Self {
        self.on_drag_end = Some(Box::new(handler));
        self
    }

    /// Whether a pointer drag is in progress (typed access).
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// The value the slider paints while dragging (typed access); `None` when
    /// no drag is in progress. The application-owned value is unchanged until
    /// the release reports it.
    pub fn drag_preview(&self) -> Option<f32> {
        self.dragging.then_some(self.preview)
    }

    /// Current value (typed access seam).
    pub fn value(&self) -> f32 {
        self.value
    }

    /// `(minimum, maximum)`.
    pub fn range(&self) -> (f32, f32) {
        (self.min, self.max)
    }

    /// Replace the range; the value and drag preview clamp into it. Emits
    /// nothing: the application decides whether to update its own state.
    pub fn set_range(&mut self, min: f32, max: f32) -> bool {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        if min == self.min && max == self.max {
            return false;
        }
        self.min = min;
        self.max = max;
        self.value = self.value.clamp(min, max);
        self.preview = self.value;
        true
    }

    fn span(&self) -> f32 {
        (self.max - self.min).max(f32::EPSILON)
    }

    /// Clamp to the range and, when snapping is on, round onto the `step` grid
    /// (measured from `min`).
    fn quantize(&self, value: f32) -> f32 {
        let value = value.clamp(self.min, self.max);
        if self.snap && self.step.is_finite() && self.step > 0.0 {
            let steps = ((value - self.min) / self.step).round();
            (self.min + steps * self.step).clamp(self.min, self.max)
        } else {
            value
        }
    }

    fn value_at(&self, x: f32) -> f32 {
        let inner_left = self.rect.left + 8.0;
        let inner_right = self.rect.right - 8.0;
        let t = ((x - inner_left) / (inner_right - inner_left).max(1.0)).clamp(0.0, 1.0);
        self.quantize(self.min + t * self.span())
    }

    fn shown(&self) -> f32 {
        if self.dragging {
            self.preview
        } else {
            self.value
        }
    }

    /// Move the value by `direction` steps (adjust mode).
    fn adjust(&mut self, direction: i32) -> Option<M> {
        let value = (self.value + self.step * direction as f32).clamp(self.min, self.max);
        if (value - self.value).abs() < f32::EPSILON {
            return None;
        }
        self.value = value;
        self.preview = value;
        Some((self.on_change)(value))
    }
}

impl<M: 'static> Widget<M> for Slider<M> {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
    }

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

    observer_methods!();

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        if self.tick_labels.is_empty() { 32.0 } else { 44.0 }
    }

    fn adjustable(&self) -> bool {
        true
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    fn set_value(&mut self, value: f32) {
        self.value = value.clamp(self.min, self.max);
        self.preview = self.value;
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let mut info = A11yInfo::new(A11yRole::Slider, self.id.0)
            .range(self.min as f64, self.max as f64, self.value as f64);
        if let Some(format) = &self.value_text {
            info = info.value(format(self.value));
        }
        Some(info)
    }

    fn a11y_set_value(&mut self, text: &str) -> Option<M> {
        if !self.enabled {
            return None;
        }
        let parsed: f32 = text.trim().parse().ok()?;
        let value = self.quantize(parsed);
        self.value = value;
        self.preview = value;
        Some((self.on_change)(value))
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let pal = ctx.palette;
        let has_labels = !self.tick_labels.is_empty();
        let center_y = if has_labels {
            self.rect.top + 14.0
        } else {
            self.rect.center().y
        };
        if let Some(format) = &self.value_text {
            ctx.text(
                Rect::new(self.rect.left, self.rect.top, self.rect.right, self.rect.top + 14.0),
                &format(self.shown()),
                TextStyleRole::Small,
                TextAlign::Right,
                pal.muted,
            );
        }
        let thickness = if state.hovered || state.pressed || state.adjusting {
            6.0
        } else {
            4.0
        };
        let track = Rect::new(
            self.rect.left + 8.0,
            center_y - thickness * 0.5,
            self.rect.right - 8.0,
            center_y + thickness * 0.5,
        );
        // The surface styles the track: `.corner_radius` rounds it (and the
        // fill), `.background` overrides the groove color and
        // `.transparent_background` hides the groove (a bare fill).
        let track_color = if self.enabled {
            pal.scroll_track
        } else {
            pal.button_disabled
        };
        self.surface.fill(ctx.canvas, track, track_color);
        let ratio = ((self.shown() - self.min) / self.span()).clamp(0.0, 1.0);
        let filled = Rect::new(
            track.left,
            track.top,
            track.left + track.width() * ratio,
            track.bottom,
        );
        self.surface.shape(
            ctx.canvas,
            filled,
            if self.enabled { pal.accent } else { pal.text_disabled },
        );
        if !self.tick_values.is_empty() {
            let tick_top = track.bottom + 2.0;
            for value in &self.tick_values {
                let t = ((value - self.min) / self.span()).clamp(0.0, 1.0);
                let x = track.left + track.width() * t;
                ctx.canvas.fill_rect(
                    Rect::new(x, tick_top, x + 1.0, tick_top + 4.0),
                    if self.enabled { pal.muted } else { pal.text_disabled },
                );
            }
        } else if self.ticks > 0 {
            let tick_top = track.bottom + 2.0;
            for index in 0..=self.ticks {
                let t = index as f32 / self.ticks as f32;
                let x = track.left + track.width() * t;
                ctx.canvas.fill_rect(
                    Rect::new(x, tick_top, x + 1.0, tick_top + 4.0),
                    if self.enabled { pal.muted } else { pal.text_disabled },
                );
            }
        }
        if has_labels {
            let count = self.tick_labels.len();
            for (index, label) in self.tick_labels.iter().enumerate() {
                // Labels follow explicit tick values when the two lists line
                // up; otherwise they are evenly spaced.
                let t = if self.tick_values.len() == count && count > 0 {
                    ((self.tick_values[index] - self.min) / self.span()).clamp(0.0, 1.0)
                } else if count <= 1 {
                    0.0
                } else {
                    index as f32 / (count - 1) as f32
                };
                let x = track.left + track.width() * t;
                let rect = Rect::new(
                    (x - 40.0).max(self.rect.left),
                    self.rect.top + 24.0,
                    (x + 40.0).min(self.rect.right),
                    self.rect.bottom,
                );
                ctx.text(
                    rect,
                    label,
                    TextStyleRole::Small,
                    TextAlign::Center,
                    if self.enabled { pal.muted } else { pal.text_disabled },
                );
            }
        }
        let knob_size = if state.hovered || state.pressed { 16.0 } else { 12.0 };
        let knob_x = track.left + track.width() * ratio;
        let knob = Rect::new(
            knob_x - knob_size * 0.5,
            center_y - knob_size * 0.5,
            knob_x + knob_size * 0.5,
            center_y + knob_size * 0.5,
        );
        ctx.canvas.fill_rect(
            knob,
            if self.enabled { pal.text } else { pal.text_disabled },
        );
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::PressAt(pos) => {
                self.dragging = true;
                self.preview = self.value_at(pos.x);
                self.on_drag_start.as_ref().map(|handler| handler(self.preview))
            }
            WidgetInput::DragTo(pos) => {
                if self.dragging {
                    self.preview = self.value_at(pos.x);
                    self.on_drag.as_ref().map(|handler| handler(self.preview))
                } else {
                    None
                }
            }
            WidgetInput::ReleaseAt(pos) => {
                if !self.dragging {
                    return None;
                }
                self.dragging = false;
                self.value = self.value_at(pos.x);
                self.preview = self.value;
                if let Some(handler) = &self.on_drag_end {
                    self.extra = Some(handler(self.value));
                }
                Some((self.on_change)(self.value))
            }
            WidgetInput::Step(direction) => self.adjust(direction),
            WidgetInput::Key(key) => {
                let direction = match key.code {
                    crate::event::KeyCode::Up | crate::event::KeyCode::Right => 1,
                    crate::event::KeyCode::Down | crate::event::KeyCode::Left => -1,
                    crate::event::KeyCode::PageUp => 10,
                    crate::event::KeyCode::PageDown => -10,
                    _ => return None,
                };
                self.adjust(direction)
            }
            _ => None,
        }
    }

    fn take_extra_message(&mut self) -> Option<M> {
        self.extra.take()
    }
}

observer_builders!(Slider);
surface_builders!(Slider);

pub struct ProgressBar {
    id: WidgetId,
    rect: Rect,
    value: f32,
    label: String,
    /// Paint the percentage on the right edge.
    show_value: bool,
    /// Unknown duration: paint a moving segment instead of a fill.
    indeterminate: bool,
    /// Position of the indeterminate segment, `0..=1` (wrapped).
    phase: f32,
    surface: SurfaceStyle,
}

impl ProgressBar {
    pub fn new(id: &'static str, value: f32) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 200.0, 28.0),
            value: value.clamp(0.0, 1.0),
            label: String::new(),
            show_value: false,
            indeterminate: false,
            phase: 0.0,
            surface: SurfaceStyle::new(),
        }
    }

    /// Uniform corner radius of the track and fill, in DIPs.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.surface = self.surface.with_radius(radius);
        self
    }

    /// Per-corner radii.
    pub fn corners(mut self, corners: crate::geometry::Corners) -> Self {
        self.surface = self.surface.with_corners(corners);
        self
    }

    /// Override the track color.
    pub fn background(mut self, color: Color) -> Self {
        self.surface = self.surface.with_background(color);
        self
    }

    /// Suppress the track so only the fill shows.
    pub fn transparent_background(mut self, transparent: bool) -> Self {
        self.surface = self.surface.with_transparent(transparent);
        self
    }

    /// The configured surface (typed access).
    pub fn surface(&self) -> &SurfaceStyle {
        &self.surface
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Paint the current percentage next to the bar.
    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    /// Unknown progress: a moving segment rather than a determinate fill.
    /// Advance it from an application timer with [`ProgressBar::set_phase`].
    pub fn indeterminate(mut self) -> Self {
        self.indeterminate = true;
        self
    }

    /// Starting phase of the indeterminate segment.
    pub fn phase(mut self, phase: f32) -> Self {
        self.phase = phase.rem_euclid(1.0);
        self
    }

    /// Move the indeterminate segment. Returns whether the phase changed.
    pub fn set_phase(&mut self, phase: f32) -> bool {
        let phase = phase.rem_euclid(1.0);
        if (phase - self.phase).abs() < f32::EPSILON {
            return false;
        }
        self.phase = phase;
        true
    }

    pub fn is_indeterminate(&self) -> bool {
        self.indeterminate
    }

    /// Current progress in `0..=1` (typed access seam).
    pub fn value(&self) -> f32 {
        self.value
    }
}

impl<M> Widget<M> for ProgressBar {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
    }

    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::PROGRESS
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

    fn focusable(&self) -> bool {
        false
    }

    fn set_progress(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let pal = ctx.palette;
        let track = Rect::new(
            self.rect.left,
            self.rect.center().y - 3.0,
            self.rect.right,
            self.rect.center().y + 3.0,
        );
        self.surface.fill(ctx.canvas, track, pal.scroll_track);
        if self.indeterminate {
            let width = (track.width() * 0.3).max(24.0).min(track.width());
            let left = track.left + (track.width() - width) * self.phase;
            self.surface.shape(
                ctx.canvas,
                Rect::new(left, track.top, left + width, track.bottom),
                pal.accent,
            );
        } else {
            self.surface.shape(
                ctx.canvas,
                Rect::new(track.left, track.top, track.left + track.width() * self.value, track.bottom),
                pal.accent,
            );
        }
        if self.show_value && !self.indeterminate {
            ctx.text(
                self.rect,
                &format!("{:.0}%", self.value * 100.0),
                TextStyleRole::Small,
                TextAlign::Right,
                pal.muted,
            );
        } else if !self.label.is_empty() {
            ctx.text(
                self.rect,
                &self.label,
                TextStyleRole::Small,
                TextAlign::Right,
                pal.muted,
            );
        }
    }

    /// Determinate progress: RangeValue over 0..=1 with a percentage value
    /// text. An indeterminate bar reports a status instead of a range. Out of
    /// the tree until the instance opts in.
    fn a11y(&self) -> Option<A11yInfo> {
        let name = if self.label.is_empty() {
            self.id.0.to_string()
        } else {
            self.label.clone()
        };
        let info = A11yInfo::new(A11yRole::ProgressBar, name).display_only();
        Some(if self.indeterminate {
            info.value("进行中")
        } else {
            info.range(0.0, 1.0, self.value as f64)
                .value(format!("{:.0}%", self.value * 100.0))
        })
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }
}

static PAGER_SKIN_CHILDREN: &[&str] = &["page"];
const PAGE_SKIN_ATTRS: &[&str] = &["label"];

pub struct Pager<M> {
    id: WidgetId,
    rect: Rect,
    page: usize,
    pages: usize,
    enabled: bool,
    label: String,
    /// Per-page labels (a `<page>` child list); when present they replace the
    /// format/built-in label for the pages that have one.
    labels: Vec<String>,
    /// Custom label builder; receives the 1-based page and the page count.
    format: Option<Box<dyn Fn(usize, usize) -> String>>,
    on_nav: Box<dyn Fn(i32) -> M>,
}

impl<M> Pager<M> {
    pub fn new(
        id: &'static str,
        page: usize,
        pages: usize,
        on_nav: impl Fn(i32) -> M + 'static,
    ) -> Self {
        let mut pager = Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 360.0, 32.0),
            page,
            pages: pages.max(1),
            enabled: true,
            label: String::new(),
            labels: Vec::new(),
            format: None,
            on_nav: Box::new(on_nav),
        };
        pager.refresh_label();
        pager
    }

    /// Replace the default "第 N / M 页" label. The closure receives the
    /// 1-based page and the page count. A per-page label list (`set_page_labels`)
    /// is cleared: there is one label mechanism at a time.
    pub fn label_with(mut self, format: impl Fn(usize, usize) -> String + 'static) -> Self {
        self.labels.clear();
        self.format = Some(Box::new(format));
        self.refresh_label();
        self
    }

    /// Runtime label format swap (typed access). Always repaints.
    pub fn set_label_format(&mut self, format: impl Fn(usize, usize) -> String + 'static) -> bool {
        self.labels.clear();
        self.format = Some(Box::new(format));
        self.refresh_label();
        true
    }

    /// Drop the custom format and go back to the built-in label.
    pub fn clear_label_format(&mut self) -> bool {
        if self.format.take().is_none() {
            return false;
        }
        self.refresh_label();
        true
    }

    /// Install per-page labels (the `<page label>` child list): the page count
    /// becomes the list length and the painted label is the current page's
    /// entry. A custom format is dropped. Returns whether anything changed.
    pub fn set_page_labels(&mut self, labels: Vec<String>) -> bool {
        if self.labels == labels {
            return false;
        }
        self.format = None;
        self.labels = labels;
        self.pages = self.labels.len().max(1);
        self.page = self.page.min(self.pages - 1);
        self.refresh_label();
        true
    }

    /// The per-page labels as installed by `set_page_labels` (typed access).
    pub fn page_labels(&self) -> &[String] {
        &self.labels
    }

    /// The label currently painted (typed access / diagnostics).
    pub fn label_text(&self) -> &str {
        &self.label
    }

    fn refresh_label(&mut self) {
        if let Some(label) = self.labels.get(self.page) {
            self.label = label.clone();
            return;
        }
        self.label = match &self.format {
            Some(format) => format(self.page + 1, self.pages),
            None => format!("第 {} / {} 页", self.page + 1, self.pages),
        };
    }

    /// Current page (typed access seam).
    pub fn page(&self) -> usize {
        self.page
    }

    /// Page count.
    pub fn pages(&self) -> usize {
        self.pages
    }

    /// Change the page count, clamping the current page and refreshing the
    /// painted label. Per-page labels beyond the new count are dropped. Emits
    /// nothing.
    pub fn set_pages(&mut self, pages: usize) -> bool {
        let pages = pages.max(1);
        if pages == self.pages {
            return false;
        }
        self.pages = pages;
        self.page = self.page.min(pages - 1);
        self.labels.truncate(pages);
        self.refresh_label();
        true
    }

    fn zones(&self) -> (Rect, Rect) {
        let width = 80.0_f32.min(self.rect.width() * 0.3);
        (
            Rect::new(self.rect.left, self.rect.top, self.rect.left + width, self.rect.bottom),
            Rect::new(self.rect.right - width, self.rect.top, self.rect.right, self.rect.bottom),
        )
    }
}

impl<M: 'static> Widget<M> for Pager<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
    }

    fn skin_children(&self) -> &'static [&'static str] {
        PAGER_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(|child| {
            child.name == "page"
                && child
                    .attr("label")
                    .is_some_and(|label| !label.trim().is_empty())
                && child.attrs_are(PAGE_SKIN_ATTRS)
                && child.children.is_empty()
        })
    }

    /// `<page label>` elements install per-page labels; the page count becomes
    /// their number and `selected` (shared content) still applies afterwards.
    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        let labels = children
            .iter()
            .filter_map(|child| child.attr("label").map(str::to_string))
            .collect();
        self.set_page_labels(labels);
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

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_selected(&mut self, page: usize) {
        self.page = page.min(self.pages.saturating_sub(1));
        self.refresh_label();
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let pal = ctx.palette;
        let (prev, next) = self.zones();
        let can_prev = self.enabled && self.page > 0;
        let can_next = self.enabled && self.page + 1 < self.pages;
        let prev_state = WidgetState {
            enabled: can_prev,
            hovered: state.hovered,
            ..Default::default()
        };
        let next_state = WidgetState {
            enabled: can_next,
            hovered: state.hovered,
            ..Default::default()
        };
        for (zone, label, zone_state) in [
            (prev, "上一页", prev_state),
            (next, "下一页", next_state),
        ] {
            ctx.canvas.fill_rect(
                zone,
                if zone_state.enabled {
                    pal.button_bg
                } else {
                    pal.button_disabled
                },
            );
            ctx.text(
                zone,
                label,
                TextStyleRole::Small,
                TextAlign::Center,
                if zone_state.enabled {
                    pal.text
                } else {
                    pal.text_disabled
                },
            );
        }
        ctx.text(
            Rect::new(prev.right, self.rect.top, next.left, self.rect.bottom),
            &self.label,
            TextStyleRole::Small,
            TextAlign::Center,
            pal.muted,
        );
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        let (prev, next) = self.zones();
        match input {
            WidgetInput::ClickAt(pos) => {
                if pos.x <= prev.right && self.page > 0 {
                    Some((self.on_nav)(-1))
                } else if pos.x >= next.left && self.page + 1 < self.pages {
                    Some((self.on_nav)(1))
                } else {
                    None
                }
            }
            WidgetInput::Activate => Some((self.on_nav)(1)),
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let name = if self.label.is_empty() {
            self.id.0.to_string()
        } else {
            self.label.clone()
        };
        Some(
            A11yInfo::new(A11yRole::Button, name)
                .value(format!("{}/{}", self.page + 1, self.pages)),
        )
    }
}

pub struct ColorSwatch {
    id: WidgetId,
    rect: Rect,
    color: Color,
    label: String,
}

impl ColorSwatch {
    pub fn new(id: &'static str, color: Color, label: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 120.0, 32.0),
            color,
            label: label.into(),
        }
    }

    pub fn color(&self) -> Color {
        self.color
    }

    /// Replace the swatch color silently (the skin's `color` attribute).
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn set_label_text(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }
}

impl<M> Widget<M> for ColorSwatch {
    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        SWATCH_SKIN_ATTRS
    }

    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::LABEL
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        // A swatch is opaque; an RGBA value drops its alpha silently.
        if name == "color"
            && let Some(color) = value.as_color()
        {
            self.color = color;
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

    fn set_label(&mut self, label: String) {
        self.label = label;
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let pal = ctx.palette;
        let swatch = Rect::new(self.rect.left, self.rect.top, self.rect.left + 40.0, self.rect.bottom);
        ctx.canvas.frame(swatch, pal.explicit_border, self.color);
        let text_color = if self.color.luminance() > 0.55 {
            Color::new(20, 20, 20)
        } else {
            Color::new(240, 240, 240)
        };
        ctx.text(
            swatch.inset(4.0, 0.0),
            &self.label,
            TextStyleRole::Small,
            TextAlign::Center,
            text_color,
        );
    }

    /// A labeled swatch reads as text; the hex value goes in the description
    /// so a color-blind user still gets the exact color.
    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Text, self.label.clone())
                .description(format!(
                    "#{:02X}{:02X}{:02X}",
                    self.color.r, self.color.g, self.color.b
                ))
                .display_only(),
        )
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_focus_ring_wraps_the_track_inside_the_widget() {
        let mut toggle = Toggle::new("t", "开关", false, |on| on);
        toggle.set_rect(Rect::from_xywh(0.0, 0.0, 190.0, 36.0));
        let ring = toggle.focus_ring_rect();
        // Stays inside the layout cell so the dirty-rect blit covers it.
        assert!(ring.left >= 0.0 && ring.right <= 190.0);
        assert!(ring.top >= 0.0 && ring.bottom <= 36.0);
        // Encloses the 44 DIP track and leaves the label text alone.
        assert!(ring.width() >= 44.0);
        assert!(ring.left > 100.0);
        // The ring is a pill, not a rectangle.
        assert_eq!(toggle.focus_ring_radius(), Some(ring.height() / 2.0));
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
    fn tabs_badge_can_be_set_and_cleared_at_runtime() {
        let mut tabs: Tabs<usize> = Tabs::new("tabs", &["一", "二", "三"], 0, |index| index);
        assert_eq!(tabs.badge_text(2), None);
        assert!(tabs.set_badge(2, Some("新".to_string())));
        assert_eq!(tabs.badge_text(2), Some("新"));
        // Setting the same text is not a change.
        assert!(!tabs.set_badge(2, Some("新".to_string())));
        assert!(tabs.set_badge(2, None));
        assert_eq!(tabs.badge_text(2), None);
        // Out-of-range indices are ignored.
        assert!(!tabs.set_badge(7, Some("x".to_string())));
    }

    #[test]
    fn segmented_arrows_change_selection_and_wrap() {
        use crate::event::KeyCode;
        let mut segmented: Segmented<usize> =
            Segmented::new("seg", &["a", "b", "c"], 0, |index| index);
        assert!(segmented.adjustable());
        assert_eq!(segmented.input(key(KeyCode::Right)), Some(1));
        assert_eq!(segmented.input(key(KeyCode::Right)), Some(2));
        // Wrap-around, like a radio group.
        assert_eq!(segmented.input(key(KeyCode::Right)), Some(0));
        assert_eq!(segmented.input(key(KeyCode::End)), Some(2));
        assert_eq!(segmented.input(key(KeyCode::Home)), Some(0));
        assert_eq!(segmented.input(key(KeyCode::Left)), Some(2));
    }

    #[test]
    fn tabs_arrows_change_selection() {
        use crate::event::KeyCode;
        let mut tabs: Tabs<usize> = Tabs::new("tabs", &["a", "b", "c"], 1, |index| index);
        assert!(tabs.adjustable());
        assert_eq!(tabs.input(key(KeyCode::Right)), Some(2));
        assert_eq!(tabs.input(key(KeyCode::Left)), Some(1));
        // Selecting the current index emits nothing.
        assert_eq!(tabs.input(key(KeyCode::Home)), Some(0));
        assert_eq!(tabs.input(key(KeyCode::Home)), None);
    }

    #[test]
    fn composite_tooltip_anchor_is_the_selected_cell() {
        let mut segmented: Segmented<()> =
            Segmented::new("seg", &["a", "b", "c"], 1, |_| ());
        segmented.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 32.0));
        let cell = segmented.tooltip_rect();
        assert!((cell.left - 100.0).abs() < 0.01);
        assert!((cell.right - 199.0).abs() < 0.01);

        let mut tabs: Tabs<()> = Tabs::new("tabs", &["a", "b"], 0, |_| ());
        tabs.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 32.0));
        let cell = tabs.tooltip_rect();
        assert!((cell.left - 0.0).abs() < 0.01);
        assert!((cell.right - 100.0).abs() < 0.01);
    }

    #[test]
    fn segmented_tracks_the_hovered_cell() {
        let mut segmented: Segmented<()> =
            Segmented::new("seg", &["a", "b", "c"], 0, |_| ());
        segmented.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 32.0));
        assert!(segmented.repaint_on_move());
        assert_eq!(segmented.hovered(), None);
        segmented.input(WidgetInput::HoverAt(Point::new(150.0, 10.0)));
        assert_eq!(segmented.hovered(), Some(1));
        segmented.input(WidgetInput::HoverLeave);
        assert_eq!(segmented.hovered(), None);
        // Disabling clears and ignores the hover.
        segmented.input(WidgetInput::HoverAt(Point::new(150.0, 10.0)));
        segmented.set_enabled(false);
        assert_eq!(segmented.hovered(), None);
        segmented.input(WidgetInput::HoverAt(Point::new(50.0, 10.0)));
        assert_eq!(segmented.hovered(), None);
    }

    #[test]
    fn tabs_track_the_hovered_cell() {
        let mut tabs: Tabs<()> = Tabs::new("tabs", &["a", "b", "c"], 0, |_| ());
        tabs.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 32.0));
        assert!(tabs.repaint_on_move());
        tabs.input(WidgetInput::HoverAt(Point::new(150.0, 10.0)));
        assert_eq!(tabs.hovered(), Some(1));
        tabs.input(WidgetInput::HoverAt(Point::new(250.0, 10.0)));
        assert_eq!(tabs.hovered(), Some(2));
        tabs.input(WidgetInput::HoverLeave);
        assert_eq!(tabs.hovered(), None);
    }

    #[test]
    fn toggle_label_side_moves_the_track_to_the_leading_edge() {
        let toggle = Toggle::new("t", "开关", false, |on| on);
        let start = toggle.track_rect();
        assert!((start.right - 158.0).abs() < 0.01);

        let mut toggle = Toggle::new("t", "开关", false, |on| on).label_side(LabelSide::End);
        toggle.set_rect(Rect::from_xywh(0.0, 0.0, 160.0, 36.0));
        let track = toggle.track_rect();
        assert!((track.left - 2.0).abs() < 0.01);
        // The label now starts after the track.
        let label = toggle.label_rect(track);
        assert!((label.left - (track.right + 10.0)).abs() < 0.01);
        // The focus ring still hugs the track and stays in the cell.
        let ring = toggle.focus_ring_rect();
        assert!(ring.left >= 0.0 && ring.right <= 160.0);
        assert!(ring.left < 10.0);
    }

    #[test]
    fn toggle_on_and_off_labels_are_stored_independently() {
        let toggle = Toggle::new("t", "静音", true, |on| on)
            .on_label("开")
            .off_label("关");
        assert_eq!(toggle.on_label, "开");
        assert_eq!(toggle.off_label, "关");
    }

    #[test]
    fn disabled_segments_are_skipped_by_arrows_and_clicks() {
        use crate::event::KeyCode;
        let mut segmented: Segmented<usize> =
            Segmented::new("seg", &["a", "b", "c"], 0, |index| index).disable(1);
        segmented.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 32.0));
        assert!(segmented.option_enabled(0));
        assert!(!segmented.option_enabled(1));
        // Right skips the disabled middle option.
        assert_eq!(segmented.input(key(KeyCode::Right)), Some(2));
        // Clicking the disabled cell emits nothing and keeps the selection.
        assert_eq!(
            segmented.input(WidgetInput::ClickAt(Point::new(150.0, 10.0))),
            None
        );
        assert_eq!(segmented.selected(), 2);
        // Hover never lands on a disabled cell.
        segmented.input(WidgetInput::HoverAt(Point::new(150.0, 10.0)));
        assert_eq!(segmented.hovered(), None);
        // End/Home land on the extremes when enabled.
        segmented.input(WidgetInput::HoverLeave);
        assert_eq!(segmented.input(key(KeyCode::Home)), Some(0));
        // The a11y children carry the per-item state.
        let children = segmented.a11y().unwrap().children;
        assert!(children[0].enabled);
        assert!(!children[1].enabled);
        assert!(children[2].enabled);
    }

    #[test]
    fn tabs_close_target_emits_close_instead_of_selecting() {
        use crate::event::KeyCode;
        #[derive(Debug, PartialEq)]
        enum Msg {
            Select(usize),
            Close(usize),
        }
        let mut tabs: Tabs<Msg> = Tabs::new("tabs", &["a", "b", "c"], 0, Msg::Select)
            .closable(Msg::Close)
            .badge(2, "9");
        tabs.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 32.0));
        assert!(tabs.is_closable());
        assert_eq!(tabs.badge_text(2), Some("9"));
        // A click on the middle cell selects it.
        assert_eq!(
            tabs.input(WidgetInput::ClickAt(Point::new(150.0, 10.0))),
            Some(Msg::Select(1))
        );
        // A click on the close target closes instead (and does not select).
        let close = tabs.close_rect(1).unwrap();
        assert_eq!(
            tabs.input(WidgetInput::ClickAt(close.center())),
            Some(Msg::Close(1))
        );
        assert_eq!(tabs.selected(), 1);
        // Delete closes the selected tab from the keyboard.
        assert_eq!(tabs.input(key(KeyCode::Delete)), Some(Msg::Close(1)));
    }

    #[test]
    fn slider_ticks_and_value_label_are_configurable() {
        let slider: Slider<()> = Slider::new("v", 0.5, 0.0..=1.0, |_| ())
            .ticks(4)
            .tick_labels(&["0", "50", "100"])
            .value_text(|value| format!("{:.0}%", value * 100.0));
        assert_eq!(slider.tick_count(), 4);
        assert_eq!(slider.value_label().as_deref(), Some("50%"));
        // Label rows need a taller cell than the bare track.
        assert!(slider.preferred_height(240.0) > 32.0);
        assert_eq!(slider.a11y().unwrap().value.as_deref(), Some("50%"));

        let plain: Slider<()> = Slider::new("v", 0.5, 0.0..=1.0, |_| ());
        assert_eq!(plain.preferred_height(240.0), 32.0);
        assert!(plain.value_label().is_none());
    }

    #[test]
    fn surface_styles_round_the_fill_and_the_focus_ring() {
        use crate::geometry::Corners;
        // One uniform radius shapes the fill and the ring together.
        let pill = Button::new("b", "胶囊", 1u32).corner_radius(18.0);
        assert_eq!(pill.surface().corners, Corners::uniform(18.0));
        assert_eq!(pill.focus_ring_radius(), Some(18.0));

        // A transparent background keeps the border, still rounded.
        let outline = Button::new("o", "描边", 1u32)
            .transparent_background(true)
            .corner_radius(10.0);
        assert!(outline.surface().transparent);
        assert_eq!(outline.focus_ring_radius(), Some(10.0));

        // Mixed corners round the fill but keep the rectangular ring, because
        // the ring stroke has one radius.
        let cut = Button::new("c", "切角", 1u32).corners(Corners::new(10.0, 10.0, 0.0, 0.0));
        assert_eq!(cut.surface().corners, Corners::new(10.0, 10.0, 0.0, 0.0));
        assert_eq!(cut.focus_ring_radius(), None);

        // A negative radius clamps to square instead of flipping.
        assert_eq!(
            Button::new("s", "直角", 1u32)
                .corner_radius(-4.0)
                .surface()
                .corners,
            Corners::ZERO
        );
    }

    #[test]
    fn progress_bar_surface_rounds_the_track_and_fill() {
        use crate::geometry::Corners;
        let bar = ProgressBar::new("p", 0.5)
            .corner_radius(12.0)
            .background(Color::new(1, 2, 3))
            .transparent_background(true);
        assert_eq!(bar.surface().corners, Corners::uniform(12.0));
        assert_eq!(bar.surface().background, Some(Color::new(1, 2, 3)));
        assert!(bar.surface().transparent);
    }

    #[test]
    fn indeterminate_progress_reports_status_instead_of_a_range() {
        let mut bar = ProgressBar::new("p", 0.4).indeterminate();
        assert!(bar.is_indeterminate());
        let info = <ProgressBar as Widget<()>>::a11y(&bar).unwrap();
        assert_eq!(info.range, None);
        assert_eq!(info.value.as_deref(), Some("进行中"));
        // The phase wraps and only reports real changes.
        assert!(bar.set_phase(1.25));
        assert!((bar.phase - 0.25).abs() < 0.001);
        assert!(!bar.set_phase(0.25));
        assert!(bar.set_phase(-0.5));
        assert!((bar.phase - 0.5).abs() < 0.001);
    }

    #[test]
    fn tab_label_stays_centered_next_to_right_side_chrome() {
        let cell = Rect::from_xywh(100.0, 0.0, 100.0, 30.0);
        let tabs: Tabs<()> = Tabs::new("tabs", &["a", "b"], 0, |_| ()).badge(1, "9");
        // Centered (the default): the badge must not shrink the centering box.
        assert_eq!(tabs.label_alignment(), TextAlign::Center);
        let rect = tabs.label_rect(cell, 180.0);
        assert_eq!((rect.left, rect.right), (100.0, 200.0));
        // Left/right alignment stops before the badge/close chrome.
        let tabs = tabs.label_align(TextAlign::Left);
        let rect = tabs.label_rect(cell, 180.0);
        assert_eq!((rect.left, rect.right), (100.0, 180.0));
        // Without chrome, every alignment uses the whole cell.
        let rect = tabs.label_rect(cell, 199.0);
        assert_eq!((rect.left, rect.right), (100.0, 200.0));
    }

    #[test]
    fn slider_can_snap_drags_to_a_discrete_step() {
        let x = |t: f32| 8.0 + 224.0 * t;
        let mut slider: Slider<f32> = Slider::new("v", 0.0, 0.0..=100.0, |value| value)
            .discrete(25.0);
        slider.set_rect(Rect::from_xywh(0.0, 0.0, 240.0, 32.0));
        assert!(slider.is_snapped());
        assert_eq!(slider.step_size(), 25.0);
        // 37% rounds onto the 25 grid while dragging and on release.
        slider.input(WidgetInput::PressAt(Point::new(x(0.37), 16.0)));
        assert_eq!(slider.preview, 25.0);
        slider.input(WidgetInput::DragTo(Point::new(x(0.62), 16.0)));
        assert_eq!(slider.preview, 50.0);
        assert_eq!(
            slider.input(WidgetInput::ReleaseAt(Point::new(x(0.62), 16.0))),
            Some(50.0)
        );
        assert_eq!(slider.value(), 50.0);
        // The UIA value path snaps too (87 → 75 on a 25 grid).
        let message = <Slider<f32> as Widget<f32>>::a11y_set_value(&mut slider, "87");
        assert_eq!(message, Some(75.0));
        assert!(slider.set_snap(false));
        assert!(!slider.set_snap(false));

        // An ordinary slider keeps the exact pointer value.
        let mut plain: Slider<f32> = Slider::new("v", 0.0, 0.0..=100.0, |value| value);
        plain.set_rect(Rect::from_xywh(0.0, 0.0, 240.0, 32.0));
        assert!(!plain.is_snapped());
        plain.input(WidgetInput::PressAt(Point::new(x(0.37), 16.0)));
        assert!((plain.preview - 37.0).abs() < 0.01);

        // The grid is measured from `min`, not from zero.
        let mut offset: Slider<f32> =
            Slider::new("v", 10.0, 10.0..=110.0, |value| value).discrete(25.0);
        offset.set_rect(Rect::from_xywh(0.0, 0.0, 240.0, 32.0));
        offset.input(WidgetInput::PressAt(Point::new(x(0.5), 16.0)));
        assert_eq!(offset.preview, 60.0);
    }

    #[test]
    fn slider_reports_the_drag_lifecycle() {
        let x = |t: f32| 8.0 + 224.0 * t;
        let mut slider: Slider<&'static str> = Slider::new("v", 0.0, 0.0..=100.0, |_| "change")
            .on_drag_start(|_| "start")
            .on_drag(|_| "drag")
            .on_drag_end(|_| "end");
        slider.set_rect(Rect::from_xywh(0.0, 0.0, 240.0, 32.0));
        // Press: the drag start is the primary message and the value previews.
        assert_eq!(
            slider.input(WidgetInput::PressAt(Point::new(x(0.2), 16.0))),
            Some("start")
        );
        assert!(slider.is_dragging());
        assert!((slider.drag_preview().unwrap() - 20.0).abs() < 0.01);
        // Every move while held reports the value under the pointer.
        assert_eq!(
            slider.input(WidgetInput::DragTo(Point::new(x(0.4), 16.0))),
            Some("drag")
        );
        assert!((slider.drag_preview().unwrap() - 40.0).abs() < 0.01);
        // The application-owned value is untouched until the release.
        assert_eq!(slider.value(), 0.0);
        // Release: the usual on_change is primary, on_drag_end rides beside it.
        assert_eq!(
            slider.input(WidgetInput::ReleaseAt(Point::new(x(0.4), 16.0))),
            Some("change")
        );
        assert_eq!(slider.take_extra_message(), Some("end"));
        assert!(!slider.is_dragging());
        assert_eq!(slider.drag_preview(), None);
        assert!((slider.value() - 40.0).abs() < 0.01);
        // The extra slot is drained, so it never leaks into another input.
        assert_eq!(slider.take_extra_message(), None);
    }

    #[test]
    fn slider_ticks_are_display_only_and_separate_from_the_step() {
        let slider: Slider<()> = Slider::new("v", 0.0, 0.0..=100.0, |_| ())
            .discrete(5.0)
            .ticks_at(&[0.0, 25.0, 50.0, 75.0, 100.0])
            .tick_labels(&["0", "25", "50", "75", "100"]);
        // The value grid and the painted marks are independent settings.
        assert_eq!(slider.step_size(), 5.0);
        assert!(slider.is_snapped());
        assert_eq!(slider.tick_values(), &[0.0, 25.0, 50.0, 75.0, 100.0]);
        // Ticks alone do not snap: the value stays continuous.
        let plain: Slider<()> = Slider::new("v", 0.0, 0.0..=100.0, |_| ()).ticks(4);
        assert!(!plain.is_snapped());
        assert_eq!(plain.step_size(), 1.0);
        assert!(plain.tick_values().is_empty());
    }

    #[test]
    fn pager_labels_exist_at_creation_and_accept_a_custom_format() {
        let pager: Pager<()> = Pager::new("pages", 0, 5, |_| ());
        // The initial label used to stay empty until the first page change.
        assert_eq!(pager.label_text(), "第 1 / 5 页");

        let mut pager: Pager<()> = Pager::new("pages", 1, 5, |_| ())
            .label_with(|page, pages| format!("{page}/{pages}"));
        assert_eq!(pager.label_text(), "2/5");
        assert!(pager.set_label_format(|page, pages| format!("({page} 共 {pages})")));
        assert_eq!(pager.label_text(), "(2 共 5)");
        assert!(pager.set_pages(2));
        assert_eq!(pager.label_text(), "(2 共 2)");
        assert!(pager.clear_label_format());
        assert_eq!(pager.label_text(), "第 2 / 2 页");
    }

    #[test]
    fn the_page_child_list_sets_labels_and_the_page_count() {
        let mut pager: Pager<()> = Pager::new("pages", 0, 5, |_| ());
        let mut first = SkinElement::new("page", 2, 3);
        first.push_attr("label", "封面");
        let mut second = SkinElement::new("page", 3, 3);
        second.push_attr("label", "曲目");
        let mut third = SkinElement::new("page", 4, 3);
        third.push_attr("label", "详情");
        let children = [first, second, third];
        assert!(<Pager<()> as Widget<()>>::validate_skin_children(&pager, &children));
        <Pager<()> as Widget<()>>::apply_skin_children(&mut pager, &children);
        assert_eq!(pager.pages(), 3);
        assert_eq!(pager.page_labels(), ["封面", "曲目", "详情"]);
        assert_eq!(pager.label_text(), "封面");
        <Pager<()> as Widget<()>>::set_selected(&mut pager, 2);
        assert_eq!(pager.label_text(), "详情");
        // `selected` clamps to the last page.
        <Pager<()> as Widget<()>>::set_selected(&mut pager, 9);
        assert_eq!(pager.page(), 2);

        // A missing or blank label rejects the list.
        let mut blank = SkinElement::new("page", 6, 3);
        blank.push_attr("label", " ");
        assert!(!<Pager<()> as Widget<()>>::validate_skin_children(&pager, &[blank]));
        let mut unlabeled = SkinElement::new("page", 7, 3);
        unlabeled.push_attr("name", "封面");
        assert!(!<Pager<()> as Widget<()>>::validate_skin_children(&pager, &[unlabeled]));

        // Shrinking drops the labels past the new count; growing past them
        // falls back to the built-in label for the unlabeled pages.
        assert!(pager.set_pages(2));
        assert_eq!(pager.page_labels().len(), 2);
        assert_eq!(pager.page(), 1);
        assert_eq!(pager.label_text(), "曲目");
        assert!(pager.set_pages(4));
        <Pager<()> as Widget<()>>::set_selected(&mut pager, 3);
        assert_eq!(pager.label_text(), "第 4 / 4 页");
    }

    #[test]
    fn the_option_child_list_sets_segmented_and_tab_labels() {
        let mut segmented: Segmented<()> =
            Segmented::new("seg", &["低", "中"], 1, |_| ()).disable(0);
        let mut low = SkinElement::new("option", 2, 3);
        low.push_attr("label", "小");
        let mut mid = SkinElement::new("option", 3, 3);
        mid.push_attr("label", "中");
        let mut high = SkinElement::new("option", 4, 3);
        high.push_attr("label", "大");
        let children = [low, mid, high];
        assert!(<Segmented<()> as Widget<()>>::validate_skin_children(
            &segmented,
            &children
        ));
        <Segmented<()> as Widget<()>>::apply_skin_children(&mut segmented, &children);
        assert_eq!(
            segmented.options().iter().map(String::as_str).collect::<Vec<_>>(),
            ["小", "中", "大"]
        );
        // The per-option disabled flag keeps its index; the selection stays.
        assert!(!segmented.option_enabled(0));
        assert_eq!(segmented.selected(), 1);

        let mut tabs: Tabs<()> = Tabs::new("tabs", &["一"], 0, |_| ()).badge(0, "2");
        let mut first = SkinElement::new("option", 2, 3);
        first.push_attr("label", "概览");
        let mut second = SkinElement::new("option", 3, 3);
        second.push_attr("label", "详情");
        let children = [first, second];
        assert!(<Tabs<()> as Widget<()>>::validate_skin_children(&tabs, &children));
        <Tabs<()> as Widget<()>>::apply_skin_children(&mut tabs, &children);
        assert_eq!(
            tabs.labels().iter().map(String::as_str).collect::<Vec<_>>(),
            ["概览", "详情"]
        );
        assert_eq!(tabs.badge_text(0), Some("2"));
        assert_eq!(tabs.badge_text(1), None);

        // A blank label rejects both lists; so does an unknown attribute.
        let mut blank = SkinElement::new("option", 5, 3);
        blank.push_attr("label", " ");
        assert!(!<Segmented<()> as Widget<()>>::validate_skin_children(
            &segmented,
            &[blank.clone()]
        ));
        assert!(!<Tabs<()> as Widget<()>>::validate_skin_children(&tabs, &[blank]));
        let mut typo = SkinElement::new("option", 6, 3);
        typo.push_attr("label", "x");
        typo.push_attr("title", "y");
        assert!(!<Segmented<()> as Widget<()>>::validate_skin_children(
            &segmented,
            &[typo]
        ));
    }
}
