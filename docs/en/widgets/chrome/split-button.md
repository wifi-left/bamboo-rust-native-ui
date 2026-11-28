# SplitButton

> A button whose body runs a primary action and whose arrow opens an alternatives menu.

## Introduction

`SplitButton<M>` paints one control with two zones: the body (left of the last
26 DIP) runs a primary action directly, and the chevron zone at the right edge
opens a native menu of alternatives. A 1-DIP divider separates the zones, and a
hover over the arrow tints only that zone.

Clicking the body, or pressing Enter/Space/`Activate`, reports `on_primary`;
clicking the arrow, an `Alt+key` access key or an accessibility expand request
opens the menu. Menu commands use the same native popup and command ids as a
`MenuButton`, and `on_command` maps them to application messages.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::SplitButton;
use bamboo_native_ui::{Menu, Rect, Ui, WidgetId};

enum Msg {
    ExportNow,
    Command(u32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    SplitButton::new(
        "export",
        "Export(&E)",
        Menu::new()
            .item(800, "Export as MP3")
            .item(801, "Export as FLAC")
            .separator()
            .item(802, "Export Settings..."),
        || Msg::ExportNow,
        Msg::Command,
    )
    .glyph('⇩'),
);
ui.set_rect(WidgetId::new("export"), Rect::from_xywh(16.0, 16.0, 170.0, 30.0));
```

The body handler takes no argument; the menu handler takes the `u32` command.
The label may carry an access key (`"Export(&E)"`), which opens the menu rather
than running the primary action.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. It is construction-only chrome: it
declares no widget-local attributes and no child elements.

```xml
<widget id="export" x="16" y="16" w="170" h="30"/>
```

The menu, primary action and command table stay in Rust.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, label, menu, on_primary, on_command)` | `&'static str, impl AsRef<str>, Menu, impl Fn() -> M, impl Fn(u32) -> M` | — | Creates the button; `170 × 30` DIP, enabled. |
| `.glyph(char)` | `char` | none | Leading MDL2 codepoint or text character. |
| `.disabled(bool)` | `bool` | `false` | Starts disabled; neither zone acts. |
| `.set_menu(Menu)` | `Menu` | — | Replaces the alternatives menu and drops a pending request. |

`preferred_height` is 30 DIP. The right 26 DIP are the arrow zone; the divider
sits at its left edge. The body uses `button_bg`, `button_hover` and
`button_pressed`; the chevron is muted.

### Behaviour

- **Pointer.** A hover tracks which zone the pointer is in and repaints only
  when it changes. A click in the body emits the primary message; a click in
  the arrow zone opens the menu anchored at the arrow's bottom edge, with the
  whole button as the avoid rectangle.
- **Keyboard.** `Activate` and Enter/Space run the primary action. `Alt+key`
  matching the label's access key opens the alternatives menu.
- **Accessibility.** Role `Menu` named by the label, described as the primary
  action plus menu, with the menu entries as children. Expanding opens the
  menu; invoking an enabled leaf runs `on_command`.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `impl Fn() -> M` passed to `new` as `on_primary` | The body zone is clicked, or `Enter`/`Space`/`Activate` runs the primary action. |
| `impl Fn(u32) -> M` passed to `new` as `on_command` | A command is chosen in the alternatives menu, or a UIA menu child is invoked. |

Opening the menu and hovering emit nothing.

### Common observers

This widget installs the shared observer builders. All are additive: the built-in behaviour still runs after the handler returns.

| Builder | Handler | Fires when |
|---|---|---|
| `.on_click(f)` | `Fn(Point) -> Option<M>` | Left-button click on the widget. |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | Right-button click, before the context menu. |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | Platform double click. |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | Key press while the widget has focus. |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | Key release while the widget has focus. |
| `.on_focus_gained(f)` | `Fn(FocusEvent) -> Option<M>` | Focus moves onto the widget. |
| `.on_focus_lost(f)` | `Fn(FocusEvent) -> Option<M>` | Focus leaves the widget. |

### Widget-specific callbacks

| Callback | Signature | Fires when |
|---|---|---|
| `new(id, label, menu, on_primary, on_command)` | `&'static str, impl AsRef<str>, Menu, impl Fn() -> M, impl Fn(u32) -> M` | The body runs the primary action, or a menu command is selected. |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("export"), Trigger::Click, |event, ctx| {
    // Return Some(Msg::...) to queue a message; call ctx.default() to keep the
    // built-in action, which then runs after the handler.
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

The button reacts to `Click`, `HoverEnter`/`HoverLeave` and `Activate`; the
other triggers reach the binding but change nothing.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties (`visible`,
`tooltip`, `name`, `description`, `automation-id`, `role`, `expose`, enabled,
focus policy) apply. The button paints from the palette and has no
`SurfaceStyle`, so `radius`, `corners` and `background` are reported as
`xml.style.unsupported`. It declares no shared content and no child list.

Widget-local attributes: none. Child elements: none.

## See also

- `./menu-button.md`
- `./menu-bar.md`
- `./toolbar.md`
- `../controls/button.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
