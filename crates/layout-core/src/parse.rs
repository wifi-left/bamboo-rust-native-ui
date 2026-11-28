// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The `.buixml` reader: XML syntax, M7 schema (D123).
//!
//! Parsing is pure: it produces the full model or a list of diagnostics and
//! mutates nothing else. Application is a separate step (`Ui::apply_layout`),
//! which is what makes a hot reload atomic: a file that does not parse is
//! never half-applied.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::a11y::A11yRole;
use crate::event::Cursor;
use crate::geometry::Corners;
use crate::diag::{Diagnostics, LayoutDiagnostic};
use crate::expr::{self, Expr, Var};
use crate::model::{
    BackgroundAttr, ContentAttr, ContentHeight, Geometry, Horizontal, LayoutConstants,
    LayoutDocument, LayoutEntry, LayoutVariant, PropAttrs, RadiusAttr, RustBinding, ScrollEntry,
    StyleAttrs, Vertical, PALETTE_TOKENS, palette_token_for_name,
};
use crate::skin::SkinElement;
use crate::widget::WidgetId;

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 50_000;
const MAX_SKIN_ELEMENTS: usize = 10_000;
const MAX_DIAGNOSTICS: usize = 100;

impl LayoutDocument {
    /// Parse one `.buixml` document.
    pub fn parse(text: &str) -> Result<Self, Vec<LayoutDiagnostic>> {
        Parser::new(text).run()
    }
}

struct Parser<'a> {
    source: &'a str,
    diagnostics: Diagnostics,
    constants: LayoutConstants,
    variants: Vec<LayoutVariant>,
    current: Option<usize>,
    entries: usize,
    saw_root: bool,
    saw_layout: bool,
    /// Variant index of the `<widget>` whose child elements are being read.
    skin_parent: Option<usize>,
    /// Open child elements of the current `<widget>` (depth ≥ 3).
    skin_stack: Vec<SkinElement>,
    /// One flag per open depth-≥3 element: whether it is being collected.
    skin_collect: Vec<bool>,
    skin_nodes: usize,
    skin_limit_reported: bool,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            diagnostics: Diagnostics::new(MAX_DIAGNOSTICS),
            constants: LayoutConstants::default(),
            variants: Vec::new(),
            current: None,
            entries: 0,
            saw_root: false,
            saw_layout: false,
            skin_parent: None,
            skin_stack: Vec::new(),
            skin_collect: Vec::new(),
            skin_nodes: 0,
            skin_limit_reported: false,
        }
    }

    fn run(mut self) -> Result<LayoutDocument, Vec<LayoutDiagnostic>> {
        if self.source.len() > MAX_BYTES {
            let mut diagnostics = Diagnostics::new(MAX_DIAGNOSTICS);
            diagnostics.error(
                "xml.structure",
                1,
                1,
                String::new(),
                format!("the file is larger than {MAX_BYTES} bytes"),
            );
            return Err(diagnostics.into_vec());
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
                        "xml.syntax",
                        line,
                        column,
                        String::new(),
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
                    // `trim_text(true)` drops whitespace-only nodes, so any
                    // remaining text is real content — which the schema forbids.
                    let (line, column) = position(source, before);
                    self.diagnostics.error(
                        "xml.structure",
                        line,
                        column,
                        String::new(),
                        "text nodes are not allowed; only whitespace is ignored",
                    );
                }
                Event::CData(_) => self.structural(source, before, "CDATA sections are not allowed"),
                Event::GeneralRef(_) => {
                    self.structural(source, before, "character/entity references in content are not allowed")
                }
                Event::PI(_) => self.structural(source, before, "processing instructions are not allowed"),
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
                Event::End(end) => {
                    let name = String::from_utf8_lossy(end.name().as_ref()).into_owned();
                    let depth = stack.len().saturating_sub(1);
                    if depth >= 3 {
                        if self.skin_collect.pop() == Some(true) {
                            if let Some(node) = self.skin_stack.pop() {
                                self.attach_skin(node);
                            }
                        }
                    } else if depth == 2 && name == "widget" {
                        self.skin_parent = None;
                    } else if name == "layout" {
                        self.current = None;
                    }
                    stack.pop();
                }
            }
            if self.diagnostics.has_errors() && self.diagnostics.len() >= MAX_DIAGNOSTICS {
                break;
            }
        }

        self.validate();
        if self.diagnostics.has_errors() {
            return Err(self.diagnostics.into_vec());
        }
        if self.variants.is_empty() {
            self.diagnostics
                .error("xml.structure", 1, 1, String::new(), "no <layout> variant");
            return Err(self.diagnostics.into_vec());
        }

        let mut document = LayoutDocument {
            generation: 0,
            canonical: String::new(),
            constants: self.constants,
            variants: self.variants,
        };
        document.canonical = crate::write::to_xml(&document);
        document.generation = crate::generation_hash(&document.canonical);
        Ok(document)
    }

    fn structural(&mut self, source: &str, offset: usize, message: &str) {
        let (line, column) = position(source, offset);
        self.diagnostics
            .error("xml.structure", line, column, String::new(), message);
    }

    #[allow(clippy::too_many_arguments)]
    fn on_element(
        &mut self,
        name: &str,
        start: &BytesStart<'_>,
        depth: usize,
        empty: bool,
        source: &str,
        before: usize,
        after: usize,
    ) {
        let (line, column) = element_position(source, before, after);
        let attrs = match read_attrs(start) {
            Ok(attrs) => attrs,
            Err(message) => {
                self.diagnostics
                    .error("xml.syntax", line, column, String::new(), message);
                return;
            }
        };
        match depth {
            0 => {
                if name != "layouts" {
                    self.unknown_element(name, line, column, String::new());
                    return;
                }
                self.on_layouts(&attrs, line, column);
            }
            1 => {
                if !self.saw_root {
                    self.diagnostics.error(
                        "xml.structure",
                        line,
                        column,
                        String::new(),
                        format!("<{name}> is not inside <layouts>"),
                    );
                    return;
                }
                match name {
                    "const" => self.on_const(&attrs, line, column),
                    "layout" => {
                        self.on_layout(&attrs, line, column);
                        if !empty {
                            self.current = Some(self.variants.len() - 1);
                        }
                    }
                    _ => self.unknown_element(name, line, column, String::new()),
                }
            }
            2 => {
                if self.current.is_none() {
                    self.diagnostics.error(
                        "xml.structure",
                        line,
                        column,
                        String::new(),
                        format!("<{name}> must be a child of <layout>"),
                    );
                    return;
                }
                match name {
                    "widget" => {
                        if self.on_widget(&attrs, line, column) && !empty {
                            self.skin_parent = self.current;
                        }
                    }
                    "scroll" => self.on_scroll(&attrs, line, column),
                    "rust" => self.on_rust(&attrs, empty, line, column),
                    _ => self.unknown_element(name, line, column, String::new()),
                }
            }
            _ => self.on_skin_element(name, &attrs, depth, empty, line, column),
        }
    }

    /// A child element of a `<widget>`: data for the widget-declared skin
    /// protocol (§6.3). Nothing here is schema-checked — the applier validates
    /// names and values against the target widget's declarations.
    fn on_skin_element(
        &mut self,
        name: &str,
        attrs: &[(String, String)],
        depth: usize,
        empty: bool,
        line: usize,
        column: usize,
    ) {
        let collects = self.skin_parent.is_some()
            && (depth == 3 || self.skin_collect.last() == Some(&true));
        if !collects {
            self.unknown_element(name, line, column, String::new());
            if !empty {
                self.skin_collect.push(false);
            }
            return;
        }
        if self.skin_nodes >= MAX_SKIN_ELEMENTS {
            if !self.skin_limit_reported {
                self.skin_limit_reported = true;
                self.diagnostics.error(
                    "xml.structure",
                    line,
                    column,
                    String::new(),
                    format!("a widget has more than {MAX_SKIN_ELEMENTS} child elements"),
                );
            }
            self.skin_parent = None;
            if !empty {
                self.skin_collect.push(false);
            }
            return;
        }
        self.skin_nodes += 1;
        let mut node = SkinElement::new(name, line, column);
        node.attrs = attrs.to_vec();
        if empty {
            self.attach_skin(node);
        } else {
            self.skin_stack.push(node);
            self.skin_collect.push(true);
        }
    }

    /// Hand a finished child element to its parent element or to the widget
    /// entry's child list.
    fn attach_skin(&mut self, node: SkinElement) {
        if let Some(parent) = self.skin_stack.last_mut() {
            parent.children.push(node);
            return;
        }
        if let Some(variant) = self.skin_parent {
            if let Some(entry) = self.variants[variant].entries.last_mut() {
                entry.skin_children.push(node);
            }
        }
    }

    fn unknown_element(&mut self, name: &str, line: usize, column: usize, path: String) {
        self.diagnostics.error(
            "xml.unknown-element",
            line,
            column,
            path,
            format!("<{name}> is not part of the schema"),
        );
    }

    fn unknown_attr(
        &mut self,
        element: &str,
        name: &str,
        line: usize,
        column: usize,
        path: &str,
    ) {
        self.diagnostics.error(
            "xml.unknown-attribute",
            line,
            column,
            path,
            format!("`{name}` is not an attribute of <{element}>"),
        );
    }

    fn missing(&mut self, element: &str, name: &str, line: usize, column: usize, path: &str) {
        self.diagnostics.error(
            "xml.missing-attribute",
            line,
            column,
            path,
            format!("<{element}> requires `{name}`"),
        );
    }

    fn on_layouts(&mut self, attrs: &[(String, String)], line: usize, column: usize) {
        self.saw_root = true;
        let mut version = None;
        for (name, value) in attrs {
            match name.as_str() {
                "version" => version = Some(value.clone()),
                _ => self.unknown_attr("layouts", name, line, column, "layouts"),
            }
        }
        match version.as_deref() {
            Some("1") => {}
            Some(other) => self.diagnostics.error(
                "xml.version",
                line,
                column,
                "layouts",
                format!("unsupported version `{other}`; expected `1`"),
            ),
            None => self.diagnostics.error(
                "xml.version",
                line,
                column,
                "layouts",
                "missing `version`; expected `1`",
            ),
        }
    }

    fn on_const(&mut self, attrs: &[(String, String)], line: usize, column: usize) {
        if self.saw_layout {
            self.diagnostics.error(
                "xml.structure",
                line,
                column,
                "const",
                "<const> must come before the first <layout>",
            );
            return;
        }
        let mut name_attr = None;
        let mut value_attr = None;
        for (name, value) in attrs {
            match name.as_str() {
                "name" => name_attr = Some(value.clone()),
                "value" => value_attr = Some(value.clone()),
                _ => self.unknown_attr("const", name, line, column, "const"),
            }
        }
        let Some(name) = name_attr else {
            self.missing("const", "name", line, column, "const");
            return;
        };
        let path = format!("const/{name}");
        if !valid_name(&name) {
            self.diagnostics.error(
                "xml.const.invalid",
                line,
                column,
                path,
                "a constant name must match `[a-z][a-z0-9_]*`",
            );
            return;
        }
        if Var::is_reserved(&name) {
            self.diagnostics.error(
                "xml.const.reserved",
                line,
                column,
                path,
                format!("`{name}` is a space variable"),
            );
            return;
        }
        if self.constants.get(&name).is_some() {
            self.diagnostics.error(
                "xml.const.duplicate",
                line,
                column,
                path,
                format!("constant `{name}` is already declared"),
            );
            return;
        }
        let Some(value_text) = value_attr else {
            self.missing("const", "value", line, column, "const");
            return;
        };
        let expression = match expr::parse_expr(&value_text, &self.constants) {
            Ok(expression) => expression,
            Err(error) => {
                self.diagnostics
                    .error(error.code, line, column, path, error.message);
                return;
            }
        };
        match expr::eval_constant(&expression, &self.constants) {
            Ok(value) => {
                self.constants
                    .insert(name, value, expr::to_source(&expression));
            }
            Err(error) => self
                .diagnostics
                .error(error.code, line, column, path, error.message),
        }
    }

    fn on_layout(&mut self, attrs: &[(String, String)], line: usize, column: usize) {
        self.saw_layout = true;
        let mut id = None;
        let mut min_width = 0.0;
        let mut min_height = 0.0;
        for (name, value) in attrs {
            match name.as_str() {
                "id" => id = Some(value.clone()),
                "min-width" => {
                    if let Some(number) = self.non_negative(value, "min-width", line, column) {
                        min_width = number;
                    }
                }
                "min-height" => {
                    if let Some(number) = self.non_negative(value, "min-height", line, column) {
                        min_height = number;
                    }
                }
                _ => self.unknown_attr("layout", name, line, column, "layout"),
            }
        }
        let Some(id) = id else {
            self.missing("layout", "id", line, column, "layout");
            // Keep the variant count in step so `current` stays meaningful.
            self.variants.push(LayoutVariant {
                id: String::new(),
                min_width,
                min_height,
                entries: Vec::new(),
                scrolls: Vec::new(),
                rust_bindings: Vec::new(),
            });
            return;
        };
        self.variants.push(LayoutVariant {
            id,
            min_width,
            min_height,
            entries: Vec::new(),
            scrolls: Vec::new(),
            rust_bindings: Vec::new(),
        });
    }

    fn on_widget(&mut self, attrs: &[(String, String)], line: usize, column: usize) -> bool {
        let id = attrs
            .iter()
            .find(|(name, _)| name == "id")
            .map(|(_, value)| value.clone());
        let Some(id) = id else {
            self.missing("widget", "id", line, column, "layout");
            return false;
        };
        if id.is_empty() {
            self.missing("widget", "id", line, column, "layout");
            return false;
        }
        let variant_id = self
            .current
            .and_then(|index| self.variants.get(index))
            .map(|variant| variant.id.clone())
            .unwrap_or_default();
        let path = format!("layout/{variant_id}/{id}");

        let mut x = None;
        let mut right = None;
        let mut w = None;
        let mut y = None;
        let mut bottom = None;
        let mut h = None;
        let mut type_name: Option<String> = None;
        let mut command: Option<u32> = None;
        let mut parent: Option<WidgetId> = None;
        let mut props = PropAttrs::default();
        let mut style = StyleAttrs::default();
        let mut content: Vec<ContentAttr> = Vec::new();
        let mut skin_attrs: Vec<(String, String)> = Vec::new();
        let mut rust_attrs: Vec<(&'static str, String)> = Vec::new();

        for (name, value) in attrs {
            // Inline Rust is stored verbatim for the compiled mode; the dynamic
            // applier rejects it. Geometry values reach `Expr::Rust` through
            // `expr_attr` instead.
            if let Some(code) = value.strip_prefix("rust:") {
                if let Some(scalar) = crate::model::scalar_attr_name(name) {
                    let code = code.trim();
                    if code.is_empty() {
                        self.diagnostics.error(
                            "xml.attr.value",
                            line,
                            column,
                            &path,
                            format!("`{name}` has an empty inline-Rust value"),
                        );
                    } else {
                        rust_attrs.push((scalar, code.to_string()));
                    }
                    continue;
                }
            }
            match name.as_str() {
                "id" => {}
                "type" => {
                    if valid_type_name(value) {
                        type_name = Some(value.clone());
                    } else {
                        self.diagnostics.error(
                            "xml.attr.value",
                            line,
                            column,
                            &path,
                            format!(
                                "`{value}` is not a type name; expected `[a-z][a-z0-9_.-]*`"
                            ),
                        );
                    }
                }
                "command" => {
                    command = self.uint_attr(value, "command", line, column, &path);
                }
                "parent" => {
                    let name = value.trim();
                    if name.is_empty() {
                        self.diagnostics.error(
                            "xml.attr.value",
                            line,
                            column,
                            &path,
                            "`parent` must name a scroll panel",
                        );
                    } else if name == id {
                        self.diagnostics.error(
                            "xml.attr.value",
                            line,
                            column,
                            &path,
                            "a widget cannot be its own parent",
                        );
                    } else {
                        parent = Some(WidgetId::intern(name));
                    }
                }
                "x" => x = self.expr_attr(value, "x", line, column, &path),
                "right" => right = self.expr_attr(value, "right", line, column, &path),
                "w" => w = self.expr_attr(value, "w", line, column, &path),
                "y" => y = self.expr_attr(value, "y", line, column, &path),
                "bottom" => bottom = self.expr_attr(value, "bottom", line, column, &path),
                "h" => h = self.expr_attr(value, "h", line, column, &path),
                "visible" => props.visible = self.bool_attr(value, "visible", line, column, &path),
                "enabled" => props.enabled = self.bool_attr(value, "enabled", line, column, &path),
                "interactive" => {
                    props.interactive = self.bool_attr(value, "interactive", line, column, &path)
                }
                "focusable" => {
                    props.focusable = self.bool_attr(value, "focusable", line, column, &path)
                }
                "focus-order" => {
                    props.focus_order = self.int_attr(value, "focus-order", line, column, &path)
                }
                "read-only" => {
                    props.read_only = self.bool_attr(value, "read-only", line, column, &path)
                }
                "tooltip" => props.tooltip = Some(value.clone()),
                "name" => props.name = Some(value.clone()),
                "description" => props.description = Some(value.clone()),
                "automation-id" => props.automation_id = Some(value.clone()),
                "role" => props.role = self.role_attr(value, line, column, &path),
                "expose" => props.expose = self.bool_attr(value, "expose", line, column, &path),
                "cursor" => props.cursor = self.cursor_attr(value, line, column, &path),
                "radius" => style.radius = self.radius_attr(value, line, column, &path),
                "corners" => style.corners = self.corners_attr(value, line, column, &path),
                "background" => {
                    style.background = self.background_attr(value, line, column, &path)
                }
                "font-size" => {
                    style.font_size = self.positive_attr(value, "font-size", line, column, &path)
                }
                "font-family" => {
                    if value.trim().is_empty() {
                        self.diagnostics.error(
                            "xml.attr.value",
                            line,
                            column,
                            &path,
                            "`font-family` must not be empty",
                        );
                    } else {
                        style.font_family = Some(value.clone());
                    }
                }
                "text" => self.add_content(
                    &mut content,
                    ContentAttr::Text(value.clone()),
                    line,
                    column,
                    &path,
                ),
                "label" => self.add_content(
                    &mut content,
                    ContentAttr::Label(value.clone()),
                    line,
                    column,
                    &path,
                ),
                "value" => {
                    if let Some(number) = self.number_attr(value, "value", line, column, &path) {
                        self.add_content(
                            &mut content,
                            ContentAttr::Value(number),
                            line,
                            column,
                            &path,
                        );
                    }
                }
                "progress" => {
                    if let Some(number) = self.number_attr(value, "progress", line, column, &path)
                    {
                        self.add_content(
                            &mut content,
                            ContentAttr::Progress(number),
                            line,
                            column,
                            &path,
                        );
                    }
                }
                "number" => {
                    if let Some(number) = self.int_attr(value, "number", line, column, &path) {
                        self.add_content(
                            &mut content,
                            ContentAttr::Number(number as i64),
                            line,
                            column,
                            &path,
                        );
                    }
                }
                "checked" => {
                    if let Some(on) = self.bool_attr(value, "checked", line, column, &path) {
                        self.add_content(
                            &mut content,
                            ContentAttr::Checked(on),
                            line,
                            column,
                            &path,
                        );
                    }
                }
                "selected" => {
                    if let Some(index) = self.uint_attr(value, "selected", line, column, &path) {
                        self.add_content(
                            &mut content,
                            ContentAttr::Selected(index),
                            line,
                            column,
                            &path,
                        );
                    }
                }
                // Anything else is a local attribute of the widget-declared
                // skin protocol (§6.2). Its name and value are checked against
                // the target widget's declarations at apply time, where the
                // widget type is known.
                _ => skin_attrs.push((name.clone(), value.clone())),
            }
        }

        if style.radius.is_some() && style.corners.is_some() {
            self.diagnostics.error(
                "xml.attr.conflict",
                line,
                column,
                &path,
                "`radius` and `corners` are mutually exclusive",
            );
        }
        if command.is_some() && type_name.is_none() {
            self.diagnostics.error(
                "xml.attr.conflict",
                line,
                column,
                &path,
                "`command` is only meaningful on a created widget (`type`)",
            );
        }
        let Some(geometry) = self.geometry(x, right, w, y, bottom, h, line, column, &path) else {
            return false;
        };
        let entry = LayoutEntry {
            id: WidgetId::intern(&id),
            line,
            column,
            path,
            type_name,
            command,
            parent,
            geometry,
            props,
            style,
            content,
            skin_attrs,
            skin_children: Vec::new(),
            rust_attrs,
        };
        self.entries += 1;
        if self.entries > MAX_ENTRIES {
            self.diagnostics.error(
                "xml.structure",
                line,
                column,
                String::new(),
                format!("the document has more than {MAX_ENTRIES} entries"),
            );
            return false;
        }
        if let Some(index) = self.current {
            self.variants[index].entries.push(entry);
            return true;
        }
        false
    }

    fn on_scroll(&mut self, attrs: &[(String, String)], line: usize, column: usize) {
        let id = attrs
            .iter()
            .find(|(name, _)| name == "id")
            .map(|(_, value)| value.clone());
        let Some(id) = id else {
            self.missing("scroll", "id", line, column, "layout");
            return;
        };
        let variant_id = self
            .current
            .and_then(|index| self.variants.get(index))
            .map(|variant| variant.id.clone())
            .unwrap_or_default();
        let path = format!("layout/{variant_id}/{id}");

        let mut x = None;
        let mut right = None;
        let mut w = None;
        let mut y = None;
        let mut bottom = None;
        let mut h = None;
        let mut content_height = ContentHeight::Keep;

        for (name, value) in attrs {
            match name.as_str() {
                "id" => {}
                "x" => x = self.expr_attr(value, "x", line, column, &path),
                "right" => right = self.expr_attr(value, "right", line, column, &path),
                "w" => w = self.expr_attr(value, "w", line, column, &path),
                "y" => y = self.expr_attr(value, "y", line, column, &path),
                "bottom" => bottom = self.expr_attr(value, "bottom", line, column, &path),
                "h" => h = self.expr_attr(value, "h", line, column, &path),
                "content-height" => {
                    if value == "auto" {
                        content_height = ContentHeight::Auto;
                    } else if let Some(expression) =
                        self.expr_attr(value, "content-height", line, column, &path)
                    {
                        content_height = ContentHeight::Exact(expression);
                    }
                }
                _ => self.unknown_attr("scroll", name, line, column, &path),
            }
        }

        let Some(geometry) = self.geometry(x, right, w, y, bottom, h, line, column, &path) else {
            return;
        };
        let entry = ScrollEntry {
            id: WidgetId::intern(&id),
            line,
            column,
            path,
            geometry,
            content_height,
        };
        if let Some(index) = self.current {
            self.variants[index].scrolls.push(entry);
        }
    }

    /// A `<rust target="id" code="expr"/>` child-list binding (§16.2). It is a
    /// sibling of `<widget>` so its code can be a plain attribute value; the
    /// target names the widget whose declared child list it replaces.
    fn on_rust(
        &mut self,
        attrs: &[(String, String)],
        empty: bool,
        line: usize,
        column: usize,
    ) {
        let Some(index) = self.current else {
            return;
        };
        if !empty {
            self.diagnostics.error(
                "xml.structure",
                line,
                column,
                "rust",
                "<rust> must be an empty element",
            );
        }
        let mut target = None;
        let mut code = None;
        for (name, value) in attrs {
            match name.as_str() {
                "target" => target = Some(value.clone()),
                "code" => code = Some(value.clone()),
                _ => self.unknown_attr("rust", name, line, column, "rust"),
            }
        }
        let Some(target) = target else {
            self.missing("rust", "target", line, column, "rust");
            return;
        };
        let Some(code) = code else {
            self.missing("rust", "code", line, column, "rust");
            return;
        };
        if target.trim().is_empty() {
            self.diagnostics.error(
                "xml.attr.value",
                line,
                column,
                "rust",
                "`target` must name a widget id",
            );
            return;
        }
        let code = code.trim();
        if code.is_empty() {
            self.diagnostics.error(
                "xml.attr.value",
                line,
                column,
                "rust",
                "`code` must not be empty",
            );
            return;
        }
        let variant_id = self.variants[index].id.clone();
        let path = format!("layout/{variant_id}/{target}");
        self.variants[index].rust_bindings.push(RustBinding {
            target,
            code: code.to_string(),
            line,
            column,
            path,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn geometry(
        &mut self,
        x: Option<Expr>,
        right: Option<Expr>,
        w: Option<Expr>,
        y: Option<Expr>,
        bottom: Option<Expr>,
        h: Option<Expr>,
        line: usize,
        column: usize,
        path: &str,
    ) -> Option<Geometry> {
        let horizontal = match (x, right, w) {
            (Some(x), None, Some(w)) => Horizontal::LeftWidth { x, w },
            (None, Some(right), Some(w)) => Horizontal::RightWidth { right, w },
            (Some(x), Some(right), None) => Horizontal::LeftRight { x, right },
            (Some(_), Some(_), Some(_)) => {
                self.diagnostics.error(
                    "xml.geom.overdetermined",
                    line,
                    column,
                    path,
                    "give exactly two of `x`, `right`, `w`",
                );
                return None;
            }
            _ => {
                self.diagnostics.error(
                    "xml.geom.underdetermined",
                    line,
                    column,
                    path,
                    "the horizontal rectangle needs `x`+`w`, `right`+`w` or `x`+`right`",
                );
                return None;
            }
        };
        let vertical = match (y, bottom, h) {
            (Some(y), None, Some(h)) => Vertical::TopHeight { y, h },
            (None, Some(bottom), Some(h)) => Vertical::BottomHeight { bottom, h },
            (Some(y), Some(bottom), None) => Vertical::TopBottom { y, bottom },
            (Some(_), Some(_), Some(_)) => {
                self.diagnostics.error(
                    "xml.geom.overdetermined",
                    line,
                    column,
                    path,
                    "give exactly two of `y`, `bottom`, `h`",
                );
                return None;
            }
            _ => {
                self.diagnostics.error(
                    "xml.geom.underdetermined",
                    line,
                    column,
                    path,
                    "the vertical rectangle needs `y`+`h`, `bottom`+`h` or `y`+`bottom`",
                );
                return None;
            }
        };
        Some(Geometry {
            horizontal,
            vertical,
        })
    }

    fn add_content(
        &mut self,
        content: &mut Vec<ContentAttr>,
        value: ContentAttr,
        line: usize,
        column: usize,
        path: &str,
    ) {
        let duplicate = content.iter().any(|item| item.rank() == value.rank());
        let conflict = match &value {
            ContentAttr::Text(_) => content
                .iter()
                .any(|item| matches!(item, ContentAttr::Label(_))),
            ContentAttr::Label(_) => content
                .iter()
                .any(|item| matches!(item, ContentAttr::Text(_))),
            ContentAttr::Value(_) | ContentAttr::Progress(_) | ContentAttr::Number(_) => {
                content.iter().any(|item| {
                    matches!(
                        item,
                        ContentAttr::Value(_) | ContentAttr::Progress(_) | ContentAttr::Number(_)
                    )
                })
            }
            _ => false,
        };
        if duplicate || conflict {
            self.diagnostics.error(
                "xml.attr.conflict",
                line,
                column,
                path,
                format!(
                    "`{}` conflicts with another content attribute on this entry",
                    value.attribute_name()
                ),
            );
        } else {
            content.push(value);
        }
    }

    fn expr_attr(
        &mut self,
        value: &str,
        name: &str,
        line: usize,
        column: usize,
        path: &str,
    ) -> Option<Expr> {
        if let Some(code) = value.strip_prefix("rust:") {
            let code = code.trim();
            if code.is_empty() {
                self.diagnostics.error(
                    "xml.attr.value",
                    line,
                    column,
                    path,
                    format!("{name}: an inline-Rust value must not be empty"),
                );
                return None;
            }
            return Some(Expr::Rust(code.to_string()));
        }
        match expr::parse_expr(value, &self.constants) {
            Ok(expression) => Some(expression),
            Err(error) => {
                self.diagnostics.error(
                    error.code,
                    line,
                    column,
                    path,
                    format!("{name}: {}", error.message),
                );
                None
            }
        }
    }

    fn bool_attr(
        &mut self,
        value: &str,
        name: &str,
        line: usize,
        column: usize,
        path: &str,
    ) -> Option<bool> {
        match value {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => {
                self.diagnostics.error(
                    "xml.attr.value",
                    line,
                    column,
                    path,
                    format!("`{name}` expects `true`, `false`, `1` or `0`"),
                );
                None
            }
        }
    }

    fn number_attr(
        &mut self,
        value: &str,
        name: &str,
        line: usize,
        column: usize,
        path: &str,
    ) -> Option<f64> {
        match value.trim().parse::<f64>() {
            Ok(number) if number.is_finite() => Some(number),
            _ => {
                self.diagnostics.error(
                    "xml.attr.value",
                    line,
                    column,
                    path,
                    format!("`{name}` expects a finite number"),
                );
                None
            }
        }
    }

    fn int_attr(
        &mut self,
        value: &str,
        name: &str,
        line: usize,
        column: usize,
        path: &str,
    ) -> Option<i32> {
        match value.trim().parse::<i32>() {
            Ok(number) => Some(number),
            Err(_) => {
                self.diagnostics.error(
                    "xml.attr.value",
                    line,
                    column,
                    path,
                    format!("`{name}` expects an integer"),
                );
                None
            }
        }
    }

    fn uint_attr(
        &mut self,
        value: &str,
        name: &str,
        line: usize,
        column: usize,
        path: &str,
    ) -> Option<u32> {
        match value.trim().parse::<u32>() {
            Ok(number) => Some(number),
            Err(_) => {
                self.diagnostics.error(
                    "xml.attr.value",
                    line,
                    column,
                    path,
                    format!("`{name}` expects a non-negative integer"),
                );
                None
            }
        }
    }

    fn non_negative(
        &mut self,
        value: &str,
        name: &str,
        line: usize,
        column: usize,
    ) -> Option<f32> {
        match value.trim().parse::<f32>() {
            Ok(number) if number.is_finite() && number >= 0.0 => Some(number),
            _ => {
                self.diagnostics.error(
                    "xml.attr.value",
                    line,
                    column,
                    "layout",
                    format!("`{name}` expects a non-negative number"),
                );
                None
            }
        }
    }

    fn positive_attr(
        &mut self,
        value: &str,
        name: &str,
        line: usize,
        column: usize,
        path: &str,
    ) -> Option<f32> {
        match value.trim().parse::<f32>() {
            Ok(number) if number.is_finite() && number > 0.0 => Some(number),
            _ => {
                self.diagnostics.error(
                    "xml.attr.value",
                    line,
                    column,
                    path,
                    format!("`{name}` expects a number greater than zero"),
                );
                None
            }
        }
    }

    fn radius_attr(
        &mut self,
        value: &str,
        line: usize,
        column: usize,
        path: &str,
    ) -> Option<RadiusAttr> {
        Some(match value {
            "small" => RadiusAttr::Small,
            "medium" => RadiusAttr::Medium,
            "large" => RadiusAttr::Large,
            "pill" => RadiusAttr::Pill,
            other => match other.trim().parse::<f32>() {
                Ok(number) if number.is_finite() && number >= 0.0 => RadiusAttr::Dips(number),
                _ => {
                    self.diagnostics.error(
                        "xml.attr.value",
                        line,
                        column,
                        path,
                        "`radius` expects a non-negative number, `small`, `medium`, `large` or `pill`",
                    );
                    return None;
                }
            },
        })
    }

    fn corners_attr(
        &mut self,
        value: &str,
        line: usize,
        column: usize,
        path: &str,
    ) -> Option<Corners> {
        let parts: Vec<&str> = value
            .split(|ch: char| ch.is_whitespace() || ch == ',')
            .filter(|part| !part.is_empty())
            .collect();
        if parts.len() != 4 {
            self.diagnostics.error(
                "xml.attr.value",
                line,
                column,
                path,
                "`corners` expects four numbers: `tl tr br bl`",
            );
            return None;
        }
        let mut radii = [0.0f32; 4];
        for (slot, part) in radii.iter_mut().zip(parts) {
            match part.parse::<f32>() {
                Ok(number) if number.is_finite() && number >= 0.0 => *slot = number,
                _ => {
                    self.diagnostics.error(
                        "xml.attr.value",
                        line,
                        column,
                        path,
                        "`corners` expects four non-negative numbers",
                    );
                    return None;
                }
            }
        }
        Some(Corners::new(radii[0], radii[1], radii[2], radii[3]))
    }

    fn background_attr(
        &mut self,
        value: &str,
        line: usize,
        column: usize,
        path: &str,
    ) -> Option<BackgroundAttr> {
        if value == "none" {
            return Some(BackgroundAttr::None);
        }
        // One color grammar for every literal: hex, decimal channels and the
        // functional forms. A surface fill is opaque, so alpha is dropped.
        if let Some(color) = crate::skin::parse_color(value) {
            return Some(BackgroundAttr::Literal(color.color));
        }
        match palette_token_for_name(value) {
            Some(token) => Some(BackgroundAttr::Token(token)),
            None => {
                self.diagnostics.error(
                    "xml.attr.value",
                    line,
                    column,
                    path,
                    format!(
                        "`{value}` is not a palette token or a color (`#rrggbb`, `r,g,b`, `rgb(r,g,b)`, …); known tokens: {}",
                        PALETTE_TOKENS.join(", ")
                    ),
                );
                None
            }
        }
    }

    fn role_attr(
        &mut self,
        value: &str,
        line: usize,
        column: usize,
        path: &str,
    ) -> Option<A11yRole> {
        Some(match value {
            "button" => A11yRole::Button,
            "toggle" => A11yRole::Toggle,
            "slider" => A11yRole::Slider,
            "text-field" => A11yRole::TextField,
            "number-field" => A11yRole::NumberField,
            "radio-group" => A11yRole::RadioGroup,
            "tab-list" => A11yRole::TabList,
            "list" => A11yRole::List,
            "multi-select-list" => A11yRole::MultiSelectList,
            "radio-item" => A11yRole::RadioItem,
            "tab-item" => A11yRole::TabItem,
            "list-item" => A11yRole::ListItem,
            "tree" => A11yRole::Tree,
            "tree-item" => A11yRole::TreeItem,
            "grid" => A11yRole::Grid,
            "grid-cell" => A11yRole::GridCell,
            "column-header" => A11yRole::ColumnHeader,
            "heading" => A11yRole::Heading,
            "group" => A11yRole::Group,
            "separator" => A11yRole::Separator,
            "text" => A11yRole::Text,
            "combo-box" => A11yRole::ComboBox,
            "progress-bar" => A11yRole::ProgressBar,
            "menu-item" => A11yRole::MenuItem,
            "menu" => A11yRole::Menu,
            "menu-bar" => A11yRole::MenuBar,
            "tool-bar" => A11yRole::ToolBar,
            "status-bar" => A11yRole::StatusBar,
            _ => {
                self.diagnostics.error(
                    "xml.attr.value",
                    line,
                    column,
                    path,
                    format!("`{value}` is not an accessibility role"),
                );
                return None;
            }
        })
    }

    fn cursor_attr(
        &mut self,
        value: &str,
        line: usize,
        column: usize,
        path: &str,
    ) -> Option<Cursor> {
        Some(match value {
            "arrow" => Cursor::Arrow,
            "hand" => Cursor::Hand,
            "i-beam" => Cursor::IBeam,
            "size-we" => Cursor::SizeWE,
            "size-ns" => Cursor::SizeNS,
            _ => {
                self.diagnostics.error(
                    "xml.attr.value",
                    line,
                    column,
                    path,
                    format!("`{value}` is not a cursor"),
                );
                return None;
            }
        })
    }

    fn validate(&mut self) {
        if !self.saw_root {
            self.diagnostics.error(
                "xml.structure",
                1,
                1,
                String::new(),
                "the root element must be <layouts>",
            );
        }
        let mut layout_ids: Vec<String> = Vec::new();
        let mut minimums: Vec<(f32, f32)> = Vec::new();
        let mut base_count = 0;
        for index in 0..self.variants.len() {
            let id = self.variants[index].id.clone();
            let minimum = (
                self.variants[index].min_width,
                self.variants[index].min_height,
            );
            if layout_ids.iter().any(|existing| *existing == id) {
                self.diagnostics.error(
                    "xml.duplicate-layout-id",
                    1,
                    1,
                    format!("layout/{id}"),
                    format!("variant `{id}` is declared twice"),
                );
            }
            layout_ids.push(id);
            if minimums.contains(&minimum) {
                self.diagnostics.error(
                    "xml.layout.base",
                    1,
                    1,
                    "layouts",
                    format!(
                        "two variants share the minimums ({}, {})",
                        minimum.0, minimum.1
                    ),
                );
            }
            minimums.push(minimum);
            if minimum == (0.0, 0.0) {
                base_count += 1;
            }
        }
        if base_count == 0 && !self.variants.is_empty() {
            self.diagnostics.error(
                "xml.layout.base",
                1,
                1,
                "layouts",
                "one variant must have min-width=\"0\" and min-height=\"0\"",
            );
        }

        // Created widgets are content-scoped: the document's widget set must
        // not depend on the active breakpoint, so every variant declares the
        // same typed ids with the same type, command and scroll parent (§17).
        let mut created: Vec<(
            WidgetId,
            String,
            Option<u32>,
            Option<WidgetId>,
            usize,
            usize,
            String,
            String,
        )> = Vec::new();
        for index in 0..self.variants.len() {
            let variant_id = self.variants[index].id.clone();
            for entry in &self.variants[index].entries {
                let Some(type_name) = entry.type_name.as_ref() else {
                    continue;
                };
                match created.iter_mut().find(|(id, ..)| *id == entry.id) {
                    Some((_, existing_type, existing_command, existing_parent, ..)) => {
                        if *existing_type != *type_name
                            || *existing_command != entry.command
                            || *existing_parent != entry.parent
                        {
                            let (line, column, path) =
                                (entry.line, entry.column, entry.path.clone());
                            self.diagnostics.error(
                                "xml.type.conflict",
                                line,
                                column,
                                path,
                                "variants disagree on this widget's type, command or parent",
                            );
                        }
                    }
                    None => created.push((
                        entry.id,
                        type_name.clone(),
                        entry.command,
                        entry.parent,
                        entry.line,
                        entry.column,
                        entry.path.clone(),
                        variant_id.clone(),
                    )),
                }
            }
        }

        for index in 0..self.variants.len() {
            let variant_id = self.variants[index].id.clone();
            for (id, type_name, command, parent, line, column, path, first_variant) in &created {
                match self.variants[index]
                    .entries
                    .iter()
                    .find(|entry| entry.id == *id)
                {
                    None => self.diagnostics.error(
                        "xml.type.conflict",
                        *line,
                        *column,
                        path.clone(),
                        format!(
                            "`{}` is created by variant `{first_variant}`; every variant must declare it",
                            id.name()
                        ),
                    ),
                    Some(entry)
                        if entry.type_name() != Some(type_name.as_str())
                            || entry.command() != *command
                            || entry.parent() != *parent =>
                    {
                        self.diagnostics.error(
                            "xml.type.conflict",
                            entry.line,
                            entry.column,
                            entry.path.clone(),
                            format!(
                                "variant `{variant_id}` disagrees on `{}`'s type, command or parent",
                                id.name()
                            ),
                        );
                    }
                    Some(_) => {}
                }
            }
            let entries: Vec<WidgetId> = self.variants[index]
                .entries
                .iter()
                .map(|entry| entry.id)
                .collect();
            for (position, id) in entries.iter().enumerate() {
                if entries[..position].contains(id) {
                    self.diagnostics.error(
                        "xml.duplicate-widget-id",
                        self.variants[index].entries[position].line,
                        self.variants[index].entries[position].column,
                        format!("layout/{variant_id}/{}", id.name()),
                        format!("widget `{}` is declared twice in this variant", id.name()),
                    );
                }
            }
            let scrolls: Vec<WidgetId> = self.variants[index]
                .scrolls
                .iter()
                .map(|entry| entry.id)
                .collect();
            for (position, id) in scrolls.iter().enumerate() {
                if scrolls[..position].contains(id) {
                    self.diagnostics.error(
                        "xml.duplicate-widget-id",
                        self.variants[index].scrolls[position].line,
                        self.variants[index].scrolls[position].column,
                        format!("layout/{variant_id}/{}", id.name()),
                        format!("scroll panel `{}` is declared twice in this variant", id.name()),
                    );
                }
            }
            // A `<rust>` binding must name one widget of this variant exactly
            // once, and a widget has either a static child list or a binding.
            for (position, binding) in self.variants[index].rust_bindings.iter().enumerate() {
                if self.variants[index].rust_bindings[..position]
                    .iter()
                    .any(|existing| existing.target == binding.target)
                {
                    self.diagnostics.error(
                        "xml.rust.duplicate",
                        binding.line,
                        binding.column,
                        binding.path.clone(),
                        format!("<rust> targets `{}` twice in this variant", binding.target),
                    );
                }
                match self.variants[index]
                    .entries
                    .iter()
                    .find(|entry| entry.id.name() == binding.target)
                {
                    None => self.diagnostics.error(
                        "xml.rust.target",
                        binding.line,
                        binding.column,
                        binding.path.clone(),
                        format!(
                            "no <widget> with id `{}` in this variant",
                            binding.target
                        ),
                    ),
                    Some(entry) if !entry.skin_children.is_empty() => self.diagnostics.error(
                        "xml.rust.conflict",
                        binding.line,
                        binding.column,
                        binding.path.clone(),
                        format!(
                            "`{}` declares both a static child list and a <rust> binding",
                            binding.target
                        ),
                    ),
                    Some(_) => {}
                }
            }
        }

        // Content is written once per generation, independent of the active
        // breakpoint, so every variant that declares it must agree.
        let mut seen: Vec<(
            WidgetId,
            Vec<ContentAttr>,
            Vec<(&'static str, String)>,
            usize,
            usize,
            String,
        )> = Vec::new();
        for index in 0..self.variants.len() {
            let variant_id = self.variants[index].id.clone();
            for position in 0..self.variants[index].entries.len() {
                let entry = &self.variants[index].entries[position];
                let rust_content: Vec<(&'static str, String)> = entry
                    .rust_attrs
                    .iter()
                    .filter(|(name, _)| crate::model::is_content_attr_name(name))
                    .cloned()
                    .collect();
                if entry.content.is_empty() && rust_content.is_empty() {
                    continue;
                }
                match seen.iter_mut().find(|(id, ..)| *id == entry.id) {
                    Some((_, existing, existing_rust, ..)) => {
                        if !same_content(existing, &entry.content)
                            || *existing_rust != rust_content
                        {
                            let line = entry.line;
                            let column = entry.column;
                            let path = entry.path.clone();
                            self.diagnostics.error(
                                "xml.content.conflict",
                                line,
                                column,
                                path,
                                "variants disagree on this widget's content initial value",
                            );
                        }
                    }
                    None => seen.push((
                        entry.id,
                        entry.content.clone(),
                        rust_content,
                        entry.line,
                        entry.column,
                        format!("layout/{variant_id}/{}", entry.id.name()),
                    )),
                }
            }
        }

        // Local skin data is content-scoped too (§6.2 rule 5): written once per
        // generation, independent of the active breakpoint. A `<rust>` binding
        // is the child-list half of that data, so it participates here: a widget
        // has one child list across every variant.
        let mut skin_seen: Vec<(
            WidgetId,
            Vec<(String, String)>,
            Vec<SkinElement>,
            Option<String>,
        )> = Vec::new();
        for index in 0..self.variants.len() {
            for position in 0..self.variants[index].entries.len() {
                let entry = &self.variants[index].entries[position];
                let binding = self.variants[index]
                    .rust_bindings
                    .iter()
                    .find(|binding| binding.target == entry.id.name())
                    .map(|binding| binding.code.clone());
                if !entry.has_skin() && binding.is_none() {
                    continue;
                }
                match skin_seen.iter().find(|(id, ..)| *id == entry.id) {
                    Some((_, attrs, children, code)) => {
                        if *attrs != entry.skin_attrs
                            || !same_skin_children(children, &entry.skin_children)
                            || *code != binding
                        {
                            let (line, column, path) =
                                (entry.line, entry.column, entry.path.clone());
                            self.diagnostics.error(
                                "xml.skin.conflict",
                                line,
                                column,
                                path,
                                "variants disagree on this widget's local skin data",
                            );
                        }
                    }
                    None => skin_seen.push((
                        entry.id,
                        entry.skin_attrs.clone(),
                        entry.skin_children.clone(),
                        binding,
                    )),
                }
            }
        }
    }
}

fn same_skin_children(left: &[SkinElement], right: &[SkinElement]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.same_data(right))
}

fn same_content(left: &[ContentAttr], right: &[ContentAttr]) -> bool {
    left.len() == right.len()
        && left.iter().all(|item| {
            right
                .iter()
                .any(|other| other.rank() == item.rank() && other == item)
        })
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

/// The position of the element tag inside the span between two reader
/// positions (the leading whitespace/text belongs to the previous event).
fn element_position(source: &str, before: usize, after: usize) -> (usize, usize) {
    let end = after.min(source.len());
    let start = before.min(end);
    let span = &source[start..end];
    let offset = span.find('<').map(|index| start + index).unwrap_or(start);
    position(source, offset)
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    // No `-`: `cw-2` must always read as a subtraction, never as a name.
    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

/// A registry type name: `button`, `list-view`, `vendor.meter`, `x-meter`.
/// Lowercase start, then lowercase letters, digits, `_`, `-` and `.`.
fn valid_type_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    chars.all(|ch| {
        ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-' || ch == '.'
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> LayoutDocument {
        LayoutDocument::parse(text).unwrap_or_else(|diagnostics| {
            panic!(
                "expected a valid document: {:?}",
                diagnostics
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>()
            )
        })
    }

    fn diagnostics(text: &str) -> Vec<LayoutDiagnostic> {
        LayoutDocument::parse(text).expect_err("expected a rejected document")
    }

    fn codes(text: &str) -> Vec<&'static str> {
        diagnostics(text).iter().map(|item| item.code).collect()
    }

    const MINIMAL: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <layout id="base" min-width="0" min-height="0">
    <widget id="a" x="16" y="16" w="120" h="32"/>
  </layout>
</layouts>
"#;

    #[test]
    fn minimal_document_parses() {
        let document = parse(MINIMAL);
        assert_eq!(document.variants().len(), 1);
        assert_eq!(document.variants()[0].id(), "base");
        assert_eq!(document.variants()[0].entries().len(), 1);
        assert_eq!(document.variants()[0].entries()[0].id().name(), "a");
    }

    #[test]
    fn generation_ignores_formatting_and_attribute_order() {
        let first = parse(MINIMAL);
        let second = parse(
            r#"<layouts version="1"><layout min-height="0" id="base" min-width="0"><widget h="32" w="120" y="16" id="a" x="16"/></layout></layouts>"#,
        );
        assert_eq!(first.generation(), second.generation());
        assert_eq!(first.to_xml(), second.to_xml());
    }

    #[test]
    fn to_xml_round_trips() {
        let document = parse(
            r#"<layouts version="1">
  <const name="pad" value="16"/>
  <layout id="base" min-width="0" min-height="0">
    <scroll id="page" x="pad" y="8" w="cw-2*pad" h="ch-16" content-height="auto"/>
    <widget id="a" x="16" y="16" w="120" h="32"
            label="主操作" tooltip="样式来自文件" name="主操作"
            radius="medium" background="accent" font-size="18"/>
  </layout>
</layouts>"#,
        );
        let xml = document.to_xml();
        let again = parse(&xml);
        assert_eq!(document.generation(), again.generation());
        assert_eq!(xml, again.to_xml());
        assert_eq!(
            again
                .select(crate::geometry::Size::new(0.0, 0.0))
                .entries()
                .len(),
            1
        );
        assert_eq!(
            again
                .select(crate::geometry::Size::new(0.0, 0.0))
                .scrolls()
                .len(),
            1
        );
    }

    #[test]
    fn widget_children_are_stored_as_raw_elements() {
        let document = parse(
            r#"<layouts version="1"><layout id="base" min-width="0" min-height="0">
  <widget id="a" x="0" y="0" w="10" h="10" x-level="72">
    <x-item title="夜曲" level="72"/>
    <x-group label="g">
      <x-item title="晴天"/>
    </x-group>
  </widget>
</layout></layouts>"#,
        );
        let entry = &document.variants()[0].entries()[0];
        assert_eq!(
            entry.skin_attrs(),
            [("x-level".to_string(), "72".to_string())]
        );
        let children = entry.skin_children();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].name, "x-item");
        assert_eq!(children[0].attr("title"), Some("夜曲"));
        assert_eq!(children[1].children.len(), 1);
        assert_eq!(children[1].children[0].name, "x-item");
        // Line/column point at the opening tag for diagnostics.
        assert!(children[0].line >= 2, "{:?}", children[0]);
        // The canonical text keeps the raw values and the source order, so the
        // model round-trips byte for byte.
        let xml = document.to_xml();
        assert!(xml.contains("x-level=\"72\""), "{xml}");
        assert!(
            xml.contains("<x-item title=\"夜曲\" level=\"72\"/>"),
            "{xml}"
        );
        assert!(xml.contains("</x-group>"), "{xml}");
        let again = parse(&xml);
        assert_eq!(document.generation(), again.generation());
        assert_eq!(xml, again.to_xml());
    }

    #[test]
    fn local_skin_data_is_variant_invariant() {
        let text = r#"<layouts version="1">
  <layout id="base" min-width="0" min-height="0">
    <widget id="a" x="0" y="0" w="10" h="10" x-level="72"/>
  </layout>
  <layout id="wide" min-width="1000" min-height="700">
    <widget id="a" x="0" y="0" w="10" h="10" x-level="40"/>
  </layout>
</layouts>"#;
        assert!(codes(text).contains(&"xml.skin.conflict"));
        // The same data at different offsets is not a conflict: positions are
        // not part of the data model.
        let agreeing = r#"<layouts version="1">
  <layout id="base" min-width="0" min-height="0">
    <widget id="a" x="0" y="0" w="10" h="10" x-level="72">
      <x-item label="夜曲"/>
    </widget>
  </layout>
  <layout id="wide" min-width="1000" min-height="700">
    <widget id="a" x="0" y="0" w="10" h="10" x-level="72"><x-item label="夜曲"/></widget>
  </layout>
</layouts>"#;
        parse(agreeing);
    }

    #[test]
    fn created_widget_entries_round_trip() {
        let text = r#"<layouts version="1"><layout id="base" min-width="0" min-height="0">
  <widget id="save" type="button" command="101" x="0" y="0" w="80" h="30" label="保存"/>
</layout></layouts>"#;
        let document = parse(text);
        let entry = &document.variants()[0].entries()[0];
        assert_eq!(entry.type_name(), Some("button"));
        assert_eq!(entry.command(), Some(101));
        let xml = document.to_xml();
        assert!(
            xml.contains("<widget id=\"save\" type=\"button\" command=\"101\""),
            "{xml}"
        );
        let again = parse(&xml);
        assert_eq!(xml, again.to_xml());
        assert_eq!(document.generation(), again.generation());
    }

    #[test]
    fn a_created_widgets_scroll_parent_round_trips() {
        let text = r#"<layouts version="1"><layout id="base" min-width="0" min-height="0">
  <widget id="row" type="label" parent="panel" x="0" y="20" w="80" h="20" label="行"/>
</layout></layouts>"#;
        let document = parse(text);
        let entry = &document.variants()[0].entries()[0];
        assert_eq!(entry.parent(), Some(WidgetId::new("panel")));
        let xml = document.to_xml();
        assert!(
            xml.contains("<widget id=\"row\" type=\"label\" parent=\"panel\""),
            "{xml}"
        );
        let again = parse(&xml);
        assert_eq!(xml, again.to_xml());
        assert_eq!(document.generation(), again.generation());

        // `parent` is content-scoped like the type and the command: every
        // variant must agree.
        let disagreeing = r#"<layouts version="1">
  <layout id="base" min-width="0" min-height="0">
    <widget id="row" type="label" parent="panel" x="0" y="20" w="80" h="20"/>
  </layout>
  <layout id="wide" min-width="1000" min-height="700">
    <widget id="row" type="label" parent="other" x="0" y="20" w="80" h="20"/>
  </layout>
</layouts>"#;
        assert!(codes(disagreeing).contains(&"xml.type.conflict"));

        // A widget cannot be its own parent, and the parent name is required.
        assert!(codes(
            r#"<layouts version="1"><layout id="base" min-width="0" min-height="0">
  <widget id="row" type="label" parent="row" x="0" y="0" w="80" h="20"/>
</layout></layouts>"#
        )
        .contains(&"xml.attr.value"));
        assert!(codes(
            r#"<layouts version="1"><layout id="base" min-width="0" min-height="0">
  <widget id="row" type="label" parent=" " x="0" y="0" w="80" h="20"/>
</layout></layouts>"#
        )
        .contains(&"xml.attr.value"));
    }

    #[test]
    fn created_widget_errors() {
        let text = |body: &str| {
            format!(
                r#"<layouts version="1"><layout id="base" min-width="0" min-height="0">{body}</layout></layouts>"#
            )
        };
        // A type name must be a lowercase identifier path.
        assert!(codes(&text(
            r#"<widget id="a" type="Button" x="0" y="0" w="10" h="10"/>"#
        ))
        .contains(&"xml.attr.value"));
        assert!(codes(&text(
            r#"<widget id="a" type="x y" x="0" y="0" w="10" h="10"/>"#
        ))
        .contains(&"xml.attr.value"));
        // `command` only makes sense on a created widget.
        assert!(codes(&text(
            r#"<widget id="a" command="3" x="0" y="0" w="10" h="10"/>"#
        ))
        .contains(&"xml.attr.conflict"));
        // Every variant must declare the created id with the same type.
        assert!(codes(
            r#"<layouts version="1">
  <layout id="base" min-width="0" min-height="0">
    <widget id="a" type="label" x="0" y="0" w="10" h="10"/>
  </layout>
  <layout id="wide" min-width="1000" min-height="700">
    <widget id="b" x="0" y="0" w="10" h="10"/>
  </layout>
</layouts>"#
        )
        .contains(&"xml.type.conflict"));
        assert!(codes(
            r#"<layouts version="1">
  <layout id="base" min-width="0" min-height="0">
    <widget id="a" type="label" x="0" y="0" w="10" h="10"/>
  </layout>
  <layout id="wide" min-width="1000" min-height="700">
    <widget id="a" type="button" x="0" y="0" w="10" h="10"/>
  </layout>
</layouts>"#
        )
        .contains(&"xml.type.conflict"));
        // Agreeing declarations are accepted (different geometry per variant).
        parse(
            r#"<layouts version="1">
  <layout id="base" min-width="0" min-height="0">
    <widget id="a" type="label" x="0" y="0" w="10" h="10"/>
  </layout>
  <layout id="wide" min-width="1000" min-height="700">
    <widget id="a" type="label" x="0" y="0" w="20" h="10"/>
  </layout>
</layouts>"#,
        );
    }

    #[test]
    fn rust_child_list_bindings_round_trip() {
        let text = r#"<layouts version="1"><layout id="base" min-width="0" min-height="0">
  <widget id="a" x="0" y="0" w="10" h="10"/>
  <rust target="a" code="rows(&amp;self)"/>
</layout></layouts>"#;
        let document = parse(text);
        let variant = &document.variants()[0];
        assert_eq!(variant.rust_bindings().len(), 1);
        let binding = &variant.rust_bindings()[0];
        assert_eq!(binding.id(), "a");
        assert_eq!(binding.code(), "rows(&self)");
        assert!(binding.line() >= 2);
        // The canonical writer keeps the binding (escaped again) and the
        // generation hash covers the code.
        let xml = document.to_xml();
        assert!(
            xml.contains("<rust target=\"a\" code=\"rows(&amp;self)\"/>"),
            "{xml}"
        );
        let again = parse(&xml);
        assert_eq!(xml, again.to_xml());
        assert_eq!(document.generation(), again.generation());

        let changed = parse(&text.replace("rows(&amp;self)", "rows(&amp;other)"));
        assert_ne!(document.generation(), changed.generation());
    }

    #[test]
    fn rust_child_list_binding_errors() {
        let bind = |body: &str| {
            format!(
                r#"<layouts version="1"><layout id="base" min-width="0" min-height="0">
  <widget id="a" x="0" y="0" w="10" h="10"/>{body}
</layout></layouts>"#
            )
        };
        assert!(codes(&bind(r#"<rust code="rows()"/>"#)).contains(&"xml.missing-attribute"));
        assert!(codes(&bind(r#"<rust target="a"/>"#)).contains(&"xml.missing-attribute"));
        assert!(codes(&bind(r#"<rust target="a" code="  "/>"#)).contains(&"xml.attr.value"));
        assert!(codes(&bind(r#"<rust target="a" code="x()" extra="1"/>"#))
            .contains(&"xml.unknown-attribute"));
        assert!(codes(&bind(r#"<rust target="missing" code="rows()"/>"#))
            .contains(&"xml.rust.target"));
        assert!(codes(&bind(r#"<rust target="a" code="rows()"/><rust target="a" code="more()"/>"#))
            .contains(&"xml.rust.duplicate"));
        // A widget has either a static child list or a binding, never both.
        assert!(codes(
            r#"<layouts version="1"><layout id="base" min-width="0" min-height="0">
  <widget id="a" x="0" y="0" w="10" h="10"><x-item label="x"/></widget>
  <rust target="a" code="rows()"/>
</layout></layouts>"#
        )
        .contains(&"xml.rust.conflict"));
        // Bindings are content: every variant that declares one must agree.
        assert!(codes(
            r#"<layouts version="1">
  <layout id="base" min-width="0" min-height="0">
    <widget id="a" x="0" y="0" w="10" h="10"/>
    <rust target="a" code="rows()"/>
  </layout>
  <layout id="wide" min-width="1000" min-height="700">
    <widget id="a" x="0" y="0" w="20" h="10"/>
    <rust target="a" code="more()"/>
  </layout>
</layouts>"#
        )
        .contains(&"xml.skin.conflict"));
    }

    #[test]
    fn bom_and_astral_text_are_accepted() {
        let text = format!("\u{feff}{MINIMAL}");
        let document = parse(&text);
        assert_eq!(document.variants().len(), 1);
        let text = r#"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="10" h="10" label="🎵 夜曲"/></layout></layouts>"#;
        let document = parse(text);
        assert!(document.to_xml().contains("🎵 夜曲"));
    }

    #[test]
    fn structure_errors() {
        assert!(codes("<layouts/>").contains(&"xml.version"));
        assert!(codes("<layouts version=\"2\"/>").contains(&"xml.version"));
        assert!(codes("<!DOCTYPE layouts><layouts version=\"1\"/>").contains(&"xml.structure"));
        assert!(codes("<layouts version=\"1\"><![CDATA[x]]></layouts>").contains(&"xml.structure"));
        assert!(codes("<layouts version=\"1\">text</layouts>").contains(&"xml.structure"));
        assert!(codes("<layouts version=\"1\"><bogus/></layouts>").contains(&"xml.unknown-element"));
        assert!(codes("<layouts version=\"1\"><layout id=\"base\"><bogus/></layout></layouts>")
            .contains(&"xml.unknown-element"));
        // Child elements are data only under <widget>; a <scroll> has none.
        assert!(codes("<layouts version=\"1\"><layout id=\"base\"><scroll id=\"p\" x=\"0\" y=\"0\" w=\"1\" h=\"1\"><item/></scroll></layout></layouts>")
            .contains(&"xml.unknown-element"));
        // A nested element under an element that does not collect is refused,
        // and its own children too.
        assert!(codes("<layouts version=\"1\"><layout id=\"base\"><bogus><item/></bogus></layout></layouts>")
            .contains(&"xml.unknown-element"));
    }

    #[test]
    fn attribute_errors() {
        assert!(codes(r#"<layouts version="1"><layout id="base"><widget x="0" y="0" w="1" h="1"/></layout></layouts>"#)
            .contains(&"xml.missing-attribute"));
        // A name outside the shared schema is a local skin attribute now; the
        // widget's declaration is what decides, and that happens at apply time.
        let document = parse(
            r#"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="1" h="1" colour="red"/></layout></layouts>"#,
        );
        assert_eq!(
            document.variants()[0].entries()[0].skin_attrs(),
            [("colour".to_string(), "red".to_string())]
        );
        assert!(codes(r#"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="1" h="1" visible="yes"/></layout></layouts>"#)
            .contains(&"xml.attr.value"));
        assert!(codes(r#"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="1" h="1" role="widget"/></layout></layouts>"#)
            .contains(&"xml.attr.value"));
        assert!(codes(r#"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="1" h="1" background="accent2"/></layout></layouts>"#)
            .contains(&"xml.attr.value"));
        assert!(codes(r#"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="1" h="1" radius="medium" corners="1 1 1 1"/></layout></layouts>"#)
            .contains(&"xml.attr.conflict"));
        assert!(codes(r#"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="1" h="1" text="x" label="y"/></layout></layouts>"#)
            .contains(&"xml.attr.conflict"));
        assert!(codes(r#"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="1" h="1" value="1" progress="0.5"/></layout></layouts>"#)
            .contains(&"xml.attr.conflict"));
    }

    #[test]
    fn geometry_errors() {
        assert!(codes(r#"<layouts version="1"><layout id="base"><widget id="a" w="1" y="0" h="1"/></layout></layouts>"#)
            .contains(&"xml.geom.underdetermined"));
        assert!(codes(r#"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="1" h="1" right="2"/></layout></layouts>"#)
            .contains(&"xml.geom.overdetermined"));
        assert!(codes(r#"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="1"/></layout></layouts>"#)
            .contains(&"xml.geom.underdetermined"));
        // Division by zero is an apply-time error: the expression parses.
        let document = parse(
            r#"<layouts version="1"><layout id="base"><widget id="a" x="1/0" y="0" w="1" h="1"/></layout></layouts>"#,
        );
        let space = crate::model::LayoutSpace::client(
            crate::geometry::Size::new(100.0, 100.0),
            document.constants(),
        );
        assert_eq!(
            document.variants()[0].entries()[0]
                .resolve(space)
                .unwrap_err()
                .code,
            "xml.expr.div-zero"
        );
        assert!(codes(r#"<layouts version="1"><layout id="base"><widget id="a" x="pad" y="0" w="1" h="1"/></layout></layouts>"#)
            .contains(&"xml.const.undefined"));
    }

    #[test]
    fn constant_errors() {
        assert!(codes(r#"<layouts version="1"><const name="cw" value="10"/><layout id="base"/></layouts>"#)
            .contains(&"xml.const.reserved"));
        assert!(codes(r#"<layouts version="1"><const name="pad" value="1"/><const name="pad" value="2"/><layout id="base"/></layouts>"#)
            .contains(&"xml.const.duplicate"));
        assert!(codes(r#"<layouts version="1"><const name="Pad" value="1"/><layout id="base"/></layouts>"#)
            .contains(&"xml.const.invalid"));
        assert!(codes(r#"<layouts version="1"><const name="pad" value="cw-16"/><layout id="base"/></layouts>"#)
            .contains(&"xml.const.vars"));
        assert!(codes(r#"<layouts version="1"><const name="pad" value="50%"/><layout id="base"/></layouts>"#)
            .contains(&"xml.const.vars"));
        let document = parse(
            r#"<layouts version="1"><const name="pad" value="1"/><const name="gap" value="pad+1"/><layout id="base"/></layouts>"#,
        );
        assert_eq!(document.constants().get("gap"), Some(2.0));
    }

    #[test]
    fn variant_errors() {
        assert!(codes(r#"<layouts version="1"><layout id="wide" min-width="1000"/></layouts>"#)
            .contains(&"xml.layout.base"));
        assert!(codes(r#"<layouts version="1"><layout id="base"/><layout id="base"/></layouts>"#)
            .contains(&"xml.duplicate-layout-id"));
        assert!(codes(r#"<layouts version="1"><layout id="a"/><layout id="b"/></layouts>"#)
            .contains(&"xml.layout.base"));
        assert!(codes(
            r#"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="1" h="1"/><widget id="a" x="0" y="0" w="1" h="1"/></layout></layouts>"#
        )
        .contains(&"xml.duplicate-widget-id"));
    }

    #[test]
    fn content_must_agree_across_variants() {
        let text = r#"<layouts version="1">
  <layout id="base"><widget id="a" x="0" y="0" w="1" h="1" label="x"/></layout>
  <layout id="wide" min-width="1000"><widget id="a" x="0" y="0" w="2" h="1" label="x"/></layout>
</layouts>"#;
        parse(text);
        let text = r#"<layouts version="1">
  <layout id="base"><widget id="a" x="0" y="0" w="1" h="1" label="x"/></layout>
  <layout id="wide" min-width="1000"><widget id="a" x="0" y="0" w="2" h="1" label="y"/></layout>
</layouts>"#;
        assert!(codes(text).contains(&"xml.content.conflict"));
    }

    #[test]
    fn constants_resolve_in_entries() {
        let document = parse(
            r#"<layouts version="1">
  <const name="pad" value="16"/>
  <const name="double" value="2*pad"/>
  <layout id="base">
    <widget id="a" x="pad" y="0" w="cw-2*double" h="10"/>
  </layout>
</layouts>"#,
        );
        let constants = document.constants();
        assert_eq!(constants.get("double"), Some(32.0));
        let space = crate::model::LayoutSpace::client(
            crate::geometry::Size::new(400.0, 300.0),
            constants,
        );
        let rect = document.variants()[0].entries()[0].resolve(space).unwrap();
        assert_eq!(rect, crate::geometry::Rect::from_xywh(16.0, 0.0, 336.0, 10.0));
    }

    #[test]
    fn zero_size_is_not_an_error() {
        let document = parse(
            r#"<layouts version="1"><layout id="base"><widget id="a" x="0" y="0" w="0" h="10"/></layout></layouts>"#,
        );
        let space = crate::model::LayoutSpace::client(
            crate::geometry::Size::new(100.0, 100.0),
            document.constants(),
        );
        let rect = document.variants()[0].entries()[0].resolve(space).unwrap();
        assert_eq!(rect.width(), 0.0);
    }

    #[test]
    fn diagnostics_carry_positions() {
        let text = "<layouts version=\"1\">\n  <layout id=\"base\">\n    <widget id=\"a\" x=\"oops\"/>\n  </layout>\n</layouts>";
        let diagnostics = diagnostics(text);
        let item = diagnostics
            .iter()
            .find(|item| item.code == "xml.const.undefined")
            .expect("expression error");
        assert_eq!(item.line, 3);
        assert!(item.path.contains("a"));
    }
    #[test]
    fn inline_rust_values_are_kept_and_refused_by_the_dynamic_evaluator() {
        let text = r#"<layouts version="1"><layout id="base"><widget id="w"
            x="rust:PAD" y="0" w="rust:cw - 2.0*PAD" h="10"
            label="rust:TITLE" visible="rust:!cfg!(test)" radius="rust:RADIUS"/></layout></layouts>"#;
        let document = parse(text);
        let entry = &document.variants()[0].entries()[0];
        assert!(matches!(
            entry.geometry.horizontal,
            crate::model::Horizontal::LeftWidth {
                x: Expr::Rust(_),
                ..
            }
        ));
        assert_eq!(entry.rust_attr("label"), Some("TITLE"));
        assert_eq!(entry.rust_attr("visible"), Some("!cfg!(test)"));
        assert_eq!(entry.rust_attr("radius"), Some("RADIUS"));
        // The canonical text keeps the values, so the generation hash covers them.
        let again = LayoutDocument::parse(&document.to_xml()).expect("round trip");
        assert_eq!(again.to_xml(), document.to_xml());

        // Dynamic mode never evaluates code: resolving the rectangle refuses.
        let space = crate::model::LayoutSpace::client(
            crate::geometry::Size::new(100.0, 100.0),
            document.constants(),
        );
        let error = entry.resolve(space).expect_err("inline Rust is compiled-only");
        assert_eq!(error.code, "xml.rust.unsupported");
    }

    #[test]
    fn a_widget_local_name_is_not_an_inline_rust_scalar() {
        let document = parse(
            r#"<layouts version="1"><layout id="base"><widget id="m" x="0" y="0" w="10" h="10" x-level="rust:42"/></layout></layouts>"#,
        );
        let entry = &document.variants()[0].entries()[0];
        assert!(entry.rust_attrs.is_empty());
        assert_eq!(
            entry.skin_attrs(),
            &[("x-level".to_string(), "rust:42".to_string())]
        );
    }
}
