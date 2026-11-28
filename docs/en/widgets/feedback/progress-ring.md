# ProgressRing

> A circular determinate progress indicator with an optional percentage.

## Introduction

`ProgressRing` paints a full circular track and an accent arc proportional to
its value between `0.0` and `1.0`. The arc is drawn as a ring of overlapping
dots (about sixty for a full circle), so it needs no arc primitive and no
assumption about the page colour. The track uses the palette's scroll-track
colour, the arc the accent colour, and the dots start at twelve o'clock and
sweep clockwise.

The widget is display-only: it is not focusable, takes no input and emits no
message. Set the value from the application, usually through the typed access
seam. `show_value(true)` paints `"{:.0}%"` centred in the ring.

It reports an accessibility `ProgressBar` node with a `0.0..=1.0` range and the
formatted percentage as its value.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::ProgressRing;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(ProgressRing::new("import", 0.35).thickness(5.0).show_value(true));
ui.set_rect(WidgetId::new("import"), Rect::from_xywh(16.0, 16.0, 56.0, 56.0));
```

`set_value` is silent, so the typed seam must report the change for the ring to
be damaged and repainted:

```rust
ui.edit_widget::<ProgressRing>(WidgetId::new("import"), |ring| {
    ring.set_value(0.72);
    true
});
```

`Ui::set_value` is not wired to this widget: the ring reports no `VALUE`
content capability, so nothing happens on that path.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. The document can position it and set
generic properties:

```xml
<widget id="import" x="16" y="16" w="56" h="56" name="Import progress"/>
```

The value, thickness and percentage readout are Rust-side state; the document
cannot set them even though the shared content vocabulary has a `value` name.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, value)` | `&'static str, f32` | — | Creates a `56 × 56` ring; the value is clamped to `0.0..=1.0`. |
| `.value(f32)` | `f32` | — | Builder form of the value, clamped to `0.0..=1.0`. |
| `.thickness(f32)` | `f32` | `5.0` | Dot diameter in DIPs, clamped to `1.0..=12.0`. |
| `.show_value(bool)` | `bool` | `false` | Paints the percentage in the centre. |
| `.value_now()` | `f32` | — | The current value. |
| `.set_value(f32)` | `f32` | — | Silent setter, clamped to `0.0..=1.0`. |

### Behaviour

- `thickness` is the dot size, not a stroke width; the dot spacing follows the
  radius, so a large ring stays smooth and a short value arc costs fewer draws.
- A value of `0.0` draws the track only.
- The preferred height is 56 DIPs and the widget is not focusable.
- Accessibility: role `progress-bar`, `display_only`, `range(0.0, 1.0, value)`
  and `value("{n}%")`; named by the widget id unless `WidgetProps::name` is set.

## Events

The ring is display-only: its `input` always returns `None`, so it emits no
message and fires no callback of its own. Progress is application-driven.

### Messages

None. The constructor takes no callback.

### Common observers

This widget installs none of the shared observer builders. It is not focusable
and consumes no input, so `on_click`, `on_right_click`, `on_double_click`,
`on_key_down`, `on_key_up`, `on_focus_gained` and `on_focus_lost` do not exist
on it.

### Widget-specific callbacks

None. Set the value with `set_value(f32)` (or the `.value(f32)` builder) through
`Ui::edit_widget`; the closure returning `true` is what marks the ring damaged,
because the setter is silent.

### Ui triggers

Any widget id can be bound with `Ui::bind(WidgetId::new("import"), Trigger::HoverEnter, |event, ctx| ...)`;
returning `Some(Msg::...)` queues a message, and the built-in behaviour still
runs after the handler returns (`ctx.run_default()` runs it inline, calling
neither suppresses it). The ring has no built-in behaviour, so a binding is the
only way to react.

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

Only the pointer triggers are meaningful for the ring; `Activate`, the key and
focus triggers never fire because it is not focusable, and `ValueChange` does not
fire on `set_value` — the ring reports an accessibility value but does not
announce the change.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- **Geometry:** normal `widget` rules apply; a square is the natural shape.
- **Content:** none. `value` is rejected with `xml.content.unsupported` (a
  warning) because `content_caps` is `NONE`.
- **Local attributes:** none. `skin_attrs()` is empty.
- **Style tokens:** `font-size` and `font-family` apply (paint-level only); the
  widget has no `SurfaceStyle`, so `radius`, `corners` and `background` are
  `xml.style.unsupported` warnings and are skipped.
- **Children:** none.
- **Properties:** the generic runtime set (`visible`, `enabled`, `tooltip`,
  `name`, `expose`, `automation-id`, …) applies as for any addressed widget.

## See also

- `spinner.md`
- `../controls/progress-bar.md`
- `../data/gauge.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
