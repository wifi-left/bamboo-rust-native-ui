# Banner

> 一条内联通知条，带有类别、可选操作和可选关闭目标。

## 介绍

`Banner<M>` 在页面内绘制一条通知：一个带色调、带边框的面板，包含一个前置图标字形、一个标题、一个可选正文行，以及可选的右对齐操作标签和一个 `×` 关闭目标。类别决定强调色，因此无论主题强调色如何，警告始终保持黄色、错误始终保持红色；面板填充是该颜色混入面板令牌 14% 的结果，边框则将其与面板混合。

在附加处理器之前，banner 只是装饰。使用 `.action(...)` 或 `.closable(...)` 后它变得可交互：它会采用手形光标，跟踪两个目标上的悬停，并在点击时发出处理器的消息。绘制的操作矩形按其文本测量，因此悬停和点击保持一致。`M` 是任一处理器返回的消息类型。

无障碍节点是一个仅用于显示的 `Group`，以标题命名，正文作为其描述。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{Banner, BannerKind};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Retry, Dismiss }

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Banner::new("update", BannerKind::Info, "Update available")
        .body("Version 0.2 is ready and installs on restart.")
        .action("Retry", || Msg::Retry)
        .closable(|| Msg::Dismiss),
);
ui.set_rect(WidgetId::new("update"), Rect::from_xywh(16.0, 16.0, 420.0, 64.0));
```

带正文的 banner 会绘制两行文本，并偏好 64 DIP 的高度；仅有标题时它偏好 44。构造函数的矩形为 `420 × 60`，因此请为它提供与其内容匹配的高度。

## 用法（XML）

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址：

```xml
<widget id="update" x="16" y="16" w="420" h="64"
        label="Update available" body="Version 0.2 is ready." radius="medium"/>
```

`label` 是共享内容，会写入标题（`set_label`）。`body` 是控件局部属性。类别、操作和关闭处理器仅在构造时设置，无法在 XML 中声明。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, kind, title)` | `&'static str, BannerKind, impl Into<String>` | — | 创建一个正文为空的 `420 × 60` banner。 |
| `.body(text)` | `impl Into<String>` | `""` | 小号柔和样式的第二行。 |
| `.body_text()` | `&str` | — | 读回正文。 |
| `.set_body(text)` | `impl Into<String>` | — | 静默替换正文（皮肤的 `body`）。 |
| `.action(label, handler)` | `impl Into<String>, impl Fn() -> M + 'static` | `None` | 右对齐的操作标签及其消息。 |
| `.closable(handler)` | `impl Fn() -> M + 'static` | `None` | 绘制一个发出该处理器的 `×` 目标。 |
| `.kind()` | `BannerKind` | 构造值 | 读取类别。 |
| `.title()` | `&str` | 构造值 | 读取标题。 |

`BannerKind` 包括 `Info`（默认）、`Success`、`Warning`、`Error`，固定颜色为 `(84,132,220)`、`(52,132,88)`、`(176,136,34)` 和 `(178,42,42)`。

### 行为

- 关闭目标是一个 20 DIP 的正方形，距右上角内缩 8 DIP。
- 操作位于关闭目标左侧，其尺寸为标签宽度加 16 DIP；在两个目标之外点击不会发出任何内容。
- 仅当存在操作或关闭处理器时 `interactive()`（以及手形光标）才为 true；也只有此时才会请求悬停重绘。
- 实例采用 `SurfaceStyle`：`radius` 默认使用 medium 令牌（6 DIP），并且 `corner_radius`、`corners`、`background`、`transparent_background` 和 `surface` 都可用。`background` 覆盖会替换由类别推导出的填充；边框仍会绘制。
- 无正文时首选高度为 44，有正文时为 64。
- 无障碍：`Group`、仅显示，name = 标题，description = 正文，即使实例可交互也是如此。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `.action` 的 `Fn() -> M` | 左键点击落在绘制的操作标签内。 |
| 传给 `.closable` 的 `Fn() -> M` | 左键点击落在 20 DIP 的 `×` 目标内。 |

在其他任何位置（图标、标题、正文）点击都不会发出任何内容。

### 通用观察器

banner 不安装共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。下面的回调是它的事件接缝；其他任何需求都通过 `Ui::bind` 处理。

### 控件专属回调

| 方法 | 处理器 | 触发时机 |
|---|---|---|
| `.action(label, handler)` | `impl Into<String>, impl Fn() -> M + 'static` | 点击右对齐的操作标签；同时使 banner 变为 `interactive()`（手形光标、悬停跟踪）。 |
| `.closable(handler)` | `impl Fn() -> M + 'static` | 点击关闭目标；同时使 banner 变为 `interactive()`。 |

### Ui 触发器

任何 id 都可以用 `Ui::bind(WidgetId::new("update"), Trigger::Click, |event, ctx| ...)` 绑定；`Some(Msg::...)` 会被排队，并且内置操作之后仍会运行（`ctx.run_default()` 会内联运行它，两者都不抑制它）。使用 `event.pos` 区分操作点击和关闭点击。

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

此处 `Click` 和 `HoverEnter`/`HoverLeave` 有意义；banner 不可聚焦，因此 `Activate` 和按键/焦点触发器永远不会触发；它也不携带值，因此 `ValueChange` 也不会触发。

## XML 定义

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。

- **几何：** 适用普通的 `widget` 规则。
- **内容：** `label`（`content_caps = LABEL`）写入标题；不支持 `text`。
- **局部属性：** `body`（文本）。皮肤写入是静默的，不会触发回调。
- **样式令牌：** `radius`、`corners`、`background`、`font-size` 和 `font-family`，因为 banner 采用 `SurfaceStyle`。`background="none"` 保留边框并去掉填充。
- **子控件：** 无。
- **无法表达：** `kind`、操作标签/处理器和关闭处理器仅为构造函数参数。

## 另见

- `toast.md`
- `../display/card.md`
- `../index.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
