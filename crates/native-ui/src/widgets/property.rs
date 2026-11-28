// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Sectioned property grid: key/value rows grouped under section headers, with
//! an in-grid editor per row kind.
//!
//! The application owns the data ([`PropertyRow`]s grouped in
//! [`PropertySection`]s) and picks a [`PropertyKind`] per row: plain text, an
//! inline text/number field, a drop-down choice, a check box or a star rating.
//! The grid edits the value itself and reports the new value through
//! [`PropertyGrid::on_change`]; `on_activate` remains for read-only rows the
//! application inspects elsewhere. Every editing surface honors both the
//! per-row [`PropertyRow::read_only`] flag and the widget-level read-only state
//! (`.read_only(true)` or `WidgetProps::read_only`), so a partially or wholly
//! locked inspector stays navigable but immutable.
//!
//! Text and number rows use the container's inline-editing session (typed
//! characters, IME, clipboard paste, Tab/Enter commit, Escape cancel, caret
//! blink). A choice opens a small overlay list inside the grid. Only the
//! entries intersecting the viewport are painted, hit-tested or published to
//! accessibility. The widget paints no background; it layers over the page like
//! a table.

use std::cell::RefCell;

use crate::a11y::{A11yChild, A11yInfo, A11yRole, A11yScroll};
use crate::backend::{TextAlign, TextOverflow};
use crate::event::{Cursor, KeyCode};
use crate::geometry::{Point, Rect};
use crate::menu::Menu;
use crate::skin::{SkinAttrDecl, SkinElement, SkinValue, SkinValueKind};
use crate::widget::{
    TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState,
};

use super::inline::PaintedField;
use super::scrollbar::{self, BarHit, ScrollBarState, ScrollBarStyle};

static PROPERTY_SKIN_CHILDREN: &[&str] = &["section"];
static PROPERTY_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("row-height", SkinValueKind::Number),
    SkinAttrDecl::new("header-height", SkinValueKind::Number),
    SkinAttrDecl::new("key-width", SkinValueKind::Number),
    SkinAttrDecl::new("grid-lines", SkinValueKind::Bool),
    SkinAttrDecl::new("empty-text", SkinValueKind::Text),
];
const SECTION_SKIN_ATTRS: &[&str] = &["title"];

/// How a row's value is displayed and edited.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyKind {
    /// Plain text; a double click or Enter asks the application through
    /// [`PropertyGrid::on_activate`].
    ReadOnly,
    /// Inline text field (typing, IME, paste; Enter commits, Escape reverts).
    Text,
    /// Inline numeric field. The draft only accepts `0-9`, `.` and `-`, clamps
    /// to `min..=max` on commit and formats with `decimals` places. Up/Down
    /// adjust by `step` while editing.
    Number {
        min: f64,
        max: f64,
        step: f64,
        decimals: usize,
    },
    /// Drop-down of the given options; the value is the option's text.
    Choice(Vec<String>),
    /// Check box toggling between the `on` and `off` value texts.
    Check { on: String, off: String },
    /// Star rating; the value is the star count (`0..=stars`).
    Rating { stars: u8 },
}

impl PropertyKind {
    /// Whether the grid itself edits this row (a read-only kind is inspected
    /// through `on_activate`).
    pub fn is_editable(&self) -> bool {
        !matches!(self, PropertyKind::ReadOnly)
    }

    fn is_text(&self) -> bool {
        matches!(self, PropertyKind::Text | PropertyKind::Number { .. })
    }

    fn choice_value(&self, value: &str) -> Option<usize> {
        match self {
            PropertyKind::Choice(options) => options.iter().position(|option| option == value),
            _ => None,
        }
    }

    fn rating_value(&self, value: &str) -> u8 {
        let stars = match self {
            PropertyKind::Rating { stars } => *stars,
            _ => return 0,
        };
        value
            .trim()
            .parse::<u8>()
            .map(|value| value.min(stars))
            .unwrap_or(0)
    }

    fn checked_value(&self, value: &str) -> bool {
        match self {
            PropertyKind::Check { on, .. } => value == on,
            _ => false,
        }
    }

    /// Normalize a committed draft for text-like kinds. `None` keeps the old
    /// value (a non-numeric number draft, or a kind that is not text-edited).
    fn normalize(&self, draft: &str) -> Option<String> {
        match self {
            PropertyKind::Text => Some(draft.to_string()),
            PropertyKind::Number {
                min, max, decimals, ..
            } => {
                let value = draft.trim().parse::<f64>().ok()?.clamp(*min, *max);
                Some(if *decimals == 0 {
                    format!("{}", value.round() as i64)
                } else {
                    format!("{value:.decimals$}", decimals = *decimals)
                })
            }
            _ => None,
        }
    }

    /// Next value for a keyboard/assistive activation of a rating.
    fn next_rating(&self, value: &str) -> Option<String> {
        match self {
            PropertyKind::Rating { stars } => {
                let next = (self.rating_value(value) + 1).min(*stars);
                Some(next.to_string())
            }
            _ => None,
        }
    }
}

/// One key/value pair. `value` stays a `String` so the application can store
/// whatever it displays; the kind only decides the editor.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyRow {
    key: String,
    value: String,
    kind: PropertyKind,
    read_only: bool,
}

impl PropertyRow {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            kind: PropertyKind::ReadOnly,
            read_only: false,
        }
    }

    /// Inline text editor.
    pub fn text(mut self) -> Self {
        self.kind = PropertyKind::Text;
        self
    }

    /// Inline numeric editor with a 1.0 step and no decimals.
    pub fn number(self, min: f64, max: f64) -> Self {
        self.number_with(min, max, 1.0, 0)
    }

    /// Inline numeric editor with an explicit step and decimal count.
    pub fn number_with(mut self, min: f64, max: f64, step: f64, decimals: usize) -> Self {
        self.kind = PropertyKind::Number {
            min: min.min(max),
            max: min.max(max),
            step: step.abs().max(f64::EPSILON),
            decimals,
        };
        self
    }

    /// Drop-down of the given options.
    pub fn choice<I, S>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.kind = PropertyKind::Choice(options.into_iter().map(Into::into).collect());
        self
    }

    /// Check box toggling between two value texts (`on`/`off`).
    pub fn check(mut self, on: impl Into<String>, off: impl Into<String>) -> Self {
        self.kind = PropertyKind::Check {
            on: on.into(),
            off: off.into(),
        };
        self
    }

    /// Star rating carrying the star count in the value.
    pub fn rating(mut self, stars: u8) -> Self {
        self.kind = PropertyKind::Rating {
            stars: stars.clamp(1, 10),
        };
        self
    }

    /// Lock this row; it still paints its value and can be selected, but no
    /// editor opens.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn kind(&self) -> &PropertyKind {
        &self.kind
    }

    pub fn is_editable(&self) -> bool {
        self.kind.is_editable()
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    /// Replace the value; returns whether it changed.
    pub fn set_value(&mut self, value: impl Into<String>) -> bool {
        let value = value.into();
        if self.value == value {
            return false;
        }
        self.value = value;
        true
    }

    /// Replace the key; returns whether it changed.
    pub fn set_key(&mut self, key: impl Into<String>) -> bool {
        let key = key.into();
        if self.key == key {
            return false;
        }
        self.key = key;
        true
    }

    /// Build a row from a `<row>` element; the caller validates first.
    pub(crate) fn from_skin(element: &SkinElement) -> Option<Self> {
        let mut row = Self::new(element.attr("key")?, element.attr("value").unwrap_or(""));
        row.kind = match element.attr("kind").unwrap_or("read-only") {
            "text" => PropertyKind::Text,
            "number" => PropertyKind::Number {
                min: element.attr_number("min").unwrap_or(0.0),
                max: element.attr_number("max").unwrap_or(100.0),
                step: element.attr_number("step").unwrap_or(1.0),
                decimals: element.attr_number("decimals").unwrap_or(0.0) as usize,
            },
            "choice" => PropertyKind::Choice(
                element
                    .attr("options")
                    .unwrap_or("")
                    .split('|')
                    .map(str::trim)
                    .filter(|option| !option.is_empty())
                    .map(str::to_string)
                    .collect(),
            ),
            "check" => PropertyKind::Check {
                on: element.attr("on").unwrap_or("").to_string(),
                off: element.attr("off").unwrap_or("").to_string(),
            },
            "rating" => PropertyKind::Rating {
                stars: element.attr_number("stars").unwrap_or(5.0) as u8,
            },
            _ => PropertyKind::ReadOnly,
        };
        row.read_only = element.attr_bool("read-only").unwrap_or(false);
        Some(row)
    }

    /// Validate one `<row>` element. `kind` selects which optional attributes
    /// are legal, so a `min` on a text row is a typo instead of a silently
    /// ignored value; a number step must be positive, a choice needs
    /// non-empty `|`-separated options, a check needs both value texts and a
    /// rating's star count is 1..=10.
    pub(crate) fn valid_skin_row(element: &SkinElement) -> bool {
        if !element
            .attr("key")
            .is_some_and(|key| !key.trim().is_empty())
            || !element.children.is_empty()
            || !element.attr_bool_ok("read-only")
            || !element.attr_number_ok("min")
            || !element.attr_number_ok("max")
            || !element.attr_number_ok("step")
            || !element.attr_number_ok("decimals")
            || !element.attr_number_ok("stars")
        {
            return false;
        }
        let kind = element.attr("kind").unwrap_or("read-only");
        let allowed: &[&str] = match kind {
            "read-only" | "text" => &["key", "value", "kind", "read-only"],
            "number" => &[
                "key", "value", "kind", "read-only", "min", "max", "step", "decimals",
            ],
            "choice" => &["key", "value", "kind", "read-only", "options"],
            "check" => &["key", "value", "kind", "read-only", "on", "off"],
            "rating" => &["key", "value", "kind", "read-only", "stars"],
            _ => return false,
        };
        if !element.attrs_are(allowed) {
            return false;
        }
        match kind {
            "number" => {
                element.attr_number("step").is_none_or(|step| step > 0.0)
                    && element.attr_number("decimals").is_none_or(|decimals| {
                        (0.0..=10.0).contains(&decimals) && decimals.fract() == 0.0
                    })
            }
            "choice" => element.attr("options").is_some_and(|options| {
                !options.trim().is_empty()
                    && options.split('|').all(|option| !option.trim().is_empty())
            }),
            "check" => {
                element
                    .attr("on")
                    .is_some_and(|text| !text.trim().is_empty())
                    && element
                        .attr("off")
                        .is_some_and(|text| !text.trim().is_empty())
            }
            "rating" => element.attr_number("stars").is_none_or(|stars| {
                (1.0..=10.0).contains(&stars) && stars.fract() == 0.0
            }),
            _ => true,
        }
    }
}

/// A titled group of rows. The header is part of the scrolling content.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropertySection {
    title: String,
    rows: Vec<PropertyRow>,
}

impl PropertySection {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            rows: Vec::new(),
        }
    }

    pub fn row(mut self, row: PropertyRow) -> Self {
        self.rows.push(row);
        self
    }

    pub fn rows(mut self, rows: Vec<PropertyRow>) -> Self {
        self.rows = rows;
        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn rows_ref(&self) -> &[PropertyRow] {
        &self.rows
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Build a section (and its rows) from a `<section>` element; the caller
    /// validates first.
    pub(crate) fn from_skin(element: &SkinElement) -> Option<Self> {
        let mut section = Self::new(element.attr("title").unwrap_or(""));
        section.rows = element
            .children
            .iter()
            .filter_map(PropertyRow::from_skin)
            .collect();
        Some(section)
    }

    /// Validate one `<section>`: a fixed attribute set and only `<row>`
    /// children, each validated by its kind.
    pub(crate) fn valid_skin_section(element: &SkinElement) -> bool {
        element.name == "section"
            && element.attrs_are(SECTION_SKIN_ATTRS)
            && element
                .children
                .iter()
                .all(|row| row.name == "row" && PropertyRow::valid_skin_row(row))
    }
}

/// One scrolling line: a section header or one of its rows.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    section: usize,
    /// `None` for the section header.
    row: Option<usize>,
    top: f32,
    height: f32,
}

/// An open inline text/number session: the entry the embedded painted field
/// covers.
struct Draft {
    entry: usize,
}

/// An open choice drop-down.
struct Dropdown {
    entry: usize,
    options: Vec<String>,
    highlight: usize,
    hovered: Option<usize>,
    rect: Rect,
    pressed: Option<usize>,
}

impl Dropdown {
    const ITEM_HEIGHT: f32 = 24.0;
    const PADDING: f32 = 4.0;

    /// Option index under `pos`, when it lies inside the list.
    fn option_at(&self, pos: Point) -> Option<usize> {
        if !self.rect.contains_point(pos) {
            return None;
        }
        let y = pos.y - self.rect.top - Self::PADDING;
        if y < 0.0 {
            return None;
        }
        let index = (y / Self::ITEM_HEIGHT).floor() as usize;
        let row_top = self.rect.top + Self::PADDING + index as f32 * Self::ITEM_HEIGHT;
        (index < self.options.len() && pos.y < row_top + Self::ITEM_HEIGHT).then_some(index)
    }

    fn item_rect(&self, index: usize) -> Rect {
        let top = self.rect.top + Self::PADDING + index as f32 * Self::ITEM_HEIGHT;
        Rect::new(
            self.rect.left,
            top,
            self.rect.right,
            top + Self::ITEM_HEIGHT,
        )
    }
}

/// Default width of the key column in DIPs.
const DEFAULT_KEY_WIDTH: f32 = 170.0;
/// Horizontal padding inside the key/value cells.
const CELL_PADDING: f32 = 10.0;
/// Vertical inset of an editable value field inside its row.
const FIELD_INSET: f32 = 3.0;
/// Width reserved at the right of a choice field for its chevron.
const CHOICE_ARROW_ROOM: f32 = 18.0;

pub struct PropertyGrid<M> {
    id: WidgetId,
    rect: Rect,
    sections: Vec<PropertySection>,
    entries: Vec<Entry>,
    content_height: f32,
    header_height: f32,
    row_height: f32,
    key_width: f32,
    grid_lines: bool,
    scroll: f32,
    /// Entry index of the keyboard cursor (a row entry, never a header).
    selected: Option<usize>,
    /// Entry index under the pointer (a row entry).
    hover: Option<usize>,
    press: Option<usize>,
    bar: ScrollBarState,
    style: ScrollBarStyle,
    repaint_hint: Option<Rect>,
    empty_text: String,
    draft: Option<Draft>,
    /// The embedded painted field that edits the draft row.
    field: RefCell<PaintedField<M>>,
    dropdown: Option<Dropdown>,
    /// The widget's own lock (builder).
    read_only_own: bool,
    /// The instance's `WidgetProps::read_only`, pushed by the container.
    read_only_props: bool,
    on_select: Option<Box<dyn Fn(&str) -> M>>,
    on_activate: Option<Box<dyn Fn(&str, &str) -> M>>,
    on_change: Option<Box<dyn Fn(&str, &str) -> M>>,
}

impl<M> PropertyGrid<M> {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 320.0, 240.0),
            sections: Vec::new(),
            entries: Vec::new(),
            content_height: 0.0,
            header_height: 28.0,
            row_height: 26.0,
            key_width: DEFAULT_KEY_WIDTH,
            grid_lines: true,
            scroll: 0.0,
            selected: None,
            hover: None,
            press: None,
            bar: ScrollBarState::default(),
            style: ScrollBarStyle::default(),
            repaint_hint: None,
            empty_text: String::new(),
            draft: None,
            field: RefCell::new(PaintedField::new()),
            dropdown: None,
            read_only_own: false,
            read_only_props: false,
            on_select: None,
            on_activate: None,
            on_change: None,
        }
    }

    pub fn sections(mut self, sections: Vec<PropertySection>) -> Self {
        self.set_sections(sections);
        self
    }

    pub fn header_height(mut self, height: f32) -> Self {
        self.header_height = height.max(0.0);
        self.rebuild();
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(16.0);
        self.rebuild();
        self
    }

    /// Width of the key column; clamped to 60 % of the visible width while
    /// painting so the value column always has room.
    pub fn key_width(mut self, width: f32) -> Self {
        self.key_width = width.max(40.0);
        self
    }

    /// Row separators (default on).
    pub fn grid_lines(mut self, grid: bool) -> Self {
        self.grid_lines = grid;
        self
    }

    /// Centered message painted while there are no sections.
    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.empty_text = text.into();
        self
    }

    /// Lock every editor and activation. Navigation and selection still work.
    /// `WidgetProps::read_only` also locks the grid at runtime.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only_own = read_only;
        self
    }

    /// Whether every editor is locked (the builder flag or the pushed
    /// `WidgetProps::read_only`).
    pub fn is_read_only(&self) -> bool {
        self.locked()
    }

    /// A row became the cursor (click or keyboard).
    pub fn on_select(mut self, handler: impl Fn(&str) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    /// A read-only row was activated (double click, Enter or an assistive
    /// invoke). Editable rows edit themselves instead.
    pub fn on_activate(mut self, handler: impl Fn(&str, &str) -> M + 'static) -> Self {
        self.on_activate = Some(Box::new(handler));
        self
    }

    /// The grid committed an edit. The value is already updated; the
    /// application persists it. Fires for text/number commits, choice picks,
    /// check box toggles and rating changes.
    pub fn on_change(mut self, handler: impl Fn(&str, &str) -> M + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
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

    pub fn sections_ref(&self) -> &[PropertySection] {
        &self.sections
    }

    pub fn section(&self, index: usize) -> Option<&PropertySection> {
        self.sections.get(index)
    }

    /// Number of data rows over all sections (headers excluded).
    pub fn row_count(&self) -> usize {
        self.sections.iter().map(PropertySection::row_count).sum()
    }

    /// Scroll offset in DIPs.
    pub fn scroll_offset(&self) -> f32 {
        self.scroll
    }

    /// The first row with `key`, in section order.
    pub fn row_by_key(&self, key: &str) -> Option<&PropertyRow> {
        self.sections
            .iter()
            .flat_map(|section| section.rows.iter())
            .find(|row| row.key == key)
    }

    pub fn value(&self, key: &str) -> Option<&str> {
        self.row_by_key(key).map(PropertyRow::value)
    }

    /// Cursor position as `(section, row)`.
    pub fn selected_path(&self) -> Option<(usize, usize)> {
        let entry = self.entries.get(self.selected?)?;
        Some((entry.section, entry.row?))
    }

    /// The cursor row, when one is selected.
    pub fn selected_row(&self) -> Option<&PropertyRow> {
        self.entry_row(self.entries.get(self.selected?)?)
    }

    pub fn selected_key(&self) -> Option<&str> {
        self.selected_row().map(PropertyRow::key)
    }

    /// Check state of the first `Check` row with `key`.
    pub fn checked(&self, key: &str) -> Option<bool> {
        let row = self.row_by_key(key)?;
        matches!(row.kind, PropertyKind::Check { .. }).then(|| row.kind.checked_value(&row.value))
    }

    /// Set a `Check` row by key. Returns whether the value changed.
    pub fn set_checked(&mut self, key: &str, checked: bool) -> bool {
        let Some((section, row)) = self.path_of_key(key) else {
            return false;
        };
        let PropertyKind::Check { on, off } = self.sections[section].rows[row].kind.clone() else {
            return false;
        };
        let value = if checked { on } else { off };
        self.set_value_at(section, row, value)
    }

    /// Rating of the first `Rating` row with `key`.
    pub fn rating(&self, key: &str) -> Option<u8> {
        let row = self.row_by_key(key)?;
        matches!(row.kind, PropertyKind::Rating { .. }).then(|| row.kind.rating_value(&row.value))
    }

    /// Set a `Rating` row by key (clamped to its star count).
    pub fn set_rating(&mut self, key: &str, rating: u8) -> bool {
        let Some((section, row)) = self.path_of_key(key) else {
            return false;
        };
        let stars = match self.sections[section].rows[row].kind {
            PropertyKind::Rating { stars } => stars,
            _ => return false,
        };
        self.set_value_at(section, row, rating.min(stars).to_string())
    }

    /// Replace the key of the first row with `key`; returns whether it changed.
    pub fn set_key(&mut self, key: &str, new_key: impl Into<String>) -> bool {
        let position = self
            .sections
            .iter()
            .enumerate()
            .find_map(|(section, group)| {
                group
                    .rows
                    .iter()
                    .position(|row| row.key == key)
                    .map(|row| (section, row))
            });
        let Some((section, row)) = position else {
            return false;
        };
        let changed = self.sections[section].rows[row].set_key(new_key);
        if changed {
            self.rebuild();
        }
        changed
    }

    /// Replace the value of the first row with `key`. Returns whether it
    /// changed; an unknown key reports false.
    pub fn set_value(&mut self, key: &str, value: impl Into<String>) -> bool {
        let value = value.into();
        let Some((section, row)) = self.path_of_key(key) else {
            return false;
        };
        self.set_value_at(section, row, value)
    }

    /// Lock or unlock one row (section/row indices).
    pub fn set_row_read_only(&mut self, section: usize, row: usize, read_only: bool) -> bool {
        let Some(row) = self
            .sections
            .get_mut(section)
            .and_then(|section| section.rows.get_mut(row))
        else {
            return false;
        };
        if row.read_only == read_only {
            return false;
        }
        row.read_only = read_only;
        true
    }

    // --- runtime appearance (skin locals) ---------------------------------

    /// Row height in DIPs (16 minimum); the entry table is rebuilt so the
    /// cursor keeps its key. Returns whether it changed.
    pub fn set_row_height(&mut self, height: f32) -> bool {
        let height = height.max(16.0);
        if (self.row_height - height).abs() < 0.01 {
            return false;
        }
        let previous = self.selected_key().map(str::to_string);
        self.row_height = height;
        self.rebuild_keeping(previous);
        true
    }

    pub fn row_height_value(&self) -> f32 {
        self.row_height
    }

    /// Section header height in DIPs; `0` leaves no header band. Returns
    /// whether it changed.
    pub fn set_header_height(&mut self, height: f32) -> bool {
        let height = height.max(0.0);
        if (self.header_height - height).abs() < 0.01 {
            return false;
        }
        let previous = self.selected_key().map(str::to_string);
        self.header_height = height;
        self.rebuild_keeping(previous);
        true
    }

    pub fn header_height_value(&self) -> f32 {
        self.header_height
    }

    /// Key column width in DIPs (40 minimum). Returns whether it changed.
    pub fn set_key_width(&mut self, width: f32) -> bool {
        let width = width.max(40.0);
        if (self.key_width - width).abs() < 0.01 {
            return false;
        }
        let previous = self.selected_key().map(str::to_string);
        self.key_width = width;
        self.rebuild_keeping(previous);
        true
    }

    pub fn key_width_value(&self) -> f32 {
        self.key_width
    }

    /// Row separators. Returns whether it changed.
    pub fn set_grid_lines(&mut self, grid: bool) -> bool {
        if self.grid_lines == grid {
            return false;
        }
        self.grid_lines = grid;
        true
    }

    pub fn grid_lines_enabled(&self) -> bool {
        self.grid_lines
    }

    /// Message painted while there are no rows.
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

    /// Replace every section. The cursor stays on its key when it still
    /// exists. Returns whether anything changed.
    pub fn set_sections(&mut self, sections: Vec<PropertySection>) -> bool {
        if self.sections == sections {
            return false;
        }
        let previous = self.selected_key().map(str::to_string);
        self.close_draft();
        self.dropdown = None;
        self.sections = sections;
        self.rebuild_keeping(previous);
        true
    }

    /// Replace the rows of one section. The cursor stays on its key when the
    /// key survives. Returns whether anything changed.
    pub fn set_section_rows(&mut self, index: usize, rows: Vec<PropertyRow>) -> bool {
        let Some(section) = self.sections.get(index) else {
            return false;
        };
        if section.rows == rows {
            return false;
        }
        let previous = self.selected_key().map(str::to_string);
        self.close_draft();
        self.dropdown = None;
        self.sections[index].rows = rows;
        self.rebuild_keeping(previous);
        true
    }

    /// Move the cursor to the first row with `key` and reveal it. Returns
    /// whether a row with that key exists.
    pub fn select_key(&mut self, key: &str) -> bool {
        let Some(index) = self.entry_for_key(key) else {
            return false;
        };
        self.selected = Some(index);
        self.ensure_selected_visible();
        true
    }

    /// Scroll the minimum distance that makes the row `key` fully visible.
    pub fn ensure_key_visible(&mut self, key: &str) -> bool {
        let Some(index) = self.entry_for_key(key) else {
            return false;
        };
        let before = self.scroll;
        self.ensure_entry_visible(index);
        (self.scroll - before).abs() > 0.01
    }

    /// Scroll so the row `key` sits at the viewport top (its section header
    /// stays visible when the row starts a section). Returns whether the
    /// offset changed.
    pub fn scroll_to_key(&mut self, key: &str) -> bool {
        let Some(index) = self.entry_for_key(key) else {
            return false;
        };
        let before = self.scroll;
        self.scroll = self.reveal_top(index);
        self.clamp_scroll();
        (self.scroll - before).abs() > 0.01
    }

    /// Client rectangle of one row (header excluded), even when it is scrolled
    /// out; the application can place an editor over [`PropertyGrid::value_rect`].
    pub fn row_rect(&self, section: usize, row: usize) -> Option<Rect> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.section == section && entry.row == Some(row))?;
        self.entry_rect(index)
    }

    /// The value field rectangle of one row in client coordinates.
    pub fn value_rect(&self, section: usize, row: usize) -> Option<Rect> {
        self.row_rect(section, row)
            .map(|rect| self.value_area(rect))
    }

    // --- internals --------------------------------------------------------

    fn locked(&self) -> bool {
        self.read_only_own || self.read_only_props
    }

    fn row_locked(&self, row: &PropertyRow) -> bool {
        self.locked() || row.read_only
    }

    fn path_of_key(&self, key: &str) -> Option<(usize, usize)> {
        self.sections
            .iter()
            .enumerate()
            .find_map(|(section, group)| {
                group
                    .rows
                    .iter()
                    .position(|row| row.key == key)
                    .map(|row| (section, row))
            })
    }

    fn set_value_at(&mut self, section: usize, row: usize, value: impl Into<String>) -> bool {
        let Some(row) = self
            .sections
            .get_mut(section)
            .and_then(|section| section.rows.get_mut(row))
        else {
            return false;
        };
        row.set_value(value)
    }

    fn entry_path(&self, index: usize) -> Option<(usize, usize)> {
        let entry = self.entries.get(index)?;
        Some((entry.section, entry.row?))
    }

    /// Flatten the sections into scrolling entries, then keep the cursor on
    /// its key when possible.
    fn rebuild(&mut self) {
        let previous = self.selected_key().map(str::to_string);
        self.rebuild_keeping(previous);
    }

    /// Rebuild the entry list, restoring the cursor to `previous` (a key) when
    /// it still exists. Callers that change the sections must capture the key
    /// *before* mutating them, because the old entry index can already point
    /// at a different row. Open editor surfaces are index-based and close.
    fn rebuild_keeping(&mut self, previous: Option<String>) {
        self.close_draft();
        self.dropdown = None;
        self.entries.clear();
        let mut top = 0.0;
        for (section_index, section) in self.sections.iter().enumerate() {
            self.entries.push(Entry {
                section: section_index,
                row: None,
                top,
                height: self.header_height,
            });
            top += self.header_height;
            for row_index in 0..section.rows.len() {
                self.entries.push(Entry {
                    section: section_index,
                    row: Some(row_index),
                    top,
                    height: self.row_height,
                });
                top += self.row_height;
            }
        }
        self.content_height = top;
        self.selected = previous
            .and_then(|key| self.entry_for_key(&key))
            .or_else(|| self.first_row_entry());
        self.hover = None;
        self.press = None;
        self.clamp_scroll();
        self.ensure_selected_visible();
    }

    fn entry_row(&self, entry: &Entry) -> Option<&PropertyRow> {
        entry
            .row
            .and_then(|row| self.sections.get(entry.section)?.rows.get(row))
    }

    fn entry_for_key(&self, key: &str) -> Option<usize> {
        self.entries.iter().position(|entry| {
            entry
                .row
                .is_some_and(|_| self.entry_row(entry).is_some_and(|row| row.key == key))
        })
    }

    fn first_row_entry(&self) -> Option<usize> {
        self.entries.iter().position(|entry| entry.row.is_some())
    }

    fn last_row_entry(&self) -> Option<usize> {
        self.entries.iter().rposition(|entry| entry.row.is_some())
    }

    fn body_rect(&self) -> Rect {
        self.rect
    }

    fn max_scroll(&self) -> f32 {
        (self.content_height - self.body_rect().height()).max(0.0)
    }

    fn has_scrollbar(&self) -> bool {
        self.max_scroll() > 0.5 && self.body_rect().height() > 0.0
    }

    /// Content area without the scrollbar strip.
    fn content_rect(&self) -> Rect {
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
            self.content_height,
            self.body_rect().height(),
            self.scroll,
            true,
            &self.style,
        )
    }

    /// Right edge of the key column (clamped so the value column keeps room).
    fn key_right(&self) -> f32 {
        let content = self.content_rect();
        (content.left + self.key_width.min(content.width() * 0.6)).min(content.right)
    }

    /// Value cell of a row: the field rectangle for editable rows, the text
    /// rectangle otherwise.
    fn value_area(&self, row: Rect) -> Rect {
        let content = self.content_rect();
        let left = (self.key_right() + CELL_PADDING).min(content.right);
        let right = (content.right - CELL_PADDING).max(left);
        Rect::new(
            left,
            row.top + FIELD_INSET,
            right,
            (row.bottom - FIELD_INSET).max(row.top + FIELD_INSET),
        )
    }

    fn entry_rect(&self, index: usize) -> Option<Rect> {
        let entry = self.entries.get(index)?;
        let content = self.content_rect();
        let top = self.rect.top + entry.top - self.scroll;
        Some(Rect::new(
            content.left,
            top,
            content.right,
            top + entry.height,
        ))
    }

    fn entry_at(&self, pos: Point) -> Option<usize> {
        if !self.content_rect().contains_point(pos) {
            return None;
        }
        let y = pos.y - self.rect.top + self.scroll;
        self.entries
            .iter()
            .position(|entry| y >= entry.top && y < entry.top + entry.height)
    }

    fn row_entry_at(&self, pos: Point) -> Option<usize> {
        self.entry_at(pos)
            .filter(|index| self.entries[*index].row.is_some())
    }

    /// Entry index whose value cell is an editable field under `pos`.
    fn editable_value_at(&self, pos: Point) -> Option<usize> {
        let index = self.row_entry_at(pos)?;
        let row = self.entry_row(self.entries.get(index)?)?;
        if !row.is_editable() || self.row_locked(row) {
            return None;
        }
        let rect = self.entry_rect(index)?;
        self.value_area(rect).contains_point(pos).then_some(index)
    }

    /// Entry index whose value cell is a text/number field under `pos`.
    fn text_value_at(&self, pos: Point) -> Option<usize> {
        let index = self.editable_value_at(pos)?;
        self.entry_row(self.entries.get(index)?)?
            .kind
            .is_text()
            .then_some(index)
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    /// Scroll target that reveals an entry: the first row of a section keeps
    /// its header in view instead of pinning the row to the top edge.
    fn reveal_top(&self, index: usize) -> f32 {
        let Some(entry) = self.entries.get(index) else {
            return 0.0;
        };
        if entry.row == Some(0) {
            (entry.top - self.header_height).max(0.0)
        } else {
            entry.top
        }
    }

    fn ensure_entry_visible(&mut self, index: usize) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        let viewport = self.body_rect().height();
        if viewport <= 0.0 {
            return;
        }
        let top = self.reveal_top(index);
        let bottom = entry.top + entry.height;
        if top < self.scroll {
            self.scroll = top;
        } else if bottom > self.scroll + viewport {
            self.scroll = bottom - viewport;
        }
        self.clamp_scroll();
    }

    fn ensure_selected_visible(&mut self) {
        if let Some(index) = self.selected {
            self.ensure_entry_visible(index);
        }
    }

    fn select(&mut self, index: usize) -> Option<M> {
        if self
            .entries
            .get(index)
            .is_none_or(|entry| entry.row.is_none())
        {
            return None;
        }
        self.selected = Some(index);
        self.ensure_selected_visible();
        let row = self.entry_row(self.entries.get(index)?)?;
        self.on_select.as_ref().map(|handler| handler(&row.key))
    }

    /// Message for a read-only row the application inspects elsewhere.
    fn activate_read_only(&self) -> Option<M> {
        let row = self.selected_row()?;
        self.on_activate
            .as_ref()
            .map(|handler| handler(&row.key, &row.value))
    }

    fn change_message(&self, key: &str, value: &str) -> Option<M> {
        self.on_change.as_ref().map(|handler| handler(key, value))
    }

    /// Apply a new value to row `index` and build the `on_change` message.
    fn commit_value(&mut self, index: usize, value: impl Into<String>) -> Option<M> {
        let (section, row) = self.entry_path(index)?;
        if !self.sections[section].rows[row].set_value(value) {
            return None;
        }
        let row = &self.sections[section].rows[row];
        self.change_message(&row.key, &row.value)
    }

    /// Toggle a check row.
    fn toggle_check(&mut self, index: usize) -> Option<M> {
        let row = self.entry_row(self.entries.get(index)?)?;
        let PropertyKind::Check { on, off } = &row.kind else {
            return None;
        };
        let next = if row.value == *on {
            off.clone()
        } else {
            on.clone()
        };
        self.commit_value(index, next)
    }

    /// Set a rating from a value-field click at `pos`.
    fn set_rating_at(&mut self, index: usize, pos: Point) -> Option<M> {
        let row = self.entry_row(self.entries.get(index)?)?;
        let PropertyKind::Rating { stars } = row.kind else {
            return None;
        };
        let field = self.value_area(self.entry_rect(index)?);
        if !field.contains_point(pos) || field.width() <= 0.0 {
            return None;
        }
        let cell = field.width() / stars as f32;
        let value = (((pos.x - field.left) / cell).floor().max(0.0) as u8 + 1).min(stars);
        self.commit_value(index, value.to_string())
    }

    /// Adjust a rating row by `delta` (keyboard).
    fn adjust_rating(&mut self, index: usize, delta: i32) -> Option<M> {
        let row = self.entry_row(self.entries.get(index)?)?;
        let PropertyKind::Rating { stars } = row.kind else {
            return None;
        };
        let current = row.kind.rating_value(&row.value) as i32;
        let next = (current + delta).clamp(0, stars as i32) as u8;
        if next as i32 == current {
            return None;
        }
        self.commit_value(index, next.to_string())
    }

    /// Open the choice drop-down for row `index`.
    fn open_dropdown(&mut self, index: usize) -> bool {
        let Some(row) = self
            .entries
            .get(index)
            .and_then(|entry| self.entry_row(entry))
        else {
            return false;
        };
        let PropertyKind::Choice(options) = &row.kind else {
            return false;
        };
        if options.is_empty() || self.row_locked(row) {
            return false;
        }
        let options = options.clone();
        let value = row.value.clone();
        let Some(row_rect) = self.entry_rect(index) else {
            return false;
        };
        let field = self.value_area(row_rect);
        let count = options.len() as f32;
        let height = count * Dropdown::ITEM_HEIGHT + Dropdown::PADDING * 2.0;
        let width = field.width().max(140.0).min(self.rect.width());
        let left = field.left.min(self.rect.right - width).max(self.rect.left);
        let below = field.bottom + 2.0;
        let top = if below + height <= self.rect.bottom {
            below
        } else {
            (field.top - 2.0 - height).max(self.rect.top)
        };
        let rect = Rect::new(
            left,
            top,
            left + width,
            (top + height).min(self.rect.bottom),
        );
        let highlight = row
            .kind
            .choice_value(&value)
            .unwrap_or(0)
            .min(options.len() - 1);
        self.dropdown = Some(Dropdown {
            entry: index,
            options,
            highlight,
            hovered: None,
            rect,
            pressed: None,
        });
        true
    }

    /// Enter the inline text/number editor for `index`.
    fn begin_draft(&mut self, index: usize) -> bool {
        self.begin_draft_at(index, None)
    }

    /// Close an open session without applying it (a lock or a rebuild).
    fn close_draft(&mut self) {
        if self.draft.take().is_some() {
            self.field.get_mut().finish(false);
        }
    }

    /// Start a text/number session. `position` places the caret from a pointer
    /// click; without one the whole value starts selected (Enter/assistive
    /// activation, typing then replaces it). The value is edited by the
    /// embedded painted field: a value containing a newline gets the multiline
    /// model (soft wrap, Up/Down across lines, Enter inserts a newline,
    /// Ctrl+Enter/Tab/blur commits).
    fn begin_draft_at(&mut self, index: usize, position: Option<Point>) -> bool {
        let Some(row) = self
            .entries
            .get(index)
            .and_then(|entry| self.entry_row(entry))
        else {
            return false;
        };
        if !row.kind.is_text() || self.row_locked(row) {
            return false;
        }
        let text = row.value.clone();
        self.selected = Some(index);
        self.ensure_selected_visible();
        self.dropdown = None;
        let Some(rect) = self.entry_rect(index) else {
            return false;
        };
        let field = self.value_area(rect);
        if !self
            .field
            .get_mut()
            .begin(field, &text, position, crate::geometry::Insets::xy(6.0, 2.0))
        {
            return false;
        }
        self.draft = Some(Draft { entry: index });
        true
    }

    /// Commit (`true`) or discard the open draft.
    fn finish_draft(&mut self, commit: bool) -> Option<M> {
        let draft = self.draft.take()?;
        let text = self.field.get_mut().finish(commit)?;
        let row = self.entry_row(self.entries.get(draft.entry)?)?;
        let value = row.kind.normalize(&text)?;
        self.commit_value(draft.entry, value)
    }

    /// Edit the draft as keyboard input. The field owns every key, so this
    /// always consumes; a number draft steps its value with Up/Down instead of
    /// moving the caret (the field leaves Up/Down alone in a single line).
    fn key_in_draft(&mut self, key: crate::event::KeyEvent) {
        let kind = self.draft.as_ref().and_then(|draft| {
            self.entries
                .get(draft.entry)
                .and_then(|entry| self.entry_row(entry))
                .map(|row| row.kind.clone())
        });
        if self.draft.is_none() {
            return;
        }
        if key.code == KeyCode::Up || key.code == KeyCode::Down {
            let Some(PropertyKind::Number {
                min,
                max,
                step,
                decimals,
            }) = kind
            else {
                self.field.get_mut().key(key);
                return;
            };
            let current = self
                .field
                .borrow()
                .text()
                .trim()
                .parse::<f64>()
                .unwrap_or(min);
            let next =
                (current + if key.code == KeyCode::Up { step } else { -step }).clamp(min, max);
            let text = if decimals == 0 {
                format!("{}", next.round() as i64)
            } else {
                format!("{next:.decimals$}", decimals = decimals)
            };
            self.field.get_mut().set_text(text);
            return;
        }
        self.field.get_mut().key(key);
    }

    /// Move the cursor to the next/previous row entry, skipping headers.
    fn step_cursor(&mut self, delta: i32) -> bool {
        let Some(current) = self.selected else {
            return self.first_row_entry().is_some_and(|index| {
                self.selected = Some(index);
                self.ensure_selected_visible();
                true
            });
        };
        let mut index = current as isize + delta as isize;
        while index >= 0 && (index as usize) < self.entries.len() {
            if self.entries[index as usize].row.is_some() {
                self.selected = Some(index as usize);
                self.ensure_selected_visible();
                return true;
            }
            index += delta as isize;
        }
        false
    }

    fn page_rows(&self) -> usize {
        ((self.body_rect().height() / self.row_height).floor() as usize).max(1)
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
            self.content_height,
            self.body_rect().height(),
            true,
        );
        self.clamp_scroll();
    }

    /// Entry indices to publish/paint: every entry intersecting the viewport
    /// plus the first look-ahead entry after it.
    fn visible_entries(&self) -> Vec<usize> {
        let max_top = self.scroll + self.body_rect().height();
        let mut indices = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            let bottom = entry.top + entry.height;
            if bottom <= self.scroll {
                continue;
            }
            indices.push(index);
            if entry.top >= max_top {
                break;
            }
        }
        indices
    }

    /// Activate the cursor row (Enter/Space in adjust mode, a11y Invoke).
    fn activate_cursor(&mut self) -> Option<M> {
        let index = self.selected?;
        let row = self.entry_row(self.entries.get(index)?)?;
        if self.row_locked(row) {
            return None;
        }
        match row.kind.clone() {
            PropertyKind::ReadOnly => self.activate_read_only(),
            PropertyKind::Text | PropertyKind::Number { .. } => {
                self.begin_draft(index);
                None
            }
            PropertyKind::Choice(_) => {
                self.open_dropdown(index);
                None
            }
            PropertyKind::Check { .. } => self.toggle_check(index),
            PropertyKind::Rating { .. } => {
                let next = row.kind.next_rating(&row.value)?;
                self.commit_value(index, next)
            }
        }
    }

    /// Activate one row at `pos` (value-field release). `true` means an editor
    /// surface opened; the caller then suppresses the selection message.
    fn activate_at(&mut self, index: usize, pos: Point) -> (Option<M>, bool) {
        let Some(row) = self
            .entries
            .get(index)
            .and_then(|entry| self.entry_row(entry))
        else {
            return (None, false);
        };
        if self.row_locked(row) {
            return (None, false);
        }
        match row.kind.clone() {
            PropertyKind::Text | PropertyKind::Number { .. } => {
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.entry == index)
                {
                    return (None, false);
                }
                let opened = self.begin_draft_at(index, Some(pos));
                (None, opened)
            }
            PropertyKind::Choice(_) => {
                let opened = self.open_dropdown(index);
                (None, opened)
            }
            PropertyKind::Check { .. } => (self.toggle_check(index), false),
            PropertyKind::Rating { .. } => (self.set_rating_at(index, pos), false),
            PropertyKind::ReadOnly => (None, false),
        }
    }

    fn input_press(&mut self, pos: Point) -> Option<M> {
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
                    }
                    return None;
                }
                Some(BarHit::Track) => {
                    if let Some(thumb) = thumb {
                        self.scroll = scrollbar::track_click_offset(
                            pos,
                            track,
                            thumb,
                            self.content_height,
                            self.body_rect().height(),
                            self.scroll,
                            true,
                            &self.style,
                        );
                        self.clamp_scroll();
                        self.bar.active = true;
                        self.bar.hovered = true;
                        self.bar.grab = thumb.height() * 0.5;
                    }
                    return None;
                }
                None => {}
            }
        }
        self.press = self.row_entry_at(pos);
        None
    }

    /// Route input while the choice list is up. The list owns the pointer and
    /// the keyboard until it picks an option or is dismissed.
    fn input_dropdown(&mut self, input: WidgetInput) -> Option<M> {
        let Some(mut dropdown) = self.dropdown.take() else {
            return None;
        };
        match input {
            WidgetInput::HoverAt(pos) => {
                dropdown.hovered = dropdown.option_at(pos);
                self.dropdown = Some(dropdown);
                None
            }
            WidgetInput::HoverLeave => {
                dropdown.hovered = None;
                self.dropdown = Some(dropdown);
                None
            }
            WidgetInput::PressAt(pos) => {
                if let Some(option) = dropdown.option_at(pos) {
                    dropdown.pressed = Some(option);
                    dropdown.highlight = option;
                    self.dropdown = Some(dropdown);
                    None
                } else {
                    // The list closes; the press continues on the grid.
                    self.input_press(pos)
                }
            }
            WidgetInput::ReleaseAt(pos) => {
                let Some(pressed) = dropdown.pressed else {
                    self.dropdown = Some(dropdown);
                    return None;
                };
                let Some(option) = dropdown.option_at(pos) else {
                    self.dropdown = Some(dropdown);
                    return None;
                };
                if pressed != option {
                    self.dropdown = Some(dropdown);
                    return None;
                }
                let value = dropdown.options.get(option)?.clone();
                let entry = dropdown.entry;
                self.commit_value(entry, value)
            }
            WidgetInput::Key(key) => {
                let count = dropdown.options.len();
                match key.code {
                    KeyCode::Up => dropdown.highlight = dropdown.highlight.saturating_sub(1),
                    KeyCode::Down => {
                        dropdown.highlight = (dropdown.highlight + 1).min(count.saturating_sub(1));
                    }
                    KeyCode::Home => dropdown.highlight = 0,
                    KeyCode::End => dropdown.highlight = count.saturating_sub(1),
                    KeyCode::Enter | KeyCode::Space => {
                        let Some(value) = dropdown.options.get(dropdown.highlight).cloned() else {
                            self.dropdown = Some(dropdown);
                            return None;
                        };
                        let entry = dropdown.entry;
                        return self.commit_value(entry, value);
                    }
                    KeyCode::Escape => return None,
                    _ => {}
                }
                self.dropdown = Some(dropdown);
                None
            }
            _ => {
                self.dropdown = Some(dropdown);
                None
            }
        }
    }
}

impl<M: 'static> Widget<M> for PropertyGrid<M> {
    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        PROPERTY_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("row-height", SkinValue::Number(height)) => *height >= 16.0,
            ("header-height", SkinValue::Number(height)) => *height >= 0.0,
            ("key-width", SkinValue::Number(width)) => *width >= 40.0,
            ("grid-lines", SkinValue::Bool(_)) => true,
            ("empty-text", SkinValue::Text(_)) => true,
            _ => false,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("row-height", SkinValue::Number(height)) => {
                self.set_row_height(*height as f32);
            }
            ("header-height", SkinValue::Number(height)) => {
                self.set_header_height(*height as f32);
            }
            ("key-width", SkinValue::Number(width)) => {
                self.set_key_width(*width as f32);
            }
            ("grid-lines", SkinValue::Bool(grid)) => {
                self.set_grid_lines(*grid);
            }
            ("empty-text", SkinValue::Text(text)) => {
                self.set_empty_text(*text);
            }
            _ => {}
        }
    }

    fn skin_children(&self) -> &'static [&'static str] {
        PROPERTY_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children
            .iter()
            .all(PropertySection::valid_skin_section)
    }

    /// `<section>` elements replace every section (their `<row>` children
    /// carry the kind and its parameters).
    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        let sections = children
            .iter()
            .filter_map(PropertySection::from_skin)
            .collect();
        self.set_sections(sections);
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
        self.ensure_selected_visible();
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        220.0
    }

    fn read_only(&self) -> bool {
        self.locked()
    }

    fn set_read_only(&mut self, read_only: bool) {
        self.read_only_props = read_only;
        if read_only {
            self.close_draft();
        }
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    fn repaint_on_move_at(&self, pos: Point) -> bool {
        if self.bar.active {
            return true;
        }
        if let Some(dropdown) = &self.dropdown {
            return dropdown.option_at(pos) != dropdown.hovered;
        }
        if self.has_scrollbar()
            && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some()
        {
            return !self.bar.hovered;
        }
        self.bar.hovered || self.row_entry_at(pos) != self.hover
    }

    fn take_repaint_hint(&mut self) -> Option<Rect> {
        self.repaint_hint.take()
    }

    fn scrolls_with_wheel(&self) -> bool {
        true
    }

    fn adjustable(&self) -> bool {
        true
    }

    /// The cursor covers a row field (an arrow over a track keeps its shape).
    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        if let Some(dropdown) = &self.dropdown
            && dropdown.rect.contains_point(pos)
        {
            return Some(Cursor::Hand);
        }
        if self.has_scrollbar() && self.scrollbar_rect().contains_point(pos) {
            return Some(Cursor::Arrow);
        }
        self.editable_value_at(pos).map(|_| Cursor::Hand)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let pal = ctx.palette;
        let content = self.content_rect();

        ctx.canvas.push_clip(content);
        if self.entries.is_empty() {
            if !self.empty_text.is_empty() {
                ctx.text(
                    content,
                    &self.empty_text,
                    TextStyleRole::Small,
                    TextAlign::Center,
                    pal.muted,
                );
            }
        } else {
            let key_right = self.key_right();
            let key_rect_left = content.left + CELL_PADDING;
            let key_text_right = (key_right - CELL_PADDING * 0.5).max(key_rect_left);
            for index in self.visible_entries() {
                let Some(rect) = self.entry_rect(index) else {
                    continue;
                };
                let entry = self.entries[index];
                if !rect.intersects(content) {
                    continue;
                }
                let hovered = state.hovered && self.hover == Some(index);
                match entry.row {
                    None => self.paint_header(ctx, rect, content, entry, pal),
                    Some(_) => {
                        let Some(row) = self.entry_row(&entry) else {
                            continue;
                        };
                        let locked = self.row_locked(row);
                        let cursor = state.focused
                            && self.selected == Some(index)
                            && self.dropdown.is_none();
                        if state.pressed && self.press == Some(index) {
                            ctx.canvas.fill_rect(rect, pal.row_pressed);
                        } else if hovered {
                            ctx.canvas.fill_rect(rect, pal.row_hover);
                        } else if cursor {
                            ctx.canvas.fill_rect(rect, pal.row_selected);
                        }
                        ctx.text_overflow(
                            Rect::new(key_rect_left, rect.top, key_text_right, rect.bottom),
                            &row.key,
                            TextStyleRole::Body,
                            TextAlign::Left,
                            pal.muted,
                            TextOverflow::Ellipsis,
                        );
                        let field = self.value_area(rect);
                        self.paint_value(ctx, state, index, row, field, locked, cursor, hovered);
                    }
                }
                if self.grid_lines && entry.row.is_some() {
                    ctx.canvas.fill_rect(
                        Rect::new(content.left, rect.bottom - 1.0, content.right, rect.bottom),
                        pal.divider,
                    );
                }
            }
        }
        ctx.canvas.pop_clip();

        if self.has_scrollbar() {
            scrollbar::paint(
                ctx,
                self.scrollbar_rect(),
                self.thumb_rect(),
                &self.bar,
                &self.style,
            );
        }
        if let Some(dropdown) = &self.dropdown {
            self.paint_dropdown(ctx, dropdown);
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if self.dropdown.is_some() {
            return self.input_dropdown(input);
        }
        match input {
            WidgetInput::HoverAt(pos) => {
                let over_bar = self.has_scrollbar()
                    && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some();
                let bar_before = self.bar.hovered;
                let row_before = self.hover;
                if over_bar {
                    self.bar.hovered = true;
                    self.hover = None;
                } else {
                    self.bar.hovered = false;
                    self.hover = self.row_entry_at(pos);
                }
                if self.bar.hovered != bar_before && self.hover == row_before {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::HoverLeave => {
                let bar_before = self.bar.hovered;
                let row_before = self.hover;
                self.hover = None;
                if !self.bar.active {
                    self.bar.hovered = false;
                }
                if self.bar.hovered != bar_before && self.hover == row_before {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::PressAt(pos) => self.input_press(pos),
            WidgetInput::DragTo(pos) => {
                if self.bar.active {
                    self.drag_thumb(pos, self.bar.grab);
                    return None;
                }
                if self.draft.is_some() {
                    self.field.get_mut().input(WidgetInput::DragTo(pos));
                }
                None
            }
            WidgetInput::ReleaseAt(pos) => {
                if self.bar.active {
                    self.bar.active = false;
                    self.repaint_hint = Some(self.scrollbar_rect());
                    return None;
                }
                let editing_here = self.draft.as_ref().is_some_and(|draft| {
                    self.entry_rect(draft.entry)
                        .is_some_and(|rect| self.value_area(rect).contains_point(pos))
                });
                if editing_here {
                    self.field.get_mut().input(WidgetInput::ReleaseAt(pos));
                    self.press = None;
                    return None;
                }
                let pressed = self.press.take()?;
                let hit = self.row_entry_at(pos)?;
                if pressed != hit {
                    return None;
                }
                // A click on another row commits an open draft first.
                let committed = if self.draft.as_ref().is_some_and(|draft| draft.entry != hit) {
                    self.finish_draft(true)
                } else {
                    None
                };
                let selection = self.select(hit);
                let activated = self.editable_value_at(pos).map(|index| {
                    let (message, opened) = self.activate_at(index, pos);
                    (message, opened)
                });
                match activated {
                    Some((message, true)) => committed.or(message),
                    Some((message, false)) => committed.or(message).or(selection),
                    None => committed.or(selection),
                }
            }
            WidgetInput::ClickAt(pos) => {
                if self.has_scrollbar()
                    && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some()
                {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                None
            }
            WidgetInput::DoubleClickAt(pos) => {
                let hit = self.row_entry_at(pos)?;
                let selection = self.select(hit);
                let row = self.entry_row(self.entries.get(hit)?)?;
                if row.is_editable() && !self.row_locked(row) {
                    return selection;
                }
                self.activate_read_only().or(selection)
            }
            WidgetInput::Wheel(delta) => {
                // Editing pins the field to its row; the wheel never moves the
                // rows underneath it.
                if self.draft.is_some() {
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
                if self.draft.is_some() {
                    self.key_in_draft(key);
                    return None;
                }
                let page = self.page_rows() as i32;
                match key.code {
                    KeyCode::Up => {
                        self.step_cursor(-1);
                    }
                    KeyCode::Down => {
                        self.step_cursor(1);
                    }
                    KeyCode::Left | KeyCode::Right => {
                        let delta = if key.code == KeyCode::Left { -1 } else { 1 };
                        let rating = self.selected.is_some_and(|index| {
                            self.entry_row(&self.entries[index]).is_some_and(|row| {
                                matches!(row.kind, PropertyKind::Rating { .. })
                                    && !self.row_locked(row)
                            })
                        });
                        if let Some(index) = self.selected.filter(|_| rating) {
                            return self.adjust_rating(index, delta);
                        }
                    }
                    KeyCode::PageUp => {
                        for _ in 0..page {
                            if !self.step_cursor(-1) {
                                break;
                            }
                        }
                    }
                    KeyCode::PageDown => {
                        for _ in 0..page {
                            if !self.step_cursor(1) {
                                break;
                            }
                        }
                    }
                    KeyCode::Home => {
                        if let Some(index) = self.first_row_entry() {
                            self.selected = Some(index);
                            self.ensure_selected_visible();
                        }
                    }
                    KeyCode::End => {
                        if let Some(index) = self.last_row_entry() {
                            self.selected = Some(index);
                            self.ensure_selected_visible();
                        }
                    }
                    KeyCode::Enter | KeyCode::Space => return self.activate_cursor(),
                    _ => {}
                }
                None
            }
            WidgetInput::Activate => self.activate_cursor(),
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let body = self.body_rect();
        let children = self
            .visible_entries()
            .into_iter()
            .filter_map(|index| {
                let entry = *self.entries.get(index)?;
                let rect = self.entry_rect(index)?;
                let offscreen = !rect.intersects(body);
                let clipped = rect.intersect(body).unwrap_or(rect);
                let child = match entry.row {
                    None => {
                        let name = self
                            .sections
                            .get(entry.section)
                            .map(|section| section.title().to_string())
                            .unwrap_or_default();
                        A11yChild::new(index, name, clipped)
                            .role(A11yRole::Heading)
                            .offscreen(offscreen)
                    }
                    Some(_) => {
                        let row = self.entry_row(&entry)?;
                        // The editing row publishes the session's text, so a
                        // screen reader follows what is being typed.
                        let editing = self
                            .draft
                            .as_ref()
                            .is_some_and(|draft| draft.entry == index);
                        let value = if editing {
                            self.field.borrow().text().to_string()
                        } else {
                            row.value.clone()
                        };
                        let locked = self.row_locked(row);
                        let mut child =
                            A11yChild::new(index, format!("{}: {value}", row.key), clipped)
                                .selected(self.selected == Some(index))
                                .enabled(!locked)
                                .offscreen(offscreen);
                        // Editable kinds publish the value; a locked row keeps
                        // it read-only, and `SetValue` applies through the
                        // same commit path as Enter (D103).
                        if !matches!(row.kind, PropertyKind::ReadOnly) {
                            child = child.value(value.clone()).editable(!locked);
                        }
                        child
                    }
                };
                Some(child)
            })
            .collect();
        Some(
            A11yInfo::new(A11yRole::List, self.id.0)
                .children(children)
                .read_only(self.locked())
                .scroll(A11yScroll::new().vertical(
                    self.scroll,
                    self.body_rect().height(),
                    self.content_height,
                )),
        )
    }

    /// A published row is one logical element: selecting it moves the cursor
    /// (UIA `ListItem` carries no Invoke here, so no editor opens).
    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        if self
            .entries
            .get(index)
            .is_none_or(|entry| entry.row.is_none())
        {
            return None;
        }
        self.select(index)
    }

    fn a11y_scroll(&mut self, _horizontal: Option<f32>, vertical: Option<f32>) -> bool {
        let Some(target) = vertical else {
            return false;
        };
        let before = self.scroll;
        self.scroll = target.clamp(0.0, self.max_scroll());
        (self.scroll - before).abs() > 0.01
    }

    /// A UIA Value `SetValue` on a row applies the text through the same
    /// normalisation and `on_change` report the inline editor uses: numbers
    /// are parsed/clamped/formatted, a choice must name one of its options, a
    /// check accepts its on/off text (or true/false) and a rating clamps to
    /// its star count. Read-only kinds and locked rows refuse.
    fn a11y_set_value_at(&mut self, child: Option<usize>, text: &str) -> Option<M> {
        let index = child?;
        let row = self.entry_row(self.entries.get(index)?)?.clone();
        if self.row_locked(&row) {
            return None;
        }
        let next = match &row.kind {
            PropertyKind::ReadOnly => return None,
            PropertyKind::Text | PropertyKind::Number { .. } => row.kind.normalize(text)?,
            PropertyKind::Choice(options) => {
                let text = text.trim();
                options
                    .iter()
                    .find(|option| option.as_str() == text)?
                    .clone()
            }
            PropertyKind::Check { on, off } => {
                let text = text.trim();
                if text.eq_ignore_ascii_case("true") || text == on {
                    on.clone()
                } else if text.eq_ignore_ascii_case("false") || text == off {
                    off.clone()
                } else {
                    return None;
                }
            }
            PropertyKind::Rating { stars } => {
                let value: u8 = text.trim().parse().ok()?;
                value.min(*stars).to_string()
            }
        };
        self.commit_value(index, next)
    }

    fn tooltip_rect(&self) -> Rect {
        self.selected
            .and_then(|index| self.entry_rect(index))
            .unwrap_or(self.rect)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.row_entry_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        self.selected
    }

    fn focus_ring_rect(&self) -> Rect {
        self.rect().inset(1.0, 1.0)
    }

    fn paint_bounds(&self) -> Rect {
        match &self.dropdown {
            Some(dropdown) => self.rect.union(dropdown.rect),
            None => self.rect,
        }
    }

    fn overlay(&self) -> bool {
        self.dropdown.is_some()
    }

    fn hit_test(&self, pos: Point) -> bool {
        self.rect.contains_point(pos)
            || self
                .dropdown
                .as_ref()
                .is_some_and(|dropdown| dropdown.rect.contains_point(pos))
    }

    fn dismiss(&mut self) -> bool {
        self.dropdown.take().is_some()
    }

    fn owns_navigation_keys(&self) -> bool {
        self.dropdown.is_some()
    }

    fn inline_edit(&self) -> bool {
        self.draft.is_some()
    }

    fn inline_edit_at(&self, pos: Point) -> bool {
        self.text_value_at(pos).is_some()
    }

    fn editing(&self) -> bool {
        self.draft.is_some()
    }

    fn begin_inline_edit(&mut self, position: Option<Point>) -> bool {
        if self.locked() {
            return false;
        }
        match position {
            Some(pos) => {
                let Some(index) = self.text_value_at(pos) else {
                    return false;
                };
                // A click inside the open draft only moves the caret (and can
                // start a selection drag); it never resets the typed text.
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.entry == index)
                {
                    return self.field.get_mut().place_at(pos);
                }
                if self.draft.is_some() {
                    return false;
                }
                self.begin_draft_at(index, Some(pos))
            }
            None => self.selected.is_some_and(|index| self.begin_draft(index)),
        }
    }

    fn finish_edit(&mut self, commit: bool) -> Option<M> {
        self.finish_draft(commit)
    }

    fn insert_text(&mut self, text: &str) -> bool {
        let numeric = {
            let Some(draft) = self.draft.as_ref() else {
                return false;
            };
            matches!(
                self.entry_row(&self.entries[draft.entry]).map(|row| &row.kind),
                Some(PropertyKind::Number { .. })
            )
        };
        self.field.get_mut().insert_filtered(text, |ch| {
            !numeric || ch.is_ascii_digit() || ch == '-' || ch == '.'
        })
    }

    fn set_composition(&mut self, text: &str, cursor: usize) -> bool {
        if self.draft.is_none() {
            return false;
        }
        self.field.get_mut().set_composition(text, cursor)
    }

    fn caret_rect(&self) -> Option<Rect> {
        self.draft.as_ref()?;
        self.field.borrow().caret_rect()
    }

    fn multiline_edit(&self) -> bool {
        self.draft.is_some() && self.field.borrow().multiline()
    }

    /// The standard edit menu while a text/number draft is open (supplied by
    /// the shared painted field; the container narrows Paste from the
    /// clipboard). No draft means no built-in menu.
    fn edit_menu(&self) -> Option<Menu> {
        self.field.borrow().edit_menu()
    }

    fn edit_command(&mut self, command: u32) -> bool {
        self.field.get_mut().edit_command(command)
    }

    fn take_clipboard(&mut self) -> Option<String> {
        self.field.get_mut().take_clipboard()
    }
}

// --- painting helpers ------------------------------------------------------

impl<M> PropertyGrid<M> {
    fn paint_header(
        &self,
        ctx: &mut WidgetPaint<'_>,
        rect: Rect,
        content: Rect,
        entry: Entry,
        pal: crate::Palette,
    ) {
        // Section header: panel band with an accent tick.
        ctx.canvas.fill_rect(rect, pal.panel);
        let title = self
            .sections
            .get(entry.section)
            .map(|section| section.title())
            .unwrap_or("");
        if title.is_empty() {
            return;
        }
        let center = rect.center().y;
        ctx.canvas.fill_rect(
            Rect::new(
                content.left + CELL_PADDING,
                center - 6.0,
                content.left + CELL_PADDING + 3.0,
                center + 6.0,
            ),
            pal.accent,
        );
        ctx.text_overflow(
            Rect::new(
                content.left + CELL_PADDING + 12.0,
                rect.top,
                content.right - CELL_PADDING,
                rect.bottom,
            ),
            title,
            TextStyleRole::Small,
            TextAlign::Left,
            pal.muted,
            TextOverflow::Ellipsis,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_value(
        &self,
        ctx: &mut WidgetPaint<'_>,
        state: &WidgetState,
        index: usize,
        row: &PropertyRow,
        field: Rect,
        locked: bool,
        cursor: bool,
        hovered: bool,
    ) {
        let pal = ctx.palette;
        let text_color = if locked { pal.text_disabled } else { pal.text };
        // The open draft owns this field's pixels.
        if self.draft.as_ref().is_some_and(|draft| draft.entry == index) {
            self.paint_draft(ctx, state, field);
            return;
        }
        match &row.kind {
            PropertyKind::ReadOnly => {
                ctx.text_overflow(
                    field,
                    &row.value,
                    TextStyleRole::Body,
                    TextAlign::Left,
                    text_color,
                    TextOverflow::Ellipsis,
                );
            }
            PropertyKind::Text | PropertyKind::Number { .. } => {
                if locked {
                    ctx.text_overflow(
                        field,
                        &row.value,
                        TextStyleRole::Body,
                        TextAlign::Left,
                        text_color,
                        TextOverflow::Ellipsis,
                    );
                    return;
                }
                paint_field(ctx, field, cursor, hovered);
                ctx.text_overflow(
                    field.inset(6.0, 0.0),
                    &row.value,
                    TextStyleRole::Body,
                    TextAlign::Left,
                    pal.text,
                    TextOverflow::Ellipsis,
                );
            }
            PropertyKind::Choice(_) => {
                if !locked {
                    paint_field(ctx, field, cursor, hovered);
                }
                let arrow = Rect::new(
                    (field.right - CHOICE_ARROW_ROOM).max(field.left),
                    field.top,
                    field.right,
                    field.bottom,
                );
                ctx.text_overflow(
                    Rect::new(field.left + 6.0, field.top, arrow.left, field.bottom),
                    &row.value,
                    TextStyleRole::Body,
                    TextAlign::Left,
                    text_color,
                    TextOverflow::Ellipsis,
                );
                ctx.text(
                    arrow,
                    "\u{E70D}",
                    TextStyleRole::Glyph,
                    TextAlign::Center,
                    if locked { pal.text_disabled } else { pal.muted },
                );
            }
            PropertyKind::Check { .. } => {
                if !locked {
                    paint_field(ctx, field, cursor, hovered);
                }
                let checked = row.kind.checked_value(&row.value);
                let size = 15.0;
                let box_rect = Rect::new(
                    field.left + 6.0,
                    field.center().y - size / 2.0,
                    field.left + 6.0 + size,
                    field.center().y + size / 2.0,
                );
                paint_check(ctx, box_rect, checked, locked, pal);
            }
            PropertyKind::Rating { stars } => {
                if !locked {
                    paint_field(ctx, field, cursor, hovered);
                }
                let stars = *stars;
                let filled = row.kind.rating_value(&row.value);
                let slot = field.width() / stars as f32;
                for star in 0..stars {
                    let left = field.left + star as f32 * slot;
                    let cell = Rect::new(left, field.top, left + slot, field.bottom);
                    let on = star < filled;
                    ctx.text(
                        cell,
                        if on { "\u{2605}" } else { "\u{2606}" },
                        TextStyleRole::Body,
                        TextAlign::Center,
                        if locked {
                            pal.text_disabled
                        } else if on {
                            pal.accent
                        } else {
                            pal.muted
                        },
                    );
                }
            }
        }
    }

    fn paint_draft(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState, field: Rect) {
        // Re-anchor the embedded painted field to the value cell (a scroll or
        // resize may have moved it) and let it paint itself: frame, selection,
        // inline IME, caret and any multiline scrolling.
        let mut painted = self.field.borrow_mut();
        if painted.rect() != field {
            painted.set_rect(field);
        }
        painted.paint(ctx, state);
    }

    fn paint_dropdown(&self, ctx: &mut WidgetPaint<'_>, dropdown: &Dropdown) {
        let pal = ctx.palette;
        ctx.canvas.push_clip(dropdown.rect);
        ctx.canvas.fill_rect(dropdown.rect, pal.card);
        paint_border(ctx, dropdown.rect, pal.explicit_border);
        for (index, option) in dropdown.options.iter().enumerate() {
            let rect = dropdown.item_rect(index);
            if rect.top >= dropdown.rect.bottom {
                break;
            }
            let highlight = dropdown.highlight == index;
            let hovered = dropdown.hovered == Some(index);
            if highlight || hovered {
                ctx.canvas.fill_rect(
                    Rect::new(
                        dropdown.rect.left + 1.0,
                        rect.top,
                        dropdown.rect.right - 1.0,
                        rect.bottom,
                    ),
                    if highlight {
                        pal.row_hover
                    } else {
                        pal.nav_hover
                    },
                );
            }
            ctx.text_overflow(
                Rect::new(rect.left + 8.0, rect.top, rect.right - 8.0, rect.bottom),
                option,
                TextStyleRole::Body,
                TextAlign::Left,
                pal.text,
                TextOverflow::Ellipsis,
            );
        }
        ctx.canvas.pop_clip();
    }
}

/// Editable-field frame, brighter on hover and accent-tinted as the cursor.
fn paint_field(ctx: &mut WidgetPaint<'_>, field: Rect, cursor: bool, hovered: bool) {
    let pal = ctx.palette;
    let border = if cursor {
        pal.accent
    } else if hovered {
        pal.explicit_border
    } else {
        pal.divider
    };
    ctx.canvas.fill_rect(field, pal.panel);
    paint_border(ctx, field, border);
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

/// A 15 DIP square check box with a two-stroke mark.
fn paint_check(
    ctx: &mut WidgetPaint<'_>,
    rect: Rect,
    checked: bool,
    locked: bool,
    pal: crate::Palette,
) {
    let border = if locked {
        pal.text_disabled
    } else {
        pal.explicit_border
    };
    ctx.canvas.fill_rect(rect, pal.panel);
    paint_border(ctx, rect, border);
    if !checked {
        return;
    }
    let mark = if locked {
        pal.text_disabled
    } else {
        pal.accent
    };
    // Down-right arm, then up-right arm; both are 2 DIP thick.
    for step in 0..4 {
        let x = rect.left + 3.0 + step as f32;
        let y = rect.top + 6.0 + step as f32;
        ctx.canvas
            .fill_rect(Rect::new(x, y, x + 2.0, y + 2.0), mark);
    }
    for step in 0..6 {
        let x = rect.left + 6.0 + step as f32;
        let y = rect.top + 9.0 - step as f32;
        ctx.canvas
            .fill_rect(Rect::new(x, y, x + 2.0, y + 2.0), mark);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Canvas;
    use crate::widget::edit_command;

    #[derive(Default)]
    struct Blank;

    impl Canvas for Blank {
        fn scale(&self) -> f32 {
            1.0
        }
        fn dpi(&self) -> f32 {
            96.0
        }
        fn fill_rect(&mut self, _rect: Rect, _color: crate::Color) {}
        fn text(
            &mut self,
            _rect: Rect,
            _text: &str,
            _style: &crate::backend::TextStyle,
            _align: TextAlign,
            _color: crate::Color,
        ) {
        }
        fn measure_text(
            &mut self,
            text: &str,
            _style: &crate::backend::TextStyle,
        ) -> crate::geometry::Size {
            crate::geometry::Size::new(text.chars().count() as f32 * 8.0, 16.0)
        }
        fn ellipse(&mut self, _rect: Rect, _color: crate::Color) {}
        fn vertical_gradient(&mut self, _rect: Rect, _top: crate::Color, _bottom: crate::Color) {}
        fn image(
            &mut self,
            _rect: Rect,
            _crop: bool,
            _id: crate::backend::ImageId,
            _data: crate::backend::ImageData<'_>,
        ) {
        }
        fn push_clip(&mut self, _rect: Rect) {}
        fn pop_clip(&mut self) {}
    }

    fn paint_blank<M: 'static>(widget: &dyn Widget<M>) {
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = Blank;
        let mut paint = WidgetPaint {
            painter: &painter,
            canvas: &mut canvas,
            palette: *painter.palette(),
            font: None,
            images: &images,
            viewport: Rect::from_xywh(0.0, 0.0, 400.0, 300.0),
            dirty: &[],
        };
        widget.paint(
            &mut paint,
            &WidgetState {
                enabled: true,
                focused: true,
                caret_on: true,
                ..Default::default()
            },
        );
    }

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

    /// Six rows over two sections: 名称 (text), 大小 (read-only), 收藏 (check),
    /// 评分 (rating), 音质 (choice), 播放次数 (number).
    fn sample(rows: usize) -> PropertyGrid<String> {
        let mut sections = vec![
            PropertySection::new("文件")
                .row(PropertyRow::new("名称", "Demo.flac").text())
                .row(PropertyRow::new("大小", "42.3 MB")),
            PropertySection::new("标签")
                .row(PropertyRow::new("收藏", "是").check("是", "否"))
                .row(PropertyRow::new("评分", "3").rating(5))
                .row(PropertyRow::new("音质", "无损").choice(["无损", "高品质", "标准"]))
                .row(PropertyRow::new("播放次数", "128").number(0.0, 999_999.0)),
        ];
        if rows > 6 {
            sections.push(
                PropertySection::new("更多").rows(
                    (6..rows)
                        .map(|index| PropertyRow::new(format!("键 {index}"), format!("值 {index}")))
                        .collect(),
                ),
            );
        }
        let mut grid = PropertyGrid::<String>::new("props")
            .sections(sections)
            .header_height(28.0)
            .row_height(26.0)
            .on_select(|key| format!("select:{key}"))
            .on_activate(|key, value| format!("open:{key}={value}"))
            .on_change(|key, value| format!("change:{key}={value}"));
        grid.set_rect(Rect::from_xywh(0.0, 0.0, 380.0, 240.0));
        grid
    }

    fn click(grid: &mut PropertyGrid<String>, pos: Point) -> Option<String> {
        grid.input(WidgetInput::HoverAt(pos));
        grid.input(WidgetInput::PressAt(pos));
        grid.input(WidgetInput::ReleaseAt(pos))
    }

    #[test]
    fn entries_flatten_headers_and_rows_in_scroll_order() {
        let grid = sample(6);
        assert_eq!(grid.entries.len(), 2 + 2 + 4, "two headers plus six rows");
        assert_eq!(grid.row_count(), 6);
        let first = &grid.entries[0];
        assert_eq!((first.section, first.row), (0, None));
        assert_eq!(first.top, 0.0);
        assert_eq!(grid.entries[1].top, 28.0);
        assert_eq!(grid.entries[2].top, 54.0);
        assert_eq!(grid.entries[3].top, 80.0, "the second header follows");
        assert_eq!(grid.content_height, 28.0 + 2.0 * 26.0 + 28.0 + 4.0 * 26.0);
        let rect = grid.row_rect(0, 0).unwrap();
        let value = grid.value_rect(0, 0).unwrap();
        assert!(value.left > rect.left + 100.0);
        assert!((value.top - rect.top - FIELD_INSET).abs() < 0.01);
        assert_eq!(grid.selected_key(), Some("名称"));
        paint_blank::<String>(&grid);
    }

    #[test]
    fn text_row_places_the_caret_selects_and_commits() {
        let mut grid = sample(6);
        let field = grid.value_rect(0, 0).unwrap();
        // A click on the value opens the session with the caret at the end;
        // the typed text appends instead of replacing.
        assert_eq!(click(&mut grid, field.center()), None);
        assert!(grid.editing());
        assert_eq!(
            grid.field.borrow().caret(),
            "Demo.flac".chars().count()
        );
        assert!(!grid.field.borrow().has_selection());
        assert!(grid.insert_text("!"));
        // Clicking near the left edge (after a paint cached the advances) moves
        // the caret without losing the typed text.
        paint_blank::<String>(&grid);
        let left = Point::new(field.left + 3.0, field.center().y);
        assert!(grid.begin_inline_edit(Some(left)));
        assert_eq!(grid.field.borrow().caret(), 0);
        assert!(grid.insert_text("新"));
        // A pointer drag selects a range; typing replaces exactly that range.
        paint_blank::<String>(&grid);
        let right = Point::new(field.left + 3.0 + 16.0, field.center().y);
        grid.begin_inline_edit(Some(left));
        grid.input(WidgetInput::DragTo(right));
        assert!(grid.field.borrow().has_selection());
        assert!(grid.insert_text("改"));
        assert_eq!(
            grid.finish_edit(true),
            Some("change:名称=改emo.flac!".to_string())
        );
        assert_eq!(grid.value("名称"), Some("改emo.flac!"));
        assert!(!grid.editing());
        // Escape reverts a keyboard-started draft (which starts selected).
        grid.begin_inline_edit(None);
        assert!(grid.field.borrow().has_selection());
        grid.insert_text("临时");
        assert_eq!(grid.finish_edit(false), None);
        assert_eq!(grid.value("名称"), Some("改emo.flac!"));
        paint_blank::<String>(&grid);
    }

    #[test]
    fn a_multiline_text_value_edits_across_lines() {
        let mut grid = PropertyGrid::<String>::new("props")
            .sections(vec![
                PropertySection::new("备注").row(PropertyRow::new("说明", "一\n二").text()),
            ])
            .on_change(|key, value| format!("change:{key}={value}"));
        grid.set_rect(Rect::from_xywh(0.0, 0.0, 380.0, 240.0));
        assert!(grid.begin_inline_edit(None));
        assert!(
            grid.multiline_edit(),
            "a value with a newline uses the multiline model"
        );
        assert!(grid.insert_text("甲\n乙"));
        // Enter inserts a line break; the container commits on Ctrl+Enter/Tab.
        grid.input(key(KeyCode::Enter));
        assert!(grid.editing(), "a plain Enter keeps the session open");
        assert_eq!(
            grid.finish_edit(true),
            Some("change:说明=甲\n乙\n".to_string())
        );
        assert_eq!(grid.value("说明"), Some("甲\n乙\n"));
        // A single-line value keeps the one-line contract.
        grid.set_sections(vec![
            PropertySection::new("备注").row(PropertyRow::new("短", "abc").text()),
        ]);
        assert!(grid.select_key("短"));
        assert!(grid.begin_inline_edit(None));
        assert!(!grid.multiline_edit());
        grid.finish_edit(false);
    }

    #[test]
    fn the_draft_has_a_standard_edit_menu_and_clipboard() {
        let mut grid = sample(6);
        grid.begin_inline_edit(None);
        let menu = grid.edit_menu().expect("a draft opens the built-in menu");
        assert_eq!(
            menu.command(edit_command::SELECT_ALL)
                .map(|item| item.label().to_string()),
            Some("全选(&A)".to_string())
        );
        assert_eq!(
            menu.command(edit_command::PASTE)
                .map(|item| item.label().to_string()),
            Some("粘贴(&P)".to_string())
        );
        // Copy reports the selected range through the clipboard seam.
        assert!(grid.edit_command(edit_command::COPY));
        assert_eq!(grid.take_clipboard().as_deref(), Some("Demo.flac"));
        // Cut copies and deletes it; the emptied draft stays open.
        grid.begin_inline_edit(None);
        assert!(grid.edit_command(edit_command::CUT));
        assert_eq!(grid.take_clipboard().as_deref(), Some("Demo.flac"));
        assert_eq!(grid.field.borrow().text(), "");
        grid.insert_text("x");
        assert!(grid.edit_menu().is_some());
        grid.finish_edit(false);
        assert!(grid.edit_menu().is_none(), "no draft, no edit menu");
    }

    #[test]
    fn number_draft_filters_input_clamps_and_steps() {
        let mut grid = sample(6);
        grid.select_key("播放次数");
        assert!(grid.begin_inline_edit(None));
        assert!(grid.insert_text("12a3.5"));
        assert_eq!(grid.field.borrow().text(), "123.5");
        // Up steps by one and rewrites the draft (123.5 + 1 rounds to 125).
        grid.input(key(KeyCode::Up));
        assert_eq!(grid.field.borrow().text(), "125");
        assert_eq!(
            grid.finish_edit(true),
            Some("change:播放次数=125".to_string())
        );
        // Out-of-range commits clamp; non-numeric drafts revert.
        grid.begin_inline_edit(None);
        grid.insert_text("9999999");
        assert_eq!(
            grid.finish_edit(true),
            Some("change:播放次数=999999".to_string())
        );
        grid.begin_inline_edit(None);
        grid.field.get_mut().set_text("-".to_string());
        assert_eq!(grid.finish_edit(true), None);
        assert_eq!(grid.value("播放次数"), Some("999999"));
    }

    #[test]
    fn check_rating_and_choice_edit_through_clicks() {
        let mut grid = sample(6);
        let check = grid.value_rect(1, 0).unwrap().center();
        assert_eq!(click(&mut grid, check), Some("change:收藏=否".to_string()));
        assert_eq!(grid.value("收藏"), Some("否"));
        assert_eq!(grid.checked("收藏"), Some(false));
        assert_eq!(click(&mut grid, check), Some("change:收藏=是".to_string()));

        // The rating field is split into five cells; the fourth star is 4.
        let rating = grid.value_rect(1, 1).unwrap();
        let fourth = Point::new(rating.left + rating.width() * 0.7, rating.center().y);
        assert_eq!(click(&mut grid, fourth), Some("change:评分=4".to_string()));
        assert_eq!(grid.rating("评分"), Some(4));
        // Left/Right adjust the cursor row.
        grid.select_key("评分");
        assert_eq!(
            grid.input(key(KeyCode::Right)),
            Some("change:评分=5".to_string())
        );
        assert_eq!(
            grid.input(key(KeyCode::Left)),
            Some("change:评分=4".to_string())
        );

        // A choice opens a list; the second option is picked by a click.
        let choice = grid.value_rect(1, 2).unwrap().center();
        assert_eq!(click(&mut grid, choice), None);
        assert!(grid.overlay());
        let second = grid.dropdown.as_ref().unwrap().item_rect(1).center();
        assert_eq!(
            click(&mut grid, second),
            Some("change:音质=高品质".to_string())
        );
        assert_eq!(grid.value("音质"), Some("高品质"));
        assert!(!grid.overlay());

        // Re-open and pick the third option with the keyboard.
        click(&mut grid, choice);
        grid.input(key(KeyCode::Down));
        assert_eq!(
            grid.input(key(KeyCode::Enter)),
            Some("change:音质=标准".to_string())
        );
        assert_eq!(grid.value("音质"), Some("标准"));
        assert!(!grid.overlay());
    }

    #[test]
    fn read_only_rows_and_the_locked_grid_stay_navigable() {
        let mut grid = sample(6);
        assert!(grid.set_row_read_only(1, 0, true));
        let check = grid.value_rect(1, 0).unwrap().center();
        // A locked row still selects but does not toggle.
        assert_eq!(click(&mut grid, check), Some("select:收藏".to_string()));
        assert_eq!(grid.value("收藏"), Some("是"));
        assert!(
            !grid.set_row_read_only(1, 0, true),
            "unchanged reports false"
        );

        grid.read_only_props = true;
        assert!(grid.is_read_only());
        let field = grid.value_rect(0, 0).unwrap().center();
        assert_eq!(click(&mut grid, field), Some("select:名称".to_string()));
        assert!(!grid.editing(), "a locked grid opens no text session");
        assert!(!grid.begin_inline_edit(None));
        let rating = grid.value_rect(1, 1).unwrap().center();
        assert_eq!(grid.set_rating_at(3, rating), None);
        assert_eq!(grid.value("评分"), Some("3"));
        paint_blank::<String>(&grid);
    }

    #[test]
    fn read_only_rows_double_click_through_on_activate() {
        let mut grid = sample(6);
        let size_row = grid.row_rect(0, 1).unwrap().center();
        assert_eq!(
            grid.input(WidgetInput::DoubleClickAt(size_row)),
            Some("open:大小=42.3 MB".to_string())
        );
    }

    #[test]
    fn keyboard_cursor_skips_headers_and_enter_starts_the_row_editor() {
        let mut grid = sample(6);
        let first = grid.selected;
        grid.input(key(KeyCode::Up));
        assert_eq!(grid.selected, first, "Up on the first row stays there");
        grid.input(key(KeyCode::Down));
        assert_eq!(grid.selected_key(), Some("大小"));
        grid.input(key(KeyCode::Down));
        assert_eq!(
            grid.selected_key(),
            Some("收藏"),
            "the header between the sections is skipped"
        );
        // Enter on a check row toggles it.
        assert_eq!(
            grid.input(key(KeyCode::Enter)),
            Some("change:收藏=否".to_string())
        );
        grid.input(key(KeyCode::Home));
        assert_eq!(grid.selected_key(), Some("名称"));
        assert_eq!(
            grid.input(key(KeyCode::Enter)),
            None,
            "Enter on a text row opens its draft"
        );
        assert!(grid.editing());
        grid.finish_edit(false);
        for index in 0..10 {
            grid.input(key(KeyCode::Down));
            assert!(
                grid.entries[grid.selected.unwrap()].row.is_some(),
                "step {index} landed on a header"
            );
        }
    }

    #[test]
    fn typed_access_updates_values_sections_and_selection() {
        let mut grid = sample(6);
        assert_eq!(grid.value("音质"), Some("无损"));
        assert!(grid.set_value("音质", "标准"));
        assert!(!grid.set_value("音质", "标准"), "unchanged reports false");
        assert!(!grid.set_value("缺失", "x"));
        assert!(grid.set_checked("收藏", false));
        assert_eq!(grid.checked("收藏"), Some(false));
        assert!(grid.set_rating("评分", 5));
        assert_eq!(grid.rating("评分"), Some(5));
        assert!(
            !grid.set_rating("评分", 9),
            "clamped to the current value is a no-op"
        );
        assert_eq!(grid.rating("评分"), Some(5));
        assert!(grid.set_key("名称", "文件名"));
        assert_eq!(grid.value("文件名"), Some("Demo.flac"));
        assert!(grid.select_key("文件名"));
        assert_eq!(grid.selected_path(), Some((0, 0)));
        // Replacing a section keeps the cursor on its surviving key.
        assert!(grid.set_section_rows(
            1,
            vec![
                PropertyRow::new("收藏", "否").check("是", "否"),
                PropertyRow::new("新增", "0"),
            ]
        ));
        assert_eq!(grid.selected_key(), Some("文件名"));
        assert_eq!(grid.row_count(), 4);
        let sections = vec![PropertySection::new("仅一")];
        assert!(grid.set_sections(sections));
        assert_eq!(grid.row_count(), 0);
        paint_blank::<String>(&grid);
    }

    #[test]
    fn scrolling_scrolls_wheel_keys_and_scroll_to_key() {
        let mut grid = sample(40);
        assert!(grid.has_scrollbar());
        grid.input(WidgetInput::Wheel(-3.0));
        assert!(grid.scroll_offset() > 0.0);
        grid.input(WidgetInput::Wheel(-100_000.0));
        assert!((grid.scroll_offset() - grid.max_scroll()).abs() < 0.01);
        assert!(grid.scroll_to_key("名称"));
        assert_eq!(grid.scroll_offset(), 0.0);
        assert!(!grid.scroll_to_key("缺失"));
        assert!(grid.ensure_key_visible("键 30"));
        let before = grid.selected_key().map(str::to_string);
        let track = grid.scrollbar_rect();
        let thumb = grid.thumb_rect().unwrap();
        let over_thumb = Point::new(track.center().x, thumb.center().y);
        assert_eq!(grid.cursor_at(over_thumb), Some(Cursor::Arrow));
        grid.input(WidgetInput::PressAt(over_thumb));
        grid.input(WidgetInput::DragTo(Point::new(
            over_thumb.x,
            over_thumb.y + 20.0,
        )));
        assert_eq!(grid.selected_key().map(str::to_string), before);
        grid.input(WidgetInput::ReleaseAt(Point::new(
            over_thumb.x,
            over_thumb.y + 20.0,
        )));
        assert!(!grid.bar.active);
    }

    #[test]
    fn a11y_publishes_headers_rows_and_the_scroll_pattern() {
        let grid = sample(40);
        let info = grid.a11y().expect("grid is exposed");
        assert_eq!(info.role, A11yRole::List);
        let first = &info.children[0];
        assert_eq!(first.role, Some(A11yRole::Heading));
        assert_eq!(first.name, "文件");
        assert_eq!(info.children[1].name, "名称: Demo.flac");
        assert!(info.children[1].selected, "the cursor row is selected");
        let last = info.children.last().unwrap();
        assert!(last.offscreen, "the look-ahead entry is offscreen");
        let scroll = info.scroll.expect("scroll pattern");
        assert!(scroll.vertical.is_some_and(|axis| axis.scrollable()));
        assert!(info.children.len() < grid.entries.len());

        let mut grid = sample(6);
        assert!(grid.set_row_read_only(0, 1, true));
        let info = grid.a11y().unwrap();
        let locked = info
            .children
            .iter()
            .find(|child| child.name.starts_with("大小:"))
            .unwrap();
        assert!(!locked.enabled, "a locked row publishes IsEnabled=false");
        // Selecting through a11y moves the cursor without editing.
        let index = grid
            .entries
            .iter()
            .position(|entry| entry.section == 1 && entry.row == Some(0))
            .unwrap();
        assert_eq!(
            grid.a11y_select_child(index),
            Some("select:收藏".to_string())
        );
        assert_eq!(grid.selected_key(), Some("收藏"));
        assert_eq!(grid.value("收藏"), Some("是"), "selection does not toggle");
        assert_eq!(grid.a11y_select_child(0), None, "a header is refused");
    }

    #[test]
    fn a11y_set_value_edits_rows_through_the_commit_path() {
        let mut grid = sample(6);
        let info = grid.a11y().unwrap();
        let index_of = |prefix: &str| {
            info.children
                .iter()
                .find(|child| child.name.starts_with(prefix))
                .map(|child| child.index)
                .unwrap()
        };
        let name_row = index_of("名称:");
        let editable = info
            .children
            .iter()
            .find(|child| child.index == name_row)
            .unwrap();
        assert_eq!(editable.value.as_deref(), Some("Demo.flac"));
        assert!(!editable.read_only, "an editable row accepts SetValue");
        let read_only_row = info
            .children
            .iter()
            .find(|child| child.name.starts_with("大小:"))
            .unwrap();
        assert!(
            read_only_row.value.is_none(),
            "a read-only kind publishes no value"
        );

        assert_eq!(
            grid.a11y_set_value_at(Some(name_row), "Demo 2.flac"),
            Some("change:名称=Demo 2.flac".to_string())
        );
        assert_eq!(grid.value("名称"), Some("Demo 2.flac"));

        // Numbers parse, clamp and format; a bad draft changes nothing.
        let number_row = index_of("播放次数:");
        assert_eq!(
            grid.a11y_set_value_at(Some(number_row), " 77 "),
            Some("change:播放次数=77".to_string())
        );
        assert_eq!(grid.value("播放次数"), Some("77"));
        assert_eq!(grid.a11y_set_value_at(Some(number_row), "lots"), None);
        assert_eq!(
            grid.a11y_set_value_at(Some(number_row), "999999999"),
            Some("change:播放次数=999999".to_string()),
            "the value clamps to the range"
        );

        // A choice must name an option; check accepts its texts or booleans;
        // rating clamps to the star count.
        let choice_row = index_of("音质:");
        assert_eq!(
            grid.a11y_set_value_at(Some(choice_row), "标准"),
            Some("change:音质=标准".to_string())
        );
        assert_eq!(grid.a11y_set_value_at(Some(choice_row), "超高"), None);
        let check_row = index_of("收藏:");
        assert_eq!(
            grid.a11y_set_value_at(Some(check_row), "false"),
            Some("change:收藏=否".to_string())
        );
        let rating_row = index_of("评分:");
        assert_eq!(
            grid.a11y_set_value_at(Some(rating_row), "9"),
            Some("change:评分=5".to_string())
        );
        assert_eq!(grid.a11y_set_value_at(Some(rating_row), "x"), None);
        // Headers and locked rows refuse.
        assert_eq!(grid.a11y_set_value_at(Some(0), "x"), None);
        assert!(grid.set_row_read_only(0, 0, true));
        assert_eq!(grid.a11y_set_value_at(Some(name_row), "x"), None);
    }

    #[test]
    fn a11y_scroll_moves_the_offset() {
        let mut grid = sample(40);
        assert!(grid.a11y_scroll(None, Some(40.0)));
        assert!((grid.scroll_offset() - 40.0).abs() < 0.01);
        assert!(!grid.a11y_scroll(None, Some(40.0)));
        assert!(grid.a11y_scroll(None, Some(-10.0)));
        assert_eq!(grid.scroll_offset(), 0.0);
    }

    #[test]
    fn the_section_child_list_replaces_the_grid_data() {
        let mut grid: PropertyGrid<()> = PropertyGrid::new("props");
        let mut name = SkinElement::new("row", 3, 5);
        name.push_attr("key", "名称");
        name.push_attr("value", "夜曲");
        let mut volume = SkinElement::new("row", 4, 5);
        volume.push_attr("key", "音量");
        volume.push_attr("value", "72");
        volume.push_attr("kind", "number");
        volume.push_attr("min", "0");
        volume.push_attr("max", "100");
        volume.push_attr("step", "2");
        volume.push_attr("decimals", "1");
        let mut quality = SkinElement::new("row", 5, 5);
        quality.push_attr("key", "音质");
        quality.push_attr("value", "无损");
        quality.push_attr("kind", "choice");
        quality.push_attr("options", "无损|高品质|标准");
        let mut favourite = SkinElement::new("row", 6, 5);
        favourite.push_attr("key", "收藏");
        favourite.push_attr("value", "是");
        favourite.push_attr("kind", "check");
        favourite.push_attr("on", "是");
        favourite.push_attr("off", "否");
        favourite.push_attr("read-only", "true");
        let mut section = SkinElement::new("section", 2, 3);
        section.push_attr("title", "音频");
        for row in [name, volume, quality, favourite] {
            section.push_child(row);
        }
        let children = [section];
        assert!(<PropertyGrid<()> as Widget<()>>::validate_skin_children(&grid, &children));
        <PropertyGrid<()> as Widget<()>>::apply_skin_children(&mut grid, &children);
        let sections = grid.sections_ref();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title(), "音频");
        assert_eq!(sections[0].rows_ref().len(), 4);
        assert_eq!(sections[0].rows_ref()[0].key(), "名称");
        assert_eq!(sections[0].rows_ref()[0].kind(), &PropertyKind::ReadOnly);
        assert_eq!(
            sections[0].rows_ref()[1].kind(),
            &PropertyKind::Number {
                min: 0.0,
                max: 100.0,
                step: 2.0,
                decimals: 1
            }
        );
        assert_eq!(
            sections[0].rows_ref()[2].kind(),
            &PropertyKind::Choice(vec!["无损".into(), "高品质".into(), "标准".into()])
        );
        assert!(sections[0].rows_ref()[3].is_read_only());

        let section_of = |row: SkinElement| {
            let mut section = SkinElement::new("section", 99, 3);
            section.push_child(row);
            [section]
        };
        // A number-only attribute on a text row is a typo.
        let mut stray = SkinElement::new("row", 8, 5);
        stray.push_attr("key", "x");
        stray.push_attr("kind", "text");
        stray.push_attr("min", "0");
        assert!(!<PropertyGrid<()> as Widget<()>>::validate_skin_children(
            &grid,
            &section_of(stray)
        ));
        // An unknown kind and a choice without legal options are rejected.
        let mut unknown_kind = SkinElement::new("row", 9, 5);
        unknown_kind.push_attr("key", "x");
        unknown_kind.push_attr("kind", "dropdown");
        assert!(!<PropertyGrid<()> as Widget<()>>::validate_skin_children(
            &grid,
            &section_of(unknown_kind)
        ));
        let mut empty_choice = SkinElement::new("row", 10, 5);
        empty_choice.push_attr("key", "x");
        empty_choice.push_attr("kind", "choice");
        empty_choice.push_attr("options", "a||b");
        assert!(!<PropertyGrid<()> as Widget<()>>::validate_skin_children(
            &grid,
            &section_of(empty_choice)
        ));
    }
}
