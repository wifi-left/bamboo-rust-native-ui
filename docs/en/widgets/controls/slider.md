# Slider

> A horizontal value track with a knob, an optional value readout, tick marks and drag callbacks.

## Introduction

`Slider<M>` paints a rounded groove, an accent fill up to the current value and
a knob that grows on hover or press. It can draw evenly spaced ticks or ticks at
explicit values, a row of labels under them and a formatted value label above
the track. All of those are display only; the value grid is defined by `step`,
`snap` and `discrete`.

Pointer drags follow a three-stage lifecycle: `on_drag_start` on press,
`on_drag` on every move while held, and `on_drag_end` next to the usual
`on_change` on release. The widget keeps its own value; the application normally
mirrors it from the messages.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::Slider;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Volume(f32),
    VolumePreview(f32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Slider::new("volume", 0.6, 0.0..=1.0, Msg::Volume)
        .discrete(0.05)
        .ticks_at(&[0.0, 0.25, 0.5, 0.75, 1.0])
        .tick_labels(&["0", "25", "50", "75", "100"])
        .value_text(|value| format!("{:.0}%", value * 100.0))
        .on_drag(Msg::VolumePreview),
);
ui.set_rect(WidgetId::new("volume"), Rect::from_xywh(16.0, 16.0, 240.0, 44.0));
```

`on_change` is `Fn(f32) -> M`; the drag callbacks have the same shape. Because
`on_drag_end` reports beside `on_change`, a release delivers two messages:
`on_change(value)` first, then the `on_drag_end` message. The cell needs 44 DIP
height when tick labels are present (32 otherwise).

## Usage (XML)

`Slider` is created by the standard registry (`type="slider"`) and needs a
`command`; the standard factory needs `M: Clone` because every change re-reports
the same message.

```xml
<widget id="volume" type="slider" command="103"
        x="16" y="16" w="240" h="32" value="0.6" radius="medium"/>
```

The created slider is always `0.0..=1.0` with the default step; `value` is the
initial content value (clamped by the setter). Tick marks, labels and the value
readout are Rust-only. `Slider` declares no widget-local attributes, but it does
adopt `SurfaceStyle`, so `radius`, `corners` and `background` apply to the track.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, value, range, on_change)` | `&'static str, f32, RangeInclusive<f32>, impl Fn(f32) -> M` | — | Creates the slider; `240 × 32` DIP, `step = (max − min) / 100`. |
| `.step(f32)` | `f32` | `(max − min) / 100` | Increment for keyboard and snapped pointer input. |
| `.snap(bool)` | `bool` | `false` | Round pointer drags and UIA `SetValue` onto the step grid. |
| `.discrete(f32)` | `f32` | — | Sets `step` and enables `snap` in one call (ignored unless finite and > 0). |
| `.is_snapped()` / `.set_snap(bool)` | — / `bool` | `false` | Read or flip snapping at runtime (`set_snap` returns whether it changed). |
| `.step_size()` | — | — | The current increment. |
| `.ticks(usize)` | `usize` | `0` | Draws `count + 1` evenly spaced marks under the track; `0` hides them. |
| `.ticks_at(&[f32])` | `&[f32]` | empty | Explicit mark positions (clamped into the range); overrides `ticks`. |
| `.tick_labels(&[&str])` | `&[&str]` | empty | Labels under the track; follows `ticks_at` when the counts match, otherwise evenly spaced. |
| `.value_text(impl Fn(f32) -> String)` | closure | none | Formats a label painted at the top right. |
| `.tick_count()` / `.tick_values()` | — | — | Getters for the two tick settings. |
| `.value_label()` | — | — | The formatted value label, if configured. |
| `.on_drag_start(impl Fn(f32) -> M)` | closure | none | Called once on press with the value under the pointer. |
| `.on_drag(impl Fn(f32) -> M)` | closure | none | Called on every move while dragging. |
| `.on_drag_end(impl Fn(f32) -> M)` | closure | none | Called on release with the committed value. |
| `.is_dragging()` / `.drag_preview()` | — / `Option<f32>` | — | Whether a drag runs and the value it paints. |
| `.value()` / `.range()` | — / `(f32, f32)` | — | Current value and range. |
| `.set_range(min, max)` | `f32, f32` | — | Replaces the range (reversed pairs are swapped); clamps the value silently. |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | none | Additive click observer; `on_right_click`/`on_double_click`/focus/key observers exist beside it. |

### Behaviour

- **Snapping.** `quantize` clamps to the range and, when snapping is on, rounds
  to the nearest multiple of `step` measured from `min`. Application-driven
  `Ui::set_value` stays exact (it only clamps). The constructor does not reorder
  `range`, so pass `min <= max`; `set_range` swaps a reversed pair.
- **Dragging.** While dragging, the knob and fill follow the preview; the real
  value changes only on release. `on_drag_start`/`on_drag` are additive to that
  preview.
- **Keyboard.** Up/Right add one step, Down/Left subtract one, PageUp/PageDown
  add/subtract ten.
- **Surface.** `corner_radius` rounds the groove and fill, `background` overrides
  the groove color, `transparent_background(true)` hides the groove.
- **Accessibility.** Role `slider` with range and value; `value_text` becomes the
  UIA value string.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(f32) -> M` passed to `Slider::new` as `on_change` | A drag is released, a keyboard step changes the value, or a UIA `SetValue` lands on a new value. |

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

| Builder | Handler | Fires when |
|---|---|---|
| `.on_drag_start(f)` | `Fn(f32) -> M` | The pointer is pressed; receives the value under it. |
| `.on_drag(f)` | `Fn(f32) -> M` | Every pointer move while the button is held; receives the preview value. |
| `.on_drag_end(f)` | `Fn(f32) -> M` | The drag is released; receives the committed value, reported after `on_change`. |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("volume"), Trigger::Click, |event, ctx| {
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

The slider reacts to `Click` as a press/release drag, to `DragStart`/`DragEnd`
for pointer drags, to the arrow/PageUp/PageDown keys and to `ValueChange`; it
ignores `Wheel` and `DoubleClick` beyond the observers.

## XML definition

- **Creation:** `type="slider"` plus `command`. Missing command is
  `xml.command.missing`; `M: Clone` is required.
- **Geometry:** one horizontal form and one vertical form; prefer `h="32"` or
  `h="44"` so the knob and labels are not clipped.
- **Content:** `value` only (`content_caps = VALUE`) — it is an initial value,
  written on generation change.
- **Style:** `radius`, `corners`, `background`, `font-size`, `font-family`.
- **Properties:** the generic runtime set (`enabled`, `visible`, `tooltip`,
  `name`, …).
- **Children:** none.
- **Parent:** optional `parent="panel-id"`.

## See also

- `./progress-bar.md`
- `./pager.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
