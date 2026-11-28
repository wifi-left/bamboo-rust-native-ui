# Layout helpers

> Pure rectangle arithmetic for stacking, padding, splitting and grid layout inside a `Host::resize`.

## Overview

`bamboo_native_ui::layout` contains the small helpers the application uses to
compute widget rectangles. They are pure functions over `Rect` and run only
when the window resizes or a page is rebuilt — never on a state change. Because
they produce ordinary rectangles, painting and hit-testing share the exact same
geometry.

The helpers do not know about widgets or a tree: `VStack` and `HStack` are
cursors you advance manually, `split_h`/`split_v` divide a rectangle by
weights, `grid` tiles it, and `padded` applies `Insets`. The result is assigned
with `Ui::set_rect` (or `Ui::layout_vertical` when insertion order and
`preferred_height` are enough).

## Key types

### `VStack` and `HStack`

```rust
pub struct VStack { /* private */ }

impl VStack {
    pub fn new(area: Rect, gap: f32) -> Self;
    /// Next row of `height`; advances the cursor.
    pub fn next(&mut self, height: f32) -> Rect;
    /// Remaining space below the cursor.
    pub fn remaining(&self) -> Rect;
    pub fn cursor(&self) -> f32;
}

pub struct HStack { /* private */ }

impl HStack {
    pub fn new(area: Rect, gap: f32) -> Self;
    /// Next cell of `width`; advances the cursor.
    pub fn next(&mut self, width: f32) -> Rect;
    pub fn remaining(&self) -> Rect;
}
```

`VStack::next` returns a row spanning the full width of `area`, clamped so its
bottom never passes `area.bottom`; the cursor then advances to
`row.bottom + gap`. `remaining` reports the space below the cursor (the gap is
not included). `HStack` is the horizontal mirror: cells span the full height
and the width is clamped to `area.right`.

### `padded`, `split_h`, `split_v`, `grid`

```rust
pub fn padded(rect: Rect, insets: Insets) -> Rect;

/// Split horizontally into cells of the given weights.
pub fn split_h(rect: Rect, weights: &[f32], gap: f32) -> Vec<Rect>;

/// Split vertically into cells of the given weights.
pub fn split_v(rect: Rect, weights: &[f32], gap: f32) -> Vec<Rect>;

/// Uniform grid inside `rect`, returned row-major.
pub fn grid(rect: Rect, columns: usize, rows: usize, gap: f32) -> Vec<Rect>;
```

| Function | Behaviour |
|---|---|
| `padded` | `rect.inset_by(insets)`; positive insets shrink the rectangle. |
| `split_h` | Divides the width after subtracting `gap * (n - 1)`; each cell gets `usable * weight / total`. |
| `split_v` | Same on the height. |
| `grid` | `columns` × `rows` equal cells with gaps, row-major; both counts are clamped to at least 1. |

Weights are relative and need not sum to 1. A zero or negative total weight is
treated as `1.0`, so all cells share equally instead of producing NaN. An empty
weight list returns an empty vector.

## Example

```rust
use bamboo_native_ui::layout::{grid, padded, split_h, VStack};
use bamboo_native_ui::{Insets, Rect};

fn layout_page(client: Rect) -> Vec<(&'static str, Rect)> {
    let page = padded(client, Insets::uniform(16.0));

    // Header row, then a body split into a sidebar and a content column.
    let mut rows = VStack::new(page, 12.0);
    let header = rows.next(40.0);
    let body = rows.remaining();

    let columns = split_h(body, &[1.0, 3.0], 12.0);
    let sidebar = columns[0];
    let content = columns[1];

    // Three equal cards inside the content column.
    let cards = grid(content, 3, 1, 12.0);

    vec![
        ("header", header),
        ("sidebar", sidebar),
        ("card-0", cards[0]),
        ("card-1", cards[1]),
        ("card-2", cards[2]),
    ]
}
```

## Notes and limits

- These are layout-pass helpers. Calling them from a paint or input path
  recomputes rectangles the container already owns; recompute in `resize` or a
  page rebuild instead.
- `split_h`/`split_v` distribute the space *after* gaps, so
  `split_h(rect, &[1.0, 1.0], 10.0)` on a 100 DIP rectangle yields 45 DIP cells
  with 10 DIP between them.
- `grid` clamps `columns` and `rows` to at least 1 and cell sizes to `0.0`; it
  does not clip cells to `rect` when gaps or rounding overflow.
- `remaining` excludes the trailing gap: it starts at `cursor - gap`, clamped to
  the area's top/left.
- `VStack`/`HStack` keep no widget state; create a fresh cursor per layout pass.
- For a vertical stack of widgets in insertion order, `Ui::layout_vertical`
  wraps `VStack` with each widget's `preferred_height` (see
  [The Ui container](./ui-container.md)).

## See also

- [Geometry and color](./geometry-and-color.md)
- [The Ui container](./ui-container.md)
- [The widget trait](./widget-trait.md)
- [Label](../widgets/content/label.md)
- [First window quickstart](../quickstart/03-first-window.md)
