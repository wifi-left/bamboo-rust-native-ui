# TimePicker

> 一个 `HH:MM` 字段，会打开一个带可选选择范围的窗口内小时和分钟弹出层。

## 介绍

`TimePicker<M>` 是一个复合选择器。字段将时间绘制为 `HH:MM` 并绘制一个时钟字形。Enter 会打开一个带两列的弹出层，分别标记为 `时`（小时）和 `分`（分钟），每列显示一个以当前数值为中心的七行窗口。可选择范围之外的行绘制为禁用且无法选择。

弹出层作用于一个工作副本：点击某个小时会选中它并将焦点移到分钟，点击某个分钟会提交并关闭，Enter/Space 会提交当前的小时和分钟对。只有当已提交的数值与弹出层打开时的数值不同时，才会上报应用消息。Escape，或点击字段，会关闭并恢复到该打开时的数值。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{Time, TimePicker};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Alarm(Time),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    TimePicker::new("alarm", Time::new(9, 30), Msg::Alarm)
        .min_time(Time::new(8, 0))
        .max_time(Time::new(18, 0)),
);
ui.set_rect(WidgetId::new("alarm"), Rect::from_xywh(16.0, 16.0, 110.0, 34.0));
```

`on_change` 是 `Fn(Time) -> M`。范围是包含端点的；传给 `set_range` 的颠倒数值对会被交换。时间在 `set_value` 时、范围变化时，以及每次键盘/指针调整时都会钳制到范围内。

## 用法（XML）

`TimePicker` 不由标准注册表创建。在 Rust 中创建它，并通过 `id` 定位；文档可以预置时间和打开状态：

```xml
<widget id="alarm" x="16" y="16" w="110" h="34"
        time="09:30" open="false"/>
```

`time` 必须能通过 `Time::parse` 解析为 `HH:MM`；无效值会报 `xml.skin.value` 且不应用任何内容。`open="true"` 在生成被安装时打开弹出层。范围仅在 Rust 中可用；在构造时设置它。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, time, on_change)` | `&'static str, Time, impl Fn(Time) -> M` | — | 创建该选择器；`110 × 34` DIP，已关闭，无范围。 |
| `.enabled(bool)` / `.disabled()` | `bool` | `true` | 启用状态；禁用会关闭弹出层。 |
| `.time()` / `.value()` | — | — | 当前时间。 |
| `.set_time(Time)` / `.set_value(Time)` | `Time` | — | 替换时间，钳制到范围内；返回是否发生了更改。 |
| `.is_open()` / `.open()` / `.close()` | — | `false` | 弹出层状态。`close` 恢复到弹出层打开时的数值。 |
| `.range()` | — | `(None, None)` | `(min, max)` 边界。 |
| `.set_range(Option<Time>, Option<Time>)` | `Option<Time>, Option<Time>` | — | 包含端点的范围；交换颠倒的边界并钳制；返回是否有任何更改。 |
| `.value_range(Option<Time>, Option<Time>)` | `Option<Time>, Option<Time>` | — | `set_range` 的构建器形式。 |
| `.min_time(Time)` / `.max_time(Time)` | `Time` | 无 | 一侧边界的构建器形式。 |
| `.clear_range()` | — | — | 移除两个边界。 |
| `.selectable(Time)` | `Time` | — | 某个时间是否可被选择。 |
| `.corner_radius(f32)` / `.corners(Corners)` | — | 零 | 圆角化字段和焦点环。 |
| `.background(Color)` / `.transparent_background(bool)` | — | 主题 | 字段表面覆盖。 |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | 无 | 附加型点击观察器；旁边还有右键/双击、按键和焦点观察器。 |

### 行为

- **键盘。** 上/下将活动单位步进一，PageUp/PageDown 步进五，Home/End 跳转到 `0`/最大值，左/右在小时和分钟之间切换，Enter/Space 提交。各列从上到下递增渲染，因此“下”会增大数值。
- **指针。** 点击某个小时会设置它并移到分钟；点击某个分钟会提交并关闭。打开时点击字段会取消（恢复）。滚轮步进悬停的列；当指针位于某列之外时，步进活动的列。
- **提交。** 通过 `commit`（Enter、Space、点击分钟）关闭时，仅当数值与打开时的数值不同才上报 `on_change`。`close`（Escape、点击字段、外部按下）不上报任何内容。
- **无障碍性。** 角色为 `button`，值为 `HH:MM`，带有展开状态，并在禁用时为 `read_only`。

`Time` 值类型有自己的构造函数和辅助方法：

| 项 | 签名 | 描述 |
|---|---|---|
| `Time::new` | `(hour: u32, minute: u32) -> Time` | 将小时钳制到 `0..=23`，分钟钳制到 `0..=59`。 |
| `Time::now` | `() -> Time` | 在 Windows 上为本地时钟时间；在其他平台上为 `00:00`。 |
| `Time::parse` | `(&str) -> Option<Time>` | 解析 `HH:MM`；拒绝越界的部分。 |
| `Time::display` | `(self) -> String` | `HH:MM`，24 小时制。 |

`Time` 为 `Copy + Ord`，且 `Default` 为 `00:00`。

## 事件

### 消息

| 消息 | 触发条件 |
|---|---|
| 作为 `on_change` 传给 `TimePicker::new` 的 `Fn(Time) -> M` | 一次提交（点击分钟，或 `Enter`/`Space`）发现的数值与弹出层打开时的数值不同。 |

通过 Escape 或点击字段关闭的数值，或等于打开时数值的数值，不上报任何内容。

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

无。`on_change` 是唯一的回调；范围构建器——`value_range`、`min_time`、`max_time`、`set_range`、`clear_range`——是配置，决定哪些时间可以被选择。

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("alarm"), Trigger::Click, |event, ctx| {
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

选择器响应 `Click`（字段和列行）、`Activate`（打开弹出层）、打开时的方向键/PageUp/PageDown/Home/End/Enter 键、打开时的 `Wheel`（步进悬停的或活动的列），以及 `HoverEnter`/`HoverLeave` 以实现行悬停；它忽略 `DragStart`/`DragEnd` 和 `ValueChange`。

## XML 定义

- **创建：** 无 `type`；该控件仅可被定位。
- **几何：** 一种水平形式和一种垂直形式。字段偏好 34 DIP 高度；弹出层绘制在矩形之外。
- **内容：** 共享内容名称均不适用；时间是局部属性。
- **局部属性：** `time`（`Text`，用 `Time::parse` 校验）和 `open`（`Bool`）。
- **样式：** `radius`、`corners`、`background`、`font-size`、`font-family`。
- **属性：** 通用运行时集合（`enabled`、`visible`、`tooltip`、`name`、`focus-order`、……）。
- **子元素：** 无。

## 另见

- `./date-picker.md`
- `./color-picker.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
