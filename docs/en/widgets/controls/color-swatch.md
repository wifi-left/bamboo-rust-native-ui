# ColorSwatch

> A labeled color chip for showing a palette entry or the current color.

## Introduction

`ColorSwatch` paints a 40 DIP wide, full-height chip at its leading edge with the
theme's `explicit_border` frame and the label centered inside it. The label text
color flips to near-black on light colors (luminance above `0.55`) and near-white
on dark ones, so short labels stay readable on any swatch.

It is a display widget: not focusable, ignoring input, reporting no messages.
Make it clickable by opting into mouse input and binding a handler on the `Ui`
(`WidgetProps::mouse(true)` plus `Ui::on_click`), as the showcase does. The
widget also adopts the `LABEL` content capability so a document can set the
inside text.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::ColorSwatch;
use bamboo_native_ui::{Color, Rect, Ui, WidgetId};

enum Msg {
    InspectSwatch,
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(ColorSwatch::new("accent", Color::rgb([127, 211, 168]), "Bamboo"));
ui.set_rect(WidgetId::new("accent"), Rect::from_xywh(16.0, 16.0, 120.0, 32.0));
ui.on_click(WidgetId::new("accent"), |_event, _ctx| Some(Msg::InspectSwatch));
```

Without the `on_click` binding and `WidgetProps::mouse(true)` the swatch takes no
input. `set_color` changes the color silently and `set_label_text` changes the
text; both are also reachable from a `.buixml` document.

## Usage (XML)

`ColorSwatch` is not created by the standard registry. Create it in Rust and
address it by `id`; the document sets the label and the color:

```xml
<widget id="accent" x="16" y="16" w="120" h="32"
        label="Bamboo" color="#7FD3A8"/>
```

`color` accepts the shared color grammar — `#rgb`, `#rgba`, `#rrggbb`,
`#rrggbbaa`, decimal channels, `rgb(…)`/`rgba(…)` or a palette token. A swatch
is opaque, so an alpha component is dropped silently. `label` arrives through
`Ui::set_label`; there are no child elements.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, color, label)` | `&'static str, Color, impl Into<String>` | — | Creates the swatch; `120 × 32` DIP. |
| `.color()` | — | — | The current color. |
| `.set_color(Color)` | `Color` | — | Replaces the color silently (no repaint request by itself). |
| `.label()` | — | — | The chip text. |
| `.set_label_text(impl Into<String>)` | `String` | — | Replaces the chip text silently. |

### Behaviour

- **Layout.** The chip always occupies the left 40 DIP of the rectangle, even in
  a wider cell; the rest is empty. The label is centered in the chip with 4 DIP
  horizontal inset.
- **Contrast.** Text color is chosen from the color's luminance, not from the
  theme, so a light swatch gets dark text and vice versa.
- **Interaction.** None built in. Bind click/activate through the `Ui` when the
  swatch should act as a button, and set the role/name through `WidgetProps` for
  assistive technology.
- **Accessibility.** Role `text`, `display_only`, named from the label, with the
  description `#RRGGBB` so a color-blind user still receives the exact color. It
  joins the accessibility tree only when the instance opts in
  (`WidgetProps::expose(true)`).

## Events

### Messages

None. `ColorSwatch` reports no messages.

### Common observers

This widget does **not** install the shared observer builders. It is not
focusable, its `Widget::input` returns `None`, and it has no callback field. To
make it clickable, opt into mouse input with `WidgetProps::mouse(true)`
(`interactive="true"` in a document) and bind `Ui::on_click` or `Ui::bind` on its
id.

### Widget-specific callbacks

None. `set_color` and `set_label_text` replace state silently.

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("accent"), Trigger::Click, |event, ctx| {
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

The swatch reacts to none of them by itself; a binding only fires when the
instance has opted into mouse input.

## XML definition

- **Creation:** no `type`; this widget is addressed only.
- **Geometry:** one horizontal form and one vertical form. Heights below about
  20 DIP start clipping the chip label.
- **Content:** `label` (`content_caps = LABEL`).
- **Local attributes:** `color` only (`SkinValueKind::Color`).
- **Style:** `font-size`, `font-family`. There is no surface, so `radius`,
  `corners` and `background` are `xml.style.unsupported` and skipped.
- **Properties:** the generic runtime set (`visible`, `tooltip`, `name`,
  `interactive`, …). Set `interactive="true"` when a Rust binding should receive
  clicks.
- **Children:** none.

## See also

- `../pickers/color-picker.md`
- `./button.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
