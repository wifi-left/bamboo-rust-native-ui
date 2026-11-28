# Divider

> 一条水平或垂直的 1 DIP 分隔线，可选带居中标签。

## 介绍

`Divider` 在其矩形中间绘制一条 1 DIP 的线。水平分隔线可以携带一个居中标签：线条被分成
两段，文本周围留出 8 DIP 的间隙，文本以小型弱化样式绘制。垂直分隔线只是普通列，
会忽略任何标签。

该控件是装饰：`focusable` 为 `false`。带标签时，它会发布一个由标签命名的仅显示 `Text`
无障碍节点；不带标签时则不出现在树中。它从不发送消息，因此 `M` 只是容器的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::Divider;
use bamboo_native_ui::{Color, Rect, Ui, WidgetId};

enum Msg {}

fn build(ui: &mut Ui<Msg>) {
    ui.add(Divider::new("section_rule").label("Playback"));
    ui.set_rect(WidgetId::new("section_rule"), Rect::from_xywh(16.0, 16.0, 400.0, 16.0));

    ui.add(Divider::new("side_rule").vertical(true).color(Color::new(60, 64, 72)));
    ui.set_rect(WidgetId::new("side_rule"), Rect::from_xywh(430.0, 16.0, 1.0, 120.0));
}
```

水平分隔线通常使用较短的高度（首选的 16 DIP）；线条绘制在垂直中心。垂直分隔线使用较窄的
宽度（一个 DIP）和实用的高度。`preferred_height` 对水平分隔线报告 16，对垂直分隔线报告 40。

## 用法（XML）

`divider` 是标准注册表类型。在安装了 `WidgetRegistry::standard()` 时，用 `type="divider"`
创建它：

```xml
<widget id="section_rule" type="divider" x="16" y="16" w="400" h="16"
        label="Playback"/>
```

在 Rust 中创建的分隔线通过 id 引用：

```xml
<widget id="side_rule" x="430" y="16" w="1" h="120"/>
```

只有标签可以在 XML 中表达；`vertical` 和 `color` 只能在构造函数中设置。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建水平分隔线。 |
| `.vertical(bool)` | `bool` | `false` | 改为绘制一条 1 DIP 的垂直列。 |
| `.label(impl Into<String>)` | `impl Into<String>` | `None` | 水平分隔线上的居中标签。 |
| `.color(Color)` | `Color` | `None`（调色板 `divider`） | 线条颜色。 |

除非 `.color` 覆盖线条，否则线条颜色、标签文本和标签的弱化颜色都遵循调色板。

## 事件

### 消息

无。分隔线是纯装饰，从不返回消息。

### 通用观察器

`Divider` 不安装共享观察器构建器。它不可聚焦，`interactive()` 为 `false`，其默认
`input` 忽略每个事件，因此不会路由任何指针事件。如果确实需要点击，请使用
`WidgetProps::mouse(true)` 加上 `Ui::bind`。

### 控件专属回调

无。

### Ui 触发器

任意控件 id 都可以用 `Ui::bind(id, Trigger, handler)`（或 `Ui::on_click` /
`Ui::on_activate` 简写）绑定。返回 `Some(M)` 的处理器会将消息入队；除非处理器调用
`ctx.run_default()`，否则内置操作仍会执行。

| 触发器 | 触发条件 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告双击。 |
| `RightClick` | 次按键点击，发生在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开控件。 |
| `Wheel` | 控件上发生滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 焦点控件上按 Enter/Space，或 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 释放按键。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

分隔线默认对它们都无反应；`WidgetProps::mouse(true)` 选择启用是接收 `Click` 和指针触发器
的唯一方式。

## XML 定义

`divider` 是标准注册表类型，需要 `WidgetRegistry::standard()`。不适用 `command`。

- 几何：适用常规 `widget` 规则。水平分隔线使用较小的 `h`，垂直分隔线使用较小的 `w`。
- 共享内容：仅 `label`。
- 本地属性：无。该控件未声明 `skin_attrs`，因此 XML 中不接受 `color`。
- 样式 token：无 `SurfaceStyle`；`radius`、`corners` 和 `background` 不受支持
  （`xml.style.unsupported`，一个警告）。
- 子元素：无。

## 另见

- `../index.md`
- `label.md`
- `../display/card.md`
- `../../guides/xml-layout-authoring.md`
