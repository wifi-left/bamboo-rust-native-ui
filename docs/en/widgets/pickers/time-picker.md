# TimePicker

> A `HH:MM` field that opens an in-window hour and minute popup with an optional selectable range.

## Introduction

`TimePicker<M>` is a composite picker. The field paints the time as `HH:MM` plus a
clock glyph. Enter opens a popup with two columns, labeled `时` (hour) and `分`
(minute), each showing a seven-row window centered on the current value. Rows
outside the selectable range are painted disabled and cannot be chosen.

The popup works on a working copy: clicking an hour picks it and moves the focus
to the minutes, clicking a minute commits and closes, and Enter/Space commits
the current pair. The application message is reported only when the committed
value differs from the value the popup opened with. Escape, or clicking the
field, closes and reverts to that opening value.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{Time, TimePicker};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Alarm(Time),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    TimePicker::new("alarm", Time::new(9, 30), Msg::Alarm)
        .min_time(Time::new(8, 0))
        .max_time(Time::new(18, 0)),
);
ui.set_rect(WidgetId::new("alarm"), Rect::from_xywh(16.0, 16.0, 110.0, 34.0));
```

`on_change` is `Fn(Time) -> M`. The range is inclusive; a reversed pair passed to
`set_range` is swapped. Times clamp into the range on `set_value`, on a range
change, and on every keyboard/pointer adjustment.

## Usage (XML)

`TimePicker` is not created by the standard registry. Create it in Rust and
address it by `id`; the document can seed the time and the open state:

```xml
<widget id="alarm" x="16" y="16" w="110" h="34"
        time="09:30" open="false"/>
```

`time` must parse as `HH:MM` via `Time::parse`; an invalid value is
`xml.skin.value` and nothing is applied. `open="true"` opens the popup when the
generation is installed. The range is Rust-only; set it at construction.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, time, on_change)` | `&'static str, Time, impl Fn(Time) -> M` | — | Creates the picker; `110 × 34` DIP, closed, no range. |
| `.enabled(bool)` / `.disabled()` | `bool` | `true` | Enable state; disabling closes the popup. |
| `.time()` / `.value()` | — | — | The current time. |
| `.set_time(Time)` / `.set_value(Time)` | `Time` | — | Replaces the time, clamped into the range; returns whether it changed. |
| `.is_open()` / `.open()` / `.close()` | — | `false` | Popup state. `close` reverts to the value the popup opened with. |
| `.range()` | — | `(None, None)` | The `(min, max)` bounds. |
| `.set_range(Option<Time>, Option<Time>)` | `Option<Time>, Option<Time>` | — | Inclusive range; swaps reversed bounds and clamps; returns whether anything changed. |
| `.value_range(Option<Time>, Option<Time>)` | `Option<Time>, Option<Time>` | — | Builder form of `set_range`. |
| `.min_time(Time)` / `.max_time(Time)` | `Time` | none | Builder form for one bound. |
| `.clear_range()` | — | — | Removes both bounds. |
| `.selectable(Time)` | `Time` | — | Whether a time can be chosen. |
| `.corner_radius(f32)` / `.corners(Corners)` | — | zero | Rounds the field and focus ring. |
| `.background(Color)` / `.transparent_background(bool)` | — | theme | Field surface override. |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | none | Additive click observer; right/double-click, key and focus observers exist beside it. |

### Behaviour

- **Keyboard.** Up/Down step the active unit by one, PageUp/PageDown by five,
  Home/End jump to `0`/the maximum, Left/Right switch between hours and minutes,
  Enter/Space commit. The columns render top-to-bottom increasing, so Down
  increases the value.
- **Pointer.** Clicking an hour sets it and moves to the minutes; clicking a
  minute commits and closes. Clicking the field while open cancels (reverts).
  The wheel steps the hovered column, or the active one when the pointer is
  outside a column.
- **Commit.** Closing through `commit` (Enter, Space, a minute click) reports
  `on_change` only when the value differs from the value at open. `close`
  (Escape, field click, outside press) reports nothing.
- **Accessibility.** Role `button`, value `HH:MM`, expanded state, and
  `read_only` when disabled.

The `Time` value type has its own constructors and helpers:

| Item | Signature | Description |
|---|---|---|
| `Time::new` | `(hour: u32, minute: u32) -> Time` | Clamps hour to `0..=23` and minute to `0..=59`. |
| `Time::now` | `() -> Time` | Local wall-clock time on Windows; `00:00` elsewhere. |
| `Time::parse` | `(&str) -> Option<Time>` | Parses `HH:MM`; rejects out-of-range parts. |
| `Time::display` | `(self) -> String` | `HH:MM`, 24-hour. |

`Time` is `Copy + Ord`, and `Default` is `00:00`.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(Time) -> M` passed to `TimePicker::new` as `on_change` | A commit (a minute click, or `Enter`/`Space`) finds a value different from the one the popup opened with. |

A value that is closed with Escape or a field click, or that equals the opening
value, reports nothing.

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
`min_time`, `max_time`, `set_range`, `clear_range` — are configuration that
decides which times can be chosen.

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("alarm"), Trigger::Click, |event, ctx| {
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

The picker reacts to `Click` (field and column rows), `Activate` (opens the
popup), the arrow/PageUp/PageDown/Home/End/Enter keys while open, `Wheel` while
open (steps the hovered or active column), and `HoverEnter`/`HoverLeave` for row
hover; it ignores `DragStart`/`DragEnd` and `ValueChange`.

## XML definition

- **Creation:** no `type`; this widget is addressed only.
- **Geometry:** one horizontal form and one vertical form. The field prefers
  34 DIP height; the popup is painted outside the rectangle.
- **Content:** none of the shared content names apply; the time is the local
  attribute.
- **Local attributes:** `time` (`Text`, validated with `Time::parse`) and `open`
  (`Bool`).
- **Style:** `radius`, `corners`, `background`, `font-size`, `font-family`.
- **Properties:** the generic runtime set (`enabled`, `visible`, `tooltip`,
  `name`, `focus-order`, …).
- **Children:** none.

## See also

- `./date-picker.md`
- `./color-picker.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
