# Card

> 一个带标题和正文行的表面面板。

## 介绍

`Card` 绘制一个有边框的表面：分隔线颜色的 1 DIP 边框、`card` 令牌的填充、正文样式的标题，以及小号柔和样式的正文行，水平方向按卡片的 padding 内缩。`emphasis(true)` 会在前缘添加一条 3 DIP 的强调色条，并将文本移过它，标记出被选中或高亮的卡片。

该控件仅用于显示：它不可聚焦，不接受输入，且不发出任何消息。它采用 `SurfaceStyle`，因此圆角形状、填充和透明性都来自标准的表面构建器；默认表面为直角。其无障碍节点是一个仅用于显示的 `Group`，以标题命名，正文作为描述。

尽管名为卡片，它并不是容器：`Ui` 是一个扁平的控件列表，因此请自行在卡片矩形之上放置其他控件，并让它们在绘制顺序中位于卡片之后。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::Card;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Card::new("summary", "Now playing", "Track 3 of 12")
        .padding(14.0)
        .corner_radius(10.0),
);
ui.set_rect(WidgetId::new("summary"), Rect::from_xywh(16.0, 16.0, 240.0, 96.0));
```

标题位于顶部内缩之下最初的 24 DIP 内，正文从其下方开始，因此比剩余高度更长的正文会被裁剪而不是换行。通过带类型的接缝静默更新文本，并报告该变化，以便卡片被标记为损坏：

```rust
ui.edit_widget::<Card>(WidgetId::new("summary"), |card| {
    card.set_body("Track 4 of 12");
    true
});
```

## 用法（XML）

`card` 是标准注册表类型。安装了 `WidgetRegistry::standard()` 后，用 `type="card"` 创建它：

```xml
<widget id="summary" type="card" x="16" y="16" w="240" h="96"
        title="Now playing" body="Track 3 of 12" emphasis="true"
        radius="large"/>
```

工厂会构建空卡片；局部属性在同一代作用域的遍历中到达。在 Rust 中创建的卡片通过 id 寻址：

```xml
<widget id="summary" x="16" y="16" w="240" h="96" title="Now playing"/>
```

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, title, body)` | `&'static str, impl Into<String>, impl Into<String>` | — | 创建一个 `240 × 96` 的卡片。 |
| `.padding(f32)` | `f32` | `14.0` | 内边距，单位为 DIP，钳制为不低于零。 |
| `.emphasis(bool)` | `bool` | `false` | 绘制 3 DIP 的强调色条并移动文本。 |
| `.title()` / `.body()` | `&str` | — | 读取当前文本。 |
| `.set_title(text)` | `impl Into<String>` | — | 静默替换标题。 |
| `.set_body(text)` | `impl Into<String>` | — | 静默替换正文。 |
| `.is_emphasis()` | `bool` | `false` | 当前强调状态。 |
| `.set_emphasis(bool)` | `bool` | — | 静默切换强调（皮肤的 `emphasis`）。 |

表面构建器：`corner_radius(f32)`、`corners(Corners)`、`background(Color)`、`background_option(Option<Color>)`、`transparent_background(bool)` 和 `surface()`。

### 行为

- 边框始终绘制；`transparent_background(true)` 仅去掉填充。
- `background` 覆盖会替换 `card` 令牌填充。
- 首选高度为 96 DIP，且该控件不可聚焦。
- `Ui::set_label` 没有接入该卡片（`content_caps` 为 `NONE`）；请使用上面的静默设置器。
- 无障碍：`Group`、仅显示，name = 标题，description = 正文。

## 事件

`Card` 是装饰：其 `input` 始终返回 `None`，它不可聚焦，且不发出任何消息。要让卡片可点击，就把触发器绑定到其 id，并把卡片用作命中区域。

### 消息

无。构造函数不接收回调。

### 通用观察器

该控件不安装任何共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。

### 控件专属回调

无。

### Ui 触发器

任何控件 id 都可以用 `Ui::bind(WidgetId::new("summary"), Trigger::Click, |event, ctx| ...)` 绑定；返回 `Some(Msg::...)` 会排队一条消息，并且处理器返回后内置行为仍会运行（`ctx.run_default()` 会内联运行它，两者都不抑制它）。该卡片没有内置行为，因此绑定就是全部交互。

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

只有指针触发器有意义，且仅在卡片被用作可点击磁贴时；`Activate`、按键和焦点触发器永远不会触发，因为它不可聚焦；它也没有值，因此 `ValueChange` 也不会触发。

## XML 定义

- **创建：** `type="card"`，无需 `command`；若没有标准注册表，该条目为 `xml.type.unknown`。创建的控件落在页面层，位于第一个背景（`Scrim`）之前。
- **几何：** 适用普通的 `widget` 规则。
- **内容：** 无。不支持 `label` 和 `text`（`xml.content.unsupported`，警告），并会被跳过；文本是局部属性。
- **局部属性：** `title`（文本）、`body`（文本）和 `emphasis`（bool）。三者都是静默写入。
- **样式令牌：** `radius`、`corners`、`background`、`font-size` 和 `font-family`，因为卡片采用 `SurfaceStyle`。
- **子控件：** 无。可选的 `parent="panel-id"` 会将创建的条目绑定到已注册的滚动面板。

## 另见

- `group-box.md`
- `stat-card.md`
- `../feedback/banner.md`
- `../index.md`
- `../../api/theme-and-palette.md`
