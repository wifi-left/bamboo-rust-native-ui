# CustomWidget

> 一个用于应用特定组件的闭包绘制控件。

## 介绍

`CustomWidget<M>` 是添加库未提供的组件的轻量方式。它携带一个矩形以及用于绘制和输入的应用闭包，并且像内置控件一样被注册、聚焦、命中测试并发布到无障碍。将其用于不需要自有状态的小型组件；当组件拥有状态或需要自己的布局行为时，请直接实现 `Widget<M>`。

绘制闭包接收绘制上下文、控件矩形和容器状态（悬停、按下、焦点、启用）。容器会 blit 控件的脏矩形，因此闭包必须只在 `rect` 内绘制。输入闭包接收矩形和一个 `WidgetInput`，并返回 `Option<M>`；`.hit_test` 为圆形或带缺口的卡片替换默认的矩形命中区域。无障碍是固定的 `A11yInfo`（`.a11y` / `.a11y_name`），或针对每帧都变化的值使用动态闭包（`.a11y_fn`）。

已配置的表面（`.corner_radius`、`.corners`、`.background`、`.transparent_background`）会先绘制，因此闭包通常只绘制内容。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::CustomWidget;
use bamboo_native_ui::{
    A11yRole, Color, Cursor, Rect, TextAlign, TextStyleRole, Ui, WidgetId, WidgetInput,
};

enum Msg {
    Clicked,
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    CustomWidget::new("meter", |ctx, rect, state| {
        let inner = rect.inset(4.0, 4.0);
        ctx.painter
            .round_rect(&mut *ctx.canvas, inner, 8.0, ctx.palette.accent);
        ctx.text(
            inner,
            "72%",
            TextStyleRole::Body,
            TextAlign::Center,
            if state.hovered { ctx.palette.text } else { ctx.palette.muted },
        );
    })
    .rect(Rect::from_xywh(16.0, 16.0, 160.0, 40.0))
    .focusable(true)
    .interactive(true)
    .cursor(Cursor::Hand)
    .corner_radius(10.0)
    .background(Color::new(43, 47, 58))
    .a11y_name(A11yRole::Button, "Level meter")
    .input(|_rect, input| match input {
        WidgetInput::ClickAt(_) | WidgetInput::Activate => Some(Msg::Clicked),
        _ => None,
    }),
);
```

指针命中测试要落到控件上，需要 `.interactive(true)`；将其与 `.input` 配合使用。`.focusable(true)` 使其成为 Tab 停靠点，容器会绘制焦点环。`ui.add_custom(id, rect, paint)` 是仅绘制的简写。

## 用法（XML）

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。它没有内置的共享内容或子列表，也不声明局部皮肤属性，因此文档只能移动、显示/隐藏和设置其样式。

```xml
<widget id="meter" x="16" y="16" w="160" h="40" radius="10" background="#2B2F3A"/>
```

`.corner_radius`、`.corners` 和 `.background` 到达布局的 `SurfaceStyle` 接缝，因此 `radius`、`corners` 和 `background` token 会生效。闭包控件无法获得 `x-*` 属性或子元素；为此请直接实现 `Widget<M>`。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, paint)` | `&'static str, impl Fn(&mut WidgetPaint, Rect, &WidgetState)` | — | 创建该控件；`120 × 32` DIP。 |
| `.rect(Rect)` | `Rect` | 原点处的 `120 × 32` | 初始矩形（布局可能会移动它）。 |
| `.preferred_height(f32)` | `f32` | `32.0` | 在垂直堆叠中期望的高度。 |
| `.focusable(bool)` | `bool` | `false` | 接受键盘/无障碍焦点并获得焦点环。 |
| `.interactive(bool)` | `bool` | `false` | 指针命中测试可以落到控件上。 |
| `.enabled(bool)` | `bool` | `true` | 初始启用状态。 |
| `.cursor(Cursor)` | `Cursor` | 无 | 控件上方的指针光标。 |
| `.input(handler)` | `FnMut(Rect, WidgetInput) -> Option<M>` | 无 | 输入处理函数；`None` 保持静默。 |
| `.hit_test(handler)` | `Fn(Rect, Point) -> bool` | 矩形 | 非矩形命中区域。 |
| `.a11y(A11yInfo)` | `A11yInfo` | 无 | 固定的无障碍节点。 |
| `.a11y_name(role, name)` | `A11yRole, impl Into<String>` | 无 | 固定角色 + 名称的简写。 |
| `.a11y_fn(handler)` | `Fn() -> Option<A11yInfo>` | 无 | 动态无障碍节点（优先检查）。 |
| `.corner_radius(f32)` / `.corners(Corners)` | radius / corners | 零 | 表面圆角。 |
| `.background(Color)` / `.background_option(Option<Color>)` | `Color` / `Option<Color>` | 主题 | 表面填充覆盖。 |
| `.transparent_background(bool)` | `bool` | `false` | 保留边框，去掉填充。 |
| `.surface()` | `&SurfaceStyle` | — | 已配置的表面。 |

### 手动控件

拥有状态的组件直接实现 `Widget<M>`：提供 `id`、`rect`/`set_rect`、`as_any`/`as_any_mut`、`paint` 和 `input`，然后根据需要覆盖 `preferred_height`、`focusable`、`interactive`、`hit_test`、`cursor`、`a11y` 和焦点钩子。当类型拥有 `SurfaceStyle` 时转发 `surface_style` / `surface_style_mut`，并存储一个带有 `observer_methods!()` 宏（加上 `observer_builders!`）的 `Observers<M>` 字段，以支持附加型的 `on_click` / `on_key_down` / `on_focus_*` 构建器。`surface_builders!` 宏添加相同的 `.corner_radius` / `.background` 系列。

要接受文档数据，请用皮肤协议声明它：`skin_attrs` 和 `skin_children` 发布词汇，`validate_skin_attr` / `validate_skin_children` 无副作用，`apply_skin_attr` / `apply_skin_children` 静默写入。以 `x-` 开头的名称属于应用；共享内容名称是保留的。showcase 的 `crates/showcase/src/meter.rs` 是一个完整的第三方示例。

## 事件

### 消息

| 消息 | 发出条件 |
|---|---|
| 传给 `.input` 的 `impl FnMut(Rect, WidgetInput) -> Option<M>` | 在 `.interactive(true)`（指针）或 `.focusable(true)`（键盘）允许其落地时，路由到控件的每个 `WidgetInput`；返回 `None` 保持静默。 |

闭包接收控件矩形和原始输入，因此它决定哪些 `WidgetInput` 变体重要。`Ui::bind` 触发器和闭包都会排队消息；它们彼此独立。

### 通用观察器

此控件不安装任何共享的观察器构建器。`.input` 闭包是其唯一的输入接缝；手动的 `Widget<M>` 实现则改为存储一个带有 `observer_methods!` / `observer_builders!` 的 `Observers<M>` 字段。

### 控件专属回调

| 回调 | 签名 | 触发条件 |
|---|---|---|
| `.input(handler)` | `FnMut(Rect, WidgetInput) -> Option<M>` | 传递给控件的任何输入（悬停、按下、拖动、释放、点击、双击、滚轮、按键、`Step`、`Activate`）。 |
| `.hit_test(handler)` | `Fn(Rect, Point) -> bool` | 在指针事件之前：决定它是否落在控件上（默认为矩形）。 |

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("meter"), Trigger::Click, |event, ctx| {
    // 返回 Some(Msg::...) 以排队一条消息；调用 ctx.default() 以让
    // 输入闭包运行，否则它会保持被抑制状态。
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

该控件响应其输入闭包所处理的任何触发器：绑定处理函数先运行，调用 `ctx.default()` 随后将同一事件传递给 `.input`。指针触发器（包括 `Wheel`）需要 `.interactive(true)`；`Activate` 和按键输入需要 `.focusable(true)`。该控件不发布无障碍值，因此 `ValueChange` 从不触发。

## XML 定义

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。

几何遵循常规的 `<widget>` 规则。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`、`expose`、enabled、焦点策略）适用。通过 `SurfaceStyle`，支持 `radius`、`corners` 和 `background`；使用 `ctx.text` 绘制的文本遵循 `font-size` 和 `font-family`。`CustomWidget` 不发布任何内容能力，因此 `text`、`label`、`value`、`progress`、`selected` 之类会被拒绝为 `xml.content.unsupported`，除非应用实现自己的控件。

控件局部属性：无。子元素：无。仅在直接实现 `Widget<M>` 时声明 `x-*` 局部值和子元素。

## 另见

- `../../guides/xml-layout-authoring.md#custom-widgets`
- `../../api/widget-trait.md`
- `../overlay/popover.md`
- `../index.md`
- `../../quickstart/04-custom-widgets.md`
