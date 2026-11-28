# 09 — Menus, toolbars and status

> Wrap a page in application chrome: a menu bar, in-page menu buttons, a
> toolbar with overflow, a status strip and context menus — all routing through
> one command handler.

## What you will build

A shell with a painted `MenuBar`, `MenuButton`, `SplitButton`, both `Toolbar`
orientations, a `StatusBar` and a context menu. `Msg::Command(u32)` carries
every command to `Host::menu_command` or `menu_bar_command`.

**New terms on this page**

- **Command id** — a nonzero integer that names an action ("play", "export").
  Menus, toolbars and context menus report ids; your application maps them to
  messages. `0` is reserved for dismissal.
- **Command handler** — `Host::menu_command` for widget popups and context
  menus, `Host::menu_bar_command` for the native frame menu. The first also
  receives the widget id that opened the popup.
- **Menu model** — the `Menu` value you build with `.item(...)`. It describes
  labels, submenus, check state and separators; it is not a live UI object.
- **Menu bar (painted)** — `MenuBar`: an ordinary widget you position, which
  draws its own cells and opens native popups.
- **Native menu bar** — `MenuBarModel`: installed in the window frame, drawn by
  the system, and it reduces the client area.
- **Menu button / split button** — in-page buttons that open a menu; the split
  button also has a primary action.
- **Toolbar** — a strip of fixed-width command cells. Cells that do not fit move
  into a chevron **overflow** menu.
- **Status bar** — a read-only strip of segments plus an optional progress cell
  and a resize grip.
- **Context menu** — a menu attached to a widget, opened by right click, the
  Menu key or `Shift+F10`.
- **Access key (mnemonic)** — the underlined letter in `&File`, activated with
  Alt. `Alt+F` opens File.
- **`MenuEvent` / `MenuDecision`** — a hook into a popup's life cycle, and its
  answer: `Pass` (continue) or `Handled` (cancel/suppress).

## Prerequisites

- The window from [03](03-first-window.md) builds and runs.
- No feature flag is needed; the chrome widgets are always compiled.

The snippets extend `src/main.rs` in the crate folder
`bamboo-hello/bamboo-hello/`. Run `cargo run` from that folder or from the
workspace folder one level up; no new files or feature flags are involved.

## Steps

### 1. One message, one menu model

Command ids are nonzero; one `Msg::Command` and one shared `Menu` model.

```rust
use bamboo_native_ui::{Menu, MenuItem, Submenu};

#[derive(Clone, Copy, Debug)]
enum Msg {
    // One variant carries every command; the number identifies the action.
    Command(u32),
}

fn file_menu() -> Menu {
    Menu::new()
        // Ids are your own numbering (nonzero); the label is display text.
        .item(500, "New playlist")
        .item(501, "Open library...")
        // A display-only divider between groups.
        .separator()
        // Submenu nests a whole Menu under one label.
        .push(Submenu::new("Recent", Menu::new().item(504, "Nocturne")))
        // The shortcut text is painted in the native accelerator column.
        .push(MenuItem::new(506, "Export...").shortcut("Ctrl+E"))
}
```

`item` appends a command, `checked_item`/`disabled_item` set the initial state,
`separator` draws a divider and `push(Submenu::new(label, menu))` nests a submenu.

The ids in this file are arbitrary; what matters is that the application uses
the same numbers in its `Msg::Command` handler. A label is never matched
against code, so renaming "Export..." to "Export audio..." changes nothing but
the visible text.

**What just happened.** Nothing is drawn yet: `file_menu()` only builds a
description. The same value is reused by the painted menu bar, the native menu
bar and the menu buttons below, so there is one source of truth for the File
menu's contents and ids.

### 2. Add the painted menu bar

[`MenuBar`](../widgets/chrome/menu-bar.md) paints top-level cells; a click,
Enter/Space or `Alt+key` opens the cell's menu as a native popup.

```rust
use bamboo_native_ui::widgets::MenuBar;
use bamboo_native_ui::{Rect, Ui, WidgetId};

fn add_menu_bar(ui: &mut Ui<Msg>) {
    ui.add(
        // The second argument maps a chosen command id to a message.
        MenuBar::new("menubar", Msg::Command)
            // `&` marks the access key (Alt+F); it is not painted.
            .item("&File", file_menu())
            .item("&Edit", Menu::new().item(520, "Undo").item(521, "Redo")
                .checked_item(522, "Shuffle", false)),
    );
    // A painted menu bar is a normal widget: it needs a rectangle.
    ui.set_rect(WidgetId::new("menubar"), Rect::from_xywh(0.0, 0.0, 420.0, 26.0));
}
```

A label carries its access key as `&File` or `File(&F)`; the marker is removed
and underlined while Alt mode is on. Forward the tap:

```rust
if let Event::Alt(show) = *event {
    // Held Alt: underline access keys; released: back to normal.
    self.ui.set_mnemonic_underlines(show);
}
```

Only chrome widgets with a marked label react; `flush_damage` repaints them.

The painted bar is a widget like any other: it lives inside the client area at
the rectangle you give it, and it does not shrink the space available to the
rest of your page. `MenuBar::new` starts with a 420×26 rectangle, so forgetting
`set_rect` leaves the bar at the top-left and possibly on top of other widgets.

**What just happened.** Two top-level cells exist ("File" and "Edit") and can
open native popups. Choosing an item reports through `Host::menu_command` with
this bar's id and the command number; the routing in step 3 turns that into
`Msg::Command`. The bar itself has no knowledge of what "Undo" does.

### 3. Route the commands

Every chrome and context popup reports through
[`Host::menu_command`](../api/host-and-window.md); the widget id names the control.

```rust
impl Host for App {
    fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>) {
        // Built-in Undo/Cut/Copy/Paste of painted editors first; true = handled.
        if self.ui.menu_command(widget, command, window) {
            self.ui.flush_damage(window);
            return;
        }
        // Everything else is an application command.
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
        // Opening runs before the popup is built: edit enabled/checked state.
        if let MenuEvent::Opening { menu, .. } = event {
            menu.set_enabled(520, self.can_undo);
        }
        // Pass lets the selection replay through menu_command.
        MenuDecision::Pass
    }
}
```

`Opening` mutates the model, `Opened` edits the live popup, `Selected` rewrites
the command and `Closed` reports the final command or `None`.

**What just happened.** A chosen item now has one route to your code: the popup
reports through `Host::menu_command` with the control's id, that handler turns
the number into `Msg::Command`, and `apply` performs the action. `MenuEvent` is
an optional earlier hook for changing the menu before the user sees it.

### 4. The native window menu bar

A painted `MenuBar` is a widget you position. A `MenuBarModel` is installed in
the window frame: the system draws it, handles `Alt`, checkmarks and
accessibility, and reduces the client area.

```rust
use bamboo_native_ui::{MenuBarModel, WindowBuilder};

let result = WindowBuilder::new(App::new())
    // Installed in the window frame before run(): the system draws this bar.
    .menu_bar(MenuBarModel::new().item("&File", file_menu())
        .item("&Help", Menu::new().item(610, "About")))
    .run();

impl Host for App {
    // The frame bar has no widget id, so this is the whole signature.
    fn menu_bar_command(&mut self, command: u32, _window: &mut WindowHandle<'_>) {
        self.apply(Msg::Command(command));
    }
}
```

`menu_bar_command` has no widget id; `MenuEvent` runs first with source `MenuBar`.

The native bar belongs to the frame, not to your page: Windows draws it above
the client area and subtracts its height, so the `size` your `resize` receives
already excludes it. You cannot position it with `set_rect`, and widgets placed
at `y="0"` sit below it. The painted `MenuBar` from step 2 is a different
thing; use one or the other.

**What just happened.** The window now owns a system menu bar with its own Alt,
keyboard and accessibility behaviour, and its commands arrive at a separate
callback. `MenuEvent` still runs first, with `source` telling you which bar
opened the popup.

### 5. In-page menu buttons

[`MenuButton`](../widgets/chrome/menu-button.md) opens a menu below itself;
[`SplitButton`](../widgets/chrome/split-button.md) runs a primary action and
opens alternatives from its arrow.

```rust
use bamboo_native_ui::widgets::{MenuButton, SplitButton};

// MenuButton: one button; "Actions(&A)" gives the label "Actions" and Alt+A.
ui.add(MenuButton::new("actions", "Actions(&A)",
    Menu::new().item(700, "Play").checked_item(701, "Shuffle", false)
        .separator().submenu("Add to", Menu::new().item(702, "Queue").item(703, "Favorites")),
    Msg::Command).glyph('≡'));

// SplitButton: a primary action (799) plus an arrow that opens the menu.
ui.add(SplitButton::new("export", "Export(&E)",
    Menu::new().item(720, "MP3").item(721, "FLAC"),
    || Msg::Command(799), Msg::Command));
```

Both open the same native popup; a selection arrives in `Host::menu_command` with
the button's id.

The split button's primary half does not open a popup: it runs the closure
directly, so a plain click sends `Msg::Command(799)` and only the arrow asks for
a format. Both widgets still need rectangles from `resize` or a document, like
any other widget; without one they keep their constructor rectangles and can
overlap the page.

**What just happened.** Both controls reuse the menu values from step 1. The
menu button reports `700`-`703` through `Host::menu_command`; the split button's
main half reports `799` immediately, and its arrow reports `720` or `721`.

### 6. Toolbars, overflow and vertical mode

A [`Toolbar`](../widgets/chrome/toolbar.md) is a strip of fixed-width cells;
extra items move into a chevron overflow menu, so the split needs no text
measurement.

```rust
use bamboo_native_ui::widgets::{Toolbar, ToolbarItem};

ui.add(Toolbar::new("toolbar", vec![
    // Each cell is fixed width; extras move into the chevron overflow.
    ToolbarItem::new(600, "Play").glyph('▶'),
    ToolbarItem::new(601, "Pause").glyph('❚'),
    // A separator takes no focus and no overflow slot.
    ToolbarItem::separator(),
    // checked(true) starts the item drawn as a checked toggle.
    ToolbarItem::new(602, "Favorite").checked(true),
    // icon_only() makes a square cell; name is its accessible label.
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

The toolbar does not wrap: when the available width runs out, whole cells move
into the overflow menu, so a cell is either fully visible or in the chevron.
`vertical()` uses the height instead of the width for the same decision. Both
toolbars need a rectangle; `item_width`/`item_height` size the cells, not the
bar.

**What just happened.** The horizontal strip lays out as many 104 DIP cells as
fit and puts the rest behind the chevron. The vertical strip does the same
downward. Choosing any cell (visible or overflow) produces `Msg::Command` with
that cell's id.

### 7. Status bar

[`StatusBar`](../widgets/chrome/status-bar.md) is a read-only strip of segments,
an optional progress cell and the diagonal size grip.

```rust
use bamboo_native_ui::widgets::{StatusBar, StatusSegment};
use bamboo_native_ui::TextAlign;

ui.add(StatusBar::new("status")
    // A segment with a glyph and text.
    .segment(StatusSegment::new("Ready").icon('♪'))
    // A fixed-width, right-aligned segment.
    .segment(StatusSegment::new("600 tracks").align(TextAlign::Right).width(160.0))
    // Initial progress and the bottom-right size grip.
    .progress(0.62).size_grip(true)
    // Accessible name of the strip (defaults to the widget id).
    .label("Status bar"));

// set_progress clamps to 0..=1 and returns whether it changed anything.
let changed = ui.edit_widget::<StatusBar>(WidgetId::new("status"), |bar| bar.set_progress(0.75));
if changed { ui.flush_damage(window); }
```

A `.width` segment stays fixed; `set_progress` clamps to `0..=1`.

The status bar is read-only: it has no command ids and reports no messages.
Progress is application state pushed in, not a slider the user can drag. As with
every other widget on this page, the bar needs a rectangle before it is visible
where you expect.

**What just happened.** The strip painted two segments, a 62% progress cell and
the grip, then `set_progress(0.75)` moved the fill and scheduled one repaint
through `flush_damage`.

### 8. Context menus

Attach a menu with [`WidgetProps::context_menu`](../api/props-and-narration.md).
Right click, the Menu key and `Shift+F10` open it; commands return through
`Host::menu_command`.

```rust
use bamboo_native_ui::widgets::Button;
use bamboo_native_ui::WidgetProps;

ui.add_with(Button::new("primary", "Play", Msg::Command(600)),
    // The menu is attached through the props, not through the button type.
    WidgetProps::new().context_menu(Menu::new()
        .item(800, "Play now").item(801, "Add to queue")
        .separator().disabled_item(802, "Remove")));
```

The model is static; rebuild it for live state with
`ui.set_context_menu(WidgetId::new("primary"), Some(menu))`.

The button's own click still sends `Msg::Command(600)`; the context menu sends
`800`, `801` or `802` through `Host::menu_command` with the button's id. The two
paths do not interfere, which is why a context menu can offer alternatives
without changing the primary action.

**What just happened.** The button now answers a right click (or the Menu key,
or `Shift+F10`) with a native popup, while a normal click keeps its original
message. Dismissing the menu sends nothing.

## Expected result

- The menu bar and both buttons open native popups; choices call
  `Host::menu_command` with the control's id.
- The toolbar overflows into a chevron menu as the window narrows.
- The status strip shows segments, progress and a size grip.
- Right-clicking the button opens its context menu.

**Run it.** From the workspace folder `bamboo-hello/` or the crate folder
`bamboo-hello/bamboo-hello/`:

```powershell
cargo run
```

The window appears with the chrome in place: a menu bar at the top, the in-page
buttons, both toolbars and the status strip at the bottom. Open File with the
mouse or `Alt+F`; choosing an item reaches your handler. To make the result
obvious while you are wiring it, print the command in `apply` or write it into
a label.

## Troubleshooting

- **The window opens without the chrome** — the widgets were added but never
  given rectangles. Every chrome widget keeps a constructor rectangle until
  `set_rect` (from `resize`) or a document moves it, so bars and toolbars can
  stack at the origin and hide each other.
- **Commands never arrive** — implement `Host::menu_command`. For a
  `MenuBarModel` in the frame, implement `menu_bar_command` instead.
- **A widget menu works but the native bar does nothing** — they use different
  callbacks. A painted `MenuBar`, `MenuButton`, `SplitButton`, toolbar and
  context menu report to `menu_command(widget, command)`; only the frame
  `MenuBarModel` reports to `menu_bar_command(command)`. Implementing one does
  not cover the other, and a missing callback drops the command.
- **The program panics while building a menu** — a command id is `0`.
  `Menu::item(0, ...)` panics in debug builds because `0` is reserved for
  dismissal; number your commands from 1.
- **Access keys are never underlined** — the host did not forward `Event::Alt`.
  Call `Ui::set_mnemonic_underlines(show)`.
- **Toolbar items seem missing** — they moved into overflow. Widen the bar,
  reduce `item_width`, or check `visible_count()`. A cell is never half
  visible.
- **The painted menu bar is not where you expect** — it is a widget; give it a
  rectangle with `set_rect`. Without one it keeps its 420×26 constructor
  rectangle at the client origin.
- **The native menu bar does not appear** — it must be installed on the
  `WindowBuilder` with `.menu_bar(...)` before `.run()`. Adding a painted
  `MenuBar` widget is a different menu; it does not become the frame bar.
- **A menu item is permanently grey** — the item was built with
  `disabled_item`, or `MenuEvent::Opening` called `set_enabled(false)`. Check
  the live condition used there (`self.can_undo` in the example).
- **The context menu does not open** — the widget is disabled, its rectangle is
  empty or off-screen (so the click lands outside it), or its props were never
  registered with `add_with`. Right click, the Menu key and `Shift+F10` all use
  the same model.
- **The progress segment does not move** — call `flush_damage(window)` after
  `set_progress`. The builder `.progress(0.62)` only sets the initial value.

## Next

[Widget catalog](../widgets/index.md): every control, its XML definition and its
events. The full menu API is in [Menus](../api/menus.md).
