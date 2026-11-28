# Sparkline

> A compact bar strip that summarizes a short numeric series.

## Introduction

`Sparkline` paints one vertical bar per sample, bottom-aligned inside its
rectangle. Bars are scaled from the series minimum to its maximum, and the
reported range always includes the zero baseline. It is a display-only widget:
it is not focusable, takes no input and emits no message. It opts into the
accessibility tree as a text node that describes the sample count and range.

Use it for a trend beside a readout (bitrate history, volume changes, load
over time). For a single value use a `Gauge` or a progress ring; for a
clickable rating use `RatingStars`.

## Usage (Rust)

```rust
use bamboo_native_ui::{Color, Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::Sparkline;

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Sparkline::new("volume_history", &[3.0, 6.0, 4.0, 9.0, 12.0, 8.0])
        .gap(1.0)
        .color(Color::rgb([127, 211, 168])),
);
ui.set_rect(
    WidgetId::new("volume_history"),
    Rect::from_xywh(16.0, 72.0, 160.0, 36.0),
);
```

`Sparkline::new` copies the samples and defaults to the accent color with a
2-DIP gap. Bars are laid out across the rectangle; if the computed bar width
falls below half a DIP nothing is painted. Replace the series silently through
`set_data`, or update a live widget through the typed seam:

```rust
ui.edit_widget(WidgetId::new("volume_history"), |spark: &mut Sparkline| {
    spark.set_data(vec![4.0, 7.0, 11.0, 6.0]);
    true // the setter is silent; report the change to mark damage
});
```

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. The `<point>` child list replaces the
samples and the `color`/`gap` locals adjust its appearance:

```xml
<widget id="volume_history" x="16" y="72" w="160" h="36" color="accent" gap="1">
  <point value="3"/>
  <point value="6"/>
  <point value="4"/>
  <point value="9"/>
  <point value="12"/>
</widget>
```

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, data)` | `&'static str, &[f32]` | — | Creates the widget and copies the samples. |
| `.color(Color)` | `Color` | accent | Bar color. |
| `.gap(f32)` | `f32` | `2.0` | Space between bars in DIPs, clamped to `>= 0`. |
| `.data()` | `&[f32]` | — | The current samples. |
| `.set_data(Vec<f32>)` | `Vec<f32>` | — | Replaces the samples silently. |
| `.range()` | `(f32, f32)` | — | `(min, max)` of the samples including the zero baseline. |
| `.set_color(Option<Color>)` | `Option<Color>` | `None` | Sets the bar color; `None` restores the accent. Returns whether it changed. |
| `.color_value()` | `Option<Color>` | — | The explicit color, if any. |
| `.set_gap(f32)` | `f32` | `2.0` | Gap in DIPs (`>= 0`). Returns whether it changed. |
| `.gap_value()` | `f32` | — | The current gap. |

Behaviour: the widget requests 36 DIPs of preferred height, is not focusable,
and exposes a display-only `Text` accessibility node named by the widget id.
The description reports the sample count and the range from `range()`.

## Events

`Sparkline` is display-only: its `input` always returns `None`, so it emits no
message and fires no callback of its own.

### Messages

None. The constructor takes an id and the sample slice; samples are replaced
silently with `set_data` or through the `<point>` child list.

### Common observers

This widget does not install the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`). It is not focusable and consumes no input.

### Widget-specific callbacks

None.

### Ui triggers

Any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("volume_history"), Trigger::Click, |event, ctx| {
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

The sparkline reacts to none of them itself — it has no built-in behaviour — so
a binding is the only way to react to input over it.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules: exactly one horizontal form
(`x`+`w`, `right`+`w` or `x`+`right`) and one vertical form (`y`+`h`,
`bottom`+`h` or `y`+`bottom`). Generic properties (`visible`, `tooltip`,
`name`, `description`, `expose`, focus policy, cursor) apply. `font-size` and
`font-family` are paint-level. The widget paints from the palette and has no
`SurfaceStyle`, so `radius`, `corners` and `background` are reported as
`xml.style.unsupported` and skipped.

Local attributes:

| Attribute | Kind | Description |
|---|---|---|
| `color` | color | Bar color (palette token or literal). |
| `gap` | number | Space between bars in DIPs; must be `>= 0`. |

Child elements:

| Element | Attributes | Description |
|---|---|---|
| `<point>` | `value` (required, number) | One sample; the list replaces every sample. |

The child list is widget content: it is written only when the document
generation changes, is variant-invariant, and never fires a callback. There is
no shared `value` attribute; use `<point>` or `set_data`.

## See also

- `gauge.md`
- `rating-stars.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
