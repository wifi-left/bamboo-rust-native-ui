# Table

> 虚拟化表格，带有可排序、可调整大小、可重排的列和就地单元格编辑。

## 介绍

`Table<M>` 在虚拟化主体上方绘制一个固定表头带。列由几何信息外加标题和对齐方式构成；行是单元格字符串的列表，只有与主体相交的行才被绘制、命中测试或发布到无障碍功能。排序由应用程序负责：点击表头会报告其列索引，应用程序重新排序并调用 `set_sort`，后者绘制指示标记。分隔条可调整固定列的大小，表头拖动可重排某一列，可编辑表格可就地编辑单元格。

## 用法（Rust）

```rust
use bamboo_native_ui::{Rect, TextAlign, Ui, WidgetId};
use bamboo_native_ui::widgets::{Table, TableColumn, TableRow};

enum Msg {
    Select(usize),
    Open(usize),
    Sort(usize),
    Edit(usize, usize, String),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Table::new("tracks")
        .columns(vec![
            TableColumn::new("#", 56.0).align(TextAlign::Right),
            TableColumn::flex("Title"),
            TableColumn::new("Length", 72.0).align(TextAlign::Right),
        ])
        .rows(vec![
            TableRow::new(["1", "Nocturne", "3:58"]),
            TableRow::new(["2", "Sunny Day", "4:29"]).disabled(true),
        ])
        .striped(true)
        .editable(true)
        .on_select(Msg::Select)
        .on_activate(Msg::Open)
        .on_header_click(Msg::Sort)
        .on_cell_edit(Msg::Edit),
);
ui.set_rect(WidgetId::new("tracks"), Rect::from_xywh(16.0, 56.0, 520.0, 300.0));
```

点击表头后，重新排序并绘制指示标记；调整后的宽度或移动后的列在其回调运行时已应用。

```rust
// 在 Msg::Sort(column) 分支中，对应用程序的行排序之后：
ui.edit_widget(WidgetId::new("tracks"), |table: &mut Table<Msg>| {
    table.set_rows(sorted);
    table.set_sort(Some((column, ascending)))
});
```

`on_cell_edit(row, column, text)` 在草稿被应用后触发；请持久化该值。

## 用法（XML）

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。表格的子元素列表就是完整数据：`<column>` 元素替换列，`<row>` 元素替换行。

```xml
<widget id="tracks" x="16" y="56" w="520" h="300"
        header-height="30" row-height="32" cell-padding="10"
        striped="true" grid-lines="true" editable="true" selected="0">
  <column title="#" width="56" align="right"/>
  <column title="Title" flex="true"/>
  <column title="Length" width="72" align="right"/>
  <row><cell text="1"/><cell text="Nocturne"/><cell text="3:58"/></row>
  <row><cell text="2"/><cell text="Sunny Day"/><cell text="4:29"/></row>
</widget>
```

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建空表格。 |
| `.columns(Vec<TableColumn>)` / `.rows(Vec<TableRow>)` | model | empty | 替换列或行。 |
| `.header_height(f32)` / `.row_height(f32)` | `f32` | `30.0` / `32.0` | 表头带和主体行高；行高限制为 `>= 16`。 |
| `.cell_padding(f32)` | `f32` | `10.0` | 表头和单元格矩形内的水平内边距。 |
| `.striped(bool)` / `.grid_lines(bool)` | `bool` | `false` / `true` | 交替行着色；行分隔线和列分隔线。 |
| `.empty_text(impl Into<String>)` / `.sort(column, ascending)` | text / `usize, bool` | `""` / none | 空消息；初始排序指示标记（行不会被重排）。 |
| `.on_select(impl Fn(usize) -> M)` | row handler | `None` | 某一行被点击（会先被选中）。 |
| `.on_activate(impl Fn(usize) -> M)` | row handler | `None` | 在不可编辑的行上按 Enter 或双击。 |
| `.on_header_click(impl Fn(usize) -> M)` | column handler | `None` | 表头单元格被点击。 |
| `.on_column_resize(impl Fn(usize, f32) -> M)` / `.on_column_reorder(impl Fn(usize, usize) -> M)` | column handlers | `None` | 分隔条拖动调整了固定列的大小，或表头拖动移动了某一列（两者均已应用）。 |
| `.editable(bool)` | `bool` | `false` | 激活就地单元格编辑。 |
| `.on_cell_edit(impl Fn(usize, usize, String) -> M)` | row, column, text | `None` | 单元格草稿被提交（已应用）。 |
| `.scrollbar_style(ScrollBarStyle)` | style | theme default | 两个滚动条的外观。 |

`TableColumn`：`new(title, width)`（固定，宽度 `>= 0`）、`flex(title)`、`.align(TextAlign)`、`title()`、`width()`、`is_flex()`、`alignment()`、`set_title()`。`TableRow`：`new(cells)` 接受任意字符串迭代器、`.disabled(bool)`、`.cell(column)`（超过最后一个单元格时返回空字符串）以及公开的 `cells`/`disabled` 字段。

类型化访问和运行时 setter：

| 方法 | 类型 | 描述 |
|---|---|---|
| `.columns_ref()` / `.rows_ref()` / `.row(usize)` / `.row_count()` / `.set_columns(..)` / `.set_rows(..)` | mixed | 当前数据；setter 会替换它。 |
| `.selected_row()` / `.selected_cell()` / `.select_cell(row, column)` | cursors | 行光标、单元格光标、带限制的移动。 |
| `.cell_text(row, column)` / `.cell_rect(row, column)` / `.column_span(usize)` / `.set_column_width(usize, f32)` | mixed | 单元格文本、裁剪后的单元格矩形、列范围；调整大小（flex 会忽略它）。 |
| `.set_cell(row, column, text)` | `bool` | 写入一个单元格，如有需要会补齐该行。 |
| `.set_sort(Option<(usize, bool)>)` / `.sort_order()` / `.move_column(from, to)` | mixed | 绘制/清除箭头；移除并重新插入一列。 |
| `.edit_cell(row, column)` / `.is_editing_cell()` | `bool` | 打开或查询就地编辑会话。 |
| `.scroll_to_row`、`.ensure_row_visible`、`.scroll_offset`、`.scroll_x_offset`、`.visible_rows` | mixed | 滚动和视口。 |

行为：表头为排序箭头预留空间。调整大小仅针对固定列，并在 24 DIP 处停止；表头拖动至少移动 6 DIP 后才变为重排。就地编辑通过 Enter、F2 或双击启动（Enter/Tab 提交，Escape 取消）；带换行符的值会使用多行编辑器。按键按行/列、页和边界移动。角色为 `Grid`，带有 `ColumnHeader` 子节点和逐单元格值。

## 事件

### 消息

每个回调都是返回消息的构建器闭包（`Fn(...) -> M`）；`new` 只接收 id，因此不向构造函数传递任何内容。

| 消息 | 发出时机 |
|---|---|
| 传给 `.on_select` 的 `Fn(usize) -> M` | 某一行被点击并被选中。 |
| 传给 `.on_activate` 的 `Fn(usize) -> M` | 在不可编辑的行上按 Enter 或双击。 |
| 传给 `.on_header_click` 的 `Fn(usize) -> M` | 表头单元格被点击（应用程序重新排序）。 |
| 传给 `.on_column_resize` 的 `Fn(usize, f32) -> M` | 分隔条拖动结束；新宽度已应用。 |
| 传给 `.on_column_reorder` 的 `Fn(usize, usize) -> M` | 表头拖动放置；列的移动已应用。 |
| 传给 `.on_cell_edit` 的 `Fn(usize, usize, String) -> M` | 单元格草稿被提交；新文本已应用。 |

### 通用观察器

该控件不安装共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。下面的回调是它的事件接缝；其他任何内容都通过 `Ui::bind` 处理。

### 控件专属回调

| 方法 | 处理器 | 触发时机 |
|---|---|---|
| `.on_select(handler)` | `impl Fn(usize) -> M + 'static` | 在某一行上按下并释放；该行在处理器运行前被选中。 |
| `.on_activate(handler)` | `impl Fn(usize) -> M + 'static` | 按 Enter、UIA invoke，或双击不可编辑的行。 |
| `.on_header_click(handler)` | `impl Fn(usize) -> M + 'static` | 表头单元格被点击；返回列索引并用 `set_sort` 重新排序。 |
| `.on_column_resize(handler)` | `impl Fn(usize, f32) -> M + 'static` | 固定列的表头分隔条移动；`(column, new_width)`，已应用。 |
| `.on_column_reorder(handler)` | `impl Fn(usize, usize) -> M + 'static` | 表头拖动移动了某一列；`(from, to)`，已应用。 |
| `.on_cell_edit(handler)` | `impl Fn(usize, usize, String) -> M + 'static` | 就地编辑会话提交；`(row, column, text)`，已应用。 |

`on_cell_edit` 仅在 `.editable(true)` 打开了会话时运行；双击可编辑单元格会开始编辑而不是激活。

### Ui 触发器

任何控件 id 都可以通过 `Ui::bind` 进行绑定：

```rust
ui.bind(WidgetId::new("tracks"), Trigger::Click, |event, ctx| {
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

它响应 `Click`（选择）、`DoubleClick`（激活或编辑）、`Activate`、`Wheel`（两个轴）以及 `ValueChange`（行/单元格选择）。表头的调整大小和重排响应按下/移动/释放输入，而非这些触发器；`RightClick` 没有内置效果。

## XML 定义

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。

几何布局遵循常规的 `<widget>` 规则。通用属性适用，包括 `selected`（行索引）、`read-only`（锁定单元格编辑）和 `name`。`font-size`/`font-family` 属于绘制层。表格从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported` 并跳过。

| 属性 | 类型 | 描述 |
|---|---|---|
| `row-height` | number | 主体行高；必须 `>= 16`。 |
| `header-height` | number | 表头带高度；`0` 会隐藏它。 |
| `cell-padding` | number | 水平单元格内边距；`>= 0`。 |
| `striped` | bool | 交替行上的面板色着色。 |
| `grid-lines` | bool | 分隔线（默认开启）。 |
| `editable` | bool | 启用单元格编辑。 |
| `empty-text` | text | 没有行时的消息。 |

子元素：

| 元素 | 属性 | 描述 |
|---|---|---|
| `<column>` | `title`（必需，非空）、`width`（`>= 0`）、`flex`（bool）、`align`（`left`/`center`/`right`） | 一列；`flex` 会忽略 `width`。 |
| `<row>` | `disabled`（bool）；子元素 `<cell text>` | 一行；`text` 必需且可以为空。 |

子元素列表属于内容：仅在文档生成变化时写入，且列和行会一起替换，因此只改变其中之一的文档也必须同时声明两者。

## 另见

- `tree-view.md`
- `property-grid.md`
- `list-view.md`
- `../../guides/xml-layout-authoring.md`
