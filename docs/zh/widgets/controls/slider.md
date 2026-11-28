# Slider

> 一条带有旋钮、可选数值读数、刻度标记和拖动回调的水平数值轨道。

## 介绍

`Slider<M>` 绘制一条圆角凹槽、一段到当前数值为止的强调色填充，以及一个在悬停或按下时会变大的旋钮。它可以绘制等距刻度或位于显式数值处的刻度、其下的一行标签，以及轨道上方一个格式化的数值标签。所有这些都仅用于显示；数值网格由 `step`、`snap` 和 `discrete` 定义。

指针拖动遵循三阶段生命周期：按下时触发 `on_drag_start`，按住时每次移动触发 `on_drag`，释放时在通常的 `on_change` 旁边触发 `on_drag_end`。控件维护自己的数值；应用通常从消息中镜像它。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::Slider;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Volume(f32),
    VolumePreview(f32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Slider::new("volume", 0.6, 0.0..=1.0, Msg::Volume)
        .discrete(0.05)
        .ticks_at(&[0.0, 0.25, 0.5, 0.75, 1.0])
        .tick_labels(&["0", "25", "50", "75", "100"])
        .value_text(|value| format!("{:.0}%", value * 100.0))
        .on_drag(Msg::VolumePreview),
);
ui.set_rect(WidgetId::new("volume"), Rect::from_xywh(16.0, 16.0, 240.0, 44.0));
```

`on_change` 是 `Fn(f32) -> M`；拖动回调具有相同的形状。由于 `on_drag_end` 在 `on_change` 旁边上报，一次释放会投递两条消息：先是 `on_change(value)`，然后是 `on_drag_end` 消息。存在刻度标签时，单元格需要 44 DIP 高度（否则为 32）。

## 用法（XML）

`Slider` 由标准注册表创建（`type="slider"`）并需要一个 `command`；标准工厂需要 `M: Clone`，因为每次更改都会重复上报同一消息。

```xml
<widget id="volume" type="slider" command="103"
        x="16" y="16" w="240" h="32" value="0.6" radius="medium"/>
```

创建的滑块始终为 `0.0..=1.0`，并使用默认步长；`value` 是初始内容值（由 setter 钳制）。刻度标记、标签和数值读数仅在 Rust 中可用。`Slider` 不声明任何控件局部属性，但它确实采用 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会应用到轨道上。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, value, range, on_change)` | `&'static str, f32, RangeInclusive<f32>, impl Fn(f32) -> M` | — | 创建该滑块；`240 × 32` DIP，`step = (max − min) / 100`。 |
| `.step(f32)` | `f32` | `(max − min) / 100` | 键盘和吸附指针输入的增量。 |
| `.snap(bool)` | `bool` | `false` | 将指针拖动和 UIA `SetValue` 舍入到步长网格上。 |
| `.discrete(f32)` | `f32` | — | 一次调用设置 `step` 并启用 `snap`（除非为有限且 > 0 的值，否则忽略）。 |
| `.is_snapped()` / `.set_snap(bool)` | — / `bool` | `false` | 在运行时读取或翻转吸附（`set_snap` 返回是否发生了更改）。 |
| `.step_size()` | — | — | 当前增量。 |
| `.ticks(usize)` | `usize` | `0` | 在轨道下方绘制 `count + 1` 个等距标记；`0` 隐藏它们。 |
| `.ticks_at(&[f32])` | `&[f32]` | 空 | 显式标记位置（钳制到范围内）；覆盖 `ticks`。 |
| `.tick_labels(&[&str])` | `&[&str]` | 空 | 轨道下方的标签；当数量匹配时跟随 `ticks_at`，否则等距分布。 |
| `.value_text(impl Fn(f32) -> String)` | 闭包 | 无 | 格式化绘制在右上角的标签。 |
| `.tick_count()` / `.tick_values()` | — | — | 两个刻度设置的 getter。 |
| `.value_label()` | — | — | 格式化的数值标签，如果已配置。 |
| `.on_drag_start(impl Fn(f32) -> M)` | 闭包 | 无 | 按下时以指针下方的数值调用一次。 |
| `.on_drag(impl Fn(f32) -> M)` | 闭包 | 无 | 拖动期间每次移动时调用。 |
| `.on_drag_end(impl Fn(f32) -> M)` | 闭包 | 无 | 释放时以已提交的数值调用。 |
| `.is_dragging()` / `.drag_preview()` | — / `Option<f32>` | — | 是否正在拖动以及它绘制的数值。 |
| `.value()` / `.range()` | — / `(f32, f32)` | — | 当前数值和范围。 |
| `.set_range(min, max)` | `f32, f32` | — | 替换范围（颠倒的数值对会被交换）；静默钳制数值。 |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | 无 | 附加型点击观察器；旁边还有 `on_right_click`/`on_double_click`/焦点/按键观察器。 |

### 行为

- **吸附。** `quantize` 将数值钳制到范围内，并在开启吸附时，从 `min` 起舍入到 `step` 的最近倍数。应用驱动的 `Ui::set_value` 保持精确（它只做钳制）。构造函数不会重新排列 `range`，因此请传入 `min <= max`；`set_range` 会交换颠倒的数值对。
- **拖动。** 拖动期间，旋钮和填充跟随预览；真实数值仅在释放时更改。`on_drag_start`/`on_drag` 是对该预览的附加。
- **键盘。** 上/右增加一个步长，下/左减少一个，PageUp/PageDown 增加/减少十个。
- **表面。** `corner_radius` 圆角化凹槽和填充，`background` 覆盖凹槽颜色，`transparent_background(true)` 隐藏凹槽。
- **无障碍性。** 角色为 `slider`，带有范围和数值；`value_text` 成为 UIA 值字符串。

## 事件

### 消息

| 消息 | 触发条件 |
|---|---|
| 作为 `on_change` 传给 `Slider::new` 的 `Fn(f32) -> M` | 一次拖动被释放，一个键盘步长更改了数值，或一次 UIA `SetValue` 落在一个新数值上。 |

### 通用观察器

该控件安装了共享的观察器构建器。它们都是附加型的：处理函数返回后，内置行为仍会执行。

| 构建器 | 处理函数 | 触发条件 |
|---|---|---|
| `.on_click(f)` | `Fn(Point) -> Option<M>` | 在控件上的左键点击。 |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | 右键点击，在上下文菜单之前。 |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | 平台双击。 |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | 控件获得焦点时的按键按下。 |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | 控件获得焦点时的按键释放。 |
| `.on_focus_gained(f)` | `Fn(FocusEvent) -> Option<M>` | 焦点移入该控件。 |
| `.on_focus_lost(f)` | `Fn(FocusEvent) -> Option<M>` | 焦点离开该控件（在编辑器提交之后）。 |

### 控件专属回调

| 构建器 | 处理函数 | 触发条件 |
|---|---|---|
| `.on_drag_start(f)` | `Fn(f32) -> M` | 指针被按下；接收其下方的数值。 |
| `.on_drag(f)` | `Fn(f32) -> M` | 按住按钮期间每次指针移动；接收预览值。 |
| `.on_drag_end(f)` | `Fn(f32) -> M` | 拖动被释放；接收已提交的数值，在 `on_change` 之后上报。 |

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("volume"), Trigger::Click, |event, ctx| {
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

滑块把 `Click` 当作按下/释放拖动，把 `DragStart`/`DragEnd` 当作指针拖动，并响应方向键/PageUp/PageDown 键和 `ValueChange`；除观察器之外，它忽略 `Wheel` 和 `DoubleClick`。

## XML 定义

- **创建：** `type="slider"` 加上 `command`。缺少命令会报 `xml.command.missing`；需要 `M: Clone`。
- **几何：** 一种水平形式和一种垂直形式；优先使用 `h="32"` 或 `h="44"`，以免旋钮和标签被裁剪。
- **内容：** 仅 `value`（`content_caps = VALUE`）——它是一个初始值，在生成变化时写入。
- **样式：** `radius`、`corners`、`background`、`font-size`、`font-family`。
- **属性：** 通用运行时集合（`enabled`、`visible`、`tooltip`、`name`、……）。
- **子元素：** 无。
- **父元素：** 可选的 `parent="panel-id"`。

## 另见

- `./progress-bar.md`
- `./pager.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
