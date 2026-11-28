// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The M7 compiled mode (D128): turn a `.buixml` document into plain Rust.
//!
//! The output is an ordinary module (checked in and `include!`d, or produced by
//! a proc macro later) that describes the same layout the dynamic parser would
//! produce, as `static` data:
//!
//! - every rectangle becomes a small `fn(Size) -> Result<Rect, RectError>` with
//!   the expressions folded to native arithmetic (constants inlined);
//! - properties and surface tokens become `LazyLock` statics;
//! - content and data child lists become `&'static [CompiledContent]` /
//!   `CompiledElement` tables.
//!
//! `install` / `LAYOUT` then run through `Ui::apply_compiled`, which shares the
//! dynamic applier's claim, content and skin writes — so the two paths behave
//! identically and `layout_parity` can assert it.
//!
//! Inline Rust (`rust:` values) is spliced verbatim, so `rustc` checks it. With
//! [`Options::context`] (`context = Type`) the generated `install` takes
//! `context: &Type` and evaluates the values that read it per apply
//! (`CompiledValues`); a `<rust target=...>` binding becomes a per-apply child
//! list there and a `LazyLock` static otherwise (D146).
//!
//! The generator prints source line/column in every diagnostic and the
//! original file name in the header, so a failure still points at the
//! `.buixml` file rather than the generated Rust.

use std::fmt::Write as _;

use bamboo_native_ui_layout_core::a11y::A11yRole;
use bamboo_native_ui_layout_core::diag::LayoutDiagnostic;
use bamboo_native_ui_layout_core::event::Cursor;
use bamboo_native_ui_layout_core::expr::{Axis, BinOp, Expr, Var};
use bamboo_native_ui_layout_core::model::{
    BackgroundAttr, ContentAttr, ContentHeight, Geometry, Horizontal, LayoutConstants,
    LayoutDocument, LayoutEntry, PropAttrs, RadiusAttr, ScrollEntry, StyleAttrs, Vertical,
};
use bamboo_native_ui_layout_core::skin::SkinElement;

/// Default identifier for the generated module, derived from a file name:
/// `layout-demo.buixml` → `layout_demo`.
pub fn module_name(source_name: &str) -> String {
    let stem = source_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(source_name)
        .split('.')
        .next()
        .unwrap_or("layout");
    let mut name = String::new();
    for (index, ch) in stem.chars().enumerate() {
        if ch.is_ascii_alphanumeric() {
            if index == 0 && ch.is_ascii_digit() {
                name.push('_');
            }
            name.push(ch.to_ascii_lowercase());
        } else {
            name.push('_');
        }
    }
    if name.is_empty() {
        "layout".to_string()
    } else {
        name
    }
}

/// Generate the compiled module for one document.
///
/// `source_name` is only used in the header and the module name; `module` is
/// the identifier the output is wrapped in (see [`module_name`]).
pub fn generate(xml: &str, source_name: &str, module: &str) -> Result<String, Vec<LayoutDiagnostic>> {
    generate_with(
        xml,
        source_name,
        Options {
            module,
            context: None,
        },
    )
}

/// Code-generation options.
#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    /// The generated module identifier (defaults to [`module_name`]).
    pub module: &'a str,
    /// `context = Type`: the generated `install` takes `context: &Type` and
    /// inline Rust may read it (D146). Without a context, inline Rust is
    /// evaluated once into statics.
    pub context: Option<&'a str>,
}

/// Generate the compiled module with explicit options.
pub fn generate_with(
    xml: &str,
    source_name: &str,
    options: Options<'_>,
) -> Result<String, Vec<LayoutDiagnostic>> {
    let document = LayoutDocument::parse(xml)?;
    if let Some(context) = options.context {
        let trimmed = context.trim();
        if trimmed.is_empty() {
            // The context parameter is spliced into `&Type`; an empty type is a
            // caller bug, not a document error, but the module must not be
            // generated broken.
            return Err(vec![LayoutDiagnostic::error(
                "xml.rust.context",
                1,
                1,
                "context".to_string(),
                "`context` must name a type, e.g. `context = AppLayout`",
            )]);
        }
    }
    Ok(Emitter::new(&document, source_name, options).emit())
}

struct Emitter<'a> {
    document: &'a LayoutDocument,
    source: &'a str,
    module: String,
    /// The `install` argument type when inline Rust may read a context.
    context: Option<String>,
    out: String,
    /// Statics behind the entry tables (`CompiledElement` lists). Rust items
    /// may be referenced before their definition, so they are staged at the
    /// end of the module.
    extra: String,
    elements: usize,
    empty_props: bool,
    empty_style: bool,
    /// Whether any `<rust>` binding exists (imports `SkinElement`).
    uses_bindings: bool,
}

impl<'a> Emitter<'a> {
    fn new(document: &'a LayoutDocument, source: &'a str, options: Options<'_>) -> Self {
        Self {
            document,
            source,
            module: options.module.to_string(),
            context: options.context.map(|context| context.trim().to_string()),
            out: String::new(),
            extra: String::new(),
            elements: 0,
            empty_props: false,
            empty_style: false,
            uses_bindings: document
                .variants()
                .iter()
                .any(|variant| !variant.rust_bindings().is_empty()),
        }
    }

    fn emit(mut self) -> String {
        let entries: usize = self
            .document
            .variants()
            .iter()
            .map(|variant| variant.entries().len())
            .sum();
        let scrolls: usize = self
            .document
            .variants()
            .iter()
            .map(|variant| variant.scrolls().len())
            .sum();
        writeln!(
            self.out,
            "// @generated by bamboo-native-ui-layout-codegen — do not edit by hand."
        )
        .unwrap();
        writeln!(
            self.out,
            "// Source: {} · generation 0x{:016x}",
            self.source,
            self.document.generation()
        )
        .unwrap();
        writeln!(
            self.out,
            "// {} variant(s), {} widget entries, {} scroll entries.",
            self.document.variants().len(),
            entries,
            scrolls
        )
        .unwrap();
        writeln!(self.out, "//").unwrap();
        writeln!(
            self.out,
            "// Apply it with `ui.apply_compiled(&{m}::LAYOUT, &palette, client, &mut sink)`",
            m = self.module
        )
        .unwrap();
        match self.context.as_deref() {
            Some(context) => {
                writeln!(
                    self.out,
                    "// or the module's `install(ui, sink, palette, client, context: &{context})` helper,"
                )
                .unwrap();
                writeln!(
                    self.out,
                    "// which is the only way to evaluate the inline Rust that reads the context."
                )
                .unwrap();
            }
            None => {
                writeln!(
                    self.out,
                    "// or the module's `install(ui, sink, palette, client)` helper."
                )
                .unwrap();
            }
        }
        writeln!(self.out, "pub mod {} {{", self.module).unwrap();
        writeln!(
            self.out,
            "    #![allow(dead_code, unused_imports, unused_parens, unused_variables, non_upper_case_globals, clippy::all)]"
        )
        .unwrap();
        writeln!(self.out).unwrap();
        writeln!(self.out, "    use std::sync::LazyLock;").unwrap();
        // Inline Rust (`rust:` values) names items from the including module
        // directly; a glob import keeps those paths short. Explicit imports
        // below win over the glob on a clash.
        writeln!(self.out, "    #[allow(unused_imports)]").unwrap();
        writeln!(self.out, "    use super::*;").unwrap();
        writeln!(self.out).unwrap();
        writeln!(self.out, "    use bamboo_native_ui::geometry::{{Rect, Size}};").unwrap();
        writeln!(
            self.out,
            "    use bamboo_native_ui::layout_xml::compiled::{{"
        )
        .unwrap();
        writeln!(
            self.out,
            "        edges, CompiledContent, CompiledElement, CompiledEntry, CompiledHeight,"
        )
        .unwrap();
        writeln!(
            self.out,
            "        CompiledLayout, CompiledScroll, CompiledTarget, CompiledVariant, RectError,"
        )
        .unwrap();
        if self.context.is_some() {
            writeln!(
                self.out,
                "        CompiledEntryValues, CompiledValues, TargetKind,"
            )
            .unwrap();
        } else {
            writeln!(self.out, "        TargetKind,").unwrap();
        }
        writeln!(self.out, "    }};").unwrap();
        writeln!(
            self.out,
            "    use bamboo_native_ui::layout_xml::{{"
        )
        .unwrap();
        writeln!(
            self.out,
            "        BackgroundAttr, ContentAttr, LayoutFailure, LayoutReport, PropAttrs, RadiusAttr,"
        )
        .unwrap();
        writeln!(self.out, "        StyleAttrs,").unwrap();
        writeln!(self.out, "    }};").unwrap();
        if self.uses_bindings {
            writeln!(
                self.out,
                "    use bamboo_native_ui::skin::SkinElement;"
            )
            .unwrap();
        }
        writeln!(
            self.out,
            "    use bamboo_native_ui::{{A11yRole, Color, Corners, Cursor, Palette, Ui, UiHost}};"
        )
        .unwrap();
        writeln!(self.out).unwrap();

        let variant_count = self.document.variants().len();
        for index in 0..variant_count {
            self.emit_variant(index);
        }
        self.emit_targets();

        writeln!(self.out, "    static __variants: &[CompiledVariant] = &[").unwrap();
        for index in 0..variant_count {
            let variant = &self.document.variants()[index];
            writeln!(
                self.out,
                "        CompiledVariant {{ id: {}, min_width: {}, min_height: {}, entries: __entries_{}, scrolls: __scrolls_{} }},",
                rust_string(variant.id()),
                number32(variant.min_width),
                number32(variant.min_height),
                index,
                index
            )
            .unwrap();
        }
        writeln!(self.out, "    ];").unwrap();
        writeln!(self.out).unwrap();

        if self.empty_props {
            writeln!(
                self.out,
                "    static __empty_props: LazyLock<PropAttrs> = LazyLock::new(PropAttrs::default);"
            )
            .unwrap();
        }
        if self.empty_style {
            writeln!(
                self.out,
                "    static __empty_style: LazyLock<StyleAttrs> = LazyLock::new(StyleAttrs::default);"
            )
            .unwrap();
        }
        self.out.push_str(&self.extra);

        writeln!(self.out).unwrap();
        writeln!(
            self.out,
            "    /// The whole document as static data (generation 0x{:016x}).",
            self.document.generation()
        )
        .unwrap();
        writeln!(
            self.out,
            "    pub static LAYOUT: CompiledLayout = CompiledLayout {{"
        )
        .unwrap();
        writeln!(self.out, "        name: {},", rust_string(self.source)).unwrap();
        writeln!(
            self.out,
            "        generation: 0x{:016x},",
            self.document.generation()
        )
        .unwrap();
        writeln!(self.out, "        variants: __variants,").unwrap();
        writeln!(self.out, "        targets: __targets,").unwrap();
        writeln!(self.out, "    }};").unwrap();
        writeln!(self.out).unwrap();
        writeln!(
            self.out,
            "    /// Apply the document; the report matches the dynamic applier's."
        )
        .unwrap();
        writeln!(self.out, "    pub fn install<M: 'static, H: UiHost>(").unwrap();
        writeln!(self.out, "        ui: &mut Ui<M>,").unwrap();
        writeln!(self.out, "        sink: &mut H,").unwrap();
        writeln!(self.out, "        palette: &Palette,").unwrap();
        writeln!(self.out, "        client: Size,").unwrap();
        match self.context.as_deref() {
            Some(context) => {
                writeln!(self.out, "        context: &{context},").unwrap();
                writeln!(
                    self.out,
                    "    ) -> Result<LayoutReport, LayoutFailure> {{"
                )
                .unwrap();
                let mut values = String::new();
                for index in 0..variant_count {
                    if index > 0 {
                        values.push_str(", ");
                    }
                    values.push_str(&format!("__values_{index}(context)"));
                }
                writeln!(
                    self.out,
                    "        let values = CompiledValues {{ variants: vec![{values}] }};"
                )
                .unwrap();
                writeln!(
                    self.out,
                    "        ui.apply_compiled_with(&LAYOUT, &values, palette, client, sink)"
                )
                .unwrap();
            }
            None => {
                writeln!(
                    self.out,
                    "    ) -> Result<LayoutReport, LayoutFailure> {{"
                )
                .unwrap();
                writeln!(
                    self.out,
                    "        ui.apply_compiled(&LAYOUT, palette, client, sink)"
                )
                .unwrap();
            }
        }
        writeln!(self.out, "    }}").unwrap();
        writeln!(self.out, "}}").unwrap();
        self.out
    }

    fn emit_variant(&mut self, index: usize) {
        let document = self.document;
        let variant = &document.variants()[index];
        writeln!(
            self.out,
            "    // ---- variant {:?} (min {} x {}) ----",
            variant.id(),
            number32(variant.min_width),
            number32(variant.min_height)
        )
        .unwrap();
        let scroll_base = variant.entries().len();
        let entries: Vec<&LayoutEntry> = variant.entries().iter().collect();
        let bindings: Vec<(usize, &str, &str)> = variant
            .rust_bindings()
            .iter()
            .enumerate()
            .map(|(position, binding)| (position, binding.id(), binding.code()))
            .collect();
        // Without a context a binding is one more static value: evaluated once,
        // like every other inline-Rust value of such a module.
        if self.context.is_none() {
            for (position, _, code) in &bindings {
                writeln!(
                    self.out,
                    "    static __children_{}_{}: LazyLock<Vec<SkinElement>> = LazyLock::new(|| {{ {code} }});",
                    index, position
                )
                .unwrap();
                writeln!(self.out).unwrap();
            }
        }
        for (position, entry) in entries.iter().enumerate() {
            self.emit_entry(index, position, entry);
        }        let scrolls: Vec<&ScrollEntry> = variant.scrolls().iter().collect();
        for (position, scroll) in scrolls.iter().enumerate() {
            self.emit_scroll(index, position, scroll);
        }
        writeln!(
            self.out,
            "    static __entries_{}: &[CompiledEntry] = &[",
            index
        )
        .unwrap();
        for (position, entry) in entries.iter().enumerate() {
            let children = if entry.skin_children().is_empty() {
                "&[]".to_string()
            } else {
                self.emit_elements(entry.skin_children())
            };
            let binding = binding_index(&bindings, entry.id().name());
            let rust_children = match (binding, self.context.is_some()) {
                (Some(position), false) => format!("Some(&__children_{}_{})", index, position),
                _ => "None".to_string(),
            };
            // A context module computes the inline Rust that reads it per apply,
            // so its statics hold only the literal values.
            let static_rust: &[(&'static str, String)] = if self.context.is_some() {
                &[]
            } else {
                &entry.rust_attrs
            };
            let content = self.content_slice(&entry.content, static_rust);
            let skin_attrs = self.skin_attrs(entry.skin_attrs());
            let props = if props_fields(&entry.props, static_rust).is_empty() {
                self.empty_props = true;
                "&__empty_props".to_string()
            } else {
                format!("&__props_{}_{}", index, position)
            };
            let style = if style_fields(&entry.style, static_rust).is_empty() {
                self.empty_style = true;
                "&__empty_style".to_string()
            } else {
                format!("&__style_{}_{}", index, position)
            };
            let type_name = match entry.type_name() {
                Some(name) => format!("Some({})", rust_string(name)),
                None => "None".to_string(),
            };
            let command = match entry.command() {
                Some(command) => format!("Some({command})"),
                None => "None".to_string(),
            };
            let parent = match entry.parent() {
                Some(parent) => format!("Some({})", rust_string(parent.name())),
                None => "None".to_string(),
            };
            let line = format!(
                "        CompiledEntry {{ id: {}, line: {}, path: {}, type_name: {}, command: {}, parent: {}, rect: __rect_{}_{}, props: {}, style: {}, content: {}, skin_attrs: {}, skin_children: {}, rust_children: {} }},",
                rust_string(entry.id().name()),
                entry.line(),
                rust_string(entry.path()),
                type_name,
                command,
                parent,
                index,
                position,
                props,
                style,
                content,
                skin_attrs,
                children,
                rust_children
            );
            writeln!(self.out, "{}", line).unwrap();
        }
        writeln!(self.out, "    ];").unwrap();
        writeln!(
            self.out,
            "    static __scrolls_{}: &[CompiledScroll] = &[",
            index
        )
        .unwrap();
        for (position, scroll) in scrolls.iter().enumerate() {
            let height = self.scroll_height(index, position, scroll);
            let line = format!(
                "        CompiledScroll {{ id: {}, line: {}, path: {}, rect: __rect_{}_{}, height: {} }},",
                rust_string(scroll.id().name()),
                scroll.line(),
                rust_string(scroll.path()),
                index,
                scroll_base + position,
                height
            );
            writeln!(self.out, "{}", line).unwrap();
        }
        writeln!(self.out, "    ];").unwrap();
        if self.context.is_some() {
            self.emit_values(index, &entries, &bindings);
        }
        writeln!(self.out).unwrap();
    }

    /// The per-variant context values (`context = Type`): one function that
    /// lists the entries whose inline Rust must be evaluated for this apply.
    fn emit_values(
        &mut self,
        index: usize,
        entries: &[&LayoutEntry],
        bindings: &[(usize, &str, &str)],
    ) {
        let context = self.context.clone().expect("context mode");
        let mut calls: Vec<String> = Vec::new();
        for (position, entry) in entries.iter().enumerate() {
            if !needs_values(entry) && binding_index(bindings, entry.id().name()).is_none() {
                continue;
            }
            calls.push(format!("__values_{index}_{position}(context)"));
            self.emit_values_entry(index, position, entry, bindings);
        }
        writeln!(
            self.out,
            "    fn __values_{index}(context: &{context}) -> Vec<CompiledEntryValues<'_>> {{"
        )
        .unwrap();
        writeln!(self.out, "        vec![{}]", calls.join(", ")).unwrap();
        writeln!(self.out, "    }}").unwrap();
    }

    fn emit_values_entry(
        &mut self,
        variant: usize,
        position: usize,
        entry: &LayoutEntry,
        bindings: &[(usize, &str, &str)],
    ) {
        let context = self.context.clone().expect("context mode");
        writeln!(
            self.out,
            "    // {} (line {}) — context values",
            entry.path(),
            entry.line()
        )
        .unwrap();
        writeln!(
            self.out,
            "    fn __values_{variant}_{position}(context: &{context}) -> CompiledEntryValues<'_> {{"
        )
        .unwrap();
        writeln!(self.out, "        CompiledEntryValues {{").unwrap();
        if geometry_has_rust(&entry.geometry) {
            writeln!(
                self.out,
                "            rect: Some(Box::new(move |s: Size| -> Result<Rect, RectError> {{"
            )
            .unwrap();
            let body = self.geometry_body(&entry.geometry, 16);
            self.out.push_str(&body);
            writeln!(self.out, "            }})),").unwrap();
        }
        if any_rust(&entry.rust_attrs, is_prop_name) {
            let fields = props_fields(&entry.props, &entry.rust_attrs);
            writeln!(
                self.out,
                "            props: Some(PropAttrs {{ {fields}..PropAttrs::default() }}),"
            )
            .unwrap();
        }
        if any_rust(&entry.rust_attrs, is_style_name) {
            let fields = style_fields(&entry.style, &entry.rust_attrs);
            writeln!(
                self.out,
                "            style: Some(StyleAttrs {{ {fields}..StyleAttrs::default() }}),"
            )
            .unwrap();
        }
        if any_rust(&entry.rust_attrs, is_content_name) {
            let content = content_attrs(&entry.content, &entry.rust_attrs);
            writeln!(self.out, "            content: Some({content}),").unwrap();
        }
        if let Some(binding) = binding_index(bindings, entry.id().name()) {
            writeln!(
                self.out,
                "            children: Some({}),",
                bindings[binding].2
            )
            .unwrap();
        }
        writeln!(
            self.out,
            "            ..CompiledEntryValues::new({})",
            rust_string(entry.id().name())
        )
        .unwrap();
        writeln!(self.out, "        }}").unwrap();
        writeln!(self.out, "    }}").unwrap();
        writeln!(self.out).unwrap();
    }

    fn emit_entry(&mut self, variant: usize, position: usize, entry: &LayoutEntry) {
        let context_mode = self.context.is_some();
        if geometry_has_rust(&entry.geometry) && context_mode {
            // The static descriptor cannot see the context; the generated
            // install supplies the rectangle (see `emit_values_entry`).
            writeln!(
                self.out,
                "    fn __rect_{}_{}(s: Size) -> Result<Rect, RectError> {{",
                variant, position
            )
            .unwrap();
            writeln!(self.out, "        let _ = s;").unwrap();
            writeln!(self.out, "        Err(RectError::Context)").unwrap();
            writeln!(self.out, "    }}").unwrap();
            writeln!(self.out).unwrap();
        } else {
            self.emit_geometry_function(
                &format!("__rect_{}_{}", variant, position),
                &entry.geometry,
                entry.line(),
                entry.path(),
            );
        }
        // A context-computed property/style is not in the statics.
        let rust: &[(&'static str, String)] = if context_mode {
            &[]
        } else {
            &entry.rust_attrs
        };
        self.emit_props(variant, position, &entry.props, rust);
        self.emit_style(variant, position, &entry.style, rust);
    }

    fn emit_scroll(&mut self, variant: usize, position: usize, scroll: &ScrollEntry) {
        let rect = format!(
            "__rect_{}_{}",
            variant,
            self.document.variants()[variant].entries().len() + position
        );
        self.emit_geometry_function(&rect, &scroll.geometry, scroll.line(), scroll.path());
        if let ContentHeight::Exact(expression) = scroll.content_height() {
            writeln!(
                self.out,
                "    fn __height_{}_{}(s: Size) -> Option<f32> {{",
                variant, position
            )
            .unwrap();
            writeln!(
                self.out,
                "        let value: f64 = {};",
                expr_source(expression, Axis::Vertical, self.document.constants())
            )
            .unwrap();
            writeln!(self.out, "        if value.is_finite() && value >= 0.0 {{").unwrap();
            writeln!(self.out, "            Some(value as f32)").unwrap();
            writeln!(self.out, "        }} else {{").unwrap();
            writeln!(self.out, "            None").unwrap();
            writeln!(self.out, "        }}").unwrap();
            writeln!(self.out, "    }}").unwrap();
            writeln!(self.out).unwrap();
        }
    }

    fn scroll_height(&self, variant: usize, position: usize, scroll: &ScrollEntry) -> String {
        match scroll.content_height() {
            ContentHeight::Keep => "CompiledHeight::Keep".to_string(),
            ContentHeight::Exact(_) => {
                format!("CompiledHeight::Exact(__height_{}_{})", variant, position)
            }
            ContentHeight::Auto => "CompiledHeight::Auto".to_string(),
        }
    }

    fn emit_props(
        &mut self,
        variant: usize,
        position: usize,
        props: &PropAttrs,
        rust: &[(&'static str, String)],
    ) {
        let fields = props_fields(props, rust);
        if fields.is_empty() {
            self.empty_props = true;
            return;
        }
        writeln!(
            self.out,
            "    static __props_{}_{}: LazyLock<PropAttrs> = LazyLock::new(|| PropAttrs {{ {}..PropAttrs::default() }});",
            variant,
            position,
            fields
        )
        .unwrap();
    }

    fn emit_style(
        &mut self,
        variant: usize,
        position: usize,
        style: &StyleAttrs,
        rust: &[(&'static str, String)],
    ) {
        let fields = style_fields(style, rust);
        if fields.is_empty() {
            self.empty_style = true;
            return;
        }
        writeln!(
            self.out,
            "    static __style_{}_{}: LazyLock<StyleAttrs> = LazyLock::new(|| StyleAttrs {{ {}..StyleAttrs::default() }});",
            variant,
            position,
            fields
        )
        .unwrap();
    }

    fn emit_geometry_function(
        &mut self,
        name: &str,
        geometry: &Geometry,
        line: usize,
        path: &str,
    ) {
        writeln!(self.out, "    // {} (line {})", path, line).unwrap();
        writeln!(
            self.out,
            "    fn {}(s: Size) -> Result<Rect, RectError> {{",
            name
        )
        .unwrap();
        let body = self.geometry_body(geometry, 8);
        self.out.push_str(&body);
        writeln!(self.out, "    }}").unwrap();
        writeln!(self.out).unwrap();
    }

    /// The statements of one rectangle expression, every line indented by
    /// `indent` spaces; the last line is the `edges(..)` expression. The same
    /// body is spliced into a static `fn` and into a captured closure, so the
    /// two forms stay bit-identical.
    fn geometry_body(&self, geometry: &Geometry, indent: usize) -> String {
        let pad = " ".repeat(indent);
        let mut out = String::new();
        // Inline Rust reads the schema's space variables as f64 locals.
        writeln!(out, "{pad}let cw = s.width as f64;").unwrap();
        writeln!(out, "{pad}let ch = s.height as f64;").unwrap();
        writeln!(out, "{pad}let cx = cw / 2.0;").unwrap();
        writeln!(out, "{pad}let cy = ch / 2.0;").unwrap();
        let constants = self.document.constants();
        for (label, expression) in horizontal_values(&geometry.horizontal, constants) {
            writeln!(out, "{pad}let {} = {};", label, expression).unwrap();
        }
        for (label, expression) in vertical_values(&geometry.vertical, constants) {
            writeln!(out, "{pad}let {} = {};", label, expression).unwrap();
        }
        match &geometry.horizontal {
            Horizontal::LeftWidth { .. } => {
                writeln!(
                    out,
                    "{pad}if w < 0.0 {{ return Err(RectError::Negative(\"w\")); }}"
                )
                .unwrap();
                writeln!(out, "{pad}let left = x;").unwrap();
                writeln!(out, "{pad}let right = x + w;").unwrap();
            }
            Horizontal::RightWidth { .. } => {
                writeln!(
                    out,
                    "{pad}if right < 0.0 {{ return Err(RectError::Negative(\"right\")); }}"
                )
                .unwrap();
                writeln!(
                    out,
                    "{pad}if w < 0.0 {{ return Err(RectError::Negative(\"w\")); }}"
                )
                .unwrap();
                writeln!(out, "{pad}let left = s.width as f64 - right - w;").unwrap();
                writeln!(out, "{pad}let right = s.width as f64 - right;").unwrap();
            }
            Horizontal::LeftRight { .. } => {
                writeln!(
                    out,
                    "{pad}if right < 0.0 {{ return Err(RectError::Negative(\"right\")); }}"
                )
                .unwrap();
                writeln!(out, "{pad}let left = x;").unwrap();
                writeln!(out, "{pad}let right = s.width as f64 - right;").unwrap();
                writeln!(
                    out,
                    "{pad}if right < left {{ return Err(RectError::Negative(\"w\")); }}"
                )
                .unwrap();
            }
        }
        match &geometry.vertical {
            Vertical::TopHeight { .. } => {
                writeln!(
                    out,
                    "{pad}if h < 0.0 {{ return Err(RectError::Negative(\"h\")); }}"
                )
                .unwrap();
                writeln!(out, "{pad}let top = y;").unwrap();
                writeln!(out, "{pad}let bottom = y + h;").unwrap();
            }
            Vertical::BottomHeight { .. } => {
                writeln!(
                    out,
                    "{pad}if bottom < 0.0 {{ return Err(RectError::Negative(\"bottom\")); }}"
                )
                .unwrap();
                writeln!(
                    out,
                    "{pad}if h < 0.0 {{ return Err(RectError::Negative(\"h\")); }}"
                )
                .unwrap();
                writeln!(out, "{pad}let top = s.height as f64 - bottom - h;").unwrap();
                writeln!(out, "{pad}let bottom = s.height as f64 - bottom;").unwrap();
            }
            Vertical::TopBottom { .. } => {
                writeln!(
                    out,
                    "{pad}if bottom < 0.0 {{ return Err(RectError::Negative(\"bottom\")); }}"
                )
                .unwrap();
                writeln!(out, "{pad}let top = y;").unwrap();
                writeln!(out, "{pad}let bottom = s.height as f64 - bottom;").unwrap();
                writeln!(
                    out,
                    "{pad}if bottom < top {{ return Err(RectError::Negative(\"h\")); }}"
                )
                .unwrap();
            }
        }
        writeln!(out, "{pad}edges(left, top, right, bottom)").unwrap();
        out
    }

    fn content_slice(
        &self,
        content: &[ContentAttr],
        rust: &[(&'static str, String)],
    ) -> String {
        if content.is_empty() && !rust.iter().any(|(name, _)| is_content_name(name)) {
            return "&[]".to_string();
        }
        let mut out = String::from("&[");
        for (index, item) in content.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&match item {
                ContentAttr::Text(text) => format!("CompiledContent::Text({})", rust_string(text)),
                ContentAttr::Label(text) => {
                    format!("CompiledContent::Label({})", rust_string(text))
                }
                ContentAttr::Value(value) => {
                    format!("CompiledContent::Value({})", number64(*value))
                }
                ContentAttr::Progress(value) => {
                    format!("CompiledContent::Progress({})", number64(*value))
                }
                ContentAttr::Number(value) => format!("CompiledContent::Number({})", value),
                ContentAttr::Checked(value) => format!("CompiledContent::Checked({})", value),
                ContentAttr::Selected(value) => format!("CompiledContent::Selected({})", value),
            });
        }
        for (name, code) in rust.iter().filter(|(name, _)| is_content_name(name)) {
            if out.len() > 2 {
                out.push_str(", ");
            }
            out.push_str(&match *name {
                "text" => format!("CompiledContent::Text({code})"),
                "label" => format!("CompiledContent::Label({code})"),
                "value" => format!("CompiledContent::Value({code})"),
                "progress" => format!("CompiledContent::Progress({code})"),
                "number" => format!("CompiledContent::Number({code})"),
                "checked" => format!("CompiledContent::Checked({code})"),
                _ => format!("CompiledContent::Selected({code})"),
            });
        }
        out.push(']');
        out
    }

    fn skin_attrs(&self, attrs: &[(String, String)]) -> String {
        if attrs.is_empty() {
            return "&[]".to_string();
        }
        let mut out = String::from("&[");
        for (index, (name, value)) in attrs.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push('(');
            out.push_str(&rust_string(name));
            out.push_str(", ");
            out.push_str(&rust_string(value));
            out.push(')');
        }
        out.push(']');
        out
    }

    fn emit_elements(&mut self, list: &[SkinElement]) -> String {
        if list.is_empty() {
            return "&[]".to_string();
        }
        let name = format!("__elems_{}", self.elements);
        self.elements += 1;
        let mut body = String::new();
        writeln!(body, "    static {}: &[CompiledElement] = &[", name).unwrap();
        for element in list {
            let children = self.emit_elements(&element.children);
            let attrs = element
                .attrs
                .iter()
                .map(|(name, value)| format!("({}, {})", rust_string(name), rust_string(value)))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(
                body,
                "        CompiledElement {{ name: {}, line: {}, column: {}, attrs: &[{}], children: {} }},",
                rust_string(&element.name),
                element.line,
                element.column,
                attrs,
                children
            )
            .unwrap();
        }
        writeln!(body, "    ];").unwrap();
        self.extra.push_str(&body);
        name
    }

    fn emit_targets(&mut self) {
        let document: &'a LayoutDocument = self.document;
        let mut targets: Vec<(&'static str, usize, String, bool)> = Vec::new();
        for variant in document.variants() {
            for entry in variant.entries() {
                push_target(&mut targets, entry.id().name(), entry.line(), entry.path(), false);
            }
            for scroll in variant.scrolls() {
                push_target(&mut targets, scroll.id().name(), scroll.line(), scroll.path(), true);
            }
        }
        writeln!(self.out, "    static __targets: &[CompiledTarget] = &[").unwrap();
        for (id, line, path, scroll) in targets {
            writeln!(
                self.out,
                "        CompiledTarget {{ id: {}, kind: TargetKind::{}, line: {}, path: {} }},",
                rust_string(id),
                if scroll { "Scroll" } else { "Widget" },
                line,
                rust_string(&path)
            )
            .unwrap();
        }
        writeln!(self.out, "    ];").unwrap();
        writeln!(self.out).unwrap();
    }
}

fn push_target(
    targets: &mut Vec<(&'static str, usize, String, bool)>,
    id: &'static str,
    line: usize,
    path: &str,
    scroll: bool,
) {
    if targets
        .iter()
        .any(|target| target.0 == id && target.3 == scroll)
    {
        return;
    }
    targets.push((id, line, path.to_string(), scroll));
}

fn horizontal_values(
    form: &Horizontal,
    constants: &LayoutConstants,
) -> Vec<(&'static str, String)> {
    let (first, second) = match form {
        Horizontal::LeftWidth { x, w } => (("x", x), ("w", w)),
        Horizontal::RightWidth { right, w } => (("right", right), ("w", w)),
        Horizontal::LeftRight { x, right } => (("x", x), ("right", right)),
    };
    vec![
        (first.0, expr_source(first.1, Axis::Horizontal, constants)),
        (second.0, expr_source(second.1, Axis::Horizontal, constants)),
    ]
}

fn vertical_values(form: &Vertical, constants: &LayoutConstants) -> Vec<(&'static str, String)> {
    let (first, second) = match form {
        Vertical::TopHeight { y, h } => (("y", y), ("h", h)),
        Vertical::BottomHeight { bottom, h } => (("bottom", bottom), ("h", h)),
        Vertical::TopBottom { y, bottom } => (("y", y), ("bottom", bottom)),
    };
    vec![
        (first.0, expr_source(first.1, Axis::Vertical, constants)),
        (second.0, expr_source(second.1, Axis::Vertical, constants)),
    ]
}

fn expr_source(expression: &Expr, axis: Axis, constants: &LayoutConstants) -> String {
    match expression {
        Expr::Num(value) => format!("({})", number64(*value)),
        Expr::Var(var) => match var {
            Var::Left | Var::Top => "(0.0)".to_string(),
            Var::Right | Var::Cw => "(s.width as f64)".to_string(),
            Var::Bottom | Var::Ch => "(s.height as f64)".to_string(),
            Var::Cx => "((s.width as f64) / 2.0)".to_string(),
            Var::Cy => "((s.height as f64) / 2.0)".to_string(),
        },
        Expr::Const(name) => match constants.get(name) {
            Some(value) => format!("({})", number64(value)),
            None => "(0.0)".to_string(),
        },
        Expr::Percent(value) => {
            let base = match axis {
                Axis::Horizontal => "(s.width as f64)",
                Axis::Vertical => "(s.height as f64)",
            };
            format!("({} / 100.0 * {})", number64(*value), base)
        }
        Expr::Rust(code) => format!("({code})"),
        Expr::Neg(inner) => format!("(-{})", expr_source(inner, axis, constants)),
        Expr::Bin(operator, left, right) => {
            let symbol = match operator {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
            };
            format!(
                "({} {} {})",
                expr_source(left, axis, constants),
                symbol,
                expr_source(right, axis, constants)
            )
        }
    }
}

fn props_fields(props: &PropAttrs, rust: &[(&'static str, String)]) -> String {
    let mut fields = String::new();
    let mut push = |name: &str, value: String| {
        fields.push_str(&format!("{}: {}, ", name, value));
    };
    if let Some(value) = props.visible {
        push("visible", format!("Some({})", value));
    }
    if let Some(value) = props.enabled {
        push("enabled", format!("Some({})", value));
    }
    if let Some(value) = props.interactive {
        push("interactive", format!("Some({})", value));
    }
    if let Some(value) = props.focusable {
        push("focusable", format!("Some({})", value));
    }
    if let Some(value) = props.focus_order {
        push("focus_order", format!("Some({})", value));
    }
    if let Some(value) = props.read_only {
        push("read_only", format!("Some({})", value));
    }
    if let Some(value) = props.tooltip.as_ref() {
        push("tooltip", format!("Some({}.to_string())", rust_string(value)));
    }
    if let Some(value) = props.name.as_ref() {
        push("name", format!("Some({}.to_string())", rust_string(value)));
    }
    if let Some(value) = props.description.as_ref() {
        push(
            "description",
            format!("Some({}.to_string())", rust_string(value)),
        );
    }
    if let Some(value) = props.automation_id.as_ref() {
        push(
            "automation_id",
            format!("Some({}.to_string())", rust_string(value)),
        );
    }
    if let Some(value) = props.role {
        push("role", format!("Some(A11yRole::{})", role_name(value)));
    }
    if let Some(value) = props.expose {
        push("expose", format!("Some({})", value));
    }
    if let Some(value) = props.cursor {
        push("cursor", format!("Some(Cursor::{})", cursor_name(value)));
    }
    for (name, code) in rust {
        let field = match *name {
            "visible" => Some(("visible", format!("Some({code})"))),
            "enabled" => Some(("enabled", format!("Some({code})"))),
            "interactive" => Some(("interactive", format!("Some({code})"))),
            "focusable" => Some(("focusable", format!("Some({code})"))),
            "focus-order" => Some(("focus_order", format!("Some({code})"))),
            "read-only" => Some(("read_only", format!("Some({code})"))),
            "tooltip" => Some(("tooltip", format!("Some(({code}).to_string())"))),
            "name" => Some(("name", format!("Some(({code}).to_string())"))),
            "description" => Some(("description", format!("Some(({code}).to_string())"))),
            "automation-id" => Some(("automation_id", format!("Some(({code}).to_string())"))),
            "role" => Some(("role", format!("Some({code})"))),
            "expose" => Some(("expose", format!("Some({code})"))),
            "cursor" => Some(("cursor", format!("Some({code})"))),
            _ => None,
        };
        if let Some((field, value)) = field {
            push(field, value);
        }
    }
    fields
}

fn style_fields(style: &StyleAttrs, rust: &[(&'static str, String)]) -> String {
    let mut fields = String::new();
    if let Some(radius) = style.radius {
        let value = match radius {
            RadiusAttr::Dips(value) => format!("RadiusAttr::Dips({})", number32(value)),
            RadiusAttr::Small => "RadiusAttr::Small".to_string(),
            RadiusAttr::Medium => "RadiusAttr::Medium".to_string(),
            RadiusAttr::Large => "RadiusAttr::Large".to_string(),
            RadiusAttr::Pill => "RadiusAttr::Pill".to_string(),
        };
        fields.push_str(&format!("radius: Some({}), ", value));
    }
    if let Some(corners) = style.corners {
        fields.push_str(&format!(
            "corners: Some(Corners::new({}, {}, {}, {})), ",
            number32(corners.top_left),
            number32(corners.top_right),
            number32(corners.bottom_right),
            number32(corners.bottom_left)
        ));
    }
    if let Some(background) = style.background {
        let value = match background {
            BackgroundAttr::Token(token) => {
                format!("BackgroundAttr::Token({})", rust_string(token))
            }
            BackgroundAttr::Literal(color) => format!(
                "BackgroundAttr::Literal(Color::new({}, {}, {}))",
                color.r, color.g, color.b
            ),
            BackgroundAttr::None => "BackgroundAttr::None".to_string(),
        };
        fields.push_str(&format!("background: Some({}), ", value));
    }
    if let Some(size) = style.font_size {
        fields.push_str(&format!("font_size: Some({}), ", number32(size)));
    }
    if let Some(family) = style.font_family.as_ref() {
        fields.push_str(&format!(
            "font_family: Some({}.to_string()), ",
            rust_string(family)
        ));
    }
    for (name, code) in rust {
        let field = match *name {
            "radius" => Some(("radius", format!("Some({code})"))),
            "corners" => Some(("corners", format!("Some({code})"))),
            "background" => Some(("background", format!("Some({code})"))),
            "font-size" => Some(("font_size", format!("Some({code})"))),
            "font-family" => Some(("font_family", format!("Some(({code}).to_string())"))),
            _ => None,
        };
        if let Some((field, value)) = field {
            fields.push_str(&format!("{}: {}, ", field, value));
        }
    }
    fields
}

fn is_content_name(name: &str) -> bool {
    matches!(
        name,
        "text" | "label" | "value" | "progress" | "number" | "checked" | "selected"
    )
}

/// The generic `WidgetProps` scalars (the schema's property vocabulary).
fn is_prop_name(name: &str) -> bool {
    matches!(
        name,
        "visible"
            | "enabled"
            | "interactive"
            | "focusable"
            | "focus-order"
            | "read-only"
            | "tooltip"
            | "name"
            | "description"
            | "automation-id"
            | "role"
            | "expose"
            | "cursor"
    )
}

/// The surface/font scalars.
fn is_style_name(name: &str) -> bool {
    matches!(
        name,
        "radius" | "corners" | "background" | "font-size" | "font-family"
    )
}

fn any_rust(rust: &[(&'static str, String)], predicate: fn(&str) -> bool) -> bool {
    rust.iter().any(|(name, _)| predicate(name))
}

/// Whether an entry declares any inline-Rust scalar (properties, style or
/// content). A binding is handled separately by the caller.
fn has_rust_scalar(entry: &LayoutEntry) -> bool {
    entry
        .rust_attrs
        .iter()
        .any(|(name, _)| is_prop_name(name) || is_style_name(name) || is_content_name(name))
}

/// Whether the entry needs context-computed values: a scalar or its rectangle
/// reads inline Rust.
fn needs_values(entry: &LayoutEntry) -> bool {
    has_rust_scalar(entry) || geometry_has_rust(&entry.geometry)
}

/// The position of the `<rust>` binding that targets `id`, if any.
fn binding_index(bindings: &[(usize, &str, &str)], id: &str) -> Option<usize> {
    bindings
        .iter()
        .position(|(_, target, _)| *target == id)
}

fn geometry_has_rust(geometry: &Geometry) -> bool {
    let horizontal = match &geometry.horizontal {
        Horizontal::LeftWidth { x, w } => [x, w],
        Horizontal::RightWidth { right, w } => [right, w],
        Horizontal::LeftRight { x, right } => [x, right],
    };
    let vertical = match &geometry.vertical {
        Vertical::TopHeight { y, h } => [y, h],
        Vertical::BottomHeight { bottom, h } => [bottom, h],
        Vertical::TopBottom { y, bottom } => [y, bottom],
    };
    horizontal.into_iter().any(expr_has_rust) || vertical.into_iter().any(expr_has_rust)
}

fn expr_has_rust(expression: &Expr) -> bool {
    match expression {
        Expr::Rust(_) => true,
        Expr::Neg(inner) => expr_has_rust(inner),
        Expr::Bin(_, left, right) => expr_has_rust(left) || expr_has_rust(right),
        _ => false,
    }
}

/// The owned [`ContentAttr`] list of a context module: literals first, then the
/// inline-Rust values, in the order the static table uses.
fn content_attrs(content: &[ContentAttr], rust: &[(&'static str, String)]) -> String {
    let mut items: Vec<String> = content
        .iter()
        .map(|item| match item {
            ContentAttr::Text(text) => {
                format!("ContentAttr::Text({}.to_string())", rust_string(text))
            }
            ContentAttr::Label(text) => {
                format!("ContentAttr::Label({}.to_string())", rust_string(text))
            }
            ContentAttr::Value(value) => format!("ContentAttr::Value({})", number64(*value)),
            ContentAttr::Progress(value) => {
                format!("ContentAttr::Progress({})", number64(*value))
            }
            ContentAttr::Number(value) => format!("ContentAttr::Number({})", value),
            ContentAttr::Checked(value) => format!("ContentAttr::Checked({})", value),
            ContentAttr::Selected(value) => format!("ContentAttr::Selected({})", value),
        })
        .collect();
    for (name, code) in rust.iter().filter(|(name, _)| is_content_name(name)) {
        items.push(match *name {
            "text" => format!("ContentAttr::Text(({code}).to_string())"),
            "label" => format!("ContentAttr::Label(({code}).to_string())"),
            "value" => format!("ContentAttr::Value({code})"),
            "progress" => format!("ContentAttr::Progress({code})"),
            "number" => format!("ContentAttr::Number({code})"),
            "checked" => format!("ContentAttr::Checked({code})"),
            _ => format!("ContentAttr::Selected({code})"),
        });
    }
    format!("vec![{}]", items.join(", "))
}

fn role_name(role: A11yRole) -> &'static str {
    match role {
        A11yRole::Button => "Button",
        A11yRole::Toggle => "Toggle",
        A11yRole::Slider => "Slider",
        A11yRole::TextField => "TextField",
        A11yRole::NumberField => "NumberField",
        A11yRole::RadioGroup => "RadioGroup",
        A11yRole::TabList => "TabList",
        A11yRole::List => "List",
        A11yRole::MultiSelectList => "MultiSelectList",
        A11yRole::RadioItem => "RadioItem",
        A11yRole::TabItem => "TabItem",
        A11yRole::ListItem => "ListItem",
        A11yRole::Tree => "Tree",
        A11yRole::TreeItem => "TreeItem",
        A11yRole::Grid => "Grid",
        A11yRole::GridCell => "GridCell",
        A11yRole::ColumnHeader => "ColumnHeader",
        A11yRole::Heading => "Heading",
        A11yRole::Group => "Group",
        A11yRole::Separator => "Separator",
        A11yRole::Text => "Text",
        A11yRole::ComboBox => "ComboBox",
        A11yRole::ProgressBar => "ProgressBar",
        A11yRole::MenuItem => "MenuItem",
        A11yRole::Menu => "Menu",
        A11yRole::MenuBar => "MenuBar",
        A11yRole::ToolBar => "ToolBar",
        A11yRole::StatusBar => "StatusBar",
    }
}

fn cursor_name(cursor: Cursor) -> &'static str {
    match cursor {
        Cursor::Arrow => "Arrow",
        Cursor::Hand => "Hand",
        Cursor::IBeam => "IBeam",
        Cursor::SizeWE => "SizeWE",
        Cursor::SizeNS => "SizeNS",
    }
}

/// A Rust string literal with every non-printable character escaped.
fn rust_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{{{:x}}}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Shortest round-tripping `f64` literal (`18` → `18.0`).
fn number64(value: f64) -> String {
    let text = format!("{:?}", value);
    if text.contains('.') || text.contains('e') || text.contains('E') {
        text
    } else {
        format!("{text}.0")
    }
}

/// Shortest round-tripping `f32` literal.
fn number32(value: f32) -> String {
    let text = format!("{:?}", value);
    if text.contains('.') || text.contains('e') || text.contains('E') {
        text
    } else {
        format!("{text}.0")
    }
}
