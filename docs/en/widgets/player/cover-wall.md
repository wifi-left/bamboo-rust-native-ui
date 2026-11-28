# CoverWall

> A virtualized, scrollable grid of square album covers with captions.

## Introduction

`CoverWall<M>` is the player's browse surface. It lays out `Cover` tiles in a
vertical grid: covers fill the width at the largest tile width that still fits
the target `.tile(size)`, the actual cell grows to share the leftover space, and
only the visible rows are painted and hit-tested. The wall scrolls with the
shared themed scrollbar and the wheel.

A click selects a cover and reports `on_select`; a double click or Enter opens
it through `on_open`, so a single click never starts playback by accident. The
selection frame is painted inside a 2-DIP band reserved on every tile, so
selecting a cover never resizes the artwork or shifts the grid.

Artwork is an optional page image registered with `Ui::set_image`. An unset or
unknown image paints a hash-tinted gradient tile with the title's first character.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{Cover, CoverWall};
use bamboo_native_ui::{ImageId, Rect, Ui, WidgetId};

enum Msg {
    Select(usize),
    Open(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    CoverWall::new("wall")
        .covers(vec![
            Cover::new("Nocturne").subtitle("Jay Chou").image(ImageId(7)),
            Cover::new("Bamboo").subtitle("Playlist").corner_radius(4.0),
            Cover::new("Sunny Day").subtitle("Jay Chou"),
        ])
        .tile(148.0)
        .gap(14.0)
        .caption_height(42.0)
        .corner_radius(6.0)
        .empty_text("No covers")
        .on_select(Msg::Select)
        .on_open(Msg::Open),
);
ui.set_rect(WidgetId::new("wall"), Rect::from_xywh(16.0, 56.0, 520.0, 320.0));
```

`Cover::image` takes an `ImageId` (the number passed to `Ui::set_image`); there
is no image-name registry. `Cover::corner_radius` overrides the wall default for
one tile, and `Some(0.0)` is a square crop. Replace the data at runtime with
`set_covers` (or `Ui::edit_widget`), which keeps the cursor in range, clears a
hover/press and reveals the selected cover.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. Its child list is `<cover>` and replaces
every cover:

```xml
<widget id="wall" x="16" y="56" w="520" h="320"
        gap="14" corner-radius="6" caption-height="42"
        empty-text="No covers" selected="0">
  <cover title="Nocturne" subtitle="Jay Chou" image="7" corner-radius="4"/>
  <cover title="Bamboo" subtitle="Playlist"/>
  <cover title="Sunny Day" subtitle="Jay Chou"/>
</widget>
```

`title` is required and must be non-empty; `image` is the raw numeric
`ImageId`; `corner-radius` clamps to `>= 0`. The shared `selected` attribute is a
cover index and applies after the child list.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates an empty wall; `520 × 320` DIP. |
| `.covers(Vec<Cover>)` / `.set_covers` | `Vec<Cover>` | empty | Replaces the model and keeps the cursor in range. |
| `.tile(f32)` | `f32` | `132.0` | Target tile width in DIPs, clamped to `>= 48`. |
| `.gap(f32)` | `f32` | `14.0` | Gap between tiles, clamped to `>= 0`. |
| `.caption_height(f32)` | `f32` | `42.0` | Caption band under the artwork, clamped to `>= 0`. |
| `.corner_radius(f32)` | `f32` | `0.0` | Wall-default artwork radius, clamped to `>= 0`. |
| `.empty_text(text)` | text | `""` | Centered message while the wall is empty. |
| `.on_select(handler)` | `Fn(usize) -> M` | none | A click changed the current cover. |
| `.on_open(handler)` | `Fn(usize) -> M` | none | A double click, Enter or `Activate` opened a cover. |
| `.scrollbar_style(ScrollBarStyle)` | style | theme default | Thickness, insets and colors of the bar. |

Typed access and runtime setters:

| Method | Type | Description |
|---|---|---|
| `.covers_ref()` | `&[Cover]` | The current model. |
| `.selected_index()` | `usize` | Selected cover (clamped into range). |
| `.columns()` | `usize` | Columns in the current row (at least one). |
| `.visible_range()` | `(usize, usize)` | Painted viewport range, end exclusive. |
| `.cell_rect(usize)` / `.scroll_offset()` | `Option<Rect>` / `f32` | Tile rectangle and scroll offset. |
| `.scroll_to_index(usize)` | `bool` | Scrolls the index's row to the top (clamped). |
| `.set_gap` / `.set_caption_height` / `.set_empty_text` | `bool` | Runtime locals; return whether they changed. |
| `.gap_value()` / `.corner_radius_value()` / `.caption_height_value()` / `.empty_text_value()` | values | Read the local values. |
| `.scrollbar()` | `&ScrollBarStyle` | The current bar style. |

`Cover` fields are public: `title`, `subtitle`, `image: Option<ImageId>` and
`corner_radius: Option<f32>`. Builders: `new(title)`, `subtitle`, `image`,
`corner_radius`.

### Behaviour

- **Layout.** Columns are computed from the available width, the target tile
  and the gap; a shown scrollbar takes its strip out of the row width. Tile and
  content geometry do not change with selection. The title ellipsizes and the
  subtitle paints only when `caption_height >= 34`.
- **Pointer.** A release over the pressed tile selects it without reveal-
  scrolling; the wheel scrolls about one row per notch and clamps. The
  scrollbar thumb drags and a track press jumps.
- **Keyboard.** Arrows move by tile or row, PageUp/PageDown by a page, Home/End
  jump, and Enter opens. Keyboard selection reveals the row.
- **Accessibility.** Role `List` with the visible covers as children, the
  selected one marked, a one-row look-ahead marked offscreen and a vertical
  scroll state. Invoking a child selects it.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `impl Fn(usize) -> M` passed to `.on_select` | A pointer release over the pressed tile selects it, or a UIA child invoke selects it. Keyboard arrow/Home/End movement changes the selection silently. |
| `impl Fn(usize) -> M` passed to `.on_open` | A double click on a tile, `Enter` on the selected tile or `Activate` opens it. |

Both handlers receive the cover index. Replacing the model with `set_covers`
or setting `selected` from a document emits nothing.

### Common observers

This widget installs none of the shared observer builders. Use `on_select` /
`on_open` or `Ui::bind`.

### Widget-specific callbacks

| Callback | Signature | Fires when |
|---|---|---|
| `.on_select(handler)` | `impl Fn(usize) -> M` | The current cover changed by click or UIA invoke. |
| `.on_open(handler)` | `impl Fn(usize) -> M` | A cover is opened by double click, `Enter` or `Activate`. |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("wall"), Trigger::DoubleClick, |event, ctx| {
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

The wall reacts to `Click`, `DoubleClick`, `HoverEnter`/`HoverLeave`, `Wheel`,
`Activate` and `ValueChange`; a right click has no built-in effect.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties (`visible`,
`tooltip`, `name`, `description`, `automation-id`, `role`, `expose`, focus
policy) apply. Content support is `selected` only. The wall paints from the
palette and has no `SurfaceStyle`, so `radius`, `corners` and `background` are
reported as `xml.style.unsupported`.

Widget-local attributes:

| Attribute | Kind | Description |
|---|---|---|
| `gap` | number `>= 0` | Gap between tiles. |
| `corner-radius` | number `>= 0` | Artwork radius default. |
| `caption-height` | number `>= 0` | Caption band height. |
| `empty-text` | text | Message painted while there are no covers. |

Child elements:

| Element | Attributes | Description |
|---|---|---|
| `<cover>` | `title` (required, non-empty), `subtitle`, `image` (numeric `ImageId`), `corner-radius` | One tile; the list replaces every cover. |

Unknown child attributes are rejected atomically; the previous wall stays
intact.

## See also

- `./playlist-view.md`
- `./waveform-seek-bar.md`
- `../data/list-view.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
