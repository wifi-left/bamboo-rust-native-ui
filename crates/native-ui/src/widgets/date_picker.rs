// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! An in-window date picker: a field showing a date and a month-grid popup.
//!
//! There is no native date dialog. The picker is an ordinary retained widget,
//! so it follows the theme, the focus ring, the observer/surface builders and
//! the accessibility tree like every other control (D115).

use crate::a11y::{A11yInfo, A11yRole};
use crate::backend::TextAlign;
use crate::event::{Cursor, KeyCode};
use crate::geometry::{Point, Rect};
use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

use super::observers::{Observers, observer_builders, observer_methods};
use crate::skin::{SkinAttrDecl, SkinValue, SkinValueKind};
use crate::surface::{SurfaceStyle, surface_builders};

static DATE_PICKER_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("date", SkinValueKind::Text),
    SkinAttrDecl::new("open", SkinValueKind::Bool),
];

const CELL_W: f32 = 34.0;
const CELL_H: f32 = 30.0;
const HEADER_H: f32 = 32.0;
const WEEKDAY_H: f32 = 22.0;
const POPUP_PAD: f32 = 6.0;
const GAP: f32 = 4.0;
const ARROW_W: f32 = 30.0;
const GRID_COLS: usize = 7;
const GRID_ROWS: usize = 6;
const WEEKDAYS: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];

/// A calendar date without a time. Used by [`DatePicker`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Default for Date {
    fn default() -> Self {
        Self {
            year: 1970,
            month: 1,
            day: 1,
        }
    }
}

impl Date {
    /// A valid date: the month clamps to `1..=12` and the day to that month's
    /// length.
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        let year = year.max(1);
        let month = month.clamp(1, 12);
        let day = day.clamp(1, Self::days_in_month(year, month));
        Self { year, month, day }
    }

    /// The current local date.
    pub fn today() -> Self {
        #[cfg(windows)]
        {
            let time = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
            Self::new(time.wYear as i32, time.wMonth as u32, time.wDay as u32)
        }
        #[cfg(not(windows))]
        {
            Self::default()
        }
    }

    pub fn days_in_month(year: i32, month: u32) -> u32 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if Self::is_leap(year) => 29,
            2 => 28,
            _ => 30,
        }
    }

    fn is_leap(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    /// `YYYY-MM-DD`.
    pub fn display(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Parse `YYYY-MM-DD` (the inverse of [`Date::display`]). Out-of-range
    /// parts are clamped by [`Date::new`].
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('-');
        let year: i32 = parts.next()?.parse().ok()?;
        let month: u32 = parts.next()?.parse().ok()?;
        let day: u32 = parts.next()?.parse().ok()?;
        if parts.next().is_some() || !(1..=12).contains(&month) || day == 0 {
            return None;
        }
        Some(Self::new(year, month, day))
    }

    /// Monday-first position in a week (`0` = Monday … `6` = Sunday).
    pub fn monday_index(self) -> u32 {
        ((self.days_since_epoch() + 4).rem_euclid(7) as u32 + 6) % 7
    }

    /// The same time `days` calendar days later (or earlier).
    pub fn add_days(self, days: i64) -> Self {
        let (year, month, day) = civil_from_days(self.days_since_epoch() + days);
        Self { year, month, day }
    }

    /// First day of the month.
    pub fn first_of_month(self) -> Self {
        Self::new(self.year, self.month, 1)
    }

    fn days_since_epoch(self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }
}

// Howard Hinnant's civil-date algorithms (days since 1970-01-01).
fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let y = year as i64 - if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = month as i64;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Swap a reversed range so `min <= max`; an unset side stays unbounded.
fn normalize_range(min: Option<Date>, max: Option<Date>) -> (Option<Date>, Option<Date>) {
    match (min, max) {
        (Some(min), Some(max)) if min > max => (Some(max), Some(min)),
        other => other,
    }
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

/// A date field with a month-grid popup. Click the field (or press Enter) to
/// open the calendar, click a day or press Enter to choose, and Escape or a
/// press outside to close. Arrows move the cursor, PageUp/PageDown change the
/// month, Home/End jump to the start/end of the week.
pub struct DatePicker<M> {
    id: WidgetId,
    rect: Rect,
    date: Date,
    /// The month the grid shows.
    view_year: i32,
    view_month: u32,
    /// The keyboard/hover cursor inside the grid.
    cursor: Date,
    open: bool,
    /// The popup flips above the field when it does not fit below (resolved
    /// during paint, used by hit-testing).
    flip_up: std::cell::Cell<bool>,
    /// Hovered day within the shown month.
    hover: Option<u32>,
    enabled: bool,
    /// Selectable date range (inclusive); `None` is unbounded on that side.
    min_date: Option<Date>,
    max_date: Option<Date>,
    on_change: Box<dyn Fn(Date) -> M>,
    observers: Observers<M>,
    surface: SurfaceStyle,
}

impl<M> DatePicker<M> {
    pub fn new(id: &'static str, date: Date, on_change: impl Fn(Date) -> M + 'static) -> Self {
        let date = Date::new(date.year, date.month, date.day);
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 200.0, 34.0),
            date,
            view_year: date.year,
            view_month: date.month,
            cursor: date,
            open: false,
            flip_up: std::cell::Cell::new(false),
            hover: None,
            enabled: true,
            min_date: None,
            max_date: None,
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

    /// The current date (typed access).
    pub fn date(&self) -> Date {
        self.date
    }

    /// The current date (the common picker accessor).
    pub fn value(&self) -> Date {
        self.date
    }

    /// Replace the date; returns whether it changed. The value is clamped into
    /// the selectable range. A disabled picker still accepts the
    /// application's value.
    pub fn set_date(&mut self, date: Date) -> bool {
        self.set_value(date)
    }

    /// Replace the date (the common picker setter).
    pub fn set_value(&mut self, date: Date) -> bool {
        let date = self.clamp(date);
        if date == self.date {
            return false;
        }
        self.date = date;
        if !self.open {
            self.view_year = date.year;
            self.view_month = date.month;
            self.cursor = date;
        }
        true
    }

    /// The selectable range, `(min, max)`.
    pub fn range(&self) -> (Option<Date>, Option<Date>) {
        (self.min_date, self.max_date)
    }

    /// Set the inclusive selectable range; the current value/cursor clamp into
    /// it. Returns whether anything changed.
    pub fn set_range(&mut self, min: Option<Date>, max: Option<Date>) -> bool {
        let (min, max) = normalize_range(min, max);
        if (min, max) == (self.min_date, self.max_date) {
            return false;
        }
        self.min_date = min;
        self.max_date = max;
        self.normalize();
        true
    }

    /// Builder form of [`DatePicker::set_range`].
    pub fn value_range(mut self, min: Option<Date>, max: Option<Date>) -> Self {
        self.set_range(min, max);
        self
    }

    /// The earliest selectable date.
    pub fn min_date(mut self, min: Date) -> Self {
        self.set_range(Some(min), self.max_date);
        self
    }

    /// The latest selectable date.
    pub fn max_date(mut self, max: Date) -> Self {
        self.set_range(self.min_date, Some(max));
        self
    }

    /// Remove the selectable range.
    pub fn clear_range(mut self) -> Self {
        self.set_range(None, None);
        self
    }

    /// Whether a date can be chosen.
    pub fn selectable(&self, date: Date) -> bool {
        self.min_date.map_or(true, |min| date >= min)
            && self.max_date.map_or(true, |max| date <= max)
    }

    fn clamp(&self, date: Date) -> Date {
        let mut date = Date::new(date.year, date.month, date.day);
        if let Some(min) = self.min_date
            && date < min
        {
            date = min;
        }
        if let Some(max) = self.max_date
            && date > max
        {
            date = max;
        }
        date
    }

    fn normalize(&mut self) {
        self.date = self.clamp(self.date);
        self.cursor = self.clamp(self.cursor);
        if self.open {
            self.view_year = self.cursor.year;
            self.view_month = self.cursor.month;
        } else {
            self.view_year = self.date.year;
            self.view_month = self.date.month;
            self.cursor = self.date;
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The month the grid shows, `(year, month)`.
    pub fn view_month(&self) -> (i32, u32) {
        (self.view_year, self.view_month)
    }

    /// Open the calendar (returns whether it changed state).
    pub fn open(&mut self) -> bool {
        if self.open || !self.enabled {
            return false;
        }
        self.view_year = self.date.year;
        self.view_month = self.date.month;
        self.cursor = self.date;
        self.open = true;
        true
    }

    /// Close the calendar without choosing (returns whether it changed state).
    pub fn close(&mut self) -> bool {
        if !self.open {
            return false;
        }
        self.open = false;
        self.hover = None;
        self.flip_up.set(false);
        true
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
        POPUP_PAD * 2.0 + HEADER_H + WEEKDAY_H + CELL_H * GRID_ROWS as f32
    }

    fn popup_rect(&self) -> Rect {
        let width = (CELL_W * GRID_COLS as f32 + POPUP_PAD * 2.0).max(self.rect.width());
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

    fn grid_left(&self) -> f32 {
        let popup = self.popup_rect();
        popup.left + (popup.width() - CELL_W * GRID_COLS as f32) * 0.5
    }

    fn grid_top(&self) -> f32 {
        self.popup_rect().top + POPUP_PAD + HEADER_H + WEEKDAY_H
    }

    fn cell_rect(&self, row: usize, col: usize) -> Rect {
        Rect::from_xywh(
            self.grid_left() + col as f32 * CELL_W,
            self.grid_top() + row as f32 * CELL_H,
            CELL_W,
            CELL_H,
        )
    }

    fn weekday_rect(&self, col: usize) -> Rect {
        let popup = self.popup_rect();
        Rect::from_xywh(
            self.grid_left() + col as f32 * CELL_W,
            popup.top + POPUP_PAD + HEADER_H,
            CELL_W,
            WEEKDAY_H,
        )
    }

    fn prev_rect(&self) -> Rect {
        let popup = self.popup_rect();
        Rect::from_xywh(
            popup.left + POPUP_PAD,
            popup.top + POPUP_PAD,
            ARROW_W,
            HEADER_H,
        )
    }

    fn next_rect(&self) -> Rect {
        let popup = self.popup_rect();
        Rect::from_xywh(
            popup.right - POPUP_PAD - ARROW_W,
            popup.top + POPUP_PAD,
            ARROW_W,
            HEADER_H,
        )
    }

    fn title_rect(&self) -> Rect {
        let popup = self.popup_rect();
        Rect::new(
            self.prev_rect().right,
            popup.top + POPUP_PAD,
            self.next_rect().left,
            popup.top + POPUP_PAD + HEADER_H,
        )
    }

    fn first_lead(&self) -> usize {
        Date::new(self.view_year, self.view_month, 1).monday_index() as usize
    }

    fn day_cell(&self, day: u32) -> Rect {
        let index = self.first_lead() + day.saturating_sub(1) as usize;
        self.cell_rect(index / GRID_COLS, index % GRID_COLS)
    }

    fn day_at(&self, pos: Point) -> Option<u32> {
        if !self.open || !self.popup_rect().contains_point(pos) {
            return None;
        }
        let lead = self.first_lead();
        let days = Date::days_in_month(self.view_year, self.view_month);
        for day in 1..=days {
            let index = lead + day as usize - 1;
            if index / GRID_COLS >= GRID_ROWS {
                break;
            }
            if self.day_cell(day).contains_point(pos) {
                return Some(day);
            }
        }
        None
    }

    fn cursor_date(&self) -> Date {
        Date::new(self.view_year, self.view_month, self.cursor.day)
    }

    fn select(&mut self, day: u32) -> Option<M> {
        let date = Date::new(self.view_year, self.view_month, day);
        if !self.selectable(date) {
            return None;
        }
        self.date = date;
        self.cursor = date;
        self.open = false;
        self.hover = None;
        self.flip_up.set(false);
        Some((self.on_change)(date))
    }

    fn step_month(&mut self, delta: i32) {
        let mut index = self.view_year as i64 * 12 + self.view_month as i64 - 1 + delta as i64;
        if index < 12 {
            index = 12;
        }
        self.view_year = (index / 12) as i32;
        self.view_month = (index % 12 + 1) as u32;
        let days = Date::days_in_month(self.view_year, self.view_month);
        self.cursor = Date::new(self.view_year, self.view_month, self.cursor.day.min(days));
        // Keep the cursor inside the selectable range; the shown month follows
        // the cursor so the two never disagree.
        self.cursor = self.clamp(self.cursor);
        self.view_year = self.cursor.year;
        self.view_month = self.cursor.month;
        self.hover = None;
    }

    fn step_days(&mut self, delta: i64) {
        let next = self.clamp(self.cursor_date().add_days(delta));
        self.cursor = next;
        self.view_year = next.year;
        self.view_month = next.month;
        self.hover = None;
    }

    fn step_week(&mut self, to_start: bool) {
        let index = self.cursor.monday_index();
        let delta = if to_start {
            -(index as i64)
        } else {
            6 - index as i64
        };
        self.step_days(delta);
    }

    fn key(&mut self, code: KeyCode) -> Option<M> {
        match code {
            KeyCode::Left => self.step_days(-1),
            KeyCode::Right => self.step_days(1),
            KeyCode::Up => self.step_days(-7),
            KeyCode::Down => self.step_days(7),
            KeyCode::PageUp => self.step_month(-1),
            KeyCode::PageDown => self.step_month(1),
            KeyCode::Home => self.step_week(true),
            KeyCode::End => self.step_week(false),
            KeyCode::Enter | KeyCode::Space => {
                let day = self.cursor.day;
                return self.select(day);
            }
            _ => {}
        }
        None
    }
}

impl<M: 'static> Widget<M> for DatePicker<M> {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        DATE_PICKER_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("date", SkinValue::Text(text)) => Date::parse(text).is_some(),
            _ => true,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("date", SkinValue::Text(text)) => {
                if let Some(date) = Date::parse(text) {
                    self.set_date(date);
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
        self.open && self.day_at(pos) != self.hover
    }

    /// The popup owns the wheel while it is open, so a notch never scrolls the
    /// page under the calendar.
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
        let color = if self.enabled {
            palette.text
        } else {
            palette.text_disabled
        };
        ctx.text(
            Rect::new(
                self.rect.left + 9.0,
                self.rect.top,
                self.rect.right - 28.0,
                self.rect.bottom,
            ),
            &self.date.display(),
            TextStyleRole::Body,
            TextAlign::Left,
            color,
        );
        ctx.text(
            Rect::new(
                self.rect.right - 26.0,
                self.rect.top,
                self.rect.right - 6.0,
                self.rect.bottom,
            ),
            "\u{E787}", // Segoe MDL2 Calendar
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
        for (rect, glyph) in [
            (self.prev_rect(), '\u{E76B}'), // ChevronLeft
            (self.next_rect(), '\u{E76C}'), // ChevronRight
        ] {
            ctx.text(
                rect,
                &glyph.to_string(),
                TextStyleRole::Glyph,
                TextAlign::Center,
                palette.text,
            );
        }
        ctx.text(
            self.title_rect(),
            &format!("{} 年 {} 月", self.view_year, self.view_month),
            TextStyleRole::Body,
            TextAlign::Center,
            palette.text,
        );
        for (col, label) in WEEKDAYS.iter().enumerate() {
            ctx.text(
                self.weekday_rect(col),
                label,
                TextStyleRole::Small,
                TextAlign::Center,
                palette.muted,
            );
        }
        let lead = self.first_lead();
        let days = Date::days_in_month(self.view_year, self.view_month);
        let today = Date::today();
        for day in 1..=days {
            let index = lead + day as usize - 1;
            let row = index / GRID_COLS;
            if row >= GRID_ROWS {
                break;
            }
            let rect = self.cell_rect(row, index % GRID_COLS);
            let date = Date::new(self.view_year, self.view_month, day);
            let selectable = self.selectable(date);
            let selected = date == self.date;
            let cursor = self.cursor_date() == date;
            let hovered = selectable && self.hover == Some(day);
            let inner = rect.inset(2.0, 2.0);
            if selected {
                ctx.canvas.fill_round_rect(inner, 6.0, palette.accent);
            } else if hovered {
                ctx.canvas.fill_round_rect(inner, 6.0, palette.button_hover);
            }
            if cursor && !selected {
                let ring = if selectable {
                    palette.accent
                } else {
                    palette.muted
                };
                ctx.canvas.stroke_round_rect(inner, 6.0, 1.0, ring);
            }
            let color = if selected {
                palette.accent_text
            } else if !selectable || !self.enabled {
                palette.text_disabled
            } else if date == today {
                palette.accent
            } else {
                palette.text
            };
            ctx.text(
                rect,
                &day.to_string(),
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
                if self.prev_rect().contains_point(pos) {
                    self.step_month(-1);
                    return None;
                }
                if self.next_rect().contains_point(pos) {
                    self.step_month(1);
                    return None;
                }
                if let Some(day) = self.day_at(pos) {
                    return self.select(day);
                }
                None
            }
            WidgetInput::Activate => {
                self.open();
                None
            }
            WidgetInput::HoverAt(pos) => {
                if self.open {
                    let day = self.day_at(pos).filter(|day| {
                        self.selectable(Date::new(self.view_year, self.view_month, *day))
                    });
                    if day != self.hover {
                        self.hover = day;
                    }
                }
                None
            }
            WidgetInput::HoverLeave => {
                self.hover = None;
                None
            }
            WidgetInput::Key(key) if self.open => self.key(key.code),
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
            .value(self.date.display())
            .description("日期选择");
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

observer_builders!(DatePicker);
surface_builders!(DatePicker);

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

    fn picker() -> DatePicker<Date> {
        let mut picker = DatePicker::new("date", Date::new(2026, 11, 26), |date| date);
        picker.set_rect(Rect::from_xywh(20.0, 20.0, 200.0, 34.0));
        picker
    }

    #[test]
    fn date_math_handles_leap_years_and_weekdays() {
        assert_eq!(Date::days_in_month(2024, 2), 29);
        assert_eq!(Date::days_in_month(2025, 2), 28);
        assert_eq!(Date::days_in_month(2000, 2), 29);
        assert_eq!(Date::days_in_month(1900, 2), 28);
        // 2000-01-01 was a Saturday; 2024-02-29 a Thursday.
        assert_eq!(Date::new(2000, 1, 1).monday_index(), 5);
        assert_eq!(Date::new(2024, 2, 29).monday_index(), 3);
        // Crossing a month by days round-trips.
        let date = Date::new(2026, 1, 31);
        assert_eq!(date.add_days(1), Date::new(2026, 2, 1));
        assert_eq!(date.add_days(-30), Date::new(2026, 1, 1));
        assert_eq!(Date::new(2026, 13, 40), Date::new(2026, 12, 31));
        assert_eq!(Date::new(2026, 11, 26).display(), "2026-11-26");
    }

    #[test]
    fn the_calendar_opens_moves_and_selects_with_the_keyboard() {
        let mut picker = picker();
        assert!(!picker.is_open());
        // Enter (Activate) opens the grid on the current month.
        picker.input(WidgetInput::Activate);
        assert!(picker.is_open());
        assert_eq!(picker.view_month(), (2026, 11));
        assert_eq!(picker.cursor_date(), Date::new(2026, 11, 26));

        // One day right, then a week down.
        picker.input(WidgetInput::Key(key(KeyCode::Right)));
        assert_eq!(picker.cursor_date(), Date::new(2026, 11, 27));
        picker.input(WidgetInput::Key(key(KeyCode::Down)));
        assert_eq!(picker.cursor_date(), Date::new(2026, 12, 4));
        assert_eq!(picker.view_month(), (2026, 12));

        // PageUp goes back a month and keeps the day where possible.
        picker.input(WidgetInput::Key(key(KeyCode::PageUp)));
        assert_eq!(picker.view_month(), (2026, 11));
        assert_eq!(picker.cursor_date(), Date::new(2026, 11, 4));

        // Enter selects the cursor and closes.
        let message = picker.input(WidgetInput::Key(key(KeyCode::Enter)));
        assert_eq!(message, Some(Date::new(2026, 11, 4)));
        assert!(!picker.is_open());
        assert_eq!(picker.date(), Date::new(2026, 11, 4));
    }

    #[test]
    fn a_pointer_click_on_a_day_selects_it() {
        let mut picker = picker();
        picker.input(WidgetInput::Activate);
        assert!(picker.is_open());
        let rect = picker.day_cell(15);
        let center = rect.center();
        assert_eq!(
            picker.input(WidgetInput::ClickAt(center)),
            Some(Date::new(2026, 11, 15))
        );
        assert!(!picker.is_open());
        assert_eq!(picker.date(), Date::new(2026, 11, 15));
    }

    #[test]
    fn a_click_on_the_field_toggles_and_escape_closes() {
        let mut picker = picker();
        let field = picker.rect();
        picker.input(WidgetInput::ClickAt(field.center()));
        assert!(picker.is_open());
        // A second click on the field closes without changing the value.
        picker.input(WidgetInput::ClickAt(field.center()));
        assert!(!picker.is_open());
        assert_eq!(picker.date(), Date::new(2026, 11, 26));
        // The container calls `dismiss` on Escape/outside press.
        picker.input(WidgetInput::Activate);
        assert!(picker.is_open());
        assert!(picker.dismiss());
        assert!(!picker.is_open());
        assert!(!picker.dismiss());
    }

    #[test]
    fn the_month_arrows_step_without_selecting() {
        let mut picker = picker();
        picker.input(WidgetInput::Activate);
        let prev = picker.prev_rect().center();
        assert_eq!(picker.input(WidgetInput::ClickAt(prev)), None);
        assert_eq!(picker.view_month(), (2026, 10));
        assert!(picker.is_open());
        let next = picker.next_rect().center();
        picker.input(WidgetInput::ClickAt(next));
        assert_eq!(picker.view_month(), (2026, 11));
    }

    #[test]
    fn the_selectable_range_clamps_and_blocks_outside_days() {
        let mut picker = DatePicker::new("date", Date::new(2026, 11, 10), |date| date)
            .value_range(Some(Date::new(2026, 11, 5)), Some(Date::new(2026, 11, 15)));
        picker.set_rect(Rect::from_xywh(20.0, 20.0, 200.0, 34.0));
        assert!(picker.selectable(Date::new(2026, 11, 5)));
        assert!(!picker.selectable(Date::new(2026, 11, 4)));
        // Application values clamp into the range.
        assert!(picker.set_value(Date::new(2026, 10, 1)));
        assert_eq!(picker.value(), Date::new(2026, 11, 5));
        assert!(picker.set_value(Date::new(2026, 12, 31)));
        assert_eq!(picker.value(), Date::new(2026, 11, 15));

        // Keyboard movement clamps at the boundary.
        picker.set_value(Date::new(2026, 11, 14));
        picker.input(WidgetInput::Activate);
        picker.input(WidgetInput::Key(key(KeyCode::Right)));
        assert_eq!(picker.cursor_date(), Date::new(2026, 11, 15));
        picker.input(WidgetInput::Key(key(KeyCode::Right)));
        assert_eq!(picker.cursor_date(), Date::new(2026, 11, 15));

        // A click on an out-of-range day does not select.
        let outside = picker.day_cell(16).center();
        assert_eq!(picker.input(WidgetInput::ClickAt(outside)), None);
        assert!(picker.is_open());

        // A reversed range normalizes.
        let reversed = DatePicker::new("rd", Date::new(2026, 11, 10), |date| date)
            .value_range(Some(Date::new(2026, 12, 1)), Some(Date::new(2026, 11, 1)));
        assert_eq!(
            reversed.range(),
            (Some(Date::new(2026, 11, 1)), Some(Date::new(2026, 12, 1)))
        );
    }

    #[test]
    fn typed_access_sets_the_date_and_a11y_publishes_the_open_state() {
        let mut picker = picker();
        assert!(picker.set_date(Date::new(2024, 2, 29)));
        assert!(!picker.set_date(Date::new(2024, 2, 29)));
        assert_eq!(picker.date(), Date::new(2024, 2, 29));
        let info = Widget::<Date>::a11y(&picker).expect("a11y");
        assert_eq!(info.value.as_deref(), Some("2024-02-29"));
        assert_eq!(info.expanded, Some(false));
        picker.input(WidgetInput::Activate);
        let info = Widget::<Date>::a11y(&picker).expect("a11y");
        assert_eq!(info.expanded, Some(true));
        // The ExpandCollapse action mirrors the open state.
        assert_eq!(
            <DatePicker<Date> as Widget<Date>>::a11y_expand(&mut picker, None, false),
            None
        );
        assert!(!picker.is_open());
    }
}
