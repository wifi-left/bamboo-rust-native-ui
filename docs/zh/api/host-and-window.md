# 宿主与窗口

> 仅限 Windows 的应用侧：`Host` 回调、绘制上下文，以及 `WindowBuilder` / `WindowHandle` API。

## 概述

这里的一切都仅限 Windows：`host` 和 `platform` 是 `#[cfg(windows)]`
模块，而 `Host`、`PaintCtx`、`CloseReason`、`WindowVisualState`、
`WindowBuilder` 和 `WindowHandle` 仅在
Windows 上从 crate 根部重导出。控件、绘制器和事件层保持平台无关；Win32 被
限制在后端和这个宿主层中。

应用实现 `Host`，永远看不到 `WM_*` 消息。窗口
将消息转换成规范化的 `Event` 和生命周期回调，以一个借用的脏矩形列表调用
`Host::paint`，并将每个回调交给一个
`WindowHandle`——它可以失效、打开菜单和对话框、调整托盘
图标或启动拖动。`WindowHandle` 实现了 `Invalidator` 和 `UiHost`，因此
`&mut window` 正是 `Ui` 方法所期望的 sink。每个窗口有一个持久的后台
缓冲区，且只有脏包围盒会被位块传输，因此空闲即零绘制。

## 关键类型

### `PaintCtx`

```rust
pub struct PaintCtx<'a> {
    pub painter: &'a Painter,
    pub canvas: &'a mut dyn Canvas,
    /// 已合并的脏矩形，单位为 DIP。
    pub dirty: &'a [Rect],
    pub size: Size,
    pub dpi: f32,
    /// 其原生编辑子窗口已打开的控件；其关闭字段的插入符被抑制。
    pub editing: Option<WidgetId>,
    /// 框架绘制编辑器的插入符闪烁相位。
    pub caret_on: bool,
    /// 只有遮罩层发生变化；从背景开始绘制控件。
    pub overlay: bool,
    /// 本帧将背景混合到缓存的页面上。
    pub overlay_first: bool,
    /// 输出：获得焦点的已绘制编辑器的插入符矩形，由 `Ui::paint` 填充。
    pub caret: Option<Rect>,
}

impl PaintCtx<'_> {
    pub fn scale(&self) -> f32;      // dpi / 96.0
    pub fn hits(&self, rect: Rect) -> bool;
    pub fn full(&self) -> Rect;      // Rect::from_size(self.size)
}
```

`hits` 是廉价的"这个控件是否需要绘制"检查；`full` 是客户区。
`overlay_first` 仅对将背景混合到缓存页面的那一帧为 true，
因此后续的遮罩帧不会再去动被调暗的页面。

### `CloseReason` 与 `WindowVisualState`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// 标题栏关闭、Alt+F4、系统菜单、任务管理器的"结束任务"。
    User,
    /// 系统正在注销或关机。
    System,
    /// `WindowHandle::request_close`。
    Programmatic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowVisualState { #[default] Normal, Minimized, Maximized }
```

### `Renderer`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Renderer { #[default] Gdi, D2dSoftware, D2dHardware }
```

`Renderer::parse` 映射 `gdi`/`d2d-software`/`d2d-hardware`（以及一些
别名）；`from_env` 读取 `BAMBOO_UI_RENDERER`；`name` 返回选择器字符串。

## `Host` 钩子

`paint` 是唯一必需的方法；其他每个钩子都有一个什么都不做的默认实现
（或不否决任何东西）。

### 绘制、计时与布局

| 钩子 | 签名与说明 |
|---|---|
| `paint` | `fn paint(&mut self, ctx: &mut PaintCtx<'_>)` ——只有 `ctx.dirty` 中的矩形可见。 |
| `overlay_paint` | `fn overlay_paint(&self) -> bool { false }` —— `true` 保留缓存的页面，只合成遮罩。 |
| `timer` | `fn timer(&mut self, window: &mut WindowHandle<'_>)` ——动画/心跳滴答。 |
| `resize` | `fn resize(&mut self, size: Size, dpi: f32, window: &mut WindowHandle<'_>)` ——唯一允许重新计算布局的地方。 |
| `dpi_changed` | `fn dpi_changed(&mut self, dpi: f32, window: &mut WindowHandle<'_>)` ——字体缓存已被丢弃。 |

### 生命周期

| 钩子 | 签名与说明 |
|---|---|
| `started` | `fn started(&mut self, window: &mut WindowHandle<'_>)` ——已创建且可见；安装计时器、请求焦点。 |
| `loaded` | `fn loaded(&mut self, window: &mut WindowHandle<'_>)` ——首帧已显示在屏幕上。 |
| `visibility_changed` | `fn visibility_changed(&mut self, visible: bool, window: &mut WindowHandle<'_>)` —— `WM_SHOWWINDOW`，在 `started` 之后。 |
| `activated` | `fn activated(&mut self, active: bool, window: &mut WindowHandle<'_>)` ——前台/非活动。 |
| `visual_state_changed` | `fn visual_state_changed(&mut self, state: WindowVisualState, window: &mut WindowHandle<'_>)` ——真实的最小化/最大化/还原。 |
| `moved` | `fn moved(&mut self, position: Point, window: &mut WindowHandle<'_>)` ——框架左上角，以屏幕 DIP 为单位。 |
| `close_requested` | `fn close_requested(&mut self, reason: CloseReason, window: &mut WindowHandle<'_>) -> bool { true }` —— `false` 否决关闭。 |
| `destroying`、`destroy` | `fn destroying(&mut self, window: &mut WindowHandle<'_>)`；`fn destroy(&mut self) {}`。 |

### 输入、文本、拖动与 shell

| 钩子 | 签名与说明 |
|---|---|
| `event` | `fn event(&mut self, event: &Event, window: &mut WindowHandle<'_>) -> bool { false }` ——每个规范化输入；被消费时返回 `true`。 |
| `edit_changed` | `fn edit_changed(&mut self, id: WidgetId, text: &str, window: &mut WindowHandle<'_>)` ——输入时原生编辑器的文本发生变化。 |
| `edit_committed` | `fn edit_committed(&mut self, id: WidgetId, text: &str, window: &mut WindowHandle<'_>)` ——转发给 `Ui::commit_edit`。 |
| `edit_canceled` | `fn edit_canceled(&mut self, id: WidgetId, window: &mut WindowHandle<'_>)` ——未提交即关闭。 |
| `ime_compose` | `fn ime_compose(&mut self, text: &str, cursor: usize, window: &mut WindowHandle<'_>)` ——转发给 `Ui::ime_compose`。 |
| `ime_commit` | `fn ime_commit(&mut self, text: &str, window: &mut WindowHandle<'_>)` ——转发给 `Ui::ime_commit`。 |
| `files_dropped` | `fn files_dropped(&mut self, paths: Vec<String>, window: &mut WindowHandle<'_>)`。 |
| `drag_enter` | `fn drag_enter(&mut self, data: &DragData, pos: Point, window: &mut WindowHandle<'_>) -> DropEffect`。 |
| `drag_over` | `fn drag_over(&mut self, data: &DragData, pos: Point, window: &mut WindowHandle<'_>) -> DropEffect`。 |
| `drag_leave` | `fn drag_leave(&mut self, window: &mut WindowHandle<'_>)`。 |
| `drag_drop` | `fn drag_drop(&mut self, data: DragData, pos: Point, window: &mut WindowHandle<'_>) -> DropEffect`。 |
| `drag_finished` | `fn drag_finished(&mut self, effect: DropEffect, window: &mut WindowHandle<'_>)`。 |
| `menu_event` | `fn menu_event(&mut self, event: &mut MenuEvent<'_>, window: &mut WindowHandle<'_>) -> MenuDecision` —— `Handled` 取消/消费。 |
| `menu_command` | `fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>)`。 |
| `menu_bar_command` | `fn menu_bar_command(&mut self, command: u32, window: &mut WindowHandle<'_>)`。 |
| `dialog_finished` | `fn dialog_finished(&mut self, id: DialogId, result: DialogResult, window: &mut WindowHandle<'_>)`。 |
| `tray_event` | `fn tray_event(&mut self, event: TrayEvent, window: &mut WindowHandle<'_>)`。 |
| `hotkey` | `fn hotkey(&mut self, hotkey: Hotkey, window: &mut WindowHandle<'_>)`。 |
| `hotkey_captured` | `fn hotkey_captured(&mut self, hotkey: Option<Hotkey>, window: &mut WindowHandle<'_>)`。 |
| `a11y_nodes` | `fn a11y_nodes(&mut self) -> Vec<A11yNode>` ——默认不暴露任何内容。 |
| `a11y_action` | `fn a11y_action(&mut self, action: A11yAction, window: &mut WindowHandle<'_>)` ——通过 `Ui::apply_a11y` 应用。 |

## `WindowBuilder`

```rust
impl WindowBuilder {
    pub fn new(host: impl Host + 'static) -> Self;
    pub fn title(self, title: impl Into<String>) -> Self;
    pub fn size(self, width: f32, height: f32) -> Self;
    pub fn min_size(self, width: f32, height: f32) -> Self;
    pub fn theme(self, settings: ThemeSettings, dark: bool) -> Self;
    pub fn renderer(self, renderer: Renderer) -> Self;
    pub fn menu_bar(self, menu_bar: MenuBarModel) -> Self;
    // 还有：owner、app_id、window_hotkey、window_hotkeys。
    /// 创建窗口、运行消息循环、返回退出码。
    pub fn run(self) -> windows::core::Result<i32>;
}
```

`new` 设置的默认值：标题 `"Bamboo Native UI"`、尺寸 900 × 620 DIP、最小
320 × 240、`ThemeSettings::default()`、深色模式 `true`、无菜单栏、无
渲染器覆盖。`menu_bar` 安装系统绘制的窗口栏（命令
在 `MenuEvent` 钩子之后通过 `Host::menu_bar_command` 到达）、`owner`
使一个自有对话框以另一个 `HWND` 为中心，而 `window_hotkey` 注册
一个在窗口拥有焦点时触发 `Host::menu_bar_command` 的组合键。

### 渲染器选择

1. 调用时的 `WindowBuilder::renderer(...)`。
2. 否则 `BAMBOO_UI_RENDERER`（`gdi`、`d2d-software`、`d2d-hardware`）。
3. 否则 `Renderer::Gdi`，即默认值。

所有模式都绘制到同一个持久后台缓冲区，并位块传输脏矩形框。一个
Direct2D 模式通过 `ID2D1DCRenderTarget` 绘制——当驱动程序
提供目标时用硬件，否则用 Direct2D 的软件光栅化器——而一个
无法启动的模式会在 `BAMBOO_UI_DEBUG` 下记录日志，并永久切换到 GDI。
`WindowHandle::renderer` 报告当前活动的渲染器。GDI/D2D 后端需要
Windows 上的 `feature = "gdi"`（默认特性）。

## `WindowHandle`

`WindowHandle<'_>` 实现了 `Invalidator` 和 `UiHost`，因此 `&mut window` 是
`Ui` 调用的 sink。常用操作：

| 领域 | 方法 |
|---|---|
| 绘制 | `invalidate`、`invalidate_all`、`invalidate_force`、`invalidate_all_force`、`flush`、`invalidations`、`resync_overlay` |
| 几何 | `size`、`dpi`、`scale`、`frame_position`、`renderer`、`set_cursor`、`set_animation_timer` |
| 生命周期 | `request_close`、`is_visible`、`is_active`、`visual_state`、`raise`、`hide`、`minimize`、`maximize`、`restore` |
| 编辑 / 无障碍 | `end_edit`、`a11y_refresh`、`a11y_focus`、`a11y_announce`、`a11y_announce_with` |
| 菜单 | `open_menu`、`menu_click`、`set_menu_bar`、`menu_bar_installed`、`edit_menu_bar`、`dismiss_menu`、`is_menu_open`、`active_menu_widget` |
| 拖动 | `start_drag`、`set_system_drag_enabled`、`system_drag_enabled` |
| Shell | `show_dialog`、`show_color_window`、`set_tray_icon`、`notify`、`show_tray_balloon`、`copy_text` |
| 热键与杂项 | `register_hotkey`、`unregister_hotkey`、`register_window_hotkey`、`clear_window_hotkeys`、`begin_hotkey_capture`、`cancel_hotkey_capture`、`tooltip_visible`、`is_foreground`、`hwnd` |

## 示例

```rust
use bamboo_native_ui::widgets::Label;
use bamboo_native_ui::{
    Event, Host, PaintCtx, Rect, Size, Ui, WidgetId, WindowBuilder, WindowHandle,
};

struct App { ui: Ui<Msg> }

enum Msg { Quit }

impl Host for App {
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        let full = ctx.full();
        let background = ctx.painter.palette().background;
        ctx.painter.fill(ctx.canvas, full, background);
        self.ui.paint(ctx);
    }

    fn resize(&mut self, size: Size, _dpi: f32, _window: &mut WindowHandle<'_>) {
        if self.ui.is_empty() {
            self.ui.add(Label::new("title", "Hello, Bamboo"));
        }
        let rect = Rect::from_xywh(16.0, 16.0, size.width - 32.0, 28.0);
        self.ui.set_rect(WidgetId::new("title"), rect);
    }

    fn event(&mut self, event: &Event, window: &mut WindowHandle<'_>) -> bool {
        match *event {
            Event::PointerDown { pos, button, modifiers } => {
                self.ui.pointer_down_with(pos, button, modifiers, window)
            }
            Event::PointerUp { pos, button, modifiers } => {
                self.ui.pointer_up_with(pos, button, modifiers, window)
            }
            _ => false,
        }
    }
}

fn main() -> windows::core::Result<()> {
    let _exit_code = WindowBuilder::new(App { ui: Ui::new() })
        .title("Hello")
        .size(640.0, 360.0)
        .run()?;
    Ok(())
}
```

## 说明与限制

- 仅限 Windows：`Host`、`PaintCtx`、`WindowBuilder`、`WindowHandle`、
  `CloseReason` 和 `WindowVisualState` 在非 Windows 构建中不存在；
  可移植部分（`Ui`、`Widget`、`Painter`、`Canvas`、`Event`）存在。
- `paint` 只能在 `ctx.dirty` 内绘制；使用 `ctx.hits(rect)` 廉价地跳过
  控件。
- 不要在 `Host::event` 内运行模态循环：菜单和对话框都是延迟的，
  框架在回调返回后显示它们，并通过
  `menu_command`、`menu_event` 或 `dialog_finished` 报告结果。
- 关闭是一个请求：从 `close_requested` 返回 `false` 会保持
  窗口打开，并抑制销毁回调。
- `WindowBuilder::renderer` 优先于 `BAMBOO_UI_RENDERER`；无效值会被
  忽略（`from_env` 返回 `None`）。`run` 会阻塞直到消息循环
  退出，并将退出码作为 `windows::core::Result<i32>` 返回。一个
  `WindowHandle` 借用仅在接收它的回调内有效。

## 另见

- [事件与输入](./events-and-input.md)
- [Ui 容器](./ui-container.md)
- [绘制与 Canvas](./painting-and-canvas.md)
- [托盘图标与通知](./tray-and-notifications.md)
- [Button](../widgets/controls/button.md)
- [第一个窗口快速上手](../quickstart/03-first-window.md)
