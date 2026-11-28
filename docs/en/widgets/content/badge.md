# Badge

> A small status chip with a tone colour and an optional remove target.

## Introduction

`Badge<M>` paints a pill-shaped status chip: small text on a tinted fill, with
an optional close glyph (`×`) on the right. Five tones are available.
`Neutral` and `Accent` follow the palette; `Success`, `Warning` and `Danger`
use fixed colours for both light and dark palettes, so they read the same
regardless of the accent setting.

A plain badge is decoration (`focusable` is `false`, the a11y node is a
display-only `Text`). `.removable(handler)` makes the instance interactive
because it owns a close target: clicking the `×` or pressing Delete/Backspace
while the instance is focused emits the handler's message.
The hover tint is limited to the close target, not the whole chip. `M` is the
message type returned by the remove handler.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{Badge, BadgeTone};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { RemoveTag }

fn build(ui: &mut Ui<Msg>) {
    ui.add(Badge::new("lossless", "Lossless").tone(BadgeTone::Success));
    ui.set_rect(WidgetId::new("lossless"), Rect::from_xywh(16.0, 16.0, 80.0, 22.0));

    ui.add(
        Badge::new("flac", "FLAC")
            .tone(BadgeTone::Neutral)
            .removable(|| Msg::RemoveTag),
    );
    ui.set_rect(WidgetId::new("flac"), Rect::from_xywh(16.0, 46.0, 72.0, 22.0));
}
```

The removable badge reserves room for its close target, so give it enough
width for the text plus one box height. The chip radius is always half the
height.

## Usage (XML)

`badge` is a standard registry type. Create it with `type="badge"` when
`WidgetRegistry::standard()` is installed:

```xml
<widget id="lossless" type="badge" x="16" y="16" w="80" h="22"
        label="Lossless" tone="success"/>
```

The factory starts with empty text; the `label` content fills it. A badge
created in Rust is addressed by id:

```xml
<widget id="flac" x="16" y="46" w="72" h="22" label="FLAC"/>
```

`tone` accepts `neutral`, `accent`, `success`, `warning` or `danger`. The
removable handler is constructor-only and cannot be declared in XML.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, text)` | `&'static str, impl Into<String>` | — | Creates the chip. |
| `.tone(BadgeTone)` | `BadgeTone` | `BadgeTone::Neutral` | Emphasis colour. |
| `.removable(handler)` | `impl Fn() -> M + 'static` | `None` | Paint a close target and emit on remove. |

Getters and setters: `text()`, `is_removable()`, `tone_value()` and
`set_tone(BadgeTone)`. `BadgeTone::parse(&str)` parses the skin names.

Behaviour notes:

- The close target is the last `min(height, 24)` DIPs of the chip.
- `Delete`/`Backspace` remove the badge when the instance receives keyboard
  input; make it `focusable` for that path.
- `interactive()` and `repaint_on_move()` are true only for a removable badge.
- Accessibility: `A11yRole::Text`, display-only, named by the badge text.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn() -> M` passed to `.removable(handler)` | The close target is clicked, or Delete/Backspace reaches the badge while it has keyboard input. |

### Common observers

`Badge` does not install the shared observer builders. Its only callback is
the widget-specific `.removable(handler)`; a badge is interactive only when
that handler is attached (`interactive()` and `repaint_on_move()` are then
true). Bind extra pointer triggers with `Ui::bind` if needed.

### Widget-specific callbacks

| Callback | Handler | Fires when |
|---|---|---|
| `.removable(handler)` | `impl Fn() -> M + 'static` | The close target is clicked, or Delete/Backspace is pressed while the badge receives keyboard input. |

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

A removable badge reacts to `Click` (only inside the close target) and to
`HoverEnter`/`HoverLeave` (the close-target tint); Delete/Backspace are plain
key events, and `Activate` is not handled.

## XML definition

`badge` is a standard registry type and needs `WidgetRegistry::standard()`.
No `command` applies, because the standard factory has no message.

- Geometry: normal `widget` rules apply.
- Shared content: `label` only.
- Local attributes: `tone` (the five names above). An unknown tone is
  `xml.skin.value`.
- Style tokens: the widget has no `SurfaceStyle`; `radius`, `corners` and
  `background` are unsupported (`xml.style.unsupported`, a warning) and
  skipped.
- Child elements: none.

## See also

- `../index.md`
- `kbd.md`
- `../controls/button.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
