# CopyableText

> Read-only text with a copy affordance and an application copy message.

## Introduction

`CopyableText<M>` paints a line of text and, when `.copyable(handler)` is set,
a copy glyph on the right. Clicking the widget, activating it, or pressing
Ctrl+C while it is focused emits `handler()`. The widget never touches the
platform clipboard: the application writes the clipboard from the message (for
example with `WindowHandle::copy_text`), which keeps the widget testable and
platform-neutral.

Without a handler the text is plain decoration and the glyph is not painted.
With a handler the instance is pointer-interactive and shows the hand cursor;
the glyph turns accent-coloured on hover. `M` is the message type returned by
the copy handler.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::CopyableText;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { CopyDeviceId }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        CopyableText::new("device_id", "Bamboo-7F3A")
            .copyable(|| Msg::CopyDeviceId),
    );
    ui.set_rect(WidgetId::new("device_id"), Rect::from_xywh(16.0, 16.0, 200.0, 22.0));
}
```

Handle the message by writing the same string to the clipboard. Make the
instance `focusable` (through `WidgetProps`) when Ctrl+C from the keyboard
should work; the pointer click works without it.

## Usage (XML)

`CopyableText` is not in the standard registry. Create it in Rust and address
it by `id` from the document:

```xml
<widget id="device_id" x="16" y="16" w="200" h="22" text="Bamboo-7F3A"/>
```

Both `text` and `label` are accepted as shared content. The copy handler is
constructor-only; a `type` entry cannot create this widget.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, text)` | `&'static str, impl Into<String>` | — | Creates the text line. |
| `.copyable(handler)` | `impl Fn() -> M + 'static` | `None` | Paint the glyph and emit on click/activate/Ctrl+C. |
| `.style(LabelStyle)` | `LabelStyle` | `LabelStyle::Small` | Theme text role. |

Getters and setters: `text()`, `color_value()`, `text_style()`,
`set_color(Option<Color>)` and `set_text_style(LabelStyle)`.

Behaviour notes:

- The copy glyph is `U+E8C8` and occupies `min(height, 18)` DIPs on the right;
  the text box ends 4 DIPs before it.
- `interactive()` and `cursor()` are set only when a handler is attached.
- Accessibility: `A11yRole::Text`, display-only, named by the text.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn() -> M` passed to `.copyable(handler)` | The copy glyph is clicked, the text is activated (Enter/Space), or Ctrl+C is pressed while it has focus. |

### Common observers

`CopyableText` does not install the shared observer builders. Its only
callback is the widget-specific `.copyable(handler)`; without it `input`
returns `None`, `interactive()` is `false` and no glyph is painted.

### Widget-specific callbacks

| Callback | Handler | Fires when |
|---|---|---|
| `.copyable(handler)` | `impl Fn() -> M + 'static` | Click on the line, `Activate` (Enter/Space), or Ctrl+C while focused. The widget never writes the clipboard itself. |

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

With a handler attached, `Click` and `Activate` emit the copy message and
`HoverEnter`/`HoverLeave` tint the glyph; Ctrl+C is a plain key event, not a
trigger. Without a handler none of the pointer triggers are delivered.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- Geometry: normal `widget` rules apply.
- Shared content: `text` and `label`.
- Local attributes: `color` and `text-style` (`body`, `small`, `title`).
- Style tokens: no `SurfaceStyle`; `radius`, `corners` and `background` are
  unsupported (`xml.style.unsupported`, a warning).
- Child elements: none.

## See also

- `../index.md`
- `label.md`
- `marquee-text.md`
- `../../guides/xml-layout-authoring.md`
