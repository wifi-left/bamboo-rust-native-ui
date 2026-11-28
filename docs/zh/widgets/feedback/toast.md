# Toast

> 一条瞬态状态行，带有可选操作，由应用程序显示并计时。

## 介绍

`Toast` 绘制标准的瞬态状态消息：在卡片色表面上的一行小号文本，带一条 3 DIP 的前置条，正常情况下为强调色，在错误时为显式边框色。可选的一个右对齐操作标签（`Undo`、`View`）绘制在同一行内。

`Snackbar` 是 `Toast` 的类型别名（播放器词汇）；API 完全相同。该控件初始为隐藏。应用程序决定消息何时出现（`show`，或容器的 `Ui::set_message`）、它停留多久（其配置的 `duration`）以及它何时再次隐藏；该控件不启动任何定时器。`Ui::set_message` 还会清除容器的可见性覆盖，因此被 `set_visible` 隐藏的 toast 可以再次通过消息显示。

toast 仅用于显示且不可聚焦。其无障碍节点是一个仅用于显示的 `Text`；应用程序还应通过 `WindowHandle::a11y_announce` 播报文本，以便屏幕阅读器在无需移动焦点的情况下听到它。

## 用法（Rust）

```rust
use std::time::Duration;

use bamboo_native_ui::widgets::Toast;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Undo }

let mut ui: Ui<Msg> = Ui::new();
ui.add(Toast::new("toast").duration_secs(3.0).action("Undo"));
ui.set_rect(WidgetId::new("toast"), Rect::from_xywh(16.0, 560.0, 320.0, 44.0));
```

从应用程序事件中显示它。`.show()` 设置文本和错误标志，并使 toast 可见；闭包必须报告该变化。随后应用程序读取一次生存期并拥有定时器。`Ui::set_message(id, text, error)` 是容器快捷方式，并且还会清除可见性覆盖。

```rust
ui.edit_widget::<Toast>(WidgetId::new("toast"), |toast| {
    toast.show("Track removed", false);
    true
});
let lifetime = ui
    .with_widget::<Toast, _>(WidgetId::new("toast"), |toast| toast.duration_value())
    .flatten()
    .unwrap_or(Duration::from_millis(2200));

// 当应用程序的定时器到期时：
ui.edit_widget::<Toast>(WidgetId::new("toast"), |toast| {
    toast.hide();
    true
});
```

对于带操作的 toast，将点击触发器绑定到该 toast，并使用 `action_at(pos)` 区分操作点击和正文点击。

## 用法（XML）

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址：

```xml
<widget id="toast" x="16" y="560" w="320" h="44"
        message="Track removed" error="false" radius="medium"/>
```

`message` 和 `error` 是控件局部属性，但两者都不改变可见性：toast 保持隐藏，直到应用程序显示它。持续时间和操作标签仅在构造时设置。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建一个无文本、隐藏的 `320 × 44` toast。 |
| `.duration(Duration)` / `.duration_secs(f32)` | `Duration` / `f32` | `None` | 应用程序用于自动隐藏的生存期；秒数被钳制为不低于零。 |
| `.duration_value()` | `Option<Duration>` | — | 读取已配置的生存期。 |
| `.action(label)` | `impl Into<String>` | `""` | 右对齐的操作标签。 |
| `.action_text()` | `&str` | — | 读取操作标签。 |
| `.action_rect()` | `Option<Rect>` | — | 上次绘制时的操作矩形。 |
| `.action_at(pos)` | `bool` | `false` | `pos` 是否命中绘制的操作标签。 |
| `.show(text, error)` | `impl Into<String>, bool` | — | 设置文本和错误标志，然后使 toast 可见。 |
| `.hide()` | — | — | 使其不可见；文本被保留。 |
| `.is_visible()` | `bool` | `false` | 当前可见性。 |
| `.text()` / `.is_error()` | `&str` / `bool` | `""` / `false` | 读取当前消息。 |
| `.set_text(text)` / `.set_error(bool)` | text / `bool` | — | 静默的消息/错误替换（皮肤 `message`/`error`）。 |

表面构建器：`corner_radius(f32)`、`corners(Corners)`、`background(Color)`、`background_option(Option<Color>)`、`transparent_background(bool)` 和 `surface()`。默认填充是 `card` 令牌；前置条和操作文本使用强调色，或在错误时使用 `explicit_border`。

### 行为

- 首选高度为 44 DIP；构造函数矩形为 `320 × 44`。
- `duration` 是供应用程序参考的数据，绝不是控件的定时器。
- 操作标签使用小号样式测量，至少 28 DIP 宽；在发生一次带操作的绘制之前，`action_rect` 为 `None`。
- `set_text`/`set_error` 不会显示隐藏的 toast。
- `toast_size(painter, canvas, text)` 为一个运行段设置尺寸（宽 `160..=520`，高 44）。
- 无障碍：`Text`、仅显示；描述会标记一条错误消息。

## 事件

toast 没有回调：`input` 始终返回 `None`，且该控件不启动任何定时器。应用程序显示它、读取 `duration_value()`，并在自己的定时器到期时隐藏它；toast 不可聚焦，因此键盘触发器永远不会触发。

### 消息

无。`.action(label)` 仅声明标签；它不附加处理器。

### 通用观察器

该控件不安装任何共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。取而代之，用 `WidgetProps::mouse(true)` 让实例选择加入鼠标输入，将 `Click` 触发器绑定到 toast id，并使用 `action_at(event.pos)` 区分操作点击和正文点击。

### 控件专属回调

| 接缝 | 签名 | 作用 |
|---|---|---|
| `.action(label)` | `impl Into<String>` | 绘制右对齐的操作标签。 |
| `.action_at(pos)` | `fn(&self, Point) -> bool` | `pos` 是否命中上次绘制的标签。 |
| `.action_rect()` | `fn(&self) -> Option<Rect>` | 同一个矩形；在带操作的绘制之前为 `None`。 |
| `.show(text, error)` / `.hide()` | `fn(&mut self, ...)` | 应用驱动的显示/隐藏；配合 `Ui::set_message` 和应用程序定时器使用。 |

### Ui 触发器

任何 id 都可以用 `Ui::bind(WidgetId::new("toast"), Trigger::Click, |event, ctx| ...)` 绑定；`Some(Msg::Undo)` 会被排队，并且内置行为之后仍会运行（`ctx.run_default()` 会内联运行它，两者都不抑制它）。toast 没有内置行为，因此点击绑定加上 `action_at` 就是全部交互。

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

只能观察到指针触发器，且仅在实例选择加入鼠标输入之后；`Activate`、按键和焦点触发器永远不会触发，因为 toast 不可聚焦；它也不携带值，因此 `ValueChange` 也不会触发。

## XML 定义

该控件不是由标准注册表创建的。请在 Rust 中创建它，并从文档中通过 `id` 寻址。

- **几何：** 适用普通的 `widget` 规则；通常的放置位置是底边居中。
- **内容：** 无；`label` 和 `text` 是 `xml.content.unsupported` 警告。
- **局部属性：** `message`（文本）和 `error`（bool）；两者都是静默的，并将可见性留给应用程序。
- **样式令牌：** `radius`、`corners`、`background`、`font-size`、`font-family`，因为 toast 采用 `SurfaceStyle`。
- **子控件：** 无。**无法表达：** 持续时间和操作标签。

## 另见

- `banner.md`
- `scrim.md`
- `../controls/progress-bar.md`
- `../index.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
