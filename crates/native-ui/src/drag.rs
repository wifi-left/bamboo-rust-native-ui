// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Data carried by a drag and the effect the drop reports.
//!
//! A drag is a system drag: it leaves the window and the process (OLE
//! `DoDragDrop`), so the payload is limited to what the shell can transport —
//! Unicode text and file lists. An application that needs a richer payload
//! encodes it into the text (JSON, a URL, an id) instead of a process-local
//! value.

use std::path::PathBuf;

/// What a drag carries across windows and applications.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragData {
    /// Unicode text (`CF_UNICODETEXT`): a field selection, a path, a URL or
    /// an application-encoded object.
    Text(String),
    /// A file/folder list (`CF_HDROP`), as dropped by Explorer.
    Files(Vec<PathBuf>),
}

impl DragData {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub fn files(paths: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        Self::Files(paths.into_iter().map(Into::into).collect())
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Files(_) => None,
        }
    }

    pub fn as_files(&self) -> Option<&[PathBuf]> {
        match self {
            Self::Files(paths) => Some(paths),
            Self::Text(_) => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Text(text) => text.is_empty(),
            Self::Files(paths) => paths.is_empty(),
        }
    }
}

/// What the drop target did with the data. A source reads this after its drag
/// loop ends (a `Move` tells it to remove the original rows/files).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropEffect {
    /// No target accepted the data.
    #[default]
    None,
    /// The target copied the data; the source keeps it.
    Copy,
    /// The target took the data; the source may remove it.
    Move,
    /// The target created a link/reference.
    Link,
}

impl DropEffect {
    pub fn accepted(self) -> bool {
        self != Self::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_data_exposes_the_right_payload() {
        let text = DragData::text("hello");
        assert_eq!(text.as_text(), Some("hello"));
        assert!(text.as_files().is_none());
        let files = DragData::files(["C:\\a.mp3", "C:\\b.mp3"]);
        assert_eq!(files.as_files().map(|paths| paths.len()), Some(2));
        assert!(files.as_text().is_none());
        assert!(DropEffect::Copy.accepted());
        assert!(!DropEffect::None.accepted());
    }
}
