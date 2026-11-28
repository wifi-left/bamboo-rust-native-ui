# MenuBar

> A painted top-level menu bar (File / Edit / View) whose items open native menus.

## Introduction

`MenuBar<M>` paints a horizontal strip of top-level items. Each item owns a
declarative `Menu`; clicking an item, pressing Enter/Space or an `Alt+key`
access key asks the host for that item's menu as a native popup below its cell.
The native popup is the same deferred `TrackPopupMenu` path a context menu uses,
so menus stay instant, keyboard/model-correct and accessible without a second
painted menu system; it follows the system menu colors rather than the page
palette.

A focused closed bar keeps Left/Right for its top-level cursor only while there
is another item in that direction; at the ends, and for Up/Down, the key moves
focus instead of being swallowed. Selected commands arrive through the
constructor's handler as application ids; the host also replays them through
`Host::menu_command` with this widget's id.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::MenuBar;
use bamboo_native_ui::{Menu, Rect, Ui, WidgetId};

enum Msg {
    Command(u32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    MenuBar::new("menubar", Msg::Command)
        .item(
            "File(&F)",
            Menu::new()
                .item(500, "New Playlist")
                .separator()
                .item(501, "Open Library...")
                .disabled_item(502, "Export"),
        )
        .item(
            "View(&V)",
            Menu::new()
                .item(540, "Show Lyrics")
                .item(541, "Full Screen"),
        ),
);
ui.set_rect(WidgetId::new("menubar"), Rect::from_xywh(0.0, 0.0, 420.0, 26.0));
```

The label may carry an access key in either order (`"文件(&F)"` or `"&File"`);
the marker is removed from the painted text and underlined while the
application's Alt mode is on. `.item` appends; build the whole bar in one chain
or replace it later with `set_items`.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. It is construction-only chrome: it
declares no widget-local attributes and no child elements.

```xml
<widget id="menubar" x="0" y="0" w="420" h="26"/>
```

Menus, labels and commands stay in Rust because a document cannot construct
widgets or message values.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, on_command)` | `&'static str, impl Fn(u32) -> M` | — | Creates an empty bar; `420 × 26` DIP. |
| `.item(label, menu)` | `impl AsRef<str>, Menu` | — | Appends one top-level item. |
| `.disabled(bool)` | `bool` | `false` | Starts disabled (ignores input, clears pending menus). |
| `.len()` / `.is_empty()` | `usize` / `bool` | — | Number of top-level items. |
| `.cursor()` | `usize` | `0` | Keyboard/selection cursor over the top-level items. |
| `.set_items(Vec<(String, Menu)>)` | pairs | — | Replaces every item and clamps the cursor. |
| `.set_item_menu(index, menu)` | `usize, Menu` | — | Replaces one cell's menu; returns whether it existed. |
| `.insert_item(index, label, menu)` | `usize, &str, Menu` | — | Inserts a cell (clamped to the end); returns `&mut Self`. |
| `.remove_item(index)` | `usize` | — | Removes a cell; returns its `(label, menu)` or `None`. |
| `.set_cursor(index)` | `usize` | — | Moves the cursor when `index` is in range. |

### Behaviour

- **Layout.** The bar paints `palette.panel` with a 1-DIP bottom divider and
  measures each cell from its body text plus 12 DIP of padding per side, with a
  42-DIP minimum. Cell geometry is cached and invalidated by a new width, item
  set or font size.
- **Pointer.** Hover and the focused cursor cell paint `nav_hover`; a click
  opens the clicked cell's menu and moves the cursor there.
- **Keyboard.** Left/Right wrap the cursor inside the bar when it owns the key;
  Home/End jump; Down/Up and Enter/Space open the cursor cell. `mnemonic_key`
  opens the matching label's cell.
- **Accessibility.** Role `MenuBar`; each top-level item is a child, and the
  cursor item's menu entries are published after them (offset by 1000) so a
  screen reader can reach commands without opening the native popup. Invoking a
  published entry runs `on_command`.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `impl Fn(u32) -> M` passed to `new` as `on_command` | A command is chosen in the item's native popup, an accessibility entry (published after the top-level items) is invoked, or the host replays the selection through `Host::menu_command`. |

The `u32` is the application command id from the `Menu`.

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
| `new(id, on_command)` | `&'static str, impl Fn(u32) -> M` | A menu command is selected or invoked. |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("menubar"), Trigger::Click, |event, ctx| {
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

The bar reacts to `Click`, `HoverEnter`/`HoverLeave` and `Activate`; wheel,
drag and right click have no built-in effect.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties (`visible`,
`tooltip`, `name`, `description`, `automation-id`, `role`, `expose`, enabled,
focus policy) apply. The bar paints from the palette and has no `SurfaceStyle`,
so `radius`, `corners` and `background` are reported as `xml.style.unsupported`.
It declares no shared content and no child list.

Widget-local attributes: none. Child elements: none. Because the item set is
construction-only, `<cell>` or `<tab>` children are rejected as
`xml.skin.unknown`.

## See also

- `./menu-button.md`
- `./split-button.md`
- `./toolbar.md`
- `./ribbon.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
