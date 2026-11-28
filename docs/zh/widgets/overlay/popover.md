# Popover

> 一个感知锚点的浮层卡片，带有缺口、视口钳制和轻量关闭。

## 介绍

`Popover<M>` 是浮出层的主题表面：一个带边框的圆角面板，并可带有指向锚点矩形的可选缺口。它是一张浮层卡片，不是容器。应用仍然拥有布局，因此定位是一个辅助函数：使用锚点、卡片尺寸、`PopoverPlacement` 和客户端视口调用 `place_popover`，然后将返回的矩形设置在控件上。

卡片绘制在应用的内容控件之后。将这些控件作为普通兄弟节点添加在 popover **之后**，定位在 `Popover::content_rect` 内，这样它们会绘制在卡片之上并首先接收指针输入。popover 本身不绘制任何子项。

使用 `dismiss_on_outside`（默认开启）时，控件可见期间其命中区域覆盖整个客户端视口：在卡片之外的按下会返回 `on_dismiss` 消息并被吞掉，因此后方的页面既不会收到点击也不会收到悬停变化。在卡片上的按下也会被吞掉，且不产生消息。Escape 是应用要处理的按键（或使用卡片内的关闭控件）。对于非模态、允许页面接收点击的卡片，请关闭该选项；此时只有卡片矩形属于命中区域。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{place_popover, Label, Popover, PopoverPlacement};
use bamboo_native_ui::{Rect, Size, Ui, WidgetId};

enum Msg {
    Dismiss,
}

let anchor = Rect::from_xywh(16.0, 16.0, 120.0, 32.0);
let viewport = Rect::from_xywh(0.0, 0.0, 800.0, 600.0);
let card = place_popover(
    anchor,
    Size::new(240.0, 120.0),
    PopoverPlacement::Auto,
    viewport,
    8.0,
);

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Popover::new("flyout")
        .anchor(anchor)
        .pad(12.0)
        .arrow(true)
        .dismiss_on_outside(true)
        .name("Actions")
        .description("Press outside to close")
        .on_dismiss(|| Msg::Dismiss),
);
ui.set_rect(WidgetId::new("flyout"), card);

// 内容控件是添加在卡片之后的兄弟节点，位于其内边距之内。
let content = card.inset(12.0, 12.0);
ui.add(Label::new("flyout_title", "Nocturne"));
ui.set_rect(WidgetId::new("flyout_title"), content);
```

`place_popover` 不会移动卡片；它只计算矩形。`content_rect()` 是卡片按 `pad` 内缩后的区域，因此匹配的内缩无需类型化访问即可得到相同区域。通过对 popover **及其**内容控件调用 `Ui::set_visible`（它以宿主作为 `UiHost` 接收端）来显示和隐藏浮出层。

## 用法（XML）

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。它不接受任何控件局部属性，也没有子元素。

```xml
<widget id="flyout" x="16" y="56" w="240" h="120" visible="false"/>
```

卡片在 Rust 中通过 `place_popover` 定位；文档通常只切换 `visible`。其内容控件是带有各自 id 的独立条目。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建该卡片；`240 × 120` DIP。 |
| `.anchor(Rect)` | `Rect` | 无 | 缺口指向的锚点（不移动卡片）。 |
| `.pad(f32)` | `f32` | `12.0` | 卡片边缘与 `content_rect` 之间的内缩，钳制 `>= 0`。 |
| `.corner_radius(f32)` | `f32` | `6.0`（`radius::MEDIUM`） | 卡片圆角，钳制 `>= 0`。 |
| `.dismiss_on_outside(bool)` | `bool` | `true` | 在卡片之外的按下会关闭。 |
| `.arrow(bool)` | `bool` | `true` | 绘制指向锚点的缺口。 |
| `.name(text)` | text | `""` | 卡片组的无障碍名称。 |
| `.description(text)` | text | `""` | 无障碍描述。 |
| `.on_dismiss(handler)` | `Fn() -> M` | 无 | 由外部按下发出的消息。 |
| `.content_rect()` | `Rect` | — | 卡片按 `pad` 内缩后的区域，用于内容定位。 |
| `.set_anchor(Rect)` | `Rect` | — | 运行时锚点更新。 |
| `.set_viewport(Rect)` / `.viewport()` | `Rect` | 上次绘制 | 轻量关闭命中区域所使用的客户端视口。 |
| `.dismiss_on_outside_enabled()` | `bool` | — | 当前关闭模式。 |

`preferred_height` 为 120 DIP，卡片不可聚焦。

### 行为

- **定位。** `place_popover(anchor, size, placement, viewport, gap)` 对 `Below` 将卡片居中放置在锚点下方，对 `Above` 放置在上方，对 `Right`/`Left` 放置在旁边；`gap` 钳制为 `>= 0`。`Auto` 依次尝试下方、上方、右侧、然后左侧，并回退到空间更大的垂直一侧。非空视口会以 8-DIP 边距钳制结果；空视口表示不钳制（初始布局阶段）。
- **缺口。** 当箭头开启、锚点不与卡片重叠且卡片有锚点时，缺口绘制在面向锚点的边缘上。存在缺口时，控件的绘制边界会增大 10 DIP。
- **输入。** 卡片上的按下会被吞掉。开启关闭功能时，客户端视口内的外部按下会发出关闭消息；没有视口时只有卡片本身会被命中。卡片使用箭头光标且不绘制指针状态，因此跨页面移动不会重绘它。
- **无障碍。** 角色为 `Group`，以 `.name` 命名，由 `.description` 描述。

## 事件

### 消息

| 消息 | 发出条件 |
|---|---|
| 传给 `.on_dismiss` 的 `impl Fn() -> M` | 当 `dismiss_on_outside` 开启时，按下落在卡片之外。 |

在卡片上的按下，以及关闭功能关闭时在视口内的按下，都不发出任何消息。Escape 不在此处理；由应用进行路由。

### 通用观察器

此控件不安装任何共享的观察器构建器。请使用 `on_dismiss` 或 `Ui::bind`。

### 控件专属回调

| 回调 | 签名 | 触发条件 |
|---|---|---|
| `.on_dismiss(handler)` | `impl Fn() -> M` | 外部按下请求关闭。 |

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("flyout"), Trigger::Click, |event, ctx| {
    // 返回 Some(Msg::...) 以排队一条消息；调用 ctx.default() 以保留
    // 内置动作，随后会在处理函数之后运行。
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

卡片本身不响应任何触发器：它吞掉其命中区域内的按下，不绘制指针状态，只有外部按下才会产生 `on_dismiss`。

## XML 定义

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。

几何遵循常规的 `<widget>` 规则。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`）适用。卡片在 `explicit_border` 边框上绘制 `palette.panel`，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported`；请改用 Rust 中的 `.corner_radius`。它不声明任何共享内容和子列表。

控件局部属性：无。子元素：无。

## 另见

- `../chrome/menu-button.md`
- `../custom/custom-widget.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
