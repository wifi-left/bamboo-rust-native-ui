// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The widget-declared skin protocol (M7 §6.2–6.4, D127).
//!
//! The shared layout vocabulary (geometry, generic properties, surface/font
//! tokens and the content initial values) is fixed by the parser. Everything a
//! single widget type accepts beyond that is published by the widget itself
//! through six provided `Widget` methods, so supporting a component is
//! a local implementation and a widget outside this crate needs no registry,
//! no `TypeId` table and no feature flag.
//!
//! The types live here, compiled unconditionally: a third-party widget can
//! declare support in a build without `layout-xml`. The applier validates
//! every declaration and every value before it mutates anything
//! (`Ui::apply_layout` in `bamboo-native-ui`).
//!
//! Names beginning with `x-` are reserved for user code; the library never
//! claims one. The shared content names are reserved for the schema (see
//! [`is_shared_name`]).

use crate::color::{Color, Rgba};

/// How the applier parses and checks one declared attribute value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkinValueKind {
    /// The raw text; the widget interprets it (dates, enums, names).
    Text,
    /// A finite decimal number.
    Number,
    /// `true` / `false` / `1` / `0`.
    Bool,
    /// `#rgb`/`#rrggbb` (opaque), `#rgba`/`#rrggbbaa`, a decimal channel list
    /// (`r,g,b` / `r,g,b,a`), or the functional forms `rgb(r,g,b)` /
    /// `rgba(r,g,b,a)`. Whitespace is ignored anywhere in the literal.
    Color,
}

impl SkinValueKind {
    /// Parse one raw attribute value. `None` is `xml.skin.value`.
    pub fn parse(self, raw: &str) -> Option<SkinValue<'_>> {
        match self {
            Self::Text => Some(SkinValue::Text(raw)),
            Self::Number => match raw.trim().parse::<f64>() {
                Ok(number) if number.is_finite() => Some(SkinValue::Number(number)),
                _ => None,
            },
            Self::Bool => match raw.trim() {
                "true" | "1" => Some(SkinValue::Bool(true)),
                "false" | "0" => Some(SkinValue::Bool(false)),
                _ => None,
            },
            Self::Color => parse_color(raw).map(SkinValue::Color),
        }
    }

    /// Parse one value, letting `resolve` name a color first (the layout
    /// applier resolves palette tokens such as `accent`/`card`/`text`). Other
    /// kinds ignore the resolver, and a name it does not know still falls back
    /// to the literal color grammar.
    pub fn parse_with<'a>(
        self,
        raw: &'a str,
        resolve: impl FnOnce(&str) -> Option<Color>,
    ) -> Option<SkinValue<'a>> {
        if self == Self::Color
            && let Some(color) = resolve(raw.trim())
        {
            return Some(SkinValue::Color(Rgba::opaque(color)));
        }
        self.parse(raw)
    }
}

/// One attribute a widget accepts in a layout document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkinAttrDecl {
    pub name: &'static str,
    pub kind: SkinValueKind,
}

impl SkinAttrDecl {
    pub const fn new(name: &'static str, kind: SkinValueKind) -> Self {
        Self { name, kind }
    }
}

/// A parsed local attribute value, handed to a widget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SkinValue<'a> {
    Text(&'a str),
    Number(f64),
    Bool(bool),
    /// One color value with alpha, from `#rgb`/`#rrggbb` (opaque), `#rgba`/
    /// `#rrggbbaa`, or a palette token (opaque). This is the single color
    /// representation of the skin protocol: a widget that cannot show
    /// transparency reads the RGB part through [`SkinValue::as_color`] and
    /// silently drops the alpha.
    Color(Rgba),
}

impl SkinValue<'_> {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(number) => Some(*number),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(on) => Some(*on),
            _ => None,
        }
    }

    /// The color's RGB part. The alpha is dropped here — a widget that cannot
    /// represent transparency uses this and stays silent about the alpha.
    pub fn as_color(&self) -> Option<Color> {
        self.as_rgba().map(|rgba| rgba.color)
    }

    /// The full color value with alpha. A widget that supports transparency
    /// (the color picker) uses this.
    pub fn as_rgba(&self) -> Option<Rgba> {
        match self {
            Self::Color(rgba) => Some(*rgba),
            _ => None,
        }
    }
}

/// One data child element (`<item/>`, `<track/>`, a user `<x-item/>`).
///
/// The model owns its text (a parsed document is self-contained and survives
/// the source buffer), so this is a plain owned tree; child values are raw
/// strings and never expressions. The widget validates and parses them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkinElement {
    pub name: String,
    /// Attributes in source order; a duplicate name is a parse error.
    pub attrs: Vec<(String, String)>,
    pub children: Vec<SkinElement>,
    /// 1-based position of the opening tag, for diagnostics.
    pub line: usize,
    pub column: usize,
}

impl SkinElement {
    pub fn new(name: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            name: name.into(),
            attrs: Vec::new(),
            children: Vec::new(),
            line,
            column,
        }
    }

    /// The raw value of one attribute.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// A `Bool` attribute; `None` when absent or malformed.
    pub fn attr_bool(&self, name: &str) -> Option<bool> {
        SkinValueKind::Bool.parse(self.attr(name)?)?.as_bool()
    }

    /// A `Number` attribute; `None` when absent or malformed.
    pub fn attr_number(&self, name: &str) -> Option<f64> {
        SkinValueKind::Number
            .parse(self.attr(name)?)?
            .as_number()
    }

    /// Whether an optional `Bool` attribute is absent or parses.
    pub fn attr_bool_ok(&self, name: &str) -> bool {
        self.attr(name)
            .is_none_or(|raw| SkinValueKind::Bool.parse(raw).is_some())
    }

    /// Whether an optional `Number` attribute is absent or parses.
    pub fn attr_number_ok(&self, name: &str) -> bool {
        self.attr(name)
            .is_none_or(|raw| SkinValueKind::Number.parse(raw).is_some())
    }

    /// Whether every attribute name is in `allowed` (a typo guard for a child
    /// element's fixed vocabulary).
    pub fn attrs_are(&self, allowed: &[&str]) -> bool {
        self.attrs
            .iter()
            .all(|(name, _)| allowed.contains(&name.as_str()))
    }

    pub fn push_attr(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.attrs.push((name.into(), value.into()));
    }

    pub fn push_child(&mut self, child: SkinElement) {
        self.children.push(child);
    }

    /// Structural equality ignoring the source positions, so two variants that
    /// declare the same data at different offsets do not look different.
    pub fn same_data(&self, other: &SkinElement) -> bool {
        self.name == other.name
            && self.attrs == other.attrs
            && self.children.len() == other.children.len()
            && self
                .children
                .iter()
                .zip(&other.children)
                .all(|(left, right)| left.same_data(right))
    }
}

/// The content names the schema owns. A widget declaration that reuses one is
/// `xml.skin.reserved`.
pub fn is_shared_name(name: &str) -> bool {
    matches!(
        name,
        "text" | "label" | "value" | "progress" | "number" | "checked" | "selected"
    )
}

/// Parse one color literal into the skin's single color value. Whitespace is
/// ignored anywhere; the accepted forms are:
///
/// - `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`
/// - decimal channels `r,g,b` or `r,g,b,a`, separated by commas *or* spaces
///   (0..255; a bare fractional alpha `0.5` is read as 0..1)
/// - `rgb(r,g,b)` and `rgba(r,g,b,a)`, with either separator; the functional
///   alpha follows CSS (`0..1`, or `0..255` when it exceeds 1)
///
/// Both `SkinValueKind::Color` and the layout's `background` literal use this
/// parser, so the XML speaks one color grammar.
pub fn parse_color(raw: &str) -> Option<Rgba> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(hex) = trimmed.strip_prefix('#') {
        let compact: String = hex.chars().filter(|ch| !ch.is_whitespace()).collect();
        return rgba_from_hex(&compact);
    }
    let (body, functional) = match function_inner(trimmed) {
        Some(inner) => (inner, true),
        None => (trimmed, false),
    };
    let mut parts = split_channels(body).into_iter();
    let r = parse_channel(parts.next()?)?;
    let g = parse_channel(parts.next()?)?;
    let b = parse_channel(parts.next()?)?;
    let alpha = match parts.next() {
        Some(token) => parse_alpha(token, functional)?,
        None => 255,
    };
    // A fourth separator means too many channels.
    if parts.next().is_some() {
        return None;
    }
    Some(Rgba::new(Color::new(r, g, b), alpha))
}

/// The inside of `rgb(...)`/`rgba(...)` (a trailing `)` is required and no
/// other text may follow). Returns `None` for a bare channel list.
fn function_inner(text: &str) -> Option<&str> {
    let open = text.find('(')?;
    let name = text.get(..open)?.trim();
    if !name.eq_ignore_ascii_case("rgb") && !name.eq_ignore_ascii_case("rgba") {
        return None;
    }
    let close = text.rfind(')')?;
    if close < open || !text.get(close + 1..)?.trim().is_empty() {
        return None;
    }
    text.get(open + 1..close)
}

/// Split channels on commas *or* whitespace, so `255,0,0`, `255 0 0` and
/// `rgb(255 0 0)` all read the same.
fn split_channels(body: &str) -> Vec<&str> {
    body.split(|ch: char| ch == ',' || ch.is_whitespace())
        .filter(|token| !token.is_empty())
        .collect()
}

/// One RGB channel: a decimal 0..255 (clamped and rounded).
fn parse_channel(token: &str) -> Option<u8> {
    let value: f64 = token.parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    Some(value.round().clamp(0.0, 255.0) as u8)
}

/// The alpha channel. A functional `rgba(...)` follows CSS (`0..1`, or `0..255`
/// above 1); a bare decimal list is 0..255, where a fractional value below 1
/// (`0.5`) is a fraction of 255.
fn parse_alpha(token: &str, functional: bool) -> Option<u8> {
    let value: f64 = token.parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    let alpha = if functional {
        if value <= 1.0 {
            value.clamp(0.0, 1.0) * 255.0
        } else {
            value.clamp(0.0, 255.0)
        }
    } else if value < 1.0 {
        value.clamp(0.0, 1.0) * 255.0
    } else {
        value.clamp(0.0, 255.0)
    };
    Some(alpha.round() as u8)
}

/// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` (the `#` already stripped).
fn rgba_from_hex(hex: &str) -> Option<Rgba> {
    let digits: Vec<u8> = hex
        .bytes()
        .map(|byte| (byte as char).to_digit(16).map(|value| value as u8))
        .collect::<Option<Vec<u8>>>()?;
    // CSS-style shorthand (`#rgb`, `#rgba`) expands each digit by 17.
    let at = |index: usize, short: bool| {
        if short {
            digits[index] * 17
        } else {
            digits[index]
        }
    };
    let rgb = |short: bool| Color::new(at(0, short), at(1, short), at(2, short));
    match digits.len() {
        3 => Some(Rgba::opaque(rgb(true))),
        4 => Some(Rgba::new(rgb(true), at(3, true))),
        6 => Some(Rgba::opaque(Color::new(
            digits[0] * 16 + digits[1],
            digits[2] * 16 + digits[3],
            digits[4] * 16 + digits[5],
        ))),
        8 => Some(Rgba::new(
            Color::new(
                digits[0] * 16 + digits[1],
                digits[2] * 16 + digits[3],
                digits[4] * 16 + digits[5],
            ),
            digits[6] * 16 + digits[7],
        )),
        _ => None,
    }
}

