# 托盘图标与通知

> 带菜单的通知区域图标，以及会回退到托盘气泡的系统 Toast。

## 概述

一个窗口可以安装多个通知区域（托盘）图标。`WindowHandle::set_tray_icon` 管理主图标
（id `0`）；`WindowHandle::set_tray_icon_at` 管理任何其他 id。宿主拥有
`NOTIFYICONDATAW` 注册、菜单弹出以及任务栏重启后的重新添加。点击和菜单选择会以携带图标
id 的 `TrayEvent` 在 UI 线程上到达。

`Notification` 描述面向应用的单个 Toast；`WindowHandle::notify` 通过 Windows 通知平台
（WinRT `ToastNotificationManager`）投递它。Toast 无法显示时（未注册 AUMID 的未打包
应用，或平台不可用），若已安装托盘图标，窗口会回退到托盘图标气泡。

`TrayIcon`、`Menu`、`Notification` 和 `NotificationResult` 是跨平台模型；托盘注册、
`notify` 以及 `WindowHandle` 方法仅限 Windows。

## 关键类型

### `TrayIcon`

```rust
pub struct TrayIcon { /* private */ }

impl TrayIcon {
    pub fn new(tooltip: impl Into<String>) -> Self; // 应用图标，无菜单
}
```

构建方法：`tooltip(text)`、`menu(Menu)`、`icon_file(path)`。读取方法：
`tooltip_text()`、`menu_model() -> &Menu`、`image() -> &TrayImage`。宿主会将工具提示截断
到原生的 127 字符上限。

### `TrayImage` / `TrayAction` / `TrayEvent`

```rust
pub enum TrayImage { Default, File(String) }

pub enum TrayAction { Click, DoubleClick, RightClick, MiddleClick, KeySelect }

pub enum TrayEvent {
    Action { icon: u32, action: TrayAction },
    Command { icon: u32, command: u32 },
}
```

`TrayImage::Default` 是系统应用程序图标；`File` 加载一个 `.ico`，失败时回退到默认图标。
`Command` 携带来自该图标 `Menu` 的非零 id。两个变体都在 UI 线程上从通知消息运行，且从不在
`Host` 回调执行期间运行。

### `Notification`

```rust
pub struct Notification { /* private */ }

impl Notification {
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self;
}
```

构建方法：`body`、`app_id(id)`、`tag(tag)`、`group(group)`、`launch(arguments)`、
`silent(bool)`。读取方法：`title_text`、`body_text`、`app_id_str`、`tag_text`、
`group_text`、`launch_text`、`is_silent`。

辅助方法：`Notification::escape(text) -> String`（XML 转义）以及
`to_toast_xml() -> String`（`ToastGeneric` 负载）。`tag` + `group` 让应用能够在操作中心
替换或移除一个通知。`launch` 传递参数，由*下一次*启动从命令行读取；Toast 仅用于视觉呈现，
因此没有进程内激活回调。

### `NotificationResult`

```rust
pub enum NotificationResult { Toast, Balloon, Failed }
```

对于 `Toast` 和 `Balloon`，`shown()` 为 `true`。

### `WindowHandle` 与 `Host`

| API | 签名 |
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

## 示例

构建一个带小菜单的托盘图标：

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

处理其事件并显示一个通知：

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

## 说明与限制

- 托盘注册和 `notify` 仅限 Windows。`TrayIcon`、`Notification` 和 `NotificationResult` 是
  可在任何地方使用的纯模型。
- `set_tray_icon` / `set_tray_icon_at` 会就地更新现有图标；传入 `None` 可移除它。窗口销毁时
  图标会被移除，并在资源管理器重启后重新添加。
- 在 `TrayEvent::Action { RightClick }` 之后，右键点击仍会显示该图标的菜单；该通知让应用得
  以先更新菜单。
- `notify` 优先使用每个通知的 `app_id`，其次是窗口的 `WindowBuilder::app_id`，最后是进程
  标识。未打包的桌面应用必须调用一次 `register_notification_shortcut`（它会写入一个携带
  AUMID 的开始菜单快捷方式），Toast 才能出现。
- `notify` 必须在窗口线程上运行；WinRT 在那里初始化。
- Toast 仅用于视觉呈现：点击它会通过 Shell 启动应用，因此请用 `launch` 传递下一次启动要读取
  的参数。

## 另见

- `menus.md`（托盘菜单模型）
- `dialogs.md`（气泡使用的 `MessageIcon`）
- `accessibility.md`（面向屏幕阅读器的告知）
- `../quickstart/03-first-window.md`
