# NavRail

> A vertical sidebar of single-selection navigation rows with optional icons.

## Introduction

`NavRail<M>` paints a vertical list of navigation rows. Exactly one row is
selected; it gets the active fill and a 3-DIP accent bar, and its label and
icon use the accent text color. Clicking another row, or moving with the arrow
keys in adjust mode, changes the selection and reports the row index through
`on_select`.

Rows come from `NavItem` values: a label plus an optional one-character icon.
The rail paints inside its own rectangle and does not scroll; size it to the
number of rows (`rows * row_height`) or clip deliberately.

## Usage (Rust)

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::{NavItem, NavRail};

enum Msg {
    Navigate(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    NavRail::new("sidebar", &["All tracks", "Recent", "Favorites"], 1, Msg::Navigate)
        .icon(0, '\u{E8FD}')
        .icon(1, '\u{E81C}')
        .icon(2, '\u{E734}')
        .row_height(34.0)
        .label("Library"),
);
ui.set_rect(WidgetId::new("sidebar"), Rect::from_xywh(16.0, 16.0, 180.0, 102.0));
```

`new` takes the labels, the initial selection (clamped) and the callback.
`icon(index, char)` decorates one row; a private-use codepoint is drawn with
the glyph face and any other character with the UI face. Replace the rows at
runtime with a `Vec<NavItem>`:

```rust
ui.edit_widget(WidgetId::new("sidebar"), |rail: &mut NavRail<Msg>| {
    rail.set_items(vec![NavItem::new("Queue").icon('\u{E8FD}')])
});
```

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. `<step>` children replace the rows; the
shared `selected` attribute picks the row:

```xml
<widget id="sidebar" x="16" y="16" w="180" h="102"
        row-height="34" selected="1" name="Library">
  <step label="All tracks" icon="&#xE8FD;"/>
  <step label="Recent" icon="&#xE81C;"/>
  <step label="Favorites"/>
</widget>
```

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, labels, selected, on_select)` | `&'static str, &[&str], usize, impl Fn(usize) -> M` | — | Creates one row per label; the selection is clamped. |
| `.icon(index, char)` | `usize, char` | none | Leading icon of one row. |
| `.row_height(f32)` | `f32` | `34.0` | Row height in DIPs, clamped to `>= 18`. |
| `.label(impl Into<String>)` | text | widget id | Accessible name of the rail. |
| `.disabled(bool)` | `bool` | `false` | Paints disabled and ignores input. |
| `.len()` / `.is_empty()` | `usize` / `bool` | — | Row count. |
| `.selected()` | `usize` | constructor value | Selected row. |
| `.set_selected(usize)` | silent | — | Selects a row in range without a message. |
| `.items()` | `&[NavItem]` | — | The row model. |
| `.set_items(Vec<NavItem>)` | `bool` | — | Replaces the rows silently; clamps the selection and clears hover. |
| `.select(usize)` | `Option<M>` | — | Selects through the normal path; the current row and an out-of-range index return `None`. |
| `.set_row_height(f32)` / `.row_height_value()` | `bool` / `f32` | — | Runtime row height (`>= 18`). |

`NavItem::new(label)` creates a row; `.icon(char)` adds a leading glyph, and
`label()` / `icon_glyph()` read it back.

Behaviour: clicking the already selected row emits nothing. In adjust mode
Up/Left and Down/Right step with wrapping, Home/End select the first/last row,
and Enter/Space select the current row (which emits nothing). The preferred
height is `rows * row_height`. The accessibility role is `List` with the
selected row marked selected.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(usize) -> M` passed to `new` as `on_select` | A different row becomes selected, by click or by arrow/Home/End navigation; the argument is the row index. Clicking the already selected row emits nothing. |

`set_selected`, `set_items` and the XML `selected` attribute are silent.

### Common observers

This widget does not install the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`). The constructor's `on_select` is its event
seam; anything else goes through `Ui::bind`.

### Widget-specific callbacks

| Method | Handler | Fires when |
|---|---|---|
| `new(id, labels, selected, on_select)` | `&'static str, &[&str], usize, impl Fn(usize) -> M + 'static` | Stores the selection callback; it is the only callback the widget declares. |
| `.select(index)` | returns `Option<M>` | Runtime form: selects normally and returns the message, or `None` for the current row, an out-of-range index or a disabled rail. |

### Ui triggers

Any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("sidebar"), Trigger::ValueChange, |event, ctx| {
    // Return Some(Msg::...) to queue a message; the built-in action still runs
    // unless ctx.run_default() is called explicitly.
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

It reacts to `Click`, `Activate`, `HoverEnter`/`HoverLeave` and `ValueChange`
(the selected row changed). The rail does not scroll, so `Wheel` does nothing,
and `RightClick` and the drag triggers have no built-in effect.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties apply,
including `selected` (row index, applied after the child list), `enabled` and
`name`. `font-size` and `font-family` are paint-level. The rail paints from the
palette and has no `SurfaceStyle`, so `radius`, `corners` and `background` are
reported as `xml.style.unsupported` and skipped.

Local attributes:

| Attribute | Kind | Description |
|---|---|---|
| `row-height` | number | Row height; must be `>= 18`. |

Child elements:

| Element | Attributes | Description |
|---|---|---|
| `<step>` | `label` (required, non-empty), `icon` (one character) | One row; the list is replaced. |

The child list is content: written only when the document generation changes,
variant-invariant and silent.

## See also

- `breadcrumb.md`
- `accordion.md`
- `../index.md`
- `../../api/widget-trait.md`
