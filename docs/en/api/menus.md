# Menus

> Declarative popup menus, the native menu bar model, and the hook that observes every menu lifecycle.

## Overview

A `Menu` describes items, separators and submenus. On Windows the host flattens
it into a native `HMENU` and shows it with `TrackPopupMenu`, so the selection
comes back as the item's command id through `Host::menu_command` — the
application never sees `WM_COMMAND`. Commands are application-defined positive
ids; `0` is reserved for a dismissed popup.

`MenuBarModel` models the native window menu bar the system draws under the title
bar. `MenuEvent` is the common lifecycle hook for every popup the framework opens
(context menus, chrome menus, the tray menu, programmatic opens and the menu bar).

The `Menu`, `MenuItem`, `MenuEntry`, `Submenu`, `MenuPath` and `MnemonicLabel`
models are cross-platform plain data; the native popup, the system menu bar and
`WindowHandle` methods are Windows-only.

## Key types

### `Menu`

```rust
pub struct Menu { /* private */ }

impl Menu {
    pub fn new() -> Self;
}
```

Builder methods (consume `self`):

| Method | Signature | Description |
|---|---|---|
| `item` | `item(self, command: u32, label: impl Into<String>) -> Self` | A plain item. |
| `shortcut_item` | `shortcut_item(self, command, label, shortcut) -> Self` | Item with accelerator text in the native right column. |
| `checked_item` | `checked_item(self, command, label, checked: bool) -> Self` | Item that mirrors a toggle. |
| `disabled_item` | `disabled_item(self, command, label) -> Self` | Visible but not selectable. |
| `separator` | `separator(self) -> Self` | A horizontal separator. |
| `submenu` | `submenu(self, label, menu: Menu) -> Self` | A nested menu. |
| `push` | `push(self, entry: impl Into<MenuEntry>) -> Self` | Append a `MenuItem`, `Submenu` or `MenuEntry`. |

Runtime edits (take `&mut self`): `insert(index, entry)`, `remove(index)`,
`remove_command(command)`, `item_mut(command)`, `set_label`, `set_checked`,
`set_enabled`, `set_command(old, new)`, `submenu_at(&MenuPath)`,
`submenu_at_mut`, `entry_mut(&MenuPath)`. Readers: `entries()`, `len()`,
`is_empty()`, `command(u32) -> Option<&MenuItem>`, `visit_items(&mut impl
FnMut(&MenuItem))`.

### `MenuItem` and `MenuIcon`

```rust
pub struct MenuItem { /* private */ }
impl MenuItem { pub fn new(command: u32, label: impl Into<String>) -> Self; }
```

`new` asserts `command != 0`. Builders: `checked(bool)`, `disabled(bool)`,
`shortcut(text)`, `icon(MenuIcon)`. Setters: `set_label`, `set_shortcut`,
`set_icon`, `set_checked`, `set_disabled`, `set_command` (ignores `0`). Getters
include `command()`, `label()`, `shortcut_text() -> Option<&str>`,
`hotkey() -> Option<Hotkey>` (parses the shortcut text), `icon_image()`,
`is_checked()`, `is_disabled()`.

`MenuIcon::from_rgba(width: u16, height: u16, rgba: impl Into<Vec<u8>>)` builds a
16×16-conventional bitmap from top-down RGBA; getters are `width`, `height`,
`rgba`.

### `MenuEntry` and `Submenu`

`MenuEntry` is `Item(MenuItem) | Separator | Submenu(Submenu)`, with helpers
`MenuEntry::item(command, label)`, `separator()`, `submenu(label, menu)`,
`disabled(bool)`, `checked(bool)`, `set_label`, `command()`. `Submenu::new(label,
menu)` plus `disabled(bool)`, `label()`, `menu()`, `is_disabled()`.

### `MenuPath` / `MenuStep`

`MenuPath::root()` (or `new()`), `.index(usize)`, `.label(&str)`, `.then(step)`,
`.steps()`, `.is_root()`. A `MenuStep` is `Index(usize)` or `Label(String)`.

### `MenuBarModel`

```rust
pub struct MenuBarModel { /* private */ }

impl MenuBarModel {
    pub fn new() -> Self;
    pub fn item(self, label: impl Into<String>, menu: Menu) -> Self;
}
```

Also `push(entry)`, `separator()`, `entries()`, `len()`, `is_empty()`,
`insert(index, label, menu)`, `remove(index)`, `set_menu(index, menu)`,
`item_mut(command)`, `set_checked`, `set_enabled`, `set_label`,
`mnemonic_index(ch) -> Option<usize>`, `mnemonic_indices(ch) -> Vec<usize>`, and
`hotkeys() -> Vec<(Hotkey, u32)>` (enabled item shortcuts that parse as chords).

### `MenuEvent` / `MenuDecision` / `MenuSource`

`MenuEvent<'a>` is `Opening { widget, source, anchor, menu }`,
`Opened { widget, source, menu }`, `Selected { widget, source, command:
&mut u32 }`, `Closed { widget, source, command: Option<u32> }`.
`MenuDecision` is `Pass` (default) or `Handled`. `MenuSource` is `ContextMenu`,
`Chrome`, `Programmatic`, `Tray`, `MenuBar`.

### Wiring

| API | Signature |
|---|---|
| `Ui::set_menu` | `set_menu(&mut self, id: WidgetId, menu: Option<Menu>)` |
| `Ui::open_menu` | `open_menu(&mut self, id, anchor: Point, sink) -> bool` |
| `Ui::open_menu_for` | `open_menu_for(&mut self, id, sink) -> bool` |
| `WindowHandle::set_menu_bar` | `set_menu_bar(&mut self, menu_bar: Option<MenuBarModel>)` |
| `WindowHandle::open_menu` | `open_menu(&mut self, widget, menu: Menu, anchor: Point, avoid: Option<Rect>)` |
| `Host::menu_command` | `fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>)` |
| `Host::menu_event` | `fn menu_event(&mut self, event: &mut MenuEvent<'_>, window) -> MenuDecision` |
| `Host::menu_bar_command` | `fn menu_bar_command(&mut self, command: u32, window)` |

## Example

```rust
use bamboo_native_ui::{Menu, Ui, WidgetId};

fn install_player_menu(ui: &mut Ui<Msg>) {
    let menu = Menu::new()
        .shortcut_item(101, "Play", "Ctrl+P")
        .item(102, "Pause")
        .separator()
        .checked_item(103, "Shuffle", true)
        .submenu("More", Menu::new().item(104, "Properties"))
        .disabled_item(105, "Equalizer");
    ui.set_menu(WidgetId::new("player"), Some(menu));
}
```

The selection arrives as an id:

```rust
use bamboo_native_ui::{WidgetId, WindowHandle};

fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>) {
    if widget != WidgetId::new("player") {
        return;
    }
    match command {
        101 => self.play(window),
        102 => self.pause(window),
        103 => self.toggle_shuffle(window),
        _ => {}
    }
}
```

To mirror live state before the popup appears, edit the model in the hook:

```rust
use bamboo_native_ui::{MenuDecision, MenuEvent, WindowHandle};

fn menu_event(
    &mut self,
    event: &mut MenuEvent<'_>,
    _window: &mut WindowHandle<'_>,
) -> MenuDecision {
    if let MenuEvent::Opening { menu, .. } = event {
        menu.set_checked(103, self.shuffle);
    }
    MenuDecision::Pass
}
```

## Notes and limits

- Command ids are nonzero; `MenuItem::new` panics on `0`, which a dismissed popup
  reserves. `MenuDecision::Handled` on `Opening` cancels the popup; on `Selected`
  it suppresses the default replay.
- An open popup keeps the model it was built from; `Ui::set_menu` edits apply to
  the next open. `Host::menu_event` during `Opened` and
  `WindowHandle::edit_open_menu` close and rebuild the live popup.
- The native popup and the system menu bar are Windows-only and follow system
  colors/DPI. The menu bar's `&` access keys are handled by the shell;
  `MenuBarModel::mnemonic_index` resolves `Alt+key` for owner-drawn titles.
- Painted labels (buttons, menu-bar widgets) use `MnemonicLabel::parse("&File")`
  which strips the marker and reports the key; `Widget::mnemonic` exposes it and
  `Ui` routes `Alt+key`.
- Menu-bar item shortcuts become window accelerators automatically while the
  model is installed (`MenuBarModel::hotkeys`); `WindowHandle::register_window_hotkey`
  can override one.
- Edge and doubled separators are dropped when the native popup is built. An
  empty menu is never shown.

## See also

- `hotkeys.md` (accelerator parsing and window shortcuts)
- `tray-and-notifications.md` (the tray menu)
- `accessibility.md` (Menu/MenuBar roles)
- `../widgets/controls/button.md`
