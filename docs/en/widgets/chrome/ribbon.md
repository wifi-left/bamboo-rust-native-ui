# Ribbon

> A tab band over grouped large and small command tiles.

## Introduction

`Ribbon<M>` is the application's command surface: a row of tabs (`RibbonTab`)
over a body of labelled groups (`RibbonGroup`), each holding commands
(`RibbonItem`). Large items are full-height tiles with the glyph above the
label; consecutive small items fill three-row columns with the glyph beside the
label. Groups paint a divider and a bottom label.

Clicking a tab selects it and reports `on_tab`; clicking an enabled item runs
its `u32` command through `on_command`. In keyboard adjust mode Left/Right
switch tabs while the cursor is on the tab band (or move between items once it
is in the body), Down enters the body, arrows move geometrically and
Enter/Space runs the cursor item. Tab titles may carry `&` access keys
(`Alt+K`); labels follow the application's Alt underline mode.

The model is Rust-owned; a ribbon cannot be rebuilt from a document.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{
    Ribbon, RibbonGroup, RibbonItem, RibbonItemSize, RibbonTab,
};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Command(u32),
    Tab(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Ribbon::new(
        "ribbon",
        vec![
            RibbonTab::new("Home(&H)")
                .group(
                    RibbonGroup::new("Clipboard")
                        .item(RibbonItem::large(900, "Paste").glyph('\u{E77F}'))
                        .item(RibbonItem::small(901, "Cut"))
                        .item(RibbonItem::small(902, "Copy")),
                )
                .group(
                    RibbonGroup::new("Playback")
                        .item(RibbonItem::large(903, "Play").glyph('\u{E768}'))
                        .item(RibbonItem::small(904, "Next")),
                ),
            RibbonTab::new("Insert(&I)")
                .group(RibbonGroup::new("Cover").item(RibbonItem::large(910, "Add Cover"))),
        ],
        Msg::Command,
    )
    .label("Ribbon")
    .on_tab(Msg::Tab),
);
ui.set_rect(WidgetId::new("ribbon"), Rect::from_xywh(0.0, 0.0, 720.0, 140.0));
```

`RibbonItem::large` and `RibbonItem::small` are the two constructors;
`RibbonItemSize` is the enum they set (`Large` is the default variant). Small
items after a large one start a new column, and no column holds more than
three small items.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. It is construction-only chrome: it
declares no widget-local attributes and no child elements.

```xml
<widget id="ribbon" x="0" y="0" w="720" h="140"/>
```

Tabs, groups, commands and the command table stay in Rust; a document cannot
construct widget instances or message values.

## Properties

`Ribbon`:

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, tabs, on_command)` | `&'static str, Vec<RibbonTab>, impl Fn(u32) -> M` | — | Creates the ribbon; `720 × 140` DIP. |
| `.label(text)` | text | widget id | Accessible name. |
| `.on_tab(handler)` | `Fn(usize) -> M` | none | Selected tab changed by click, access key or UIA. |
| `.disabled(bool)` | `bool` | `false` | Starts disabled. |
| `.tab_height(f32)` | `f32` | `28.0` | Tab band height, clamped to `>= 20`. |
| `.small_height(f32)` | `f32` | `22.0` | Height of one small row, clamped to `>= 18`. |
| `.tabs()` / `.set_tabs(Vec<RibbonTab>)` | tabs | — | Read or replace the model (resets the body cursor). |
| `.tab_count()` / `.selected_tab()` | `usize` | — | Tab count and selected index. |
| `.select_tab(index)` | `bool` | — | Selects a tab (clamped); returns whether it changed. |
| `.tab_rect(index)` / `.item_rect(group, item)` | `Option<Rect>` | — | Geometry of a tab header or an item on the selected tab. |
| `.set_checked(tab, group, item, bool)` | `bool` | — | Runtime checkmark. |
| `.set_enabled_item(tab, group, item, bool)` | `bool` | — | Runtime item enable. |

`RibbonTab`, `RibbonGroup`, `RibbonItem`:

| Method | Type | Description |
|---|---|---|
| `RibbonTab::new(title)` | `impl AsRef<str>` | A tab; the title may carry an access key. |
| `.group(RibbonGroup)` | `RibbonGroup` | Appends a group. |
| `RibbonGroup::new(label)` | `impl Into<String>` | A labelled group. |
| `.item(RibbonItem)` | `RibbonItem` | Appends a command. |
| `RibbonItem::large(command, label)` / `::small(...)` | `u32, impl AsRef<str>` | Full tile or one small row. |
| `.glyph(char)` / `.disabled(bool)` / `.checked(bool)` | — | Icon and item state. |

### Behaviour

- **Geometry.** Tabs are 88 DIP wide in the `tab_height` band. Large items are
  58 DIP wide and span the body; small items are 96 DIP wide and 22 DIP tall in
  columns of at most three. A 16-DIP label band closes each group.
- **Pointer.** Hovering a tab or item repaints only when the target changes; a
  click selects the tab or runs the item. Selected tabs merge into the body and
  paint a 2-DIP accent top edge.
- **Keyboard.** With the cursor on the band, Left/Right change tabs and
  Home/End jump; Down enters the first item. In the body, Left/Right move
  linearly, Up/Down pick the nearest item geometrically, and Up from the first
  item returns to the band. Enter/Space run the cursor item.
- **Accessibility.** Role `TabList`; tabs are children and the selected tab's
  commands follow them as Button children (offset by 100). Invoking a tab
  selects it; invoking a command runs it.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `impl Fn(u32) -> M` passed to `new` as `on_command` | An enabled item is clicked, run by `Enter`/`Space` on the cursor item, or invoked through accessibility or UIA. |
| `impl Fn(usize) -> M` passed to `.on_tab` | A tab is selected by click, an `Alt+key` access key or a UIA invoke; the index is the new tab. |

Hovering, moving the cursor and `select_tab` (the Rust setter) emit nothing.

### Common observers

This widget installs none of the shared observer builders. Use `on_command` /
`on_tab` or `Ui::bind`.

### Widget-specific callbacks

| Callback | Signature | Fires when |
|---|---|---|
| `new(id, tabs, on_command)` | `&'static str, Vec<RibbonTab>, impl Fn(u32) -> M` | A command item runs. |
| `.on_tab(handler)` | `impl Fn(usize) -> M` | The selected tab changes from user input. |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("ribbon"), Trigger::Click, |event, ctx| {
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

The ribbon reacts to `Click`, `HoverEnter`/`HoverLeave` and `Activate`; wheel,
drag and right click have no built-in effect.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties (`visible`,
`tooltip`, `name`, `description`, `automation-id`, `role`, `expose`, enabled,
focus policy) apply. The ribbon paints from the palette and has no
`SurfaceStyle`, so `radius`, `corners` and `background` are reported as
`xml.style.unsupported`. It declares no shared content and no child list.

Widget-local attributes: none. Child elements: none (`<tab>` and `<command>`
are rejected as `xml.skin.unknown`).

## See also

- `./toolbar.md`
- `./menu-bar.md`
- `./status-bar.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
