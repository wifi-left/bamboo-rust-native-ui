# 08 — Lists, tables and trees

> Fill a page with three virtualized data views: a 640-row list, a sortable and
> editable table, and an expandable tree — in Rust and as `.buixml` child lists.

## What you will build

A track browser: [`ListView`](../widgets/data/list-view.md) is a flat list with a
subtitle and trailing text; [`Table`](../widgets/data/table.md) adds sorting,
in-place editing and selection; [`TreeView`](../widgets/data/tree-view.md)
expands, renames and accepts drag moves. All three paint only the rows in their
viewport, so thousands of rows cost a viewport per frame.

## Prerequisites

- The window from [03](03-first-window.md) builds and runs.
- Step 4 needs `layout-xml` enabled as in [05](05-xml-dynamic.md).
- No feature flag is needed for the three widgets themselves.

## Steps

### 1. A virtualized list

`ListView` owns its rows, scroll offset, wheel handling, hover/press state and
its integrated scrollbar. Create it empty, then hand it rows with `Ui::set_rows`.

```rust
use bamboo_native_ui::widgets::ListView;
use bamboo_native_ui::{Row, Ui, WidgetId, WindowHandle};

enum Msg {
    PlayRow(usize),
}

fn build_list(ui: &mut Ui<Msg>) {
    ui.add(
        ListView::new("tracks")
            .row_height(44.0)
            .on_activate(Msg::PlayRow),
    );
    let rows = (0..640)
        .map(|index| {
            Row::new(format!("Track {:03}", index + 1))
                .subtitle(format!("Artist {} · Album {}", index % 17, index % 9))
                .trailing(format!("{}:{:02}", 2 + index % 3, (index * 7) % 60))
        })
        .collect();
    ui.set_rows(WidgetId::new("tracks"), rows);
}

fn scroll_to_track(ui: &mut Ui<Msg>, window: &mut WindowHandle<'_>) {
    ui.edit_widget::<ListView<Msg>>(WidgetId::new("tracks"), |list| list.scroll_to_row(320));
    ui.flush_damage(window);
}
```

`Row::new(title)` takes anything that converts to `String`; `.subtitle`,
`.trailing`, `.current(true)` and `.disabled(true)` are optional. `row_height`
has a 24 DIP floor, and `on_activate` fires for a pressed and released row and
for Enter when the list has focus. `scroll_to_row` puts a row at the top edge;
`ensure_row_visible` moves the minimum distance. `edit_widget` records pending
damage, which `flush_damage` turns into one invalidation.

Virtualization is the point: paint, hit-testing and the accessibility tree use
`visible_rows()`, so work is proportional to on-screen rows. A
[`ScrollView`](../api/scrolling.md) around the list leaves the wheel notch to it
because `scrolls_with_wheel()` is `true`.

### 2. A sortable, editable table

A `Table` is a fixed header band plus rows painted from data. Columns are
geometry only: a fixed width, or a flex column that shares the leftover width.
Sorting stays application-owned; the widget never compares values it cannot know.

```rust
use bamboo_native_ui::widgets::{Table, TableColumn, TableRow};
use bamboo_native_ui::{TextAlign, Ui, WidgetId};

enum Msg {
    Select(usize),
    Open(usize),
    Sort(usize),
    Edit(usize, usize, String),
}

fn build_table(ui: &mut Ui<Msg>) {
    let rows = (0..1_000)
        .map(|index| {
            TableRow::new([
                format!("{:04}", index + 1),
                format!("Track {:03}", index + 1),
                format!("{}:{:02}", 2 + index % 3, (index * 11) % 60),
            ])
        })
        .collect::<Vec<_>>();
    ui.add(
        Table::new("library")
            .columns(vec![
                TableColumn::new("#", 56.0).align(TextAlign::Right),
                TableColumn::flex("Title"),
                TableColumn::new("Length", 76.0).align(TextAlign::Right),
            ])
            .rows(rows)
            .sort(1, true)
            .on_select(Msg::Select)
            .on_activate(Msg::Open)
            .on_header_click(Msg::Sort)
            .editable(true)
            .on_cell_edit(Msg::Edit),
    );
}
```

`.sort(1, true)` paints the initial indicator only. A header click emits the
column index; the application sorts, then calls `set_sort` so the indicator
follows. `editable(true)` starts an in-place session on Enter, `F2` or a double
click; the widget applies the committed text before `on_cell_edit` runs, so the
handler only stores it. `on_select` fires after the clicked row is selected;
`on_activate` fires on Enter or a double click. `on_column_resize` and
`on_column_reorder` exist if the layout is persisted.

### 3. An expandable tree

`TreeView` flattens the visible nodes into a cached row list on structural
change — never during a paint. Rows carry a path instead of a label, so messages
are cheap and stay valid while the tree shape is unchanged.

```rust
use bamboo_native_ui::widgets::{TreeItem, TreeView};
use bamboo_native_ui::Ui;

enum Msg {
    Toggle(Vec<usize>, bool),
    Rename(Vec<usize>, String),
    Move(Vec<usize>, Vec<usize>),
}

fn build_tree(ui: &mut Ui<Msg>) {
    ui.add(
        TreeView::new("tree")
            .items(library())
            .row_height(26.0)
            .indent(16.0)
            .renameable(true)
            .draggable(true)
            .on_toggle(|path, expanded| Msg::Toggle(path.to_vec(), expanded))
            .on_rename(|path, label| Msg::Rename(path.to_vec(), label.to_string()))
            .on_move(|from, to| Msg::Move(from.to_vec(), to.to_vec())),
    );
}

fn library() -> Vec<TreeItem> {
    vec![
        TreeItem::new("Library")
            .glyph('♪')
            .secondary("3 albums")
            .expanded(true)
            .child(
                TreeItem::new("Night")
                    .expanded(true)
                    .child(TreeItem::new("Nocturne").glyph('♫'))
                    .child(TreeItem::new("Sunny Day").glyph('♫')),
            )
            .child(TreeItem::new("Favorites").secondary("12").disabled(true)),
    ]
}
```

`TreeItem::new(label)` plus `.child(...)` (or `.children(vec)`) builds the
hierarchy; `.expanded(true)` is the initial state, `.glyph` adds an icon and
`.secondary` a muted right-aligned note. A chevron click, Right/Left or Space
expands and collapses; the widget updates its own flag and reports `on_toggle`.
`renameable(true)` lets `F2` rename: Enter/Tab commit, Escape reverts, an empty
label is ignored, and `on_rename` runs after the new label is applied.

Dragging is on by default. Restrict the allowed drops with
`TreeDragRules::new().reorder(false).nest(true)` from
`bamboo_native_ui::widgets::tree`, and enable `drag_rules(rules)`. The tree
applies a valid move itself and reports it through `on_move`, so the application
only mirrors the new hierarchy in its own model.

### 4. Describe the data in `.buixml`

A document supplies the child list of a widget that declares one. The three
views accept `<item>`, `<column>`/`<row>`/`<cell>` and nested `<node>`. The
widgets must exist in Rust first: none of the three is a registry type.

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <layout id="base" min-width="0" min-height="0">
    <widget id="tracks" x="12" y="12" w="cw-24" h="200"
            row-height="40" alternating="true" empty-text="No tracks">
      <item title="Nocturne" subtitle="Chopin" trailing="3:58" current="true"/>
    </widget>

    <widget id="library" x="12" y="224" w="cw-24" h="200"
            row-height="26" header-height="26" striped="true" editable="true">
      <column title="#" width="56" align="right"/>
      <column title="Title" flex="true"/>
      <column title="Length" width="76" align="right"/>
      <row disabled="true">
        <cell text="01"/>
        <cell text="Nocturne"/>
        <cell text="3:58"/>
      </row>
    </widget>

    <widget id="tree" x="12" y="436" w="cw-24" h="220"
            row-height="26" indent="16" renameable="true">
      <node label="Library" glyph="♪" secondary="3 albums" expanded="true">
        <node label="Night" expanded="true">
          <node label="Nocturne" glyph="♫"/>
          <node label="Sunny Day" glyph="♫"/>
        </node>
        <node label="Favorites" secondary="12" disabled="true"/>
      </node>
    </widget>
  </layout>
</layouts>
```

Apply it exactly as in [05](05-xml-dynamic.md). Child lists are **data, not a
binding**: they replace the widget's model when the document generation changes,
never on a resize. A generation change while an edit session is open skips that
widget (`xml.content.session-open`), so a reload cannot overwrite a half-typed
cell.

## Expected result

- The list scrolls through 640 rows; only the viewport's rows are painted.
- A header click sorts and moves the indicator; a double click edits a cell.
- The tree expands, renames on `F2` and accepts a drag move inside the rules.
- Loading the document replaces each view's rows once; resizing does not.

## Troubleshooting

- **The list is empty** — it was created but never given rows. Call
  `ui.set_rows(WidgetId::new("tracks"), rows)`; there is no rows builder.
- **The header click does not sort** — the widget only reports the column. Sort
  the data and call `Table::set_sort`; `.sort` alone paints an indicator.
- **Inline editing does not start** — `editable(true)` is off, or the cell is
  read-only. Start with Enter, `F2` or a double click.
- **A tree path points at the wrong node after a reload** — paths are index
  chains. Rebuild the application's path mapping when the order changes.

## Next

[09 — Menus, toolbars and status](09-menus-toolbars-status.md): wrap these views
in application chrome and route command ids.
