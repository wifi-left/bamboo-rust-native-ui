# 10 — 对话框、托盘与热键

> 原生消息和文件对话框、托盘图标、通知以及全局快捷键。

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

## 本节新术语

- **对话框** —— 应用为了提问或让用户做选择而打开的小窗口。**原生对话框**
  由 Windows 自己绘制（标准的 yes/no 框或文件选择器），因此与系统其余部分
  风格一致。
- **模态** —— 模态对话框打开期间，它的所有者窗口在对话框被应答或关闭之前
  不接受点击。
- **异步结果** —— `show_dialog` 立即返回一个 `DialogId`（打开对话框的句柄，
  而不是答案）。答案稍后在 `dialog_finished` 回调中到达，因此对话框打开时
  应用仍在继续运行。
- **文件筛选器** —— 文件选择器中的模式列表（`*.mp3;*.flac`），决定提供
  哪些文件。
- **托盘图标** —— Windows 通知区域（时钟旁边）里的小图标。窗口隐藏时，
  它让应用仍然可以访问。
- **气泡提示** —— 指向托盘图标的旧式通知气泡；现代通知无法显示时，
  Windows 会回退到它。
- **通知 / toast** —— Windows 在屏幕角落显示的简短弹出提示。
- **热键** —— 应用响应的按键组合。**全局热键**在任何应用获得焦点时都有效；
  **窗口热键**仅在当前窗口活动时有效。**组合键**就是那个组合本身
  （`Ctrl+Shift+M`），**捕获**则是指请用户按下下一个组合键。

本页所有 Rust 代码都写入 crate 的 `src/main.rs`
（`bamboo-hello/bamboo-hello/src/main.rs`），与第 03 页是同一个文件。

## 步骤

### 1. 导入集成类型

```rust
// 对话框、托盘和热键：应用传入描述；宿主负责构建
// Win32 对象并在有结果时回调。
use bamboo_native_ui::{
    DialogId, DialogResult, FileDialog, FileFilter, Host, Hotkey, HotkeyModifiers,
    KeyCode, Menu, MessageAnswer, MessageButtons, MessageDefault, MessageDialog, MessageIcon,
    Notification, NotificationResult, TrayAction, TrayEvent, TrayIcon, TrayImage, Ui, WidgetId,
    WindowHandle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    ChooseFile, // “Choose file”按钮：打开 shell 选择器
    Remove,     // “Remove history”按钮：询问一个是/否问题
    Capture,    // “Change shortcut”按钮：捕获下一个组合键
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
    confirm: Option<DialogId>, // 当前打开的 yes/no 问题（如果有）
    hotkey: Option<Hotkey>,    // 当前注册的组合键
}

impl App {
    fn status(&mut self, text: String, window: &mut WindowHandle<'_>) {
        self.ui.set_label(WidgetId::new("status"), text);
        // 对话框/托盘回调在 `event` 之外运行，所以在这里刷新。
        self.ui.flush_damage(window);
    }
}
```

### 3. 用 `MessageDialog` 提问

`MessageDialog::new(text)` 是一个模态的 `OK` 框；`.icon`、`.buttons` 和
`.default_button` 选择字形、按钮和焦点。`.parallel()` 释放所有者。

```rust
fn open_remove(&mut self, window: &mut WindowHandle<'_>) {
    // 这是一份描述，而不是 Win32 句柄：标题、字形、按钮和初始焦点。
    let dialog = MessageDialog::new("Clear the click history?")
        .title("Bamboo Hello").icon(MessageIcon::Warning)
        .buttons(MessageButtons::YesNo).default_button(MessageDefault::Second);
    // 立即返回；答案稍后在 `dialog_finished` 中到达。
    self.confirm = Some(window.show_dialog(dialog));
}
```

答案是 `Option<MessageAnswer>`；`None` 表示关闭。一个
`.button(label, command)` 使其成为报告 `Custom(command)` 的任务对话框。

**刚才发生了什么** —— 点击 **Remove history** 运行了 `open_remove`，它描述了
这个问题并交给宿主。调用立刻返回一个 `DialogId`，因此点击处理器在对话框仍然
显示在屏幕上时就结束了。用户应答时，宿主用同一个 id 调用
`Host::dialog_finished`；应用把它与 `self.confirm` 比较，就知道这个答案属于
哪个对话框。

### 4. 选择文件或文件夹

```rust
fn open_file(&mut self, window: &mut WindowHandle<'_>) {
    window.show_dialog(
        FileDialog::open() // “打开”选择器；`::save`/`::folder` 是另外两个
            .title("Open audio")
            // 只有在“Audio”筛选器下才会提供这些扩展名。
            .filter("Audio", "*.mp3;*.flac;*.wav")
            .filter("All files", "*.*")
            .initial_dir("C:\\Music") // 选择器启动时所在的文件夹
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
    // 托盘菜单：命令 1 和 2 会在 `tray_event` 中返回。
    let menu = Menu::new().item(1, "Show window").separator().item(2, "Quit");
    window.set_tray_icon(Some(TrayIcon::new("Bamboo Hello").menu(menu)));

    let chord = Hotkey::parse("Ctrl+Shift+M").expect("a valid chord");
    self.hotkey = Some(chord);
    let _ = window.register_hotkey(chord); // 全局：其他应用获得焦点时也有效
    // 窗口快捷键无需系统注册；它报告命令 301。
    let _ = window.register_window_hotkey(Hotkey::parse("Ctrl+K").unwrap(), 301);
}

fn tray_event(&mut self, event: TrayEvent, window: &mut WindowHandle<'_>) {
    match event {
        // 左键单击或用键盘选中：把窗口带回来。
        TrayEvent::Action { action: TrayAction::Click | TrayAction::KeySelect, .. } => {
            window.show()
        }
        TrayEvent::Action { .. } => {} // DoubleClick、RightClick、MiddleClick
        TrayEvent::Command { command: 1, .. } => window.show(), // “Show window”
        TrayEvent::Command { command: 2, .. } => window.quit(0), // “Quit”
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
            // 在我们自己的确认框上选 Yes：清空计数器。
            self.clicks = 0;
            self.confirm = None;
            self.status("History cleared".to_string(), window);
        }
        (true, _) => {
            // 选 No，或对话框被关闭：计数器保持不变。
            self.confirm = None;
            self.status("History kept".to_string(), window);
        }
        (false, DialogResult::File(Some(path))) => {
            // 文件选择器（而不是确认框）返回了一个路径。
            self.status(format!("Chosen: {path}"), window);
            self.notify(&path, window);
        }
        (false, _) => {}
    }
}

fn notify(&mut self, path: &str, window: &mut WindowHandle<'_>) {
    // 先尝试 toast；Windows 可能拒绝，于是改用托盘气泡提示。
    let outcome = window.notify(&Notification::new("Track queued", path));
    let text = match outcome {
        NotificationResult::Toast => "Toast shown",      // 现代的角落弹出提示
        NotificationResult::Balloon => "Tray balloon shown", // 旧式气泡
        NotificationResult::Failed => "No notifier",
    };
    self.status(text.to_string(), window);
}
```

未打包的进程需要一个 AppUserModelID（`WindowBuilder::app_id` 加上
`window.register_notification_shortcut`）。当 toast 被拒绝时，`notify`
回退到托盘图标上的气泡提示（第 5 步安装了一个）；
`NotificationResult::shown()` 测试“任一路径是否奏效”。

### 7. 注册、响应并捕获快捷键

`Host::hotkey` 在窗口存在时触发，即使另一个应用拥有
焦点；系统拥有该组合键时 `register_hotkey` 返回 `false`。

```rust
fn hotkey(&mut self, hotkey: Hotkey, window: &mut WindowHandle<'_>) {
    if Some(hotkey) == self.hotkey {
        // 全局触发，即使另一个应用拥有焦点。
        self.clicks += 1;
        self.status(format!("{} · {} click(s)", hotkey.label(), self.clicks), window);
    }
}

fn menu_bar_command(&mut self, command: u32, window: &mut WindowHandle<'_>) {
    if command == 301 {
        // 窗口快捷键：把焦点移到第一个按钮。
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
        // Esc、焦点丢失或 `cancel_hotkey_capture` 会报告 `None`。
        self.status("Capture canceled".to_string(), window);
        return;
    };
    let shown = chord.label();
    let ok = window.register_hotkey(chord); // 被其他应用占用时为 false
    if ok { self.hotkey = Some(chord); }
    let text = if ok { format!("Registered {shown}") } else { format!("{shown} is taken") };
    self.status(text, window);
}
```

从 `Msg::Capture` 处理器调用 `window.begin_hotkey_capture()`。保留
旧组合键直到新组合键被接受，或先调用 `unregister_hotkey` 以
释放它；两次注册同一组合键是一个无操作，返回 `true`。

### 8. 运行它

从 crate 文件夹运行 —— 即内层的 `bamboo-hello/bamboo-hello`，也就是
存放 `src/main.rs` 的那个。外层的 `bamboo-hello` 文件夹也可以，因为它把
这个 crate 列为唯一的工作区成员。

```powershell
# 工作目录：bamboo-hello/bamboo-hello（含 src/main.rs 的 crate）
cargo run
```

一个标题为 “Bamboo Hello” 的窗口带着集成带出现。托盘图标
出现在通知区域；如果 Windows 把它藏起来了，点击时钟旁边的 `^` 折叠箭头。
即使另一个窗口拥有焦点，`Ctrl+Shift+M` 也会让状态标签中的点击计数
增加。成功启动会打印 Cargo 的
`Finished` 和 `Running` 行；编译错误会打印出错的行并返回
非零退出码。

**刚才发生了什么** —— Cargo 编译了 crate，`WindowBuilder::run` 进入
Windows 消息循环，`started` 安装了托盘图标和两个
快捷键。从那一刻起，消息循环把点击、热键和对话框结果投递给
你的 `Host` 方法。

## 预期结果

运行 `cargo run` 并在另一个窗口获得焦点时按下 `Ctrl+Shift+M`
会计数一次点击。**Remove history** → Yes 清除它。**Choose file** 显示
路径并发布一个通知（或托盘气泡提示）。托盘的左键单击恢复由
`close_requested` 隐藏的窗口。

- 一个模态警告，第二个按钮获得焦点；Yes 清除，其他任何操作
  保留计数。
- shell 选择器以音频筛选器打开；所选路径被显示。
- 一个带 Show/Quit 的托盘图标；`Ctrl+Shift+M` 全局有效，`Ctrl+K`
  在窗口活动时将焦点置于第一个按钮。
- 选择文件后出现一个通知或气泡提示；**Change shortcut** 安装
  新的全局组合键。

如果其中某一项始终不出现，故障排查中对应的条目会指出
常见原因。

## 故障排查

- **对话框从不出现** —— `show_dialog` 在消息循环启动之前被调用，或窗口
  被隐藏了。从按钮处理器或 `started` 之后运行的其他回调中打开对话框。
- **Choose file 打开了一个以上的选择器** —— 因为结果是异步的，每次点击都会
  再次调用 `show_dialog`。保存返回的 `DialogId`，或在选择器已打开时
  忽略点击。
- **`dialog_finished` 从不运行** —— 从回调中显示对话框，而不是在循环启动之前。
- **多文件选择被忽略** —— 使用 `.multi_select(true)` 时，结果是
  `DialogResult::Files`，而不是 `DialogResult::File(Some(..))`；两个分支都要处理。
- **`register_hotkey` 返回 `false`** —— 另一个进程拥有该组合键；`registered_hotkeys()` 列出你的。
- **热键不触发** —— 没有实现 `Host::hotkey`、组合键从未注册，或被另一个应用
  占用。与全局热键不同，窗口热键还要求这个窗口处于活动状态。
- **热键触发了但没有任何变化** —— 比较
  `Some(hotkey) == self.hotkey` 失败了；`hotkey` 可能仍保存着旧组合键。
  先注册新组合键，再存储它。
- **`Hotkey::parse` 返回 `None`** —— 没有修饰键且不是功能键、两个
  键（`Ctrl+A+B`），或一个未知键名。
- **点击 Change shortcut 后什么也没发生** —— 没有从 `Msg::Capture` 处理器
  调用 `begin_hotkey_capture`；Esc 会取消捕获。
- **没有通知或气泡提示** —— 设置 `WindowBuilder::app_id`、注册快捷键，并安装一个托盘图标。
- **通知被拒绝但没有气泡提示** —— 气泡提示需要一个托盘图标；
  只有 `set_tray_icon` 在 `started` 中运行过，`notify` 才会回退。
- **没有托盘图标** —— `set_tray_icon` 在窗口存在之前运行了。从
  `started` 调用它。Windows 也可能把新图标藏进溢出区域：打开 `^`
  折叠箭头找到它。
- **托盘图标在，但它的菜单没反应** —— 没有实现 `Host::tray_event`，
  或命令编号与 `Menu` 项不一致。
- **关闭窗口时退出而不是隐藏** —— `Host::close_requested` 必须
  返回 `false`，并且必须存在一个托盘图标，用户才能把它恢复回来。

## 下一步

[11 — 主题与图像](11-theming-and-images.md)：切换浅色和深色，
加载 `.buitheme` 调色板并将渐变馈送给头像和封面墙。
