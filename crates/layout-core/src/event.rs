// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Mouse cursor shapes.
//!
//! The layout model stores a cursor override, so the enum lives here with the
//! other value types; `bamboo-native-ui::event` re-exports it beside the input
//! vocabulary.

/// Mouse cursor shapes a host may request. Widgets suggest one through
/// `Widget::cursor`; the container applies it while the pointer is over them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cursor {
    #[default]
    Arrow,
    Hand,
    IBeam,
    SizeWE,
    SizeNS,
}
