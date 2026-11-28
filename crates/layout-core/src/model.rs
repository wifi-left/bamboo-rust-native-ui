// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The parsed layout model (M7 §1–§6).
//!
//! A [`LayoutDocument`] is immutable after parsing: variants, entries,
//! constants and the content initial values. Geometry is resolved on demand
//! against a [`LayoutSpace`], because the same entry is evaluated against the
//! client area or a scroll panel's content area depending on who owns the
//! widget.

use crate::color::Color;
use crate::event::Cursor;
use crate::a11y::A11yRole;
use crate::geometry::{Corners, Rect, Size};
use crate::diag::LayoutDiagnostic;
use crate::expr::{self, Axis, Expr};
use crate::widget::WidgetId;

/// Document-scope numeric constants, resolved in declaration order.
#[derive(Clone, Debug, Default)]
pub struct LayoutConstants {
    values: Vec<Constant>,
}

#[derive(Clone, Debug)]
struct Constant {
    name: String,
    value: f64,
    /// Canonical expression text, so the writer never inlines a constant.
    source: String,
}

impl LayoutConstants {
    /// The value of a declared constant.
    pub fn get(&self, name: &str) -> Option<f64> {
        self.values
            .iter()
            .find(|candidate| candidate.name == name)
            .map(|candidate| candidate.value)
    }

    /// Declared constants in document order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, f64)> + '_ {
        self.values
            .iter()
            .map(|constant| (constant.name.as_str(), constant.value))
    }

    /// Source expression of every constant, in document order.
    pub fn sources(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.values
            .iter()
            .map(|constant| (constant.name.as_str(), constant.source.as_str()))
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Insert one definition; `false` when the name is already taken.
    pub fn insert(&mut self, name: String, value: f64, source: String) -> bool {
        if self.get(&name).is_some() {
            return false;
        }
        self.values.push(Constant {
            name,
            value,
            source,
        });
        true
    }
}

/// Which coordinate system an entry is resolved in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntrySpace {
    /// The window client area.
    Client,
    /// The content area of a scroll panel.
    Content(WidgetId),
}

/// Resolver input: the extent of the space plus the document's constants.
#[derive(Clone, Copy, Debug)]
pub struct LayoutSpace<'a> {
    width: f32,
    height: f32,
    constants: &'a LayoutConstants,
}

impl<'a> LayoutSpace<'a> {
    /// The client area of the window.
    pub fn client(size: Size, constants: &'a LayoutConstants) -> Self {
        Self {
            width: size.width,
            height: size.height,
            constants,
        }
    }

    /// A scroll panel's content area.
    pub fn content(width: f32, height: f32, constants: &'a LayoutConstants) -> Self {
        Self {
            width,
            height,
            constants,
        }
    }

    pub fn size(self) -> Size {
        Size::new(self.width, self.height)
    }

    pub fn width(self) -> f32 {
        self.width
    }

    pub fn height(self) -> f32 {
        self.height
    }

    pub fn constants(self) -> &'a LayoutConstants {
        self.constants
    }
}

/// A parsed `.bskin` file.
#[derive(Clone, Debug)]
pub struct LayoutDocument {
    pub generation: u64,
    pub canonical: String,
    pub constants: LayoutConstants,
    pub variants: Vec<LayoutVariant>,
}

impl LayoutDocument {
    /// Content is written by the applier only when this changes.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn variants(&self) -> &[LayoutVariant] {
        &self.variants
    }

    pub fn constants(&self) -> &LayoutConstants {
        &self.constants
    }

    /// The variant that applies at `client` (plural client DIPs): among the
    /// variants whose minimums both fit, the greatest `(min-width,
    /// min-height)`; the base variant otherwise.
    pub fn select(&self, client: Size) -> &LayoutVariant {
        let mut best: Option<&LayoutVariant> = None;
        for variant in &self.variants {
            if variant.min_width > client.width || variant.min_height > client.height {
                continue;
            }
            best = match best {
                None => Some(variant),
                Some(current) => {
                    if (variant.min_width, variant.min_height)
                        > (current.min_width, current.min_height)
                    {
                        Some(variant)
                    } else {
                        Some(current)
                    }
                }
            };
        }
        best.or_else(|| {
            self.variants
                .iter()
                .find(|variant| variant.min_width == 0.0 && variant.min_height == 0.0)
        })
        .unwrap_or_else(|| &self.variants[0])
    }
}

/// One `<layout>` breakpoint variant.
#[derive(Clone, Debug)]
pub struct LayoutVariant {
    pub id: String,
    pub min_width: f32,
    pub min_height: f32,
    pub entries: Vec<LayoutEntry>,
    pub scrolls: Vec<ScrollEntry>,
    /// `<rust target=...>` child-list bindings, in source order (§16.2).
    pub rust_bindings: Vec<RustBinding>,
}

impl LayoutVariant {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn minimums(&self) -> (f32, f32) {
        (self.min_width, self.min_height)
    }

    pub fn entries(&self) -> &[LayoutEntry] {
        &self.entries
    }

    pub fn scrolls(&self) -> &[ScrollEntry] {
        &self.scrolls
    }

    pub fn entry(&self, id: WidgetId) -> Option<&LayoutEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Inline-Rust child-list bindings, in source order.
    pub fn rust_bindings(&self) -> &[RustBinding] {
        &self.rust_bindings
    }
}

/// A `<rust target="id" code="expr"/>` data binding (§16.2): inline Rust that
/// produces one widget's declaration child list.
///
/// A binding is content — written once per generation, like a static child
/// list — and only the compiled mode evaluates it: the dynamic applier refuses
/// the document with `xml.rust.unsupported`. The expression must produce
/// `Vec<SkinElement>`; the list goes through the same widget-declared
/// validation (`validate_skin_children`) as `<item>`/`<row>` elements.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustBinding {
    /// The target widget id as written in the file.
    pub target: String,
    /// The Rust expression, verbatim.
    pub code: String,
    /// 1-based position of the opening tag.
    pub line: usize,
    pub column: usize,
    /// `layout/{variant}/{target}`, for diagnostics.
    pub path: String,
}

impl RustBinding {
    pub fn id(&self) -> &str {
        &self.target
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

/// One `<widget>` entry.
#[derive(Clone, Debug)]
pub struct LayoutEntry {
    pub id: WidgetId,
    pub line: usize,
    pub column: usize,
    pub path: String,
    /// `type="…"` (§17): the registry type that creates this widget when the id
    /// does not exist yet. `None` is the address-only entry every earlier round
    /// used.
    pub type_name: Option<String>,
    /// `command="…"`: the application command id a created interactive widget
    /// reports (resolved through the registry's `CommandMap`). Only meaningful
    /// with `type_name`.
    pub command: Option<u32>,
    /// `parent="panel-id"`: the scroll panel this entry belongs to (M7.7
    /// follow-up). On a created entry it is the membership the applier installs;
    /// on an addressed entry it asserts the application's own membership
    /// (`xml.parent.missing` when it does not match).
    pub parent: Option<WidgetId>,
    pub geometry: Geometry,
    pub props: PropAttrs,
    pub style: StyleAttrs,
    pub content: Vec<ContentAttr>,
    /// Raw local attributes (widget-declared skin protocol, §6.2), in source
    /// order. Values are never expressions.
    pub skin_attrs: Vec<(String, String)>,
    /// Data child elements (`<item/>`, `<track/>`, `<x-item/>`), in source
    /// order (§6.3).
    pub skin_children: Vec<crate::skin::SkinElement>,
    /// Scalar attributes whose value is inline Rust (`rust:` prefix), in source
    /// order. The dynamic applier refuses them (`xml.rust.unsupported`); the
    /// compiled generator splices the text into the generated module.
    pub rust_attrs: Vec<(&'static str, String)>,
}

impl LayoutEntry {
    pub fn id(&self) -> WidgetId {
        self.id
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    /// The registry type that creates this widget, when the file declares one.
    pub fn type_name(&self) -> Option<&str> {
        self.type_name.as_deref()
    }

    /// The application command id of a created widget.
    pub fn command(&self) -> Option<u32> {
        self.command
    }

    /// The scroll panel this entry belongs to (`parent="…"`), when declared.
    /// The id is a scroll panel's, not a widget's.
    pub fn parent(&self) -> Option<WidgetId> {
        self.parent
    }

    /// Raw local attributes, in source order (the writer sorts them).
    pub fn skin_attrs(&self) -> &[(String, String)] {
        &self.skin_attrs
    }

    /// Data child elements, in source order.
    pub fn skin_children(&self) -> &[crate::skin::SkinElement] {
        &self.skin_children
    }

    /// Whether this entry declares anything the widget-declared protocol owns.
    pub fn has_skin(&self) -> bool {
        !self.skin_attrs.is_empty() || !self.skin_children.is_empty()
    }

    /// Whether any scalar attribute is inline Rust.
    pub fn has_rust(&self) -> bool {
        !self.rust_attrs.is_empty()
    }

    /// The inline-Rust value of one attribute name.
    pub fn rust_attr(&self, name: &str) -> Option<&str> {
        self.rust_attrs
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, code)| code.as_str())
    }

    /// Resolve the rectangle in `space`. Errors carry this entry's position.
    pub fn resolve(&self, space: LayoutSpace<'_>) -> Result<Rect, LayoutDiagnostic> {
        resolve_geometry(&self.geometry, space, self.line, self.column, &self.path)
    }
}

/// One `<scroll>` entry: a registered panel's viewport and content height.
#[derive(Clone, Debug)]
pub struct ScrollEntry {
    pub id: WidgetId,
    pub line: usize,
    pub column: usize,
    pub path: String,
    pub geometry: Geometry,
    pub content_height: ContentHeight,
}

impl ScrollEntry {
    pub fn id(&self) -> WidgetId {
        self.id
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn resolve(&self, space: LayoutSpace<'_>) -> Result<Rect, LayoutDiagnostic> {
        resolve_geometry(&self.geometry, space, self.line, self.column, &self.path)
    }

    pub fn content_height(&self) -> &ContentHeight {
        &self.content_height
    }
}

/// What `<scroll content-height>` asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum ContentHeight {
    /// Leave the panel's content extent alone.
    Keep,
    /// Set it exactly (still floored by the children, per `ScrollView`).
    Exact(Expr),
    /// Grow to fit viewport, children and the previous extent — never shrink.
    Auto,
}

/// Horizontal rectangle form (§3.4).
#[derive(Clone, Debug)]
pub enum Horizontal {
    LeftWidth { x: Expr, w: Expr },
    RightWidth { right: Expr, w: Expr },
    LeftRight { x: Expr, right: Expr },
}

/// Vertical rectangle form (§3.4).
#[derive(Clone, Debug)]
pub enum Vertical {
    TopHeight { y: Expr, h: Expr },
    BottomHeight { bottom: Expr, h: Expr },
    TopBottom { y: Expr, bottom: Expr },
}

/// A complete rectangle specification.
#[derive(Clone, Debug)]
pub struct Geometry {
    pub horizontal: Horizontal,
    pub vertical: Vertical,
}

fn resolve_geometry(
    geometry: &Geometry,
    space: LayoutSpace<'_>,
    line: usize,
    column: usize,
    path: &str,
) -> Result<Rect, LayoutDiagnostic> {
    let width = space.width as f64;
    let height = space.height as f64;
    let constants = space.constants();
    let attribute = |expr: &Expr, axis: Axis, name: &str| -> Result<f64, LayoutDiagnostic> {
        expr::eval(expr, width, height, axis, constants).map_err(|error| {
            LayoutDiagnostic::error(
                error.code,
                line,
                column,
                path.to_string(),
                format!("{name}: {}", error.message),
            )
        })
    };
    let negative = |name: &str| {
        LayoutDiagnostic::error(
            "xml.geom.negative",
            line,
            column,
            path.to_string(),
            format!("`{name}` must not be negative"),
        )
    };

    let (left, right, _rect_width) = match &geometry.horizontal {
        Horizontal::LeftWidth { x, w } => {
            let x_value = attribute(x, Axis::Horizontal, "x")?;
            let w_value = attribute(w, Axis::Horizontal, "w")?;
            if w_value < 0.0 {
                return Err(negative("w"));
            }
            (x_value, x_value + w_value, w_value)
        }
        Horizontal::RightWidth { right, w } => {
            let right_value = attribute(right, Axis::Horizontal, "right")?;
            let w_value = attribute(w, Axis::Horizontal, "w")?;
            if right_value < 0.0 {
                return Err(negative("right"));
            }
            if w_value < 0.0 {
                return Err(negative("w"));
            }
            (
                width - right_value - w_value,
                width - right_value,
                w_value,
            )
        }
        Horizontal::LeftRight { x, right } => {
            let x_value = attribute(x, Axis::Horizontal, "x")?;
            let right_value = attribute(right, Axis::Horizontal, "right")?;
            if right_value < 0.0 {
                return Err(negative("right"));
            }
            let w_value = width - x_value - right_value;
            if w_value < 0.0 {
                return Err(LayoutDiagnostic::error(
                    "xml.geom.negative",
                    line,
                    column,
                    path.to_string(),
                    "the stretched width is negative".to_string(),
                ));
            }
            (x_value, width - right_value, w_value)
        }
    };

    let (top, bottom, _rect_height) = match &geometry.vertical {
        Vertical::TopHeight { y, h } => {
            let y_value = attribute(y, Axis::Vertical, "y")?;
            let h_value = attribute(h, Axis::Vertical, "h")?;
            if h_value < 0.0 {
                return Err(negative("h"));
            }
            (y_value, y_value + h_value, h_value)
        }
        Vertical::BottomHeight { bottom, h } => {
            let bottom_value = attribute(bottom, Axis::Vertical, "bottom")?;
            let h_value = attribute(h, Axis::Vertical, "h")?;
            if bottom_value < 0.0 {
                return Err(negative("bottom"));
            }
            if h_value < 0.0 {
                return Err(negative("h"));
            }
            (
                height - bottom_value - h_value,
                height - bottom_value,
                h_value,
            )
        }
        Vertical::TopBottom { y, bottom } => {
            let y_value = attribute(y, Axis::Vertical, "y")?;
            let bottom_value = attribute(bottom, Axis::Vertical, "bottom")?;
            if bottom_value < 0.0 {
                return Err(negative("bottom"));
            }
            let h_value = height - y_value - bottom_value;
            if h_value < 0.0 {
                return Err(LayoutDiagnostic::error(
                    "xml.geom.negative",
                    line,
                    column,
                    path.to_string(),
                    "the stretched height is negative".to_string(),
                ));
            }
            (y_value, height - bottom_value, h_value)
        }
    };

    Ok(Rect::new(
        left as f32,
        top as f32,
        right as f32,
        bottom as f32,
    ))
}

/// `WidgetProps` fields one entry declares (§4).
#[derive(Clone, Debug, Default)]
pub struct PropAttrs {
    pub visible: Option<bool>,
    pub enabled: Option<bool>,
    pub interactive: Option<bool>,
    pub focusable: Option<bool>,
    pub focus_order: Option<i32>,
    pub read_only: Option<bool>,
    pub tooltip: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub automation_id: Option<String>,
    pub role: Option<A11yRole>,
    pub expose: Option<bool>,
    pub cursor: Option<Cursor>,
}

impl PropAttrs {
    /// Any `WidgetProps` field (as opposed to `Ui` state like visibility).
    pub fn has_props(&self) -> bool {
        self.interactive.is_some()
            || self.focusable.is_some()
            || self.focus_order.is_some()
            || self.read_only.is_some()
            || self.tooltip.is_some()
            || self.name.is_some()
            || self.description.is_some()
            || self.automation_id.is_some()
            || self.role.is_some()
            || self.expose.is_some()
            || self.cursor.is_some()
    }
}

/// Surface and font tokens one entry declares (§5).
#[derive(Clone, Debug, Default)]
pub struct StyleAttrs {
    pub radius: Option<RadiusAttr>,
    pub corners: Option<Corners>,
    pub background: Option<BackgroundAttr>,
    pub font_size: Option<f32>,
    pub font_family: Option<String>,
}

impl StyleAttrs {
    pub fn has_surface(&self) -> bool {
        self.radius.is_some() || self.corners.is_some() || self.background.is_some()
    }

    pub fn has_font(&self) -> bool {
        self.font_size.is_some() || self.font_family.is_some()
    }
}

/// `radius` value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RadiusAttr {
    Dips(f32),
    Small,
    Medium,
    Large,
    Pill,
}

impl RadiusAttr {
    pub fn resolve(self, height: f32) -> f32 {
        match self {
            Self::Dips(value) => value,
            Self::Small => crate::radius::SMALL,
            Self::Medium => crate::radius::MEDIUM,
            Self::Large => crate::radius::LARGE,
            Self::Pill => crate::radius::pill(height),
        }
    }
}

/// `background` value: a palette token, a literal colour, or no fill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BackgroundAttr {
    Token(&'static str),
    Literal(Color),
    None,
}

/// One content initial value (§6).
#[derive(Clone, Debug, PartialEq)]
pub enum ContentAttr {
    Text(String),
    Label(String),
    Value(f64),
    Progress(f64),
    Number(i64),
    Checked(bool),
    Selected(u32),
}

impl ContentAttr {
    /// Attribute order used by the writer and by the variant equality check.
    pub fn rank(&self) -> u8 {
        match self {
            Self::Text(_) => 0,
            Self::Label(_) => 1,
            Self::Value(_) => 2,
            Self::Progress(_) => 3,
            Self::Number(_) => 4,
            Self::Checked(_) => 5,
            Self::Selected(_) => 6,
        }
    }

    pub fn attribute_name(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Label(_) => "label",
            Self::Value(_) => "value",
            Self::Progress(_) => "progress",
            Self::Number(_) => "number",
            Self::Checked(_) => "checked",
            Self::Selected(_) => "selected",
        }
    }
}

/// The schema scalar attributes that accept inline Rust (`rust:` prefix).
///
/// Geometry (`x`/`right`/`w`/`y`/`bottom`/`h`) goes through [`crate::expr::Expr::Rust`]
/// instead; `id` and widget-local `x-*` names never accept it.
pub fn scalar_attr_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "visible" => "visible",
        "enabled" => "enabled",
        "interactive" => "interactive",
        "focusable" => "focusable",
        "focus-order" => "focus-order",
        "read-only" => "read-only",
        "tooltip" => "tooltip",
        "name" => "name",
        "description" => "description",
        "automation-id" => "automation-id",
        "role" => "role",
        "expose" => "expose",
        "cursor" => "cursor",
        "radius" => "radius",
        "corners" => "corners",
        "background" => "background",
        "font-size" => "font-size",
        "font-family" => "font-family",
        "text" => "text",
        "label" => "label",
        "value" => "value",
        "progress" => "progress",
        "number" => "number",
        "checked" => "checked",
        "selected" => "selected",
        _ => return None,
    })
}

/// Whether an attribute name is one of the shared content values.
pub fn is_content_attr_name(name: &str) -> bool {
    matches!(
        name,
        "text" | "label" | "value" | "progress" | "number" | "checked" | "selected"
    )
}

/// The canonical content order (the writer and the conflict check both use it).
pub fn content_rank(name: &str) -> u8 {
    match name {
        "text" => 0,
        "label" => 1,
        "value" => 2,
        "progress" => 3,
        "number" => 4,
        "checked" => 5,
        "selected" => 6,
        _ => 7,
    }
}

/// Whether a name is one of the palette tokens.
pub fn palette_token_for_name(name: &str) -> Option<&'static str> {
    PALETTE_TOKENS.iter().copied().find(|token| *token == name)
}

/// All palette token names, for diagnostics.
pub const PALETTE_TOKENS: &[&str] = &[
    "background",
    "sidebar",
    "player",
    "cover",
    "nav-active",
    "accent",
    "text",
    "muted",
    "divider",
    "row-selected",
    "scroll-track",
    "scroll-thumb",
    "accent-text",
    "search-bg",
    "button-bg",
    "button-hover",
    "button-pressed",
    "button-active",
    "button-active-hover",
    "explicit-border",
    "nav-hover",
    "row-hover",
    "nav-pressed",
    "row-pressed",
    "scroll-thumb-hover",
    "card",
    "panel",
    "button-disabled",
    "text-disabled",
    "play-button",
    "play-hover",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn space(width: f32, height: f32) -> LayoutConstants {
        let _ = (width, height);
        LayoutConstants::default()
    }

    fn entry(horizontal: Horizontal, vertical: Vertical) -> Geometry {
        Geometry {
            horizontal,
            vertical,
        }
    }

    fn num(value: f64) -> Expr {
        Expr::Num(value)
    }

    fn var(var: crate::expr::Var) -> Expr {
        Expr::Var(var)
    }

    #[test]
    fn all_legal_forms_resolve() {
        let constants = space(400.0, 300.0);
        let client = LayoutSpace::client(Size::new(400.0, 300.0), &constants);
        let left_width = entry(
            Horizontal::LeftWidth {
                x: num(16.0),
                w: num(100.0),
            },
            Vertical::TopHeight {
                y: num(8.0),
                h: num(20.0),
            },
        );
        assert_eq!(
            resolve_geometry(&left_width, client, 1, 1, "x").unwrap(),
            Rect::from_xywh(16.0, 8.0, 100.0, 20.0)
        );

        let right_width = entry(
            Horizontal::RightWidth {
                right: num(16.0),
                w: num(100.0),
            },
            Vertical::BottomHeight {
                bottom: num(10.0),
                h: num(20.0),
            },
        );
        assert_eq!(
            resolve_geometry(&right_width, client, 1, 1, "x").unwrap(),
            Rect::from_xywh(284.0, 270.0, 100.0, 20.0)
        );

        let stretched = entry(
            Horizontal::LeftRight {
                x: num(16.0),
                right: num(16.0),
            },
            Vertical::TopBottom {
                y: num(8.0),
                bottom: num(8.0),
            },
        );
        assert_eq!(
            resolve_geometry(&stretched, client, 1, 1, "x").unwrap(),
            Rect::from_xywh(16.0, 8.0, 368.0, 284.0)
        );
    }

    #[test]
    fn escapes_use_the_axis_extent() {
        let constants = space(400.0, 300.0);
        let content = LayoutSpace::content(400.0, 300.0, &constants);
        let centred = entry(
            Horizontal::LeftWidth {
                x: Expr::Bin(
                    crate::expr::BinOp::Sub,
                    Box::new(var(crate::expr::Var::Cx)),
                    Box::new(num(22.0)),
                ),
                w: num(44.0),
            },
            Vertical::TopHeight {
                y: Expr::Percent(25.0),
                h: num(10.0),
            },
        );
        assert_eq!(
            resolve_geometry(&centred, content, 1, 1, "x").unwrap(),
            Rect::from_xywh(178.0, 75.0, 44.0, 10.0)
        );
    }

    #[test]
    fn negative_and_stretched_errors() {
        let constants = LayoutConstants::default();
        let client = LayoutSpace::client(Size::new(100.0, 100.0), &constants);
        let negative = entry(
            Horizontal::LeftWidth {
                x: num(0.0),
                w: num(-1.0),
            },
            Vertical::TopHeight {
                y: num(0.0),
                h: num(10.0),
            },
        );
        assert_eq!(
            resolve_geometry(&negative, client, 1, 1, "x").unwrap_err().code,
            "xml.geom.negative"
        );
        let stretched = entry(
            Horizontal::LeftRight {
                x: num(80.0),
                right: num(80.0),
            },
            Vertical::TopHeight {
                y: num(0.0),
                h: num(10.0),
            },
        );
        assert_eq!(
            resolve_geometry(&stretched, client, 1, 1, "x").unwrap_err().code,
            "xml.geom.negative"
        );
        let negative_right = entry(
            Horizontal::RightWidth {
                right: num(-1.0),
                w: num(10.0),
            },
            Vertical::TopHeight {
                y: num(0.0),
                h: num(10.0),
            },
        );
        assert_eq!(
            resolve_geometry(&negative_right, client, 1, 1, "x").unwrap_err().code,
            "xml.geom.negative"
        );
    }

    #[test]
    fn selection_prefers_the_greatest_minimums() {
        let doc = LayoutDocument {
            generation: 0,
            canonical: String::new(),
            constants: LayoutConstants::default(),
            variants: vec![
                LayoutVariant {
                    id: "base".into(),
                    min_width: 0.0,
                    min_height: 0.0,
                    entries: Vec::new(),
                    scrolls: Vec::new(),
                    rust_bindings: Vec::new(),
                },
                LayoutVariant {
                    id: "wide".into(),
                    min_width: 1000.0,
                    min_height: 700.0,
                    entries: Vec::new(),
                    scrolls: Vec::new(),
                    rust_bindings: Vec::new(),
                },
                LayoutVariant {
                    id: "tall".into(),
                    min_width: 800.0,
                    min_height: 900.0,
                    entries: Vec::new(),
                    scrolls: Vec::new(),
                    rust_bindings: Vec::new(),
                },
            ],
        };
        assert_eq!(doc.select(Size::new(759.0, 559.0)).id(), "base");
        assert_eq!(doc.select(Size::new(800.0, 700.0)).id(), "base");
        assert_eq!(doc.select(Size::new(800.0, 900.0)).id(), "tall");
        assert_eq!(doc.select(Size::new(1000.0, 700.0)).id(), "wide");
        assert_eq!(doc.select(Size::new(4000.0, 3000.0)).id(), "wide");
    }
}
