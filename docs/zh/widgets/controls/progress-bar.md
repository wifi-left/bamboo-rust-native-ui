# ProgressBar

> 一条确定或不确定的进度轨道，可选带标签或百分比。

## 介绍

`ProgressBar` 绘制一条 6 DIP 高、垂直居中于其高度的轨道，以及一段与 `value`（0.0 到 1.0）成比例的强调色填充。使用 `indeterminate()` 时，它改为绘制一个移动的片段——轨道长度的 30%，至少 24 DIP 宽——其位置由 `phase` 决定。用 `set_phase` 从应用定时器推进相位。

该进度条是一个显示控件：它不可聚焦、忽略输入，也不上报任何消息。标签或百分比会绘制在轨道右边缘上方。它采用 `SurfaceStyle`，因此半径、按角半径和背景覆盖会应用到凹槽和填充上。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::ProgressBar;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Tick,
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    ProgressBar::new("import", 0.35)
        .show_value(true)
        .corner_radius(3.0),
);
ui.set_rect(WidgetId::new("import"), Rect::from_xywh(16.0, 16.0, 200.0, 28.0));
```

用 `.indeterminate()` 启动一个未知时长的进度条，并从帧定时器推进它，例如使用定时器计算出的相位：

```rust
ui.edit_widget::<ProgressBar>(WidgetId::new("import"), |bar| bar.set_phase(0.37));
```

`set_phase` 使用 `rem_euclid(1.0)` 进行环绕，并返回相位是否发生了更改，因此定时器在数值未变时可以跳过重绘。

## 用法（XML）

`ProgressBar` 由标准注册表创建（`type="progress"`）。它没有消息，也不需要 `command`：

```xml
<widget id="import" type="progress"
        x="16" y="16" w="200" h="24" progress="0.35"
        background="none"/>
```

文件设置初始进度和样式。标签、数值读数、不确定模式和相位仅在 Rust 中可用；对创建的控件调用 `Ui::edit_widget`，或不带 `type` 声明并直接在 Rust 中构建它。`ProgressBar` 不声明任何控件局部属性。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, value)` | `&'static str, f32` | — | 创建该进度条；`200 × 28` DIP，数值钳制到 `0..=1`，无标签。 |
| `.label(impl Into<String>)` | `String` | `""` | 绘制在右边缘的文本。 |
| `.show_value(bool)` | `bool` | `false` | 在右边缘绘制 `"{:.0}%"` 而不是标签。 |
| `.indeterminate()` | — | `false` | 使用移动片段而非成比例的填充。 |
| `.phase(f32)` | `f32` | `0.0` | 起始相位，环绕到 `0..=1`。 |
| `.set_phase(f32)` | `f32` | — | 运行时推进相位；返回是否发生了更改。 |
| `.is_indeterminate()` | — | — | 是否绘制移动片段。 |
| `.value()` | — | — | `0..=1` 中的当前进度。 |
| `.corner_radius(f32)` | `f32` | `0.0` | 凹槽和填充的统一半径。 |
| `.corners(Corners)` | `Corners` | 零 | 按角半径。 |
| `.background(Color)` | `Color` | 主题 | 覆盖凹槽颜色。 |
| `.transparent_background(bool)` | `bool` | `false` | 仅显示填充。 |
| `.surface()` | `&SurfaceStyle` | — | 已配置的表面。 |

### 行为

- **数值来源。** `Ui::set_progress` 写入数值并将其钳制到 `0..=1`；它也接受文档的 `progress` 内容。
- **文本优先级。** `show_value` 优先于 `label`。在 `indeterminate` 模式下，百分比被抑制，改为绘制标签（如果有）。
- **尺寸。** 轨道始终为 6 DIP 高，垂直居中，并横跨整个宽度；更高的单元格只会在其周围增加空白。
- **无障碍性。** 角色为 `progress-bar`、`display_only`，名称来自标签或控件 id。确定的进度条上报一个 `0..=1` 的范围及其百分比；不确定的进度条改为上报状态文本而非范围。该实例仅在选择加入时才加入无障碍树（`WidgetProps::expose(true)`）。

## 事件

### 消息

无。`ProgressBar` 不上报任何消息；用 `Ui::set_progress` 或 `set_phase` 写入其状态。

### 通用观察器

该控件**不**安装共享的观察器构建器。它没有 `Observers` 字段，不可聚焦，且其 `Widget::input` 返回 `None`，因此也没有构造函数回调或控件专属处理函数。当需要触发器来触发应用侧效果时，在控件 id 上使用 `Ui::bind`。

### 控件专属回调

无。

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("import"), Trigger::ValueChange, |event, ctx| {
    // 返回 Some(Msg::...) 以排队一条消息；除非显式调用 ctx.run_default()，
    // 否则内置动作仍会执行。
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

进度条自身不响应其中任何一个：它是一个显示控件，因此绑定只会在（空的）内置处理之前观察事件。

## XML 定义

- **创建：** `type="progress"`，不需要 `command`（工厂会忽略它）。没有 `type` 的 `command` 是唯一的架构错误。
- **几何：** 一种水平形式和一种垂直形式；给出足够容纳轨道和文本的高度。
- **内容：** 仅 `progress`（`content_caps = PROGRESS`）。即使 Rust 类型有 `value()` getter，`value` 也会被拒绝。
- **样式：** `radius`、`corners`、`background`、`font-size`、`font-family`。`background="none"` 仅保留填充。
- **属性：** 通用运行时集合（`visible`、`tooltip`、`name`、……）。`enabled` 会被架构接受，但 `ProgressBar` 没有禁用外观并忽略它。
- **子元素：** 无。
- **父元素：** 可选的 `parent="panel-id"`。

## 另见

- `./slider.md`
- `./pager.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
