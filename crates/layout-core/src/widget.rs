// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Stable widget identity.
//!
//! The layout model addresses widgets by [`WidgetId`], so the type lives here;
//! `bamboo-native-ui::widget` re-exports it beside the widget contract.

/// Stable widget identity. Using `&'static str` keeps ids allocation-free and
/// readable in logs; the M7 layout skin maps rectangles by these names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WidgetId(pub &'static str);

impl WidgetId {
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// A stable id for a widget the application creates at runtime (a dynamic
    /// list row, a page built from data, an M7 schema node).
    ///
    /// The text is interned once for the process and later calls with the same
    /// text return the same `&'static str`, so `WidgetId` keeps its cheap
    /// `Copy`/`Eq`/`Hash`/`&'static str` contract and every container table
    /// keeps working. The first call for a *distinct* name leaks one small
    /// string (the interner's storage).
    ///
    /// Draw names from a bounded set — `format!("row-{index}")` over a bounded
    /// row count, or a fixed id pool — when a long session creates unbounded
    /// layers; a fresh UUID per rebuild would grow the intern table forever.
    pub fn intern(name: &str) -> Self {
        use std::collections::HashSet;
        use std::sync::{Mutex, OnceLock};

        static INTERNED: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
        let set = INTERNED.get_or_init(|| Mutex::new(HashSet::new()));
        let mut set = set.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(interned) = set.get(name) {
            return Self(*interned);
        }
        // One allocation per distinct id for the process; see the caveat above.
        let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
        set.insert(leaked);
        Self(leaked)
    }

    /// The stable string behind the id.
    pub const fn name(self) -> &'static str {
        self.0
    }
}
