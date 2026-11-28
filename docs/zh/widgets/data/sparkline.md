# Sparkline

> 一条紧凑的柱状条，用于概括一段简短的数值序列。

## 介绍

`Sparkline` 为每个样本绘制一根垂直柱，在其矩形内底部对齐。柱高按序列的最小值到最大值进行缩放，且报告的范围始终包含零基线。它是一个纯显示控件：不可聚焦、不接收输入，也不发出消息。它以文本节点的形式加入无障碍树，描述样本数量和范围。

在读数旁展示趋势时可使用它（码率历史、音量变化、负载随时间变化）。对于单个数值，请使用 `Gauge` 或进度环；对于可点击的评分，请使用 `RatingStars`。

## 用法（Rust）

```rust
use bamboo_native_ui::{Color, Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::Sparkline;

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Sparkline::new("volume_history", &[3.0, 6.0, 4.0, 9.0, 12.0, 8.0])
        .gap(1.0)
        .color(Color::rgb([127, 211, 168])),
);
ui.set_rect(
    WidgetId::new("volume_history"),
    Rect::from_xywh(16.0, 72.0, 160.0, 36.0),
);
```

`Sparkline::new` 会复制样本，默认使用强调色和 2 DIP 的间距。柱在矩形内进行布局；如果计算出的柱宽低于半个 DIP，则不绘制任何内容。可通过 `set_data` 静默替换序列，或通过类型化接缝更新活动控件：

```rust
ui.edit_widget(WidgetId::new("volume_history"), |spark: &mut Sparkline| {
    spark.set_data(vec![4.0, 7.0, 11.0, 6.0]);
    true // 该 setter 是静默的；上报变更以标记需要重绘
});
```

## 用法（XML）

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。`<point>` 子元素列表会替换样本，`color`/`gap` 局部属性调整其外观：

```xml
<widget id="volume_history" x="16" y="72" w="160" h="36" color="accent" gap="1">
  <point value="3"/>
  <point value="6"/>
  <point value="4"/>
  <point value="9"/>
  <point value="12"/>
</widget>
```

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, data)` | `&'static str, &[f32]` | — | 创建控件并复制样本。 |
| `.color(Color)` | `Color` | accent | 柱的颜色。 |
| `.gap(f32)` | `f32` | `2.0` | 柱之间的间距（DIP），下限限制为 `>= 0`。 |
| `.data()` | `&[f32]` | — | 当前样本。 |
| `.set_data(Vec<f32>)` | `Vec<f32>` | — | 静默替换样本。 |
| `.range()` | `(f32, f32)` | — | 样本（含零基线）的 `(min, max)`。 |
| `.set_color(Option<Color>)` | `Option<Color>` | `None` | 设置柱的颜色；`None` 恢复为强调色。返回是否发生变化。 |
| `.color_value()` | `Option<Color>` | — | 显式设置的颜色（如有）。 |
| `.set_gap(f32)` | `f32` | `2.0` | 间距（DIP，`>= 0`）。返回是否发生变化。 |
| `.gap_value()` | `f32` | — | 当前间距。 |

行为：该控件请求 36 DIP 的首选高度，不可聚焦，并暴露一个以控件 id 命名的纯显示 `Text` 无障碍节点。描述中会报告样本数量以及来自 `range()` 的范围。

## 事件

`Sparkline` 是纯显示控件：其 `input` 始终返回 `None`，因此不发出消息，也不触发其自身的任何回调。

### 消息

无。构造函数接收一个 id 和样本切片；样本通过 `set_data` 或 `<point>` 子元素列表静默替换。

### 通用观察器

该控件不安装共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。它不可聚焦，也不消费任何输入。

### 控件专属回调

无。

### Ui 触发器

任何控件 id 都可以通过 `Ui::bind` 进行绑定：

```rust
ui.bind(WidgetId::new("volume_history"), Trigger::Click, |event, ctx| {
    // 返回 Some(Msg::...) 以入队一条消息；除非显式调用 ctx.run_default()，
    // 内置操作仍会执行。
    None
});
```

| 触发器 | 触发时机 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告双击。 |
| `RightClick` | 次级按钮点击，在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开该控件。 |
| `Wheel` | 控件上的滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 聚焦控件上按下 Enter/空格，或 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 释放按钮。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

迷你图自身不对其中任何触发器作出反应——它没有内置行为——因此绑定是在其上响应输入的唯一方式。

## XML 定义

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。

几何布局遵循常规的 `<widget>` 规则：恰好一个水平形式（`x`+`w`、`right`+`w` 或 `x`+`right`）和一个垂直形式（`y`+`h`、`bottom`+`h` 或 `y`+`bottom`）。通用属性（`visible`、`tooltip`、`name`、`description`、`expose`、焦点策略、光标）均适用。`font-size` 和 `font-family` 属于绘制层。该控件从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported` 并跳过。

局部属性：

| 属性 | 类型 | 描述 |
|---|---|---|
| `color` | color | 柱的颜色（调色板 token 或字面量）。 |
| `gap` | number | 柱之间的间距（DIP）；必须 `>= 0`。 |

子元素：

| 元素 | 属性 | 描述 |
|---|---|---|
| `<point>` | `value`（必需，number） | 一个样本；该列表会替换每个样本。 |

子元素列表属于控件内容：仅在文档生成变化时写入，与变体无关，且从不触发回调。没有共享的 `value` 属性；请使用 `<point>` 或 `set_data`。

## 另见

- `gauge.md`
- `rating-stars.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
