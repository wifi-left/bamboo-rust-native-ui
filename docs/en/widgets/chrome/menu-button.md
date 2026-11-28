# MenuButton

> A button that opens a declarative menu as a native popup.

## Introduction

`MenuButton<M>` paints a button with a label, an optional leading glyph and a
trailing chevron. Clicking it, pressing Enter/Space or an `Alt+key` access key
asks the host for its `Menu` as a native popup anchored below the button. The
popup uses the same `TrackPopupMenu` path as a context menu, so it follows the
system menu colors; the button itself is page content and uses the palette.

The menu is declarative and can carry separators, checked items, disabled items,
shortcuts, icons and submenus. A selected command is delivered to the
constructor's `on_command` handler as a `u32` application id; the host also
replays it through `Host::menu_command` with this widget's id. The widget does
not interpret commands.

A closed menu has no on-screen rows, so its accessible children report the
button's rectangle; invoking one runs the command directly.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::MenuButton;
use bamboo_native_ui::{Menu, MenuItem, Rect, Ui, WidgetId};

enum Msg {
    Command(u32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    MenuButton::new(
        "actions",
        "Actions(&A)",
        Menu::new()
            .item(700, "Play")
            .checked_item(701, "Shuffle", true)
            .separator()
            .submenu("Add to...", Menu::new().item(710, "Queue").item(711, "Favorites"))
            .push(MenuItem::new(720, "Track Info").shortcut("F2")),
        Msg::Command,
    )
    .glyph('≡'),
);
ui.set_rect(WidgetId::new("actions"), Rect::from_xywh(16.0, 16.0, 140.0, 30.0));
```

`MenuButton::new` takes the label, the menu and the command handler. The label
may carry an access key (`"文件(&F)"` / `"&File"`); the marker is stripped from
the painted text and underlined while Alt mode is on, and `Alt+key` opens the
menu.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. It is construction-only chrome: it
declares no widget-local attributes and no child elements.

```xml
<widget id="actions" x="16" y="16" w="140" h="30"/>
```

The menu and its commands stay in Rust; a document cannot construct widget
instances or message values.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, label, menu, on_command)` | `&'static str, impl AsRef<str>, Menu, impl Fn(u32) -> M` | — | Creates the button; `140 × 30` DIP, enabled. |
| `.glyph(char)` | `char` | none | Leading MDL2 codepoint or text character. |
| `.disabled(bool)` | `bool` | `false` | Starts disabled; a disabled button opens nothing. |
| `.menu()` | `&Menu` | — | The current menu model. |
| `.set_menu(Menu)` | `Menu` | — | Replaces the menu and drops a pending request. |
| `.set_label(&str)` | `&str` | — | Replaces the label (re-parsed for its access key). |

`preferred_height` is 30 DIP. The button paints `button_bg` with
`button_hover` / `button_pressed` states and a muted chevron at the right edge;
the glyph strip is 20 DIP wide starting 10 DIP from the left.

### Behaviour

- **Pointer.** A click opens the menu with its anchor at the button's
  bottom-left corner; the widget's rectangle is passed as the avoid rectangle.
  Hover and press states follow the normal button palette.
- **Keyboard.** Enter/Space (`Activate`) opens the menu; an `Alt+key` matching
  the label's access key opens it too. A disabled button refuses both.
- **Accessibility.** Role `Menu` named by the label; its children are the menu
  entries with menu-item roles, checked/disabled state and the button's
  rectangle. `a11y_expand(true)` opens the menu, and
  `a11y_select_child(index)` runs an enabled leaf item through `on_command`.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `impl Fn(u32) -> M` passed to `new` as `on_command` | A command is chosen in the native popup, an accessibility child (a leaf menu item) is invoked, or the host replays the selection through `Host::menu_command`. |

The `u32` is the application command id from the `Menu`; opening the menu emits nothing.

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
| `new(id, label, menu, on_command)` | `&'static str, impl AsRef<str>, Menu, impl Fn(u32) -> M` | A menu command is selected or invoked. |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("actions"), Trigger::Click, |event, ctx| {
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
other triggers reach the binding but open nothing.

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

- `./menu-bar.md`
- `./split-button.md`
- `../controls/button.md`
- `../overlay/popover.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
