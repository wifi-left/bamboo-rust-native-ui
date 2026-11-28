// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The `.buitheme` document (M7.8, D150): a whole palette as data.
//!
//! A theme document is a sibling grammar of `.buixml`: one root `<theme>` with
//! a required `base` (`light`/`dark`), an optional `accent` and any number of
//! `<color name value/>` overrides. The model does not know the host's
//! `Palette`; [`ThemeDocument::colors`] enumerates the overrides in declaration
//! order and the window crate maps them onto its palette.
//!
//! `palette` derivation is `Palette::new(settings, base)` followed by the
//! overrides, so a theme may change one token or the whole table. The canonical
//! text is [`ThemeDocument::to_xml`] and [`ThemeDocument::generation`] is its
//! FNV-1a hash, exactly like a layout document.

use crate::color::Color;
use crate::diag::LayoutDiagnostic;

/// Light or dark derivation base. Required: the document does not depend on
/// the application's current mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeBase {
    Light,
    Dark,
}

impl ThemeBase {
    pub fn is_dark(self) -> bool {
        self == Self::Dark
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

/// One `<color>` override. `name` is a palette token in kebab-case (the
/// `background` token vocabulary of the layout schema).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeColor {
    pub name: String,
    pub color: Color,
    /// 1-based source line of the `<color>` element.
    pub line: usize,
    pub column: usize,
}

impl ThemeColor {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn color(&self) -> Color {
        self.color
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn column(&self) -> usize {
        self.column
    }
}

/// A parsed `.buitheme` document.
#[derive(Clone, Debug)]
pub struct ThemeDocument {
    /// Schema version; only `1` is accepted.
    pub version: u32,
    /// Optional human-readable name (`夜蓝`).
    pub name: Option<String>,
    pub base: ThemeBase,
    /// Optional accent override. The palette's accent-derived fields follow it
    /// (`Palette::new` derives them from the accent).
    pub accent: Option<Color>,
    /// `<color>` overrides in declaration order.
    pub colors: Vec<ThemeColor>,
    /// Canonical model text.
    pub canonical: String,
    /// FNV-1a of [`ThemeDocument::canonical`].
    pub generation: u64,
}

impl ThemeDocument {
    pub fn version(&self) -> u32 {
        self.version
    }

    /// The optional `<theme name>`, trimmed.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn base(&self) -> ThemeBase {
        self.base
    }

    pub fn accent(&self) -> Option<Color> {
        self.accent
    }

    /// The `<color>` overrides in declaration order.
    pub fn colors(&self) -> &[ThemeColor] {
        &self.colors
    }

    /// The overrides are content: a change means a new palette to install.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The canonical model text (byte-identical between two saves).
    pub fn to_xml(&self) -> String {
        crate::theme::to_xml(self)
    }

    /// One `<color>` override by token name.
    pub fn color(&self, name: &str) -> Option<Color> {
        self.colors
            .iter()
            .find(|color| color.name == name)
            .map(|color| color.color)
    }
}

/// Build the canonical text. Colors are written as opaque `#rrggbb` (a theme
/// palette has no alpha; the parser drops it like a widget without
/// transparency).
pub(crate) fn to_xml(document: &ThemeDocument) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    out.push_str("<theme version=\"");
    out.push_str(&document.version.to_string());
    out.push('"');
    if let Some(name) = document.name.as_deref() {
        out.push_str(" name=\"");
        out.push_str(&escape(name));
        out.push('"');
    }
    out.push_str(" base=\"");
    out.push_str(document.base.name());
    out.push('"');
    if let Some(accent) = document.accent {
        out.push_str(" accent=\"");
        out.push_str(&color_source(accent));
        out.push('"');
    }
    if document.colors.is_empty() {
        out.push_str("/>\n");
        return out;
    }
    out.push_str(">\n");
    for color in &document.colors {
        out.push_str("  <color name=\"");
        out.push_str(&escape(&color.name));
        out.push_str("\" value=\"");
        out.push_str(&color_source(color.color));
        out.push_str("\"/>\n");
    }
    out.push_str("</theme>\n");
    out
}

fn color_source(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// A theme document rejected at parse time.
pub type ThemeDiagnostics = Vec<LayoutDiagnostic>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_names_round_trip() {
        for base in [ThemeBase::Light, ThemeBase::Dark] {
            assert_eq!(ThemeBase::from_name(base.name()), Some(base));
            assert_eq!(base.is_dark(), base == ThemeBase::Dark);
        }
        assert_eq!(ThemeBase::from_name("auto"), None);
    }
}
