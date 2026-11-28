# TooltipCard

> 一个带样式的页内提示面板，包含标题和正文行。

## 介绍

`TooltipCard` 为位于页面内的提示绘制一个小型圆角面板：正文样式下的一个标题，以及可选的一行小号柔和样式正文，位于面板颜色上 1 DIP 的边框内，使用中等圆角半径。它在绘制和命中测试顺序中是一个普通控件，因此应用程序用常规矩形和可见性 API 来定位、显示和隐藏它。

这不是原生宿主的 tooltip。M4 的 tooltip 是一个独立的顶层窗口，平台为 `WidgetProps::tooltip` 显示它，具有自己的停留和锚点行为。`TooltipCard` 只共享调色板家族和圆角形状；它是一个页面控件，用于内联说明、标注和引导提示。

该卡片仅用于显示：它不可聚焦，不接受输入，且不发出任何消息。其无障碍节点是一个仅用于显示的 `Text`，以标题命名，正文作为描述。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::TooltipCard;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(TooltipCard::new("hint", "Now playing").body("Space toggles playback."));
ui.set_rect(WidgetId::new("hint"), Rect::from_xywh(296.0, 120.0, 240.0, 56.0));
```

将它放在提示应出现的位置，例如它所说明的控件旁边。显示和隐藏就是通常的可见性切换；该卡片没有自身的打开/关闭状态。

对于应在悬停某个控件时出现的提示，请改用通用的 `WidgetProps::tooltip`：该路径会打开原生 tooltip 窗口，并且还会输入无障碍帮助文本。

## 用法（XML）

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。文档可以为它定位并设置诸如 `visible`、`expose` 和 `name` 之类的通用属性：

```xml
<widget id="hint" x="296" y="120" w="240" h="56" name="Playback hint"/>
```

标题和正文仅在构造时设置；文档无法设置它们。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, title)` | `&'static str, impl Into<String>` | — | 创建一个正文为空的 `240 × 56` 卡片。 |
| `.body(text)` | `impl Into<String>` | `""` | 小号柔和样式的第二行。 |

### 行为

- 该面板使用 medium 半径令牌（6 DIP）、`explicit_border` 轮廓和 `panel` 填充；两者都是直接绘制的，而非通过 `SurfaceStyle`。
- 仅有标题的卡片保持标题自身的行高；添加正文后，布局会切换为从顶部开始绘制的双行排列。
- 首选高度为 56 DIP，且该控件不可聚焦。
- 标题和正文没有静默设置器；请在构造时重新构建或传入新文本。
- 无障碍：`Text`、仅显示，name = 标题，description = 正文。

## 事件

`TooltipCard` 仅用于显示：其 `input` 始终返回 `None`，因此它不发出任何消息，也不触发自身的任何回调。显示和隐藏是应用程序执行的普通可见性切换。

### 消息

无。构造函数不接收回调。

### 通用观察器

该控件不安装任何共享的观察器构建器。它不可聚焦，也不消费输入，因此其上不存在 `on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained` 和 `on_focus_lost`。

### 控件专属回调

无。

### Ui 触发器

任何控件 id 都可以用 `Ui::bind(WidgetId::new("hint"), Trigger::Click, |event, ctx| ...)` 绑定；返回 `Some(Msg::...)` 会排队一条消息，并且处理器返回后内置行为仍会运行（`ctx.run_default()` 会内联运行它，两者都不抑制它）。该卡片没有内置行为，因此绑定是做出响应的唯一方式。

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

只有指针触发器对提示卡片有意义；`Activate`、按键和焦点触发器永远不会触发，因为它不可聚焦；它也没有值，因此 `ValueChange` 也不会触发。

## XML 定义

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。

- **几何：** 适用普通的 `widget` 规则；240 DIP 宽适合一行正文。
- **内容：** 无。不支持 `label` 和 `text`（`xml.content.unsupported`，一条警告），并会被跳过。
- **局部属性：** 无。`skin_attrs()` 为空。
- **样式令牌：** 适用 `font-size` 和 `font-family`；该卡片没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 是 `xml.style.unsupported` 警告，会被跳过。
- **子控件：** 无。
- **属性：** 适用通用运行时集合（`visible`、`tooltip`、`name`、`description`、`expose`、`automation-id`、……）。

## 另见

- `banner.md`
- `../display/card.md`
- `../content/label.md`
- `../index.md`
- `../../api/widget-trait.md`
