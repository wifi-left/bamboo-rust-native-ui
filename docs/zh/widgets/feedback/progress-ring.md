# ProgressRing

> 一种圆形的确定性进度指示器，可附带百分比。

## 介绍

`ProgressRing` 绘制一个完整的圆形轨道，以及一段与其 `0.0` 到 `1.0` 之间的值成比例的强调色弧。该弧绘制为相互重叠的圆点组成的环（整圆约六十个点），因此它不需要弧线图元，也不对页面颜色做任何假设。轨道使用调色板的滚动条轨道颜色，弧使用强调色，圆点从十二点钟方向开始顺时针扫过。

该控件仅用于显示：它不可聚焦，不接受输入，且不发出任何消息。由应用程序设置值，通常通过带类型的访问接缝。`show_value(true)` 会在圆环中央绘制 `"{:.0}%"`。

它报告一个无障碍 `ProgressBar` 节点，范围为 `0.0..=1.0`，值为格式化后的百分比。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::ProgressRing;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(ProgressRing::new("import", 0.35).thickness(5.0).show_value(true));
ui.set_rect(WidgetId::new("import"), Rect::from_xywh(16.0, 16.0, 56.0, 56.0));
```

`set_value` 是静默的，因此带类型的接缝必须报告该变化，圆环才会被标记为损坏并重绘：

```rust
ui.edit_widget::<ProgressRing>(WidgetId::new("import"), |ring| {
    ring.set_value(0.72);
    true
});
```

`Ui::set_value` 没有接入该控件：该圆环不报告 `VALUE` 内容能力，因此那条路径上不会有任何效果。

## 用法（XML）

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。文档可以为它定位并设置通用属性：

```xml
<widget id="import" x="16" y="16" w="56" h="56" name="Import progress"/>
```

值、厚度和百分比读数都属于 Rust 侧的状态；尽管共享的内容词汇中有一个 `value` 名称，文档也无法设置它们。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, value)` | `&'static str, f32` | — | 创建一个 `56 × 56` 的圆环；值被钳制到 `0.0..=1.0`。 |
| `.value(f32)` | `f32` | — | 值的构建器形式，钳制到 `0.0..=1.0`。 |
| `.thickness(f32)` | `f32` | `5.0` | 圆点直径，单位为 DIP，钳制到 `1.0..=12.0`。 |
| `.show_value(bool)` | `bool` | `false` | 在中央绘制百分比。 |
| `.value_now()` | `f32` | — | 当前值。 |
| `.set_value(f32)` | `f32` | — | 静默设置器，钳制到 `0.0..=1.0`。 |

### 行为

- `thickness` 是圆点尺寸，而不是描边宽度；圆点间距随半径变化，因此大圆环保持平滑，而较短的弧花费更少的绘制。
- 值为 `0.0` 时仅绘制轨道。
- 首选高度为 56 DIP，且该控件不可聚焦。
- 无障碍：角色 `progress-bar`、`display_only`、`range(0.0, 1.0, value)` 和 `value("{n}%")`；除非设置了 `WidgetProps::name`，否则以控件 id 命名。

## 事件

圆环仅用于显示：其 `input` 始终返回 `None`，因此它不发出任何消息，也不触发自身的任何回调。进度由应用程序驱动。

### 消息

无。构造函数不接收回调。

### 通用观察器

该控件不安装任何共享的观察器构建器。它不可聚焦，也不消费输入，因此其上不存在 `on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained` 和 `on_focus_lost`。

### 控件专属回调

无。通过 `Ui::edit_widget` 使用 `set_value(f32)`（或 `.value(f32)` 构建器）设置值；让闭包返回 `true` 正是标记圆环损坏的方式，因为该设置器是静默的。

### Ui 触发器

任何控件 id 都可以用 `Ui::bind(WidgetId::new("import"), Trigger::HoverEnter, |event, ctx| ...)` 绑定；返回 `Some(Msg::...)` 会排队一条消息，并且处理器返回后内置行为仍会运行（`ctx.run_default()` 会内联运行它，两者都不抑制它）。圆环没有内置行为，因此绑定是做出响应的唯一方式。

| 触发器 | 触发时机 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告双击。 |
| `RightClick` | 次键点击，在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开控件。 |
| `Wheel` | 控件上的滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 聚焦控件上的 Enter/Space，或 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 按钮被释放。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

只有指针触发器对圆环有意义；`Activate`、按键和焦点触发器永远不会触发，因为它不可聚焦；而 `ValueChange` 在 `set_value` 上不会触发——圆环报告无障碍值，但不播报该变化。

## XML 定义

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。

- **几何：** 适用普通的 `widget` 规则；正方形是自然形状。
- **内容：** 无。`value` 会被拒绝并给出 `xml.content.unsupported`（一条警告），因为 `content_caps` 为 `NONE`。
- **局部属性：** 无。`skin_attrs()` 为空。
- **样式令牌：** 适用 `font-size` 和 `font-family`（仅绘制层面）；该控件没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 是 `xml.style.unsupported` 警告，会被跳过。
- **子控件：** 无。
- **属性：** 适用通用运行时集合（`visible`、`enabled`、`tooltip`、`name`、`expose`、`automation-id`、……），与任何被寻址的控件一样。

## 另见

- `spinner.md`
- `../controls/progress-bar.md`
- `../data/gauge.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
