# Tabs

> 一条标签页条带，一次选中一个页面，可选带徽标和关闭目标。

## 介绍

`Tabs<M>` 在一条 1 DIP 的基线上绘制一行等宽的标签单元格。选中的标签获得一条强调色下划线，每侧内缩 8 DIP；悬停的标签获得 `row_hover`。每个标签可以携带一个右对齐的徽标（一个计数或一段短标签），并且在设置了 `closable` 时，还会携带一个小的 `×` 关闭目标。

选中时通过 `on_select` 上报标签索引。关闭时通过单独的 `on_close` 处理函数上报，并且**不会**选中该标签。控件拥有选中的索引和标签/徽标列表；徽标可以在运行时更新，用于实时的通知计数。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::Tabs;
use bamboo_native_ui::{Rect, TextAlign, Ui, WidgetId};

enum Msg {
    Tab(usize),
    CloseTab(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Tabs::new("pages", &["Now playing", "Queue", "History"], 0, Msg::Tab)
        .badge(1, "12")
        .closable(Msg::CloseTab)
        .label_align(TextAlign::Left),
);
ui.set_rect(WidgetId::new("pages"), Rect::from_xywh(16.0, 16.0, 360.0, 32.0));
```

`on_select` 和 `on_close` 都接受 `Fn(usize) -> M`。`label_align` 接受 `TextAlign`（`Left`、`Center`、`Right`）；徽标或关闭目标占据右边缘时，居中的标签仍保持居中，而左/右对齐的标签会停在该装饰元素之前。控件不会移除已关闭的标签——应用更新其状态并调用 `set_labels`（或由文档来完成）。

## 用法（XML）

`Tabs` 不由标准注册表创建。在 Rust 中创建它，并通过 `id` 定位；文档提供标签、初始选中项和标签对齐方式：

```xml
<widget id="pages" x="16" y="16" w="360" h="32"
        selected="1" label-align="left">
  <option label="Now playing"/>
  <option label="Queue"/>
  <option label="History"/>
</widget>
```

`<option>` 要求一个非空的 `label`，不接受其他属性，也没有子元素。徽标和 `closable` 由应用拥有：文件表达的是标签，而不是编辑器状态。`selected` 在标签之后应用；徽标对保留的标签保持其位置。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, labels, selected, on_select)` | `&'static str, &[&'static str], usize, impl Fn(usize) -> M` | — | 创建该条带；`360 × 32` DIP，不可关闭，无徽标，标签居中。 |
| `.closable(on_close)` | `impl Fn(usize) -> M` | 不可关闭 | 在每个标签上绘制一个关闭目标。 |
| `.badge(index, text)` | `usize, impl Into<String>` | 无 | 为一个标签附加徽标。 |
| `.set_badge(index, Option<String>)` | `usize, Option<String>` | — | 在运行时设置或清除徽标；返回是否发生了更改。 |
| `.badge_text(index)` | — | — | 徽标文本，忽略空字符串。 |
| `.is_closable()` | — | — | 是否绘制关闭目标。 |
| `.label_align(TextAlign)` | `TextAlign` | `Center` | 每个单元格内的标签对齐方式。 |
| `.label_alignment()` | — | — | 当前对齐方式。 |
| `.set_label_align(TextAlign)` | `TextAlign` | — | 运行时更改对齐方式；返回是否发生了更改。 |
| `.selected()` | — | — | 选中的索引。 |
| `.labels()` | — | — | 标签页标签。 |
| `.set_labels(Vec<String>)` | `Vec<String>` | — | 替换标签，调整徽标大小，钳制选中项；返回是否发生了更改。 |
| `.select(usize)` | `usize` | — | 通过常规路径选中；未改变或越界时返回 `None`。 |
| `.hovered()` | — | — | 悬停的标签索引，如果有的话。 |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | 无 | 附加型点击观察器；旁边还有右键/双击、按键和焦点观察器。 |

### 行为

- **指针。** 点击会选中其所在标签并通过 `on_select` 上报。点击落在 12 × 12 的关闭目标内会改为上报 `on_close(index)`，并保持选中项不变。
- **键盘。** 左/上 和 右/下 带环绕地逐项移动，Home/End 跳转到第一个/最后一个标签，Enter/Space 重新上报选中的索引，`Delete` 关闭选中的标签（仅在可关闭时）。
- **徽标。** 作为强调色标签绘制在右边缘；标签矩形相应收缩以腾出空间。徽标文本是应用数据，可以在每一帧变化而无需重建控件。
- **无障碍性。** 角色为 `tab-list`，每个标签一个子元素，携带标签、边界和选中状态。

## 事件

### 消息

| 消息 | 触发条件 |
|---|---|
| 作为 `on_select` 传给 `Tabs::new` 的 `Fn(usize) -> M` | 某个标签被点击，键盘移动了选中项（方向键、Home/End），或 `Enter`/`Space` 重新上报选中的标签。 |

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

| 构建器 | 处理函数 | 触发条件 |
|---|---|---|
| `.closable(on_close)` | `Fn(usize) -> M` | 点击落在标签的 `×` 目标上，或在选中的标签上按下 `Delete`。该标签不会被选中。 |

`badge`/`set_badge` 仅更改绘制，不上报任何内容。

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("pages"), Trigger::Click, |event, ctx| {
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

条带响应 `Click`（包括关闭目标）、`Activate`、方向键/Home/End 键和 `Delete`，以及 `HoverEnter`/`HoverLeave` 以设置每个标签的悬停；它忽略 `Wheel`、`DragStart`/`DragEnd` 和 `ValueChange`。

## XML 定义

- **创建：** 无 `type`；该控件仅可被定位。在应用文档之前，应用必须已添加 `Tabs` 实例。
- **几何：** 一种水平形式（`x`+`w`、`right`+`w`、`x`+`right`）和一种垂直形式（`y`+`h`、`bottom`+`h`、`y`+`bottom`）。
- **内容：** `selected`；外加局部 `label-align` 属性，其值为 `left`、`center` 或 `right`（其他任何值会报 `xml.skin.value`）。
- **样式：** `font-size` 和 `font-family`。Tabs 没有表面，因此 `radius`、`corners` 和 `background` 会报 `xml.style.unsupported` 并被跳过。
- **属性：** 通用运行时集合（`enabled`、`visible`、`tooltip`、`name`、……）。`tooltip-at` 风格的每个标签提示通过 Rust 中的 `WidgetProps` 设置，而不是在文档中。
- **子元素：** 仅 `<option label="…"/>` 元素。

## 另见

- `./segmented.md`
- `./pager.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
