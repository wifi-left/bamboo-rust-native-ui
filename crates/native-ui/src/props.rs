// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Per-instance widget properties and narration overrides.
//!
//! Widgets are built once per page, so presentation and accessibility details
//! that differ between instances of the same widget type live here instead of
//! in the widget structs. [`WidgetProps`] is applied at creation time with
//! [`crate::Ui::add_with`] or at runtime with [`crate::Ui::set_props`].
//!
//! The rules that must not silently change:
//!
//! - Defaults stay untouched: an unset field keeps the widget's own behavior,
//!   so existing pages narrate exactly as before.
//! - Narration feeds the same [`A11yInfo`] the provider tree is built from, so
//!   a screen reader, `--a11y-dump` and `--self-test` observe one value.
//! - Lazy formatters receive the widget's current derived info, so a name can
//!   follow live state (playback, counts) without rebuilding the tree.

use std::rc::Rc;
use std::sync::Arc;

use crate::a11y::{A11yChild, A11yInfo, A11yRole};
use crate::backend::{FontFace, TextStyle};
use crate::event::Cursor;
use crate::menu::Menu;
use crate::theme::Palette;

/// Per-widget font override. Only UI text is affected: icon glyphs keep the
/// MDL2 face so they never turn into tofu. A size is an absolute DIP on top of
/// the theme scale (the backend multiplies by the DPI).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontSpec {
    pub(crate) size: Option<f32>,
    pub(crate) family: Option<String>,
}

impl FontSpec {
    pub fn new() -> Self {
        Self::default()
    }

    /// Absolute text size in DIPs for this widget.
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size.max(1.0));
        self
    }

    /// Explicit family for this widget's UI text.
    pub fn family(mut self, family: impl Into<String>) -> Self {
        let family = family.into();
        if !family.trim().is_empty() {
            self.family = Some(family);
        }
        self
    }

    pub fn is_empty(&self) -> bool {
        self.size.is_none() && self.family.is_none()
    }

    /// Merge onto the painter's role style. Glyph faces are returned untouched.
    pub fn apply(&self, base: &TextStyle) -> TextStyle {
        let mut style = base.clone();
        if matches!(style.face, FontFace::Glyph) {
            return style;
        }
        if let Some(size) = self.size {
            style.size = size;
        }
        if let Some(family) = &self.family {
            style.face = FontFace::Named(Arc::from(family.as_str()));
        }
        style
    }
}

/// Where an accessible string comes from.
#[derive(Clone)]
enum TextSource {
    Fixed(String),
    /// Built from the widget's current derived info on every snapshot.
    Lazy(Rc<dyn Fn(&A11yInfo) -> String>),
}

impl TextSource {
    fn resolve(&self, info: &A11yInfo) -> String {
        match self {
            TextSource::Fixed(text) => text.clone(),
            TextSource::Lazy(format) => format(info),
        }
    }
}

/// Application-owned narration for one widget: the name, the description and
/// the per-option/row formatter. Anything left unset falls back to the widget's
/// own derived text.
#[derive(Clone, Default)]
pub struct Narration {
    name: Option<TextSource>,
    description: Option<TextSource>,
    row: Option<Rc<dyn Fn(usize, &A11yChild) -> String>>,
}

impl Narration {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fixed accessible name (replaces the derived label).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(TextSource::Fixed(name.into()));
        self
    }

    /// Name built from the widget's current info, e.g. a playback counter.
    pub fn name_with(mut self, format: impl Fn(&A11yInfo) -> String + 'static) -> Self {
        self.name = Some(TextSource::Lazy(Rc::new(format)));
        self
    }

    /// Fixed accessible description (UIA HelpText/FullDescription).
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(TextSource::Fixed(description.into()));
        self
    }

    /// Description built from the widget's current info.
    pub fn description_with(
        mut self,
        format: impl Fn(&A11yInfo) -> String + 'static,
    ) -> Self {
        self.description = Some(TextSource::Lazy(Rc::new(format)));
        self
    }

    /// Rename options/tabs/rows published by a composite control. The index is
    /// the widget-level index (a scrolled list row keeps its absolute index).
    pub fn row(mut self, format: impl Fn(usize, &A11yChild) -> String + 'static) -> Self {
        self.row = Some(Rc::new(format));
        self
    }

    /// Resolved name, or `None` when the derived name should be used.
    pub(crate) fn resolve_name(&self, info: &A11yInfo) -> Option<String> {
        self.name.as_ref().map(|source| source.resolve(info))
    }

    /// Resolved description, or `None` when the widget has none.
    pub(crate) fn resolve_description(&self, info: &A11yInfo) -> Option<String> {
        self.description.as_ref().map(|source| source.resolve(info))
    }

    /// Apply the row formatter to every published child; returns true when a
    /// formatter is installed.
    pub(crate) fn apply_rows(&self, children: &mut [A11yChild]) -> bool {
        let Some(format) = self.row.as_ref() else {
            return false;
        };
        for child in children {
            child.name = format(child.index, child);
        }
        true
    }
}

/// Creation-time and runtime properties for one widget instance.
///
/// Only the fields the retained-tree model can honor are here: layout
/// (margins, grow, alignment) stays application-computed per D1, and font
/// overrides live with the painter's cache, not per widget.
#[derive(Clone, Default)]
pub struct WidgetProps {
    pub(crate) visible: Option<bool>,
    pub(crate) enabled: Option<bool>,
    pub(crate) focusable: Option<bool>,
    pub(crate) focus_order: Option<i32>,
    pub(crate) cursor: Option<Cursor>,
    pub(crate) interactive: Option<bool>,
    pub(crate) read_only: Option<bool>,
    pub(crate) tooltip: Option<String>,
    /// Per-item overrides for composite controls (absolute row/option index).
    /// `None` entries fall back to `tooltip`; `Some("")` explicitly suppresses
    /// the tip for that item.
    pub(crate) item_tooltips: Option<Vec<Option<String>>>,
    /// Declarative menu shown on right click, the menu key or Shift+F10.
    pub(crate) context_menu: Option<Menu>,
    pub(crate) automation_id: Option<String>,
    pub(crate) palette: Option<Palette>,
    /// Per-widget UI font (size and/or family); `None` uses the theme role
    /// fonts, so defaults are untouched.
    pub(crate) font: Option<FontSpec>,
    pub(crate) role: Option<A11yRole>,
    pub(crate) narration: Option<Narration>,
    pub(crate) exposed: Option<bool>,
}

impl WidgetProps {
    pub fn new() -> Self {
        Self::default()
    }

    /// Per-instance visibility (the container owns input, Tab and the tree).
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = Some(visible);
        self
    }

    /// Per-instance enabled state.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    /// Per-instance focusability (overrides the widget type's default).
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = Some(focusable);
        self
    }

    /// Tab-order weight; smaller values come first. Default `0`.
    pub fn focus_order(mut self, weight: i32) -> Self {
        self.focus_order = Some(weight);
        self
    }

    /// Cursor shape while hovering.
    pub fn cursor(mut self, cursor: Cursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    /// Whether pointer hit-testing may land on the widget. Display widgets
    /// default to pass-through; `mouse(true)` makes a card or toast clickable
    /// and enables trigger bindings.
    pub fn mouse(mut self, interactive: bool) -> Self {
        self.interactive = Some(interactive);
        self
    }

    /// Read-only state for editors (no native editing session) and the a11y
    /// tree.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = Some(read_only);
        self
    }

    /// Default tooltip text, shown by the host after its hover/focus dwell.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Tooltip for one item of a composite control (absolute list row,
    /// segmented option or tab index). Items without an entry fall back to
    /// [`WidgetProps::tooltip`]; an empty string suppresses the tip for that
    /// item.
    pub fn tooltip_at(mut self, index: usize, text: impl Into<String>) -> Self {
        let tips = self.item_tooltips.get_or_insert_with(Vec::new);
        if tips.len() <= index {
            tips.resize(index + 1, None);
        }
        tips[index] = Some(text.into());
        self
    }

    /// Resolve the tooltip for `item`: the per-item entry wins (including an
    /// empty entry, which suppresses the tip), otherwise the widget default.
    pub(crate) fn tooltip_for(&self, item: Option<usize>) -> Option<&str> {
        if let Some(item) = item
            && let Some(tips) = &self.item_tooltips
            && let Some(Some(text)) = tips.get(item)
        {
            return (!text.is_empty()).then_some(text.as_str());
        }
        self.tooltip.as_deref().filter(|text| !text.is_empty())
    }

    /// Context menu for this widget: right click, the menu key or Shift+F10
    /// asks the host to show it. Commands come back through
    /// [`crate::Host::menu_command`]. Runtime changes use
    /// [`crate::Ui::set_context_menu`]; the model is static, so a menu that
    /// reflects live state is rebuilt by the application.
    pub fn context_menu(mut self, menu: Menu) -> Self {
        self.context_menu = Some(menu);
        self
    }

    /// UIA `AutomationId`; defaults to the widget id when unset.
    pub fn automation_id(mut self, id: impl Into<String>) -> Self {
        self.automation_id = Some(id.into());
        self
    }

    /// Draw this widget with a different palette (emphasis, warning panels).
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = Some(palette);
        self
    }

    /// Replace this widget's UI font override.
    pub fn font(mut self, spec: FontSpec) -> Self {
        self.font = (!spec.is_empty()).then_some(spec);
        self
    }

    /// Absolute text size in DIPs for this widget's UI text (icon glyphs keep
    /// their own face and size).
    pub fn font_size(self, size: f32) -> Self {
        self.font_spec(|spec| spec.size(size))
    }

    /// Explicit font family for this widget's UI text.
    pub fn font_family(self, family: impl Into<String>) -> Self {
        self.font_spec(|spec| spec.family(family))
    }

    fn font_spec(mut self, update: impl FnOnce(FontSpec) -> FontSpec) -> Self {
        let spec = self.font.take().unwrap_or_default();
        let spec = update(spec);
        self.font = (!spec.is_empty()).then_some(spec);
        self
    }

    /// Override the accessibility role derived from the widget type.
    pub fn role(mut self, role: A11yRole) -> Self {
        self.role = Some(role);
        self
    }

    /// Narration overrides for this instance.
    pub fn narrate(mut self, narration: Narration) -> Self {
        self.narration = Some(narration);
        self
    }

    /// Shorthand for [`WidgetProps::narrate`] with a fixed name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        let narration = self.narration.take().unwrap_or_default().name(name);
        self.narration = Some(narration);
        self
    }

    /// Shorthand for a fixed accessible description.
    pub fn describe(mut self, description: impl Into<String>) -> Self {
        let narration = self
            .narration
            .take()
            .unwrap_or_default()
            .description(description);
        self.narration = Some(narration);
        self
    }

    /// Force the widget in or out of the accessibility tree. Display-only
    /// widgets are out by default even when a focus policy puts them in Tab.
    pub fn expose(mut self, exposed: bool) -> Self {
        self.exposed = Some(exposed);
        self
    }
}

/// Which content initial values a widget's setters accept (M7 §6).
///
/// Published by [`crate::Widget::content_caps`], so the layout-skin applier
/// warns instead of silently writing a value the widget cannot take.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentCaps(u16);

impl ContentCaps {
    pub const NONE: Self = Self(0);
    /// [`crate::Widget::set_text`].
    pub const TEXT: Self = Self(1 << 0);
    /// [`crate::Widget::set_label`].
    pub const LABEL: Self = Self(1 << 1);
    /// [`crate::Widget::set_value`].
    pub const VALUE: Self = Self(1 << 2);
    /// [`crate::Widget::set_progress`].
    pub const PROGRESS: Self = Self(1 << 3);
    /// [`crate::Widget::set_number`].
    pub const NUMBER: Self = Self(1 << 4);
    /// [`crate::Widget::set_toggle`].
    pub const TOGGLED: Self = Self(1 << 5);
    /// [`crate::Widget::set_selected`].
    pub const SELECTION: Self = Self(1 << 6);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn bits(self) -> u16 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str) -> A11yInfo {
        A11yInfo::new(A11yRole::Button, name)
    }

    #[test]
    fn unset_narration_keeps_the_derived_strings() {
        let narration = Narration::new();
        assert_eq!(narration.resolve_name(&info("derived")), None);
        let mut children = vec![A11yChild::new(3, "row", Default::default())];
        assert!(!narration.apply_rows(&mut children));
        assert_eq!(children[0].name, "row");
    }

    #[test]
    fn fixed_and_lazy_names_see_the_current_info() {
        let narration = Narration::new().name_with(|info| format!("{}!", info.name));
        assert_eq!(narration.resolve_name(&info("Play")).as_deref(), Some("Play!"));

        let static_name = Narration::new().name("Transport");
        assert_eq!(static_name.resolve_name(&info("Play")).as_deref(), Some("Transport"));
    }

    #[test]
    fn row_formatter_sees_the_absolute_index() {
        let narration =
            Narration::new().row(|index, child| format!("第 {} 首 {}", index + 1, child.name));
        let mut children =
            vec![A11yChild::new(10, "夜曲", Default::default()).selected(true)];
        assert!(narration.apply_rows(&mut children));
        assert_eq!(children[0].name, "第 11 首 夜曲");
    }

    #[test]
    fn props_chain_keeps_only_the_fields_that_were_set() {
        let props = WidgetProps::new()
            .mouse(true)
            .focus_order(2)
            .automation_id("card-1")
            .name("Details");
        assert_eq!(props.focusable, None);
        assert_eq!(props.interactive, Some(true));
        assert_eq!(props.focus_order, Some(2));
        assert_eq!(props.automation_id.as_deref(), Some("card-1"));
        assert_eq!(
            props
                .narration
                .as_ref()
                .and_then(|narration| narration.resolve_name(&info("x")))
                .as_deref(),
            Some("Details")
        );
    }

    #[test]
    fn per_item_tooltips_override_fall_back_and_suppress() {
        let props = WidgetProps::new()
            .tooltip("default")
            .tooltip_at(1, "one")
            .tooltip_at(3, "");
        assert_eq!(props.tooltip_for(None), Some("default"));
        assert_eq!(props.tooltip_for(Some(0)), Some("default"));
        assert_eq!(props.tooltip_for(Some(1)), Some("one"));
        // An explicit empty entry suppresses the default for that item.
        assert_eq!(props.tooltip_for(Some(3)), None);
        // Items past the configured entries fall back to the default.
        assert_eq!(props.tooltip_for(Some(9)), Some("default"));
    }

    #[test]
    fn font_spec_merges_over_the_role_style_and_skips_glyphs() {
        use crate::backend::{FontFace, TextStyle};

        let spec = FontSpec::new().size(18.0).family("Consolas");
        let base = TextStyle::ui(13.5);
        let style = spec.apply(&base);
        assert_eq!(style.size, 18.0);
        assert_eq!(style.face, FontFace::Named("Consolas".into()));
        assert_eq!(style.weight, base.weight);

        // Icon glyphs keep their own face and size.
        let glyph = TextStyle::glyph(15.0);
        assert_eq!(spec.apply(&glyph), glyph);

        // Empty specs are dropped so the default path sees no override.
        let props = WidgetProps::new()
            .font_size(16.0)
            .font_family("Cascadia Mono");
        let font = props.font.as_ref().expect("font override");
        assert_eq!(font.size, Some(16.0));
        assert_eq!(font.family.as_deref(), Some("Cascadia Mono"));
        assert!(WidgetProps::new().font(FontSpec::new()).font.is_none());
    }
}
