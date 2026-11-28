// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Navigation/layout widgets: the accordion, breadcrumb, navigation rail and
//! splitter.
//!
//! All four paint inside their own rectangle and leave sibling layout to the
//! application (D1). The accordion adds open-body height, the splitter reports
//! drag positions so the app can resize panes, and the breadcrumb/nav rail
//! emit the clicked destination.

use std::cell::RefCell;

use crate::a11y::{A11yChild, A11yInfo, A11yRole};
use crate::backend::{TextAlign, TextStyle};
use crate::event::{Cursor, KeyCode};
use crate::geometry::{Point, Rect};
use crate::text::wrap_ranges;
use crate::skin::{SkinAttrDecl, SkinElement, SkinValue, SkinValueKind};use crate::widget::{TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

static ACCORDION_SKIN_ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("open", SkinValueKind::Number)];

/// A single collapsible section is just an accordion built with one title.
pub type Collapsible<M> = Accordion<M>;

const BODY_INSET: f32 = 12.0;
const BODY_PAD_Y: f32 = 8.0;
const CHEVRON_COLLAPSED: char = '\u{E70D}';
const CHEVRON_EXPANDED: char = '\u{E70E}';

#[derive(Default)]
struct BodyCache {
    revision: u64,
    width: f32,
    font_size: f32,
    ranges: Vec<(usize, usize)>,
}

struct Section {
    title: String,
    body: String,
    open: bool,
    /// Bumped when the body text changes, so the wrap cache cannot reuse
    /// ranges for different text of the same length.
    revision: u64,
}

pub struct Accordion<M> {
    id: WidgetId,
    rect: Rect,
    sections: Vec<Section>,
    header_height: f32,
    body_height: f32,
    /// Only one section open at a time (the accordion mode).
    single_open: bool,
    enabled: bool,
    /// Keyboard/UIA section cursor.
    cursor: usize,
    hover: Option<usize>,
    label: Option<String>,
    chevron_collapsed: char,
    chevron_expanded: char,
    wrap: RefCell<Vec<BodyCache>>,
    on_toggle: Box<dyn Fn(usize, bool) -> M>,
}

impl<M> Accordion<M> {
    /// `titles` fixes the section count; bodies start empty and closed.
    pub fn new(
        id: &'static str,
        titles: &[&str],
        on_toggle: impl Fn(usize, bool) -> M + 'static,
    ) -> Self {
        let sections = titles
            .iter()
            .map(|title| Section {
                title: (*title).to_string(),
                body: String::new(),
                open: false,
                revision: 0,
            })
            .collect::<Vec<_>>();
        let cache = (0..sections.len()).map(|_| BodyCache::default()).collect();
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 280.0, 120.0),
            sections,
            header_height: 34.0,
            body_height: 56.0,
            single_open: false,
            enabled: true,
            cursor: 0,
            hover: None,
            label: None,
            chevron_collapsed: CHEVRON_COLLAPSED,
            chevron_expanded: CHEVRON_EXPANDED,
            wrap: RefCell::new(cache),
            on_toggle: Box::new(on_toggle),
        }
    }

    /// Body text for one section (painted with word wrap while it is open).
    pub fn body(mut self, index: usize, text: impl Into<String>) -> Self {
        if let Some(section) = self.sections.get_mut(index) {
            section.body = text.into();
            section.revision = section.revision.wrapping_add(1);
        }
        self
    }

    /// Start with one section open or closed.
    pub fn open(mut self, index: usize, open: bool) -> Self {
        self.set_expanded(index, open);
        self
    }

    /// Only one section open at a time (default: independent sections).
    pub fn single_open(mut self, single: bool) -> Self {
        self.single_open = single;
        self
    }

    /// Header row height in DIPs (default 34).
    pub fn header_height(mut self, height: f32) -> Self {
        self.header_height = height.max(16.0);
        self
    }

    /// Body area height of an open section in DIPs (default 56).
    pub fn body_height(mut self, height: f32) -> Self {
        self.body_height = height.max(0.0);
        self
    }

    /// Chevron characters for the collapsed and expanded states. A private-use
    /// codepoint is drawn with the glyph face; a text character uses the UI
    /// face.
    pub fn chevrons(mut self, collapsed: char, expanded: char) -> Self {
        self.chevron_collapsed = collapsed;
        self.chevron_expanded = expanded;
        self
    }

    /// Accessible name of the container (defaults to the widget id).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn len(&self) -> usize {
        self.sections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    pub fn is_expanded(&self, index: usize) -> bool {
        self.sections.get(index).is_some_and(|section| section.open)
    }

    /// First open section (single-open accordions have at most one).
    pub fn open_index(&self) -> Option<usize> {
        self.sections.iter().position(|section| section.open)
    }

    /// Set a section's state without emitting a message. Returns whether
    /// anything changed.
    pub fn set_expanded(&mut self, index: usize, open: bool) -> bool {
        if index >= self.sections.len() {
            return false;
        }
        let mut changed = false;
        if open && self.single_open {
            for (other, section) in self.sections.iter_mut().enumerate() {
                if other != index && section.open {
                    section.open = false;
                    changed = true;
                }
            }
        }
        if self.sections[index].open != open {
            self.sections[index].open = open;
            changed = true;
        }
        changed
    }

    /// Replace a section body at runtime (silent).
    pub fn set_body(&mut self, index: usize, text: impl Into<String>) {
        if let Some(section) = self.sections.get_mut(index) {
            section.body = text.into();
            section.revision = section.revision.wrapping_add(1);
        }
    }

    /// Section title (silent).
    pub fn set_title(&mut self, index: usize, title: impl Into<String>) {
        if let Some(section) = self.sections.get_mut(index) {
            section.title = title.into();
        }
    }

    /// Toggle a section through the normal path and return the widget's
    /// message.
    pub fn toggle(&mut self, index: usize) -> Option<M> {
        self.toggle_index(index)
    }

    fn toggle_index(&mut self, index: usize) -> Option<M> {
        if !self.enabled || index >= self.sections.len() {
            return None;
        }
        let open = !self.sections[index].open;
        if open && self.single_open {
            for (other, section) in self.sections.iter_mut().enumerate() {
                if other != index {
                    section.open = false;
                }
            }
        }
        self.sections[index].open = open;
        self.cursor = index;
        Some((self.on_toggle)(index, open))
    }

    /// Header rectangle of one section (closed bodies take no space).
    fn header_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.sections.len() {
            return None;
        }
        let mut top = self.rect.top;
        for (current, section) in self.sections.iter().enumerate() {
            let header = Rect::new(
                self.rect.left,
                top,
                self.rect.right,
                top + self.header_height,
            );
            if current == index {
                return Some(header);
            }
            top = header.bottom;
            if section.open {
                top += self.body_height;
            }
        }
        None
    }

    fn body_rect(&self, index: usize) -> Option<Rect> {
        let header = self.header_rect(index)?;
        if !self.is_expanded(index) {
            return None;
        }
        Some(Rect::new(
            self.rect.left + BODY_INSET,
            header.bottom + BODY_PAD_Y,
            (self.rect.right - BODY_INSET).max(self.rect.left + BODY_INSET + 1.0),
            (header.bottom + self.body_height - BODY_PAD_Y).max(header.bottom + BODY_PAD_Y),
        ))
    }

    fn header_at(&self, pos: Point) -> Option<usize> {
        if !self.rect.contains_point(pos) {
            return None;
        }
        let mut top = self.rect.top;
        for (index, section) in self.sections.iter().enumerate() {
            let header = Rect::new(
                self.rect.left,
                top,
                self.rect.right,
                top + self.header_height,
            );
            if header.contains_point(pos) {
                return Some(index);
            }
            top = header.bottom;
            if section.open {
                let body = Rect::new(self.rect.left, top, self.rect.right, top + self.body_height);
                if body.contains_point(pos) {
                    return None;
                }
                top += self.body_height;
            }
        }
        None
    }

    fn step(&mut self, delta: i32) {
        if self.sections.is_empty() {
            return;
        }
        let len = self.sections.len() as i32;
        self.cursor = (self.cursor as i32 + delta).rem_euclid(len) as usize;
    }

    fn chevron_char(&self, index: usize) -> char {
        if self.is_expanded(index) {
            self.chevron_expanded
        } else {
            self.chevron_collapsed
        }
    }

    fn paint_body(
        &self,
        ctx: &mut WidgetPaint<'_>,
        index: usize,
        rect: Rect,
        style: &TextStyle,
        color: crate::Color,
    ) {
        let section = &self.sections[index];
        if section.body.is_empty() || rect.height() < 1.0 {
            return;
        }
        let width = rect.width().max(1.0);
        let mut caches = self.wrap.borrow_mut();
        let cache = &mut caches[index];
        if cache.revision != section.revision
            || cache.width != width
            || cache.font_size != style.size
        {
            cache.ranges = wrap_ranges(&section.body, width, &mut |slice| {
                ctx.canvas.measure_text(slice, style).width
            });
            cache.revision = section.revision;
            cache.width = width;
            cache.font_size = style.size;
        }
        let line_height = ctx.canvas.measure_text("Ag", style).height.max(style.size);
        for (line, &(start, end)) in cache.ranges.iter().enumerate() {
            let top = rect.top + line as f32 * line_height;
            if top + line_height > rect.bottom {
                break;
            }
            ctx.canvas.text(
                Rect::new(rect.left, top, rect.right, top + line_height),
                &section.body[start..end],
                style,
                TextAlign::Left,
                color,
            );
        }
    }
}

impl<M: 'static> Widget<M> for Accordion<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        ACCORDION_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("open", SkinValue::Number(index)) => *index >= 0.0,
            _ => true,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        if let ("open", SkinValue::Number(index)) = (name, value) {
            let target = *index as usize;
            for section in 0..self.sections.len() {
                self.set_expanded(section, section == target);
            }
            if target < self.sections.len() {
                self.cursor = target;
            }
        }
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
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        let open = self.sections.iter().filter(|section| section.open).count();
        self.sections.len() as f32 * self.header_height + open as f32 * self.body_height
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover = None;
        }
    }

    fn adjustable(&self) -> bool {
        true
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Hand)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        if self.sections.is_empty() {
            return;
        }
        let palette = ctx.palette;
        let body_style = ctx.style(ctx.painter.body());
        let mut top = self.rect.top;
        // Bodies and the last header are clipped to the widget cell; the app
        // sizes the rectangle to the open state.
        ctx.canvas.push_clip(self.rect);
        for (index, section) in self.sections.iter().enumerate() {
            if top >= self.rect.bottom {
                break;
            }
            let header = Rect::new(
                self.rect.left,
                top,
                self.rect.right,
                (top + self.header_height).min(self.rect.bottom),
            );
            if self.enabled && self.hover == Some(index) {
                ctx.canvas.fill_rect(header, palette.row_hover);
            }
            let color = if self.enabled {
                palette.text
            } else {
                palette.text_disabled
            };
            let title = Rect::new(
                header.left + BODY_INSET,
                header.top,
                (header.right - 30.0).max(header.left + BODY_INSET + 1.0),
                header.bottom,
            );
            ctx.text(
                title,
                &section.title,
                TextStyleRole::Body,
                TextAlign::Left,
                color,
            );
            let chevron = Rect::new(header.right - 28.0, header.top, header.right - 8.0, header.bottom);
            let mut buf = [0u8; 4];
            let glyph = self.chevron_char(index).encode_utf8(&mut buf);
            let code = self.chevron_char(index) as u32;
            let tint = if self.enabled {
                palette.muted
            } else {
                palette.text_disabled
            };
            if (0xE000..=0xF8FF).contains(&code) {
                ctx.text(chevron, glyph, TextStyleRole::Glyph, TextAlign::Center, tint);
            } else {
                ctx.text(chevron, glyph, TextStyleRole::Small, TextAlign::Center, tint);
            }
            if header.bottom <= self.rect.bottom {
                ctx.canvas.fill_rect(
                    Rect::new(header.left, header.bottom - 1.0, header.right, header.bottom),
                    palette.divider,
                );
            }
            top = header.bottom;
            if section.open && top < self.rect.bottom {
                if let Some(body) = self.body_rect(index) {
                    let body_color = if self.enabled {
                        palette.muted
                    } else {
                        palette.text_disabled
                    };
                    self.paint_body(ctx, index, body, &body_style, body_color);
                }
                top += self.body_height;
            }
        }
        ctx.canvas.pop_clip();
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::HoverAt(pos) => {
                self.hover = self.header_at(pos);
                None
            }
            WidgetInput::HoverLeave => {
                self.hover = None;
                None
            }
            WidgetInput::ClickAt(pos) => {
                let index = self.header_at(pos)?;
                self.toggle_index(index)
            }
            WidgetInput::Activate => self.toggle_index(self.cursor),
            WidgetInput::Key(key) => match key.code {
                KeyCode::Up | KeyCode::Left => {
                    self.step(-1);
                    None
                }
                KeyCode::Down | KeyCode::Right => {
                    self.step(1);
                    None
                }
                KeyCode::Home => {
                    self.cursor = 0;
                    None
                }
                KeyCode::End => {
                    self.cursor = self.sections.len().saturating_sub(1);
                    None
                }
                KeyCode::Enter | KeyCode::Space => self.toggle_index(self.cursor),
                _ => None,
            },
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        // Each section is a ListItem that reports its open state through the
        // ExpandCollapse pattern; SelectionItem (the open section is selected)
        // keeps the list-shaped navigation. See D49/D98.
        let children = self
            .sections
            .iter()
            .enumerate()
            .filter_map(|(index, section)| {
                Some(
                    A11yChild::new(index, section.title.clone(), self.header_rect(index)?)
                        .selected(section.open)
                        .expanded(section.open)
                        .enabled(self.enabled),
                )
            })
            .collect();
        let name = self
            .label
            .clone()
            .unwrap_or_else(|| self.id.0.to_string());
        Some(A11yInfo::new(A11yRole::List, name).children(children))
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        self.toggle_index(index)
    }

    /// A UIA Expand/Collapse request carries the section index and drives the
    /// same state change as the header click (in single-open mode the other
    /// sections close silently).
    fn a11y_expand(&mut self, child: Option<usize>, expanded: bool) -> Option<M> {
        let index = child?;
        if !self.enabled || index >= self.sections.len() || self.sections[index].open == expanded {
            return None;
        }
        self.set_expanded(index, expanded);
        self.cursor = index;
        Some((self.on_toggle)(index, expanded))
    }

    fn set_selected(&mut self, index: usize) {
        if index < self.sections.len() {
            self.cursor = index;
        }
    }

    fn tooltip_rect(&self) -> Rect {
        self.header_rect(self.cursor).unwrap_or(self.rect)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.header_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        (!self.sections.is_empty()).then_some(self.cursor.min(self.sections.len() - 1))
    }
}

const SEP_CELL: f32 = 18.0;

#[derive(Default)]
struct CrumbCache {
    valid: bool,
    revision: u64,
    width: f32,
    font_size: f32,
    rects: Vec<Rect>,
}

static BREADCRUMB_SKIN_CHILDREN: &[&str] = &["step"];
const BREADCRUMB_STEP_ATTRS: &[&str] = &["label"];
static BREADCRUMB_SKIN_ATTRS: &[SkinAttrDecl] =
    &[SkinAttrDecl::new("separator", SkinValueKind::Text)];
static NAVRAIL_SKIN_CHILDREN: &[&str] = &["step"];
const NAVRAIL_STEP_ATTRS: &[&str] = &["label", "icon"];
static NAVRAIL_SKIN_ATTRS: &[SkinAttrDecl] =
    &[SkinAttrDecl::new("row-height", SkinValueKind::Number)];

/// A path of clickable crumbs. The last item is the current page and is not
/// clickable; earlier items emit their index through `on_navigate`.
///
/// Items are measured while painting and cached (width/font/revision), so the
/// paint path and hit-testing share one layout and an idle repaint does not
/// measure again.
pub struct Breadcrumb<M> {
    id: WidgetId,
    rect: Rect,
    items: Vec<String>,
    separator: char,
    label: Option<String>,
    enabled: bool,
    cursor: usize,
    hover: Option<usize>,
    revision: u64,
    cache: RefCell<CrumbCache>,
    on_navigate: Box<dyn Fn(usize) -> M>,
}

impl<M> Breadcrumb<M> {
    pub fn new(
        id: &'static str,
        items: &[&str],
        on_navigate: impl Fn(usize) -> M + 'static,
    ) -> Self {
        let cursor = items.len().saturating_sub(1);
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 260.0, 24.0),
            items: items.iter().map(|item| (*item).to_string()).collect(),
            separator: '\u{E76C}',
            label: None,
            enabled: true,
            cursor,
            hover: None,
            revision: 0,
            cache: RefCell::new(CrumbCache::default()),
            on_navigate: Box::new(on_navigate),
        }
    }

    /// Separator character (default MDL2 chevron-right U+E76C; a text
    /// character uses the UI face).
    pub fn separator(mut self, separator: char) -> Self {
        self.separator = separator;
        self
    }

    /// Runtime separator swap; the measured trail is invalidated. Returns
    /// whether it changed.
    pub fn set_separator(&mut self, separator: char) -> bool {
        if self.separator == separator {
            return false;
        }
        self.separator = separator;
        self.revision = self.revision.wrapping_add(1);
        true
    }

    pub fn separator_char(&self) -> char {
        self.separator
    }

    /// Accessible name of the trail (defaults to the widget id).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// Index of the current (last) crumb.
    pub fn current_index(&self) -> Option<usize> {
        (!self.items.is_empty()).then(|| self.items.len() - 1)
    }

    /// Replace the trail (silent); the cursor moves to the new current crumb.
    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.cursor = self.items.len().saturating_sub(1);
        self.hover = None;
        self.revision = self.revision.wrapping_add(1);
    }

    fn index_at(&self, pos: Point) -> Option<usize> {
        if !self.rect.contains_point(pos) {
            return None;
        }
        let cache = self.cache.borrow();
        cache
            .rects
            .iter()
            .position(|rect| rect.contains_point(pos))
    }

    /// Navigate to `index` through the normal path (the current crumb does
    /// nothing) and return the widget's message.
    pub fn navigate(&mut self, index: usize) -> Option<M> {
        if !self.enabled || index >= self.items.len().saturating_sub(1) {
            return None;
        }
        self.cursor = index;
        Some((self.on_navigate)(index))
    }

    fn step_cursor(&mut self, delta: i32) {
        if self.items.is_empty() {
            return;
        }
        let len = self.items.len() as i32;
        self.cursor = (self.cursor as i32 + delta).rem_euclid(len) as usize;
    }

    fn ensure_cache(&self, ctx: &mut WidgetPaint<'_>) -> TextStyle {
        let style = ctx.style(ctx.painter.body());
        let mut cache = self.cache.borrow_mut();
        if cache.valid
            && cache.revision == self.revision
            && cache.width == self.rect.width()
            && cache.font_size == style.size
        {
            return style;
        }
        let mut rects = Vec::with_capacity(self.items.len());
        let mut x = self.rect.left;
        for (index, item) in self.items.iter().enumerate() {
            let width = ctx.canvas.measure_text(item, &style).width;
            let rect = Rect::new(
                x,
                self.rect.top,
                (x + width).min(self.rect.right),
                self.rect.bottom,
            );
            rects.push(rect);
            x = rect.right;
            if index + 1 < self.items.len() {
                x = (x + SEP_CELL).min(self.rect.right);
            }
        }
        cache.rects = rects;
        cache.valid = true;
        cache.revision = self.revision;
        cache.width = self.rect.width();
        cache.font_size = style.size;
        style
    }
}

impl<M: 'static> Widget<M> for Breadcrumb<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        BREADCRUMB_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("separator", SkinValue::Text(text)) => text.chars().count() == 1,
            _ => false,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        if let ("separator", SkinValue::Text(text)) = (name, value)
            && let Some(separator) = text.chars().next()
        {
            self.set_separator(separator);
        }
    }

    fn skin_children(&self) -> &'static [&'static str] {
        BREADCRUMB_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(|child| {
            child.name == "step"
                && child
                    .attr("label")
                    .is_some_and(|label| !label.trim().is_empty())
                && child.attrs_are(BREADCRUMB_STEP_ATTRS)
                && child.children.is_empty()
        })
    }

    /// `<step label>` elements replace the trail; as in `set_items`, the cursor
    /// moves to the last crumb (the current page).
    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        self.set_items(
            children
                .iter()
                .filter_map(|child| child.attr("label").map(str::to_string))
                .collect(),
        );
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
        if rect != self.rect {
            self.rect = rect;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        24.0
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover = None;
        }
    }

    fn adjustable(&self) -> bool {
        true
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        let hovered = self.index_at(pos);
        match (hovered, self.current_index()) {
            (Some(index), Some(current)) if index != current => Some(Cursor::Hand),
            _ => Some(Cursor::Arrow),
        }
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        if self.items.is_empty() {
            return;
        }
        let style = self.ensure_cache(ctx);
        let palette = ctx.palette;
        let current = self.items.len() - 1;
        ctx.canvas.push_clip(self.rect);
        let cache = self.cache.borrow();
        for (index, item) in self.items.iter().enumerate() {
            let Some(&rect) = cache.rects.get(index) else {
                break;
            };
            if rect.left >= self.rect.right {
                break;
            }
            let color = if !self.enabled {
                palette.text_disabled
            } else if index == current {
                palette.text
            } else {
                palette.accent_text
            };
            ctx.canvas.text(rect, item, &style, TextAlign::Left, color);
            if self.enabled && index != current && self.hover == Some(index) {
                ctx.canvas.fill_rect(
                    Rect::new(rect.left, rect.bottom - 3.0, rect.right, rect.bottom - 2.0),
                    palette.accent,
                );
            }
            if index != current {
                let sep = Rect::new(
                    rect.right + 4.0,
                    self.rect.top,
                    rect.right + SEP_CELL,
                    self.rect.bottom,
                );
                let mut buf = [0u8; 4];
                let glyph = self.separator.encode_utf8(&mut buf);
                let glyph_face = (0xE000..=0xF8FF).contains(&(self.separator as u32));
                let tint = if self.enabled {
                    palette.muted
                } else {
                    palette.text_disabled
                };
                if glyph_face {
                    ctx.canvas
                        .text(sep, glyph, &TextStyle::glyph(10.0), TextAlign::Left, tint);
                } else {
                    ctx.canvas
                        .text(sep, glyph, &style, TextAlign::Left, tint);
                }
            }
        }
        ctx.canvas.pop_clip();
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::HoverAt(pos) => {
                self.hover = self.index_at(pos);
                None
            }
            WidgetInput::HoverLeave => {
                self.hover = None;
                None
            }
            WidgetInput::ClickAt(pos) => {
                let index = self.index_at(pos)?;
                self.navigate(index)
            }
            WidgetInput::Activate => self.navigate(self.cursor),
            WidgetInput::Key(key) => match key.code {
                KeyCode::Left => {
                    self.step_cursor(-1);
                    None
                }
                KeyCode::Right => {
                    self.step_cursor(1);
                    None
                }
                KeyCode::Home => {
                    self.cursor = 0;
                    None
                }
                KeyCode::End => {
                    self.cursor = self.items.len().saturating_sub(1);
                    None
                }
                KeyCode::Enter | KeyCode::Space => self.navigate(self.cursor),
                _ => None,
            },
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let current = self.current_index();
        let cache = self.cache.borrow();
        let children = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                A11yChild::new(
                    index,
                    item.clone(),
                    cache.rects.get(index).copied().unwrap_or(self.rect),
                )
                .selected(Some(index) == current)
                .enabled(self.enabled)
            })
            .collect();
        let name = self
            .label
            .clone()
            .unwrap_or_else(|| self.id.0.to_string());
        Some(A11yInfo::new(A11yRole::List, name).children(children))
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        self.navigate(index)
    }

    fn set_selected(&mut self, index: usize) {
        self.cursor = index.min(self.items.len().saturating_sub(1));
    }

    fn tooltip_rect(&self) -> Rect {
        self.cache
            .borrow()
            .rects
            .get(self.cursor)
            .copied()
            .unwrap_or(self.rect)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.index_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        (!self.items.is_empty()).then_some(self.cursor.min(self.items.len() - 1))
    }
}

/// One rail row: a label and an optional leading glyph.
#[derive(Clone, Debug, PartialEq)]
pub struct NavItem {
    pub label: String,
    pub icon: Option<char>,
}

impl NavItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
        }
    }

    /// Leading icon (an MDL2 glyph or any text character).
    pub fn icon(mut self, glyph: char) -> Self {
        self.icon = Some(glyph);
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn icon_glyph(&self) -> Option<char> {
        self.icon
    }
}

/// A vertical navigation rail (sidebar item list). One row is selected; click
/// or arrows in adjust mode change it and emit the index. Icons are optional
/// MDL2 glyphs (or any text character).
pub struct NavRail<M> {
    id: WidgetId,
    rect: Rect,
    items: Vec<NavItem>,
    selected: usize,
    row_height: f32,
    enabled: bool,
    hover: Option<usize>,
    label: Option<String>,
    on_select: Box<dyn Fn(usize) -> M>,
}

impl<M> NavRail<M> {
    pub fn new(
        id: &'static str,
        labels: &[&str],
        selected: usize,
        on_select: impl Fn(usize) -> M + 'static,
    ) -> Self {
        let items = labels
            .iter()
            .map(|label| NavItem {
                label: (*label).to_string(),
                icon: None,
            })
            .collect();
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 180.0, 140.0),
            items,
            selected: selected.min(labels.len().saturating_sub(1)),
            row_height: 34.0,
            enabled: true,
            hover: None,
            label: None,
            on_select: Box::new(on_select),
        }
    }

    /// Leading icon for one row. A private-use codepoint is drawn with the
    /// glyph face; a text character uses the UI face.
    pub fn icon(mut self, index: usize, glyph: char) -> Self {
        if let Some(item) = self.items.get_mut(index) {
            item.icon = Some(glyph);
        }
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(18.0);
        self
    }

    /// Row height in DIPs (18 minimum). Returns whether it changed.
    pub fn set_row_height(&mut self, height: f32) -> bool {
        let height = height.max(18.0);
        if (self.row_height - height).abs() < 0.01 {
            return false;
        }
        self.row_height = height;
        true
    }

    pub fn row_height_value(&self) -> f32 {
        self.row_height
    }

    /// Accessible name of the rail (defaults to the widget id).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn set_selected(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected = index;
        }
    }

    /// The rail rows (typed access).
    pub fn items(&self) -> &[NavItem] {
        &self.items
    }

    /// Replace the rows (silent). The selection clamps and the hover clears.
    /// Returns whether the list changed.
    pub fn set_items(&mut self, items: Vec<NavItem>) -> bool {
        if self.items == items {
            return false;
        }
        self.items = items;
        self.selected = self.selected.min(self.items.len().saturating_sub(1));
        self.hover = None;
        true
    }

    /// Select a row through the normal path and return the message.
    pub fn select(&mut self, index: usize) -> Option<M> {
        self.select_index(index)
    }

    fn row_rect(&self, index: usize) -> Option<Rect> {
        (index < self.items.len()).then(|| {
            let top = self.rect.top + index as f32 * self.row_height;
            Rect::new(self.rect.left, top, self.rect.right, top + self.row_height)
        })
    }

    fn row_at(&self, pos: Point) -> Option<usize> {
        if !self.rect.contains_point(pos) {
            return None;
        }
        let row = ((pos.y - self.rect.top) / self.row_height).floor();
        if row < 0.0 {
            return None;
        }
        let index = row as usize;
        (index < self.items.len()).then_some(index)
    }

    fn select_index(&mut self, index: usize) -> Option<M> {
        if !self.enabled || index >= self.items.len() || index == self.selected {
            return None;
        }
        self.selected = index;
        Some((self.on_select)(index))
    }

    fn step(&mut self, delta: i32) -> Option<M> {
        if self.items.is_empty() {
            return None;
        }
        let len = self.items.len() as i32;
        let next = (self.selected as i32 + delta).rem_euclid(len) as usize;
        self.select_index(next)
    }
}

impl<M: 'static> Widget<M> for NavRail<M> {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::SELECTION
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        NAVRAIL_SKIN_ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("row-height", SkinValue::Number(height)) => *height >= 18.0,
            _ => false,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        if let ("row-height", SkinValue::Number(height)) = (name, value) {
            self.set_row_height(*height as f32);
        }
    }

    fn skin_children(&self) -> &'static [&'static str] {
        NAVRAIL_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(|child| {
            child.name == "step"
                && child
                    .attr("label")
                    .is_some_and(|label| !label.trim().is_empty())
                && child.attrs_are(NAVRAIL_STEP_ATTRS)
                && child
                    .attr("icon")
                    .is_none_or(|icon| icon.chars().count() == 1)
                && child.children.is_empty()
        })
    }

    /// `<step label icon>` elements replace the rows; `selected` applies
    /// afterwards.
    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        let items = children
            .iter()
            .filter_map(|child| {
                let mut item = NavItem::new(child.attr("label")?);
                if let Some(icon) = child.attr("icon").and_then(|icon| icon.chars().next()) {
                    item.icon = Some(icon);
                }
                Some(item)
            })
            .collect();
        self.set_items(items);
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
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        self.items.len() as f32 * self.row_height
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover = None;
        }
    }

    fn adjustable(&self) -> bool {
        true
    }

    fn repaint_on_move(&self) -> bool {
        true
    }

    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        (self.row_at(pos).is_some()).then_some(Cursor::Hand)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        for index in 0..self.items.len() {
            let Some(row) = self.row_rect(index) else {
                break;
            };
            if row.top >= self.rect.bottom {
                break;
            }
            let selected = index == self.selected;
            let hovered = self.enabled && self.hover == Some(index);
            if selected {
                ctx.canvas.fill_rect(row, palette.nav_active);
                ctx.canvas.fill_rect(
                    Rect::new(row.left, row.top + 4.0, row.left + 3.0, row.bottom - 4.0),
                    palette.accent,
                );
            } else if hovered {
                ctx.canvas.fill_rect(row, palette.nav_hover);
            }
            let mut text_left = row.left + 14.0;
            if let Some(icon) = self.items[index].icon {
                let icon_rect = Rect::new(
                    row.left + 12.0,
                    row.top,
                    row.left + 34.0,
                    row.bottom,
                );
                let mut buf = [0u8; 4];
                let glyph = icon.encode_utf8(&mut buf);
                let glyph_face = (0xE000..=0xF8FF).contains(&(icon as u32));
                let tint = if !self.enabled {
                    palette.text_disabled
                } else if selected {
                    palette.accent_text
                } else {
                    palette.muted
                };
                if glyph_face {
                    ctx.text(icon_rect, glyph, TextStyleRole::Glyph, TextAlign::Left, tint);
                } else {
                    ctx.text(icon_rect, glyph, TextStyleRole::Body, TextAlign::Left, tint);
                }
                text_left = row.left + 40.0;
            }
            let color = if !self.enabled {
                palette.text_disabled
            } else if selected {
                palette.accent_text
            } else {
                palette.text
            };
            ctx.text(
                Rect::new(text_left, row.top, row.right - 10.0, row.bottom),
                &self.items[index].label,
                TextStyleRole::Body,
                TextAlign::Left,
                color,
            );
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::HoverAt(pos) => {
                self.hover = self.row_at(pos);
                None
            }
            WidgetInput::HoverLeave => {
                self.hover = None;
                None
            }
            WidgetInput::ClickAt(pos) => {
                let index = self.row_at(pos)?;
                self.select_index(index)
            }
            WidgetInput::Activate => self.select_index(self.selected),
            WidgetInput::Key(key) => match key.code {
                KeyCode::Up | KeyCode::Left => self.step(-1),
                KeyCode::Down | KeyCode::Right => self.step(1),
                KeyCode::Home => self.select_index(0),
                KeyCode::End => self.select_index(self.items.len().saturating_sub(1)),
                KeyCode::Enter | KeyCode::Space => self.select_index(self.selected),
                _ => None,
            },
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let children = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                Some(
                    A11yChild::new(index, item.label.clone(), self.row_rect(index)?)
                        .selected(index == self.selected)
                        .enabled(self.enabled),
                )
            })
            .collect();
        let name = self
            .label
            .clone()
            .unwrap_or_else(|| self.id.0.to_string());
        Some(A11yInfo::new(A11yRole::List, name).children(children))
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        self.select_index(index)
    }

    fn set_selected(&mut self, index: usize) {
        NavRail::set_selected(self, index);
    }

    fn tooltip_rect(&self) -> Rect {
        self.row_rect(self.selected).unwrap_or(self.rect)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.row_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        (!self.items.is_empty()).then_some(self.selected)
    }
}

/// A draggable divider between two panes. The widget paints the strip; the
/// application keeps the pane rectangles (D1) and re-lays them out from the
/// positions reported by `on_drag`.
///
/// `on_drag` receives the new divider centre in client DIPs (the press grab
/// offset is preserved), so the app clamps it to its pane minimums and sets
/// the pane/splitter rectangles plus invalidations.
pub struct Splitter<M> {
    id: WidgetId,
    rect: Rect,
    vertical: bool,
    active: bool,
    hover: bool,
    enabled: bool,
    grab: f32,
    label: Option<String>,
    on_drag: Box<dyn Fn(f32) -> M>,
}

impl<M> Splitter<M> {
    /// A vertical divider that drags horizontally (between left/right panes).
    pub fn vertical(id: &'static str, on_drag: impl Fn(f32) -> M + 'static) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 8.0, 200.0),
            vertical: true,
            active: false,
            hover: false,
            enabled: true,
            grab: 0.0,
            label: None,
            on_drag: Box::new(on_drag),
        }
    }

    /// A horizontal divider that drags vertically (between top/bottom panes).
    pub fn horizontal(id: &'static str, on_drag: impl Fn(f32) -> M + 'static) -> Self {
        Self {
            rect: Rect::from_xywh(0.0, 0.0, 200.0, 8.0),
            vertical: false,
            ..Self::vertical(id, on_drag)
        }
    }

    /// Accessible name (defaults to the widget id).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Whether the divider is vertical (drags horizontally).
    pub fn is_vertical(&self) -> bool {
        self.vertical
    }

    fn axis(&self, pos: Point) -> f32 {
        if self.vertical { pos.x } else { pos.y }
    }

    fn center(&self) -> f32 {
        if self.vertical {
            self.rect.center().x
        } else {
            self.rect.center().y
        }
    }
}

impl<M: 'static> Widget<M> for Splitter<M> {
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
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        if self.vertical { 200.0 } else { 8.0 }
    }

    /// Not a Tab stop, but it takes the pointer.
    fn focusable(&self) -> bool {
        false
    }

    fn interactive(&self) -> bool {
        true
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.active = false;
            self.hover = false;
        }
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(if self.vertical {
            Cursor::SizeWE
        } else {
            Cursor::SizeNS
        })
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        let color = if !self.enabled {
            palette.divider
        } else if self.active {
            palette.accent
        } else if self.hover {
            palette.scroll_thumb_hover
        } else {
            palette.divider
        };
        if self.vertical {
            let x = self.rect.center().x;
            ctx.canvas.fill_rect(
                Rect::new(x - 0.5, self.rect.top, x + 0.5, self.rect.bottom),
                color,
            );
            let y = self.rect.center().y;
            for offset in [-8.0, 0.0, 8.0] {
                ctx.canvas.fill_rect(
                    Rect::new(x - 1.5, y + offset - 1.5, x + 1.5, y + offset + 1.5),
                    color,
                );
            }
        } else {
            let y = self.rect.center().y;
            ctx.canvas.fill_rect(
                Rect::new(self.rect.left, y - 0.5, self.rect.right, y + 0.5),
                color,
            );
            let x = self.rect.center().x;
            for offset in [-8.0, 0.0, 8.0] {
                ctx.canvas.fill_rect(
                    Rect::new(x + offset - 1.5, y - 1.5, x + offset + 1.5, y + 1.5),
                    color,
                );
            }
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::HoverEnter => {
                self.hover = true;
                None
            }
            WidgetInput::HoverLeave => {
                if !self.active {
                    self.hover = false;
                }
                None
            }
            WidgetInput::PressAt(pos) | WidgetInput::DragStart(pos) => {
                self.active = true;
                self.hover = true;
                self.grab = self.axis(pos) - self.center();
                None
            }
            WidgetInput::DragTo(pos) => {
                if !self.active {
                    return None;
                }
                Some((self.on_drag)(self.axis(pos) - self.grab))
            }
            WidgetInput::ReleaseAt(_) | WidgetInput::DragEnd(_) => {
                self.active = false;
                self.hover = false;
                None
            }
            WidgetInput::Activate => {
                // Enter enters adjust mode for adjustable widgets; a splitter
                // has no keyboard adjustment, so ignore it.
                None
            }
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let name = self
            .label
            .clone()
            .unwrap_or_else(|| self.id.0.to_string());
        Some(A11yInfo::new(A11yRole::Separator, name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Canvas, TextAlign, TextStyle};
    use crate::color::Color;

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

    fn accordion() -> Accordion<(usize, bool)> {
        let mut accordion = Accordion::new("a", &["播放", "音效"], |index, open| (index, open))
            .body(0, "第一段说明，包含足够长的文本用于换行。")
            .body(1, "第二段说明。")
            .open(0, true);
        accordion.set_rect(Rect::from_xywh(0.0, 0.0, 280.0, 124.0));
        accordion
    }

    /// A canvas that measures like a fixed 8×16 font and records nothing.
    #[derive(Default)]
    struct Blank;

    impl Canvas for Blank {
        fn scale(&self) -> f32 {
            1.0
        }
        fn dpi(&self) -> f32 {
            96.0
        }
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
        fn text(
            &mut self,
            _rect: Rect,
            _text: &str,
            _style: &TextStyle,
            _align: TextAlign,
            _color: Color,
        ) {
        }
        fn measure_text(&mut self, text: &str, _style: &TextStyle) -> crate::geometry::Size {
            crate::geometry::Size::new(text.chars().count() as f32 * 8.0, 16.0)
        }
        fn ellipse(&mut self, _rect: Rect, _color: Color) {}
        fn vertical_gradient(&mut self, _rect: Rect, _top: Color, _bottom: Color) {}
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

    #[test]
    fn clicking_a_header_toggles_it_and_clicking_the_body_does_not() {
        let mut accordion = accordion();
        // The second header starts after the first header and its open body.
        let second_header_y = 34.0 + 56.0 + 10.0;
        assert_eq!(
            accordion.input(WidgetInput::ClickAt(Point::new(20.0, second_header_y))),
            Some((1, true))
        );
        assert!(accordion.is_expanded(0) && accordion.is_expanded(1));
        // A press inside an open body is not a toggle.
        assert_eq!(
            accordion.input(WidgetInput::ClickAt(Point::new(20.0, 50.0))),
            None
        );
        // Collapsing the first section moves the second header up.
        assert_eq!(
            accordion.input(WidgetInput::ClickAt(Point::new(20.0, 10.0))),
            Some((0, false))
        );
    }

    #[test]
    fn single_open_closes_the_previous_section() {
        let mut accordion = accordion().single_open(true);
        assert_eq!(accordion.open_index(), Some(0));
        assert_eq!(accordion.toggle(1), Some((1, true)));
        assert!(!accordion.is_expanded(0));
        assert_eq!(accordion.open_index(), Some(1));
        // Toggling the only open section collapses it.
        assert_eq!(accordion.toggle(1), Some((1, false)));
        assert_eq!(accordion.open_index(), None);
    }

    #[test]
    fn keyboard_moves_the_cursor_and_enter_toggles() {
        let mut accordion = accordion();
        assert!(accordion.adjustable());
        accordion.input(key(KeyCode::Down));
        assert_eq!(accordion.focus_item(), Some(1));
        assert_eq!(accordion.input(key(KeyCode::Space)), Some((1, true)));
        assert!(accordion.is_expanded(1));
        accordion.input(key(KeyCode::Home));
        assert_eq!(accordion.focus_item(), Some(0));
        assert_eq!(accordion.input(key(KeyCode::Enter)), Some((0, false)));
        assert!(!accordion.is_expanded(0));
    }

    #[test]
    fn closed_bodies_take_no_space_and_open_ones_add_their_height() {
        let mut accordion: Accordion<()> =
            Accordion::new("a", &["一", "二", "三"], |_, _| ());
        accordion.set_rect(Rect::from_xywh(0.0, 0.0, 280.0, 200.0));
        assert_eq!(accordion.preferred_height(280.0), 3.0 * 34.0);
        assert_eq!(accordion.header_rect(1).unwrap().top, 34.0);
        accordion.set_expanded(0, true);
        assert_eq!(accordion.preferred_height(280.0), 3.0 * 34.0 + 56.0);
        assert_eq!(accordion.header_rect(1).unwrap().top, 34.0 + 56.0);
        assert_eq!(accordion.header_rect(2).unwrap().top, 34.0 + 56.0 + 34.0);
    }

    #[test]
    fn set_expanded_is_silent_and_push_to_a11y_as_a_selected_list_item() {
        let mut accordion = accordion();
        assert!(accordion.set_expanded(1, true));
        assert!(!accordion.set_expanded(1, true));
        let info = accordion.a11y().unwrap();
        assert_eq!(info.role, A11yRole::List);
        assert_eq!(info.name, "a");
        assert_eq!(info.children.len(), 2);
        assert!(info.children[0].selected);
        assert!(info.children[1].selected);
        assert_eq!(info.children[0].expanded, Some(true));
        assert_eq!(accordion.a11y_select_child(0), Some((0, false)));
        assert!(!accordion.a11y().unwrap().children[0].selected);
    }

    #[test]
    fn a11y_expand_drives_the_same_state_as_the_header_click() {
        let mut accordion: Accordion<(usize, bool)> = Accordion::new(
            "a",
            &["一", "二"],
            |index, open| (index, open),
        )
        .single_open(true)
        .open(0, true);
        accordion.set_rect(Rect::from_xywh(0.0, 0.0, 280.0, 124.0));
        // Expanding a closed section closes the open one and reports that
        // section.
        assert_eq!(accordion.a11y_expand(Some(1), true), Some((1, true)));
        assert!(accordion.is_expanded(1) && !accordion.is_expanded(0));
        assert_eq!(accordion.focus_item(), Some(1));
        // A request for the state the section is already in changes nothing.
        assert_eq!(accordion.a11y_expand(Some(1), true), None);
        assert_eq!(accordion.a11y_expand(None, false), None);
        assert_eq!(accordion.a11y_expand(Some(9), false), None);
        assert_eq!(accordion.a11y_expand(Some(1), false), Some((1, false)));
        assert!(!accordion.is_expanded(1));
        accordion.set_enabled(false);
        assert_eq!(accordion.a11y_expand(Some(0), true), None);
    }

    #[test]
    fn painting_an_open_accordion_does_not_panic() {
        let accordion = accordion();
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
        <Accordion<(usize, bool)> as Widget<(usize, bool)>>::paint(
            &accordion,
            &mut paint,
            &WidgetState {
                enabled: true,
                ..Default::default()
            },
        );
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
                ..Default::default()
            },
        );
    }

    #[test]
    fn breadcrumb_navigates_only_its_trailing_crumbs() {
        let mut crumb: Breadcrumb<usize> =
            Breadcrumb::new("b", &["首页", "曲库", "夜曲"], |index| index).label("位置");
        crumb.set_rect(Rect::from_xywh(0.0, 0.0, 260.0, 24.0));
        paint_blank(&crumb);
        // 8 px per character plus an 18 px separator cell:
        // 首页 0..16, 曲库 34..50, 夜曲 68..84.
        assert_eq!(
            crumb.input(WidgetInput::ClickAt(Point::new(8.0, 10.0))),
            Some(0)
        );
        assert_eq!(
            crumb.input(WidgetInput::ClickAt(Point::new(76.0, 10.0))),
            None,
            "the current crumb is not a navigation target"
        );
        assert_eq!(crumb.cursor_at(Point::new(8.0, 10.0)), Some(Cursor::Hand));
        assert_eq!(crumb.cursor_at(Point::new(76.0, 10.0)), Some(Cursor::Arrow));

        crumb.input(key(KeyCode::Home));
        assert_eq!(crumb.input(WidgetInput::Activate), Some(0));
        crumb.input(key(KeyCode::Right));
        crumb.input(key(KeyCode::End));
        // End parks on the current crumb, which emits nothing.
        assert_eq!(crumb.input(WidgetInput::Activate), None);
    }

    #[test]
    fn breadcrumb_a11y_is_a_list_with_the_current_crumb_selected() {
        let mut crumb: Breadcrumb<usize> =
            Breadcrumb::new("b", &["首页", "曲库", "夜曲"], |index| index).label("位置");
        let info = crumb.a11y().unwrap();
        assert_eq!(info.role, A11yRole::List);
        assert_eq!(info.name, "位置");
        assert_eq!(info.children.len(), 3);
        assert!(!info.children[0].selected);
        assert!(info.children[2].selected);
        assert_eq!(crumb.a11y_select_child(1), Some(1));
        assert_eq!(crumb.focus_item(), Some(1));
        crumb.set_items(vec!["一".to_string()]);
        assert_eq!(crumb.a11y().unwrap().children.len(), 1);
        assert_eq!(crumb.current_index(), Some(0));
    }

    #[test]
    fn nav_rail_selects_rows_by_click_and_arrows() {
        let mut rail: NavRail<usize> =
            NavRail::new("n", &["全部", "最近", "收藏"], 0, |index| index).icon(1, '♪');
        rail.set_rect(Rect::from_xywh(0.0, 0.0, 180.0, 102.0));
        assert_eq!(
            rail.input(WidgetInput::ClickAt(Point::new(20.0, 40.0))),
            Some(1)
        );
        assert_eq!(rail.selected(), 1);
        assert_eq!(
            rail.input(WidgetInput::ClickAt(Point::new(20.0, 40.0))),
            None,
            "selecting the selected row emits nothing"
        );
        assert_eq!(rail.input(key(KeyCode::Down)), Some(2));
        assert_eq!(rail.input(key(KeyCode::Down)), Some(0));
        assert_eq!(rail.input(key(KeyCode::End)), Some(2));
        rail.set_selected(0);
        assert_eq!(rail.selected(), 0);
        assert!(rail.adjustable());
    }

    #[test]
    fn nav_rail_a11y_selects_the_current_row() {
        let mut rail: NavRail<usize> =
            NavRail::new("n", &["全部", "最近", "收藏"], 0, |index| index).label("导航");
        let info = rail.a11y().unwrap();
        assert_eq!(info.role, A11yRole::List);
        assert_eq!(info.name, "导航");
        assert_eq!(info.children.len(), 3);
        assert_eq!(
            info.children.iter().position(|child| child.selected),
            Some(0)
        );
        assert_eq!(rail.a11y_select_child(2), Some(2));
        assert_eq!(rail.selected(), 2);
        assert_eq!(rail.focus_item(), Some(2));
    }

    #[test]
    fn vertical_splitter_reports_the_dragged_centre_with_the_grab_offset() {
        let mut split: Splitter<f32> = Splitter::vertical("s", |position| position).label("拖动");
        split.set_rect(Rect::from_xywh(100.0, 0.0, 8.0, 200.0));
        assert_eq!(split.cursor(), Some(Cursor::SizeWE));
        assert!(!split.is_active());
        split.input(WidgetInput::PressAt(Point::new(106.0, 100.0)));
        assert!(split.is_active());
        // Pressed 2 DIPs right of the centre (104): the reported centre keeps
        // that 2 DIP offset while the pointer moves.
        assert_eq!(
            split.input(WidgetInput::DragTo(Point::new(130.0, 100.0))),
            Some(128.0)
        );
        split.input(WidgetInput::ReleaseAt(Point::new(130.0, 100.0)));
        assert!(!split.is_active());
        assert_eq!(
            split.input(WidgetInput::DragTo(Point::new(200.0, 100.0))),
            None,
            "a drag without a press emits nothing"
        );
        <Splitter<f32> as Widget<f32>>::set_enabled(&mut split, false);
        assert_eq!(
            split.input(WidgetInput::PressAt(Point::new(104.0, 100.0))),
            None,
            "a disabled splitter ignores the pointer"
        );
    }

    #[test]
    fn horizontal_splitter_uses_the_y_axis() {
        let mut split: Splitter<f32> = Splitter::horizontal("s", |position| position);
        split.set_rect(Rect::from_xywh(0.0, 100.0, 200.0, 8.0));
        assert_eq!(split.cursor(), Some(Cursor::SizeNS));
        assert!(!split.is_vertical());
        split.input(WidgetInput::PressAt(Point::new(100.0, 104.0)));
        assert_eq!(
            split.input(WidgetInput::DragTo(Point::new(100.0, 140.0))),
            Some(140.0)
        );
    }

    #[test]
    fn splitter_a11y_is_a_named_separator() {
        let split: Splitter<f32> =
            Splitter::vertical("s", |position| position).label("拖动调整面板");
        assert!(!split.focusable());
        assert!(split.interactive());
        let info = split.a11y().unwrap();
        assert_eq!(info.role, A11yRole::Separator);
        assert_eq!(info.name, "拖动调整面板");
    }

    #[test]
    fn the_step_child_list_sets_the_breadcrumb_and_rail() {
        let mut crumb: Breadcrumb<()> = Breadcrumb::new("crumb", &["根"], |_| ());
        let mut a = SkinElement::new("step", 2, 3);
        a.push_attr("label", "音乐");
        let mut b = SkinElement::new("step", 3, 3);
        b.push_attr("label", "专辑");
        let steps = [a, b];
        assert!(<Breadcrumb<()> as Widget<()>>::validate_skin_children(
            &crumb, &steps
        ));
        <Breadcrumb<()> as Widget<()>>::apply_skin_children(&mut crumb, &steps);
        assert_eq!(
            crumb.items().iter().map(String::as_str).collect::<Vec<_>>(),
            ["音乐", "专辑"]
        );
        assert_eq!(crumb.current_index(), Some(1));

        let mut rail: NavRail<()> = NavRail::new("rail", &["一"], 0, |_| ());
        let mut home = SkinElement::new("step", 2, 3);
        home.push_attr("label", "首页");
        home.push_attr("icon", "⌂");
        let mut list = SkinElement::new("step", 3, 3);
        list.push_attr("label", "曲库");
        let steps = [home, list];
        assert!(<NavRail<()> as Widget<()>>::validate_skin_children(
            &rail, &steps
        ));
        <NavRail<()> as Widget<()>>::apply_skin_children(&mut rail, &steps);
        assert_eq!(rail.items().len(), 2);
        assert_eq!(rail.items()[0].label(), "首页");
        assert_eq!(rail.items()[0].icon_glyph(), Some('⌂'));
        assert_eq!(rail.items()[1].icon_glyph(), None);

        // A breadcrumb step has no icon; a rail icon is exactly one character.
        let mut icon_on_crumb = SkinElement::new("step", 4, 3);
        icon_on_crumb.push_attr("label", "x");
        icon_on_crumb.push_attr("icon", "⌂");
        assert!(!<Breadcrumb<()> as Widget<()>>::validate_skin_children(
            &crumb,
            &[icon_on_crumb]
        ));
        let mut long_icon = SkinElement::new("step", 5, 3);
        long_icon.push_attr("label", "x");
        long_icon.push_attr("icon", "ab");
        assert!(!<NavRail<()> as Widget<()>>::validate_skin_children(
            &rail,
            &[long_icon]
        ));
    }
}
