# Avatar

> A round initials avatar, or a square-cropped image clipped to a circle.

## Introduction

`Avatar` paints a filled circle with up to two initials, or an image when one
is registered with `Ui::set_image`. Initials are derived from the name: two
words give their first letters, one ASCII word gives its first two characters,
and one non-ASCII word gives its first character. `.initials` overrides the
derivation.

Without an explicit `.tint`, the circle colour is picked deterministically from
the name among six fixed tints, so the same artist keeps one colour across
pages. `.ring(true)` draws a 1 DIP ring for overlapping avatars. A registered
image is center-cropped to a square and anti-aliased to the circle; an
unregistered id falls back to the initials. Hit-testing follows the circle, not
the rectangle corners.

The widget is decoration (`focusable` is `false`) and publishes a display-only
`Text` node named by the full name (or the initials when the name is empty).
It sends no message, so `M` is only the container's message type.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::Avatar;
use bamboo_native_ui::{Color, ImageId, Rect, Ui, WidgetId};

enum Msg {}

fn build(ui: &mut Ui<Msg>) {
    ui.add(Avatar::new("artist", "Annie Lennox"));
    ui.set_rect(WidgetId::new("artist"), Rect::from_xywh(16.0, 16.0, 40.0, 40.0));

    ui.add(
        Avatar::new("me", "竹韵")
            .initials("ZY")
            .tint(Color::new(96, 134, 208))
            .ring(true),
    );
    ui.set_rect(WidgetId::new("me"), Rect::from_xywh(64.0, 16.0, 40.0, 40.0));

    ui.add(Avatar::new("cover", "Nocturne").image(ImageId(1)));
    ui.set_rect(WidgetId::new("cover"), Rect::from_xywh(112.0, 16.0, 40.0, 40.0));
}
```

The rectangle should be square; the circle uses the smaller of the width and
height. `ImageId(1)` must refer to an image registered with `Ui::set_image`
(for example `UiImage::gradient`); an unknown id silently falls back to
the initials.

## Usage (XML)

`avatar` is a standard registry type. Create it with `type="avatar"` when
`WidgetRegistry::standard()` is installed:

```xml
<widget id="artist" type="avatar" x="16" y="16" w="40" h="40"
        label="Annie Lennox"/>
```

Local attributes override the derived look:

```xml
<widget id="cover" type="avatar" x="64" y="16" w="40" h="40"
        initials="ZY" tint="#6090d0" ring="true" image="1"/>
```

`image` is the numeric `ImageId` of an image the application registered; there
is no image-name registry in a page. A Rust-created avatar is addressed by id:

```xml
<widget id="me" x="112" y="16" w="40" h="40"/>
```

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, name)` | `&'static str, impl Into<String>` | — | Creates the avatar and derives initials. |
| `.initials(impl Into<String>)` | `impl Into<String>` | derived from `name` | Explicit circle text. |
| `.image(ImageId)` | `ImageId` | `None` | Show a registered image instead of initials. |
| `.tint(Color)` | `Color` | `None` (name-based fixed tint) | Circle fill colour. |
| `.ring(bool)` | `bool` | `false` | Draw a 1 DIP ring around the circle. |

Getters and setters: `name()`, `initials_text()`, `set_initials`,
`tint_color()`, `set_tint(Option<Color>)`, `has_ring()`, `set_ring(bool)`,
`image_id()` and `set_image_id(Option<ImageId>)`.

The text colour on the initials is chosen from the circle's luminance (near
white on dark fills, near black on light fills).

## Events

### Messages

None. `Avatar` paints initials or an image and never returns a message.

### Common observers

`Avatar` does not install the shared observer builders. It is not focusable
and `interactive()` is `false`; its circular `hit_test` only matters once the
instance is opted into pointer input with `WidgetProps::mouse(true)`, after
which `Ui::bind` / `Ui::on_click` can receive clicks.

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

By default the avatar reacts to none of them; with `WidgetProps::mouse(true)`
a `Click` inside the circle reaches the binding. Other triggers are not
meaningful.

## XML definition

`avatar` is a standard registry type and needs `WidgetRegistry::standard()`.
No `command` applies.

- Geometry: normal `widget` rules apply; a square rectangle gives a circle.
- Shared content: `label` only. It replaces the name and re-derives the
  initials.
- Local attributes: `initials` (text), `tint` (colour grammar or palette
  token), `ring` (bool) and `image` (a whole, non-negative number used as the
  `ImageId`).
- Style tokens: no `SurfaceStyle`; `radius`, `corners` and `background` are
  unsupported (`xml.style.unsupported`, a warning).
- Child elements: none.

## See also

- `../index.md`
- `label.md`
- `../display/card.md`
- `../../guides/xml-layout-authoring.md`
