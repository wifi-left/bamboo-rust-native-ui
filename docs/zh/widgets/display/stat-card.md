# StatCard

> 一个紧凑的指标块，带有大号数值和说明文字。

## 介绍

`StatCard` 绘制一个小型统计：标题样式和强调文本颜色下的数值，其下方是小号柔和样式的说明文字，水平方向按卡片的 padding 内缩。`emphasis(true)` 将背景从 `card` 令牌切换为 `play_button` 色调，标记出一个高亮数字。框架在直角矩形上是 1 DIP 的分隔线边框。

该控件仅用于显示：它不可聚焦，不接受输入，且不发出任何消息。它不采用 `SurfaceStyle`，因此形状始终为直角，也没有圆角或背景构建器。其无障碍节点是一个仅用于显示的 `Text`，以 `"{caption} {value}"` 命名，数值作为该节点的值。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::StatCard;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(StatCard::new("tracks", "600", "Tracks").emphasis(true));
ui.set_rect(WidgetId::new("tracks"), Rect::from_xywh(16.0, 16.0, 140.0, 72.0));
```

数值是第一个构造函数参数，说明文字是第二个。通过带类型的接缝静默更新数值（这两个字段都没有 getter）：

```rust
ui.edit_widget::<StatCard>(WidgetId::new("tracks"), |stat| {
    stat.set_value("612");
    true
});
```

## 用法（XML）

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址：

```xml
<widget id="tracks" x="16" y="16" w="140" h="72" label="612"/>
```

`label` 是共享内容，但 `StatCard::set_label` 写入的是**数值**，而不是说明文字。说明文字仅在构造时设置，因此用 Rust 创建的卡片是常规路径，文档只负责为其定位。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, value, label)` | `&'static str, impl Into<String>, impl Into<String>` | — | 创建一个 `140 × 72`、padding 为 12 的统计块。 |
| `.padding(f32)` | `f32` | `12.0` | 内边距，单位为 DIP，钳制为不低于零。 |
| `.emphasis(bool)` | `bool` | `false` | 使用强调的背景色调。 |
| `.set_value(text)` | `impl Into<String>` | — | 静默替换数值。 |

### 行为

- 数值占据从 `top + 8` 到 `top + 44` 的带，说明文字占据其下方的条带；过长的字符串会被裁剪。
- `Ui::set_label` 路由到 `set_value`（`content_caps = LABEL`），而不是说明文字。
- 首选高度为 72 DIP，且该控件不可聚焦。
- 无障碍：`Text`、仅显示，name 为 `"{label} {value}"`，`value(value)`；当需要时使用 `WidgetProps::expose(true)` 暴露它。

## 事件

`StatCard` 仅用于显示：其 `input` 始终返回 `None`，它不可聚焦，且不发出任何消息。数值通过 `set_value` 或 `Ui::set_label` 到达，从不通过事件到达。

### 消息

无。构造函数不接收回调。

### 通用观察器

该控件不安装任何共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。

### 控件专属回调

无。

### Ui 触发器

任何控件 id 都可以用 `Ui::bind(WidgetId::new("tracks"), Trigger::Click, |event, ctx| ...)` 绑定；返回 `Some(Msg::...)` 会排队一条消息，并且处理器返回后内置行为仍会运行（`ctx.run_default()` 会内联运行它，两者都不抑制它）。该统计块没有内置行为，因此绑定是让该块可点击的唯一方式。

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

只有指针触发器有意义，且仅当该统计块同时作为可点击磁贴时；`Activate`、按键和焦点触发器永远不会触发，因为该控件不可聚焦；`ValueChange` 永远不会触发，因为该控件不接受任何可能改变其无障碍值的输入。

## XML 定义

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。

- **几何：** 适用普通的 `widget` 规则；`140 × 72` 是设计尺寸。
- **内容：** `label`（`content_caps = LABEL`）写入数值。不支持 `text` 和 `value`（`xml.content.unsupported`，警告），并会被跳过。
- **局部属性：** 无。`skin_attrs()` 为空。
- **样式令牌：** 适用 `font-size` 和 `font-family`；该控件没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 是 `xml.style.unsupported` 警告，会被跳过。
- **子控件：** 无。
- **属性：** 适用通用运行时集合（`visible`、`tooltip`、`name`、`expose`、`automation-id`、……）。

## 另见

- `card.md`
- `empty-state.md`
- `../feedback/banner.md`
- `../index.md`
- `../../api/theme-and-palette.md`
