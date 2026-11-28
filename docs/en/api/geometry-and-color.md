# Geometry and color

> Platform-neutral value types for layout, hit-testing, damage tracking and painting.

## Overview

Geometry lives in `bamboo_native_ui::geometry` (re-exported from
`bamboo-native-ui-layout-core`); `Point`, `Size`, `Insets`, `Corners` and `Rect`
are also re-exported at the crate root. Colors live in
`bamboo_native_ui::color`, re-exported as `Color` and `Rgba`.

All geometry is measured in **DIPs** (device-independent pixels): one DIP is one
pixel at 96 DPI, and a backend multiplies by `dpi / 96` (`Canvas::scale`) when
it draws. Layout, hit-testing, `Ui::set_rect` and the widget contract therefore
share one coordinate space; only the backend converts to physical pixels.

A `Rect` is stored as four edges rather than origin plus size, so tiling never
accumulates rounding drift. `contains` is half-open: the right and bottom edges
are exclusive, so adjacent cells share an edge without a point belonging to
both. `merge_damage` keeps distant dirty rectangles separate and merges the
ones that overlap or touch.

## Key types

### Value types

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point { pub x: f32, pub y: f32 }
impl Point { pub const fn new(x: f32, y: f32) -> Self; }

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size { pub width: f32, pub height: f32 }
impl Size {
    pub const fn new(width: f32, height: f32) -> Self;
    pub const ZERO: Self = Self::new(0.0, 0.0);
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets { pub left: f32, pub top: f32, pub right: f32, pub bottom: f32 }
impl Insets {
    pub const fn uniform(value: f32) -> Self;
    pub const fn xy(x: f32, y: f32) -> Self;
    pub const ZERO: Self = Self::uniform(0.0);
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners {
    pub top_left: f32, pub top_right: f32, pub bottom_right: f32, pub bottom_left: f32,
}
impl Corners {
    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self;
    pub const fn uniform(radius: f32) -> Self;
    pub const ZERO: Self = Self::uniform(0.0);
    pub fn max(self) -> f32; pub fn clamped(self, width: f32, height: f32) -> Self;
    pub fn to_array(self) -> [f32; 4]; pub fn map(self, f: impl Fn(f32) -> f32) -> Self;
}
```

Corners are clockwise from the top-left. Every radius is clamped to half the
shorter side at paint time, so `Corners::uniform(rect.height() / 2.0)` is a
pill. `Insets` are positive inwards: `Rect::inset_by` shrinks by them.

### `Rect` and `PxRect`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect { pub left: f32, pub top: f32, pub right: f32, pub bottom: f32 }
impl Rect {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self;
    pub const fn from_size(size: Size) -> Self;
    pub const fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self;
    pub fn width(self) -> f32; pub fn height(self) -> f32; pub fn size(self) -> Size;
    pub fn center(self) -> Point; pub fn is_empty(self) -> bool;
    pub fn contains(self, x: f32, y: f32) -> bool;
    pub fn contains_point(self, point: Point) -> bool;
    pub fn intersects(self, other: Rect) -> bool;
    pub fn intersect(self, other: Rect) -> Option<Rect>;
    pub fn union(self, other: Rect) -> Rect;
    pub fn inset(self, dx: f32, dy: f32) -> Rect;
    pub fn inset_by(self, insets: Insets) -> Rect;
    pub fn inflate(self, dx: f32, dy: f32) -> Rect;
    pub fn translate(self, dx: f32, dy: f32) -> Rect;
    pub fn offset_into(self, outer: Rect, offset: Point) -> Rect;
    pub fn to_px(self, scale: f32) -> PxRect;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PxRect { pub left: i32, pub top: i32, pub right: i32, pub bottom: i32 }
impl PxRect {
    pub fn width(self) -> i32; pub fn height(self) -> i32; pub fn is_empty(self) -> bool;
    pub fn intersect(self, other: PxRect) -> Option<PxRect>;
    pub fn union(self, other: PxRect) -> PxRect;
    pub fn area(self) -> i64;
}
```

`from_xywh` builds from origin and size. `width`/`height` floor at zero and
`contains` is half-open. Empty rectangles never intersect and `union` ignores
them. `inset` with negative values inflates. `to_px` snaps outward (`floor`
left/top, `ceil` right/bottom); it is the only DIP-to-pixel conversion in the
public API.

### `merge_damage`

```rust
pub fn merge_damage(list: &mut Vec<Rect>, rect: Rect, cap: usize);
```

Adds `rect`, merging with every rectangle it overlaps or touches (the scan
restarts after a merge, since the union may now touch an earlier piece). Empty
rectangles are ignored. Past `cap` the list collapses to one bounding
rectangle: tracking pieces then costs more than the extra pixels.

### `Color` and `Rgba`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color { pub r: u8, pub g: u8, pub b: u8 }
impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self;
    pub const fn rgb(rgb: [u8; 3]) -> Self;
    pub const fn from_hex(hex: u32) -> Self;   // 0xRRGGBB
    pub const fn to_hex(self) -> u32; pub const fn to_rgb(self) -> [u8; 3];
    pub fn luminance(self) -> f32;
    pub fn mix(self, other: Color, t: f32) -> Color;
    pub fn lighten(self, t: f32) -> Color; pub fn darken(self, t: f32) -> Color;
    #[cfg(windows)] pub fn to_colorref(self) -> u32;
    #[cfg(windows)] pub const fn from_colorref(value: u32) -> Self;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba { pub color: Color, pub alpha: u8 }
impl Rgba {
    pub const fn new(color: Color, alpha: u8) -> Self;
    pub const fn opaque(color: Color) -> Self;
    pub fn alpha_unit(self) -> f32;
    pub fn display(self) -> String;   // #RRGGBB when opaque, else #RRGGBBAA
}
```

`Color` is opaque RGB; transparency is a separate `Rgba` value, used by the
skin protocol and surfaces that paint alpha. `luminance` uses Rec. 601 weights;
`mix`, `lighten` and `darken` clamp their factor to `0..=1`.

## Example

```rust
use bamboo_native_ui::geometry::merge_damage;
use bamboo_native_ui::{Color, Rect};

fn repaint_two_widgets() {
    let mut damage: Vec<Rect> = Vec::new();
    merge_damage(&mut damage, Rect::from_xywh(16.0, 16.0, 240.0, 24.0), 32);
    merge_damage(&mut damage, Rect::from_xywh(16.0, 60.0, 240.0, 32.0), 32);
    // Overlaps both, so the three pieces collapse into one.
    merge_damage(&mut damage, Rect::from_xywh(16.0, 36.0, 240.0, 28.0), 32);
    assert_eq!(damage.len(), 1);

    let accent = Color::from_hex(0x7FD3A8);
    assert!(accent.lighten(0.12).luminance() > accent.luminance());

    // What the backend blits at 150% DPI.
    let physical = damage[0].to_px(1.5);
    assert!(physical.right >= physical.left);
}
```

## Notes and limits

- All geometry is DIPs; a backend reports the ratio through `Canvas::dpi` and
  `Canvas::scale`. Value types are `Copy` with no identity — widget identity is
  `WidgetId` (see [The widget trait](./widget-trait.md)).
- `Color` has no alpha; use `Rgba` where transparency matters.
  `to_colorref`/`from_colorref` are Windows-only (`#[cfg(windows)]`, BGR order).
- A damage list merged past `cap` becomes one bounding rectangle; `Ui` feeds it
  through `mark_dirty` / `flush_damage` (see
  [The Ui container](./ui-container.md)).

## See also

- [The Ui container](./ui-container.md)
- [Painting and Canvas](./painting-and-canvas.md)
- [Theme and palette](./theme-and-palette.md)
- [Layout helpers](./layout-helpers.md)
- [Label](../widgets/content/label.md)
- [Architecture overview](../overview/architecture.md)
