# RatingStars

> A clickable star rating with hover highlighting and keyboard adjustment.

## Introduction

`RatingStars<M>` paints a row of five-pointed stars, filled up to the current
value. Clicking a star rates that many stars, hovering highlights the value
under the pointer, and in adjust mode the arrow keys, Home and End change it.
It is a real control: it reports the new rating through the `on_rate` closure
as the application's message `M`.

The widget occupies a `Slider` accessibility role with a `0..=stars` range and
supports `SetValue` from assistive technology. A read-only instance still
paints and publishes its value but ignores clicks, hover and keys.

## Usage (Rust)

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::RatingStars;

enum Msg {
    Rate(u8),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    RatingStars::new("rating", 5, 3, Msg::Rate)
        .clearable(true)
        .read_only(false),
);
ui.set_rect(WidgetId::new("rating"), Rect::from_xywh(16.0, 16.0, 130.0, 28.0));
```

`on_rate` runs on every accepted change, including the clear gesture. With
`clearable(true)`, clicking the star that equals the current value resets the
rating to zero and reports `on_rate(0)`. `set_rating` is the silent path for
pushing a stored value:

```rust
ui.edit_widget(WidgetId::new("rating"), |stars: &mut RatingStars<Msg>| {
    stars.set_rating(4);
    true // report the silent setter so the widget is damaged
});
```

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. The shared `value` content attribute sets
the rating silently; generic properties can lock or disable it:

```xml
<widget id="rating" x="16" y="16" w="130" h="28" value="3" name="Rating"/>
```

`value` is rounded and clamped to the star count (a value above the count fills
every star). It does not fire `on_rate`, so the initial value is written once
per document generation without a callback.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, stars, value, on_rate)` | `&'static str, usize, u8, impl Fn(u8) -> M` | — | Creates the widget; `stars` is at least 1 and `value` is clamped to it. |
| `.clearable(bool)` | `bool` | `false` | Clicking the current value clears the rating to zero. |
| `.read_only(bool)` | `bool` | `false` | Ignores clicks, hover and keys but keeps painting the value. |
| `.disabled(bool)` | `bool` | `false` | Disables the widget (paints disabled colors, ignores input). |
| `.value()` | `u8` | — | The current rating. |
| `.hover_value()` | `Option<u8>` | `None` | Hovered value under the pointer. |
| `.set_rating(u8)` | `u8` | — | Sets the rating silently, clamped to the star count. |
| `.rate(u8)` | `u8` | — | Rates through the normal path and returns the message; `None` when disabled, read-only or unchanged. |

Behaviour: the preferred height is 28 DIPs. Mouse input is divided into equal
star cells; a click maps to the cell under the pointer. Keys: Left/Down
decrement, Right/Up increment, Home sets zero and End sets the star count. No
message is produced when the value does not change. The whole rectangle is the
tooltip anchor.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(u8) -> M` passed to `new` as `on_rate` | An accepted rating change: a click on a star, an arrow/Home/End key in adjust mode, a UIA range request, or the clear gesture (`0`). Silent paths (`set_rating`, the XML `value` attribute) never fire it. |

### Common observers

This widget does not install the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`). The constructor's `on_rate` is its event
seam; anything else goes through `Ui::bind`.

### Widget-specific callbacks

| Method | Handler | Fires when |
|---|---|---|
| `new(id, stars, value, on_rate)` | `&'static str, usize, u8, impl Fn(u8) -> M + 'static` | Stores the rating callback; it runs whenever `rate` accepts a change. |
| `.rate(u8)` | returns `Option<M>` | Runtime form: rates normally and returns the message, or `None` when disabled, read-only or unchanged. |

### Ui triggers

Any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("rating"), Trigger::ValueChange, |event, ctx| {
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

It reacts to `Click` (rate), `HoverEnter`/`HoverLeave` (preview) and
`ValueChange` (range changed); the arrow, Home and End keys change the value in
adjust mode. `Activate`, `DoubleClick`, `RightClick`, `Wheel` and the drag
triggers have no built-in meaning here.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties apply:
`enabled` suppresses input and paints the disabled colors, and `name`
overrides the accessibility name (default: widget id). The generic `read-only`
property is published to assistive technology but does not lock this widget;
call the Rust `.read_only(true)` builder to lock the control. `font-size` and
`font-family` are paint-level. The widget paints from the palette and has no
`SurfaceStyle`, so `radius`, `corners` and `background` are reported as
`xml.style.unsupported` and skipped.

Local attributes and child elements: none. The only content attribute is the
shared `value` (number). `clearable` and the rate callback are Rust-only.

## See also

- `sparkline.md`
- `gauge.md`
- `../index.md`
- `../../api/widget-trait.md`
