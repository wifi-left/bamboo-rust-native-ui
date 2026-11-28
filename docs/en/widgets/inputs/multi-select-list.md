# MultiSelectList

> A virtualized list of checkable rows with Ctrl/Shift multi-selection.

## Introduction

`MultiSelectList<M>` paints a scrollable list of rows, each with a check box
and a label. Selection is a per-row checked flag, independent of the other
rows. Only the rows intersecting the viewport are painted and published, so a
long list stays cheap.

A plain click toggles the clicked row and moves the anchor. Ctrl+click toggles
one row and moves the anchor there; Shift+click selects the inclusive range
from the anchor and keeps it (a Shift+click before any earlier click behaves
like a plain toggle). The wheel scrolls by fractional DIPs (so rows and the
thumb move together), and the themed bar can be hovered, clicked and dragged.
Keyboard: Up/Down move the cursor, Home/End jump, Space/Enter toggle the
cursor row.

Every change emits `on_change(selected)` with the full sorted list of checked
indexes; a Shift range that changes nothing emits nothing. Accessibility is
`A11yRole::MultiSelectList` with one child per visible row (carrying
`selected`) plus a vertical scroll range. `M` is the message type returned by
`on_change`.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::MultiSelectList;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Formats(Vec<usize>) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        MultiSelectList::new(
            "formats",
            &["Lossless", "Local library", "Auto cover"],
            Msg::Formats,
        )
        .checked(&[0, 2])
        .row_height(28.0),
    );
    ui.set_rect(WidgetId::new("formats"), Rect::from_xywh(16.0, 16.0, 220.0, 160.0));
}
```

`.checked(&[usize])` seeds the selection without emitting. The default rectangle
is 220×160. `set_selection(&[usize])`, `set_checked(index, bool)` and
`set_rows(Vec<String>)` synchronise the widget from the model silently.

## Usage (XML)

`MultiSelectList` is not in the standard registry. Create it in Rust and
address it by `id` from the document:

```xml
<widget id="formats" x="16" y="16" w="220" h="160" selected="0"/>
```

`selected` is the only shared content (`content_caps` exposes `SELECTION`); it
moves the keyboard cursor, not the checked flags. Row labels and the checked
set are application data, not XML children — this widget declares no
`skin_children`.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, rows, on_change)` | `&'static str, &[&str], impl Fn(Vec<usize>) -> M + 'static` | — | Creates the list. |
| `.checked(&[usize])` | `&[usize]` | none checked | Mark rows checked at creation (no message). |
| `.check(char)` | `char` | `'\u{E73E}'` | Checked-state mark glyph. |
| `.square()` | — | rounded | Sharp row check boxes (radius 0). |
| `.corner_radius(f32)` | `f32` | `4.0` | Row check-box radius, clamped to half the box. |
| `.row_height(f32)` | `f32` | `28.0` | Row height in DIPs (minimum 18). |
| `.scrollbar_style(ScrollBarStyle)` | `ScrollBarStyle` | default themed bar | Bar thickness, insets and colours. |

Getters and setters: `selected() -> Vec<usize>`, `is_checked(index)`,
`set_checked(index, bool)`, `set_selection(&[usize])`, `set_rows(Vec<String>)`,
`corner_radius_value()`, `set_corner_radius(f32)` and `scrollbar()`. Observer
hooks (`on_click`, `on_right_click`, `on_double_click`, `on_key_down`,
`on_key_up`, `on_focus_gained`, `on_focus_lost`) run before the built-in
behavior.

Behaviour notes:

- `multi_selectable()` and `adjustable()` are true; Ctrl/Shift clicks arrive as
  `SelectWith`.
- A press on the scrollbar is a bar drag, never a row toggle.
- The list clips its rows to the rectangle and keeps the cursor visible when
  it moves.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(Vec<usize>) -> M` passed to `new` (`on_change`) | The checked set changes; the argument is the full ascending list of checked indexes. A Shift range that changes nothing emits nothing. |

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
callback.

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

A `Click` on a row toggles it and `Activate` toggles the cursor row;
`HoverEnter`/`HoverLeave` highlight a row, `Wheel` scrolls, and
`DragStart`/`DragEnd` drive the scrollbar. `ValueChange` follows the checked
set.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- Geometry: normal `widget` rules apply; the height decides how many rows are
  visible before scrolling.
- Shared content: `selected` (moves the cursor silently).
- Local attributes: none.
- Style tokens: no `SurfaceStyle`; `radius`, `corners` and `background` are
  unsupported (`xml.style.unsupported`, a warning).
- Child elements: none.

## See also

- `../index.md`
- `combo-box.md`
- `checkbox.md`
- `../data/list-view.md`
- `../../guides/xml-layout-authoring.md`
