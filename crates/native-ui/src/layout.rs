// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Layout helpers for the retained widget tree.
//!
//! These run only when the window resizes or a page is rebuilt; state changes
//! never re-run layout. They are pure functions over [`Rect`], so painting and
//! hit-testing share the same rectangles.

use crate::geometry::{Insets, Rect};

/// Vertical stack cursor.
pub struct VStack {
    area: Rect,
    gap: f32,
    cursor: f32,
}

impl VStack {
    pub fn new(area: Rect, gap: f32) -> Self {
        Self {
            area,
            gap,
            cursor: area.top,
        }
    }

    /// Next row of `height`; advances the cursor.
    pub fn next(&mut self, height: f32) -> Rect {
        let rect = Rect::new(
            self.area.left,
            self.cursor,
            self.area.right,
            (self.cursor + height).min(self.area.bottom),
        );
        self.cursor = rect.bottom + self.gap;
        rect
    }

    /// Remaining space below the cursor.
    pub fn remaining(&self) -> Rect {
        Rect::new(
            self.area.left,
            (self.cursor - self.gap).max(self.area.top),
            self.area.right,
            self.area.bottom,
        )
    }

    pub fn cursor(&self) -> f32 {
        self.cursor
    }
}

/// Horizontal stack cursor.
pub struct HStack {
    area: Rect,
    gap: f32,
    cursor: f32,
}

impl HStack {
    pub fn new(area: Rect, gap: f32) -> Self {
        Self {
            area,
            gap,
            cursor: area.left,
        }
    }

    /// Next cell of `width`; advances the cursor.
    pub fn next(&mut self, width: f32) -> Rect {
        let rect = Rect::new(
            self.cursor,
            self.area.top,
            (self.cursor + width).min(self.area.right),
            self.area.bottom,
        );
        self.cursor = rect.right + self.gap;
        rect
    }

    pub fn remaining(&self) -> Rect {
        Rect::new(
            (self.cursor - self.gap).max(self.area.left),
            self.area.top,
            self.area.right,
            self.area.bottom,
        )
    }
}

pub fn padded(rect: Rect, insets: Insets) -> Rect {
    rect.inset_by(insets)
}

/// Split `rect` horizontally into cells of the given weights.
pub fn split_h(rect: Rect, weights: &[f32], gap: f32) -> Vec<Rect> {
    split(rect, weights, gap, true)
}

/// Split `rect` vertically into cells of the given weights.
pub fn split_v(rect: Rect, weights: &[f32], gap: f32) -> Vec<Rect> {
    split(rect, weights, gap, false)
}

fn split(rect: Rect, weights: &[f32], gap: f32, horizontal: bool) -> Vec<Rect> {
    let count = weights.len();
    if count == 0 {
        return Vec::new();
    }
    let total_weight: f32 = weights.iter().copied().sum();
    let total_weight = if total_weight > 0.0 { total_weight } else { 1.0 };
    let span = if horizontal { rect.width() } else { rect.height() };
    let usable = (span - gap * (count.saturating_sub(1)) as f32).max(0.0);
    let mut out = Vec::with_capacity(count);
    let mut offset = if horizontal { rect.left } else { rect.top };
    for weight in weights {
        let size = usable * weight / total_weight;
        let cell = if horizontal {
            Rect::new(offset, rect.top, offset + size, rect.bottom)
        } else {
            Rect::new(rect.left, offset, rect.right, offset + size)
        };
        out.push(cell);
        offset += size + gap;
    }
    out
}

/// Uniform grid inside `rect`.
pub fn grid(rect: Rect, columns: usize, rows: usize, gap: f32) -> Vec<Rect> {
    let columns = columns.max(1);
    let rows = rows.max(1);
    let cell_w = ((rect.width() - gap * (columns - 1) as f32) / columns as f32).max(0.0);
    let cell_h = ((rect.height() - gap * (rows - 1) as f32) / rows as f32).max(0.0);
    let mut out = Vec::with_capacity(columns * rows);
    for row in 0..rows {
        for column in 0..columns {
            let left = rect.left + column as f32 * (cell_w + gap);
            let top = rect.top + row as f32 * (cell_h + gap);
            out.push(Rect::new(left, top, left + cell_w, top + cell_h));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_stack_advances_and_clamps() {
        let mut stack = VStack::new(Rect::new(0.0, 0.0, 100.0, 100.0), 8.0);
        assert_eq!(stack.next(20.0), Rect::new(0.0, 0.0, 100.0, 20.0));
        assert_eq!(stack.next(20.0), Rect::new(0.0, 28.0, 100.0, 48.0));
        let last = stack.next(200.0);
        assert_eq!(last.bottom, 100.0);
    }

    #[test]
    fn split_honours_gaps_and_weights() {
        let cells = split_h(Rect::new(0.0, 0.0, 100.0, 10.0), &[1.0, 1.0], 10.0);
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0], Rect::new(0.0, 0.0, 45.0, 10.0));
        assert_eq!(cells[1], Rect::new(55.0, 0.0, 100.0, 10.0));
    }

    #[test]
    fn grid_covers_the_area() {
        let cells = grid(Rect::new(0.0, 0.0, 100.0, 100.0), 3, 2, 5.0);
        assert_eq!(cells.len(), 6);
        assert!((cells[5].right - 100.0).abs() < 0.01);
        assert!((cells[5].bottom - 100.0).abs() < 0.01);
    }

    #[test]
    fn split_with_no_weights_is_empty() {
        assert!(split_v(Rect::new(0.0, 0.0, 10.0, 10.0), &[], 4.0).is_empty());
    }
}
