# MarqueeText

> A single-line title that scrolls when it does not fit.

## Introduction

`MarqueeText` paints one line of text. When the measured text fits the
rectangle it paints statically, left-aligned. When it does not, the text is
clipped and scrolled: it holds at the start for `pause` seconds, travels
`span + gap` DIPs at `speed` DIPs per second, holds at the end, then repeats.
Two copies are painted with `gap` DIPs between them so the wrap looks seamless.

The widget owns no timer. The application advances it from its animation tick
with `Ui::edit_widget::<MarqueeText>(id, |m| m.advance(dt))` and invalidates
only when `advance` returns `true` (the painted offset changed). It never takes
input and publishes a display-only `Text` node named by the full string.

## Usage (Rust)

```rust
use std::time::Duration;
use bamboo_native_ui::widgets::MarqueeText;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        MarqueeText::new("now_playing", "Now playing: 竹韵 · 夜曲 · Lossless")
            .speed(36.0)
            .gap(40.0)
            .pause(1.2),
    );
    ui.set_rect(WidgetId::new("now_playing"), Rect::from_xywh(16.0, 16.0, 240.0, 22.0));

    // Call this from the host's animation tick, ~60 times a second.
    ui.edit_widget::<MarqueeText>(WidgetId::new("now_playing"), |m| {
        m.advance(Duration::from_millis(16))
    });
}
```

A new text resets the animation to the start. `is_scrolling()` reports whether
the last paint measured an overflow, so a host can run the timer only while it
is needed.

## Usage (XML)

`MarqueeText` is not in the standard registry. Create it in Rust and address it
by `id` from the document:

```xml
<widget id="now_playing" x="16" y="16" w="240" h="22"
        text="Now playing: 竹韵 · 夜曲 · Lossless"
        offset="0" text-style="body"/>
```

`offset` seeds the painted offset; the application's next `advance` continues
from the time it keeps, so this is an initial value, not a seek.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, text)` | `&'static str, impl Into<String>` | — | Creates the line. |
| `.speed(f32)` | `f32` | `40.0` | Scroll speed in DIPs per second (minimum 1). |
| `.gap(f32)` | `f32` | `48.0` | Blank space between the end and the repeated start (minimum 0). |
| `.pause(f32)` | `f32` | `1.5` | Hold time at each end, in seconds (minimum 0). |
| `.style(LabelStyle)` | `LabelStyle` | `LabelStyle::Body` | Theme text role. |

Getters and setters: `text()`, `offset()`, `is_scrolling()`,
`set_offset(f32)`, `color_value()`, `text_style()`, `set_color(Option<Color>)`
and `set_text_style(LabelStyle)`. `advance(dt) -> bool` is the animation step.

## Events

### Messages

None. `MarqueeText` only paints; animation is driven by `advance(dt)`, not by
input.

### Common observers

`MarqueeText` does not install the shared observer builders. It never takes
input, is not focusable and `interactive()` is `false`, so no pointer or key
event reaches it. Use `Ui::edit_widget::<MarqueeText>` for the animation tick
instead of an event callback.

### Widget-specific callbacks

None. `advance(Duration) -> bool` is an animation step, not a callback; the
container calls it from its own timer.

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

The marquee reacts to none of them; it has no interactive path at all.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- Geometry: normal `widget` rules apply. Use a single-line height such as 22
  DIPs.
- Shared content: `text` and `label`.
- Local attributes: `offset` (a non-negative number, in DIPs), `color` and
  `text-style` (`body`, `small`, `title`).
- Style tokens: no `SurfaceStyle`; `radius`, `corners` and `background` are
  unsupported (`xml.style.unsupported`, a warning).
- Child elements: none.

## See also

- `../index.md`
- `label.md`
- `copyable-text.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
