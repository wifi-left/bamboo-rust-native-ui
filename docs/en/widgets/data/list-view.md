# ListView

> A virtualized single-column list of title/subtitle/trailing rows.

## Introduction

`ListView<M>` paints a vertical list from a `Vec<Row>` data model, one row
height at a time, and only paints, hit-tests and publishes the rows that
intersect the viewport. It owns its scroll offset, wheel handling, row
hover/press feedback and an integrated scrollbar. A row can carry a second
line, right-aligned trailing text, a "current" highlight and a disabled flag.

The widget is a composite control with a keyboard row cursor. Clicking a row
(on release) or pressing Enter reports the row index through `on_activate`;
there is no separate selection callback. Use `ListView` for a queue or result
list; use `Table` when you need columns, and `TreeView` for a hierarchy.

## Usage (Rust)

```rust
use bamboo_native_ui::{Rect, Row, Ui, WidgetId};
use bamboo_native_ui::widgets::ListView;

enum Msg {
    Play(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    ListView::new("queue")
        .row_height(44.0)
        .alternating(true)
        .empty_text("Nothing queued")
        .on_activate(Msg::Play),
);
ui.set_rect(WidgetId::new("queue"), Rect::from_xywh(16.0, 56.0, 320.0, 360.0));
ui.set_rows(
    WidgetId::new("queue"),
    vec![
        Row::new("Nocturne").subtitle("Jay Chou").trailing("3:58").current(true),
        Row::new("Sunny Day").subtitle("Jay Chou").trailing("4:29"),
    ],
);
```

`Row::new` takes the title; `subtitle`, `trailing`, `current` and `disabled`
are builder methods. `Ui::set_rows` is the silent way to replace the model and
keeps the row cursor in range. The `current` row paints the selected fill and
a 3-DIP accent bar; a `disabled` row paints with the disabled text color and
ignores a click. A mouse activation does not move the keyboard row cursor, and
it does not change any `Row::current` flag; the application owns both.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. The `<item>` child list replaces every
row:

```xml
<widget id="queue" x="16" y="56" w="320" h="360"
        row-height="44" alternating="true" empty-text="Nothing queued"
        selected="0">
  <item title="Nocturne" subtitle="Jay Chou" trailing="3:58" current="true"/>
  <item title="Sunny Day" subtitle="Jay Chou" trailing="4:29"/>
  <item title="Rice Field" subtitle="Jay Chou" trailing="3:43" disabled="true"/>
</widget>
```

The shared `selected` attribute (a visible-row index) applies after the child
list and moves the keyboard cursor without reporting a message.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates the list with 44-DIP rows. |
| `.row_height(f32)` | `f32` | `44.0` | Row height in DIPs, clamped to `>= 24`. |
| `.alternating(bool)` | `bool` | `false` | Tints every other row with the panel color. |
| `.empty_text(impl Into<String>)` | text | `""` | Centered message painted while there are no rows. |
| `.on_activate(impl Fn(usize) -> M)` | index handler | `None` | A row was clicked or activated. |
| `.scrollbar_style(ScrollBarStyle)` | style | theme default | Scrollbar thickness, insets and colors. |

Typed access and runtime setters:

| Method | Type | Description |
|---|---|---|
| `.rows()` | `&[Row]` | The current data model. |
| `.selected_row()` | `usize` | Keyboard row cursor (default 0). |
| `.visible_rows()` | `(usize, usize)` | Published viewport range, end exclusive. |
| `.scroll_offset()` | `f32` | Current scroll offset in DIPs. |
| `.scroll_to_row(usize)` | `bool` | Scrolls the row to the top, clamped. |
| `.ensure_row_visible(usize)` | `bool` | Minimal scroll that reveals the row. |
| `.set_row_height(f32)` | `bool` | Runtime row height (`>= 24`), keeping the cursor visible. |
| `.set_alternating(bool)` / `.is_alternating()` | `bool` | Runtime striping. |
| `.set_empty_text(text)` / `.empty_text_ref()` | text | Runtime empty message. |
| `.scrollbar()` | `&ScrollBarStyle` | The current style. |

Behaviour: rows are virtualized, so a thousand-row list costs one viewport of
work. Wheel input scrolls the list by a few rows and the list claims the notch
from an enclosing `ScrollView`. In adjust mode Up/Down move the cursor,
PageUp/PageDown move by a page, Home/End jump, and Enter activates. The
accessibility role is `List`; only viewport rows are published, and the
one-row look-ahead is marked offscreen.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(usize) -> M` passed to `.on_activate` | A row is clicked (on release) or activated with Enter/Activate; the argument is the row index. |

There is no selection callback: moving the keyboard cursor is silent.

### Common observers

`ListView` installs the shared observer builders (via `observer_methods!` /
`observer_builders!`). All are additive: the built-in row handling still runs
after the handler returns, and the handler's message is queued beside it.

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

| Method | Handler | Fires when |
|---|---|---|
| `.on_activate(handler)` | `impl Fn(usize) -> M + 'static` | Press and release on a row (a disabled row is ignored), Enter in adjust mode, or a UIA Invoke; the message is queued after the row was highlighted. |

The callback is message-returning: it builds the application's `M` and runs
after the widget has updated its cursor/press state.

### Ui triggers

Any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("queue"), Trigger::Click, |event, ctx| {
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

It reacts to `Click` and `Activate` (row activation), `Wheel` (scroll) and
`ValueChange` (the selected row changed). `DoubleClick`, `RightClick` and the
drag triggers have no built-in effect; scrollbar dragging uses press/move/release
input instead.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules (one horizontal and one vertical
form). Generic properties (`visible`, `tooltip`, `name`, `description`,
`automation-id`, `role`, `expose`, focus policy, cursor) apply. `font-size` and
`font-family` are paint-level. The list paints from the palette and has no
`SurfaceStyle`, so `radius`, `corners` and `background` are reported as
`xml.style.unsupported` and skipped.

Local attributes:

| Attribute | Kind | Description |
|---|---|---|
| `row-height` | number | Row height in DIPs; must be `>= 24`. |
| `alternating` | bool | Panel tint on alternate rows. |
| `empty-text` | text | Centered message while the list is empty. |

Child elements:

| Element | Attributes | Description |
|---|---|---|
| `<item>` | `title` (required, non-empty), `subtitle`, `trailing`, `current` (bool), `disabled` (bool) | One row; the list replaces every row. |

The child list is content: written only when the document generation changes,
variant-invariant, and silent. Declare every `<item>` when the list changes.

## See also

- `table.md`
- `tree-view.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
