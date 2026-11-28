# EmptyState

> 为一个没有内容的视图提供居中的标题和提示。

## 介绍

`EmptyState` 为空的视图绘制零内容状态：正文文本颜色下的一个标题，以及其下一行小号柔和样式的提示，两者都在控件的矩形上水平居中，并围绕其垂直中心堆叠。它不绘制框架或填充，因此直接位于页面背景之上。

该控件仅用于显示且是惰性的：它不可聚焦，不接受输入，且不发出任何消息。除构造函数外没有其他构建器，也没有运行时设置器；两段文本在该实例的生命周期内固定不变。其无障碍节点是一个仅用于显示的 `Text`，以标题命名，提示作为描述。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::EmptyState;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(EmptyState::new(
    "queue_empty",
    "Nothing queued",
    "Add a track to start playing.",
));
ui.set_rect(WidgetId::new("queue_empty"), Rect::from_xywh(16.0, 96.0, 320.0, 120.0));
```

将矩形居中于空列表原本会占据的区域，使两行落在内容本该所在的位置。要更改文本，请移除并重新添加该控件；没有静默设置器。

## 用法（XML）

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。文档可以为它定位并设置诸如 `visible`、`expose` 和 `name` 之类的通用属性：

```xml
<widget id="queue_empty" x="16" y="96" w="320" h="120" name="Queue is empty"/>
```

标题和提示仅在构造时设置，且该条目不接受任何内容属性；文档仅提供几何和叙述。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, title, hint)` | `&'static str, impl Into<String>, impl Into<String>` | — | 创建一个 `320 × 120` 的空状态。 |

### 行为

- 标题居中，其基线带为 `centre.y - 28` 到 `centre.y - 4`；提示占据 `centre.y` 到 `centre.y + 24`。
- 首选高度为 120 DIP，且该控件不可聚焦。
- 不绘制任何填充、边框或表面；没有 `SurfaceStyle`。
- 无障碍：`Text`、仅显示，name = 标题，description = 提示。

## 事件

`EmptyState` 是惰性的：其 `input` 始终返回 `None`，它不可聚焦，且不发出任何消息。如果空视图需要行动号召，请在该矩形之上放置一个真正的 `Button`，而不是绑定文本块。

### 消息

无。构造函数不接收回调。

### 通用观察器

该控件不安装任何共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。

### 控件专属回调

无。

### Ui 触发器

任何控件 id 都可以用 `Ui::bind(WidgetId::new("queue_empty"), Trigger::Click, |event, ctx| ...)` 绑定；返回 `Some(Msg::...)` 会排队一条消息，并且处理器返回后内置行为仍会运行（`ctx.run_default()` 会内联运行它，两者都不抑制它）。空状态没有内置行为；绑定它会把整个居中块变成一个点击目标。

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

只能观察到指针触发器，且仅在刻意将该块设为可点击时；`Activate`、按键和焦点触发器永远不会触发，因为该控件不可聚焦；它也没有值，因此 `ValueChange` 也不会触发。

## XML 定义

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。

- **几何：** 适用普通的 `widget` 规则；宽而短的矩形适合这对居中的文本行。
- **内容：** 无。不支持 `label` 和 `text`（`xml.content.unsupported`，警告），并会被跳过。
- **局部属性：** 无。`skin_attrs()` 为空。
- **样式令牌：** 适用 `font-size` 和 `font-family`；`radius`、`corners` 和 `background` 是 `xml.style.unsupported` 警告，并会被跳过，因为没有填充任何内容。
- **子控件：** 无。
- **属性：** 适用通用运行时集合（`visible`、`tooltip`、`name`、`description`、`expose`、`automation-id`、……）。

## 另见

- `stat-card.md`
- `section-title.md`
- `../data/list-view.md`
- `../index.md`
- `../../api/widget-trait.md`
