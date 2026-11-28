# RadioGroup

> 垂直单选组，自行绘制并管理其选项。

## 介绍

`RadioGroup<M>` 是一个控件，绘制一栈单选框行并拥有选择：点击某行、按 Space/Enter，
或使用方向键移动，会选择该选项并发出 `on_select(index)`。各行高度为 `row_height` DIP
（默认 28），`gap` 为 4 DIP。

该组的行为类似原生单选组。上/左向后步进，下/右向前步进，循环回绕并跳过禁用选项；
Home 选择选项 0，End 选择最后一个选项（各自仅在该选项启用且与当前选项不同时生效）。
禁用的选项会被指针、键盘和无障碍树跳过。仅当索引实际变化时才发出选择更改。

无障碍为 `A11yRole::RadioGroup`，每个选项一个子节点（携带 `selected`、`enabled`
和行矩形）。`M` 是 `on_select` 返回的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::RadioGroup;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Mode(usize) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        RadioGroup::new("mode", &["In order", "Shuffle", "By popularity"], 0, Msg::Mode)
            .disable(2),
    );
    ui.set_rect(WidgetId::new("mode"), Rect::from_xywh(16.0, 16.0, 200.0, 96.0));
}
```

默认矩形为 200×96。`preferred_height` 为
`options.len() * (row_height + gap) - gap`，因此调整行尺寸的宿主可以用它代替硬编码的 96。
`Ui::set_selected` 会静默移动选择。

## 用法（XML）

`RadioGroup` 不在标准注册表中。在 Rust 中创建它，并从文档中通过 `id` 引用：

```xml
<widget id="mode" x="16" y="16" w="200" h="96" selected="1"/>
```

`selected` 是唯一支持的共享内容（`content_caps` 公开 `SELECTION`），它会在不发出消息的
情况下移动选择。选项标签是控件拥有的数据，而不是 XML 子元素；此控件没有 `<option>` 列表。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, options, selected, on_select)` | `&'static str, &[&str], usize, impl Fn(usize) -> M + 'static` | — | 创建该组。 |
| `.disable(index)` | `usize` | 无禁用 | 将一个选项标记为禁用（越界会被忽略）。 |

Getter 与 setter：`selected()`、`option_enabled(index)` 和
`set_option_disabled(index, bool)`。观察器钩子（`on_click`、
`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、
`on_focus_gained`、`on_focus_lost`）在内置行为之前运行。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `new`（`on_select`）的 `Fn(usize) -> M` | 选择移动到另一个启用的选项；参数是新索引。 |

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

点击某行会选中它，`HoverEnter`/`HoverLeave` 会高亮某行；方向键/Home/End 移动选择，
但它们不是触发器。`Activate` 会重新选中当前选项，因此不发出任何消息。`ValueChange`
跟随选择变化。

## XML 定义

此控件不由标准注册表创建。在 Rust 中创建它，并从文档中通过 `id` 引用。

- 几何：适用常规 `widget` 规则。给它足以容纳每一行的高度；该组不会将行裁剪到矩形内。
- 共享内容：`selected`（非负索引；越界索引会被控件忽略）。
- 本地属性：无。
- 样式 token：无 `SurfaceStyle`；`radius`、`corners` 和 `background` 不受支持
  （`xml.style.unsupported`，一个警告）。
- 子元素：无。行标签和禁用标志是构造函数数据。

## 另见

- `../index.md`
- `radio-button.md`
- `checkbox.md`
- `../../guides/xml-layout-authoring.md`
