# Divider

> A horizontal or vertical 1 DIP rule, optionally with a centered label.

## Introduction

`Divider` paints a single 1 DIP line across the middle of its rectangle. A
horizontal rule may carry a centered label: the line is split into two
segments with an 8 DIP gap around the text, which is painted small and muted.
A vertical rule is a plain column and ignores any label.

The widget is decoration: `focusable` is `false`. With a label it publishes a
display-only `Text` accessibility node named by the label; without one it is
absent from the tree. It never sends a message, so `M` is only the container's
message type.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::Divider;
use bamboo_native_ui::{Color, Rect, Ui, WidgetId};

enum Msg {}

fn build(ui: &mut Ui<Msg>) {
    ui.add(Divider::new("section_rule").label("Playback"));
    ui.set_rect(WidgetId::new("section_rule"), Rect::from_xywh(16.0, 16.0, 400.0, 16.0));

    ui.add(Divider::new("side_rule").vertical(true).color(Color::new(60, 64, 72)));
    ui.set_rect(WidgetId::new("side_rule"), Rect::from_xywh(430.0, 16.0, 1.0, 120.0));
}
```

A horizontal divider usually gets a short height (16 DIPs is the preferred
height); the line is drawn at the vertical center. A vertical divider gets a
narrow width (one DIP) and a useful height. `preferred_height` reports 16 for a
horizontal rule and 40 for a vertical one.

## Usage (XML)

`divider` is a standard registry type. Create it with `type="divider"` when
`WidgetRegistry::standard()` is installed:

```xml
<widget id="section_rule" type="divider" x="16" y="16" w="400" h="16"
        label="Playback"/>
```

A Rust-created divider is addressed by id:

```xml
<widget id="side_rule" x="430" y="16" w="1" h="120"/>
```

Only the label is expressible in XML; `vertical` and `color` are
constructor-only.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates a horizontal divider. |
| `.vertical(bool)` | `bool` | `false` | Draw a 1 DIP vertical column instead. |
| `.label(impl Into<String>)` | `impl Into<String>` | `None` | Centered label on a horizontal rule. |
| `.color(Color)` | `Color` | `None` (palette `divider`) | Line colour. |

The line colour, the label text and the label's muted colour follow the
palette unless `.color` overrides the line.

## Events

### Messages

None. The divider is pure decoration and never returns a message.

### Common observers

`Divider` does not install the shared observer builders. It is not focusable,
`interactive()` is `false` and its default `input` ignores every event, so no
pointer event is routed. Use `WidgetProps::mouse(true)` plus `Ui::bind` if a
click is really needed.

### Widget-specific callbacks

None.

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

The divider reacts to none of them by default; a `WidgetProps::mouse(true)`
opt-in is the only way to receive `Click` and the pointer triggers.

## XML definition

`divider` is a standard registry type and needs `WidgetRegistry::standard()`.
No `command` applies.

- Geometry: normal `widget` rules apply. Use a small `h` for a horizontal rule
  and a small `w` for a vertical one.
- Shared content: `label` only.
- Local attributes: none. The widget declares no `skin_attrs`, so `color` is
  not accepted in XML.
- Style tokens: no `SurfaceStyle`; `radius`, `corners` and `background` are
  unsupported (`xml.style.unsupported`, a warning).
- Child elements: none.

## See also

- `../index.md`
- `label.md`
- `../display/card.md`
- `../../guides/xml-layout-authoring.md`
