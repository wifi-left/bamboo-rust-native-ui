# Accordion

> 堆叠的可折叠分区，可有一个或多个正文处于打开状态。

## 介绍

`Accordion<M>` 绘制垂直的分区表头堆栈，每个打开的表头下方附带一个可选的包裹正文。点击表头，或在键盘光标上按下 Enter 或空格，会切换它，并通过 `on_toggle` 闭包报告 `(section, open)`。分区默认相互独立；`single_open` 会关闭先前打开的分区。

该控件只在自身矩形内绘制正文文本这一点上算是容器；它不拥有子控件。应用程序根据打开状态调整矩形大小，垂直布局时使用 `preferred_height`。每个分区都暴露展开/折叠状态，打开的分区在无障碍树中被选中。

`Collapsible<M>` 是 `Accordion<M>` 的类型别名；用一个标题构建它。

## 用法（Rust）

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::Accordion;

enum Msg {
    Toggle(usize, bool),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Accordion::new("settings", &["Playback", "Audio"], Msg::Toggle)
        .body(0, "Resume where you left off and keep the queue.")
        .body(1, "Equalizer presets are applied to every output.")
        .open(0, true)
        .single_open(true)
        .header_height(34.0)
        .body_height(56.0)
        .label("Playback settings"),
);
ui.set_rect(WidgetId::new("settings"), Rect::from_xywh(16.0, 16.0, 300.0, 124.0));
```

`new` 固定分区数量；正文初始为空且处于关闭状态。`body` 设置文本，`open` 设置初始状态而不发出消息，`single_open` 强制最多只有一个打开的分区。单分区的 `Collapsible` 就是 `Accordion::new(id, &["Title"], handler)`。

## 用法（XML）

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。`open` 局部属性是一个分区索引，只打开该分区：

```xml
<widget id="settings" x="16" y="16" w="300" h="124" open="0" name="Playback settings"/>
```

分区标题、正文文本和切换回调属于 Rust 侧状态。没有子元素：文档无法添加或重命名分区标题。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, titles, on_toggle)` | `&'static str, &[&str], impl Fn(usize, bool) -> M` | — | 为每个标题创建一个关闭且为空的分区。 |
| `.body(index, text)` | `usize, impl Into<String>` | `""` | 某个分区的正文文本（打开时自动换行）。 |
| `.open(index, bool)` | `usize, bool` | `false` | 初始打开状态；静默。 |
| `.single_open(bool)` | `bool` | `false` | 一次只打开一个分区。 |
| `.header_height(f32)` | `f32` | `34.0` | 表头行高，限制为 `>= 16`。 |
| `.body_height(f32)` | `f32` | `56.0` | 打开分区的正文区域，`>= 0`。 |
| `.chevrons(collapsed, expanded)` | `char, char` | MDL2 `U+E70D`、`U+E70E` | 表头标记。 |
| `.label(impl Into<String>)` | text | widget id | 容器的无障碍名称。 |
| `.disabled(bool)` | `bool` | `false` | 绘制为禁用并忽略输入。 |
| `.len()` / `.is_empty()` | `usize` / `bool` | — | 分区数量。 |
| `.is_expanded(index)` | `bool` | — | 某个分区的状态。 |
| `.open_index()` | `Option<usize>` | — | 第一个打开的分区。 |
| `.set_expanded(index, bool)` | `bool` | — | 静默状态变更；在单开模式下会关闭其他分区。返回是否有任何变化。 |
| `.set_body(index, text)` / `.set_title(index, text)` | silent setters | — | 运行时替换正文/标题。 |
| `.toggle(index)` | `Option<M>` | — | 通过常规路径切换并返回消息。 |

行为：首选高度为 `headers * header_height + open bodies * body_height`，且绘制被裁剪到控件矩形内，因此某个分区打开时应用程序必须扩大矩形。调整模式下的键盘输入用 Up/Left 和 Down/Right 移动光标（循环），Home/End 跳转，Enter/空格切换。点击打开的正文内部不产生任何操作。无障碍角色为 `List`：每个分区是一个带 `expanded`/`selected` 状态的项，UIA 展开/折叠请求会驱动与点击相同的路径。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 作为 `on_toggle` 传给 `new` 的 `Fn(usize, bool) -> M` | 某个分区被切换：`(section_index, open)`。在表头点击、光标上的 Enter/空格以及 UIA 展开/折叠请求时触发。 |

`.open(index, bool)` 和 `set_expanded` 是静默的，从不触发它。

### 通用观察器

该控件不安装共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。构造函数的 `on_toggle` 就是它的事件接缝；其他任何内容都通过 `Ui::bind` 处理。

### 控件专属回调

| 方法 | 处理器 | 触发时机 |
|---|---|---|
| `new(id, titles, on_toggle)` | `&'static str, &[&str], impl Fn(usize, bool) -> M + 'static` | 存储切换回调；它是该控件声明的唯一回调。 |
| `.toggle(index)` | 返回 `Option<M>` | 运行时形式：正常切换并返回消息，或在禁用或索引越界时返回 `None`。 |

### Ui 触发器

任何控件 id 都可以通过 `Ui::bind` 进行绑定：

```rust
ui.bind(WidgetId::new("settings"), Trigger::Activate, |event, ctx| {
    // 返回 Some(Msg::...) 以入队一条消息；除非显式调用 ctx.run_default()，
    // 内置操作仍会执行。
    None
});
```

| 触发器 | 触发时机 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告双击。 |
| `RightClick` | 次级按钮点击，在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开该控件。 |
| `Wheel` | 控件上的滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 聚焦控件上按下 Enter/空格，或 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 释放按钮。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

它响应 `Click`（表头切换）、`Activate`、`HoverEnter`/`HoverLeave` 以及 `ValueChange`（某分区打开或关闭）；点击打开的正文内部不产生任何操作。`Wheel`、`RightClick` 以及拖动触发器没有内置效果。

## XML 定义

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。

几何布局遵循常规的 `<widget>` 规则。通用属性适用，包括 `enabled` 和 `name`。`font-size` 和 `font-family` 属于绘制层。手风琴从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported` 并跳过。

局部属性：

| 属性 | 类型 | 描述 |
|---|---|---|
| `open` | number | 要打开的分区索引；其他每个分区都会关闭。必须 `>= 0`；越界的索引会关闭所有分区。 |

子元素：无。标题向量在构造时固定，文档无法添加、移除或重命名分区标题。共享的 `selected` 属性会被接受（它移动键盘光标），但不会打开分区；请使用 `open`。

## 另见

- `nav-rail.md`
- `breadcrumb.md`
- `../index.md`
- `../../api/widget-trait.md`
