# Label

> Static single-line or wrapping text with an optional link style and chip background.

## Introduction

`Label` paints one string with a theme text role: `LabelStyle::Body`, `Small`
or `Title`. A single-line label truncates with an ellipsis by default (or hard
clips when `.ellipsis(false)`); `.wrap(true)` lays the text out over as many
lines as the rectangle height holds, and a hard `\n` always starts a new line.
Text can be left, center or right aligned.

A plain label is decoration. `focusable` is `false` and the accessibility tree
sees a display-only `Text` node. A link label (`.link(true)`) paints in the
accent text colour with a 1 DIP underline, takes pointer input so a `Click`
binding fires, and shows the hand cursor; it is still not a Tab stop, so the
application opts into focus when keyboard activation matters.

A label may also paint a chip-style background through its `SurfaceStyle`
(`.background`, `.corner_radius`); a plain label paints no fill. The
widget never sends a message itself, so `M` is only the container's message
type.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{Label, LabelStyle};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { OpenLyrics }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        Label::new("now_title", "Now playing")
            .style(LabelStyle::Title)
            .wrap(true),
    );
    ui.set_rect(WidgetId::new("now_title"), Rect::from_xywh(16.0, 16.0, 320.0, 30.0));

    ui.add(Label::new("lyrics_link", "Open lyrics ›").link(true));
    ui.set_rect(WidgetId::new("lyrics_link"), Rect::from_xywh(16.0, 54.0, 160.0, 22.0));
    ui.on_click(WidgetId::new("lyrics_link"), |_event, _ctx| Some(Msg::OpenLyrics));
}
```

The second constructor argument is the text. Rectangles are set after
`add` because the framework has no measure pass; `preferred_height` only
reports 20 (Small), 24 (Body) or 30 (Title) DIPs for hosts that size rows. A
link label needs the `on_click` (or `on_activate`) binding shown above.

## Usage (XML)

`label` is one of the standard registry types, so a document can create it
when `WidgetRegistry::standard()` has been installed with
`Ui::set_widget_registry`:

```xml
<widget id="now_title" type="label" x="16" y="16" w="320" h="30"
        label="Now playing" text-style="title"/>
```

A label created in Rust is addressed by id instead, with no `type`:

```xml
<widget id="lyrics_link" x="16" y="54" w="160" h="22"/>
```

The file may set `color` and `text-style` locally on either form. `wrap`,
`ellipsis`, `align` and `link` are constructor-only and are not expressible in
XML.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, text)` | `&'static str, impl Into<String>` | — | Creates the label. |
| `.wrap(bool)` | `bool` | `false` | Wrap onto as many lines as the height holds. |
| `.ellipsis(bool)` | `bool` | `true` | Single-line edge: ellipsis, or hard clip when `false`. |
| `.align(TextAlign)` | `TextAlign` | `TextAlign::Left` | Horizontal alignment. |
| `.style(LabelStyle)` | `LabelStyle` | `LabelStyle::Body` | Theme text role. |
| `.link(bool)` | `bool` | `false` | Accent text plus underline, pointer input and hand cursor. |
| `.color(Color)` | `Color` | palette `text` (link: `accent_text`) | Explicit text colour. |
| `.corner_radius(f32)` | `f32` | `0.0` | Uniform corner radius of the chip fill. |
| `.corners(Corners)` | `Corners` | zero | Per-corner radii of the chip fill. |
| `.background(Color)` | `Color` | `None` | Chip fill colour; `None` paints no fill. |
| `.background_option(Option<Color>)` | `Option<Color>` | `None` | Set or clear the chip fill. |

Getters and setters: `text()`, `color_value()`, `text_style()`, `surface()`,
`set_color(Option<Color>)` and `set_text_style(LabelStyle)`. The container's
`Ui::set_label` replaces the text silently.

## Events

### Messages

None. The label never sends a message itself; `M` is only the container's
message type.

### Common observers

`Label` does not install the shared observer builders. A plain label is
decoration (`interactive()` is `false`, `focusable()` is `false`), so no input
is routed to it. A `.link(true)` label takes pointer input and a `Click`
binding fires; any label can be opted into pointer input with
`WidgetProps::mouse(true)`. Use `Ui::bind` / `Ui::on_click` /
`Ui::on_activate` rather than widget observers.

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

Only `Click` is meaningful, and only on a link label or a mouse-opt-in label;
hover only switches the hand cursor. `Activate` needs the application to make
the label focusable first.

## XML definition

`label` is a standard registry type. A `type="label"` entry creates the widget
(factory text is empty; the file's `label` content is written right after
insertion) and needs `WidgetRegistry::standard()` installed; no `command` is
read because the widget is not interactive by default.

- Geometry: normal `widget` rules apply. Choose exactly one horizontal form
  (`x`+`w`, `right`+`w`, `x`+`right`) and one vertical form (`y`+`h`,
  `bottom`+`h`, `y`+`bottom`).
- Shared content: `label` only. `text` is not accepted (`content_caps` exposes
  `LABEL`).
- Local attributes: `color` (colour grammar or palette token) and `text-style`
  (`body`, `small`, `title`).
- Style tokens: `radius`, `corners` and `background` are supported (they shape
  the chip fill); `font-size` and `font-family` apply as well.
- Child elements: none.

## See also

- `../index.md`
- `../controls/button.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
- `../../api/widget-trait.md`
