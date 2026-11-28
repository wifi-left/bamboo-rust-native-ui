// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Applying a document to the live widget tree (`Ui::apply_layout`, M7 §8).
//!
//! Application is split in two phases. The first resolves every id, every
//! rectangle and every expression against the current client size; any error
//! returns [`LayoutFailure`] and **mutates nothing**, so a bad hot reload
//! leaves the previous layout on screen. The second phase captures baselines
//! on a generation change, restores claimed fields, and applies only the
//! selected variant.
//!
//! A *claim* is one field the document owns for an id. The claim set is the
//! union over every variant of the attributes that variant declares, so an id
//! declared in the wide variant but not the base returns to the application's
//! value when the window crosses the breakpoint — and a field the document
//! never mentions is never touched (D129).
//!
//! The mutation half works on [`Prepared`], an owned/built-resolved plan, so
//! the compiled mode (`layout_xml::compiled`, D128) shares every claim, content
//! and skin write with the dynamic path; only the planning step differs.

use std::borrow::Cow;
use std::fmt;

use crate::event::Cursor;
use crate::geometry::{Corners, Rect, Size};
use crate::layout_xml::diag::{Diagnostics, LayoutDiagnostic};
#[cfg(feature = "layout-xml")]
use crate::layout_xml::expr::{self, Axis};
use crate::layout_xml::model::{BackgroundAttr, ContentAttr, EntrySpace, PropAttrs, StyleAttrs};
#[cfg(feature = "layout-xml")]
use crate::layout_xml::model::{ContentHeight, LayoutDocument, LayoutSpace, LayoutVariant};
use crate::layout_xml::palette_token;
use crate::props::{ContentCaps, WidgetProps};
use crate::scroll::ScrollChild;
use crate::skin::SkinElement;
use crate::surface::SurfaceStyle;
use crate::theme::Palette;
use crate::ui::{Ui, UiHost};
use crate::widget::WidgetId;

/// What `Ui::apply_layout` did.
#[derive(Debug)]
pub struct LayoutReport {
    /// Canonical-model generation this apply belongs to.
    pub generation: u64,
    /// Variant selected for the client size.
    pub variant_id: String,
    /// Widget and scroll entries applied.
    pub applied_entries: usize,
    /// Content initial values written (generation change only).
    pub content_writes: usize,
    /// Widgets whose local skin data (child lists and/or declared attributes)
    /// was written (generation change only).
    pub skin_writes: usize,
    /// Widgets this apply created from the widget registry (M7.7).
    pub created: usize,
    /// Non-fatal problems (capability, open session, clamped value).
    pub warnings: Vec<LayoutDiagnostic>,
}

impl LayoutReport {
    /// At most one line, suitable for a status bar.
    pub fn summary(&self) -> String {
        let warning = self
            .warnings
            .first()
            .map(|item| item.to_string())
            .unwrap_or_else(|| "无警告".to_string());
        format!(
            "{} · {} 项 · 内容 {} 次 · 局部 {} 项 · {}",
            self.variant_id, self.applied_entries, self.content_writes, self.skin_writes, warning
        )
    }
}

impl fmt::Display for LayoutReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

/// Every reason an apply was rejected; nothing changed.
#[derive(Clone, Debug)]
pub struct LayoutFailure {
    pub diagnostics: Vec<LayoutDiagnostic>,
}

impl LayoutFailure {
    pub(crate) fn new(mut diagnostics: Vec<LayoutDiagnostic>) -> Self {
        diagnostics.sort_by_key(|item| !item.is_error());
        Self { diagnostics }
    }

    /// The first error, or the first diagnostic when only warnings exist.
    pub fn first(&self) -> Option<&LayoutDiagnostic> {
        self.diagnostics
            .iter()
            .find(|item| item.is_error())
            .or_else(|| self.diagnostics.first())
    }
}

impl fmt::Display for LayoutFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.first() {
            Some(item) => write!(f, "{item}"),
            None => write!(f, "layout failed"),
        }
    }
}

impl std::error::Error for LayoutFailure {}

/// One resolved entry, for the debug form and tests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppliedEntry {
    pub id: WidgetId,
    pub rect: Rect,
    pub space: EntrySpace,
}

/// What one id's document-owned fields are (M7 §8.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClaimFields {
    pub visible: bool,
    pub enabled: bool,
    pub focusable: bool,
    pub focus_order: bool,
    pub cursor: bool,
    /// Any `WidgetProps` field, including the font.
    pub props_any: bool,
    /// Radius or per-corner radii.
    pub corners: bool,
    /// Background fill / transparency.
    pub background: bool,
}

impl ClaimFields {
    /// The empty set (a plain claim descriptor).
    pub const EMPTY: Self = Self {
        visible: false,
        enabled: false,
        focusable: false,
        focus_order: false,
        cursor: false,
        props_any: false,
        corners: false,
        background: false,
    };

    /// Fold one entry's declared fields into this set.
    pub fn merge(&mut self, props: &PropAttrs, style: &StyleAttrs) {
        self.visible |= props.visible.is_some();
        self.enabled |= props.enabled.is_some();
        self.focusable |= props.focusable.is_some();
        self.focus_order |= props.focus_order.is_some();
        self.cursor |= props.cursor.is_some();
        self.props_any |= props.has_props() || style.has_font();
        self.corners |= style.radius.is_some() || style.corners.is_some();
        self.background |= style.background.is_some();
    }
}

/// One id's baseline, captured when a generation is installed.
pub(crate) struct Claim {
    pub(crate) id: WidgetId,
    /// Recorded for the debug form; rectangles are never restored.
    pub(crate) rect: Rect,
    pub(crate) props: Option<WidgetProps>,
    pub(crate) visible_override: Option<bool>,
    pub(crate) widget_visible: bool,
    pub(crate) enabled: bool,
    pub(crate) focusable: Option<bool>,
    pub(crate) focus_order: i32,
    pub(crate) cursor: Option<Cursor>,
    pub(crate) surface: Option<SurfaceStyle>,
    pub(crate) fields: ClaimFields,
}

/// One widget a document created (M7.7): the id, the registry type it was
/// built from and the command it reports. Kept with the installed layout so a
/// generation change can remove what the new document no longer declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedCreation {
    pub id: WidgetId,
    pub type_name: String,
    pub command: Option<u32>,
    /// The scroll panel this widget belongs to (`parent="…"`), when declared.
    pub parent: Option<WidgetId>,
}

/// Read-only view of the installed document (`Ui::installed_layout`).
pub struct AppliedLayout {
    pub(crate) generation: u64,
    pub(crate) client: Size,
    pub(crate) variant_id: String,
    pub(crate) entries: Vec<AppliedEntry>,
    pub(crate) content_writes: usize,
    pub(crate) skin_writes: usize,
    pub(crate) warnings: Vec<LayoutDiagnostic>,
    pub(crate) claims: Vec<Claim>,
    pub(crate) created: Vec<AppliedCreation>,
}

impl AppliedLayout {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn client(&self) -> Size {
        self.client
    }

    pub fn variant_id(&self) -> &str {
        &self.variant_id
    }

    pub fn entries(&self) -> &[AppliedEntry] {
        &self.entries
    }

    pub fn content_writes(&self) -> usize {
        self.content_writes
    }

    /// Widgets whose local skin data was written by the last generation load.
    pub fn skin_writes(&self) -> usize {
        self.skin_writes
    }

    pub fn warnings(&self) -> &[LayoutDiagnostic] {
        &self.warnings
    }

    /// Resolved rectangle of one applied entry.
    pub fn rect(&self, id: WidgetId) -> Option<Rect> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.rect)
    }

    /// Ids the document owns fields for, sorted by insertion order.
    pub fn claimed_ids(&self) -> impl Iterator<Item = WidgetId> + '_ {
        self.claims.iter().map(|claim| claim.id)
    }

    /// Widgets this document created, in declaration order (M7.7).
    pub fn created(&self) -> &[AppliedCreation] {
        &self.created
    }

    /// Ids of the widgets this document created.
    pub fn created_ids(&self) -> impl Iterator<Item = WidgetId> + '_ {
        self.created.iter().map(|creation| creation.id)
    }

    /// The rectangle an id had when the generation was installed (debug form;
    /// rectangles are never restored).
    pub fn baseline_rect(&self, id: WidgetId) -> Option<Rect> {
        self.claims
            .iter()
            .find(|claim| claim.id == id)
            .map(|claim| claim.rect)
    }
}

// ---------------------------------------------------------------------------
// The prepared plan (shared by the dynamic and compiled modes)
// ---------------------------------------------------------------------------

/// One resolved panel: viewport, resulting content extent and height rule.
pub(crate) struct PanelPlan<'a> {
    pub(crate) id: WidgetId,
    pub(crate) viewport: Rect,
    /// Content extent this panel will have after the new viewport.
    pub(crate) content: Size,
    pub(crate) height: PanelHeight<'a>,
}

/// What `<scroll content-height>` applies at mutation time.
pub(crate) enum PanelHeight<'a> {
    Keep,
    /// Re-evaluated against the post-viewport content extent, like the
    /// dynamic applier; a non-finite or negative value writes nothing.
    Exact(Box<dyn Fn(Size) -> Option<f32> + 'a>),
    /// Grow to fit viewport, children and the previous extent — never shrink.
    Auto,
}

/// One resolved widget entry, with the attribute views the applier writes.
pub(crate) struct PreparedWidget<'a> {
    pub(crate) id: WidgetId,
    pub(crate) rect: Rect,
    pub(crate) space: EntrySpace,
    pub(crate) props: &'a PropAttrs,
    pub(crate) style: &'a StyleAttrs,
    pub(crate) content: Cow<'a, [ContentAttr]>,
    pub(crate) skin_attrs: Cow<'a, [(String, String)]>,
    pub(crate) skin_children: Cow<'a, [SkinElement]>,
}

impl PreparedWidget<'_> {
    fn has_skin(&self) -> bool {
        !self.skin_attrs.is_empty() || !self.skin_children.is_empty()
    }
}

/// A fully resolved document: every id checked, every rectangle and variant
/// chosen. Both planning paths produce this; [`Ui::install_prepared`] mutates.
pub(crate) struct Prepared<'a, M> {
    pub(crate) generation: u64,
    pub(crate) variant_id: String,
    pub(crate) panels: Vec<PanelPlan<'a>>,
    pub(crate) widgets: Vec<PreparedWidget<'a>>,
    pub(crate) warnings: Vec<LayoutDiagnostic>,
    /// Merged claim fields per id (union over every variant).
    pub(crate) claims: Vec<(WidgetId, ClaimFields)>,
    /// Widgets to remove before applying (document-owned ids the new
    /// generation no longer declares, or replaces with another type).
    pub(crate) remove: Vec<WidgetId>,
    /// Widgets to insert (created from the registry), in declaration order.
    pub(crate) create: Vec<Box<dyn crate::widget::Widget<M>>>,
    /// Every widget the document owns after this apply (document order).
    pub(crate) created: Vec<AppliedCreation>,
}

/// Which half of a widget's local skin data one write pass applies.
#[derive(Clone, Copy, Debug)]
enum SkinPhase {
    Children,
    Attributes,
}

fn kind_name(kind: crate::skin::SkinValueKind) -> &'static str {
    match kind {
        crate::skin::SkinValueKind::Text => "text",
        crate::skin::SkinValueKind::Number => "a finite number",
        crate::skin::SkinValueKind::Bool => "`true`, `false`, `1` or `0`",
        crate::skin::SkinValueKind::Color => "a colour (`#rgb` or `#rrggbb`)",
    }
}

pub(crate) fn capability(content: &ContentAttr) -> (ContentCaps, &'static str) {
    match content {
        ContentAttr::Text(_) => (ContentCaps::TEXT, "text"),
        ContentAttr::Label(_) => (ContentCaps::LABEL, "label"),
        ContentAttr::Value(_) => (ContentCaps::VALUE, "value"),
        ContentAttr::Progress(_) => (ContentCaps::PROGRESS, "progress"),
        ContentAttr::Number(_) => (ContentCaps::NUMBER, "number"),
        ContentAttr::Checked(_) => (ContentCaps::TOGGLED, "checked"),
        ContentAttr::Selected(_) => (ContentCaps::SELECTION, "selected"),
    }
}

/// One `type` entry mapped from either consumption mode (M7.7): everything a
/// factory may read, with the rectangle already resolved.
pub(crate) struct CreateRequest<'a> {
    pub(crate) id: WidgetId,
    pub(crate) type_name: &'a str,
    pub(crate) command: Option<u32>,
    /// The scroll panel the entry declares as its `parent`, if any.
    pub(crate) parent: Option<WidgetId>,
    pub(crate) rect: Rect,
    pub(crate) content: Vec<ContentAttr>,
    pub(crate) attrs: Vec<(String, String)>,
    pub(crate) children: Vec<SkinElement>,
    pub(crate) line: usize,
    pub(crate) path: &'a str,
}

/// The resolved creation half of one apply.
pub(crate) struct CreatedPlan<M> {
    pub(crate) create: Vec<Box<dyn crate::widget::Widget<M>>>,
    pub(crate) remove: Vec<WidgetId>,
    pub(crate) created: Vec<AppliedCreation>,
}

impl<M> Ui<M> {
    /// Apply one parsed document to the live widget tree.
    ///
    /// Resolves and validates every id, rectangle and expression first; on any
    /// error nothing is mutated and the previously installed layout stays
    /// active. Otherwise one damage union is flushed and the applied state is
    /// kept for [`Ui::installed_layout`].
    #[cfg(feature = "layout-xml")]
    pub fn apply_layout(
        &mut self,
        document: &LayoutDocument,
        palette: &Palette,
        client: Size,
        sink: &mut impl UiHost,
    ) -> Result<LayoutReport, LayoutFailure>
    where
        M: 'static,
    {
        let variant = document.select(client);
        let prepared = match self.plan_layout(document, variant, palette, client, sink) {
            Ok(prepared) => prepared,
            Err(failure) => {
                #[cfg(feature = "debug-form")]
                {
                    let first = failure
                        .first()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "layout apply failed".to_string());
                    self.debug_event("error", format!("apply: {first}"));
                }
                return Err(failure);
            }
        };
        Ok(self.install_prepared(prepared, palette, client, sink))
    }

    /// Hand every claimed field back to the application and drop the document.
    /// Rectangles and content are not restored.
    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub fn clear_layout(&mut self, sink: &mut impl UiHost) {
        let Some(layout) = self.take_layout_state() else {
            return;
        };
        // The document owns the widgets it created: hand them back with the
        // claimed fields (§17).
        for id in layout.created_ids().collect::<Vec<_>>() {
            self.remove(id, sink);
        }
        self.restore_claims(&layout.claims, sink);
        self.flush_damage(sink);
        #[cfg(feature = "debug-form")]
        self.debug_event("layout", "cleared");
    }

    /// Mutate the tree from a resolved plan: restore/capture claims, move
    /// rectangles, apply the claimed fields, write content and local skin data
    /// once per generation, flush one damage union and record the result.
    ///
    /// Shared by `apply_layout` (dynamic) and `apply_compiled` (generated
    /// code); D128 promises both paths behave identically.
    pub(crate) fn install_prepared(
        &mut self,
        prepared: Prepared<'_, M>,
        palette: &Palette,
        client: Size,
        sink: &mut impl UiHost,
    ) -> LayoutReport {
        let Prepared {
            generation,
            variant_id,
            panels,
            widgets,
            warnings,
            claims: targets,
            remove,
            create,
            created,
        } = prepared;

        let previous = self.take_layout_state();
        let generation_changed =
            previous.as_ref().map(|layout| layout.generation()) != Some(generation);
        if generation_changed {
            // Document-owned widgets the new generation does not declare (or
            // replaces) go first, so a replacement id is free for insertion.
            for id in &remove {
                self.remove(*id, sink);
            }
            if let Some(previous) = previous.as_ref() {
                self.restore_claims(&previous.claims, sink);
            }
        }
        let kept_created = previous
            .as_ref()
            .filter(|_| !generation_changed)
            .map(|previous| previous.created.clone());

        // Insert the created widgets before claims are captured, so they get
        // the same claim record (baseline, owned fields) as every other entry.
        if !create.is_empty() {
            // Created widgets belong to the page layer: before the first
            // backdrop (a modal scrim), or at the end when there is none, in
            // declaration order (§17).
            let mut at = self.page_insert_index();
            for widget in create {
                let id = widget.id();
                self.insert_boxed(at, widget);
                at += 1;
                if let Some(planned) = widgets.iter().find(|planned| planned.id == id) {
                    // A scrolled widget is positioned by its panel (content
                    // coordinates are not client DIPs); the panel's viewport is
                    // invalidated by the attach/apply below.
                    if planned.space == EntrySpace::Client {
                        sink.invalidate(planned.rect);
                    }
                }
            }
            // A created widget that declared `parent` is not in the panel's
            // child list yet; the panel does not know it until now (§17).
            self.attach_scroll_children(&widgets, sink);
        }

        let mut claims = match previous {
            Some(previous) if !generation_changed => previous.claims,
            _ => self.capture_claims(&targets),
        };

        self.apply_panels(&panels, &widgets, sink);
        self.apply_client_rects(&widgets, sink);
        for planned in &widgets {
            if let Some(claim) = claims.iter_mut().find(|claim| claim.id == planned.id) {
                self.apply_claim(claim, Some(planned), palette, sink);
            }
        }
        for claim in claims.iter_mut() {
            if !widgets.iter().any(|planned| planned.id == claim.id) {
                self.apply_claim(claim, None, palette, sink);
            }
        }

        // Order rule (§6.2 rule 6): child lists first, then the shared content
        // (including `selected`), then the local attributes. All three phases
        // are content-scoped, so a resize never rewrites anything.
        let (content_writes, skin_writes) = if generation_changed {
            let skin_writes = widgets
                .iter()
                .filter(|planned| planned.has_skin() && !self.is_editing_on(planned.id, sink))
                .count();
            self.write_skin(&widgets, SkinPhase::Children, palette, sink);
            let content = self.write_content(&widgets, sink);
            self.write_skin(&widgets, SkinPhase::Attributes, palette, sink);
            (content, skin_writes)
        } else {
            (0, 0)
        };

        self.flush_damage(sink);

        let entries: Vec<AppliedEntry> = widgets
            .iter()
            .map(|planned| AppliedEntry {
                id: planned.id,
                rect: planned.rect,
                space: planned.space,
            })
            .collect();
        let applied_entries = widgets.len() + panels.len();
        let report = LayoutReport {
            generation,
            variant_id: variant_id.clone(),
            applied_entries,
            content_writes,
            skin_writes,
            created: created.len(),
            warnings: warnings.clone(),
        };
        self.put_layout_state(AppliedLayout {
            generation,
            client,
            variant_id,
            entries,
            content_writes,
            skin_writes,
            warnings,
            claims,
            created: kept_created.unwrap_or(created),
        });
        #[cfg(feature = "debug-form")]
        {
            self.debug_event(
                "layout",
                format!(
                    "apply variant={} gen={} entries={} content={} skin={} warnings={}",
                    report.variant_id,
                    report.generation,
                    report.applied_entries,
                    report.content_writes,
                    report.skin_writes,
                    report.warnings.len()
                ),
            );
            for warning in &report.warnings {
                self.debug_event("warning", warning.to_string());
            }
        }
        report
    }

    // -- document-created widgets (M7.7) -----------------------------------

    /// Append a created widget to the scroll panel it declared as `parent`
    /// (M7.7 follow-up). The panel does not know the id before insertion, so
    /// the child record is appended here and the panel is re-registered to
    /// position it at `content rect - offset`. A declared panel is re-applied
    /// by [`Ui::apply_panels`] afterwards; an application-registered panel that
    /// the document does not lay out still gets the child this way.
    fn attach_scroll_children(
        &mut self,
        widgets: &[PreparedWidget<'_>],
        sink: &mut impl UiHost,
    ) {
        for planned in widgets {
            let EntrySpace::Content(panel) = planned.space else {
                continue;
            };
            let Some(mut view) = self.scroll_view(panel).cloned() else {
                continue;
            };
            if view.contains(planned.id) {
                continue;
            }
            let mut children = view.children_ref().to_vec();
            children.push(ScrollChild::new(planned.id, planned.rect));
            view.set_children(children);
            self.set_scroll_view(view, sink);
        }
    }

    /// One `type` entry as either consumption mode sees it, for
    /// [`Ui::plan_creations`].
    pub(crate) fn plan_creations(
        &self,
        requests: &[CreateRequest<'_>],
        generation_changed: bool,
        palette: &Palette,
        client: Size,
        errors: &mut Diagnostics,
    ) -> CreatedPlan<M>
    where
        M: 'static,
    {
        let previous: Vec<AppliedCreation> = self
            .installed_layout()
            .map(|layout| layout.created().to_vec())
            .unwrap_or_default();
        let mut plan = CreatedPlan {
            create: Vec::new(),
            remove: Vec::new(),
            created: Vec::new(),
        };
        for request in requests {
            plan.created.push(AppliedCreation {
                id: request.id,
                type_name: request.type_name.to_string(),
                command: request.command,
                parent: request.parent,
            });
        }
        // A widget the previous generation created and this one no longer
        // declares is removed; a type/command/parent change replaces it (§17).
        if generation_changed {
            for prior in &previous {
                if !plan.created.iter().any(|item| item.id == prior.id) {
                    plan.remove.push(prior.id);
                }
            }
        }

        // Document order is part of the model: a kept created widget keeps its
        // *old* insertion slot, so a reorder or a middle insertion would leave
        // the page layer in the previous order. When the resulting order (kept
        // ids in their old order, then the new ids) differs from the document
        // order, every kept id is removed and re-created so the block is
        // rebuilt by declaration order. A same-generation apply never restacks.
        let restack = generation_changed && {
            let kept: Vec<WidgetId> = previous
                .iter()
                .map(|prior| prior.id)
                .filter(|id| plan.created.iter().any(|item| item.id == *id))
                .collect();
            let fresh: Vec<WidgetId> = plan
                .created
                .iter()
                .map(|item| item.id)
                .filter(|id| !previous.iter().any(|prior| prior.id == *id))
                .collect();
            let mut resulting = kept;
            resulting.extend(fresh);
            resulting != plan.created.iter().map(|item| item.id).collect::<Vec<_>>()
        };

        for request in requests {
            let prior = previous.iter().find(|item| item.id == request.id);
            let same = prior.is_some_and(|item| {
                item.type_name == request.type_name
                    && item.command == request.command
                    && item.parent == request.parent
            });
            let exists = self.contains(request.id);
            if exists && same && !restack {
                // The id exists as this document's own widget: update it.
                continue;
            }
            if exists {
                if !prior.is_some() || !generation_changed {
                    errors.error(
                        "xml.id.duplicate",
                        request.line,
                        1,
                        request.path,
                        format!(
                            "`{}` already exists and this generation does not create it",
                            request.id.name()
                        ),
                    );
                    continue;
                }
                if !plan.remove.contains(&request.id) {
                    plan.remove.push(request.id);
                }
            }
            let Some(registry) = self.widget_registry() else {
                errors.error(
                    "xml.type.unknown",
                    request.line,
                    1,
                    request.path,
                    format!(
                        "no widget registry is installed; `{}` cannot be created",
                        request.type_name
                    ),
                );
                continue;
            };
            let Some(factory) = registry.factory(request.type_name) else {
                errors.error(
                    "xml.type.unknown",
                    request.line,
                    1,
                    request.path,
                    format!("no widget type `{}` is registered", request.type_name),
                );
                continue;
            };
            let spec = crate::creation::WidgetSpec {
                id: request.id,
                rect: request.rect,
                content: &request.content,
                attrs: &request.attrs,
                children: &request.children,
                command: request.command,
                line: request.line,
                path: request.path,
            };
            let ctx = crate::creation::FactoryCtx {
                palette,
                client,
                commands: registry.commands(),
            };
            match factory(&spec, &ctx) {
                Ok(widget) if widget.id() == request.id => plan.create.push(widget),
                Ok(widget) => errors.error(
                    "xml.type.create",
                    request.line,
                    1,
                    request.path,
                    format!(
                        "the `{}` factory built `{}` instead of `{}`",
                        request.type_name,
                        widget.id().name(),
                        request.id.name()
                    ),
                ),
                Err(crate::creation::FactoryError::Missing(field)) => errors.error(
                    "xml.command.missing",
                    request.line,
                    1,
                    request.path,
                    format!(
                        "`{}` needs a `{field}` the entry does not supply",
                        request.type_name
                    ),
                ),
                Err(crate::creation::FactoryError::Rejected(message)) => errors.error(
                    "xml.type.create",
                    request.line,
                    1,
                    request.path,
                    format!(
                        "the `{}` factory refused the entry: {message}",
                        request.type_name
                    ),
                ),
            }
        }
        plan
    }

    // -- planning (dynamic) ------------------------------------------------

    #[cfg(feature = "layout-xml")]
    fn plan_layout<'d>(
        &self,
        document: &'d LayoutDocument,
        variant: &'d LayoutVariant,
        palette: &Palette,
        client: Size,
        sink: &impl UiHost,
    ) -> Result<Prepared<'d, M>, LayoutFailure>
    where
        M: 'static,
    {
        let constants = document.constants();
        let mut errors = Diagnostics::new(100);
        let mut warnings = Diagnostics::new(100);
        let mut panels: Vec<PanelPlan<'d>> = Vec::new();
        let mut spaces: Vec<(WidgetId, Size)> = Vec::new();

        // Inline-Rust data bindings are code too: a runtime file never
        // evaluates them (§16.2).
        for binding in variant.rust_bindings() {
            errors.error(
                "xml.rust.unsupported",
                binding.line(),
                1,
                binding.path(),
                format!(
                    "`{}` binds its child list to inline Rust; the dynamic mode never evaluates code",
                    binding.id()
                ),
            );
        }

        // Scroll panels, parents before children (a nested viewport is
        // expressed in the parent's content space).
        let mut order: Vec<usize> = (0..variant.scrolls().len()).collect();
        order.sort_by_key(|index| self.panel_depth(variant.scrolls()[*index].id()));
        for index in order {
            let entry = &variant.scrolls()[index];
            let id = entry.id();
            let Some(existing) = self.scroll_view(id) else {
                errors.error(
                    "xml.id.unknown-scroll",
                    entry.line(),
                    1,
                    entry.path(),
                    format!("no scroll panel `{}` is registered", id.name()),
                );
                continue;
            };
            if self.contains(id) {
                errors.error(
                    "xml.id.ambiguous",
                    entry.line(),
                    1,
                    entry.path(),
                    format!("`{}` is both a widget and a scroll panel", id.name()),
                );
                continue;
            }
            let space = match existing.parent() {
                Some(parent) => {
                    let size = self.panel_content_size(parent, &spaces);
                    LayoutSpace::content(size.width, size.height, constants)
                }
                None => LayoutSpace::client(client, constants),
            };
            let viewport = match entry.resolve(space) {
                Ok(viewport) => viewport,
                Err(diagnostic) => {
                    errors.push(diagnostic);
                    continue;
                }
            };
            if viewport.width() <= 0.0 || viewport.height() <= 0.0 {
                warnings.warning(
                    "xml.value.zero-size",
                    entry.line(),
                    1,
                    entry.path(),
                    "the viewport has a zero side",
                );
            }
            let mut probe = existing.clone();
            probe.set_viewport(viewport);
            if let ContentHeight::Exact(expression) = entry.content_height() {
                let current = existing.content_rect().size();
                match expr::eval(
                    expression,
                    current.width as f64,
                    current.height as f64,
                    Axis::Vertical,
                    constants,
                ) {
                    Ok(height) if height >= 0.0 => probe.set_content_override(Some(Size::new(
                        current.width,
                        height as f32,
                    ))),
                    Ok(_) => errors.error(
                        "xml.geom.negative",
                        entry.line(),
                        1,
                        entry.path(),
                        "`content-height` must not be negative",
                    ),
                    Err(error) => errors.error(
                        error.code,
                        entry.line(),
                        1,
                        entry.path(),
                        format!("content-height: {}", error.message),
                    ),
                }
            }
            probe.resolve();
            let content = probe.content_rect().size();
            spaces.push((id, content));
            let height = match entry.content_height() {
                ContentHeight::Keep => PanelHeight::Keep,
                ContentHeight::Exact(expression) => PanelHeight::Exact(Box::new(move |size| {
                    expr::eval(
                        expression,
                        size.width as f64,
                        size.height as f64,
                        Axis::Vertical,
                        constants,
                    )
                    .ok()
                    .filter(|height| *height >= 0.0)
                    .map(|height| height as f32)
                })),
                ContentHeight::Auto => PanelHeight::Auto,
            };
            panels.push(PanelPlan {
                id,
                viewport,
                content,
                height,
            });
        }

        // Scroll membership (`parent="…"`, M7.7 follow-up): validate the
        // declaration once, before either entry loop resolves anything. A
        // created entry joins the panel; an addressed entry asserts the
        // application's own membership.
        let mut invalid_parents: Vec<WidgetId> = Vec::new();
        for entry in variant.entries() {
            let Some(parent) = entry.parent() else {
                continue;
            };
            if self.scroll_view(parent).is_none() {
                errors.error(
                    "xml.parent.missing",
                    entry.line(),
                    1,
                    entry.path(),
                    format!(
                        "`{}` is not a registered scroll panel",
                        parent.name()
                    ),
                );
                invalid_parents.push(entry.id());
                continue;
            }
            if self.contains(parent) {
                errors.error(
                    "xml.parent.missing",
                    entry.line(),
                    1,
                    entry.path(),
                    format!(
                        "`{}` is both a widget and a scroll panel; `parent` is ambiguous",
                        parent.name()
                    ),
                );
                invalid_parents.push(entry.id());
                continue;
            }
            if entry.type_name().is_none() && self.scroll_owner_of(entry.id()) != Some(parent)
            {
                errors.error(
                    "xml.parent.missing",
                    entry.line(),
                    1,
                    entry.path(),
                    format!(
                        "`{}` is not a child of scroll panel `{}`",
                        entry.id().name(),
                        parent.name()
                    ),
                );
            }
        }

        // Document-created widgets (M7.7): a typed entry is resolved once here
        // so the factory sees the same rectangle the applier writes, and the
        // widget loop below finds the staged widget. A created widget without a
        // parent is client-space; with `parent` its rectangle is in the panel's
        // content space (§17: v1 stays flat otherwise).
        let generation_changed = self
            .installed_layout()
            .map(|layout| layout.generation())
            != Some(document.generation());
        let mut typed_rects: Vec<(WidgetId, Rect)> = Vec::new();
        let mut requests: Vec<CreateRequest<'_>> = Vec::new();
        for entry in variant.entries() {
            let Some(type_name) = entry.type_name() else {
                continue;
            };
            let id = entry.id();
            let parent = entry
                .parent()
                .filter(|_| !invalid_parents.contains(&id));
            let layout_space = match parent {
                Some(panel) => {
                    let size = self.panel_content_size(panel, &spaces);
                    LayoutSpace::content(size.width, size.height, constants)
                }
                None => LayoutSpace::client(client, constants),
            };
            let rect = match entry.resolve(layout_space) {
                Ok(rect) => rect,
                Err(diagnostic) => {
                    errors.push(diagnostic);
                    continue;
                }
            };
            if rect.width() <= 0.0 || rect.height() <= 0.0 {
                warnings.warning(
                    "xml.value.zero-size",
                    entry.line(),
                    1,
                    entry.path(),
                    "the rectangle has a zero side",
                );
            }
            typed_rects.push((id, rect));
            requests.push(CreateRequest {
                id,
                type_name,
                command: entry.command(),
                parent,
                rect,
                content: entry.content.clone(),
                attrs: entry.skin_attrs.clone(),
                children: entry.skin_children.clone(),
                line: entry.line(),
                path: entry.path(),
            });
        }
        let creation = self.plan_creations(&requests, generation_changed, palette, client, &mut errors);

        // Widget entries.
        let mut widgets = Vec::new();
        for entry in variant.entries() {
            let id = entry.id();
            let staged = creation.create.iter().any(|widget| widget.id() == id);
            if !self.contains(id) && !staged {
                errors.error(
                    "xml.id.unknown-widget",
                    entry.line(),
                    1,
                    entry.path(),
                    format!("no widget `{}` exists", id.name()),
                );
                continue;
            }
            if self.scroll_view(id).is_some() {
                errors.error(
                    "xml.id.ambiguous",
                    entry.line(),
                    1,
                    entry.path(),
                    format!("`{}` is both a widget and a scroll panel", id.name()),
                );
                continue;
            }
            for (name, _) in &entry.rust_attrs {
                errors.error(
                    "xml.rust.unsupported",
                    entry.line(),
                    1,
                    entry.path(),
                    format!("`{name}` uses inline Rust; the dynamic mode never evaluates code"),
                );
            }
            let (space, layout_space) =
                match entry.parent().filter(|_| !invalid_parents.contains(&id)) {
                    Some(panel) => {
                        let size = self.panel_content_size(panel, &spaces);
                        (
                            EntrySpace::Content(panel),
                            LayoutSpace::content(size.width, size.height, constants),
                        )
                    }
                    None => match self.scroll_owner_of(id) {
                        None => (EntrySpace::Client, LayoutSpace::client(client, constants)),
                        Some(panel) => {
                            let size = self.panel_content_size(panel, &spaces);
                            (
                                EntrySpace::Content(panel),
                                LayoutSpace::content(size.width, size.height, constants),
                            )
                        }
                    },
                };
            let typed = typed_rects.iter().find(|(candidate, _)| *candidate == id);
            let rect = match typed {
                Some((_, rect)) => *rect,
                None => match entry.resolve(layout_space) {
                    Ok(rect) => rect,
                    Err(diagnostic) => {
                        errors.push(diagnostic);
                        continue;
                    }
                },
            };
            if rect.width() <= 0.0 || rect.height() <= 0.0 {
                if typed.is_none() {
                    warnings.warning(
                        "xml.value.zero-size",
                        entry.line(),
                        1,
                        entry.path(),
                        "the rectangle has a zero side",
                    );
                }
            }
            // The target's declarations are read from the staged widget when
            // this apply creates it, or from the tree.
            let target = creation
                .create
                .iter()
                .find(|widget| widget.id() == id)
                .map(|widget| widget.as_ref())
                .or_else(|| self.widget_ref(id));
            if entry.style.has_surface()
                && target.and_then(|widget| widget.surface_style()).is_none()
            {
                warnings.warning(
                    "xml.style.unsupported",
                    entry.line(),
                    1,
                    entry.path(),
                    "this widget does not adopt a SurfaceStyle; the style is skipped",
                );
            }
            if !entry.content.is_empty() {
                let caps = target
                    .map(|widget| widget.content_caps())
                    .unwrap_or(ContentCaps::NONE);
                for item in &entry.content {
                    let (cap, name) = capability(item);
                    if !caps.contains(cap) {
                        warnings.warning(
                            "xml.content.unsupported",
                            entry.line(),
                            1,
                            entry.path(),
                            format!("`{name}` is not supported by this widget"),
                        );
                    }
                    if let ContentAttr::Progress(value) = item {
                        if !(0.0..=1.0).contains(value) {
                            warnings.warning(
                                "xml.value.clamped",
                                entry.line(),
                                1,
                                entry.path(),
                                format!("progress {value} is clamped to 0..1"),
                            );
                        }
                    }
                }
                if self.is_editing_on(id, sink) {
                    warnings.warning(
                        "xml.content.session-open",
                        entry.line(),
                        1,
                        entry.path(),
                        "an edit session is open; the content initial value is skipped",
                    );
                }
            }
            if entry.has_skin() {
                if let Some(widget) = target {
                    self.plan_skin(
                        widget,
                        entry.line(),
                        entry.path(),
                        entry.skin_attrs(),
                        entry.skin_children(),
                        palette,
                        &mut errors,
                    );
                }
                if self.is_editing_on(id, sink) {
                    warnings.warning(
                        "xml.content.session-open",
                        entry.line(),
                        1,
                        entry.path(),
                        "an edit session is open; the local skin data is skipped",
                    );
                }
            }
            widgets.push(PreparedWidget {
                id,
                rect,
                space,
                props: &entry.props,
                style: &entry.style,
                content: Cow::Borrowed(&entry.content[..]),
                skin_attrs: Cow::Borrowed(&entry.skin_attrs[..]),
                skin_children: Cow::Borrowed(&entry.skin_children[..]),
            });
        }

        if errors.has_errors() {
            let mut all = errors.into_vec();
            all.extend(warnings.into_vec());
            return Err(LayoutFailure::new(all));
        }
        Ok(Prepared {
            generation: document.generation(),
            variant_id: variant.id().to_string(),
            panels,
            widgets,
            warnings: warnings.into_vec(),
            claims: claim_targets(document),
            remove: creation.remove,
            create: creation.create,
            created: creation.created,
        })
    }


    pub(crate) fn panel_content_size(&self, panel: WidgetId, planned: &[(WidgetId, Size)]) -> Size {
        planned
            .iter()
            .find(|(id, _)| *id == panel)
            .map(|(_, size)| *size)
            .or_else(|| self.scroll_view(panel).map(|view| view.content_rect().size()))
            .unwrap_or(Size::ZERO)
    }

    pub(crate) fn panel_depth(&self, id: WidgetId) -> usize {
        let mut depth = 0;
        let mut current = self.scroll_view(id).and_then(|view| view.parent());
        while let Some(parent) = current {
            depth += 1;
            if depth > 32 {
                break;
            }
            current = self.scroll_view(parent).and_then(|view| view.parent());
        }
        depth
    }

    // -- mutation ----------------------------------------------------------

    fn apply_panels(
        &mut self,
        panels: &[PanelPlan<'_>],
        widgets: &[PreparedWidget<'_>],
        sink: &mut impl UiHost,
    ) {
        for panel in panels {
            let Some(mut view) = self.scroll_view(panel.id).cloned() else {
                continue;
            };
            view.set_viewport(panel.viewport);
            let mut children = view.children_ref().to_vec();
            for planned in widgets {
                if planned.space != EntrySpace::Content(panel.id) {
                    continue;
                }
                if let Some(child) = children.iter_mut().find(|child| child.id == planned.id) {
                    child.rect = planned.rect;
                }
            }
            let padding = view.padding_insets();
            view.set_children(children);
            match &panel.height {
                PanelHeight::Keep => {}
                PanelHeight::Exact(evaluate) => {
                    if let Some(height) = evaluate(panel.content) {
                        if height >= 0.0 {
                            view.set_content_override(Some(Size::new(
                                panel.content.width,
                                height,
                            )));
                        }
                    }
                }
                PanelHeight::Auto => {
                    let bottom = view
                        .children_ref()
                        .iter()
                        .map(|child| child.rect.bottom.max(0.0))
                        .fold(0.0f32, f32::max)
                        + padding.bottom;
                    let height = panel
                        .content
                        .height
                        .max(panel.viewport.height())
                        .max(bottom);
                    view.set_content_override(Some(Size::new(panel.content.width, height)));
                }
            }
            self.set_scroll_view(view, sink);
        }
    }

    fn apply_client_rects(&mut self, widgets: &[PreparedWidget<'_>], sink: &mut impl UiHost) {
        for planned in widgets {
            if planned.space != EntrySpace::Client {
                continue;
            }
            if self.rect(planned.id) == Some(planned.rect) {
                continue;
            }
            self.move_rect(planned.id, planned.rect, sink);
        }
    }

    fn apply_claim(
        &mut self,
        claim: &mut Claim,
        entry: Option<&PreparedWidget<'_>>,
        palette: &Palette,
        sink: &mut impl UiHost,
    ) {
        if !self.contains(claim.id) {
            return;
        }
        let declared_props = entry
            .map(|entry| entry.props.has_props() || entry.style.has_font())
            .unwrap_or(false);
        if claim.fields.props_any || declared_props {
            let mut props = claim.props.clone().unwrap_or_default();
            if let Some(entry) = entry {
                let attrs = entry.props;
                if let Some(value) = attrs.interactive {
                    props.interactive = Some(value);
                }
                if let Some(value) = attrs.focusable {
                    props.focusable = Some(value);
                }
                if let Some(value) = attrs.focus_order {
                    props.focus_order = Some(value);
                }
                if let Some(value) = attrs.read_only {
                    props.read_only = Some(value);
                }
                if let Some(value) = attrs.tooltip.as_ref() {
                    props.tooltip = Some(value.clone());
                }
                if let Some(value) = attrs.automation_id.as_ref() {
                    props.automation_id = Some(value.clone());
                }
                if let Some(value) = attrs.role {
                    props.role = Some(value);
                }
                if let Some(value) = attrs.expose {
                    props.exposed = Some(value);
                }
                if let Some(value) = attrs.cursor {
                    props.cursor = Some(value);
                }
                if attrs.name.is_some() || attrs.description.is_some() {
                    let mut narration = props.narration.clone().unwrap_or_default();
                    if let Some(name) = attrs.name.as_ref() {
                        narration = narration.name(name.clone());
                    }
                    if let Some(description) = attrs.description.as_ref() {
                        narration = narration.description(description.clone());
                    }
                    props.narration = Some(narration);
                }
                if entry.style.font_size.is_some() || entry.style.font_family.is_some() {
                    let mut font = props.font.clone().unwrap_or_default();
                    if let Some(size) = entry.style.font_size {
                        font.size = Some(size);
                    }
                    if let Some(family) = entry.style.font_family.as_ref() {
                        font.family = Some(family.clone());
                    }
                    props.font = if font.is_empty() { None } else { Some(font) };
                }
            }
            self.set_props(claim.id, props, sink);
        }
        if claim.fields.cursor {
            let cursor = entry.and_then(|entry| entry.props.cursor).or(claim.cursor);
            self.set_cursor(claim.id, cursor);
        }
        if claim.fields.visible {
            match entry.and_then(|entry| entry.props.visible) {
                Some(value) => self.set_visible(claim.id, value, sink),
                None => match claim.visible_override {
                    Some(value) => self.set_visible(claim.id, value, sink),
                    None => self.clear_visibility_override(claim.id, claim.widget_visible, sink),
                },
            }
        }
        if claim.fields.enabled {
            let value = entry
                .and_then(|entry| entry.props.enabled)
                .unwrap_or(claim.enabled);
            self.set_enabled(claim.id, value, sink);
        }
        if claim.fields.focusable || claim.fields.focus_order {
            let focusable = entry
                .and_then(|entry| entry.props.focusable)
                .or(claim.focusable);
            let weight = entry
                .and_then(|entry| entry.props.focus_order)
                .unwrap_or(claim.focus_order);
            self.set_focus_policy(claim.id, focusable, weight);
        }
        if claim.fields.corners || claim.fields.background {
            let mut surface = claim.surface.unwrap_or_default();
            if let Some(entry) = entry {
                let height = entry.rect.height();
                if let Some(radius) = entry.style.radius {
                    surface.corners = Corners::uniform(radius.resolve(height));
                }
                if let Some(corners) = entry.style.corners {
                    surface.corners = corners;
                }
                if let Some(background) = entry.style.background {
                    match background {
                        BackgroundAttr::None => surface.transparent = true,
                        BackgroundAttr::Literal(color) => {
                            surface.background = Some(color);
                            surface.transparent = false;
                        }
                        BackgroundAttr::Token(token) => {
                            surface.background = palette_token(palette, token);
                            surface.transparent = false;
                        }
                    }
                }
            }
            self.set_surface_style_of(claim.id, surface);
        }
    }

    /// Validate one entry's widget-declared local attributes and child lists
    /// against the target widget's declarations. Pure: no mutation, no
    /// callbacks (§6.2 rules 3–4). Shared by both planning paths.
    pub(crate) fn plan_skin(
        &self,
        widget: &dyn crate::widget::Widget<M>,
        line: usize,
        path: &str,
        skin_attrs: &[(String, String)],
        skin_children: &[SkinElement],
        palette: &Palette,
        errors: &mut Diagnostics,
    ) {
        let declarations = widget.skin_attrs();
        let child_names = widget.skin_children();
        for declaration in declarations {
            if crate::skin::is_shared_name(declaration.name) {
                errors.error(
                    "xml.skin.reserved",
                    line,
                    1,
                    path,
                    format!(
                        "`{}` is a shared content name; a widget cannot redeclare it",
                        declaration.name
                    ),
                );
            }
        }
        for (name, raw) in skin_attrs {
            let Some(declaration) = declarations
                .iter()
                .find(|declaration| declaration.name == name.as_str())
            else {
                errors.error(
                    "xml.skin.unknown",
                    line,
                    1,
                    path,
                    format!("`{name}` is not a skin attribute of this widget"),
                );
                continue;
            };
            let Some(value) = declaration
                .kind
                .parse_with(raw, |name| palette_token(palette, name))
            else {
                errors.error(
                    "xml.skin.value",
                    line,
                    1,
                    path,
                    format!("`{name}` expects {}", kind_name(declaration.kind)),
                );
                continue;
            };
            if !widget.validate_skin_attr(name, &value) {
                errors.error(
                    "xml.skin.value",
                    line,
                    1,
                    path,
                    format!("`{name}` value `{raw}` is not acceptable for this widget"),
                );
            }
        }
        for child in skin_children {
            if !child_names
                .iter()
                .any(|name| *name == child.name.as_str())
            {
                errors.error(
                    "xml.skin.unknown",
                    child.line,
                    child.column,
                    path,
                    format!("<{}> is not a child element of this widget", child.name),
                );
            }
        }
        if !skin_children.is_empty() && !widget.validate_skin_children(skin_children) {
            errors.error(
                "xml.skin.value",
                line,
                1,
                path,
                "the widget rejected this child list",
            );
        }
    }

    /// Write every planned widget's local skin data, one phase at a time so the
    /// order rule holds across widgets (§6.2 rule 6).
    fn write_skin(
        &mut self,
        widgets: &[PreparedWidget<'_>],
        phase: SkinPhase,
        palette: &Palette,
        sink: &mut impl UiHost,
    ) {
        for planned in widgets {
            if !planned.has_skin() {
                continue;
            }
            if self.is_editing_on(planned.id, sink) {
                continue;
            }
            match phase {
                SkinPhase::Children => {
                    if !planned.skin_children.is_empty() {
                        self.apply_skin_of(planned.id, &[], &planned.skin_children, palette);
                    }
                }
                SkinPhase::Attributes => {
                    if !planned.skin_attrs.is_empty() {
                        self.apply_skin_of(planned.id, &planned.skin_attrs, &[], palette);
                    }
                }
            }
        }
    }

    fn write_content(&mut self, widgets: &[PreparedWidget<'_>], sink: &mut impl UiHost) -> usize {
        let mut writes = 0;
        for planned in widgets {
            if planned.content.is_empty() {
                continue;
            }
            if self.is_editing_on(planned.id, sink) {
                continue;
            }
            let caps = self.content_caps_of(planned.id).unwrap_or(ContentCaps::NONE);
            for item in planned.content.iter() {
                let (cap, _) = capability(item);
                if !caps.contains(cap) {
                    continue;
                }
                match item {
                    ContentAttr::Text(text) => self.set_text(planned.id, text.clone()),
                    ContentAttr::Label(text) => self.set_label(planned.id, text.clone()),
                    ContentAttr::Value(value) => self.set_value(planned.id, *value as f32),
                    ContentAttr::Progress(value) => {
                        self.set_progress(planned.id, (*value as f32).clamp(0.0, 1.0));
                    }
                    ContentAttr::Number(value) => self.set_number(planned.id, *value),
                    ContentAttr::Checked(value) => self.set_toggle(planned.id, *value),
                    ContentAttr::Selected(value) => self.set_selected(planned.id, *value as usize),
                }
            }
            writes += 1;
        }
        writes
    }

    // -- claims ------------------------------------------------------------

    fn capture_claims(&self, targets: &[(WidgetId, ClaimFields)]) -> Vec<Claim> {
        targets
            .iter()
            .filter_map(|(id, fields)| {
                let (widget_visible, enabled) = self.widget_flags(*id)?;
                let (focus_order, focusable) = self.focus_policy_of(*id);
                Some(Claim {
                    id: *id,
                    rect: self.rect(*id)?,
                    props: self.props_snapshot(*id),
                    visible_override: self.visibility_override(*id),
                    widget_visible,
                    enabled,
                    focusable,
                    focus_order,
                    cursor: self.cursor_override_of(*id),
                    surface: self.surface_style_of(*id),
                    fields: *fields,
                })
            })
            .collect()
    }

    fn restore_claims(&mut self, claims: &[Claim], sink: &mut impl UiHost) {
        for claim in claims {
            if !self.contains(claim.id) {
                continue;
            }
            if claim.fields.props_any {
                self.set_props(claim.id, claim.props.clone().unwrap_or_default(), sink);
            }
            if claim.fields.cursor {
                self.set_cursor(claim.id, claim.cursor);
            }
            if claim.fields.focusable || claim.fields.focus_order {
                self.set_focus_policy(claim.id, claim.focusable, claim.focus_order);
            }
            if claim.fields.visible {
                match claim.visible_override {
                    Some(value) => self.set_visible(claim.id, value, sink),
                    None => self.clear_visibility_override(claim.id, claim.widget_visible, sink),
                }
            }
            if claim.fields.enabled {
                self.set_enabled(claim.id, claim.enabled, sink);
            }
            if claim.fields.corners || claim.fields.background {
                if let Some(surface) = claim.surface {
                    self.set_surface_style_of(claim.id, surface);
                }
            }
        }
    }
}

/// Merge the claim fields of every variant, in document order.
#[cfg(feature = "layout-xml")]
fn claim_targets(document: &LayoutDocument) -> Vec<(WidgetId, ClaimFields)> {
    let mut targets: Vec<(WidgetId, ClaimFields)> = Vec::new();
    for variant in document.variants() {
        for entry in variant.entries() {
            match targets.iter_mut().find(|(id, _)| *id == entry.id()) {
                Some((_, fields)) => fields.merge(&entry.props, &entry.style),
                None => {
                    let mut fields = ClaimFields::EMPTY;
                    fields.merge(&entry.props, &entry.style);
                    targets.push((entry.id(), fields));
                }
            }
        }
    }
    targets
}

#[cfg(all(test, feature = "layout-xml"))]
mod tests {
    use super::*;
    use crate::geometry::Insets;
    use crate::scroll::{ScrollChild, ScrollView};
    use crate::ui::Invalidator;
    use crate::widget::{ScrollBars, Widget};
    use crate::widgets::{
        Accordion, Avatar, Badge, BadgeTone, Banner, BannerKind, Breadcrumb, Button, Card,
        ColorPicker, ColorSwatch, ComboBox, CopyableText, CoverWall, Date, DatePicker, Divider,
        Icon, Kbd, Label, LabelStyle, ListView, MarqueeText, NavRail, NumberInput, Pager,
        PlaybackTransport, PlaylistView, PropertyGrid, PropertyKind, RepeatMode, RichText,
        RunStyle, SearchBox, SectionTitle, Segmented, Slider, Sparkline, Spinner, StatusBar,
        Table, Tabs, TextEdit, Time, TimePicker, Toast, TreeView,
    };
    #[derive(Default)]
    struct TestHost {
        invalidated: Vec<Rect>,
        editing: Option<WidgetId>,
    }

    impl Invalidator for TestHost {
        fn invalidate(&mut self, rect: Rect) {
            self.invalidated.push(rect);
        }
    }

    impl UiHost for TestHost {
        fn editing_id(&self) -> Option<WidgetId> {
            self.editing
        }
    }

    fn sample_ui() -> Ui<&'static str> {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("action", "原操作", "clicked"));
        ui.add(Label::new("title", "原标题"));
        ui.add(Slider::new("slider", 0.0, 0.0..=1.0, |_| "changed"));
        ui.add(Divider::new("rule"));
        for id in ["action", "title", "slider", "rule"] {
            ui.set_rect(WidgetId::new(id), Rect::from_xywh(0.0, 0.0, 40.0, 20.0));
        }
        let mut host = TestHost::default();
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("page"),
                Rect::from_xywh(0.0, 0.0, 300.0, 200.0),
                ScrollBars::Vertical,
            )
            .children([ScrollChild::new(
                WidgetId::new("title"),
                Rect::from_xywh(0.0, 0.0, 40.0, 20.0),
            )]),
            &mut host,
        );
        ui
    }

    const DOC: &str = r#"<layouts version="1">
  <const name="pad" value="16"/>
  <layout id="base" min-width="0" min-height="0">
    <scroll id="page" x="0" y="0" w="300" h="200" content-height="auto"/>
    <widget id="action" x="pad" y="8" w="120" h="36"
            label="主操作" tooltip="样式来自文件" name="主操作"
            radius="medium" background="accent"/>
    <widget id="title" x="4" y="4" w="100" h="20" label="新标题"/>
    <widget id="slider" x="16" y="60" w="200" h="28" value="0.75"/>
    <widget id="rule" x="0" y="0" w="0" h="10" radius="small"/>
  </layout>
  <layout id="wide" min-width="1000" min-height="700">
    <scroll id="page" x="0" y="0" w="cw" h="ch" content-height="auto"/>
    <widget id="action" x="pad" y="8" w="120" h="36" tooltip="宽版提示"/>
    <widget id="title" x="4" y="4" w="100" h="20" label="新标题"/>
    <widget id="slider" x="16" y="60" w="200" h="28" value="0.75"/>
    <widget id="rule" x="0" y="0" w="0" h="10" radius="small"/>
  </layout>
</layouts>"#;

    fn document() -> LayoutDocument {
        LayoutDocument::parse(DOC).expect("valid document")
    }

    fn palette() -> Palette {
        Palette::new(&crate::theme::ThemeSettings::default(), true)
    }

    #[test]
    fn apply_sets_geometry_properties_style_and_content() {
        let mut ui = sample_ui();
        let mut host = TestHost::default();
        let document = document();
        let report = ui
            .apply_layout(&document, &palette(), Size::new(600.0, 400.0), &mut host)
            .expect("applies");
        assert_eq!(report.variant_id, "base");
        assert_eq!(report.content_writes, 3);
        assert_eq!(
            ui.rect(WidgetId::new("action")),
            Some(Rect::from_xywh(16.0, 8.0, 120.0, 36.0))
        );
        let surface = ui
            .surface_style_of(WidgetId::new("action"))
            .expect("button has a surface");
        assert_eq!(surface.corners, Corners::uniform(6.0));
        assert_eq!(surface.background, Some(palette().accent));
        let text = ui
            .with_widget::<Label, _>(WidgetId::new("title"), |label| label.text().to_string())
            .expect("label");
        assert_eq!(text, "新标题");
        let value = ui
            .with_widget::<Slider<&'static str>, _>(WidgetId::new("slider"), |slider| slider.value())
            .expect("slider");
        assert!((value - 0.75).abs() < 1e-6);
        let narration = ui
            .narration(WidgetId::new("action"))
            .map(|narration| narration.clone());
        assert!(narration.is_some());
    }

    #[test]
    fn scroll_children_use_content_coordinates() {
        let mut ui = sample_ui();
        let mut host = TestHost::default();
        ui.apply_layout(&document(), &palette(), Size::new(600.0, 400.0), &mut host)
            .expect("applies");
        // (4, 4) inside the panel content rect whose origin is the viewport.
        assert_eq!(
            ui.rect(WidgetId::new("title")),
            Some(Rect::from_xywh(4.0, 4.0, 100.0, 20.0))
        );
        let entry = ui
            .installed_layout()
            .expect("installed")
            .entries()
            .iter()
            .find(|entry| entry.id == WidgetId::new("title"))
            .copied()
            .expect("entry");
        assert_eq!(entry.space, EntrySpace::Content(WidgetId::new("page")));
    }

    #[test]
    fn second_apply_same_generation_writes_no_content() {
        let mut ui = sample_ui();
        let mut host = TestHost::default();
        let document = document();
        let palette = palette();
        ui.apply_layout(&document, &palette, Size::new(600.0, 400.0), &mut host)
            .expect("first");
        let report = ui
            .apply_layout(&document, &palette, Size::new(600.0, 400.0), &mut host)
            .expect("second");
        assert_eq!(report.content_writes, 0);
    }

    #[test]
    fn warnings_do_not_abort() {
        let mut ui = sample_ui();
        let mut host = TestHost::default();
        let report = ui
            .apply_layout(&document(), &palette(), Size::new(600.0, 400.0), &mut host)
            .expect("applies");
        let codes: Vec<&str> = report.warnings.iter().map(|item| item.code).collect();
        assert!(codes.contains(&"xml.style.unsupported"), "{codes:?}");
        assert!(codes.contains(&"xml.value.zero-size"), "{codes:?}");
    }

    #[test]
    fn atomic_failure_changes_nothing() {
        let mut ui = sample_ui();
        let mut host = TestHost::default();
        let bad = LayoutDocument::parse(
            r#"<layouts version="1"><layout id="base"><widget id="missing" x="0" y="0" w="1" h="1"/></layout></layouts>"#,
        )
        .expect("parses");
        let before = ui.rect(WidgetId::new("action"));
        let failure = ui
            .apply_layout(&bad, &palette(), Size::new(600.0, 400.0), &mut host)
            .expect_err("rejects");
        assert_eq!(failure.first().map(|item| item.code), Some("xml.id.unknown-widget"));
        assert_eq!(ui.rect(WidgetId::new("action")), before);
        assert!(ui.installed_layout().is_none());
    }

    #[test]
    fn breakpoint_switch_restores_claimed_fields() {
        let mut ui = sample_ui();
        let mut host = TestHost::default();
        let document = document();
        let palette = palette();
        ui.apply_layout(&document, &palette, Size::new(1200.0, 800.0), &mut host)
            .expect("wide");
        let tooltip = |ui: &Ui<&'static str>| {
            ui.props_snapshot(WidgetId::new("action"))
                .and_then(|props| props.tooltip)
        };
        assert_eq!(tooltip(&ui).as_deref(), Some("宽版提示"));
        let report = ui
            .apply_layout(&document, &palette, Size::new(600.0, 400.0), &mut host)
            .expect("base");
        assert_eq!(report.variant_id, "base");
        // The base variant declares the tooltip too, so it stays.
        assert_eq!(tooltip(&ui).as_deref(), Some("样式来自文件"));
    }

    #[test]
    fn clear_layout_restores_the_surface() {
        let mut ui = sample_ui();
        let mut host = TestHost::default();
        ui.apply_layout(&document(), &palette(), Size::new(600.0, 400.0), &mut host)
            .expect("applies");
        ui.clear_layout(&mut host);
        let surface = ui
            .surface_style_of(WidgetId::new("action"))
            .expect("surface remains readable");
        assert_eq!(surface, SurfaceStyle::new());
        assert!(ui.installed_layout().is_none());
    }

    #[test]
    fn an_open_session_skips_the_content_write() {
        let mut ui = sample_ui();
        let mut host = TestHost {
            editing: Some(WidgetId::new("title")),
            ..TestHost::default()
        };
        let report = ui
            .apply_layout(&document(), &palette(), Size::new(600.0, 400.0), &mut host)
            .expect("applies");
        let codes: Vec<&str> = report.warnings.iter().map(|item| item.code).collect();
        assert!(codes.contains(&"xml.content.session-open"), "{codes:?}");
        assert_eq!(report.content_writes, 2);
        let text = ui
            .with_widget::<Label, _>(WidgetId::new("title"), |label| label.text().to_string())
            .expect("label");
        assert_eq!(text, "原标题");
    }

    #[test]
    fn auto_content_height_grows_and_never_shrinks() {
        let mut ui = sample_ui();
        let mut host = TestHost::default();
        let document = LayoutDocument::parse(
            r#"<layouts version="1"><layout id="base">
  <scroll id="page" x="0" y="0" w="300" h="100" content-height="auto"/>
  <widget id="title" x="0" y="0" w="100" h="400" label="x"/>
</layout></layouts>"#,
        )
        .expect("valid");
        ui.apply_layout(&document, &palette(), Size::new(600.0, 400.0), &mut host)
            .expect("applies");
        let panel = ui.scroll_view(WidgetId::new("page")).expect("panel");
        assert!(panel.content_size().height >= 400.0);
        assert_eq!(panel.content_size().width, 300.0);
        assert_eq!(panel.padding_insets(), Insets::ZERO);
    }

    // A third-party-shaped widget: it declares one local scalar and one data
    // child name of its own, outside the library's vocabulary.
    static PROBE_ATTRS: &[crate::skin::SkinAttrDecl] = &[crate::skin::SkinAttrDecl::new(
        "x-level",
        crate::skin::SkinValueKind::Number,
    )];
    static PROBE_RESERVED_ATTRS: &[crate::skin::SkinAttrDecl] = &[
        crate::skin::SkinAttrDecl::new("x-level", crate::skin::SkinValueKind::Number),
        crate::skin::SkinAttrDecl::new("text", crate::skin::SkinValueKind::Text),
    ];
    static PROBE_CHILDREN: &[&str] = &["x-item"];

    #[derive(Clone)]
    struct SkinProbe {
        id: WidgetId,
        rect: Rect,
        level: f64,
        items: Vec<String>,
        applies: usize,
        items_when_applied: Option<usize>,
        declare_shared: bool,
    }

    impl SkinProbe {
        fn new() -> Self {
            Self {
                id: WidgetId::new("probe"),
                rect: Rect::from_xywh(0.0, 0.0, 80.0, 20.0),
                level: 0.0,
                items: Vec::new(),
                applies: 0,
                items_when_applied: None,
                declare_shared: false,
            }
        }
    }

    impl Widget<&'static str> for SkinProbe {
        fn id(&self) -> WidgetId {
            self.id
        }

        fn rect(&self) -> Rect {
            self.rect
        }

        fn set_rect(&mut self, rect: Rect) {
            self.rect = rect;
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn paint(&self, _ctx: &mut crate::widget::WidgetPaint<'_>, _state: &crate::widget::WidgetState) {}

        fn input(&mut self, _input: crate::widget::WidgetInput) -> Option<&'static str> {
            None
        }

        fn skin_attrs(&self) -> &'static [crate::skin::SkinAttrDecl] {
            if self.declare_shared {
                PROBE_RESERVED_ATTRS
            } else {
                PROBE_ATTRS
            }
        }

        fn validate_skin_attr(
            &self,
            name: &str,
            value: &crate::skin::SkinValue<'_>,
        ) -> bool {
            match (name, value) {
                ("x-level", crate::skin::SkinValue::Number(level)) => (0.0..=100.0).contains(level),
                _ => true,
            }
        }

        fn apply_skin_attr(&mut self, name: &str, value: &crate::skin::SkinValue<'_>) {
            if let ("x-level", crate::skin::SkinValue::Number(level)) = (name, value) {
                self.level = *level;
                self.items_when_applied = Some(self.items.len());
                self.applies += 1;
            }
        }

        fn skin_children(&self) -> &'static [&'static str] {
            PROBE_CHILDREN
        }

        fn apply_skin_children(&mut self, children: &[crate::skin::SkinElement]) {
            self.items = children
                .iter()
                .filter_map(|child| child.attr("title").map(str::to_string))
                .collect();
        }
    }

    fn skin_ui() -> Ui<&'static str> {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(SkinProbe::new());
        ui
    }

    const SKIN_DOC: &str = r#"<layouts version="1"><layout id="base">
  <widget id="probe" x="0" y="0" w="80" h="20" x-level="72">
    <x-item title="夜曲"/>
    <x-item title="晴天"/>
  </widget>
</layout></layouts>"#;

    fn probe(ui: &Ui<&'static str>) -> SkinProbe {
        ui.with_widget::<SkinProbe, _>(WidgetId::new("probe"), |probe| probe.clone())
            .expect("probe")
    }

    #[test]
    fn skin_data_applies_once_per_generation_children_first() {
        let mut ui = skin_ui();
        let mut host = TestHost::default();
        let document = LayoutDocument::parse(SKIN_DOC).expect("valid");
        let report = ui
            .apply_layout(&document, &palette(), Size::new(600.0, 400.0), &mut host)
            .expect("applies");
        assert_eq!(report.skin_writes, 1);
        let state = probe(&ui);
        assert_eq!(state.level, 72.0);
        assert_eq!(state.items, ["夜曲", "晴天"]);
        // The child list was in place before the attribute was applied.
        assert_eq!(state.items_when_applied, Some(2));
        assert_eq!(state.applies, 1);

        // A second apply of the same generation writes nothing, so an
        // application change survives a resize.
        ui.edit_widget::<SkinProbe>(WidgetId::new("probe"), |probe| {
            probe.level = 5.0;
            true
        });
        let report = ui
            .apply_layout(&document, &palette(), Size::new(700.0, 500.0), &mut host)
            .expect("re-applies");
        assert_eq!(report.skin_writes, 0);
        assert_eq!(probe(&ui).level, 5.0);
    }

    #[test]
    fn undeclared_skin_names_reject_before_any_mutation() {
        let mut ui = skin_ui();
        let mut host = TestHost::default();
        let cases = [
            (
                r#"<widget id="probe" x="0" y="0" w="80" h="20" x-colour="red"/>"#,
                "xml.skin.unknown",
            ),
            (
                r#"<widget id="probe" x="0" y="0" w="80" h="20" x-level="abc"/>"#,
                "xml.skin.value",
            ),
            (
                r#"<widget id="probe" x="0" y="0" w="80" h="20" x-level="200"/>"#,
                "xml.skin.value",
            ),
            (
                r#"<widget id="probe" x="0" y="0" w="80" h="20"><y-item/></widget>"#,
                "xml.skin.unknown",
            ),
        ];
        for (entry, code) in cases {
            let text = format!("<layouts version=\"1\"><layout id=\"base\">{entry}</layout></layouts>");
            let document = LayoutDocument::parse(&text).expect("parses");
            let failure = ui
                .apply_layout(&document, &palette(), Size::new(600.0, 400.0), &mut host)
                .expect_err("rejects");
            assert_eq!(failure.first().map(|item| item.code), Some(code), "{entry}");
            let state = probe(&ui);
            assert_eq!(state.level, 0.0, "{entry}");
            assert!(state.items.is_empty(), "{entry}");
            assert_eq!(state.applies, 0, "{entry}");
            assert!(ui.installed_layout().is_none(), "{entry}");
        }
    }

    #[test]
    fn a_widget_cannot_redeclare_a_shared_name() {
        let mut ui: Ui<&'static str> = Ui::new();
        let mut probe_widget = SkinProbe::new();
        probe_widget.declare_shared = true;
        ui.add(probe_widget);
        let mut host = TestHost::default();
        let document = LayoutDocument::parse(SKIN_DOC).expect("valid");
        let failure = ui
            .apply_layout(&document, &palette(), Size::new(600.0, 400.0), &mut host)
            .expect_err("rejects");
        assert_eq!(
            failure.first().map(|item| item.code),
            Some("xml.skin.reserved")
        );
        assert_eq!(probe(&ui).level, 0.0);
    }

    #[test]
    fn every_declared_widget_id_is_claimed() {
        let mut ui = sample_ui();
        let mut host = TestHost::default();
        ui.apply_layout(&document(), &palette(), Size::new(600.0, 400.0), &mut host)
            .expect("applies");
        let layout = ui.installed_layout().expect("installed");
        let mut ids: Vec<&str> = layout.claimed_ids().map(|id| id.name()).collect();
        ids.sort_unstable();
        assert_eq!(ids, ["action", "rule", "slider", "title"]);
    }

    // Every built-in widget that declares local scalars, each attribute written
    // through the real applier. The typed-access assertions are the coverage
    // gate: a declaration without a working setter fails here, so the
    // vocabulary cannot drift from the implementation (M7.3).
    const BUILTIN_SKIN_DOC: &str = r##"<layouts version="1"><layout id="base">
  <widget id="card" x="0" y="0" w="100" h="60" title="新标题" body="新正文" emphasis="true"/>
  <widget id="icon" x="0" y="0" w="20" h="20" glyph="B" tint="#102030"/>
  <widget id="badge" x="0" y="0" w="40" h="20" tone="danger"/>
  <widget id="avatar" x="0" y="0" w="30" h="30" initials="XY" tint="#445566" ring="true" image="7"/>
  <widget id="kbd" x="0" y="0" w="40" h="20" shortcut="Ctrl+K"/>
  <widget id="swatch" x="0" y="0" w="80" h="24" color="#ff0000"/>
  <widget id="toast" x="0" y="0" w="100" h="30" message="提示" error="true"/>
  <widget id="banner" x="0" y="0" w="200" h="50" body="横幅正文"/>
  <widget id="spinner" x="0" y="0" w="20" h="20" phase="90" animating="false"/>
  <widget id="edit" x="0" y="0" w="100" h="30" cue="请输入" color="#010203"/>
  <widget id="search" x="0" y="0" w="100" h="30" cue="搜索词" color="#040506"/>
  <widget id="num" x="0" y="0" w="100" h="30" prefix="¥" suffix="元"/>
  <widget id="acc" x="0" y="0" w="100" h="80" open="1"/>
  <widget id="transport" x="0" y="0" w="200" h="30" playing="true" shuffle="true" repeat="one"/>
  <widget id="marquee" x="0" y="0" w="100" h="20" offset="12" color="#0a0b0c" text-style="small"/>
  <widget id="label" x="0" y="0" w="100" h="24" color="#102030" text-style="title"/>
  <widget id="section" x="0" y="0" w="100" h="24" color="accent"/>
  <widget id="copy" x="0" y="0" w="100" h="22" color="#445566" text-style="body"/>
  <widget id="rich" x="0" y="0" w="100" h="24" color="#778899"/>
  <widget id="date" x="0" y="0" w="100" h="30" date="2024-03-05"/>
  <widget id="time" x="0" y="0" w="100" h="30" time="07:45"/>
  <widget id="color" x="0" y="0" w="100" h="30" color="#123456"/>
</layout></layouts>"##;

    fn builtin_ui() -> Ui<&'static str> {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Card::new("card", "旧标题", "旧正文"));
        ui.add(Icon::new("icon", 'A'));
        ui.add(Badge::new("badge", "标签"));
        ui.add(Avatar::new("avatar", "张三"));
        ui.add(Kbd::new("kbd", "Ctrl"));
        ui.add(ColorSwatch::new("swatch", crate::Color::new(0, 0, 0), "色"));
        ui.add(Toast::new("toast"));
        ui.add(Banner::new("banner", BannerKind::Info, "横幅"));
        ui.add(Spinner::new("spinner"));
        ui.add(TextEdit::new("edit", "", |_| "edit"));
        ui.add(SearchBox::new("search", "", |_| "search"));
        ui.add(NumberInput::new("num", 0, 0..=100, |_| "num"));
        ui.add(Accordion::new("acc", &["一", "二"], |_, _| "acc"));
        ui.add(PlaybackTransport::new("transport", |_| "transport"));
        ui.add(MarqueeText::new("marquee", "滚动"));
        ui.add(Label::new("label", "标签"));
        ui.add(SectionTitle::new("section", "小节"));
        ui.add(CopyableText::<&'static str>::new("copy", "复制"));
        ui.add(RichText::<&'static str>::new("rich"));
        ui.add(DatePicker::new("date", Date::new(2000, 1, 1), |_| "date"));
        ui.add(TimePicker::new("time", Time::new(0, 0), |_| "time"));
        ui.add(ColorPicker::new("color", crate::Color::new(0, 0, 0), |_| "color"));
        ui
    }

    fn read<T: 'static + Widget<&'static str>, R>(ui: &Ui<&'static str>, id: &'static str, f: impl Fn(&T) -> R) -> R {
        ui.with_widget::<T, _>(WidgetId::new(id), f).expect("widget")
    }

    #[test]
    fn builtin_local_scalars_reach_the_widget() {
        let mut ui = builtin_ui();
        let mut host = TestHost::default();
        let document = LayoutDocument::parse(BUILTIN_SKIN_DOC).expect("valid");
        let report = ui
            .apply_layout(&document, &palette(), Size::new(800.0, 600.0), &mut host)
            .expect("applies");
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        let (title, body, emphasis) = read::<Card, _>(&ui, "card", |card| {
            (card.title().to_string(), card.body().to_string(), card.is_emphasis())
        });
        assert_eq!((title.as_str(), body.as_str(), emphasis), ("新标题", "新正文", true));

        assert_eq!(read::<Icon, _>(&ui, "icon", |icon| icon.glyph()), 'B');
        assert_eq!(
            read::<Icon, _>(&ui, "icon", |icon| icon.tint_color()),
            Some(crate::Color::new(0x10, 0x20, 0x30))
        );

        assert_eq!(read::<Badge<&'static str>, _>(&ui, "badge", |badge| badge.tone_value()), BadgeTone::Danger);

        let (initials, tint, ring, image) = read::<Avatar, _>(&ui, "avatar", |avatar| {
            (
                avatar.initials_text().to_string(),
                avatar.tint_color(),
                avatar.has_ring(),
                avatar.image_id(),
            )
        });
        assert_eq!(initials, "XY");
        assert_eq!(tint, Some(crate::Color::new(0x44, 0x55, 0x66)));
        assert!(ring);
        assert_eq!(
            image,
            Some(crate::backend::ImageId(7)),
            "the numeric ImageId, the `<cover image>` convention"
        );

        assert_eq!(read::<Kbd, _>(&ui, "kbd", |kbd| kbd.text().to_string()), "Ctrl+K");
        assert_eq!(read::<ColorSwatch, _>(&ui, "swatch", |swatch| swatch.color()), crate::Color::new(255, 0, 0));

        let (message, error) = read::<Toast, _>(&ui, "toast", |toast| {
            (toast.text().to_string(), toast.is_error())
        });
        assert_eq!((message.as_str(), error), ("提示", true));

        assert_eq!(read::<Banner<&'static str>, _>(&ui, "banner", |banner| banner.body_text().to_string()), "横幅正文");

        let (phase, animating) = read::<Spinner, _>(&ui, "spinner", |spinner| {
            (spinner.phase(), spinner.is_animating())
        });
        assert!((phase - 90.0).abs() < 0.01, "{phase}");
        assert!(!animating);

        assert_eq!(read::<TextEdit<&'static str>, _>(&ui, "edit", |edit| edit.cue_text().to_string()), "请输入");
        assert_eq!(read::<SearchBox<&'static str>, _>(&ui, "search", |search| search.cue_text().to_string()), "搜索词");
        let (prefix, suffix) = read::<NumberInput<&'static str>, _>(&ui, "num", |number| {
            (number.prefix_text().to_string(), number.suffix_text().to_string())
        });
        assert_eq!((prefix.as_str(), suffix.as_str()), ("¥", "元"));

        let (first, second) = read::<Accordion<&'static str>, _>(&ui, "acc", |accordion| {
            (accordion.is_expanded(0), accordion.is_expanded(1))
        });
        assert!(!first && second);

        let (playing, shuffle, repeat) = read::<PlaybackTransport<&'static str>, _>(&ui, "transport", |transport| {
            (
                transport.is_playing(),
                transport.shuffle_enabled(),
                transport.repeat_mode(),
            )
        });
        assert!(playing && shuffle);
        assert_eq!(repeat, RepeatMode::One);

        assert!((read::<MarqueeText, _>(&ui, "marquee", |marquee| marquee.offset()) - 12.0).abs() < 0.01);
        assert_eq!(
            read::<MarqueeText, _>(&ui, "marquee", |marquee| (
                marquee.color_value(),
                marquee.text_style()
            )),
            (Some(crate::Color::new(0x0a, 0x0b, 0x0c)), LabelStyle::Small)
        );

        // M7.8: the text colour and role on the text-bearing widgets.
        assert_eq!(
            read::<Label, _>(&ui, "label", |label| (
                label.color_value(),
                label.text_style()
            )),
            (Some(crate::Color::new(0x10, 0x20, 0x30)), LabelStyle::Title)
        );
        assert_eq!(
            read::<SectionTitle, _>(&ui, "section", |title| title.color_value()),
            Some(palette().accent),
            "a palette token resolves before the widget sees it"
        );
        assert_eq!(
            read::<CopyableText<&'static str>, _>(&ui, "copy", |copy| (
                copy.color_value(),
                copy.text_style()
            )),
            (Some(crate::Color::new(0x44, 0x55, 0x66)), LabelStyle::Body)
        );
        assert_eq!(
            read::<RichText<&'static str>, _>(&ui, "rich", |rich| rich.color_value()),
            Some(crate::Color::new(0x77, 0x88, 0x99))
        );
        assert_eq!(
            read::<TextEdit<&'static str>, _>(&ui, "edit", |edit| edit.text_color_value()),
            Some(crate::Color::new(0x01, 0x02, 0x03))
        );
        assert_eq!(
            read::<SearchBox<&'static str>, _>(&ui, "search", |search| search.color_value()),
            Some(crate::Color::new(0x04, 0x05, 0x06))
        );

        assert_eq!(read::<DatePicker<&'static str>, _>(&ui, "date", |picker| picker.value()), Date::new(2024, 3, 5));
        assert_eq!(read::<TimePicker<&'static str>, _>(&ui, "time", |picker| picker.value()), Time::new(7, 45));
        assert_eq!(
            read::<ColorPicker<&'static str>, _>(&ui, "color", |picker| picker.value()),
            crate::Color::new(0x12, 0x34, 0x56)
        );
    }

    #[test]
    fn a_builtin_rejects_an_unparsable_local_value_atomically() {
        let cases = [
            (r#"<widget id="badge" x="0" y="0" w="40" h="20" tone="bogus"/>"#, "badge"),
            (r#"<widget id="icon" x="0" y="0" w="20" h="20" glyph="AB"/>"#, "icon"),
            (r#"<widget id="date" x="0" y="0" w="100" h="30" date="03/05/2024"/>"#, "date"),
            (r#"<widget id="acc" x="0" y="0" w="100" h="80" open="-1"/>"#, "acc"),
            (r#"<widget id="transport" x="0" y="0" w="200" h="30" repeat="loop"/>"#, "transport"),
            (r#"<widget id="avatar" x="0" y="0" w="30" h="30" image="-1"/>"#, "avatar"),
            (r#"<widget id="avatar" x="0" y="0" w="30" h="30" image="1.5"/>"#, "avatar"),
            (r#"<widget id="label" x="0" y="0" w="40" h="20" text-style="huge"/>"#, "label"),
            (r#"<widget id="label" x="0" y="0" w="40" h="20" color="not-a-color"/>"#, "label"),
            (r#"<widget id="edit" x="0" y="0" w="40" h="20" color="rgb(1,2)"/>"#, "edit"),
        ];
        for (entry, id) in cases {
            let mut ui = builtin_ui();
            let mut host = TestHost::default();
            let text = format!("<layouts version=\"1\"><layout id=\"base\">{entry}</layout></layouts>");
            let document = LayoutDocument::parse(&text).expect("parses");
            let failure = ui
                .apply_layout(&document, &palette(), Size::new(800.0, 600.0), &mut host)
                .expect_err("rejects");
            assert_eq!(failure.first().map(|item| item.code), Some("xml.skin.value"), "{entry}");
            assert!(ui.installed_layout().is_none(), "{entry}");
            // Nothing was applied: the widget keeps its construction value.
            let _ = id;
            if id == "badge" {
                assert_eq!(read::<Badge<&'static str>, _>(&ui, "badge", |badge| badge.tone_value()), BadgeTone::Neutral);
            }
        }
    }

    // M7.3+: a local `Color` attribute (a widget's own tint/color) accepts the
    // same palette tokens as `background`, so a skin can follow the theme
    // instead of hard-coding hex. An unknown name is still rejected.
    #[test]
    fn local_colors_accept_palette_tokens() {
        let mut ui = builtin_ui();
        let mut host = TestHost::default();
        let document = LayoutDocument::parse(
            r##"<layouts version="1"><layout id="base">
  <widget id="icon" x="0" y="0" w="20" h="20" tint="accent"/>
  <widget id="swatch" x="0" y="0" w="80" h="24" color="card"/>
</layout></layouts>"##,
        )
        .expect("valid");
        let palette = palette();
        ui.apply_layout(&document, &palette, Size::new(800.0, 600.0), &mut host)
            .expect("applies");
        assert_eq!(
            read::<Icon, _>(&ui, "icon", |icon| icon.tint_color()),
            Some(palette.accent)
        );
        assert_eq!(
            read::<ColorSwatch, _>(&ui, "swatch", |swatch| swatch.color()),
            palette.card
        );

        // Hex still parses, and an unknown token rejects the document.
        let hex = LayoutDocument::parse(
            r##"<layouts version="1"><layout id="base">
  <widget id="icon" x="0" y="0" w="20" h="20" tint="#112233"/>
</layout></layouts>"##,
        )
        .expect("valid");
        ui.apply_layout(&hex, &palette, Size::new(800.0, 600.0), &mut host)
            .expect("applies");
        assert_eq!(
            read::<Icon, _>(&ui, "icon", |icon| icon.tint_color()),
            Some(crate::Color::new(0x11, 0x22, 0x33))
        );

        // An RGBA value drops alpha on a widget that has none (Icon) and keeps
        // it on the color picker, whose default color is the same attribute.
        let translucent = LayoutDocument::parse(
            r##"<layouts version="1"><layout id="base">
  <widget id="icon" x="0" y="0" w="20" h="20" tint="#ff000080"/>
  <widget id="color" x="0" y="0" w="100" h="30" color="#12345678"/>
</layout></layouts>"##,
        )
        .expect("valid");
        ui.apply_layout(&translucent, &palette, Size::new(800.0, 600.0), &mut host)
            .expect("applies");
        assert_eq!(
            read::<Icon, _>(&ui, "icon", |icon| icon.tint_color()),
            Some(crate::Color::new(255, 0, 0))
        );
        assert_eq!(
            read::<ColorPicker<&'static str>, _>(&ui, "color", |picker| picker.rgba()),
            crate::Rgba::new(crate::Color::new(0x12, 0x34, 0x56), 0x78)
        );

        let unknown = LayoutDocument::parse(
            r##"<layouts version="1"><layout id="base">
  <widget id="icon" x="0" y="0" w="20" h="20" tint="not-a-token"/>
</layout></layouts>"##,
        )
        .expect("valid");
        let failure = ui
            .apply_layout(&unknown, &palette, Size::new(800.0, 600.0), &mut host)
            .expect_err("rejects");
        assert_eq!(
            failure.first().map(|item| item.code),
            Some("xml.skin.value")
        );
    }

    // M7.4: the data-only child lists. `<item>`/`<track>` share `Row`,
    // `<option>` feeds the combo box, `<cover>` the wall and `<segment>` the
    // status bar; `selected` (shared content) applies after the data.
    const CHILD_LIST_DOC: &str = r##"<layouts version="1"><layout id="base">
  <widget id="list" x="0" y="0" w="300" h="200" selected="1">
    <item title="夜曲" subtitle="周杰伦" trailing="3:58"/>
    <item title="晴天" subtitle="周杰伦" current="true"/>
  </widget>
  <widget id="playlist" x="0" y="0" w="300" h="200" selected="1">
    <track title="稻香" subtitle="周杰伦" trailing="3:43"/>
    <track title="七里香" subtitle="周杰伦" disabled="true"/>
  </widget>
  <widget id="combo" x="0" y="0" w="200" h="32" selected="2">
    <option label="低"/>
    <option label="中"/>
    <option label="高"/>
  </widget>
  <widget id="wall" x="0" y="0" w="400" h="300" selected="1">
    <cover title="专辑甲" subtitle="歌手" image="7" corner-radius="6"/>
    <cover title="专辑乙"/>
  </widget>
  <widget id="status" x="0" y="0" w="400" h="24">
    <segment text="就绪"/>
    <segment text="12 首" icon="♪" width="90" align="right"/>
  </widget>
</layout></layouts>"##;

    fn child_list_ui() -> Ui<&'static str> {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(ListView::new("list"));
        ui.add(PlaylistView::new("playlist"));
        ui.add(ComboBox::new("combo", &["x"], 0, |_| "combo"));
        ui.add(CoverWall::new("wall"));
        ui.add(StatusBar::new("status"));
        ui.add(Table::new("table"));
        ui.add(TreeView::new("tree"));
        ui.add(PropertyGrid::new("props"));
        ui.add(Pager::new("pager", 0, 1, |_| "nav"));
        ui.add(Tabs::new("tabs", &["x"], 0, |_| "tab"));
        ui.add(Segmented::new("seg", &["x"], 0, |_| "seg"));
        ui.add(Breadcrumb::new("crumb", &["x"], |_| "crumb"));
        ui.add(NavRail::new("rail", &["x"], 0, |_| "rail"));
        ui.add(RichText::new("rich"));
        ui.add(Sparkline::new("spark", &[0.0]));
        ui
    }

    #[test]
    fn built_in_child_lists_replace_the_data_models() {
        let mut ui = child_list_ui();
        let mut host = TestHost::default();
        let document = LayoutDocument::parse(CHILD_LIST_DOC).expect("valid");
        let report = ui
            .apply_layout(&document, &palette(), Size::new(800.0, 600.0), &mut host)
            .expect("applies");
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        let rows = read::<ListView<&'static str>, _>(&ui, "list", |list| {
            (list.rows().to_vec(), list.selected_row())
        });
        assert_eq!(rows.0.len(), 2);
        assert_eq!(rows.0[0].title, "夜曲");
        assert_eq!(rows.0[0].subtitle, "周杰伦");
        assert_eq!(rows.0[0].trailing, "3:58");
        assert!(rows.0[1].current);
        assert_eq!(rows.1, 1);

        let tracks = read::<PlaylistView<&'static str>, _>(&ui, "playlist", |list| {
            (list.rows_ref().to_vec(), list.selected_index())
        });
        assert_eq!(tracks.0.len(), 2);
        assert_eq!(tracks.0[0].title, "稻香");
        assert!(tracks.0[1].disabled);
        assert_eq!(tracks.1, 1);

        let (items, selected, text) = read::<ComboBox<&'static str>, _>(&ui, "combo", |combo| {
            (
                combo.items().to_vec(),
                combo.selected(),
                combo.selected_text().map(str::to_string),
            )
        });
        assert_eq!(items, ["低", "中", "高"]);
        assert_eq!(selected, 2);
        assert_eq!(text.as_deref(), Some("高"));

        let (covers, selected) = read::<CoverWall<&'static str>, _>(&ui, "wall", |wall| {
            (wall.covers_ref().to_vec(), wall.selected_index())
        });
        assert_eq!(covers.len(), 2);
        assert_eq!(covers[0].title, "专辑甲");
        assert_eq!(covers[0].subtitle, "歌手");
        assert_eq!(covers[0].image, Some(crate::backend::ImageId(7)));
        assert_eq!(covers[0].corner_radius, Some(6.0));
        assert_eq!(covers[1].image, None);
        assert_eq!(selected, 1);

        let segments = read::<StatusBar, _>(&ui, "status", |bar| {
            bar.segments()
                .iter()
                .map(|segment| {
                    (
                        segment.text().to_string(),
                        segment.icon_glyph(),
                        segment.fixed_width(),
                        segment.text_align(),
                    )
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0], ("就绪".to_string(), None, None, crate::backend::TextAlign::Left));
        assert_eq!(segments[1].0, "12 首");
        assert_eq!(segments[1].1, Some('♪'));
        assert_eq!(segments[1].2, Some(90.0));
        assert_eq!(segments[1].3, crate::backend::TextAlign::Right);
    }

    // M7.4 step 2: the structural child lists. `Table` columns and rows,
    // `TreeView` nested nodes, `PropertyGrid` sections with typed rows and
    // `Pager` per-page labels. `selected` still applies after the data.
    const STRUCTURAL_CHILD_LIST_DOC: &str = r##"<layouts version="1"><layout id="base">
  <widget id="table" x="0" y="0" w="400" h="200" selected="1">
    <column title="编号" width="64" align="right"/>
    <column title="曲名" flex="true"/>
    <row disabled="true"><cell text="01"/><cell text="夜曲"/></row>
    <row><cell text="02"/><cell text="晴天"/></row>
  </widget>
  <widget id="tree" x="0" y="0" w="300" h="200" selected="1">
    <node label="音乐库" glyph="♪" expanded="true">
      <node label="专辑" secondary="12" expanded="true">
        <node label="夜曲" glyph="♫" disabled="true"/>
      </node>
    </node>
  </widget>
  <widget id="props" x="0" y="0" w="300" h="200">
    <section title="音频">
      <row key="名称" value="夜曲"/>
      <row key="音量" value="72" kind="number" min="0" max="100" step="2" decimals="1"/>
      <row key="音质" value="无损" kind="choice" options="无损|高品质|标准"/>
      <row key="收藏" value="否" kind="check" on="是" off="否" read-only="true"/>
      <row key="评分" value="4" kind="rating" stars="5"/>
      <row key="备注" value="可编辑" kind="text"/>
    </section>
  </widget>
  <widget id="pager" x="0" y="0" w="300" h="32" selected="2">
    <page label="封面"/>
    <page label="曲目"/>
    <page label="详情"/>
    <page label="设置"/>
  </widget>
</layout></layouts>"##;

    #[test]
    fn structural_child_lists_replace_the_data_models() {
        let mut ui = child_list_ui();
        let mut host = TestHost::default();
        let document = LayoutDocument::parse(STRUCTURAL_CHILD_LIST_DOC).expect("valid");
        let report = ui
            .apply_layout(&document, &palette(), Size::new(800.0, 600.0), &mut host)
            .expect("applies");
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        let table = read::<Table<&'static str>, _>(&ui, "table", |table| {
            (
                table
                    .columns_ref()
                    .iter()
                    .map(|column| {
                        (
                            column.title().to_string(),
                            column.width(),
                            column.is_flex(),
                            column.alignment(),
                        )
                    })
                    .collect::<Vec<_>>(),
                table
                    .rows_ref()
                    .iter()
                    .map(|row| (row.cells.clone(), row.disabled))
                    .collect::<Vec<_>>(),
                table.selected_row(),
            )
        });
        assert_eq!(table.0.len(), 2);
        assert_eq!(
            table.0[0],
            ("编号".to_string(), 64.0, false, crate::backend::TextAlign::Right)
        );
        assert_eq!(table.0[1], ("曲名".to_string(), 0.0, true, crate::backend::TextAlign::Left));
        assert_eq!(table.1.len(), 2);
        assert_eq!(table.1[0], (vec!["01".to_string(), "夜曲".to_string()], true));
        assert_eq!(table.1[1], (vec!["02".to_string(), "晴天".to_string()], false));
        assert_eq!(table.2, 1);

        let tree = read::<TreeView<&'static str>, _>(&ui, "tree", |tree| {
            (
                tree.items_ref().to_vec(),
                tree.visible_row_count(),
                tree.selected_row(),
            )
        });
        assert_eq!(tree.0.len(), 1);
        assert_eq!(tree.0[0].label, "音乐库");
        assert_eq!(tree.0[0].glyph, Some('♪'));
        assert!(tree.0[0].expanded);
        let album = &tree.0[0].children[0];
        assert_eq!(album.label, "专辑");
        assert_eq!(album.secondary, "12");
        assert!(album.expanded);
        assert_eq!(album.children[0].label, "夜曲");
        assert_eq!(album.children[0].glyph, Some('♫'));
        assert!(album.children[0].disabled);
        // root, 专辑 and 夜曲 are the three visible rows; `selected="1"` is 专辑.
        assert_eq!(tree.1, 3);
        assert_eq!(tree.2, 1);

        let props = read::<PropertyGrid<&'static str>, _>(&ui, "props", |grid| {
            let sections = grid.sections_ref();
            (
                sections.len(),
                sections[0].title().to_string(),
                sections[0]
                    .rows_ref()
                    .iter()
                    .map(|row| {
                        (
                            row.key().to_string(),
                            row.value().to_string(),
                            row.kind().clone(),
                            row.is_read_only(),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        });
        assert_eq!(props.0, 1);
        assert_eq!(props.1, "音频");
        assert_eq!(props.2.len(), 6);
        assert_eq!(props.2[0].0, "名称");
        assert_eq!(props.2[0].2, PropertyKind::ReadOnly);
        assert_eq!(
            props.2[1].2,
            PropertyKind::Number {
                min: 0.0,
                max: 100.0,
                step: 2.0,
                decimals: 1
            }
        );
        assert_eq!(
            props.2[2].2,
            PropertyKind::Choice(vec!["无损".to_string(), "高品质".to_string(), "标准".to_string()])
        );
        assert_eq!(
            props.2[3].2,
            PropertyKind::Check {
                on: "是".to_string(),
                off: "否".to_string()
            }
        );
        assert!(props.2[3].3);
        assert_eq!(props.2[4].2, PropertyKind::Rating { stars: 5 });
        assert_eq!(props.2[5].2, PropertyKind::Text);

        let pager = read::<Pager<&'static str>, _>(&ui, "pager", |pager| {
            (
                pager.pages(),
                pager.page(),
                pager.page_labels().to_vec(),
                pager.label_text().to_string(),
            )
        });
        assert_eq!(pager.0, 4);
        assert_eq!(pager.1, 2);
        assert_eq!(pager.2, ["封面", "曲目", "详情", "设置"]);
        assert_eq!(pager.3, "详情");
    }

    // M7.4 step 3: the label-owning lists. Tabs and Segmented share the
    // `<option label>` vocabulary, Breadcrumb/NavRail use `<step>`, RichText
    // `<run>` and the sparkline `<point value>`.
    const LABEL_CHILD_LIST_DOC: &str = r##"<layouts version="1"><layout id="base">
  <widget id="tabs" x="0" y="0" w="400" h="32" selected="1">
    <option label="概览"/>
    <option label="详情"/>
    <option label="设置"/>
  </widget>
  <widget id="seg" x="0" y="0" w="300" h="32" selected="2">
    <option label="低"/>
    <option label="中"/>
    <option label="高"/>
  </widget>
  <widget id="crumb" x="0" y="0" w="300" h="24">
    <step label="音乐"/>
    <step label="专辑"/>
    <step label="夜曲"/>
  </widget>
  <widget id="rail" x="0" y="0" w="180" h="140" selected="1">
    <step label="首页" icon="⌂"/>
    <step label="曲库"/>
    <step label="设置"/>
  </widget>
  <widget id="rich" x="0" y="0" w="300" h="24">
    <run text="重点："/>
    <run text="夜曲" style="strong"/>
    <run text="查看" style="link" link="7"/>
  </widget>
  <widget id="spark" x="0" y="0" w="160" h="36">
    <point value="1"/>
    <point value="3.5"/>
    <point value="-2"/>
  </widget>
</layout></layouts>"##;

    #[test]
    fn label_child_lists_replace_the_models() {
        let mut ui = child_list_ui();
        let mut host = TestHost::default();
        let document = LayoutDocument::parse(LABEL_CHILD_LIST_DOC).expect("valid");
        let report = ui
            .apply_layout(&document, &palette(), Size::new(800.0, 600.0), &mut host)
            .expect("applies");
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        let tabs = read::<Tabs<&'static str>, _>(&ui, "tabs", |tabs| {
            (tabs.labels().to_vec(), tabs.selected())
        });
        assert_eq!(
            tabs.0,
            ["概览".to_string(), "详情".to_string(), "设置".to_string()]
        );
        assert_eq!(tabs.1, 1);

        let seg = read::<Segmented<&'static str>, _>(&ui, "seg", |seg| {
            (seg.options().to_vec(), seg.selected())
        });
        assert_eq!(
            seg.0,
            ["低".to_string(), "中".to_string(), "高".to_string()]
        );
        assert_eq!(seg.1, 2);

        let crumb = read::<Breadcrumb<&'static str>, _>(&ui, "crumb", |crumb| {
            (crumb.items().to_vec(), crumb.current_index())
        });
        assert_eq!(
            crumb.0,
            ["音乐".to_string(), "专辑".to_string(), "夜曲".to_string()]
        );
        assert_eq!(crumb.1, Some(2));

        let rail = read::<NavRail<&'static str>, _>(&ui, "rail", |rail| {
            (
                rail.items()
                    .iter()
                    .map(|item| (item.label.clone(), item.icon))
                    .collect::<Vec<_>>(),
                rail.selected(),
            )
        });
        assert_eq!(
            rail.0,
            [
                ("首页".to_string(), Some('⌂')),
                ("曲库".to_string(), None),
                ("设置".to_string(), None),
            ]
        );
        assert_eq!(rail.1, 1);

        let rich = read::<RichText<&'static str>, _>(&ui, "rich", |rich| {
            (
                rich.runs_ref()
                    .iter()
                    .map(|run| (run.text.clone(), run.style, run.link))
                    .collect::<Vec<_>>(),
                rich.plain_text(),
            )
        });
        assert_eq!(
            rich.0,
            [
                ("重点：".to_string(), RunStyle::Normal, 0),
                ("夜曲".to_string(), RunStyle::Strong, 0),
                ("查看".to_string(), RunStyle::Link, 7),
            ]
        );
        assert_eq!(rich.1, "重点：夜曲查看");

        let spark = read::<Sparkline, _>(&ui, "spark", |spark| spark.data().to_vec());
        assert_eq!(spark, [1.0, 3.5, -2.0]);
    }

    // M7.5 appearance/interaction locals (D144): every declared name goes
    // through the real applier and reaches a typed getter. No callbacks.
    const APPEARANCE_LOCAL_DOC: &str = r##"<layouts version="1"><layout id="base">
  <widget id="a_table" x="0" y="0" w="400" h="200" row-height="30" header-height="20"
          cell-padding="4" striped="true" grid-lines="false" editable="true" empty-text="无数据"/>
  <widget id="a_tree" x="0" y="0" w="300" h="200" row-height="24" indent="12"
          renameable="true" draggable="false" empty-text="空"/>
  <widget id="a_props" x="0" y="0" w="300" h="200" row-height="22" header-height="18"
          key-width="120" grid-lines="false" empty-text="空"/>
  <widget id="a_list" x="0" y="0" w="300" h="200" row-height="28" alternating="true" empty-text="空"/>
  <widget id="a_playlist" x="0" y="0" w="300" h="200" row-height="30" alternating="true" empty-text="空"/>
  <widget id="a_wall" x="0" y="0" w="300" h="200" gap="6" corner-radius="8"
          caption-height="12" empty-text="空"/>
  <widget id="a_rail" x="0" y="0" w="180" h="140" row-height="26"/>
  <widget id="a_tabs" x="0" y="0" w="400" h="32" label-align="right">
    <option label="概览"/>
  </widget>
  <widget id="a_rich" x="0" y="0" w="300" h="24" wrap="false" align="center" text-style="small">
    <run text="文本"/>
  </widget>
  <widget id="a_spark" x="0" y="0" w="160" h="36" color="#336699" gap="4">
    <point value="1"/>
  </widget>
  <widget id="a_crumb" x="0" y="0" w="300" h="24" separator=">">
    <step label="音乐"/>
  </widget>
</layout></layouts>"##;

    fn appearance_ui() -> Ui<&'static str> {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Table::new("a_table"));
        ui.add(TreeView::new("a_tree"));
        ui.add(PropertyGrid::new("a_props"));
        ui.add(ListView::new("a_list"));
        ui.add(PlaylistView::new("a_playlist"));
        ui.add(CoverWall::new("a_wall"));
        ui.add(NavRail::new("a_rail", &["一"], 0, |_| "rail"));
        ui.add(Tabs::new("a_tabs", &["一"], 0, |_| "tab"));
        ui.add(RichText::new("a_rich"));
        ui.add(Sparkline::new("a_spark", &[0.0]));
        ui.add(Breadcrumb::new("a_crumb", &["根"], |_| "crumb"));
        ui
    }

    #[test]
    fn appearance_locals_reach_the_widgets() {
        let mut ui = appearance_ui();
        let mut host = TestHost::default();
        let document = LayoutDocument::parse(APPEARANCE_LOCAL_DOC).expect("valid");
        let report = ui
            .apply_layout(&document, &palette(), Size::new(800.0, 600.0), &mut host)
            .expect("applies");
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        let table = read::<Table<&'static str>, _>(&ui, "a_table", |table| {
            (
                table.row_height_value(),
                table.header_height_value(),
                table.cell_padding_value(),
                table.is_striped(),
                table.grid_lines_enabled(),
                table.is_editable(),
                table.empty_text_ref().to_string(),
            )
        });
        assert_eq!(table, (30.0, 20.0, 4.0, true, false, true, "无数据".to_string()));

        let tree = read::<TreeView<&'static str>, _>(&ui, "a_tree", |tree| {
            (
                tree.row_height_value(),
                tree.indent_value(),
                tree.is_renameable(),
                tree.is_draggable(),
                tree.empty_text_ref().to_string(),
            )
        });
        assert_eq!(tree, (24.0, 12.0, true, false, "空".to_string()));

        let props = read::<PropertyGrid<&'static str>, _>(&ui, "a_props", |grid| {
            (
                grid.row_height_value(),
                grid.header_height_value(),
                grid.key_width_value(),
                grid.grid_lines_enabled(),
                grid.empty_text_ref().to_string(),
            )
        });
        assert_eq!(props, (22.0, 18.0, 120.0, false, "空".to_string()));

        let list = read::<ListView<&'static str>, _>(&ui, "a_list", |list| {
            (
                list.row_height_value(),
                list.is_alternating(),
                list.empty_text_ref().to_string(),
            )
        });
        assert_eq!(list, (28.0, true, "空".to_string()));

        let playlist = read::<PlaylistView<&'static str>, _>(&ui, "a_playlist", |list| {
            (
                list.row_height_value(),
                list.is_alternating(),
                list.empty_text_ref().to_string(),
            )
        });
        assert_eq!(playlist, (30.0, true, "空".to_string()));

        let wall = read::<CoverWall<&'static str>, _>(&ui, "a_wall", |wall| {
            (
                wall.gap_value(),
                wall.corner_radius_value(),
                wall.caption_height_value(),
                wall.empty_text_value().to_string(),
            )
        });
        assert_eq!(wall, (6.0, 8.0, 12.0, "空".to_string()));

        let rail = read::<NavRail<&'static str>, _>(&ui, "a_rail", |rail| {
            rail.row_height_value()
        });
        assert_eq!(rail, 26.0);

        let tabs = read::<Tabs<&'static str>, _>(&ui, "a_tabs", |tabs| {
            tabs.label_alignment()
        });
        assert_eq!(tabs, crate::backend::TextAlign::Right);

        let rich = read::<RichText<&'static str>, _>(&ui, "a_rich", |rich| {
            (rich.wrap_enabled(), rich.alignment(), rich.text_style())
        });
        assert_eq!(
            rich,
            (
                false,
                crate::backend::TextAlign::Center,
                crate::widgets::LabelStyle::Small
            )
        );

        let spark = read::<Sparkline, _>(&ui, "a_spark", |spark| {
            (spark.color_value(), spark.gap_value())
        });
        assert_eq!(spark, (Some(crate::color::Color::new(0x33, 0x66, 0x99)), 4.0));

        let crumb = read::<Breadcrumb<&'static str>, _>(&ui, "a_crumb", |crumb| {
            crumb.separator_char()
        });
        assert_eq!(crumb, '>');
    }

    #[test]
    fn a_malformed_local_attribute_rejects_atomically() {
        let cases = [
            (r#"row-height="4""#, "a_table"),
            (r#"header-height="-1""#, "a_table"),
            (r#"indent="2""#, "a_tree"),
            (r#"key-width="10""#, "a_props"),
            (r#"row-height="10""#, "a_list"),
            (r#"row-height="10""#, "a_playlist"),
            (r#"gap="-1""#, "a_wall"),
            (r#"row-height="4""#, "a_rail"),
            (r#"label-align="middle""#, "a_tabs"),
            (r#"align="justify""#, "a_rich"),
            (r#"text-style="huge""#, "a_rich"),
            (r#"gap="-2""#, "a_spark"),
            (r#"separator="ab""#, "a_crumb"),
        ];
        for (attribute, id) in cases {
            let mut ui = appearance_ui();
            let mut host = TestHost::default();
            let text = format!(
                "<layouts version=\"1\"><layout id=\"base\">\
                 <widget id=\"{id}\" x=\"0\" y=\"0\" w=\"300\" h=\"200\" {attribute}/>\
                 </layout></layouts>"
            );
            let document = LayoutDocument::parse(&text).expect("parses");
            let failure = ui
                .apply_layout(&document, &palette(), Size::new(800.0, 600.0), &mut host)
                .expect_err("rejects");
            assert_eq!(failure.first().map(|item| item.code), Some("xml.skin.value"), "{attribute}");
            assert!(ui.installed_layout().is_none(), "{attribute}");
        }
    }

    #[test]
    fn a_malformed_child_list_rejects_atomically() {
        let cases = [
            (r#"<item subtitle="无标题"/>"#, "list"),
            (r#"<item title="夜曲" colour="red"/>"#, "list"),
            (r#"<item title="夜曲" current="maybe"/>"#, "list"),
            (r#"<option/>"#, "combo"),
            (r#"<cover/>"#, "wall"),
            (r#"<segment text=""/>"#, "status"),
            (r#"<column title=""/>"#, "table"),
            (r#"<row><cell/></row>"#, "table"),
            (r#"<column title="x" align="middle"/>"#, "table"),
            (r#"<node label="x" glyph="ab"/>"#, "tree"),
            (r#"<section><row key=""/></section>"#, "props"),
            (r#"<section><row key="x" kind="number" step="0"/></section>"#, "props"),
            (r#"<section><row key="x" kind="choice"/></section>"#, "props"),
            (r#"<section><row key="x" kind="text" min="0"/></section>"#, "props"),
            (r#"<page/>"#, "pager"),
            (r#"<option label=""/>"#, "tabs"),
            (r#"<option title="x"/>"#, "seg"),
            (r#"<step label="x" icon="⌂"/>"#, "crumb"),
            (r#"<step label="x" icon="ab"/>"#, "rail"),
            (r#"<run text="x" style="bold"/>"#, "rich"),
            (r#"<run text="x" link="1"/>"#, "rich"),
            (r#"<point/>"#, "spark"),
            (r#"<point value="x"/>"#, "spark"),
        ];
        for (child, id) in cases {
            let (open, close) = match id {
                "list" => ("<widget id=\"list\" x=\"0\" y=\"0\" w=\"300\" h=\"200\">", "</widget>"),
                "combo" => ("<widget id=\"combo\" x=\"0\" y=\"0\" w=\"200\" h=\"32\">", "</widget>"),
                "wall" => ("<widget id=\"wall\" x=\"0\" y=\"0\" w=\"400\" h=\"300\">", "</widget>"),
                "table" => ("<widget id=\"table\" x=\"0\" y=\"0\" w=\"400\" h=\"200\">", "</widget>"),
                "tree" => ("<widget id=\"tree\" x=\"0\" y=\"0\" w=\"300\" h=\"200\">", "</widget>"),
                "props" => ("<widget id=\"props\" x=\"0\" y=\"0\" w=\"300\" h=\"200\">", "</widget>"),
                "pager" => ("<widget id=\"pager\" x=\"0\" y=\"0\" w=\"300\" h=\"32\">", "</widget>"),
                "tabs" => ("<widget id=\"tabs\" x=\"0\" y=\"0\" w=\"400\" h=\"32\">", "</widget>"),
                "seg" => ("<widget id=\"seg\" x=\"0\" y=\"0\" w=\"300\" h=\"32\">", "</widget>"),
                "crumb" => ("<widget id=\"crumb\" x=\"0\" y=\"0\" w=\"300\" h=\"24\">", "</widget>"),
                "rail" => ("<widget id=\"rail\" x=\"0\" y=\"0\" w=\"180\" h=\"140\">", "</widget>"),
                "rich" => ("<widget id=\"rich\" x=\"0\" y=\"0\" w=\"300\" h=\"24\">", "</widget>"),
                "spark" => ("<widget id=\"spark\" x=\"0\" y=\"0\" w=\"160\" h=\"36\">", "</widget>"),
                _ => ("<widget id=\"status\" x=\"0\" y=\"0\" w=\"400\" h=\"24\">", "</widget>"),
            };
            let mut ui = child_list_ui();
            let mut host = TestHost::default();
            let text = format!(
                "<layouts version=\"1\"><layout id=\"base\">{open}{child}{close}</layout></layouts>"
            );
            let document = LayoutDocument::parse(&text).expect("parses");
            let failure = ui
                .apply_layout(&document, &palette(), Size::new(800.0, 600.0), &mut host)
                .expect_err("rejects");
            assert_eq!(failure.first().map(|item| item.code), Some("xml.skin.value"), "{child}");
            assert!(ui.installed_layout().is_none(), "{child}");
            // The data model was never replaced.
            if id == "list" {
                assert!(read::<ListView<&'static str>, _>(&ui, "list", |list| list.rows().len()) == 0);
            }
            if id == "tree" {
                assert!(
                    read::<TreeView<&'static str>, _>(&ui, "tree", |tree| tree.items_ref().len())
                        == 0
                );
            }
        }
    }
}
