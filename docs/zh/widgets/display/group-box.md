# GroupBox

> 一个带标题的框架，用于将放置在其内的普通控件分组。

## 介绍

`GroupBox` 绘制一个类似 fieldset 的框架：卡片色填充、显式边框颜色的 1 DIP 边框、顶部带内的小号标题，以及该带下方的一条 1 DIP 分隔线。该带高 28 DIP，除非矩形宽于 20 DIP 且高于 28，否则标题和分隔线都会被跳过。

该框架仅为装饰。`Ui` 没有容器树，因此应用程序将普通控件作为兄弟控件放在该矩形内：先添加 group box 使其绘制在它们之后，并留出 28 DIP 的标题带用于标题。该控件不接受输入、不发出消息且不可聚焦；其无障碍节点是一个仅用于显示的 `Group`，只有当实例选择加入时才会加入控件树。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{GroupBox, Toggle};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Shuffle(bool) }

let mut ui: Ui<Msg> = Ui::new();
ui.add(GroupBox::new("playback_group", "Playback options"));
ui.add(Toggle::new("shuffle", "Shuffle", false, Msg::Shuffle));

ui.set_rect(WidgetId::new("playback_group"), Rect::from_xywh(16.0, 16.0, 240.0, 120.0));
ui.set_rect(WidgetId::new("shuffle"), Rect::from_xywh(28.0, 56.0, 180.0, 24.0));
```

`header_height()` 返回 28 DIP 的带，因此布局可以计算第一个子控件从哪里开始，而无需硬编码偏移。在运行时重命名该组通过共享的标签设置器完成：

```rust
ui.set_label(WidgetId::new("playback_group"), "Playback");
```

## 用法（XML）

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址：

```xml
<widget id="playback_group" x="16" y="16" w="240" h="120" label="Playback options"/>
```

`label` 是共享内容，会写入标题。该框架没有控件局部属性，因此其外观是固定的。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, title)` | `&'static str, impl Into<String>` | — | 创建一个 `240 × 120` 的框架。 |
| `.title()` | `&str` | — | 读取标题。 |
| `.header_height()` | `f32` | `28.0` | 标题带的高度；子控件从其下方开始。 |

### 行为

- 首选高度为 120 DIP，且该控件不可聚焦。
- 该框架在 `card` 之上绘制 `explicit_border`；它不采用 `SurfaceStyle`，因此没有圆角、背景或透明性覆盖。
- 标题是位于 `left + 10` 处的小号文本，在带内顶部对齐；分隔线在 `top + 28` 处从 `left + 1` 延伸到 `right - 1`。
- `Ui::set_label` 会静默重命名它（`content_caps = LABEL`）；没有公开的 `set_title`。
- 无障碍：`Group`、仅显示，以标题命名。

## 事件

该框架是装饰：其 `input` 始终返回 `None`，它不可聚焦，且不发出任何消息。事件归属于放置在其内的控件，而不是框架本身。

### 消息

无。构造函数不接收回调。

### 通用观察器

该控件不安装任何共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。

### 控件专属回调

无。

### Ui 触发器

任何控件 id 都可以用 `Ui::bind(WidgetId::new("playback_group"), Trigger::Click, |event, ctx| ...)` 绑定；返回 `Some(Msg::...)` 会排队一条消息，并且处理器返回后内置行为仍会运行（`ctx.run_default()` 会内联运行它，两者都不抑制它）。group box 没有内置行为，因此绑定仅报告落在框架本身上的按下，而不会报告落在子控件上的按下。

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

正常使用中没有任何触发器有意义；指针触发器只会在框架自身的区域上触发，而 `Activate`、按键/焦点触发器和 `ValueChange` 永远不会触发，因为框架不可聚焦且不携带值。

## XML 定义

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。

- **几何：** 适用普通的 `widget` 规则；为其提供至少 20 × 28 DIP，否则标题带会被省略。
- **内容：** `label`（`content_caps = LABEL`）写入标题；不支持 `text`。
- **局部属性：** 无。`skin_attrs()` 为空。
- **样式令牌：** 适用 `font-size` 和 `font-family`；`radius`、`corners` 和 `background` 是 `xml.style.unsupported` 警告，会被跳过。
- **子控件：** 无。其内显示的控件是布局文档中的普通兄弟控件，而不是该组的子控件。
- **属性：** 适用通用运行时集合，包括 `expose`，这正是该组加入无障碍树的方式。

## 另见

- `card.md`
- `section-title.md`
- `../navigation/accordion.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
