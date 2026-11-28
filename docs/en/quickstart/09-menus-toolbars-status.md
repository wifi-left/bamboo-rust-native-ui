# 09 — Menus, toolbars and status

> Wrap a page in application chrome: a menu bar, in-page menu buttons, a
> toolbar with overflow, a status strip and context menus — all routing through
> one command handler.

## What you will build

A shell with a painted `MenuBar`, `MenuButton`, `SplitButton`, both `Toolbar`
orientations, a `StatusBar` and a context menu. `Msg::Command(u32)` carries
every command to `Host::menu_command` or `menu_bar_command`.

## Prerequisites

- The window from [03](03-first-window.md) builds and runs.
- No feature flag is needed; the chrome widgets are always compiled.

## Steps

### 1. One message, one menu model

Command ids are nonzero; one `Msg::Command` and one shared `Menu` model.

```rust
use bamboo_native_ui::{Menu, MenuItem, Submenu};

#[derive(Clone, Copy, Debug)]
enum Msg {
    Command(u32),
}

fn file_menu() -> Menu {
    Menu::new()
        .item(500, "New playlist")
        .item(501, "Open library...")
        .separator()
        .push(Submenu::new("Recent", Menu::new().item(504, "Nocturne")))
        .push(MenuItem::new(506, "Export...").shortcut("Ctrl+E"))
}
```

`item` appends a command, `checked_item`/`disabled_item` set the initial state,
`separator` draws a divider and `push(Submenu::new(label, menu))` nests a submenu.

### 2. Add the painted menu bar

[`MenuBar`](../widgets/chrome/menu-bar.md) paints top-level cells; a click,
Enter/Space or `Alt+key` opens the cell's menu as a native popup.

```rust
use bamboo_native_ui::widgets::MenuBar;
use bamboo_native_ui::{Rect, Ui, WidgetId};

fn add_menu_bar(ui: &mut Ui<Msg>) {
    ui.add(
        MenuBar::new("menubar", Msg::Command)
            .item("&File", file_menu())
            .item("&Edit", Menu::new().item(520, "Undo").item(521, "Redo")
                .checked_item(522, "Shuffle", false)),
    );
    ui.set_rect(WidgetId::new("menubar"), Rect::from_xywh(0.0, 0.0, 420.0, 26.0));
}
```

A label carries its access key as `&File` or `File(&F)`; the marker is removed
and underlined while Alt mode is on. Forward the tap:

```rust
if let Event::Alt(show) = *event {
    self.ui.set_mnemonic_underlines(show);
}
```

Only chrome widgets with a marked label react; `flush_damage` repaints them.

### 3. Route the commands

Every chrome and context popup reports through
[`Host::menu_command`](../api/host-and-window.md); the widget id names the control.

```rust
impl Host for App {
    fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>) {
        if self.ui.menu_command(widget, command, window) {
            self.ui.flush_damage(window);
            return;
        }
        self.apply(Msg::Command(command));
    }
}
```

`Ui::menu_command` handles the built-in Undo/Cut/Copy/Paste menus of painted
editors and returns `false` otherwise. `Host::menu_command` runs after the popup
closed, outside its modal loop.

Return `MenuDecision::Handled` to cancel `Opening` or suppress a `Selected` replay.

```rust
use bamboo_native_ui::{MenuDecision, MenuEvent};

impl Host for App {
    fn menu_event(&mut self, event: &mut MenuEvent<'_>, _window: &mut WindowHandle<'_>) -> MenuDecision {
        if let MenuEvent::Opening { menu, .. } = event {
            menu.set_enabled(520, self.can_undo);
        }
        MenuDecision::Pass
    }
}
```

`Opening` mutates the model, `Opened` edits the live popup, `Selected` rewrites
the command and `Closed` reports the final command or `None`.

### 4. The native window menu bar

A painted `MenuBar` is a widget you position. A `MenuBarModel` is installed in
the window frame: the system draws it, handles `Alt`, checkmarks and
accessibility, and reduces the client area.

```rust
use bamboo_native_ui::{MenuBarModel, WindowBuilder};

let result = WindowBuilder::new(App::new())
    .menu_bar(MenuBarModel::new().item("&File", file_menu())
        .item("&Help", Menu::new().item(610, "About")))
    .run();

impl Host for App {
    fn menu_bar_command(&mut self, command: u32, _window: &mut WindowHandle<'_>) {
        self.apply(Msg::Command(command));
    }
}
```

`menu_bar_command` has no widget id; `MenuEvent` runs first with source `MenuBar`.

### 5. In-page menu buttons

[`MenuButton`](../widgets/chrome/menu-button.md) opens a menu below itself;
[`SplitButton`](../widgets/chrome/split-button.md) runs a primary action and
opens alternatives from its arrow.

```rust
use bamboo_native_ui::widgets::{MenuButton, SplitButton};

ui.add(MenuButton::new("actions", "Actions(&A)",
    Menu::new().item(700, "Play").checked_item(701, "Shuffle", false)
        .separator().submenu("Add to", Menu::new().item(702, "Queue").item(703, "Favorites")),
    Msg::Command).glyph('≡'));

ui.add(SplitButton::new("export", "Export(&E)",
    Menu::new().item(720, "MP3").item(721, "FLAC"),
    || Msg::Command(799), Msg::Command));
```

Both open the same native popup; a selection arrives in `Host::menu_command` with
the button's id.

### 6. Toolbars, overflow and vertical mode

A [`Toolbar`](../widgets/chrome/toolbar.md) is a strip of fixed-width cells;
extra items move into a chevron overflow menu, so the split needs no text
measurement.

```rust
use bamboo_native_ui::widgets::{Toolbar, ToolbarItem};

ui.add(Toolbar::new("toolbar", vec![
    ToolbarItem::new(600, "Play").glyph('▶'),
    ToolbarItem::new(601, "Pause").glyph('❚'),
    ToolbarItem::separator(),
    ToolbarItem::new(602, "Favorite").checked(true),
    ToolbarItem::new(603, "").glyph('\u{E721}').icon_only().name("Search"),
], Msg::Command).item_width(104.0));

ui.add(Toolbar::new("side", vec![
    ToolbarItem::new(620, "").glyph('▶').icon_only().name("Play"),
    ToolbarItem::new(621, "").glyph('\u{E713}').icon_only().name("Settings"),
], Msg::Command).vertical().item_height(40.0));
```

`separator()` splits groups and takes no focus or overflow slot. `icon_only()`
makes a square cell; give it a `.name`. `.checked(true)` paints a toggle.
`visible_count()` reports how many fit; `vertical()` stacks cells and moves
overflow to the bottom. Commands arrive with the toolbar's id.

### 7. Status bar

[`StatusBar`](../widgets/chrome/status-bar.md) is a read-only strip of segments,
an optional progress cell and the diagonal size grip.

```rust
use bamboo_native_ui::widgets::{StatusBar, StatusSegment};
use bamboo_native_ui::TextAlign;

ui.add(StatusBar::new("status")
    .segment(StatusSegment::new("Ready").icon('♪'))
    .segment(StatusSegment::new("600 tracks").align(TextAlign::Right).width(160.0))
    .progress(0.62).size_grip(true)
    .label("Status bar"));

let changed = ui.edit_widget::<StatusBar>(WidgetId::new("status"), |bar| bar.set_progress(0.75));
if changed { ui.flush_damage(window); }
```

A `.width` segment stays fixed; `set_progress` clamps to `0..=1`.

### 8. Context menus

Attach a menu with [`WidgetProps::context_menu`](../api/props-and-narration.md).
Right click, the Menu key and `Shift+F10` open it; commands return through
`Host::menu_command`.

```rust
use bamboo_native_ui::widgets::Button;
use bamboo_native_ui::WidgetProps;

ui.add_with(Button::new("primary", "Play", Msg::Command(600)),
    WidgetProps::new().context_menu(Menu::new()
        .item(800, "Play now").item(801, "Add to queue")
        .separator().disabled_item(802, "Remove")));
```

The model is static; rebuild it for live state with
`ui.set_context_menu(WidgetId::new("primary"), Some(menu))`.

## Expected result

- The menu bar and both buttons open native popups; choices call
  `Host::menu_command` with the control's id.
- The toolbar overflows into a chevron menu as the window narrows.
- The status strip shows segments, progress and a size grip.
- Right-clicking the button opens its context menu.

## Troubleshooting

- **Commands never arrive** — implement `Host::menu_command`. For a
  `MenuBarModel` in the frame, implement `menu_bar_command` instead.
- **Access keys are never underlined** — the host did not forward `Event::Alt`.
  Call `Ui::set_mnemonic_underlines(show)`.
- **Toolbar items seem missing** — they moved into overflow. Widen the bar,
  reduce `item_width`, or check `visible_count()`.
- **The progress segment does not move** — call `flush_damage(window)` after
  `set_progress`.

## Next

[Widget catalog](../widgets/index.md): every control, its XML definition and its
events. The full menu API is in [Menus](../api/menus.md).
