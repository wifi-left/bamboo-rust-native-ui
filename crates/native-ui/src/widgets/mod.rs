// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Built-in widgets.

pub mod chrome;
pub mod color_picker;
pub mod combo;
pub mod content;
pub mod controls;
pub mod cover_wall;
pub mod custom;
pub mod data;
pub mod date_picker;
pub mod display;
pub mod edit;
pub mod editor;
pub mod feedback;
pub mod inputs;
pub(crate) mod inline;
pub mod list;
pub mod multi_select;
pub mod nav;
pub mod observers;
pub mod player;
pub mod playlist;
pub mod popover;
pub mod property;
pub mod scrollbar;
pub mod table;
pub mod text_content;
pub mod time_picker;
pub mod tree;

pub use chrome::{
    MenuBar, MenuButton, Ribbon, RibbonGroup, RibbonItem, RibbonItemSize, RibbonTab, SplitButton,
    StatusBar, StatusSegment, Toolbar, ToolbarItem,
};
pub use color_picker::ColorPicker;
pub use combo::ComboBox;
pub use content::{Avatar, Badge, BadgeTone, Divider, Icon, Kbd, Label, LabelStyle};
pub use controls::{
    Button, ButtonVariant, ColorSwatch, LabelSide, Pager, ProgressBar, Segmented, Slider, Tabs,
    Toggle,
};
pub use cover_wall::{Cover, CoverWall};
pub use custom::CustomWidget;
pub use data::{Gauge, RatingStars, Sparkline};
pub use date_picker::{Date, DatePicker};
pub use display::{Card, EmptyState, GroupBox, SectionTitle, StatCard, Toast, toast_size};
pub use edit::{NumberInput, TextEdit};
pub use editor::EditColors;
pub use feedback::{
    Banner, BannerKind, ProgressRing, Scrim, Snackbar, Spinner, TooltipCard,
};
pub use inputs::{
    Checkbox, NumberStepper, PasswordBox, RadioButton, RadioGroup, SearchBox, Switch, TextArea,
};
pub use list::ListView;
pub use multi_select::MultiSelectList;
pub use nav::{Accordion, Breadcrumb, Collapsible, NavItem, NavRail, Splitter};
pub use player::{PlaybackTransport, RepeatMode, TransportAction, WaveformSeekBar};
pub use playlist::PlaylistView;
pub use popover::{Popover, PopoverPlacement, place_popover};
pub use property::{PropertyGrid, PropertyKind, PropertyRow, PropertySection};
pub use scrollbar::{ScrollBarColors, ScrollBarStyle};
pub use table::{Table, TableColumn, TableRow};
pub use time_picker::{Time, TimePicker};
pub use tree::{TreeItem, TreeView};
pub use text_content::{CopyableText, MarqueeText, RichRun, RichText, RunStyle};
