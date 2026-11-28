# Scrolling

> A scrollable viewport over ordinary widgets, plus the shared scrollbar model.

## Overview

`Ui` is a flat widget map, so a scroll view is a **container region**, not a
`Widget` that owns children. The application supplies the viewport and lays its
children out once in *content* coordinates (the viewport's top-left corner is the
content origin). `Ui` keeps every child's rectangle at `content rect - offset`,
so hit-testing, painting, typed access, the native editor, tooltips and the
accessibility tree keep reading plain client rectangles.

Bars are painted and dragged by the container using the shared
`widgets::scrollbar` math; a panel stores only the style and per-bar pointer
state. This module is cross-platform (the wheel line count comes from the system
on Windows and defaults to 3 elsewhere).

## Key types

### `ScrollView`

```rust
pub struct ScrollView { /* private */ }

impl ScrollView {
    pub const DEFAULT_WHEEL_STEP: f32 = 48.0;
    pub fn new(id: WidgetId, viewport: Rect, axes: ScrollBars) -> Self;
}
```

Builder methods (all consume `self`):

| Method | Signature | Description |
|---|---|---|
| `child` | `child(self, id: WidgetId, rect: Rect) -> Self` | Append a child in content coordinates. |
| `children` | `children(self, impl IntoIterator<Item = ScrollChild>) -> Self` | Replace the child list. |
| `content` | `content(self, content: Size) -> Self` | Explicit content extent (floored by the children). |
| `offset` | `offset(self, offset: Point) -> Self` | Initial offset, clamped on registration. |
| `style` | `style(self, style: ScrollBarStyle) -> Self` | Bar geometry and colors. |
| `wheel_step` | `wheel_step(self, dips: f32) -> Self` | Wheel travel per notch, clamped to at least 1 DIP. |
| `padding` | `padding(self, dips: f32) -> Self` | Uniform margin between viewport edge and content. |
| `padding_by` | `padding_by(self, insets: Insets) -> Self` | Per-side margin. |
| `name` / `named` | `name(self, impl Into<String>) -> Self` | Accessibility `Group` name (also `named`). |
| `nested_in` | `nested_in(self, parent: WidgetId) -> Self` | Nest inside another panel (D92). |

Getters include `id()`, `viewport()`, `configured_viewport()`, `clip_rect()`,
`axes()`, `style_ref()`, `children_ref()`, `child_rect(id)`, `offset_of()`,
`content_size()`, `max_offset()`, `wheel_step_dips()`, `padding_insets()`,
`padded_rect()`, `content_rect()`, `name_ref()`, `parent()` and `contains(id)`.
Mutators include `set_offset`, `set_viewport`, `set_axes`, `set_style`,
`set_children`, `set_content_override`.

`ScrollBars` (from `bamboo_native_ui`) is `None | Vertical (default) |
Horizontal | Both`.

### `ScrollChild`

```rust
pub struct ScrollChild { pub id: WidgetId, pub rect: Rect }
impl ScrollChild { pub fn new(id: WidgetId, rect: Rect) -> Self; }
```

`rect` is in content coordinates; the container positions the widget at
`rect - offset`.

### `ScrollBarStyle` / `ScrollBarColors`

From `bamboo_native_ui::widgets::scrollbar` (re-exported as
`widgets::{ScrollBarStyle, ScrollBarColors}`). `ScrollBarStyle` fields and
defaults:

| Field | Type | Default |
|---|---|---|
| `thickness` | `f32` | `10.0` |
| `inset` | `f32` | `0.0` |
| `thumb_inset` | `f32` | `2.0` |
| `min_thumb` | `f32` | `24.0` |
| `page_on_track_click` | `bool` | `false` |
| `colors` | `Option<ScrollBarColors>` | `None` (resolve from the palette) |

Builders: `thickness` (min 2.0), `inset`, `thumb_inset`, `min_thumb` (min 4.0),
`page_on_track_click`, `colors`. `ScrollBarColors { track, thumb, thumb_hover,
thumb_active }` is plain data; `ScrollBarColors::from_palette(&Palette)` derives
it. The module also exposes `wheel_lines() -> f32` (system notch count, 3 on
non-Windows) and `PAGE_LINES: f32 = 100.0`.

### `Ui` methods

| Method | Signature |
|---|---|
| `set_scroll_view` | `set_scroll_view(&mut self, view: ScrollView, sink: &mut impl UiHost)` |
| `scroll_view` | `scroll_view(&self, id: WidgetId) -> Option<&ScrollView>` |
| `remove_scroll_view` | `remove_scroll_view(&mut self, id: WidgetId, sink: &mut impl UiHost) -> bool` |
| `scroll_offset` | `scroll_offset(&self, id: WidgetId) -> Option<Point>` |
| `set_scroll_offset` | `set_scroll_offset(&mut self, id, offset: Point, sink) -> bool` |
| `scroll_by` | `scroll_by(&mut self, id, dx: f32, dy: f32, sink) -> bool` |
| `scroll_to_child` | `scroll_to_child(&mut self, child: WidgetId, sink) -> bool` |
| `scroll_owner_of` | `scroll_owner_of(&self, child: WidgetId) -> Option<WidgetId>` |
| `set_scroll_viewport` | `set_scroll_viewport(&mut self, id, viewport: Rect, sink) -> bool` |

## Example

```rust
use bamboo_native_ui::{Rect, ScrollView, Size, Ui, WidgetId};
use bamboo_native_ui::ScrollBars;

fn register_queue(ui: &mut Ui<Msg>, sink: &mut impl bamboo_native_ui::UiHost) {
    let view = ScrollView::new(
        WidgetId::new("queue"),
        Rect::from_xywh(16.0, 48.0, 320.0, 480.0),
        ScrollBars::Vertical,
    )
    .padding(8.0)
    .child(WidgetId::new("row-0"), Rect::from_xywh(0.0, 0.0, 320.0, 56.0))
    .child(WidgetId::new("row-1"), Rect::from_xywh(0.0, 56.0, 320.0, 56.0))
    .name("Play queue");
    ui.set_scroll_view(view, sink);
    ui.scroll_to_child(WidgetId::new("row-1"), sink);
}
```

`row-0` and `row-1` are ordinary widgets added with `ui.add(...)`; the panel is
re-registered after a resize or a child-layout change, and it keeps its live
offset. `scroll_to_child` scrolls the owning panel the least distance that
brings the child fully into view.

## Notes and limits

- `ScrollView` is a container primitive managed by `Ui`, not a `Widget`. It has
  no builder-props, no `type=` in `.buixml`, and it is registered with
  `set_scroll_view`, not `add`.
- Call `set_scroll_view` again after a resize or child re-layout. Re-registering
  the same id keeps the live offset; use `set_scroll_offset` to change it.
- A bar is shown only while its axis actually overflows the padded viewport, so
  a short panel shows no empty track. `page_on_track_click` (default `false`)
  makes a track click jump so the thumb centers; `true` pages by ~90%.
- Wheel travel is `wheel_step` DIPs per notch (default
  `ScrollView::DEFAULT_WHEEL_STEP`, 48 DIPs = three text lines). A "scroll a
  page" system setting arrives as the large marker `PAGE_LINES`.
- A named panel publishes one `A11yRole::Group` node and the Scroll pattern;
  see `accessibility.md`.
- Nested panels (`nested_in`) move and clip with their parent; removing a parent
  turns its nested panels back into root panels at their configured viewport.

## See also

- `accessibility.md` (Scroll pattern state)
- `drag-and-drop.md` (drop targets inside a scrolled list)
- `../widgets/data/list-view.md`
- `../widgets/data/tree-view.md`
- `../widgets/inputs/text-edit.md`
