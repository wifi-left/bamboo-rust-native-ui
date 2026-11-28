# SectionTitle

> 一个为页面某一部分加标签的小号柔和标题。

## 介绍

`SectionTitle` 在其矩形内左对齐地绘制一行小号文本，除非设置了显式颜色，否则使用调色板的柔和颜色。它是用于列表、分组和次要内容上方的轻量页内标题；它不绘制框架、填充或分隔线。

该控件仅用于显示：它不可聚焦，不接受输入，且不发出任何消息。其无障碍节点是一个仅用于显示的 `Heading`，以标签命名，并在标题级别 2 上暴露，因此屏幕阅读器可以在各节之间跳转。

对于带换行和对齐选项的普通正文文本，请使用 `Label`；当某节除标题外还需要框架时，请使用 `GroupBox`。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::SectionTitle;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(SectionTitle::new("queue_title", "Up next"));
ui.set_rect(WidgetId::new("queue_title"), Rect::from_xywh(16.0, 16.0, 200.0, 24.0));

ui.add(SectionTitle::new("recent_title", "Recently played"));
ui.set_rect(WidgetId::new("recent_title"), Rect::from_xywh(16.0, 52.0, 200.0, 24.0));
```

没有 `color` 构建器；颜色是一个静默设置器，并通过 `color_value()` 读回：

```rust
use bamboo_native_ui::Color;

ui.edit_widget::<SectionTitle>(WidgetId::new("queue_title"), |title| {
    title.set_color(Some(Color::new(120, 160, 120)));
    true
});
```

传入 `None` 会恢复为柔和的调色板颜色。重命名通过共享的标签设置器 `Ui::set_label` 完成。

## 用法（XML）

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址：

```xml
<widget id="queue_title" x="16" y="16" w="200" h="24"
        label="Up next" color="muted"/>
```

`label` 是共享内容，会写入文本。`color` 是一个控件局部属性；它接受常见的颜色语法（调色板令牌或诸如 `#7fd3a8` 的字面量），并在到达控件之前针对当前生效的调色板解析。没有 `none` 值：省略该属性以保持柔和的默认颜色。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, label)` | `&'static str, impl Into<String>` | — | 创建一个 `200 × 24` 的标题。 |
| `.set_color(Option<Color>)` | `Option<Color>` | `None` | 静默颜色覆盖；`None` 使用柔和令牌。 |
| `.color_value()` | `Option<Color>` | `None` | 显式颜色（如果有）。 |

### 行为

- 文本以小号文本角色绘制，左对齐，垂直方向填满矩形；过长的标签会被裁剪，而不是换行。
- 首选高度为 24 DIP，且该控件不可聚焦。
- `Ui::set_label` 会静默重命名它（`content_caps = LABEL`）。
- 无障碍：`Heading`、仅显示，以标签命名，标题级别 2。

## 事件

`SectionTitle` 是一个标签：其 `input` 始终返回 `None`，它不可聚焦，且不发出任何消息。文本和颜色通过该类型的设置器到达，从不通过事件到达。

### 消息

无。构造函数不接收回调。

### 通用观察器

该控件不安装任何共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。

### 控件专属回调

无。

### Ui 触发器

任何控件 id 都可以用 `Ui::bind(WidgetId::new("queue_title"), Trigger::Click, |event, ctx| ...)` 绑定；返回 `Some(Msg::...)` 会排队一条消息，并且处理器返回后内置行为仍会运行（`ctx.run_default()` 会内联运行它，两者都不抑制它）。该标题没有内置行为，因此绑定仅报告文本行本身的按下。

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

对于标题没有任何触发器有意义；指针触发器只会在标签自身的 24 DIP 带上触发，而 `Activate`、按键/焦点触发器和 `ValueChange` 永远不会触发，因为该控件不可聚焦且不携带值。

## XML 定义

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。

- **几何：** 适用普通的 `widget` 规则；24 DIP 是设计高度。
- **内容：** `label`（`content_caps = LABEL`）写入文本；不支持 `text`。
- **局部属性：** `color`（一个颜色值，令牌或字面量）。未知名称或格式错误的字面量会得到 `xml.skin.value`。
- **样式令牌：** 适用 `font-size` 和 `font-family`；该控件没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 是 `xml.style.unsupported` 警告，会被跳过。
- **子控件：** 无。
- **属性：** 适用通用运行时集合，包括 `role`（它可以覆盖默认的 `heading` 令牌）和 `name`。

## 另见

- `../content/label.md`
- `group-box.md`
- `card.md`
- `../index.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
