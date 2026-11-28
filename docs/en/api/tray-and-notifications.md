# Tray icons and notifications

> Notification-area icons with menus, and system toasts that fall back to a tray balloon.

## Overview

A window may install several notification-area (tray) icons.
`WindowHandle::set_tray_icon` manages the primary icon (id `0`);
`WindowHandle::set_tray_icon_at` manages any further id. The host owns the
`NOTIFYICONDATAW` registrations, the menu popups and the taskbar-restart re-add.
Clicks and menu selections arrive as `TrayEvent` carrying the icon id, on the UI
thread.

`Notification` is the application-facing description of one toast;
`WindowHandle::notify` posts it through the Windows notification platform
(WinRT `ToastNotificationManager`). When the toast cannot be shown (an
unpackaged app without a registered AUMID, or the platform unavailable), the
window falls back to a tray-icon balloon when one is installed.

`TrayIcon`, `Menu`, `Notification` and `NotificationResult` are cross-platform
models; the tray registration, `notify` and `WindowHandle` methods are
Windows-only.

## Key types

### `TrayIcon`

```rust
pub struct TrayIcon { /* private */ }

impl TrayIcon {
    pub fn new(tooltip: impl Into<String>) -> Self; // app icon, no menu
}
```

Builders: `tooltip(text)`, `menu(Menu)`, `icon_file(path)`. Getters:
`tooltip_text()`, `menu_model() -> &Menu`, `image() -> &TrayImage`. The host
truncates the tooltip to the native 127-character limit.

### `TrayImage` / `TrayAction` / `TrayEvent`

```rust
pub enum TrayImage { Default, File(String) }

pub enum TrayAction { Click, DoubleClick, RightClick, MiddleClick, KeySelect }

pub enum TrayEvent {
    Action { icon: u32, action: TrayAction },
    Command { icon: u32, command: u32 },
}
```

`TrayImage::Default` is the system application icon; `File` loads an `.ico` and
falls back to the default on failure. `Command` carries a nonzero id from the
icon's `Menu`. Both variants run on the UI thread from the notification message,
never while a `Host` callback is in progress.

### `Notification`

```rust
pub struct Notification { /* private */ }

impl Notification {
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self;
}
```

Builders: `body`, `app_id(id)`, `tag(tag)`, `group(group)`, `launch(arguments)`,
`silent(bool)`. Getters: `title_text`, `body_text`, `app_id_str`, `tag_text`,
`group_text`, `launch_text`, `is_silent`.

Helpers: `Notification::escape(text) -> String` (XML escaping) and
`to_toast_xml() -> String` (the `ToastGeneric` payload). `tag` + `group` let the
application replace or remove a notification in the Action Center. `launch`
passes arguments that the *next* launch reads from the command line; toasts are
visual only, so there is no in-process activation callback.

### `NotificationResult`

```rust
pub enum NotificationResult { Toast, Balloon, Failed }
```

`shown()` is `true` for `Toast` and `Balloon`.

### `WindowHandle` and `Host`

| API | Signature |
|---|---|
| `set_tray_icon` | `set_tray_icon(&mut self, tray: Option<TrayIcon>)` |
| `set_tray_icon_at` | `set_tray_icon_at(&mut self, id: u32, tray: Option<TrayIcon>) -> bool` |
| `tray_icon_visible` | `tray_icon_visible(&self) -> bool` |
| `notify` | `notify(&mut self, notification: &Notification) -> NotificationResult` |
| `show_tray_balloon` | `show_tray_balloon(&mut self, title, text, icon: MessageIcon) -> bool` |
| `show_tray_balloon_on` | `show_tray_balloon_on(&mut self, id, title, text, icon) -> bool` |
| `register_notification_shortcut` | `register_notification_shortcut(&self, display_name: &str) -> bool` |
| `WindowBuilder::app_id` | `app_id(mut self, id: impl Into<String>) -> Self` |
| `Host::tray_event` | `fn tray_event(&mut self, event: TrayEvent, window: &mut WindowHandle<'_>)` |

## Example

Constructing a tray icon with a small menu:

```rust
use bamboo_native_ui::{Menu, TrayIcon};

fn install_tray(window: &mut bamboo_native_ui::WindowHandle<'_>) {
    let menu = Menu::new()
        .item(401, "Show window")
        .item(402, "Next track")
        .separator()
        .item(403, "Quit");
    window.set_tray_icon(Some(
        TrayIcon::new("Bamboo Player")
            .tooltip("Bamboo Player — idle")
            .menu(menu)
            .icon_file("assets\\app.ico"),
    ));
}
```

Handling its events and showing a notification:

```rust
use bamboo_native_ui::{Notification, TrayAction, TrayEvent, WindowHandle};

fn tray_event(&mut self, event: TrayEvent, window: &mut WindowHandle<'_>) {
    match event {
        TrayEvent::Action { action: TrayAction::Click, .. } => window.show(),
        TrayEvent::Command { command: 403, .. } => window.quit(0),
        _ => {}
    }
}

fn track_changed(&mut self, title: &str, window: &mut WindowHandle<'_>) {
    let notification = Notification::new("Now playing", title).tag("track");
    let _ = window.notify(&notification);
}
```

## Notes and limits

- The tray registration and `notify` are Windows-only. `TrayIcon`,
  `Notification` and `NotificationResult` are plain models available everywhere.
- `set_tray_icon` / `set_tray_icon_at` update an existing icon in place; pass
  `None` to remove it. Icons are removed when the window is destroyed and
  re-added if Explorer restarts.
- A right click still shows the icon's menu after `TrayEvent::Action {
  RightClick }`; the notification lets the application update the menu first.
- `notify` prefers the per-notification `app_id`, then the window's
  `WindowBuilder::app_id`, then the process identity. An unpackaged desktop app
  must call `register_notification_shortcut` once (it writes a Start-menu
  shortcut carrying the AUMID) before toasts can appear.
- `notify` must run on the window thread; WinRT is initialized there.
- Toasts are visual only: clicking one starts the application through the shell,
  so use `launch` for arguments the next launch reads.

## See also

- `menus.md` (the tray menu model)
- `dialogs.md` (`MessageIcon` used by balloons)
- `accessibility.md` (announcements for screen readers)
- `../quickstart/03-first-window.md`
