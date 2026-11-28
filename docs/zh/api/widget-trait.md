# 控件 trait

> 保留式控件的契约：身份、绘制、输入翻译，以及可选的编辑、无障碍和皮肤协议。

## 概述

`Widget<M>` 就是 `Ui<M>` 所存储的东西：每次页面重建时创建一次，并保留
到下一次重建，它既不拥有悬停、按下、焦点，也不拥有失效状态——
这些由容器持有。它的两项必需职责是：为某个
`WidgetState` 绘制其矩形，以及将高层输入翻译为类型为 `M` 的可选消息。

其余每个方法都提供了一个保守的默认实现，因此当框架演进时控件可以保持不变：
一个普通按钮会忽略内联编辑、
拖放、皮肤和遮罩钩子，而第三方控件可以从 crate 外部选择性加入。一个
永不发出消息的控件可以使用 `M = ()`。

## 关键类型

### `WidgetId`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WidgetId(pub &'static str);
impl WidgetId {
    pub const fn new(name: &'static str) -> Self;
    /// 每个进程对运行时文本驻留一次。
    pub fn intern(name: &str) -> Self;
    pub const fn name(self) -> &'static str;
}
```

相等性按文本判定：`WidgetId::intern("row-1")` 等于 `WidgetId::new("row-1")`。
字面量用 `new`，对于有界的运行时名称集合用 `intern`；每个
不同的驻留名称会泄漏一个小字符串。

### `WidgetState` 与 `WidgetPaint`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetState {
    pub hovered: bool, pub pressed: bool, pub focused: bool, pub adjusting: bool,
    pub enabled: bool, pub focus_visible: bool, pub editing: bool, pub caret_on: bool,
}

pub struct WidgetPaint<'a> {
    pub painter: &'a Painter,
    pub canvas: &'a mut dyn Canvas,
    pub palette: Palette,
    pub font: Option<FontSpec>,
    pub images: &'a ImageStore,
    pub viewport: Rect,
    pub dirty: &'a [Rect],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyleRole { Body, Small, Title, Brand, Glyph }
```

`WidgetPaint` 的文本辅助方法会应用每个实例的字体覆盖（绝不应用到
`Glyph` 文本片段），其图像辅助方法对未知 `ImageId` 返回 `false`。

### `WidgetInput`

```rust
#[derive(Clone, Copy, Debug)]
pub enum WidgetInput {
    HoverEnter, HoverLeave, HoverAt(Point), PressAt(Point), DragTo(Point),
    ReleaseAt(Point), ClickAt(Point), SelectWith(Point, PointerModifiers),
    DoubleClickAt(Point), RightClickAt(Point), DragStart(Point), DragEnd(Point),
    Wheel(f32), Step(i32), Activate, Char(char), Key(KeyEvent), KeyUp(KeyEvent),
}
```

容器已经完成命中测试的解析。`SelectWith` 只发给
`multi_selectable` 控件；`Step` 是滑块调整。

### 其他契约类型

```rust
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub title: String, pub subtitle: String, pub trailing: String,
    pub current: bool, pub disabled: bool,
}
// 流式构建器：new、subtitle、trailing、current、disabled。

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollBars { None, #[default] Vertical, Horizontal, Both }

pub struct EditRequest<'a> {
    pub id: WidgetId, pub rect: Rect, pub text: &'a str, pub cue: &'a str,
    pub password: bool, pub multiline: bool, pub wrap: bool,
    pub scrollbars: ScrollBars, pub numbers_only: bool, pub align: TextAlign,
    pub font_size: Option<f32>, pub font_family: Option<String>, pub max_length: u32,
}
```

`SessionSnapshot`（由 `take_session` /
`restore_session` 使用的捕获值）也是一个普通结构体，从 `widget` 重导出。

## 必需方法

```rust
pub trait Widget<M> {
    fn id(&self) -> WidgetId;
    fn rect(&self) -> Rect;
    fn set_rect(&mut self, rect: Rect);

    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState);
    fn input(&mut self, input: WidgetInput) -> Option<M>;
}
```

`as_any`/`as_any_mut` 是供 `Ui::with_widget` 和
`Ui::edit_widget` 使用的向下转型接缝；无论哪种情况，容器都保留失效和无障碍规则。

## 绘制与输入契约

- **绘制**（Paint）是 `&self` 且无副作用。只绘制与
  `ctx.dirty` 相交的内容；后缓冲区保留其余部分。全表面效果（例如一层遮罩）
  必须限制在脏块范围内——在已绘制的像素上合成 alpha 在视觉上是错误的。
  绘制路径上不得分配。
- **输入**（Input）是 `&mut self` 并返回消息或 `None`。每次输入之后
  容器会取出 `take_extra_message`（一条额外的附加消息）和
  `take_repaint_hint`（一个更窄的重绘矩形），因此两者都不会泄漏到后续
  事件中。
- 容器决定控件能看到什么：`interactive()` 控制指针
  命中测试，`hit_test` 缩小形状，`visible()` 将其从输入、
  焦点和树中移除，而 `paint_bounds()` 是被失效的内容（弹窗
  报告其并集）。`paints_pointer_state()` 让背景可以跳过悬停
  失效。

## 提供的方法

### 布局、可见性、绘制提示与输入

| 方法 | 默认值与说明 |
|---|---|
| `preferred_height`、`enabled`、`focusable`、`visible` | `36.0`；`true`；`true`；`true`。隐藏的控件不接收输入、Tab、绘制或无障碍。 |
| `interactive`、`hit_test`、`cursor`、`cursor_at` | `self.focusable()`；整个矩形；`None`（回退到 `Hand`/`Arrow`）；按位置的光标。 |
| `paint_bounds`、`overlay`、`is_backdrop` | `rect()`；`false`（最后绘制、最先命中）；`false`（开始模态层；仅 `Scrim`）。 |
| `paints_pointer_state`、`repaint_on_move`、`repaint_on_move_at` | `true`；`false`；按位置的变体。控制悬停驱动的失效。 |
| `take_repaint_hint`、`scrolls_with_wheel` | `None`（更窄的重绘）；`false`（控件消费滚轮档位）。 |
| `focus_inset`、`focus_ring_rect`、`focus_ring_radius` | `2.0`；按它内缩的矩形；`None` 表示矩形环（胶囊返回一个半径）。 |
| `take_extra_message`、`adjustable`、`multi_selectable` | `None`（在主消息之后排队）；`false`（方向键调整模式）；`false`（选择加入 `SelectWith`）。 |
| `mnemonic`、`mnemonic_key`、`set_mnemonic_underlines` | `None`（`&File` → `F`）；`false`（占用一个 `Alt+key`）；`false`（返回视觉是否发生变化）。 |
| `owns_navigation_keys`、`owns_navigation_key`、`dismiss` | `false`（打开的遮罩拥有方向键/Enter/Space）；按键对应的变体；`false`（Escape 关闭遮罩）。 |

### 编辑——原生与内联

| 方法 | 默认值与说明 |
|---|---|
| `wants_edit`、`wants_edit_at`、`edit_request`、`commit_text` | `false`；更窄的单击区域；`None`；`None`。一个原生 `EDIT` 子窗口；关闭时控件继续绘制自己的文本。 |
| `inline_edit`、`inline_edit_at`、`editing`、`multiline_edit` | `false`；更窄的按压区域；`false`；`false`（Enter 插入换行）。 |
| `begin_inline_edit`、`finish_edit` | `false`；`None`。开始/重新定位，然后提交或还原。 |
| `insert_text`、`observe_char`、`take_change_message` | `false`；`None`；`None`。键入的字符、粘贴和已提交的 IME；观察者在插入之前运行。 |
| `set_composition`、`take_clipboard`、`caret_rect`、`edit_menu`、`edit_command` | `false`；`None`；`None`；`None`；`false`。IME 组合、复制的文本、供 IME/闪烁使用的插入符、内置菜单、保留的命令 id。 |

### 触摸、拖放文本与会话

| 方法 | 默认值与说明 |
|---|---|
| `long_press`、`grab_selection_handle` | `false`；选中该词并显示手柄。`false`；按压抓住了已绘制的手柄。 |
| `take_drag_text`、`insert_dropped_text` | `None`；复制出选区。`false`；插入被放置的字符串。 |
| `take_session`、`restore_session` | `None`；`false`。供 `Ui::remove_preserving_session` 捕获/恢复。 |
| `observe_click`、`observe_right_click`、`observe_double_click`、`observe_key_down`、`observe_key_up`、`observe_focus_gained`、`observe_focus_lost` | 全部为 `None`。附加观察者：每个在内置行为之前运行，内置行为仍然会运行。 |

### 无障碍、状态与菜单

| 方法 | 默认值与说明 |
|---|---|
| `a11y`、`a11y_select_child`、`a11y_set_value`、`a11y_set_value_at`、`a11y_expand`、`a11y_scroll` | `None`/`None`/`None`/转发/`None`（幂等展开）/`false`（UIA 滚动）。`a11y` 返回 `None` 会隐藏该控件。 |
| `set_enabled`、`set_value`、`set_toggle`、`set_selected`、`set_rows`、`set_label`、`set_progress`、`set_visible`、`set_message`、`set_text`、`set_number` | 全部为空操作。由容器的设置器推送；控件忽略它不使用的那些。 |
| `read_only`、`set_read_only` | `false`；空操作。由容器与 `WidgetProps::read_only` 进行逻辑或。 |
| `take_native_menu`、`native_menu`、`set_native_menu`、`tooltip_rect`、`child_at`、`focus_item` | `None`；`None`；`false`；`rect()`；`None`；`None`。弹窗请求、控件会打开的菜单，以及工具提示子项查询。 |

## 表面与皮肤协议

```rust
fn surface_style(&self) -> Option<&SurfaceStyle> { None }
fn surface_style_mut(&mut self) -> Option<&mut SurfaceStyle> { None }
fn content_caps(&self) -> ContentCaps { ContentCaps::NONE }

fn skin_attrs(&self) -> &'static [SkinAttrDecl] { &[] }
fn validate_skin_attr(&self, _name: &str, _value: &SkinValue<'_>) -> bool { true }
fn apply_skin_attr(&mut self, _name: &str, _value: &SkinValue<'_>) {}
fn skin_children(&self) -> &'static [&'static str] { &[] }
fn validate_skin_children(&self, _children: &[SkinElement]) -> bool { true }
fn apply_skin_children(&mut self, _children: &[SkinElement]) {}

pub struct SkinAttrDecl { pub name: &'static str, pub kind: SkinValueKind }
pub enum SkinValueKind { Text, Number, Bool, Color }
pub enum SkinValue<'a> { Text(&'a str), Number(f64), Bool(bool), Color(Rgba) }
```

控件公布它接受的本地属性和数据子元素。
`validate_*` 在应用任何内容之前针对每个值运行；`apply_*` 不得
触发回调或创建、移动、聚焦、移除控件。`x-` 名称保留给用户代码，
且 `skin` 模块始终编译——第三方控件无需 `feature = "layout-xml"` 即可选择加入，
不过只有该特性（或 `layout-compiled`）才会安装一个使用它的文档。

## 示例

```rust
use bamboo_native_ui::widget::{Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};
use bamboo_native_ui::{Color, Rect, TextAlign, TextStyleRole};

enum Msg {
    Ping,
}

struct Meter {
    id: WidgetId, rect: Rect, value: f32,
}

impl Widget<Msg> for Meter {
    fn id(&self) -> WidgetId { self.id }
    fn rect(&self) -> Rect { self.rect }
    fn set_rect(&mut self, rect: Rect) { self.rect = rect; }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let fill = if state.hovered { ctx.palette.accent } else { Color::new(60, 60, 60) };
        let label = format!("{:.0}%", self.value * 100.0);
        ctx.painter.round_rect(ctx.canvas, self.rect, 6.0, fill);
        ctx.text(self.rect, &label, TextStyleRole::Small, TextAlign::Center, ctx.palette.text);
    }

    fn input(&mut self, input: WidgetInput) -> Option<Msg> {
        matches!(input, WidgetInput::ClickAt(_) | WidgetInput::Activate).then_some(Msg::Ping)
    }
}
```

## 说明与限制

- `paint` 是 `&self` 且不得分配：在 `input`/设置器中构建字符串，
  在 `paint` 中借用它们。任何在绘制期间会变化的东西都应属于
  更早计算的可变状态，绝不能放在 `paint` 中。
- `WidgetState::default()` 的 `enabled: false`；请通过
  容器构造状态（或设置每个字段），而不是依赖默认值。
- `EditRequest` 从控件借用文本和提示；宿主不得在调用之后
  保留它。
- 保留的编辑器命令 id 位于 `widget::edit_command`（`UNDO`…`BOM`）；
  应用菜单不得复用该范围。

## 另见

- [Ui 容器](./ui-container.md)
- [事件与输入](./events-and-input.md)
- [绘制与 Canvas](./painting-and-canvas.md)
- [主题与调色板](./theme-and-palette.md)
- [文本编辑](../widgets/inputs/text-edit.md)
- [自定义控件快速上手](../quickstart/04-custom-widgets.md)
- [XML 布局编写](../guides/xml-layout-authoring.md)
