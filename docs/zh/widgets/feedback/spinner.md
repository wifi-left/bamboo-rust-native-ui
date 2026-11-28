# Spinner

> 一种活动指示器，绘制为十二个圆点组成的旋转圆环。

## 介绍

`Spinner` 在一个圆周上绘制十二个圆点，并带有一条逐渐淡出的尾巴。头部圆点使用
强调色（或显式指定的颜色），其后的每个圆点都朝页面背景混合，因此这个圆环读起来
像是在旋转，却不需要弧线图元，也不需要自身的定时器。

该控件仅用于显示：它不可聚焦，忽略输入，且不发出任何消息。动画时钟由应用程序
持有，应用通过自身的帧定时器调用 `advance(dt)` 推进绘制相位，因此空闲页面
仍然不做任何周期性工作。这与 `MarqueeText` 和不确定态的 `ProgressBar` 采用
相同的应用驱动模型。

其无障碍节点是一个仅用于显示的 `Text`，以控件 id 命名；只有实例选择加入后，
该节点才加入控件树，而 `WidgetProps::name` 为它提供一条有用的播报。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::Spinner;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(Spinner::new("busy").speed(360.0));
ui.set_rect(WidgetId::new("busy"), Rect::from_xywh(16.0, 16.0, 24.0, 24.0));
```

从宿主的动画 tick 驱动它。`dt` 是距上一个 tick 的时间；`advance` 会报告绘制
相位是否发生变化，因此暂停或长度为 0 的 tick 不会记录任何损坏：

```rust
fn tick(ui: &mut Ui<Msg>, dt: std::time::Duration) {
    ui.edit_widget::<Spinner>(WidgetId::new("busy"), |spinner| spinner.advance(dt));
}
```

24 DIP 的正方形是自然尺寸；圆环在你分配的任何矩形里都切于较短的一边。

## 用法（XML）

`spinner` 是标准注册表类型。在 `Ui` 上安装 `WidgetRegistry::standard()` 后，
用 `type="spinner"` 创建它：

```xml
<widget id="busy" type="spinner" x="16" y="16" w="24" h="24"
        phase="90" animating="true"/>
```

工厂创建一个默认 spinner；文件提供初始相位和动画标志。`speed` 和 `color`
仅在构造时设置，保留在 Rust 中。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建一个处于相位 0 的 `24 × 24` spinner。 |
| `.speed(f32)` | `f32`（度/秒） | `300.0` | 旋转速度，会被钳制为至少 `1.0`。 |
| `.color(Color)` | `Color` | 调色板 `accent` | 圆点颜色；尾巴仍会朝背景淡出。 |
| `.phase()` | `f32` | `0.0` | 当前相位，单位为度。 |
| `.set_phase(f32)` | `f32` | — | 设置相位，用 `rem_euclid` 环绕到 `0..360`。 |
| `.is_animating()` | `bool` | `true` | `advance` 是否会改变相位。 |
| `.set_animating(bool)` | `bool` | — | 暂停或恢复，同时保留已绘制的相位。 |
| `.advance(Duration)` | `Duration` | — | 增加 `speed × dt` 度；返回变化是否超过 `0.001°`。 |

### 行为

- 圆环始终是相隔 30° 的十二个圆点，因此绘制成本恒定；圆点直径由半径推导，并钳制到 `1.5..3.0` DIP。
- 首选高度为 24 DIP，且该控件不可聚焦。
- 暂停时，或步长最多为 `0.001°`（一次空操作的 tick）时，`advance` 返回 `false`；头部越过 360° 时，时钟继续运行。

## 事件

spinner 仅用于显示：其 `input` 始终返回 `None`，因此它不发出任何消息，也不触发自身的任何回调。动画由应用程序驱动，`advance(dt)` 返回 `true` 是该控件产生的唯一“已改变”信号。

### 消息

无。构造函数不接收回调。

### 通用观察器

该控件不安装任何共享的观察器构建器。它不可聚焦，也不消费输入，因此其上不存在 `on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained` 和 `on_focus_lost`。

### 控件专属回调

无。通过 `Ui::edit_widget` 使用 `set_phase(degrees)`、`set_animating(bool)` 和 `advance(dt)` 来驱动该控件。

### Ui 触发器

任何控件 id 都可以用 `Ui::bind(WidgetId::new("busy"), Trigger::HoverEnter, |event, ctx| ...)` 绑定；返回 `Some(Msg::...)` 会排队一条消息，并且处理器返回后内置行为仍会运行（`ctx.run_default()` 会内联运行它，两者都不抑制它）。在 spinner 上，内置行为始终是“什么都不做”，因此绑定是做出响应的唯一方式。

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

每个触发器都会到达该 id 上的绑定，但只有指针触发器（`Click`、`HoverEnter`/`HoverLeave`）与 spinner 的行为有关；`Activate`、按键和焦点触发器永远不会触发，因为它不可聚焦；`ValueChange` 永远不会触发，因为 `advance` 不报告任何无障碍值。

## XML 定义

- **创建：** `type="spinner"`，无需 `command`。若没有标准注册表，该条目为 `xml.type.unknown`。创建的控件落在页面层，位于第一个背景（`Scrim`）之前。
- **几何：** 适用普通的 `widget` 规则（水平和垂直两种形式）。圆环在分配的矩形内自切。
- **内容：** 无。不支持 `label` 和 `text`（`xml.content.unsupported`，一条警告），并会被跳过。
- **局部属性：** `phase`（数字，度）和 `animating`（bool）。未知名称会得到 `xml.skin.unknown`。
- **样式令牌：** 接受 `font-size` 和 `font-family`，但不绘制任何内容（spinner 不绘制文本）；该控件没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 是 `xml.style.unsupported` 警告，会被跳过。
- **子控件：** 无。可选的 `parent="panel-id"` 会将创建的条目绑定到已注册的滚动面板。

## 另见

- `progress-ring.md`
- `../controls/progress-bar.md`
- `../data/gauge.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
- `../../api/widget-trait.md`
