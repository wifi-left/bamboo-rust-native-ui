# Splitter

> A draggable divider that reports where the pane boundary should move.

## Introduction

`Splitter<M>` paints a thin divider with three grip dots and reports pointer
drags to the application. It does not own or move the panes: the application
keeps the pane rectangles and re-lays them out from the position reported by
`on_drag`.

`Splitter::vertical` paints a vertical divider between left and right panes and
drags on the x axis; `Splitter::horizontal` paints a horizontal divider between
top and bottom panes and drags on the y axis. The divider is not a Tab stop and
has no keyboard adjustment, but it takes the pointer and exposes a
`Separator` accessibility role.

## Usage (Rust)

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::Splitter;

enum Msg {
    Split(f32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(Splitter::vertical("split", Msg::Split).label("Resize panes"));
ui.set_rect(WidgetId::new("split"), Rect::from_xywh(296.0, 16.0, 8.0, 300.0));
```

`on_drag` receives the new divider centre in client DIPs, with the offset
between the press point and the old centre preserved (pressing 2 DIPs right of
the centre keeps that 2-DIP offset). React by clamping to pane minimums and
assigning the pane and splitter rectangles:

```rust
fn split_at(ui: &mut Ui<Msg>, centre: f32) {
    let left_width = (centre - 16.0).clamp(120.0, 300.0);
    ui.set_rect(WidgetId::new("left"), Rect::from_xywh(16.0, 16.0, left_width, 300.0));
    ui.set_rect(WidgetId::new("split"), Rect::from_xywh(16.0 + left_width + 4.0, 16.0, 8.0, 300.0));
}
```

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. It accepts geometry, generic properties
and an accessible name:

```xml
<widget id="split" x="296" y="16" w="8" h="300" name="Resize panes"/>
```

The callback and the orientation are constructor arguments; the document cannot
change them.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `Splitter::vertical(id, on_drag)` | `&'static str, impl Fn(f32) -> M` | — | Vertical divider (8×200) that drags horizontally. |
| `Splitter::horizontal(id, on_drag)` | `&'static str, impl Fn(f32) -> M` | — | Horizontal divider (200×8) that drags vertically. |
| `.label(impl Into<String>)` | text | widget id | Accessible name. |
| `.disabled(bool)` | `bool` | `false` | Ignores the pointer and paints the plain divider color. |
| `.is_active()` | `bool` | — | Whether a drag is in flight. |
| `.is_vertical()` | `bool` | — | Divider orientation. |

Behaviour: the strip paints a 1-DIP center line plus three 3×3 DIP grip dots.
The color is the divider token normally, `scroll_thumb_hover` on hover and the
accent while dragging (a disabled splitter stays on the divider color). The
cursor is `SizeWE` for a vertical divider and `SizeNS` for a horizontal one.
`on_drag` fires only while a press is held; a drag without a press emits
nothing. The preferred height is 200 DIPs for a vertical divider and 8 DIPs for
a horizontal one. The accessibility role is `Separator`.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(f32) -> M` passed to `Splitter::vertical` / `Splitter::horizontal` as `on_drag` | The pointer moves while the divider is pressed; `f32` is the new divider centre in client DIPs. The grab offset from the press point is preserved. |

A drag without a press, or any keyboard input, emits nothing.

### Common observers

This widget does not install the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`). The constructor's `on_drag` is its event
seam; anything else goes through `Ui::bind`.

### Widget-specific callbacks

| Method | Handler | Fires when |
|---|---|---|
| `Splitter::vertical(id, on_drag)` | `&'static str, impl Fn(f32) -> M + 'static` | Stores the drag callback for a horizontal drag (vertical divider). |
| `Splitter::horizontal(id, on_drag)` | `&'static str, impl Fn(f32) -> M + 'static` | Stores the drag callback for a vertical drag (horizontal divider). |

Both constructors declare the same callback field; orientation only changes
which axis is reported. `is_active()` reports whether a drag is in flight.

### Ui triggers

Any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("split"), Trigger::DragStart, |event, ctx| {
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

It reacts to `HoverEnter`/`HoverLeave` (hover color) and `DragStart`/`DragEnd`
(the drag session), with the drag positions delivered through `on_drag`. It is
not focusable, so `Activate` never fires, and it carries no value, so neither
does `ValueChange`; `Click`, `DoubleClick`, `RightClick` and `Wheel` have no
built-in effect.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules; give a vertical divider a narrow
width and a horizontal divider a small height. Generic properties apply,
including `enabled` and `name`. `font-size` and `font-family` are paint-level.
The splitter paints from the palette and has no `SurfaceStyle`, so `radius`,
`corners` and `background` are reported as `xml.style.unsupported` and
skipped.

The widget declares no local attributes and no child elements. Orientation and
the drag callback are fixed at construction.

## See also

- `nav-rail.md`
- `../../api/widget-trait.md`
- `../index.md`
