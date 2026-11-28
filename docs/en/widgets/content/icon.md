# Icon

> A single centered glyph, tinted and sized to its cell.

## Introduction

`Icon` paints one character centered in its rectangle. The glyph is always
drawn with the glyph face (Segoe MDL2 Assets), so use an MDL2 code point such
as `U+E721`; a character the face does not contain shows as a missing glyph.
The default size is 80% of the cell height and the default tint is the
palette's `muted` colour.

Icons are decoration: `focusable` is `false` and the accessibility tree sees a
display-only `Text` node with an empty name, so an icon that carries meaning
should get a narration name through `WidgetProps`. The widget never sends a
message, so `M` is only the container's message type.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::Icon;
use bamboo_native_ui::{Color, Rect, Ui, WidgetId};

enum Msg {}

fn build(ui: &mut Ui<Msg>) {
    ui.add(Icon::new("search_icon", '\u{E721}').size(18.0));
    ui.set_rect(WidgetId::new("search_icon"), Rect::from_xywh(16.0, 16.0, 24.0, 24.0));

    ui.add(Icon::new("star", '\u{E734}').tint(Color::new(226, 168, 58)));
    ui.set_rect(WidgetId::new("star"), Rect::from_xywh(48.0, 16.0, 24.0, 24.0));
}
```

`.size` is in DIPs and must be at least 1; without it the glyph follows the
current cell height. `.tint` overrides the muted palette colour. The glyph is
centered in the whole rectangle.

## Usage (XML)

`icon` is a standard registry type. Create it with `type="icon"` when
`WidgetRegistry::standard()` is installed:

```xml
<widget id="search_icon" type="icon" x="16" y="16" w="24" h="24"
        glyph="&#xE721;" tint="muted"/>
```

The factory starts with a space glyph; the `glyph` attribute supplies the real
character. A Rust-created icon is addressed by id only:

```xml
<widget id="star" x="48" y="16" w="24" h="24"/>
```

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, glyph)` | `&'static str, char` | — | Creates the icon. |
| `.size(f32)` | `f32` | `None` (80% of the cell height) | Glyph size in DIPs, clamped to at least 1. |
| `.tint(Color)` | `Color` | `None` (palette `muted`) | Glyph colour. |

Getters and setters: `glyph()`, `tint_color()`, `set_glyph(char)` and
`set_tint(Option<Color>)`.

## Events

### Messages

None. `Icon` only paints a glyph and never returns a message.

### Common observers

`Icon` does not install the shared observer builders. Its `input` always
returns `None`, it is not focusable and `interactive()` is `false`, so it is
not hit-tested. To make it clickable, install `WidgetProps::mouse(true)` and
bind with `Ui::bind` / `Ui::on_click`.

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

By default the icon reacts to none of them; only a `WidgetProps::mouse(true)`
opt-in routes `Click` (and the pointer triggers) to a binding. `ValueChange`
and `Activate` are never produced.

## XML definition

`icon` is a standard registry type and needs `WidgetRegistry::standard()`
installed before the document is applied. No `command` applies.

- Geometry: normal `widget` rules apply (one horizontal and one vertical
  form).
- Shared content: none. An icon has no `label`/`text` content capability.
- Local attributes: `glyph` (exactly one character) and `tint` (colour grammar
  or palette token; alpha is dropped).
- Style tokens: the widget has no `SurfaceStyle`, so `radius`, `corners` and
  `background` are reported as unsupported (`xml.style.unsupported`, a warning)
  and skipped.
- Child elements: none.

## See also

- `../index.md`
- `../display/card.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
