# Segmented

> 一条由互斥选项组成的水平条带，类似以按钮形式绘制的单选组。

## 介绍

`Segmented<M>` 将其矩形划分为相等的单元格，用分隔线颜色为每个单元格描边，并用 `button_active` 填充选中的单元格。悬停在启用的单元格上时会将其绘制为 `row_hover`。选项可以逐个禁用；禁用的单元格绘制为 `button_disabled`，忽略指针输入，并在键盘导航中被跳过。

该控件行为类似单选组：点击某个单元格会选中它，并通过 `on_select` 上报其索引。`M` 是应用消息；控件拥有选中的索引，因此键盘导航与绘制保持协调一致。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::Segmented;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Filter(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Segmented::new("filter", &["All", "Recent", "Favorites"], 0, Msg::Filter)
        .disable(2),
);
ui.set_rect(WidgetId::new("filter"), Rect::from_xywh(16.0, 16.0, 300.0, 32.0));
```

`on_select` 是 `Fn(usize) -> M`；`disable(index)` 是 `set_option_disabled` 的构建器形式。第三个参数是初始选中的索引。选项标签在构造时为 `&'static str`；之后可以用 `set_options(Vec<String>)` 更改它们。

## 用法（XML）

`Segmented` 不由标准注册表创建。在 Rust 中创建它，并从文档中通过 `id` 定位：

```xml
<widget id="filter" x="16" y="16" w="300" h="32" selected="1">
  <option label="All"/>
  <option label="Recent"/>
  <option label="Favorites"/>
</widget>
```

一个 `<option>` 必须具有非空的 `label`，没有其他属性，也没有子元素；文档生成变化时，该列表会替换选项标签。每个选项的禁用标志由应用拥有，无法在此表达。`selected` 在标签之后应用。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, options, selected, on_select)` | `&'static str, &[&'static str], usize, impl Fn(usize) -> M` | — | 创建该条带；`300 × 32` DIP，所有选项均启用。 |
| `.disable(index)` | `usize` | — | 禁用一个选项（构建器形式）。 |
| `.set_option_disabled(index, disabled)` | `usize, bool` | — | 运行时按选项设置状态；返回是否发生了更改。 |
| `.option_enabled(index)` | `usize` | — | 该选项是否可被选中。 |
| `.selected()` | — | — | 选中的索引。 |
| `.options()` | — | — | 选项标签。 |
| `.set_options(Vec<String>)` | `Vec<String>` | — | 替换标签；调整禁用标志的大小，钳制选中项，清除悬停。返回是否发生了更改。 |
| `.select(usize)` | `usize` | — | 通过常规路径选中；对禁用或越界的索引返回 `None`。 |
| `.hovered()` | — | — | 悬停的单元格索引，如果有的话。 |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | 无 | 附加型点击观察器。 |

### 行为

- **键盘。** 左/上后退一步，右/下前进一步，两者都带环绕并跳过禁用的选项。Home/End 跳转到第一个/最后一个启用的选项。当前索引启用时，Enter/Space（`Activate`）会重新上报该索引。
- **指针。** 点击禁用的单元格不上报任何内容并保持选中项；悬停永远不会落在禁用的单元格上。
- **绘制。** 每个单元格宽度为 `width / count`，末端距离下一个单元格差 1 DIP，并用分隔线颜色描边；选中的标签使用 `accent_text`，其他标签使用 `muted`，禁用的标签使用 `text_disabled`。
- **无障碍性。** 角色为 `radio-group`，其子元素携带每个选项的名称、边界、选中状态和启用状态。

## 事件

### 消息

| 消息 | 触发条件 |
|---|---|
| 作为 `on_select` 传给 `Segmented::new` 的 `Fn(usize) -> M` | 某个单元格被点击，键盘移动了选中项（方向键、Home/End），或 `Enter`/`Space` 重新上报已启用的选中项。参数是新索引。 |

禁用的选项从不上报，重新选中当前索引也不上报任何内容。

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

无。`on_select` 是唯一的回调；`select(index)` 和 `set_option_disabled` 是状态接缝，会在适用时通过它上报。

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("filter"), Trigger::Click, |event, ctx| {
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

条带响应 `Click`、`Activate` 以及方向键/Home/End 键，并响应 `HoverEnter`/`HoverLeave` 以设置每个单元格的悬停；它忽略 `Wheel`、`DragStart`/`DragEnd` 和 `ValueChange`。

## XML 定义

- **创建：** 无 `type`；该控件仅可被定位。文档的几何/内容处理针对应用已添加的实例运行。
- **几何：** `x`+`w`、`right`+`w`、`x`+`right` 之一，外加一种垂直形式；尺寸为零会给出警告。
- **内容：** `selected`（非负整数）。`label`/`text` 会被拒绝。
- **样式：** `font-size` 和 `font-family` 生效。Segmented 不采用表面，因此 `radius`、`corners` 和 `background` 会以 `xml.style.unsupported` 上报并被跳过。
- **属性：** 通用运行时集合（`enabled`、`visible`、`tooltip`、`name`、`focus-order`、……）。
- **子元素：** 仅 `<option label="…"/>` 元素，如上所示。

## 另见

- `./tabs.md`
- `./button.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
