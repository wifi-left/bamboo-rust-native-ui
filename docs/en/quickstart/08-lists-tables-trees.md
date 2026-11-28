# 08 — Lists, tables and trees

> Fill a page with three virtualized data views: a 640-row list, a sortable and
> editable table, and an expandable tree — in Rust and as `.buixml` child lists.

## What you will build

A track browser: [`ListView`](../widgets/data/list-view.md) is a flat list with a
subtitle and trailing text; [`Table`](../widgets/data/table.md) adds sorting,
in-place editing and selection; [`TreeView`](../widgets/data/tree-view.md)
expands, renames and accepts drag moves. All three paint only the rows in their
viewport, so thousands of rows cost a viewport per frame.

**New terms on this page**

- **Virtualization** — the widget holds every row, but paints, hit-tests and
  describes only the rows inside the visible rectangle. Per-frame cost follows
  the viewport, not the row count. This is why 640 or 1,000 rows feel the same
  as ten.
- **Viewport** — the visible window onto the scrollable content. Scrolling
  changes which rows sit inside it; the rows themselves are not rebuilt.
- **Row** — one line of list data. `Row::new(title)` can carry a subtitle and
  trailing text.
- **Column** — table geometry only: a fixed width, or flex to share the leftover
  width. A column does not know how to compare its values.
- **Cell** — one value at a row/column intersection; cells are what editing
  changes.
- **Selection** — which row is the keyboard cursor (`selected_row`).
- **Activation** — the "do the thing" signal: `on_activate` fires when a row is
  clicked or activated with Enter. Selection and activation are separate
  concepts, even when one gesture produces both.
- **Model** — the data the widget owns (its rows or tree items). You replace it
  with `Ui::set_rows`, `.items(...)` or an XML child list.
- **Data child elements** — XML elements such as `<item>`, `<column>`, `<row>`,
  `<cell>` and `<node>` that supply a widget's model from the document.

## Prerequisites

- The window from [03](03-first-window.md) builds and runs.
- Step 4 needs `layout-xml` enabled as in [05](05-xml-dynamic.md).
- No feature flag is needed for the three widgets themselves.

Everything here goes into the crate folder `bamboo-hello/bamboo-hello/`: the
Rust snippets extend `src/main.rs`, and the step-4 XML is a new file under
`layouts/` next to `src` (for example `layouts/views.buixml`). Run `cargo run`
from that crate folder or from the workspace folder one level up.

## Steps

### 1. A virtualized list

`ListView` owns its rows, scroll offset, wheel handling, hover/press state and
its integrated scrollbar. Create it empty, then hand it rows with `Ui::set_rows`.

All three builders on this page are functions you call once from `App::new`.
Without the call the view does not exist; without a rectangle (from `resize`
or the step-4 document) it keeps its constructor rectangle and may overlap the
page or sit where you did not intend.

```rust
use bamboo_native_ui::widgets::ListView;
use bamboo_native_ui::{Row, Ui, WidgetId, WindowHandle};

enum Msg {
    PlayRow(usize),
}

fn build_list(ui: &mut Ui<Msg>) {
    ui.add(
        ListView::new("tracks")
            // Fixed row height: the first visible row is scroll / height.
            .row_height(44.0)
            // Called for a clicked row and for Enter on the keyboard cursor.
            .on_activate(Msg::PlayRow),
    );
    // The model holds all 640 rows; the widget paints only the viewport.
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
    // edit_widget reaches the concrete ListView by id.
    ui.edit_widget::<ListView<Msg>>(WidgetId::new("tracks"), |list| list.scroll_to_row(320));
    // The scroll changed damage; turn it into one invalidation.
    ui.flush_damage(window);
}
```

`Row::new(title)` takes anything that converts to `String`; `.subtitle`,
`.trailing`, `.current(true)` and `.disabled(true)` are optional. `row_height`
has a 24 DIP floor, and `on_activate` fires for a pressed and released row and
for Enter when the list has focus. `scroll_to_row` puts a row at the top edge;
`ensure_row_visible` moves the minimum distance. `edit_widget` records pending
damage, which `flush_damage` turns into one invalidation.

`selected_row` is the keyboard cursor, moved by Up/Down, PageUp/PageDown,
Home/End. `on_activate` is the action signal and carries the activated row
index; the application decides what "play row 320" means. A click activates
the row under the pointer without needing a prior selection.

**What just happened.** The model has 640 rows, but the widget only touches the
handful that intersect its viewport (plus at most one partially visible row).
Scrolling changes an offset and paints the rows that moved into the viewport;
the rest stay data. `scroll_to_row(320)` jumps deep into the 640 rows, and
`flush_damage` schedules exactly one repaint.

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
    // 1,000 rows: the table virtualizes them like the list does.
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
                // Fixed width, right-aligned numbers.
                TableColumn::new("#", 56.0).align(TextAlign::Right),
                // Flex: takes the width the fixed columns leave.
                TableColumn::flex("Title"),
                TableColumn::new("Length", 76.0).align(TextAlign::Right),
            ])
            .rows(rows)
            // Initial arrow only; the widget does not sort anything.
            .sort(1, true)
            .on_select(Msg::Select)
            .on_activate(Msg::Open)
            .on_header_click(Msg::Sort)
            // Enter, F2 or a double click opens an in-place editor.
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

Columns carry geometry, not meaning: `TableColumn::flex("Title")` has no
comparator, so the widget cannot sort. A header click hands you the index, your
code sorts its own `Vec<TableRow>`, calls `set_rows`, and tells the widget which
arrow to paint with `set_sort(Some((column, ascending)))`. The same split
applies to editing: the committed string is stored in the widget and reported;
persisting it in your model is your job.

**What just happened.** The table fills its header band from the three column
definitions and paints only the rows in view. Clicking a row selects it and
reports the index; double-clicking (or Enter) activates it; an edit session
reports the new text after it has been applied locally.

### 3. An expandable tree

`TreeView` flattens the visible nodes into a cached row list on structural
change — never during a paint. Rows carry a path instead of a label, so messages
are cheap and stay valid while the tree shape is unchanged.

```rust
use bamboo_native_ui::widgets::{TreeItem, TreeView};
use bamboo_native_ui::Ui;

enum Msg {
    // Paths are index chains from the root, not labels: cheap and stable
    // while the tree shape is unchanged.
    Toggle(Vec<usize>, bool),
    Rename(Vec<usize>, String),
    Move(Vec<usize>, Vec<usize>),
}

fn build_tree(ui: &mut Ui<Msg>) {
    ui.add(
        TreeView::new("tree")
            .items(library())
            .row_height(26.0)
            // Space per nesting level, in DIPs.
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
            // Initial state only; the widget owns the flag afterwards.
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

A path such as `[0, 1]` means "the second child of the first root". It stays
valid while the structure is unchanged, which is why the callbacks hand you
indices instead of strings. Renaming does not change a path; moving nodes does.

Dragging is on by default. Restrict the allowed drops with
`TreeDragRules::new().reorder(false).nest(true)` from
`bamboo_native_ui::widgets::tree`, and enable `drag_rules(rules)`. The tree
applies a valid move itself and reports it through `on_move`, so the application
only mirrors the new hierarchy in its own model.

**What just happened.** The tree flattened the initial hierarchy into a row
list once, painted the visible rows, and now mutates its own expansion state on
input. `on_toggle`, `on_rename` and `on_move` tell the application what changed,
so it can update its model without driving the widget's repaint.

### 4. Describe the data in `.buixml`

A document supplies the child list of a widget that declares one. The three
views accept `<item>`, `<column>`/`<row>`/`<cell>` and nested `<node>`. The
widgets must exist in Rust first: none of the three is a registry type.

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <layout id="base" min-width="0" min-height="0">
    <!-- The id must already exist in Rust; <item> supplies the rows. -->
    <widget id="tracks" x="12" y="12" w="cw-24" h="200"
            row-height="40" alternating="true" empty-text="No tracks">
      <item title="Nocturne" subtitle="Chopin" trailing="3:58" current="true"/>
    </widget>

    <!-- <column> children define geometry; <row>/<cell> supply the data. -->
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

    <!-- Nested <node> elements mirror the TreeItem hierarchy. -->
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

Save this as `layouts/views.buixml`, next to `main.buixml`, and load it with the
same pattern from [05](05-xml-dynamic.md)
(`LayoutDocument::parse(include_str!("../layouts/views.buixml"))`) before calling
`apply_layout`. Child lists are **data, not a binding**: they replace the
widget's model when the document generation changes, never on a resize. A
generation change while an edit session is open skips that widget
(`xml.content.session-open`), so a reload cannot overwrite a half-typed cell.

None of the three views is a standard registry type, so the document cannot
create them: each `id` needs a matching `ListView`/`Table`/`TreeView` in Rust.
The document still supplies their rectangles and the appearance attributes
shown; what it cannot do is create the widget objects themselves.

**What just happened.** The document applied one variant, then replaced each
view's model: one `<item>` became one list row, three `<column>` plus one
`<row>` of `<cell>` became the table's header and data, and the nested
`<node>` tree became the tree's hierarchy. Resizing the window keeps the
models; only a generation change (a different document) reloads them.

## Expected result

- The list scrolls through 640 rows; only the viewport's rows are painted.
- A header click sorts and moves the indicator; a double click edits a cell.
- The tree expands, renames on `F2` and accepts a drag move inside the rules.
- Loading the document replaces each view's rows once; resizing does not.

**Run it.** From the workspace folder `bamboo-hello/` or the crate folder
`bamboo-hello/bamboo-hello/`:

```powershell
cargo run
```

Three views appear stacked in the content area: the list, the table and the
tree. Scroll the list and watch the scrollbar thumb move while the window keeps
repainting only the viewport. Click a table header: the arrow changes after your
handler sorts. Press `F2` in the tree and type a new name.

## Troubleshooting

- **`LayoutDocument` does not resolve** — step 4 needs the `layout-xml` feature;
  the three widgets themselves compile without it.
- **The list is empty** — it was created but never given rows. Call
  `ui.set_rows(WidgetId::new("tracks"), rows)`; there is no rows builder. The id
  must match the widget that was added, and `set_rows` on a missing id does
  nothing and reports nothing. An XML `<item>` list also fails to show if its
  `<widget>` id does not exist.
- **The header click does not sort** — the widget only reports the column. Sort
  the data, install the new row order with `set_rows` (through `edit_widget`),
  and call `set_sort` on the same table so the arrow follows. `.sort` alone
  paints an indicator.
- **Inline editing does not start** — `editable(true)` is off, or the cell is
  read-only. Start with Enter, `F2` or a double click. With XML, `editable="true"`
  must be on the `<widget>`, not on the `<row>`.
- **Edits do not survive a reload** — the widget stores and reports the
  committed text; persist it in your model from `on_cell_edit`. Without that,
  the next document generation or a rebuilt row list restores the old text.
- **A tree path points at the wrong node after a reload** — paths are index
  chains. Rebuild the application's path mapping when the order changes.
- **A tree drag does nothing** — the drop is outside the configured
  `TreeDragRules` (for example a reorder when only nesting is allowed), or the
  target node is disabled. The tree validates and applies nothing, so no message
  arrives.
- **Only part of a view is visible, or a view overlaps another** — the
  rectangles come from either `set_rect` or the document. Whichever ran last
  wins; if `apply_layout` failed, the widgets keep the rectangles they had
  before, which may be their constructor defaults. Check the first
  `layout rejected:` line.
- **The XML data does not appear** — the document was parsed but the apply was
  rejected, so no child list was installed. Read the first `layout rejected:`
  line: `xml.syntax` means malformed XML, `xml.id.unknown-widget` an id that no
  Rust widget uses.

## Next

[09 — Menus, toolbars and status](09-menus-toolbars-status.md): wrap these views
in application chrome and route command ids.
