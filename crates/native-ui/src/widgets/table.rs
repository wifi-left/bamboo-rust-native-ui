// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Virtualized table: a fixed header band plus rows painted from data.
//!
//! Columns are geometry only (fixed DIP widths plus flex columns sharing the
//! remainder), rows are `Vec<String>` cells, and only the rows that intersect
//! the body are painted, hit-tested or published to accessibility — a
//! thousand-row table costs a viewport's worth of work per frame. Sorting is
//! application-owned: clicking a header emits its column index, the
//! application re-sorts and calls [`Table::set_sort`], which paints the
//! indicator. This keeps the widget free of comparison rules (numbers, dates,
//! locale collation) it cannot know.

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

static TABLE_SKIN_CHILDREN: &[&str] = &["column", "row"];
static TABLE_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("row-height", SkinValueKind::Number),
    SkinAttrDecl::new("header-height", SkinValueKind::Number),
    SkinAttrDecl::new("cell-padding", SkinValueKind::Number),
    SkinAttrDecl::new("striped", SkinValueKind::Bool),
    SkinAttrDecl::new("grid-lines", SkinValueKind::Bool),
    SkinAttrDecl::new("editable", SkinValueKind::Bool),
    SkinAttrDecl::new("empty-text", SkinValueKind::Text),
];

/// One column: a header title and a width.
///
/// A fixed column takes `width` DIPs. A flex column ([`TableColumn::flex`])
/// shares the space the fixed columns leave, so a "name" column can absorb the
/// window width while numeric columns stay aligned.
#[derive(Clone, Debug, PartialEq)]
pub struct TableColumn {
    title: String,
    width: f32,
    flex: bool,
    align: TextAlign,
}

impl TableColumn {
    pub fn new(title: impl Into<String>, width: f32) -> Self {
        Self {
            title: title.into(),
            width: width.max(0.0),
            flex: false,
            align: TextAlign::Left,
        }
    }

    /// A column that shares the leftover width with the other flex columns.
    pub fn flex(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: 0.0,
            flex: true,
            align: TextAlign::Left,
        }
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn is_flex(&self) -> bool {
        self.flex
    }

    pub fn alignment(&self) -> TextAlign {
        self.align
    }

    pub fn set_title(&mut self, title: impl Into<String>) -> bool {
        let title = title.into();
        if self.title == title {
            return false;
        }
        self.title = title;
        true
    }

    /// The fixed child vocabulary of a `<column>` element.
    const SKIN_ATTRS: &'static [&'static str] = &["title", "width", "flex", "align"];

    /// Build a column from a skin child element, or `None` when the required
    /// `title` is missing. The caller validates first.
    pub(crate) fn from_skin(element: &SkinElement) -> Option<Self> {
        let title = element.attr("title")?;
        let align = match element.attr("align") {
            Some("center") => TextAlign::Center,
            Some("right") => TextAlign::Right,
            _ => TextAlign::Left,
        };
        Some(if element.attr_bool("flex").unwrap_or(false) {
            Self::flex(title).align(align)
        } else {
            let width = element.attr_number("width").unwrap_or(0.0) as f32;
            Self::new(title, width).align(align)
        })
    }

    /// Validate one `<column>` child element: a non-empty title, the fixed
    /// vocabulary, a non-negative width and a known alignment.
    pub(crate) fn valid_skin_column(element: &SkinElement) -> bool {
        element
            .attr("title")
            .is_some_and(|title| !title.trim().is_empty())
            && element.attrs_are(Self::SKIN_ATTRS)
            && element.children.is_empty()
            && element.attr_number_ok("width")
            && element.attr_number("width").is_none_or(|width| width >= 0.0)
            && element.attr_bool_ok("flex")
            && element
                .attr("align")
                .is_none_or(|align| matches!(align, "left" | "center" | "right"))
    }
}

/// One table row: one string per column. Missing cells paint empty, so a
/// column added at runtime does not break existing data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableRow {
    pub cells: Vec<String>,
    pub disabled: bool,
}

impl TableRow {
    pub fn new<I, S>(cells: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            cells: cells.into_iter().map(Into::into).collect(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Cell text, or `""` when the row is shorter than the column count.
    pub fn cell(&self, column: usize) -> &str {
        self.cells.get(column).map(String::as_str).unwrap_or("")
    }

    /// The fixed child vocabulary of a `<row>` element and of its `<cell>`
    /// children.
    const SKIN_ATTRS: &'static [&'static str] = &["disabled"];
    const CELL_SKIN_ATTRS: &'static [&'static str] = &["text"];

    /// Build a row from a skin child element; an empty `<cell text=""/>` is a
    /// legitimate empty cell. The caller validates first.
    pub(crate) fn from_skin(element: &SkinElement) -> Option<Self> {
        Some(Self {
            cells: element
                .children
                .iter()
                .map(|cell| cell.attr("text").unwrap_or("").to_string())
                .collect(),
            disabled: element.attr_bool("disabled").unwrap_or(false),
        })
    }

    /// Validate one `<row>` child element: a fixed attribute set, a good
    /// `disabled` flag, and only `<cell text>` children.
    pub(crate) fn valid_skin_row(element: &SkinElement) -> bool {
        element.attrs_are(Self::SKIN_ATTRS)
            && element.attr_bool_ok("disabled")
            && element.children.iter().all(|cell| {
                cell.name == "cell"
                    && cell.attr("text").is_some()
                    && cell.attrs_are(Self::CELL_SKIN_ATTRS)
                    && cell.children.is_empty()
            })
    }
}

pub struct Table<M> {
    id: WidgetId,
    rect: Rect,
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    header_height: f32,
    row_height: f32,
    cell_padding: f32,
    scroll: f32,
    /// Horizontal offset; `0` until the fixed columns overflow the viewport.
    scroll_x: f32,
    hover_row: Option<usize>,
    hover_header: Option<usize>,
    press_row: Option<usize>,
    press_header: Option<usize>,
    /// Press x of the last header press (it separates a click from a drag).
    press_x: f32,
    /// In-flight drag of a header divider.
    resizing: Option<ColumnResize>,
    /// In-flight header drag that moves a whole column.
    reordering: Option<ColumnDrag>,
    bar: ScrollBarState,
    /// Horizontal bar state (only visible while the columns overflow).
    hbar: ScrollBarState,
    style: ScrollBarStyle,
    /// Narrow repaint for the input that just ran (bar chrome only).
    repaint_hint: Option<Rect>,
    selected: usize,
    /// Cursor column; paired with `selected` it names the active cell.
    selected_col: usize,
    /// Cells can be edited in place (Enter, F2 or a double click).
    editable: bool,
    /// The instance is locked (`WidgetProps::read_only`): cells stay
    /// navigable but no draft opens.
    read_only: bool,
    /// Open inline cell session.
    draft: Option<CellDraft>,
    /// The embedded painted field that edits the draft cell.
    field: RefCell<PaintedField<M>>,
    striped: bool,
    grid_lines: bool,
    /// `(column, ascending)` as last set by the application.
    sort: Option<(usize, bool)>,
    empty_text: String,
    on_select: Option<Box<dyn Fn(usize) -> M>>,
    on_activate: Option<Box<dyn Fn(usize) -> M>>,
    on_header: Option<Box<dyn Fn(usize) -> M>>,
    on_column_resize: Option<Box<dyn Fn(usize, f32) -> M>>,
    on_column_reorder: Option<Box<dyn Fn(usize, usize) -> M>>,
    on_cell_edit: Option<Box<dyn Fn(usize, usize, String) -> M>>,
}

/// A header-divider drag: the fixed column being widened and the press offset.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ColumnResize {
    column: usize,
    start_x: f32,
    start_width: f32,
}

/// A header drag that moves a whole column: the dragged column and the
/// insertion slot (`0..=len` in pre-removal index space).
#[derive(Clone, Copy, Debug, PartialEq)]
struct ColumnDrag {
    from: usize,
    insert: usize,
}

/// An open inline cell session: which cell the embedded painted field covers.
struct CellDraft {
    row: usize,
    column: usize,
}

/// Space reserved at a header cell's right edge for the sort arrow (kept even
/// when unsorted, so a title never shifts when the sort column changes).
const SORT_ARROW_ROOM: f32 = 16.0;
/// Width a column can never be dragged below.
const MIN_COLUMN_WIDTH: f32 = 24.0;
/// Grabbable strip around a header divider, on each side.
const RESIZE_GRAB: f32 = 4.0;
/// Horizontal travel before a header press becomes a reorder drag.
const REORDER_DRAG: f32 = 6.0;
/// Widget-level child indices for column headers start above any cell index, so
/// a UIA SelectionItem request on a header can never select a real cell.
const HEADER_INDEX_BASE: usize = 1 << 30;

impl<M> Table<M> {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 480.0, 260.0),
            columns: Vec::new(),
            rows: Vec::new(),
            header_height: 30.0,
            row_height: 32.0,
            cell_padding: 10.0,
            scroll: 0.0,
            scroll_x: 0.0,
            hover_row: None,
            hover_header: None,
            press_row: None,
            press_header: None,
            press_x: 0.0,
            resizing: None,
            reordering: None,
            bar: ScrollBarState::default(),
            hbar: ScrollBarState::default(),
            style: ScrollBarStyle::default(),
            repaint_hint: None,
            selected: 0,
            selected_col: 0,
            editable: false,
            read_only: false,
            draft: None,
            field: RefCell::new(PaintedField::new()),
            striped: false,
            grid_lines: true,
            sort: None,
            empty_text: String::new(),
            on_select: None,
            on_activate: None,
            on_header: None,
            on_column_resize: None,
            on_column_reorder: None,
            on_cell_edit: None,
        }
    }

    pub fn columns(mut self, columns: Vec<TableColumn>) -> Self {
        self.columns = columns;
        self.clamp_sort();
        self
    }

    pub fn rows(mut self, rows: Vec<TableRow>) -> Self {
        self.set_rows(rows);
        self
    }

    pub fn header_height(mut self, height: f32) -> Self {
        self.header_height = height.max(0.0);
        self.clamp_scroll();
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(16.0);
        self.clamp_scroll();
        self
    }

    /// Horizontal padding inside every header/cell rectangle.
    pub fn cell_padding(mut self, padding: f32) -> Self {
        self.cell_padding = padding.max(0.0);
        self
    }

    /// Tint every other row with the panel color.
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }

    /// Row separators and column lines (default on).
    pub fn grid_lines(mut self, grid: bool) -> Self {
        self.grid_lines = grid;
        self
    }

    /// Centered message painted while there are no rows.
    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.empty_text = text.into();
        self
    }

    /// Initial sort indicator; the application still sorts the data.
    pub fn sort(mut self, column: usize, ascending: bool) -> Self {
        self.sort = Some((column, ascending));
        self.clamp_sort();
        self
    }

    /// A row was clicked (it is selected before this fires).
    pub fn on_select(mut self, handler: impl Fn(usize) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    /// Enter or a double click on the keyboard/clicked row.
    pub fn on_activate(mut self, handler: impl Fn(usize) -> M + 'static) -> Self {
        self.on_activate = Some(Box::new(handler));
        self
    }

    /// A header cell was clicked; return the column index. The application
    /// re-sorts and calls [`Table::set_sort`].
    pub fn on_header_click(mut self, handler: impl Fn(usize) -> M + 'static) -> Self {
        self.on_header = Some(Box::new(handler));
        self
    }

    /// A fixed column's width changed through its header divider (the widget
    /// already shows the new width). The application can persist the layout
    /// and push it back with [`Table::set_columns`].
    pub fn on_column_resize(mut self, handler: impl Fn(usize, f32) -> M + 'static) -> Self {
        self.on_column_resize = Some(Box::new(handler));
        self
    }

    /// A header drag moved a column. The widget has already applied the move
    /// (`from` → `to`, the same as `remove(from)` then `insert(to)`), so the
    /// application only needs to persist the new order.
    pub fn on_column_reorder(mut self, handler: impl Fn(usize, usize) -> M + 'static) -> Self {
        self.on_column_reorder = Some(Box::new(handler));
        self
    }

    /// Let the table edit cells in place. Enter on the cursor cell, `F2` or a
    /// double click starts a typed session (Enter/Tab commit, Escape cancels);
    /// the widget already shows the new text when the handler runs, so the
    /// application only needs to store it (and may re-sort).
    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }

    /// A cell was edited in place: `(row, column, text)` after the widget
    /// applied the draft.
    pub fn on_cell_edit(
        mut self,
        handler: impl Fn(usize, usize, String) -> M + 'static,
    ) -> Self {
        self.on_cell_edit = Some(Box::new(handler));
        self
    }

    pub fn scrollbar_style(mut self, style: ScrollBarStyle) -> Self {
        self.style = style;
        self
    }

    pub fn scrollbar(&self) -> &ScrollBarStyle {
        &self.style
    }

    // --- runtime appearance (skin locals) ---------------------------------

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

    /// Header band height in DIPs; `0` hides the header. Returns whether it
    /// changed.
    pub fn set_header_height(&mut self, height: f32) -> bool {
        let height = height.max(0.0);
        if (self.header_height - height).abs() < 0.01 {
            return false;
        }
        self.header_height = height;
        self.clamp_scroll();
        true
    }

    pub fn header_height_value(&self) -> f32 {
        self.header_height
    }

    /// Horizontal padding inside header/cell rectangles. Returns whether it
    /// changed.
    pub fn set_cell_padding(&mut self, padding: f32) -> bool {
        let padding = padding.max(0.0);
        if (self.cell_padding - padding).abs() < 0.01 {
            return false;
        }
        self.cell_padding = padding;
        true
    }

    pub fn cell_padding_value(&self) -> f32 {
        self.cell_padding
    }

    /// Tint every other row with the panel color. Returns whether it changed.
    pub fn set_striped(&mut self, striped: bool) -> bool {
        if self.striped == striped {
            return false;
        }
        self.striped = striped;
        true
    }

    pub fn is_striped(&self) -> bool {
        self.striped
    }

    /// Row separators and column lines. Returns whether it changed.
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

    /// In-place cell editing; locking closes an open draft. Returns whether it
    /// changed.
    pub fn set_editable(&mut self, editable: bool) -> bool {
        if self.editable == editable {
            return false;
        }
        self.editable = editable;
        if !editable {
            self.close_draft();
        }
        true
    }

    pub fn is_editable(&self) -> bool {
        self.editable
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

    // --- typed access -----------------------------------------------------

    pub fn columns_ref(&self) -> &[TableColumn] {
        &self.columns
    }

    pub fn rows_ref(&self) -> &[TableRow] {
        &self.rows
    }

    pub fn row(&self, index: usize) -> Option<&TableRow> {
        self.rows.get(index)
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Keyboard/click row cursor.
    pub fn selected_row(&self) -> usize {
        self.selected
    }

    /// `(first, end)` rows intersecting the body, end exclusive. The last
    /// published row is the look-ahead row the container marks offscreen.
    pub fn visible_rows(&self) -> (usize, usize) {
        self.visible_range()
    }

    /// The active cell `(row, column)`.
    pub fn selected_cell(&self) -> (usize, usize) {
        (self.selected, self.selected_col.min(self.columns.len().saturating_sub(1)))
    }

    /// Move the cell cursor. The row cursor is clamped like a click; returns
    /// whether anything moved.
    pub fn select_cell(&mut self, row: usize, column: usize) -> bool {
        if self.rows.is_empty() || self.columns.is_empty() {
            return false;
        }
        let row = row.min(self.rows.len() - 1);
        let column = column.min(self.columns.len() - 1);
        let before = (self.selected, self.selected_col);
        self.selected = row;
        self.selected_col = column;
        self.ensure_selected_visible();
        before != (self.selected, self.selected_col)
    }

    /// Cell text, or `None` for an out-of-range cell.
    pub fn cell_text(&self, row: usize, column: usize) -> Option<&str> {
        self.rows.get(row).map(|row| row.cell(column))
    }

    /// Client rectangle of a cell, clipped to the scrolling body. The
    /// application can use it to place its own editor over a cell.
    pub fn cell_rect(&self, row: usize, column: usize) -> Option<Rect> {
        if row >= self.rows.len() || column >= self.columns.len() {
            return None;
        }
        let body = self.body_rect();
        let top = body.top + row as f32 * self.row_height - self.scroll;
        let (left, width) = self.column_span(column)?;
        let rect = Rect::new(left, top, left + width, top + self.row_height);
        Some(rect.intersect(self.content_rect()).unwrap_or(rect))
    }

    /// Whether an inline cell session is open.
    pub fn is_editing_cell(&self) -> bool {
        self.draft.is_some()
    }

    /// Start editing one cell (the same path as Enter/F2). Returns whether the
    /// session opened.
    pub fn edit_cell(&mut self, row: usize, column: usize) -> bool {
        self.begin_draft_at(row, column, None)
    }

    pub fn scroll_offset(&self) -> f32 {
        self.scroll
    }

    /// Horizontal scroll offset (nonzero only while the columns overflow).
    pub fn scroll_x_offset(&self) -> f32 {
        self.scroll_x
    }

    /// Set a fixed column's width the way a divider drag would. Returns
    /// whether the width changed; a flex column ignores it (its width is the
    /// leftover).
    pub fn set_column_width(&mut self, index: usize, width: f32) -> bool {
        let Some(column) = self.columns.get_mut(index) else {
            return false;
        };
        if column.flex {
            return false;
        }
        let width = width.max(MIN_COLUMN_WIDTH);
        if (column.width - width).abs() < 0.01 {
            return false;
        }
        column.width = width;
        self.clamp_scroll();
        true
    }

    /// Scroll horizontally to `offset` (clamped to the overflow).
    pub fn set_scroll_x(&mut self, offset: f32) -> bool {
        let before = self.scroll_x;
        self.scroll_x = offset.clamp(0.0, self.max_scroll_x());
        (self.scroll_x - before).abs() > 0.01
    }

    /// Move one column to a new position the way a header drag does. The sort
    /// indicator follows its column. Returns whether the order changed.
    pub fn move_column(&mut self, from: usize, to: usize) -> bool {
        let len = self.columns.len();
        if from >= len || to >= len || from == to {
            return false;
        }
        let column = self.columns.remove(from);
        self.columns.insert(to, column);
        if let Some((sort, ascending)) = self.sort {
            self.sort = Some((remap_index(sort, from, to), ascending));
        }
        // The cell cursor follows the column it sat on.
        self.selected_col = remap_index(self.selected_col, from, to);
        true
    }

    pub fn sort_order(&self) -> Option<(usize, bool)> {
        self.sort
    }

    /// Paint a sort indicator for `column` (ascending/descending) or clear it
    /// with `None`. The widget does not reorder rows itself. Returns whether
    /// the state changed.
    pub fn set_sort(&mut self, sort: Option<(usize, bool)>) -> bool {
        if let Some((column, _)) = sort
            && column >= self.columns.len()
        {
            return false;
        }
        if self.sort == sort {
            return false;
        }
        self.sort = sort;
        true
    }

    /// Replace the column set. The sort indicator is dropped when its column
    /// no longer exists.
    pub fn set_columns(&mut self, columns: Vec<TableColumn>) -> bool {
        if self.columns == columns {
            return false;
        }
        self.columns = columns;
        self.clamp_sort();
        self.selected_col = self
            .selected_col
            .min(self.columns.len().saturating_sub(1));
        self.close_draft();
        true
    }

    pub fn set_rows(&mut self, rows: Vec<TableRow>) -> bool {
        if self.rows == rows {
            self.clamp_scroll();
            return false;
        }
        self.rows = rows;
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
        // A draft points at a row index that may now hold different data.
        self.close_draft();
        self.clamp_scroll();
        self.ensure_selected_visible();
        true
    }

    pub fn set_cell(&mut self, row: usize, column: usize, text: impl Into<String>) -> bool {
        let text = text.into();
        let Some(row) = self.rows.get_mut(row) else {
            return false;
        };
        if row.cells.len() <= column {
            row.cells.resize(column + 1, String::new());
        }
        if row.cells[column] == text {
            return false;
        }
        row.cells[column] = text;
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

    /// Scroll the minimum distance that makes `row` fully visible.
    pub fn ensure_row_visible(&mut self, row: usize) -> bool {
        if self.rows.is_empty() {
            return false;
        }
        let row = row.min(self.rows.len() - 1);
        let before = self.scroll;
        let top = row as f32 * self.row_height;
        let bottom = top + self.row_height;
        let viewport = self.content_rect().height();
        if top < self.scroll {
            self.scroll = top;
        } else if bottom > self.scroll + viewport {
            self.scroll = bottom - viewport;
        }
        self.clamp_scroll();
        (self.scroll - before).abs() > 0.01
    }

    // --- geometry ---------------------------------------------------------

    fn body_rect(&self) -> Rect {
        Rect::new(
            self.rect.left,
            (self.rect.top + self.header_height).min(self.rect.bottom),
            self.rect.right,
            self.rect.bottom,
        )
    }

    fn content_height(&self) -> f32 {
        self.rows.len() as f32 * self.row_height
    }

    /// `(vertical bar, horizontal bar)` for the current content and viewport.
    /// A bar shrinks the room the other axis has, so both are resolved
    /// together; the loop converges because each bar either appears or
    /// disappears once.
    fn bars_visible(&self) -> (bool, bool) {
        let body = self.body_rect();
        let thickness = self.style.inset + self.style.thickness;
        let fixed = self.width_split().0;
        let (mut v, mut h) = (false, false);
        for _ in 0..3 {
            let visible_w = body.width() - if v { thickness } else { 0.0 };
            let visible_h = body.height() - if h { thickness } else { 0.0 };
            let need_v = self.content_height() > visible_h + 0.5;
            let need_h = fixed > visible_w + 0.5;
            if (need_v, need_h) == (v, h) {
                break;
            }
            v = need_v;
            h = need_h;
        }
        (v, h)
    }

    fn max_scroll(&self) -> f32 {
        (self.content_height() - self.content_rect().height()).max(0.0)
    }

    fn has_scrollbar(&self) -> bool {
        self.max_scroll() > 0.5 && self.content_rect().height() > 0.0
    }

    fn max_scroll_x(&self) -> f32 {
        (self.columns_total() - self.content_rect().width()).max(0.0)
    }

    fn has_hscrollbar(&self) -> bool {
        self.max_scroll_x() > 0.5 && self.content_rect().width() > 0.0
    }

    /// Rows and columns area without the scrollbar strips.
    fn content_rect(&self) -> Rect {
        let (v, h) = self.bars_visible();
        let body = self.body_rect();
        let mut rect = body;
        if v {
            rect.right = (rect.right - self.style.inset - self.style.thickness).max(rect.left);
        }
        if h {
            rect.bottom = (rect.bottom - self.style.inset - self.style.thickness).max(rect.top);
        }
        rect
    }

    /// Vertical track, shortened so it does not run under the horizontal bar.
    fn scrollbar_rect(&self) -> Rect {
        let body = self.body_rect();
        let (_, h) = self.bars_visible();
        let bottom = if h { self.content_rect().bottom } else { body.bottom };
        scrollbar::track_rect(
            Rect::new(body.left, body.top, body.right, bottom),
            true,
            &self.style,
        )
    }

    /// Horizontal track, shortened so it does not run under the vertical bar.
    fn hscrollbar_rect(&self) -> Rect {
        let body = self.body_rect();
        let (v, _) = self.bars_visible();
        let right = if v { self.content_rect().right } else { body.right };
        scrollbar::track_rect(
            Rect::new(body.left, body.top, right, body.bottom),
            false,
            &self.style,
        )
    }

    fn thumb_rect(&self) -> Option<Rect> {
        if !self.has_scrollbar() {
            return None;
        }
        scrollbar::thumb_rect(
            self.scrollbar_rect(),
            self.content_height(),
            self.content_rect().height(),
            self.scroll,
            true,
            &self.style,
        )
    }

    fn hthumb_rect(&self) -> Option<Rect> {
        if !self.has_hscrollbar() {
            return None;
        }
        scrollbar::thumb_rect(
            self.hscrollbar_rect(),
            self.columns_total(),
            self.content_rect().width(),
            self.scroll_x,
            false,
            &self.style,
        )
    }

    /// `(fixed width sum, flex column count)` for the current columns.
    fn width_split(&self) -> (f32, usize) {
        let fixed = self
            .columns
            .iter()
            .filter(|column| !column.flex)
            .map(|column| column.width)
            .sum();
        let flex = self.columns.iter().filter(|column| column.flex).count();
        (fixed, flex)
    }

    /// Width a flex column receives; `0` when there is none.
    fn flex_width(&self) -> f32 {
        let (fixed, flex) = self.width_split();
        if flex == 0 {
            return 0.0;
        }
        ((self.content_rect().width() - fixed) / flex as f32).max(0.0)
    }

    fn column_width(&self, index: usize) -> f32 {
        self.columns
            .get(index)
            .map(|column| {
                if column.flex {
                    self.flex_width()
                } else {
                    column.width
                }
            })
            .unwrap_or(0.0)
    }

    /// Whole column extent (flex columns fill the viewport, fixed columns can
    /// overflow it and drive the horizontal bar).
    fn columns_total(&self) -> f32 {
        (0..self.columns.len())
            .map(|index| self.column_width(index))
            .sum()
    }

    /// Left edge and width of one column in client coordinates (including the
    /// horizontal scroll offset). The application can use it to open an editor
    /// over a cell; `None` for an unknown column.
    pub fn column_span(&self, index: usize) -> Option<(f32, f32)> {
        if index >= self.columns.len() {
            return None;
        }
        let mut left = self.content_rect().left - self.scroll_x;
        for position in 0..index {
            left += self.column_width(position);
        }
        Some((left, self.column_width(index)))
    }

    /// Client x of a column's trailing divider.
    fn divider_x(&self, index: usize) -> Option<f32> {
        self.columns.get(index)?;
        Some(self.column_span(index)?.0 + self.column_width(index))
    }

    /// The fixed column whose trailing divider is under `pos`.
    fn resize_target(&self, pos: Point) -> Option<usize> {
        if self.header_height <= 0.0 || !self.header_rect().contains_point(pos) {
            return None;
        }
        let content = self.content_rect();
        if pos.x < content.left || pos.x > content.right {
            return None;
        }
        for index in 0..self.columns.len().saturating_sub(1) {
            if self.columns[index].flex {
                continue;
            }
            let edge = self.divider_x(index)?;
            if edge >= content.left && edge <= content.right && (pos.x - edge).abs() <= RESIZE_GRAB {
                return Some(index);
            }
        }
        None
    }

    fn column_at(&self, x: f32) -> Option<usize> {
        let content = self.content_rect();
        if x < content.left || x > content.right {
            return None;
        }
        let mut left = content.left - self.scroll_x;
        for (index, _) in self.columns.iter().enumerate() {
            let width = self.column_width(index);
            if x >= left && x < left + width {
                return Some(index);
            }
            left += width;
        }
        None
    }

    /// Insertion slot for a header drag: the number of column centres left of
    /// `x` (`0..=columns.len()`).
    fn insertion_slot(&self, x: f32) -> usize {
        let mut left = self.content_rect().left - self.scroll_x;
        let mut slot = 0;
        for index in 0..self.columns.len() {
            let width = self.column_width(index);
            if x >= left + width * 0.5 {
                slot = index + 1;
            } else {
                break;
            }
            left += width;
        }
        slot
    }

    fn header_rect(&self) -> Rect {
        Rect::new(
            self.rect.left,
            self.rect.top,
            self.rect.right,
            (self.rect.top + self.header_height).min(self.rect.bottom),
        )
    }

    fn header_at(&self, pos: Point) -> Option<usize> {
        if self.header_height <= 0.0 || !self.header_rect().contains_point(pos) {
            return None;
        }
        self.column_at(pos.x)
    }

    fn row_at(&self, pos: Point) -> Option<usize> {
        if self.rows.is_empty() {
            return None;
        }
        let body = self.body_rect();
        if !body.contains_point(pos) || !self.content_rect().contains_point(pos) {
            return None;
        }
        let y = pos.y - body.top + self.scroll;
        if y < 0.0 {
            return None;
        }
        let index = (y / self.row_height).floor() as usize;
        (index < self.rows.len()).then_some(index)
    }

    fn visible_range(&self) -> (usize, usize) {
        if self.rows.is_empty() {
            return (0, 0);
        }
        let body = self.body_rect().height();
        if body <= 0.0 {
            return (0, 0);
        }
        let start = (self.scroll / self.row_height).floor().max(0.0) as usize;
        let count = (body / self.row_height).ceil() as usize + 1;
        let end = (start + count).min(self.rows.len());
        (start.min(self.rows.len()), end)
    }

    fn page_rows(&self) -> usize {
        ((self.body_rect().height() / self.row_height).floor() as usize).max(1)
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
        self.scroll_x = self.scroll_x.clamp(0.0, self.max_scroll_x());
    }

    fn clamp_sort(&mut self) {
        if let Some((column, _)) = self.sort
            && column >= self.columns.len()
        {
            self.sort = None;
        }
    }

    fn ensure_selected_visible(&mut self) {
        if !self.rows.is_empty() {
            self.ensure_row_visible(self.selected);
            self.ensure_column_visible(self.selected_col);
        }
    }

    /// Scroll the minimum distance that makes a column (with a little padding
    /// past its divider) fully visible.
    fn ensure_column_visible(&mut self, column: usize) {
        if self.columns.is_empty() {
            return;
        }
        let column = column.min(self.columns.len() - 1);
        let Some((left, width)) = self.column_span(column) else {
            return;
        };
        let content = self.content_rect();
        let left = left - self.cell_padding;
        let right = left + width + self.cell_padding;
        let offset = self.scroll_x;
        if left < content.left {
            self.scroll_x = (offset - (content.left - left)).max(0.0);
        } else if right > content.right {
            self.scroll_x = offset + (right - content.right);
        }
        self.scroll_x = self.scroll_x.clamp(0.0, self.max_scroll_x());
    }

    /// Cell under `pos` (a row and a column), if any.
    fn cell_at(&self, pos: Point) -> Option<(usize, usize)> {
        let row = self.row_at(pos)?;
        let column = self.column_at(pos.x)?;
        Some((row, column))
    }

    /// Published child index of a cell (`row * columns + column`); the a11y
    /// tree keeps cells on this widget-level numbering.
    fn cell_index(&self, row: usize, column: usize) -> usize {
        row * self.columns.len().max(1) + column
    }

    fn draft_locked(&self) -> bool {
        !self.editable || self.columns.is_empty()
    }

    fn cell_editable(&self, row: usize, column: usize) -> bool {
        !self.draft_locked()
            && !self.read_only
            && column < self.columns.len()
            && self
                .rows
                .get(row)
                .is_some_and(|row| !row.disabled)
    }

    /// Start a typed session on a cell. `position` places the caret from a
    /// pointer click; without one the whole value starts selected. The cell is
    /// edited by the embedded painted field: a value containing a newline gets
    /// the multiline model (soft wrap, Up/Down across lines, Enter inserts a
    /// newline, Ctrl+Enter/Tab/blur commits).
    fn begin_draft_at(&mut self, row: usize, column: usize, position: Option<Point>) -> bool {
        if !self.cell_editable(row, column) {
            return false;
        }
        if self
            .draft
            .as_ref()
            .is_some_and(|draft| draft.row == row && draft.column == column)
        {
            return false;
        }
        let text = self.rows[row].cell(column).to_string();
        self.selected = row;
        self.selected_col = column;
        self.ensure_selected_visible();
        let Some(field) = self.cell_rect(row, column) else {
            return false;
        };
        let padding = crate::geometry::Insets::xy(self.cell_padding, 2.0);
        if !self
            .field
            .get_mut()
            .begin(field, &text, position, padding)
        {
            return false;
        }
        self.draft = Some(CellDraft { row, column });
        true
    }

    /// Drop an open session without applying it (rows/columns replaced, a
    /// lock, or an assistive-technology value write).
    fn close_draft(&mut self) {
        if self.draft.take().is_some() {
            self.field.get_mut().finish(false);
        }
    }

    /// Apply the open draft to its row and report it. `None` when no session
    /// is open or the text did not change.
    fn commit_draft(&mut self) -> Option<M> {
        let draft = self.draft.take()?;
        let text = self.field.get_mut().finish(true)?;
        let changed = {
            let row = self.rows.get_mut(draft.row)?;
            if row.cells.len() <= draft.column {
                row.cells.resize(draft.column + 1, String::new());
            }
            if row.cells[draft.column] == text {
                false
            } else {
                row.cells[draft.column] = text.clone();
                true
            }
        };
        if !changed {
            return None;
        }
        let CellDraft { row, column } = draft;
        self.on_cell_edit
            .as_ref()
            .map(|handler| handler(row, column, text))
    }

    /// Commit (`true`) or discard the open draft.
    fn finish_draft(&mut self, commit: bool) -> Option<M> {
        if !commit {
            self.close_draft();
            return None;
        }
        self.commit_draft()
    }

    /// Edit input while a draft is open. The field owns every key the
    /// container forwards (Tab/Escape/Enter and Ctrl+Enter are handled there).
    fn key_in_draft(&mut self, key: crate::event::KeyEvent) {
        self.field.get_mut().key(key);
    }

    /// Paint the open draft: the embedded field is re-anchored to its cell
    /// first (a resize or a programmatic scroll may have moved it) and then
    /// paints itself — frame, selection, inline IME, caret and any multiline
    /// scrolling, exactly like a standalone painted field.
    fn paint_draft(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState, draft: &CellDraft) {
        let Some(rect) = self.cell_rect(draft.row, draft.column) else {
            return;
        };
        let mut field = self.field.borrow_mut();
        if field.rect() != rect {
            field.set_rect(rect);
        }
        field.paint(ctx, state);
    }

    fn select(&mut self, index: usize) -> Option<M> {
        if self.rows.is_empty() {
            self.selected = 0;
            return None;
        }
        self.selected = index.min(self.rows.len() - 1);
        self.ensure_selected_visible();
        self.on_select
            .as_ref()
            .map(|handler| handler(self.selected))
    }

    fn activate(&self) -> Option<M> {
        if self.rows.is_empty() || self.rows[self.selected].disabled {
            return None;
        }
        self.on_activate
            .as_ref()
            .map(|handler| handler(self.selected.min(self.rows.len() - 1)))
    }

    fn selected_row_rect(&self) -> Option<Rect> {
        if self.rows.is_empty() {
            return None;
        }
        let body = self.body_rect();
        let index = self.selected.min(self.rows.len() - 1);
        let top = body.top + index as f32 * self.row_height - self.scroll;
        let row = Rect::new(
            self.content_rect().left,
            top,
            self.content_rect().right,
            top + self.row_height,
        );
        row.intersect(body)
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
            self.content_rect().height(),
            true,
        );
        self.clamp_scroll();
    }

    fn drag_hthumb(&mut self, pos: Point, grab: f32) {
        let Some(thumb) = self.hthumb_rect() else {
            return;
        };
        self.scroll_x = scrollbar::drag_offset(
            pos,
            self.hscrollbar_rect(),
            thumb,
            grab,
            self.columns_total(),
            self.content_rect().width(),
            false,
        );
        self.clamp_scroll();
    }

    /// Union of the bar strips whose hover state just changed (a narrow
    /// repaint that never touches the rows).
    fn bar_hint(&self, vertical: bool, horizontal: bool) -> Option<Rect> {
        let mut hint: Option<Rect> = None;
        if vertical {
            hint = Some(self.scrollbar_rect());
        }
        if horizontal {
            let rect = self.hscrollbar_rect();
            hint = Some(hint.map_or(rect, |hint| hint.union(rect)));
        }
        hint
    }
}

impl<M: 'static> Widget<M> for Table<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        TABLE_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("row-height", SkinValue::Number(height)) => *height >= 16.0,
            ("header-height", SkinValue::Number(height))
            | ("cell-padding", SkinValue::Number(height)) => *height >= 0.0,
            ("striped" | "grid-lines" | "editable", SkinValue::Bool(_)) => true,
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
            ("cell-padding", SkinValue::Number(padding)) => {
                self.set_cell_padding(*padding as f32);
            }
            ("striped", SkinValue::Bool(striped)) => {
                self.set_striped(*striped);
            }
            ("grid-lines", SkinValue::Bool(grid)) => {
                self.set_grid_lines(*grid);
            }
            ("editable", SkinValue::Bool(editable)) => {
                self.set_editable(*editable);
            }
            ("empty-text", SkinValue::Text(text)) => {
                self.set_empty_text(*text);
            }
            _ => {}
        }
    }

    fn skin_children(&self) -> &'static [&'static str] {
        TABLE_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(|child| match child.name.as_str() {
            "column" => TableColumn::valid_skin_column(child),
            "row" => TableRow::valid_skin_row(child),
            _ => false,
        })
    }

    /// The child list is the complete table data: `<column>` elements replace
    /// the column set and `<row>` elements replace the rows, in source order.
    /// A document that changes one of them declares both.
    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        let columns = children
            .iter()
            .filter(|child| child.name == "column")
            .filter_map(TableColumn::from_skin)
            .collect();
        let rows = children
            .iter()
            .filter(|child| child.name == "row")
            .filter_map(TableRow::from_skin)
            .collect();
        self.set_columns(columns);
        self.set_rows(rows);
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
        self.selected = index;
        self.selected_col = self
            .selected_col
            .min(self.columns.len().saturating_sub(1));
        self.ensure_selected_visible();
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    /// Only a real hover change repaints (a different row or header cell, the
    /// bar tint, or a thumb drag, which moves the rows).
    fn repaint_on_move_at(&self, pos: Point) -> bool {
        if self.draft.is_some() {
            return true;
        }
        if self.bar.active
            || self.hbar.active
            || self.resizing.is_some()
            || self.reordering.is_some()
        {
            return true;
        }
        if self.has_scrollbar()
            && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some()
        {
            return !self.bar.hovered;
        }
        if self.has_hscrollbar()
            && scrollbar::hit(self.hscrollbar_rect(), self.hthumb_rect(), pos).is_some()
        {
            return !self.hbar.hovered;
        }
        let header = self.header_at(pos);
        let row = if header.is_some() {
            None
        } else {
            self.row_at(pos)
        };
        self.bar.hovered || self.hbar.hovered || header != self.hover_header || row != self.hover_row
    }

    /// Bar-hover/press chrome changes repaint the scrollbar strip only.
    fn take_repaint_hint(&mut self) -> Option<Rect> {
        self.repaint_hint.take()
    }

    /// The table scrolls its rows with the wheel (D65).
    fn scrolls_with_wheel(&self) -> bool {
        true
    }

    /// Enter enters row navigation; arrows/PageUp/PageDown/Home/End move the
    /// row cursor, Enter again activates it, Esc/Backspace leave.
    fn adjustable(&self) -> bool {
        true
    }

    /// A header divider shows the horizontal resize cursor; a scrollbar track
    /// shows the arrow (dragging a thumb is not a click).
    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        if self.resizing.is_some() || self.resize_target(pos).is_some() {
            return Some(Cursor::SizeWE);
        }
        if (self.has_scrollbar() && self.scrollbar_rect().contains_point(pos))
            || (self.has_hscrollbar() && self.hscrollbar_rect().contains_point(pos))
        {
            return Some(Cursor::Arrow);
        }
        // An editable cell reads as a text field even when the session is
        // closed.
        if let Some((row, column)) = self.cell_at(pos)
            && self.cell_editable(row, column)
        {
            return Some(Cursor::IBeam);
        }
        None
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let (start, end) = self.visible_range();
        let body = self.body_rect();
        let content = self.content_rect();
        let columns = self.columns.len();
        let mut children = Vec::new();
        if columns > 0 {
            // Column headers come first (HeaderItem role, not selectable). They
            // use an out-of-range child index so they never alias a cell.
            if self.header_height > 0.0 {
                let header = self.header_rect();
                for (column, model) in self.columns.iter().enumerate() {
                    let Some((left, width)) = self.column_span(column) else {
                        continue;
                    };
                    let rect = Rect::new(left, header.top, left + width, header.bottom)
                        .intersect(content)
                        .unwrap_or(Rect::new(left, header.top, left + width, header.top));
                    children.push(
                        A11yChild::new(
                            HEADER_INDEX_BASE + column,
                            model.title().to_string(),
                            rect,
                        )
                        .role(A11yRole::ColumnHeader),
                    );
                }
            }
            for index in start..end {
                let row = &self.rows[index];
                let top = body.top + index as f32 * self.row_height - self.scroll;
                let row_rect = Rect::new(content.left, top, content.right, top + self.row_height);
                let offscreen = !row_rect.intersects(body);
                let row_rect = row_rect.intersect(body).unwrap_or(row_rect);
                for column in 0..columns {
                    let Some((left, width)) = self.column_span(column) else {
                        continue;
                    };
                    let rect = Rect::new(left, row_rect.top, left + width, row_rect.bottom)
                        .intersect(content)
                        .unwrap_or(Rect::new(left, top, left + width, top + self.row_height));
                    let selected = index == self.selected
                        && column == self.selected_col.min(columns - 1);
                    // The editing cell publishes the session's text, so a
                    // screen reader follows what is being typed instead of the
                    // last committed value.
                    let editing = self
                        .draft
                        .as_ref()
                        .is_some_and(|draft| draft.row == index && draft.column == column);
                    let text = if editing {
                        self.field.borrow().text().to_string()
                    } else {
                        row.cell(column).to_string()
                    };
                    let editable = self.cell_editable(index, column);
                    children.push(
                        A11yChild::new(self.cell_index(index, column), text.clone(), rect)
                            .selected(selected)
                            .enabled(!row.disabled)
                            .offscreen(offscreen)
                            .grid_cell(index, column)
                            .value(text)
                            .editable(editable),
                    );
                }
            }
        }
        Some(
            A11yInfo::new(A11yRole::Grid, self.id.0)
                .children(children)
                .grid(self.rows.len(), columns)
                .scroll(
                    A11yScroll::new()
                        .horizontal(
                            self.scroll_x,
                            self.content_rect().width(),
                            self.columns_total(),
                        )
                        .vertical(
                            self.scroll,
                            self.content_rect().height(),
                            self.content_height(),
                        ),
                ),
        )
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        if index >= HEADER_INDEX_BASE {
            return None;
        }
        let columns = self.columns.len().max(1);
        let row = index / columns;
        let column = index % columns;
        if row >= self.rows.len() || column >= self.columns.len() {
            return None;
        }
        self.selected = row;
        self.selected_col = column;
        self.ensure_selected_visible();
        None
    }

    /// A UIA `SetValue` on a cell: apply the text like a committed draft and
    /// report it through `on_cell_edit`. Locked, disabled and header children
    /// are ignored.
    fn a11y_set_value_at(&mut self, child: Option<usize>, text: &str) -> Option<M> {
        let index = child?;
        if index >= HEADER_INDEX_BASE {
            return None;
        }
        let columns = self.columns.len();
        if columns == 0 {
            return None;
        }
        let row = index / columns;
        let column = index % columns;
        if self
            .draft
            .as_ref()
            .is_some_and(|draft| (draft.row, draft.column) == (row, column))
        {
            self.close_draft();
        }
        if !self.cell_editable(row, column) || !self.set_cell(row, column, text) {
            return None;
        }
        self.selected = row;
        self.selected_col = column;
        self.ensure_selected_visible();
        let text = text.to_string();
        self.on_cell_edit
            .as_ref()
            .map(|handler| handler(row, column, text))
    }

    fn a11y_scroll(&mut self, horizontal: Option<f32>, vertical: Option<f32>) -> bool {
        let before = (self.scroll, self.scroll_x);
        if let Some(target) = horizontal {
            self.scroll_x = target.clamp(0.0, self.max_scroll_x());
        }
        if let Some(target) = vertical {
            self.scroll = target.clamp(0.0, self.max_scroll());
        }
        (self.scroll - before.0).abs() > 0.01 || (self.scroll_x - before.1).abs() > 0.01
    }

    fn tooltip_rect(&self) -> Rect {
        let (row, column) = self.selected_cell();
        self.cell_rect(row, column)
            .or_else(|| self.selected_row_rect())
            .unwrap_or(self.rect)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        let (row, column) = self.cell_at(pos)?;
        Some(self.cell_index(row, column))
    }

    fn focus_item(&self) -> Option<usize> {
        (!self.rows.is_empty() && !self.columns.is_empty())
            .then(|| self.cell_index(self.selected, self.selected_col))
    }

    fn focus_ring_rect(&self) -> Rect {
        self.rect().inset(1.0, 1.0)
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        if read_only {
            self.close_draft();
        }
    }

    // --- framework-painted cell editor -------------------------------------

    fn inline_edit(&self) -> bool {
        self.draft.is_some()
    }

    fn inline_edit_at(&self, pos: Point) -> bool {
        self.draft
            .as_ref()
            .is_some_and(|draft| self.cell_at(pos) == Some((draft.row, draft.column)))
            && self.editable
    }

    fn editing(&self) -> bool {
        self.draft.is_some()
    }

    fn multiline_edit(&self) -> bool {
        self.draft.is_some() && self.field.borrow().multiline()
    }

    fn begin_inline_edit(&mut self, position: Option<Point>) -> bool {
        match position {
            Some(pos) => {
                let Some((row, column)) = self.cell_at(pos) else {
                    return false;
                };
                // A click inside the open draft only moves the caret (and can
                // start a selection drag); it never resets the typed text.
                if let Some(draft) = self.draft.as_ref() {
                    if draft.row == row && draft.column == column {
                        return self.field.get_mut().place_at(pos);
                    }
                    return false;
                }
                self.begin_draft_at(row, column, Some(pos))
            }
            None => {
                let (row, column) = self.selected_cell();
                self.begin_draft_at(row, column, None)
            }
        }
    }

    fn finish_edit(&mut self, commit: bool) -> Option<M> {
        self.finish_draft(commit)
    }

    fn insert_text(&mut self, text: &str) -> bool {
        if self.draft.is_none() {
            return false;
        }
        self.field.get_mut().insert_text(text)
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

    /// The standard edit menu while a cell draft is open (the shared painted
    /// field supplies it: undo/redo, cut/copy/paste, delete, select all and
    /// the Unicode-control submenu).
    fn edit_menu(&self) -> Option<Menu> {
        self.field.borrow().edit_menu()
    }

    fn edit_command(&mut self, command: u32) -> bool {
        self.field.get_mut().edit_command(command)
    }

    fn take_clipboard(&mut self) -> Option<String> {
        self.field.get_mut().take_clipboard()
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let pal = ctx.palette;
        let header = self.header_rect();
        let body = self.body_rect();
        let content = self.content_rect();

        // Header band: panel fill, titles, sort arrow, bottom rule.
        if self.header_height > 0.0 && !header.is_empty() {
            let band = Rect::new(content.left, header.top, content.right, header.bottom);
            ctx.canvas.push_clip(band);
            ctx.canvas.fill_rect(band, pal.panel);
            let mut left = content.left - self.scroll_x;
            for (index, column) in self.columns.iter().enumerate() {
                let width = self.column_width(index);
                let cell = Rect::new(
                    left + self.cell_padding,
                    header.top,
                    (left + width - self.cell_padding - SORT_ARROW_ROOM)
                        .max(left + self.cell_padding),
                    header.bottom,
                );
                if state.hovered && self.hover_header == Some(index) {
                    ctx.canvas.fill_rect(
                        Rect::new(left, header.top, left + width, header.bottom),
                        pal.nav_hover,
                    );
                }
                ctx.text_overflow(
                    cell,
                    &column.title,
                    TextStyleRole::Small,
                    column.align,
                    pal.muted,
                    TextOverflow::Ellipsis,
                );
                if let Some((sort_column, ascending)) = self.sort
                    && sort_column == index
                {
                    let arrow = Rect::new(
                        left + width - self.cell_padding - 10.0,
                        header.center().y - 3.0,
                        left + width - self.cell_padding - 2.0,
                        header.center().y + 3.0,
                    );
                    paint_sort_arrow(&mut *ctx.canvas, arrow, ascending, pal.accent);
                }
                if self.grid_lines && index + 1 < self.columns.len() {
                    ctx.canvas.fill_rect(
                        Rect::new(left + width, header.top, left + width + 1.0, header.bottom),
                        pal.divider,
                    );
                }
                left += width;
            }
            ctx.canvas.pop_clip();
            if self.grid_lines {
                ctx.canvas.fill_rect(
                    Rect::new(
                        content.left,
                        header.bottom - 1.0,
                        content.right,
                        header.bottom,
                    ),
                    pal.divider,
                );
            }
        }

        ctx.canvas.push_clip(body);
        if self.rows.is_empty() {
            if !self.empty_text.is_empty() {
                ctx.text(
                    Rect::new(content.left, content.top, content.right, content.bottom),
                    &self.empty_text,
                    TextStyleRole::Small,
                    TextAlign::Center,
                    pal.muted,
                );
            }
        } else {
            let (start, end) = self.visible_range();
            for index in start..end {
                let row = &self.rows[index];
                let top = body.top + index as f32 * self.row_height - self.scroll;
                let row_rect = Rect::new(content.left, top, content.right, top + self.row_height);
                if !row_rect.intersects(body) {
                    continue;
                }
                if self.striped && index % 2 == 1 {
                    ctx.canvas.fill_rect(row_rect, pal.panel);
                }
                if state.pressed && self.press_row == Some(index) {
                    ctx.canvas.fill_rect(row_rect, pal.row_pressed);
                } else if (state.hovered && self.hover_row == Some(index))
                    || (state.focused && index == self.selected)
                {
                    ctx.canvas.fill_rect(row_rect, pal.row_hover);
                }
                let color = if row.disabled {
                    pal.text_disabled
                } else {
                    pal.text
                };
                let mut left = content.left - self.scroll_x;
                for (column, _) in self.columns.iter().enumerate() {
                    let width = self.column_width(column);
                    let align = self
                        .columns
                        .get(column)
                        .map(|column| column.align)
                        .unwrap_or(TextAlign::Left);
                    let cell = Rect::new(
                        left + self.cell_padding,
                        top,
                        (left + width - self.cell_padding).max(left + self.cell_padding),
                        top + self.row_height,
                    );
                    if let Some(draft) = self
                        .draft
                        .as_ref()
                        .filter(|draft| draft.row == index && draft.column == column)
                    {
                        self.paint_draft(ctx, state, draft);
                    } else {
                        ctx.text_overflow(
                            cell,
                            row.cell(column),
                            TextStyleRole::Body,
                            align,
                            color,
                            TextOverflow::Ellipsis,
                        );
                    }
                    if self.grid_lines && column + 1 < self.columns.len() {
                        ctx.canvas.fill_rect(
                            Rect::new(left + width, top, left + width + 1.0, top + self.row_height),
                            pal.divider,
                        );
                    }
                    left += width;
                }
                if self.grid_lines {
                    ctx.canvas.fill_rect(
                        Rect::new(
                            content.left,
                            row_rect.bottom - 1.0,
                            content.right,
                            row_rect.bottom,
                        ),
                        pal.divider,
                    );
                }
                // The active cell gets a hairline accent frame while the table
                // has focus, so keyboard cell navigation is visible. It paints
                // after the row separator, or the divider would cover its
                // bottom edge.
                if state.focused
                    && index == self.selected
                    && self.draft.is_none()
                    && !self.columns.is_empty()
                {
                    let column = self.selected_col.min(self.columns.len() - 1);
                    if let Some((left, width)) = self.column_span(column) {
                        let cursor = Rect::new(
                            left,
                            top,
                            left + width,
                            (top + self.row_height).min(content.bottom),
                        );
                        paint_border(ctx, cursor, pal.accent);
                    }
                }
            }
        }
        ctx.canvas.pop_clip();

        // Column drag: an accent line marks the insertion slot across the
        // header and body, so the drop position is visible while moving.
        if let Some(drag) = self.reordering {
            let mut x = content.left - self.scroll_x;
            for index in 0..drag.insert.min(self.columns.len()) {
                x += self.column_width(index);
            }
            let x = x.clamp(content.left, (content.right - 2.0).max(content.left));
            ctx.canvas.fill_rect(
                Rect::new(x, header.top, x + 2.0, content.bottom),
                pal.accent,
            );
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
        if self.has_hscrollbar() {
            scrollbar::paint(
                ctx,
                self.hscrollbar_rect(),
                self.hthumb_rect(),
                &self.hbar,
                &self.style,
            );
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        match input {
            WidgetInput::HoverAt(pos) => {
                let over_v = self.has_scrollbar()
                    && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some();
                let over_h = self.has_hscrollbar()
                    && scrollbar::hit(self.hscrollbar_rect(), self.hthumb_rect(), pos)
                        .is_some();
                let bar_before = (self.bar.hovered, self.hbar.hovered);
                let rows_before = (self.hover_row, self.hover_header);
                self.bar.hovered = over_v;
                self.hbar.hovered = over_h;
                if over_v || over_h {
                    self.hover_row = None;
                    self.hover_header = None;
                } else {
                    self.hover_header = self.header_at(pos);
                    self.hover_row = if self.hover_header.is_some() {
                        None
                    } else {
                        self.row_at(pos)
                    };
                }
                if (self.hover_row, self.hover_header) == rows_before
                    && (self.bar.hovered, self.hbar.hovered) != bar_before
                {
                    self.repaint_hint = self.bar_hint(
                        self.bar.hovered != bar_before.0,
                        self.hbar.hovered != bar_before.1,
                    );
                }
                None
            }
            WidgetInput::HoverLeave => {
                let bar_before = (self.bar.hovered, self.hbar.hovered);
                let rows_before = (self.hover_row, self.hover_header);
                self.hover_row = None;
                self.hover_header = None;
                if !self.bar.active {
                    self.bar.hovered = false;
                }
                if !self.hbar.active {
                    self.hbar.hovered = false;
                }
                if (self.hover_row, self.hover_header) == rows_before
                    && (self.bar.hovered, self.hbar.hovered) != bar_before
                {
                    self.repaint_hint = self.bar_hint(
                        self.bar.hovered != bar_before.0,
                        self.hbar.hovered != bar_before.1,
                    );
                }
                None
            }
            WidgetInput::PressAt(pos) => {
                // A press inside the open draft keeps the caret drag going;
                // the container places the caret through `begin_inline_edit`.
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| self.cell_at(pos) == Some((draft.row, draft.column)))
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
                                    self.content_rect().height(),
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
                if self.has_hscrollbar() {
                    let track = self.hscrollbar_rect();
                    let thumb = self.hthumb_rect();
                    match scrollbar::hit(track, thumb, pos) {
                        Some(BarHit::Thumb) => {
                            if let Some(thumb) = thumb {
                                self.hbar.active = true;
                                self.hbar.hovered = true;
                                self.hbar.grab = pos.x - thumb.left;
                                self.repaint_hint = Some(self.hscrollbar_rect());
                                return None;
                            }
                        }
                        Some(BarHit::Track) => {
                            if let Some(thumb) = thumb {
                                self.scroll_x = scrollbar::track_click_offset(
                                    pos,
                                    track,
                                    thumb,
                                    self.columns_total(),
                                    self.content_rect().width(),
                                    self.scroll_x,
                                    false,
                                    &self.style,
                                );
                                self.clamp_scroll();
                                self.hbar.active = true;
                                self.hbar.hovered = true;
                                self.hbar.grab = thumb.width() * 0.5;
                                return None;
                            }
                        }
                        None => {}
                    }
                }
                // A press on a header divider starts a width drag instead of a
                // header click.
                if let Some(column) = self.resize_target(pos) {
                    self.resizing = Some(ColumnResize {
                        column,
                        start_x: pos.x,
                        start_width: self.column_width(column),
                    });
                    self.press_header = None;
                    self.press_row = None;
                    return None;
                }
                self.press_header = self.header_at(pos);
                self.press_x = pos.x;
                self.press_row = if self.press_header.is_some() {
                    None
                } else {
                    self.row_at(pos)
                };
                // A press on a cell moves the cell cursor there too, so the
                // accent frame (and `selected_cell`) follows a pointer click
                // into another column, not just a keyboard move.
                if self.press_row.is_some() {
                    if let Some(column) = self.column_at(pos.x) {
                        self.selected_col = column.min(self.columns.len().saturating_sub(1));
                    }
                }
                None
            }
            WidgetInput::DragTo(pos) => {
                if self.bar.active {
                    self.drag_thumb(pos, self.bar.grab);
                } else if self.hbar.active {
                    self.drag_hthumb(pos, self.hbar.grab);
                }
                if let Some(resize) = self.resizing {
                    let width = (resize.start_width + pos.x - resize.start_x)
                        .max(MIN_COLUMN_WIDTH);
                    if let Some(column) = self.columns.get_mut(resize.column) {
                        column.width = width;
                    }
                    self.clamp_scroll();
                } else if let Some(from) = self.press_header {
                    // A header drag past the threshold becomes a column move;
                    // below it the release stays a header click.
                    if self.reordering.is_none()
                        && (pos.x - self.press_x).abs() >= REORDER_DRAG
                    {
                        self.reordering = Some(ColumnDrag {
                            from,
                            insert: from,
                        });
                    }
                    if let Some(mut drag) = self.reordering {
                        drag.insert = self.insertion_slot(pos.x);
                        self.reordering = Some(drag);
                    }
                } else if let Some(draft) = self.draft.as_ref()
                    && self.cell_at(pos) == Some((draft.row, draft.column))
                {
                    // A drag across the open cell extends its selection.
                    self.field.get_mut().input(WidgetInput::DragTo(pos));
                }
                None
            }
            WidgetInput::ReleaseAt(pos) => {
                // A release on another cell commits the draft first; the
                // clicked cell then moves the cursor as usual.
                if let Some(draft) = self.draft.as_ref() {
                    let same = self.cell_at(pos) == Some((draft.row, draft.column));
                    if same {
                        self.field.get_mut().input(WidgetInput::ReleaseAt(pos));
                        self.press_row = None;
                        return None;
                    }
                    {
                        let message = self.commit_draft();
                        let moved = if let Some((row, column)) = self.cell_at(pos) {
                            self.selected = row;
                            self.selected_col = column;
                            self.ensure_selected_visible();
                            true
                        } else {
                            false
                        };
                        self.press_row = None;
                        if let Some(message) = message {
                            return Some(message);
                        }
                        return moved
                            .then(|| self.on_select.as_ref().map(|handler| handler(self.selected)))
                            .flatten();
                    }
                }
                if self.bar.active {
                    self.bar.active = false;
                    self.repaint_hint = Some(self.scrollbar_rect());
                    return None;
                }
                if self.hbar.active {
                    self.hbar.active = false;
                    self.repaint_hint = Some(self.hscrollbar_rect());
                    return None;
                }
                if let Some(resize) = self.resizing.take() {
                    self.clamp_scroll();
                    let width = self.column_width(resize.column);
                    return self
                        .on_column_resize
                        .as_ref()
                        .map(|handler| handler(resize.column, width));
                }
                if let Some(drag) = self.reordering.take() {
                    self.press_header = None;
                    let to = if drag.insert > drag.from {
                        drag.insert - 1
                    } else {
                        drag.insert
                    };
                    if to == drag.from || !self.move_column(drag.from, to) {
                        return None;
                    }
                    return self
                        .on_column_reorder
                        .as_ref()
                        .map(|handler| handler(drag.from, to));
                }
                if let Some(pressed) = self.press_header.take() {
                    if self.header_at(pos) == Some(pressed) {
                        return self.on_header.as_ref().map(|handler| handler(pressed));
                    }
                    return None;
                }
                let pressed = self.press_row.take();
                let hit = self.row_at(pos);
                if let (Some(pressed), Some(hit)) = (pressed, hit)
                    && pressed == hit
                    && !self.rows.get(hit).map(|row| row.disabled).unwrap_or(true)
                {
                    return self.select(hit);
                }
                None
            }
            WidgetInput::ClickAt(pos) => {
                // Selection already ran on the release; a bar click changes
                // nothing and must not repaint the rows underneath it.
                if self.has_scrollbar()
                    && scrollbar::hit(self.scrollbar_rect(), self.thumb_rect(), pos).is_some()
                {
                    self.repaint_hint = Some(self.scrollbar_rect());
                }
                if self.has_hscrollbar()
                    && scrollbar::hit(self.hscrollbar_rect(), self.hthumb_rect(), pos).is_some()
                {
                    self.repaint_hint = Some(self.hscrollbar_rect());
                }
                None
            }
            WidgetInput::DoubleClickAt(pos) => {
                let Some((row, column)) = self.cell_at(pos) else {
                    return None;
                };
                if self.rows.get(row).map(|row| row.disabled).unwrap_or(true) {
                    return None;
                }
                // A double click inside the open cell selects a word, like a
                // painted field does.
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| (draft.row, draft.column) == (row, column))
                {
                    self.field.get_mut().input(WidgetInput::DoubleClickAt(pos));
                    return None;
                }
                self.selected = row;
                self.selected_col = column;
                self.ensure_selected_visible();
                if self.cell_editable(row, column) {
                    self.begin_draft_at(row, column, Some(pos));
                    return None;
                }
                self.on_activate.as_ref().map(|handler| handler(row))
            }
            WidgetInput::Wheel(delta) => {
                // While editing, the wheel belongs to the field (a multiline
                // value scrolls inside its cell); moving the rows under the
                // open field would detach it.
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
                use crate::event::KeyCode;
                if self.draft.is_some() {
                    self.key_in_draft(key);
                    return None;
                }
                let page = self.page_rows();
                match key.code {
                    KeyCode::Up => {
                        self.selected = self.selected.saturating_sub(1);
                        self.ensure_selected_visible();
                    }
                    KeyCode::Down => {
                        if self.selected + 1 < self.rows.len() {
                            self.selected += 1;
                        }
                        self.ensure_selected_visible();
                    }
                    KeyCode::Left if !self.columns.is_empty() => {
                        self.selected_col = self.selected_col.saturating_sub(1);
                        self.ensure_selected_visible();
                    }
                    KeyCode::Right if !self.columns.is_empty() => {
                        self.selected_col =
                            (self.selected_col + 1).min(self.columns.len() - 1);
                        self.ensure_selected_visible();
                    }
                    KeyCode::PageUp => {
                        self.selected = self.selected.saturating_sub(page);
                        self.ensure_selected_visible();
                    }
                    KeyCode::PageDown => {
                        self.selected =
                            (self.selected + page).min(self.rows.len().saturating_sub(1));
                        self.ensure_selected_visible();
                    }
                    KeyCode::Home => {
                        self.selected = 0;
                        self.ensure_selected_visible();
                    }
                    KeyCode::End => {
                        self.selected = self.rows.len().saturating_sub(1);
                        self.ensure_selected_visible();
                    }
                    // Enter edits the cursor cell when the table is editable
                    // (F2 is the platform's Edit chord); otherwise it
                    // activates the row.
                    KeyCode::Enter | KeyCode::Function(2) => {
                        let (row, column) = self.selected_cell();
                        if self.cell_editable(row, column) {
                            self.begin_draft_at(row, column, None);
                            return None;
                        }
                        if key.code == KeyCode::Enter {
                            return self.activate();
                        }
                    }
                    _ => {}
                }
                None
            }
            WidgetInput::Activate => {
                let (row, column) = self.selected_cell();
                if self.cell_editable(row, column) {
                    self.begin_draft_at(row, column, None);
                    return None;
                }
                self.activate()
            }
            _ => None,
        }
    }
}

/// Index `index` after the element at `from` moved to `to`.
fn remap_index(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < index && index <= to {
        index - 1
    } else if to <= index && index < from {
        index + 1
    } else {
        index
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

/// Small solid triangle: an up arrow widens downward, a down arrow narrows.
fn paint_sort_arrow(
    canvas: &mut dyn crate::backend::Canvas,
    rect: Rect,
    ascending: bool,
    color: crate::Color,
) {
    if rect.is_empty() {
        return;
    }
    let rows = rect.height().round().max(1.0) as i32;
    let center = rect.center().x;
    let full = rect.width();
    for row in 0..rows {
        let fraction = (row as f32 + 1.0) / rows as f32;
        let width = if ascending {
            full * fraction
        } else {
            full * (1.0 - fraction) + full / rows as f32
        };
        let y = rect.top + row as f32;
        canvas.fill_rect(
            Rect::new(center - width / 2.0, y, center + width / 2.0, y + 1.0),
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::KeyCode;

    fn table(rows: usize) -> Table<usize> {
        let mut table: Table<usize> = Table::new("test")
            .row_height(30.0)
            .columns(vec![
                TableColumn::new("序号", 60.0),
                TableColumn::flex("名称"),
                TableColumn::new("时长", 80.0).align(TextAlign::Right),
            ])
            .on_select(|index| index)
            .on_activate(|index| 1000 + index)
            .on_header_click(|column| 100 + column);
        table.set_rect(Rect::from_xywh(0.0, 0.0, 400.0, 200.0));
        table.set_rows(
            (0..rows)
                .map(|index| TableRow::new(["数字", &format!("行 {index}"), "3:45"]))
                .collect(),
        );
        table
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

    /// A canvas that records every fill, so a test can check the paint order
    /// (the row divider must not cover the cell cursor's bottom edge).
    #[derive(Default)]
    struct FillRecorder {
        fills: Vec<(Rect, crate::Color)>,
    }

    impl crate::backend::Canvas for FillRecorder {
        fn scale(&self) -> f32 {
            1.0
        }
        fn dpi(&self) -> f32 {
            96.0
        }
        fn fill_rect(&mut self, rect: Rect, color: crate::Color) {
            self.fills.push((rect, color));
        }
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

    fn paint_focused(table: &Table<usize>) -> (Vec<(Rect, crate::Color)>, crate::Palette) {
        let painter = crate::painter::Painter::new();
        let palette = *painter.palette();
        let images = crate::image::ImageStore::new();
        let mut canvas = FillRecorder::default();
        let mut paint = WidgetPaint {
            painter: &painter,
            canvas: &mut canvas,
            palette,
            font: None,
            images: &images,
            viewport: Rect::from_xywh(0.0, 0.0, 400.0, 200.0),
            dirty: &[],
        };
        let state = WidgetState {
            hovered: false,
            pressed: false,
            focused: true,
            adjusting: false,
            enabled: true,
            focus_visible: true,
            editing: false,
            caret_on: true,
        };
        table.paint(&mut paint, &state);
        (canvas.fills, palette)
    }

    #[test]
    fn the_focused_cell_cursor_paints_after_the_row_divider() {
        let mut table = table(3);
        table.select_cell(1, 1);
        let cell = table.cell_rect(1, 1).expect("cell");
        let (fills, palette) = paint_focused(&table);
        // The last fill across the cursor's bottom edge is its accent border,
        // not the row divider painted afterwards (which used to cover it).
        let bottom = fills.iter().rev().find(|(rect, _)| {
            (rect.top - (cell.bottom - 1.0)).abs() < 0.01
                && (rect.bottom - cell.bottom).abs() < 0.01
                && rect.left <= cell.left + 0.5
                && rect.right >= cell.left + 0.5
        });
        let (rect, color) = bottom.expect("a fill on the cursor bottom edge");
        assert_eq!(*color, palette.accent);
        assert!((rect.right - cell.right).abs() < 0.01, "the accent edge spans the cell");
    }

    #[test]
    fn fixed_columns_keep_their_width_and_the_flex_column_fills_the_rest() {
        let table = table(3);
        let body = table.content_rect();
        assert_eq!(table.column_width(0), 60.0);
        assert_eq!(table.column_width(2), 80.0);
        assert!((table.column_width(1) - (body.width() - 140.0)).abs() < 0.01);
        let (left, width) = table.column_span(1).unwrap();
        assert!((left - (body.left + 60.0)).abs() < 0.01);
        assert!((width - table.column_width(1)).abs() < 0.01);
        assert_eq!(table.column_at(body.left + 10.0), Some(0));
        assert_eq!(table.column_at(body.left + 70.0), Some(1));
        assert_eq!(table.column_at(body.right + 10.0), None);
    }

    #[test]
    fn visible_range_covers_the_body_plus_a_look_ahead_row() {
        let table = table(1000);
        let (start, end) = table.visible_range();
        assert_eq!(start, 0);
        // 200 - 30 header = 170 body; 170 / 30 is 5.67 -> 6 rows + look-ahead.
        assert_eq!(end, 7);
    }

    #[test]
    fn wheel_scrolls_and_clamps_and_the_header_does_not_scroll() {
        let mut table = table(1000);
        table.input(WidgetInput::Wheel(-3.0));
        assert!(table.scroll_offset() > 0.0);
        table.input(WidgetInput::Wheel(-100_000.0));
        assert!((table.scroll_offset() - table.max_scroll()).abs() < 0.01);
        // The header stays a header: no row starts inside it.
        assert_eq!(table.row_at(Point::new(20.0, 10.0)), None);
        assert_eq!(table.header_at(Point::new(20.0, 10.0)), Some(0));
    }

    #[test]
    fn header_clicks_report_the_column_and_never_select_a_row() {
        let mut table = table(20);
        let header = table.header_rect();
        table.input(WidgetInput::PressAt(Point::new(
            header.center().x,
            header.center().y - 4.0,
        )));
        let column = table.column_at(header.center().x).unwrap();
        assert_eq!(
            table.input(WidgetInput::ReleaseAt(Point::new(
                header.center().x,
                header.center().y - 4.0
            ))),
            Some(100 + column)
        );
        assert_eq!(
            table.selected_row(),
            0,
            "a header click does not move the row cursor"
        );
        // Releasing over another column is not a header click.
        table.input(WidgetInput::PressAt(Point::new(10.0, 10.0)));
        assert_eq!(
            table.input(WidgetInput::ReleaseAt(Point::new(390.0, 10.0))),
            None
        );
    }

    #[test]
    fn clicking_a_row_selects_it_and_enter_or_double_click_activates() {
        let mut table = table(20);
        let body = table.body_rect();
        let y = body.top + table.row_height * 2.0 + 4.0;
        table.input(WidgetInput::PressAt(Point::new(40.0, y)));
        assert_eq!(
            table.input(WidgetInput::ReleaseAt(Point::new(40.0, y))),
            Some(2)
        );
        assert_eq!(table.selected_row(), 2);
        assert_eq!(table.input(WidgetInput::Activate), Some(1002));
        assert_eq!(
            table.input(key(KeyCode::Enter)),
            Some(1002),
            "Enter in adjust mode activates the row cursor"
        );
        assert_eq!(
            table.input(WidgetInput::DoubleClickAt(Point::new(40.0, y))),
            Some(1002)
        );
    }

    #[test]
    fn disabled_rows_do_not_select_or_activate() {
        let mut table: Table<usize> = Table::new("t")
            .row_height(30.0)
            .columns(vec![TableColumn::flex("名称")])
            .on_activate(|index| index)
            .on_select(|index| index);
        table.set_rect(Rect::from_xywh(0.0, 0.0, 300.0, 120.0));
        table.set_rows(vec![
            TableRow::new(["可用"]),
            TableRow::new(["禁用"]).disabled(true),
        ]);
        let body = table.body_rect();
        let y = body.top + 45.0;
        table.input(WidgetInput::PressAt(Point::new(30.0, y)));
        assert_eq!(
            table.input(WidgetInput::ReleaseAt(Point::new(30.0, y))),
            None
        );
        assert_eq!(table.selected_row(), 0);
        assert_eq!(
            table.input(WidgetInput::DoubleClickAt(Point::new(30.0, y))),
            None
        );
    }

    #[test]
    fn arrow_keys_move_the_cursor_and_keep_it_visible() {
        let mut table = table(1000);
        for _ in 0..10 {
            table.input(key(KeyCode::Down));
        }
        assert_eq!(table.selected_row(), 10);
        assert!(table.scroll_offset() > 0.0);
        let (start, end) = table.visible_rows();
        assert!(start <= 10 && 10 < end, "the cursor row is published");
        table.input(key(KeyCode::End));
        assert_eq!(table.selected_row(), 999);
        assert!((table.scroll_offset() - table.max_scroll()).abs() < 0.01);
        table.input(key(KeyCode::Home));
        assert_eq!(table.selected_row(), 0);
        assert_eq!(table.scroll_offset(), 0.0);
        table.input(key(KeyCode::PageDown));
        assert_eq!(table.selected_row(), table.page_rows());
    }

    #[test]
    fn sort_state_is_application_driven_and_validated() {
        let mut table = table(3);
        assert_eq!(table.sort_order(), None);
        assert!(table.set_sort(Some((2, true))));
        assert_eq!(table.sort_order(), Some((2, true)));
        assert!(!table.set_sort(Some((2, true))), "unchanged reports false");
        assert!(
            !table.set_sort(Some((9, true))),
            "out-of-range columns are refused"
        );
        assert_eq!(table.sort_order(), Some((2, true)));
        // Dropping the column drops the indicator; the builder clamps too.
        table.set_columns(vec![TableColumn::flex("a"), TableColumn::flex("b")]);
        assert_eq!(table.sort_order(), None);
        let clamped = table.sort(5, false);
        assert_eq!(clamped.sort_order(), None);
    }

    #[test]
    fn a11y_publishes_cells_rows_and_columns_offscreen_and_clipped() {
        let table = table(100);
        let info = table.a11y().expect("table is exposed");
        assert_eq!(info.role, A11yRole::Grid);
        assert_eq!(info.grid, Some((100, 3)));
        let (_, end) = table.visible_rows();
        // Three column headers, then one cell per visible row and column.
        assert_eq!(info.children.len(), 3 + end * 3);
        assert_eq!(info.children[0].name, "序号");
        assert_eq!(info.children[1].name, "名称");
        assert_eq!(info.children[2].name, "时长");
        assert_eq!(
            info.children[0].role,
            Some(A11yRole::ColumnHeader),
            "headers publish the header role"
        );
        assert!(info.children[0].index >= HEADER_INDEX_BASE);
        assert_eq!(info.children[3].name, "数字");
        assert_eq!(info.children[4].name, "行 0");
        assert_eq!(info.children[5].name, "3:45");
        assert_eq!(info.children[3].row, Some(0));
        assert_eq!(info.children[3].column, Some(0));
        assert_eq!(info.children[4].row, Some(0));
        assert_eq!(info.children[4].column, Some(1));
        assert_eq!(info.children[3].index, 0);
        assert_eq!(info.children[4].index, 1);
        assert_eq!(info.children[6].index, 3, "row 1 starts at 3");
        assert_eq!(
            info.children
                .iter()
                .filter(|child| child.selected)
                .count(),
            1,
            "exactly the cursor cell is selected"
        );
        assert!(info.children[3].selected);
        let body = table.body_rect();
        // The look-ahead row publishes all three cells as offscreen.
        let last_row: Vec<_> = info
            .children
            .iter()
            .filter(|child| child.row == Some(end - 1))
            .collect();
        assert_eq!(last_row.len(), 3);
        assert!(last_row.iter().all(|child| child.offscreen));
        for child in info
            .children
            .iter()
            .filter(|child| child.row.is_some() && child.row != Some(end - 1))
        {
            assert!(child.rect.top >= body.top);
            assert!(child.rect.bottom <= body.bottom);
        }
    }

    #[test]
    fn a11y_selection_moves_the_cell_cursor() {
        let mut table = table(50);
        // Cell index 1 is row 0, column 1: the cursor lands on it.
        assert_eq!(table.a11y_select_child(4), None); // row 1, column 1
        assert_eq!(table.selected_cell(), (1, 1));
        assert!(table.a11y_select_child(99999).is_none());
        assert_eq!(table.selected_cell(), (1, 1), "an unknown cell is ignored");
        // A header index is never a cell.
        assert!(table
            .a11y_select_child(HEADER_INDEX_BASE + 2)
            .is_none());
        assert_eq!(table.selected_cell(), (1, 1), "a header is not selectable");
    }

    #[test]
    fn a_uia_value_set_edits_the_cell_and_reports_it() {
        let mut table = editable_table();
        let info = table.a11y().expect("exposed");
        // The cell (0, 1) is after the three header children.
        let child = info.children[3 + 1].index;
        assert_eq!(info.children[3 + 1].value.as_deref(), Some("名 0"));
        assert!(!info.children[3 + 1].read_only, "editable cells say so");
        assert_eq!(
            table.a11y_set_value_at(Some(child), "自检"),
            Some((0, 1, "自检".to_string()))
        );
        assert_eq!(table.cell_text(0, 1), Some("自检"));
        assert!(table.a11y_set_value_at(Some(child), "自检").is_none());
        // Headers and locked tables refuse the edit.
        assert!(table
            .a11y_set_value_at(Some(HEADER_INDEX_BASE), "x")
            .is_none());
        table.set_read_only(true);
        assert!(table.a11y_set_value_at(Some(child), "y").is_none());
        assert_eq!(table.cell_text(0, 1), Some("自检"));
        let info = table.a11y().expect("exposed");
        assert!(info.children[3 + 1].read_only, "a locked grid is read-only");
    }

    #[test]
    fn arrow_keys_move_the_cell_cursor_across_columns() {
        let mut table = table(3);
        let (_, columns) = table.selected_cell();
        assert_eq!(columns, 0);
        table.input(key(KeyCode::Right));
        assert_eq!(table.selected_cell(), (0, 1));
        table.input(key(KeyCode::Right));
        table.input(key(KeyCode::Right));
        assert_eq!(table.selected_cell(), (0, 2), "the column clamps at the end");
        table.input(key(KeyCode::Down));
        assert_eq!(table.selected_cell(), (1, 2));
        table.input(key(KeyCode::Left));
        assert_eq!(table.selected_cell(), (1, 1));
    }

    fn editable_table() -> Table<(usize, usize, String)> {
        let mut table: Table<(usize, usize, String)> = Table::new("editable")
            .row_height(28.0)
            .columns(vec![
                TableColumn::new("编号", 60.0),
                TableColumn::flex("名称"),
                TableColumn::new("时长", 80.0),
            ])
            .editable(true)
            .on_cell_edit(|row, column, text| (row, column, text));
        table.set_rect(Rect::from_xywh(0.0, 0.0, 400.0, 200.0));
        table.set_rows(
            (0..5)
                .map(|index| TableRow::new([format!("{index}"), format!("名 {index}"), "1:00".to_string()]))
                .collect(),
        );
        table
    }

    #[test]
    fn enter_starts_a_cell_draft_and_typing_commits_it() {
        let mut table = editable_table();
        table.input(key(KeyCode::Right));
        // Enter opens the session without emitting a row activation.
        assert_eq!(table.input(key(KeyCode::Enter)), None);
        assert!(table.is_editing_cell());
        assert_eq!(table.selected_cell(), (0, 1));
        // Typing replaces the selected (all) text.
        assert!(table.insert_text("新名"));
        assert_eq!(table.finish_edit(true), Some((0, 1, "新名".to_string())));
        assert!(!table.is_editing_cell());
        assert_eq!(table.cell_text(0, 1), Some("新名"));
        // Escape reverts.
        table.edit_cell(0, 1);
        assert!(table.insert_text("草稿"));
        assert_eq!(table.finish_edit(false), None);
        assert_eq!(table.cell_text(0, 1), Some("新名"));
    }

    #[test]
    fn an_unchanged_draft_reports_nothing_and_locked_tables_stay_read_only() {
        let mut table = editable_table();
        table.edit_cell(1, 1);
        assert_eq!(table.finish_edit(true), None, "no change, no message");
        // A container read-only push closes the draft and blocks new ones.
        table.set_read_only(true);
        assert!(table.read_only());
        assert!(!table.edit_cell(0, 1));
        assert_eq!(table.input(key(KeyCode::Enter)), None);
        assert!(table.is_editing_cell() == false);
        // A disabled row never edits.
        table.set_read_only(false);
        let mut rows = table.rows_ref().to_vec();
        rows[2].disabled = true;
        table.set_rows(rows);
        assert!(!table.edit_cell(2, 1));
    }

    #[test]
    fn double_click_edits_an_editable_cell_and_places_the_caret() {
        let mut table = editable_table();
        let (left, width) = table.column_span(1).expect("column 1");
        let body = table.body_rect();
        let point = Point::new(left + width * 0.5, body.top + 10.0);
        assert_eq!(table.input(WidgetInput::DoubleClickAt(point)), None);
        assert!(table.is_editing_cell());
        assert_eq!(table.selected_cell(), (0, 1));
        // The click placed the caret, so typing appends; a click inside the
        // open cell only moves the caret and keeps the draft.
        assert!(table.insert_text("AB"));
        assert!(table.begin_inline_edit(Some(Point::new(left + 4.0, body.top + 10.0))));
        assert_eq!(table.finish_edit(true), Some((0, 1, "名 0AB".to_string())));
    }

    #[test]
    fn a_multiline_cell_edits_across_lines_and_takes_a_painted_newline() {
        let mut table = editable_table();
        let mut rows = table.rows_ref().to_vec();
        rows[0].cells[1] = "第一行\n第二行".to_string();
        table.set_rows(rows);
        assert!(table.edit_cell(0, 1));
        assert!(
            table.multiline_edit(),
            "a value with a newline uses the multiline model"
        );
        // The keyboard session starts with the whole value selected.
        assert!(table.insert_text("新\n内容"));
        assert_eq!(table.field.borrow().text(), "新\n内容");
        // Enter inserts a line break instead of committing.
        table.input(key(KeyCode::Enter));
        assert!(table.editing(), "the session stays open on a plain Enter");
        assert_eq!(table.field.borrow().text(), "新\n内容\n");
        assert_eq!(
            table.finish_edit(true),
            Some((0, 1, "新\n内容\n".to_string()))
        );
        assert!(!table.editing());
        // A single-line cell keeps the one-line contract.
        assert!(table.edit_cell(0, 0));
        assert!(!table.multiline_edit());
        table.input(key(KeyCode::Enter));
        assert!(!table.field.borrow().text().contains('\n'));
        table.finish_edit(false);
    }

    #[test]
    fn a_draft_publishes_its_live_text_and_commits_by_applying_it() {
        let mut table = editable_table();
        table.edit_cell(0, 1);
        assert!(table.insert_text("X"));
        let info = table.a11y().expect("exposed while editing");
        // The editing cell publishes the session text (a screen reader follows
        // the typing); its neighbours keep the committed data. The three
        // header children come first.
        assert_eq!(info.children[3 + 1].name, "X");
        assert_eq!(info.children[3].name, "0");
        assert_eq!(table.commit_draft(), Some((0, 1, "X".to_string())));
        assert_eq!(table.cell_text(0, 1), Some("X"));
        assert_eq!(table.commit_draft(), None, "the session closed");
    }

    #[test]
    fn a_click_on_another_cell_commits_the_draft_and_moves_the_cursor() {
        let mut editable_table = editable_table();
        let table = &mut editable_table;
        table.edit_cell(0, 1);
        assert!(table.insert_text("改"));
        let (left, width) = table.column_span(2).expect("column 2");
        let body = table.body_rect();
        let target = Point::new(left + width * 0.5, body.top + 10.0);
        table.input(WidgetInput::PressAt(target));
        assert_eq!(
            table.input(WidgetInput::ReleaseAt(target)),
            Some((0, 1, "改".to_string()))
        );
        assert!(!table.is_editing_cell());
        assert_eq!(table.selected_cell(), (0, 2));
        assert_eq!(table.cell_text(0, 1), Some("改"));
    }

    #[test]
    fn a_pointer_click_moves_the_cell_cursor_to_the_clicked_column() {
        let mut table = editable_table();
        table.select_cell(0, 1);
        let (left, _) = table.column_span(0).expect("column 0");
        let body = table.body_rect();
        let pos = Point::new(left + 8.0, body.top + 10.0);
        table.input(WidgetInput::PressAt(pos));
        table.input(WidgetInput::ReleaseAt(pos));
        assert_eq!(table.selected_cell(), (0, 0));
        // An open draft commits first and the cursor still lands on the
        // clicked column.
        table.edit_cell(0, 1);
        assert!(table.insert_text("改"));
        let (left, _) = table.column_span(2).expect("column 2");
        let pos = Point::new(left + 8.0, body.top + 10.0);
        table.input(WidgetInput::PressAt(pos));
        assert_eq!(
            table.input(WidgetInput::ReleaseAt(pos)),
            Some((0, 1, "改".to_string()))
        );
        assert_eq!(table.selected_cell(), (0, 2));
    }

    #[test]
    fn the_cursor_frame_follows_the_selected_cell_geometry() {
        let mut table = editable_table();
        table.select_cell(2, 1);
        let (left, width) = table.column_span(1).expect("column 1");
        let rect = table.cell_rect(2, 1).expect("cell rect");
        assert!((rect.left - left).abs() < 0.01);
        assert!((rect.width() - width).abs() < 0.01);
        let body = table.body_rect();
        assert!((rect.top - (body.top + 2.0 * table.row_height)).abs() < 0.01);
        assert!(table.cell_rect(99, 0).is_none());
        assert!(table.cell_rect(0, 99).is_none());
        assert_eq!(table.focus_item(), Some(2 * 3 + 1));
    }

    #[test]
    fn scrollbar_thumb_drags_and_a_bar_press_never_selects() {
        let mut table = table(1000);
        let track = table.scrollbar_rect();
        let thumb = table.thumb_rect().expect("rows overflow the body");
        let over_thumb = Point::new(track.center().x, thumb.center().y);
        assert_eq!(table.cursor_at(over_thumb), Some(Cursor::Arrow));
        assert_eq!(table.cursor_at(Point::new(10.0, 10.0)), None);
        table.input(WidgetInput::HoverAt(over_thumb));
        assert!(table.bar.hovered);
        assert!(table.hover_row.is_none());
        table.input(WidgetInput::PressAt(over_thumb));
        assert!(table.bar.active);
        let before = table.scroll_offset();
        table.input(WidgetInput::DragTo(Point::new(
            over_thumb.x,
            over_thumb.y + 40.0,
        )));
        assert!(table.scroll_offset() > before);
        assert_eq!(
            table.input(WidgetInput::ReleaseAt(Point::new(
                over_thumb.x,
                over_thumb.y + 40.0
            ))),
            None
        );
        assert!(!table.bar.active);
    }

    #[test]
    fn dragging_a_header_divider_resizes_the_fixed_column_and_reports_it() {
        let mut table: Table<(usize, f32)> = Table::new("t")
            .row_height(30.0)
            .columns(vec![
                TableColumn::new("A", 80.0),
                TableColumn::new("B", 80.0),
                TableColumn::flex("C"),
            ])
            .on_column_resize(|column, width| (column, width));
        table.set_rect(Rect::from_xywh(0.0, 0.0, 400.0, 160.0));
        table.set_rows(
            (0..10)
                .map(|index| TableRow::new(["a", "b", &format!("{index}")]))
                .collect(),
        );
        let (left, width) = table.column_span(0).expect("column 0");
        let edge = Point::new(left + width, 10.0);
        assert_eq!(table.cursor_at(edge), Some(Cursor::SizeWE));
        table.input(WidgetInput::PressAt(edge));
        assert!(table.resizing.is_some(), "a divider press starts a drag");
        table.input(WidgetInput::DragTo(Point::new(edge.x + 40.0, 10.0)));
        let message = table.input(WidgetInput::ReleaseAt(Point::new(edge.x + 40.0, 10.0)));
        assert_eq!(message, Some((0, 120.0)));
        assert_eq!(table.columns_ref()[0].width(), 120.0);
        assert!(!table.bar.active && table.resizing.is_none());
        // The flex column still absorbs the leftover.
        let content = table.content_rect().width();
        assert!((table.column_width(2) - (content - 200.0)).abs() < 0.01);

        // Dragging past the minimum clamps instead of vanishing.
        let (left, width) = table.column_span(0).expect("column 0");
        let edge = Point::new(left + width, 10.0);
        table.input(WidgetInput::PressAt(edge));
        table.input(WidgetInput::DragTo(Point::new(left - 500.0, 10.0)));
        table.input(WidgetInput::ReleaseAt(Point::new(left - 500.0, 10.0)));
        assert_eq!(table.columns_ref()[0].width(), MIN_COLUMN_WIDTH);
    }

    #[test]
    fn fixed_columns_that_overflow_scroll_horizontally_and_publish_the_axis() {
        let mut table: Table<()> = Table::new("t")
            .row_height(30.0)
            .columns(vec![
                TableColumn::new("A", 300.0),
                TableColumn::new("B", 300.0),
                TableColumn::new("C", 300.0),
            ]);
        table.set_rect(Rect::from_xywh(0.0, 0.0, 400.0, 160.0));
        table.set_rows((0..2).map(|index| TableRow::new([format!("{index}")])).collect());
        let content = table.content_rect();
        assert!(!table.has_scrollbar(), "two rows fit");
        assert!(table.has_hscrollbar(), "900 DIPs of columns do not");
        assert_eq!(table.columns_total(), 900.0);
        assert!((table.max_scroll_x() - (900.0 - content.width())).abs() < 0.01);

        // The horizontal bar sits under the content and the row area shrinks.
        let hbar = table.hscrollbar_rect();
        assert!(hbar.top >= content.bottom - 0.01);
        assert!((hbar.right - content.right).abs() < 0.01);
        let thumb = table.hthumb_rect().expect("the columns overflow");

        // A thumb drag moves the offset and clamps at the end.
        let point = thumb.center();
        table.input(WidgetInput::PressAt(point));
        assert!(table.hbar.active);
        let before = table.scroll_x_offset();
        table.input(WidgetInput::DragTo(Point::new(hbar.right + 400.0, point.y)));
        table.input(WidgetInput::ReleaseAt(Point::new(hbar.right + 400.0, point.y)));
        assert!(!table.hbar.active);
        assert!(table.scroll_x_offset() > before);
        assert!((table.scroll_x_offset() - table.max_scroll_x()).abs() < 0.01);

        // Hit-testing follows the offset: the third column reaches the
        // viewport, and a scrolled-out cell is reported as the last column.
        assert_eq!(table.column_at(content.right - 4.0), Some(2));
        assert!(table.set_scroll_x(0.0));
        assert_eq!(table.column_at(content.left + 4.0), Some(0));

        // The a11y snapshot publishes the horizontal axis, and a UIA request
        // moves it like the bar.
        let info = table.a11y().expect("table is exposed");
        let scroll = info.scroll.expect("scroll state");
        assert!(scroll.horizontal.is_some_and(|axis| axis.scrollable()));
        assert!(table.a11y_scroll(Some(table.max_scroll_x()), None));
        assert_eq!(table.scroll_x_offset(), table.max_scroll_x());
        assert!(!table.a11y_scroll(Some(table.max_scroll_x()), None));
    }

    #[test]
    fn dragging_a_header_cell_moves_the_column_and_reports_it() {
        let mut table: Table<(usize, usize)> = Table::new("t")
            .header_height(30.0)
            .row_height(28.0)
            .columns(vec![
                TableColumn::new("A", 60.0),
                TableColumn::new("B", 100.0),
                TableColumn::new("C", 140.0),
            ])
            .on_header_click(|column| (100, column))
            .on_column_reorder(|from, to| (from, to));
        table.set_rect(Rect::from_xywh(0.0, 0.0, 400.0, 200.0));
        table.set_rows(vec![TableRow::new(["a", "b", "c"])]);
        table.set_sort(Some((0, true)));

        let (left, width) = table.column_span(0).expect("column 0");
        let start = Point::new(left + width * 0.5, 10.0);
        let (last_left, last_width) = table.column_span(2).expect("column 2");
        let end = Point::new(last_left + last_width + 10.0, 10.0);
        table.input(WidgetInput::PressAt(start));
        // A tiny move stays a click.
        table.input(WidgetInput::DragTo(Point::new(start.x + 3.0, 10.0)));
        assert!(table.reordering.is_none());
        table.input(WidgetInput::DragTo(end));
        assert!(table.reordering.is_some(), "past the threshold it is a move");
        assert_eq!(table.input(WidgetInput::ReleaseAt(end)), Some((0, 2)));
        let titles: Vec<&str> = table
            .columns_ref()
            .iter()
            .map(|column| column.title())
            .collect();
        assert_eq!(titles, ["B", "C", "A"]);
        assert_eq!(
            table.sort_order(),
            Some((2, true)),
            "the sort indicator follows its column"
        );
        assert!(!table.move_column(0, 0));
        assert!(!table.move_column(9, 0));

        // A plain header click still emits the click after a drag happened.
        let (left, width) = table.column_span(1).expect("column 1");
        let point = Point::new(left + width * 0.5, 10.0);
        table.input(WidgetInput::PressAt(point));
        assert_eq!(table.input(WidgetInput::ReleaseAt(point)), Some((100, 1)));
    }

    #[test]
    fn typed_access_updates_cells_rows_and_scroll() {
        let mut table = table(50);
        assert!(table.set_cell(3, 1, "改过"));
        assert_eq!(table.row(3).map(|row| row.cell(1)), Some("改过"));
        assert!(!table.set_cell(3, 1, "改过"), "unchanged reports false");
        assert!(!table.set_cell(99, 0, "x"));
        assert_eq!(table.row_count(), 50);
        assert!(table.scroll_to_row(40));
        assert!(!table.ensure_row_visible(40), "already at the viewport top");
        table.scroll_to_row(0);
        assert!(table.ensure_row_visible(40), "scrolls just far enough");
        assert!(table.set_rows(vec![TableRow::new(["唯一"])]));
        assert_eq!(table.row_count(), 1);
        assert_eq!(table.selected_row(), 0);
        assert_eq!(table.scroll_offset(), 0.0);
        // Missing cells paint empty rather than panicking.
        assert_eq!(table.row(0).unwrap().cell(5), "");
    }

    #[test]
    fn the_column_and_row_child_lists_replace_the_table_data() {
        let mut table: Table<()> = Table::new("test");
        let mut title = SkinElement::new("column", 2, 3);
        title.push_attr("title", "名称");
        title.push_attr("width", "120");
        title.push_attr("align", "right");
        let mut flex = SkinElement::new("column", 3, 3);
        flex.push_attr("title", "备注");
        flex.push_attr("flex", "true");
        let mut row = SkinElement::new("row", 4, 3);
        row.push_attr("disabled", "true");
        let mut cell = SkinElement::new("cell", 5, 5);
        cell.push_attr("text", "夜曲");
        row.push_child(cell);
        let mut empty = SkinElement::new("cell", 6, 5);
        empty.push_attr("text", "");
        row.push_child(empty);
        let children = [title, flex, row];
        assert!(<Table<()> as Widget<()>>::validate_skin_children(&table, &children));
        <Table<()> as Widget<()>>::apply_skin_children(&mut table, &children);
        assert_eq!(table.columns_ref().len(), 2);
        assert_eq!(table.columns_ref()[0].title(), "名称");
        assert_eq!(table.columns_ref()[0].width(), 120.0);
        assert_eq!(table.columns_ref()[0].alignment(), TextAlign::Right);
        assert!(table.columns_ref()[1].is_flex());
        assert_eq!(table.rows_ref().len(), 1);
        assert!(table.rows_ref()[0].disabled);
        assert_eq!(table.rows_ref()[0].cell(0), "夜曲");
        assert_eq!(table.rows_ref()[0].cell(1), "");

        // A cell attribute is `text`; a typo rejects the whole list.
        let mut typo = SkinElement::new("cell", 8, 5);
        typo.push_attr("txt", "x");
        let mut bad_row = SkinElement::new("row", 7, 3);
        bad_row.push_child(typo);
        assert!(!<Table<()> as Widget<()>>::validate_skin_children(&table, &[bad_row]));
        // An unknown alignment and a negative width are rejected too.
        let mut bad_align = SkinElement::new("column", 9, 3);
        bad_align.push_attr("title", "x");
        bad_align.push_attr("align", "middle");
        assert!(!<Table<()> as Widget<()>>::validate_skin_children(&table, &[bad_align]));
        let mut bad_width = SkinElement::new("column", 10, 3);
        bad_width.push_attr("title", "x");
        bad_width.push_attr("width", "-4");
        assert!(!<Table<()> as Widget<()>>::validate_skin_children(&table, &[bad_width]));
    }
}
