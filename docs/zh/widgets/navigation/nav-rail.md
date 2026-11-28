# NavRail

> 一个垂直侧栏，包含带可选图标的单选导航行。

## 介绍

`NavRail<M>` 绘制一个垂直的导航行列表。恰好一行处于选中状态；它会获得激活填充和一条 3 DIP 的强调色条，其标签和图标使用强调色文本色。点击另一行，或在调整模式下用方向键移动，会改变选择，并通过 `on_select` 报告行索引。

行来自 `NavItem` 值：一个标签加一个可选的单字符图标。侧栏在自身矩形内绘制且不滚动；请按行数调整其大小（`rows * row_height`），或有意进行裁剪。

## 用法（Rust）

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::{NavItem, NavRail};

enum Msg {
    Navigate(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    NavRail::new("sidebar", &["All tracks", "Recent", "Favorites"], 1, Msg::Navigate)
        .icon(0, '\u{E8FD}')
        .icon(1, '\u{E81C}')
        .icon(2, '\u{E734}')
        .row_height(34.0)
        .label("Library"),
);
ui.set_rect(WidgetId::new("sidebar"), Rect::from_xywh(16.0, 16.0, 180.0, 102.0));
```

`new` 接收标签、初始选择（带限制）以及回调。`icon(index, char)` 为一个行添加装饰；私用区码点使用字形字体绘制，其他任何字符使用 UI 字体绘制。在运行时用 `Vec<NavItem>` 替换行：

```rust
ui.edit_widget(WidgetId::new("sidebar"), |rail: &mut NavRail<Msg>| {
    rail.set_items(vec![NavItem::new("Queue").icon('\u{E8FD}')])
});
```

## 用法（XML）

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。`<step>` 子元素会替换行；共享的 `selected` 属性选择行：

```xml
<widget id="sidebar" x="16" y="16" w="180" h="102"
        row-height="34" selected="1" name="Library">
  <step label="All tracks" icon="&#xE8FD;"/>
  <step label="Recent" icon="&#xE81C;"/>
  <step label="Favorites"/>
</widget>
```

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, labels, selected, on_select)` | `&'static str, &[&str], usize, impl Fn(usize) -> M` | — | 为每个标签创建一行；选择会被限制。 |
| `.icon(index, char)` | `usize, char` | none | 某一行的前导图标。 |
| `.row_height(f32)` | `f32` | `34.0` | 行高（DIP），限制为 `>= 18`。 |
| `.label(impl Into<String>)` | text | widget id | 侧栏的无障碍名称。 |
| `.disabled(bool)` | `bool` | `false` | 绘制为禁用并忽略输入。 |
| `.len()` / `.is_empty()` | `usize` / `bool` | — | 行数。 |
| `.selected()` | `usize` | constructor value | 选中的行。 |
| `.set_selected(usize)` | silent | — | 在范围内选择一行而不发出消息。 |
| `.items()` | `&[NavItem]` | — | 行模型。 |
| `.set_items(Vec<NavItem>)` | `bool` | — | 静默替换行；限制选择并清除悬停。 |
| `.select(usize)` | `Option<M>` | — | 通过常规路径选择；当前行和越界索引返回 `None`。 |
| `.set_row_height(f32)` / `.row_height_value()` | `bool` / `f32` | — | 运行时行高（`>= 18`）。 |

`NavItem::new(label)` 创建一行；`.icon(char)` 添加一个前导字形，`label()` / `icon_glyph()` 将其读回。

行为：点击已选中的行不发出任何内容。在调整模式下，Up/Left 和 Down/Right 循环步进，Home/End 选择第一个/最后一个行，Enter/空格选择当前行（不发出任何内容）。首选高度为 `rows * row_height`。无障碍角色为 `List`，选中的行被标记为选中。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 作为 `on_select` 传给 `new` 的 `Fn(usize) -> M` | 另一行通过点击或方向键/Home/End 导航变为选中；参数为行索引。点击已选中的行不发出任何内容。 |

`set_selected`、`set_items` 以及 XML `selected` 属性都是静默的。

### 通用观察器

该控件不安装共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。构造函数的 `on_select` 就是它的事件接缝；其他任何内容都通过 `Ui::bind` 处理。

### 控件专属回调

| 方法 | 处理器 | 触发时机 |
|---|---|---|
| `new(id, labels, selected, on_select)` | `&'static str, &[&str], usize, impl Fn(usize) -> M + 'static` | 存储选择回调；它是该控件声明的唯一回调。 |
| `.select(index)` | 返回 `Option<M>` | 运行时形式：正常选择并返回消息，或在当前行、越界索引或禁用侧栏时返回 `None`。 |

### Ui 触发器

任何控件 id 都可以通过 `Ui::bind` 进行绑定：

```rust
ui.bind(WidgetId::new("sidebar"), Trigger::ValueChange, |event, ctx| {
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

它响应 `Click`、`Activate`、`HoverEnter`/`HoverLeave` 以及 `ValueChange`（选中行变化）。该侧栏不滚动，因此 `Wheel` 不产生任何操作，`RightClick` 以及拖动触发器也没有内置效果。

## XML 定义

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。

几何布局遵循常规的 `<widget>` 规则。通用属性适用，包括 `selected`（行索引，在子元素列表之后应用）、`enabled` 和 `name`。`font-size` 和 `font-family` 属于绘制层。侧栏从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported` 并跳过。

局部属性：

| 属性 | 类型 | 描述 |
|---|---|---|
| `row-height` | number | 行高；必须 `>= 18`。 |

子元素：

| 元素 | 属性 | 描述 |
|---|---|---|
| `<step>` | `label`（必需，非空）、`icon`（单个字符） | 一行；列表会被替换。 |

子元素列表属于内容：仅在文档生成变化时写入，与变体无关且静默。

## 另见

- `breadcrumb.md`
- `accordion.md`
- `../index.md`
- `../../api/widget-trait.md`
