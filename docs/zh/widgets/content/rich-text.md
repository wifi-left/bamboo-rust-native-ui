# RichText

> 带内联强调和可点击链接片段的换行文本。

## 介绍

`RichText<M>` 将一系列 `RichRun` 值排布为一个段落。各片段一起测量，因此单词换行可以跨越
片段边界，且断点处的空格不会被绘制。每个片段有一个 `RunStyle`：

- `Normal` 使用调色板文本颜色。
- `Strong` 使用半粗字重。
- `Muted` 使用调色板的弱化颜色。
- `Link` 使用强调文本颜色，绘制下划线，并可点击。

安装 `.on_link(handler)` 后，点击链接片段会发出 `handler(run.link)`；光标仅在链接矩形上
为手形，整个字符串仍是无障碍名称。没有处理器时控件不可交互。`RichRun` 是值类型，
`RunStyle` 是强调枚举；两者都由 `bamboo_native_ui::widgets` 重新导出。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{RichRun, RichText, RunStyle};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Link(u32) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        RichText::new("note")
            .run("Inline styles: ")
            .strong("bold")
            .run(", ")
            .muted("muted")
            .run(", ")
            .link(7, "details")
            .on_link(Msg::Link),
    );
    ui.set_rect(WidgetId::new("note"), Rect::from_xywh(16.0, 16.0, 320.0, 40.0));

    ui.add(RichText::new("explicit").runs(vec![
        RichRun::new("Hello "),
        RichRun::new("world").style(RunStyle::Strong),
    ]));
    ui.set_rect(WidgetId::new("explicit"), Rect::from_xywh(16.0, 64.0, 320.0, 24.0));
}
```

`.link(id, text)` 存储一个不透明的 `u32` id，处理器会接收到它；用它来区分不同链接。
`plain_text()` 返回拼接后的各片段，它同时也是无障碍名称和复制文本。

## 用法（XML）

`RichText` 不在标准注册表中。在 Rust 中创建它，并从文档中通过 `id` 引用。
`<run>` 子元素列表会替换内联片段：

```xml
<widget id="note" x="16" y="16" w="320" h="40" wrap="true" align="left">
  <run text="Inline styles: "/>
  <run text="bold" style="strong"/>
  <run text=", " />
  <run text="details" style="link" link="7"/>
</widget>
```

同一条目上的共享 `label` 会在子元素之后写入，因此会用单个普通片段替换所有片段。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建空的富文本。 |
| `.run(text)` | `impl Into<String>` | — | 追加一个普通片段。 |
| `.strong(text)` | `impl Into<String>` | — | 追加一个半粗片段。 |
| `.muted(text)` | `impl Into<String>` | — | 追加一个弱化片段。 |
| `.link(id, text)` | `u32, impl Into<String>` | — | 追加一个携带 `id` 的链接片段。 |
| `.runs(Vec<RichRun>)` | `Vec<RichRun>` | 空 | 替换所有片段。 |
| `.wrap(bool)` | `bool` | `true` | 在单词边界处换行长行。 |
| `.align(TextAlign)` | `TextAlign` | `TextAlign::Left` | 行对齐方式。 |
| `.style(LabelStyle)` | `LabelStyle` | `LabelStyle::Body` | 基础主题文本角色。 |
| `.on_link(handler)` | `impl Fn(u32) -> M + 'static` | `None` | 点击时发出链接 id。 |

运行时接缝：`set_wrap(bool)`、`set_align(TextAlign)`、
`set_text_style(LabelStyle)`（均返回是否发生变化）、
`set_color(Option<Color>)`、`set_runs(Vec<RichRun>)`、`wrap_enabled()`、
`alignment()`、`text_style()`、`color_value()`、`runs_ref()` 和
`plain_text()`。

### `RichRun` 与 `RunStyle`

| 项 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `RichRun::new(text)` | `impl Into<String>` | — | 创建具有 `Normal` 样式和链接 id 0 的片段。 |
| `RichRun::style(RunStyle)` | `RunStyle` | `RunStyle::Normal` | 强调。 |
| `RichRun::text` | `String` | — | 公共字段；片段的文本。 |
| `RichRun::link` | `u32` | `0` | 公共字段；传给链接处理器的 id。 |

`RunStyle` 有 `Normal`（默认）、`Strong`、`Muted` 和 `Link`。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `.on_link(handler)` 的 `Fn(u32) -> M` | 点击落在 `RunStyle::Link` 片段上；参数是该片段的链接 id。 |

### 通用观察器

`RichText` 不安装共享观察器构建器。它唯一的回调是控件专属的 `.on_link(handler)`；
没有它时控件不可交互，且没有片段可点击。

### 控件专属回调

| 回调 | 处理器 | 触发条件 |
|---|---|---|
| `.on_link(handler)` | `impl Fn(u32) -> M + 'static` | 点击落在链接片段绘制的矩形内；接收该片段的 `link` id。 |

### Ui 触发器

任意控件 id 都可以用 `Ui::bind(id, Trigger, handler)`（或 `Ui::on_click` /
`Ui::on_activate` 简写）绑定。返回 `Some(M)` 的处理器会将消息入队；除非处理器调用
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

只有 `Click` 有意义，且仅在设置了 `.on_link` 时、在链接片段上才有意义；
`HoverEnter`/`HoverLeave` 仅在此类片段上切换手形光标。其余触发器不予处理。

## XML 定义

此控件不由标准注册表创建。在 Rust 中创建它，并从文档中通过 `id` 引用。

- 几何：适用常规 `widget` 规则。
- 共享内容：仅 `label`。它会用单个普通片段替换所有片段。
- 本地属性：`wrap`（bool）、`align`（`left`、`center`、`right`）、
  `text-style`（`body`、`small`、`title`）和 `color`（普通/加粗片段；
  弱化和链接片段保持其调色板角色）。
- 子元素：`<run>`，带 `text`（必填、非空）、`style`
  （`normal`、`strong`、`muted`、`link`）和 `link`（`u32`，仅当
  `style="link"` 时合法）。`<run>` 不带子元素。
- 样式 token：无 `SurfaceStyle`；`radius`、`corners` 和 `background` 不受支持
  （`xml.style.unsupported`，一个警告）。

## 另见

- `../index.md`
- `label.md`
- `copyable-text.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
