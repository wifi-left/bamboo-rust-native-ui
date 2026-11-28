// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Application chrome: menu buttons, a menu bar, a toolbar and a status bar.
//!
//! Menus are **native**: a widget only asks for one through
//! [`crate::Widget::take_native_menu`], and the host shows the same deferred
//! `TrackPopupMenu` popup a context menu uses (never inside a `Host`
//! callback). That keeps menus instant, keyboard/model-correct and accessible
//! without the framework painting a second menu system. The cost is that the
//! popup follows the system menu colors instead of the page palette — the
//! documented trade-off of the native context-menu host.
//!
//! Commands are application ids, replayed through
//! [`crate::Host::menu_command`] with the widget that opened the menu.
//!
//! The status bar and the toolbar's cells are painted by the framework: they
//! are page content, not system chrome.

use std::cell::RefCell;

use crate::a11y::{A11yChild, A11yInfo, A11yRole};
use crate::backend::TextAlign;
use crate::event::{Cursor, KeyCode};
use crate::geometry::{Point, Rect};
use crate::menu::{Menu, MenuEntry};
use crate::mnemonic::MnemonicLabel;
use crate::skin::SkinElement;
use crate::theme::Palette;
use crate::widget::{
    NativeMenuRequest, TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState,
};
use super::observers::{Observers, observer_builders, observer_methods};

const DEFAULT_HEIGHT: f32 = 30.0;
const ARROW_ZONE: f32 = 26.0;
const CHEVRON_DOWN: char = '\u{E70D}';
const OVERFLOW_GLYPH: char = '\u{E712}';
/// Published menu entries start at this index so they cannot collide with a
/// chrome widget's own children (top-level menu items, toolbar buttons). The
/// offset is an accessibility detail only; `a11y_select_child` maps it back to
/// the menu entry index.
const POPUP_CHILD_BASE: usize = 1000;

fn chrome_color(palette: &Palette, enabled: bool, emphasized: bool) -> crate::Color {
    if !enabled {
        palette.text_disabled
    } else if emphasized {
        palette.accent_text
    } else {
        palette.text
    }
}

fn draw_glyph(ctx: &mut WidgetPaint<'_>, rect: Rect, glyph: char, color: crate::Color) {
    let mut buf = [0u8; 4];
    let text = glyph.encode_utf8(&mut buf);
    // A private-use codepoint is an MDL2 icon; anything else is UI text.
    let role = if (0xE000..=0xF8FF).contains(&(glyph as u32)) {
        TextStyleRole::Glyph
    } else {
        TextStyleRole::Body
    };
    ctx.text(rect, text, role, TextAlign::Center, color);
}

/// Paint one left-aligned label with its access key underlined (when the
/// application's Alt mode is on). The label was parsed at construction, so
/// this allocates nothing: the prefix is a borrowed slice and measuring does
/// not build a string.
fn paint_mnemonic(
    ctx: &mut WidgetPaint<'_>,
    rect: Rect,
    label: &MnemonicLabel,
    role: TextStyleRole,
    color: crate::Color,
    underline: bool,
) {
    ctx.text_overflow(
        rect,
        label.text(),
        role,
        TextAlign::Left,
        color,
        crate::backend::TextOverflow::Ellipsis,
    );
    if !underline {
        return;
    }
    let (Some(key_byte), key_len) = (label.key_byte(), label.key_len()) else {
        return;
    };
    let text = label.text();
    let Some(prefix) = text.get(..key_byte) else {
        return;
    };
    let Some(key) = text.get(key_byte..key_byte + key_len) else {
        return;
    };
    let style = match role {
        TextStyleRole::Body => ctx.style(ctx.painter.body()),
        TextStyleRole::Small => ctx.style(ctx.painter.small()),
        TextStyleRole::Title => ctx.style(ctx.painter.title()),
        TextStyleRole::Brand => ctx.style(ctx.painter.brand()),
        TextStyleRole::Glyph => ctx.style(ctx.painter.glyph()),
    };
    let prefix_width = ctx.canvas.measure_text(prefix, &style).width;
    let key_width = ctx.canvas.measure_text(key, &style).width;
    let left = rect.left + prefix_width + 1.0;
    let right = (rect.left + prefix_width + key_width - 1.0).min(rect.right);
    if right <= left {
        return;
    }
    let y = rect.bottom - 4.0;
    ctx.canvas
        .fill_rect(Rect::new(left, y, right, y + 1.0), color);
}

/// Publish a menu's entries for accessibility: separators are skipped and the
/// rest keep their menu entry index (the caller adds the popup offset).
fn menu_children(menu: &Menu) -> Vec<A11yChild> {
    menu.entries()
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| match entry {
            MenuEntry::Item(item) => Some(
                A11yChild::new(index, item.label.clone(), Rect::default())
                    .checked(item.checked)
                    .enabled(!item.disabled)
                    .role(A11yRole::MenuItem),
            ),
            MenuEntry::Submenu(submenu) => Some(
                A11yChild::new(index, submenu.label.clone(), Rect::default())
                    .enabled(!submenu.disabled)
                    .role(A11yRole::MenuItem),
            ),
            MenuEntry::Separator => None,
        })
        .collect()
}

/// The command a menu entry would run, if it is an enabled leaf item.
fn entry_command(menu: &Menu, entry: usize) -> Option<u32> {
    match menu.entries().get(entry) {
        Some(MenuEntry::Item(item)) if !item.disabled => Some(item.command),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// MenuButton
// ---------------------------------------------------------------------------

/// A button that opens a declarative [`Menu`] as a native popup.
///
/// The label may carry an access key (`"文件(&F)"`). Clicking, pressing
/// Enter/Space or `Alt+key` asks the host for the menu below the button; the
/// selected command arrives through [`crate::Host::menu_command`] with this
/// widget's id.
pub struct MenuButton<M> {
    id: WidgetId,
    rect: Rect,
    label: MnemonicLabel,
    glyph: Option<char>,
    menu: Menu,
    enabled: bool,
    underlines: bool,
    pending: Option<NativeMenuRequest>,
    on_command: Box<dyn Fn(u32) -> M>,
    observers: Observers<M>,
}

impl<M> MenuButton<M> {
    pub fn new(
        id: &'static str,
        label: impl AsRef<str>,
        menu: Menu,
        on_command: impl Fn(u32) -> M + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 140.0, DEFAULT_HEIGHT),
            label: MnemonicLabel::parse(label.as_ref()),
            glyph: None,
            menu,
            enabled: true,
            underlines: true,
            pending: None,
            on_command: Box::new(on_command),
            observers: Observers::new(),
        }
    }

    /// Leading icon glyph (an MDL2 codepoint or a text character).
    pub fn glyph(mut self, glyph: char) -> Self {
        self.glyph = Some(glyph);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    /// Replace the menu model.
    pub fn set_menu(&mut self, menu: Menu) {
        self.menu = menu;
        self.pending = None;
    }

    pub fn set_label(&mut self, label: &str) {
        self.label = MnemonicLabel::parse(label);
    }

    fn open(&mut self) {
        if self.enabled && !self.menu.is_empty() {
            self.pending = Some(NativeMenuRequest {
                menu: self.menu.clone(),
                anchor: Point::new(self.rect.left, self.rect.bottom),
                avoid: Some(self.rect),
            });
        }
    }

    /// Entries published for accessibility. A closed menu has no on-screen
    /// rows, so they report the button's rectangle.
    fn menu_children(&self) -> Vec<A11yChild> {
        menu_children(&self.menu)
            .into_iter()
            .map(|mut child| {
                child.rect = self.rect;
                child
            })
            .collect()
    }
}

impl<M: 'static> Widget<M> for MenuButton<M> {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    observer_methods!();

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        DEFAULT_HEIGHT
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.pending = None;
        }
    }

    fn set_mnemonic_underlines(&mut self, show: bool) -> bool {
        if self.underlines == show {
            return false;
        }
        self.underlines = show;
        true
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Hand)
    }

    fn mnemonic(&self) -> Option<char> {
        self.label.key()
    }

    fn mnemonic_key(&mut self, ch: char) -> bool {
        if !self.enabled || !self.label.matches(ch) {
            return false;
        }
        self.open();
        true
    }

    fn take_native_menu(&mut self) -> Option<NativeMenuRequest> {
        self.pending.take()
    }

    fn native_menu(&self) -> Option<NativeMenuRequest> {
        if !self.enabled || self.menu.is_empty() {
            return None;
        }
        Some(NativeMenuRequest {
            menu: self.menu.clone(),
            anchor: Point::new(self.rect.left, self.rect.bottom),
            avoid: Some(self.rect),
        })
    }

    fn set_native_menu(&mut self, menu: Menu) -> bool {
        self.set_menu(menu);
        true
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        let background = if !state.enabled {
            palette.button_disabled
        } else if state.pressed {
            palette.button_pressed
        } else if state.hovered {
            palette.button_hover
        } else {
            palette.button_bg
        };
        ctx.canvas.fill_rect(self.rect, background);
        let color = chrome_color(&palette, state.enabled, false);
        let mut left = self.rect.left + 10.0;
        if let Some(glyph) = self.glyph {
            draw_glyph(
                ctx,
                Rect::new(left, self.rect.top, left + 20.0, self.rect.bottom),
                glyph,
                color,
            );
            left += 24.0;
        }
        paint_mnemonic(
            ctx,
            Rect::new(
                left,
                self.rect.top,
                self.rect.right - 20.0,
                self.rect.bottom,
            ),
            &self.label,
            TextStyleRole::Body,
            color,
            self.underlines,
        );
        draw_glyph(
            ctx,
            Rect::new(
                self.rect.right - 18.0,
                self.rect.top,
                self.rect.right - 4.0,
                self.rect.bottom,
            ),
            CHEVRON_DOWN,
            if state.enabled {
                palette.muted
            } else {
                palette.text_disabled
            },
        );
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::ClickAt(_) | WidgetInput::Activate => {
                self.open();
                None
            }
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(A11yInfo::new(A11yRole::Menu, self.label.text()).children(self.menu_children()))
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        let command = entry_command(&self.menu, index)?;
        Some((self.on_command)(command))
    }

    fn a11y_expand(&mut self, _child: Option<usize>, expanded: bool) -> Option<M> {
        if expanded {
            self.open();
        }
        None
    }

    fn focus_ring_rect(&self) -> Rect {
        self.rect.inset(1.0, 1.0)
    }
}

observer_builders!(MenuButton);

// ---------------------------------------------------------------------------
// SplitButton
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitZone {
    Body,
    Arrow,
}

/// A split button: the body runs the primary action, the arrow opens a native
/// menu with the alternatives.
pub struct SplitButton<M> {
    id: WidgetId,
    rect: Rect,
    label: MnemonicLabel,
    glyph: Option<char>,
    menu: Menu,
    enabled: bool,
    underlines: bool,
    hover_zone: Option<SplitZone>,
    pending: Option<NativeMenuRequest>,
    on_command: Box<dyn Fn(u32) -> M>,
    on_primary: Box<dyn Fn() -> M>,
    observers: Observers<M>,
}

impl<M> SplitButton<M> {
    pub fn new(
        id: &'static str,
        label: impl AsRef<str>,
        menu: Menu,
        on_primary: impl Fn() -> M + 'static,
        on_command: impl Fn(u32) -> M + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 170.0, DEFAULT_HEIGHT),
            label: MnemonicLabel::parse(label.as_ref()),
            glyph: None,
            menu,
            enabled: true,
            underlines: true,
            hover_zone: None,
            pending: None,
            on_command: Box::new(on_command),
            on_primary: Box::new(on_primary),
            observers: Observers::new(),
        }
    }

    pub fn glyph(mut self, glyph: char) -> Self {
        self.glyph = Some(glyph);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn set_menu(&mut self, menu: Menu) {
        self.menu = menu;
        self.pending = None;
    }

    fn arrow_left(&self) -> f32 {
        self.rect.right - ARROW_ZONE
    }

    fn zone_at(&self, pos: Point) -> Option<SplitZone> {
        if !self.rect.contains_point(pos) {
            return None;
        }
        Some(if pos.x >= self.arrow_left() {
            SplitZone::Arrow
        } else {
            SplitZone::Body
        })
    }

    fn open(&mut self) {
        if self.enabled && !self.menu.is_empty() {
            self.pending = Some(NativeMenuRequest {
                menu: self.menu.clone(),
                anchor: Point::new(self.arrow_left(), self.rect.bottom),
                avoid: Some(self.rect),
            });
        }
    }

    /// Entries published for accessibility (no on-screen rows while closed).
    fn menu_children(&self) -> Vec<A11yChild> {
        menu_children(&self.menu)
            .into_iter()
            .map(|mut child| {
                child.rect = self.rect;
                child
            })
            .collect()
    }
}

impl<M: 'static> Widget<M> for SplitButton<M> {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    observer_methods!();

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        DEFAULT_HEIGHT
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover_zone = None;
            self.pending = None;
        }
    }

    fn set_mnemonic_underlines(&mut self, show: bool) -> bool {
        if self.underlines == show {
            return false;
        }
        self.underlines = show;
        true
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Hand)
    }

    fn mnemonic(&self) -> Option<char> {
        self.label.key()
    }

    fn mnemonic_key(&mut self, ch: char) -> bool {
        if !self.enabled || !self.label.matches(ch) {
            return false;
        }
        self.open();
        true
    }

    fn take_native_menu(&mut self) -> Option<NativeMenuRequest> {
        self.pending.take()
    }

    fn native_menu(&self) -> Option<NativeMenuRequest> {
        if !self.enabled || self.menu.is_empty() {
            return None;
        }
        Some(NativeMenuRequest {
            menu: self.menu.clone(),
            anchor: Point::new(self.arrow_left(), self.rect.bottom),
            avoid: Some(self.rect),
        })
    }

    fn set_native_menu(&mut self, menu: Menu) -> bool {
        self.set_menu(menu);
        true
    }

    fn repaint_on_move_at(&self, pos: Point) -> bool {
        self.zone_at(pos) != self.hover_zone
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        let arrow_hovered = state.hovered && self.hover_zone == Some(SplitZone::Arrow);
        let body_active =
            state.pressed || (state.hovered && self.hover_zone == Some(SplitZone::Body));
        let base = if !state.enabled {
            palette.button_disabled
        } else if body_active {
            palette.button_pressed
        } else if state.hovered {
            palette.button_hover
        } else {
            palette.button_bg
        };
        ctx.canvas.fill_rect(self.rect, base);
        if arrow_hovered && state.enabled {
            ctx.canvas.fill_rect(
                Rect::new(
                    self.arrow_left(),
                    self.rect.top,
                    self.rect.right,
                    self.rect.bottom,
                ),
                palette.button_hover,
            );
        }
        ctx.canvas.fill_rect(
            Rect::new(
                self.arrow_left(),
                self.rect.top + 4.0,
                self.arrow_left() + 1.0,
                self.rect.bottom - 4.0,
            ),
            palette.explicit_border,
        );
        let color = chrome_color(&palette, state.enabled, false);
        let mut left = self.rect.left + 10.0;
        if let Some(glyph) = self.glyph {
            draw_glyph(
                ctx,
                Rect::new(left, self.rect.top, left + 20.0, self.rect.bottom),
                glyph,
                color,
            );
            left += 24.0;
        }
        paint_mnemonic(
            ctx,
            Rect::new(
                left,
                self.rect.top,
                self.arrow_left() - 8.0,
                self.rect.bottom,
            ),
            &self.label,
            TextStyleRole::Body,
            color,
            self.underlines,
        );
        draw_glyph(
            ctx,
            Rect::new(
                self.arrow_left(),
                self.rect.top,
                self.rect.right,
                self.rect.bottom,
            ),
            CHEVRON_DOWN,
            if state.enabled {
                palette.muted
            } else {
                palette.text_disabled
            },
        );
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::HoverAt(pos) => {
                self.hover_zone = self.zone_at(pos);
                None
            }
            WidgetInput::HoverLeave => {
                self.hover_zone = None;
                None
            }
            WidgetInput::ClickAt(pos) => match self.zone_at(pos)? {
                SplitZone::Arrow => {
                    self.open();
                    None
                }
                SplitZone::Body => Some((self.on_primary)()),
            },
            WidgetInput::Activate => Some((self.on_primary)()),
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Menu, self.label.text())
                .description("主操作与菜单")
                .children(self.menu_children()),
        )
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        let command = entry_command(&self.menu, index)?;
        Some((self.on_command)(command))
    }

    fn a11y_expand(&mut self, _child: Option<usize>, expanded: bool) -> Option<M> {
        if expanded {
            self.open();
        }
        None
    }

    fn focus_ring_rect(&self) -> Rect {
        self.rect.inset(1.0, 1.0)
    }
}

observer_builders!(SplitButton);

// ---------------------------------------------------------------------------
// MenuBar
// ---------------------------------------------------------------------------

struct BarItem {
    label: MnemonicLabel,
    menu: Menu,
}

#[derive(Default)]
struct BarLayout {
    valid: bool,
    revision: u64,
    width: f32,
    font_size: f32,
    cells: Vec<Rect>,
}

const BAR_ITEM_PADDING: f32 = 12.0;
const BAR_MIN_CELL: f32 = 42.0;

/// A painted top-level menu bar (File/Edit/View…).
///
/// Clicking an item (or Enter / `Alt+key`) asks the host for that item's native
/// menu below its cell. A focused closed bar keeps Left/Right for its
/// top-level cursor only while there is another item in that direction; at the
/// ends (and for Up/Down) the key moves focus instead of being swallowed.
pub struct MenuBar<M> {
    id: WidgetId,
    rect: Rect,
    items: Vec<BarItem>,
    enabled: bool,
    underlines: bool,
    hover: Option<usize>,
    cursor: usize,
    revision: u64,
    layout: RefCell<BarLayout>,
    pending: Option<NativeMenuRequest>,
    on_command: Box<dyn Fn(u32) -> M>,
    observers: Observers<M>,
}

impl<M> MenuBar<M> {
    pub fn new(id: &'static str, on_command: impl Fn(u32) -> M + 'static) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 420.0, 26.0),
            items: Vec::new(),
            enabled: true,
            underlines: true,
            hover: None,
            cursor: 0,
            revision: 0,
            layout: RefCell::new(BarLayout::default()),
            pending: None,
            on_command: Box::new(on_command),
            observers: Observers::new(),
        }
    }

    /// Append a top-level menu. The label may carry an access key (`"&文件"`).
    pub fn item(mut self, label: impl AsRef<str>, menu: Menu) -> Self {
        self.items.push(BarItem {
            label: MnemonicLabel::parse(label.as_ref()),
            menu,
        });
        self.revision = self.revision.wrapping_add(1);
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

    /// The keyboard/selection cursor over the top-level items.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Replace every top-level menu.
    pub fn set_items(&mut self, items: Vec<(String, Menu)>) {
        self.items = items
            .into_iter()
            .map(|(label, menu)| BarItem {
                label: MnemonicLabel::parse(&label),
                menu,
            })
            .collect();
        self.cursor = self.cursor.min(self.items.len().saturating_sub(1));
        self.hover = None;
        self.pending = None;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Replace one top-level cell's menu without a rebuild (a runtime command
    /// set); the cell keeps its label and the bar keeps its cursor.
    pub fn set_item_menu(&mut self, index: usize, menu: Menu) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        item.menu = menu;
        self.pending = None;
        self.revision = self.revision.wrapping_add(1);
        true
    }

    /// Insert a top-level cell at `index` (clamped to the end).
    pub fn insert_item(&mut self, index: usize, label: &str, menu: Menu) -> &mut Self {
        let index = index.min(self.items.len());
        self.items.insert(
            index,
            BarItem {
                label: MnemonicLabel::parse(label),
                menu,
            },
        );
        self.hover = None;
        self.pending = None;
        self.revision = self.revision.wrapping_add(1);
        self
    }

    /// Remove a top-level cell; `None` when the index is out of range.
    pub fn remove_item(&mut self, index: usize) -> Option<(String, Menu)> {
        if index >= self.items.len() {
            return None;
        }
        let item = self.items.remove(index);
        self.cursor = self.cursor.min(self.items.len().saturating_sub(1));
        self.hover = None;
        self.pending = None;
        self.revision = self.revision.wrapping_add(1);
        Some((item.label.text().to_string(), item.menu))
    }

    pub fn set_cursor(&mut self, index: usize) {
        if index < self.items.len() {
            self.cursor = index;
        }
    }

    fn ensure_layout(&self, ctx: &mut WidgetPaint<'_>) {
        // The labels paint through `paint_mnemonic`, which uses the body role;
        // measuring with the same style keeps a cell wide enough for its text.
        let style = ctx.style(ctx.painter.body());
        let mut cache = self.layout.borrow_mut();
        if cache.valid
            && cache.revision == self.revision
            && cache.width == self.rect.width()
            && cache.font_size == style.size
        {
            return;
        }
        let mut cells = Vec::with_capacity(self.items.len());
        let mut x = self.rect.left;
        for item in &self.items {
            let width = ctx
                .canvas
                .measure_text(item.label.text(), &style)
                .width
                .max(10.0)
                + BAR_ITEM_PADDING * 2.0;
            let width = width.max(BAR_MIN_CELL);
            let cell = Rect::new(
                x,
                self.rect.top,
                (x + width).min(self.rect.right),
                self.rect.bottom,
            );
            cells.push(cell);
            x += width;
        }
        cache.cells = cells;
        cache.valid = true;
        cache.revision = self.revision;
        cache.width = self.rect.width();
        cache.font_size = style.size;
    }

    /// Cell rectangle from the paint cache, or a heuristic before the first
    /// paint (interaction always follows a paint in the normal flow).
    fn cell_rect(&self, index: usize) -> Rect {
        let cache = self.layout.borrow();
        if cache.valid && cache.revision == self.revision {
            if let Some(rect) = cache.cells.get(index) {
                return *rect;
            }
        }
        let width = 90.0;
        Rect::new(
            self.rect.left + index as f32 * width,
            self.rect.top,
            self.rect.left + (index + 1) as f32 * width,
            self.rect.bottom,
        )
    }

    fn index_at(&self, pos: Point) -> Option<usize> {
        if !self.rect.contains_point(pos) {
            return None;
        }
        let cache = self.layout.borrow();
        if cache.valid && cache.revision == self.revision {
            return cache.cells.iter().position(|cell| cell.contains_point(pos));
        }
        let index = ((pos.x - self.rect.left) / 90.0).floor();
        (index >= 0.0 && (index as usize) < self.items.len()).then_some(index as usize)
    }

    fn open_item(&mut self, index: usize) {
        if index >= self.items.len() {
            return;
        }
        let anchor = self.cell_rect(index);
        self.cursor = index;
        self.pending = Some(NativeMenuRequest {
            menu: self.items[index].menu.clone(),
            anchor: Point::new(anchor.left, anchor.bottom),
            avoid: Some(anchor),
        });
    }

    fn step_cursor(&mut self, delta: i32) {
        if self.items.is_empty() {
            return;
        }
        let len = self.items.len() as i32;
        self.cursor = (self.cursor as i32 + delta).rem_euclid(len) as usize;
    }
}

impl<M: 'static> Widget<M> for MenuBar<M> {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    observer_methods!();

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        if rect != self.rect {
            self.rect = rect;
            self.layout.borrow_mut().valid = false;
            self.pending = None;
        }
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        26.0
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover = None;
            self.pending = None;
        }
    }

    fn set_mnemonic_underlines(&mut self, show: bool) -> bool {
        if self.underlines == show {
            return false;
        }
        self.underlines = show;
        true
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Hand)
    }

    fn mnemonic_key(&mut self, ch: char) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(index) = self.items.iter().position(|item| item.label.matches(ch)) else {
            return false;
        };
        self.open_item(index);
        true
    }

    fn take_native_menu(&mut self) -> Option<NativeMenuRequest> {
        self.pending.take()
    }

    /// The cursor cell's menu: what Enter / `Alt+key` would open, without
    /// opening it.
    fn native_menu(&self) -> Option<NativeMenuRequest> {
        if !self.enabled {
            return None;
        }
        let menu = self.items.get(self.cursor)?.menu.clone();
        if menu.is_empty() {
            return None;
        }
        let cell = self.cell_rect(self.cursor);
        Some(NativeMenuRequest {
            menu,
            anchor: Point::new(cell.left, cell.bottom),
            avoid: Some(cell),
        })
    }

    fn repaint_on_move_at(&self, pos: Point) -> bool {
        self.index_at(pos) != self.hover
    }

    /// A focused closed bar keeps Left/Right for its top-level cursor only
    /// while there is another item in that direction; at the ends (and for
    /// Up/Down) the key moves focus instead of being swallowed.
    fn owns_navigation_key(&self, key: KeyCode) -> bool {
        match key {
            KeyCode::Left => self.cursor > 0,
            KeyCode::Right => self.cursor + 1 < self.items.len(),
            _ => false,
        }
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        self.ensure_layout(ctx);
        ctx.canvas.fill_rect(self.rect, palette.panel);
        ctx.canvas.fill_rect(
            Rect::new(
                self.rect.left,
                self.rect.bottom - 1.0,
                self.rect.right,
                self.rect.bottom,
            ),
            palette.divider,
        );
        let cache = self.layout.borrow();
        for (index, item) in self.items.iter().enumerate() {
            let Some(&cell) = cache.cells.get(index) else {
                break;
            };
            let hovered = self.enabled && self.hover == Some(index);
            let focused = state.focused && self.cursor == index;
            if hovered || focused {
                ctx.canvas.fill_rect(cell, palette.nav_hover);
            }
            let color = chrome_color(&palette, self.enabled, focused);
            let label_rect = Rect::new(
                cell.left + BAR_ITEM_PADDING,
                cell.top,
                cell.right - BAR_ITEM_PADDING,
                cell.bottom,
            );
            paint_mnemonic(
                ctx,
                label_rect,
                &item.label,
                TextStyleRole::Body,
                color,
                self.underlines,
            );
        }
        drop(cache);
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
                self.cursor = index;
                self.open_item(index);
                None
            }
            WidgetInput::Activate => {
                self.open_item(self.cursor);
                None
            }
            WidgetInput::Key(key) => {
                match key.code {
                    KeyCode::Left => self.step_cursor(-1),
                    KeyCode::Right => self.step_cursor(1),
                    KeyCode::Home => self.cursor = 0,
                    KeyCode::End => {
                        self.cursor = self.items.len().saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Up | KeyCode::Enter | KeyCode::Space => {
                        self.open_item(self.cursor);
                    }
                    _ => {}
                }
                None
            }
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let mut children: Vec<A11yChild> = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                A11yChild::new(index, item.label.text(), self.cell_rect(index))
                    .selected(self.cursor == index)
                    .enabled(self.enabled)
            })
            .collect();
        // The cursor item's commands are published after the top-level items,
        // so a screen reader can reach them without opening the native popup.
        if let Some(item) = self.items.get(self.cursor) {
            let cell = self.cell_rect(self.cursor);
            for mut child in menu_children(&item.menu) {
                child.index += POPUP_CHILD_BASE;
                child.rect = cell;
                children.push(child);
            }
        }
        Some(
            A11yInfo::new(
                A11yRole::MenuBar,
                self.items
                    .iter()
                    .map(|item| item.label.text())
                    .collect::<Vec<_>>()
                    .join(" / "),
            )
            .children(children),
        )
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        if let Some(entry) = index.checked_sub(POPUP_CHILD_BASE) {
            let menu = &self.items.get(self.cursor)?.menu;
            let command = entry_command(menu, entry)?;
            return Some((self.on_command)(command));
        }
        self.open_item(index);
        None
    }

    fn a11y_expand(&mut self, _child: Option<usize>, expanded: bool) -> Option<M> {
        if expanded {
            self.open_item(self.cursor);
        }
        None
    }

    fn focus_ring_rect(&self) -> Rect {
        self.rect.inset(1.0, 1.0)
    }

    fn tooltip_rect(&self) -> Rect {
        self.cell_rect(self.cursor)
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.index_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        (!self.items.is_empty()).then(|| self.cursor.min(self.items.len() - 1))
    }
}

observer_builders!(MenuBar);

// ---------------------------------------------------------------------------
// Toolbar
// ---------------------------------------------------------------------------

/// One toolbar entry: a command button or a separator between groups.
#[derive(Clone, Debug)]
pub struct ToolbarItem {
    label: MnemonicLabel,
    glyph: Option<char>,
    command: u32,
    enabled: bool,
    checked: bool,
    icon_only: bool,
    separator: bool,
    /// Accessible/tooltip name for a cell whose painted label is empty
    /// (an icon-only button).
    name: String,
}

impl ToolbarItem {
    pub fn new(command: u32, label: impl AsRef<str>) -> Self {
        Self {
            label: MnemonicLabel::parse(label.as_ref()),
            glyph: None,
            command,
            enabled: true,
            checked: false,
            icon_only: false,
            separator: false,
            name: String::new(),
        }
    }

    /// A group separator: a 1 DIP rule that owns no command, is skipped by the
    /// keyboard cursor, takes no hover and never appears in the overflow menu.
    pub fn separator() -> Self {
        Self {
            label: MnemonicLabel::default(),
            glyph: None,
            command: 0,
            enabled: false,
            checked: false,
            icon_only: false,
            separator: true,
            name: String::new(),
        }
    }

    /// Paint only the glyph: the cell becomes square on a horizontal bar. Give
    /// it a name with [`ToolbarItem::name`] so screen readers and tooltips can
    /// identify it.
    pub fn icon_only(mut self) -> Self {
        self.icon_only = true;
        self
    }

    /// Accessible/tooltip name (defaults to the painted label).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Leading icon glyph (an MDL2 codepoint or a text character).
    pub fn glyph(mut self, glyph: char) -> Self {
        self.glyph = Some(glyph);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    /// A toggled item paints accent-tinted and reports a checkmark in the
    /// overflow menu.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn command(&self) -> u32 {
        self.command
    }

    pub fn label(&self) -> &str {
        self.label.text()
    }

    /// Name used by accessibility, tooltips and the overflow menu.
    pub fn display_name(&self) -> &str {
        if !self.name.is_empty() {
            &self.name
        } else {
            self.label.text()
        }
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_separator(&self) -> bool {
        self.separator
    }

    /// Whether the cell paints only a glyph (explicit, or a glyph with no
    /// label).
    pub fn is_icon_only(&self) -> bool {
        !self.separator && (self.icon_only || (self.label.is_empty() && self.glyph.is_some()))
    }
}

const TOOL_CELL_PADDING: f32 = 10.0;
const TOOL_MIN_CELL: f32 = 40.0;
const OVERFLOW_SIZE: f32 = 32.0;
/// Width/height of a group separator on a horizontal/vertical bar.
const TOOL_SEPARATOR: f32 = 9.0;
/// Default cell width in DIPs. Toolbars use fixed cells (icons + labels) so
/// the overflow split is pure geometry: the accessibility tree, hit-testing
/// and a UIA action see the same layout before the widget is ever painted.
/// Icon-only cells use the bar's thickness instead, so they stay square.
const TOOL_CELL_WIDTH: f32 = 104.0;

/// A strip of command buttons with an overflow menu.
///
/// Items that do not fit move into a chevron menu instead of being clipped.
/// Cells have a fixed width (configurable through [`Toolbar::item_width`]) so
/// the overflow boundary needs no text measurement and cannot disagree with
/// the painted layout. Separators split the strip into visual groups and take
/// no focus, hover or menu slot; icon-only cells are square. A horizontal bar
/// uses Left/Right for its cursor and a vertical bar Up/Down; Enter/Space runs
/// the current item (or opens the overflow list). The toolbar stays one Tab
/// stop and publishes `ToolBar` with one child per command.
pub struct Toolbar<M> {
    id: WidgetId,
    rect: Rect,
    items: Vec<ToolbarItem>,
    enabled: bool,
    underlines: bool,
    hover: Option<usize>,
    /// Item index, or `items.len()` for the overflow button.
    cursor: usize,
    item_width: f32,
    item_height: f32,
    vertical: bool,
    pending: Option<NativeMenuRequest>,
    on_command: Box<dyn Fn(u32) -> M>,
    observers: Observers<M>,
}

impl<M> Toolbar<M> {
    pub fn new(
        id: &'static str,
        items: Vec<ToolbarItem>,
        on_command: impl Fn(u32) -> M + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 420.0, 32.0),
            items,
            enabled: true,
            underlines: true,
            hover: None,
            cursor: 0,
            item_width: TOOL_CELL_WIDTH,
            item_height: 32.0,
            vertical: false,
            pending: None,
            on_command: Box::new(on_command),
            observers: Observers::new(),
        }
    }

    /// Stack the cells vertically (an app toolbar or a task pane). The
    /// overflow button moves to the bottom and Up/Down drive the cursor.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    /// Width of one command cell on a horizontal bar in DIPs (clamped so the
    /// label keeps room).
    pub fn item_width(mut self, width: f32) -> Self {
        self.item_width = width.max(TOOL_MIN_CELL);
        self
    }

    /// Height of one command cell on a vertical bar in DIPs.
    pub fn item_height(mut self, height: f32) -> Self {
        self.item_height = height.max(24.0);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn is_vertical(&self) -> bool {
        self.vertical
    }

    pub fn mnemonic_underlines(&self) -> bool {
        self.underlines
    }

    pub fn items(&self) -> &[ToolbarItem] {
        &self.items
    }

    pub fn set_items(&mut self, items: Vec<ToolbarItem>) {
        self.items = items;
        self.cursor = 0;
        self.normalize_cursor();
        self.hover = None;
        self.pending = None;
    }

    /// Insert one item at `index` (clamped to the end), without a rebuild.
    pub fn insert_item(&mut self, index: usize, item: ToolbarItem) -> &mut Self {
        let index = index.min(self.items.len());
        self.items.insert(index, item);
        self.normalize_cursor();
        self.hover = None;
        self.pending = None;
        self
    }

    /// Remove the item at `index`; `None` when the index is out of range.
    pub fn remove_item(&mut self, index: usize) -> Option<ToolbarItem> {
        if index >= self.items.len() {
            return None;
        }
        let item = self.items.remove(index);
        self.normalize_cursor();
        self.hover = None;
        self.pending = None;
        Some(item)
    }

    /// Rename one item's label without a rebuild.
    pub fn set_item_label(&mut self, index: usize, label: &str) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        if item.is_separator() {
            return false;
        }
        item.label = MnemonicLabel::parse(label);
        true
    }

    /// Extent of one cell along the bar: a width on horizontal bars, a height
    /// on vertical ones. Separators are thin; icon-only cells are square.
    fn cell_extent(&self, index: usize) -> f32 {
        let item = &self.items[index];
        if item.is_separator() {
            TOOL_SEPARATOR
        } else if self.vertical {
            self.item_height.max(24.0)
        } else if item.is_icon_only() {
            self.rect.height().max(24.0)
        } else {
            self.item_width.max(TOOL_MIN_CELL)
        }
    }

    /// The split between inline cells and the overflow button.
    fn metrics(&self) -> (usize, Option<Rect>) {
        let free = if self.vertical {
            (self.rect.height() - 8.0).max(0.0)
        } else {
            (self.rect.width() - 8.0).max(0.0)
        };
        let total: f32 = (0..self.items.len())
            .map(|index| self.cell_extent(index))
            .sum();
        if total <= free {
            return (self.items.len(), None);
        }
        let limit = (free - OVERFLOW_SIZE).max(0.0);
        let mut used = 0.0;
        let mut visible = 0;
        for index in 0..self.items.len() {
            let extent = self.cell_extent(index);
            if used + extent > limit {
                break;
            }
            used += extent;
            visible += 1;
        }
        let visible = visible.min(self.items.len().saturating_sub(1));
        let overflow = if self.vertical {
            Rect::new(
                self.rect.left,
                self.rect.bottom - 4.0 - OVERFLOW_SIZE,
                self.rect.right,
                self.rect.bottom - 4.0,
            )
        } else {
            Rect::new(
                self.rect.right - 4.0 - OVERFLOW_SIZE,
                self.rect.top,
                self.rect.right - 4.0,
                self.rect.bottom,
            )
        };
        (visible, Some(overflow))
    }

    /// Items that fit outside the overflow button.
    pub fn visible_count(&self) -> usize {
        self.metrics().0
    }

    /// Toggle one item's checked state without rebuilding the toolbar.
    pub fn set_checked(&mut self, index: usize, checked: bool) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        if item.is_separator() || item.checked == checked {
            return false;
        }
        item.checked = checked;
        true
    }

    pub fn set_enabled_item(&mut self, index: usize, enabled: bool) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        if item.is_separator() || item.enabled == enabled {
            return false;
        }
        item.enabled = enabled;
        true
    }

    fn overflow_menu(&self) -> Menu {
        let visible = self.visible_count();
        let mut menu = Menu::new();
        for item in self
            .items
            .iter()
            .skip(visible)
            .filter(|item| !item.is_separator())
        {
            menu = menu.push(
                crate::menu::MenuItem::new(item.command, item.display_name())
                    .checked(item.checked)
                    .disabled(!item.enabled),
            );
        }
        menu
    }

    fn open_overflow(&mut self) {
        let Some(anchor) = self.metrics().1 else {
            return;
        };
        let menu = self.overflow_menu();
        if !menu.is_empty() {
            self.pending = Some(NativeMenuRequest {
                menu,
                anchor: Point::new(anchor.left, anchor.bottom),
                avoid: Some(anchor),
            });
        }
    }

    fn overflow_rect(&self) -> Option<Rect> {
        self.metrics().1
    }

    /// Hit as an item index, `items.len()` for the overflow button, or `None`.
    /// Separators are never hit.
    fn index_at(&self, pos: Point) -> Option<usize> {
        if !self.rect.contains_point(pos) {
            return None;
        }
        let (visible, overflow) = self.metrics();
        if overflow.is_some_and(|rect| rect.contains_point(pos)) {
            return Some(self.items.len());
        }
        let local = if self.vertical {
            pos.y - self.rect.top
        } else {
            pos.x - self.rect.left
        };
        if local < 4.0 {
            return None;
        }
        let mut offset = 4.0;
        for index in 0..visible {
            let extent = self.cell_extent(index);
            if local < offset + extent {
                return (!self.items[index].is_separator()).then_some(index);
            }
            offset += extent;
        }
        None
    }

    fn cell_rect(&self, index: usize) -> Rect {
        let (visible, _) = self.metrics();
        if index >= visible {
            return self.overflow_rect().unwrap_or(self.rect);
        }
        let offset: f32 = (0..index).map(|prior| self.cell_extent(prior)).sum();
        let extent = self.cell_extent(index);
        if self.vertical {
            Rect::new(
                self.rect.left,
                self.rect.top + 4.0 + offset,
                self.rect.right,
                self.rect.top + 4.0 + offset + extent,
            )
        } else {
            Rect::new(
                self.rect.left + 4.0 + offset,
                self.rect.top,
                self.rect.left + 4.0 + offset + extent,
                self.rect.bottom,
            )
        }
    }

    /// Command indices the cursor can rest on: inline commands, then the
    /// overflow button when some items are hidden. Separators are skipped.
    fn cursor_targets(&self) -> Vec<usize> {
        let visible = self.visible_count();
        let mut targets: Vec<usize> = (0..visible)
            .filter(|&index| !self.items[index].is_separator())
            .collect();
        if visible < self.items.len() {
            targets.push(self.items.len());
        }
        if targets.is_empty() {
            targets.push(0);
        }
        targets
    }

    fn normalize_cursor(&mut self) {
        if self.items.is_empty() {
            self.cursor = 0;
            return;
        }
        let visible = self.visible_count();
        if self.cursor >= visible && visible < self.items.len() {
            self.cursor = self.items.len();
            return;
        }
        if self.cursor < self.items.len() && self.items[self.cursor].is_separator() {
            let right = (self.cursor..self.items.len())
                .find(|&index| !self.items[index].is_separator());
            let left = (0..self.cursor)
                .rev()
                .find(|&index| !self.items[index].is_separator());
            self.cursor = right.or(left).unwrap_or(0);
        }
    }

    fn run_cursor(&mut self) -> Option<M> {
        let visible = self.visible_count();
        if self.cursor >= visible {
            if visible < self.items.len() {
                self.open_overflow();
            }
            return None;
        }
        let item = self.items.get(self.cursor)?;
        if item.is_separator() || !item.enabled {
            return None;
        }
        let command = item.command;
        Some((self.on_command)(command))
    }

    fn click_at(&mut self, pos: Point) -> Option<M> {
        let index = self.index_at(pos)?;
        self.cursor = index;
        if index == self.items.len() {
            self.open_overflow();
            return None;
        }
        let item = self.items.get(index)?;
        if !item.enabled {
            return None;
        }
        let command = item.command;
        Some((self.on_command)(command))
    }

    fn step_cursor(&mut self, delta: i32) {
        let targets = self.cursor_targets();
        let position = targets
            .iter()
            .position(|&index| index == self.cursor)
            .unwrap_or(0);
        let next = (position as i32 + delta).clamp(0, targets.len() as i32 - 1) as usize;
        self.cursor = targets[next];
    }

    fn has_neighbor(&self, delta: i32) -> bool {
        let targets = self.cursor_targets();
        if targets.len() <= 1 {
            return false;
        }
        let position = targets.iter().position(|&index| index == self.cursor);
        match position {
            Some(position) => {
                let next = position as i32 + delta;
                next >= 0 && next < targets.len() as i32
            }
            None => true,
        }
    }
}

impl<M: 'static> Widget<M> for Toolbar<M> {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    observer_methods!();

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        if rect != self.rect {
            self.rect = rect;
            self.pending = None;
        }
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        if self.vertical {
            let cells: f32 = (0..self.items.len())
                .map(|index| self.cell_extent(index))
                .sum();
            8.0 + cells
        } else {
            32.0
        }
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover = None;
            self.pending = None;
        }
    }

    fn set_mnemonic_underlines(&mut self, show: bool) -> bool {
        if self.underlines == show {
            return false;
        }
        self.underlines = show;
        true
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Hand)
    }

    fn take_native_menu(&mut self) -> Option<NativeMenuRequest> {
        self.pending.take()
    }

    /// The overflow menu: the items that do not fit inline, without opening the
    /// popup (`None` while everything fits).
    fn native_menu(&self) -> Option<NativeMenuRequest> {
        if !self.enabled {
            return None;
        }
        let anchor = self.overflow_rect()?;
        let menu = self.overflow_menu();
        if menu.is_empty() {
            return None;
        }
        Some(NativeMenuRequest {
            menu,
            anchor: Point::new(anchor.left, anchor.bottom),
            avoid: Some(anchor),
        })
    }

    fn repaint_on_move_at(&self, pos: Point) -> bool {
        self.index_at(pos) != self.hover
    }

    /// A focused closed strip keeps its axis keys for the cursor only while
    /// the cursor has another cell to move to; the other axis and the edges
    /// move focus.
    fn owns_navigation_key(&self, key: KeyCode) -> bool {
        match (self.vertical, key) {
            (true, KeyCode::Up) => self.has_neighbor(-1),
            (true, KeyCode::Down) => self.has_neighbor(1),
            (false, KeyCode::Left) => self.has_neighbor(-1),
            (false, KeyCode::Right) => self.has_neighbor(1),
            _ => false,
        }
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        let (visible, overflow) = self.metrics();
        ctx.canvas.fill_rect(self.rect, palette.panel);
        if self.vertical {
            ctx.canvas.fill_rect(
                Rect::new(
                    self.rect.right - 1.0,
                    self.rect.top,
                    self.rect.right,
                    self.rect.bottom,
                ),
                palette.divider,
            );
        } else {
            ctx.canvas.fill_rect(
                Rect::new(
                    self.rect.left,
                    self.rect.bottom - 1.0,
                    self.rect.right,
                    self.rect.bottom,
                ),
                palette.divider,
            );
        }
        for (index, item) in self.items.iter().take(visible).enumerate() {
            let cell = self.cell_rect(index);
            if item.is_separator() {
                let center = cell.center();
                let line = if self.vertical {
                    Rect::new(cell.left + 6.0, center.y, cell.right - 6.0, center.y + 1.0)
                } else {
                    Rect::new(center.x, cell.top + 6.0, center.x + 1.0, cell.bottom - 6.0)
                };
                ctx.canvas.fill_rect(line, palette.divider);
                continue;
            }
            let hovered = self.enabled && self.hover == Some(index);
            let focused = state.focused && self.cursor == index;
            let background = if item.checked && item.enabled {
                palette.button_active
            } else if hovered || focused {
                palette.button_hover
            } else {
                palette.panel
            };
            if background != palette.panel {
                let inset = if self.vertical {
                    cell.inset(4.0, 2.0)
                } else {
                    cell.inset(2.0, 4.0)
                };
                ctx.canvas.fill_rect(inset, background);
            }
            let color = chrome_color(&palette, item.enabled, item.checked);
            if item.is_icon_only() {
                if let Some(glyph) = item.glyph {
                    draw_glyph(ctx, cell.inset(4.0, 4.0), glyph, color);
                }
                continue;
            }
            let mut left = cell.left + TOOL_CELL_PADDING;
            if let Some(glyph) = item.glyph {
                draw_glyph(
                    ctx,
                    Rect::new(left, cell.top, left + 18.0, cell.bottom),
                    glyph,
                    color,
                );
                left += 22.0;
            }
            paint_mnemonic(
                ctx,
                Rect::new(
                    left,
                    cell.top,
                    cell.right - TOOL_CELL_PADDING + 2.0,
                    cell.bottom,
                ),
                &item.label,
                TextStyleRole::Small,
                color,
                self.underlines,
            );
        }
        if let Some(overflow) = overflow {
            let hovered = self.enabled && self.hover == Some(self.items.len());
            if hovered {
                let inset = if self.vertical {
                    overflow.inset(4.0, 2.0)
                } else {
                    overflow.inset(2.0, 4.0)
                };
                ctx.canvas.fill_rect(inset, palette.button_hover);
            }
            draw_glyph(
                ctx,
                overflow,
                if self.vertical {
                    CHEVRON_DOWN
                } else {
                    OVERFLOW_GLYPH
                },
                if self.enabled {
                    palette.muted
                } else {
                    palette.text_disabled
                },
            );
        }
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
            WidgetInput::ClickAt(pos) => self.click_at(pos),
            WidgetInput::Activate => self.run_cursor(),
            WidgetInput::Key(key) => {
                match key.code {
                    KeyCode::Up if self.vertical => self.step_cursor(-1),
                    KeyCode::Down if self.vertical => self.step_cursor(1),
                    KeyCode::Left if !self.vertical => self.step_cursor(-1),
                    KeyCode::Right if !self.vertical => self.step_cursor(1),
                    KeyCode::Home => self.cursor = *self.cursor_targets().first().unwrap_or(&0),
                    KeyCode::End => self.cursor = *self.cursor_targets().last().unwrap_or(&0),
                    KeyCode::Enter | KeyCode::Space => return self.run_cursor(),
                    _ => {}
                }
                None
            }
            _ => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let visible = self.visible_count();
        let mut children: Vec<A11yChild> = self
            .items
            .iter()
            .take(visible)
            .enumerate()
            .filter(|(_, item)| !item.is_separator())
            .map(|(index, item)| {
                A11yChild::new(index, item.display_name(), self.cell_rect(index))
                    .selected(item.checked)
                    .enabled(self.enabled && item.enabled)
            })
            .collect();
        if visible < self.items.len() {
            let rect = self
                .overflow_rect()
                .unwrap_or_else(|| self.rect.inset(1.0, 1.0));
            children.push(A11yChild::new(self.items.len(), "更多", rect));
        }
        // Hidden commands stay reachable: they are published as menu items
        // whether or not the native overflow popup is open.
        let rect = self.overflow_rect().unwrap_or(self.rect);
        for mut child in menu_children(&self.overflow_menu()) {
            child.index += POPUP_CHILD_BASE;
            child.rect = rect;
            children.push(child);
        }
        Some(
            A11yInfo::new(
                A11yRole::ToolBar,
                self.items
                    .iter()
                    .filter(|item| !item.is_separator())
                    .map(ToolbarItem::display_name)
                    .collect::<Vec<_>>()
                    .join(" / "),
            )
            .children(children),
        )
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        if let Some(entry) = index.checked_sub(POPUP_CHILD_BASE) {
            let command = entry_command(&self.overflow_menu(), entry)?;
            return Some((self.on_command)(command));
        }
        if index == self.items.len() {
            self.open_overflow();
            return None;
        }
        let item = self.items.get(index)?;
        if item.is_separator() || !item.enabled || index >= self.visible_count() {
            return None;
        }
        let command = item.command;
        Some((self.on_command)(command))
    }

    fn a11y_expand(&mut self, _child: Option<usize>, expanded: bool) -> Option<M> {
        if expanded {
            self.open_overflow();
        }
        None
    }

    fn focus_ring_rect(&self) -> Rect {
        self.rect.inset(1.0, 1.0)
    }

    fn tooltip_rect(&self) -> Rect {
        if self.cursor < self.visible_count()
            && !self
                .items
                .get(self.cursor)
                .is_some_and(|item| item.is_separator())
        {
            self.cell_rect(self.cursor)
        } else {
            self.overflow_rect().unwrap_or(self.rect)
        }
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        self.index_at(pos)
    }

    fn focus_item(&self) -> Option<usize> {
        Some(self.cursor)
    }
}

observer_builders!(Toolbar);

// ---------------------------------------------------------------------------
// StatusBar
// ---------------------------------------------------------------------------

static STATUS_SKIN_CHILDREN: &[&str] = &["segment"];
const STATUS_SEGMENT_ATTRS: &[&str] = &["text", "icon", "width", "align"];

/// One status-bar cell: text, an optional icon and an optional fixed width.
#[derive(Clone, Debug)]
pub struct StatusSegment {    text: String,
    icon: Option<char>,
    width: Option<f32>,
    align: TextAlign,
}

impl StatusSegment {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            icon: None,
            width: None,
            align: TextAlign::Left,
        }
    }

    /// Leading icon glyph (an MDL2 codepoint or a text character).
    pub fn icon(mut self, glyph: char) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// Fixed width in DIPs; unset segments share the remaining space.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(20.0));
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn icon_glyph(&self) -> Option<char> {
        self.icon
    }

    pub fn fixed_width(&self) -> Option<f32> {
        self.width
    }

    pub fn text_align(&self) -> TextAlign {
        self.align
    }
}

const STATUS_PADDING: f32 = 10.0;
/// Width reserved by the diagonal size grip at the right edge.
const STATUS_GRIP: f32 = 14.0;
/// Width of the optional progress segment.
const STATUS_PROGRESS: f32 = 96.0;

/// A read-only bottom strip of status segments.
///
/// The bar is decoration for pointer and keyboard input (not a Tab stop), but
/// it publishes a `StatusBar` node with one text child per segment, so a
/// screen reader can read it. Segment 0 is the message segment: the generic
/// `Ui::set_text` path updates it when the application has no typed access.
/// An optional progress segment and the classic size grip occupy the right
/// edge and are reserved out of the segments' width.
pub struct StatusBar {
    id: WidgetId,
    rect: Rect,
    segments: Vec<StatusSegment>,
    label: Option<String>,
    size_grip: bool,
    progress: Option<f32>,
}

impl StatusBar {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 420.0, 24.0),
            segments: Vec::new(),
            label: None,
            size_grip: false,
            progress: None,
        }
    }

    pub fn segment(mut self, segment: StatusSegment) -> Self {
        self.segments.push(segment);
        self
    }

    /// Append a plain left-aligned text segment.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.segments.push(StatusSegment::new(text));
        self
    }

    /// Accessible name of the strip (defaults to the widget id).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Draw the classic diagonal size grip at the right edge (for a strip that
    /// sits on the window's bottom row).
    pub fn size_grip(mut self, show: bool) -> Self {
        self.size_grip = show;
        self
    }

    /// A right-aligned progress segment, `0..=1`. Segment widths shrink to
    /// leave room; the value clamps.
    pub fn progress(mut self, value: f32) -> Self {
        self.progress = Some(value.clamp(0.0, 1.0));
        self
    }

    pub fn segments(&self) -> &[StatusSegment] {
        &self.segments
    }

    /// Replace every segment (silent; the skin's `<segment>` child list).
    pub fn set_segments(&mut self, segments: Vec<StatusSegment>) {
        self.segments = segments;
    }

    pub fn has_size_grip(&self) -> bool {
        self.size_grip
    }

    pub fn progress_value(&self) -> Option<f32> {
        self.progress
    }

    /// Update the progress segment; returns whether the value changed.
    pub fn set_progress(&mut self, value: f32) -> bool {
        let value = value.clamp(0.0, 1.0);
        if self.progress == Some(value) {
            return false;
        }
        self.progress = Some(value);
        true
    }

    pub fn set_segment(&mut self, index: usize, text: impl Into<String>) -> bool {
        let Some(segment) = self.segments.get_mut(index) else {
            return false;
        };
        let text = text.into();
        if segment.text == text {
            return false;
        }
        segment.text = text;
        true
    }

    /// Width taken by the grip and the progress segment at the right edge.
    fn reserved_right(&self) -> f32 {
        let grip = if self.size_grip { STATUS_GRIP } else { 0.0 };
        let progress = if self.progress.is_some() {
            STATUS_PROGRESS
        } else {
            0.0
        };
        grip + progress
    }

    /// Segment rectangles: fixed widths first, the rest share what is left of
    /// the area not reserved for the grip/progress at the right edge.
    fn segment_rects(&self) -> Vec<Rect> {
        let mut widths = Vec::with_capacity(self.segments.len());
        let mut fixed = 0.0;
        let mut flexible = 0;
        for segment in &self.segments {
            match segment.width {
                Some(width) => {
                    fixed += width;
                    widths.push(Some(width));
                }
                None => {
                    flexible += 1;
                    widths.push(None);
                }
            }
        }
        let usable = (self.rect.width() - self.reserved_right()).max(0.0);
        let right_edge = self.rect.left + usable;
        let free = (usable - fixed).max(0.0);
        let each = if flexible > 0 {
            free / flexible as f32
        } else {
            0.0
        };
        let mut x = self.rect.left;
        let mut rects = Vec::with_capacity(self.segments.len());
        for width in widths {
            let width = width.unwrap_or(each);
            rects.push(Rect::new(
                x,
                self.rect.top,
                (x + width).min(right_edge),
                self.rect.bottom,
            ));
            x += width;
        }
        rects
    }

    fn progress_rect(&self) -> Option<Rect> {
        self.progress?;
        let right = self.rect.right
            - if self.size_grip { STATUS_GRIP } else { 0.0 }
            - STATUS_PADDING;
        Some(Rect::new(
            (right - STATUS_PROGRESS).max(self.rect.left),
            self.rect.top,
            right,
            self.rect.bottom,
        ))
    }
}

impl<M: 'static> Widget<M> for StatusBar {
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::TEXT
            .union(crate::props::ContentCaps::PROGRESS)
    }

    fn skin_children(&self) -> &'static [&'static str] {
        STATUS_SKIN_CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(|child| {
            child.name == "segment"
                && child
                    .attr("text")
                    .is_some_and(|text| !text.trim().is_empty())
                && child.attrs_are(STATUS_SEGMENT_ATTRS)
                && child
                    .attr("icon")
                    .is_none_or(|icon| icon.chars().count() == 1)
                && child.attr_number_ok("width")
                && child
                    .attr("align")
                    .is_none_or(|align| matches!(align, "left" | "center" | "right"))
        })
    }

    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        let segments = children
            .iter()
            .filter_map(|child| {
                let text = child.attr("text")?.to_string();
                let mut segment = StatusSegment::new(text);
                if let Some(icon) = child.attr("icon")
                    && let Some(ch) = icon.chars().next()
                {
                    segment = segment.icon(ch);
                }
                if let Some(width) = child.attr_number("width") {
                    segment = segment.width(width.max(20.0) as f32);
                }
                if let Some(align) = child.attr("align") {
                    segment = segment.align(match align {
                        "center" => TextAlign::Center,
                        "right" => TextAlign::Right,
                        _ => TextAlign::Left,
                    });
                }
                Some(segment)
            })
            .collect();
        self.set_segments(segments);
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
        24.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn interactive(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        ctx.canvas.fill_rect(self.rect, palette.panel);
        ctx.canvas.fill_rect(
            Rect::new(
                self.rect.left,
                self.rect.top,
                self.rect.right,
                self.rect.top + 1.0,
            ),
            palette.divider,
        );
        let rects = self.segment_rects();
        for (index, segment) in self.segments.iter().enumerate() {
            let Some(rect) = rects.get(index).copied() else {
                break;
            };
            let mut left = rect.left + STATUS_PADDING;
            if let Some(icon) = segment.icon {
                draw_glyph(
                    ctx,
                    Rect::new(left, rect.top, left + 16.0, rect.bottom),
                    icon,
                    palette.muted,
                );
                left += 20.0;
            }
            let content = Rect::new(left, rect.top, rect.right - STATUS_PADDING, rect.bottom);
            ctx.text(
                content,
                &segment.text,
                TextStyleRole::Small,
                segment.align,
                palette.muted,
            );
            if index + 1 < self.segments.len() {
                ctx.canvas.fill_rect(
                    Rect::new(
                        rect.right - 1.0,
                        rect.top + 5.0,
                        rect.right,
                        rect.bottom - 5.0,
                    ),
                    palette.divider,
                );
            }
        }
        // Optional progress segment: a thin track with an accent fill.
        if let Some(progress) = self.progress
            && let Some(rect) = self.progress_rect()
        {
            let center = rect.center().y;
            let track = Rect::new(rect.left, center - 3.0, rect.right, center + 3.0);
            ctx.canvas.fill_rect(track, palette.divider);
            let filled = track.width() * progress;
            if filled > 0.0 {
                ctx.canvas.fill_rect(
                    Rect::new(track.left, track.top, track.left + filled, track.bottom),
                    palette.accent,
                );
            }
        }
        // Classic diagonal size grip: three rows of dots at the bottom-right.
        if self.size_grip {
            let right = self.rect.right - 4.0;
            let bottom = self.rect.bottom - 4.0;
            for (row, count) in [(0, 1), (1, 2), (2, 3)] {
                for column in 0..count {
                    let x = right - (count - 1 - column) as f32 * 4.0;
                    let y = bottom - row as f32 * 4.0;
                    ctx.canvas.fill_rect(
                        Rect::new(x - 1.0, y - 1.0, x + 1.0, y + 1.0),
                        palette.scroll_thumb,
                    );
                }
            }
        }
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn set_progress(&mut self, value: f32) {
        self.set_progress(value);
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let rects = self.segment_rects();
        let mut children: Vec<A11yChild> = self
            .segments
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                A11yChild::new(
                    index,
                    segment.text.clone(),
                    rects.get(index).copied().unwrap_or(self.rect),
                )
            })
            .collect();
        if let (Some(progress), Some(rect)) = (self.progress, self.progress_rect()) {
            children.push(
                A11yChild::new(
                    self.segments.len(),
                    format!("进度 {}%", (progress * 100.0).round() as i32),
                    rect,
                )
                .role(A11yRole::ProgressBar),
            );
        }
        let name = self.label.clone().unwrap_or_else(|| self.id.0.to_string());
        Some(
            A11yInfo::new(A11yRole::StatusBar, name)
                .children(children)
                .display_only(),
        )
    }

    fn set_text(&mut self, text: String) {
        self.set_segment(0, text);
    }
}

// ---------------------------------------------------------------------------
// Ribbon
// ---------------------------------------------------------------------------

/// Size of one ribbon command: a full-height tile or a row in a three-row
/// column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RibbonItemSize {
    #[default]
    Large,
    Small,
}

/// One ribbon command. Commands are application ids like menu commands.
#[derive(Clone, Debug)]
pub struct RibbonItem {
    command: u32,
    label: MnemonicLabel,
    glyph: Option<char>,
    size: RibbonItemSize,
    enabled: bool,
    checked: bool,
}

impl RibbonItem {
    /// A full-height tile (glyph above the label).
    pub fn large(command: u32, label: impl AsRef<str>) -> Self {
        Self::new(command, label, RibbonItemSize::Large)
    }

    /// A row in a three-row column (glyph beside the label).
    pub fn small(command: u32, label: impl AsRef<str>) -> Self {
        Self::new(command, label, RibbonItemSize::Small)
    }

    fn new(command: u32, label: impl AsRef<str>, size: RibbonItemSize) -> Self {
        Self {
            command,
            label: MnemonicLabel::parse(label.as_ref()),
            glyph: None,
            size,
            enabled: true,
            checked: false,
        }
    }

    /// Leading icon glyph (an MDL2 codepoint or a text character).
    pub fn glyph(mut self, glyph: char) -> Self {
        self.glyph = Some(glyph);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn command(&self) -> u32 {
        self.command
    }

    pub fn label(&self) -> &str {
        self.label.text()
    }

    pub fn size(&self) -> RibbonItemSize {
        self.size
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// A labelled group of commands (the divider-separated cells along one tab).
#[derive(Clone, Debug)]
pub struct RibbonGroup {
    label: String,
    items: Vec<RibbonItem>,
}

impl RibbonGroup {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            items: Vec::new(),
        }
    }

    pub fn item(mut self, item: RibbonItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn items(&self) -> &[RibbonItem] {
        &self.items
    }
}

/// One ribbon tab: a title and the groups it shows.
#[derive(Clone, Debug)]
pub struct RibbonTab {
    title: MnemonicLabel,
    groups: Vec<RibbonGroup>,
}

impl RibbonTab {
    pub fn new(title: impl AsRef<str>) -> Self {
        Self {
            title: MnemonicLabel::parse(title.as_ref()),
            groups: Vec::new(),
        }
    }

    pub fn group(mut self, group: RibbonGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Title as painted (the access-key marker removed).
    pub fn title(&self) -> &str {
        self.title.text()
    }

    pub fn groups(&self) -> &[RibbonGroup] {
        &self.groups
    }
}

const RIBBON_TAB_WIDTH: f32 = 88.0;
const RIBBON_TAB_PADDING: f32 = 10.0;
const RIBBON_GROUP_GAP: f32 = 8.0;
const RIBBON_GROUP_LABEL: f32 = 16.0;
const RIBBON_PAD: f32 = 6.0;
const RIBBON_LARGE_WIDTH: f32 = 58.0;
const RIBBON_SMALL_WIDTH: f32 = 96.0;
const RIBBON_SMALL_HEIGHT: f32 = 22.0;
/// Published command children start at this index so they cannot collide with
/// the tab children.
const RIBBON_ITEM_BASE: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq)]
struct RibbonItemLayout {
    rect: Rect,
    group: usize,
    item: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RibbonGroupLayout {
    rect: Rect,
    label: Rect,
}

/// Cached geometry: fixed cell sizes mean no text measurement, but caching
/// keeps the paint path allocation-free.
#[derive(Clone, Debug, Default)]
struct RibbonLayout {
    rect: Rect,
    revision: u64,
    tab: usize,
    groups: Vec<RibbonGroupLayout>,
    items: Vec<RibbonItemLayout>,
}

/// A ribbon: a tab band over grouped command surfaces.
///
/// The application owns the model (`RibbonTab` → `RibbonGroup` →
/// `RibbonItem`). Large items are full-height tiles (glyph above the label);
/// consecutive small items fill three-row columns. Groups are drawn with a
/// divider and a bottom label. Clicking a tab selects it, clicking an enabled
/// item runs its command; in keyboard adjust mode Left/Right switch tabs (or
/// move between items once the cursor is in the body), Down enters the body,
/// arrows move geometrically and Enter runs the cursor item. The ribbon is one
/// Tab stop and publishes a `TabList` with its tabs plus a `Button` per
/// command. Tab titles may carry `&` access keys (`Alt+K` selects the tab);
/// labels follow the application's Alt underline mode.
pub struct Ribbon<M> {
    id: WidgetId,
    rect: Rect,
    tabs: Vec<RibbonTab>,
    selected: usize,
    /// Cursor: `None` on the tab band, otherwise a flat item index.
    cursor: Option<usize>,
    hover_tab: Option<usize>,
    hover_item: Option<usize>,
    press_item: Option<usize>,
    enabled: bool,
    underlines: bool,
    label: Option<String>,
    revision: u64,
    tab_height: f32,
    small_height: f32,
    large_width: f32,
    small_width: f32,
    layout: RefCell<RibbonLayout>,
    on_command: Box<dyn Fn(u32) -> M>,
    on_tab: Option<Box<dyn Fn(usize) -> M>>,
}

impl<M> Ribbon<M> {
    pub fn new(id: &'static str, tabs: Vec<RibbonTab>, on_command: impl Fn(u32) -> M + 'static) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 720.0, 140.0),
            tabs,
            selected: 0,
            cursor: None,
            hover_tab: None,
            hover_item: None,
            press_item: None,
            enabled: true,
            underlines: true,
            label: None,
            revision: 0,
            tab_height: 28.0,
            small_height: RIBBON_SMALL_HEIGHT,
            large_width: RIBBON_LARGE_WIDTH,
            small_width: RIBBON_SMALL_WIDTH,
            layout: RefCell::new(RibbonLayout::default()),
            on_command: Box::new(on_command),
            on_tab: None,
        }
    }

    /// Accessible name of the ribbon (defaults to the widget id).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The selected tab changed (also fired from `Alt+key` and UIA).
    pub fn on_tab(mut self, handler: impl Fn(usize) -> M + 'static) -> Self {
        self.on_tab = Some(Box::new(handler));
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    pub fn tab_height(mut self, height: f32) -> Self {
        self.tab_height = height.max(20.0);
        self
    }

    pub fn small_height(mut self, height: f32) -> Self {
        self.small_height = height.max(18.0);
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn mnemonic_underlines(&self) -> bool {
        self.underlines
    }

    pub fn tabs(&self) -> &[RibbonTab] {
        &self.tabs
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn selected_tab(&self) -> usize {
        self.selected
    }

    pub fn set_tabs(&mut self, tabs: Vec<RibbonTab>) {
        self.tabs = tabs;
        self.selected = self.selected.min(self.tabs.len().saturating_sub(1));
        self.cursor = None;
        self.hover_tab = None;
        self.hover_item = None;
        self.press_item = None;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Select `index` (clamped into range); returns whether it changed.
    pub fn select_tab(&mut self, index: usize) -> bool {
        if self.tabs.is_empty() {
            return false;
        }
        let index = index.min(self.tabs.len() - 1);
        if self.selected == index {
            return false;
        }
        self.selected = index;
        self.cursor = None;
        self.hover_item = None;
        self.press_item = None;
        true
    }

    /// Rectangle of one tab header.
    pub fn tab_rect(&self, index: usize) -> Option<Rect> {
        (index < self.tabs.len()).then(|| {
            let left = self.rect.left + index as f32 * RIBBON_TAB_WIDTH;
            Rect::new(
                left,
                self.rect.top,
                left + RIBBON_TAB_WIDTH,
                self.rect.top + self.tab_height,
            )
        })
    }

    /// Rectangle of an item on the selected tab.
    pub fn item_rect(&self, group: usize, item: usize) -> Option<Rect> {
        self.ensure_layout();
        let cache = self.layout.borrow();
        cache
            .items
            .iter()
            .find(|entry| entry.group == group && entry.item == item)
            .map(|entry| entry.rect)
    }

    /// Toggle one command's checked state; returns whether it changed.
    pub fn set_checked(&mut self, tab: usize, group: usize, item: usize, checked: bool) -> bool {
        let Some(entry) = self
            .tabs
            .get_mut(tab)
            .and_then(|tab| tab.groups.get_mut(group))
            .and_then(|group| group.items.get_mut(item))
        else {
            return false;
        };
        if entry.checked == checked {
            return false;
        }
        entry.checked = checked;
        true
    }

    /// Enable/disable one command; returns whether it changed.
    pub fn set_enabled_item(&mut self, tab: usize, group: usize, item: usize, enabled: bool) -> bool {
        let Some(entry) = self
            .tabs
            .get_mut(tab)
            .and_then(|tab| tab.groups.get_mut(group))
            .and_then(|group| group.items.get_mut(item))
        else {
            return false;
        };
        if entry.enabled == enabled {
            return false;
        }
        entry.enabled = enabled;
        true
    }

    fn ensure_layout(&self) {
        let mut cache = self.layout.borrow_mut();
        if cache.rect == self.rect && cache.revision == self.revision && cache.tab == self.selected {
            return;
        }
        *cache = self.build_layout();
    }

    fn build_layout(&self) -> RibbonLayout {
        let body_top = self.rect.top + self.tab_height;
        let content_top = body_top + 4.0;
        let content_bottom = (self.rect.bottom - RIBBON_GROUP_LABEL - 2.0).max(content_top);
        let mut groups = Vec::new();
        let mut items = Vec::new();
        let mut x = self.rect.left + RIBBON_PAD;
        if let Some(tab) = self.tabs.get(self.selected) {
            for (group_index, group) in tab.groups.iter().enumerate() {
                let group_left = x;
                let mut column_x = x;
                let mut index = 0;
                while index < group.items.len() {
                    match group.items[index].size {
                        RibbonItemSize::Large => {
                            let rect = Rect::new(
                                column_x,
                                content_top,
                                column_x + self.large_width,
                                content_bottom,
                            );
                            items.push(RibbonItemLayout {
                                rect,
                                group: group_index,
                                item: index,
                            });
                            column_x += self.large_width + 2.0;
                            index += 1;
                        }
                        RibbonItemSize::Small => {
                            let mut row = 0;
                            while index < group.items.len()
                                && group.items[index].size == RibbonItemSize::Small
                                && row < 3
                            {
                                let top = content_top + row as f32 * self.small_height;
                                let rect = Rect::new(
                                    column_x,
                                    top,
                                    column_x + self.small_width,
                                    (top + self.small_height).min(content_bottom),
                                );
                                items.push(RibbonItemLayout {
                                    rect,
                                    group: group_index,
                                    item: index,
                                });
                                index += 1;
                                row += 1;
                            }
                            column_x += self.small_width + 2.0;
                        }
                    }
                }
                let width = (column_x - 2.0 - group_left).max(1.0);
                let rect = Rect::new(group_left, body_top, group_left + width, self.rect.bottom);
                groups.push(RibbonGroupLayout {
                    rect,
                    label: Rect::new(
                        rect.left,
                        self.rect.bottom - RIBBON_GROUP_LABEL,
                        rect.right,
                        self.rect.bottom,
                    ),
                });
                x = rect.right + RIBBON_GROUP_GAP;
            }
        }
        RibbonLayout {
            rect: self.rect,
            revision: self.revision,
            tab: self.selected,
            groups,
            items,
        }
    }

    /// Tab header under `pos`, if any.
    fn tab_at(&self, pos: Point) -> Option<usize> {
        if pos.y > self.rect.top + self.tab_height || pos.x < self.rect.left {
            return None;
        }
        let index = ((pos.x - self.rect.left) / RIBBON_TAB_WIDTH).floor();
        if index < 0.0 {
            return None;
        }
        let index = index as usize;
        (index < self.tabs.len()).then_some(index)
    }

    /// Flat item index under `pos` on the selected tab.
    fn item_at(&self, pos: Point) -> Option<usize> {
        if pos.y <= self.rect.top + self.tab_height {
            return None;
        }
        self.ensure_layout();
        let cache = self.layout.borrow();
        cache
            .items
            .iter()
            .position(|entry| entry.rect.contains_point(pos))
    }

    /// Current command data: `(command, enabled)`.
    fn command_at(&self, flat: usize) -> Option<(u32, bool)> {
        self.ensure_layout();
        let (group, item) = {
            let cache = self.layout.borrow();
            let entry = cache.items.get(flat)?;
            (entry.group, entry.item)
        };
        let entry = self.tabs.get(self.selected)?.groups.get(group)?.items.get(item)?;
        Some((entry.command, entry.enabled))
    }

    fn run_item(&self, flat: usize) -> Option<M> {
        let (command, enabled) = self.command_at(flat)?;
        if !enabled || !self.enabled {
            return None;
        }
        Some((self.on_command)(command))
    }

    fn item_count(&self) -> usize {
        self.ensure_layout();
        self.layout.borrow().items.len()
    }

    /// Move the body cursor by geometry: up/down pick the nearest item whose
    /// rectangle is above/below, then the nearest horizontally.
    fn move_vertical(&mut self, up: bool) {
        let Some(flat) = self.cursor else {
            return;
        };
        self.ensure_layout();
        let current = self.layout.borrow().items.get(flat).map(|entry| entry.rect);
        let Some(current) = current else {
            return;
        };
        let next = {
            let cache = self.layout.borrow();
            let mut best: Option<(usize, f32, f32)> = None;
            for (index, entry) in cache.items.iter().enumerate() {
                if index == flat {
                    continue;
                }
                let above = entry.rect.bottom <= current.top + 0.5;
                let below = entry.rect.top >= current.bottom - 0.5;
                if (up && !above) || (!up && !below) {
                    continue;
                }
                let vertical = if up {
                    current.top - entry.rect.bottom
                } else {
                    entry.rect.top - current.bottom
                };
                let horizontal = (entry.rect.center().x - current.center().x).abs();
                let key = (vertical, horizontal);
                if best.is_none_or(|(_, best_v, best_h)| {
                    key.0 < best_v - 0.01 || ((key.0 - best_v).abs() <= 0.01 && key.1 < best_h)
                }) {
                    best = Some((index, key.0, key.1));
                }
            }
            best.map(|(index, _, _)| index)
        };
        match next {
            Some(index) => self.cursor = Some(index),
            None if up => self.cursor = None,
            None => {}
        }
    }

    fn select_tab_from_user(&mut self, index: usize) -> Option<M> {
        if !self.select_tab(index) {
            return None;
        }
        self.on_tab.as_ref().map(|handler| handler(index))
    }
}

impl<M: 'static> Widget<M> for Ribbon<M> {
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
        140.0
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover_tab = None;
            self.hover_item = None;
            self.press_item = None;
        }
    }

    fn adjustable(&self) -> bool {
        true
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Hand)
    }

    fn repaint_on_move_at(&self, pos: Point) -> bool {
        self.tab_at(pos) != self.hover_tab || self.item_at(pos) != self.hover_item
    }

    fn set_mnemonic_underlines(&mut self, show: bool) -> bool {
        if self.underlines == show {
            return false;
        }
        self.underlines = show;
        true
    }

    fn mnemonic_key(&mut self, ch: char) -> bool {
        let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.title.matches(ch))
        else {
            return false;
        };
        self.select_tab(index);
        true
    }

    fn focus_ring_rect(&self) -> Rect {
        self.rect.inset(1.0, 1.0)
    }

    fn tooltip_rect(&self) -> Rect {
        match self.cursor {
            Some(flat) => {
                self.ensure_layout();
                self.layout
                    .borrow()
                    .items
                    .get(flat)
                    .map(|entry| entry.rect)
                    .unwrap_or(self.rect)
            }
            None => self
                .tab_rect(self.selected)
                .unwrap_or(self.rect),
        }
    }

    fn child_at(&self, pos: Point) -> Option<usize> {
        if let Some(tab) = self.tab_at(pos) {
            return Some(tab);
        }
        self.item_at(pos).map(|flat| RIBBON_ITEM_BASE + flat)
    }

    fn focus_item(&self) -> Option<usize> {
        match self.cursor {
            Some(flat) => Some(RIBBON_ITEM_BASE + flat),
            None => Some(self.selected),
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        self.ensure_layout();
        let mut children: Vec<A11yChild> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                A11yChild::new(
                    index,
                    tab.title.text(),
                    self.tab_rect(index).unwrap_or(self.rect),
                )
                .selected(index == self.selected)
            })
            .collect();
        if let Some(tab) = self.tabs.get(self.selected) {
            let cache = self.layout.borrow();
            for (flat, entry) in cache.items.iter().enumerate() {
                let Some(item) = tab
                    .groups
                    .get(entry.group)
                    .and_then(|group| group.items.get(entry.item))
                else {
                    continue;
                };
                children.push(
                    A11yChild::new(RIBBON_ITEM_BASE + flat, item.label.text(), entry.rect)
                        .checked(item.checked)
                        .enabled(self.enabled && item.enabled)
                        .role(A11yRole::Button),
                );
            }
        }
        let name = self.label.clone().unwrap_or_else(|| self.id.0.to_string());
        Some(A11yInfo::new(A11yRole::TabList, name).children(children))
    }

    fn a11y_select_child(&mut self, index: usize) -> Option<M> {
        if index < self.tabs.len() {
            return self.select_tab_from_user(index);
        }
        let flat = index.checked_sub(RIBBON_ITEM_BASE)?;
        self.cursor = Some(flat);
        self.run_item(flat)
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        let tab_bottom = self.rect.top + self.tab_height;
        // Band and body. The selected tab merges into the body.
        ctx.canvas
            .fill_rect(Rect::new(self.rect.left, self.rect.top, self.rect.right, tab_bottom), palette.background);
        ctx.canvas.fill_rect(
            Rect::new(self.rect.left, tab_bottom, self.rect.right, self.rect.bottom),
            palette.panel,
        );
        ctx.canvas.fill_rect(
            Rect::new(self.rect.left, tab_bottom - 1.0, self.rect.right, tab_bottom),
            palette.divider,
        );
        for (index, tab) in self.tabs.iter().enumerate() {
            let Some(tab_rect) = self.tab_rect(index) else {
                continue;
            };
            let selected = index == self.selected;
            let hovered = self.enabled && self.hover_tab == Some(index);
            if selected {
                ctx.canvas.fill_rect(
                    Rect::new(tab_rect.left + 2.0, tab_rect.top, tab_rect.right - 2.0, tab_bottom + 1.0),
                    palette.panel,
                );
                ctx.canvas.fill_rect(
                    Rect::new(tab_rect.left + 2.0, tab_rect.top, tab_rect.right - 2.0, tab_rect.top + 2.0),
                    palette.accent,
                );
            } else if hovered || (state.focused && self.cursor.is_none() && self.selected == index) {
                ctx.canvas.fill_rect(
                    tab_rect.inset(2.0, 4.0),
                    palette.row_hover,
                );
            }
            let color = if selected {
                palette.accent_text
            } else if self.enabled {
                palette.text
            } else {
                palette.text_disabled
            };
            paint_mnemonic(
                ctx,
                Rect::new(
                    tab_rect.left + RIBBON_TAB_PADDING,
                    tab_rect.top,
                    tab_rect.right - RIBBON_TAB_PADDING,
                    tab_rect.bottom,
                ),
                &tab.title,
                TextStyleRole::Body,
                color,
                self.underlines,
            );
        }

        self.ensure_layout();
        let cache = self.layout.borrow();
        if let Some(tab) = self.tabs.get(self.selected) {
            for (group_index, group) in cache.groups.iter().enumerate() {
                if group_index + 1 < cache.groups.len() {
                    let x = group.rect.right + RIBBON_GROUP_GAP * 0.5;
                    ctx.canvas.fill_rect(
                        Rect::new(
                            x,
                            group.rect.top + 4.0,
                            x + 1.0,
                            group.label.top - 2.0,
                        ),
                        palette.divider,
                    );
                }
                if let Some(ribbon_group) = tab.groups.get(group_index) {
                    ctx.text_overflow(
                        group.label,
                        ribbon_group.label(),
                        TextStyleRole::Small,
                        TextAlign::Center,
                        palette.muted,
                        crate::backend::TextOverflow::Ellipsis,
                    );
                }
            }
            for (flat, entry) in cache.items.iter().enumerate() {
                let Some(item) = tab
                    .groups
                    .get(entry.group)
                    .and_then(|group| group.items.get(entry.item))
                else {
                    continue;
                };
                let hovered = self.enabled && self.hover_item == Some(flat);
                let pressed = self.enabled && self.press_item == Some(flat);
                let cursor = state.focused && self.cursor == Some(flat);
                let background = if pressed {
                    palette.button_pressed
                } else if item.checked && item.enabled {
                    palette.button_active
                } else if hovered || cursor {
                    palette.button_hover
                } else {
                    palette.panel
                };
                if background != palette.panel {
                    ctx.canvas.fill_rect(entry.rect.inset(2.0, 2.0), background);
                }
                let color = chrome_color(&palette, self.enabled && item.enabled, item.checked);
                match item.size {
                    RibbonItemSize::Large => {
                        if let Some(glyph) = item.glyph {
                            let glyph_rect = Rect::new(
                                entry.rect.left,
                                entry.rect.top + 6.0,
                                entry.rect.right,
                                entry.rect.top + entry.rect.height() * 0.58,
                            );
                            draw_glyph(ctx, glyph_rect, glyph, color);
                        }
                        ctx.text_overflow(
                            Rect::new(
                                entry.rect.left + 2.0,
                                entry.rect.top + entry.rect.height() * 0.58,
                                entry.rect.right - 2.0,
                                entry.rect.bottom - 2.0,
                            ),
                            item.label.text(),
                            TextStyleRole::Small,
                            TextAlign::Center,
                            color,
                            crate::backend::TextOverflow::Ellipsis,
                        );
                    }
                    RibbonItemSize::Small => {
                        let mut left = entry.rect.left + 6.0;
                        if let Some(glyph) = item.glyph {
                            draw_glyph(
                                ctx,
                                Rect::new(left, entry.rect.top, left + 16.0, entry.rect.bottom),
                                glyph,
                                color,
                            );
                            left += 20.0;
                        }
                        paint_mnemonic(
                            ctx,
                            Rect::new(left, entry.rect.top, entry.rect.right - 4.0, entry.rect.bottom),
                            &item.label,
                            TextStyleRole::Small,
                            color,
                            self.underlines,
                        );
                    }
                }
            }
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        match input {
            WidgetInput::HoverAt(pos) => {
                self.hover_tab = self.tab_at(pos);
                self.hover_item = self.item_at(pos);
                None
            }
            WidgetInput::HoverLeave => {
                self.hover_tab = None;
                self.hover_item = None;
                self.press_item = None;
                None
            }
            WidgetInput::PressAt(pos) => {
                self.press_item = self.item_at(pos);
                None
            }
            WidgetInput::ReleaseAt(_) => {
                self.press_item = None;
                None
            }
            WidgetInput::ClickAt(pos) => {
                if let Some(index) = self.tab_at(pos) {
                    return self.select_tab_from_user(index);
                }
                if let Some(flat) = self.item_at(pos) {
                    self.cursor = Some(flat);
                    return self.run_item(flat);
                }
                None
            }
            WidgetInput::Activate => {
                match self.cursor {
                    Some(flat) => self.run_item(flat),
                    None => {
                        self.cursor = (self.item_count() > 0).then_some(0);
                        None
                    }
                }
            }
            WidgetInput::Key(key) => {
                match key.code {
                    KeyCode::Left => match self.cursor {
                        Some(flat) => self.cursor = Some(flat.saturating_sub(1)),
                        None => {
                            if self.selected > 0 {
                                self.select_tab(self.selected - 1);
                            }
                        }
                    },
                    KeyCode::Right => match self.cursor {
                        Some(flat) => {
                            let last = self.item_count().saturating_sub(1);
                            self.cursor = Some((flat + 1).min(last));
                        }
                        None => {
                            if self.selected + 1 < self.tabs.len() {
                                self.select_tab(self.selected + 1);
                            }
                        }
                    },
                    KeyCode::Up => match self.cursor {
                        Some(flat) => {
                            self.move_vertical(true);
                            if self.cursor == Some(flat) && flat == 0 {
                                self.cursor = None;
                            }
                        }
                        None => {}
                    },
                    KeyCode::Down => {
                        if self.cursor.is_none() {
                            self.cursor = (self.item_count() > 0).then_some(0);
                        } else {
                            self.move_vertical(false);
                        }
                    }
                    KeyCode::Home => match self.cursor {
                        Some(_) => self.cursor = Some(0),
                        None => {
                            self.select_tab(0);
                        }
                    },
                    KeyCode::End => match self.cursor {
                        Some(_) => {
                            let last = self.item_count().saturating_sub(1);
                            self.cursor = Some(last);
                        }
                        None => {
                            self.select_tab(self.tabs.len().saturating_sub(1));
                        }
                    },
                    KeyCode::Enter | KeyCode::Space => {
                        return self.input(WidgetInput::Activate);
                    }
                    _ => {}
                }
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y::{A11yAction, A11yActionKind};
    use crate::backend::{Canvas, ImageData, TextStyle};
    use crate::event::KeyEvent;
    use crate::ui::{Invalidator, Ui, UiHost};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            vk: 0,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        }
    }

    fn sample_menu() -> Menu {
        Menu::new()
            .item(10, "打开")
            .checked_item(11, "随机", true)
            .separator()
            .submenu("更多", Menu::new().item(12, "复制"))
            .disabled_item(13, "禁用")
    }

    /// A canvas that only measures: 7 DIPs per character, so the menu-bar
    /// layout cache is deterministic in tests.
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
            _style: &TextStyle,
            _align: TextAlign,
            _color: crate::Color,
        ) {
        }
        fn measure_text(&mut self, text: &str, _style: &TextStyle) -> crate::Size {
            crate::Size::new(text.chars().count() as f32 * 7.0, 16.0)
        }
        fn ellipse(&mut self, _rect: Rect, _color: crate::Color) {}
        fn vertical_gradient(&mut self, _rect: Rect, _top: crate::Color, _bottom: crate::Color) {}
        fn image(
            &mut self,
            _rect: Rect,
            _crop: bool,
            _id: crate::backend::ImageId,
            _data: ImageData<'_>,
        ) {
        }
        fn push_clip(&mut self, _rect: Rect) {}
        fn pop_clip(&mut self) {}
    }

    /// Paint one widget so its measurement caches are populated.
    fn paint_widget<W: Widget<u32>>(widget: &mut W) {
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let dirty = [Rect::from_xywh(0.0, 0.0, 600.0, 400.0)];
        let mut canvas = Blank;
        let mut paint = WidgetPaint {
            painter: &painter,
            canvas: &mut canvas,
            palette: *painter.palette(),
            font: None,
            images: &images,
            viewport: Rect::from_xywh(0.0, 0.0, 600.0, 400.0),
            dirty: &dirty,
        };
        let state = WidgetState {
            enabled: true,
            ..Default::default()
        };
        <W as Widget<u32>>::paint(widget, &mut paint, &state);
    }

    fn set_rect<W: Widget<u32>>(widget: &mut W, rect: Rect) {
        <W as Widget<u32>>::set_rect(widget, rect);
    }

    fn menu_request<W: Widget<u32>>(widget: &mut W) -> Option<NativeMenuRequest> {
        <W as Widget<u32>>::take_native_menu(widget)
    }

    #[test]
    fn menu_button_click_requests_the_native_menu_below_it() {
        let mut button: MenuButton<u32> =
            MenuButton::new("mb", "文件(&F)", sample_menu(), |command| command);
        set_rect(&mut button, Rect::from_xywh(10.0, 10.0, 120.0, 30.0));
        assert!(menu_request(&mut button).is_none());
        button.input(WidgetInput::ClickAt(Point::new(20.0, 20.0)));
        let request = menu_request(&mut button).expect("menu requested");
        assert_eq!(request.menu.len(), 5);
        assert_eq!(request.anchor, Point::new(10.0, 40.0));
        assert_eq!(
            request.avoid,
            Some(Rect::from_xywh(10.0, 10.0, 120.0, 30.0))
        );
        // The request is one-shot.
        assert!(menu_request(&mut button).is_none());
        // Enter does the same.
        button.input(WidgetInput::Activate);
        assert!(menu_request(&mut button).is_some());
    }

    #[test]
    fn a_disabled_menu_button_requests_nothing() {
        let mut button: MenuButton<u32> =
            MenuButton::new("mb", "File", sample_menu(), |command| command).disabled(true);
        set_rect(&mut button, Rect::from_xywh(0.0, 0.0, 120.0, 30.0));
        button.input(WidgetInput::ClickAt(Point::new(10.0, 10.0)));
        assert!(menu_request(&mut button).is_none());
    }

    #[test]
    fn split_button_body_runs_primary_and_arrow_requests_the_menu() {
        let mut button: SplitButton<u32> =
            SplitButton::new("sb", "导出", sample_menu(), || 99, |command| command);
        set_rect(&mut button, Rect::from_xywh(0.0, 0.0, 160.0, 30.0));
        assert_eq!(
            button.input(WidgetInput::ClickAt(Point::new(20.0, 15.0))),
            Some(99)
        );
        assert!(menu_request(&mut button).is_none());
        assert_eq!(
            button.input(WidgetInput::ClickAt(Point::new(150.0, 15.0))),
            None
        );
        let request = menu_request(&mut button).expect("menu requested");
        assert_eq!(request.menu.len(), 5);
        // The popup hangs below the arrow, not below the whole body.
        assert_eq!(request.anchor, Point::new(134.0, 30.0));
        assert_eq!(request.avoid, Some(Rect::from_xywh(0.0, 0.0, 160.0, 30.0)));
        assert_eq!(button.input(WidgetInput::Activate), Some(99));
    }

    #[test]
    fn menu_bar_requests_the_clicked_items_menu_and_alt_opens_one() {
        let mut bar: MenuBar<u32> = MenuBar::new("bar", |command| command)
            .item("编辑(&E)", Menu::new().item(20, "撤销"))
            .item("视图(&V)", Menu::new().item(30, "放大"));
        set_rect(&mut bar, Rect::from_xywh(0.0, 0.0, 400.0, 26.0));
        paint_widget(&mut bar);
        // Cells start at the bar's left edge: 编辑, then 视图.
        let cells = bar.layout.borrow().cells.clone();
        bar.input(WidgetInput::ClickAt(Point::new(cells[1].left + 5.0, 10.0)));
        let request = menu_request(&mut bar).expect("menu requested");
        assert_eq!(
            request.menu.command(30).map(|item| item.label()),
            Some("放大")
        );
        assert_eq!(request.anchor, Point::new(cells[1].left, 26.0));
        assert_eq!(request.avoid, Some(cells[1]));
        assert_eq!(bar.cursor(), 1);

        assert!(bar.mnemonic_key('e'));
        let request = menu_request(&mut bar).expect("alt menu requested");
        assert_eq!(
            request.menu.command(20).map(|item| item.label()),
            Some("撤销")
        );
        assert_eq!(request.anchor, Point::new(cells[0].left, 26.0));
    }

    #[test]
    fn menu_bar_publishes_top_level_items_and_the_cursor_menu_commands() {
        let mut bar: MenuBar<u32> = MenuBar::new("bar", |command| command)
            .item("文件(F)", sample_menu())
            .item("编辑(E)", Menu::new().item(20, "撤销"));
        set_rect(&mut bar, Rect::from_xywh(0.0, 0.0, 400.0, 26.0));
        paint_widget(&mut bar);
        let info = bar.a11y().unwrap();
        assert_eq!(info.role, A11yRole::MenuBar);
        assert!(info.children[0].selected);
        // Two top-level items plus the four File entries.
        assert_eq!(info.children.len(), 6);
        assert!(info.children.iter().any(|child| child.name == "文件(F)"));
        assert_eq!(
            info.children
                .iter()
                .find(|child| child.name == "随机")
                .and_then(|child| child.checked),
            Some(true)
        );
        // The published command entry runs the command.
        let entry = info
            .children
            .iter()
            .position(|child| child.name == "打开")
            .unwrap();
        let index = info.children[entry].index;
        assert_eq!(bar.a11y_select_child(index), Some(10));
    }

    #[test]
    fn toolbar_moves_overflowing_items_into_a_native_menu() {
        let items: Vec<ToolbarItem> = (0..8)
            .map(|index| ToolbarItem::new(index + 1, format!("命令 {}", index + 1)).glyph('★'))
            .collect();
        let mut toolbar: Toolbar<u32> = Toolbar::new("tb", items, |command| command);
        set_rect(&mut toolbar, Rect::from_xywh(0.0, 0.0, 240.0, 32.0));
        let visible = toolbar.visible_count();
        assert!(visible < 8, "the strip must overflow at 240 DIPs");

        let info = toolbar.a11y().unwrap();
        assert_eq!(info.role, A11yRole::ToolBar);
        let hidden: Vec<usize> = info
            .children
            .iter()
            .skip_while(|child| child.index != POPUP_CHILD_BASE)
            .map(|child| child.index)
            .collect();
        assert_eq!(hidden.len(), 8 - visible);

        // Clicking the overflow button requests a menu of the hidden items.
        toolbar.input(WidgetInput::ClickAt(Point::new(
            toolbar.rect.right - 10.0,
            15.0,
        )));
        let request = menu_request(&mut toolbar).expect("overflow menu requested");
        assert_eq!(request.menu.len(), 8 - visible);

        // The published hidden commands run directly.
        assert_eq!(
            toolbar.a11y_select_child(hidden[0]),
            Some(visible as u32 + 1)
        );
    }

    #[test]
    fn toolbar_runs_items_and_reports_checked_state() {
        let items = vec![
            ToolbarItem::new(1, "播放").glyph('▶'),
            ToolbarItem::new(2, "收藏").checked(true),
            ToolbarItem::new(3, "禁用").disabled(true),
        ];
        let mut toolbar: Toolbar<u32> = Toolbar::new("tb", items, |command| command);
        set_rect(&mut toolbar, Rect::from_xywh(0.0, 0.0, 400.0, 32.0));
        assert_eq!(toolbar.visible_count(), 3);
        let info = toolbar.a11y().unwrap();
        assert_eq!(info.children.len(), 3);
        assert!(info.children[1].selected);
        assert!(!info.children[2].enabled);
        assert_eq!(toolbar.a11y_select_child(2), None);
        assert_eq!(toolbar.a11y_select_child(0), Some(1));
        assert!(toolbar.set_checked(0, true));
        assert!(!toolbar.set_checked(0, true));
    }

    #[test]
    fn toolbar_keyboard_cursor_runs_the_highlighted_item() {
        let items = vec![ToolbarItem::new(5, "A"), ToolbarItem::new(6, "B")];
        let mut toolbar: Toolbar<u32> = Toolbar::new("tb", items, |command| command);
        set_rect(&mut toolbar, Rect::from_xywh(0.0, 0.0, 400.0, 32.0));
        assert_eq!(toolbar.input(WidgetInput::Activate), Some(5));
        toolbar.input(WidgetInput::Key(key(KeyCode::Right)));
        assert_eq!(toolbar.input(WidgetInput::Activate), Some(6));
        toolbar.input(WidgetInput::Key(key(KeyCode::Home)));
        assert_eq!(toolbar.input(WidgetInput::Activate), Some(5));
    }

    #[test]
    fn hovering_inside_one_toolbar_cell_repaints_nothing() {
        let items: Vec<ToolbarItem> = (0..4)
            .map(|index| ToolbarItem::new(index + 1, "命令"))
            .collect();
        let mut toolbar: Toolbar<u32> = Toolbar::new("tb", items, |command| command);
        set_rect(&mut toolbar, Rect::from_xywh(0.0, 0.0, 400.0, 32.0));
        toolbar.input(WidgetInput::HoverAt(Point::new(20.0, 15.0)));
        // Same cell: no repaint. Next cell: repaint.
        assert!(!toolbar.repaint_on_move_at(Point::new(30.0, 16.0)));
        assert!(toolbar.repaint_on_move_at(Point::new(150.0, 16.0)));
    }

    #[test]
    fn status_bar_publishes_segments_and_updates_the_message_slot() {
        let mut bar = StatusBar::new("status")
            .text("就绪")
            .segment(
                StatusSegment::new("600 首")
                    .align(TextAlign::Right)
                    .width(120.0),
            )
            .label("状态栏");
        set_rect(&mut bar, Rect::from_xywh(0.0, 0.0, 500.0, 24.0));
        let info = <StatusBar as Widget<u32>>::a11y(&bar).unwrap();
        assert_eq!(info.role, A11yRole::StatusBar);
        assert!(info.display_only);
        assert_eq!(info.children.len(), 2);
        assert_eq!(info.children[1].name, "600 首");
        assert_eq!(info.children[1].rect, Rect::new(380.0, 0.0, 500.0, 24.0));

        assert!(bar.set_segment(0, "正在播放"));
        assert!(!bar.set_segment(0, "正在播放"));
        <StatusBar as Widget<u32>>::set_text(&mut bar, "已暂停".to_string());
        assert_eq!(bar.segments()[0].text(), "已暂停");
    }

    #[test]
    fn ui_surfaces_the_menu_request_to_the_host() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(MenuButton::new(
            "mb",
            "文件(&F)",
            sample_menu(),
            |command| command,
        ));
        ui.set_rect(
            WidgetId::new("mb"),
            Rect::from_xywh(10.0, 10.0, 120.0, 30.0),
        );
        let mut sink = TestSink::default();
        ui.pointer_down(Point::new(20.0, 20.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 20.0), &mut sink);
        assert_eq!(sink.menus.len(), 1);
        let (widget, anchor, menu, avoid) = &sink.menus[0];
        assert_eq!(*widget, WidgetId::new("mb"));
        assert_eq!(*anchor, Point::new(10.0, 40.0));
        assert_eq!(menu.len(), 5);
        assert_eq!(*avoid, Some(Rect::from_xywh(10.0, 10.0, 120.0, 30.0)));
    }

    #[test]
    fn ui_routes_alt_access_keys_to_the_chrome_widget() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(MenuButton::new(
            "mb",
            "文件(&F)",
            sample_menu(),
            |command| command,
        ));
        ui.set_rect(
            WidgetId::new("mb"),
            Rect::from_xywh(10.0, 10.0, 120.0, 30.0),
        );
        let mut sink = TestSink::default();
        let alt_f = KeyEvent {
            code: KeyCode::Letter(5),
            vk: 0x46,
            ctrl: false,
            shift: false,
            alt: true,
            repeat: false,
        };
        assert!(ui.key(alt_f, &mut sink));
        assert_eq!(ui.focus_id(), Some(WidgetId::new("mb")));
        assert_eq!(sink.menus.len(), 1);
    }

    #[test]
    fn ui_publishes_menu_commands_as_menu_items() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(MenuButton::new("mb", "File", sample_menu(), |command| {
            command
        }));
        ui.set_rect(
            WidgetId::new("mb"),
            Rect::from_xywh(10.0, 10.0, 120.0, 30.0),
        );
        let mut sink = TestSink::default();
        let nodes = ui.a11y_nodes();
        assert_eq!(nodes[0].role, A11yRole::Menu);
        assert_eq!(nodes[0].children.len(), 4);
        assert!(
            nodes
                .iter()
                .all(|node| { node.child.is_none() || node.role == A11yRole::MenuItem })
        );

        // UIA Invoke on a published item runs its command.
        let message = ui.apply_a11y(
            &A11yAction {
                kind: A11yActionKind::Invoke,
                id: WidgetId::new("mb"),
                child: Some(1),
                value: None,
                number: None,
            },
            &mut sink,
        );
        assert_eq!(message, Some(11));
        // And an expand request asks the host for the popup.
        ui.apply_a11y(
            &A11yAction {
                kind: A11yActionKind::Expand,
                id: WidgetId::new("mb"),
                child: None,
                value: None,
                number: None,
            },
            &mut sink,
        );
        assert_eq!(sink.menus.len(), 1);
    }

    #[test]
    fn arrows_leave_a_focused_toolbar_at_the_edges() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(crate::widgets::Button::new("a", "A", 1));
        ui.add(Toolbar::new(
            "tb",
            vec![ToolbarItem::new(5, "A"), ToolbarItem::new(6, "B")],
            |command| command,
        ));
        ui.set_rect(WidgetId::new("a"), Rect::from_xywh(0.0, 0.0, 100.0, 30.0));
        ui.set_rect(WidgetId::new("tb"), Rect::from_xywh(0.0, 50.0, 300.0, 32.0));
        let mut sink = TestSink::default();
        ui.key(key(KeyCode::Tab), &mut sink);
        ui.key(key(KeyCode::Tab), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("tb")));

        // Right moves inside the strip while there is a cell to the right.
        ui.key(key(KeyCode::Right), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("tb")));
        // At the last cell Right moves focus out instead of being swallowed.
        ui.key(key(KeyCode::Right), &mut sink);
        assert_ne!(ui.focus_id(), Some(WidgetId::new("tb")));

        // Up/Down always leave a closed strip.
        ui.key(key(KeyCode::Tab), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("tb")));
        ui.key(key(KeyCode::Up), &mut sink);
        assert_ne!(ui.focus_id(), Some(WidgetId::new("tb")));
    }

    #[test]
    fn arrows_leave_a_focused_menu_bar_and_enter_opens_it() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(MenuBar::new("bar", |command| command).item("文件(F)", sample_menu()));
        ui.add(crate::widgets::Button::new("b", "B", 2));
        ui.set_rect(WidgetId::new("bar"), Rect::from_xywh(0.0, 0.0, 400.0, 26.0));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 40.0, 100.0, 30.0));
        let mut sink = TestSink::default();
        ui.key(key(KeyCode::Tab), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("bar")));

        // Up leaves the bar.
        ui.key(key(KeyCode::Up), &mut sink);
        assert_ne!(ui.focus_id(), Some(WidgetId::new("bar")));

        // Enter (a single press) asks for the current top-level menu.
        ui.key(key(KeyCode::Tab), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("bar")));
        ui.key(key(KeyCode::Enter), &mut sink);
        assert_eq!(sink.menus.len(), 1);
        assert_eq!(sink.menus[0].0, WidgetId::new("bar"));
    }

    #[test]
    fn chrome_widgets_report_their_menu_without_opening_it() {
        // A menu button reports its own model at its own anchor.
        let mut button: MenuButton<u32> =
            MenuButton::new("mb", "文件", sample_menu(), |command| command);
        set_rect(&mut button, Rect::from_xywh(10.0, 10.0, 120.0, 30.0));
        let reported = <MenuButton<u32> as Widget<u32>>::native_menu(&button).expect("menu");
        assert_eq!(reported.menu.len(), 5);
        assert_eq!(reported.anchor, Point::new(10.0, 40.0));
        // Reporting does not consume the pending request slot.
        assert!(menu_request(&mut button).is_none());

        let mut split: SplitButton<u32> = SplitButton::new(
            "sb",
            "导出",
            sample_menu(),
            || 1,
            |command| command,
        );
        set_rect(&mut split, Rect::from_xywh(0.0, 0.0, 160.0, 30.0));
        let reported = <SplitButton<u32> as Widget<u32>>::native_menu(&split).expect("menu");
        assert_eq!(reported.anchor, Point::new(160.0 - ARROW_ZONE, 30.0));

        // A menu bar reports the cursor cell's menu, not the first one.
        let mut bar: MenuBar<u32> = MenuBar::new("bar", |command| command)
            .item("文件", sample_menu())
            .item("编辑", Menu::new().item(20, "撤销"));
        set_rect(&mut bar, Rect::from_xywh(0.0, 0.0, 400.0, 26.0));
        paint_widget(&mut bar);
        let first = <MenuBar<u32> as Widget<u32>>::native_menu(&bar).expect("menu");
        assert!(first.menu.command(10).is_some());
        bar.set_cursor(1);
        let second = <MenuBar<u32> as Widget<u32>>::native_menu(&bar).expect("menu");
        assert_eq!(second.menu.len(), 1);
        assert!(second.anchor.x > first.anchor.x);

        // A toolbar reports the overflow menu only while items overflow.
        let items: Vec<ToolbarItem> = (1..=20)
            .map(|command| ToolbarItem::new(command, format!("项{command}")))
            .collect();
        let mut toolbar: Toolbar<u32> = Toolbar::new("tb", items, |command| command);
        set_rect(&mut toolbar, Rect::from_xywh(0.0, 0.0, 300.0, 32.0));
        let overflow = <Toolbar<u32> as Widget<u32>>::native_menu(&toolbar).expect("overflow");
        assert_eq!(overflow.menu.len(), 20 - toolbar.visible_count());
    }

    #[test]
    fn set_native_menu_replaces_a_buttons_model() {
        let mut button: MenuButton<u32> =
            MenuButton::new("mb", "文件", sample_menu(), |command| command);
        let replaced = Menu::new().item(30, "新建");
        assert!(<MenuButton<u32> as Widget<u32>>::set_native_menu(
            &mut button,
            replaced.clone()
        ));
        assert_eq!(button.menu(), &replaced);
        assert!(menu_request(&mut button).is_none());

        // A widget without a menu slot reports false and keeps the container's
        // registry in charge.
        let mut plain: crate::widgets::Button<u32> = crate::widgets::Button::new("b", "B", 1);
        assert!(!<crate::widgets::Button<u32> as Widget<u32>>::set_native_menu(
            &mut plain,
            replaced
        ));
    }

    #[test]
    fn menu_bar_and_toolbar_items_can_be_edited_at_runtime() {
        let mut bar: MenuBar<u32> = MenuBar::new("bar", |command| command).item(
            "文件",
            Menu::new().item(1, "打开"),
        );
        assert!(bar.set_item_menu(0, Menu::new().item(2, "保存")));
        assert!(!bar.set_item_menu(5, Menu::new()));
        bar.insert_item(1, "视图", Menu::new().item(3, "缩放"));
        assert_eq!(bar.len(), 2);
        paint_widget(&mut bar);
        bar.set_cursor(1);
        let view = <MenuBar<u32> as Widget<u32>>::native_menu(&bar).expect("view menu");
        assert!(view.menu.command(3).is_some());
        let removed = bar.remove_item(0).expect("removed");
        assert_eq!(removed.0, "文件");
        assert_eq!(bar.len(), 1);
        assert!(bar.remove_item(9).is_none());

        let mut toolbar: Toolbar<u32> = Toolbar::new(
            "tb",
            vec![ToolbarItem::new(1, "一"), ToolbarItem::new(2, "二")],
            |command| command,
        );
        set_rect(&mut toolbar, Rect::from_xywh(0.0, 0.0, 600.0, 32.0));
        toolbar.insert_item(1, ToolbarItem::new(3, "三"));
        assert_eq!(toolbar.items().len(), 3);
        assert!(toolbar.set_item_label(0, "壹"));
        assert_eq!(toolbar.remove_item(1).map(|item| item.command), Some(3));
        assert!(toolbar.remove_item(9).is_none());
        assert_eq!(toolbar.items().len(), 2);
    }

    #[test]
    fn toolbar_separators_split_groups_and_never_take_input() {
        let items = vec![
            ToolbarItem::new(1, "A"),
            ToolbarItem::separator(),
            ToolbarItem::new(2, "B"),
        ];
        let mut toolbar: Toolbar<u32> = Toolbar::new("tb", items, |command| command);
        set_rect(&mut toolbar, Rect::from_xywh(0.0, 0.0, 400.0, 32.0));
        assert_eq!(toolbar.visible_count(), 3);
        // The separator owns no child and no menu slot.
        let info = toolbar.a11y().unwrap();
        assert_eq!(info.children.len(), 2);
        assert_eq!(info.children[1].name, "B");
        // Hovering the separator hits nothing.
        let separator = toolbar.cell_rect(1);
        assert_eq!(separator.width(), TOOL_SEPARATOR);
        assert_eq!(toolbar.index_at(separator.center()), None);
        // The keyboard cursor skips it.
        assert_eq!(toolbar.input(WidgetInput::Activate), Some(1));
        toolbar.input(WidgetInput::Key(key(KeyCode::Right)));
        assert_eq!(toolbar.input(WidgetInput::Activate), Some(2));
        toolbar.input(WidgetInput::Key(key(KeyCode::Left)));
        assert_eq!(toolbar.input(WidgetInput::Activate), Some(1));
    }

    #[test]
    fn toolbar_icon_only_cells_are_square_and_named() {
        let items = vec![
            ToolbarItem::new(1, "")
                .glyph('★')
                .icon_only()
                .name("收藏"),
            ToolbarItem::new(2, "播放"),
        ];
        let mut toolbar: Toolbar<u32> = Toolbar::new("tb", items, |command| command);
        set_rect(&mut toolbar, Rect::from_xywh(0.0, 0.0, 400.0, 32.0));
        let icon = toolbar.cell_rect(0);
        let label = toolbar.cell_rect(1);
        assert!((icon.width() - 32.0).abs() < 0.01, "icon cell {}", icon.width());
        assert!((label.left - icon.right).abs() < 0.01);
        let info = toolbar.a11y().unwrap();
        assert_eq!(info.children[0].name, "收藏");
        assert_eq!(info.children[1].name, "播放");
        // Clicking the square cell runs its command.
        assert_eq!(
            toolbar.input(WidgetInput::ClickAt(icon.center())),
            Some(1)
        );
    }

    #[test]
    fn vertical_toolbar_stacks_cells_and_uses_up_down() {
        let items = vec![
            ToolbarItem::new(1, "A"),
            ToolbarItem::new(2, "B"),
            ToolbarItem::new(3, "C"),
        ];
        let mut toolbar: Toolbar<u32> = Toolbar::new("tb", items, |command| command)
            .vertical()
            .item_height(30.0);
        set_rect(&mut toolbar, Rect::from_xywh(0.0, 0.0, 160.0, 200.0));
        assert!(toolbar.is_vertical());
        assert_eq!(toolbar.visible_count(), 3);
        assert_eq!(toolbar.cell_rect(0).top, 4.0);
        assert_eq!(toolbar.cell_rect(1).top, 34.0);
        assert_eq!(toolbar.index_at(Point::new(20.0, 40.0)), Some(1));
        assert!(toolbar.owns_navigation_key(KeyCode::Down));
        assert!(!toolbar.owns_navigation_key(KeyCode::Right));
        toolbar.input(WidgetInput::Key(key(KeyCode::Down)));
        assert_eq!(toolbar.input(WidgetInput::Activate), Some(2));
        toolbar.input(WidgetInput::Key(key(KeyCode::Up)));
        assert!(!toolbar.owns_navigation_key(KeyCode::Up));
    }

    #[test]
    fn vertical_toolbar_overflows_at_the_bottom() {
        let items: Vec<ToolbarItem> = (0..6)
            .map(|index| ToolbarItem::new(index + 1, format!("命令 {index}")))
            .collect();
        let mut toolbar: Toolbar<u32> = Toolbar::new("tb", items, |command| command)
            .vertical()
            .item_height(30.0);
        set_rect(&mut toolbar, Rect::from_xywh(0.0, 0.0, 160.0, 100.0));
        let visible = toolbar.visible_count();
        assert!(visible < 6);
        let overflow = toolbar.overflow_rect().expect("the column overflows");
        assert_eq!(overflow.bottom, 96.0);
        assert_eq!(toolbar.index_at(overflow.center()), Some(6));
        toolbar.input(WidgetInput::ClickAt(overflow.center()));
        let request = menu_request(&mut toolbar).expect("overflow menu requested");
        assert_eq!(request.menu.len(), 6 - visible);
    }

    #[test]
    fn the_alt_mode_toggles_chrome_underlines() {
        let mut toolbar: Toolbar<u32> =
            Toolbar::new("tb", vec![ToolbarItem::new(1, "&A")], |command| command);
        assert!(toolbar.mnemonic_underlines());
        assert!(<Toolbar<u32> as Widget<u32>>::set_mnemonic_underlines(
            &mut toolbar, false
        ));
        assert!(!toolbar.mnemonic_underlines());
        assert!(!<Toolbar<u32> as Widget<u32>>::set_mnemonic_underlines(
            &mut toolbar, false
        ));
        assert!(<Toolbar<u32> as Widget<u32>>::set_mnemonic_underlines(
            &mut toolbar, true
        ));

        let mut bar: MenuBar<u32> = MenuBar::new("bar", |command| command)
            .item("&文件", Menu::new().item(1, "打开"));
        assert!(<MenuBar<u32> as Widget<u32>>::set_mnemonic_underlines(
            &mut bar, false
        ));
        let button: MenuButton<u32> = MenuButton::new("mb", "&操作", sample_menu(), |command| command);
        assert!(button.underlines);
    }

    #[test]
    fn the_container_marks_only_chrome_when_underlines_change() {
        use crate::widgets::controls::Button;
        let mut ui: Ui<u32> = Ui::new();
        ui.add(MenuBar::new("bar", |command| command).item("&文件", Menu::new()));
        ui.set_rect(WidgetId::new("bar"), Rect::from_xywh(0.0, 0.0, 300.0, 26.0));
        ui.add(Button::new("plain", "普通", 7));
        ui.set_rect(
            WidgetId::new("plain"),
            Rect::from_xywh(0.0, 30.0, 100.0, 30.0),
        );
        let mut sink = TestSink::default();
        ui.set_mnemonic_underlines(false);
        assert!(
            sink.invalidations.is_empty(),
            "the flip records damage; it does not invalidate per widget"
        );
        assert!(ui.flush_damage(&mut sink));
        assert_eq!(
            sink.invalidations,
            vec![Rect::from_xywh(0.0, 0.0, 300.0, 26.0)]
        );
        // The second call changes nothing, so nothing repaints.
        ui.set_mnemonic_underlines(false);
        assert!(!ui.flush_damage(&mut sink));
        assert_eq!(sink.invalidations.len(), 1);
    }

    #[test]
    fn status_bar_reserves_the_size_grip_and_progress_segment() {
        let mut bar = StatusBar::new("status")
            .text("就绪")
            .size_grip(true)
            .progress(0.5);
        set_rect(&mut bar, Rect::from_xywh(0.0, 0.0, 500.0, 24.0));
        assert!(bar.has_size_grip());
        assert_eq!(bar.progress_value(), Some(0.5));
        // One flexible segment stops before the grip and the progress bar.
        assert_eq!(bar.segment_rects()[0].right, 500.0 - 14.0 - 96.0);
        let progress = bar.progress_rect().expect("progress bar");
        assert_eq!(progress.right, 500.0 - 14.0 - 10.0);
        assert!((progress.width() - 96.0).abs() < 0.01);
        // Values clamp and only real changes report.
        assert!(bar.set_progress(1.5));
        assert_eq!(bar.progress_value(), Some(1.0));
        assert!(!bar.set_progress(1.0));
        // The progress bar is published as its own child.
        let info = <StatusBar as Widget<u32>>::a11y(&bar).unwrap();
        assert_eq!(info.children.len(), 2);
        assert_eq!(info.children[1].role, Some(A11yRole::ProgressBar));
        assert_eq!(info.children[1].name, "进度 100%");
    }

    fn sample_ribbon() -> Ribbon<u32> {
        Ribbon::new(
            "rib",
            vec![
                RibbonTab::new("开始(&K)").group(
                    RibbonGroup::new("播放")
                        .item(RibbonItem::large(1, "播放").glyph('▶'))
                        .item(RibbonItem::small(2, "暂停"))
                        .item(RibbonItem::small(3, "下一曲"))
                        .item(RibbonItem::small(4, "上一曲")),
                )
                .group(
                    RibbonGroup::new("组织")
                        .item(RibbonItem::small(5, "收藏").checked(true))
                        .item(RibbonItem::small(6, "禁用").disabled(true)),
                ),
                RibbonTab::new("插入(&I)")
                    .group(RibbonGroup::new("封面").item(RibbonItem::large(10, "添加封面"))),
            ],
            |command| command,
        )
    }

    #[test]
    fn ribbon_lays_out_tabs_and_three_row_columns() {
        let mut ribbon = sample_ribbon();
        set_rect(&mut ribbon, Rect::from_xywh(0.0, 0.0, 720.0, 140.0));
        assert_eq!(ribbon.tab_count(), 2);
        assert_eq!(ribbon.tab_rect(0).unwrap().left, 0.0);
        assert_eq!(ribbon.tab_rect(1).unwrap().left, RIBBON_TAB_WIDTH);
        // The large tile is full height; the three small rows share one column.
        let large = ribbon.item_rect(0, 0).expect("large tile");
        assert!((large.width() - RIBBON_LARGE_WIDTH).abs() < 0.01);
        let first = ribbon.item_rect(0, 1).expect("first small row");
        let second = ribbon.item_rect(0, 2).expect("second small row");
        let third = ribbon.item_rect(0, 3).expect("third small row");
        assert!((first.left - second.left).abs() < 0.01);
        assert!((second.top - first.top - RIBBON_SMALL_HEIGHT).abs() < 0.01);
        assert!((third.top - second.top - RIBBON_SMALL_HEIGHT).abs() < 0.01);
        assert!(third.bottom <= ribbon.rect().bottom);
        // Group 1 starts a new column to the right of the small column.
        let checked = ribbon.item_rect(1, 0).expect("checked item");
        assert!(checked.left > third.right);
    }

    #[test]
    fn ribbon_click_selects_tabs_and_runs_items() {
        let mut ribbon = sample_ribbon();
        set_rect(&mut ribbon, Rect::from_xywh(0.0, 0.0, 720.0, 140.0));
        let second = ribbon.tab_rect(1).unwrap().center();
        assert_eq!(ribbon.input(WidgetInput::ClickAt(second)), None);
        assert_eq!(ribbon.selected_tab(), 1);
        let first = ribbon.tab_rect(0).unwrap().center();
        ribbon.input(WidgetInput::ClickAt(first));
        assert_eq!(ribbon.selected_tab(), 0);
        let item = ribbon.item_rect(0, 1).unwrap().center();
        assert_eq!(ribbon.input(WidgetInput::ClickAt(item)), Some(2));
        let disabled = ribbon.item_rect(1, 1).unwrap().center();
        assert_eq!(ribbon.input(WidgetInput::ClickAt(disabled)), None);
    }

    #[test]
    fn ribbon_keyboard_switches_tabs_moves_items_and_runs() {
        let mut ribbon = sample_ribbon();
        set_rect(&mut ribbon, Rect::from_xywh(0.0, 0.0, 720.0, 140.0));
        // The band cursor starts on the selected tab.
        ribbon.input(WidgetInput::Key(key(KeyCode::Right)));
        assert_eq!(ribbon.selected_tab(), 1);
        ribbon.input(WidgetInput::Key(key(KeyCode::Left)));
        assert_eq!(ribbon.selected_tab(), 0);
        // Down enters the body at the first item; Enter runs it.
        ribbon.input(WidgetInput::Key(key(KeyCode::Down)));
        assert_eq!(ribbon.input(WidgetInput::Activate), Some(1));
        ribbon.input(WidgetInput::Key(key(KeyCode::Right)));
        assert_eq!(ribbon.input(WidgetInput::Activate), Some(2));
        // Down moves into the next row of the same column.
        ribbon.input(WidgetInput::Key(key(KeyCode::Down)));
        assert_eq!(ribbon.input(WidgetInput::Activate), Some(3));
        // Up at the first row returns to the tab band.
        ribbon.input(WidgetInput::Key(key(KeyCode::Home)));
        ribbon.input(WidgetInput::Key(key(KeyCode::Up)));
        ribbon.input(WidgetInput::Key(key(KeyCode::Right)));
        assert_eq!(ribbon.selected_tab(), 1);
    }

    #[test]
    fn ribbon_checked_disabled_and_runtime_edits() {
        let mut ribbon = sample_ribbon();
        set_rect(&mut ribbon, Rect::from_xywh(0.0, 0.0, 720.0, 140.0));
        let info = ribbon.a11y().expect("the ribbon is exposed");
        assert_eq!(info.role, A11yRole::TabList);
        assert_eq!(info.name, "rib");
        // Two tabs plus the six items of the selected tab.
        assert_eq!(info.children.len(), 8);
        assert!(info.children[0].selected);
        assert_eq!(info.children[2].name, "播放");
        assert_eq!(info.children[2].role, Some(A11yRole::Button));
        // 收藏 is checked; 禁用 is disabled.
        assert_eq!(info.children[6].checked, Some(true));
        assert!(!info.children[7].enabled);
        assert!(ribbon.set_checked(0, 1, 0, false));
        assert!(!ribbon.set_checked(0, 1, 0, false));
        assert!(ribbon.set_enabled_item(0, 1, 1, true));
        // Selecting the second tab through UIA changes the published items.
        assert_eq!(ribbon.a11y_select_child(1), None);
        assert_eq!(ribbon.selected_tab(), 1);
        let info = ribbon.a11y().unwrap();
        assert_eq!(info.children.len(), 3);
        assert_eq!(info.children[2].name, "添加封面");
        // Invoking a command child runs it.
        assert_eq!(ribbon.a11y_select_child(RIBBON_ITEM_BASE), Some(10));
    }

    #[test]
    fn ribbon_mnemonics_select_tabs_and_underlines_toggle() {
        let mut ribbon = sample_ribbon();
        set_rect(&mut ribbon, Rect::from_xywh(0.0, 0.0, 720.0, 140.0));
        assert!(ribbon.mnemonic_key('i'));
        assert_eq!(ribbon.selected_tab(), 1);
        assert!(ribbon.mnemonic_underlines());
        assert!(<Ribbon<u32> as Widget<u32>>::set_mnemonic_underlines(
            &mut ribbon, false
        ));
        assert!(!ribbon.mnemonic_underlines());
    }

    #[derive(Default)]
    struct TestSink {
        menus: Vec<(WidgetId, Point, Menu, Option<Rect>)>,
        invalidations: Vec<Rect>,
    }
    impl Invalidator for TestSink {
        fn invalidate(&mut self, rect: Rect) {
            self.invalidations.push(rect);
        }
    }

    impl UiHost for TestSink {
        fn show_context_menu(&mut self, request: crate::ui::ContextMenuRequest<'_>) {
            self.menus.push((
                request.widget,
                request.anchor,
                request.menu.clone(),
                request.avoid,
            ));
        }
    }
}
