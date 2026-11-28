// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The library's own color window (D118), shown by
//! [`crate::ColorWindow`].
//!
//! It is an ordinary owned window driven by the same [`Ui`] and widgets as the
//! application: an HSV field (saturation/value square + hue bar), a hex field,
//! an RGB/HSV mode switch, one slider + number editor per channel, the
//! built-in presets and OK/Cancel. It follows the page theme and never calls
//! the shell's color chooser. The dialog body runs on the worker thread that
//! `WindowState::show_dialog` spawns, so no window state is shared.

use std::cell::RefCell;
use std::rc::Rc;

use windows::Win32::Foundation::HWND;

use crate::backend::TextAlign;
use crate::color::{Color, Rgba};
use crate::dialog::ColorWindow;
use crate::event::{Cursor, Event, KeyCode};
use crate::geometry::{Point, Rect, Size};
use crate::host::{Host, PaintCtx};
use crate::platform::window::{WindowBuilder, WindowHandle};
use crate::theme::ThemeSettings;
use crate::ui::Ui;
use crate::widget::{Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};
use crate::widgets::{Button, ColorPicker, NumberInput, Segmented, Slider, TextEdit};

const WINDOW_W: f32 = 620.0;
const WINDOW_H: f32 = 440.0;
const PAD: f32 = 16.0;
/// The saturation/value square and the hue bar next to it.
const SQUARE_W: f32 = 260.0;
const SQUARE_H: f32 = 196.0;
const HUE_W: f32 = 20.0;
const HUE_GAP: f32 = 10.0;
const COL_GAP: f32 = 20.0;
const ROW_H: f32 = 30.0;
const ROW_GAP: f32 = 8.0;
/// The channel-name column left of the sliders.
const LABEL_W: f32 = 42.0;
const LABEL_GAP: f32 = 8.0;
const PRESET_COLS: usize = 16;
const PRESET_CELL: f32 = 26.0;

/// Show the window and block this (dialog worker) thread until it closes.
pub(crate) fn show(
    owner: Option<HWND>,
    dialog: &ColorWindow,
    theme: &ThemeSettings,
    dark: bool,
) -> Option<Rgba> {
    let result: Rc<RefCell<Option<Rgba>>> = Rc::new(RefCell::new(None));
    let host = ColorHost::new(dialog, result.clone());
    let mut builder = WindowBuilder::new(host)
        .title("颜色")
        .size(WINDOW_W, WINDOW_H)
        .min_size(540.0, 400.0)
        .theme(theme.clone(), dark);
    if let Some(owner) = owner {
        builder = builder.owner(owner.0 as isize);
    }
    if let Err(error) = builder.run() {
        if std::env::var_os("BAMBOO_UI_DEBUG").is_some() {
            eprintln!("bamboo-ui: color window failed: {error}");
        }
        return None;
    }
    let picked = *result.borrow();
    picked
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Rgb,
    Hsv,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Msg {
    /// The HSV field moved.
    Field(Color),
    /// A preset swatch was picked.
    Preset(Color),
    /// The RGB/HSV selector.
    Mode(usize),
    /// `(channel, 0..=255, from_slider)`. A slider already holds the value it
    /// reports; a number editor does not update its slider, so the write-back
    /// differs.
    Channel(usize, f32, bool),
    /// The hex field committed.
    Hex,
    Ok,
    Cancel,
}

struct ColorHost {
    ui: Ui<Msg>,
    draft: Rgba,
    allow_alpha: bool,
    mode: Mode,
    result: Rc<RefCell<Option<Rgba>>>,
    size: Size,
}

impl ColorHost {
    fn new(dialog: &ColorWindow, result: Rc<RefCell<Option<Rgba>>>) -> Self {
        let mut ui: Ui<Msg> = Ui::new();
        ui.add(ColorField::new("cw_field", dialog.initial.color, Msg::Field));
        ui.add(SwatchBoard::new(
            "cw_presets",
            ColorPicker::<Msg>::palette(),
            Msg::Preset,
        ));
        ui.add(
            Segmented::new("cw_mode", &["RGB", "HSV"], 0, Msg::Mode),
        );
        ui.add(
            TextEdit::new("cw_hex", "", |_text| Msg::Hex)
                .painted(),
        );
        for channel in 0..3 {
            ui.add(
                Slider::new(
                    channel_id("cw_s", channel),
                    0.0,
                    0.0..=255.0,
                    move |value| Msg::Channel(channel, value, true),
                )
                .discrete(1.0)
                // Follow the drag: the field, the other channels, the hex and
                // the number editor update while the knob is still moving.
                .on_drag(move |value| Msg::Channel(channel, value, true)),
            );
            ui.add(NumberInput::new(
                channel_id("cw_n", channel),
                0,
                0..=255,
                move |value| Msg::Channel(channel, value as f32, false),
            ));
        }
        ui.add_with(
            Slider::new(
                channel_id("cw_s", 3),
                255.0,
                0.0..=255.0,
                |value| Msg::Channel(3, value, true),
            )
            .discrete(1.0)
            .on_drag(|value| Msg::Channel(3, value, true)),
            crate::props::WidgetProps::new().visible(dialog.allow_alpha),
        );
        ui.add_with(
            NumberInput::new(
                channel_id("cw_n", 3),
                255,
                0..=255,
                |value| Msg::Channel(3, value as f32, false),
            ),
            crate::props::WidgetProps::new().visible(dialog.allow_alpha),
        );
        ui.add(Button::new("cw_cancel", "取消", Msg::Cancel));
        ui.add(Button::new("cw_ok", "确定", Msg::Ok).primary());
        let host = Self {
            ui,
            draft: dialog.initial,
            allow_alpha: dialog.allow_alpha,
            mode: Mode::Rgb,
            result,
            size: Size::new(WINDOW_W, WINDOW_H),
        };
        host
    }

    /// The channel values the editors show: RGB bytes or HSV scaled to bytes.
    fn channel_values(&self) -> [u8; 3] {
        let color = self.draft.color;
        match self.mode {
            Mode::Rgb => [color.r, color.g, color.b],
            Mode::Hsv => {
                let (h, s, v) = rgb_to_hsv(color);
                [
                    (h / 360.0 * 255.0).round().clamp(0.0, 255.0) as u8,
                    (s * 255.0).round().clamp(0.0, 255.0) as u8,
                    (v * 255.0).round().clamp(0.0, 255.0) as u8,
                ]
            }
        }
    }

    fn set_channel(&mut self, channel: usize, value: f32) {
        let value = value.round().clamp(0.0, 255.0) as u8;
        if channel == 3 {
            self.draft.alpha = value;
            return;
        }
        let color = self.draft.color;
        self.draft.color = match self.mode {
            Mode::Rgb => {
                let mut rgb = [color.r, color.g, color.b];
                rgb[channel] = value;
                Color::new(rgb[0], rgb[1], rgb[2])
            }
            Mode::Hsv => {
                let (mut h, mut s, mut v) = rgb_to_hsv(color);
                let scaled = value as f32 / 255.0;
                match channel {
                    0 => h = (scaled * 360.0).min(359.9),
                    1 => s = scaled,
                    _ => v = scaled,
                }
                hsv_to_rgb(h, s, v)
            }
        };
    }

    fn hex(&self) -> String {
        self.draft.display()
    }

    fn apply(&mut self, message: Msg, window: &mut WindowHandle<'_>) {
        match message {
            Msg::Field(color) => {
                self.draft.color = color;
                self.sync(true);
            }
            Msg::Preset(color) => {
                self.draft.color = color;
                self.sync(false);
            }
            Msg::Mode(mode) => {
                self.mode = if mode == 1 { Mode::Hsv } else { Mode::Rgb };
                self.sync(false);
            }
            Msg::Channel(channel, value, from_slider) => {
                self.set_channel(channel, value);
                // A slider already shows its own value; a number editor needs
                // its slider written back.
                self.sync(from_slider);
            }
            Msg::Hex => {
                let text = self
                    .ui
                    .with_widget::<TextEdit<Msg>, _>(WidgetId::new("cw_hex"), |field| {
                        field.text().to_string()
                    })
                    .unwrap_or_default();
                if let Some(rgba) = parse_hex(&text, self.allow_alpha) {
                    self.draft = rgba;
                }
                self.sync(false);
            }
            Msg::Ok => {
                *self.result.borrow_mut() = Some(self.draft);
                window.quit(0);
            }
            Msg::Cancel => {
                *self.result.borrow_mut() = None;
                window.quit(0);
            }
        }
    }

    /// Push the draft into every control. `skip_sliders` keeps the slider the
    /// user is dragging from being written back under the pointer (the field
    /// and the number editors still follow live).
    fn sync(&mut self, skip_sliders: bool) {
        let values = self.channel_values();
        let color = self.draft.color;
        let alpha = self.draft.alpha;
        self.ui.edit_widget::<ColorField<Msg>>(
            WidgetId::new("cw_field"),
            |field| field.set_color(color),
        );
        for channel in 0..3 {
            self.ui
                .set_number(WidgetId::new(channel_id("cw_n", channel)), values[channel] as i64);
            if !skip_sliders {
                self.ui
                    .set_value(WidgetId::new(channel_id("cw_s", channel)), values[channel] as f32);
            }
        }
        if self.allow_alpha {
            self.ui
                .set_number(WidgetId::new(channel_id("cw_n", 3)), alpha as i64);
            if !skip_sliders {
                self.ui
                    .set_value(WidgetId::new(channel_id("cw_s", 3)), alpha as f32);
            }
        }
        if self.ui.focus_id() != Some(WidgetId::new("cw_hex")) {
            self.ui.set_text(WidgetId::new("cw_hex"), self.hex());
        }
        self.ui.edit_widget::<SwatchBoard<Msg>>(
            WidgetId::new("cw_presets"),
            |board| board.set_selected(color),
        );
        // The content setters recorded their own damage; the event loop hands
        // it to the window once after the whole batch (see `event`).
    }

    fn layout(&mut self, window: &mut WindowHandle<'_>) {
        let size = self.size;
        let right = size.width - PAD;
        let left_col = PAD + SQUARE_W + HUE_GAP + HUE_W + COL_GAP;
        self.ui.set_rect(
            WidgetId::new("cw_field"),
            Rect::from_xywh(PAD, PAD, SQUARE_W + HUE_GAP + HUE_W, SQUARE_H),
        );
        let row_top = PAD;
        self.ui.set_rect(
            WidgetId::new("cw_hex"),
            Rect::from_xywh(left_col, row_top, right - left_col, ROW_H),
        );
        self.ui.set_rect(
            WidgetId::new("cw_mode"),
            Rect::from_xywh(
                left_col,
                row_top + ROW_H + 10.0,
                right - left_col,
                ROW_H,
            ),
        );
        let rows = if self.allow_alpha { 4 } else { 3 };
        for channel in 0..rows {
            let top = row_top + (ROW_H + 10.0) * 2.0 + 6.0 + (ROW_H + ROW_GAP) * channel as f32;
            let number_w = 58.0;
            let label_x = left_col + LABEL_W + LABEL_GAP;
            self.ui.set_rect(
                WidgetId::new(channel_id("cw_s", channel)),
                Rect::from_xywh(
                    label_x,
                    top,
                    right - number_w - 8.0 - label_x,
                    ROW_H,
                ),
            );
            self.ui.set_rect(
                WidgetId::new(channel_id("cw_n", channel)),
                Rect::from_xywh(right - number_w, top, number_w, ROW_H),
            );
        }
        let presets_top = PAD + SQUARE_H + 34.0;
        self.ui.set_rect(
            WidgetId::new("cw_presets"),
            Rect::from_xywh(
                PAD,
                presets_top,
                size.width - PAD * 2.0,
                PRESET_CELL * 3.0,
            ),
        );
        let button_y = size.height - PAD - 36.0;
        self.ui.set_rect(
            WidgetId::new("cw_cancel"),
            Rect::from_xywh(right - 140.0 - 12.0 - 140.0, button_y, 140.0, 36.0),
        );
        self.ui.set_rect(
            WidgetId::new("cw_ok"),
            Rect::from_xywh(right - 140.0, button_y, 140.0, 36.0),
        );
        window.invalidate_all();
    }
}

fn channel_id(prefix: &str, channel: usize) -> &'static str {
    const SLIDER_IDS: [&str; 4] = ["cw_s0", "cw_s1", "cw_s2", "cw_s3"];
    const NUMBER_IDS: [&str; 4] = ["cw_n0", "cw_n1", "cw_n2", "cw_n3"];
    match prefix {
        "cw_s" => SLIDER_IDS[channel],
        _ => NUMBER_IDS[channel],
    }
}

impl Host for ColorHost {
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        let palette = *ctx.painter.palette();
        for rect in ctx.dirty {
            ctx.canvas.fill_rect(*rect, palette.background);
        }
        let labels = match self.mode {
            Mode::Rgb => ["红色", "绿色", "蓝色"],
            Mode::Hsv => ["色相", "饱和度", "明度"],
        };
        let left_col = PAD + SQUARE_W + HUE_GAP + HUE_W + COL_GAP;
        let row_top = PAD + (ROW_H + 10.0) * 2.0 + 6.0;
        for (index, label) in labels.iter().enumerate() {
            let top = row_top + (ROW_H + ROW_GAP) * index as f32;
            ctx.painter.text(
                ctx.canvas,
                Rect::from_xywh(left_col, top, LABEL_W, ROW_H),
                label,
                ctx.painter.small(),
                TextAlign::Left,
                palette.text,
            );
        }
        if self.allow_alpha {
            let top = row_top + (ROW_H + ROW_GAP) * 3.0;
            ctx.painter.text(
                ctx.canvas,
                Rect::from_xywh(left_col, top, LABEL_W, ROW_H),
                "透明度",
                ctx.painter.small(),
                TextAlign::Left,
                palette.text,
            );
        }
        ctx.painter.text(
            ctx.canvas,
            Rect::from_xywh(PAD, PAD + SQUARE_H + 8.0, 200.0, 20.0),
            "基本颜色",
            ctx.painter.small(),
            TextAlign::Left,
            palette.muted,
        );
        self.ui.paint(ctx);
    }

    fn event(&mut self, event: &Event, window: &mut WindowHandle<'_>) -> bool {
        let handled = match *event {
            Event::PointerMove { pos, .. } => {
                self.ui.pointer_move(pos, window);
                window.set_cursor(self.ui.hover_cursor());
                true
            }
            Event::PointerLeave => {
                self.ui.pointer_leave(window);
                window.set_cursor(Cursor::Arrow);
                true
            }
            Event::PointerDown {
                pos,
                button,
                modifiers,
            } => {
                self.ui.pointer_down_with(pos, button, modifiers, window);
                true
            }
            Event::PointerUp {
                pos,
                button,
                modifiers,
            } => {
                self.ui.pointer_up_with(pos, button, modifiers, window);
                true
            }
            Event::PointerDoubleClick { pos, .. } => {
                self.ui.pointer_double_click(pos, window);
                true
            }
            Event::Wheel {
                pos,
                delta_x,
                delta_y,
            } => self.ui.wheel_xy(pos, delta_x, delta_y, window),
            Event::Key(key) => {
                // The focused control gets the key first: Enter invokes a
                // focused button (the same action a screen reader's Invoke
                // performs), commits an open editor and so on. Only a key no
                // control consumed falls back to the dialog defaults, so the
                // assistive action and the window's confirm never fire twice.
                let handled = self.ui.key(key, window);
                if !handled {
                    match key.code {
                        KeyCode::Escape => {
                            self.apply(Msg::Cancel, window);
                            return true;
                        }
                        KeyCode::Enter if !key.ctrl && !key.alt => {
                            self.apply(Msg::Ok, window);
                            return true;
                        }
                        _ => {}
                    }
                }
                handled
            }
            Event::KeyUp(key) => self.ui.key_up(key, window),
            Event::Char(ch) => self.ui.char(ch, window),
            _ => false,
        };
        for message in self.ui.drain_messages() {
            self.apply(message, window);
        }
        // Programmatic content changes collect locally; one union rect
        // repaints them after the whole event, so a multi-control sync never
        // repaints twice.
        self.ui.flush_damage(window);
        handled
    }

    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        self.size = size;
        self.layout(window);
    }

    fn started(&mut self, window: &mut WindowHandle<'_>) {
        self.ui.set_focus(WidgetId::new("cw_ok"), window);
        self.sync(false);
        self.ui.flush_damage(window);
    }
}

// ---------------------------------------------------------------------------
// HSV math
// ---------------------------------------------------------------------------

/// `(hue 0..360, saturation 0..1, value 0..1)`.
pub(crate) fn rgb_to_hsv(color: Color) -> (f32, f32, f32) {
    let r = color.r as f32 / 255.0;
    let g = color.g as f32 / 255.0;
    let b = color.b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let hue = if hue < 0.0 { hue + 360.0 } else { hue };
    let saturation = if max <= f32::EPSILON { 0.0 } else { delta / max };
    (hue, saturation, max)
}

/// RGB from `(hue 0..360, saturation 0..1, value 0..1)`.
pub(crate) fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> Color {
    let hue = hue.rem_euclid(360.0);
    let saturation = saturation.clamp(0.0, 1.0);
    let value = value.clamp(0.0, 1.0);
    let chroma = value * saturation;
    let x = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = value - chroma;
    let (r, g, b) = match (hue / 60.0) as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    Color::new(
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    )
}

/// Parse `#RRGGBB` / `RRGGBB` (and `#RRGGBBAA` when alpha is allowed).
fn parse_hex(text: &str, allow_alpha: bool) -> Option<Rgba> {
    let digits: String = text.chars().filter(|ch| ch.is_ascii_hexdigit()).collect();
    let color = match digits.len() {
        6 | 8 => Color::from_hex(u32::from_str_radix(&digits[..6], 16).ok()?),
        _ => return None,
    };
    let alpha = if allow_alpha && digits.len() == 8 {
        u8::from_str_radix(&digits[6..8], 16).ok()?
    } else {
        255
    };
    Some(Rgba::new(color, alpha))
}

// ---------------------------------------------------------------------------
// HSV field
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldPart {
    Square,
    Hue,
}

/// The saturation/value square and the hue bar. Dragging either one reports a
/// new color on every move (live preview), like the platform pickers.
pub struct ColorField<M> {
    id: WidgetId,
    rect: Rect,
    hue: f32,
    saturation: f32,
    value: f32,
    drag: Option<FieldPart>,
    /// Region touched by the last drag update, used as the repaint hint so a
    /// square drag repaints the marker instead of the whole gradient.
    pending: Option<Rect>,
    on_change: Box<dyn Fn(Color) -> M>,
}

impl<M> ColorField<M> {
    pub fn new(id: &'static str, color: Color, on_change: impl Fn(Color) -> M + 'static) -> Self {
        let (hue, saturation, value) = rgb_to_hsv(color);
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, SQUARE_W + HUE_GAP + HUE_W, SQUARE_H),
            hue,
            saturation,
            value,
            drag: None,
            pending: None,
            on_change: Box::new(on_change),
        }
    }

    /// Follow a color that came from another control. Returns whether the
    /// stored HSV changed enough to repaint.
    pub fn set_color(&mut self, color: Color) -> bool {
        // During a drag the widget owns the HSV values: the color it reported
        // comes back quantized through RGB, and writing that over the exact
        // pointer-derived saturation/value made the marker lag and jump.
        if self.drag.is_some() {
            return false;
        }
        let (hue, saturation, value) = rgb_to_hsv(color);
        let mut changed = false;
        // A gray has no hue, and 0°/360° are the same color: keep the bar
        // where the user left it instead of snapping it to the top when a
        // drag ends on an equivalent hue.
        if saturation > 0.001 {
            let delta = (hue - self.hue).rem_euclid(360.0);
            if delta.min(360.0 - delta) > 1.0 {
                self.hue = hue;
                changed = true;
            }
        }
        if (saturation - self.saturation).abs() >= 0.004
            || (value - self.value).abs() >= 0.004
        {
            changed = true;
        }
        self.saturation = saturation;
        self.value = value;
        changed
    }

    pub fn color(&self) -> Color {
        hsv_to_rgb(self.hue, self.saturation, self.value)
    }

    fn square(&self) -> Rect {
        Rect::new(
            self.rect.left,
            self.rect.top,
            self.rect.right - HUE_W - HUE_GAP,
            self.rect.bottom,
        )
    }

    fn hue_bar(&self) -> Rect {
        Rect::new(
            self.rect.right - HUE_W,
            self.rect.top,
            self.rect.right,
            self.rect.bottom,
        )
    }

    /// The painted marker of `part` with room for its anti-aliased ring. The
    /// square cursor is clamped into the square and the hue thumb into the
    /// strip: decorations outside the widget rectangle would not be inside the
    /// damage hint and would leave white remnants when the marker moves.
    fn marker_rect(&self, part: FieldPart) -> Rect {
        match part {
            FieldPart::Square => {
                let (cx, cy) = self.cursor_center();
                Rect::new(cx - 8.0, cy - 8.0, cx + 8.0, cy + 8.0)
            }
            FieldPart::Hue => {
                let bar = self.hue_bar();
                let y = self.hue_thumb_y();
                Rect::new(bar.left, y - 4.0, bar.right, y + 4.0)
            }
        }
    }

    /// The saturation/value cursor centre, kept a full radius inside the
    /// square so the white ring never paints outside the field.
    fn cursor_center(&self) -> (f32, f32) {
        const RADIUS: f32 = 6.0;
        let square = self.square();
        let cx = (square.left + self.saturation * square.width())
            .clamp(square.left + RADIUS, (square.right - RADIUS).max(square.left + RADIUS));
        let cy = (square.top + (1.0 - self.value) * square.height())
            .clamp(square.top + RADIUS, (square.bottom - RADIUS).max(square.top + RADIUS));
        (cx, cy)
    }

    /// The hue thumb's centre, two pixels inside the strip.
    fn hue_thumb_y(&self) -> f32 {
        let bar = self.hue_bar();
        let y = bar.top + (self.hue / 359.9) * bar.height();
        y.clamp(bar.top + 2.0, (bar.bottom - 2.0).max(bar.top + 2.0))
    }

    /// Record the touched region for the next repaint hint. A hue change also
    /// recolours the whole saturation/value square, so that case keeps the
    /// full field.
    fn note_change(&mut self, part: FieldPart, before: Rect) {
        self.pending = match part {
            FieldPart::Square => Some(before.union(self.marker_rect(part))),
            FieldPart::Hue => Some(self.rect),
        };
    }

    fn part_at(&self, pos: Point) -> Option<FieldPart> {
        if self.square().contains_point(pos) {
            Some(FieldPart::Square)
        } else if self.hue_bar().contains_point(pos) {
            Some(FieldPart::Hue)
        } else {
            None
        }
    }

    fn update(&mut self, pos: Point, part: FieldPart) {
        match part {
            FieldPart::Square => {
                let square = self.square();
                self.saturation =
                    ((pos.x - square.left) / square.width()).clamp(0.0, 1.0);
                self.value = 1.0 - ((pos.y - square.top) / square.height()).clamp(0.0, 1.0);
            }
            FieldPart::Hue => {
                let bar = self.hue_bar();
                let t = ((pos.y - bar.top) / bar.height()).clamp(0.0, 1.0);
                self.hue = (t * 359.9).min(359.9);
            }
        }
    }
}

impl<M: 'static> Widget<M> for ColorField<M> {
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
        SQUARE_H
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Hand)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        // A drag repaint is usually a small marker region; repaint the
        // gradient columns and hue bands that intersect it instead of the
        // whole field (the back buffer keeps the rest).
        let hit = |rect: Rect| {
            ctx.dirty.is_empty() || ctx.dirty.iter().any(|dirty| dirty.intersects(rect))
        };
        let square = self.square();
        let hue_color = hsv_to_rgb(self.hue, 1.0, 1.0);
        // Bands: a vertical white→hue / black gradient per column. 64 columns
        // keep the banding under ~4 DIP at the default size.
        let columns = 64;
        let width = square.width() / columns as f32;
        for index in 0..columns {
            let rect = Rect::new(
                square.left + index as f32 * width,
                square.top,
                square.left + (index + 1) as f32 * width + 0.5,
                square.bottom,
            );
            if !hit(rect) {
                continue;
            }
            let t = index as f32 / (columns - 1) as f32;
            let top = Color::new(255, 255, 255).mix(hue_color, t);
            ctx.canvas.vertical_gradient(rect, top, Color::new(0, 0, 0));
        }
        if hit(square) {
            ctx.canvas
                .stroke_round_rect(square, 2.0, 1.0, palette.divider);
        }
        let bar = self.hue_bar();
        let bands = 36;
        let band_h = bar.height() / bands as f32;
        for index in 0..bands {
            let rect = Rect::new(
                bar.left,
                bar.top + index as f32 * band_h,
                bar.right,
                bar.top + (index + 1) as f32 * band_h + 0.5,
            );
            if !hit(rect) {
                continue;
            }
            let t = index as f32 / (bands - 1) as f32;
            ctx.canvas
                .fill_rect(rect, hsv_to_rgb(t * 359.9, 1.0, 1.0));
        }
        if hit(bar) {
            ctx.canvas
                .stroke_round_rect(bar, 2.0, 1.0, palette.divider);
        }
        // Hue thumb: a white bar inside the strip (never protruding past the
        // widget rectangle, or moving it leaves white remnants).
        let hue_y = self.hue_thumb_y();
        let thumb = Rect::new(bar.left + 1.0, hue_y - 2.0, bar.right - 1.0, hue_y + 2.0);
        if hit(thumb) {
            ctx.canvas.fill_round_rect(thumb, 2.0, Color::new(255, 255, 255));
        }
        // Saturation/value cursor, clamped inside the square.
        let (cx, cy) = self.cursor_center();
        let outer = Rect::new(cx - 6.0, cy - 6.0, cx + 6.0, cy + 6.0);
        if hit(outer) {
            ctx.canvas.fill_circle(outer, Color::new(255, 255, 255));
            ctx.canvas.fill_circle(outer.inset(2.0, 2.0), self.color());
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        match input {
            WidgetInput::PressAt(pos) => {
                let part = self.part_at(pos)?;
                self.drag = Some(part);
                let before = self.marker_rect(part);
                self.update(pos, part);
                self.note_change(part, before);
                Some((self.on_change)(self.color()))
            }
            WidgetInput::DragTo(pos) => {
                let part = self.drag?;
                let before = self.marker_rect(part);
                self.update(pos, part);
                self.note_change(part, before);
                Some((self.on_change)(self.color()))
            }
            WidgetInput::ReleaseAt(pos) => {
                let part = self.drag.take()?;
                let before = self.marker_rect(part);
                self.update(pos, part);
                self.note_change(part, before);
                Some((self.on_change)(self.color()))
            }
            _ => None,
        }
    }

    /// A drag owns the marker, so every move repaints; a plain hover has no
    /// visual state to update.
    fn repaint_on_move_at(&self, _pos: Point) -> bool {
        self.drag.is_some()
    }

    fn take_repaint_hint(&mut self) -> Option<Rect> {
        self.pending.take()
    }
}

// ---------------------------------------------------------------------------
// Preset swatches
// ---------------------------------------------------------------------------

/// The built-in palette as a clickable grid of circles.
pub struct SwatchBoard<M> {
    id: WidgetId,
    rect: Rect,
    colors: &'static [Color],
    selected: Option<Color>,
    hover: Option<usize>,
    on_pick: Box<dyn Fn(Color) -> M>,
}

impl<M> SwatchBoard<M> {
    pub fn new(
        id: &'static str,
        colors: &'static [Color],
        on_pick: impl Fn(Color) -> M + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, PRESET_CELL * PRESET_COLS as f32, PRESET_CELL * 3.0),
            colors,
            selected: None,
            hover: None,
            on_pick: Box::new(on_pick),
        }
    }

    pub fn set_selected(&mut self, color: Color) -> bool {
        let selected = self.colors.iter().copied().find(|candidate| *candidate == color);
        if selected == self.selected {
            return false;
        }
        self.selected = selected;
        true
    }

    fn rows(&self) -> usize {
        self.colors.len().div_ceil(PRESET_COLS).max(1)
    }

    fn cell(&self, index: usize) -> Rect {
        let col = index % PRESET_COLS;
        let row = index / PRESET_COLS;
        Rect::from_xywh(
            self.rect.left + col as f32 * PRESET_CELL,
            self.rect.top + row as f32 * PRESET_CELL,
            PRESET_CELL,
            PRESET_CELL,
        )
    }

    fn index_at(&self, pos: Point) -> Option<usize> {
        (0..self.colors.len()).find(|index| self.cell(*index).contains_point(pos))
    }
}

impl<M: 'static> Widget<M> for SwatchBoard<M> {
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
        PRESET_CELL * self.rows() as f32
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Hand)
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    fn repaint_on_move_at(&self, pos: Point) -> bool {
        self.index_at(pos) != self.hover
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        for (index, color) in self.colors.iter().enumerate() {
            let cell = self.cell(index);
            let radius = (PRESET_CELL * 0.5 - 3.0).max(4.0);
            let center = cell.center();
            let dot = Rect::new(
                center.x - radius,
                center.y - radius,
                center.x + radius,
                center.y + radius,
            );
            let selected = self.selected == Some(*color);
            let hovered = self.hover == Some(index);
            if selected || hovered {
                ctx.canvas.fill_circle(dot.inset(-1.0, -1.0), palette.accent);
            }
            ctx.canvas.fill_circle(dot, *color);
            if selected || hovered {
                ctx.canvas.fill_circle(dot.inset(1.0, 1.0), *color);
            }
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        match input {
            WidgetInput::ClickAt(pos) => self
                .index_at(pos)
                .and_then(|index| self.colors.get(index).copied())
                .map(|color| (self.on_pick)(color)),
            WidgetInput::HoverAt(pos) => {
                let hover = self.index_at(pos);
                if hover != self.hover {
                    self.hover = hover;
                }
                None
            }
            WidgetInput::HoverLeave => {
                self.hover = None;
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_round_trips_the_primaries() {
        for color in [
            Color::new(255, 0, 0),
            Color::new(0, 255, 0),
            Color::new(0, 0, 255),
            Color::new(0x19, 0x71, 0xC2),
            Color::new(128, 128, 128),
        ] {
            let (h, s, v) = rgb_to_hsv(color);
            let back = hsv_to_rgb(h, s, v);
            assert!(
                (back.r as i32 - color.r as i32).abs() <= 1
                    && (back.g as i32 - color.g as i32).abs() <= 1
                    && (back.b as i32 - color.b as i32).abs() <= 1,
                "{color:?} -> {h:?}/{s:?}/{v:?} -> {back:?}"
            );
        }
        assert_eq!(rgb_to_hsv(Color::new(255, 0, 0)), (0.0, 1.0, 1.0));
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), Color::new(0, 255, 0));
    }

    #[test]
    fn hex_parsing_accepts_short_and_alpha_forms() {
        assert_eq!(
            parse_hex("#1971C2", false),
            Some(Rgba::opaque(Color::new(0x19, 0x71, 0xC2)))
        );
        assert_eq!(
            parse_hex("1971C2", false),
            Some(Rgba::opaque(Color::new(0x19, 0x71, 0xC2)))
        );
        assert_eq!(parse_hex("#1971C280", false).map(|rgba| rgba.alpha), Some(255));
        assert_eq!(parse_hex("#1971C280", true).map(|rgba| rgba.alpha), Some(0x80));
        assert_eq!(parse_hex("#1971", true), None);
        assert_eq!(parse_hex("", true), None);
    }

    #[test]
    fn the_field_reports_the_dragged_color() {
        let mut field = ColorField::<Color>::new("field", Color::new(255, 0, 0), |color| color);
        field.set_rect(Rect::from_xywh(0.0, 0.0, SQUARE_W + HUE_GAP + HUE_W, SQUARE_H));
        // Top-left of the square: white (saturation 0, value 1).
        let corner = field.square().inset(2.0, 2.0);
        let message = field.input(WidgetInput::PressAt(Point::new(corner.left, corner.top)));
        assert!(message.is_some_and(|color| color.r > 245 && color.g > 245));
        // Full saturation at the square's right edge, then the middle of the
        // hue bar: a saturated hue around 180°.
        let square = field.square();
        let vivid = field
            .input(WidgetInput::PressAt(Point::new(square.right - 2.0, square.top + 2.0)))
            .expect("square press");
        assert!(vivid.r > 245 && vivid.g < 20, "{vivid:?}");
        let bar = field.hue_bar();
        let color = field
            .input(WidgetInput::PressAt(bar.center()))
            .expect("hue press");
        assert!(color.g > 120 && color.b > 120 && color.r < 120, "{color:?}");
        field.input(WidgetInput::ReleaseAt(bar.center()));
        assert!(field.drag.is_none());
    }

    #[test]
    fn a_field_drag_repaints_the_marker_and_keeps_the_exact_hsv() {
        let mut field = ColorField::<Color>::new("field", Color::new(9, 214, 69), |color| color);
        field.set_rect(Rect::from_xywh(0.0, 0.0, SQUARE_W + HUE_GAP + HUE_W, SQUARE_H));
        let square = field.square();
        let start = square.center();
        field.input(WidgetInput::PressAt(start));
        let enter = field.take_repaint_hint().expect("press hint");
        assert!(enter.width() < square.width(), "press hint is the marker");

        // Every move owns the marker and costs only its two positions.
        let next = Point::new(start.x + 20.0, start.y + 10.0);
        assert!(field.repaint_on_move_at(next));
        field.input(WidgetInput::DragTo(next));
        let hint = field.take_repaint_hint().expect("drag hint");
        assert!(
            hint.width() <= 36.5 && hint.height() <= 36.5,
            "a square drag hint stays around the marker: {hint:?}"
        );
        // The host writes the quantized color back on every message; while
        // dragging, that must not overwrite the pointer-derived HSV (the old
        // 0.004 threshold made the marker jump).
        let exact = (field.saturation, field.value);
        assert!(!field.set_color(Color::new(255, 255, 255)));
        assert_eq!((field.saturation, field.value), exact);

        // A hue drag changes the square too, so its hint is the whole field.
        field.input(WidgetInput::ReleaseAt(next));
        let bar = field.hue_bar();
        field.input(WidgetInput::PressAt(bar.center()));
        assert_eq!(field.take_repaint_hint(), Some(field.rect));
        assert!(field.repaint_on_move_at(bar.center()));
    }

    #[test]
    fn field_markers_never_paint_outside_the_widget() {
        let mut field = ColorField::<Color>::new("field", Color::new(9, 214, 69), |color| color);
        field.set_rect(Rect::from_xywh(0.0, 0.0, SQUARE_W + HUE_GAP + HUE_W, SQUARE_H));
        let square = field.square();
        for (saturation, value) in [(0.0, 1.0), (1.0, 1.0), (0.0, 0.0), (1.0, 0.0)] {
            field.saturation = saturation;
            field.value = value;
            let (cx, cy) = field.cursor_center();
            assert!(square.contains_point(Point::new(cx, cy)), "{saturation}/{value}");
            assert!(
                square.left <= cx - 6.0
                    && cx + 6.0 <= square.right
                    && square.top <= cy - 6.0
                    && cy + 6.0 <= square.bottom,
                "the ring stays inside the square at {saturation}/{value}"
            );
        }
        let bar = field.hue_bar();
        for hue in [0.0, 180.0, 359.9] {
            field.hue = hue;
            let y = field.hue_thumb_y();
            assert!(
                bar.top <= y - 2.0 && y + 2.0 <= bar.bottom,
                "hue thumb stays inside the strip at {hue}"
            );
            let hint = field.marker_rect(FieldPart::Hue);
            assert!(bar.contains_point(hint.center()), "hint centre inside the bar");
        }
    }

    #[test]
    fn the_hue_bar_does_not_wrap_at_the_bottom() {
        let mut field = ColorField::<Color>::new("field", Color::new(255, 0, 0), |color| color);
        let bar = field.hue_bar();
        field.input(WidgetInput::PressAt(Point::new(bar.center().x, bar.bottom - 1.0)));
        let dragged = field.hue;
        assert!(dragged > 350.0, "bottom of the bar is ~360°, got {dragged}");
        // The host writes the picked color back: its red is hue 0, which must
        // not snap the thumb to the top of the bar.
        field.set_color(field.color());
        assert!(
            (field.hue - dragged).abs() < 1.0,
            "hue wrapped to {} from {dragged}",
            field.hue
        );
        // A real hue change still moves it.
        let color = field.input(WidgetInput::PressAt(bar.center())).expect("hue");
        assert!((field.hue - 180.0).abs() < 3.0, "{color:?}");
    }

    #[test]
    fn the_preset_board_picks_a_swatch() {
        let mut board = SwatchBoard::<Color>::new("board", ColorPicker::<Color>::palette(), |color| color);
        board.set_rect(Rect::from_xywh(0.0, 0.0, PRESET_CELL * PRESET_COLS as f32, PRESET_CELL * 3.0));
        let second = board.cell(1).center();
        assert_eq!(
            board.input(WidgetInput::ClickAt(second)),
            Some(Color::from_hex(0x333333))
        );
    }
}
