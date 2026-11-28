# Gauge

> An open-arc gauge that shows a value between zero and one.

## Introduction

`Gauge` paints a 270-degree dot ring. The full track is drawn in the scroll
track color and the value arc in the accent color, starting at 135 degrees and
sweeping clockwise with an opening at the bottom. The center optionally shows
a percentage or a custom label.

It is a display-only widget: it takes no input and emits no message, and it
opts into the accessibility tree as a display-only `ProgressBar` node with a
`0.0..=1.0` range. The application updates it in Rust; there is no XML value
attribute. For a linear readout use a progress bar, and for a compact history
use a `Sparkline`.

## Usage (Rust)

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::Gauge;

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(Gauge::new("load", 0.65).thickness(4.0).label("Load"));
ui.set_rect(WidgetId::new("load"), Rect::from_xywh(16.0, 16.0, 96.0, 96.0));
```

The constructor clamps the value to `0.0..=1.0`. `label` replaces the
percentage text painted in the center; without a label, `show_value(true)`
paints `"{n}%"`. Update the arc silently with `set_value` through the typed
seam:

```rust
ui.edit_widget(WidgetId::new("load"), |gauge: &mut Gauge| {
    gauge.set_value(0.72);
    true // report the silent setter so the widget is damaged
});
```

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. The document can position it and set
generic properties such as `expose` and `name`:

```xml
<widget id="load" x="16" y="16" w="96" h="96" expose="true" name="Load"/>
```

The value, thickness and label are Rust-side state; the document cannot set
them.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, value)` | `&'static str, f32` | — | Creates the gauge; the value is clamped to `0.0..=1.0`. |
| `.thickness(f32)` | `f32` | `4.0` | Ring thickness in DIPs, clamped to `1.0..=12.0`. |
| `.show_value(bool)` | `bool` | `true` | Paints the percentage in the center when there is no label. |
| `.label(impl Into<String>)` | text | `None` | Center text; replaces the percentage. |
| `.value_now()` | `f32` | — | The current value. |
| `.set_value(f32)` | `f32` | — | Sets the value silently, clamped to `0.0..=1.0`. |

Behaviour: the preferred height is 96 DIPs and the widget is not focusable.
The accessibility name is the label when set, otherwise the widget id; the
node is marked display-only and publishes `range(0.0, 1.0, value)` plus the
formatted percentage.

## Events

`Gauge` is display-only: its `input` always returns `None`, so it emits no
message and fires no callback of its own.

### Messages

None. The constructor takes an id and the value; updates go through the silent
`set_value` setter.

### Common observers

This widget does not install the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`). It is not focusable and consumes no input.

### Widget-specific callbacks

None.

### Ui triggers

Any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("load"), Trigger::Click, |event, ctx| {
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

The gauge reacts to none of them itself — it has no built-in behaviour — so a
binding is the only way to react to input over it. `ValueChange` never fires
because the reading only changes through the silent setter.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules (one horizontal and one vertical
form). Generic properties such as `visible`, `expose`, `name`, `description`
and `tooltip` apply. `font-size` and `font-family` are paint-level. The gauge paints from the palette and has no `SurfaceStyle`, so
`radius`, `corners` and `background` are reported as `xml.style.unsupported`
and skipped.

The widget declares no local attributes and no child elements. There is no
shared `value` content attribute, so a document cannot change the gauge's
reading; call `set_value` (or rebuild the widget) in Rust.

## See also

- `sparkline.md`
- `rating-stars.md`
- `../index.md`
- `../../api/widget-trait.md`
