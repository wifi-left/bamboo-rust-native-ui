// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The normalized writer (M7 §11).
//!
//! `to_xml` is the canonical model text: the generation hash is computed over
//! it and the debug form saves it. Two saves of the same model are
//! byte-identical, and `parse(to_xml(parse(x)))` equals `parse(x)`.

use crate::a11y::A11yRole;
use crate::event::Cursor;
use crate::expr;
use crate::model::{
    BackgroundAttr, ContentAttr, ContentHeight, Horizontal, LayoutDocument, LayoutEntry,
    RadiusAttr, ScrollEntry, Vertical,
};
use crate::skin::SkinElement;

const INDENT: &str = "  ";
const LINE_BUDGET: usize = 100;

impl LayoutDocument {
    /// The canonical model text (byte-identical between two saves).
    pub fn to_xml(&self) -> String {
        self.canonical.clone()
    }
}

pub(crate) fn to_xml(document: &LayoutDocument) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    out.push_str("<layouts version=\"1\">\n");
    for (name, source) in document.constants.sources() {
        out.push_str(INDENT);
        out.push_str("<const name=\"");
        out.push_str(&escape(name));
        out.push_str("\" value=\"");
        out.push_str(&escape(source));
        out.push_str("\"/>\n");
    }
    for variant in &document.variants {
        out.push_str(INDENT);
        out.push_str("<layout id=\"");
        out.push_str(&escape(&variant.id));
        out.push('"');
        if variant.min_width != 0.0 {
            out.push_str(" min-width=\"");
            out.push_str(&expr::number(variant.min_width as f64));
            out.push('"');
        }
        if variant.min_height != 0.0 {
            out.push_str(" min-height=\"");
            out.push_str(&expr::number(variant.min_height as f64));
            out.push('"');
        }
        out.push_str(">\n");
        for scroll in &variant.scrolls {
            write_scroll(&mut out, scroll);
        }
        for entry in &variant.entries {
            write_widget(&mut out, entry);
        }
        for binding in &variant.rust_bindings {
            out.push_str(INDENT);
            out.push_str(INDENT);
            out.push_str("<rust target=\"");
            out.push_str(&escape(&binding.target));
            out.push_str("\" code=\"");
            out.push_str(&escape(&binding.code));
            out.push_str("\"/>\n");
        }
        out.push_str(INDENT);
        out.push_str("</layout>\n");
    }
    out.push_str("</layouts>\n");
    out
}

fn write_widget(out: &mut String, entry: &LayoutEntry) {
    let mut geometry: Vec<(u8, &'static str, String)> = Vec::new();
    match &entry.geometry.horizontal {
        Horizontal::LeftWidth { x, w } => {
            geometry.push((0, "x", expr::to_source(x)));
            geometry.push((4, "w", expr::to_source(w)));
        }
        Horizontal::RightWidth { right, w } => {
            geometry.push((1, "right", expr::to_source(right)));
            geometry.push((4, "w", expr::to_source(w)));
        }
        Horizontal::LeftRight { x, right } => {
            geometry.push((0, "x", expr::to_source(x)));
            geometry.push((1, "right", expr::to_source(right)));
        }
    }
    match &entry.geometry.vertical {
        Vertical::TopHeight { y, h } => {
            geometry.push((2, "y", expr::to_source(y)));
            geometry.push((5, "h", expr::to_source(h)));
        }
        Vertical::BottomHeight { bottom, h } => {
            geometry.push((3, "bottom", expr::to_source(bottom)));
            geometry.push((5, "h", expr::to_source(h)));
        }
        Vertical::TopBottom { y, bottom } => {
            geometry.push((2, "y", expr::to_source(y)));
            geometry.push((3, "bottom", expr::to_source(bottom)));
        }
    }
    geometry.sort_by_key(|(rank, ..)| *rank);
    let geometry: Vec<(&'static str, String)> = geometry
        .into_iter()
        .map(|(_, name, value)| (name, value))
        .collect();

    let mut props: Vec<(&'static str, String)> = Vec::new();
    if let Some(value) = entry.props.visible {
        props.push(("visible", boolean(value)));
    }
    if let Some(value) = entry.props.enabled {
        props.push(("enabled", boolean(value)));
    }
    if let Some(value) = entry.props.interactive {
        props.push(("interactive", boolean(value)));
    }
    if let Some(value) = entry.props.focusable {
        props.push(("focusable", boolean(value)));
    }
    if let Some(value) = entry.props.focus_order {
        props.push(("focus-order", value.to_string()));
    }
    if let Some(value) = entry.props.read_only {
        props.push(("read-only", boolean(value)));
    }
    if let Some(value) = entry.props.tooltip.as_ref() {
        props.push(("tooltip", value.clone()));
    }
    if let Some(value) = entry.props.name.as_ref() {
        props.push(("name", value.clone()));
    }
    if let Some(value) = entry.props.description.as_ref() {
        props.push(("description", value.clone()));
    }
    if let Some(value) = entry.props.automation_id.as_ref() {
        props.push(("automation-id", value.clone()));
    }
    if let Some(value) = entry.props.role {
        props.push(("role", role_token(value).to_string()));
    }
    if let Some(value) = entry.props.expose {
        props.push(("expose", boolean(value)));
    }
    if let Some(value) = entry.props.cursor {
        props.push(("cursor", cursor_token(value).to_string()));
    }

    let mut style: Vec<(&'static str, String)> = Vec::new();
    if let Some(value) = entry.style.radius {
        style.push(("radius", radius_source(value)));
    }
    if let Some(value) = entry.style.corners {
        style.push((
            "corners",
            format!(
                "{} {} {} {}",
                expr::number(value.top_left as f64),
                expr::number(value.top_right as f64),
                expr::number(value.bottom_right as f64),
                expr::number(value.bottom_left as f64)
            ),
        ));
    }
    if let Some(value) = entry.style.background {
        style.push(("background", background_source(value)));
    }
    if let Some(value) = entry.style.font_size {
        style.push(("font-size", expr::number(value as f64)));
    }
    if let Some(value) = entry.style.font_family.as_ref() {
        style.push(("font-family", value.clone()));
    }

    let mut content: Vec<(&'static str, String)> = entry
        .content
        .iter()
        .map(|item| (item.attribute_name(), content_source(item)))
        .collect();
    content.sort_by_key(|(name, _)| content_rank(name));

    // Inline-Rust scalar values are schema attributes, so they are written with
    // the shared vocabulary (sorted) rather than with the widget-local `x-*`
    // block; the parser only accepts them there.
    let mut rust: Vec<(&'static str, String)> = entry
        .rust_attrs
        .iter()
        .map(|(name, code)| (*name, format!("rust:{code}")))
        .collect();
    rust.sort_by_key(|(name, _)| *name);

    // Widget-declared local attributes come last, sorted by name; child
    // elements follow the attributes in source order (§11).
    let mut local: Vec<(String, String)> = entry.skin_attrs.clone();
    local.sort_by(|left, right| left.0.cmp(&right.0));

    // A created widget carries its registry type, command and scroll parent
    // right after the id, before any geometry (§17).
    let mut kind: Vec<(&'static str, String)> = Vec::new();
    if let Some(type_name) = entry.type_name.as_ref() {
        kind.push(("type", type_name.clone()));
    }
    if let Some(command) = entry.command {
        kind.push(("command", command.to_string()));
    }
    if let Some(parent) = entry.parent {
        kind.push(("parent", parent.name().to_string()));
    }

    let groups = vec![
        vec![("id", entry.id.name().to_string())],
        kind,
        geometry,
        props,
        style,
        content,
        rust,
    ];
    write_tag(
        out,
        "widget",
        &groups,
        &local,
        &entry.skin_children,
        &format!("{INDENT}{INDENT}"),
    );
}

fn write_scroll(out: &mut String, entry: &ScrollEntry) {
    let mut geometry: Vec<(u8, &'static str, String)> = Vec::new();
    match &entry.geometry.horizontal {
        Horizontal::LeftWidth { x, w } => {
            geometry.push((0, "x", expr::to_source(x)));
            geometry.push((4, "w", expr::to_source(w)));
        }
        Horizontal::RightWidth { right, w } => {
            geometry.push((1, "right", expr::to_source(right)));
            geometry.push((4, "w", expr::to_source(w)));
        }
        Horizontal::LeftRight { x, right } => {
            geometry.push((0, "x", expr::to_source(x)));
            geometry.push((1, "right", expr::to_source(right)));
        }
    }
    match &entry.geometry.vertical {
        Vertical::TopHeight { y, h } => {
            geometry.push((2, "y", expr::to_source(y)));
            geometry.push((5, "h", expr::to_source(h)));
        }
        Vertical::BottomHeight { bottom, h } => {
            geometry.push((3, "bottom", expr::to_source(bottom)));
            geometry.push((5, "h", expr::to_source(h)));
        }
        Vertical::TopBottom { y, bottom } => {
            geometry.push((2, "y", expr::to_source(y)));
            geometry.push((3, "bottom", expr::to_source(bottom)));
        }
    }
    geometry.sort_by_key(|(rank, ..)| *rank);
    let geometry: Vec<(&'static str, String)> = geometry
        .into_iter()
        .map(|(_, name, value)| (name, value))
        .collect();

    let content = match entry.content_height() {
        ContentHeight::Keep => Vec::new(),
        ContentHeight::Auto => vec![("content-height", "auto".to_string())],
        ContentHeight::Exact(expression) => {
            vec![("content-height", expr::to_source(expression))]
        }
    };

    let groups = vec![
        vec![("id", entry.id.name().to_string())],
        geometry,
        content,
    ];
    write_tag(out, "scroll", &groups, &[], &[], &format!("{INDENT}{INDENT}"));
}

/// Write one element: the shared attribute groups, then widget-declared local
/// attributes, then child elements (which make it non-self-closing).
fn write_tag(
    out: &mut String,
    tag: &str,
    groups: &[Vec<(&'static str, String)>],
    local: &[(String, String)],
    children: &[SkinElement],
    indent: &str,
) {
    let mut flat = String::new();
    for (name, value) in groups.iter().flatten() {
        flat.push(' ');
        flat.push_str(name);
        flat.push_str("=\"");
        flat.push_str(&escape(value));
        flat.push('"');
    }
    for (name, value) in local {
        flat.push(' ');
        flat.push_str(name);
        flat.push_str("=\"");
        flat.push_str(&escape(value));
        flat.push('"');
    }
    let ending = if children.is_empty() { "/>" } else { ">" };
    let one_line = format!("{indent}<{tag}{flat}{ending}");
    if one_line.chars().count() <= LINE_BUDGET {
        out.push_str(&one_line);
        out.push('\n');
        write_children(out, tag, children, indent);
        return;
    }

    let pad = " ".repeat(indent.chars().count() + tag.chars().count() + 2);
    out.push_str(indent);
    out.push('<');
    out.push_str(tag);
    let mut first = true;
    for group in groups.iter().filter(|group| !group.is_empty()) {
        if first {
            out.push(' ');
            first = false;
        } else {
            out.push('\n');
            out.push_str(&pad);
        }
        let mut separator = "";
        for (name, value) in group {
            out.push_str(separator);
            separator = " ";
            out.push_str(name);
            out.push_str("=\"");
            out.push_str(&escape(value));
            out.push('"');
        }
    }
    if !local.is_empty() {
        if first {
            out.push(' ');
        } else {
            out.push('\n');
            out.push_str(&pad);
        }
        let mut separator = "";
        for (name, value) in local {
            out.push_str(separator);
            separator = " ";
            out.push_str(name);
            out.push_str("=\"");
            out.push_str(&escape(value));
            out.push('"');
        }
    }
    out.push_str(ending);
    out.push('\n');
    write_children(out, tag, children, indent);
}

fn write_children(out: &mut String, tag: &str, children: &[SkinElement], indent: &str) {
    if children.is_empty() {
        return;
    }
    let child_indent = format!("{indent}{INDENT}");
    for child in children {
        write_skin_element(out, child, &child_indent);
    }
    out.push_str(indent);
    out.push_str("</");
    out.push_str(tag);
    out.push_str(">\n");
}

/// One data child element: attributes in source order, nested children after.
fn write_skin_element(out: &mut String, element: &SkinElement, indent: &str) {
    let mut flat = String::new();
    for (name, value) in &element.attrs {
        flat.push(' ');
        flat.push_str(name);
        flat.push_str("=\"");
        flat.push_str(&escape(value));
        flat.push('"');
    }
    let ending = if element.children.is_empty() { "/>" } else { ">" };
    let one_line = format!("{indent}<{}{flat}{ending}", element.name);
    if one_line.chars().count() <= LINE_BUDGET {
        out.push_str(&one_line);
        out.push('\n');
    } else {
        out.push_str(indent);
        out.push('<');
        out.push_str(&element.name);
        let pad = " ".repeat(indent.chars().count() + element.name.chars().count() + 2);
        for (name, value) in &element.attrs {
            out.push('\n');
            out.push_str(&pad);
            out.push_str(name);
            out.push_str("=\"");
            out.push_str(&escape(value));
            out.push('"');
        }
        out.push_str(ending);
        out.push('\n');
    }
    if element.children.is_empty() {
        return;
    }
    let child_indent = format!("{indent}{INDENT}");
    for child in &element.children {
        write_skin_element(out, child, &child_indent);
    }
    out.push_str(indent);
    out.push_str("</");
    out.push_str(&element.name);
    out.push_str(">\n");
}

fn boolean(value: bool) -> String {
    if value {
        "true".to_string()
    } else {
        "false".to_string()
    }
}

fn radius_source(value: RadiusAttr) -> String {
    match value {
        RadiusAttr::Dips(radius) => expr::number(radius as f64),
        RadiusAttr::Small => "small".to_string(),
        RadiusAttr::Medium => "medium".to_string(),
        RadiusAttr::Large => "large".to_string(),
        RadiusAttr::Pill => "pill".to_string(),
    }
}

fn background_source(value: BackgroundAttr) -> String {
    match value {
        BackgroundAttr::Token(token) => token.to_string(),
        BackgroundAttr::Literal(color) => {
            format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
        }
        BackgroundAttr::None => "none".to_string(),
    }
}

fn content_source(value: &ContentAttr) -> String {
    match value {
        ContentAttr::Text(text) | ContentAttr::Label(text) => text.clone(),
        ContentAttr::Value(value) | ContentAttr::Progress(value) => expr::number(*value),
        ContentAttr::Number(value) => value.to_string(),
        ContentAttr::Checked(value) => boolean(*value),
        ContentAttr::Selected(value) => value.to_string(),
    }
}

fn content_rank(name: &str) -> u8 {
    match name {
        "text" => 0,
        "label" => 1,
        "value" => 2,
        "progress" => 3,
        "number" => 4,
        "checked" => 5,
        _ => 6,
    }
}

fn role_token(role: A11yRole) -> &'static str {
    match role {
        A11yRole::Button => "button",
        A11yRole::Toggle => "toggle",
        A11yRole::Slider => "slider",
        A11yRole::TextField => "text-field",
        A11yRole::NumberField => "number-field",
        A11yRole::RadioGroup => "radio-group",
        A11yRole::TabList => "tab-list",
        A11yRole::List => "list",
        A11yRole::MultiSelectList => "multi-select-list",
        A11yRole::RadioItem => "radio-item",
        A11yRole::TabItem => "tab-item",
        A11yRole::ListItem => "list-item",
        A11yRole::Tree => "tree",
        A11yRole::TreeItem => "tree-item",
        A11yRole::Grid => "grid",
        A11yRole::GridCell => "grid-cell",
        A11yRole::ColumnHeader => "column-header",
        A11yRole::Heading => "heading",
        A11yRole::Group => "group",
        A11yRole::Separator => "separator",
        A11yRole::Text => "text",
        A11yRole::ComboBox => "combo-box",
        A11yRole::ProgressBar => "progress-bar",
        A11yRole::MenuItem => "menu-item",
        A11yRole::Menu => "menu",
        A11yRole::MenuBar => "menu-bar",
        A11yRole::ToolBar => "tool-bar",
        A11yRole::StatusBar => "status-bar",
    }
}

fn cursor_token(cursor: Cursor) -> &'static str {
    match cursor {
        Cursor::Arrow => "arrow",
        Cursor::Hand => "hand",
        Cursor::IBeam => "i-beam",
        Cursor::SizeWE => "size-we",
        Cursor::SizeNS => "size-ns",
    }
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
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> LayoutDocument {
        LayoutDocument::parse(
            r#"<layouts version="1">
  <const name="pad" value="16"/>
  <layout id="base" min-width="0" min-height="0">
    <scroll id="page" x="pad" y="8" w="cw-2*pad" h="ch-16" content-height="auto"/>
    <widget id="a" x="16" y="16" w="120" h="32"
            label="主 &amp; 操作" tooltip="样式来自文件" name="主操作" expose="false"
            role="button" cursor="hand" radius="medium" background="accent" font-size="18"/>
    <widget id="b" x="cx-22" y="100" w="44" h="44" value="0.5" selected="2"/>
  </layout>
</layouts>"#,
        )
        .expect("valid document")
    }

    #[test]
    fn canonical_text_is_stable() {
        let first = document().to_xml();
        let second = LayoutDocument::parse(&first).expect("round trip").to_xml();
        assert_eq!(first, second);
    }

    #[test]
    fn constants_are_preserved_as_expressions() {
        let xml = document().to_xml();
        assert!(xml.contains("<const name=\"pad\" value=\"16\"/>"), "{xml}");
    }

    #[test]
    fn attribute_order_is_fixed() {
        let text = r#"<layouts version="1"><layout id="base"><widget id="a" radius="medium" label="x" h="32" w="120" y="16" x="16"/></layout></layouts>"#;
        let document = LayoutDocument::parse(text).expect("valid");
        let xml = document.to_xml();
        let line = xml
            .lines()
            .find(|line| line.contains("id=\"a\""))
            .expect("widget a");
        let id = line.find("id=\"a\"").unwrap();
        let x = line.find("x=\"16\"").unwrap();
        let y = line.find("y=\"16\"").unwrap();
        let w = line.find("w=\"120\"").unwrap();
        let h = line.find("h=\"32\"").unwrap();
        let radius = line.find("radius=").unwrap();
        let label = line.find("label=").unwrap();
        assert!(
            id < x && x < y && y < w && w < h && h < radius && radius < label,
            "{line}"
        );
    }

    #[test]
    fn strings_are_escaped() {
        let xml = document().to_xml();
        assert!(xml.contains("label=\"主 &amp; 操作\""), "{xml}");
    }

    #[test]
    fn long_entries_wrap() {
        let text = format!(
            r#"<layouts version="1"><layout id="base"><widget id="a" x="16" y="16" w="120" h="32" label="{}"/></layout></layouts>"#,
            "长".repeat(60)
        );
        let xml = LayoutDocument::parse(&text).expect("valid").to_xml();
        assert!(xml.lines().any(|line| line.trim_start().starts_with("x=\"16\"")));
    }

    #[test]
    fn literal_colors_round_trip() {
        let text = r##"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="1" h="1" background="#abc"/></layout></layouts>"##;
        let document = LayoutDocument::parse(text).expect("valid");
        assert!(document.to_xml().contains("background=\"#aabbcc\""));
        LayoutDocument::parse(&document.to_xml()).expect("round trip");
    }
}
