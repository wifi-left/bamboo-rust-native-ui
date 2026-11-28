// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Named corner radii the layout schema's `radius` token resolves to.
//!
//! `bamboo-native-ui::theme` re-exports this module, so widgets keep using
//! `theme::radius::*`. `pill` is a helper, not a token: half the height.

pub const SMALL: f32 = 4.0;
pub const MEDIUM: f32 = 6.0;
pub const LARGE: f32 = 10.0;

/// A half-height radius (a pill).
pub fn pill(height: f32) -> f32 {
    height.max(0.0) / 2.0
}
