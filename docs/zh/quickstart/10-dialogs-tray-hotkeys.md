# 10 — 对话框、托盘与热键

> 原生消息和文件对话框、一个托盘图标、toast 以及全局快捷键。

## 你将构建

[03](03-first-window.md) 中的问候窗口获得一条集成带。
**Remove history** 询问一个是/否问题，**Choose file** 打开 shell
选择器，一个托盘图标恢复窗口并提供菜单，选择文件后发布
一条通知，`Ctrl+Shift+M` 在 Windows 的任意位置计数点击。
**Change shortcut** 捕获下一个组合键并注册它。

结果是异步的：`show_dialog` 返回一个 `DialogId`，答案
到达 `Host::dialog_finished`。托盘和热键回调在 UI 线程上、
穿插在其他回调之间运行。

## 前置条件

- [03](03-first-window.md) 中的项目可以构建并运行。
- 无需额外特性；这些是默认 Windows 宿主的一部分。

## 步骤

### 1. 导入集成类型

```rust
use bamboo_native_ui::{
    DialogId, DialogResult, FileDialog, FileFilter, Host, Hotkey, HotkeyModifiers,
    KeyCode, Menu, MessageAnswer, MessageButtons, MessageDefault, MessageDialog, MessageIcon,
    Notification, NotificationResult, TrayAction, TrayEvent, TrayIcon, TrayImage, Ui, WidgetId,
    WindowHandle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    ChooseFile,
    Remove,
    Capture,
}
```

应用传递描述；宿主将它们转换为 Win32 对象。

### 2. 扩展应用状态

保留第 03 页的 `paint`、`resize` 和 `event`。将 `Msg::ChooseFile`、
`Remove` 和 `Capture` 从 `apply` 路由到 `open_file`、`open_remove` 和
`begin_hotkey_capture`。将 `Host` 方法放在 `impl Host for App` 中，将
`notify` 等辅助函数放在 `impl App` 中。

```rust
struct App {
    ui: Ui<Msg>,
    clicks: u32,
    confirm: Option<DialogId>,
    hotkey: Option<Hotkey>,
}

impl App {
    fn status(&mut self, text: String, window: &mut WindowHandle<'_>) {
        self.ui.set_label(WidgetId::new("status"), text);
        // Dialog/tray callbacks run outside `event`, so flush here.
        self.ui.flush_damage(window);
    }
}
```

### 3. 用 `MessageDialog` 提问

`MessageDialog::new(text)` 是一个模态的 `OK` 框；`.icon`、`.buttons` 和
`.default_button` 选择字形、按钮和焦点。`.parallel()` 释放所有者。

```rust
fn open_remove(&mut self, window: &mut WindowHandle<'_>) {
    let dialog = MessageDialog::new("Clear the click history?")
        .title("Bamboo Hello").icon(MessageIcon::Warning)
        .buttons(MessageButtons::YesNo).default_button(MessageDefault::Second);
    self.confirm = Some(window.show_dialog(dialog));
}
```

答案是 `Option<MessageAnswer>`；`None` 表示关闭。一个
`.button(label, command)` 使其成为报告 `Custom(command)` 的任务对话框。

### 4. 选择文件或文件夹

```rust
fn open_file(&mut self, window: &mut WindowHandle<'_>) {
    window.show_dialog(
        FileDialog::open()
            .title("Open audio")
            .filter("Audio", "*.mp3;*.flac;*.wav")
            .filter("All files", "*.*")
            .initial_dir("C:\\Music")
            .initial_name("track.mp3"),
    );
}
```

`FileDialog::open`/`::save`/`::folder` 构建三个选择器。`.filter` 接受
`FileFilter::new(label, pattern)` 这样的一对参数；`.multi_select(true)` 使打开
选择器报告 `DialogResult::Files`。

### 5. 安装托盘图标并注册热键

在 `started` 中窗口存在后安装图标。菜单复用
控件 `Menu` 模型及其非零应用命令。

```rust
fn started(&mut self, window: &mut WindowHandle<'_>) {
    let menu = Menu::new().item(1, "Show window").separator().item(2, "Quit");
    window.set_tray_icon(Some(TrayIcon::new("Bamboo Hello").menu(menu)));

    let chord = Hotkey::parse("Ctrl+Shift+M").expect("a valid chord");
    self.hotkey = Some(chord);
    let _ = window.register_hotkey(chord);
    // A window shortcut needs no system registration; it reports command 301.
    let _ = window.register_window_hotkey(Hotkey::parse("Ctrl+K").unwrap(), 301);
}

fn tray_event(&mut self, event: TrayEvent, window: &mut WindowHandle<'_>) {
    match event {
        TrayEvent::Action { action: TrayAction::Click | TrayAction::KeySelect, .. } => {
            window.show()
        }
        TrayEvent::Action { .. } => {} // DoubleClick, RightClick, MiddleClick
        TrayEvent::Command { command: 1, .. } => window.show(),
        TrayEvent::Command { command: 2, .. } => window.quit(0),
        TrayEvent::Command { .. } => {}
    }
}
```

`TrayIcon::new` 保留 `TrayImage::Default`；`.icon_file(path)` 设置
`TrayImage::File`。`set_tray_icon_at` 添加第二个图标；从
`Host::close_requested` 返回 `false` 以留在托盘中。

### 6. 解析对话框并发布通知

```rust
fn dialog_finished(&mut self, id: DialogId, result: DialogResult, window: &mut WindowHandle<'_>) {
    match (Some(id) == self.confirm, result) {
        (true, DialogResult::Message(Some(MessageAnswer::Yes))) => {
            self.clicks = 0;
            self.confirm = None;
            self.status("History cleared".to_string(), window);
        }
        (true, _) => {
            self.confirm = None;
            self.status("History kept".to_string(), window);
        }
        (false, DialogResult::File(Some(path))) => {
            self.status(format!("Chosen: {path}"), window);
            self.notify(&path, window);
        }
        (false, _) => {}
    }
}

fn notify(&mut self, path: &str, window: &mut WindowHandle<'_>) {
    let outcome = window.notify(&Notification::new("Track queued", path));
    let text = match outcome {
        NotificationResult::Toast => "Toast shown",
        NotificationResult::Balloon => "Tray balloon shown",
        NotificationResult::Failed => "No notifier",
    };
    self.status(text.to_string(), window);
}
```

未打包的进程需要一个 AppUserModelID（`WindowBuilder::app_id` 加上
`window.register_notification_shortcut`）。当 toast 被拒绝时，`notify`
回退到托盘图标上的气泡（第 5 步安装了一个）；
`NotificationResult::shown()` 测试“任一路径是否奏效”。

### 7. 注册、响应并捕获快捷键

`Host::hotkey` 在窗口存在时触发，即使另一个应用拥有
焦点；系统拥有该组合键时 `register_hotkey` 返回 `false`。

```rust
fn hotkey(&mut self, hotkey: Hotkey, window: &mut WindowHandle<'_>) {
    if Some(hotkey) == self.hotkey {
        self.clicks += 1;
        self.status(format!("{} · {} click(s)", hotkey.label(), self.clicks), window);
    }
}

fn menu_bar_command(&mut self, command: u32, window: &mut WindowHandle<'_>) {
    if command == 301 {
        self.ui.set_focus(WidgetId::new("greet"), window);
    }
}
```

`Hotkey::new(HotkeyModifiers::CTRL.with_shift(), KeyCode::Letter(12))` 返回
`Option<Hotkey>`；`Letter(12)` 即 `M`。`parse` 拒绝裸露的可打印键，
因为全局的 `A` 会在任何地方吞掉普通输入。

要替换组合键，`begin_hotkey_capture` 消费下一个非修饰键
并通过 `Host::hotkey_captured` 报告它（Esc、焦点丢失或
`cancel_hotkey_capture` 报告 `None`）；不会自动注册任何内容。

```rust
fn hotkey_captured(&mut self, hotkey: Option<Hotkey>, window: &mut WindowHandle<'_>) {
    let Some(chord) = hotkey else {
        self.status("Capture canceled".to_string(), window);
        return;
    };
    let shown = chord.label();
    let ok = window.register_hotkey(chord);
    if ok { self.hotkey = Some(chord); }
    let text = if ok { format!("Registered {shown}") } else { format!("{shown} is taken") };
    self.status(text, window);
}
```

从 `Msg::Capture` 处理器调用 `window.begin_hotkey_capture()`。保留
旧组合键直到新组合键被接受，或先调用 `unregister_hotkey` 以
释放它；两次注册同一组合键是一个无操作，返回 `true`。

## 预期结果

运行 `cargo run` 并在另一个窗口获得焦点时按下 `Ctrl+Shift+M`
会计数一次点击。**Remove history** → Yes 清除它。**Choose file** 显示
路径并发布一个 toast（或托盘气泡）。托盘的左键单击恢复由
`close_requested` 隐藏的窗口。

- 一个模态警告，第二个按钮获得焦点；Yes 清除，其他任何操作
  保留计数。
- shell 选择器以音频过滤器打开；所选路径被显示。
- 一个带 Show/Quit 的托盘图标；`Ctrl+Shift+M` 全局有效，`Ctrl+K`
  在窗口活动时将焦点置于第一个按钮。
- 选择文件后出现一个 toast 或气泡；**Change shortcut** 安装
  新的全局组合键。

## 故障排查

- **`dialog_finished` 从不运行** —— 从回调中显示对话框，而不是在循环启动之前。
- **`register_hotkey` 返回 `false`** —— 另一个进程拥有该组合键；`registered_hotkeys()` 列出你的。
- **`Hotkey::parse` 返回 `None`** —— 没有修饰键且不是功能键、两个
  键（`Ctrl+A+B`），或一个未知键名。
- **没有 toast 或气泡** —— 设置 `WindowBuilder::app_id`、注册快捷键，并安装一个托盘图标。
- **没有托盘图标** —— `set_tray_icon` 在窗口存在之前运行了。从
  `started` 调用它。

## 下一步

[11 — 主题与图像](11-theming-and-images.md)：切换浅色和深色，
加载 `.buitheme` 调色板并将渐变馈送给头像和封面墙。
