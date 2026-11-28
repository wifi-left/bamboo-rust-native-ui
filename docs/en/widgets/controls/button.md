# Button

> A push button that reports one application message when clicked or activated.

## Introduction

`Button<M>` is a simple, non-composite control. It paints a rounded fill and a
centered label, or — when a glyph is set — a glyph strip on the left followed by
a left-aligned label. The `ButtonVariant` selects a neutral, accent or
destructive treatment.

A click, or `Enter`/`Space` while focused, reports the message given to
`Button::new`. The message is cloned on every report, so `M: Clone`. Additive
observers (`on_click`, `on_focus_lost`, …) run before the built-in action, which
still runs; they never replace it.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{Button, ButtonVariant};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Save,
    Delete,
    DeleteClicked,
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(Button::new("save", "Save", Msg::Save).primary());
ui.add(
    Button::new("delete", "Delete", Msg::Delete)
        .variant(ButtonVariant::Danger)
        .glyph('\u{E74D}')
        .glyph_gap(6.0)
        .on_click(|_pos| Some(Msg::DeleteClicked)),
);
ui.set_rect(WidgetId::new("save"), Rect::from_xywh(16.0, 16.0, 120.0, 36.0));
ui.set_rect(WidgetId::new("delete"), Rect::from_xywh(16.0, 60.0, 120.0, 36.0));
```

`on_click` takes `Fn(Point) -> Option<M>` and is additive: the
`Msg::DeleteClicked` observer fires first, then the built-in `Msg::Delete` is
still reported. Use
`.disabled()` for a permanently disabled instance, or `Ui::set_enabled` to flip
it at runtime.

## Usage (XML)

`Button` is created by the standard registry (`type="button"`). An interactive
type needs a `command`, resolved through the registry's command table:

```rust
use bamboo_native_ui::creation::WidgetRegistry;
use bamboo_native_ui::Ui;

let mut ui: Ui<Msg> = Ui::new();
let mut registry = WidgetRegistry::<Msg>::standard();
registry.set_commands(|command| match command {
    101 => Some(Msg::Save),
    _ => None,
});
ui.set_widget_registry(registry);
```

```xml
<widget id="save" type="button" command="101"
        x="16" y="16" w="120" h="36" label="Save"/>
```

The file supplies `label` (content) and geometry/style. It cannot express the
variant, the glyph or the observers; those stay Rust-side, either by building
the widget in Rust or by calling `Ui::edit_widget` on the created one. `Button`
declares no widget-local attributes.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, label, message)` | `&'static str, impl Into<String>, M` | — | Creates the button; `120 × 36` DIP, secondary, enabled. |
| `.glyph(char)` | `char` | none | Paints a glyph in a strip at the left edge. |
| `.glyph_gap(f32)` | `f32` | `0.0` | Space between the glyph strip and the label (negative clamps to 0). |
| `.variant(ButtonVariant)` | `ButtonVariant` | `Secondary` | Emphasis: neutral, accent or destructive. |
| `.primary()` | — | — | Shorthand for `ButtonVariant::Primary`. |
| `.danger()` | — | — | Shorthand for `ButtonVariant::Danger`. |
| `.disabled()` | — | `false` | Starts disabled (`button_disabled` fill, `text_disabled` text). |
| `.corner_radius(f32)` | `f32` | `0.0` | Rounds fill and, when uniform, the focus ring. |
| `.corners(Corners)` | `Corners` | zero | Per-corner radii (non-uniform keeps a rectangular ring). |
| `.background(Color)` / `.background_option(Option<Color>)` | `Color` / `Option<Color>` | theme | Overrides the fill; `None` returns to the theme. |
| `.transparent_background(bool)` | `bool` | `false` | Keeps the border, drops the fill. |
| `.surface()` | `&SurfaceStyle` | — | The configured surface. |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | none | Additive click observer. |

### Behaviour

- **Variants.** `Secondary` uses `button_bg` / `button_hover` / `button_pressed`.
  `Primary` uses `button_active` / `button_active_hover` / `button_pressed`.
  `Danger` uses a fixed red that is independent of the accent color and
  lightens on hover, darkens when pressed.
- **Glyph layout.** With a glyph the strip spans the left 8–34 DIP; the label is
  painted from the strip's right edge plus `glyph_gap` to 6 DIP before the right
  edge. Without a glyph the label is centered in the rectangle inset 10 DIP
  horizontally.
- **Accessibility.** Role `button`; the accessible name is the label, or the
  widget id when the label is empty. The same message is reported for a mouse
  click and for `Activate` (Enter/Space).

## Events

### Messages

| Message | Emitted when |
|---|---|
| `M` passed to `Button::new(id, label, message)` | Clicked, or `Enter`/`Space` while focused and enabled. |

The message is cloned on every report, so `M: Clone`.

### Common observers

This widget installs the shared observer builders. All are additive: the
built-in behaviour still runs after the handler returns.

| Builder | Handler | Fires when |
|---|---|---|
| `.on_click(f)` | `Fn(Point) -> Option<M>` | Left-button click on the widget. |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | Right-button click, before the context menu. |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | Platform double click. |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | Key press while the widget has focus. |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | Key release while the widget has focus. |
| `.on_focus_gained(f)` | `Fn(FocusEvent) -> Option<M>` | Focus moves onto the widget. |
| `.on_focus_lost(f)` | `Fn(FocusEvent) -> Option<M>` | Focus leaves the widget (after an editor commits). |

### Widget-specific callbacks

None. The constructor message is the only callback; `glyph`, `variant`,
`disabled` and the surface builders are configuration.

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("save"), Trigger::Click, |event, ctx| {
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

The button reacts to `Click` and `Activate` (both report the constructor
message) and to `HoverEnter`/`HoverLeave` for its hover fill; it ignores
`Wheel`, `DragStart`/`DragEnd` and `ValueChange`.

## XML definition

- **Creation:** `type="button"` plus `command` (a `u32`). An absent `command`, or
  a registry without a command table, is `xml.command.missing`. Both variants of
  a multi-variant document must declare the same `type`/`command`/`parent`.
- **Geometry:** the normal rules — horizontally one of `x`+`w`, `right`+`w`,
  `x`+`right`; vertically one of `y`+`h`, `bottom`+`h`, `y`+`bottom`.
- **Content:** `label` only (`content_caps` is `LABEL`); `text`, `value` and
  `progress` are rejected as `xml.content.unsupported`.
- **Style:** `radius`, `corners`, `background`, `font-size`, `font-family`, the
  same tokens as every surface-owning widget; `radius` and `corners` conflict.
- **Properties:** the generic runtime set (`enabled`, `visible`, `tooltip`,
  `name`, `focus-order`, …).
- **Children:** none. An optional `parent="panel-id"` places a created button in
  a registered scroll panel.

## See also

- `./toggle.md`
- `./segmented.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
