// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Platform-neutral accessibility model.
//!
//! Widgets describe themselves once per snapshot ([`Widget::a11y`]), `Ui`
//! flattens that into a tree of [`A11yNode`]s, and the platform layer exposes
//! the tree through UI Automation. Strings are owned so a snapshot can outlive
//! the paint pass and cross into a screen reader.
//!
//! [`Widget::a11y`]: crate::widget::Widget::a11y

use crate::geometry::Rect;
use crate::widget::WidgetId;

/// How a transient announcement should interrupt a screen reader.
///
/// `Polite` keeps only the newest message (status ticks, repeated toasts);
/// `Assertive` interrupts the current utterance (errors, destructive actions).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnnouncementPriority {
    #[default]
    Polite,
    Assertive,
}

pub use bamboo_native_ui_layout_core::a11y::A11yRole;

/// One axis of a scrollable element's viewport, in DIPs: where the content
/// sits (`offset`), how much of it is visible (`viewport`) and how much there
/// is (`content`). UIA derives the Scroll pattern's percentages from it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct A11yScrollAxis {
    /// Current offset from the content origin.
    pub offset: f32,
    /// The visible extent (the viewport height/width).
    pub viewport: f32,
    /// The whole content extent.
    pub content: f32,
}

impl A11yScrollAxis {
    pub fn new(offset: f32, viewport: f32, content: f32) -> Self {
        Self {
            offset,
            viewport,
            content,
        }
    }

    /// How far the axis can still move.
    pub fn max_offset(&self) -> f32 {
        (self.content - self.viewport).max(0.0)
    }

    /// Whether there is anything to scroll. UIA reports `NoScroll` otherwise.
    pub fn scrollable(&self) -> bool {
        self.max_offset() > 0.5
    }

    /// Scroll position as a UIA percentage (`0..=100`), `None` when the axis
    /// cannot scroll.
    pub fn percent(&self) -> Option<f64> {
        let max = self.max_offset();
        (max > 0.5).then(|| (self.offset.clamp(0.0, max) / max * 100.0) as f64)
    }

    /// Visible fraction as a UIA percentage (`0..=100`), `None` when the axis
    /// cannot scroll.
    pub fn view_percent(&self) -> Option<f64> {
        (self.max_offset() > 0.5)
            .then(|| (self.viewport / self.content.max(1.0) * 100.0).clamp(0.0, 100.0) as f64)
    }

    /// Absolute offset a UIA percentage points at (clamped to the range).
    pub fn target(&self, percent: f64) -> f32 {
        (percent.clamp(0.0, 100.0) / 100.0) as f32 * self.max_offset()
    }
}

/// The Scroll pattern state of a scrollable element. An axis that cannot
/// scroll is still reported so a client learns it exists but is not
/// scrollable; leave it `None` when the element has no such axis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct A11yScroll {
    pub horizontal: Option<A11yScrollAxis>,
    pub vertical: Option<A11yScrollAxis>,
}

impl A11yScroll {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn horizontal(mut self, offset: f32, viewport: f32, content: f32) -> Self {
        self.horizontal = Some(A11yScrollAxis::new(offset, viewport, content));
        self
    }

    pub fn vertical(mut self, offset: f32, viewport: f32, content: f32) -> Self {
        self.vertical = Some(A11yScrollAxis::new(offset, viewport, content));
        self
    }

    pub fn is_empty(&self) -> bool {
        self.horizontal.is_none() && self.vertical.is_none()
    }

    /// Whether either axis can actually move. The UIA Scroll pattern is only
    /// advertised while something overflows.
    pub fn scrollable(&self) -> bool {
        self.horizontal.is_some_and(|axis| axis.scrollable())
            || self.vertical.is_some_and(|axis| axis.scrollable())
    }
}

/// One option / tab / visible row published by a widget.
#[derive(Clone, Debug)]
pub struct A11yChild {
    /// Widget-level index of the option/row (a list row keeps its absolute
    /// index while the viewport scrolls).
    pub index: usize,
    pub name: String,
    pub selected: bool,
    /// The item itself is disabled (a segmented option), independent of the
    /// container's state. UIA reports `IsEnabled` from this flag.
    pub enabled: bool,
    /// Client rectangle in DIPs. A virtualized list clips this to its viewport.
    pub rect: Rect,
    /// The element exists in the snapshot but is scrolled out of view (a list
    /// publishes one look-ahead row). UIA reports `IsOffscreen` from this flag.
    pub offscreen: bool,
    /// Checkmark state of a menu item (UIA `ToggleState`). `None` for elements
    /// that are not checkable.
    pub checked: Option<bool>,
    /// Open state of an expandable row (a tree node). `None` for rows that
    /// cannot expand; UIA reports those as `LeafNode`.
    pub expanded: Option<bool>,
    /// Explicit role. Unset falls back to the container role's child role;
    /// set it when one widget publishes two child kinds (a toolbar's buttons
    /// and its overflow menu items).
    pub role: Option<A11yRole>,
    /// Value a ValuePattern-capable child reports (a grid cell's text).
    pub value: Option<String>,
    /// Whether a child that exposes the Value pattern accepts `SetValue`.
    pub read_only: bool,
    /// Grid cell coordinates inside an [`A11yRole::Grid`] container.
    pub row: Option<usize>,
    pub column: Option<usize>,
}

impl A11yChild {
    pub fn new(index: usize, name: impl Into<String>, rect: Rect) -> Self {
        Self {
            index,
            name: name.into(),
            selected: false,
            enabled: true,
            rect,
            offscreen: false,
            checked: None,
            expanded: None,
            role: None,
            value: None,
            read_only: true,
            row: None,
            column: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Per-item enabled state (a disabled segmented option).
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn offscreen(mut self, offscreen: bool) -> Self {
        self.offscreen = offscreen;
        self
    }

    /// Checkmark state for a menu item.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Open state of an expandable row (a tree node). Only expandable rows
    /// carry it; a leaf row stays `None` and reports `LeafNode`.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Override the role the child would inherit from its container.
    pub fn role(mut self, role: A11yRole) -> Self {
        self.role = Some(role);
        self
    }

    /// Grid coordinates of a cell child (row, column), for IGridItemProvider.
    pub fn grid_cell(mut self, row: usize, column: usize) -> Self {
        self.row = Some(row);
        self.column = Some(column);
        self
    }

    /// Value reported through the child's ValuePattern (a cell's text).
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Whether the child accepts `SetValue` (default: read-only).
    pub fn editable(mut self, editable: bool) -> Self {
        self.read_only = !editable;
        self
    }
}

/// What a widget reports about itself.
#[derive(Clone, Debug)]
pub struct A11yInfo {
    pub role: A11yRole,
    /// Accessible name (label text, cue banner, or the widget id).
    pub name: String,
    /// Longer explanation (UIA HelpText / FullDescription). Narration
    /// overrides this; widgets may supply a default (a card's body).
    pub description: Option<String>,
    /// Current value for editors, sliders keep it in `range`.
    pub value: Option<String>,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    /// `(minimum, maximum, current)` for sliders and number fields.
    pub range: Option<(f64, f64, f64)>,
    pub read_only: bool,
    /// Current selection or caret of a closed text field, as byte offsets into
    /// `value`. A degenerate `(n, n)` range is a caret with no selection;
    /// `None` means the widget models no text selection at all.
    pub text_selection: Option<(usize, usize)>,
    /// Elements the widget publishes (options, tabs, visible rows).
    pub children: Vec<A11yChild>,
    /// Decoration by default: kept out of the accessibility tree and out of
    /// Tab unless the application opts the instance in (focusable, exposed or
    /// narrated). Titles, cards and progress bars are display-only.
    pub display_only: bool,
    /// An expandable element's open state (a menu button, a menu bar). UIA
    /// reports it through the ExpandCollapse state property.
    pub expanded: Option<bool>,
    /// Scroll position and extents for a scrollable element (a list, a tree,
    /// a scroll view). UIA reports it through the Scroll pattern.
    pub scroll: Option<A11yScroll>,
    /// `(rows, columns)` of a [`A11yRole::Grid`] container; the realized cells
    /// are published as `children`. UIA reports the full extent through
    /// IGridProvider even though only the viewport is realized.
    pub grid: Option<(usize, usize)>,
}

impl A11yInfo {
    pub fn new(role: A11yRole, name: impl Into<String>) -> Self {
        Self {
            role,
            name: name.into(),
            description: None,
            value: None,
            checked: None,
            selected: None,
            range: None,
            read_only: false,
            text_selection: None,
            children: Vec::new(),
            display_only: false,
            expanded: None,
            scroll: None,
            grid: None,
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    pub fn range(mut self, min: f64, max: f64, value: f64) -> Self {
        self.range = Some((min, max, value));
        self
    }

    /// Closed-field selection or caret (byte offsets into the value).
    pub fn text_selection(mut self, start: usize, end: usize) -> Self {
        self.text_selection = Some((start.min(end), start.max(end)));
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn children(mut self, children: Vec<A11yChild>) -> Self {
        self.children = children;
        self
    }

    pub fn display_only(mut self) -> Self {
        self.display_only = true;
        self
    }

    /// Whether an expandable element (a menu, a menu bar) is open.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Publish the Scroll pattern state (a list, a tree, a scroll view).
    pub fn scroll(mut self, scroll: A11yScroll) -> Self {
        self.scroll = Some(scroll);
        self
    }

    /// Publish a grid's full row/column extent (its realized cells are the
    /// `children`).
    pub fn grid(mut self, rows: usize, columns: usize) -> Self {
        self.grid = Some((rows, columns));
        self
    }
}

/// One element of the flattened tree. Index positions in `parent`/`children`
/// refer to the vector returned by [`crate::Ui::a11y_nodes`].
#[derive(Clone, Debug)]
pub struct A11yNode {
    pub id: WidgetId,
    pub role: A11yRole,
    /// Option/row index inside the widget for child elements.
    pub child: Option<usize>,
    pub name: String,
    /// UIA `AutomationId`; falls back to the widget id in the provider.
    pub automation_id: Option<String>,
    /// UIA HelpText / FullDescription (card body, narration override).
    pub description: Option<String>,
    pub value: Option<String>,
    pub enabled: bool,
    pub focusable: bool,
    pub focused: bool,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub range: Option<(f64, f64, f64)>,
    pub read_only: bool,
    /// Closed-field selection or caret (byte offsets into `value`).
    pub text_selection: Option<(usize, usize)>,
    /// Client rectangle in DIPs; the platform layer converts to screen pixels.
    /// Scrolled-out list rows keep their geometric rectangle and set the flag.
    pub rect: Rect,
    /// The element is scrolled out of its container's viewport.
    pub offscreen: bool,
    pub expanded: Option<bool>,
    /// Scroll pattern state; `None` for elements that do not scroll.
    pub scroll: Option<A11yScroll>,
    /// Grid cell coordinates (an [`A11yRole::GridCell`] node).
    pub grid_row: Option<usize>,
    pub grid_column: Option<usize>,
    /// `(rows, columns)` published by an [`A11yRole::Grid`] node.
    pub grid: Option<(usize, usize)>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}

/// A request from an assistive technology. `Ui::apply_a11y` turns it into the
/// same widget input a user would produce, so no separate code path exists.
#[derive(Clone, Debug, PartialEq)]
pub enum A11yActionKind {
    Invoke,
    Toggle,
    Select,
    SetValue,
    Focus,
    /// Open an expandable element (a menu, a menu bar).
    Expand,
    /// Close it again.
    Collapse,
    /// Move a scrollable element's viewport (UIA Scroll pattern). Each value
    /// is an absolute target offset in DIPs; `None` leaves that axis alone.
    Scroll {
        horizontal: Option<f32>,
        vertical: Option<f32>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct A11yAction {
    pub kind: A11yActionKind,
    pub id: WidgetId,
    pub child: Option<usize>,
    pub value: Option<String>,
    pub number: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_axis_reports_percentages_and_targets() {
        let axis = A11yScrollAxis::new(0.0, 100.0, 400.0);
        assert!(axis.scrollable());
        assert_eq!(axis.max_offset(), 300.0);
        assert_eq!(axis.percent(), Some(0.0));
        assert_eq!(axis.view_percent(), Some(25.0));
        assert_eq!(axis.target(50.0), 150.0);
        assert_eq!(axis.target(140.0), 300.0);
        // An offset past the end still reports the end, not 115%.
        let end = A11yScrollAxis::new(320.0, 100.0, 400.0);
        assert_eq!(end.percent(), Some(100.0));
    }

    #[test]
    fn scroll_axis_without_overflow_is_not_scrollable() {
        let axis = A11yScrollAxis::new(0.0, 400.0, 380.0);
        assert!(!axis.scrollable());
        assert_eq!(axis.percent(), None);
        assert_eq!(axis.view_percent(), None);
        assert_eq!(axis.target(50.0), 0.0);
        assert!(A11yScroll::new().is_empty());
        assert!(!A11yScroll::new().vertical(0.0, 10.0, 20.0).is_empty());
    }
}
