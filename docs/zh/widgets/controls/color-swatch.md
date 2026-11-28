# ColorSwatch

> 一个带标签的颜色色块，用于显示调色板条目或当前颜色。

## 介绍

`ColorSwatch` 在其前缘绘制一个 40 DIP 宽、占满高度的色块，使用主题的 `explicit_border` 边框，并在其中居中绘制标签。标签文本颜色在浅色（亮度高于 `0.55`）上翻转为接近黑色，在深色上翻转为接近白色，因此短标签在任何色块上都保持可读。

它是一个显示控件：不可聚焦、忽略输入、不上报任何消息。通过选择加入鼠标输入并在 `Ui` 上绑定处理函数（`WidgetProps::mouse(true)` 加上 `Ui::on_click`）使其可点击，正如 showcase 所做的那样。该控件还采用 `LABEL` 内容能力，因此文档可以设置内部文本。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::ColorSwatch;
use bamboo_native_ui::{Color, Rect, Ui, WidgetId};

enum Msg {
    InspectSwatch,
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(ColorSwatch::new("accent", Color::rgb([127, 211, 168]), "Bamboo"));
ui.set_rect(WidgetId::new("accent"), Rect::from_xywh(16.0, 16.0, 120.0, 32.0));
ui.on_click(WidgetId::new("accent"), |_event, _ctx| Some(Msg::InspectSwatch));
```

如果没有 `on_click` 绑定和 `WidgetProps::mouse(true)`，色块不接受任何输入。`set_color` 静默地更改颜色，`set_label_text` 更改文本；两者也都可以从 `.buixml` 文档中访问。

## 用法（XML）

`ColorSwatch` 不由标准注册表创建。在 Rust 中创建它，并通过 `id` 定位；文档设置标签和颜色：

```xml
<widget id="accent" x="16" y="16" w="120" h="32"
        label="Bamboo" color="#7FD3A8"/>
```

`color` 接受共享的颜色语法——`#rgb`、`#rgba`、`#rrggbb`、`#rrggbbaa`、十进制通道、`rgb(…)`/`rgba(…)` 或调色板标记。色块是不透明的，因此 alpha 分量会被静默丢弃。`label` 通过 `Ui::set_label` 传入；没有子元素。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, color, label)` | `&'static str, Color, impl Into<String>` | — | 创建该色块；`120 × 32` DIP。 |
| `.color()` | — | — | 当前颜色。 |
| `.set_color(Color)` | `Color` | — | 静默替换颜色（其本身不会请求重绘）。 |
| `.label()` | — | — | 色块文本。 |
| `.set_label_text(impl Into<String>)` | `String` | — | 静默替换色块文本。 |

### 行为

- **布局。** 色块始终占据矩形左侧的 40 DIP，即使在更宽的单元格中也是如此；其余部分为空。标签在色块中居中，水平内缩 4 DIP。
- **对比度。** 文本颜色根据颜色的亮度而非主题来选择，因此浅色色块获得深色文本，反之亦然。
- **交互。** 无内置交互。当色块应充当按钮时，通过 `Ui` 绑定点击/激活；并通过 `WidgetProps` 为辅助技术设置角色/名称。
- **无障碍性。** 角色为 `text`、`display_only`，名称来自标签，描述为 `#RRGGBB`，以便色盲用户仍能获得准确的颜色。该实例仅在选择加入时才加入无障碍树（`WidgetProps::expose(true)`）。

## 事件

### 消息

无。`ColorSwatch` 不上报任何消息。

### 通用观察器

该控件**不**安装共享的观察器构建器。它不可聚焦，其 `Widget::input` 返回 `None`，并且没有回调字段。要使其可点击，用 `WidgetProps::mouse(true)`（文档中为 `interactive="true"`）选择加入鼠标输入，并在其 id 上绑定 `Ui::on_click` 或 `Ui::bind`。

### 控件专属回调

无。`set_color` 和 `set_label_text` 静默替换状态。

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("accent"), Trigger::Click, |event, ctx| {
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

色块自身不响应其中任何一个；只有当实例已选择加入鼠标输入时，绑定才会触发。

## XML 定义

- **创建：** 无 `type`；该控件仅可被定位。
- **几何：** 一种水平形式和一种垂直形式。低于约 20 DIP 的高度会开始裁剪色块标签。
- **内容：** `label`（`content_caps = LABEL`）。
- **局部属性：** 仅 `color`（`SkinValueKind::Color`）。
- **样式：** `font-size`、`font-family`。没有表面，因此 `radius`、`corners` 和 `background` 会报 `xml.style.unsupported` 并被跳过。
- **属性：** 通用运行时集合（`visible`、`tooltip`、`name`、`interactive`、……）。当 Rust 绑定应接收点击时，设置 `interactive="true"`。
- **子元素：** 无。

## 另见

- `../pickers/color-picker.md`
- `./button.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
