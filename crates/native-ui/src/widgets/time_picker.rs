// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! An in-window time picker: a field showing `HH:MM` and a two-column popup
//! (hours, minutes). It follows the same common picker API as
//! [`DatePicker`](super::DatePicker): `value`/`set_value`, `open`/`close`,
//! an inclusive range and an `on_change` event (D115).

use crate::a11y::{A11yInfo, A11yRole};
use crate::backend::TextAlign;
use crate::event::{Cursor, KeyCode};
use crate::geometry::{Point, Rect};
use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

use super::observers::{Observers, observer_builders, observer_methods};
use crate::skin::{SkinAttrDecl, SkinValue, SkinValueKind};
use crate::surface::{SurfaceStyle, surface_builders};

static TIME_PICKER_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("time", SkinValueKind::Text),
    SkinAttrDecl::new("open", SkinValueKind::Bool),
];

const COL_W: f32 = 64.0;
const COL_GAP: f32 = 8.0;
const POPUP_PAD: f32 = 10.0;
const LABEL_H: f32 = 18.0;
const ROW_H: f32 = 26.0;
const ROWS: usize = 7;
const GAP: f32 = 4.0;

/// A wall-clock time without a date.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Time {
    pub hour: u32,
    pub minute: u32,
}

impl Default for Time {
    fn default() -> Self {
        Self { hour: 0, minute: 0 }
    }
}

impl Time {
    /// Clamps the hour to `0..=23` and the minute to `0..=59`.
    pub fn new(hour: u32, minute: u32) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
        }
    }

    /// The current local wall-clock time.
    pub fn now() -> Self {
        #[cfg(windows)]
        {
            let time = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
            Self::new(time.wHour as u32, time.wMinute as u32)
        }
        #[cfg(not(windows))]
        {
            Self::default()
        }
    }

    /// `HH:MM` (24-hour).
    pub fn display(self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    /// Parse `HH:MM` (the inverse of [`Time::display`]).
    pub fn parse(text: &str) -> Option<Self> {
        let (hour, minute) = text.trim().split_once(':')?;
        let hour: u32 = hour.parse().ok()?;
        let minute: u32 = minute.parse().ok()?;
        if hour > 23 || minute > 59 {
            return None;
        }
        Some(Self::new(hour, minute))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TimeUnit {
    Hour,
    Minute,
}

/// A time field with an hour/minute popup. Enter opens the popup; Down/Up step
/// the active unit (the columns render top-to-bottom increasing), Left/Right
/// switch units, Enter commits and Escape cancels. Clicking an hour picks it
/// and moves to the minutes; clicking a minute commits and closes. The wheel
/// steps the unit under the pointer.
pub struct TimePicker<M> {
    id: WidgetId,
    rect: Rect,
    time: Time,
    /// The value when the popup opened, for Escape.
    opened_time: Time,
    open: bool,
    flip_up: std::cell::Cell<bool>,
    unit: TimeUnit,
    hover: Option<(TimeUnit, u32)>,
    enabled: bool,
    min_time: Option<Time>,
    max_time: Option<Time>,
    on_change: Box<dyn Fn(Time) -> M>,
    observers: Observers<M>,
    surface: SurfaceStyle,
}

impl<M> TimePicker<M> {
    pub fn new(id: &'static str, time: Time, on_change: impl Fn(Time) -> M + 'static) -> Self {
        let time = Time::new(time.hour, time.minute);
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 110.0, 34.0),
            time,
            opened_time: time,
            open: false,
            flip_up: std::cell::Cell::new(false),
            unit: TimeUnit::Hour,
            hover: None,
            enabled: true,
            min_time: None,
            max_time: None,
            on_change: Box::new(on_change),
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

    /// The current time (typed access).
    pub fn time(&self) -> Time {
        self.time
    }

    /// The current time (the common picker accessor).
    pub fn value(&self) -> Time {
        self.time
    }

    /// Replace the time; it clamps into the selectable range. Returns whether
    /// it changed.
    pub fn set_time(&mut self, time: Time) -> bool {
        self.set_value(time)
    }

    pub fn set_value(&mut self, time: Time) -> bool {
        let time = self.clamp(time);
        if time == self.time {
            return false;
        }
        self.time = time;
        if !self.open {
            self.opened_time = time;
        }
        true
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the popup (returns whether it changed state).
    pub fn open(&mut self) -> bool {
        if self.open || !self.enabled {
            return false;
        }
        self.opened_time = self.time;
        self.unit = TimeUnit::Hour;
        self.hover = None;
        self.open = true;
        true
    }

    /// Close the popup, reverting to the value it opened with (the common
    /// picker "cancel" close).
    pub fn close(&mut self) -> bool {
        if !self.open {
            return false;
        }
        self.time = self.opened_time;
        self.open = false;
        self.hover = None;
        self.flip_up.set(false);
        true
    }

    pub fn range(&self) -> (Option<Time>, Option<Time>) {
        (self.min_time, self.max_time)
    }

    pub fn set_range(&mut self, min: Option<Time>, max: Option<Time>) -> bool {
        let (min, max) = normalize_range(min, max);
        if (min, max) == (self.min_time, self.max_time) {
            return false;
        }
        self.min_time = min;
        self.max_time = max;
        self.time = self.clamp(self.time);
        self.opened_time = self.clamp(self.opened_time);
        true
    }

    pub fn value_range(mut self, min: Option<Time>, max: Option<Time>) -> Self {
        self.set_range(min, max);
        self
    }

    pub fn min_time(mut self, min: Time) -> Self {
        self.set_range(Some(min), self.max_time);
        self
    }

    pub fn max_time(mut self, max: Time) -> Self {
        self.set_range(self.min_time, Some(max));
        self
    }

    pub fn clear_range(mut self) -> Self {
        self.set_range(None, None);
        self
    }

    pub fn selectable(&self, time: Time) -> bool {
        self.min_time.map_or(true, |min| time >= min)
            && self.max_time.map_or(true, |max| time <= max)
    }

    fn clamp(&self, time: Time) -> Time {
        let mut time = Time::new(time.hour, time.minute);
        if let Some(min) = self.min_time
            && time < min
        {
            time = min;
        }
        if let Some(max) = self.max_time
            && time > max
        {
            time = max;
        }
        time
    }

    fn hour_enabled(&self, hour: u32) -> bool {
        let first = Time::new(hour, 0);
        let last = Time::new(hour, 59);
        self.min_time.map_or(true, |min| last >= min)
            && self.max_time.map_or(true, |max| first <= max)
    }

    fn minute_enabled(&self, hour: u32, minute: u32) -> bool {
        self.selectable(Time::new(hour, minute))
    }

    fn set_unit(&mut self, unit: TimeUnit, value: u32) {
        let candidate = match unit {
            TimeUnit::Hour => Time::new(value, self.time.minute),
            TimeUnit::Minute => Time::new(self.time.hour, value),
        };
        self.time = self.clamp(candidate);
    }

    fn adjust(&mut self, unit: TimeUnit, delta: i32) {
        let current = match unit {
            TimeUnit::Hour => self.time.hour,
            TimeUnit::Minute => self.time.minute,
        } as i32;
        let max = match unit {
            TimeUnit::Hour => 23,
            TimeUnit::Minute => 59,
        };
        let next = (current + delta).clamp(0, max) as u32;
        self.set_unit(unit, next);
    }

    fn commit(&mut self) -> Option<M> {
        let changed = self.time != self.opened_time;
        self.open = false;
        self.hover = None;
        self.flip_up.set(false);
        changed.then(|| (self.on_change)(self.time))
    }

    fn resolve_direction(&self, viewport: Rect) {
        let height = Self::popup_height();
        let below = self.rect.bottom + GAP + height;
        let above = self.rect.top - GAP - height;
        let fits_below = viewport.is_empty() || below <= viewport.bottom;
        let can_above = viewport.is_empty() || above >= viewport.top;
        self.flip_up.set(!fits_below && can_above);
    }

    fn popup_height() -> f32 {
        POPUP_PAD * 2.0 + LABEL_H + ROW_H * ROWS as f32
    }

    fn popup_rect(&self) -> Rect {
        let width = POPUP_PAD * 2.0 + COL_W * 2.0 + COL_GAP;
        let height = Self::popup_height();
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

    fn column_left(&self, unit: TimeUnit) -> f32 {
        let popup = self.popup_rect();
        let offset = match unit {
            TimeUnit::Hour => 0.0,
            TimeUnit::Minute => COL_W + COL_GAP,
        };
        popup.left + POPUP_PAD + offset
    }

    fn label_rect(&self, unit: TimeUnit) -> Rect {
        let popup = self.popup_rect();
        Rect::from_xywh(
            self.column_left(unit),
            popup.top + POPUP_PAD,
            COL_W,
            LABEL_H,
        )
    }

    fn row_rect(&self, unit: TimeUnit, row: usize) -> Rect {
        let popup = self.popup_rect();
        let top = popup.top + POPUP_PAD + LABEL_H + row as f32 * ROW_H;
        Rect::from_xywh(self.column_left(unit), top, COL_W, ROW_H)
    }

    fn window_start(&self, unit: TimeUnit) -> u32 {
        let value = match unit {
            TimeUnit::Hour => self.time.hour,
            TimeUnit::Minute => self.time.minute,
        };
        let count = match unit {
            TimeUnit::Hour => 24,
            TimeUnit::Minute => 60,
        };
        let half = ROWS as u32 / 2;
        value.saturating_sub(half).min(count - ROWS as u32)
    }

    #[cfg(test)]
    fn value_rect(&self, unit: TimeUnit, value: u32) -> Option<Rect> {
        let start = self.window_start(unit);
        let row = value.checked_sub(start)? as usize;
        (row < ROWS).then(|| self.row_rect(unit, row))
    }

    fn hit_value(&self, pos: Point) -> Option<(TimeUnit, u32)> {
        if !self.open || !self.popup_rect().contains_point(pos) {
            return None;
        }
        for unit in [TimeUnit::Hour, TimeUnit::Minute] {
            let start = self.window_start(unit);
            for row in 0..ROWS {
                if self.row_rect(unit, row).contains_point(pos) {
                    return Some((unit, start + row as u32));
                }
            }
        }
        None
    }

    fn key(&mut self, code: KeyCode) -> Option<M> {
        // The columns render top-to-bottom increasing, so Down steps to the
        // larger value and Up to the smaller one; the wheel already follows
        // that order (see `input`).
        match code {
            KeyCode::Up => self.adjust(self.unit, -1),
            KeyCode::Down => self.adjust(self.unit, 1),
            KeyCode::PageUp => self.adjust(self.unit, -5),
            KeyCode::PageDown => self.adjust(self.unit, 5),
            KeyCode::Home => self.set_unit(self.unit, 0),
            KeyCode::End => {
                let max = match self.unit {
                    TimeUnit::Hour => 23,
                    TimeUnit::Minute => 59,
                };
                self.set_unit(self.unit, max);
            }
            KeyCode::Left | KeyCode::Right => {
                self.unit = match self.unit {
                    TimeUnit::Hour => TimeUnit::Minute,
                    TimeUnit::Minute => TimeUnit::Hour,
                };
            }
            KeyCode::Enter | KeyCode::Space => return self.commit(),
            _ => {}
        }
        None
    }
}

impl<M: 'static> Widget<M> for TimePicker<M> {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        TIME_PICKER_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("time", SkinValue::Text(text)) => Time::parse(text).is_some(),
            _ => true,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("time", SkinValue::Text(text)) => {
                if let Some(time) = Time::parse(text) {
                    self.set_time(time);
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
        self.open && self.hit_value(pos) != self.hover
    }

    /// The popup owns the wheel while it is open: a notch steps the hovered
    /// unit instead of scrolling the page under the popup.
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
        ctx.text(
            Rect::new(
                self.rect.left + 9.0,
                self.rect.top,
                self.rect.right - 28.0,
                self.rect.bottom,
            ),
            &self.time.display(),
            TextStyleRole::Body,
            TextAlign::Left,
            if self.enabled {
                palette.text
            } else {
                palette.text_disabled
            },
        );
        ctx.text(
            Rect::new(
                self.rect.right - 26.0,
                self.rect.top,
                self.rect.right - 6.0,
                self.rect.bottom,
            ),
            "\u{E823}", // Segoe MDL2 Clock
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
        for unit in [TimeUnit::Hour, TimeUnit::Minute] {
            let active = unit == self.unit;
            ctx.text(
                self.label_rect(unit),
                match unit {
                    TimeUnit::Hour => "时",
                    TimeUnit::Minute => "分",
                },
                TextStyleRole::Small,
                TextAlign::Center,
                if active { palette.accent } else { palette.muted },
            );
            let start = self.window_start(unit);
            let current = match unit {
                TimeUnit::Hour => self.time.hour,
                TimeUnit::Minute => self.time.minute,
            };
            for row in 0..ROWS {
                let value = start + row as u32;
                let enabled = match unit {
                    TimeUnit::Hour => self.hour_enabled(value),
                    TimeUnit::Minute => self.minute_enabled(self.time.hour, value),
                };
                let rect = self.row_rect(unit, row);
                let selected = value == current;
                let hovered = enabled && self.hover == Some((unit, value));
                let inner = rect.inset(3.0, 2.0);
                if selected {
                    ctx.canvas.fill_round_rect(inner, 5.0, palette.accent);
                } else if hovered {
                    ctx.canvas.fill_round_rect(inner, 5.0, palette.button_hover);
                }
                let color = if selected {
                    palette.accent_text
                } else if !enabled {
                    palette.text_disabled
                } else if active {
                    palette.text
                } else {
                    palette.muted
                };
                ctx.text(
                    rect,
                    &format!("{value:02}"),
                    TextStyleRole::Small,
                    TextAlign::Center,
                    color,
                );
            }
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
                    // Clicking the field cancels the popup.
                    self.close();
                    return None;
                }
                match self.hit_value(pos) {
                    Some((TimeUnit::Hour, hour)) if self.hour_enabled(hour) => {
                        self.set_unit(TimeUnit::Hour, hour);
                        self.unit = TimeUnit::Minute;
                        None
                    }
                    Some((TimeUnit::Minute, minute))
                        if self.minute_enabled(self.time.hour, minute) =>
                    {
                        self.set_unit(TimeUnit::Minute, minute);
                        self.commit()
                    }
                    _ => None,
                }
            }
            WidgetInput::Activate => {
                self.open();
                None
            }
            WidgetInput::HoverAt(pos) => {
                if self.open {
                    let hit = self.hit_value(pos);
                    if hit != self.hover {
                        self.hover = hit;
                    }
                }
                None
            }
            WidgetInput::HoverLeave => {
                self.hover = None;
                None
            }
            WidgetInput::Key(key) if self.open => self.key(key.code),
            WidgetInput::Wheel(delta) if self.open => {
                let unit = self.hover.map(|(unit, _)| unit).unwrap_or(self.unit);
                self.adjust(unit, if delta > 0.0 { -1 } else { 1 });
                None
            }
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
                    | KeyCode::PageUp
                    | KeyCode::PageDown
                    | KeyCode::Enter
                    | KeyCode::Space
            )
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let mut info = A11yInfo::new(A11yRole::Button, self.id.0)
            .value(self.time.display())
            .description("时间选择");
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

observer_builders!(TimePicker);
surface_builders!(TimePicker);

fn normalize_range(min: Option<Time>, max: Option<Time>) -> (Option<Time>, Option<Time>) {
    match (min, max) {
        (Some(min), Some(max)) if min > max => (Some(max), Some(min)),
        other => other,
    }
}

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

    fn picker() -> TimePicker<Time> {
        let mut picker = TimePicker::new("time", Time::new(9, 30), |time| time);
        picker.set_rect(Rect::from_xywh(20.0, 20.0, 110.0, 34.0));
        picker
    }

    #[test]
    fn time_values_clamp_and_display() {
        assert_eq!(Time::new(25, 61), Time::new(23, 59));
        assert_eq!(Time::new(9, 5).display(), "09:05");
    }

    #[test]
    fn clicking_a_minute_commits_and_closes() {
        let mut picker = picker();
        picker.input(WidgetInput::Activate);
        assert!(picker.is_open());
        // The seven-row window is centered on the current value: 27..=33.
        let rect = picker.value_rect(TimeUnit::Minute, 33).expect("visible minute");
        assert_eq!(
            picker.input(WidgetInput::ClickAt(rect.center())),
            Some(Time::new(9, 33))
        );
        assert!(!picker.is_open());
        assert_eq!(picker.value(), Time::new(9, 33));
    }

    #[test]
    fn keyboard_switches_units_and_enter_commits() {
        let mut picker = picker();
        picker.input(WidgetInput::Activate);
        // Active unit starts on the hour; Down moves to the larger value
        // because the column renders increasing downwards.
        picker.input(WidgetInput::Key(key(KeyCode::Down)));
        assert_eq!(picker.value(), Time::new(10, 30));
        picker.input(WidgetInput::Key(key(KeyCode::Up)));
        assert_eq!(picker.value(), Time::new(9, 30));
        // Right switches to minutes.
        picker.input(WidgetInput::Key(key(KeyCode::Right)));
        picker.input(WidgetInput::Key(key(KeyCode::Down)));
        assert_eq!(picker.value(), Time::new(9, 31));
        assert_eq!(
            picker.input(WidgetInput::Key(key(KeyCode::Enter))),
            Some(Time::new(9, 31))
        );
        assert!(!picker.is_open());
    }

    #[test]
    fn escape_reverts_the_uncommitted_time() {
        let mut picker = picker();
        picker.input(WidgetInput::Activate);
        picker.input(WidgetInput::Key(key(KeyCode::Down)));
        assert_eq!(picker.value(), Time::new(10, 30));
        assert!(picker.dismiss());
        assert_eq!(picker.value(), Time::new(9, 30));
        assert!(!picker.is_open());
    }

    #[test]
    fn the_wheel_steps_the_hovered_unit_and_does_not_scroll_the_page() {
        let mut picker = picker();
        picker.input(WidgetInput::Activate);
        // Wheel down over the hour column: the next hour.
        let hour = picker.row_rect(TimeUnit::Hour, 3).center();
        picker.input(WidgetInput::HoverAt(hour));
        picker.input(WidgetInput::Wheel(-1.0));
        assert_eq!(picker.value(), Time::new(10, 30));
        // Wheel up steps back to the smaller value.
        picker.input(WidgetInput::Wheel(1.0));
        assert_eq!(picker.value(), Time::new(9, 30));
        // The open popup consumes the wheel so the page under it stays put.
        assert!(<TimePicker<Time> as Widget<Time>>::scrolls_with_wheel(&picker));
        picker.dismiss();
        assert!(!<TimePicker<Time> as Widget<Time>>::scrolls_with_wheel(&picker));
    }

    #[test]
    fn the_range_clamps_values_and_blocks_minutes() {
        let mut picker = TimePicker::new("time", Time::new(9, 0), |time| time)
            .value_range(Some(Time::new(9, 30)), Some(Time::new(17, 0)));
        picker.set_rect(Rect::from_xywh(20.0, 20.0, 110.0, 34.0));
        // The initial value clamps into the range.
        assert_eq!(picker.value(), Time::new(9, 30));
        // Already at the low bound: the clamped value is unchanged.
        assert!(!picker.set_value(Time::new(8, 0)));
        assert_eq!(picker.value(), Time::new(9, 30));
        assert!(picker.set_value(Time::new(23, 0)));
        assert_eq!(picker.value(), Time::new(17, 0));

        // A minute before the range is not selectable (the window centers on
        // the current 09:30, so 27..=33 are visible; 28 is below the range).
        assert!(picker.set_value(Time::new(9, 30)));
        picker.input(WidgetInput::Activate);
        let minute = picker
            .value_rect(TimeUnit::Minute, 28)
            .expect("visible minute");
        assert_eq!(picker.input(WidgetInput::ClickAt(minute.center())), None);
        assert!(picker.is_open());
        // The keyboard still clamps into the range.
        picker.input(WidgetInput::Key(key(KeyCode::Right)));
        picker.input(WidgetInput::Key(key(KeyCode::Down)));
        assert!(picker.value() >= Time::new(9, 30));

        // A reversed range normalizes.
        assert!(picker.set_range(Some(Time::new(18, 0)), Some(Time::new(8, 0))));
        assert_eq!(picker.range(), (Some(Time::new(8, 0)), Some(Time::new(18, 0))));
    }

    #[test]
    fn a11y_publishes_the_value_and_open_state() {
        let mut picker = picker();
        let info = Widget::<Time>::a11y(&picker).expect("a11y");
        assert_eq!(info.value.as_deref(), Some("09:30"));
        assert_eq!(info.expanded, Some(false));
        picker.input(WidgetInput::Activate);
        let info = Widget::<Time>::a11y(&picker).expect("a11y");
        assert_eq!(info.expanded, Some(true));
    }
}
