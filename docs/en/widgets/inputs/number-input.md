# NumberInput

> An integer field with a range, optional stepper zones, fixed decimals and custom formatting.

## Introduction

`NumberInput<M>` paints a framed number field with a prefix and suffix around
the number and, with `.steppers()`, a pair of up/down zones on the right edge.
Clicking the text area opens a painted typing session that accepts digits, a
decimal point and (for a range with a negative minimum) a minus sign. Committing
parses the text into an integer, clamps it into the constructor range, and
emits `on_change(value)` when the value changed; unparsable text keeps the old
value. Clicking a stepper zone increments/decrements by `step_size` and emits
immediately; the stepper zones are click targets, so a press there never opens
the editor.

The value stays an integer. `.decimals(n)` changes only the display and parse
format (`value = 250` with 2 decimals paints `2.50`, typing `2.75` commits
`275`), so steppers and the accessibility range pattern keep working in raw
units. `.format`/`.parse` replace that fixed-point form with the application's
own text conversion.

The alias `NumberStepper<M>` is the same type (see
`bamboo_native_ui::widgets::NumberStepper`). Accessibility is
`A11yRole::NumberField` with the current value, range and `read_only` flag. `M`
is the message type returned by `on_change`.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::NumberInput;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Volume(i64) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        NumberInput::new("volume", 60, 0..=100, Msg::Volume)
            .suffix("%")
            .step_size(5)
            .steppers(),
    );
    ui.set_rect(WidgetId::new("volume"), Rect::from_xywh(16.0, 16.0, 120.0, 34.0));

    ui.add(NumberInput::new("price", 250, 0..=100_000, Msg::Volume).decimals(2).prefix("¥"));
}
```

The constructor clamps the initial value into the range. Pair `.format` with a
`.parse` that understands the same text; without a custom parser the fixed-point
form is used. `set_value` and `step` change the value silently; only the
steppers and a committed edit emit.

## Usage (XML)

`NumberInput` is not in the standard registry. Create it in Rust and address it
by `id` from the document:

```xml
<widget id="volume" x="16" y="16" w="120" h="34"
        number="60" suffix="%"/>
```

`number` is the shared content (`content_caps` exposes `NUMBER`) and is written
through `Ui::set_number`, clamped into the range. `prefix` and `suffix` are
local attributes. The field adopts `SurfaceStyle`, so `radius`/`corners` and
`background` apply.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, value, range, on_change)` | `&'static str, i64, RangeInclusive<i64>, impl Fn(i64) -> M + 'static` | — | Creates the field (value clamped into range). |
| `.steppers()` | — | off | Show the up/down zones on the right edge. |
| `.step_size(i64)` | `i64` | `1` | Increment used by the steppers (minimum 1). |
| `.decimals(u8)` | `u8` | `0` | Fixed decimal places for display and parsing (maximum 9). |
| `.prefix(impl Into<String>)` | `impl Into<String>` | `""` | Text painted before the number. |
| `.suffix(impl Into<String>)` | `impl Into<String>` | `""` | Text painted after the number. |
| `.format(Fn(i64) -> String)` | `impl Fn(i64) -> String + 'static` | `None` | Custom display text. |
| `.parse(Fn(&str) -> Option<i64>)` | `impl Fn(&str) -> Option<i64> + 'static` | `None` | Custom parser for committed text. |
| `.align(TextAlign)` | `TextAlign` | `TextAlign::Right` | Number alignment. |
| `.font_size(f32)` | `f32` | `None` (theme body) | Per-field text size in DIPs. |
| `.read_only(bool)` | `bool` | `false` | Refuse typing and steppers; keep focus/selection/copy. |
| `.disabled()` | — | enabled | Disable the field. |
| `.corner_radius(f32)` | `f32` | `0.0` | Field frame corner radius. |
| `.corners(Corners)` | `Corners` | zero | Per-corner frame radii. |
| `.background(Color)` | `Color` | `None` (palette `search_bg`) | Frame fill override. |
| `.background_option(Option<Color>)` | `Option<Color>` | `None` | Set or clear the fill override. |
| `.transparent_background(bool)` | `bool` | `false` | Suppress the fill, keeping the border. |

Getters and setters: `value()`, `display()`, `is_editing()`, `is_read_only()`,
`prefix_text()`, `suffix_text()`, `set_prefix`, `set_suffix`,
`set_value(i64) -> bool` (silent, clamped), `step(i64) -> bool` (silent,
steps by raw units within the range) and `surface()`.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(i64) -> M` passed to `new` (`on_change`) | A stepper click or a committed edit changes the value; the argument is the new clamped integer. Unparsable text keeps the old value and emits nothing. |

### Common observers

`NumberInput` does not install the shared observer builders. Its mechanism is
the constructor's `on_change` callback and the inline painted session; use
`Ui::bind` for additional triggers.

### Widget-specific callbacks

None beyond `on_change`. `.format`/`.parse` are value display/parse hooks and
`.set_value`/`.step` are silent setters, not event callbacks.

### Ui triggers

Any widget id can be bound with `Ui::bind(id, Trigger, handler)` (or the
`Ui::on_click` / `Ui::on_activate` shorthands). A handler that returns
`Some(M)` queues a message; the built-in action still runs unless the handler
calls `ctx.run_default()`.

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

The field reacts to `Click` on a stepper zone and `Activate` (opens the typing
session); `HoverEnter`/`HoverLeave` highlight the stepper under the pointer.
The wheel is not consumed. `ValueChange` follows a value change.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- Geometry: normal `widget` rules apply; the default height is 34 DIPs.
- Shared content: `number`.
- Local attributes: `prefix` and `suffix` (text).
- Style tokens: `radius`, `corners` and `background` are supported through
  `SurfaceStyle`; `font-size` and `font-family` apply.
- Child elements: none. The range, steppers, decimals and format/parse hooks
  are constructor-only.

## See also

- `../index.md`
- `text-edit.md`
- `../controls/slider.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
