# Button

> 一个按钮控件，在点击或激活时上报一条应用消息。

## 介绍

`Button<M>` 是一个简单的非复合控件。它绘制一个圆角填充和居中的标签，或者——当设置了字形（glyph）时——先在左边缘绘制一条字形条带，随后是一个左对齐的标签。`ButtonVariant` 选择中性、强调或破坏性样式。

点击，或在获得焦点时按下 `Enter`/`Space`，会上报传给 `Button::new` 的消息。每次上报时都会克隆该消息，因此 `M: Clone`。附加型观察器（`on_click`、`on_focus_lost` 等）在内置动作之前运行，而内置动作仍然会执行；它们从不会取代内置动作。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{Button, ButtonVariant};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Save,
    Delete,
    DeleteClicked,
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(Button::new("save", "Save", Msg::Save).primary());
ui.add(
    Button::new("delete", "Delete", Msg::Delete)
        .variant(ButtonVariant::Danger)
        .glyph('\u{E74D}')
        .glyph_gap(6.0)
        .on_click(|_pos| Some(Msg::DeleteClicked)),
);
ui.set_rect(WidgetId::new("save"), Rect::from_xywh(16.0, 16.0, 120.0, 36.0));
ui.set_rect(WidgetId::new("delete"), Rect::from_xywh(16.0, 60.0, 120.0, 36.0));
```

`on_click` 接受 `Fn(Point) -> Option<M>`，且是附加型的：`Msg::DeleteClicked` 观察器先触发，随后内置的 `Msg::Delete` 仍会上报。使用 `.disabled()` 创建一个永久禁用的实例，或使用 `Ui::set_enabled` 在运行时切换其启用状态。

## 用法（XML）

`Button` 由标准注册表创建（`type="button"`）。交互类型需要一个 `command`，通过注册表的命令表解析：

```rust
use bamboo_native_ui::creation::WidgetRegistry;
use bamboo_native_ui::Ui;

let mut ui: Ui<Msg> = Ui::new();
let mut registry = WidgetRegistry::<Msg>::standard();
registry.set_commands(|command| match command {
    101 => Some(Msg::Save),
    _ => None,
});
ui.set_widget_registry(registry);
```

```xml
<widget id="save" type="button" command="101"
        x="16" y="16" w="120" h="36" label="Save"/>
```

文件提供 `label`（内容）以及几何和样式。它无法表达变体、字形或观察器；这些仍需留在 Rust 侧，要么在 Rust 中构建控件，要么对已创建的控件调用 `Ui::edit_widget`。`Button` 不声明任何控件局部属性。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, label, message)` | `&'static str, impl Into<String>, M` | — | 创建该按钮；`120 × 36` DIP，次要样式，已启用。 |
| `.glyph(char)` | `char` | 无 | 在左边缘的条带中绘制一个字形。 |
| `.glyph_gap(f32)` | `f32` | `0.0` | 字形条带与标签之间的间距（负值会被钳制为 0）。 |
| `.variant(ButtonVariant)` | `ButtonVariant` | `Secondary` | 强调：中性、强调或破坏性。 |
| `.primary()` | — | — | `ButtonVariant::Primary` 的简写。 |
| `.danger()` | — | — | `ButtonVariant::Danger` 的简写。 |
| `.disabled()` | — | `false` | 以禁用状态启动（`button_disabled` 填充、`text_disabled` 文本）。 |
| `.corner_radius(f32)` | `f32` | `0.0` | 圆角化填充，并在均匀时圆角化焦点环。 |
| `.corners(Corners)` | `Corners` | 零 | 按角设置半径（非均匀时保持矩形焦点环）。 |
| `.background(Color)` / `.background_option(Option<Color>)` | `Color` / `Option<Color>` | 主题 | 覆盖填充；`None` 恢复为使用主题。 |
| `.transparent_background(bool)` | `bool` | `false` | 保留边框，去除填充。 |
| `.surface()` | `&SurfaceStyle` | — | 已配置的表面。 |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | 无 | 附加型点击观察器。 |

### 行为

- **变体。** `Secondary` 使用 `button_bg` / `button_hover` / `button_pressed`。`Primary` 使用 `button_active` / `button_active_hover` / `button_pressed`。`Danger` 使用一种固定的红色，独立于强调色，悬停时变亮、按下时变暗。
- **字形布局。** 有字形时，条带占据左侧 8–34 DIP；标签从条带右边缘加上 `glyph_gap` 绘制到距右边缘 6 DIP 处。无字形时，标签在水平内缩 10 DIP 的矩形内居中。
- **无障碍性。** 角色为 `button`；无障碍名称是标签，当标签为空时为控件 id。鼠标点击和 `Activate`（Enter/Space）会上报相同的消息。

## 事件

### 消息

| 消息 | 触发条件 |
|---|---|
| 传给 `Button::new(id, label, message)` 的 `M` | 被点击，或在获得焦点且启用时按下 `Enter`/`Space`。 |

每次上报时都会克隆该消息，因此 `M: Clone`。

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

无。构造函数消息是唯一的回调；`glyph`、`variant`、`disabled` 以及表面构建器都是配置。

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("save"), Trigger::Click, |event, ctx| {
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

按钮响应 `Click` 和 `Activate`（两者都上报构造函数消息），并响应 `HoverEnter`/`HoverLeave` 以设置其悬停填充；它忽略 `Wheel`、`DragStart`/`DragEnd` 和 `ValueChange`。

## XML 定义

- **创建：** `type="button"` 加上 `command`（一个 `u32`）。缺少 `command`，或注册表没有命令表，会报 `xml.command.missing`。多变体文档的两个变体必须声明相同的 `type`/`command`/`parent`。
- **几何：** 常规规则——水平方向为 `x`+`w`、`right`+`w`、`x`+`right` 之一；垂直方向为 `y`+`h`、`bottom`+`h`、`y`+`bottom` 之一。
- **内容：** 仅 `label`（`content_caps` 为 `LABEL`）；`text`、`value` 和 `progress` 会以 `xml.content.unsupported` 被拒绝。
- **样式：** `radius`、`corners`、`background`、`font-size`、`font-family`，与每个拥有表面的控件相同的标记；`radius` 和 `corners` 冲突。
- **属性：** 通用运行时集合（`enabled`、`visible`、`tooltip`、`name`、`focus-order`、……）。
- **子元素：** 无。可选的 `parent="panel-id"` 将创建的按钮放入已注册的滚动面板中。

## 另见

- `./toggle.md`
- `./segmented.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
