# Table

> A virtualized table with sortable, resizable, reorderable columns and inline cell editing.

## Introduction

`Table<M>` paints a fixed header band over a virtualized body. Columns are
geometry plus a title and alignment; rows are lists of cell strings, and only
rows intersecting the body are painted, hit-tested or published to
accessibility. Sorting is application-owned: a header click reports its column
index, the application re-sorts and calls `set_sort`, which paints the
indicator. Dividers resize fixed columns, a header drag reorders a column, and
an editable table edits cells in place.

## Usage (Rust)

```rust
use bamboo_native_ui::{Rect, TextAlign, Ui, WidgetId};
use bamboo_native_ui::widgets::{Table, TableColumn, TableRow};

enum Msg {
    Select(usize),
    Open(usize),
    Sort(usize),
    Edit(usize, usize, String),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Table::new("tracks")
        .columns(vec![
            TableColumn::new("#", 56.0).align(TextAlign::Right),
            TableColumn::flex("Title"),
            TableColumn::new("Length", 72.0).align(TextAlign::Right),
        ])
        .rows(vec![
            TableRow::new(["1", "Nocturne", "3:58"]),
            TableRow::new(["2", "Sunny Day", "4:29"]).disabled(true),
        ])
        .striped(true)
        .editable(true)
        .on_select(Msg::Select)
        .on_activate(Msg::Open)
        .on_header_click(Msg::Sort)
        .on_cell_edit(Msg::Edit),
);
ui.set_rect(WidgetId::new("tracks"), Rect::from_xywh(16.0, 56.0, 520.0, 300.0));
```

After a header click, re-sort and paint the indicator; a resized width or a
moved column is already applied when its callback runs.

```rust
// In the Msg::Sort(column) arm, after sorting the application's rows:
ui.edit_widget(WidgetId::new("tracks"), |table: &mut Table<Msg>| {
    table.set_rows(sorted);
    table.set_sort(Some((column, ascending)))
});
```

`on_cell_edit(row, column, text)` fires after the draft is applied; persist the value.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. A table child list is the complete data:
`<column>` elements replace the columns and `<row>` elements replace the rows.

```xml
<widget id="tracks" x="16" y="56" w="520" h="300"
        header-height="30" row-height="32" cell-padding="10"
        striped="true" grid-lines="true" editable="true" selected="0">
  <column title="#" width="56" align="right"/>
  <column title="Title" flex="true"/>
  <column title="Length" width="72" align="right"/>
  <row><cell text="1"/><cell text="Nocturne"/><cell text="3:58"/></row>
  <row><cell text="2"/><cell text="Sunny Day"/><cell text="4:29"/></row>
</widget>
```

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates an empty table. |
| `.columns(Vec<TableColumn>)` / `.rows(Vec<TableRow>)` | model | empty | Replaces the columns or rows. |
| `.header_height(f32)` / `.row_height(f32)` | `f32` | `30.0` / `32.0` | Header band and body row height; row height is clamped to `>= 16`. |
| `.cell_padding(f32)` | `f32` | `10.0` | Horizontal padding inside header and cell rectangles. |
| `.striped(bool)` / `.grid_lines(bool)` | `bool` | `false` / `true` | Alternate-row tint; row separators and column lines. |
| `.empty_text(impl Into<String>)` / `.sort(column, ascending)` | text / `usize, bool` | `""` / none | Empty message; initial sort indicator (rows are not reordered). |
| `.on_select(impl Fn(usize) -> M)` | row handler | `None` | A row was clicked (it is selected first). |
| `.on_activate(impl Fn(usize) -> M)` | row handler | `None` | Enter or a double click on a non-editable row. |
| `.on_header_click(impl Fn(usize) -> M)` | column handler | `None` | A header cell was clicked. |
| `.on_column_resize(impl Fn(usize, f32) -> M)` / `.on_column_reorder(impl Fn(usize, usize) -> M)` | column handlers | `None` | A divider drag resized a fixed column, or a header drag moved a column (both already applied). |
| `.editable(bool)` | `bool` | `false` | Activates in-place cell editing. |
| `.on_cell_edit(impl Fn(usize, usize, String) -> M)` | row, column, text | `None` | A cell draft was committed (already applied). |
| `.scrollbar_style(ScrollBarStyle)` | style | theme default | Scrollbar look for both bars. |

`TableColumn`: `new(title, width)` (fixed, width `>= 0`), `flex(title)`,
`.align(TextAlign)`, `title()`, `width()`, `is_flex()`, `alignment()`,
`set_title()`. `TableRow`: `new(cells)` from any iterator of strings,
`.disabled(bool)`, `.cell(column)` (empty string past the last cell) and the
public `cells`/`disabled` fields.

Typed access and runtime setters:

| Method | Type | Description |
|---|---|---|
| `.columns_ref()` / `.rows_ref()` / `.row(usize)` / `.row_count()` / `.set_columns(..)` / `.set_rows(..)` | mixed | Current data; the setters replace it. |
| `.selected_row()` / `.selected_cell()` / `.select_cell(row, column)` | cursors | Row cursor, cell cursor, clamped move. |
| `.cell_text(row, column)` / `.cell_rect(row, column)` / `.column_span(usize)` / `.set_column_width(usize, f32)` | mixed | Cell text, clipped cell rectangle, column extent; resize (flex ignores it). |
| `.set_cell(row, column, text)` | `bool` | Writes one cell, padding the row if needed. |
| `.set_sort(Option<(usize, bool)>)` / `.sort_order()` / `.move_column(from, to)` | mixed | Paint/clear the arrow; remove and reinsert a column. |
| `.edit_cell(row, column)` / `.is_editing_cell()` | `bool` | Open or query an inline session. |
| `.scroll_to_row`, `.ensure_row_visible`, `.scroll_offset`, `.scroll_x_offset`, `.visible_rows` | mixed | Scrolling and viewport. |

Behaviour: the header reserves sort-arrow room. Resize targets only fixed
columns and stop at 24 DIPs; a header drag travels at least 6 DIPs before it
becomes a reorder. Inline editing starts on Enter, F2 or a double click
(Enter/Tab commits, Escape cancels); a newline value gets the multiline editor.
Keys move by row/column, page and edge. The role is `Grid` with `ColumnHeader`
children and per-cell values.

## Events

### Messages

Every callback is a message-returning builder closure (`Fn(...) -> M`); `new`
takes only the id, so nothing is passed to the constructor.

| Message | Emitted when |
|---|---|
| `Fn(usize) -> M` passed to `.on_select` | A row was clicked and selected. |
| `Fn(usize) -> M` passed to `.on_activate` | Enter or a double click on a non-editable row. |
| `Fn(usize) -> M` passed to `.on_header_click` | A header cell was clicked (the application re-sorts). |
| `Fn(usize, f32) -> M` passed to `.on_column_resize` | A divider drag ended; the new width is already applied. |
| `Fn(usize, usize) -> M` passed to `.on_column_reorder` | A header drag dropped; the column move is already applied. |
| `Fn(usize, usize, String) -> M` passed to `.on_cell_edit` | A cell draft was committed; the new text is already applied. |

### Common observers

This widget does not install the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`). The callbacks below are its event seam;
anything else goes through `Ui::bind`.

### Widget-specific callbacks

| Method | Handler | Fires when |
|---|---|---|
| `.on_select(handler)` | `impl Fn(usize) -> M + 'static` | Press and release on a row; the row is selected before the handler runs. |
| `.on_activate(handler)` | `impl Fn(usize) -> M + 'static` | Enter, a UIA invoke, or a double click on a row that is not editable. |
| `.on_header_click(handler)` | `impl Fn(usize) -> M + 'static` | A header cell was clicked; return the column index and re-sort with `set_sort`. |
| `.on_column_resize(handler)` | `impl Fn(usize, f32) -> M + 'static` | A fixed column's header divider moved; `(column, new_width)`, already applied. |
| `.on_column_reorder(handler)` | `impl Fn(usize, usize) -> M + 'static` | A header drag moved a column; `(from, to)`, already applied. |
| `.on_cell_edit(handler)` | `impl Fn(usize, usize, String) -> M + 'static` | An inline session committed; `(row, column, text)`, already applied. |

`on_cell_edit` only runs when `.editable(true)` opened the session; a double
click on an editable cell starts editing instead of activating.

### Ui triggers

Any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("tracks"), Trigger::Click, |event, ctx| {
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

It reacts to `Click` (select), `DoubleClick` (activate or edit), `Activate`,
`Wheel` (both axes) and `ValueChange` (row/cell selection). Header resizing and
reordering respond to press/move/release input rather than to these triggers;
`RightClick` has no built-in effect.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties apply,
including `selected` (a row index), `read-only` (locks cell editing) and
`name`. `font-size`/`font-family` are paint-level. The table paints from the
palette and has no `SurfaceStyle`, so `radius`, `corners` and `background` are
reported as `xml.style.unsupported` and skipped.

| Attribute | Kind | Description |
|---|---|---|
| `row-height` | number | Body row height; must be `>= 16`. |
| `header-height` | number | Header band height; `0` hides it. |
| `cell-padding` | number | Horizontal cell padding; `>= 0`. |
| `striped` | bool | Panel tint on alternate rows. |
| `grid-lines` | bool | Separators (default on). |
| `editable` | bool | Enables cell editing. |
| `empty-text` | text | Message while there are no rows. |

Child elements:

| Element | Attributes | Description |
|---|---|---|
| `<column>` | `title` (required, non-empty), `width` (`>= 0`), `flex` (bool), `align` (`left`/`center`/`right`) | One column; `flex` ignores `width`. |
| `<row>` | `disabled` (bool); children `<cell text>` | One row; `text` is required and may be empty. |

The child list is content: written only when the document generation changes,
and columns and rows replace together, so a document that changes one declares
both.

## See also

- `tree-view.md`
- `property-grid.md`
- `list-view.md`
- `../../guides/xml-layout-authoring.md`
