# Ui 容器

> 负责命中测试、焦点、输入路由、损伤跟踪和绘制的保留式控件树。

## 概述

`Ui<M>` 是面向应用的容器：它按绘制和命中测试顺序，把页面的每个控件以
`Box<dyn Widget<M>>` 持有，并且由它——而不是各控件——持有悬停、按下、焦点、拖动和滚动
状态。控件在每次页面重建时添加一次，并持续到下一次重建；状态变化会就地更新它们。

容器从不计算布局。应用通过
`set_rect`（或 `layout_vertical`）分配矩形，状态变化只会使
受影响的矩形失效。消息要么来自输入路由（在内部排队，通过 `drain_messages` 取出），要么由
触发它们的方法返回。

大多数方法接收一个 sink：`&mut impl Invalidator` 接受脏矩形
（`WindowHandle` 实现了它），而 `&mut impl UiHost` 会添加原生编辑器、
工具提示、剪贴板、上下文菜单和拖动钩子。除了
`invalidate` 之外，每个 `UiHost` 方法都有空操作（no-op）默认实现，因此测试用的 sink 就是 `impl UiHost for Sink {}`。
`Ui` 是平台无关的；`UiHost`、`WindowHandle` 和窗口则
仅限 Windows。`paint` 需要一个 `PaintCtx`（见
[宿主与窗口](./host-and-window.md)）。

## 关键类型

### `Ui<M>`、`Invalidator`、`UiHost`

```rust
pub struct Ui<M> { /* private */ }

impl<M> Ui<M> { pub fn new() -> Self; }
impl<M> Default for Ui<M> { /* Ui::new() */ }

pub trait Invalidator {
    fn invalidate(&mut self, rect: Rect);
}

/// 用于原生编辑器、工具提示、剪贴板和菜单的 sink；除
/// `invalidate` 外的所有方法都有默认实现。
pub trait UiHost: Invalidator {
    fn begin_edit(&mut self, request: &EditRequest<'_>);
    fn end_edit(&mut self);
    fn move_edit(&mut self, rect: Rect);
    fn editing_id(&self) -> Option<WidgetId>;
    fn set_tooltip(&mut self, target: Option<TooltipRequest<'_>>);
    fn show_context_menu(&mut self, request: ContextMenuRequest<'_>);
    fn clipboard_text(&mut self) -> Option<String>;
    fn copy_to_clipboard(&mut self, text: &str);
    fn set_caret_rect(&mut self, rect: Option<Rect>);
    // 还有：set_soft_keyboard, soft_keyboard_open, set_touch_keyboard, start_drag。
}
```

### `Trigger` 与 `TriggerEvent`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Click, DoubleClick, RightClick, HoverEnter, HoverLeave, Wheel, Activate,
    DragStart, DragEnd, ValueChange,
}

#[derive(Clone, Copy, Debug)]
pub struct TriggerEvent {
    pub trigger: Trigger,
    pub pos: Option<Point>,
    pub wheel_delta: f32,
}

pub struct TriggerContext<'a, M = ()> { /* private */ }

impl<M> TriggerContext<'_, M> {
    /// 在处理器返回之后运行控件自身的行为。
    pub fn default(&mut self);
    /// 立即运行控件自身的行为并返回其消息。
    pub fn run_default(&mut self) -> Option<M>;
}
```

### 焦点类型

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusReason { Pointer, Keyboard, Programmatic, A11y, Relocated }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusEvent { pub id: WidgetId, pub reason: FocusReason }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusOutcome { #[default] Allow, Redirect(WidgetId), Cancel }
```

焦点丢失钩子返回 `Cancel` 会中止迁移；返回 `Redirect` 则把焦点
改送到另一个控件（每次迁移只允许一次重定向）。

## 方法

### 构建与树

| 方法 | 行为 |
|---|---|
| `new` | 空树和空的损伤列表。 |
| `add`、`add_boxed`、`add_custom` | 追加一个控件（较晚的控件绘制在上层、赢得命中测试，并在并列时保持 Tab 顺序）；`add_custom` 包装一个绘制闭包。 |
| `add_with` | 在首次布局之前应用创建时的 `WidgetProps` 来添加。 |
| `insert`、`insert_boxed`、`insert_before` | 在某个位置插入（改变 z 序和 Tab 并列顺序）；`insert_before` 对未知锚点进行追加。 |
| `raise`、`lower` | 移到绘制顺序的顶部/底部；已在目标位置时返回 `false`。 |
| `remove` | 关闭打开的编辑器、迁移焦点并移除；未知时返回 `false`。 |
| `remove_preserving_session`、`has_pending_session`、`discard_pending_sessions` | 为重新插入而捕获一个已绘制的编辑会话，或检查、丢弃它。 |
| `clear`、`rebuild(area, sink, build)` | 丢弃一切；`rebuild` 结束编辑器、清空、运行 `build`、使其失效并返回编辑提交消息。 |
| `contains`、`is_empty`、`len`、`ids` | 查询树；`ids` 按绘制/命中顺序返回。 |

### 几何与类型化访问

| 方法 | 行为 |
|---|---|
| `set_rect`、`rect` | 用于布局阶段的静默赋值；当前矩形。 |
| `move_rect` | 移动并重绘旧矩形和新矩形，保持原生编辑器对齐。 |
| `layout_vertical(area, gap)` | 使用 `preferred_height` 按插入顺序堆叠。 |
| `with_widget::<W, R>`、`edit_widget::<W>` | 通过向下转型进行类型化读取/可变访问；`edit_widget` 返回是否应重绘。 |

### 绑定、焦点与属性

| 方法 | 行为 |
|---|---|
| `bind`、`on_click`、`on_activate`、`unbind` | 绑定一个高层 `Trigger`（重新绑定会替换）；Click 和 Activate 的便捷包装器。 |
| `on_focus_gained`、`on_focus_lost` | 返回 `FocusOutcome` 的焦点钩子。 |
| `set_focus`、`focus_id`、`focus_visible`、`show_focus_ring` | 以编程方式设置焦点和焦点环状态。 |
| `set_focus_scope`、`focus_scope` | 限制 Tab 导航（`None` 移除陷阱）。 |
| `set_focusable`、`set_focus_order` | 运行时焦点能力；Tab 权重（较小者优先）。 |
| `set_props`、`narration` | 在运行时应用 `WidgetProps`；读取已安装的朗读。 |
| `set_cursor`、`hover_cursor` | 光标覆盖；被悬停控件请求的光标。 |
| `adjusting` | 获得焦点的控件处于键盘调整模式。 |

### 指针、滚轮、键盘与编辑

| 方法 | 行为 |
|---|---|
| `pointer_move`、`pointer_leave` | 悬停、拖动跟踪、清除悬停。 |
| `pointer_down`、`pointer_down_button`、`pointer_down_with` | 按下，可带按钮和按下时的修饰键。 |
| `pointer_up`、`pointer_up_button`、`pointer_up_with` | 释放，可带按钮和释放时的修饰键。 |
| `pointer_double_click` | 平台双击。 |
| `wheel`、`wheel_xy` | 垂直档位；双轴（自滚动的控件优先于视图）。 |
| `key`、`key_up`、`pad_key` | 导航/激活/助记符/编辑（返回 `false` 则让宿主尝试全局快捷键）；释放转发；游戏手柄映射的按键。 |
| `char`、`ime_compose`、`ime_commit` | 翻译后的字符以及 IME 组合/结果。 |
| `set_pointer_kind`、`pointer_kind`、`long_press`、`hover_id`、`press_id`、`hover_wants_text` | 设备类型、触摸驻留和交互状态查询。 |
| `sync_edit`、`commit_edit`、`finish_inline_edit` | 重新对齐打开的原生编辑器、交付已提交文本、关闭已绘制的草稿。 |

### 拖放

| 方法 | 行为 |
|---|---|
| `set_drag_source`、`clear_drag_source`、`set_drop_target`、`clear_drop_target`、`set_drop_hover`、`clear_drop_hover` | 控件拖动的负载、放置和悬停回调。 |
| `set_text_drag_source`、`clear_text_drag_source`、`set_text_drop_target`、`clear_text_drop_target`、`set_text_drop_hover`、`clear_text_drop_hover` | 选中文本的变体。 |
| `set_files_drop_target` | 接受 shell 文件放置。 |
| `is_dragging`、`drag_data`、`drag_text`、`drag_source`、`drag_target`、`cancel_drag` | 会话查询与取消。 |
| `drag_enter`、`drag_over`、`drag_leave`、`drag_drop`、`drag_finished` | 系统（OLE）拖动生命周期；返回 `DropEffect`。 |

### 滚动

| 方法 | 行为 |
|---|---|
| `set_scroll_view`、`scroll_view`、`scroll_views`、`remove_scroll_view` | 注册、读取或丢弃一个 `ScrollView` 面板。 |
| `scroll_offset`、`set_scroll_offset`、`scroll_by` | 读取或更改被钳制的偏移。 |
| `scroll_to_child`、`scroll_owner_of` | 在每个包围面板中显示某个子项；找到它最内层的所有者。 |
| `set_scroll_viewport` | 移动或调整面板视口的大小。 |

滚动视图是一个容器区域，而不是控件：子项在内容坐标系中布局一次，
容器将每个子项定位在 `rect - offset` 处，裁剪绘制和命中测试，
并自行绘制滚动条。见
[滚动](./scrolling.md)。

### 图像与内容设置器

| 方法 | 行为 |
|---|---|
| `set_image`、`remove_image`、`image`、`images` | 页面图像存储，以 `ImageId` 为键。 |
| `set_enabled`、`set_visible` | 运行时状态；禁用的控件失去焦点，隐藏的控件退出输入、Tab、绘制和无障碍。 |
| `set_value`、`set_toggle`、`set_selected`、`select_child` | 值、切换和选中；`select_child` 走控件自身的路径并返回其消息。 |
| `set_rows`、`set_label`、`set_text`、`set_number`、`set_progress`、`set_message` | 内容设置器；控件未实现的设置器是空操作。 |
| `set_mnemonic_underlines` | 显示或隐藏 `&` 访问键下划线。 |

### 菜单

| 方法 | 行为 |
|---|---|
| `set_menu`、`remove_menu`、`menu` | 控件自身的菜单模型。 |
| `set_context_menu`、`context_menu` | 右键单击 / 菜单键菜单。 |
| `open_menu`、`open_menu_for`、`expand_menu` | 在锚点处打开、打开控件会打开的内容、在子菜单路径处重新展开。 |
| `menu_command(id, command)` | 重放一次选择；保留的编辑器 id 会被分派给控件。 |

### 绘制、损伤、消息与无障碍

| 方法 | 行为 |
|---|---|
| `paint(ctx)` | 绘制与 `ctx.dirty` 相交的控件；填充 `ctx.caret`。 |
| `mark_dirty`、`flush_damage`、`has_damage` | 将一个控件加入队列、为每个变化区域输出一个矩形、查询待处理的损伤。 |
| `overlay_active`、`overlay_split` | 存在模态背景；遮罩层开始处的索引。 |
| `drain_messages` | 取出所有排队的消息。 |
| `debug_snapshot`、`debug_event`、`clear_debug_log` | `feature = "debug-form"` 的快照和日志。 |
| `a11y_nodes`、`apply_a11y` | 供 UIA 使用的快照；应用一个屏幕阅读器操作。 |
| `installed_layout`（`layout-xml` / `layout-compiled`）、`set_widget_registry`、`widget_registry`、`widget_registry_mut`、`clear_widget_registry` | 已安装的布局；文档控件工厂注册表（始终编译）。 |

## 示例

```rust
use bamboo_native_ui::widgets::Button;
use bamboo_native_ui::{Invalidator, Point, Rect, Ui, UiHost, WidgetId};

enum Msg { Play }

struct Sink { damage: Vec<Rect> }

impl Invalidator for Sink {
    fn invalidate(&mut self, rect: Rect) { self.damage.push(rect); }
}

impl UiHost for Sink {}

fn click_play() -> Vec<Msg> {
    let mut ui: Ui<Msg> = Ui::new();
    ui.add(Button::new("play", "Play", Msg::Play));
    ui.set_rect(WidgetId::new("play"), Rect::from_xywh(16.0, 16.0, 120.0, 36.0));

    let mut sink = Sink { damage: Vec::new() };
    ui.pointer_down(Point::new(40.0, 30.0), &mut sink);
    ui.pointer_up(Point::new(40.0, 30.0), &mut sink);
    ui.flush_damage(&mut sink);
    assert!(!sink.damage.is_empty());
    ui.drain_messages()
}
```

## 说明与限制

- 应用负责布局：`set_rect` 和 `layout_vertical` 是唯一的
  入口，且 `set_rect` 是静默的，因此一次布局阶段会分配许多矩形，
  而宿主只需失效一次。`paint` 必须从 `Host::paint` 中带一个
  `PaintCtx` 运行；它读取 `dirty`、`overlay`、`editing`、`caret_on` 并写入
  `caret`。
- 一串 `set_*` 调用会把损伤排队；在宿主回调末尾调用一次 `flush_damage`，
  以便相距较远的控件保持为各自独立的重绘块。输入
  路由直接失效，无需 flush。
- 绑定可以用 `ctx.default()`
  （在处理器之后运行）或 `ctx.run_default()`（立即运行并返回
  消息）保留控件的内置行为。两者都不调用则抑制默认行为。
- `clear` 没有宿主访问权，因此要先结束原生编辑器；`rebuild` 会处理它，
  `remove` 则按需关闭或提交。`remove` 不会刷新
  无障碍快照——请调用 `WindowHandle::a11y_refresh`。

## 另见

- [控件 trait](./widget-trait.md)
- [事件与输入](./events-and-input.md)
- [绘制与 Canvas](./painting-and-canvas.md)
- [宿主与窗口](./host-and-window.md)
- [滚动](./scrolling.md)
- [Button](../widgets/controls/button.md)
- [第一个窗口快速上手](../quickstart/03-first-window.md)
