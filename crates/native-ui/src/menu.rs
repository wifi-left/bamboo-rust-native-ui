// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Declarative popup menus.
//!
//! A [`Menu`] describes items, separators and submenus; the platform host
//! turns it into a native `HMENU` and shows it with `TrackPopupMenu`. The
//! selection comes back as the item's command id through
//! [`crate::Host::menu_command`], so the application never sees `WM_COMMAND`.
//!
//! Commands are application-defined positive ids. `0` is reserved: the native
//! popup returns it when the menu was dismissed without a selection, so a
//! command id of `0` would be indistinguishable from cancel.
//!
//! A menu that depends on live state (a checkmark that follows a toggle) is
//! edited in place with the item-level setters below (`set_checked`,
//! `set_label`, `insert`, `remove_command`) and installed once with
//! [`crate::Ui::set_menu`]. Commands keep their ids, so a handler reads one id
//! no matter how the labels around it change.
//!
//! Every popup the framework opens — context menus, chrome menus, the tray
//! menu and programmatic opens — reports the same [`MenuEvent`] lifecycle to
//! [`crate::Host::menu_event`], so one hook can log, rewrite or consume
//! commands ([`MenuDecision`]).

use crate::geometry::Point;
use crate::hotkey::Hotkey;
use crate::widget::WidgetId;

/// A small RGBA bitmap for a native menu row (an item icon).
///
/// The shell draws `hbmpItem` bitmaps at its own menu-icon size, so 16×16 is
/// the conventional source size; larger images are scaled. Rows are
/// top-down RGBA; the platform layer converts them to the native 32-bit DIB
/// and frees the handles with the popup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuIcon {
    width: u16,
    height: u16,
    rgba: Vec<u8>,
}

impl MenuIcon {
    pub fn from_rgba(width: u16, height: u16, rgba: impl Into<Vec<u8>>) -> Self {
        let rgba = rgba.into();
        let width = width.max(1);
        let height = height.max(1);
        assert_eq!(
            rgba.len(),
            width as usize * height as usize * 4,
            "menu icon RGBA must be width * height * 4 bytes"
        );
        Self {
            width,
            height,
            rgba,
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// One actionable entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub(crate) command: u32,
    pub(crate) label: String,
    pub(crate) shortcut: Option<String>,
    pub(crate) icon: Option<MenuIcon>,
    pub(crate) checked: bool,
    pub(crate) disabled: bool,
}

impl MenuItem {
    /// A plain (unchecked, enabled) item. Command ids are nonzero.
    pub fn new(command: u32, label: impl Into<String>) -> Self {
        assert!(command != 0, "menu command 0 is reserved for dismissal");
        Self {
            command,
            label: label.into(),
            shortcut: None,
            icon: None,
            checked: false,
            disabled: false,
        }
    }

    /// Draw a check mark next to the label (settings-style toggles).
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Grey the item out; it stays visible but cannot be selected.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Accelerator text in the native right-hand column (`"Ctrl+P"`); the
    /// platform appends it after a tab so the shell aligns it.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// A small icon bitmap drawn in the native left-hand column.
    pub fn icon(mut self, icon: MenuIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn command(&self) -> u32 {
        self.command
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn shortcut_text(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// The accelerator label parsed as a chord (`"Ctrl+P"` → ctrl + P). `None`
    /// when the item has no shortcut or the text is not a usable chord; the
    /// native column still shows the text either way.
    pub fn hotkey(&self) -> Option<Hotkey> {
        Hotkey::parse(self.shortcut.as_deref()?)
    }

    pub fn icon_image(&self) -> Option<&MenuIcon> {
        self.icon.as_ref()
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    // Runtime edits. A menu stored in `WidgetProps`/`Ui::set_menu` can be
    // changed between opens without rebuilding the menu or the widget tree.

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    pub fn set_shortcut(&mut self, shortcut: Option<String>) {
        self.shortcut = shortcut;
    }

    pub fn set_icon(&mut self, icon: Option<MenuIcon>) {
        self.icon = icon;
    }

    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    /// Reassign the command id (a menu alias). `0` is reserved for dismissal
    /// and is ignored.
    pub fn set_command(&mut self, command: u32) {
        if command != 0 {
            self.command = command;
        }
    }
}

/// A nested menu opened from a parent entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submenu {
    pub(crate) label: String,
    pub(crate) menu: Menu,
    pub(crate) disabled: bool,
}

impl Submenu {
    pub fn new(label: impl Into<String>, menu: Menu) -> Self {
        Self {
            label: label.into(),
            menu,
            disabled: false,
        }
    }

    /// Grey the parent entry out; the submenu cannot be opened.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }
}

/// One line of a popup: an item, a separator or a submenu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    Item(MenuItem),
    Separator,
    Submenu(Submenu),
}

impl MenuEntry {
    pub fn item(command: u32, label: impl Into<String>) -> Self {
        Self::Item(MenuItem::new(command, label))
    }

    pub fn separator() -> Self {
        Self::Separator
    }

    pub fn submenu(label: impl Into<String>, menu: Menu) -> Self {
        Self::Submenu(Submenu::new(label, menu))
    }

    /// Grey out an item or a submenu entry; separators ignore it.
    pub fn disabled(mut self, disabled: bool) -> Self {
        match &mut self {
            MenuEntry::Item(item) => item.disabled = disabled,
            MenuEntry::Submenu(submenu) => submenu.disabled = disabled,
            MenuEntry::Separator => {}
        }
        self
    }

    /// Check an item; separators and submenus ignore it.
    pub fn checked(mut self, checked: bool) -> Self {
        if let MenuEntry::Item(item) = &mut self {
            item.checked = checked;
        }
        self
    }

    /// Rename an item or a submenu; a separator has no label and is unchanged.
    /// Returns whether the entry carries a label now.
    pub fn set_label(&mut self, label: impl Into<String>) -> bool {
        match self {
            MenuEntry::Item(item) => {
                item.label = label.into();
                true
            }
            MenuEntry::Submenu(submenu) => {
                submenu.label = label.into();
                true
            }
            MenuEntry::Separator => false,
        }
    }

    /// The command id of an item; separators and submenus report `None`.
    pub fn command(&self) -> Option<u32> {
        match self {
            MenuEntry::Item(item) => Some(item.command),
            _ => None,
        }
    }
}

impl From<MenuItem> for MenuEntry {
    fn from(item: MenuItem) -> Self {
        MenuEntry::Item(item)
    }
}

impl From<Submenu> for MenuEntry {
    fn from(submenu: Submenu) -> Self {
        MenuEntry::Submenu(submenu)
    }
}

/// An ordered list of popup entries.
///
/// Builders push in visual order; a `Menu` is presentation only and carries no
/// state. Edge and doubled separators are dropped by the platform layer when
/// the native popup is built.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Menu {
    entries: Vec<MenuEntry>,
}

impl Menu {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an entry built elsewhere (`MenuItem`, `Submenu` or a raw
    /// `MenuEntry`).
    pub fn push(mut self, entry: impl Into<MenuEntry>) -> Self {
        self.entries.push(entry.into());
        self
    }

    /// Append a plain item.
    pub fn item(self, command: u32, label: impl Into<String>) -> Self {
        self.push(MenuItem::new(command, label))
    }

    /// Append an item with accelerator text in the native right-hand column.
    pub fn shortcut_item(
        self,
        command: u32,
        label: impl Into<String>,
        shortcut: impl Into<String>,
    ) -> Self {
        self.push(MenuItem::new(command, label).shortcut(shortcut))
    }

    /// Append a checked item (menus that mirror a toggle).
    pub fn checked_item(self, command: u32, label: impl Into<String>, checked: bool) -> Self {
        self.push(MenuItem::new(command, label).checked(checked))
    }

    /// Append an item that is visible but not selectable.
    pub fn disabled_item(self, command: u32, label: impl Into<String>) -> Self {
        self.push(MenuItem::new(command, label).disabled(true))
    }

    /// Append a horizontal separator.
    pub fn separator(self) -> Self {
        self.push(MenuEntry::Separator)
    }

    /// Append a submenu.
    pub fn submenu(self, label: impl Into<String>, menu: Menu) -> Self {
        self.push(Submenu::new(label, menu))
    }

    pub fn entries(&self) -> &[MenuEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Find the item with `command`, including items inside submenus.
    pub fn command(&self, command: u32) -> Option<&MenuItem> {
        for entry in &self.entries {
            match entry {
                MenuEntry::Item(item) if item.command == command => return Some(item),
                MenuEntry::Submenu(submenu) => {
                    if let Some(item) = submenu.menu.command(command) {
                        return Some(item);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Visit every actionable item in visual order, submenus included.
    pub fn visit_items(&self, visit: &mut impl FnMut(&MenuItem)) {
        for entry in &self.entries {
            match entry {
                MenuEntry::Item(item) => visit(item),
                MenuEntry::Submenu(submenu) => submenu.menu.visit_items(visit),
                MenuEntry::Separator => {}
            }
        }
    }

    // -- runtime mutation -------------------------------------------------
    //
    // A stored menu (context menu or a chrome menu) is edited between opens;
    // an open popup keeps the model it was built from, so an edit always
    // applies to the *next* open. See `docs/DECISIONS.md` (D61).

    /// Insert `entry` at `index` (clamped to the end).
    pub fn insert(&mut self, index: usize, entry: impl Into<MenuEntry>) -> &mut Self {
        let index = index.min(self.entries.len());
        self.entries.insert(index, entry.into());
        self
    }

    /// Remove and return the entry at `index`.
    pub fn remove(&mut self, index: usize) -> Option<MenuEntry> {
        (index < self.entries.len()).then(|| self.entries.remove(index))
    }

    /// Remove the item with `command`, searching submenus depth-first.
    pub fn remove_command(&mut self, command: u32) -> Option<MenuItem> {
        for index in 0..self.entries.len() {
            let remove_here = match &mut self.entries[index] {
                MenuEntry::Item(item) => item.command == command,
                MenuEntry::Submenu(submenu) => {
                    if let Some(item) = submenu.menu.remove_command(command) {
                        return Some(item);
                    }
                    false
                }
                MenuEntry::Separator => false,
            };
            if remove_here {
                let Some(MenuEntry::Item(item)) = self.remove(index) else {
                    return None;
                };
                return Some(item);
            }
        }
        None
    }

    /// The mutable entry at `path` (a submenu trail plus the last step);
    /// `None` for an empty or invalid path.
    pub fn entry_mut(&mut self, path: &MenuPath) -> Option<&mut MenuEntry> {
        let (last, parents) = path.steps().split_last()?;
        let mut menu = self;
        for step in parents {
            menu = menu.submenu_entry_mut(step)?;
        }
        let index = menu.entry_index(last)?;
        menu.entries.get_mut(index)
    }

    /// The mutable item with `command`, searching submenus depth-first.
    pub fn item_mut(&mut self, command: u32) -> Option<&mut MenuItem> {
        for entry in &mut self.entries {
            match entry {
                MenuEntry::Item(item) if item.command == command => return Some(item),
                MenuEntry::Submenu(submenu) => {
                    if let Some(item) = submenu.menu.item_mut(command) {
                        return Some(item);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Rename the item with `command`; false when it does not exist.
    pub fn set_label(&mut self, command: u32, label: impl Into<String>) -> bool {
        match self.item_mut(command) {
            Some(item) => {
                item.set_label(label);
                true
            }
            None => false,
        }
    }

    /// Check/uncheck the item with `command`; false when it does not exist.
    pub fn set_checked(&mut self, command: u32, checked: bool) -> bool {
        match self.item_mut(command) {
            Some(item) => {
                item.set_checked(checked);
                true
            }
            None => false,
        }
    }

    /// Enable/disable the item with `command`; false when it does not exist.
    pub fn set_enabled(&mut self, command: u32, enabled: bool) -> bool {
        match self.item_mut(command) {
            Some(item) => {
                item.set_disabled(!enabled);
                true
            }
            None => false,
        }
    }

    /// Reassign the command id of the item stored under `old` (a menu alias).
    /// `0` and a missing item report false.
    pub fn set_command(&mut self, old: u32, new: u32) -> bool {
        if new == 0 {
            return false;
        }
        match self.item_mut(old) {
            Some(item) => {
                item.set_command(new);
                true
            }
            None => false,
        }
    }

    /// The submenu a path addresses (an empty path is this menu).
    pub fn submenu_at(&self, path: &MenuPath) -> Option<&Menu> {
        let mut menu = self;
        for step in path.steps() {
            menu = menu.submenu_at_step(step)?;
        }
        Some(menu)
    }

    /// The mutable submenu a path addresses.
    pub fn submenu_at_mut(&mut self, path: &MenuPath) -> Option<&mut Menu> {
        let mut menu = self;
        for step in path.steps() {
            menu = menu.submenu_entry_mut(step)?;
        }
        Some(menu)
    }

    fn submenu_at_step(&self, step: &MenuStep) -> Option<&Menu> {
        let index = self.entry_index(step)?;
        match self.entries.get(index)? {
            MenuEntry::Submenu(submenu) => Some(&submenu.menu),
            _ => None,
        }
    }

    fn submenu_entry_mut(&mut self, step: &MenuStep) -> Option<&mut Menu> {
        let index = self.entry_index(step)?;
        match self.entries.get_mut(index)? {
            MenuEntry::Submenu(submenu) => Some(&mut submenu.menu),
            _ => None,
        }
    }

    fn entry_index(&self, step: &MenuStep) -> Option<usize> {
        match step {
            MenuStep::Index(index) => (*index < self.entries.len()).then_some(*index),
            MenuStep::Label(label) => self.entries.iter().position(|entry| match entry {
                MenuEntry::Item(item) => item.label == *label,
                MenuEntry::Submenu(submenu) => submenu.label == *label,
                MenuEntry::Separator => false,
            }),
        }
    }
}

/// One step of a [`MenuPath`]: a position, or the first entry with a label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuStep {
    Index(usize),
    Label(String),
}

impl MenuStep {
    pub fn index(index: usize) -> Self {
        MenuStep::Index(index)
    }

    pub fn label(label: impl Into<String>) -> Self {
        MenuStep::Label(label.into())
    }
}

impl From<usize> for MenuStep {
    fn from(index: usize) -> Self {
        MenuStep::Index(index)
    }
}

impl From<&str> for MenuStep {
    fn from(label: &str) -> Self {
        MenuStep::Label(label.to_string())
    }
}

impl From<String> for MenuStep {
    fn from(label: String) -> Self {
        MenuStep::Label(label)
    }
}

/// A stable address into a menu's submenu tree.
///
/// An index step addresses a position and follows a reorder only while the
/// entries before it stay put; a label step addresses the first entry with
/// that label. Items themselves are addressed by command id, which is why the
/// id is what every handler matches on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuPath {
    steps: Vec<MenuStep>,
}

impl MenuPath {
    /// The root menu (no steps).
    pub fn root() -> Self {
        Self::default()
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn then(mut self, step: impl Into<MenuStep>) -> Self {
        self.steps.push(step.into());
        self
    }

    pub fn index(self, index: usize) -> Self {
        self.then(MenuStep::Index(index))
    }

    pub fn label(self, label: impl Into<String>) -> Self {
        self.then(MenuStep::Label(label.into()))
    }

    pub fn steps(&self) -> &[MenuStep] {
        &self.steps
    }

    pub fn is_root(&self) -> bool {
        self.steps.is_empty()
    }
}

/// A top-level item of a native window menu bar (`程序(&P)`).
///
/// The label keeps its `&` access key: the system draws the menu bar and
/// underlines the key while Alt is held, so unlike the painted menu-bar widget
/// the label is not parsed here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuBarEntry {
    label: String,
    menu: Menu,
    separator: bool,
}

impl MenuBarEntry {
    pub fn new(label: impl Into<String>, menu: Menu) -> Self {
        Self {
            label: label.into(),
            menu,
            separator: false,
        }
    }

    /// A vertical divider between groups of bar titles. It owns no popup and
    /// takes no access key, focus or mnemonic.
    pub fn separator() -> Self {
        Self {
            label: String::new(),
            menu: Menu::new(),
            separator: true,
        }
    }

    pub fn is_separator(&self) -> bool {
        self.separator
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    pub fn menu_mut(&mut self) -> &mut Menu {
        &mut self.menu
    }

    pub fn set_menu(&mut self, menu: Menu) {
        self.menu = menu;
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }
}

/// The `&` access key in a menu title (`&&` is a literal ampersand).
fn access_key(label: &str) -> Option<char> {
    let mut escaped = false;
    for character in label.chars() {
        if escaped {
            return (character != '&').then(|| character.to_ascii_lowercase());
        }
        if character == '&' {
            escaped = true;
        }
    }
    None
}

/// The model of a native window menu bar: the bar the system draws under the
/// title bar. Installed with `WindowBuilder::menu_bar` /
/// `WindowHandle::set_menu_bar`.
///
/// Commands arrive through [`crate::Host::menu_bar_command`] (after the
/// [`MenuEvent`] hook) as plain application ids, so the handler matches ids
/// exactly like a popup menu. The bar is system-drawn: its access keys,
/// keyboard behavior (`Alt`, arrows, Escape) and accessibility are the
/// shell's, at the cost of following the system colors rather than the page
/// palette.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuBarModel {
    entries: Vec<MenuBarEntry>,
}

impl MenuBarModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(mut self, entry: impl Into<MenuBarEntry>) -> Self {
        self.entries.push(entry.into());
        self
    }

    /// Append a top-level menu. The label may carry a `&` access key.
    pub fn item(self, label: impl Into<String>, menu: Menu) -> Self {
        self.push(MenuBarEntry::new(label, menu))
    }

    /// Append a vertical divider between groups of bar titles.
    pub fn separator(self) -> Self {
        self.push(MenuBarEntry::separator())
    }

    pub fn entries(&self) -> &[MenuBarEntry] {
        &self.entries
    }

    /// Window accelerators carried by the bar's drop-downs: every enabled item
    /// whose shortcut label parses as a chord, as `(hotkey, command)`. The
    /// window host registers these while the model is installed, so the
    /// accelerator column becomes live. Submenu entries are skipped (their
    /// rows are not bar commands) and the first item wins a repeated chord.
    pub fn hotkeys(&self) -> Vec<(Hotkey, u32)> {
        let mut hotkeys: Vec<(Hotkey, u32)> = Vec::new();
        for entry in &self.entries {
            if entry.separator {
                continue;
            }
            for item in entry.menu.entries() {
                let MenuEntry::Item(item) = item else {
                    continue;
                };
                if item.disabled {
                    continue;
                }
                let Some(hotkey) = item.hotkey() else {
                    continue;
                };
                if hotkeys.iter().any(|(existing, _)| *existing == hotkey) {
                    continue;
                }
                hotkeys.push((hotkey, item.command));
            }
        }
        hotkeys
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insert a top-level menu at `index` (clamped to the end).
    pub fn insert(&mut self, index: usize, label: impl Into<String>, menu: Menu) -> &mut Self {
        let index = index.min(self.entries.len());
        self.entries.insert(index, MenuBarEntry::new(label, menu));
        self
    }

    /// Remove a top-level menu.
    pub fn remove(&mut self, index: usize) -> Option<MenuBarEntry> {
        (index < self.entries.len()).then(|| self.entries.remove(index))
    }

    /// Replace one top-level menu.
    pub fn set_menu(&mut self, index: usize, menu: Menu) -> bool {
        match self.entries.get_mut(index) {
            Some(entry) => {
                entry.set_menu(menu);
                true
            }
            None => false,
        }
    }

    /// The mutable item with `command`, searching every top-level menu.
    pub fn item_mut(&mut self, command: u32) -> Option<&mut MenuItem> {
        self.entries
            .iter_mut()
            .find_map(|entry| entry.menu.item_mut(command))
    }

    /// Check/uncheck an item by command id (a live settings menu).
    pub fn set_checked(&mut self, command: u32, checked: bool) -> bool {
        self.entries
            .iter_mut()
            .any(|entry| entry.menu.set_checked(command, checked))
    }

    /// Enable/disable an item by command id.
    pub fn set_enabled(&mut self, command: u32, enabled: bool) -> bool {
        self.entries
            .iter_mut()
            .any(|entry| entry.menu.set_enabled(command, enabled))
    }

    /// The top-level index whose `&` access key matches `ch` (case
    /// insensitive). Owner-drawn titles carry no text the shell could parse,
    /// so the host resolves `Alt+key` through this lookup.
    pub fn mnemonic_index(&self, ch: char) -> Option<usize> {
        self.mnemonic_indices(ch).first().copied()
    }

    /// Every top-level index whose access key matches `ch`, in bar order.
    /// Native menu bars cycle through the matches when the same letter is
    /// pressed repeatedly.
    pub fn mnemonic_indices(&self, ch: char) -> Vec<usize> {
        let ch = ch.to_ascii_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| access_key(entry.label()) == Some(ch))
            .map(|(index, _)| index)
            .collect()
    }

    /// Rename an item by command id.
    pub fn set_label(&mut self, command: u32, label: impl Into<String>) -> bool {
        let label = label.into();
        self.entries
            .iter_mut()
            .find(|entry| entry.menu.command(command).is_some())
            .is_some_and(|entry| entry.menu.set_label(command, label.clone()))
    }
}

/// Which kind of popup a [`MenuEvent`] belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuSource {
    /// A context menu (secondary click, menu key, `Shift+F10`).
    ContextMenu,
    /// A chrome widget's own menu (menu bar, menu button, split button,
    /// toolbar overflow).
    Chrome,
    /// A menu the application opened through `Ui::open_menu` /
    /// `WindowHandle::open_menu`.
    Programmatic,
    /// The notification-area icon's menu.
    Tray,
    /// The native window menu bar (`WindowBuilder::menu_bar`): its top-level
    /// menus are system popups, so only `Selected` carries a command.
    MenuBar,
}

impl MenuSource {
    pub fn label(self) -> &'static str {
        match self {
            MenuSource::ContextMenu => "context",
            MenuSource::Chrome => "chrome",
            MenuSource::Programmatic => "programmatic",
            MenuSource::Tray => "tray",
            MenuSource::MenuBar => "menu-bar",
        }
    }
}

/// What a menu hook decided about the event it was shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuDecision {
    /// Deliver the event to the next handler (the popup selection is replayed
    /// through `Host::menu_command` / `Host::tray_event`).
    #[default]
    Pass,
    /// Consume the event. An `Opening` event cancels the popup; a `Selected`
    /// event suppresses the default replay.
    Handled,
}

/// One step in a native menu's life, delivered to [`crate::Host::menu_event`].
///
/// The order is `Opening` → `Opened` → `Selected` (a command was chosen; it can
/// be rewritten in place) → `Closed` (the popup is gone, carrying the final
/// command or `None`). `Opened` is dispatched from the owner window's message
/// loop *while the popup is up*, so its model can still be edited: the host
/// closes the popup and rebuilds it from the edited menu (the hook sees a fresh
/// `Opened` for the rebuilt popup). Return [`MenuDecision::Handled`] to cancel
/// an `Opening` event or suppress the replay of a `Selected` one.
pub enum MenuEvent<'a> {
    /// The popup is about to be built. `menu` is the model that will be shown:
    /// edit it in place (add/remove/rename/check items) or return `Handled`
    /// to cancel the popup. An empty menu is never shown, so this is also the
    /// place to install "no commands available" state.
    Opening {
        widget: Option<WidgetId>,
        source: MenuSource,
        /// Anchor in client DIPs (the cursor point for a tray menu).
        anchor: Point,
        menu: &'a mut Menu,
    },
    /// The popup is up. `menu` is the live model: an edit in place (or a
    /// `WindowHandle::edit_open_menu` from a timer) closes and rebuilds the
    /// popup, and this event fires again for the rebuilt menu. An unchanged
    /// model leaves the popup alone.
    Opened {
        widget: Option<WidgetId>,
        source: MenuSource,
        menu: &'a mut Menu,
    },
    /// The user chose `command` (or a hook is replaying one). Rewrite it
    /// through the mutable reference to alias commands; `Handled` suppresses
    /// the default replay.
    Selected {
        widget: Option<WidgetId>,
        source: MenuSource,
        command: &'a mut u32,
    },
    /// The popup closed. `command` is the (possibly rewritten) selection, or
    /// `None` when it was dismissed or canceled.
    Closed {
        widget: Option<WidgetId>,
        source: MenuSource,
        command: Option<u32>,
    },
}

impl MenuEvent<'_> {
    pub fn widget(&self) -> Option<WidgetId> {
        match self {
            MenuEvent::Opening { widget, .. }
            | MenuEvent::Opened { widget, .. }
            | MenuEvent::Selected { widget, .. }
            | MenuEvent::Closed { widget, .. } => *widget,
        }
    }

    pub fn source(&self) -> MenuSource {
        match self {
            MenuEvent::Opening { source, .. }
            | MenuEvent::Opened { source, .. }
            | MenuEvent::Selected { source, .. }
            | MenuEvent::Closed { source, .. } => *source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_keeps_visual_order_and_nesting() {
        let menu = Menu::new()
            .item(1, "Play")
            .item(2, "Queue")
            .separator()
            .submenu(
                "More",
                Menu::new().item(3, "Copy").disabled_item(4, "Properties"),
            )
            .checked_item(5, "Shuffle", true);

        assert_eq!(menu.len(), 5);
        assert!(matches!(menu.entries()[2], MenuEntry::Separator));
        let MenuEntry::Submenu(submenu) = &menu.entries()[3] else {
            panic!("expected a submenu entry");
        };
        assert_eq!(submenu.label(), "More");
        assert_eq!(submenu.menu().len(), 2);
        assert!(menu.command(4).unwrap().is_disabled());
        assert!(menu.command(5).unwrap().is_checked());
        assert!(!menu.command(1).unwrap().is_checked());
    }

    #[test]
    fn command_lookup_searches_submenus() {
        let menu = Menu::new()
            .item(10, "Top")
            .submenu("Nested", Menu::new().item(11, "Inner"));
        assert_eq!(menu.command(11).map(MenuItem::label), Some("Inner"));
        assert!(menu.command(12).is_none());
    }

    #[test]
    fn visit_items_skips_separators_and_reaches_submenus() {
        let menu = Menu::new()
            .item(1, "A")
            .separator()
            .submenu("S", Menu::new().item(2, "B"));
        let mut commands = Vec::new();
        menu.visit_items(&mut |item| commands.push(item.command()));
        assert_eq!(commands, vec![1, 2]);
    }

    #[test]
    fn entry_helpers_apply_to_the_right_variant() {
        let item = MenuEntry::item(7, "Item").checked(true).disabled(true);
        let MenuEntry::Item(item) = item else {
            panic!("expected an item");
        };
        assert!(item.is_checked() && item.is_disabled());

        let submenu = MenuEntry::submenu("S", Menu::new()).disabled(true);
        let MenuEntry::Submenu(submenu) = submenu else {
            panic!("expected a submenu");
        };
        assert!(submenu.is_disabled());

        // Separators tolerate both setters and stay separators.
        assert_eq!(
            MenuEntry::separator().checked(true).disabled(true),
            MenuEntry::Separator
        );
    }

    #[test]
    fn empty_menu_reports_empty() {
        let menu = Menu::new();
        assert!(menu.is_empty());
        assert_eq!(menu.len(), 0);
    }

    #[test]
    fn items_can_be_inserted_removed_and_edited_in_place() {
        let mut menu = Menu::new().item(1, "Play").item(2, "Queue");
        menu.insert(1, MenuEntry::separator());
        menu.insert(99, MenuEntry::item(3, "Last"));
        assert_eq!(menu.len(), 4);
        assert!(matches!(menu.entries()[1], MenuEntry::Separator));

        assert!(menu.set_label(1, "Resume"));
        assert!(menu.set_checked(2, true));
        assert!(menu.set_enabled(2, false));
        assert!(!menu.set_label(404, "missing"));
        assert_eq!(menu.command(1).map(MenuItem::label), Some("Resume"));
        assert!(menu.command(2).unwrap().is_checked());
        assert!(menu.command(2).unwrap().is_disabled());

        assert_eq!(menu.remove(1), Some(MenuEntry::Separator));
        let removed = menu.remove_command(3).expect("command 3 removed");
        assert_eq!(removed.label(), "Last");
        assert!(menu.remove_command(3).is_none());
        assert_eq!(menu.len(), 2);
    }

    #[test]
    fn command_edits_reach_items_inside_submenus() {
        let mut menu = Menu::new()
            .item(1, "Top")
            .submenu("More", Menu::new().item(2, "Inner").item(3, "Deep"));
        assert!(menu.set_checked(2, true));
        assert!(menu.set_command(3, 33));
        assert!(menu.command(3).is_none());
        assert_eq!(menu.command(33).map(MenuItem::label), Some("Deep"));
        assert!(menu.remove_command(2).is_some());
        assert_eq!(menu.submenu_at(&MenuPath::root().label("More")).map(Menu::len), Some(1));
    }

    #[test]
    fn paths_address_entries_by_position_or_label() {
        let mut menu = Menu::new()
            .item(1, "File")
            .submenu(
                "More",
                Menu::new()
                    .item(2, "Copy")
                    .submenu("Nested", Menu::new().item(3, "Inner")),
            );
        let path = MenuPath::root().label("More").index(1);
        assert_eq!(
            menu.submenu_at(&path).map(|sub| sub.command(3).map(MenuItem::label)),
            Some(Some("Inner"))
        );
        // The empty path is the root menu.
        assert_eq!(menu.submenu_at(&MenuPath::root()).map(Menu::len), Some(2));

        let entry = menu.entry_mut(&MenuPath::root().index(1).index(0));
        let entry = entry.expect("second top-level entry, first child");
        assert!(entry.set_label("Copied"));
        assert_eq!(menu.command(2).map(MenuItem::label), Some("Copied"));

        // A path through an item (not a submenu) or past the end is invalid.
        assert!(menu.submenu_at(&MenuPath::root().index(0)).is_none());
        assert!(menu.submenu_at(&MenuPath::root().index(9)).is_none());
        assert!(menu.entry_mut(&MenuPath::root()).is_none());
    }

    #[test]
    fn a_menu_bar_resolves_alt_keys_from_its_labels() {
        let bar = MenuBarModel::new()
            .item("程序(&P)", Menu::new().item(1, "A"))
            .item("帮助(&H)", Menu::new().item(2, "B"))
            .item("文件(&F) &&", Menu::new().item(3, "C"));
        assert_eq!(bar.mnemonic_index('p'), Some(0));
        assert_eq!(bar.mnemonic_index('P'), Some(0));
        assert_eq!(bar.mnemonic_index('h'), Some(1));
        // The first `&` is the key; a later `&&` is a literal ampersand.
        assert_eq!(bar.mnemonic_index('f'), Some(2));
        assert_eq!(bar.mnemonic_index('&'), None);
        assert_eq!(bar.mnemonic_index('x'), None);
    }

    #[test]
    fn a_menu_bar_publishes_window_accelerators_from_shortcut_labels() {
        let bar = MenuBarModel::new()
            .item(
                "程序(&P)",
                Menu::new()
                    .shortcut_item(1, "播放", "Ctrl+P")
                    .shortcut_item(2, "打开", "ctrl+o")
                    .push(MenuItem::new(3, "禁用项").shortcut("Ctrl+D").disabled(true))
                    .item(4, "普通项")
                    .submenu("更多", Menu::new().shortcut_item(5, "子项", "Ctrl+S")),
            )
            .separator()
            .item("帮助(&H)", Menu::new().shortcut_item(6, "关于", "F1"));
        let hotkeys = bar.hotkeys();
        assert_eq!(hotkeys.len(), 3, "{hotkeys:?}");
        assert_eq!(hotkeys[0], (Hotkey::parse("Ctrl+P").unwrap(), 1));
        assert_eq!(hotkeys[1], (Hotkey::parse("Ctrl+O").unwrap(), 2));
        assert_eq!(hotkeys[2], (Hotkey::parse("F1").unwrap(), 6));
        // A bare printable label is not a chord; the column still shows it.
        let item = MenuItem::new(7, "提醒").shortcut("Q");
        assert!(item.hotkey().is_none());
        assert_eq!(item.shortcut_text(), Some("Q"));
        // Titles and separators own no accelerators.
        assert!(MenuBarModel::new().item("空", Menu::new()).hotkeys().is_empty());
    }

    #[test]
    fn menu_event_reports_widget_and_source() {
        let mut menu = Menu::new().item(1, "One");
        let mut command = 1u32;
        let opening = MenuEvent::Opening {
            widget: Some(WidgetId::new("w")),
            source: MenuSource::Chrome,
            anchor: Point::new(3.0, 4.0),
            menu: &mut menu,
        };
        assert_eq!(opening.widget(), Some(WidgetId::new("w")));
        assert_eq!(opening.source(), MenuSource::Chrome);

        let selected = MenuEvent::Selected {
            widget: None,
            source: MenuSource::Tray,
            command: &mut command,
        };
        assert_eq!(selected.widget(), None);
        assert_eq!(selected.source(), MenuSource::Tray);
        assert_eq!(MenuDecision::default(), MenuDecision::Pass);
        assert_eq!(MenuSource::Chrome.label(), "chrome");
    }
}
