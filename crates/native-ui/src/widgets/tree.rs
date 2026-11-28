// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Virtualized tree: an expandable item hierarchy flattened into the visible
//! rows, painted and hit-tested a viewport at a time.
//!
//! The application owns the data (`TreeItem` values with an `expanded` flag);
//! the widget flattens the *visible* nodes into a cached row vector on every
//! structural change (item replacement, expand/collapse), never on a paint.
//! Rows carry their path instead of their label, so a flat row is a cheap
//! index chain and the paint reads the label from the tree. Messages carry the
//! node's path (`&[usize]`), which stays valid across a rebuild.

use std::cell::RefCell;

use crate::a11y::{A11yChild, A11yInfo, A11yRole, A11yScroll};
use crate::backend::{TextAlign, TextOverflow};
use crate::event::Cursor;
use crate::geometry::{Point, Rect};
use crate::menu::Menu;
use crate::skin::{SkinAttrDecl, SkinElement, SkinValue, SkinValueKind};
use crate::widget::{
    TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState,
};

use super::inline::PaintedField;
use super::scrollbar::{self, BarHit, ScrollBarState, ScrollBarStyle};

static TREE_SKIN_CHILDREN: &[&str] = &["node"];
static TREE_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("row-height", SkinValueKind::Number),
    SkinAttrDecl::new("indent", SkinValueKind::Number),
    SkinAttrDecl::new("renameable", SkinValueKind::Bool),
    SkinAttrDecl::new("draggable", SkinValueKind::Bool),
    SkinAttrDecl::new("empty-text", SkinValueKind::Text),
];
const NODE_SKIN_ATTRS: &[&str] = &["label", "glyph", "secondary", "expanded", "disabled"];

/// One node. `expanded` is meaningful only for a node with children; the
/// application sets it at creation or through [`TreeView::set_expanded`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeItem {
    pub label: String,
    /// Optional MDL2 glyph painted in the row's icon column.
    pub glyph: Option<char>,
    /// Optional right-aligned muted text (a count, a size, a state).
    pub secondary: String,
    pub children: Vec<TreeItem>,
    pub expanded: bool,
    pub disabled: bool,
}

impl TreeItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            glyph: None,
            secondary: String::new(),
            children: Vec::new(),
            expanded: false,
            disabled: false,
        }
    }

    pub fn child(mut self, child: TreeItem) -> Self {
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: Vec<TreeItem>) -> Self {
        self.children = children;
        self
    }

    /// MDL2 glyph shown left of the label (a folder, a playlist, a track).
    pub fn glyph(mut self, glyph: char) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// Muted right-aligned text at the end of the row.
    pub fn secondary(mut self, text: impl Into<String>) -> Self {
        self.secondary = text.into();
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn is_expandable(&self) -> bool {
        !self.children.is_empty()
    }

    /// Build a node (and its subtree) from a skin child element; `None` when
    /// the required `label` is missing. The caller validates first.
    pub(crate) fn from_skin(element: &SkinElement) -> Option<Self> {
        let mut item = Self::new(element.attr("label")?);
        if let Some(glyph) = element.attr("glyph").and_then(|glyph| glyph.chars().next()) {
            item.glyph = Some(glyph);
        }
        item.secondary = element.attr("secondary").unwrap_or("").to_string();
        item.expanded = element.attr_bool("expanded").unwrap_or(false);
        item.disabled = element.attr_bool("disabled").unwrap_or(false);
        item.children = element
            .children
            .iter()
            .filter_map(Self::from_skin)
            .collect();
        Some(item)
    }

    /// Validate one `<node>` element and its nested `<node>` subtree: a
    /// non-empty label, the fixed vocabulary, a one-character glyph and the
    /// two booleans.
    pub(crate) fn valid_skin_node(element: &SkinElement) -> bool {
        element.name == "node"
            && element
                .attr("label")
                .is_some_and(|label| !label.trim().is_empty())
            && element.attrs_are(NODE_SKIN_ATTRS)
            && element
                .attr("glyph")
                .is_none_or(|glyph| glyph.chars().count() == 1)
            && element.attr_bool_ok("expanded")
            && element.attr_bool_ok("disabled")
            && element.children.iter().all(Self::valid_skin_node)
    }
}

/// One visible row: a node path, its indentation depth and the state the paint
/// needs. Built by [`TreeView::rebuild_rows`], so a paint never walks the tree.
#[derive(Clone, Debug, PartialEq)]
struct FlatRow {
    path: Vec<usize>,
    depth: usize,
    expandable: bool,
    expanded: bool,
    disabled: bool,
    /// The node paints a glyph column and/or reserves room for secondary
    /// text, so the label starts and ends at a different column.
    has_glyph: bool,
    has_secondary: bool,
}

/// Which kind of move a drop would perform (the drag-policy categories).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeDropKind {
    /// Before/after a sibling under the same parent (a reorder).
    Reorder,
    /// Dropped onto a row's middle: the node becomes its child.
    Nest,
    /// Before/after a node at the root level, from a nested node (detaches a
    /// child from its parent).
    Detach,
    /// Before/after a node under a different, non-root parent.
    Reparent,
}

/// Which drop kinds a drag may perform. Every kind is allowed by default; a
/// disabled kind makes the drag show no drop hint over those targets and the
/// release changes nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeDragRules {
    pub reorder: bool,
    pub nest: bool,
    pub detach: bool,
    pub reparent: bool,
}

impl Default for TreeDragRules {
    fn default() -> Self {
        Self {
            reorder: true,
            nest: true,
            detach: true,
            reparent: true,
        }
    }
}

impl TreeDragRules {
    pub fn new() -> Self {
        Self::default()
    }

    /// Same-parent before/after moves.
    pub fn reorder(mut self, allow: bool) -> Self {
        self.reorder = allow;
        self
    }

    /// Drop-onto-a-row moves (becomes a child node).
    pub fn nest(mut self, allow: bool) -> Self {
        self.nest = allow;
        self
    }

    /// Root-level before/after moves from a nested node (detaches it).
    pub fn detach(mut self, allow: bool) -> Self {
        self.detach = allow;
        self
    }

    /// Before/after moves under a different, non-root parent.
    pub fn reparent(mut self, allow: bool) -> Self {
        self.reparent = allow;
        self
    }

    pub fn allows(&self, kind: TreeDropKind) -> bool {
        match kind {
            TreeDropKind::Reorder => self.reorder,
            TreeDropKind::Nest => self.nest,
            TreeDropKind::Detach => self.detach,
            TreeDropKind::Reparent => self.reparent,
        }
    }
}

type PathHandler<M> = Box<dyn Fn(&[usize]) -> M>;
type ToggleHandler<M> = Box<dyn Fn(&[usize], bool) -> M>;
type RenameHandler<M> = Box<dyn Fn(&[usize], &str) -> M>;
type MoveHandler<M> = Box<dyn Fn(&[usize], &[usize]) -> M>;

/// An open inline label session: the node path the embedded painted field
/// covers.
struct RenameDraft {
    path: Vec<usize>,
}

/// An in-flight drag: the node being moved and the drop slot it would land in.
#[derive(Clone, Debug, PartialEq)]
struct TreeDrag {
    from: Vec<usize>,
    /// Destination parent path (`[]` for the root list) and insertion index.
    parent: Vec<usize>,
    index: usize,
    /// Paint an outline around the row (drop inside) instead of a line.
    inside: bool,
    /// The row the hint is drawn on and the slot line, in client DIPs.
    target_row: usize,
    hint: Rect,
    /// A valid drop slot was found. The initial placeholder (a drag that has
    /// not moved over a legal target yet) is invalid, so releasing it never
    /// moves the node somewhere nobody pointed at.
    valid: bool,
}

pub struct TreeView<M> {
    id: WidgetId,
    rect: Rect,
    roots: Vec<TreeItem>,
    rows: Vec<FlatRow>,
    row_height: f32,
    indent: f32,
    scroll: f32,
    hover_row: Option<usize>,
    press_row: Option<usize>,
    press_chevron: bool,
    /// Press point, so a drag starts only after real travel.
    press_pos: Option<Point>,
    /// In-flight drag reorder (a node or one of its ancestors is being moved).
    drag: Option<TreeDrag>,
    bar: ScrollBarState,
    style: ScrollBarStyle,
    /// Narrow repaint for the input that just ran (bar chrome only).
    repaint_hint: Option<Rect>,
    selected: usize,
    chevrons: (char, char),
    empty_text: String,
    /// Labels can be edited in place (F2).
    renameable: bool,
    /// Press-drag moves are enabled (default true).
    draggable: bool,
    /// Which drop kinds a drag may perform.
    drag_rules: TreeDragRules,
    /// Per-gesture veto: `false` for the pressed node means this drag never
    /// starts (the press stays a click).
    on_drag_start: Option<Box<dyn Fn(&[usize]) -> bool>>,
    /// Live target filter: asked with the prospective path whenever the drop
    /// target changes. `false` makes the current target invalid.
    on_drag_over: Option<Box<dyn Fn(&[usize], &[usize]) -> bool>>,
    /// Release veto: asked with `(from, to)` before the move is applied.
    /// `false` cancels the drop.
    on_drag_end: Option<Box<dyn Fn(&[usize], &[usize]) -> bool>>,
    /// The current gesture was vetoed, so motion does not start a drag.
    drag_denied: bool,
    /// The prospective path last offered to `on_drag_over` and its verdict
    /// (the callback runs once per target change, not per mouse move).
    drag_offer: Option<(Vec<usize>, bool)>,
    /// Container read-only state: rows stay navigable but no rename opens.
    read_only: bool,
    rename: Option<RenameDraft>,
    /// The embedded painted field that edits the rename.
    field: RefCell<PaintedField<M>>,
    on_select: Option<PathHandler<M>>,
    on_activate: Option<PathHandler<M>>,
    on_toggle: Option<ToggleHandler<M>>,
    on_rename: Option<RenameHandler<M>>,
    on_move: Option<MoveHandler<M>>,
}

/// Left inset before the first chevron/indent column.
const TREE_INSET: f32 = 4.0;
/// Width reserved for the expand/collapse chevron.
const CHEVRON_WIDTH: f32 = 16.0;
/// Width reserved for the optional leading glyph.
const GLYPH_WIDTH: f32 = 18.0;
/// Width reserved at the right end for secondary text.
const SECONDARY_WIDTH: f32 = 96.0;
/// Right margin after the label/secondary column.
const TREE_MARGIN: f32 = 8.0;
/// Vertical travel before a row press becomes a drag reorder.
const DRAG_THRESHOLD: f32 = 6.0;

impl<M> TreeView<M> {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 320.0, 280.0),
            roots: Vec::new(),
            rows: Vec::new(),
            row_height: 28.0,
            indent: 18.0,
            scroll: 0.0,
            hover_row: None,
            press_row: None,
            press_chevron: false,
            press_pos: None,
            drag: None,
            bar: ScrollBarState::default(),
            style: ScrollBarStyle::default(),
            repaint_hint: None,
            selected: 0,
            // MDL2 chevron-right / chevron-down, the accordion's marks.
            chevrons: ('\u{E76C}', '\u{E70D}'),
            empty_text: String::new(),
            renameable: false,
            draggable: true,
            drag_rules: TreeDragRules::default(),
            on_drag_start: None,
            on_drag_over: None,
            on_drag_end: None,
            drag_denied: false,
            drag_offer: None,
            read_only: false,
            rename: None,
            field: RefCell::new(PaintedField::new()),
            on_select: None,
            on_activate: None,
            on_toggle: None,
            on_rename: None,
            on_move: None,
        }
    }

    pub fn items(mut self, items: Vec<TreeItem>) -> Self {
        self.set_items(items);
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(16.0);
        self.clamp_scroll();
        self
    }

    /// Indentation per level in DIPs.
    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = indent.max(8.0);
        self
    }

    /// Marks for a collapsed and an expanded node. A private-use codepoint is
    /// drawn with the glyph face, a text character with the UI face.
    pub fn chevrons(mut self, collapsed: char, expanded: char) -> Self {
        self.chevrons = (collapsed, expanded);
        self
    }

    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.empty_text = text.into();
        self
    }

    pub fn on_select(mut self, handler: impl Fn(&[usize]) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    /// Enter or a double click on the selected/clicked node.
    pub fn on_activate(mut self, handler: impl Fn(&[usize]) -> M + 'static) -> Self {
        self.on_activate = Some(Box::new(handler));
        self
    }

    /// A node was expanded or collapsed (chevron, Right/Left, Space, UIA).
    pub fn on_toggle(mut self, handler: impl Fn(&[usize], bool) -> M + 'static) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }

    /// Let `F2` rename the selected row in place. Enter/Tab commit, Escape
    /// reverts and an empty label is ignored; the widget applies the new label
    /// before reporting it through [`TreeView::on_rename`].
    pub fn renameable(mut self, renameable: bool) -> Self {
        self.renameable = renameable;
        self
    }

    /// A row's label was committed in place.
    pub fn on_rename(mut self, handler: impl Fn(&[usize], &str) -> M + 'static) -> Self {
        self.on_rename = Some(Box::new(handler));
        self
    }

    /// Enable or disable press-drag moves (default on). A turned-off tree
    /// still selects and renames; the pointer never starts a drag.
    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }

    /// Restrict which drop kinds a drag may perform (all by default).
    pub fn drag_rules(mut self, rules: TreeDragRules) -> Self {
        self.drag_rules = rules;
        self
    }

    /// Per-gesture veto: return `false` for the pressed node and this drag
    /// never starts (the press stays a click). Called once when the pointer
    /// crosses the drag threshold. The static [`TreeView::drag_rules`] decide
    /// *where* an allowed drag may land.
    pub fn on_drag_start(mut self, handler: impl Fn(&[usize]) -> bool + 'static) -> Self {
        self.on_drag_start = Some(Box::new(handler));
        self
    }

    /// Live target filter, called with `(from, to)` whenever the drop target
    /// under the pointer changes — `to` is the path the node would get on
    /// release. `false` makes that target invalid: no hint, and releasing
    /// there changes nothing. Not called per mouse move (only when the
    /// prospective path changes).
    pub fn on_drag_over(mut self, handler: impl Fn(&[usize], &[usize]) -> bool + 'static) -> Self {
        self.on_drag_over = Some(Box::new(handler));
        self
    }

    /// Release veto, called once with `(from, to)` after a valid drag target
    /// is released and before the tree changes. `false` cancels the drop (the
    /// node stays where it was); `true` applies it and then reports
    /// [`TreeView::on_move`].
    pub fn on_drag_end(mut self, handler: impl Fn(&[usize], &[usize]) -> bool + 'static) -> Self {
        self.on_drag_end = Some(Box::new(handler));
        self
    }

    /// Runtime form of [`TreeView::draggable`]; cancels a drag in flight when
    /// turned off. Returns whether the flag changed.
    pub fn set_draggable(&mut self, draggable: bool) -> bool {
        if self.draggable == draggable {
            return false;
        }
        self.draggable = draggable;
        if !draggable {
            self.drag = None;
        }
        true
    }

    pub fn is_draggable(&self) -> bool {
        self.draggable
    }

    /// Row height in DIPs (16 minimum); returns whether it changed.
    pub fn set_row_height(&mut self, height: f32) -> bool {
        let height = height.max(16.0);
        if (self.row_height - height).abs() < 0.01 {
            return false;
        }
        self.row_height = height;
        self.clamp_scroll();
        self.ensure_selected_visible();
        true
    }

    pub fn row_height_value(&self) -> f32 {
        self.row_height
    }

    /// Indentation per depth level in DIPs (8 minimum). Returns whether it
    /// changed.
    pub fn set_indent(&mut self, indent: f32) -> bool {
        let indent = indent.max(8.0);
        if (self.indent - indent).abs() < 0.01 {
            return false;
        }
        self.indent = indent;
        true
    }

    pub fn indent_value(&self) -> f32 {
        self.indent
    }

    /// Allow F2/rename; turning it off closes an open session. Returns whether
    /// the flag changed.
    pub fn set_renameable(&mut self, renameable: bool) -> bool {
        if self.renameable == renameable {
            return false;
        }
        self.renameable = renameable;
        if !renameable {
            self.close_rename();
        }
        true
    }

    pub fn is_renameable(&self) -> bool {
        self.renameable
    }

    /// Message painted while the tree has no visible rows.
    pub fn set_empty_text(&mut self, text: impl Into<String>) -> bool {
        let text = text.into();
        if self.empty_text == text {
            return false;
        }
        self.empty_text = text;
        true
    }

    pub fn empty_text_ref(&self) -> &str {
        &self.empty_text
    }

    /// Runtime form of [`TreeView::drag_rules`]. Returns whether they changed.
    pub fn set_drag_rules(&mut self, rules: TreeDragRules) -> bool {
        if self.drag_rules == rules {
            return false;
        }
        self.drag_rules = rules;
        // A slot that is no longer allowed disappears with the new rules (the
        // gesture continues as a click unless the pointer moves again).
        self.drag = None;
        self.drag_denied = true;
        true
    }

    /// A press-and-drag moved a node. The widget has already applied the move
    /// and `to` is the node's new path, so the application only needs to
    /// persist it (or push its own items back through [`TreeView::set_items`]).
    pub fn on_move(mut self, handler: impl Fn(&[usize], &[usize]) -> M + 'static) -> Self {
        self.on_move = Some(Box::new(handler));
        self
    }

    pub fn scrollbar_style(mut self, style: ScrollBarStyle) -> Self {
        self.style = style;
        self
    }

    pub fn scrollbar(&self) -> &ScrollBarStyle {
        &self.style
    }

    // --- typed access -----------------------------------------------------

    pub fn items_ref(&self) -> &[TreeItem] {
        &self.roots
    }

    /// Number of currently visible (flattened) rows.
    pub fn visible_row_count(&self) -> usize {
        self.rows.len()
    }

    /// `(first, end)` visible rows, end exclusive. The last published row is
    /// the look-ahead row the container marks offscreen.
    pub fn visible_rows(&self) -> (usize, usize) {
        self.visible_range()
    }

    pub fn selected_row(&self) -> usize {
        self.selected
    }

    /// Path of the keyboard/click cursor, or `None` on an empty tree.
    pub fn selected_path(&self) -> Option<Vec<usize>> {
        self.rows.get(self.selected).map(|row| row.path.clone())
    }

    pub fn scroll_offset(&self) -> f32 {
        self.scroll
    }

    /// Label of a flattened row (what the paint and the a11y tree read).
    pub fn row_label(&self, row: usize) -> Option<&str> {
        let path = self.rows.get(row)?.path.clone();
        self.item(&path).map(|item| item.label.as_str())
    }

    pub fn item(&self, path: &[usize]) -> Option<&TreeItem> {
        let (last, parents) = path.split_last()?;
        let mut items = &self.roots;
        for index in parents {
            items = &items.get(*index)?.children;
        }
        items.get(*last)
    }

    fn item_mut(&mut self, path: &[usize]) -> Option<&mut TreeItem> {
        let (last, parents) = path.split_last()?;
        let mut items = &mut self.roots;
        for index in parents {
            items = &mut items.get_mut(*index)?.children;
        }
        items.get_mut(*last)
    }

    pub fn is_expanded(&self, path: &[usize]) -> bool {
        self.item(path).map(|item| item.expanded).unwrap_or(false)
    }

    /// Whether an inline label session is open, and on which node.
    pub fn is_renaming(&self) -> bool {
        self.rename.is_some()
    }

    pub fn renaming_path(&self) -> Option<&[usize]> {
        self.rename.as_ref().map(|rename| rename.path.as_slice())
    }

    /// Open the inline label session on `path` (the same path F2 takes).
    pub fn begin_rename(&mut self, path: &[usize]) -> bool {
        if !self.renameable || self.read_only {
            return false;
        }
        let Some(item) = self.item(path) else {
            return false;
        };
        if item.disabled {
            return false;
        }
        let label = item.label.clone();
        let Some(position) = self.rows.iter().position(|row| row.path == path) else {
            return false;
        };
        self.selected = position;
        self.ensure_selected_visible();
        let Some(rect) = self.rename_field_rect(position) else {
            return false;
        };
        if !self
            .field
            .get_mut()
            .begin(rect, &label, None, crate::geometry::Insets::xy(3.0, 2.0))
        {
            return false;
        }
        self.rename = Some(RenameDraft {
            path: path.to_vec(),
        });
        true
    }

    /// The field rectangle over one row's label: the label cell grown by a
    /// couple of DIPs, at least 24 wide, inset from the row's top/bottom.
    fn rename_field_rect(&self, index: usize) -> Option<Rect> {
        let row = self.rows.get(index)?;
        let row_rect = self.row_rect(index);
        let label = self.label_rect(row_rect, row);
        Some(Rect::new(
            label.left - 2.0,
            row_rect.top + 2.0,
            label.right.max(label.left + 24.0),
            row_rect.bottom - 2.0,
        ))
    }

    /// Replace a node's label without opening a session (typed access).
    pub fn rename(&mut self, path: &[usize], label: impl Into<String>) -> bool {
        let label = label.into();
        let Some(item) = self.item_mut(path) else {
            return false;
        };
        if item.label == label {
            return false;
        }
        item.label = label;
        true
    }

    /// Move a node under `parent` at `index` (the drag path without a pointer).
    /// `parent` is `&[]` for the root list. Returns whether the tree changed.
    pub fn move_item(&mut self, from: &[usize], parent: &[usize], index: usize) -> bool {
        self.apply_move(from, parent, index).is_some()
    }

    /// Move the node at `from` (remove + insert) and select its new path.
    fn apply_move(&mut self, from: &[usize], parent: &[usize], index: usize) -> Option<Vec<usize>> {
        let (last, source_parent) = from.split_last()?;
        if parent.len() > from.len() && parent.starts_with(from) {
            return None;
        }
        if parent == from {
            return None;
        }
        let source = self.item(from)?;
        if source.disabled {
            return None;
        }
        // The move is a no-op when the node would land exactly where it is.
        if parent == source_parent && (*last == index || *last + 1 == index) {
            return None;
        }
        let item = if source_parent.is_empty() {
            if *last >= self.roots.len() {
                return None;
            }
            self.roots.remove(*last)
        } else {
            let parent_item = self.item_mut(source_parent)?;
            if *last >= parent_item.children.len() {
                return None;
            }
            parent_item.children.remove(*last)
        };
        let (parent, index) = Self::destination(from, parent, index);
        let inserted = if parent.is_empty() {
            let index = index.min(self.roots.len());
            self.roots.insert(index, item);
            Ok(vec![index])
        } else if let Some(target) = self.item_mut(&parent) {
            target.expanded = true;
            let index = index.min(target.children.len());
            target.children.insert(index, item);
            let mut path = parent.clone();
            path.push(index);
            Ok(path)
        } else {
            Err(item)
        };
        let new_path = match inserted {
            Ok(path) => path,
            Err(item) => {
                // Defensive: a rejected target must never drop the node. Put
                // it back where it came from and leave the tree unchanged.
                if source_parent.is_empty() {
                    let at = (*last).min(self.roots.len());
                    self.roots.insert(at, item);
                } else if let Some(parent_item) = self.item_mut(source_parent) {
                    let at = (*last).min(parent_item.children.len());
                    parent_item.children.insert(at, item);
                }
                self.rebuild_rows();
                return None;
            }
        };
        self.close_rename();
        self.drag = None;
        self.rebuild_rows();
        if let Some(position) = self.rows.iter().position(|row| row.path == new_path) {
            self.selected = position;
            self.ensure_selected_visible();
        }
        Some(new_path)
    }

    /// The post-removal destination for a drop: the parent path and insertion
    /// index with the shift from removing `from` applied. The removal shifts
    /// the sibling indices of the source parent, so a target path that goes
    /// through that list has its component decremented (dropping onto a later
    /// sibling would otherwise address a different node, or none at all).
    fn destination(from: &[usize], parent: &[usize], index: usize) -> (Vec<usize>, usize) {
        let Some((last, source_parent)) = from.split_last() else {
            return (parent.to_vec(), index);
        };
        let mut parent = parent.to_vec();
        if parent.len() > source_parent.len()
            && parent[..source_parent.len()] == *source_parent
            && parent[source_parent.len()] > *last
        {
            parent[source_parent.len()] -= 1;
        }
        let mut index = index;
        if parent == source_parent && *last < index {
            index -= 1;
        }
        (parent, index)
    }

    /// The path a slot would give `from` (for the live drag filter).
    fn slot_path(from: &[usize], slot: &TreeDrag) -> Vec<usize> {
        let (mut parent, index) = Self::destination(from, &slot.parent, slot.index);
        parent.push(index);
        parent
    }

    /// Which policy kind a slot performs.
    fn drop_kind(from: &[usize], slot: &TreeDrag) -> TreeDropKind {
        if slot.inside {
            TreeDropKind::Nest
        } else if slot.parent.as_slice() == &from[..from.len().saturating_sub(1)] {
            TreeDropKind::Reorder
        } else if slot.parent.is_empty() {
            TreeDropKind::Detach
        } else {
            TreeDropKind::Reparent
        }
    }

    /// Whether the slot is a legal move and the configured
    /// [`TreeDragRules`] allow its kind.
    fn can_drop(&self, from: &[usize], slot: &TreeDrag) -> bool {
        let (parent, index) = (&slot.parent, slot.index);
        if from.is_empty() || parent.starts_with(from) || parent == from {
            return false;
        }
        if self.item(from).is_none() {
            return false;
        }
        if !self.drag_rules.allows(Self::drop_kind(from, slot)) {
            return false;
        }
        if parent.is_empty() {
            return true;
        }
        let Some(target) = self.item(parent) else {
            return false;
        };
        !target.disabled && index <= target.children.len()
    }

    /// Drop slot under `pos`: before/after a row at its own depth, or inside
    /// the row. `None` when the pointer is outside the list.
    fn drop_slot(&self, pos: Point) -> Option<TreeDrag> {
        let index = self.row_at(pos)?;
        let row = &self.rows[index];
        if row.disabled {
            return None;
        }
        let rect = self.row_rect(index);
        let rel = (pos.y - rect.top) / self.row_height;
        let left = rect.left + TREE_INSET + row.depth as f32 * self.indent + CHEVRON_WIDTH;
        let (parent, slot, inside) = if rel < 0.25 {
            let mut parent = row.path.clone();
            let last = parent.pop().unwrap_or(0);
            (parent, last, false)
        } else if rel > 0.75 {
            let mut parent = row.path.clone();
            let last = parent.pop().unwrap_or(0);
            (parent, last + 1, false)
        } else {
            let count = self
                .item(&row.path)
                .map(|item| item.children.len())
                .unwrap_or(0);
            (row.path.clone(), count, true)
        };
        let hint = if inside {
            Rect::new(rect.left, rect.top, self.list_rect().right, rect.bottom)
        } else {
            let y = if rel < 0.25 {
                rect.top + 1.0
            } else {
                rect.bottom - 1.0
            };
            Rect::new(left, y - 1.0, self.list_rect().right, y + 1.0)
        };
        Some(TreeDrag {
            from: Vec::new(),
            parent,
            index: slot,
            inside,
            target_row: index,
            hint,
            valid: true,
        })
    }

    /// Scroll one row while a drag is held outside the list viewport. Staying
    /// inside never scrolls, so a row near the edge is still a drop target.
    fn drag_autoscroll(&mut self, pos: Point) {
        let list = self.list_rect();
        if pos.y < list.top {
            self.scroll -= self.row_height;
        } else if pos.y > list.bottom {
            self.scroll += self.row_height;
        } else {
            return;
        }
        self.clamp_scroll();
    }

    /// Drop an open rename without applying it (the tree changed under it or a
    /// lock was pushed).
    fn close_rename(&mut self) {
        if self.rename.take().is_some() {
            self.field.get_mut().finish(false);
        }
    }

    /// Commit (`true`) or discard the open rename. The label is applied first.
    fn finish_rename(&mut self, commit: bool) -> Option<M> {
        let rename = self.rename.take()?;
        let Some(label) = self.field.get_mut().finish(commit) else {
            return None;
        };
        if label.is_empty() {
            return None;
        }
        let changed = self
            .item_mut(&rename.path)
            .map(|item| std::mem::replace(&mut item.label, label.clone()) != label)
            .unwrap_or(false);
        if !changed {
            return None;
        }
        self.on_rename
            .as_ref()
            .map(|handler| handler(&rename.path, &label))
    }

    /// Replace the whole hierarchy. The cursor follows its old path when the
    /// node still exists, otherwise it clamps.
    pub fn set_items(&mut self, items: Vec<TreeItem>) -> bool {
        if self.roots == items {
            return false;
        }
        self.roots = items;
        self.close_rename();
        self.drag = None;
        self.rebuild_rows();
        true
    }

    /// Expand or collapse one node. Returns whether the state changed;
    /// expansion is idempotent.
    pub fn set_expanded(&mut self, path: &[usize], expanded: bool) -> bool {
        let Some(item) = self.item_mut(path) else {
            return false;
        };
        if !item.is_expandable() || item.expanded == expanded {
            return false;
        }
        item.expanded = expanded;
        self.rebuild_rows();
        true
    }

    /// Move the cursor to a node path. Returns whether the cursor moved; an
    /// unknown or collapsed path is ignored.
    pub fn set_selection_path(&mut self, path: &[usize]) -> bool {
        let Some(position) = self.rows.iter().position(|row| row.path == path) else {
            return false;
        };
        if self.selected == position {
            return false;
        }
        self.selected = position;
        self.ensure_selected_visible();
        true
    }

    pub fn scroll_to_row(&mut self, row: usize) -> bool {
        if self.rows.is_empty() {
            return false;
        }
        let row = row.min(self.rows.len() - 1);
        let target = (row as f32 * self.row_height).clamp(0.0, self.max_scroll());
        if (self.scroll - target).abs() < 0.01 {
            return false;
        }
        self.scroll = target;
        true
    }

    pub fn ensure_row_visible(&mut self, row: usize) -> bool {
        if self.rows.is_empty() {
            return false;
        }
        let row = row.min(self.rows.len() - 1);
        let before = self.scroll;
        let top = row as f32 * self.row_height;
        let bottom = top + self.row_height;
        if top < self.scroll {
            self.scroll = top;
        } else if bottom > self.scroll + self.rect.height() {
            self.scroll = bottom - self.rect.height();
        }
        self.clamp_scroll();
        (self.scroll - before).abs() > 0.01
    }

    fn ensure_selected_visible(&mut self) {
        if !self.rows.is_empty() {
            self.ensure_row_visible(self.selected);
        }
    }

    // --- flattening -------------------------------------------------------

    fn collect(items: &[TreeItem], prefix: &mut Vec<usize>, depth: usize, out: &mut Vec<FlatRow>) {
        for (index, item) in items.iter().enumerate() {
            prefix.push(index);
            out.push(FlatRow {
                path: prefix.clone(),
                depth,
                expandable: item.is_expandable(),
                expanded: item.expanded,
                disabled: item.disabled,
                has_glyph: item.glyph.is_some(),
                has_secondary: !item.secondary.is_empty(),
            });
            if item.expanded && item.is_expandable() {
                Self::collect(&item.children, prefix, depth + 1, out);
            }
            prefix.pop();
        }
    }

    /// Rebuild the visible-row cache and keep the cursor on the same node.
    /// A node that disappeared (its parent collapsed) moves the cursor to the
    /// nearest surviving ancestor.
    fn rebuild_rows(&mut self) {
        let selected_path = self.rows.get(self.selected).map(|row| row.path.clone());
        self.rows.clear();
        let mut prefix = Vec::new();
        Self::collect(&self.roots, &mut prefix, 0, &mut self.rows);
        let restored = selected_path.and_then(|path| {
            // The node itself first, then its closest visible ancestor.
            for length in (0..=path.len()).rev() {
                if let Some(position) = self.rows.iter().position(|row| row.path == path[..length])
                {
                    return Some(position);
                }
            }
            None
        });
        self.selected =
            restored.unwrap_or_else(|| self.selected.min(self.rows.len().saturating_sub(1)));
        self.clamp_scroll();
        self.ensure_selected_visible();
    }

    // --- geometry ---------------------------------------------------------

    fn content_height(&self) -> f32 {
        self.rows.len() as f32 * self.row_height
    }

    fn max_scroll(&self) -> f32 {
        (self.content_height() - self.rect.height()).max(0.0)
    }

    fn has_scrollbar(&self) -> bool {
        self.max_scroll() > 0.5 && self.rect.height() > 0.0
    }

    fn list_rect(&self) -> Rect {
        if self.has_scrollbar() {
            let track = self.scrollbar_rect();
            Rect::new(
                self.rect.left,
                self.rect.top,
                track.left.max(self.rect.left),
                self.rect.bottom,
            )
        } else {
            self.rect
        }
    }

    fn scrollbar_rect(&self) -> Rect {
        scrollbar::track_rect(self.rect, true, &self.style)
    }

    fn thumb_rect(&self) -> Option<Rect> {
        if !self.has_scrollbar() {
            return None;
        }
        scrollbar::thumb_rect(
            self.scrollbar_rect(),
            self.content_height(),
            self.rect.height(),
            self.scroll,
            true,
            &self.style,
        )
    }

    /// Client rectangle of a visible row (clipped by the list viewport when
    /// read through the paint), so an application can aim a pointer or place
    /// its own overlay.
    pub fn row_rect(&self, index: usize) -> Rect {
        let top = self.rect.top + index as f32 * self.row_height - self.scroll;
        Rect::new(
            self.rect.left,
            top,
            self.list_rect().right,
            top + self.row_height,
        )
    }

    fn chevron_rect(&self, row_rect: Rect, row: &FlatRow) -> Rect {
        let left = row_rect.left + TREE_INSET + row.depth as f32 * self.indent;
        Rect::new(left, row_rect.top, left + CHEVRON_WIDTH, row_rect.bottom)
    }

    fn label_rect(&self, row_rect: Rect, row: &FlatRow) -> Rect {
        let mut left = row_rect.left + TREE_INSET + row.depth as f32 * self.indent + CHEVRON_WIDTH;
        if row.has_glyph {
            left += GLYPH_WIDTH;
        }
        left += 4.0;
        let right = row_rect.right
            - TREE_MARGIN
            - if row.has_secondary {
                SECONDARY_WIDTH
            } else {
                0.0
            };
        Rect::new(left, row_rect.top, right.max(left), row_rect.bottom)
    }

    fn glyph_rect(&self, row_rect: Rect, row: &FlatRow) -> Rect {
        let left = row_rect.left + TREE_INSET + row.depth as f32 * self.indent + CHEVRON_WIDTH;
        Rect::new(left, row_rect.top, left + GLYPH_WIDTH, row_rect.bottom)
    }

    fn secondary_rect(&self, row_rect: Rect) -> Rect {
        let right = row_rect.right - TREE_MARGIN;
        Rect::new(
            (right - SECONDARY_WIDTH).max(row_rect.left),
            row_rect.top,
            right,
            row_rect.bottom,
        )
    }

    fn row_at(&self, pos: Point) -> Option<usize> {
        if self.rows.is_empty() || !self.list_rect().contains_point(pos) {
            return None;
        }
        let y = pos.y - self.rect.top + self.scroll;
        if y < 0.0 {
            return None;
        }
        let index = (y / self.row_height).floor() as usize;
        (index < self.rows.len()).then_some(index)
    }

    /// Row under the pointer and whether it landed on the chevron column.
    fn row_part_at(&self, pos: Point) -> Option<(usize, bool)> {
        let index = self.row_at(pos)?;
        let row = &self.rows[index];
        if !row.expandable {
            return Some((index, false));
        }
        let chevron = self.chevron_rect(self.row_rect(index), row);
        Some((index, chevron.contains_point(pos)))
    }

    fn visible_range(&self) -> (usize, usize) {
        if self.rows.is_empty() {
            return (0, 0);
        }
        let start = (self.scroll / self.row_height).floor().max(0.0) as usize;
        let count = (self.rect.height() / self.row_height).ceil() as usize + 1;
        let end = (start + count).min(self.rows.len());
        (start.min(self.rows.len()), end)
    }

    fn page_rows(&self) -> usize {
        ((self.rect.height() / self.row_height).floor() as usize).max(1)
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    fn selected_row_rect(&self) -> Option<Rect> {
        self.rows.get(self.selected)?;
        self.row_rect(self.selected).intersect(self.list_rect())
    }

    fn select(&mut self, index: usize) -> Option<M> {
        if self.rows.get(index)?.disabled {
            return None;
        }
        self.selected = index;
        self.ensure_selected_visible();
        self.on_select
            .as_ref()
            .map(|handler| handler(&self.rows[index].path))
    }

    fn activate(&self) -> Option<M> {
        let row = self.rows.get(self.selected)?;
        if row.disabled {
            return None;
        }
        self.on_activate.as_ref().map(|handler| handler(&row.path))
    }

    fn toggle_index(&mut self, index: usize, expanded: bool) -> Option<M> {
        let row = self.rows.get(index)?;
        if !row.expandable {
            return None;
        }
        let path = row.path.clone();
        let before = row.expanded;
        let item = self.item_mut(&path)?;
        item.expanded = expanded;
        self.rebuild_rows();
        if before == expanded {
            // Idempotent (Right on an expanded node): no message.
            return None;
        }
        self.on_toggle
            .as_ref()
            .map(|handler| handler(&path, expanded))
    }

    fn toggle(&mut self, index: usize) -> Option<M> {
        let expanded = !self.rows.get(index)?.expanded;
        self.toggle_index(index, expanded)
    }

    /// Move the cursor by `delta` visible rows, skipping disabled nodes.
    fn move_cursor(&mut self, delta: i32) {
        if self.rows.is_empty() {
            return;
        }
        let mut index = self.selected as i32;
        let limit = self.rows.len() as i32;
        loop {
            index += delta.signum();
            if index < 0 || index >= limit {
                return;
            }
            if !self.rows[index as usize].disabled {
                self.selected = index as usize;
                self.ensure_selected_visible();
                return;
            }
        }
    }

    fn cursor_to_edge(&mut self, last: bool) {
        let range: Vec<usize> = if last {
            (0..self.rows.len()).rev().collect()
        } else {
            (0..self.rows.len()).collect()
        };
        if let Some(index) = range.into_iter().find(|index| !self.rows[*index].disabled) {
            self.selected = index;
            self.ensure_selected_visible();
        }
    }

    /// Paint the open rename: the embedded painted field is re-anchored over
    /// the label row (a scroll or resize may have moved it) and paints itself
    /// with the standalone editor's frame, selection and caret.
    fn paint_rename(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState, index: usize) {
        let Some(rect) = self.rename_field_rect(index) else {
            return;
        };
        let mut field = self.field.borrow_mut();
        if field.rect() != rect {
            field.set_rect(rect);
        }
        field.paint(ctx, state);
    }

    fn drag_thumb(&mut self, pos: Point, grab: f32) {
        let Some(thumb) = self.thumb_rect() else {
            return;
        };
        self.scroll = scrollbar::drag_offset(
            pos,
            self.scrollbar_rect(),
            thumb,
            grab,
            self.content_height(),
            self.rect.height(),
            true,
        );
        self.clamp_scroll();
    }
}

impl<M: 'static> Widget<M> for TreeView<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        TREE_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("row-height", SkinValue::Number(height)) => *height >= 16.0,
            ("indent", SkinValue::Number(indent)) => *indent >= 8.0,
            ("renameable" | "draggable", SkinValue::Bool(_)) => true,
            ("empty-text", SkinValue::Text(_)) => true,
            _ => false,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("row-height", SkinValue::Number(height)) => {
                self.set_row_height(*height as f32);
            }
            ("indent", SkinValue::Number(indent)) => {
                self.set_indent(*indent as f32);
            }
            ("renameable", SkinValue::Bool(renameable)) => {
                self.set_renameable(*renameable);
            }
            ("draggable", SkinValue::Bool(draggable)) => {
                self.set_draggable(*draggable);
            }
            ("empty-text", SkinValue::Text(text)) => {
                self.set_empty_text(*text);
            }
            _ => {}
        }
    }

    fn skin_children(&self) -> &'static [&'static str] {
        TREE_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(TreeItem::valid_skin_node)
    }

    /// Nested `<node>` elements replace the whole hierarchy; `selected` (a
    /// visible-row index) still applies afterwards.
    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        let roots = children
            .iter()
            .filter_map(TreeItem::from_skin)
            .collect();
        self.set_items(roots);
    }

    fn id(&self) -> WidgetId {
        self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        self.clamp_scroll();
    }

    fn set_selected(&mut self, index: usize) {
        if index < self.rows.len() {
            self.selected = index;
            self.ensure_selected_visible();
        }
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    /// Only a real hover change repaints (a different row, the bar tint, or a
    /// thumb drag, which moves the rows).
    fn repaint_on_move_at(&self, pos: Point) -> bool {
        if self.bar.active || self.drag.is_some() || self.rename.is_some() {
            return true;
        }
        if self.has_scrollbar()
            && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some()
        {
            return !self.bar.hovered;
        }
        self.bar.hovered || self.row_at(pos) != self.hover_row
    }

    /// Bar-hover/press chrome changes repaint the scrollbar strip only.
    fn take_repaint_hint(&mut self) -> Option<Rect> {
        self.repaint_hint.take()
    }

    /// The tree scrolls its rows with the wheel (D65).
    fn scrolls_with_wheel(&self) -> bool {
        true
    }

    /// Enter enters row navigation; arrows move the cursor (Right/Left also
    /// expand/collapse or walk to the child/parent), Home/End/Page move,
    /// Enter activates and Space toggles, Esc/Backspace leave.
    fn adjustable(&self) -> bool {
        true
    }

    fn focus_ring_rect(&self) -> Rect {
        self.list_rect().inset(1.0, 1.0)
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        if read_only {
            self.close_rename();
        }
    }

    // --- framework-painted label editor ------------------------------------

    fn inline_edit(&self) -> bool {
        self.rename.is_some()
    }

    fn inline_edit_at(&self, pos: Point) -> bool {
        self.rename.as_ref().is_some_and(|rename| {
            self.row_at(pos)
                .is_some_and(|row| self.rows[row].path == rename.path)
        })
    }

    fn editing(&self) -> bool {
        self.rename.is_some()
    }

    fn multiline_edit(&self) -> bool {
        false
    }

    fn begin_inline_edit(&mut self, position: Option<Point>) -> bool {
        match position {
            Some(pos) => {
                // A press inside the open field only moves the caret.
                self.field.get_mut().place_at(pos)
            }
            None => {
                let Some(path) = self.rows.get(self.selected).map(|row| row.path.clone()) else {
                    return false;
                };
                self.begin_rename(&path)
            }
        }
    }

    fn finish_edit(&mut self, commit: bool) -> Option<M> {
        self.finish_rename(commit)
    }

    fn insert_text(&mut self, text: &str) -> bool {
        if self.rename.is_none() {
            return false;
        }
        self.field.get_mut().insert_text(text)
    }

    fn set_composition(&mut self, text: &str, cursor: usize) -> bool {
        if self.rename.is_none() {
            return false;
        }
        self.field.get_mut().set_composition(text, cursor)
    }

    fn caret_rect(&self) -> Option<Rect> {
        self.rename.as_ref()?;
        self.field.borrow().caret_rect()
    }

    /// The standard edit menu while a rename is open (supplied by the shared
    /// painted field).
    fn edit_menu(&self) -> Option<Menu> {
        self.field.borrow().edit_menu()
    }

    fn edit_command(&mut self, command: u32) -> bool {
        self.field.get_mut().edit_command(command)
    }

    fn take_clipboard(&mut self) -> Option<String> {
        self.field.get_mut().take_clipboard()
    }

    /// A scrollbar track shows the arrow (dragging a thumb is not a click).
    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        (self.has_scrollbar() && self.scrollbar_rect().contains_point(pos))
            .then_some(Cursor::Arrow)
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let (start, end) = self.visible_range();
        let list = self.list_rect();
        let children = (start..end)
            .map(|index| {
                let row = &self.rows[index];
                let rect = self.row_rect(index);
                let offscreen = !rect.intersects(list);
                let rect = rect.intersect(list).unwrap_or(rect);
                // The renaming row publishes the session's text, so a screen
                // reader follows what is being typed.
                let renaming = self
                    .rename
                    .as_ref()
                    .is_some_and(|rename| rename.path == row.path);
                let label = if renaming {
                    self.field.borrow().text().to_string()
                } else {
                    self.item(&row.path)
                        .map(|item| item.label.clone())
                        .unwrap_or_default()
                };
                let mut child = A11yChild::new(index, label, rect)
                    .selected(index == self.selected)
                    .enabled(!row.disabled)
                    .offscreen(offscreen);
                if renaming {
                    // The row being renamed carries the draft as an editable
                    // value, so a client can read and replace the whole label
                    // (the same commit Enter performs, D102).
                    child = child.value(self.field.borrow().text().to_string()).editable(true);
                }
                if row.expandable {
                    child = child.expanded(row.expanded);
                }
                child
            })
            .collect();
        Some(
            A11yInfo::new(A11yRole::Tree, self.id.0)
                .children(children)
                .scroll(A11yScroll::new().vertical(
                    self.scroll,
                    self.list_rect().height(),
                    self.content_height(),
                )),
        )
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        self.select(index);
        None
    }

    /// A UIA expand/collapse request carries the published row index; a leaf
    /// row ignores it. The message matches the chevron/Right/Left path.
    fn a11y_expand(&mut self, child: Option<usize>, expanded: bool) -> Option<M> {
        self.toggle_index(child?, expanded)
    }

    /// A UIA Value `SetValue` on the renaming row replaces the whole label and
    /// commits it (the Enter path). Rows that are not being renamed are
    /// read-only, and an empty value is ignored like an empty commit.
    fn a11y_set_value_at(&mut self, child: Option<usize>, text: &str) -> Option<M> {
        let index = child?;
        let path = self.rename.as_ref()?.path.clone();
        let row = self.rows.get(index)?;
        if row.path != path || row.disabled {
            return None;
        }
        self.field.get_mut().set_text(text.to_string());
        self.finish_rename(true)
    }

    fn a11y_scroll(&mut self, _horizontal: Option<f32>, vertical: Option<f32>) -> bool {
        let Some(target) = vertical else {
            return false;
        };
        let before = self.scroll;
        self.scroll = target.clamp(0.0, self.max_scroll());
        (self.scroll - before).abs() > 0.01
    }

    fn tooltip_rect(&self) -> Rect {
        self.selected_row_rect().unwrap_or(self.rect)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.row_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        (!self.rows.is_empty()).then_some(self.selected.min(self.rows.len() - 1))
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let pal = ctx.palette;
        if self.rows.is_empty() {
            if !self.empty_text.is_empty() {
                ctx.text(
                    self.rect,
                    &self.empty_text,
                    TextStyleRole::Small,
                    TextAlign::Center,
                    pal.muted,
                );
            }
            return;
        }
        let (start, end) = self.visible_range();
        ctx.canvas.push_clip(self.list_rect());
        for index in start..end {
            let row = &self.rows[index];
            let row_rect = self.row_rect(index);
            if !row_rect.intersects(self.list_rect()) {
                continue;
            }
            if state.pressed && self.press_row == Some(index) {
                ctx.canvas.fill_rect(row_rect, pal.row_pressed);
            } else if (state.hovered && self.hover_row == Some(index))
                || (state.focused && index == self.selected)
            {
                ctx.canvas.fill_rect(row_rect, pal.row_hover);
            }
            if row.expandable {
                let chevron = self.chevron_rect(row_rect, row);
                let mark = if row.expanded {
                    self.chevrons.1
                } else {
                    self.chevrons.0
                };
                let mut buffer = [0u8; 4];
                ctx.text(
                    chevron,
                    mark.encode_utf8(&mut buffer),
                    TextStyleRole::Glyph,
                    TextAlign::Center,
                    if row.disabled {
                        pal.text_disabled
                    } else {
                        pal.muted
                    },
                );
            }
            let color = if row.disabled {
                pal.text_disabled
            } else {
                pal.text
            };
            let item = self.item(&row.path);
            if let Some(glyph) = item.and_then(|item| item.glyph) {
                let mut buffer = [0u8; 4];
                ctx.text(
                    self.glyph_rect(row_rect, row),
                    glyph.encode_utf8(&mut buffer),
                    TextStyleRole::Glyph,
                    TextAlign::Center,
                    if row.disabled {
                        pal.text_disabled
                    } else {
                        pal.muted
                    },
                );
            }
            let label = item.map(|item| item.label.as_str()).unwrap_or("");
            if self
                .rename
                .as_ref()
                .is_some_and(|rename| rename.path == row.path)
            {
                self.paint_rename(ctx, state, index);
            } else {
                ctx.text_overflow(
                    self.label_rect(row_rect, row),
                    label,
                    TextStyleRole::Body,
                    TextAlign::Left,
                    color,
                    TextOverflow::Ellipsis,
                );
            }
            if let Some(secondary) = item
                .map(|item| item.secondary.as_str())
                .filter(|secondary| !secondary.is_empty())
            {
                ctx.text_overflow(
                    self.secondary_rect(row_rect),
                    secondary,
                    TextStyleRole::Small,
                    TextAlign::Right,
                    if row.disabled {
                        pal.text_disabled
                    } else {
                        pal.muted
                    },
                    TextOverflow::Ellipsis,
                );
            }
        }
        ctx.canvas.pop_clip();

        // Drop hint: a line between siblings, or an outline for a move inside.
        if let Some(drag) = &self.drag {
            if drag.hint.intersects(self.list_rect()) {
                if drag.inside {
                    paint_border(ctx, drag.hint, pal.accent);
                } else {
                    ctx.canvas.fill_rect(drag.hint, pal.accent);
                }
            }
        }

        if self.has_scrollbar() {
            scrollbar::paint(
                ctx,
                self.scrollbar_rect(),
                self.thumb_rect(),
                &self.bar,
                &self.style,
            );
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        match input {
            WidgetInput::HoverAt(pos) => {
                let over_bar = self.has_scrollbar()
                    && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some();
                let bar_before = self.bar.hovered;
                let rows_before = self.hover_row;
                if over_bar {
                    self.bar.hovered = true;
                    self.hover_row = None;
                } else {
                    self.bar.hovered = false;
                    self.hover_row = self.row_at(pos);
                }
                if self.bar.hovered != bar_before && self.hover_row == rows_before {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::HoverLeave => {
                let bar_before = self.bar.hovered;
                let rows_before = self.hover_row;
                self.hover_row = None;
                if !self.bar.active {
                    self.bar.hovered = false;
                }
                if self.bar.hovered != bar_before && self.hover_row == rows_before {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::PressAt(pos) => {
                // A press inside the open rename field keeps the session (the
                // container places the caret through `begin_inline_edit`).
                if self
                    .rename
                    .as_ref()
                    .is_some_and(|rename| self.row_at(pos).is_some_and(|row| self.rows[row].path == rename.path))
                {
                    self.field.get_mut().input(WidgetInput::PressAt(pos));
                    return None;
                }
                if self.has_scrollbar() {
                    let track = self.scrollbar_rect();
                    let thumb = self.thumb_rect();
                    match scrollbar::hit(track, thumb, pos) {
                        Some(BarHit::Thumb) => {
                            if let Some(thumb) = thumb {
                                self.bar.active = true;
                                self.bar.hovered = true;
                                self.bar.grab = pos.y - thumb.top;
                                self.repaint_hint = Some(self.scrollbar_rect());
                                return None;
                            }
                        }
                        Some(BarHit::Track) => {
                            if let Some(thumb) = thumb {
                                self.scroll = scrollbar::track_click_offset(
                                    pos,
                                    track,
                                    thumb,
                                    self.content_height(),
                                    self.rect.height(),
                                    self.scroll,
                                    true,
                                    &self.style,
                                );
                                self.clamp_scroll();
                                self.bar.active = true;
                                self.bar.hovered = true;
                                self.bar.grab = thumb.height() * 0.5;
                                return None;
                            }
                        }
                        None => {}
                    }
                }
                self.press_pos = Some(pos);
                // A fresh press clears the previous gesture's veto/offer.
                self.drag_denied = false;
                self.drag_offer = None;
                match self.row_part_at(pos) {
                    Some((index, chevron)) => {
                        self.press_row = Some(index);
                        self.press_chevron = chevron;
                    }
                    None => {
                        self.press_row = None;
                        self.press_chevron = false;
                    }
                }
                None
            }
            WidgetInput::DragTo(pos) => {
                if self.bar.active {
                    self.drag_thumb(pos, self.bar.grab);
                    return None;
                }
                // A drag inside the open rename field extends its selection.
                if self
                    .rename
                    .as_ref()
                    .is_some_and(|rename| self.row_at(pos).is_some_and(|row| self.rows[row].path == rename.path))
                {
                    self.field.get_mut().input(WidgetInput::DragTo(pos));
                    return None;
                }
                // A press that travels far enough on a row starts a move; the
                // drop slot is the row part under the pointer. The gesture can
                // be vetoed once by `on_drag_start`, and `draggable(false)`
                // never starts one.
                if self.drag.is_none()
                    && !self.drag_denied
                    && self.draggable
                    && let Some(pressed) = self.press_row
                    && !self.press_chevron
                    && let Some(start) = self.press_pos
                    && (pos.y - start.y).abs() >= DRAG_THRESHOLD
                    && !self.rows[pressed].disabled
                {
                    let path = self.rows[pressed].path.clone();
                    let allowed = self
                        .on_drag_start
                        .as_ref()
                        .is_none_or(|handler| handler(&path));
                    if allowed {
                        self.drag = Some(TreeDrag {
                            from: path,
                            parent: Vec::new(),
                            index: 0,
                            inside: false,
                            target_row: pressed,
                            hint: Rect::default(),
                            valid: false,
                        });
                    } else {
                        // One veto per gesture: motion no longer starts a
                        // drag until the next press.
                        self.drag_denied = true;
                    }
                }
                if let Some(from) = self.drag.as_ref().map(|drag| drag.from.clone()) {
                    self.drag_autoscroll(pos);
                    let mut accepted = self
                        .drop_slot(pos)
                        .filter(|slot| self.can_drop(&from, slot));
                    if let Some(mut slot) = accepted.take() {
                        slot.from = from.clone();
                        // The live filter sees each prospective path once.
                        let path = Self::slot_path(&from, &slot);
                        let allowed = match self.drag_offer.as_ref() {
                            Some((last, allowed)) if *last == path => *allowed,
                            _ => {
                                let allowed = self
                                    .on_drag_over
                                    .as_ref()
                                    .is_none_or(|handler| handler(&from, &path));
                                self.drag_offer = Some((path, allowed));
                                allowed
                            }
                        };
                        slot.valid = allowed;
                        if !allowed {
                            slot.hint = Rect::default();
                        }
                        self.drag = Some(slot);
                    } else if let Some(drag) = self.drag.as_mut() {
                        // No legal target here: no hint, and a release here is
                        // not a move.
                        drag.valid = false;
                        drag.hint = Rect::default();
                    }
                }
                None
            }
            WidgetInput::ReleaseAt(pos) => {
                self.drag_denied = false;
                self.drag_offer = None;
                if self.bar.active {
                    self.bar.active = false;
                    self.repaint_hint = Some(self.scrollbar_rect());
                    return None;
                }
                if let Some(drag) = self.drag.take() {
                    self.press_row = None;
                    self.press_pos = None;
                    if !drag.valid {
                        return None;
                    }
                    // The release veto sees the same prospective path the
                    // live filter did; `false` cancels the drop.
                    if let Some(handler) = self.on_drag_end.as_ref() {
                        let to = Self::slot_path(&drag.from, &drag);
                        if !handler(&drag.from, &to) {
                            return None;
                        }
                    }
                    let message = self.apply_move(&drag.from, &drag.parent, drag.index);
                    return match message {
                        Some(to) => self
                            .on_move
                            .as_ref()
                            .map(|handler| handler(&drag.from, &to)),
                        None => None,
                    };
                }
                // A click outside the open rename commits it first.
                if self.rename.is_some() {
                    let inside = self.rename.as_ref().is_some_and(|rename| {
                        self.row_at(pos)
                            .is_some_and(|row| self.rows[row].path == rename.path)
                    });
                    if !inside {
                        let message = self.finish_rename(true);
                        self.press_row = None;
                        return message;
                    }
                    self.field.get_mut().input(WidgetInput::ReleaseAt(pos));
                    self.press_row = None;
                    self.press_pos = None;
                    return None;
                }
                let pressed = self.press_row.take();
                self.press_pos = None;
                let chevron = std::mem::replace(&mut self.press_chevron, false);
                let hit = self.row_part_at(pos);
                match (pressed, hit) {
                    (Some(pressed), Some((hit, hit_chevron))) if pressed == hit => {
                        if chevron && hit_chevron {
                            return self.toggle(hit);
                        }
                        self.select(hit)
                    }
                    _ => None,
                }
            }
            WidgetInput::ClickAt(pos) => {
                // Selection already ran on the release; a bar click changes
                // nothing and must not repaint the rows underneath it.
                if self.has_scrollbar()
                    && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some()
                {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::DoubleClickAt(pos) => {
                if self.rename.is_some() {
                    // A double click inside the open field selects a word.
                    let inside = self.rename.as_ref().is_some_and(|rename| {
                        self.row_at(pos)
                            .is_some_and(|row| self.rows[row].path == rename.path)
                    });
                    if inside {
                        self.field.get_mut().input(WidgetInput::DoubleClickAt(pos));
                        return None;
                    }
                    return self.finish_rename(true);
                }
                let (index, _) = self.row_part_at(pos)?;
                if self.rows.get(index).map(|row| row.disabled).unwrap_or(true) {
                    return None;
                }
                self.selected = index;
                self.ensure_selected_visible();
                self.on_activate
                    .as_ref()
                    .map(|handler| handler(&self.rows[index].path))
            }
            WidgetInput::Wheel(delta) => {
                // Editing pins the field to its row; the wheel never moves the
                // rows underneath it.
                if self.rename.is_some() {
                    self.field.get_mut().input(WidgetInput::Wheel(delta));
                    return None;
                }
                if self.max_scroll() <= 0.0 {
                    return None;
                }
                let step = self.row_height * scrollbar::wheel_lines();
                self.scroll -= delta * step;
                self.clamp_scroll();
                None
            }
            WidgetInput::Key(key) => {
                use crate::event::KeyCode;
                if self.rename.is_some() {
                    self.field.get_mut().key(key);
                    return None;
                }
                let page = self.page_rows();
                match key.code {
                    KeyCode::Up => self.move_cursor(-1),
                    KeyCode::Down => self.move_cursor(1),
                    KeyCode::PageUp => self.move_cursor(-(page as i32)),
                    KeyCode::PageDown => self.move_cursor(page as i32),
                    KeyCode::Home => self.cursor_to_edge(false),
                    KeyCode::End => self.cursor_to_edge(true),
                    KeyCode::Right => {
                        let row = self.rows.get(self.selected)?;
                        let (expandable, expanded, depth) =
                            (row.expandable, row.expanded, row.depth);
                        if expandable && !expanded {
                            return self.toggle_index(self.selected, true);
                        }
                        if expanded
                            && self
                                .rows
                                .get(self.selected + 1)
                                .is_some_and(|next| next.depth > depth)
                        {
                            self.selected += 1;
                            self.ensure_selected_visible();
                        }
                    }
                    KeyCode::Left => {
                        let row = self.rows.get(self.selected)?;
                        if row.expandable && row.expanded {
                            return self.toggle_index(self.selected, false);
                        }
                        let parent = self.rows[..self.selected]
                            .iter()
                            .rposition(|candidate| candidate.depth < row.depth);
                        if let Some(index) = parent {
                            self.selected = index;
                            self.ensure_selected_visible();
                        }
                    }
                    KeyCode::Enter => return self.activate(),
                    KeyCode::Space => return self.toggle(self.selected),
                    KeyCode::Function(2) => {
                        let path = self.rows.get(self.selected)?.path.clone();
                        self.begin_rename(&path);
                    }
                    _ => {}
                }
                None
            }
            WidgetInput::Activate => self.activate(),
            _ => None,
        }
    }
}

/// One-pixel rectangle border, kept inside `rect`.
fn paint_border(ctx: &mut WidgetPaint<'_>, rect: Rect, color: crate::Color) {
    if rect.is_empty() {
        return;
    }
    let right = (rect.left + 1.0).min(rect.right);
    let bottom = (rect.top + 1.0).min(rect.bottom);
    ctx.canvas
        .fill_rect(Rect::new(rect.left, rect.top, rect.right, bottom), color);
    ctx.canvas.fill_rect(
        Rect::new(rect.left, rect.bottom - 1.0, rect.right, rect.bottom),
        color,
    );
    ctx.canvas
        .fill_rect(Rect::new(rect.left, rect.top, right, rect.bottom), color);
    ctx.canvas.fill_rect(
        Rect::new(rect.right - 1.0, rect.top, rect.right, rect.bottom),
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::KeyCode;

    fn key(code: KeyCode) -> WidgetInput {
        WidgetInput::Key(crate::event::KeyEvent {
            code,
            vk: 0,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        })
    }

    fn sample() -> Vec<TreeItem> {
        vec![
            TreeItem::new("音乐库")
                .expanded(true)
                .child(
                    TreeItem::new("专辑")
                        .expanded(true)
                        .child(TreeItem::new("夜曲"))
                        .child(TreeItem::new("竹韵")),
                )
                .child(TreeItem::new("歌手"))
                .child(TreeItem::new("播放列表").disabled(true)),
            TreeItem::new("下载"),
        ]
    }

    fn view() -> TreeView<Vec<usize>> {
        let mut view: TreeView<Vec<usize>> = TreeView::new("tree")
            .row_height(28.0)
            .on_select(|path| path.to_vec())
            .on_activate(|path| path.to_vec())
            .on_toggle(|path, expanded| {
                let mut value = path.to_vec();
                value.push(if expanded { 1 } else { 0 });
                value
            });
        view.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 140.0));
        view.set_items(sample());
        view
    }

    #[test]
    fn flattening_contains_only_expanded_nodes() {
        let view = view();
        let labels: Vec<String> = (0..view.visible_row_count())
            .map(|row| view.row_label(row).unwrap().to_string())
            .collect();
        assert_eq!(
            labels,
            ["音乐库", "专辑", "夜曲", "竹韵", "歌手", "播放列表", "下载"]
        );
        assert_eq!(view.rows[2].depth, 2);
        assert_eq!(view.selected_path(), Some(vec![0]));
        // Collapsing 专辑 hides its two children but keeps the siblings.
        let mut view = view;
        assert!(view.set_expanded(&[0, 0], false));
        assert_eq!(view.visible_row_count(), 5);
        assert_eq!(view.row_label(2), Some("歌手"));
        assert!(!view.set_expanded(&[0, 0], false), "collapse is idempotent");
        assert!(!view.set_expanded(&[0, 1], true), "a leaf cannot expand");
    }

    #[test]
    fn chevron_click_toggles_and_row_click_selects() {
        let mut view = view();
        let row = &view.rows[1];
        let chevron = view.chevron_rect(view.row_rect(1), row);
        let point = chevron.center();
        view.input(WidgetInput::PressAt(point));
        assert_eq!(
            view.input(WidgetInput::ReleaseAt(point)),
            Some(vec![0, 0, 0]),
            "collapse emits the toggle message"
        );
        assert_eq!(view.visible_row_count(), 5);
        // The same chevron expands it again.
        view.input(WidgetInput::PressAt(point));
        assert_eq!(
            view.input(WidgetInput::ReleaseAt(point)),
            Some(vec![0, 0, 1])
        );

        // A click on the label selects without toggling.
        let row = &view.rows[1];
        let label = view.label_rect(view.row_rect(1), row);
        let point = label.center();
        view.input(WidgetInput::PressAt(point));
        assert_eq!(view.input(WidgetInput::ReleaseAt(point)), Some(vec![0, 0]));
        assert_eq!(view.selected_path(), Some(vec![0, 0]));
        assert!(view.is_expanded(&[0, 0]));
    }

    #[test]
    fn glyphs_and_secondary_text_shift_the_label_columns() {
        let mut view: TreeView<()> = TreeView::new("tree").row_height(24.0);
        view.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 120.0));
        view.set_items(vec![
            TreeItem::new("文库")
                .glyph('\u{E8B7}')
                .child(TreeItem::new("子项")),
            TreeItem::new("普通"),
            TreeItem::new("曲目").glyph('\u{E8D6}').secondary("3 首"),
        ]);
        let plain = view.rows[1].clone();
        let decorated = view.rows[2].clone();
        let plain_label = view.label_rect(view.row_rect(1), &plain);
        let decorated_label = view.label_rect(view.row_rect(2), &decorated);
        assert_eq!(decorated_label.left - plain_label.left, GLYPH_WIDTH);
        assert_eq!(plain_label.right - decorated_label.right, SECONDARY_WIDTH);
        let glyph = view.glyph_rect(view.row_rect(2), &decorated);
        assert_eq!(glyph.width(), GLYPH_WIDTH);
        assert!(glyph.right <= decorated_label.left);
        let secondary = view.secondary_rect(view.row_rect(2));
        assert_eq!(secondary.width(), SECONDARY_WIDTH);
        assert_eq!(secondary.right, decorated_label.right + SECONDARY_WIDTH);

        // The chevron column is unaffected by a glyph: clicking it still
        // toggles the parent.
        let root = view.rows[0].clone();
        let point = view
            .chevron_rect(view.row_rect(0), &root)
            .center();
        view.input(WidgetInput::PressAt(point));
        view.input(WidgetInput::ReleaseAt(point));
        assert!(view.is_expanded(&[0]));
    }

    #[test]
    fn disabled_rows_do_not_select_or_activate() {
        let mut view = view();
        let row = &view.rows[5];
        let point = view.label_rect(view.row_rect(5), row).center();
        view.input(WidgetInput::PressAt(point));
        assert_eq!(view.input(WidgetInput::ReleaseAt(point)), None);
        assert_eq!(view.selected_path(), Some(vec![0]));
        assert_eq!(view.input(WidgetInput::DoubleClickAt(point)), None);
        assert_eq!(view.input(WidgetInput::Activate), Some(vec![0]));
    }

    #[test]
    fn keyboard_walks_children_and_parents_and_toggles() {
        let mut view = view();
        // Start at 音乐库 (expanded) -> Right moves into 专辑.
        view.input(key(KeyCode::Right));
        assert_eq!(view.selected_path(), Some(vec![0, 0]));
        assert!(view.is_expanded(&[0, 0]));
        // 专辑 is expanded -> Right moves to its first child.
        view.input(key(KeyCode::Right));
        assert_eq!(view.selected_path(), Some(vec![0, 0, 0]));
        // Left on a leaf walks to the parent.
        view.input(key(KeyCode::Left));
        assert_eq!(view.selected_path(), Some(vec![0, 0]));
        // Left on an expanded node collapses it.
        view.input(key(KeyCode::Left));
        assert!(!view.is_expanded(&[0, 0]));
        assert_eq!(view.selected_path(), Some(vec![0, 0]));
        // Left from the collapsed child walks to 音乐库.
        view.input(key(KeyCode::Left));
        assert_eq!(view.selected_path(), Some(vec![0]));
        // Down visits the collapsed node itself, then the next sibling.
        view.input(key(KeyCode::Down));
        assert_eq!(view.selected_path(), Some(vec![0, 0]));
        view.input(key(KeyCode::Down));
        assert_eq!(view.selected_path(), Some(vec![0, 1]));
        // Arrow movement skips the disabled row.
        view.input(key(KeyCode::Down));
        assert_eq!(view.selected_path(), Some(vec![1]));
        view.input(key(KeyCode::Home));
        assert_eq!(view.selected_path(), Some(vec![0]));
        view.input(key(KeyCode::End));
        assert_eq!(view.selected_path(), Some(vec![1]));
    }

    #[test]
    fn collapsing_a_parent_moves_the_cursor_to_it() {
        let mut view = view();
        assert!(view.set_selection_path(&[0, 0, 1]));
        assert_eq!(view.row_label(view.selected_row()), Some("竹韵"));
        view.input(key(KeyCode::Left));
        assert_eq!(view.selected_path(), Some(vec![0, 0]));
        // Space toggles the selected node and reports it.
        assert_eq!(view.input(key(KeyCode::Space)), Some(vec![0, 0, 0]));
    }

    #[test]
    fn selection_is_ignored_for_collapsed_or_unknown_paths() {
        let mut view = view();
        assert!(!view.set_selection_path(&[9]));
        assert!(!view.set_selection_path(&[0, 0, 0, 0]));
        assert!(view.set_selection_path(&[0, 0, 0]));
        assert_eq!(view.selected_path(), Some(vec![0, 0, 0]));
        view.set_expanded(&[0], false);
        // The cursor climbs to the surviving ancestor.
        assert_eq!(view.selected_path(), Some(vec![0]));
    }

    #[test]
    fn scrolling_publishes_the_look_ahead_row_offscreen() {
        let view = view();
        // 5 rows of 28 DIPs fit in 140 plus the look-ahead row.
        let (start, end) = view.visible_rows();
        assert_eq!(start, 0);
        assert_eq!(end, 6);
        let info = view.a11y().expect("tree is exposed");
        assert_eq!(info.role, A11yRole::Tree);
        assert_eq!(info.children.len(), 6);
        assert_eq!(info.children[2].name, "夜曲");
        assert!(info.children.last().is_some_and(|child| child.offscreen));
        // Only expandable rows publish an open state; a leaf stays `None`.
        assert_eq!(info.children[0].expanded, Some(true));
        assert_eq!(info.children[2].expanded, None);
        assert_eq!(
            info.children.iter().filter(|child| child.selected).count(),
            1
        );
    }

    #[test]
    fn uia_expand_and_collapse_toggle_the_published_row() {
        let mut view = view();
        let before = view.visible_row_count();
        // Collapse 音乐库 (row 0) through the a11y seam: its children leave
        // the flat list and the same handler a chevron click uses fires.
        assert_eq!(view.a11y_expand(Some(0), false), Some(vec![0, 0]));
        assert!(!view.is_expanded(&[0]));
        assert!(view.visible_row_count() < before);
        assert!(view.a11y_expand(Some(0), false).is_none(), "idempotent");
        // Expand it again.
        assert_eq!(view.a11y_expand(Some(0), true), Some(vec![0, 1]));
        assert!(view.is_expanded(&[0]));
        // A leaf and an out-of-range row ignore the request.
        assert!(view.a11y_expand(Some(view.visible_row_count() - 1), true).is_none());
        assert!(view.a11y_expand(Some(usize::MAX), true).is_none());
        assert!(view.a11y_expand(None, true).is_none());
    }

    #[test]
    fn the_renaming_row_publishes_an_editable_value_that_set_value_commits() {
        let mut view: TreeView<(Vec<usize>, String)> = TreeView::new("tree")
            .row_height(28.0)
            .renameable(true)
            .on_rename(|path, label| (path.to_vec(), label.to_string()));
        view.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 200.0));
        view.set_items(sample());
        assert!(view.begin_rename(&[0, 0, 0]));
        let info = view.a11y().unwrap();
        let editing = info
            .children
            .iter()
            .find(|child| child.value.is_some())
            .expect("the renaming row publishes a value");
        assert_eq!(editing.value.as_deref(), Some("夜曲"));
        assert!(!editing.read_only, "SetValue must be accepted while renaming");
        assert!(
            info.children
                .iter()
                .filter(|child| child.index != editing.index)
                .all(|child| child.value.is_none()),
            "only the renaming row carries a value"
        );
        // A row that is not renaming refuses.
        let other = info
            .children
            .iter()
            .find(|child| child.index != editing.index)
            .map(|child| child.index)
            .unwrap();
        assert!(view.a11y_set_value_at(Some(other), "x").is_none());
        // SetValue applies the label like Enter does.
        let committed = view.a11y_set_value_at(Some(editing.index), "新月");
        assert_eq!(committed, Some((vec![0, 0, 0], "新月".to_string())));
        assert!(!view.is_renaming());
        assert_eq!(
            view.item(&[0, 0, 0]).map(|item| item.label.as_str()),
            Some("新月")
        );
        // With no session open every row is read-only again.
        assert!(view.a11y_set_value_at(Some(0), "x").is_none());
    }

    #[test]
    fn a_label_with_a_newline_renames_with_the_multiline_field() {
        let mut view: TreeView<(Vec<usize>, String)> = TreeView::new("tree")
            .row_height(28.0)
            .renameable(true)
            .on_rename(|path, label| (path.to_vec(), label.to_string()));
        view.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 200.0));
        view.set_items(vec![TreeItem::new("一\n二")]);
        assert!(view.begin_rename(&[0]));
        assert!(
            view.field.borrow().multiline(),
            "a label with a newline uses the multiline model"
        );
        assert!(view.insert_text("甲\n乙"));
        view.input(key(KeyCode::Enter));
        assert!(view.is_renaming(), "a plain Enter inserts a line, not a commit");
        assert_eq!(
            view.finish_edit(true),
            Some((vec![0], "甲\n乙\n".to_string()))
        );
        assert_eq!(
            view.item(&[0]).map(|item| item.label.as_str()),
            Some("甲\n乙\n")
        );
    }

    #[test]
    fn f2_opens_a_rename_session_and_enter_commits_it() {
        let mut view: TreeView<(Vec<usize>, String)> = TreeView::new("tree")
            .row_height(28.0)
            .renameable(true)
            .on_rename(|path, label| (path.to_vec(), label.to_string()));
        view.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 200.0));
        view.set_items(sample());
        assert!(view.begin_rename(&[0, 0, 0]));
        assert!(view.is_renaming());
        assert_eq!(view.renaming_path(), Some([0, 0, 0].as_slice()));
        // The label starts fully selected, so typing replaces it.
        assert!(view.insert_text("新曲"));
        assert_eq!(
            view.finish_edit(true),
            Some((vec![0, 0, 0], "新曲".to_string()))
        );
        assert!(!view.is_renaming());
        assert_eq!(
            view.item(&[0, 0, 0]).map(|item| item.label.as_str()),
            Some("新曲")
        );
        // The keyboard chord opens it on the cursor row.
        view.set_selection_path(&[0, 0, 1]);
        view.input(key(KeyCode::Function(2)));
        assert!(view.is_renaming());
        view.field.get_mut().select_all();
        // Escape reverts.
        assert!(view.insert_text("草稿"));
        assert_eq!(view.finish_edit(false), None);
        assert_eq!(
            view.item(&[0, 0, 1]).map(|item| item.label.as_str()),
            Some("竹韵")
        );
        // An empty commit is ignored.
        assert!(view.begin_rename(&[0, 0, 0]));
        view.field.get_mut().set_text(String::new());
        assert_eq!(view.finish_edit(true), None);
        assert_eq!(
            view.item(&[0, 0, 0]).map(|item| item.label.as_str()),
            Some("新曲")
        );
        // A disabled row and a read-only tree refuse.
        assert!(!view.begin_rename(&[0, 2]));
        view.set_read_only(true);
        assert!(!view.begin_rename(&[0, 0, 0]));
    }

    #[test]
    fn dragging_a_row_into_another_parent_reparents_it() {
        let mut view: TreeView<(Vec<usize>, Vec<usize>)> = TreeView::new("tree")
            .row_height(28.0)
            .on_move(|from, to| (from.to_vec(), to.to_vec()));
        view.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 260.0));
        view.set_items(sample());
        let from = view.label_rect(view.row_rect(4), &view.rows[4]).center();
        let target_row = view.row_rect(6);
        let to = Point::new(target_row.center().x, target_row.center().y);
        view.input(WidgetInput::PressAt(from));
        view.input(WidgetInput::DragTo(to));
        assert!(view.drag.is_some(), "travel past the threshold starts a drag");
        assert_eq!(
            view.input(WidgetInput::ReleaseAt(to)),
            Some((vec![0, 1], vec![1, 0])),
            "the application is told the node's new path"
        );
        assert!(view.drag.is_none());
        // The widget applied the move: 歌手 is now 下载's child and selected.
        assert_eq!(
            view.item(&[1, 0]).map(|item| item.label.as_str()),
            Some("歌手")
        );
        assert_eq!(view.selected_path(), Some(vec![1, 0]));
        assert!(view.is_expanded(&[1]));
    }

    #[test]
    fn detach_then_reattach_a_child_leaves_no_ghost_row() {
        // Both drop sides of the detach: before the sibling (which shifts its
        // path) and after it. The re-attach must still address the intended
        // parent and never lose the node.
        for drop_before in [true, false] {
            let mut items = sample();
            items[1] = TreeItem::new("下载").child(TreeItem::new("正在下载"));
            let mut view: TreeView<(Vec<usize>, Vec<usize>)> = TreeView::new("tree")
                .row_height(28.0)
                .on_move(|from, to| (from.to_vec(), to.to_vec()));
            view.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 260.0));
            view.set_items(items);
            view.set_expanded(&[1], true);
            // Step 1: detach 正在下载 to the root level, next to 下载.
            let child = view.label_rect(view.row_rect(7), &view.rows[7]).center();
            let download = view.row_rect(6);
            let sibling = Point::new(
                download.center().x,
                if drop_before {
                    download.top + 2.0
                } else {
                    download.bottom - 2.0
                },
            );
            view.input(WidgetInput::PressAt(child));
            view.input(WidgetInput::DragTo(sibling));
            assert!(view.input(WidgetInput::ReleaseAt(sibling)).is_some());
            let labels = |view: &TreeView<(Vec<usize>, Vec<usize>)>| {
                (0..view.visible_row_count())
                    .map(|index| view.row_label(index).unwrap_or("?").to_string())
                    .collect::<Vec<_>>()
            };
            let detached: &[&str] = if drop_before {
                &["音乐库", "专辑", "夜曲", "竹韵", "歌手", "播放列表", "正在下载", "下载"]
            } else {
                &["音乐库", "专辑", "夜曲", "竹韵", "歌手", "播放列表", "下载", "正在下载"]
            };
            assert_eq!(labels(&view), detached, "the detach keeps every node");
            // Step 2: re-attach it to 下载's middle. With the `drop_before`
            // variant 下载 is now the *last* row, and the node removed before
            // it is what used to shift the target index out of range — the
            // reported "ghost row" case.
            let moved_row = (0..view.visible_row_count())
                .find(|index| view.row_label(*index) == Some("正在下载"))
                .expect("the moved row");
            let target_row = (0..view.visible_row_count())
                .find(|index| view.row_label(*index) == Some("下载"))
                .expect("the target row");
            let moved = view
                .label_rect(view.row_rect(moved_row), &view.rows[moved_row])
                .center();
            let target = view.row_rect(target_row);
            let middle = Point::new(target.center().x, target.center().y);
            view.input(WidgetInput::PressAt(moved));
            view.input(WidgetInput::DragTo(middle));
            assert!(view.input(WidgetInput::ReleaseAt(middle)).is_some());
            // 下载 is a child-bearing expanded row again and the tree has no
            // empty (ghost) row.
            let labels = labels(&view);
            assert_eq!(
                labels,
                ["音乐库", "专辑", "夜曲", "竹韵", "歌手", "播放列表", "下载", "正在下载"],
                "nothing lost, no ghost row (drop_before={drop_before})"
            );
            assert!(view.rows.iter().all(|row| view
                .item(&row.path)
                .is_some_and(|item| !item.label.is_empty())),
                "every cached row resolves to a labelled node");
            assert!(view.rows.iter().find(|row| row.path == [1]).is_some_and(|row| row.expandable));
            assert_eq!(view.item(&[1, 0]).map(|item| item.label.as_str()), Some("正在下载"));
            assert!(view.is_expanded(&[1]));
        }
    }

    fn drag_row(
        view: &mut TreeView<(Vec<usize>, Vec<usize>)>,
        start_row: usize,
        end: Point,
    ) -> Option<(Vec<usize>, Vec<usize>)> {
        let start = view
            .label_rect(view.row_rect(start_row), &view.rows[start_row])
            .center();
        view.input(WidgetInput::PressAt(start));
        view.input(WidgetInput::DragTo(end));
        view.input(WidgetInput::ReleaseAt(end))
    }

    fn drag_view(rules: TreeDragRules) -> TreeView<(Vec<usize>, Vec<usize>)> {
        let mut view: TreeView<(Vec<usize>, Vec<usize>)> = TreeView::new("tree")
            .row_height(28.0)
            .drag_rules(rules)
            .on_move(|from, to| (from.to_vec(), to.to_vec()));
        view.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 260.0));
        view.set_items(sample());
        view
    }

    #[test]
    fn drag_rules_restrict_the_drop_kinds() {
        // Reorder: 夜曲 before 竹韵 (same parent). Nest: 歌手 inside 下载.
        // Detach: 歌手 to the root after 下载. Reparent: 竹韵 under 音乐库.
        let reorder_end = {
            let view = drag_view(TreeDragRules::new());
            let row = view.row_rect(3);
            Point::new(row.center().x, row.bottom - 2.0)
        };
        let nest_end = {
            let view = drag_view(TreeDragRules::new());
            view.row_rect(6).center()
        };
        let detach_end = {
            let view = drag_view(TreeDragRules::new());
            let row = view.row_rect(6);
            Point::new(row.center().x, row.bottom - 2.0)
        };
        let reparent_end = {
            let view = drag_view(TreeDragRules::new());
            let row = view.row_rect(4);
            Point::new(row.center().x, row.bottom - 2.0)
        };

        // Default rules allow every kind.
        assert!(drag_row(&mut drag_view(TreeDragRules::new()), 2, reorder_end).is_some());
        assert!(drag_row(&mut drag_view(TreeDragRules::new()), 4, nest_end).is_some());
        assert!(drag_row(&mut drag_view(TreeDragRules::new()), 4, detach_end).is_some());
        assert!(drag_row(&mut drag_view(TreeDragRules::new()), 3, reparent_end).is_some());

        // Each disabled kind leaves the tree untouched.
        let mut view = drag_view(TreeDragRules::new().reorder(false));
        assert_eq!(drag_row(&mut view, 2, reorder_end), None);
        assert_eq!(view.row_label(2), Some("夜曲"));

        let mut view = drag_view(TreeDragRules::new().nest(false));
        assert_eq!(drag_row(&mut view, 4, nest_end), None);
        assert_eq!(view.item(&[1]).map(|item| item.children.len()), Some(0));

        let mut view = drag_view(TreeDragRules::new().detach(false));
        assert_eq!(drag_row(&mut view, 4, detach_end), None);
        assert_eq!(view.item(&[0, 1]).map(|item| item.label.as_str()), Some("歌手"));

        let mut view = drag_view(TreeDragRules::new().reparent(false));
        assert_eq!(drag_row(&mut view, 3, reparent_end), None);
        assert_eq!(view.item(&[0, 0, 1]).map(|item| item.label.as_str()), Some("竹韵"));

        // A disabled kind does not block the others.
        let mut view = drag_view(TreeDragRules::new().reorder(false));
        assert!(drag_row(&mut view, 4, nest_end).is_some());
        assert_eq!(view.item(&[1, 0]).map(|item| item.label.as_str()), Some("歌手"));
    }

    #[test]
    fn dragging_can_be_turned_off_and_vetoed_per_gesture() {
        let nest_end = {
            let view = drag_view(TreeDragRules::new());
            view.row_rect(6).center()
        };
        let mut view = drag_view(TreeDragRules::new()).draggable(false);
        let start = view
            .label_rect(view.row_rect(4), &view.rows[4])
            .center();
        view.input(WidgetInput::PressAt(start));
        view.input(WidgetInput::DragTo(nest_end));
        assert!(view.drag.is_none(), "a non-draggable tree never starts a drag");
        assert_eq!(view.input(WidgetInput::ReleaseAt(nest_end)), None);
        assert!(view.set_draggable(true));

        // A per-gesture veto: 歌手 cannot be dragged, 夜曲 can.
        let mut view = drag_view(TreeDragRules::new())
            .on_drag_start(|path| path != [0, 1]);
        let vetoed = drag_row(&mut view, 4, nest_end);
        assert_eq!(vetoed, None);
        assert_eq!(view.item(&[0, 1]).map(|item| item.label.as_str()), Some("歌手"));
        // The veto lasts for that gesture only; a new press may drag another
        // row (夜曲 into 下载).
        let mut view = drag_view(TreeDragRules::new()).on_drag_start(|path| path != [0, 1]);
        let moved = drag_row(&mut view, 2, nest_end);
        assert_eq!(moved, Some((vec![0, 0, 0], vec![1, 0])));
    }

    #[test]
    fn the_live_drag_filter_sees_each_target_once_and_can_refuse_it() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let seen: Rc<RefCell<Vec<(Vec<usize>, Vec<usize>)>>> = Rc::default();
        let log = seen.clone();
        let mut view = drag_view(TreeDragRules::new()).on_drag_over(move |from, to| {
            log.borrow_mut().push((from.to_vec(), to.to_vec()));
            // Refuse becoming 下载's child.
            to != [1, 0]
        });
        let nest = view.row_rect(6).center();
        let start = view
            .label_rect(view.row_rect(4), &view.rows[4])
            .center();
        view.input(WidgetInput::PressAt(start));
        view.input(WidgetInput::DragTo(nest));
        // Staying on the same target does not ask again.
        view.input(WidgetInput::DragTo(nest));
        assert_eq!(view.input(WidgetInput::ReleaseAt(nest)), None);
        assert_eq!(view.item(&[1]).map(|item| item.children.len()), Some(0));
        assert_eq!(*seen.borrow(), vec![(vec![0, 1], vec![1, 0])]);

        // An accepted target still moves (a sibling reorder).
        let seen_ok = seen.clone();
        view = view.on_drag_over(move |from, to| {
            seen_ok.borrow_mut().push((from.to_vec(), to.to_vec()));
            true
        });
        let row = view.row_rect(3);
        let end = Point::new(row.center().x, row.bottom - 2.0);
        assert!(drag_row(&mut view, 2, end).is_some());
        assert_eq!(view.row_label(2), Some("竹韵"));
    }

    #[test]
    fn the_release_veto_can_cancel_a_valid_drop() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let seen: Rc<RefCell<Vec<(Vec<usize>, Vec<usize>)>>> = Rc::default();
        let log = seen.clone();
        let mut view = drag_view(TreeDragRules::new()).on_drag_end(move |from, to| {
            log.borrow_mut().push((from.to_vec(), to.to_vec()));
            false
        });
        let nest = view.row_rect(6).center();
        let moved = drag_row(&mut view, 4, nest);
        assert_eq!(moved, None);
        assert!(view.drag.is_none());
        assert_eq!(*seen.borrow(), vec![(vec![0, 1], vec![1, 0])]);
        assert_eq!(view.item(&[0, 1]).map(|item| item.label.as_str()), Some("歌手"));
        assert_eq!(view.item(&[1]).map(|item| item.children.len()), Some(0));

        // The same drop applies when the veto says yes.
        let mut view = drag_view(TreeDragRules::new()).on_drag_end(|_, _| true);
        let nest = view.row_rect(6).center();
        assert_eq!(
            drag_row(&mut view, 4, nest),
            Some((vec![0, 1], vec![1, 0]))
        );
        assert_eq!(view.item(&[1, 0]).map(|item| item.label.as_str()), Some("歌手"));
    }

    #[test]
    fn dragging_between_siblings_reorders_and_a_descendant_drop_is_refused() {
        let mut view: TreeView<(Vec<usize>, Vec<usize>)> = TreeView::new("tree")
            .row_height(28.0)
            .on_move(|from, to| (from.to_vec(), to.to_vec()));
        view.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 260.0));
        view.set_items(sample());
        // 夜曲 (first child of 专辑) dragged past 竹韵's lower edge.
        let start = view.label_rect(view.row_rect(2), &view.rows[2]).center();
        let third = view.row_rect(3);
        let end = Point::new(third.center().x, third.bottom - 2.0);
        view.input(WidgetInput::PressAt(start));
        view.input(WidgetInput::DragTo(end));
        assert_eq!(
            view.input(WidgetInput::ReleaseAt(end)),
            Some((vec![0, 0, 0], vec![0, 0, 1]))
        );
        assert_eq!(view.row_label(2), Some("竹韵"));
        assert_eq!(view.row_label(3), Some("夜曲"));

        // Dropping a parent inside its own child is refused (the drag keeps
        // no valid slot and the release reports nothing).
        let root = view.label_rect(view.row_rect(0), &view.rows[0]).center();
        let child = view.row_rect(2);
        let inside = Point::new(child.center().x, child.center().y);
        view.input(WidgetInput::PressAt(root));
        view.input(WidgetInput::DragTo(inside));
        assert_eq!(view.input(WidgetInput::ReleaseAt(inside)), None);
        assert_eq!(view.row_label(0), Some("音乐库"));
    }

    #[test]
    fn typed_move_and_rename_apply_without_a_pointer() {
        let mut view = view();
        assert!(view.move_item(&[0, 1], &[1], 0));
        assert_eq!(
            view.item(&[1, 0]).map(|item| item.label.as_str()),
            Some("歌手")
        );
        // Invalid moves are refused.
        assert!(!view.move_item(&[0], &[0, 0], 0), "into a descendant");
        assert!(!view.move_item(&[9], &[], 0), "unknown source");
        assert!(!view.move_item(&[0], &[9], 0), "unknown parent");
        assert!(view.rename(&[0], "音乐"));
        assert_eq!(view.item(&[0]).map(|item| item.label.as_str()), Some("音乐"));
        assert!(!view.rename(&[0], "音乐"));
    }

    #[test]
    fn wheel_and_typed_access_scroll_a_long_tree() {
        let items: Vec<TreeItem> = (0..100)
            .map(|index| TreeItem::new(format!("节点 {index}")))
            .collect();
        let mut view: TreeView<()> = TreeView::new("tree").row_height(20.0);
        view.set_rect(Rect::from_xywh(0.0, 0.0, 240.0, 100.0));
        view.set_items(items);
        assert_eq!(view.visible_row_count(), 100);
        view.input(WidgetInput::Wheel(-4.0));
        assert!(view.scroll_offset() > 0.0);
        view.input(WidgetInput::Wheel(-10_000.0));
        assert!((view.scroll_offset() - view.max_scroll()).abs() < 0.01);
        let track = view.scrollbar_rect();
        assert_eq!(view.cursor_at(track.center()), Some(Cursor::Arrow));
        assert_eq!(view.cursor_at(Point::new(10.0, 10.0)), None);
        assert!(view.scroll_to_row(0));
        assert!(view.ensure_row_visible(60));
        assert_eq!(
            view.item(&[60]).map(|item| item.label.as_str()),
            Some("节点 60")
        );
        assert!(view.item(&[100]).is_none());
        // Replacing the data keeps the cursor inside the new tree.
        assert!(view.set_items(vec![TreeItem::new("唯一")]));
        assert_eq!(view.visible_row_count(), 1);
        assert_eq!(view.selected_row(), 0);
        assert_eq!(view.scroll_offset(), 0.0);
    }

    #[test]
    fn the_node_child_list_replaces_the_hierarchy() {
        let mut view: TreeView<()> = TreeView::new("tree").row_height(20.0);
        view.set_rect(Rect::from_xywh(0.0, 0.0, 240.0, 200.0));
        let leaf = {
            let mut leaf = SkinElement::new("node", 4, 5);
            leaf.push_attr("label", "夜曲");
            leaf.push_attr("glyph", "♪");
            leaf.push_attr("secondary", "3:32");
            leaf
        };
        let mut root = SkinElement::new("node", 2, 3);
        root.push_attr("label", "音乐库");
        root.push_attr("expanded", "true");
        root.push_child(leaf);
        let children = [root];
        assert!(<TreeView<()> as Widget<()>>::validate_skin_children(&view, &children));
        <TreeView<()> as Widget<()>>::apply_skin_children(&mut view, &children);
        assert_eq!(view.items_ref().len(), 1);
        assert_eq!(view.items_ref()[0].label, "音乐库");
        assert!(view.items_ref()[0].expanded);
        assert_eq!(view.items_ref()[0].children.len(), 1);
        assert_eq!(view.items_ref()[0].children[0].label, "夜曲");
        assert_eq!(view.items_ref()[0].children[0].glyph, Some('♪'));
        assert_eq!(view.visible_row_count(), 2);

        // Nested nodes are validated recursively; a bad leaf rejects the tree.
        let mut bad_leaf = SkinElement::new("node", 6, 5);
        bad_leaf.push_attr("label", "无图标名");
        bad_leaf.push_attr("glyph", "ab");
        let mut bad_root = SkinElement::new("node", 5, 3);
        bad_root.push_attr("label", "音乐库");
        bad_root.push_child(bad_leaf);
        assert!(!<TreeView<()> as Widget<()>>::validate_skin_children(
            &view,
            &[bad_root]
        ));
    }
}
