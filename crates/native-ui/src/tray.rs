// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Notification-area (tray) icon configuration.
//!
//! A window may install several icons: [`crate::WindowHandle::set_tray_icon`]
//! manages the primary one (id 0) and
//! [`crate::WindowHandle::set_tray_icon_at`] any further id. The host owns the
//! `NOTIFYICONDATAW` registrations, the menu popups and the taskbar-restart
//! re-add. Clicks and menu selections arrive as [`TrayEvent`] carrying the
//! icon id, on the UI thread.
//!
//! The tray menu reuses the same [`Menu`] model as widgets, so commands are
//! the same nonzero application ids.

use crate::menu::Menu;

/// Where the icon image comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayImage {
    /// The system application icon.
    Default,
    /// An icon file loaded with `LoadImageW` (`.ico`); a load failure falls
    /// back to the default icon.
    File(String),
}

/// The tray icon description: tooltip, menu and image.
///
/// The host truncates the tooltip to the native 127-character limit.
#[derive(Clone, Debug, PartialEq)]
pub struct TrayIcon {
    pub(crate) tooltip: String,
    pub(crate) menu: Menu,
    pub(crate) image: TrayImage,
}

impl TrayIcon {
    /// A tray icon with the system application icon and no menu.
    pub fn new(tooltip: impl Into<String>) -> Self {
        Self {
            tooltip: tooltip.into(),
            menu: Menu::new(),
            image: TrayImage::Default,
        }
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = tooltip.into();
        self
    }

    /// Menu shown on right click / the keyboard menu key on the icon.
    pub fn menu(mut self, menu: Menu) -> Self {
        self.menu = menu;
        self
    }

    /// Load the image from an icon file instead of the application icon.
    pub fn icon_file(mut self, path: impl Into<String>) -> Self {
        self.image = TrayImage::File(path.into());
        self
    }

    pub fn tooltip_text(&self) -> &str {
        &self.tooltip
    }

    pub fn menu_model(&self) -> &Menu {
        &self.menu
    }

    pub fn image(&self) -> &TrayImage {
        &self.image
    }
}

/// The mouse/keyboard action on a tray icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    /// Left button up (a single click).
    Click,
    /// Left double click.
    DoubleClick,
    /// Right button up. The icon's menu is still shown afterwards; the
    /// notification lets the application update it first or react on its own.
    RightClick,
    /// Middle button up.
    MiddleClick,
    /// The icon was selected with the keyboard (Enter/Space, or a shell
    /// accessibility action).
    KeySelect,
}

/// What happened to a tray icon. Every variant carries the icon's application
/// id (0 is the primary icon). Both run on the UI thread, from the
/// notification message, never while a `Host` callback is in progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayEvent {
    /// A mouse or keyboard action on the icon.
    Action { icon: u32, action: TrayAction },
    /// A command chosen from the icon's menu (nonzero id from its [`Menu`]).
    Command { icon: u32, command: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_keeps_tooltip_menu_and_image() {
        let tray = TrayIcon::new("Bamboo")
            .tooltip("正在播放")
            .menu(Menu::new().item(1, "显示").separator().item(2, "退出"))
            .icon_file("app.ico");
        assert_eq!(tray.tooltip_text(), "正在播放");
        assert_eq!(tray.menu_model().len(), 3);
        assert_eq!(tray.image(), &TrayImage::File("app.ico".to_string()));
    }

    #[test]
    fn defaults_are_app_icon_and_no_menu() {
        let tray = TrayIcon::new("t");
        assert_eq!(tray.image(), &TrayImage::Default);
        assert!(tray.menu_model().is_empty());
    }

    #[test]
    fn events_compare_by_icon_action_and_value() {
        let click = TrayEvent::Action {
            icon: 0,
            action: TrayAction::Click,
        };
        assert_eq!(
            click,
            TrayEvent::Action {
                icon: 0,
                action: TrayAction::Click
            }
        );
        assert_ne!(
            click,
            TrayEvent::Action {
                icon: 0,
                action: TrayAction::DoubleClick
            }
        );
        assert_ne!(
            TrayEvent::Command { icon: 0, command: 1 },
            TrayEvent::Command { icon: 0, command: 2 }
        );
    }
}
