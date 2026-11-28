# PropertyGrid

> 一个分区的键/值检查器，每行带有一个类型化的网格内编辑器。

## 介绍

`PropertyGrid<M>` 绘制带标题的分区表头，与键/值行交错，并就地编辑值。每一行选择一个 `PropertyKind`：只读文本、内联文本或数字字段、下拉选择、复选框或星级评分。表头是滚动内容的一部分。

该控件拥有自己的数据（包含 `PropertyRow` 值的 `PropertySection` 值），并在通过 `on_change(key, value)` 报告编辑之前先应用它，因此应用程序只需持久化它。`on_activate(key, value)` 保留给在别处检查的只读行。网格不绘制背景。

## 用法（Rust）

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::{PropertyGrid, PropertyRow, PropertySection};

enum Msg { Select(String), Inspect(String, String), Change(String, String) }
let mut ui: Ui<Msg> = Ui::new();
ui.add(
    PropertyGrid::new("props")
        .sections(vec![
            PropertySection::new("Playback")
                .row(PropertyRow::new("Volume", "80").number_with(0.0, 100.0, 5.0, 0))
                .row(PropertyRow::new("Shuffle", "Off").check("On", "Off"))
                .row(PropertyRow::new("Rating", "4").rating(5))
                .row(PropertyRow::new("Quality", "Lossless").choice(["Lossless", "High"]))
                .row(PropertyRow::new("Title", "Night Drive").text()),
            PropertySection::new("File")
                .row(PropertyRow::new("Path", "/Music/Night Drive.flac")),
        ])
        .key_width(120.0)
        .on_select(|key: &str| Msg::Select(key.to_string()))
        .on_activate(|key: &str, value: &str| Msg::Inspect(key.to_string(), value.to_string()))
        .on_change(|key: &str, value: &str| Msg::Change(key.to_string(), value.to_string())),
);
ui.set_rect(WidgetId::new("props"), Rect::from_xywh(16.0, 56.0, 340.0, 280.0));
```

`PropertyRow::new` 初始为 `ReadOnly`；`text()`、`number()`、`number_with()`、`choice()`、`check()` 和 `rating()` 用于选择编辑器。一行由其键标识，因此已存储的值通过 `set_value`、`set_checked` 或 `set_rating`（经由 `ui.edit_widget`）写回。

## 用法（XML）

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。`<section>` 元素会替换每个分区；它们的 `<row>` 子元素选择类型及其参数：

```xml
<widget id="props" x="16" y="56" w="340" h="280"
        row-height="26" header-height="28" key-width="120"
        grid-lines="true" empty-text="No properties">
  <section title="Playback">
    <row key="Volume" value="80" kind="number" min="0" max="100" step="5" decimals="0"/>
    <row key="Shuffle" value="Off" kind="check" on="On" off="Off"/>
    <row key="Rating" value="4" kind="rating" stars="5"/>
    <row key="Quality" value="Lossless" kind="choice" options="Lossless|High"/>
    <row key="Title" value="Night Drive" kind="text"/>
  </section>
  <section title="File">
    <row key="Path" value="/Music/Night Drive.flac"/>
  </section>
</widget>
```

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建空网格。 |
| `.sections(Vec<PropertySection>)` | sections | empty | 替换每个分区。 |
| `.header_height(f32)` / `.row_height(f32)` | `f32` | `28.0` / `26.0` | 表头带和行高（行 `>= 16`；表头为 `0` 则移除该带）。 |
| `.key_width(f32)` | `f32` | `170.0` | 键列宽度，`>= 40`；绘制时将其上限设为宽度的 60%。 |
| `.grid_lines(bool)` / `.empty_text(impl Into<String>)` | `bool` / text | `true` / `""` | 行分隔线；为空时居中的消息。 |
| `.read_only(bool)` | `bool` | `false` | 锁定每个编辑器；导航仍然有效。 |
| `.on_select(impl Fn(&str) -> M)` / `.on_activate(impl Fn(&str, &str) -> M)` | key handlers | `None` | 某行成为光标；某只读行被激活。 |
| `.on_change(impl Fn(&str, &str) -> M)` | key/value handler | `None` | 网格提交了一次编辑（值已应用）。 |
| `.scrollbar_style(ScrollBarStyle)` | style | theme default | 滚动条外观。 |

`PropertyRow`：`new(key, value)`（`ReadOnly`）、`.text()`、`.number(min, max)`（步长 1，0 位小数）、`.number_with(min, max, step, decimals)`、`.choice(iterable)`、`.check(on, off)`、`.rating(stars)`（限制为 `1..=10`）以及 `.read_only(bool)`；访问器 `key()`、`value()`、`kind()`、`is_editable()`、`is_read_only()`、`set_value()`、`set_key()`。`PropertySection`：`new(title)`、`.row(...)`、`.rows(...)`、`title()`、`rows_ref()`、`row_count()`。`PropertyKind` 为 `ReadOnly`、`Text`、`Number { min, max, step, decimals }`、`Choice(Vec<String>)`、`Check { on, off }` 或 `Rating { stars }`。

类型化访问和运行时 setter：

| 方法 | 类型 | 描述 |
|---|---|---|
| `.sections_ref()` / `.section(usize)` / `.row_count()` | mixed | 数据模型。 |
| `.row_by_key(key)` / `.value(key)` | mixed | 具有某键的第一行，或其值。 |
| `.set_value(key, value)` / `.set_key(key, new_key)` | `bool` | 静默替换值或键。 |
| `.checked(key)` / `.set_checked(key, bool)` / `.rating(key)` / `.set_rating(key, u8)` | mixed | 勾选和评分辅助方法。 |
| `.set_row_read_only(section, row, bool)` | `bool` | 锁定一行。 |
| `.selected_path()` / `.selected_row()` / `.selected_key()` / `.select_key(key)` | mixed | 光标。 |
| `.ensure_key_visible(key)` / `.scroll_to_key(key)` | `bool` | 显示某一行。 |
| `.row_rect(section, row)` / `.value_rect(section, row)` | `Option<Rect>` | 客户区矩形。 |
| `.set_sections`、`.set_section_rows`、`.set_row_height`、`.set_header_height`、`.set_key_width`、`.set_grid_lines`、`.set_empty_text` | mixed | 运行时 setter。 |
| `.is_read_only()` / `.scroll_offset()` | mixed | 状态。 |

行为：文本或数字行在点击或按 Enter/空格时打开内联字段；Enter/Tab 提交，Escape 还原。数字草稿仅接受数字、`.` 和 `-`，提交时限制到 `min..=max`，按 `decimals` 位小数格式化，并通过 Up/Down 按 `step` 步进。选择控件会打开一个浮层列表（方向键和 Home/End 移动，Enter/空格提交，Escape 关闭）；复选框在其两个值字符串之间切换；评分会设置被点击的星或从键盘递增。`on_change` 在模型持有文本之后触发；只读类型（或锁定的行）通过 `on_activate` 进行检查。

## 事件

### 消息

三个回调都是返回消息的构建器闭包（`Fn(&str, ...) -> M`）；`new` 只接收 id。

| 消息 | 发出时机 |
|---|---|
| 传给 `.on_select` 的 `Fn(&str) -> M` | 某行成为光标（点击或键盘）；参数为行键。 |
| 传给 `.on_activate` 的 `Fn(&str, &str) -> M` | 某只读行被激活（双击、Enter 或辅助技术 invoke）。 |
| 传给 `.on_change` 的 `Fn(&str, &str) -> M` | 网格提交了一次编辑；模型更新之后的 `(key, value)`。 |

### 通用观察器

该控件不安装共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。下面的回调是它的事件接缝；其他任何内容都通过 `Ui::bind` 处理。

### 控件专属回调

| 方法 | 处理器 | 触发时机 |
|---|---|---|
| `.on_select(handler)` | `impl Fn(&str) -> M + 'static` | 光标通过指针或键盘移动到某一行。 |
| `.on_activate(handler)` | `impl Fn(&str, &str) -> M + 'static` | 某个 `ReadOnly` 行（或锁定的行）被激活；可编辑行则改为自行编辑。 |
| `.on_change(handler)` | `impl Fn(&str, &str) -> M + 'static` | 文本/数字字段提交、选择项被选中、复选框切换或评分变化；值已应用。 |

Escape 取消编辑器且不发出任何内容；`set_value` 和其他静默 setter 从不触发 `on_change`。

### Ui 触发器

任何控件 id 都可以通过 `Ui::bind` 进行绑定：

```rust
ui.bind(WidgetId::new("props"), Trigger::ValueChange, |event, ctx| {
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

它响应 `Click`（选择或打开编辑器）、`DoubleClick`/`Activate`（激活）、`Wheel`（滚动）以及 `ValueChange`（光标或勾选/评分状态）。`RightClick` 以及拖动触发器没有内置效果。

## XML 定义

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。

几何布局遵循常规的 `<widget>` 规则。通用属性适用，包括 `read-only`（锁定编辑器）和 `name`。`font-size`/`font-family` 属于绘制层。网格从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会报告 `xml.style.unsupported`。

| 属性 | 类型 | 描述 |
|---|---|---|
| `row-height` | number | 行高；必须 `>= 16`。 |
| `header-height` | number | 分区表头高度；`>= 0`。 |
| `key-width` | number | 键列宽度；必须 `>= 40`。 |
| `grid-lines` | bool | 行分隔线。 |
| `empty-text` | text | 没有分区时的消息。 |

子元素：

| 元素 | 属性 | 描述 |
|---|---|---|
| `<section>` | `title` | 一个带标题的分组。 |
| `<row>` | `key`（必需，非空）、`value`、`kind`、`read-only` | 一行。 |

`kind` 为 `read-only`（默认）、`text`、`number`、`choice`、`check` 或 `rating`。类型专属属性：`min`/`max`/`step`/`decimals`（number）；`options`（以 `|` 分隔，非空）（choice）；`on`/`off`（check）；`stars`（整数 `1..=10`）（rating）。超出所选类型的属性是一个错误。子元素列表属于内容：在文档生成变化时写入，与变体无关且静默；没有共享的 `selected`。

## 另见

- `table.md`
- `tree-view.md`
- `list-view.md`
- `../../guides/xml-layout-authoring.md`
