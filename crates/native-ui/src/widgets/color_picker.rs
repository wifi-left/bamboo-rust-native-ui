// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! An in-window color picker: a field showing a swatch + hex and a palette
//! popup. It shares the picker surface of [`DatePicker`](super::DatePicker) and
//! [`TimePicker`](super::TimePicker): `value`/`set_value`, `open`/`close` and
//! an `on_change` event (D115).
//!
//! The popup holds the preset palette, an optional alpha row (opaque →
//! transparent) and an optional "more colors…" row that hands the user off to
//! the application (the shell's own color dialog). A value is a [`Rgba`]: a
//! [`Color`] plus an alpha byte, so an application can store translucency and
//! still read the plain color.

use crate::a11y::{A11yInfo, A11yRole};
use crate::backend::TextAlign;
use crate::color::{Color, Rgba};
use crate::event::{Cursor, KeyCode};
use crate::geometry::{Point, Rect};
use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

use super::observers::{Observers, observer_builders, observer_methods};
use crate::skin::{SkinAttrDecl, SkinValue, SkinValueKind};
use crate::surface::{SurfaceStyle, surface_builders};

static COLOR_PICKER_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("color", SkinValueKind::Color),
    SkinAttrDecl::new("open", SkinValueKind::Bool),
];

const SWATCH_W: f32 = 26.0;
const SWATCH_H: f32 = 22.0;
const COLS: usize = 8;
const ROWS: usize = 6;
const POPUP_PAD: f32 = 10.0;
const GAP: f32 = 4.0;
/// The alpha strip: a label line, then the eight steps.
const ALPHA_LABEL_H: f32 = 14.0;
const ALPHA_H: f32 = 16.0;
const ALPHA_GAP: f32 = 6.0;
const ALPHA_STEPS: usize = 8;
/// The alpha strip, transparent → opaque.
const ALPHA_VALUES: [u8; ALPHA_STEPS] = [0, 36, 73, 109, 146, 182, 219, 255];
/// Height of the "more colors…" row and the gap above it.
const MORE_H: f32 = 26.0;
const MORE_GAP: f32 = 6.0;

/// The built-in palette: one grayscale row and five hue rows.
const PALETTE: [Color; 48] = [
    // Grays.
    Color::from_hex(0x000000),
    Color::from_hex(0x333333),
    Color::from_hex(0x555555),
    Color::from_hex(0x777777),
    Color::from_hex(0x999999),
    Color::from_hex(0xBBBBBB),
    Color::from_hex(0xDDDDDD),
    Color::from_hex(0xFFFFFF),
    // Vivid.
    Color::from_hex(0xE03131),
    Color::from_hex(0xF08C00),
    Color::from_hex(0xFAB005),
    Color::from_hex(0x2F9E44),
    Color::from_hex(0x1098AD),
    Color::from_hex(0x1971C2),
    Color::from_hex(0x7048E8),
    Color::from_hex(0xD6336C),
    // Light.
    Color::from_hex(0xFF6B6B),
    Color::from_hex(0xFFA94D),
    Color::from_hex(0xFFD43B),
    Color::from_hex(0x69DB7C),
    Color::from_hex(0x66D9E8),
    Color::from_hex(0x4DABF7),
    Color::from_hex(0x9775FA),
    Color::from_hex(0xF783AC),
    // Dark.
    Color::from_hex(0xC92A2A),
    Color::from_hex(0xE8590C),
    Color::from_hex(0xE67700),
    Color::from_hex(0x2B8A3E),
    Color::from_hex(0x0C8599),
    Color::from_hex(0x1864AB),
    Color::from_hex(0x6741D9),
    Color::from_hex(0xA61E4D),
    // Pastel.
    Color::from_hex(0xFFC9C9),
    Color::from_hex(0xFFEC99),
    Color::from_hex(0xD8F5A2),
    Color::from_hex(0xB2F2BB),
    Color::from_hex(0x99E9F2),
    Color::from_hex(0xA5D8FF),
    Color::from_hex(0xD0BFFF),
    Color::from_hex(0xFCC2D7),
    // Deep.
    Color::from_hex(0x7F1D1D),
    Color::from_hex(0x7C2D12),
    Color::from_hex(0x713F12),
    Color::from_hex(0x14532D),
    Color::from_hex(0x164E63),
    Color::from_hex(0x1E3A8A),
    Color::from_hex(0x4C1D95),
    Color::from_hex(0x831843),
];

/// One cell of the popup the keyboard cursor can rest on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Swatch(usize),
    Alpha(usize),
    More,
}

/// A color field with a palette popup. Enter opens the palette; arrows move the
/// cursor, Enter selects and closes, Escape closes without changing the value.
/// With `allow_alpha` the popup adds a transparency strip; with `on_more` it
/// adds a "more colors…" row that closes the popup and reports the handoff.
pub struct ColorPicker<M> {
    id: WidgetId,
    rect: Rect,
    color: Color,
    alpha: u8,
    allow_alpha: bool,
    open: bool,
    flip_up: std::cell::Cell<bool>,
    cursor: usize,
    hover: Option<Slot>,
    enabled: bool,
    reporter: Reporter<M>,
    on_more: Option<Box<dyn Fn() -> M>>,
    observers: Observers<M>,
    surface: SurfaceStyle,
}

/// What a selection reports: the plain color or the color plus its alpha.
enum Reporter<M> {
    Color(Box<dyn Fn(Color) -> M>),
    Rgba(Box<dyn Fn(Rgba) -> M>),
}

impl<M> ColorPicker<M> {
    pub fn new(id: &'static str, color: Color, on_change: impl Fn(Color) -> M + 'static) -> Self {
        Self::with_reporter(id, Rgba::opaque(color), Reporter::Color(Box::new(on_change)))
    }

    /// Build the picker reporting [`Rgba`] values from the start, so the field
    /// shows `#RRGGBBAA` and an application can store transparency.
    pub fn new_rgba(id: &'static str, rgba: Rgba, on_change: impl Fn(Rgba) -> M + 'static) -> Self {
        Self::with_reporter(id, rgba, Reporter::Rgba(Box::new(on_change)))
    }

    fn with_reporter(id: &'static str, rgba: Rgba, reporter: Reporter<M>) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 190.0, 34.0),
            color: rgba.color,
            alpha: rgba.alpha,
            allow_alpha: false,
            open: false,
            flip_up: std::cell::Cell::new(false),
            cursor: 0,
            hover: None,
            enabled: true,
            reporter,
            on_more: None,
            observers: Observers::new(),
            surface: SurfaceStyle::new(),
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Report the full [`Rgba`] instead of the opaque [`Color`], replacing the
    /// callback given to [`ColorPicker::new`].
    pub fn on_change_rgba(mut self, on_change: impl Fn(Rgba) -> M + 'static) -> Self {
        self.reporter = Reporter::Rgba(Box::new(on_change));
        self
    }

    /// Show a "more colors…" row; activating it closes the popup and reports
    /// through this callback, which is where the application opens its own
    /// full color dialog.
    pub fn on_more(mut self, on_more: impl Fn() -> M + 'static) -> Self {
        self.on_more = Some(Box::new(on_more));
        self
    }

    /// Show the transparency strip (`0..=255`). The property controls whether
    /// the *user* can choose an alpha; a value the application set keeps its
    /// alpha either way.
    pub fn allow_alpha(mut self, allow: bool) -> Self {
        self.set_allow_alpha(allow);
        self
    }

    pub fn set_allow_alpha(&mut self, allow: bool) -> bool {
        if allow == self.allow_alpha {
            return false;
        }
        self.allow_alpha = allow;
        if !allow && let Slot::Alpha(_) = self.cursor_slot() {
            self.cursor = 0;
        }
        true
    }

    pub fn allows_alpha(&self) -> bool {
        self.allow_alpha
    }

    /// The current color (typed access).
    pub fn color(&self) -> Color {
        self.color
    }

    /// The current color (the common picker accessor).
    pub fn value(&self) -> Color {
        self.color
    }

    /// The current color together with its alpha.
    pub fn rgba(&self) -> Rgba {
        Rgba::new(self.color, self.alpha)
    }

    /// The current alpha, `0..=255`.
    pub fn alpha(&self) -> u8 {
        self.alpha
    }

    /// Replace the color; returns whether it changed. The alpha is kept.
    pub fn set_color(&mut self, color: Color) -> bool {
        self.set_value(color)
    }

    pub fn set_value(&mut self, color: Color) -> bool {
        if color == self.color {
            return false;
        }
        self.color = color;
        if self.open {
            self.cursor = Self::index_of(color).unwrap_or(self.cursor);
        }
        true
    }

    /// Replace color and alpha; returns whether anything changed. The alpha is
    /// stored even when the transparency strip is not shown.
    pub fn set_rgba(&mut self, rgba: Rgba) -> bool {
        let color_changed = self.set_value(rgba.color);
        let alpha_changed = rgba.alpha != self.alpha;
        self.alpha = rgba.alpha;
        color_changed || alpha_changed
    }

    /// Replace the alpha; returns whether it changed. The transparency strip
    /// only controls whether the *user* can pick one.
    pub fn set_alpha(&mut self, alpha: u8) -> bool {
        if alpha == self.alpha {
            return false;
        }
        self.alpha = alpha;
        true
    }

    /// The built-in palette (`8 × 6` swatches).
    pub fn palette() -> &'static [Color] {
        &PALETTE
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) -> bool {
        if self.open || !self.enabled {
            return false;
        }
        self.cursor = Self::index_of(self.color).unwrap_or(0);
        self.hover = None;
        self.open = true;
        true
    }

    pub fn close(&mut self) -> bool {
        if !self.open {
            return false;
        }
        self.open = false;
        self.hover = None;
        self.flip_up.set(false);
        true
    }

    fn index_of(color: Color) -> Option<usize> {
        PALETTE.iter().position(|candidate| *candidate == color)
    }

    fn report(&self) -> M {
        match &self.reporter {
            Reporter::Color(on_change) => on_change(self.color),
            Reporter::Rgba(on_change) => on_change(self.rgba()),
        }
    }

    fn select_swatch(&mut self, index: usize) -> Option<M> {
        let color = *PALETTE.get(index)?;
        self.open = false;
        self.hover = None;
        self.flip_up.set(false);
        if color == self.color {
            return None;
        }
        self.color = color;
        Some(self.report())
    }

    /// Apply one alpha step. The popup stays open so a translucent color can
    /// be tuned against the same palette.
    fn select_alpha(&mut self, step: usize) -> Option<M> {
        let alpha = *ALPHA_VALUES.get(step)?;
        if alpha == self.alpha {
            return None;
        }
        self.alpha = alpha;
        Some(self.report())
    }

    fn open_more(&mut self) -> Option<M> {
        let more = self.on_more.as_ref()?;
        self.open = false;
        self.hover = None;
        self.flip_up.set(false);
        Some(more())
    }

    fn activate(&mut self, slot: Slot) -> Option<M> {
        match slot {
            Slot::Swatch(index) => self.select_swatch(index),
            Slot::Alpha(step) => self.select_alpha(step),
            Slot::More => self.open_more(),
        }
    }

    fn resolve_direction(&self, viewport: Rect) {
        let height = self.popup_height();
        let below = self.rect.bottom + GAP + height;
        let above = self.rect.top - GAP - height;
        let fits_below = viewport.is_empty() || below <= viewport.bottom;
        let can_above = viewport.is_empty() || above >= viewport.top;
        self.flip_up.set(!fits_below && can_above);
    }

    /// Rows the popup stacks: the palette, the optional alpha strip and the
    /// optional "more colors…" row.
    fn rows(&self) -> usize {
        let alpha = if self.allow_alpha { 1 } else { 0 };
        let more = if self.on_more.is_some() { 1 } else { 0 };
        ROWS + alpha + more
    }

    fn popup_height(&self) -> f32 {
        let alpha = if self.allow_alpha {
            ALPHA_GAP + ALPHA_LABEL_H + ALPHA_H
        } else {
            0.0
        };
        let more = if self.on_more.is_some() {
            MORE_GAP + MORE_H
        } else {
            0.0
        };
        POPUP_PAD * 2.0 + SWATCH_H * ROWS as f32 + alpha + more
    }

    fn popup_width() -> f32 {
        POPUP_PAD * 2.0 + SWATCH_W * COLS as f32
    }

    fn popup_rect(&self) -> Rect {
        let width = Self::popup_width();
        let height = self.popup_height();
        if self.flip_up.get() {
            Rect::new(
                self.rect.left,
                self.rect.top - GAP - height,
                self.rect.left + width,
                self.rect.top - GAP,
            )
        } else {
            Rect::new(
                self.rect.left,
                self.rect.bottom + GAP,
                self.rect.left + width,
                self.rect.bottom + GAP + height,
            )
        }
    }

    fn swatch_rect(&self, index: usize) -> Rect {
        let popup = self.popup_rect();
        let col = index % COLS;
        let row = index / COLS;
        Rect::from_xywh(
            popup.left + POPUP_PAD + col as f32 * SWATCH_W,
            popup.top + POPUP_PAD + row as f32 * SWATCH_H,
            SWATCH_W,
            SWATCH_H,
        )
    }

    fn alpha_rect(&self, step: usize) -> Rect {
        let popup = self.popup_rect();
        Rect::from_xywh(
            popup.left + POPUP_PAD + step as f32 * SWATCH_W,
            popup.top
                + POPUP_PAD
                + SWATCH_H * ROWS as f32
                + ALPHA_GAP
                + ALPHA_LABEL_H,
            SWATCH_W,
            ALPHA_H,
        )
    }

    fn alpha_label_rect(&self) -> Rect {
        let popup = self.popup_rect();
        let top = popup.top + POPUP_PAD + SWATCH_H * ROWS as f32 + ALPHA_GAP;
        Rect::from_xywh(
            popup.left + POPUP_PAD,
            top,
            Self::popup_width() - POPUP_PAD * 2.0,
            ALPHA_LABEL_H,
        )
    }

    fn more_rect(&self) -> Rect {
        let popup = self.popup_rect();
        let top = popup.top
            + POPUP_PAD
            + SWATCH_H * ROWS as f32
            + if self.allow_alpha {
                ALPHA_GAP + ALPHA_LABEL_H + ALPHA_H
            } else {
                0.0
            }
            + MORE_GAP;
        Rect::from_xywh(
            popup.left + POPUP_PAD,
            top,
            Self::popup_width() - POPUP_PAD * 2.0,
            MORE_H,
        )
    }

    fn slot_at(&self, pos: Point) -> Option<Slot> {
        if !self.open || !self.popup_rect().contains_point(pos) {
            return None;
        }
        if let Some(index) = (0..PALETTE.len()).find(|index| self.swatch_rect(*index).contains_point(pos)) {
            return Some(Slot::Swatch(index));
        }
        if self.allow_alpha
            && let Some(step) = (0..ALPHA_STEPS).find(|step| self.alpha_rect(*step).contains_point(pos))
        {
            return Some(Slot::Alpha(step));
        }
        if self.on_more.is_some() && self.more_rect().contains_point(pos) {
            return Some(Slot::More);
        }
        None
    }

    fn cursor_slot(&self) -> Slot {
        let rows = self.rows();
        let row = (self.cursor / COLS).min(rows - 1);
        let col = (self.cursor % COLS).min(COLS - 1);
        if row < ROWS {
            Slot::Swatch(row * COLS + col)
        } else if self.allow_alpha && row == ROWS {
            Slot::Alpha(col)
        } else {
            Slot::More
        }
    }

    /// The top-left flat index of a row.
    fn row_start(&self, row: usize) -> usize {
        row * COLS
    }

    fn step_cursor(&mut self, key: KeyCode) {
        let rows = self.rows();
        let mut row = (self.cursor / COLS).min(rows - 1);
        let mut col = (self.cursor % COLS).min(COLS - 1);
        let more_row = rows - 1;
        match key {
            KeyCode::Left => col = col.saturating_sub(1),
            KeyCode::Right => col = (col + 1).min(COLS - 1),
            KeyCode::Up => row = row.saturating_sub(1),
            KeyCode::Down => row = (row + 1).min(more_row),
            KeyCode::Home => col = 0,
            KeyCode::End => col = COLS - 1,
            _ => {}
        }
        // The "more colors…" row is one filled button, not a grid of cells.
        if self.on_more.is_some() && row == more_row {
            col = 0;
        }
        // The last palette row may be shorter than `COLS` only if the palette
        // changes shape; keep the cursor on a real swatch.
        if row < ROWS {
            col = col.min(PALETTE.len() - 1 - row * COLS);
        }
        self.cursor = self.row_start(row) + col;
    }

    fn field_swatch_rect(&self) -> Rect {
        let size = 20.0;
        let left = self.rect.left + 8.0;
        let top = self.rect.center().y - size * 0.5;
        Rect::new(left, top, left + size, top + size)
    }

    /// Paint a checkerboard so a translucent fill reads as transparency.
    fn paint_checker(canvas: &mut dyn crate::backend::Canvas, rect: Rect, radius: f32) {
        let light = Color::new(0x66, 0x66, 0x66);
        let dark = Color::new(0x3A, 0x3A, 0x3A);
        canvas.fill_round_rect(rect, radius, light);
        let cell = 4.0;
        let inner = rect.inset(1.0, 1.0);
        let mut y = inner.top;
        let mut row = 0;
        while y < inner.bottom {
            let mut x = inner.left;
            let mut col = 0;
            while x < inner.right {
                if (row + col) % 2 == 1 {
                    let piece =
                        Rect::new(x, y, (x + cell).min(inner.right), (y + cell).min(inner.bottom));
                    canvas.fill_rect(piece, dark);
                }
                x += cell;
                col += 1;
            }
            y += cell;
            row += 1;
        }
    }

    /// Paint a swatch of `color` at `alpha` over a checkerboard.
    fn paint_swatch(
        canvas: &mut dyn crate::backend::Canvas,
        rect: Rect,
        radius: f32,
        color: Color,
        alpha: u8,
    ) {
        Self::paint_checker(canvas, rect, radius);
        if alpha >= 255 {
            canvas.fill_round_rect(rect, radius, color);
        } else {
            canvas.fill_translucent(rect, color, alpha as f32 / 255.0);
        }
    }
}

impl<M: 'static> Widget<M> for ColorPicker<M> {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        COLOR_PICKER_SKIN_ATTRS
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            // The picker accepts the alpha byte too, so `#rrggbbaa` seeds a
            // translucent default; an opaque color becomes alpha 255.
            ("color", value) => {
                if let Some(rgba) = value.as_rgba() {
                    self.set_rgba(rgba);
                }
            }
            ("open", SkinValue::Bool(open)) => {
                if *open {
                    self.open();
                } else {
                    self.close();
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

    observer_methods!();

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
            self.close();
        }
    }

    fn paint_bounds(&self) -> Rect {
        if self.open {
            self.rect.union(self.popup_rect())
        } else {
            self.rect
        }
    }

    fn overlay(&self) -> bool {
        self.open
    }

    fn hit_test(&self, pos: Point) -> bool {
        self.rect.contains_point(pos) || (self.open && self.popup_rect().contains_point(pos))
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Hand)
    }

    fn repaint_on_move(&self) -> bool {
        self.open
    }

    fn repaint_on_move_at(&self, pos: Point) -> bool {
        self.open && self.slot_at(pos) != self.hover
    }

    /// The popup owns the wheel while it is open, so a notch never scrolls the
    /// page under it (the same rule as a combo drop-down).
    fn scrolls_with_wheel(&self) -> bool {
        self.open
    }

    fn focus_ring_radius(&self) -> Option<f32> {
        self.surface.focus_radius()
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        self.resolve_direction(ctx.viewport);
        let border = if !self.enabled {
            palette.button_disabled
        } else if state.focused || self.open {
            palette.accent
        } else {
            palette.explicit_border
        };
        self.surface
            .frame(ctx.canvas, self.rect, border, palette.button_bg);
        let swatch = self.field_swatch_rect();
        Self::paint_swatch(&mut *ctx.canvas, swatch, 3.0, self.color, self.alpha);
        ctx.canvas
            .stroke_round_rect(swatch, 3.0, 1.0, palette.divider);
        ctx.text(
            Rect::new(
                swatch.right + 8.0,
                self.rect.top,
                self.rect.right - 26.0,
                self.rect.bottom,
            ),
            &self.rgba().display(),
            TextStyleRole::Small,
            TextAlign::Left,
            if self.enabled {
                palette.text
            } else {
                palette.text_disabled
            },
        );
        ctx.text(
            Rect::new(
                self.rect.right - 24.0,
                self.rect.top,
                self.rect.right - 6.0,
                self.rect.bottom,
            ),
            if self.open { "\u{E70E}" } else { "\u{E70D}" }, // ChevronUp/Down
            TextStyleRole::Glyph,
            TextAlign::Center,
            if self.open { palette.accent } else { palette.muted },
        );
        if !self.open {
            return;
        }

        let popup = self.popup_rect();
        let radius = crate::theme::radius::MEDIUM;
        ctx.canvas.fill_round_rect(popup, radius, palette.panel);
        ctx.canvas
            .stroke_round_rect(popup, radius, 1.0, palette.divider);
        let cursor = self.cursor_slot();
        for (index, color) in PALETTE.iter().enumerate() {
            let rect = self.swatch_rect(index);
            let inner = rect.inset(2.0, 2.0);
            ctx.canvas.fill_round_rect(inner, 4.0, *color);
            ctx.canvas
                .stroke_round_rect(inner, 4.0, 1.0, palette.divider);
            let selected = *color == self.color;
            let focused = cursor == Slot::Swatch(index);
            if selected {
                ctx.canvas
                    .stroke_round_rect(inner, 4.0, 2.0, palette.accent);
            } else if focused {
                ctx.canvas
                    .stroke_round_rect(inner, 4.0, 1.0, palette.accent);
            } else if self.hover == Some(Slot::Swatch(index)) {
                ctx.canvas
                    .stroke_round_rect(inner, 4.0, 1.0, palette.muted);
            }
        }
        if self.allow_alpha {
            ctx.text(
                self.alpha_label_rect(),
                "透明度",
                TextStyleRole::Small,
                TextAlign::Left,
                palette.muted,
            );
            for step in 0..ALPHA_STEPS {
                let rect = self.alpha_rect(step);
                let inner = rect.inset(2.0, 2.0);
                let alpha = ALPHA_VALUES[step];
                Self::paint_swatch(&mut *ctx.canvas, inner, 3.0, self.color, alpha);
                ctx.canvas
                    .stroke_round_rect(inner, 3.0, 1.0, palette.divider);
                if alpha == self.alpha {
                    ctx.canvas
                        .stroke_round_rect(inner, 3.0, 2.0, palette.accent);
                } else if cursor == Slot::Alpha(step) {
                    ctx.canvas
                        .stroke_round_rect(inner, 3.0, 1.0, palette.accent);
                } else if self.hover == Some(Slot::Alpha(step)) {
                    ctx.canvas
                        .stroke_round_rect(inner, 3.0, 1.0, palette.muted);
                }
            }
        }
        if self.on_more.is_some() {
            let rect = self.more_rect();
            let inner = rect.inset(1.0, 1.0);
            if cursor == Slot::More {
                ctx.canvas
                    .stroke_round_rect(inner, 5.0, 1.0, palette.accent);
            } else if self.hover == Some(Slot::More) {
                ctx.canvas.fill_round_rect(inner, 5.0, palette.button_hover);
            }
            ctx.text(
                rect,
                "更多颜色…",
                TextStyleRole::Small,
                TextAlign::Center,
                if cursor == Slot::More {
                    palette.accent
                } else {
                    palette.text
                },
            );
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::ClickAt(pos) => {
                if !self.open {
                    if self.rect.contains_point(pos) {
                        self.open();
                    }
                    return None;
                }
                if self.rect.contains_point(pos) {
                    self.close();
                    return None;
                }
                self.slot_at(pos).and_then(|slot| self.activate(slot))
            }
            WidgetInput::Activate => {
                self.open();
                None
            }
            WidgetInput::HoverAt(pos) => {
                if self.open {
                    let hover = self.slot_at(pos);
                    if hover != self.hover {
                        self.hover = hover;
                    }
                }
                None
            }
            WidgetInput::HoverLeave => {
                self.hover = None;
                None
            }
            WidgetInput::Key(key) if self.open => match key.code {
                KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End => {
                    self.step_cursor(key.code);
                    None
                }
                KeyCode::Enter | KeyCode::Space => {
                    let slot = self.cursor_slot();
                    self.activate(slot)
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn dismiss(&mut self) -> bool {
        self.close()
    }

    fn owns_navigation_keys(&self) -> bool {
        self.open
    }

    fn owns_navigation_key(&self, key: KeyCode) -> bool {
        self.open
            && matches!(
                key,
                KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Home
                    | KeyCode::End
                    | KeyCode::Enter
                    | KeyCode::Space
            )
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let mut info = A11yInfo::new(A11yRole::Button, self.id.0)
            .value(self.rgba().display())
            .description("颜色选择");
        info.expanded = Some(self.open);
        info.read_only = !self.enabled;
        Some(info)
    }

    fn a11y_expand(&mut self, _child: Option<usize>, expanded: bool) -> Option<M> {
        if expanded {
            self.open();
        } else {
            self.close();
        }
        None
    }
}

observer_builders!(ColorPicker);
surface_builders!(ColorPicker);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::KeyEvent;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            vk: 0,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        }
    }

    fn picker() -> ColorPicker<Color> {
        let mut picker = ColorPicker::new("color", Color::new(0x19, 0x71, 0xC2), |color| color);
        picker.set_rect(Rect::from_xywh(20.0, 20.0, 190.0, 34.0));
        picker
    }

    #[test]
    fn the_palette_is_the_advertised_grid() {
        assert_eq!(ColorPicker::<Color>::palette().len(), COLS * ROWS);
        assert_eq!(ColorPicker::<Color>::palette()[0].to_hex(), 0x000000);
        assert_eq!(
            ColorPicker::<Color>::palette()[COLS * ROWS - 1].to_hex(),
            0x831843
        );
    }

    #[test]
    fn the_field_swatch_stays_inside_the_field() {
        let picker = picker();
        let swatch = picker.field_swatch_rect();
        assert_eq!(swatch.width(), 20.0);
        assert_eq!(swatch.height(), 20.0);
        let field = picker.rect();
        assert!(
            field.contains_point(Point::new(swatch.left, swatch.top))
                && field.contains_point(swatch.center()),
            "{swatch:?} must sit inside {field:?}"
        );
    }

    #[test]
    fn clicking_a_swatch_selects_it_and_closes() {
        let mut picker = picker();
        picker.input(WidgetInput::Activate);
        assert!(picker.is_open());
        // The cursor starts on the current color (vivid blue, index 13).
        assert_eq!(picker.cursor, 13);
        let target = picker.swatch_rect(12).center(); // vivid cyan
        assert_eq!(
            picker.input(WidgetInput::ClickAt(target)),
            Some(Color::new(0x10, 0x98, 0xAD))
        );
        assert!(!picker.is_open());
        assert_eq!(picker.value().to_hex(), 0x1098AD);
    }

    #[test]
    fn the_keyboard_moves_the_cursor_and_enter_selects() {
        let mut picker = picker();
        picker.input(WidgetInput::Activate);
        picker.input(WidgetInput::Key(key(KeyCode::Left)));
        assert_eq!(picker.cursor, 12);
        picker.input(WidgetInput::Key(key(KeyCode::Down)));
        assert_eq!(picker.cursor, 20);
        assert_eq!(
            picker.input(WidgetInput::Key(key(KeyCode::Enter))),
            Some(Color::new(0x66, 0xD9, 0xE8))
        );
        assert!(!picker.is_open());
    }

    #[test]
    fn escape_closes_without_changing_the_value() {
        let mut picker = picker();
        let before = picker.value();
        picker.input(WidgetInput::Activate);
        picker.input(WidgetInput::Key(key(KeyCode::Right)));
        assert!(picker.dismiss());
        assert!(!picker.is_open());
        assert_eq!(picker.value(), before);
    }

    #[test]
    fn typed_access_and_a11y_report_the_color() {
        let mut picker = picker();
        assert!(picker.set_value(Color::new(1, 2, 3)));
        assert!(!picker.set_value(Color::new(1, 2, 3)));
        assert_eq!(picker.color(), Color::new(1, 2, 3));
        let info = Widget::<Color>::a11y(&picker).expect("a11y");
        assert_eq!(info.value.as_deref(), Some("#010203"));
        assert_eq!(info.expanded, Some(false));
        picker.input(WidgetInput::Activate);
        let info = Widget::<Color>::a11y(&picker).expect("a11y");
        assert_eq!(info.expanded, Some(true));
    }

    #[test]
    fn the_alpha_strip_only_exists_when_allowed() {
        let mut opaque = picker();
        // The property controls the strip, not the stored value: an
        // application can save translucency without offering the strip.
        assert!(!opaque.allows_alpha());
        assert!(opaque.set_alpha(0x80));
        assert_eq!(opaque.alpha(), 0x80);
        assert!(opaque.set_rgba(Rgba::new(Color::new(9, 9, 9), 0x40)));
        assert_eq!(opaque.rgba(), Rgba::new(Color::new(9, 9, 9), 0x40));
        assert!(!opaque.set_rgba(Rgba::new(Color::new(9, 9, 9), 0x40)));

        // Allowing alpha grows the popup and the keyboard reaches the strip.
        let mut picker = picker();
        picker.set_allow_alpha(true);
        assert!(picker.allows_alpha());
        picker.input(WidgetInput::Activate);
        assert_eq!(picker.cursor_slot(), Slot::Swatch(13));
        for _ in 0..6 {
            picker.input(WidgetInput::Key(key(KeyCode::Down)));
        }
        assert_eq!(picker.cursor_slot(), Slot::Alpha(5));
        let target = picker.alpha_rect(6).center();
        assert_eq!(
            picker.input(WidgetInput::ClickAt(target)),
            Some(Color::new(0x19, 0x71, 0xC2))
        );
        // Transparency does not close the popup.
        assert!(picker.is_open());
        assert_eq!(picker.alpha(), 219);

        // Turning the strip off keeps the value and moves the cursor back up.
        assert!(picker.set_allow_alpha(false));
        assert_eq!(picker.alpha(), 219);
        assert_ne!(picker.cursor_slot(), Slot::Alpha(5));
    }

    #[test]
    fn the_more_row_reports_the_handoff_and_closes() {
        let mut picker = ColorPicker::new("color", Color::new(0x19, 0x71, 0xC2), |color| color)
            .on_more(|| Color::new(7, 7, 7));
        picker.set_rect(Rect::from_xywh(20.0, 20.0, 190.0, 34.0));
        picker.input(WidgetInput::Activate);
        let target = picker.more_rect().center();
        assert_eq!(
            picker.input(WidgetInput::ClickAt(target)),
            Some(Color::new(7, 7, 7))
        );
        assert!(!picker.is_open());
        // The row is the last keyboard stop below the palette.
        picker.input(WidgetInput::Activate);
        for _ in 0..6 {
            picker.input(WidgetInput::Key(key(KeyCode::Down)));
        }
        assert_eq!(picker.cursor_slot(), Slot::More);
        assert_eq!(
            picker.input(WidgetInput::Key(key(KeyCode::Enter))),
            Some(Color::new(7, 7, 7))
        );
        assert!(!picker.is_open());
    }
}
