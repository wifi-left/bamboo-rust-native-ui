# PlaylistView

> A virtualized, reorderable track list: select, play and drag to reorder.

## Introduction

`PlaylistView<M>` paints a vertical list from `Vec<Row>`, reusing the shared row
model: title (track), subtitle (artist), trailing (duration), `current` (the
playing track) and `disabled`. Only viewport rows are painted, hit-tested and
published, and the list owns its scroll offset, wheel handling and themed
scrollbar.

A click selects and reports `on_select`; a double click or Enter plays through
`on_activate`. Pressing a row and dragging it moves the track: the widget paints
an accent insertion line, auto-scrolls near the viewport edges and reports the
move through `on_reorder`. The application owns the data and applies the move
itself; the widget never reorders its copy, so the model and the widget cannot
drift. `Alt+Up/Down` is the keyboard equivalent.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::PlaylistView;
use bamboo_native_ui::{Rect, Row, Ui, WidgetId};

enum Msg {
    Select(usize),
    Play(usize),
    Reorder(usize, usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    PlaylistView::new("queue")
        .rows(vec![
            Row::new("Nocturne").subtitle("Jay Chou").trailing("3:58").current(true),
            Row::new("Sunny Day").subtitle("Jay Chou").trailing("4:29"),
            Row::new("Rice Field").subtitle("Jay Chou").trailing("3:43"),
        ])
        .row_height(40.0)
        .alternating(true)
        .empty_text("The queue is empty")
        .on_select(Msg::Select)
        .on_activate(Msg::Play)
        .on_reorder(Msg::Reorder),
);
ui.set_rect(WidgetId::new("queue"), Rect::from_xywh(552.0, 56.0, 320.0, 360.0));
```

`on_reorder(from, to)` reports `to` as the final index **after** removing
`from`, so the application applies it as
`let track = rows.remove(from); rows.insert(to, track);` and pushes the result
back with `set_rows`. A drop on the same slot emits nothing.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. Its child list is `<track>` and replaces
every row:

```xml
<widget id="queue" x="552" y="56" w="320" h="360"
        row-height="40" alternating="true" empty-text="The queue is empty"
        selected="0">
  <track title="Nocturne" subtitle="Jay Chou" trailing="3:58" current="true"/>
  <track title="Sunny Day" subtitle="Jay Chou" trailing="4:29"/>
  <track title="Rice Field" subtitle="Jay Chou" trailing="3:43" disabled="true"/>
</widget>
```

`title` is required and must be non-empty. `current` and `disabled` are bool
attributes. The shared `selected` attribute applies after the child list and
moves the cursor without reporting a message.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates an empty list; `320 × 360` DIP. |
| `.rows(Vec<Row>)` / `.set_rows` | `Vec<Row>` | empty | Replaces the tracks, clamps the cursor and clears a drag. |
| `.row_height(f32)` | `f32` | `44.0` | Row height in DIPs, clamped to `>= 24`. |
| `.alternating(bool)` | `bool` | `false` | Tints every other row with the panel color. |
| `.empty_text(text)` | text | `""` | Centered message while there are no tracks. |
| `.on_select(handler)` | `Fn(usize) -> M` | none | A click changed the current track. |
| `.on_activate(handler)` | `Fn(usize) -> M` | none | A double click, Enter or `Activate` plays a track. |
| `.on_reorder(handler)` | `Fn(usize, usize) -> M` | none | A track was moved; `to` is post-removal. |
| `.scrollbar_style(ScrollBarStyle)` | style | theme default | Bar thickness, insets and colors. |

Typed access and runtime setters:

| Method | Type | Description |
|---|---|---|
| `.rows_ref()` | `&[Row]` | The current model. |
| `.selected_index()` | `usize` | Cursor index (`0` when empty). |
| `.visible_range()` | `(usize, usize)` | Published viewport range, end exclusive. |
| `.row_rect(usize)` / `.scroll_offset()` | `Option<Rect>` / `f32` | Row rectangle and scroll offset. |
| `.drag_state()` | `Option<(usize, usize)>` | `(from, insertion slot)` while a drag is in flight. |
| `.scroll_to_row(usize)` / `.ensure_row_visible(usize)` | `bool` | Top-align or minimally reveal a row. |
| `.set_row_height(f32)` | `bool` | Runtime row height (`>= 24`), keeping the cursor visible. |
| `.set_alternating(bool)` / `.is_alternating()` | `bool` | Runtime striping. |
| `.set_empty_text(text)` / `.empty_text_ref()` | text | Runtime empty message. |
| `.scrollbar()` | `&ScrollBarStyle` | The current bar style. |

### Behaviour

- **Pointer.** A release over the pressed, enabled row selects it without
  reveal-scrolling. Pressing then dragging begins a reorder; holding the
  pointer within 24 DIP of the top/bottom edge scrolls by half a row per move.
  The insertion slot is the nearest row gap, clamped to the ends.
- **Keyboard.** Up/Down, PageUp/PageDown, Home/End move the cursor and reveal
  it; Enter plays; `Alt+Up/Down` moves the selected track and reports
  `on_reorder`. Reordering does not wrap at the ends.
- **Rows.** A `current` row paints the selected fill and a 3-DIP accent bar; a
  `disabled` row paints dimmed and cannot be selected, played or dragged. The
  trailing text is right-aligned in a 64-DIP zone.
- **Accessibility.** Role `List` with the visible rows, the selected one marked,
  a one-row look-ahead marked offscreen and a vertical scroll state. Invoking a
  child selects and reveals it.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `impl Fn(usize) -> M` passed to `.on_select` | A pointer release over the pressed, enabled row, a keyboard cursor move (arrows, PageUp/PageDown, Home/End) or a UIA child invoke selects a row. |
| `impl Fn(usize) -> M` passed to `.on_activate` | A double click on an enabled row, `Enter` on the selected row or `Activate` plays it. |
| `impl Fn(usize, usize) -> M` passed to `.on_reorder` | A drag is released on a different slot, or `Alt+Up`/`Alt+Down` moves the selected row; `to` is the index after removing `from`. |

Setting `rows`, `row_height` or `selected` from a document, and a drag dropped
on its own slot, emit nothing.

### Common observers

This widget installs none of the shared observer builders. Use the three
callbacks above or `Ui::bind`.

### Widget-specific callbacks

| Callback | Signature | Fires when |
|---|---|---|
| `.on_select(handler)` | `impl Fn(usize) -> M` | The current row changed by click, keyboard or UIA invoke. |
| `.on_activate(handler)` | `impl Fn(usize) -> M` | An enabled row is played by double click, `Enter` or `Activate`. |
| `.on_reorder(handler)` | `impl Fn(usize, usize) -> M` | A drag drop or `Alt+Up`/`Alt+Down` moves a row. |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("queue"), Trigger::DragEnd, |event, ctx| {
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

The list reacts to `Click`, `DoubleClick`, `HoverEnter`/`HoverLeave`, `Wheel`,
`Activate`, `DragStart`/`DragEnd` and `ValueChange`; a right click has no
built-in effect.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties (`visible`,
`tooltip`, `name`, `description`, `automation-id`, `role`, `expose`, focus
policy) apply. Content support is `selected` only. The list paints from the
palette and has no `SurfaceStyle`, so `radius`, `corners` and `background` are
reported as `xml.style.unsupported`.

Widget-local attributes:

| Attribute | Kind | Description |
|---|---|---|
| `row-height` | number `>= 24` | Row height in DIPs. |
| `alternating` | bool | Panel tint on alternate rows. |
| `empty-text` | text | Centered message while the list is empty. |

Child elements:

| Element | Attributes | Description |
|---|---|---|
| `<track>` | `title` (required, non-empty), `subtitle`, `trailing`, `current` (bool), `disabled` (bool) | One track; the list replaces every row. |

## See also

- `./cover-wall.md`
- `./playback-transport.md`
- `../data/list-view.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
