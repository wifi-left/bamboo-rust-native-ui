# ComboBox

> 下拉列表，其展开的选项绘制在同级元素之上。

## 介绍

`ComboBox<M>` 绘制一个显示选中项和尖角符号的关闭框。点击该框（或在关闭时按下/上方向键）
会在其下方打开一个列表；点击某行会选中它、关闭列表并发出 `on_select(index)`。
打开时点击框会关闭列表而不选择，Escape 会关闭列表。

展开的列表是覆盖层，由控件自身绘制在同级元素之上并优先进行命中测试。它最多显示
`max_visible` 行（默认 8），并可通过滚轮、跟随键盘高亮或拖动其主题滚动条来滚动。
导航键：下/上空移动高亮（若已关闭则先打开），Home/End 跳转，PageUp/PageDown 翻页移动，
Enter/Space 选择，Escape 关闭。如果列表会超出视口且有空间，它会自动翻转到框的上方；
`.drop_up(true)` 会强制该方向。

无障碍为 `A11yRole::ComboBox`，其值为选中文本，其名称为 `label`（回退到值）。
`M` 是 `on_select` 返回的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::ComboBox;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Mode(usize) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        ComboBox::new("mode", &["In order", "Shuffle", "Repeat one"], 0, Msg::Mode)
            .label("Playback mode")
            .max_visible(6),
    );
    ui.set_rect(WidgetId::new("mode"), Rect::from_xywh(16.0, 16.0, 200.0, 32.0));
}
```

构造函数会将 `selected` 钳制到项目列表中。`selected_text()` 返回可见值；
`set_selected(index)` 和 `set_items(Vec<String>)` 在不发出消息的情况下更新控件，
用于模型同步。

## 用法（XML）

`ComboBox` 不在标准注册表中。在 Rust 中创建它，并从文档中通过 `id` 引用。
`<option>` 子元素列表会替换项目：

```xml
<widget id="mode" x="16" y="16" w="200" h="32" selected="0">
  <option label="In order"/>
  <option label="Shuffle"/>
  <option label="Repeat one"/>
</widget>
```

`selected` 是唯一共享内容（`content_caps` 公开 `SELECTION`），并静默应用。
每个 `<option>` 需要一个非空 `label`，不接受其他属性或子元素。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, items, selected, on_select)` | `&'static str, &[&str], usize, impl Fn(usize) -> M + 'static` | — | 创建该框。 |
| `.label(impl Into<String>)` | `impl Into<String>` | `""` | 列表的无障碍名称。 |
| `.max_visible(usize)` | `usize` | `8` | 列表滚动前显示的行数（最小 1）。 |
| `.row_height(f32)` | `f32` | `26.0` | 选项行高度，单位为 DIP（最小 16）。 |
| `.drop_up(bool)` | `bool` | `false` | 始终在框的上方打开列表。 |
| `.arrow(closed, open)` | `char, char` | `'▾', '▴'` | 两种状态下的尖角字符。 |
| `.arrow_closed(char)` | `char` | `'▾'` | 仅关闭状态的字符。 |
| `.scrollbar_style(ScrollBarStyle)` | `ScrollBarStyle` | 6 DIP 轨道，最小 12 DIP 滑块 | 展开列表滚动条的粗细和颜色。 |
| `.disabled(bool)` | `bool` | `false` | 使框变灰并忽略输入。 |

Getter 与 setter：`selected()`、`selected_text()`、`items()`、`is_open()`、
`arrow_chars()`、`scrollbar()`、`set_selected(index)` 和
`set_items(Vec<String>)`（后两者均静默）。观察器钩子（`on_click`、
`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、
`on_focus_gained`、`on_focus_lost`）在内置行为之前运行。

行为说明：

- 打开时 `overlay()` 为 true，且 `paint_bounds()` 覆盖展开列表，以使无效化到达它。
- 只有展开的列表会消费滚轮刻度；关闭的框会将它们留给周围的滚动视图。
- 私有使用区的箭头字符使用字形字体绘制；文本字符使用 UI 字体。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `new`（`on_select`）的 `Fn(usize) -> M` | 通过点击或在高亮行上按 Enter/Space 选择选项；参数是新索引。 |

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

该框对 `Click`（打开/关闭、选择某行）和 `Activate`（打开，或选择高亮行）做出反应。
`Wheel` 仅在列表展开时滚动；`DragStart`/`DragEnd` 驱动展开列表的滚动条。
`ValueChange` 跟随选择变化。

## XML 定义

此控件不由标准注册表创建。在 Rust 中创建它，并从文档中通过 `id` 引用。

- 几何：适用常规 `widget` 规则；矩形仅是关闭的框（列表在其外部绘制）。
- 共享内容：`selected`。
- 本地属性：无。
- 样式 token：无 `SurfaceStyle`；`radius`、`corners` 和 `background` 不受支持
  （`xml.style.unsupported`，一个警告）。
- 子元素：`<option>`，带必填的 `label`；列表会替换控件的项目。其他名称会报
  `xml.skin.unknown`。

## 另见

- `../index.md`
- `multi-select-list.md`
- `../controls/button.md`
- `../../guides/xml-layout-authoring.md`
