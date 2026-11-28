# Checkbox

> 带标签的开关框，会报告新的勾选状态。

## 介绍

`Checkbox<M>` 绘制一个 18 DIP 的框及其右侧的标签。点击控件或激活它会切换该框，
并发出 `on_change(new_state)`。勾选标记是一个真正的字形：默认是 MDL2 对勾 `U+E73E`，
使用字形字体绘制，也可以用 `.check(char)` 指定其他字符（私有使用区字符使用字形字体，
其他字符使用 UI 字体）。

该框默认是圆角的（`corner_radius` 为 4 DIP），`.square()` 会使其变为直角。绘制时半径会被
钳制为框的一半。禁用的复选框使用禁用调色板颜色并忽略输入。

无障碍：`A11yRole::Toggle`，名称来自标签，设置了 `checked`，并在禁用时设置
`read_only`。实例默认可聚焦。`M` 是 `on_change` 返回的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::Checkbox;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Shuffle(bool) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(Checkbox::new("shuffle", "Shuffle", false, Msg::Shuffle));
    ui.set_rect(WidgetId::new("shuffle"), Rect::from_xywh(16.0, 16.0, 180.0, 28.0));

    ui.add(Checkbox::new("square", "Square box", false, Msg::Shuffle).square());
    ui.set_rect(WidgetId::new("square"), Rect::from_xywh(16.0, 52.0, 180.0, 28.0));
}
```

构造函数接收初始状态。此后值由应用拥有：将其保存在模型中并重建，或者在处理消息后通过
`Ui::with_widget` 读取 `checked()`。`Ui::set_toggle` 会静默更新状态。

## 用法（XML）

`Checkbox` 不在标准注册表中。在 Rust 中创建它，并从文档中通过 `id` 引用：

```xml
<widget id="shuffle" x="16" y="16" w="180" h="28"
        label="Shuffle" checked="true"/>
```

`label` 和 `checked` 是支持的共享内容（`content_caps` 公开 `LABEL` 和 `TOGGLED`）。
没有本地属性或子元素。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, label, checked, on_change)` | `&'static str, impl Into<String>, bool, impl Fn(bool) -> M + 'static` | — | 创建复选框。 |
| `.check(char)` | `char` | `'\u{E73E}'` | 勾选状态的标记字形。 |
| `.square()` | — | 圆角 | 直角框角（半径为 0）。 |
| `.corner_radius(f32)` | `f32` | `4.0`（主题 `radius::SMALL`） | 外框半径，钳制为框的一半。 |
| `.disabled(bool)` | `bool` | `false` | 使控件变灰并忽略输入。 |

Getter 与 setter：`checked()`、`check_glyph()`、`corner_radius_value()` 和
`set_corner_radius(f32)`。观察器钩子（`on_click`、`on_right_click`、
`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、
`on_focus_lost`）在内置行为之前运行，并可返回消息。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `new` 的 `Fn(bool) -> M` | 框被切换；参数是新的勾选状态。 |

### 通用观察器

此控件安装共享观察器构建器。它们是累加的：处理器返回后，内置行为仍会运行。

| 构建器 | 处理器 | 触发条件 |
|---|---|---|
| `.on_click(f)` | `Fn(Point) -> Option<M>` | 在控件上左键点击。 |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | 右键点击，发生在上下文菜单之前。 |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | 平台双击。 |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | 控件拥有焦点时按下按键。 |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | 控件拥有焦点时释放按键。 |
| `.on_focus_gained(f)` | `Fn(FocusEvent) -> Option<M>` | 焦点移动到控件上。 |
| `.on_focus_lost(f)` | `Fn(FocusEvent) -> Option<M>` | 焦点离开控件（在编辑器提交之后）。 |

### 控件专属回调

无。构造函数的 `on_change` 消息回调是唯一的控件回调；上面的观察器是扩展点。

### Ui 触发器

任意控件 id 都可以用 `Ui::bind(id, Trigger, handler)`（或 `Ui::on_click` /
`Ui::on_activate` 简写）进行绑定。返回 `Some(M)` 的处理器会将消息入队；除非处理器调用
`ctx.run_default()`，否则内置操作仍会执行。

| 触发器 | 触发条件 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告双击。 |
| `RightClick` | 次按键点击，发生在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开控件。 |
| `Wheel` | 控件上发生滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 焦点控件上按 Enter/Space，或 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 释放按键。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

复选框对 `Click` 和 `Activate`（两者都切换）做出反应；`ValueChange` 跟随勾选状态变化。
悬停、滚轮和拖动触发器不予处理。

## XML 定义

此控件不由标准注册表创建。在 Rust 中创建它，并从文档中通过 `id` 引用。

- 几何：适用常规 `widget` 规则。
- 共享内容：`label` 和 `checked`。
- 本地属性：无。`skin_attrs` 为空，因此自定义名称会被拒绝。
- 样式 token：无 `SurfaceStyle`；`radius`、`corners` 和 `background` 不受支持
  （`xml.style.unsupported`，一个警告）。
- 子元素：无。勾选字形和禁用标志只能在构造函数中设置。

## 另见

- `../index.md`
- `radio-button.md`
- `radio-group.md`
- `../controls/toggle.md`
- `../../guides/xml-layout-authoring.md`
