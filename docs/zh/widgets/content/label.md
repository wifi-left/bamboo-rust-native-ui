# Label

> 带可选链接样式和 chip 背景的静态单行或换行文本。

## 介绍

`Label` 使用主题文本角色（`LabelStyle::Body`、`Small` 或 `Title`）绘制单个字符串。
单行标签默认以省略号截断（当 `.ellipsis(false)` 时则硬裁剪）；`.wrap(true)` 会在矩形高度
容纳的范围内将文本排布到多行，硬 `\n` 始终另起一行。文本可以左对齐、居中对齐或右对齐。

普通标签只是装饰。`focusable` 为 `false`，无障碍树看到的是一个仅显示的 `Text` 节点。
链接标签（`.link(true)`）以强调文本颜色绘制，带 1 DIP 下划线，接收指针输入以便 `Click`
绑定触发，并显示手形光标；它仍然不是一个 Tab 停靠点，因此当键盘激活很重要时，
需要应用主动选择启用焦点。

标签还可以通过其 `SurfaceStyle`（`.background`、`.corner_radius`）绘制 chip 风格背景；
普通标签不绘制填充。控件自身从不发送消息，因此 `M` 只是容器的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{Label, LabelStyle};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { OpenLyrics }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        Label::new("now_title", "Now playing")
            .style(LabelStyle::Title)
            .wrap(true),
    );
    ui.set_rect(WidgetId::new("now_title"), Rect::from_xywh(16.0, 16.0, 320.0, 30.0));

    ui.add(Label::new("lyrics_link", "Open lyrics ›").link(true));
    ui.set_rect(WidgetId::new("lyrics_link"), Rect::from_xywh(16.0, 54.0, 160.0, 22.0));
    ui.on_click(WidgetId::new("lyrics_link"), |_event, _ctx| Some(Msg::OpenLyrics));
}
```

第二个构造函数参数是文本。矩形在 `add` 之后设置，因为框架没有测量阶段；
`preferred_height` 仅为需要调整行尺寸的宿主报告 20（Small）、24（Body）或 30（Title）DIP。
链接标签需要上面展示的 `on_click`（或 `on_activate`）绑定。

## 用法（XML）

`label` 是标准注册表类型之一，因此当通过 `Ui::set_widget_registry` 安装了
`WidgetRegistry::standard()` 后，文档即可创建它：

```xml
<widget id="now_title" type="label" x="16" y="16" w="320" h="30"
        label="Now playing" text-style="title"/>
```

在 Rust 中创建的标签则通过 id 引用，不带 `type`：

```xml
<widget id="lyrics_link" x="16" y="54" w="160" h="22"/>
```

文件可以在任一形式下本地设置 `color` 和 `text-style`。`wrap`、`ellipsis`、`align` 和
`link` 只能在构造函数中设置，无法在 XML 中表达。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, text)` | `&'static str, impl Into<String>` | — | 创建标签。 |
| `.wrap(bool)` | `bool` | `false` | 在高度容纳的范围内换行到多行。 |
| `.ellipsis(bool)` | `bool` | `true` | 单行边缘处理：省略号，或当为 `false` 时硬裁剪。 |
| `.align(TextAlign)` | `TextAlign` | `TextAlign::Left` | 水平对齐方式。 |
| `.style(LabelStyle)` | `LabelStyle` | `LabelStyle::Body` | 主题文本角色。 |
| `.link(bool)` | `bool` | `false` | 强调文本加下划线、指针输入和手形光标。 |
| `.color(Color)` | `Color` | 调色板 `text`（链接：`accent_text`） | 显式文本颜色。 |
| `.corner_radius(f32)` | `f32` | `0.0` | chip 填充的统一圆角半径。 |
| `.corners(Corners)` | `Corners` | 零 | chip 填充的逐角半径。 |
| `.background(Color)` | `Color` | `None` | chip 填充颜色；`None` 不绘制填充。 |
| `.background_option(Option<Color>)` | `Option<Color>` | `None` | 设置或清除 chip 填充。 |

Getter 与 setter：`text()`、`color_value()`、`text_style()`、`surface()`、
`set_color(Option<Color>)` 和 `set_text_style(LabelStyle)`。容器的
`Ui::set_label` 会静默替换文本。

## 事件

### 消息

无。标签自身从不发送消息；`M` 只是容器的消息类型。

### 通用观察器

`Label` 不安装共享观察器构建器。普通标签是装饰（`interactive()` 为 `false`，
`focusable()` 为 `false`），因此没有输入会路由给它。`.link(true)` 标签接收指针输入并触发
`Click` 绑定；任何标签都可以通过 `WidgetProps::mouse(true)` 选择启用指针输入。
请使用 `Ui::bind` / `Ui::on_click` / `Ui::on_activate`，而不是控件观察器。

### 控件专属回调

无。

### Ui 触发器

任意控件 id 都可以用 `Ui::bind(id, Trigger, handler)`（或 `Ui::on_click` /
`Ui::on_activate` 简写）进行绑定。返回 `Some(M)` 的处理器会将消息入队；除非处理器调用
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

只有 `Click` 有意义，且仅在链接标签或选择启用鼠标的标签上；悬停只会切换手形光标。
`Activate` 需要应用先将标签设为可聚焦。

## XML 定义

`label` 是标准注册表类型。`type="label"` 条目会创建该控件（工厂文本为空；文件的
`label` 内容会在插入后立即写入），并且需要安装 `WidgetRegistry::standard()`；
由于该控件默认不可交互，不会读取 `command`。

- 几何：适用常规 `widget` 规则。横向形式（`x`+`w`、`right`+`w`、`x`+`right`）恰好选择
  一种，纵向形式（`y`+`h`、`bottom`+`h`、`y`+`bottom`）恰好选择一种。
- 共享内容：仅 `label`。不接受 `text`（`content_caps` 公开 `LABEL`）。
- 本地属性：`color`（颜色语法或调色板 token）和 `text-style`
  （`body`、`small`、`title`）。
- 样式 token：支持 `radius`、`corners` 和 `background`（它们塑造 chip 填充）；
  `font-size` 和 `font-family` 也同样适用。
- 子元素：无。

## 另见

- `../index.md`
- `../controls/button.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
- `../../api/widget-trait.md`
