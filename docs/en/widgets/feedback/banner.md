# Banner

> An inline notification strip with a kind, an optional action and an optional close target.

## Introduction

`Banner<M>` paints a notification inside the page: a tinted, bordered panel with
a leading icon glyph, a title, an optional body line, and optionally a
right-aligned action label and a `×` close target. The kind fixes the accent
colour, so a warning stays yellow and an error red regardless of the theme
accent; the panel fill is that colour mixed 14 % into the panel token and the
border mixes it with the panel.

The banner is decoration until a handler is attached. With `.action(...)` or
`.closable(...)` it becomes interactive: it takes the hand cursor, tracks hover
over the two targets, and emits the handler's message on click. The painted
action rectangle is measured from its text, so hover and click agree. `M` is
the message type returned by either handler.

The accessibility node is a display-only `Group` named by the title, with the
body as its description.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{Banner, BannerKind};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Retry, Dismiss }

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Banner::new("update", BannerKind::Info, "Update available")
        .body("Version 0.2 is ready and installs on restart.")
        .action("Retry", || Msg::Retry)
        .closable(|| Msg::Dismiss),
);
ui.set_rect(WidgetId::new("update"), Rect::from_xywh(16.0, 16.0, 420.0, 64.0));
```

A banner with a body paints two text lines and prefers 64 DIPs of height; with
only a title it prefers 44. The constructor's rectangle is `420 × 60`, so give
it the height that matches its content.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document:

```xml
<widget id="update" x="16" y="16" w="420" h="64"
        label="Update available" body="Version 0.2 is ready." radius="medium"/>
```

`label` is shared content and writes the title (`set_label`). `body` is a
widget-local attribute. The kind, the action and the close handler are
constructor-only and cannot be declared in XML.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, kind, title)` | `&'static str, BannerKind, impl Into<String>` | — | Creates a `420 × 60` banner with an empty body. |
| `.body(text)` | `impl Into<String>` | `""` | Second line in the small muted style. |
| `.body_text()` | `&str` | — | Reads the body back. |
| `.set_body(text)` | `impl Into<String>` | — | Silent body replacement (the skin's `body`). |
| `.action(label, handler)` | `impl Into<String>, impl Fn() -> M + 'static` | `None` | Right-aligned action label and its message. |
| `.closable(handler)` | `impl Fn() -> M + 'static` | `None` | Paints a `×` target that emits the handler. |
| `.kind()` | `BannerKind` | constructor value | Reads the kind. |
| `.title()` | `&str` | constructor value | Reads the title. |

`BannerKind` is `Info` (default), `Success`, `Warning`, `Error`, with fixed
colours `(84,132,220)`, `(52,132,88)`, `(176,136,34)` and `(178,42,42)`.

### Behaviour

- The close target is a 20 DIP square inset 8 DIPs from the top-right corner.
- The action sits left of the close target and measures its label plus 16 DIPs;
  a click outside both targets emits nothing.
- `interactive()` (and the hand cursor) is true only when an action or a close
  handler exists; hover repaints are requested only then.
- The instance adopts `SurfaceStyle`: `radius` defaults to the medium token (6
  DIPs), and `corner_radius`, `corners`, `background`,
  `transparent_background` and `surface` are available. A `background`
  override replaces the kind-derived fill; the border still paints.
- Preferred height is 44 without a body and 64 with one.
- Accessibility: `Group`, display-only, name = title, description = body, even
  when the instance is interactive.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn() -> M` passed to `.action` | A left click lands inside the painted action label. |
| `Fn() -> M` passed to `.closable` | A left click lands inside the 20 DIP `×` target. |

A click anywhere else (icon, title, body) emits nothing.

### Common observers

The banner does not install the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`). The callbacks below are its event seam;
anything else goes through `Ui::bind`.

### Widget-specific callbacks

| Method | Handler | Fires when |
|---|---|---|
| `.action(label, handler)` | `impl Into<String>, impl Fn() -> M + 'static` | Click on the right-aligned action label; also makes the banner `interactive()` (hand cursor, hover tracking). |
| `.closable(handler)` | `impl Fn() -> M + 'static` | Click on the close target; also makes the banner `interactive()`. |

### Ui triggers

Any id can be bound with `Ui::bind(WidgetId::new("update"), Trigger::Click, |event, ctx| ...)`;
`Some(Msg::...)` is queued and the built-in action still runs afterwards
(`ctx.run_default()` runs it inline, calling neither suppresses it). Use
`event.pos` to tell an action click from a close click.

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

`Click` and `HoverEnter`/`HoverLeave` are meaningful here; the banner is not
focusable, so `Activate` and the key/focus triggers never fire, and it carries no
value, so neither does `ValueChange`.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- **Geometry:** normal `widget` rules apply.
- **Content:** `label` (`content_caps = LABEL`) writes the title; `text` is
  unsupported.
- **Local attributes:** `body` (text). The skin write is silent and does not
  fire callbacks.
- **Style tokens:** `radius`, `corners`, `background`, `font-size` and
  `font-family`, because the banner adopts `SurfaceStyle`. `background="none"`
  keeps the border and drops the fill.
- **Children:** none.
- **Not expressible:** `kind`, the action label/handler and the close handler
  are constructor arguments only.

## See also

- `toast.md`
- `../display/card.md`
- `../index.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
