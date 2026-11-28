// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The compiled consumption mode (`layout-compiled`, D128).
//!
//! `bamboo-native-ui-layout-codegen` turns a `.buixml` file into a Rust module
//! that describes the same document as `static` data — rectangles as ordinary
//! functions, attributes as `LazyLock` statics — and calls
//! [`Ui::apply_compiled`]. There is no XML parser and no expression interpreter
//! in the resulting binary: the compiler folds the constants and `rustc`
//! type-checks every value.
//!
//! The runtime path is [`Ui::install_prepared`], the same one
//! `Ui::apply_layout` uses, so claims, content timing, the child/content/
//! attribute order and the damage flush behave identically; the module here
//! only resolves the descriptor into the shared prepared plan.
//!
//! Generated code is checked in and `include!`d (or produced by a proc macro
//! later). The shape is the generator's business; the stable part is this
//! vocabulary:
//!
//! - [`CompiledLayout`] — name, generation, variants and preflight targets.
//! - [`CompiledEntry`] — one widget: a rectangle function plus its attributes.
//! - [`RectFn`] / [`rect`] — the rectangle expression, evaluated natively.
//! - [`CompiledContent`] / [`CompiledHeight`] — the remaining scalar rules.
//!
//! Inline Rust (`rust:` values) belongs to this mode: the generator splices the
//! text into the generated module, where `rustc` checks it; the dynamic parser
//! rejects it so a runtime file stays portable data.

use std::sync::LazyLock;

use crate::geometry::{Rect, Size};
use crate::layout_xml::apply::{ClaimFields, LayoutFailure, LayoutReport, PanelHeight, PanelPlan, Prepared, PreparedWidget};
use crate::layout_xml::diag::{Diagnostics, LayoutDiagnostic};
use crate::layout_xml::model::{ContentAttr, EntrySpace, PropAttrs, StyleAttrs};
use crate::props::ContentCaps;
use crate::skin::SkinElement;
use crate::theme::Palette;
use crate::ui::{Ui, UiHost};
use crate::widget::WidgetId;

/// One rectangle expression: a native function over the space size.
///
/// The generator emits a named `fn` per entry, e.g.
/// `fn v0_e3_rect(s: Size) -> Result<Rect, RectError> { rect(12.0, 12.0, s.width - 24.0, 30.0) }`.
pub type RectFn = fn(Size) -> Result<Rect, RectError>;

/// Why a compiled rectangle could not be resolved at this client size.
///
/// Expressions are checked by the generator where they are constant; this
/// catches the runtime-domain failures (a zero divisor, a stretched side that
/// went negative).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RectError {
    /// A width or height expression went below zero.
    Negative(&'static str),
    /// A value is not finite (a division by zero at this size).
    NotFinite,
    /// The rectangle is computed from the layout context (`context = Type`), so
    /// it exists only through the generated `install`, never on the static
    /// descriptor.
    Context,
}

/// Build a rectangle from `x`, `y`, `width`, `height`, rejecting a negative
/// width/height side and a non-finite value. The generated functions call this.
pub fn rect(x: f32, y: f32, width: f32, height: f32) -> Result<Rect, RectError> {
    if !(x.is_finite() && y.is_finite() && width.is_finite() && height.is_finite()) {
        return Err(RectError::NotFinite);
    }
    if width < 0.0 {
        return Err(RectError::Negative("w"));
    }
    if height < 0.0 {
        return Err(RectError::Negative("h"));
    }
    Ok(Rect::from_xywh(x, y, width, height))
}

/// Build a rectangle from its edges in f64, then cast — the dynamic resolver
/// computes in f64 and converts the four edges to f32, so this is the exact
/// same rounding. A non-finite edge is rejected.
pub fn edges(left: f64, top: f64, right: f64, bottom: f64) -> Result<Rect, RectError> {
    if !(left.is_finite() && top.is_finite() && right.is_finite() && bottom.is_finite()) {
        return Err(RectError::NotFinite);
    }
    Ok(Rect::new(
        left as f32,
        top as f32,
        right as f32,
        bottom as f32,
    ))
}

/// What an id in [`CompiledLayout::targets`] must be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    Widget,
    Scroll,
}

/// One preflight target: an id the document mentions, checked before anything
/// mutates (the atomicity promise, §8.3).
#[derive(Clone, Copy, Debug)]
pub struct CompiledTarget {
    pub id: &'static str,
    pub kind: TargetKind,
    /// 1-based source line in the `.buixml` file.
    pub line: usize,
    /// Source path (`layout/wide/m7_action`), for diagnostics.
    pub path: &'static str,
}

/// One `<layout>` breakpoint variant.
pub struct CompiledVariant {
    pub id: &'static str,
    pub min_width: f32,
    pub min_height: f32,
    pub entries: &'static [CompiledEntry],
    pub scrolls: &'static [CompiledScroll],
}

/// One `<widget>` entry.
pub struct CompiledEntry {
    pub id: &'static str,
    /// 1-based source line in the `.buixml` file.
    pub line: usize,
    pub path: &'static str,
    pub rect: RectFn,
    /// `type="…"`: the registry type that creates this widget when the id does
    /// not exist (M7.7). `None` is an address-only entry.
    pub type_name: Option<&'static str>,
    /// `command="…"`: the application command a created interactive widget
    /// reports.
    pub command: Option<u32>,
    /// `parent="…"`: the scroll panel the entry belongs to (M7.7
    /// follow-up): membership for a created widget, an assertion for an
    /// addressed one.
    pub parent: Option<&'static str>,
    /// Declared properties. A shared static (usually empty) so an apply does not
    /// rebuild them.
    pub props: &'static LazyLock<PropAttrs>,
    /// Declared surface/font tokens.
    pub style: &'static LazyLock<StyleAttrs>,
    /// Static content initial values, in source order.
    pub content: &'static [CompiledContent],
    /// Widget-declared local attributes (raw strings, source order).
    pub skin_attrs: &'static [(&'static str, &'static str)],
    /// Widget-declared data child elements.
    pub skin_children: &'static [CompiledElement],
    /// A `<rust target=...>` child-list binding in a module without a context:
    /// the produced list replaces `skin_children`. A module generated with
    /// `context = Type` computes the list per apply instead (see
    /// [`CompiledValues`]).
    pub rust_children: Option<&'static LazyLock<Vec<SkinElement>>>,
}

/// One data child element as static data; [`elements`] turns a list into the
/// owned [`SkinElement`] tree the widget protocol takes.
#[derive(Clone, Copy, Debug)]
pub struct CompiledElement {
    pub name: &'static str,
    pub line: usize,
    pub column: usize,
    pub attrs: &'static [(&'static str, &'static str)],
    pub children: &'static [CompiledElement],
}

/// Materialize a static element list into the owned tree.
pub fn elements(list: &[CompiledElement]) -> Vec<SkinElement> {
    list.iter()
        .map(|element| {
            let mut built = SkinElement::new(element.name, element.line, element.column);
            for (name, value) in element.attrs {
                built.push_attr(*name, *value);
            }
            for child in elements(element.children) {
                built.push_child(child);
            }
            built
        })
        .collect()
}

/// One `<scroll>` entry.
pub struct CompiledScroll {
    pub id: &'static str,
    pub line: usize,
    pub path: &'static str,
    pub rect: RectFn,
    pub height: CompiledHeight,
}

/// `<scroll content-height>` in compiled form.
#[derive(Clone, Copy)]
pub enum CompiledHeight {
    Keep,
    /// A native function over the post-viewport content extent.
    Exact(fn(Size) -> Option<f32>),
    Auto,
}

/// One static content value with `&'static` text, so the descriptor is a plain
/// static. Converted to the model's owned [`ContentAttr`] only when a
/// generation actually writes content.
#[derive(Clone, Copy, Debug)]
pub enum CompiledContent {
    Text(&'static str),
    Label(&'static str),
    Value(f64),
    Progress(f64),
    Number(i64),
    Checked(bool),
    Selected(u32),
}

impl CompiledContent {
    fn to_attr(self) -> ContentAttr {
        match self {
            Self::Text(text) => ContentAttr::Text(text.to_string()),
            Self::Label(text) => ContentAttr::Label(text.to_string()),
            Self::Value(value) => ContentAttr::Value(value),
            Self::Progress(value) => ContentAttr::Progress(value),
            Self::Number(value) => ContentAttr::Number(value),
            Self::Checked(value) => ContentAttr::Checked(value),
            Self::Selected(value) => ContentAttr::Selected(value),
        }
    }
}

/// Per-apply values for a module generated with `context = Type` (§16.2).
///
/// Inline Rust that reads the context cannot live in a static, so the generated
/// `install` computes the affected entries on every apply and passes them here.
/// An entry without an [`CompiledEntryValues`] keeps the descriptor's statics,
/// so a plain document allocates nothing on this path.
#[derive(Default)]
pub struct CompiledValues<'a> {
    /// One list per [`CompiledLayout::variants`] index; missing indices and ids
    /// fall back to the static descriptor.
    pub variants: Vec<Vec<CompiledEntryValues<'a>>>,
}

impl<'a> CompiledValues<'a> {
    /// Whether no entry has context-computed values.
    pub fn is_empty(&self) -> bool {
        self.variants.iter().all(Vec::is_empty)
    }

    fn for_variant(&self, index: usize) -> &[CompiledEntryValues<'a>] {
        self.variants.get(index).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// One entry's context-computed values; each `Some` replaces the descriptor's
/// static field, `None` keeps it.
pub struct CompiledEntryValues<'a> {
    pub id: &'static str,
    /// A context-dependent rectangle (the static `RectFn` reports
    /// [`RectError::Context`] for such an entry, so it must come from here).
    pub rect: Option<Box<dyn Fn(Size) -> Result<Rect, RectError> + 'a>>,
    pub props: Option<PropAttrs>,
    pub style: Option<StyleAttrs>,
    /// The **complete** content list (literals and inline Rust, in the model's
    /// order): a context-computed list replaces the static table as a whole.
    pub content: Option<Vec<ContentAttr>>,
    /// The `<rust target=...>` child list produced for this apply.
    pub children: Option<Vec<SkinElement>>,
}

impl<'a> CompiledEntryValues<'a> {
    /// An empty value set for one entry id, for struct-update syntax.
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            rect: None,
            props: None,
            style: None,
            content: None,
            children: None,
        }
    }
}

/// A whole compiled document: what the codegen emits as one `static`.
pub struct CompiledLayout {
    /// Source file name, for diagnostics (`layout-demo.buixml`).
    pub name: &'static str,
    /// FNV-1a of the canonical text; content is written once per generation.
    pub generation: u64,
    pub variants: &'static [CompiledVariant],
    /// Every id the file mentions (all variants), for the preflight walk.
    pub targets: &'static [CompiledTarget],
}

impl CompiledLayout {
    /// The index (into [`CompiledLayout::variants`]) that applies at `client`:
    /// the same rule as [`crate::layout_xml::LayoutDocument::select`].
    pub fn select_index(&self, client: Size) -> usize {
        let mut best: Option<usize> = None;
        for (index, variant) in self.variants.iter().enumerate() {
            if variant.min_width > client.width || variant.min_height > client.height {
                continue;
            }
            best = match best {
                None => Some(index),
                Some(current) => {
                    let current_variant = &self.variants[current];
                    if (variant.min_width, variant.min_height)
                        > (current_variant.min_width, current_variant.min_height)
                    {
                        Some(index)
                    } else {
                        Some(current)
                    }
                }
            };
        }
        best.or_else(|| {
            self.variants
                .iter()
                .position(|variant| variant.min_width == 0.0 && variant.min_height == 0.0)
        })
        .unwrap_or(0)
    }

    /// The variant that applies at `client`.
    pub fn select(&self, client: Size) -> &CompiledVariant {
        &self.variants[self.select_index(client)]
    }

    /// Merged claim fields per id, in document order (the union over every
    /// variant; D130). Context-computed values participate, so a `visible`
    /// read from the context is claimed like a literal one.
    fn claim_targets(&self, values: &CompiledValues<'_>) -> Vec<(WidgetId, ClaimFields)> {
        let mut targets: Vec<(WidgetId, ClaimFields)> = Vec::new();
        for (index, variant) in self.variants.iter().enumerate() {
            let overrides = values.for_variant(index);
            for entry in variant.entries {
                let computed = overrides.iter().find(|item| item.id == entry.id);
                let props: &PropAttrs = computed
                    .and_then(|item| item.props.as_ref())
                    .unwrap_or(&**entry.props);
                let style: &StyleAttrs = computed
                    .and_then(|item| item.style.as_ref())
                    .unwrap_or(&**entry.style);
                match targets.iter_mut().find(|(id, _)| id.name() == entry.id) {
                    Some((_, fields)) => fields.merge(props, style),
                    None => {
                        let mut fields = ClaimFields::EMPTY;
                        fields.merge(props, style);
                        targets.push((WidgetId::new(entry.id), fields));
                    }
                }
            }
        }
        targets
    }
}

fn empty_attrs() -> std::borrow::Cow<'static, [(String, String)]> {
    std::borrow::Cow::Borrowed(&[])
}

fn empty_children() -> std::borrow::Cow<'static, [SkinElement]> {
    std::borrow::Cow::Borrowed(&[])
}

fn empty_content() -> std::borrow::Cow<'static, [ContentAttr]> {
    std::borrow::Cow::Borrowed(&[])
}

impl<M> Ui<M> {
    /// Apply one compiled document to the live widget tree.
    ///
    /// The generated module resolves the same document the dynamic parser
    /// would; this entry validates every target id first (nothing mutates on a
    /// failure) and then runs the shared [`Ui::install_prepared`]. A document
    /// generated with `context = Type` has context-dependent entries: apply it
    /// through [`Ui::apply_compiled_with`] (or the module's `install`), or such
    /// a rectangle reports [`RectError::Context`].
    #[cfg_attr(
        not(feature = "layout-xml"),
        doc = "\n\nThis build has only `layout-compiled`; use [`Ui::clear_layout`] to hand a\nprevious layout back."
    )]
    pub fn apply_compiled(
        &mut self,
        layout: &CompiledLayout,
        palette: &Palette,
        client: Size,
        sink: &mut impl UiHost,
    ) -> Result<LayoutReport, LayoutFailure>
    where
        M: 'static,
    {
        self.apply_compiled_with(layout, &CompiledValues::default(), palette, client, sink)
    }

    /// Apply a compiled document with the values its generated `install`
    /// computed for this apply (`context = Type`, §16.2).
    pub fn apply_compiled_with(
        &mut self,
        layout: &CompiledLayout,
        values: &CompiledValues<'_>,
        palette: &Palette,
        client: Size,
        sink: &mut impl UiHost,
    ) -> Result<LayoutReport, LayoutFailure>
    where
        M: 'static,
    {
        let mut errors = Diagnostics::new(100);
        let mut warnings = Diagnostics::new(100);

        // Ids the document creates itself (a `type` entry) are not required to
        // exist yet (M7.7).
        let creatable: Vec<&str> = layout
            .variants
            .iter()
            .flat_map(|variant| variant.entries)
            .filter(|entry| entry.type_name.is_some())
            .map(|entry| entry.id)
            .collect();

        // Preflight: every target exists and has the expected kind.
        for target in layout.targets {
            let id = WidgetId::new(target.id);
            let widget = self.contains(id);
            let scroll = self.scroll_view(id).is_some();
            match target.kind {
                TargetKind::Widget => {
                    if !widget && !creatable.contains(&target.id) {
                        errors.error(
                            "xml.id.unknown-widget",
                            target.line,
                            1,
                            target.path,
                            format!("no widget `{}` exists", target.id),
                        );
                    } else if scroll {
                        errors.error(
                            "xml.id.ambiguous",
                            target.line,
                            1,
                            target.path,
                            format!("`{}` is both a widget and a scroll panel", target.id),
                        );
                    }
                }
                TargetKind::Scroll => {
                    if !scroll {
                        errors.error(
                            "xml.id.unknown-scroll",
                            target.line,
                            1,
                            target.path,
                            format!("no scroll panel `{}` is registered", target.id),
                        );
                    } else if widget {
                        errors.error(
                            "xml.id.ambiguous",
                            target.line,
                            1,
                            target.path,
                            format!("`{}` is both a widget and a scroll panel", target.id),
                        );
                    }
                }
            }
        }
        if errors.has_errors() {
            return Err(LayoutFailure::new(errors.into_vec()));
        }

        let variant_index = layout.select_index(client);
        let variant = &layout.variants[variant_index];
        let computed_entries = values.for_variant(variant_index);
        let mut panels: Vec<PanelPlan<'_>> = Vec::new();
        let mut spaces: Vec<(WidgetId, Size)> = Vec::new();

        let mut order: Vec<usize> = (0..variant.scrolls.len()).collect();
        order.sort_by_key(|index| self.panel_depth(WidgetId::new(variant.scrolls[*index].id)));
        for index in order {
            let scroll = &variant.scrolls[index];
            let id = WidgetId::new(scroll.id);
            let Some(existing) = self.scroll_view(id) else {
                continue;
            };
            let space = match existing.parent() {
                Some(parent) => {
                    let size = self.panel_content_size(parent, &spaces);
                    Size::new(size.width, size.height)
                }
                None => client,
            };
            let viewport = match (scroll.rect)(space) {
                Ok(viewport) => viewport,
                Err(error) => {
                    errors.push(rect_diagnostic(error, scroll.line, scroll.path));
                    continue;
                }
            };
            if viewport.width() <= 0.0 || viewport.height() <= 0.0 {
                warnings.warning(
                    "xml.value.zero-size",
                    scroll.line,
                    1,
                    scroll.path,
                    "the viewport has a zero side",
                );
            }
            let mut probe = existing.clone();
            probe.set_viewport(viewport);
            if let CompiledHeight::Exact(evaluate) = scroll.height {
                let current = existing.content_rect().size();
                match evaluate(current) {
                    Some(height) if height >= 0.0 => probe.set_content_override(Some(Size::new(
                        current.width,
                        height,
                    ))),
                    Some(_) => errors.error(
                        "xml.geom.negative",
                        scroll.line,
                        1,
                        scroll.path,
                        "`content-height` must not be negative",
                    ),
                    None => errors.error(
                        "xml.geom.invalid",
                        scroll.line,
                        1,
                        scroll.path,
                        "`content-height` is not a finite number at this size",
                    ),
                }
            }
            probe.resolve();
            let content = probe.content_rect().size();
            spaces.push((id, content));
            let height = match scroll.height {
                CompiledHeight::Keep => PanelHeight::Keep,
                CompiledHeight::Exact(evaluate) => {
                    PanelHeight::Exact(Box::new(move |size| evaluate(size)))
                }
                CompiledHeight::Auto => PanelHeight::Auto,
            };
            panels.push(PanelPlan {
                id,
                viewport,
                content,
                height,
            });
        }

        // Scroll membership (`parent="…"`, M7.7 follow-up): the same rule as
        // the dynamic planner — a created entry joins the panel, an addressed
        // entry asserts the application's own membership.
        let mut invalid_parents: Vec<WidgetId> = Vec::new();
        for entry in variant.entries {
            let Some(parent) = entry.parent.map(WidgetId::new) else {
                continue;
            };
            let id = WidgetId::new(entry.id);
            if self.scroll_view(parent).is_none() {
                errors.error(
                    "xml.parent.missing",
                    entry.line,
                    1,
                    entry.path,
                    format!("`{}` is not a registered scroll panel", parent.name()),
                );
                invalid_parents.push(id);
                continue;
            }
            if self.contains(parent) {
                errors.error(
                    "xml.parent.missing",
                    entry.line,
                    1,
                    entry.path,
                    format!(
                        "`{}` is both a widget and a scroll panel; `parent` is ambiguous",
                        parent.name()
                    ),
                );
                invalid_parents.push(id);
                continue;
            }
            if entry.type_name.is_none() && self.scroll_owner_of(id) != Some(parent) {
                errors.error(
                    "xml.parent.missing",
                    entry.line,
                    1,
                    entry.path,
                    format!(
                        "`{}` is not a child of scroll panel `{}`",
                        entry.id,
                        parent.name()
                    ),
                );
            }
        }

        // Document-created widgets (M7.7), mirroring the dynamic planner: the
        // typed entries are resolved once, the factories run before anything
        // mutates, and the widget loop finds the staged widget.
        let generation_changed = self
            .installed_layout()
            .map(|installed| installed.generation())
            != Some(layout.generation);
        let mut typed_rects: Vec<(WidgetId, Rect)> = Vec::new();
        let mut requests: Vec<super::apply::CreateRequest<'_>> = Vec::new();
        for entry in variant.entries {
            let Some(type_name) = entry.type_name else {
                continue;
            };
            let id = WidgetId::new(entry.id);
            let parent = entry
                .parent
                .map(WidgetId::new)
                .filter(|_| !invalid_parents.contains(&id));
            let layout_space = match parent {
                Some(panel) => self.panel_content_size(panel, &spaces),
                None => client,
            };
            let computed = computed_entries.iter().find(|item| item.id == entry.id);
            let rect = match computed
                .and_then(|item| item.rect.as_ref())
                .map_or_else(|| (entry.rect)(layout_space), |compute| compute(layout_space))
            {
                Ok(rect) => rect,
                Err(error) => {
                    errors.push(rect_diagnostic(error, entry.line, entry.path));
                    continue;
                }
            };
            if rect.width() <= 0.0 || rect.height() <= 0.0 {
                warnings.warning(
                    "xml.value.zero-size",
                    entry.line,
                    1,
                    entry.path,
                    "the rectangle has a zero side",
                );
            }
            let content = match computed.and_then(|item| item.content.as_ref()) {
                Some(list) => list.clone(),
                None => entry.content.iter().map(|item| item.to_attr()).collect(),
            };
            let attrs = entry
                .skin_attrs
                .iter()
                .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                .collect::<Vec<_>>();
            let children = match computed.and_then(|item| item.children.as_deref()) {
                Some(list) => list.to_vec(),
                None => entry
                    .rust_children
                    .map(|list| list.to_vec())
                    .unwrap_or_else(|| super::compiled::elements(entry.skin_children)),
            };
            typed_rects.push((id, rect));
            requests.push(super::apply::CreateRequest {
                id,
                type_name,
                command: entry.command,
                parent,
                rect,
                content,
                attrs,
                children,
                line: entry.line,
                path: entry.path,
            });
        }
        let creation = self.plan_creations(&requests, generation_changed, palette, client, &mut errors);

        let mut widgets: Vec<PreparedWidget<'_>> = Vec::new();
        for entry in variant.entries {
            let id = WidgetId::new(entry.id);
            let staged = creation.create.iter().any(|widget| widget.id() == id);
            if !self.contains(id) && !staged {
                errors.error(
                    "xml.id.unknown-widget",
                    entry.line,
                    1,
                    entry.path,
                    format!("no widget `{}` exists", entry.id),
                );
                continue;
            }
            let computed = computed_entries.iter().find(|item| item.id == entry.id);
            let (space, layout_space) =
                match entry.parent.map(WidgetId::new).filter(|_| !invalid_parents.contains(&id)) {
                    Some(panel) => (
                        EntrySpace::Content(panel),
                        self.panel_content_size(panel, &spaces),
                    ),
                    None => match self.scroll_owner_of(id) {
                        None => (EntrySpace::Client, client),
                        Some(panel) => {
                            let size = self.panel_content_size(panel, &spaces);
                            (EntrySpace::Content(panel), size)
                        }
                    },
                };
            let typed = typed_rects.iter().find(|(candidate, _)| *candidate == id);
            let rect = match typed {
                Some((_, rect)) => *rect,
                None => match computed
                    .and_then(|item| item.rect.as_ref())
                    .map_or_else(|| (entry.rect)(layout_space), |compute| compute(layout_space))
                {
                    Ok(rect) => rect,
                    Err(error) => {
                        errors.push(rect_diagnostic(error, entry.line, entry.path));
                        continue;
                    }
                },
            };
            if rect.width() <= 0.0 || rect.height() <= 0.0 {
                if typed.is_none() {
                    warnings.warning(
                        "xml.value.zero-size",
                        entry.line,
                        1,
                        entry.path,
                        "the rectangle has a zero side",
                    );
                }
            }
            let props: &PropAttrs = computed
                .and_then(|item| item.props.as_ref())
                .unwrap_or(&**entry.props);
            let style: &StyleAttrs = computed
                .and_then(|item| item.style.as_ref())
                .unwrap_or(&**entry.style);
            // The target's declarations come from the staged widget when this
            // apply creates it, or from the tree.
            let target = creation
                .create
                .iter()
                .find(|widget| widget.id() == id)
                .map(|widget| widget.as_ref())
                .or_else(|| self.widget_ref(id));
            if style.has_surface() && target.and_then(|widget| widget.surface_style()).is_none() {
                warnings.warning(
                    "xml.style.unsupported",
                    entry.line,
                    1,
                    entry.path,
                    "this widget does not adopt a SurfaceStyle; the style is skipped",
                );
            }
            let content: std::borrow::Cow<'_, [ContentAttr]> =
                match computed.and_then(|item| item.content.as_ref()) {
                    Some(list) => std::borrow::Cow::Borrowed(list.as_slice()),
                    None if entry.content.is_empty() => empty_content(),
                    None => std::borrow::Cow::Owned(
                        entry
                            .content
                            .iter()
                            .map(|item| item.to_attr())
                            .collect::<Vec<_>>(),
                    ),
                };
            if !content.is_empty() {
                let caps = target
                    .map(|widget| widget.content_caps())
                    .unwrap_or(ContentCaps::NONE);
                for item in content.iter() {
                    let (cap, name) = super::apply::capability(item);
                    if !caps.contains(cap) {
                        warnings.warning(
                            "xml.content.unsupported",
                            entry.line,
                            1,
                            entry.path,
                            format!("`{name}` is not supported by this widget"),
                        );
                    }
                    if let ContentAttr::Progress(value) = item {
                        if !(0.0..=1.0).contains(value) {
                            warnings.warning(
                                "xml.value.clamped",
                                entry.line,
                                1,
                                entry.path,
                                format!("progress {value} is clamped to 0..1"),
                            );
                        }
                    }
                }
                if self.is_editing_on(id, sink) {
                    warnings.warning(
                        "xml.content.session-open",
                        entry.line,
                        1,
                        entry.path,
                        "an edit session is open; the content initial value is skipped",
                    );
                }
            }
            let skin_attrs = if entry.skin_attrs.is_empty() {
                empty_attrs()
            } else {
                std::borrow::Cow::Owned(
                    entry
                        .skin_attrs
                        .iter()
                        .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                        .collect::<Vec<_>>(),
                )
            };
            // A `<rust>` binding replaces the static child list; the values of a
            // context module come from the generated `install`.
            let bound_children: Option<&[SkinElement]> = computed
                .and_then(|item| item.children.as_deref())
                .or_else(|| entry.rust_children.map(|list| list.as_slice()));
            let skin_children: std::borrow::Cow<'_, [SkinElement]> = match bound_children {
                Some(list) if !list.is_empty() => std::borrow::Cow::Owned(list.to_vec()),
                _ if !entry.skin_children.is_empty() => {
                    std::borrow::Cow::Owned(super::compiled::elements(entry.skin_children))
                }
                _ => empty_children(),
            };
            if !skin_attrs.is_empty() || !skin_children.is_empty() {
                if let Some(widget) = target {
                    self.plan_skin(
                        widget,
                        entry.line,
                        entry.path,
                        skin_attrs.as_ref(),
                        skin_children.as_ref(),
                        palette,
                        &mut errors,
                    );
                }
                if self.is_editing_on(id, sink) {
                    warnings.warning(
                        "xml.content.session-open",
                        entry.line,
                        1,
                        entry.path,
                        "an edit session is open; the local skin data is skipped",
                    );
                }
            }
            widgets.push(PreparedWidget {
                id,
                rect,
                space,
                props,
                style,
                content,
                skin_attrs,
                skin_children,
            });
        }

        if errors.has_errors() {
            let mut all = errors.into_vec();
            all.extend(warnings.into_vec());
            return Err(LayoutFailure::new(all));
        }

        let prepared = Prepared {
            generation: layout.generation,
            variant_id: variant.id.to_string(),
            panels,
            widgets,
            warnings: warnings.into_vec(),
            claims: layout.claim_targets(values),
            remove: creation.remove,
            create: creation.create,
            created: creation.created,
        };
        Ok(self.install_prepared(prepared, palette, client, sink))
    }
}


fn rect_diagnostic(error: RectError, line: usize, path: &str) -> LayoutDiagnostic {
    match error {
        RectError::Negative(axis) => LayoutDiagnostic::error(
            "xml.geom.negative",
            line,
            1,
            path.to_string(),
            format!("`{axis}` must not be negative"),
        ),
        RectError::NotFinite => LayoutDiagnostic::error(
            "xml.geom.non-finite",
            line,
            1,
            path.to_string(),
            "the rectangle is not finite at this size",
        ),
        RectError::Context => LayoutDiagnostic::error(
            "xml.rust.context",
            line,
            1,
            path.to_string(),
            "this rectangle reads the layout context; apply through the module's install()",
        ),
    }
}
