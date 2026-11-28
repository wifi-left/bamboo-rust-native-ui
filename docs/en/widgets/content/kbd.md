# Kbd

> A bordered chip that shows a keyboard shortcut such as `Ctrl+K`.

## Introduction

`Kbd` paints its text inside a small rounded chip: a 1 DIP border, the palette
`button_bg` fill, the small text role and a muted foreground. The radius uses
the theme's small radius. It is a fixed presentation; there are no size or
colour builders, and a bigger rectangle simply centers the same chip text.

The widget is decoration (`focusable` is `false`) and publishes a display-only
`Text` node named by the shortcut. It sends no message, so `M` is only the
container's message type.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::Kbd;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

fn build(ui: &mut Ui<Msg>) {
    ui.add(Kbd::new("kbd_search", "Ctrl+K"));
    ui.set_rect(WidgetId::new("kbd_search"), Rect::from_xywh(16.0, 16.0, 56.0, 22.0));
}
```

Use the `Ui::set_label` seam (or `set_text`) to update the shortcut at runtime;
`set_text` is the silent skin path used by the XML `shortcut` attribute.

## Usage (XML)

`kbd` is a standard registry type. Create it with `type="kbd"` when
`WidgetRegistry::standard()` is installed:

```xml
<widget id="kbd_search" type="kbd" x="16" y="16" w="56" h="22"
        shortcut="Ctrl+K"/>
```

The factory starts with empty text; `shortcut` supplies it. A Rust-created
`Kbd` is addressed by id:

```xml
<widget id="kbd_save" x="16" y="46" w="56" h="22" label="Ctrl+S"/>
```

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, text)` | `&'static str, impl Into<String>` | — | Creates the hint chip. |

Getters and setters: `text()` and `set_text(impl Into<String>)` (silent).

## Events

### Messages

None. `Kbd` only paints a shortcut chip and never returns a message.

### Common observers

`Kbd` does not install the shared observer builders. It is not focusable,
`interactive()` is `false` and it takes no input, so no pointer event reaches
it. Opt in with `WidgetProps::mouse(true)` and bind with `Ui::bind` /
`Ui::on_click` when the hint must be clickable.

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

By default the chip reacts to none of them; only a `WidgetProps::mouse(true)`
opt-in delivers `Click` and the pointer triggers to a binding.

## XML definition

`kbd` is a standard registry type and needs `WidgetRegistry::standard()`.
No `command` applies.

- Geometry: normal `widget` rules apply. The chip fills the rectangle; 56×22
  DIPs fits three or four characters comfortably.
- Shared content: `label` only.
- Local attributes: `shortcut` (text). `shortcut` and `label` both set the same
  displayed string.
- Style tokens: no `SurfaceStyle`; `radius`, `corners` and `background` are
  unsupported (`xml.style.unsupported`, a warning).
- Child elements: none.

## See also

- `../index.md`
- `badge.md`
- `../controls/button.md`
- `../../guides/xml-layout-authoring.md`
