# StatusBar

> 一条只读的底部条带，包含状态段、一个可选的进度段和一个尺寸手柄。

## 介绍

`StatusBar` 在 `palette.panel` 上绘制一条由 `StatusSegment` 单元格组成的水平条带，带有一条 1-DIP 的顶部分隔线和段之间的细分隔线。每个段携带文本、一个可选的前导图标和一个可选的固定宽度；未设置宽度的段共享剩余空间。一个右对齐的进度段和经典的斜向尺寸手柄可以占据右边缘，并从段宽度中预留出来。

该条带对于指针和键盘输入是装饰性的：它不可聚焦、不交互，也不是 Tab 停靠点。它仍然发布一个 `StatusBar` 节点，每个段一个文本子项，以便屏幕阅读器可以读取它。段 0 是消息段，当应用没有类型化访问时，通用的 `Ui::set_text` 路径会更新它。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{StatusBar, StatusSegment};
use bamboo_native_ui::{Rect, TextAlign, Ui, WidgetId};

enum Msg {
    Tick,
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    StatusBar::new("status")
        .segment(StatusSegment::new("Ready").icon('♪'))
        .segment(
            StatusSegment::new("600 tracks · lossless")
                .align(TextAlign::Right)
                .width(160.0),
        )
        .segment(StatusSegment::new("44.1 kHz").align(TextAlign::Right).width(96.0))
        .size_grip(true)
        .progress(0.62)
        .label("Status"),
);
ui.set_rect(WidgetId::new("status"), Rect::from_xywh(0.0, 576.0, 800.0, 24.0));
```

`progress` 钳制到 `0..=1`；使用 `set_progress` 更新它而不产生消息，该方法返回值是否发生变化。使用 `set_segment(index, text)` 更新某个段的文本。

## 用法（XML）

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。其子列表为 `<segment>`，共享的 `text` 和 `progress` 内容属性也适用：

```xml
<widget id="status" x="0" y="576" w="800" h="24"
        text="Ready" progress="0.62">
  <segment text="600 tracks · lossless" align="right" width="160"/>
  <segment text="44.1 kHz" icon="♫" width="96"/>
</widget>
```

应用器先写入子列表，然后写入共享内容，因此 `text` 会覆盖第一个段的文本；`progress` 会添加（或更新）右对齐的进度段。段 0 请使用其中一种风格。

## 属性

`StatusBar`：

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建一个空条带；`420 × 24` DIP。 |
| `.segment(StatusSegment)` | segment | — | 追加一个段。 |
| `.text(text)` | text | — | 追加一个普通的左对齐段。 |
| `.label(text)` | text | 控件 id | 该条带的无障碍名称。 |
| `.size_grip(bool)` | `bool` | `false` | 绘制斜向手柄并预留 14 DIP。 |
| `.progress(f32)` | `f32` | `None` | 添加一个右对齐的进度条，钳制为 `0..=1`。 |
| `.segments()` / `.set_segments(Vec<StatusSegment>)` | segments | — | 读取或替换所有段（静默）。 |
| `.has_size_grip()` / `.progress_value()` | `bool` / `Option<f32>` | — | 当前选项。 |
| `.set_progress(f32)` | `bool` | — | 运行时进度（`0..=1`）；返回其是否发生变化。 |
| `.set_segment(index, text)` | `bool` | — | 运行时段文本；返回其是否发生变化。 |

`StatusSegment`：

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(text)` | `impl Into<String>` | — | 一个左对齐的段。 |
| `.icon(char)` | `char` | 无 | 前导 MDL2 码点或文本字符（20 DIP 条带）。 |
| `.width(f32)` | `f32` | 弹性 | 固定宽度，钳制为 `>= 20`；未设置宽度的共享其余部分。 |
| `.align(TextAlign)` | `TextAlign` | `Left` | 段内文本对齐方式。 |

### 行为

- **布局。** 每个段从边缘内缩 10 DIP；段分隔线从顶部和底部内缩 5 DIP 绘制。进度段在右边缘预留 96 DIP，手柄预留 14 DIP。
- **绘制。** 进度值绘制一条垂直居中的 6-DIP 轨道，带有强调色填充；手柄是 `scroll_thumb` 中的三行斜向点。
- **无障碍。** 角色为 `StatusBar`，仅显示，以 `.label`（或控件 id）命名。子项是各个段，当设置了进度值时还会有一个名为 `进度 N%` 的 `ProgressBar` 子项。

## 事件

### 消息

`StatusBar` 不发出任何消息。它没有构造函数消息回调，并且是只读的：应用通过 `set_progress`、`set_segment(index, text)`、`set_segments` 或通用的 `Ui::set_text` 更新它，这些都不会发出消息。

### 通用观察器

此控件不安装任何共享的观察器构建器。它不可聚焦、不交互，也不是 Tab 停靠点，因此没有可观察的输入。

### 控件专属回调

无。该条带公开静默 setter（`set_progress`、`set_segment`、`set_segments`）和一个 `Ui::set_text` 路径，而不是回调。

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("status"), Trigger::Click, |event, ctx| {
    // 返回 Some(Msg::...) 以排队一条消息；调用 ctx.default() 以保留
    // 内置动作，随后会在处理函数之后运行。
    None
});
```

| 触发器 | 触发条件 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告一次双击。 |
| `RightClick` | 次键点击，在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开该控件。 |
| `Wheel` | 控件上方的滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 在获得焦点的控件上按 Enter/Space，或一次 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 按钮被释放。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

该条带不响应其中任何一个：它仅显示且无法获得焦点，因此没有任何指针或键盘触发器会到达它。

## XML 定义

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。

几何遵循常规的 `<widget>` 规则。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`）适用。内容支持为 `text`（段 0）和 `progress`（进度段）。该条带从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported`。

控件局部属性：无（`size_grip` 和 `label` 是 Rust 构建器）。

子元素：

| 元素 | 属性 | 描述 |
|---|---|---|
| `<segment>` | `text`（必填，非空）、`icon`（一个字符）、`width`（数字，钳制为 `>= 20`）、`align`（`left`/`center`/`right`） | 一个单元格；该列表替换所有段。 |

## 另见

- `./toolbar.md`
- `./menu-bar.md`
- `../controls/progress-bar.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
