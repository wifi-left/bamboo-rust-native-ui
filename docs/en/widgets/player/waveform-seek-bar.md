# WaveformSeekBar

> A seek bar whose track is drawn from normalized waveform samples.

## Introduction

`WaveformSeekBar<M>` paints one vertical bar per slot across its rectangle. The
samples are normalized amplitudes in `0..=1`; the played part is accent-colored,
the rest muted, and a playhead with a round handle marks the current position.
A hover marker previews where a press would seek.

A press seeks, every pointer move during the drag emits a new position (so the
player scrubs), and the release commits the position and stops dragging. When
`on_drag_start` / `on_drag` / `on_drag_end` are set, those messages ride beside
the primary seek message: the widget returns the seek result and hands the
extra message back through `take_extra_message`. The application owns playback:
call `set_progress` from a playback tick, which never emits.

When no samples are set the bar paints a flat center line, so the control still
reads as a seek bar before the decoder delivers a waveform.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::WaveformSeekBar;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Seek(f32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    WaveformSeekBar::new("waveform", 0.0, Msg::Seek)
        .samples([0.2, 0.6, 0.35, 0.9, 0.5, 0.75])
        .duration(255.0)
        .bar_width(2.0)
        .gap(1.0),
);
ui.set_rect(WidgetId::new("waveform"), Rect::from_xywh(16.0, 160.0, 520.0, 56.0));
```

`new` takes the initial position (clamped to `0..=1`) and the seek handler,
which receives the new fraction. `duration` only affects the accessible value
text: with a duration it reads `m:ss`, without one it reads a percentage. Push
playback updates without a message through typed access:

```rust
ui.edit_widget::<WaveformSeekBar<Msg>>(WidgetId::new("waveform"), |bar| {
    bar.set_progress(0.42);
    true
});
```

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. The shared `value` content sets the
position silently (`0..=1`); the widget declares no widget-local attributes and
no child elements.

```xml
<widget id="waveform" x="16" y="160" w="520" h="56" value="0.32"/>
```

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, progress, on_seek)` | `&'static str, f32, impl Fn(f32) -> M` | — | Creates the bar; `480 × 56` DIP. |
| `.samples(iter)` | `impl IntoIterator<Item = f32>` | empty | Amplitudes clamped to `0..=1`, one per bar slot. |
| `.bar_width(f32)` | `f32` | `2.0` | Width of one bar, clamped to `1..=12`. |
| `.gap(f32)` | `f32` | `1.0` | Space after each bar, clamped to `0..=8`. |
| `.colors(played, unplayed)` | `Color, Color` | accent / muted mix | Explicit bar colors. |
| `.duration(f32)` | `f32` | `None` | Track seconds (`> 0`); formats the accessible value as `m:ss`. |
| `.on_drag_start(handler)` | `Fn(f32) -> M` | none | Additive: fires with the press position when a scrub starts. |
| `.on_drag(handler)` | `Fn(f32) -> M` | none | Additive: fires on each scrub move. |
| `.on_drag_end(handler)` | `Fn(f32) -> M` | none | Fires with the committed position on release. |

Typed access and runtime setters:

| Method | Type | Description |
|---|---|---|
| `.set_samples(iter)` / `.samples_ref()` | `impl IntoIterator` / `&[f32]` | Replace or read the amplitudes. |
| `.set_duration(Option<f32>)` / `.duration_value()` | `Option<f32>` | Duration used for the value text. |
| `.progress()` / `.set_progress(f32)` | `f32` | Position `0..=1`; the setter never emits. |
| `.is_dragging()` / `.drag_preview()` | `bool` / `Option<f32>` | Whether a scrub is active and the painted preview. |

### Behaviour

- **Defaults.** Played color is `palette.accent`; unplayed is `palette.muted`
  mixed 35 % toward `palette.background`. The default rectangle is
  `480 × 56` DIP; `preferred_height` is 56.
- **Pointer.** A press seeks and starts a drag; every drag move emits. A press
  outside the ends clamps to `0.0` / `1.0`. The release commits the preview and
  is the only message when `on_drag_end` is set.
- **Keyboard.** `WaveformSeekBar::STEP` is `0.02`. Left/Right adjust by one
  step, Home/End jump to the ends, and a step at an edge emits nothing.
- **Accessibility.** Role `Slider` with a `0..=1` range and a value text of
  `m:ss` or percent. `a11y_set_value` accepts `"45%"` or `"0.45"` and clamps.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `impl Fn(f32) -> M` passed to `new` as `on_seek` | A press starts a scrub, every scrub move, and a keyboard step (`Left`/`Right`/`Home`/`End`/`Activate`); `f32` is the new `0..=1` position. |
| `impl Fn(f32) -> M` passed to `.on_drag_start` | A scrub starts, with the position under the press. |
| `impl Fn(f32) -> M` passed to `.on_drag` | Every pointer move while scrubbing. |
| `impl Fn(f32) -> M` passed to `.on_drag_end` | The release commits the position. |

The drag callbacks are additive: the widget returns the seek result and hands
the drag message back through `take_extra_message`. On release the drag-end
message becomes the primary message for that input. `set_progress` and
`a11y_set_value` update the position without emitting.

### Common observers

This widget installs none of the shared observer builders. Use the constructor
seek handler, the drag callbacks, or `Ui::bind`.

### Widget-specific callbacks

| Callback | Signature | Fires when |
|---|---|---|
| `new(id, progress, on_seek)` | `&'static str, f32, impl Fn(f32) -> M` | Press, scrub move or keyboard step. |
| `.on_drag_start(handler)` | `impl Fn(f32) -> M` | Once, with the position under the initial press. |
| `.on_drag(handler)` | `impl Fn(f32) -> M` | On each drag move while the button is held. |
| `.on_drag_end(handler)` | `impl Fn(f32) -> M` | On release, with the committed position. |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("waveform"), Trigger::Click, |event, ctx| {
    // Return Some(Msg::...) to queue a message; call ctx.default() to keep the
    // built-in action, which then runs after the handler.
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

The bar reacts to `Click`, `HoverEnter`/`HoverLeave`, `DragStart`/`DragEnd`,
`Activate` and `ValueChange`; a wheel event or right click over it does nothing.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules (one horizontal and one vertical
form). Generic properties (`visible`, `tooltip`, `name`, `description`,
`automation-id`, `role`, `expose`, focus policy) apply. Content support is the
shared `value` attribute only; `text`, `label`, `progress`, `number`, `checked`
and `selected` are rejected as `xml.content.unsupported`. The bar paints from
the palette and has no `SurfaceStyle`, so `radius`, `corners` and `background`
are reported as `xml.style.unsupported` and skipped.

Widget-local attributes: none. Child elements: none.

## See also

- `./playback-transport.md`
- `./playlist-view.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
