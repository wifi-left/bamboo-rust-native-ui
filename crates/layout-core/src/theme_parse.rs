// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The `.buitheme` reader (M7.8, D150, feature `parse`).
//!
//! One root `<theme>` with `version`/`name`/`base`/`accent`, and `<color>`
//! children. Parsing is pure: the whole model or a diagnostic list. Codes are
//! stable (`theme.*`).

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::diag::{Diagnostics, LayoutDiagnostic};
use crate::theme::{ThemeBase, ThemeColor, ThemeDocument};

const MAX_BYTES: usize = 1024 * 1024;
const MAX_DIAGNOSTICS: usize = 100;
const MAX_COLORS: usize = 512;

impl ThemeDocument {
    /// Parse one `.buitheme` document.
    pub fn parse(text: &str) -> Result<Self, Vec<LayoutDiagnostic>> {
        Parser::new(text).run()
    }
}

struct Parser<'a> {
    source: &'a str,
    diagnostics: Diagnostics,
    seen_root: bool,
    version: Option<u32>,
    name: Option<String>,
    base: Option<ThemeBase>,
    accent: Option<crate::color::Color>,
    colors: Vec<ThemeColor>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            diagnostics: Diagnostics::new(MAX_DIAGNOSTICS),
            seen_root: false,
            version: None,
            name: None,
            base: None,
            accent: None,
            colors: Vec::new(),
        }
    }

    fn run(mut self) -> Result<ThemeDocument, Vec<LayoutDiagnostic>> {
        if self.source.len() > MAX_BYTES {
            self.diagnostics.error(
                "theme.syntax",
                1,
                1,
                "theme",
                format!("the file is larger than {MAX_BYTES} bytes"),
            );
            return Err(self.diagnostics.into_vec());
        }
        let source = self.source.strip_prefix('\u{feff}').unwrap_or(self.source);
        let mut reader = Reader::from_str(source);
        reader.config_mut().trim_text(true);

        let mut stack: Vec<String> = Vec::new();
        loop {
            let before = reader.buffer_position() as usize;
            let event = match reader.read_event() {
                Ok(event) => event,
                Err(error) => {
                    let (line, column) = position(source, reader.error_position() as usize);
                    self.diagnostics.error(
                        "theme.syntax",
                        line,
                        column,
                        "theme",
                        error.to_string(),
                    );
                    return Err(self.diagnostics.into_vec());
                }
            };
            let after = reader.buffer_position() as usize;
            match event {
                Event::Eof => break,
                Event::Decl(_) | Event::Comment(_) => {}
                Event::Text(_) => {
                    let (line, column) = position(source, before);
                    self.diagnostics.error(
                        "theme.structure",
                        line,
                        column,
                        "theme",
                        "text nodes are not allowed; only whitespace is ignored",
                    );
                }
                Event::CData(_) => self.structural(source, before, "CDATA sections are not allowed"),
                Event::GeneralRef(_) => self.structural(
                    source,
                    before,
                    "character/entity references in content are not allowed",
                ),
                Event::PI(_) => {
                    self.structural(source, before, "processing instructions are not allowed")
                }
                Event::DocType(_) => self.structural(source, before, "a DOCTYPE is not allowed"),
                Event::Start(start) => {
                    let name = element_name(&start);
                    let depth = stack.len();
                    self.on_element(&name, &start, depth, false, source, before, after);
                    stack.push(name);
                }
                Event::Empty(start) => {
                    let name = element_name(&start);
                    let depth = stack.len();
                    self.on_element(&name, &start, depth, true, source, before, after);
                }
                Event::End(_) => {
                    stack.pop();
                }
            }
            if self.diagnostics.has_errors() && self.diagnostics.len() >= MAX_DIAGNOSTICS {
                break;
            }
        }

        if !self.seen_root {
            self.diagnostics
                .error("theme.structure", 1, 1, "theme", "no <theme> root element");
        }
        let Some(version) = self.version else {
            if self.seen_root {
                self.missing("version", 1, 1, "theme");
            }
            return Err(self.diagnostics.into_vec());
        };
        let Some(base) = self.base else {
            if self.seen_root {
                self.missing("base", 1, 1, "theme");
            }
            return Err(self.diagnostics.into_vec());
        };
        if self.diagnostics.has_errors() {
            return Err(self.diagnostics.into_vec());
        }
        let mut document = ThemeDocument {
            version,
            name: self.name,
            base,
            accent: self.accent,
            colors: self.colors,
            canonical: String::new(),
            generation: 0,
        };
        document.canonical = crate::theme::to_xml(&document);
        document.generation = crate::generation_hash(&document.canonical);
        Ok(document)
    }

    fn structural(&mut self, source: &str, offset: usize, message: &str) {
        let (line, column) = position(source, offset);
        self.diagnostics
            .error("theme.structure", line, column, "theme", message);
    }

    fn missing(&mut self, attribute: &str, line: usize, column: usize, path: &str) {
        self.diagnostics.error(
            "theme.missing-attribute",
            line,
            column,
            path,
            format!("`{attribute}` is required"),
        );
    }

    fn unknown_attr(&mut self, element: &str, attribute: &str, line: usize, column: usize) {
        self.diagnostics.error(
            "theme.unknown-attribute",
            line,
            column,
            format!("theme/{element}"),
            format!("`{attribute}` is not an attribute of <{element}>"),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn on_element(
        &mut self,
        name: &str,
        start: &BytesStart<'_>,
        depth: usize,
        _empty: bool,
        source: &str,
        before: usize,
        after: usize,
    ) {
        let (line, column) = element_position(source, before, after);
        let attrs = match read_attrs(start) {
            Ok(attrs) => attrs,
            Err(message) => {
                self.diagnostics
                    .error("theme.syntax", line, column, "theme", message);
                return;
            }
        };
        match depth {
            0 if name == "theme" => {
                if self.seen_root {
                    self.diagnostics.error(
                        "theme.structure",
                        line,
                        column,
                        "theme",
                        "more than one <theme> root element",
                    );
                    return;
                }
                self.seen_root = true;
                self.on_theme(&attrs, line, column);
            }
            1 if self.seen_root && name == "color" => {
                self.on_color(&attrs, line, column);
            }
            _ => {
                self.diagnostics.error(
                    "theme.unknown-element",
                    line,
                    column,
                    format!("theme/{name}"),
                    format!("<{name}> is not part of the theme schema"),
                );
            }
        }
    }

    fn on_theme(&mut self, attrs: &[(String, String)], line: usize, column: usize) {
        for (name, value) in attrs {
            match name.as_str() {
                "version" => {
                    let parsed = value.trim().parse::<u32>().ok();
                    match parsed {
                        Some(1) => self.version = Some(1),
                        Some(other) => self.diagnostics.error(
                            "theme.version",
                            line,
                            column,
                            "theme",
                            format!("version {other} is not supported; expected 1"),
                        ),
                        None => self.diagnostics.error(
                            "theme.version",
                            line,
                            column,
                            "theme",
                            format!("`{value}` is not a version number"),
                        ),
                    }
                }
                "name" => {
                    let name = value.trim();
                    if !name.is_empty() {
                        self.name = Some(name.to_string());
                    }
                }
                "base" => {
                    let base = value.trim();
                    match ThemeBase::from_name(base) {
                        Some(base) => self.base = Some(base),
                        None => self.diagnostics.error(
                            "theme.base.value",
                            line,
                            column,
                            "theme",
                            format!("`{base}` is not a base; expected `light` or `dark`"),
                        ),
                    }
                }
                "accent" => match parse_literal(value) {
                    Some(color) => self.accent = Some(color),
                    None => self.diagnostics.error(
                        "theme.color.value",
                        line,
                        column,
                        "theme",
                        format!("`{value}` is not a colour literal"),
                    ),
                },
                _ => self.unknown_attr("theme", name, line, column),
            }
        }
    }

    fn on_color(&mut self, attrs: &[(String, String)], line: usize, column: usize) {
        let mut name: Option<&str> = None;
        let mut value: Option<&str> = None;
        for (attribute, raw) in attrs {
            match attribute.as_str() {
                "name" => name = Some(raw.as_str()),
                "value" => value = Some(raw.as_str()),
                _ => self.unknown_attr("color", attribute, line, column),
            }
        }
        let Some(name) = name.map(str::trim) else {
            self.missing("name", line, column, "theme/color");
            return;
        };
        if name.is_empty() {
            self.missing("name", line, column, "theme/color");
            return;
        }
        let Some(raw) = value else {
            self.missing("value", line, column, "theme/color");
            return;
        };
        if crate::model::palette_token_for_name(name).is_none() {
            self.diagnostics.error(
                "theme.color.unknown",
                line,
                column,
                format!("theme/color/{name}"),
                format!("`{name}` is not a palette token"),
            );
            return;
        }
        if self.colors.iter().any(|color| color.name == name) {
            self.diagnostics.error(
                "theme.color.duplicate",
                line,
                column,
                format!("theme/color/{name}"),
                format!("`{name}` is declared more than once"),
            );
            return;
        }
        let Some(color) = parse_literal(raw) else {
            self.diagnostics.error(
                "theme.color.value",
                line,
                column,
                format!("theme/color/{name}"),
                format!("`{raw}` is not a colour literal"),
            );
            return;
        };
        if self.colors.len() >= MAX_COLORS {
            self.diagnostics.error(
                "theme.structure",
                line,
                column,
                "theme",
                format!("more than {MAX_COLORS} <color> overrides"),
            );
            return;
        }
        self.colors.push(ThemeColor {
            name: name.to_string(),
            color,
            line,
            column,
        });
    }
}

/// A literal colour from the shared grammar; a palette token is not a literal
/// in a theme document (the document *defines* the palette), and the alpha byte
/// is dropped (palette colours are opaque).
fn parse_literal(raw: &str) -> Option<crate::color::Color> {
    crate::skin::parse_color(raw.trim()).map(|rgba| rgba.color)
}

fn element_name(start: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(start.name().as_ref()).into_owned()
}

fn read_attrs(start: &BytesStart<'_>) -> Result<Vec<(String, String)>, String> {
    let mut attrs: Vec<(String, String)> = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| error.to_string())?;
        let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
        if attrs.iter().any(|(existing, _)| *existing == key) {
            return Err(format!("duplicate attribute `{key}`"));
        }
        let value = attribute
            .normalized_value(quick_xml::XmlVersion::Explicit1_0)
            .map_err(|error| error.to_string())?
            .into_owned();
        attrs.push((key, value));
    }
    Ok(attrs)
}

/// 1-based line/column of a byte offset.
fn position(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let mut line = 1;
    let mut column = 1;
    for (index, ch) in source.char_indices() {
        if index >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn element_position(source: &str, before: usize, after: usize) -> (usize, usize) {
    let end = after.min(source.len());
    let start = before.min(end);
    let span = &source[start..end];
    let offset = span.find('<').map(|index| start + index).unwrap_or(start);
    position(source, offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> ThemeDocument {
        ThemeDocument::parse(text).unwrap_or_else(|diagnostics| {
            panic!(
                "expected a valid theme: {:?}",
                diagnostics
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>()
            )
        })
    }

    fn codes(text: &str) -> Vec<&'static str> {
        ThemeDocument::parse(text)
            .expect_err("expected a rejected theme")
            .iter()
            .map(|item| item.code)
            .collect()
    }

    const NIGHT: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<theme version="1" name="夜蓝" base="dark" accent="#7aa2f7">
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
</theme>"##;

    #[test]
    fn a_theme_round_trips_through_the_canonical_text() {
        let document = parse(NIGHT);
        assert_eq!(document.version(), 1);
        assert_eq!(document.name(), Some("夜蓝"));
        assert_eq!(document.base(), ThemeBase::Dark);
        assert_eq!(
            document.accent(),
            Some(crate::color::Color::new(0x7a, 0xa2, 0xf7))
        );
        assert_eq!(
            document.color("background"),
            Some(crate::color::Color::new(0x16, 0x18, 0x1d))
        );
        let xml = document.to_xml();
        assert!(xml.contains("<theme version=\"1\" name=\"夜蓝\" base=\"dark\" accent=\"#7aa2f7\">"), "{xml}");
        let again = parse(&xml);
        assert_eq!(xml, again.to_xml());
        assert_eq!(document.generation(), again.generation());
    }

    #[test]
    fn a_minimal_theme_has_no_overrides() {
        let document = parse(r#"<theme version="1" base="light"/>"#);
        assert_eq!(document.base(), ThemeBase::Light);
        assert_eq!(document.accent(), None);
        assert!(document.colors().is_empty());
        assert_eq!(document.to_xml(), "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<theme version=\"1\" base=\"light\"/>\n");
    }

    #[test]
    fn diagnostics_are_stable_and_specific() {
        assert!(codes(r#"<layouts version="1"/>"#).contains(&"theme.unknown-element"));
        assert!(codes(r#"<theme base="dark"/>"#).contains(&"theme.missing-attribute"));
        assert!(codes(r#"<theme version="2" base="dark"/>"#).contains(&"theme.version"));
        assert!(codes(r#"<theme version="1"/>"#).contains(&"theme.missing-attribute"));
        assert!(codes(r#"<theme version="1" base="auto"/>"#).contains(&"theme.base.value"));
        assert!(codes(r#"<theme version="1" base="dark" accent="blue"/>"#).contains(&"theme.color.value"));
        assert!(codes(r#"<theme version="1" base="dark" extra="1"/>"#).contains(&"theme.unknown-attribute"));
        assert!(codes(
            r##"<theme version="1" base="dark"><color name="nope" value="#fff"/></theme>"##
        )
        .contains(&"theme.color.unknown"));
        assert!(codes(
            r#"<theme version="1" base="dark"><color name="text" value="nope"/></theme>"#
        )
        .contains(&"theme.color.value"));
        assert!(codes(
            r##"<theme version="1" base="dark"><color name="text" value="#fff"/><color name="text" value="#000"/></theme>"##
        )
        .contains(&"theme.color.duplicate"));
        assert!(codes(
            r#"<theme version="1" base="dark"><color name="text"/></theme>"#
        )
        .contains(&"theme.missing-attribute"));
        assert!(codes(
            r#"<theme version="1" base="dark"><group/></theme>"#
        )
        .contains(&"theme.unknown-element"));
        assert!(codes(
            r#"<theme version="1" base="dark">text</theme>"#
        )
        .contains(&"theme.structure"));
        // A palette token is not a literal: the document defines the palette.
        assert!(codes(
            r#"<theme version="1" base="dark"><color name="card" value="accent"/></theme>"#
        )
        .contains(&"theme.color.value"));
        // Alpha is accepted by the grammar and dropped.
        let translucent = parse(
            r##"<theme version="1" base="dark"><color name="text" value="#11223344"/></theme>"##,
        );
        assert_eq!(
            translucent.color("text"),
            Some(crate::color::Color::new(0x11, 0x22, 0x33))
        );
    }
}
