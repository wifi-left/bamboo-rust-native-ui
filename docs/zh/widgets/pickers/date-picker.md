# DatePicker

> 一个日期字段，会打开一个带可选选择范围的窗口内月历。

## 介绍

`DatePicker<M>` 是一个复合选择器。字段将日期绘制为 `YYYY-MM-DD` 并绘制一个日历字形；点击它，或按下 Enter，会打开一个月网格弹出层。弹出层有一个带有上个月/下个月箭头和 `YYYY 年 M 月` 的标题、一行以星期一为首的工作日行，以及一个 7 × 6 的日期网格。选中的日期用强调色填充，今天用强调色着色，键盘光标用轮廓线标出，选择范围之外的日期绘制为禁用且无法选择。

选择某一天会通过 `on_change` 上报它并关闭弹出层。Escape、点击字段，或外部按下会关闭而不更改数值。弹出层作为覆盖层绘制，当下方放不下时会翻转到字段上方。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{Date, DatePicker};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    StartDate(Date),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    DatePicker::new("start", Date::today(), Msg::StartDate)
        .min_date(Date::new(2026, 1, 1))
        .max_date(Date::new(2026, 12, 31)),
);
ui.set_rect(WidgetId::new("start"), Rect::from_xywh(16.0, 16.0, 200.0, 34.0));
```

`on_change` 是 `Fn(Date) -> M`。范围是包含端点的；`min_date`/`max_date` 一次设置一侧，`value_range(Option<Date>, Option<Date>)` 同时设置两者。传给 `set_range` 的颠倒数值对会被交换，使得 `min <= max`。

## 用法（XML）

`DatePicker` 不由标准注册表创建。在 Rust 中创建它，并通过 `id` 定位；文档可以预置日期和打开状态：

```xml
<widget id="start" x="16" y="16" w="200" h="34"
        date="2026-11-28" open="false"/>
```

`date` 必须能通过 `Date::parse` 解析为 `YYYY-MM-DD`；无效值会报 `xml.skin.value` 且不应用任何内容。`open="true"` 在生成被安装时打开日历。可选择范围仅在 Rust 中可用，因为它接受 `Date` 值；在构造时用 `min_date`/`max_date` 设置它。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, date, on_change)` | `&'static str, Date, impl Fn(Date) -> M` | — | 创建该选择器；`200 × 34` DIP，已关闭，无范围。 |
| `.enabled(bool)` / `.disabled()` | `bool` | `true` | 启用状态；禁用会关闭弹出层。 |
| `.date()` / `.value()` | — | — | 当前日期。 |
| `.set_date(Date)` / `.set_value(Date)` | `Date` | — | 替换日期，钳制到范围内；返回是否发生了更改。 |
| `.range()` | — | `(None, None)` | `(min, max)` 边界。 |
| `.set_range(Option<Date>, Option<Date>)` | `Option<Date>, Option<Date>` | — | 包含端点的范围；交换颠倒的边界，钳制数值；返回是否有任何更改。 |
| `.value_range(Option<Date>, Option<Date>)` | `Option<Date>, Option<Date>` | — | `set_range` 的构建器形式。 |
| `.min_date(Date)` / `.max_date(Date)` | `Date` | 无 | 一侧边界的构建器形式。 |
| `.clear_range()` | — | — | 移除两个边界。 |
| `.selectable(Date)` | `Date` | — | 某一天是否可被选择。 |
| `.is_open()` | — | `false` | 弹出层状态。 |
| `.view_month()` | — | — | 显示的 `(year, month)`。 |
| `.open()` / `.close()` | — | — | 更改弹出层状态；两者都返回是否发生了更改。 |
| `.corner_radius(f32)` / `.corners(Corners)` | — | 零 | 圆角化字段和焦点环。 |
| `.background(Color)` / `.transparent_background(bool)` | — | 主题 | 字段表面覆盖。 |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | 无 | 附加型点击观察器；旁边还有右键/双击、按键和焦点观察器。 |

### 行为

- **键盘。** 左/右移动一天，上/下移动一周，PageUp/PageDown 移动一个月（钳制到可选择范围），Home/End 到本周的开始/结束，Enter/Space 选中。选中单元格的着色和光标环是分开的：移动光标不会更改数值。
- **范围。** 当前数值和光标都钳制到范围内，包括弹出层打开期间范围发生变化时。禁用的选择器仍接受来自应用的 `set_value`；它只拒绝用户输入。
- **关闭语义。** `close` 丢弃光标并保留已提交的数值。选择已经选中的日期仍会上报它；选择范围之外的日期不上报任何内容。
- **无障碍性。** 角色为 `button`，值为 `YYYY-MM-DD`，带有展开状态，并在禁用时为 `read_only`。

`Date` 值类型有自己的构造函数和辅助方法：

| 项 | 签名 | 描述 |
|---|---|---|
| `Date::new` | `(year: i32, month: u32, day: u32) -> Date` | 将年钳制到 `>= 1`，月钳制到 `1..=12`，日钳制到该月的长度。 |
| `Date::today` | `() -> Date` | 在 Windows 上为本地日期；在其他平台上为 `1970-01-01`。 |
| `Date::parse` | `(&str) -> Option<Date>` | 解析 `YYYY-MM-DD`；拒绝超出 `1..=12` 的月份或为 `0` 的日。 |
| `Date::display` | `(self) -> String` | `YYYY-MM-DD`。 |
| `Date::add_days` | `(self, days: i64) -> Date` | 之后或之前 `days` 个日历日的日期。 |
| `Date::first_of_month` | `(self) -> Date` | 同一月份的第一天。 |
| `Date::days_in_month` | `(year, month) -> u32` | 月份长度，能感知闰年。 |
| `Date::monday_index` | `(self) -> u32` | 星期几，`0` = 星期一 … `6` = 星期日。 |

`Date` 为 `Copy + Ord`，且 `Default` 为 1970-01-01。

## 事件

### 消息

| 消息 | 触发条件 |
|---|---|
| 作为 `on_change` 传给 `DatePicker::new` 的 `Fn(Date) -> M` | 某一天通过点击或 `Enter`/`Space` 被选中；随后弹出层关闭。选择已经选中的日期仍会上报。 |

可选择范围之外的日期不上报任何内容。

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

无。`on_change` 是唯一的回调；范围构建器——`value_range`、`min_date`、`max_date`、`set_range`、`clear_range`——是配置，决定哪些日期可以被上报。

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("start"), Trigger::Click, |event, ctx| {
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

选择器响应 `Click`（字段、日期和月份箭头）、`Activate`（打开弹出层）、打开时的方向键/PageUp/PageDown/Home/End/Enter 键，以及 `HoverEnter`/`HoverLeave` 以实现日期悬停；它忽略 `Wheel`、`DragStart`/`DragEnd` 和 `ValueChange`。

## XML 定义

- **创建：** 无 `type`；该控件仅可被定位。
- **几何：** 一种水平形式和一种垂直形式。字段偏好 34 DIP 高度；弹出层绘制在矩形之外。
- **内容：** 共享内容名称均不适用；日期是局部属性。
- **局部属性：** `date`（`Text`，用 `Date::parse` 校验）和 `open`（`Bool`）。
- **样式：** `radius`、`corners`、`background`、`font-size`、`font-family`。
- **属性：** 通用运行时集合（`enabled`、`visible`、`tooltip`、`name`、`focus-order`、……）。
- **子元素：** 无。

## 另见

- `./time-picker.md`
- `./color-picker.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
