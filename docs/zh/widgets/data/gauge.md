# Gauge

> 一个开口弧形仪表，显示零到一之间的值。

## 介绍

`Gauge` 绘制一个 270 度的点状环。完整轨道使用滚动轨道色绘制，值弧使用强调色绘制，从 135 度开始并顺时针扫过，在底部留出开口。中心可选择显示百分比或自定义标签。

它是一个纯显示控件：不接收输入，也不发出消息，并以纯显示 `ProgressBar` 节点（范围为 `0.0..=1.0`）的形式加入无障碍树。应用程序在 Rust 中更新它；它没有 XML 值属性。对于线性读数，请使用进度条；对于紧凑的历史记录，请使用 `Sparkline`。

## 用法（Rust）

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::Gauge;

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(Gauge::new("load", 0.65).thickness(4.0).label("Load"));
ui.set_rect(WidgetId::new("load"), Rect::from_xywh(16.0, 16.0, 96.0, 96.0));
```

构造函数会将值限制在 `0.0..=1.0`。`label` 会替换在中心绘制的百分比文本；在没有标签的情况下，`show_value(true)` 会绘制 `"{n}%"`。通过类型化接缝使用 `set_value` 静默更新弧线：

```rust
ui.edit_widget(WidgetId::new("load"), |gauge: &mut Gauge| {
    gauge.set_value(0.72);
    true // 上报静默 setter，以便标记控件需要重绘
});
```

## 用法（XML）

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。文档可以为其定位，并设置诸如 `expose` 和 `name` 之类的通用属性：

```xml
<widget id="load" x="16" y="16" w="96" h="96" expose="true" name="Load"/>
```

值、粗细和标签属于 Rust 侧状态；文档无法设置它们。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, value)` | `&'static str, f32` | — | 创建仪表；值被限制在 `0.0..=1.0`。 |
| `.thickness(f32)` | `f32` | `4.0` | 环的粗细（DIP），限制在 `1.0..=12.0`。 |
| `.show_value(bool)` | `bool` | `true` | 在没有标签时在中心绘制百分比。 |
| `.label(impl Into<String>)` | text | `None` | 中心文本；替换百分比。 |
| `.value_now()` | `f32` | — | 当前值。 |
| `.set_value(f32)` | `f32` | — | 静默设置值，限制在 `0.0..=1.0`。 |

行为：首选高度为 96 DIP，且控件不可聚焦。无障碍名称在有标签时使用标签，否则使用控件 id；该节点被标记为纯显示，并发布 `range(0.0, 1.0, value)` 以及格式化后的百分比。

## 事件

`Gauge` 是纯显示控件：其 `input` 始终返回 `None`，因此不发出消息，也不触发其自身的任何回调。

### 消息

无。构造函数接收一个 id 和值；更新通过静默的 `set_value` setter 进行。

### 通用观察器

该控件不安装共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。它不可聚焦，也不消费任何输入。

### 控件专属回调

无。

### Ui 触发器

任何控件 id 都可以通过 `Ui::bind` 进行绑定：

```rust
ui.bind(WidgetId::new("load"), Trigger::Click, |event, ctx| {
    // 返回 Some(Msg::...) 以入队一条消息；除非显式调用 ctx.run_default()，
    // 内置操作仍会执行。
    None
});
```

| 触发器 | 触发时机 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告双击。 |
| `RightClick` | 次级按钮点击，在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开该控件。 |
| `Wheel` | 控件上的滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 聚焦控件上按下 Enter/空格，或 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 释放按钮。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

仪表自身不对其中任何触发器作出反应——它没有内置行为——因此绑定是在其上响应输入的唯一方式。`ValueChange` 从不触发，因为读数仅通过静默 setter 改变。

## XML 定义

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。

几何布局遵循常规的 `<widget>` 规则（一个水平形式和一个垂直形式）。诸如 `visible`、`expose`、`name`、`description` 和 `tooltip` 之类的通用属性均适用。`font-size` 和 `font-family` 属于绘制层。仪表从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported` 并跳过。

该控件不声明任何局部属性和任何子元素。没有共享的 `value` 内容属性，因此文档无法改变仪表的读数；请在 Rust 中调用 `set_value`（或重建控件）。

## 另见

- `sparkline.md`
- `rating-stars.md`
- `../index.md`
- `../../api/widget-trait.md`
