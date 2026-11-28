# Toolbar

> A strip of command buttons with separators, icon-only cells and an overflow menu.

## Introduction

`Toolbar<M>` paints a row (or column) of `ToolbarItem` command cells. Items that
do not fit move into a chevron overflow menu instead of being clipped. Cells
have a fixed extent, so the overflow boundary is pure geometry: accessibility,
hit-testing and a UIA action see the same layout before the first paint.

`ToolbarItem::separator` splits the strip into visual groups; it owns no
command, is skipped by the keyboard cursor, takes no hover and never appears in
the overflow menu. An item with `.icon_only()` (or a glyph and an empty label)
paints only its glyph and becomes square on a horizontal bar. `.checked(true)`
paints an accent-tinted cell and publishes the checkmark in the overflow menu.

A horizontal bar uses Left/Right for its cursor and a vertical bar Up/Down;
Enter/Space runs the current item (or opens the overflow list). The toolbar is
one Tab stop and publishes `ToolBar` with one child per command.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{Toolbar, ToolbarItem};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Command(u32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(Toolbar::new(
    "tools",
    vec![
        ToolbarItem::new(600, "Play").glyph('▶'),
        ToolbarItem::new(601, "Pause").glyph('❚'),
        ToolbarItem::separator(),
        ToolbarItem::new(602, "Favorite").checked(true),
        ToolbarItem::new(603, "")
            .glyph('\u{E721}')
            .icon_only()
            .name("Search"),
    ],
    Msg::Command,
));
ui.set_rect(WidgetId::new("tools"), Rect::from_xywh(0.0, 32.0, 420.0, 32.0));
```

Call `.vertical()` to stack the cells (an app toolbar or task pane): the
overflow button moves to the bottom, Up/Down drive the cursor, and
`preferred_height` becomes `8 + the sum of the cell extents`.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. It is construction-only chrome: it
declares no widget-local attributes and no child elements.

```xml
<widget id="tools" x="0" y="32" w="420" h="32"/>
```

Items, glyphs, commands and the vertical flag stay in Rust.

## Properties

`Toolbar`:

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, items, on_command)` | `&'static str, Vec<ToolbarItem>, impl Fn(u32) -> M` | — | Creates the strip; `420 × 32` DIP. |
| `.vertical()` | — | horizontal | Stacks cells; overflow moves to the bottom. |
| `.item_width(f32)` | `f32` | `104.0` | Horizontal cell width, clamped to `>= 40`. |
| `.item_height(f32)` | `f32` | `32.0` | Vertical cell height, clamped to `>= 24`. |
| `.disabled(bool)` | `bool` | `false` | Starts disabled. |
| `.items()` / `.set_items(Vec<ToolbarItem>)` | items | — | Read or replace the model (resets the cursor). |
| `.insert_item(index, item)` / `.remove_item(index)` | index | — | Runtime edits without a rebuild. |
| `.set_item_label(index, &str)` | index | — | Renames one non-separator cell; returns whether it changed. |
| `.set_checked(index, bool)` / `.set_enabled_item(index, bool)` | index | — | Runtime item state; return whether it changed. |
| `.visible_count()` | `usize` | — | Items that fit outside the overflow button. |
| `.is_vertical()` / `.mnemonic_underlines()` | `bool` | — | Current flags. |

`ToolbarItem`:

| Method | Type | Default | Description |
|---|---|---|---|
| `new(command, label)` | `u32, impl AsRef<str>` | — | A command cell; label may carry an access key. |
| `separator()` | — | — | A 9-DIP group rule with no command. |
| `.glyph(char)` | `char` | none | Leading MDL2 codepoint or text character. |
| `.icon_only()` | — | off | Paints only the glyph; square on a horizontal bar. |
| `.name(text)` | text | painted label | Accessible/tooltip name for an icon-only cell. |
| `.disabled(bool)` / `.checked(bool)` | `bool` | `false` | Item state. |

### Behaviour

- **Geometry.** Horizontal bars start cells 4 DIP from the left; a command cell
  is `item_width`, a separator is 9 DIP, and an icon-only cell is square using
  the bar's height. The overflow button is 32 DIP at the right (horizontal) or
  bottom (vertical).
- **Overflow.** When the total extent exceeds the free space, trailing items are
  hidden and the chevron opens a menu built from them (separators omitted,
  disabled state preserved). Hidden commands remain in the accessibility tree.
- **Keyboard.** The axis arrows move the cursor between command cells and the
  overflow button, Home/End jump, Enter/Space runs the cursor. `owns_navigation_key`
  releases the axis key at the ends so focus can leave.
- **Accessibility.** Role `ToolBar`; one child per visible command plus a
  “more” child for the overflow, with hidden commands published as menu items.
  Invoking a child runs its command; invoking the overflow opens it.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `impl Fn(u32) -> M` passed to `new` as `on_command` | A command cell is clicked, run by `Enter`/`Space` on the cursor cell, invoked through accessibility or UIA, or chosen from the overflow menu. |

Separators own no command; moving the cursor emits nothing.

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
| `new(id, items, on_command)` | `&'static str, Vec<ToolbarItem>, impl Fn(u32) -> M` | Any visible or overflow command runs. |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("tools"), Trigger::Click, |event, ctx| {
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

The toolbar reacts to `Click`, `HoverEnter`/`HoverLeave` and `Activate`; wheel,
drag and right click have no built-in effect.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties (`visible`,
`tooltip`, `name`, `description`, `automation-id`, `role`, `expose`, enabled,
focus policy) apply. The toolbar paints from the palette and has no
`SurfaceStyle`, so `radius`, `corners` and `background` are reported as
`xml.style.unsupported`. It declares no shared content and no child list.

Widget-local attributes: none. Child elements: none. The item list is
construction-only, so `<cell>` children are rejected as `xml.skin.unknown`.

## See also

- `./ribbon.md`
- `./menu-bar.md`
- `./status-bar.md`
- `../player/playback-transport.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
