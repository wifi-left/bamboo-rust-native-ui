# Toast

> A transient status line with an optional action, shown and timed by the application.

## Introduction

`Toast` paints the standard transient status message: one small text line on a
card-coloured surface with a 3 DIP leading strip, accented normally and in the
explicit-border colour for an error. An optional right-aligned action label
(`Undo`, `View`) is painted inside the same line.

`Snackbar` is a type alias for `Toast` (player vocabulary); the API is
identical. The widget starts hidden. The application decides when a message
appears (`show`, or the container's `Ui::set_message`), how long it stays up
(the `duration` it configured) and when it hides again; the widget starts no
timer. `Ui::set_message` also clears a container visibility override, so a
toast hidden by `set_visible` can be shown again by a message.

The toast is display-only and not focusable. Its accessibility node is a
display-only `Text`; the application should also announce the text with
`WindowHandle::a11y_announce` so a screen reader hears it without a focus move.

## Usage (Rust)

```rust
use std::time::Duration;

use bamboo_native_ui::widgets::Toast;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Undo }

let mut ui: Ui<Msg> = Ui::new();
ui.add(Toast::new("toast").duration_secs(3.0).action("Undo"));
ui.set_rect(WidgetId::new("toast"), Rect::from_xywh(16.0, 560.0, 320.0, 44.0));
```

Show it from an application event. `.show()` sets the text and the error flag
and makes the toast visible; the closure must report the change. The application
then reads the lifetime once and owns the timer. `Ui::set_message(id, text,
error)` is the container shortcut and also clears a visibility override.

```rust
ui.edit_widget::<Toast>(WidgetId::new("toast"), |toast| {
    toast.show("Track removed", false);
    true
});
let lifetime = ui
    .with_widget::<Toast, _>(WidgetId::new("toast"), |toast| toast.duration_value())
    .flatten()
    .unwrap_or(Duration::from_millis(2200));

// When the application timer expires:
ui.edit_widget::<Toast>(WidgetId::new("toast"), |toast| {
    toast.hide();
    true
});
```

For an action toast, bind a click trigger to the toast and use `action_at(pos)`
to tell an action click from a body click.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document:

```xml
<widget id="toast" x="16" y="560" w="320" h="44"
        message="Track removed" error="false" radius="medium"/>
```

`message` and `error` are widget-local attributes but neither changes
visibility: the toast stays hidden until the application shows it. The
duration and the action label are constructor-only.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates a hidden `320 × 44` toast with no text. |
| `.duration(Duration)` / `.duration_secs(f32)` | `Duration` / `f32` | `None` | Lifetime the application should use for auto-hide; seconds are clamped at zero. |
| `.duration_value()` | `Option<Duration>` | — | Reads the configured lifetime. |
| `.action(label)` | `impl Into<String>` | `""` | Right-aligned action label. |
| `.action_text()` | `&str` | — | Reads the action label. |
| `.action_rect()` | `Option<Rect>` | — | Action rectangle from the last paint. |
| `.action_at(pos)` | `bool` | `false` | Whether `pos` hits the painted action label. |
| `.show(text, error)` | `impl Into<String>, bool` | — | Sets the text and error flag, then makes the toast visible. |
| `.hide()` | — | — | Makes it invisible; the text is kept. |
| `.is_visible()` | `bool` | `false` | Current visibility. |
| `.text()` / `.is_error()` | `&str` / `bool` | `""` / `false` | Reads the current message. |
| `.set_text(text)` / `.set_error(bool)` | text / `bool` | — | Silent message/error replacement (skin `message`/`error`). |

Surface builders: `corner_radius(f32)`, `corners(Corners)`,
`background(Color)`, `background_option(Option<Color>)`,
`transparent_background(bool)` and `surface()`. The default fill is the `card`
token; the strip and action text use the accent, or `explicit_border` on error.

### Behaviour

- The preferred height is 44 DIPs; the constructor rectangle is `320 × 44`.
- `duration` is advisory data for the application, never a widget timer.
- The action label is measured with the small style, at least 28 DIPs wide;
  `action_rect` is `None` until a paint with an action has happened.
- `set_text`/`set_error` do not show a hidden toast.
- `toast_size(painter, canvas, text)` sizes a run (`160..=520` wide, 44 high).
- Accessibility: `Text`, display-only; the description marks an error message.

## Events

The toast has no callbacks: `input` always returns `None` and the widget starts
no timer. The application shows it, reads `duration_value()` and hides it when
its own timer expires; toasts are not focusable, so keyboard triggers never fire.

### Messages

None. `.action(label)` declares the label only; it does not attach a handler.

### Common observers

This widget installs none of the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`). Instead, opt the instance into mouse input
with `WidgetProps::mouse(true)`, bind a `Click` trigger to the toast id, and use
`action_at(event.pos)` to distinguish an action click from a body click.

### Widget-specific callbacks

| Seam | Signature | Role |
|---|---|---|
| `.action(label)` | `impl Into<String>` | Paints the right-aligned action label. |
| `.action_at(pos)` | `fn(&self, Point) -> bool` | Whether `pos` hits the label as last painted. |
| `.action_rect()` | `fn(&self) -> Option<Rect>` | The same rectangle; `None` before a paint with an action. |
| `.show(text, error)` / `.hide()` | `fn(&mut self, ...)` | Application-driven show/hide; pair with `Ui::set_message` and the app timer. |

### Ui triggers

Any id can be bound with `Ui::bind(WidgetId::new("toast"), Trigger::Click, |event, ctx| ...)`;
`Some(Msg::Undo)` is queued and the built-in behaviour still runs afterwards
(`ctx.run_default()` runs it inline, calling neither suppresses it). The toast
has no built-in behaviour, so the click binding plus `action_at` is the whole
interaction.

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

Only pointer triggers can be observed, and only once the instance opts into mouse
input; `Activate`, the key and focus triggers never fire because the toast is not
focusable, and it carries no value, so neither does `ValueChange`.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- **Geometry:** normal `widget` rules apply; the usual placement is the bottom
  edge, centred.
- **Content:** none; `label` and `text` are `xml.content.unsupported` warnings.
- **Local attributes:** `message` (text) and `error` (bool); both are silent and
  leave visibility to the application.
- **Style tokens:** `radius`, `corners`, `background`, `font-size`,
  `font-family`, because the toast adopts `SurfaceStyle`.
- **Children:** none. **Not expressible:** the duration and action label.

## See also

- `banner.md`
- `scrim.md`
- `../controls/progress-bar.md`
- `../index.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
