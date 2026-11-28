// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Platform-neutral colors.
//!
//! The color values themselves live in `bamboo-native-ui-layout-core` (the
//! layout model stores them); this module re-exports them at their historical
//! path and adds the painter helper, which is the only part that needs the
//! canvas.

pub use bamboo_native_ui_layout_core::color::{Color, Rgba};

use crate::geometry::Rect;

/// Paint a rectangle band (used by the control painters).
pub fn fill(canvas: &mut dyn crate::backend::Canvas, rect: Rect, color: Color) {
    canvas.fill_rect(rect, color);
}
