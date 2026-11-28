// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Host-side core of the M7 layout skin (`.buixml`): shared value types, the
//! parsed document model, the canonical writer and — behind the `parse`
//! feature — the XML reader.
//!
//! The crate has no Win32 and no `Ui`: the dynamic path (`bamboo-native-ui`
//! with `layout-xml`) re-exports it next to the applier, and the compiled path
//! (codegen) builds on the same model so both modes share one schema. D128 is
//! the decision; `docs/M7-XML.md` is the contract.
//!
//! - [`model::LayoutDocument`] — the immutable parsed model.
//! - [`theme::ThemeDocument`] — a `.buitheme` palette document (M7.8) with its
//!   canonical writer and generation hash.
//! - [`parse`] (feature `parse`) — `.buixml` text to [`model::LayoutDocument`].
//! - [`model::LayoutDocument::to_xml`] — the canonical text (generation hash).
//!
//! The value types ([`geometry`], [`color`], [`skin`], [`a11y::A11yRole`],
//! [`event::Cursor`], [`widget::WidgetId`]) live here so the model needs no
//! dependency on the window crate; `bamboo-native-ui` re-exports them at their
//! historical paths.

pub mod a11y;
pub mod color;
pub mod diag;
pub mod event;
pub mod expr;
pub mod geometry;
pub mod model;
pub mod radius;
pub mod skin;
pub mod theme;
pub mod widget;

#[cfg(feature = "parse")]
pub mod parse;
#[cfg(feature = "parse")]
pub mod theme_parse;
#[cfg(feature = "parse")]
pub mod write;

pub use diag::{LayoutDiagnostic, LayoutSeverity};
pub use model::{
    EntrySpace, LayoutConstants, LayoutDocument, LayoutEntry, LayoutSpace, LayoutVariant,
    RustBinding, ScrollEntry,
};
pub use theme::{ThemeBase, ThemeColor, ThemeDocument, ThemeDiagnostics};

/// Whether a name is one of the palette tokens (the app resolves it against
/// its `Palette`).
pub use model::{palette_token_for_name, PALETTE_TOKENS};

/// FNV-1a 64 of the canonical document text: the content-generation key.
pub fn generation_hash(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
