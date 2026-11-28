# Splitter

> 一个可拖动的分隔条，报告窗格边界应移动到的位置。

## 介绍

`Splitter<M>` 绘制一个带有三个抓握点的细分隔条，并将指针拖动报告给应用程序。它不拥有也不移动窗格：应用程序保留窗格矩形，并根据 `on_drag` 报告的位置重新布局它们。

`Splitter::vertical` 在左窗格和右窗格之间绘制一个垂直分隔条，并在 x 轴上拖动；`Splitter::horizontal` 在上窗格和下窗格之间绘制一个水平分隔条，并在 y 轴上拖动。分隔条不是 Tab 停靠点，也没有键盘调整，但它接收指针并暴露 `Separator` 无障碍角色。

## 用法（Rust）

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::Splitter;

enum Msg {
    Split(f32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(Splitter::vertical("split", Msg::Split).label("Resize panes"));
ui.set_rect(WidgetId::new("split"), Rect::from_xywh(296.0, 16.0, 8.0, 300.0));
```

`on_drag` 接收客户区 DIP 中的新分隔条中心，并保留按下点与旧中心之间的偏移（在中心右侧 2 DIP 处按下会保留该 2 DIP 的偏移）。通过限制到窗格最小值并分配窗格和分隔条矩形来作出响应：

```rust
fn split_at(ui: &mut Ui<Msg>, centre: f32) {
    let left_width = (centre - 16.0).clamp(120.0, 300.0);
    ui.set_rect(WidgetId::new("left"), Rect::from_xywh(16.0, 16.0, left_width, 300.0));
    ui.set_rect(WidgetId::new("split"), Rect::from_xywh(16.0 + left_width + 4.0, 16.0, 8.0, 300.0));
}
```

## 用法（XML）

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。它接受几何信息、通用属性和一个无障碍名称：

```xml
<widget id="split" x="296" y="16" w="8" h="300" name="Resize panes"/>
```

回调以及方向是构造函数参数；文档无法更改它们。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `Splitter::vertical(id, on_drag)` | `&'static str, impl Fn(f32) -> M` | — | 垂直分隔条（8×200），可水平拖动。 |
| `Splitter::horizontal(id, on_drag)` | `&'static str, impl Fn(f32) -> M` | — | 水平分隔条（200×8），可垂直拖动。 |
| `.label(impl Into<String>)` | text | widget id | 无障碍名称。 |
| `.disabled(bool)` | `bool` | `false` | 忽略指针并绘制普通分隔条颜色。 |
| `.is_active()` | `bool` | — | 是否有拖动正在进行。 |
| `.is_vertical()` | `bool` | — | 分隔条方向。 |

行为：该条绘制一条 1 DIP 的中心线以及三个 3×3 DIP 的抓握点。正常情况下颜色为分隔条 token，悬停时为 `scroll_thumb_hover`，拖动时为强调色（禁用的分隔条保持分隔条颜色）。垂直分隔条的光标为 `SizeWE`，水平分隔条的光标为 `SizeNS`。`on_drag` 仅在按住期间触发；没有按下的拖动不发出任何内容。垂直分隔条的首选高度为 200 DIP，水平分隔条为 8 DIP。无障碍角色为 `Separator`。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 作为 `on_drag` 传给 `Splitter::vertical` / `Splitter::horizontal` 的 `Fn(f32) -> M` | 分隔条被按下时指针移动；`f32` 是客户区 DIP 中的新分隔条中心。相对于按下点的抓握偏移会被保留。 |

没有按下的拖动，或任何键盘输入，都不发出任何内容。

### 通用观察器

该控件不安装共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。构造函数的 `on_drag` 就是它的事件接缝；其他任何内容都通过 `Ui::bind` 处理。

### 控件专属回调

| 方法 | 处理器 | 触发时机 |
|---|---|---|
| `Splitter::vertical(id, on_drag)` | `&'static str, impl Fn(f32) -> M + 'static` | 存储用于水平拖动（垂直分隔条）的拖动回调。 |
| `Splitter::horizontal(id, on_drag)` | `&'static str, impl Fn(f32) -> M + 'static` | 存储用于垂直拖动（水平分隔条）的拖动回调。 |

两个构造函数声明相同的回调字段；方向仅改变报告哪个轴。`is_active()` 报告是否有拖动正在进行。

### Ui 触发器

任何控件 id 都可以通过 `Ui::bind` 进行绑定：

```rust
ui.bind(WidgetId::new("split"), Trigger::DragStart, |event, ctx| {
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

它响应 `HoverEnter`/`HoverLeave`（悬停颜色）以及 `DragStart`/`DragEnd`（拖动会话），拖动位置通过 `on_drag` 传递。它不可聚焦，因此 `Activate` 从不触发；它也不携带值，因此 `ValueChange` 也不会触发；`Click`、`DoubleClick`、`RightClick` 和 `Wheel` 没有内置效果。

## XML 定义

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。

几何布局遵循常规的 `<widget>` 规则；请为垂直分隔条指定较窄的宽度，为水平分隔条指定较小的高度。通用属性适用，包括 `enabled` 和 `name`。`font-size` 和 `font-family` 属于绘制层。分隔条从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported` 并跳过。

该控件不声明任何局部属性和任何子元素。方向和拖动回调在构造时固定。

## 另见

- `nav-rail.md`
- `../../api/widget-trait.md`
- `../index.md`
