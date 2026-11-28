# TreeView

> A virtualized, expandable hierarchy with rename and drag-and-drop moves.

## Introduction

`TreeView<M>` flattens an expandable `TreeItem` hierarchy into visible rows and
paints only those intersecting its viewport. A row can show an expand/collapse
chevron, an optional leading glyph, its label and muted secondary text. Branch
rows are flattened on structural changes, not on paint, so a deep tree still
costs one viewport of work per frame.

Messages carry the node's **path** (`&[usize]`), not its label: `[0, 2, 1]` is
the second child of the third child of the first root. A path stays valid
across a rebuild while the node exists; selection, activation, toggling,
renaming and drag moves all report paths.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::tree::TreeDragRules;
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::{TreeItem, TreeView};

enum Msg {
    Select(Vec<usize>),
    Open(Vec<usize>),
    Toggle(Vec<usize>, bool),
    Rename(Vec<usize>, String),
    Move(Vec<usize>, Vec<usize>),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    TreeView::new("library")
        .items(vec![TreeItem::new("Music").expanded(true).child(
            TreeItem::new("Albums")
                .glyph('\u{E8FD}')
                .expanded(true)
                .child(TreeItem::new("Nocturne").glyph('\u{E8D6}').secondary("3:32"))
                .child(TreeItem::new("Bamboo").glyph('\u{E8D6}').secondary("4:05")),
        )])
        .row_height(28.0)
        .indent(18.0)
        .renameable(true)
        .drag_rules(TreeDragRules::new().nest(false))
        .on_select(|path: &[usize]| Msg::Select(path.to_vec()))
        .on_activate(|path: &[usize]| Msg::Open(path.to_vec()))
        .on_toggle(|path: &[usize], open| Msg::Toggle(path.to_vec(), open))
        .on_rename(|path: &[usize], label| Msg::Rename(path.to_vec(), label.to_string()))
        .on_move(|from: &[usize], to: &[usize]| Msg::Move(from.to_vec(), to.to_vec())),
);
ui.set_rect(WidgetId::new("library"), Rect::from_xywh(16.0, 56.0, 320.0, 280.0));
```

`TreeItem::child` appends one subtree and `children` replaces the list; a moved
node is already in place when `on_move` runs, and a committed rename is already
applied when `on_rename` runs, so callbacks only persist.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. Nested `<node>` elements replace the
hierarchy:

```xml
<widget id="library" x="16" y="56" w="320" h="280"
        row-height="28" indent="18" renameable="true" draggable="true"
        selected="1">
  <node label="Music" glyph="&#xE8FD;" expanded="true">
    <node label="Albums" expanded="true">
      <node label="Nocturne" glyph="&#xE8D6;" secondary="3:32"/>
      <node label="Bamboo" glyph="&#xE8D6;" secondary="4:05"/>
    </node>
    <node label="Playlists" disabled="true"/>
  </node>
</widget>
```

The shared `selected` attribute is a **visible-row index** (not a path) and
applies after the child list; it only moves the cursor.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates an empty tree with 28-DIP rows. |
| `.items(Vec<TreeItem>)` | items | empty | Replaces the hierarchy (silent). |
| `.row_height(f32)` | `f32` | `28.0` | Row height, clamped to `>= 16`. |
| `.indent(f32)` | `f32` | `18.0` | Indentation per depth level, clamped to `>= 8`. |
| `.chevrons(collapsed, expanded)` | `char, char` | MDL2 `U+E76C`, `U+E70D` | Chevron marks. |
| `.empty_text(impl Into<String>)` | text | `""` | Message when no row is visible. |
| `.renameable(bool)` | `bool` | `false` | Enables F2 in-place label editing. |
| `.draggable(bool)` | `bool` | `true` | Enables press-drag moves. |
| `.drag_rules(TreeDragRules)` | rules | all allowed | Restricts the drop kinds. |
| `.on_select(impl Fn(&[usize]) -> M)` | path handler | `None` | A row was selected. |
| `.on_activate(impl Fn(&[usize]) -> M)` | path handler | `None` | Enter or a double click on a row. |
| `.on_toggle(impl Fn(&[usize], bool) -> M)` | path, expanded | `None` | A node was expanded or collapsed. |
| `.on_rename(impl Fn(&[usize], &str) -> M)` | path, label | `None` | A label was committed in place. |
| `.on_move(impl Fn(&[usize], &[usize]) -> M)` | from, to | `None` | A drag moved a node (already applied). |
| `.on_drag_start(impl Fn(&[usize]) -> bool)` | path | `None` | Veto this gesture; `false` keeps the press a click. |
| `.on_drag_over` / `.on_drag_end` | `impl Fn(&[usize], &[usize]) -> bool` | `None` | Filter a live target or cancel the release. |
| `.scrollbar_style(ScrollBarStyle)` | style | theme default | Scrollbar look. |

`TreeDragRules::new()` allows every drop kind; `.reorder(bool)`, `.nest(bool)`,
`.detach(bool)` and `.reparent(bool)` restrict them, and `.allows(TreeDropKind)`
queries the policy (`Reorder`, `Nest`, `Detach` or `Reparent`).

Typed access and runtime setters:

| Method | Type | Description |
|---|---|---|
| `.items_ref()` / `.item(path)` / `.visible_row_count()` / `.visible_rows()` | mixed | Hierarchy, one node, flattened count, viewport range. |
| `.is_expanded(path)` / `.set_expanded(path, bool)` | `bool` | Read or change expansion silently. |
| `.selected_path()` / `.set_selection_path(path)` / `.selected_row()` | mixed | Keyboard cursor by path or row index. |
| `.row_label(row)` / `.row_rect(row)` | `Option<&str>` / `Rect` | Label and client rectangle of a visible row. |
| `.rename(path, label)` / `.begin_rename(path)` / `.is_renaming()` | mixed | Rename without a session or open one. |
| `.move_item(from, parent, index)` | `bool` | Moves a node without a pointer. |
| `.set_items`, `.set_row_height`, `.set_indent`, `.set_renameable`, `.set_draggable`, `.set_drag_rules`, `.set_empty_text` | mixed | Runtime setters. |
| `.scroll_to_row`, `.ensure_row_visible`, `.scroll_offset` | mixed | Scrolling. |

Behaviour: Up/Down move the cursor and skip disabled rows; PageUp/PageDown move
by a page; Home/End jump to an edge. Right expands or enters a branch; Left
collapses or walks to the parent. Enter activates, Space toggles, F2 renames
(Enter/Tab commits, Escape reverts). A press-drag moves after 6 DIPs: the
top/bottom quarter inserts before/after, the middle nests, leaving the
viewport auto-scrolls. The role is `Tree`.

## Events

### Messages

Five callbacks build the application's `M`; the three drag hooks return `bool`
and veto instead of emitting. All are set on the builder; `new` takes only the
id.

| Message | Emitted when |
|---|---|
| `Fn(&[usize]) -> M` passed to `.on_select` | A row became the cursor (click or keyboard). |
| `Fn(&[usize]) -> M` passed to `.on_activate` | Enter or a double click on the selected/clicked node. |
| `Fn(&[usize], bool) -> M` passed to `.on_toggle` | A node expanded (`true`) or collapsed (`false`). |
| `Fn(&[usize], &str) -> M` passed to `.on_rename` | An in-place label edit committed (already applied). |
| `Fn(&[usize], &[usize]) -> M` passed to `.on_move` | A drag moved a node (already applied). |

### Common observers

This widget does not install the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`). The callbacks below are its event seam;
anything else goes through `Ui::bind`.

### Widget-specific callbacks

Paths are `&[usize]` from the root, e.g. `[0, 2, 1]`; every handler is
message-returning except the drag vetoes.

| Method | Handler | Fires when |
|---|---|---|
| `.on_select(handler)` | `impl Fn(&[usize]) -> M + 'static` | The cursor moved to a row, by pointer or keyboard. |
| `.on_activate(handler)` | `impl Fn(&[usize]) -> M + 'static` | Enter, a UIA invoke, or a double click on a row. |
| `.on_toggle(handler)` | `impl Fn(&[usize], bool) -> M + 'static` | Chevron click, Left/Right, Space or a UIA expand request changed a node. |
| `.on_rename(handler)` | `impl Fn(&[usize], &str) -> M + 'static` | An F2 session committed; the widget applied the label first. |
| `.on_move(handler)` | `impl Fn(&[usize], &[usize]) -> M + 'static` | A drag drop was applied (`from`, `to`). |
| `.on_drag_start(handler)` | `impl Fn(&[usize]) -> bool + 'static` | The pointer crossed the drag threshold; `false` keeps the press a click. |
| `.on_drag_over(handler)` | `impl Fn(&[usize], &[usize]) -> bool + 'static` | The prospective drop target changed; `false` marks it invalid. |
| `.on_drag_end(handler)` | `impl Fn(&[usize], &[usize]) -> bool + 'static` | Release on a valid target, before the tree changes; `false` cancels the drop. |

### Ui triggers

Any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("library"), Trigger::DragEnd, |event, ctx| {
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

It reacts to `Click` (select), `DoubleClick`/`Activate` (activate), `Wheel`
(scroll) and `ValueChange` (selection). Moves respond to press/move/release
input rather than to the drag triggers, and `RightClick` has no built-in effect.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties apply,
including `selected` (visible-row index) and `read-only` (locks renaming).
`font-size`/`font-family` are paint-level. The tree paints from the palette and
has no `SurfaceStyle`, so `radius`, `corners` and `background` are reported as
`xml.style.unsupported` and skipped.

| Attribute | Kind | Description |
|---|---|---|
| `row-height` | number | Row height; must be `>= 16`. |
| `indent` | number | Indentation per level; must be `>= 8`. |
| `renameable` | bool | Enables F2 renaming. |
| `draggable` | bool | Enables press-drag moves. |
| `empty-text` | text | Message when the tree has no visible rows. |

Child elements:

| Element | Attributes | Description |
|---|---|---|
| `<node>` | `label` (required, non-empty), `glyph` (one character), `secondary`, `expanded` (bool), `disabled` (bool) | One node; `<node>` children nest recursively. |

The child list is content (written on a generation change); callbacks stay in Rust.

## See also

- `table.md`
- `list-view.md`
- `../index.md`
- `../../api/widget-trait.md`
