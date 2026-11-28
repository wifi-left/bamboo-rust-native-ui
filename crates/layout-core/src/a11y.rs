// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The accessibility role vocabulary.
//!
//! The layout model stores a role to set on an entry, so the enum lives here
//! with the other value types; `bamboo-native-ui::a11y` re-exports it beside
//! the snapshot and UIA machinery.

/// What a node is, in terms a screen reader understands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum A11yRole {
    Button,
    /// On/off switch (TogglePattern).
    Toggle,
    Slider,
    TextField,
    NumberField,
    /// Segmented control: a single-select group of radio options.
    RadioGroup,
    TabList,
    List,
    /// Multi-selection list (checkable rows): UIA Selection with
    /// `CanSelectMultiple` true and no required selection.
    MultiSelectList,
    /// One option inside a [`A11yRole::RadioGroup`].
    RadioItem,
    /// One tab inside a [`A11yRole::TabList`].
    TabItem,
    /// One row inside a [`A11yRole::List`].
    ListItem,
    /// Hierarchical list of expandable rows: `TreeItem` children.
    Tree,
    /// One row inside a [`A11yRole::Tree`]. Supports SelectionItem and
    /// ExpandCollapse; a leaf row reports `LeafNode`.
    TreeItem,
    /// Row/column data: `GridCell` children, an IGridProvider on the container
    /// and an IGridItemProvider on each realized cell.
    Grid,
    /// One cell inside a [`A11yRole::Grid`]: SelectionItem plus its row and
    /// column coordinates.
    GridCell,
    /// A column heading of a [`A11yRole::Grid`] (not selectable, no grid-item
    /// coordinates).
    ColumnHeader,
    /// Section heading. Hidden from the tree by default; expose it so a screen
    /// reader can navigate by heading.
    Heading,
    /// Named group of related controls (a card, a stat block).
    Group,
    /// A draggable divider between two panes (no UIA pattern).
    Separator,
    /// Read-only text: status blocks, toasts, labels.
    Text,
    /// Drop-down list: a value plus an open item list.
    ComboBox,
    /// Determinate progress indicator (RangeValue).
    ProgressBar,
    /// One actionable row of a painted menu (a `Menu` or `MenuBar` child).
    MenuItem,
    /// A painted popup menu: its items are `MenuItem` children while it is
    /// open.
    Menu,
    /// Top-level menu strip (File/Edit/View…): `MenuItem` children.
    MenuBar,
    /// Command strip with an overflow menu: `Button` children.
    ToolBar,
    /// Read-only bottom status strip: `Text` children.
    StatusBar,
}

impl A11yRole {
    /// Role of a child element inside a container role.
    pub fn child_role(self) -> Self {
        match self {
            A11yRole::RadioGroup => A11yRole::RadioItem,
            A11yRole::TabList => A11yRole::TabItem,
            A11yRole::List | A11yRole::MultiSelectList => A11yRole::ListItem,
            A11yRole::Tree => A11yRole::TreeItem,
            A11yRole::Grid => A11yRole::GridCell,
            A11yRole::Menu | A11yRole::MenuBar => A11yRole::MenuItem,
            A11yRole::ToolBar => A11yRole::Button,
            A11yRole::StatusBar => A11yRole::Text,
            other => other,
        }
    }
}
