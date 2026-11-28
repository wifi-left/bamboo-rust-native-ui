# RadioButton

> 独立的单选框，在变为选中时报告。

## 介绍

`RadioButton<M>` 绘制一个单选圆点和标签。它仅在从未选中过渡到选中时发出 `on_select()`；
对已选中按钮的点击和激活不做任何事。互斥不是内置的：将同组中的另一个按钮设为未选中是
应用的责任，或者使用 [`RadioGroup`](radio-group.md)，它拥有整个选项列表。

禁用的按钮使用禁用调色板并忽略输入。焦点环贴合圆点而不是整个标签。无障碍为
`A11yRole::RadioItem`，设置了 `selected`，并在禁用时设置 `read_only`。`M` 是
`on_select` 返回的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::RadioButton;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { LocalOnly }

fn build(ui: &mut Ui<Msg>) {
    ui.add(RadioButton::new("local", "This device only", false, || Msg::LocalOnly));
    ui.set_rect(WidgetId::new("local"), Rect::from_xywh(16.0, 16.0, 180.0, 28.0));
}
```

对于真正的组，请优先使用 `RadioGroup`：它会绘制选项并为你移动选择。如果保留独立按钮，
应用在处理消息时必须在先前选中的按钮上调用 `set_checked(false)`。

## 用法（XML）

`RadioButton` 不在标准注册表中。在 Rust 中创建它，并从文档中通过 `id` 引用：

```xml
<widget id="local" x="16" y="16" w="180" h="28" label="This device only"/>
```

只有 `label` 被支持为共享内容；单选框的 `content_caps` 公开 `LABEL`，但不公开
`TOGGLED` 或 `SELECTION`，因此不会从文档写入 `checked`/`selected`。没有本地属性或子元素。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, label, selected, on_select)` | `&'static str, impl Into<String>, bool, impl Fn() -> M + 'static` | — | 创建单选框。 |
| `.disabled(bool)` | `bool` | `false` | 使按钮变灰并忽略输入。 |

Getter 与 setter：`selected()` 和 `set_checked(bool)`（静默；通常由组的所有者清除）。
观察器钩子（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、
`on_key_up`、`on_focus_gained`、`on_focus_lost`）在内置行为之前运行。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `new`（`on_select`）的 `Fn() -> M` | 按钮从未选中过渡到选中；点击已选中的按钮不会发出任何消息。 |

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

无。构造函数的 `on_select` 消息回调是唯一的控件回调。

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

按钮对 `Click` 和 `Activate` 做出反应（当前未选中时进行选中）；`ValueChange` 跟随选择变化。
其余触发器不予处理。

## XML 定义

此控件不由标准注册表创建。在 Rust 中创建它，并从文档中通过 `id` 引用。

- 几何：适用常规 `widget` 规则。
- 共享内容：`label`。
- 本地属性：无。
- 样式 token：无 `SurfaceStyle`；`radius`、`corners` 和 `background` 不受支持
  （`xml.style.unsupported`，一个警告）。
- 子元素：无。

## 另见

- `../index.md`
- `radio-group.md`
- `checkbox.md`
- `../../guides/xml-layout-authoring.md`
