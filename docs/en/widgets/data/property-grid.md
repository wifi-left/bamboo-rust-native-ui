# PropertyGrid

> A sectioned key/value inspector with a typed in-grid editor per row.

## Introduction

`PropertyGrid<M>` paints titled section headers interleaved with key/value
rows and edits the value in place. Each row picks a `PropertyKind`: read-only
text, an inline text or number field, a drop-down choice, a check box or a star
rating. The header is part of the scrolling content.

The widget owns its data (`PropertySection` values containing `PropertyRow`
values) and applies an edit before reporting it through `on_change(key,
value)`, so the application only persists it. `on_activate(key, value)` remains
for read-only rows inspected elsewhere. The grid paints no background.

## Usage (Rust)

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::{PropertyGrid, PropertyRow, PropertySection};

enum Msg { Select(String), Inspect(String, String), Change(String, String) }
let mut ui: Ui<Msg> = Ui::new();
ui.add(
    PropertyGrid::new("props")
        .sections(vec![
            PropertySection::new("Playback")
                .row(PropertyRow::new("Volume", "80").number_with(0.0, 100.0, 5.0, 0))
                .row(PropertyRow::new("Shuffle", "Off").check("On", "Off"))
                .row(PropertyRow::new("Rating", "4").rating(5))
                .row(PropertyRow::new("Quality", "Lossless").choice(["Lossless", "High"]))
                .row(PropertyRow::new("Title", "Night Drive").text()),
            PropertySection::new("File")
                .row(PropertyRow::new("Path", "/Music/Night Drive.flac")),
        ])
        .key_width(120.0)
        .on_select(|key: &str| Msg::Select(key.to_string()))
        .on_activate(|key: &str, value: &str| Msg::Inspect(key.to_string(), value.to_string()))
        .on_change(|key: &str, value: &str| Msg::Change(key.to_string(), value.to_string())),
);
ui.set_rect(WidgetId::new("props"), Rect::from_xywh(16.0, 56.0, 340.0, 280.0));
```

`PropertyRow::new` starts as `ReadOnly`; `text()`, `number()`, `number_with()`,
`choice()`, `check()` and `rating()` select the editor. A row is identified by
its key, so stored values go back through `set_value`, `set_checked` or
`set_rating` (via `ui.edit_widget`).

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. `<section>` elements replace every
section; their `<row>` children select a kind and its parameters:

```xml
<widget id="props" x="16" y="56" w="340" h="280"
        row-height="26" header-height="28" key-width="120"
        grid-lines="true" empty-text="No properties">
  <section title="Playback">
    <row key="Volume" value="80" kind="number" min="0" max="100" step="5" decimals="0"/>
    <row key="Shuffle" value="Off" kind="check" on="On" off="Off"/>
    <row key="Rating" value="4" kind="rating" stars="5"/>
    <row key="Quality" value="Lossless" kind="choice" options="Lossless|High"/>
    <row key="Title" value="Night Drive" kind="text"/>
  </section>
  <section title="File">
    <row key="Path" value="/Music/Night Drive.flac"/>
  </section>
</widget>
```

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates an empty grid. |
| `.sections(Vec<PropertySection>)` | sections | empty | Replaces every section. |
| `.header_height(f32)` / `.row_height(f32)` | `f32` | `28.0` / `26.0` | Header band and row height (row `>= 16`; `0` header removes the band). |
| `.key_width(f32)` | `f32` | `170.0` | Key column width, `>= 40`; paint caps it at 60% of the width. |
| `.grid_lines(bool)` / `.empty_text(impl Into<String>)` | `bool` / text | `true` / `""` | Row separators; centered message while empty. |
| `.read_only(bool)` | `bool` | `false` | Locks every editor; navigation still works. |
| `.on_select(impl Fn(&str) -> M)` / `.on_activate(impl Fn(&str, &str) -> M)` | key handlers | `None` | A row became the cursor; a read-only row was activated. |
| `.on_change(impl Fn(&str, &str) -> M)` | key/value handler | `None` | The grid committed an edit (value already applied). |
| `.scrollbar_style(ScrollBarStyle)` | style | theme default | Scrollbar look. |

`PropertyRow`: `new(key, value)` (`ReadOnly`), `.text()`,
`.number(min, max)` (step 1, 0 decimals), `.number_with(min, max, step,
decimals)`, `.choice(iterable)`, `.check(on, off)`, `.rating(stars)` (clamped
to `1..=10`) and `.read_only(bool)`; accessors `key()`, `value()`, `kind()`,
`is_editable()`, `is_read_only()`, `set_value()`, `set_key()`.
`PropertySection`: `new(title)`, `.row(...)`, `.rows(...)`, `title()`,
`rows_ref()`, `row_count()`. `PropertyKind` is `ReadOnly`, `Text`,
`Number { min, max, step, decimals }`, `Choice(Vec<String>)`,
`Check { on, off }` or `Rating { stars }`.

Typed access and runtime setters:

| Method | Type | Description |
|---|---|---|
| `.sections_ref()` / `.section(usize)` / `.row_count()` | mixed | Data model. |
| `.row_by_key(key)` / `.value(key)` | mixed | First row with a key, or its value. |
| `.set_value(key, value)` / `.set_key(key, new_key)` | `bool` | Replace a value or key silently. |
| `.checked(key)` / `.set_checked(key, bool)` / `.rating(key)` / `.set_rating(key, u8)` | mixed | Check and rating helpers. |
| `.set_row_read_only(section, row, bool)` | `bool` | Locks one row. |
| `.selected_path()` / `.selected_row()` / `.selected_key()` / `.select_key(key)` | mixed | Cursor. |
| `.ensure_key_visible(key)` / `.scroll_to_key(key)` | `bool` | Reveal a row. |
| `.row_rect(section, row)` / `.value_rect(section, row)` | `Option<Rect>` | Client rectangles. |
| `.set_sections`, `.set_section_rows`, `.set_row_height`, `.set_header_height`, `.set_key_width`, `.set_grid_lines`, `.set_empty_text` | mixed | Runtime setters. |
| `.is_read_only()` / `.scroll_offset()` | mixed | State. |

Behaviour: a text or number row opens an inline field on click or Enter/Space;
Enter/Tab commits and Escape reverts. A number draft accepts only digits, `.`
and `-`, clamps to `min..=max` on commit, formats with `decimals` places and
steps by `step` with Up/Down. A choice opens an overlay list (arrows and
Home/End move, Enter/Space commits, Escape closes); a check toggles between its
two value strings; a rating sets the clicked star or increments from the
keyboard. `on_change` fires after the model holds the text; a read-only kind
(or locked row) is inspected through `on_activate`.

## Events

### Messages

All three callbacks are message-returning builder closures (`Fn(&str, ...) ->
M`); `new` takes only the id.

| Message | Emitted when |
|---|---|
| `Fn(&str) -> M` passed to `.on_select` | A row became the cursor (click or keyboard); the argument is the row key. |
| `Fn(&str, &str) -> M` passed to `.on_activate` | A read-only row was activated (double click, Enter or an assistive invoke). |
| `Fn(&str, &str) -> M` passed to `.on_change` | The grid committed an edit; `(key, value)` after the model was updated. |

### Common observers

This widget does not install the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`). The callbacks below are its event seam;
anything else goes through `Ui::bind`.

### Widget-specific callbacks

| Method | Handler | Fires when |
|---|---|---|
| `.on_select(handler)` | `impl Fn(&str) -> M + 'static` | The cursor moved to a row, by pointer or keyboard. |
| `.on_activate(handler)` | `impl Fn(&str, &str) -> M + 'static` | A `ReadOnly` row (or a locked row) was activated; editable rows edit themselves instead. |
| `.on_change(handler)` | `impl Fn(&str, &str) -> M + 'static` | A text/number field committed, a choice was picked, a check box toggled or a rating changed; the value is already applied. |

Escape cancels an editor and emits nothing; `set_value` and the other silent
setters never fire `on_change`.

### Ui triggers

Any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("props"), Trigger::ValueChange, |event, ctx| {
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

It reacts to `Click` (select or open an editor), `DoubleClick`/`Activate`
(activate), `Wheel` (scroll) and `ValueChange` (cursor or checked/rating state).
`RightClick` and the drag triggers have no built-in effect.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties apply,
including `read-only` (locks editors) and `name`. `font-size`/`font-family`
are paint-level. The grid paints from the palette and has no `SurfaceStyle`,
so `radius`, `corners` and `background` report `xml.style.unsupported`.

| Attribute | Kind | Description |
|---|---|---|
| `row-height` | number | Row height; must be `>= 16`. |
| `header-height` | number | Section header height; `>= 0`. |
| `key-width` | number | Key column width; must be `>= 40`. |
| `grid-lines` | bool | Row separators. |
| `empty-text` | text | Message while there are no sections. |

Child elements:

| Element | Attributes | Description |
|---|---|---|
| `<section>` | `title` | A titled group. |
| `<row>` | `key` (required, non-empty), `value`, `kind`, `read-only` | One row. |

`kind` is `read-only` (default), `text`, `number`, `choice`, `check` or
`rating`. Kind-specific attributes: `min`/`max`/`step`/`decimals` (number);
`options` (`|`-separated, non-empty) (choice); `on`/`off` (check); `stars`
(integer `1..=10`) (rating). An attribute outside the selected kind is an
error. The child list is content: written on a document generation change,
variant-invariant and silent; there is no shared `selected`.

## See also

- `table.md`
- `tree-view.md`
- `list-view.md`
- `../../guides/xml-layout-authoring.md`
