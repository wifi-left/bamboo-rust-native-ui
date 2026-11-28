# Checkbox

> A labelled on/off box that reports the new checked state.

## Introduction

`Checkbox<M>` paints an 18 DIP box and a label to its right. Clicking the
widget or activating it toggles the box and emits `on_change(new_state)`. The
checked mark is a real glyph: the default is the MDL2 check `U+E73E`, drawn
with the glyph face, or another character with `.check(char)` (a private-use
character uses the glyph face, anything else the UI face).

The box is rounded by default (`corner_radius` 4 DIPs) and `.square()` makes it
sharp. The radius is clamped to half the box at paint time. A disabled
checkbox uses the disabled palette colours and ignores input.

Accessibility: `A11yRole::Toggle`, named by the label, with `checked` set and
`read_only` when disabled. Instances are focusable by default. `M` is the
message type returned by `on_change`.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::Checkbox;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Shuffle(bool) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(Checkbox::new("shuffle", "Shuffle", false, Msg::Shuffle));
    ui.set_rect(WidgetId::new("shuffle"), Rect::from_xywh(16.0, 16.0, 180.0, 28.0));

    ui.add(Checkbox::new("square", "Square box", false, Msg::Shuffle).square());
    ui.set_rect(WidgetId::new("square"), Rect::from_xywh(16.0, 52.0, 180.0, 28.0));
}
```

The constructor takes the initial state. The application owns the value
afterwards: keep it in the model and rebuild, or read `checked()` through
`Ui::with_widget` after handling the message. `Ui::set_toggle` updates the
state silently.

## Usage (XML)

`Checkbox` is not in the standard registry. Create it in Rust and address it by
`id` from the document:

```xml
<widget id="shuffle" x="16" y="16" w="180" h="28"
        label="Shuffle" checked="true"/>
```

`label` and `checked` are the supported shared content (`content_caps` exposes
`LABEL` and `TOGGLED`). There are no local attributes or child elements.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, label, checked, on_change)` | `&'static str, impl Into<String>, bool, impl Fn(bool) -> M + 'static` | — | Creates the checkbox. |
| `.check(char)` | `char` | `'\u{E73E}'` | Checked-state mark glyph. |
| `.square()` | — | rounded | Sharp box corners (radius 0). |
| `.corner_radius(f32)` | `f32` | `4.0` (theme `radius::SMALL`) | Outer box radius, clamped to half the box. |
| `.disabled(bool)` | `bool` | `false` | Grey the control and ignore input. |

Getters and setters: `checked()`, `check_glyph()`, `corner_radius_value()` and
`set_corner_radius(f32)`. Observer hooks (`on_click`, `on_right_click`,
`on_double_click`, `on_key_down`, `on_key_up`, `on_focus_gained`,
`on_focus_lost`) run before the built-in behavior and may return a message.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(bool) -> M` passed to `new` | The box is toggled; the argument is the new checked state. |

### Common observers

This widget installs the shared observer builders. They are additive: the
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

None. The constructor's `on_change` message callback is the only widget
callback; the observers above are the extension point.

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

The checkbox reacts to `Click` and `Activate` (both toggle); `ValueChange`
follows a checked-state change. Hover, wheel and drag triggers are not
handled.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- Geometry: normal `widget` rules apply.
- Shared content: `label` and `checked`.
- Local attributes: none. `skin_attrs` is empty, so custom names are rejected.
- Style tokens: no `SurfaceStyle`; `radius`, `corners` and `background` are
  unsupported (`xml.style.unsupported`, a warning).
- Child elements: none. The check glyph and the disabled flag are
  constructor-only.

## See also

- `../index.md`
- `radio-button.md`
- `radio-group.md`
- `../controls/toggle.md`
- `../../guides/xml-layout-authoring.md`
