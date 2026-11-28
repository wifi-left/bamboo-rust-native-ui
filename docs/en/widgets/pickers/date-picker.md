# DatePicker

> A date field that opens an in-window month calendar with an optional selectable range.

## Introduction

`DatePicker<M>` is a composite picker. The field paints the date as `YYYY-MM-DD`
and a calendar glyph; clicking it, or pressing Enter, opens a month-grid popup.
The popup has a header with previous/next month arrows and `YYYY 年 M 月`, a
Monday-first weekday row and a 7 × 6 day grid. The selected day is filled with
the accent color, today is tinted accent, the keyboard cursor is outlined, and
days outside the selectable range are painted disabled and cannot be chosen.

Choosing a day reports it through `on_change` and closes the popup. Escape, a
click on the field, or an outside press closes without changing the value. The
popup paints as an overlay and flips above the field when it does not fit below.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{Date, DatePicker};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    StartDate(Date),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    DatePicker::new("start", Date::today(), Msg::StartDate)
        .min_date(Date::new(2026, 1, 1))
        .max_date(Date::new(2026, 12, 31)),
);
ui.set_rect(WidgetId::new("start"), Rect::from_xywh(16.0, 16.0, 200.0, 34.0));
```

`on_change` is `Fn(Date) -> M`. The range is inclusive; `min_date`/`max_date` set
one side at a time and `value_range(Option<Date>, Option<Date>)` sets both. A
reversed pair passed to `set_range` is swapped so `min <= max`.

## Usage (XML)

`DatePicker` is not created by the standard registry. Create it in Rust and
address it by `id`; the document can seed the date and the open state:

```xml
<widget id="start" x="16" y="16" w="200" h="34"
        date="2026-11-28" open="false"/>
```

`date` must parse as `YYYY-MM-DD` via `Date::parse`; an invalid value is
`xml.skin.value` and nothing is applied. `open="true"` opens the calendar when
the generation is installed. The selectable range is Rust-only because it takes
`Date` values; set it with `min_date`/`max_date` at construction.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, date, on_change)` | `&'static str, Date, impl Fn(Date) -> M` | — | Creates the picker; `200 × 34` DIP, closed, no range. |
| `.enabled(bool)` / `.disabled()` | `bool` | `true` | Enable state; disabling closes the popup. |
| `.date()` / `.value()` | — | — | The current date. |
| `.set_date(Date)` / `.set_value(Date)` | `Date` | — | Replaces the date, clamped into the range; returns whether it changed. |
| `.range()` | — | `(None, None)` | The `(min, max)` bounds. |
| `.set_range(Option<Date>, Option<Date>)` | `Option<Date>, Option<Date>` | — | Inclusive range; swaps reversed bounds, clamps the value; returns whether anything changed. |
| `.value_range(Option<Date>, Option<Date>)` | `Option<Date>, Option<Date>` | — | Builder form of `set_range`. |
| `.min_date(Date)` / `.max_date(Date)` | `Date` | none | Builder form for one bound. |
| `.clear_range()` | — | — | Removes both bounds. |
| `.selectable(Date)` | `Date` | — | Whether a day can be chosen. |
| `.is_open()` | — | `false` | Popup state. |
| `.view_month()` | — | — | The displayed `(year, month)`. |
| `.open()` / `.close()` | — | — | Change the popup state; both return whether it changed. |
| `.corner_radius(f32)` / `.corners(Corners)` | — | zero | Rounds the field and focus ring. |
| `.background(Color)` / `.transparent_background(bool)` | — | theme | Field surface override. |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | none | Additive click observer; right/double-click, key and focus observers exist beside it. |

### Behaviour

- **Keyboard.** Left/Right move one day, Up/Down one week, PageUp/PageDown one
  month (clamped to the selectable range), Home/End to the start/end of the week,
  Enter/Space to select. Selected-cell coloring and the cursor ring are separate:
  moving the cursor does not change the value.
- **Range.** The current value and the cursor both clamp into the range,
  including when the range changes while the popup is open. A disabled picker
  still accepts `set_value` from the application; it only refuses user input.
- **Close semantics.** `close` discards the cursor and keeps the committed value.
  Choosing the day that is already selected still reports it; choosing a day
  outside the range reports nothing.
- **Accessibility.** Role `button`, value `YYYY-MM-DD`, expanded state, and
  `read_only` when disabled.

The `Date` value type has its own constructors and helpers:

| Item | Signature | Description |
|---|---|---|
| `Date::new` | `(year: i32, month: u32, day: u32) -> Date` | Clamps year to `>= 1`, month to `1..=12`, day to that month's length. |
| `Date::today` | `() -> Date` | Local date on Windows; `1970-01-01` elsewhere. |
| `Date::parse` | `(&str) -> Option<Date>` | Parses `YYYY-MM-DD`; rejects a month outside `1..=12` or day `0`. |
| `Date::display` | `(self) -> String` | `YYYY-MM-DD`. |
| `Date::add_days` | `(self, days: i64) -> Date` | The date `days` calendar days later or earlier. |
| `Date::first_of_month` | `(self) -> Date` | The first day of the same month. |
| `Date::days_in_month` | `(year, month) -> u32` | Month length, leap-year aware. |
| `Date::monday_index` | `(self) -> u32` | Weekday, `0` = Monday … `6` = Sunday. |

`Date` is `Copy + Ord`, and `Default` is 1970-01-01.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(Date) -> M` passed to `DatePicker::new` as `on_change` | A day is chosen by click or `Enter`/`Space`; the popup then closes. Choosing the already-selected day still reports. |

Days outside the selectable range report nothing.

### Common observers

This widget installs the shared observer builders. All are additive: the
built-in behaviour still runs after the handler returns.

| Builder | Handler | Fires when |
|---|---|---|
| `.on_click(f)` | `Fn(Point) -> Option<M>` | Left-button click on the widget. |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | Right-button click, before the context menu. |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | Platform double click. |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | Key press while the widget has focus. |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | Key release while the widget has focus. |
| `.on_focus_gained(f)` | `Fn(FocusEvent) -> Option<M>` | Focus moves onto the widget. |
| `.on_focus_lost(f)` | `Fn(FocusEvent) -> Option<M>` | Focus leaves the widget (after an editor commits). |

### Widget-specific callbacks

None. `on_change` is the only callback; the range builders — `value_range`,
`min_date`, `max_date`, `set_range`, `clear_range` — are configuration that
decides which days can be reported.

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("start"), Trigger::Click, |event, ctx| {
    // Return Some(Msg::...) to queue a message; the built-in action still runs
    // unless ctx.run_default() is called explicitly.
    None
});
```

| Trigger | Fires when |
|---|---|
| `Click` | Press and release on the same widget. |
| `DoubleClick` | The platform reports a double click. |
| `RightClick` | Secondary-button click, before the context menu. |
| `HoverEnter` / `HoverLeave` | The pointer enters / leaves the widget. |
| `Wheel` | A wheel event over the widget (`event.wheel_delta`). |
| `Activate` | Enter/Space on a focused widget, or a UIA Invoke/Toggle. |
| `DragStart` / `DragEnd` | The pointer crosses the drag threshold / the button is released. |
| `ValueChange` | The widget's accessibility value changed (after the built-in update). |

The picker reacts to `Click` (field, days and month arrows), `Activate` (opens
the popup), the arrow/PageUp/PageDown/Home/End/Enter keys while open, and
`HoverEnter`/`HoverLeave` for day hover; it ignores `Wheel`,
`DragStart`/`DragEnd` and `ValueChange`.

## XML definition

- **Creation:** no `type`; this widget is addressed only.
- **Geometry:** one horizontal form and one vertical form. The field prefers
  34 DIP height; the popup is painted outside the rectangle.
- **Content:** none of the shared content names apply; the date is the local
  attribute.
- **Local attributes:** `date` (`Text`, validated with `Date::parse`) and `open`
  (`Bool`).
- **Style:** `radius`, `corners`, `background`, `font-size`, `font-family`.
- **Properties:** the generic runtime set (`enabled`, `visible`, `tooltip`,
  `name`, `focus-order`, …).
- **Children:** none.

## See also

- `./time-picker.md`
- `./color-picker.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
