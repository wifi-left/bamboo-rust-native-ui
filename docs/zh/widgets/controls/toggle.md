# Toggle

> 一个带有文本标签和胶囊形轨道的开 / 关开关。

## 介绍

`Toggle<M>` 是简单控件，把文本标签与 44 × 22 DIP 的开关轨道和 18 DIP 的旋钮搭配在一起。关闭时旋钮位于左侧，开启时位于右侧；轨道在开启时使用 `accent`，关闭时使用 `button_bg`，指针悬停时使用 `button_hover`。

点击它，或在获得焦点时按下 Enter/Space，会翻转其状态并上报 `on_change(new_state)`。该控件为 MP3 播放器词汇提供别名：`Switch<M>` 是 `Toggle<M>` 的类型别名，由 `bamboo_native_ui::widgets`（以及 `widgets::inputs`）导出。`Ui::set_toggle` 静默地更改状态，不上报消息。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{LabelSide, Toggle};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    MuteChanged(bool),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Toggle::new("mute", "Mute", false, Msg::MuteChanged)
        .label_side(LabelSide::End)
        .on_label("开")
        .off_label("关"),
);
ui.set_rect(WidgetId::new("mute"), Rect::from_xywh(16.0, 16.0, 180.0, 36.0));
```

`on_change` 是 `Fn(bool) -> M`，接收**新**状态，因此消息携带该值。`on_label`/`off_label` 绘制在轨道内部；留空即为一个裸开关。要从应用代码更新状态，使用 `ui.set_toggle(WidgetId::new("mute"), true)`；这不会发出任何消息。

## 用法（XML）

`Toggle` 由标准注册表创建（`type="toggle"`），并且像每一个上报值的标准类型一样，需要一个 `command`。工厂构建一个开关，其回调在每次变更时克隆解析后的消息，因此标准集合需要 `M: Clone`：

```xml
<widget id="mute" type="toggle" command="102"
        x="16" y="16" w="180" h="36" label="Mute" checked="false"/>
```

文件可以用 `checked` 设置初始状态，用 `label` 设置标签。消息不区分“开”与“关”；如果应用需要新值，它必须回读状态（或用 `WidgetRegistry::register` 注册自己的工厂）。标签侧和轨道内文本仅在 Rust 中可用。`Toggle` 不声明任何控件局部属性。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, label, on, on_change)` | `&'static str, impl Into<String>, bool, impl Fn(bool) -> M` | — | 创建该开关；`160 × 36` DIP，标签在起始侧，已启用。 |
| `.label_side(LabelSide)` | `LabelSide` | `Start` | `Start` = 标签在轨道左侧；`End` = 标签在轨道右侧。 |
| `.on_label(impl Into<String>)` | `String` | `""` | 开启时绘制在轨道内部的文本。 |
| `.off_label(impl Into<String>)` | `String` | `""` | 关闭时绘制在轨道内部的文本。 |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | 无 | 附加型点击观察器。 |
| `.on_focus_gained(handler)` / `.on_focus_lost(handler)` | `Fn(FocusEvent) -> Option<M>` | 无 | 附加型焦点观察器。 |

### 行为

- **轨道几何。** 轨道为 44 × 22 DIP，紧贴标签的对侧，距矩形边缘 2 DIP；标签占据其余空间减去 10 DIP 的间距。焦点环是紧贴轨道的胶囊形，保持在布局单元格内。
- **状态。** `Toggle` 没有 `disabled()` 构建器，也没有公开的状态 getter。用 `Ui::set_enabled` 禁用它；在 Rust 中构建的开关会在其 `on_change` 消息中传递新状态。由文档创建的开关会重复上报其固定的命令，因此请从 `Ui::a11y_nodes` 读取状态，或注册一个携带该值的工厂。`Toggle` 声明 `content_caps = TOGGLED`，因此 `checked` 是文档侧的初始值。
- **无障碍性。** 角色为 `toggle`，带有选中状态；无障碍名称是标签。Enter/Space（`Activate`）的行为与点击完全一致。

## 事件

### 消息

| 消息 | 触发条件 |
|---|---|
| 传给 `Toggle::new` 的 `Fn(bool) -> M` | 状态通过点击或 `Enter`/`Space` 翻转；参数是**新**状态。 |

`Ui::set_toggle` 更改状态而不会上报消息。

### 通用观察器

该控件安装了共享的观察器构建器。它们都是附加型的：处理函数返回后，内置行为仍会执行。

| 构建器 | 处理函数 | 触发条件 |
|---|---|---|
| `.on_click(f)` | `Fn(Point) -> Option<M>` | 在控件上的左键点击。 |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | 右键点击，在上下文菜单之前。 |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | 平台双击。 |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | 控件获得焦点时的按键按下。 |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | 控件获得焦点时的按键释放。 |
| `.on_focus_gained(f)` | `Fn(FocusEvent) -> Option<M>` | 焦点移入该控件。 |
| `.on_focus_lost(f)` | `Fn(FocusEvent) -> Option<M>` | 焦点离开该控件（在编辑器提交之后）。 |

### 控件专属回调

无。传给构造函数的 `on_change` 闭包是唯一的回调。

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("mute"), Trigger::Click, |event, ctx| {
    // 返回 Some(Msg::...) 以排队一条消息；除非显式调用 ctx.run_default()，
    // 否则内置动作仍会执行。
    None
});
```

| 触发器 | 触发条件 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告一次双击。 |
| `RightClick` | 次键点击，在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开该控件。 |
| `Wheel` | 控件上方的滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 在获得焦点的控件上按 Enter/Space，或一次 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 按钮被释放。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

开关响应 `Click` 和 `Activate`（两者都翻转状态），并响应 `HoverEnter`/`HoverLeave` 以设置轨道填充；它忽略 `Wheel`、`DragStart`/`DragEnd` 和 `ValueChange`。

## XML 定义

- **创建：** `type="toggle"` 加上 `command`。缺少命令会报 `xml.command.missing`；标准工厂需要 `M: Clone`。
- **几何：** 一种水平形式（`x`+`w`、`right`+`w`、`x`+`right`）和一种垂直形式（`y`+`h`、`bottom`+`h`、`y`+`bottom`）。
- **内容：** 仅 `label` 和 `checked`。
- **样式：** `radius`、`corners`、`background`、`font-size`、`font-family`。`background` 覆盖会更改字段填充，而不是轨道状态颜色。
- **属性：** 通用运行时集合（`enabled`、`visible`、`tooltip`、`name`、……）。
- **子元素：** 无。
- **父元素：** 可选的 `parent="panel-id"`，用于已注册的滚动面板。

## 另见

- `./button.md`
- `./segmented.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
