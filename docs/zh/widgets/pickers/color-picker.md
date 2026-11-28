# ColorPicker

> 一个颜色字段，会打开一个带可选透明度条的窗口内调色板弹出层。

## 介绍

`ColorPicker<M>` 是一个复合选择器：字段在一个棋盘格（以便透明度可见）上显示一个小色块、显示为 `#RRGGBB` 或 `#RRGGBBAA` 的颜色，以及一个尖角符号。点击字段，或按下 Enter，会打开一个带有内置 8 × 6 调色板的弹出层。选中一个色块会上报该颜色并关闭弹出层。Escape 会关闭而不更改数值。

`allow_alpha` 在调色板下方增加一条八级透明度条；选择某一级会上报新的 `Rgba` 并保持弹出层打开，以便用户在同一个调色板上调整透明度。`on_more` 增加一个“更多颜色…”行，它会关闭弹出层并上报一条移交消息，应用据此打开自己的完整颜色对话框。弹出层作为覆盖层绘制，当下方没有空间时会翻转到字段上方，并在打开期间独占鼠标滚轮。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::ColorPicker;
use bamboo_native_ui::{Color, Rect, Rgba, Ui, WidgetId};

enum Msg {
    Accent(Rgba),
    OpenColorDialog,
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    ColorPicker::new_rgba("accent", Rgba::opaque(Color::rgb([25, 113, 194])), Msg::Accent)
        .allow_alpha(true)
        .on_more(|| Msg::OpenColorDialog),
);
ui.set_rect(WidgetId::new("accent"), Rect::from_xywh(16.0, 16.0, 190.0, 34.0));
```

`new` 上报 `Fn(Color) -> M`；`new_rgba` 从一开始就上报 `Fn(Rgba) -> M`。`on_change_rgba` 将 `new` 构建的选择器切换为 `Rgba` 回调。当调色板色块或透明度级更改数值时，该回调触发；选择已经为当前的颜色会关闭弹出层且不上报任何内容。

## 用法（XML）

`ColorPicker` 不由标准注册表创建。在 Rust 中创建它，并通过 `id` 定位；文档可以预置颜色和打开状态：

```xml
<widget id="accent" x="16" y="16" w="190" h="34"
        color="#1971C280" open="false"/>
```

`color` 使用共享的颜色语法并保留其 alpha，因此 `#rrggbbaa` 可预置一个半透明默认值（不透明的值会变为 alpha 255）。`open="true"` 在文档生成被安装时打开弹出层。*用户*是否可以选择 alpha 是仅在 Rust 中可用的 `allow_alpha` 标志；无论哪种方式，应用设置的数值都会保留其 alpha。没有子元素。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, color, on_change)` | `&'static str, Color, impl Fn(Color) -> M` | — | 创建一个不透明选择器；`190 × 34` DIP。 |
| `new_rgba(id, rgba, on_change)` | `&'static str, Rgba, impl Fn(Rgba) -> M` | — | 创建一个上报颜色 + alpha 的选择器。 |
| `.enabled(bool)` / `.disabled()` | `bool` | `true` | 启用状态；禁用会关闭弹出层。 |
| `.on_change_rgba(impl Fn(Rgba) -> M)` | 闭包 | 无 | 将上报切换为 `Rgba`。 |
| `.on_more(impl Fn() -> M)` | 闭包 | 无 | 增加“更多颜色…”行及其移交消息。 |
| `.allow_alpha(bool)` | `bool` | `false` | 显示透明度条。 |
| `.set_allow_alpha(bool)` | `bool` | — | 运行时切换条；返回是否发生了更改。 |
| `.allows_alpha()` | — | `false` | 是否显示该条。 |
| `.color()` / `.value()` | — | — | 当前颜色。 |
| `.rgba()` / `.alpha()` | — | — | 带 alpha 的当前颜色，以及 alpha 字节。 |
| `.set_color(Color)` / `.set_value(Color)` | `Color` | — | 替换颜色，保留 alpha；返回是否发生了更改。 |
| `.set_rgba(Rgba)` | `Rgba` | — | 替换颜色和 alpha；返回是否有任何更改。 |
| `.set_alpha(u8)` | `u8` | — | 替换 alpha；返回是否发生了更改。 |
| `ColorPicker::<M>::palette()` | — | — | 内置的 8 × 6 调色板（`&'static [Color]`）。 |
| `.is_open()` / `.open()` / `.close()` | — | `false` | 弹出层状态；`open`/`close` 返回是否更改了它。 |
| `.corner_radius(f32)` / `.corners(Corners)` | — | 零 | 圆角化字段和焦点环。 |
| `.background(Color)` / `.transparent_background(bool)` | — | 主题 | 字段表面覆盖。 |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | 无 | 附加型点击观察器；旁边还有右键/双击、按键和焦点观察器。 |

### 行为

- **字段。** 一个 20 DIP 的色块、十六进制字符串（不透明时为 `#RRGGBB`，否则为 `#RRGGBBAA`）以及一个尖角符号；聚焦或打开时边框变为 `accent`。`preferred_height` 为 34 DIP。
- **键盘。** Enter/Space 打开弹出层，方向键移动光标，Home/End 跳转到行的边缘，Enter/Space 选中，Escape 关闭并保留数值。选中当前颜色会关闭且不上报消息。
- **指针。** 点击字段切换弹出层；点击其外部（通过容器的关闭路径）会关闭它。打开时点击字段会关闭而不更改任何内容。
- **透明度。** 该条有八个固定级别：`0, 36, 73, 109, 146, 182, 219, 255`。关闭 `allow_alpha` 只会移除面向用户的条。该条和“更多”行使用内置标题 `透明度` 和 `更多颜色…`；它们不可配置。
- **无障碍性。** 角色为 `button`，值为十六进制字符串，带有展开状态，并在禁用时为 `read_only`。

## 事件

### 消息

| 消息 | 触发条件 |
|---|---|
| 作为 `on_change` 传给 `ColorPicker::new` 的 `Fn(Color) -> M` | 调色板色块或透明度级更改数值，且弹出层以该更改关闭。 |
| 传给 `ColorPicker::new_rgba` 或 `.on_change_rgba` 的 `Fn(Rgba) -> M` | 同上，上报颜色加 alpha。 |
| 传给 `.on_more` 的 `Fn() -> M` | “更多颜色…”行被激活；弹出层关闭，应用打开自己的对话框。 |

选择已经为当前的颜色会关闭弹出层且不上报任何内容。

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
| `.on_change_rgba(f)` | `Fn(Rgba) -> M` | 替换 `new` 构建的选择器上的上报器，因此更改会上报 `Rgba`。 |
| `.on_more(f)` | `Fn() -> M` | 安装“更多颜色…”行及其移交消息。 |

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

选择器响应 `Click`（字段和弹出层槽位）、`Activate`（打开弹出层）、打开时的方向键/Home/End/Enter/Space 键，以及 `HoverEnter`/`HoverLeave` 以实现槽位悬停；它忽略 `Wheel`、`DragStart`/`DragEnd` 和 `ValueChange`。

## XML 定义

- **创建：** 无 `type`；该控件仅可被定位。
- **几何：** 一种水平形式和一种垂直形式。字段本身为 `preferred_height = 34`；弹出层绘制在矩形之外。
- **内容：** 共享内容名称均不适用；颜色是局部 `color` 属性。
- **局部属性：** `color`（`Color`，接受 alpha）和 `open`（`Bool`）。
- **样式：** `radius`、`corners`、`background`、`font-size`、`font-family`。
- **属性：** 通用运行时集合（`enabled`、`visible`、`tooltip`、`name`、`focus-order`、……）。
- **子元素：** 无。

## 另见

- `../controls/color-swatch.md`
- `./date-picker.md`
- `./time-picker.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
