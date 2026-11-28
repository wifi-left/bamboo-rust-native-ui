# RatingStars

> 可点击的星级评分，带有悬停高亮和键盘调整。

## 介绍

`RatingStars<M>` 绘制一行五角星，按当前值填充。点击某颗星即评为此星数，悬停会高亮指针下方的值，在调整模式下方向键、Home 和 End 会改变它。它是真正的控件：通过 `on_rate` 闭包把新评分报告为应用程序消息 `M`。

该控件占用 `Slider` 无障碍角色，范围为 `0..=stars`，并支持来自辅助技术的 `SetValue`。只读实例仍会绘制并发布其值，但忽略点击、悬停和按键。

## 用法（Rust）

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::RatingStars;

enum Msg {
    Rate(u8),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    RatingStars::new("rating", 5, 3, Msg::Rate)
        .clearable(true)
        .read_only(false),
);
ui.set_rect(WidgetId::new("rating"), Rect::from_xywh(16.0, 16.0, 130.0, 28.0));
```

`on_rate` 在每次变更被接受时运行，包括清除手势。启用 `clearable(true)` 后，点击等于当前值的星会将评分重置为零并报告 `on_rate(0)`。`set_rating` 是推送已存储值的静默路径：

```rust
ui.edit_widget(WidgetId::new("rating"), |stars: &mut RatingStars<Msg>| {
    stars.set_rating(4);
    true // 上报静默 setter，以便标记控件需要重绘
});
```

## 用法（XML）

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。共享的 `value` 内容属性会静默设置评分；通用属性可以锁定或禁用它：

```xml
<widget id="rating" x="16" y="16" w="130" h="28" value="3" name="Rating"/>
```

`value` 会四舍五入并限制为星数（高于星数的值会填满每颗星）。它不会触发 `on_rate`，因此初始值在每个文档生成周期内写入一次，且不产生回调。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, stars, value, on_rate)` | `&'static str, usize, u8, impl Fn(u8) -> M` | — | 创建控件；`stars` 至少为 1，`value` 被限制到该值。 |
| `.clearable(bool)` | `bool` | `false` | 点击当前值会将评分清除为零。 |
| `.read_only(bool)` | `bool` | `false` | 忽略点击、悬停和按键，但继续绘制值。 |
| `.disabled(bool)` | `bool` | `false` | 禁用控件（绘制禁用颜色，忽略输入）。 |
| `.value()` | `u8` | — | 当前评分。 |
| `.hover_value()` | `Option<u8>` | `None` | 指针下方的悬停值。 |
| `.set_rating(u8)` | `u8` | — | 静默设置评分，限制为星数。 |
| `.rate(u8)` | `u8` | — | 通过常规路径评分并返回消息；在禁用、只读或未变化时返回 `None`。 |

行为：首选高度为 28 DIP。鼠标输入被划分为相等的星格；一次点击映射到指针下方的格子。按键：Left/Down 递减，Right/Up 递增，Home 设为零，End 设为星数。值未变化时不产生消息。整个矩形是工具提示锚点。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 作为 `on_rate` 传给 `new` 的 `Fn(u8) -> M` | 一次被接受的评分变更：点击某颗星、调整模式下的方向键/Home/End 键、UIA 范围请求，或清除手势（`0`）。静默路径（`set_rating`、XML `value` 属性）从不触发它。 |

### 通用观察器

该控件不安装共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。构造函数的 `on_rate` 就是它的事件接缝；其他任何内容都通过 `Ui::bind` 处理。

### 控件专属回调

| 方法 | 处理器 | 触发时机 |
|---|---|---|
| `new(id, stars, value, on_rate)` | `&'static str, usize, u8, impl Fn(u8) -> M + 'static` | 存储评分回调；每当 `rate` 接受一次变更时运行。 |
| `.rate(u8)` | 返回 `Option<M>` | 运行时形式：正常评分并返回消息，或在禁用、只读或未变化时返回 `None`。 |

### Ui 触发器

任何控件 id 都可以通过 `Ui::bind` 进行绑定：

```rust
ui.bind(WidgetId::new("rating"), Trigger::ValueChange, |event, ctx| {
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

它响应 `Click`（评分）、`HoverEnter`/`HoverLeave`（预览）和 `ValueChange`（范围变化）；在调整模式下方向键、Home 和 End 键会改变值。`Activate`、`DoubleClick`、`RightClick`、`Wheel` 以及拖动触发器在此处没有内置含义。

## XML 定义

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。

几何布局遵循常规的 `<widget>` 规则。通用属性适用：`enabled` 会抑制输入并绘制禁用颜色，`name` 会覆盖无障碍名称（默认：控件 id）。通用 `read-only` 属性会发布给辅助技术，但不会锁定该控件；请调用 Rust 的 `.read_only(true)` 构建器来锁定控件。`font-size` 和 `font-family` 属于绘制层。该控件从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported` 并跳过。

局部属性和子元素：无。唯一的内容属性是共享的 `value`（number）。`clearable` 和评分回调仅限 Rust。

## 另见

- `sparkline.md`
- `gauge.md`
- `../index.md`
- `../../api/widget-trait.md`
