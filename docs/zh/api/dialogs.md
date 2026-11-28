# 对话框

> 原生消息框、Shell 文件/字体/颜色选择器以及库自带的主题颜色窗口，全部以异步方式交付。

## 概述

对话框有两种呈现方式。**模态**（默认）在对话框显示期间禁用其属主窗口；**并行**（`.parallel()`）
则保持应用可用。两者都以异步方式返回结果：一个工作线程承载原生对话框，对话框关闭后答案会在
UI 线程上交付给 `Host::dialog_finished`。UI 线程从不进入对话框的嵌套消息循环，因此绘制和
计时器继续工作。

`Dialog` 是请求枚举；`MessageDialog`、`FileDialog`、`FontDialog`、`ColorDialog` 和
`ColorWindow` 各自转换为其形式。原生实现仅限 Windows；请求/结果类型在所有目标上都能编译。

## 关键类型

### `MessageDialog`

```rust
pub struct MessageDialog { /* private */ }

impl MessageDialog {
    pub fn new(text: impl Into<String>) -> Self; // 信息性 OK，模态
}
```

构建方法：`title`、`icon(MessageIcon)`、`buttons(MessageButtons)`、
`default_button(MessageDefault)`、`button(label, command: u32)`（自定义任务对话框按钮）、
`instruction(text)`、`icon_file(path)`、`parallel()`。
读取方法：`text()`、`dialog_title()`、`custom_buttons()`。

| 枚举 | 变体 |
|---|---|
| `MessageIcon` | `Info`（默认）、`Warning`、`Error`、`Question` |
| `MessageButtons` | `Ok`（默认）、`OkCancel`、`YesNo`、`YesNoCancel`、`RetryCancel`、`AbortRetryIgnore` |
| `MessageAnswer` | `Ok`、`Cancel`、`Yes`、`No`、`Retry`、`Abort`、`Ignore`、`Custom(u32)` |
| `MessageDefault` | `First`（默认）、`Second`、`Third` |

至少含有一个自定义 `button` 的对话框会以任务对话框显示：选中的 id 以
`MessageAnswer::Custom(u32)` 到达，按 Escape / 关闭框则报告 `MessageAnswer::Cancel`。
`MessageButton { label, command }` 使用 `MessageButton::new(label, command)` 构建。

### `FileDialog`

```rust
impl FileDialog {
    pub fn open() -> Self;
    pub fn save() -> Self;
    pub fn folder() -> Self;
}
```

构建方法：`title`、`filter(label, pattern)`、`initial_dir`、`initial_name`、
`multi_select(bool)`、`parallel()`。读取方法：`mode() -> FileDialogMode`、
`is_multi_select()`、`returns_multiple()`。

`FileDialogMode` 为 `Open | Save | Folder`。`FileFilter::new(label, pattern)` 接受一个
可见名称和一个分号分隔的列表，例如 `"*.mp3;*.flac"`。多选**打开**对话框返回
`DialogResult::Files`；保存和文件夹模式忽略该标志。

### `FontDialog` / `ColorDialog` / `ColorWindow`

| 类型 | 用途 |
|---|---|
| `FontDialog::new()` | 系统字体选择器；`.initial(FontChoice)`、`.parallel()`。 |
| `FontChoice::new(family, size)` | 结果字面/字号；`.bold(bool)`、`.italic(bool)`。 |
| `ColorDialog::new()` | 系统颜色选择器；`.initial(Color)`、`.custom_colors([Color; 16])`、`.parallel()`。 |
| `ColorWindow::new(Rgba)` | 主题化、自有的颜色窗口；`.initial(Rgba)`、`.allow_alpha(bool)`、`.parallel()`。 |

`custom_colors() -> [Color; 16]` 读取进程级的自定义色块；`set_custom_colors([Color; 16])`
替换它们（Shell 会将混合后的颜色写回，因此下一次选择器会再次提供它）。

### `DialogId` 与 `DialogResult`

```rust
pub struct DialogId(pub u32);

pub enum DialogResult {
    Message(Option<MessageAnswer>),
    File(Option<String>),
    Files(Option<Vec<String>>),
    Font(Option<FontChoice>),
    Color(Option<Color>),
    ColorWindow(Option<Rgba>),
}
```

`None` 表示被取消（Cancel、关闭框或无法显示）。`DialogResult::is_some()` 报告是否产生了值。

### `WindowHandle` 与 `Host`

| API | 签名 |
|---|---|
| `show_dialog` | `show_dialog(&mut self, dialog: impl Into<Dialog>) -> DialogId` |
| `show_color_window` | `show_color_window(&mut self, initial: Rgba, allow_alpha: bool) -> DialogId` |
| `Host::dialog_finished` | `fn dialog_finished(&mut self, id: DialogId, result: DialogResult, window: &mut WindowHandle<'_>)` |

任意数量的对话框都可以同时打开；`DialogId` 用于关联每个结果。

## 示例

```rust
use bamboo_native_ui::{MessageButtons, MessageDialog, MessageIcon};

// 在拥有 `&mut WindowHandle` 的 Host 方法内（例如 `Ui` 消息处理中）：
fn confirm_quit(window: &mut bamboo_native_ui::WindowHandle<'_>) -> bamboo_native_ui::DialogId {
    window.show_dialog(
        MessageDialog::new("Discard the current queue?")
            .title("Quit")
            .icon(MessageIcon::Warning)
            .buttons(MessageButtons::YesNo)
            .default_button(bamboo_native_ui::MessageDefault::Second),
    )
}
```

结果稍后到达：

```rust
use bamboo_native_ui::{
    DialogId, DialogResult, MessageAnswer, WindowHandle,
};

fn dialog_finished(
    &mut self,
    id: DialogId,
    result: DialogResult,
    window: &mut WindowHandle<'_>,
) {
    if id != self.quit_dialog {
        return;
    }
    if matches!(result, DialogResult::Message(Some(MessageAnswer::Yes))) {
        window.quit(0);
    }
}
```

## 说明与限制

- 原生对话框实现（`MessageBox`、Shell 文件选择器、字体/颜色选择器、任务对话框）仅限
  Windows。构建方法与结果在所有平台都能编译。
- 模态对话框会跨线程边界禁用其属主；框架的 UI 线程始终泵送消息，因此两个循环都保持响应。
  并行对话框不传递属主，并将自身带到前台。
- `dialog_finished` 在对话框线程结束后于 UI 线程上运行，因此更新 UI 是安全的，且模态属主
  已被重新启用。
- 任务对话框（自定义按钮、`instruction`、`icon_file`）需要 comctl32 v6（一个应用程序清单）。
  没有它时，请求会回退到标准消息框，自定义标签无法呈现。
- 文件选择器在工作线程上将 COM 初始化为单线程单元；从 `Show` 取消会报告为“无选择”
  （`None`）。
- 字体选择器报告磅值；该对话框以 96 DPI 作为其自身参考。

## 另见

- `../widgets/pickers/color-picker.md`
- `../widgets/controls/color-swatch.md`
- `tray-and-notifications.md`
- `../quickstart/03-first-window.md`
