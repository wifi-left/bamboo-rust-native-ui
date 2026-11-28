// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Geometry in logical pixels (DIPs).
//!
//! Rectangles are half-open: `contains` treats the right/bottom edge as
//! exclusive. Widget rectangles are stable for a window size, so a state change
//! can invalidate exactly one rectangle.

/// A point in DIPs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A size in DIPs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub const ZERO: Self = Self::new(0.0, 0.0);
}

/// Insets in DIPs (positive values shrink the rectangle).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Insets {
    pub const fn uniform(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    pub const fn xy(x: f32, y: f32) -> Self {
        Self {
            left: x,
            top: y,
            right: x,
            bottom: y,
        }
    }

    pub const ZERO: Self = Self::uniform(0.0);
}

/// Independent corner radii, clockwise from the top-left.
///
/// A rounded rectangle usually has one radius ([`Corners::uniform`]); a card
/// with one square corner (a notch, a sheet) sets them individually. Each
/// radius is clamped to half the shorter side of the rectangle when it is
/// painted.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl Corners {
    pub const fn new(
        top_left: f32,
        top_right: f32,
        bottom_right: f32,
        bottom_left: f32,
    ) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub const fn uniform(radius: f32) -> Self {
        Self::new(radius, radius, radius, radius)
    }

    pub const ZERO: Self = Self::uniform(0.0);

    /// Largest radius; a backend without per-corner support paints the whole
    /// outline with it.
    pub fn max(self) -> f32 {
        self.top_left
            .max(self.top_right)
            .max(self.bottom_right)
            .max(self.bottom_left)
    }

    /// Radii clamped for a rectangle of `width × height` (each to half the
    /// shorter side, and non-negative).
    pub fn clamped(self, width: f32, height: f32) -> Self {
        let limit = (width.min(height) / 2.0).max(0.0);
        Self {
            top_left: self.top_left.clamp(0.0, limit),
            top_right: self.top_right.clamp(0.0, limit),
            bottom_right: self.bottom_right.clamp(0.0, limit),
            bottom_left: self.bottom_left.clamp(0.0, limit),
        }
    }

    /// The corners as an array `[top-left, top-right, bottom-right, bottom-left]`.
    pub fn to_array(self) -> [f32; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }

    /// Apply `f` to every radius (used to derive the inner corners of a
    /// frame: each radius shrinks by the border thickness).
    pub fn map(self, f: impl Fn(f32) -> f32) -> Self {
        Self {
            top_left: f(self.top_left),
            top_right: f(self.top_right),
            bottom_right: f(self.bottom_right),
            bottom_left: f(self.bottom_left),
        }
    }
}

/// A rectangle in DIPs, stored as four edges.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn from_size(size: Size) -> Self {
        Self::new(0.0, 0.0, size.width, size.height)
    }

    pub const fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self::new(x, y, x + width, y + height)
    }

    pub fn width(self) -> f32 {
        (self.right - self.left).max(0.0)
    }

    pub fn height(self) -> f32 {
        (self.bottom - self.top).max(0.0)
    }

    pub fn size(self) -> Size {
        Size::new(self.width(), self.height())
    }

    pub fn center(self) -> Point {
        Point::new(
            (self.left + self.right) * 0.5,
            (self.top + self.bottom) * 0.5,
        )
    }

    pub fn is_empty(self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    pub fn contains_point(self, point: Point) -> bool {
        self.contains(point.x, point.y)
    }

    pub fn intersects(self, other: Rect) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }

    /// Intersection, or `None` when the rectangles do not overlap.
    pub fn intersect(self, other: Rect) -> Option<Rect> {
        let result = Rect::new(
            self.left.max(other.left),
            self.top.max(other.top),
            self.right.min(other.right),
            self.bottom.min(other.bottom),
        );
        (!result.is_empty()).then_some(result)
    }

    /// Smallest rectangle containing both.
    pub fn union(self, other: Rect) -> Rect {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        Rect::new(
            self.left.min(other.left),
            self.top.min(other.top),
            self.right.max(other.right),
            self.bottom.max(other.bottom),
        )
    }

    /// Shrink by `dx`/`dy` on both sides; negative values inflate.
    pub fn inset(self, dx: f32, dy: f32) -> Rect {
        Rect::new(
            self.left + dx,
            self.top + dy,
            self.right - dx,
            self.bottom - dy,
        )
    }

    pub fn inset_by(self, insets: Insets) -> Rect {
        Rect::new(
            self.left + insets.left,
            self.top + insets.top,
            self.right - insets.right,
            self.bottom - insets.bottom,
        )
    }

    pub fn inflate(self, dx: f32, dy: f32) -> Rect {
        self.inset(-dx, -dy)
    }

    pub fn translate(self, dx: f32, dy: f32) -> Rect {
        Rect::new(
            self.left + dx,
            self.top + dy,
            self.right + dx,
            self.bottom + dy,
        )
    }

    /// Offset so that `self` sits inside `outer` at `offset` from its origin.
    pub fn offset_into(self, outer: Rect, offset: Point) -> Rect {
        self.translate(outer.left + offset.x, outer.top + offset.y)
    }

    /// Snap out to whole device pixels at `scale` (DPI/96).
    pub fn to_px(self, scale: f32) -> PxRect {
        PxRect {
            left: (self.left * scale).floor() as i32,
            top: (self.top * scale).floor() as i32,
            right: (self.right * scale).ceil() as i32,
            bottom: (self.bottom * scale).ceil() as i32,
        }
    }
}

/// A rectangle in physical pixels, used by the backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PxRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PxRect {
    pub fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }

    pub fn height(self) -> i32 {
        (self.bottom - self.top).max(0)
    }

    pub fn is_empty(self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    pub fn intersect(self, other: PxRect) -> Option<PxRect> {
        let result = PxRect {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (!result.is_empty()).then_some(result)
    }

    pub fn union(self, other: PxRect) -> PxRect {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        PxRect {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    pub fn area(self) -> i64 {
        self.width() as i64 * self.height() as i64
    }
}

/// Add `rect` to a damage list, merging it with every rectangle it overlaps or
/// touches (a merge may make it touch a rectangle examined earlier, so the
/// scan restarts). Keeping the pieces separate is what makes a partial repaint
/// partial: one union rectangle over two distant widgets would repaint
/// everything between them.
///
/// `cap` bounds the list. Exceeding it collapses the list to one bounding
/// rectangle, because past that point tracking pieces costs more than the
/// extra pixels.
pub fn merge_damage(list: &mut Vec<Rect>, rect: Rect, cap: usize) {
    if rect.is_empty() {
        return;
    }
    let mut merged = rect;
    let mut index = 0;
    while index < list.len() {
        // A hair of tolerance merges rectangles that share an edge (two
        // adjacent rows) without swallowing a real gap.
        if list[index].inflate(0.01, 0.01).intersects(merged) {
            merged = list.swap_remove(index).union(merged);
            index = 0;
        } else {
            index += 1;
        }
    }
    list.push(merged);
    if list.len() > cap {
        let bounding = list.iter().copied().reduce(Rect::union).unwrap_or(merged);
        list.clear();
        list.push(bounding);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_pieces_stay_separate_until_they_touch() {
        let mut list = Vec::new();
        merge_damage(&mut list, Rect::new(0.0, 0.0, 20.0, 20.0), 32);
        merge_damage(&mut list, Rect::new(100.0, 100.0, 120.0, 120.0), 32);
        assert_eq!(list.len(), 2, "distant widgets keep separate pieces");
        // Overlapping and edge-adjacent rectangles merge.
        merge_damage(&mut list, Rect::new(15.0, 15.0, 30.0, 30.0), 32);
        assert_eq!(list.len(), 2);
        merge_damage(&mut list, Rect::new(30.0, 0.0, 40.0, 10.0), 32);
        assert_eq!(list.len(), 2);
        assert!(list.contains(&Rect::new(0.0, 0.0, 40.0, 30.0)));
    }

    #[test]
    fn damage_cap_collapses_to_one_box() {
        let mut list = Vec::new();
        for index in 0..5 {
            let x = index as f32 * 100.0;
            merge_damage(&mut list, Rect::new(x, 0.0, x + 10.0, 10.0), 4);
        }
        assert_eq!(list, vec![Rect::new(0.0, 0.0, 410.0, 10.0)]);
    }

    #[test]
    fn contains_is_half_open() {
        let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(rect.contains(0.0, 0.0));
        assert!(rect.contains(9.99, 9.99));
        assert!(!rect.contains(10.0, 5.0));
        assert!(!rect.contains(5.0, 10.0));
    }

    #[test]
    fn empty_rect_does_not_intersect() {
        let empty = Rect::new(5.0, 5.0, 5.0, 5.0);
        assert!(!empty.intersects(Rect::new(0.0, 0.0, 10.0, 10.0)));
        assert!(empty.is_empty());
    }

    #[test]
    fn union_ignores_empty() {
        let a = Rect::new(0.0, 0.0, 4.0, 4.0);
        assert_eq!(a.union(Rect::default()), a);
        let b = Rect::new(2.0, 6.0, 8.0, 9.0);
        assert_eq!(a.union(b), Rect::new(0.0, 0.0, 8.0, 9.0));
    }

    #[test]
    fn inset_and_inflate_round_trip() {
        let rect = Rect::new(10.0, 10.0, 30.0, 20.0);
        assert_eq!(rect.inset(2.0, 3.0).inflate(2.0, 3.0), rect);
    }

    #[test]
    fn to_px_snaps_outward() {
        let rect = Rect::new(1.2, 2.1, 3.4, 4.9).to_px(1.5);
        assert_eq!(
            rect,
            PxRect {
                left: 1,
                top: 3,
                right: 6,
                bottom: 8
            }
        );
    }

    #[test]
    fn px_union_and_area() {
        let a = PxRect {
            left: 0,
            top: 0,
            right: 4,
            bottom: 4,
        };
        let b = PxRect {
            left: 2,
            top: 2,
            right: 6,
            bottom: 8,
        };
        assert_eq!(
            a.union(b),
            PxRect {
                left: 0,
                top: 0,
                right: 6,
                bottom: 8
            }
        );
        assert_eq!(a.area(), 16);
    }
}
