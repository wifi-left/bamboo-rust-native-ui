# Spinner

> An activity indicator drawn as a rotating ring of twelve dots.

## Introduction

`Spinner` paints twelve dots on a circle with a fading tail. The head dot uses
the accent colour (or an explicit one) and every dot behind it is mixed toward
the page background, so the ring reads as rotation without an arc primitive and
without a timer of its own.

The widget is display-only: it is not focusable, ignores input and emits no
message. The application owns the clock and advances the painted phase from its
frame timer with `advance(dt)`, so an idle page still does no periodic work.
This is the same app-driven model as `MarqueeText` and the indeterminate
`ProgressBar`.

The accessibility node is a display-only `Text` named by the widget id; the
instance joins the tree only when it opts in, and `WidgetProps::name` gives it a
useful announcement.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::Spinner;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(Spinner::new("busy").speed(360.0));
ui.set_rect(WidgetId::new("busy"), Rect::from_xywh(16.0, 16.0, 24.0, 24.0));
```

Drive it from the host's animation tick. `dt` is the time since the previous
tick; `advance` reports whether the painted phase changed, so a paused or
zero-length tick records no damage:

```rust
fn tick(ui: &mut Ui<Msg>, dt: std::time::Duration) {
    ui.edit_widget::<Spinner>(WidgetId::new("busy"), |spinner| spinner.advance(dt));
}
```

A 24 DIP square is the natural size; the ring inscribes itself in the shorter
side of whatever rectangle you assign.

## Usage (XML)

`spinner` is a standard registry type. Create it with `type="spinner"` when
`WidgetRegistry::standard()` is installed on the `Ui`:

```xml
<widget id="busy" type="spinner" x="16" y="16" w="24" h="24"
        phase="90" animating="true"/>
```

The factory starts a default spinner; the file supplies the initial phase and
animation flag. `speed` and `color` are constructor-only and stay in Rust.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates a `24 × 24` spinner at phase 0. |
| `.speed(f32)` | `f32` (degrees/second) | `300.0` | Rotation speed, clamped to at least `1.0`. |
| `.color(Color)` | `Color` | palette `accent` | Dot colour; the tail still fades toward the background. |
| `.phase()` | `f32` | `0.0` | Current phase in degrees. |
| `.set_phase(f32)` | `f32` | — | Sets the phase, wrapped into `0..360` with `rem_euclid`. |
| `.is_animating()` | `bool` | `true` | Whether `advance` changes the phase. |
| `.set_animating(bool)` | `bool` | — | Pauses or resumes without losing the painted phase. |
| `.advance(Duration)` | `Duration` | — | Adds `speed × dt` degrees; returns whether the change exceeds `0.001°`. |

### Behaviour

- The ring is always twelve dots 30° apart, so paint cost is constant; the dot
  diameter is derived from the radius and clamped to `1.5..3.0` DIPs.
- The preferred height is 24 DIPs and the widget is not focusable.
- `advance` returns `false` when paused or when the step is at most `0.001°`
  (a no-op tick), and the clock keeps running when the head passes 360°.

## Events

The spinner is display-only: its `input` always returns `None`, so it emits no
message and fires no callback of its own. Animation is application-driven, and
`advance(dt)` returning `true` is the only "changed" signal the widget produces.

### Messages

None. The constructor takes no callback.

### Common observers

This widget installs none of the shared observer builders. It is not focusable
and consumes no input, so `on_click`, `on_right_click`, `on_double_click`,
`on_key_down`, `on_key_up`, `on_focus_gained` and `on_focus_lost` do not exist
on it.

### Widget-specific callbacks

None. Drive the widget with `set_phase(degrees)`, `set_animating(bool)` and
`advance(dt)` through `Ui::edit_widget`.

### Ui triggers

Any widget id can be bound with `Ui::bind(WidgetId::new("busy"), Trigger::HoverEnter, |event, ctx| ...)`;
returning `Some(Msg::...)` queues a message, and the built-in behaviour still
runs after the handler returns (`ctx.run_default()` runs it inline, calling
neither suppresses it). On a spinner the built-in behaviour is always "nothing",
so a binding is the only way to react.

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

Every trigger reaches a binding on the id, but only the pointer triggers
(`Click`, `HoverEnter`/`HoverLeave`) describe anything the spinner does;
`Activate`, the key and focus triggers never fire because it is not focusable,
and `ValueChange` never fires because `advance` reports no accessibility value.

## XML definition

- **Creation:** `type="spinner"`, no `command` needed. Without a standard
  registry the entry is `xml.type.unknown`. A created widget lands in the page
  layer, before the first backdrop (a `Scrim`).
- **Geometry:** normal `widget` rules apply (one horizontal and one vertical
  form). The ring inscribes itself in the assigned rectangle.
- **Content:** none. `label` and `text` are unsupported (`xml.content.unsupported`,
  a warning) and skipped.
- **Local attributes:** `phase` (number, degrees) and `animating` (bool). An
  unknown name is `xml.skin.unknown`.
- **Style tokens:** `font-size` and `font-family` are accepted but paint nothing
  (the spinner draws no text); the widget has no `SurfaceStyle`, so `radius`,
  `corners` and `background` are `xml.style.unsupported` warnings and are
  skipped.
- **Children:** none. An optional `parent="panel-id"` binds a created entry to a
  registered scroll panel.

## See also

- `progress-ring.md`
- `../controls/progress-bar.md`
- `../data/gauge.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
- `../../api/widget-trait.md`
