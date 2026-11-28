# ListView

> 一个虚拟化的单列列表，包含标题/副标题/尾部文本行。

## 介绍

`ListView<M>` 根据 `Vec<Row>` 数据模型绘制一个垂直列表，一次绘制一行高度，并且只绘制、命中测试和发布与视口相交的行。它拥有自己的滚动偏移、滚轮处理、行悬停/按下反馈以及一个集成滚动条。一行可以携带第二行、右对齐的尾部文本、"当前"高亮以及一个禁用标志。

该控件是一个带有键盘行光标的复合控件。点击某一行（在释放时）或按下 Enter，会通过 `on_activate` 报告行索引；没有单独的选择回调。队列或结果列表请使用 `ListView`；需要列时请使用 `Table`，层次结构请使用 `TreeView`。

## 用法（Rust）

```rust
use bamboo_native_ui::{Rect, Row, Ui, WidgetId};
use bamboo_native_ui::widgets::ListView;

enum Msg {
    Play(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    ListView::new("queue")
        .row_height(44.0)
        .alternating(true)
        .empty_text("Nothing queued")
        .on_activate(Msg::Play),
);
ui.set_rect(WidgetId::new("queue"), Rect::from_xywh(16.0, 56.0, 320.0, 360.0));
ui.set_rows(
    WidgetId::new("queue"),
    vec![
        Row::new("Nocturne").subtitle("Jay Chou").trailing("3:58").current(true),
        Row::new("Sunny Day").subtitle("Jay Chou").trailing("4:29"),
    ],
);
```

`Row::new` 接收标题；`subtitle`、`trailing`、`current` 和 `disabled` 是构建器方法。`Ui::set_rows` 是替换模型的静默方式，并会保持行光标在范围内。`current` 行绘制选中填充和一条 3 DIP 的强调色条；`disabled` 行使用禁用文本色绘制并忽略点击。鼠标激活不会移动键盘行光标，也不会改变任何 `Row::current` 标志；两者均由应用程序负责。

## 用法（XML）

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。`<item>` 子元素列表会替换每一行：

```xml
<widget id="queue" x="16" y="56" w="320" h="360"
        row-height="44" alternating="true" empty-text="Nothing queued"
        selected="0">
  <item title="Nocturne" subtitle="Jay Chou" trailing="3:58" current="true"/>
  <item title="Sunny Day" subtitle="Jay Chou" trailing="4:29"/>
  <item title="Rice Field" subtitle="Jay Chou" trailing="3:43" disabled="true"/>
</widget>
```

共享的 `selected` 属性（一个可见行索引）在子元素列表之后应用，并移动键盘光标而不报告消息。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建行高为 44 DIP 的列表。 |
| `.row_height(f32)` | `f32` | `44.0` | 行高（DIP），限制为 `>= 24`。 |
| `.alternating(bool)` | `bool` | `false` | 每隔一行用面板色着色。 |
| `.empty_text(impl Into<String>)` | text | `""` | 在没有行时居中绘制的消息。 |
| `.on_activate(impl Fn(usize) -> M)` | index handler | `None` | 某一行被点击或激活。 |
| `.scrollbar_style(ScrollBarStyle)` | style | theme default | 滚动条粗细、内边距和颜色。 |

类型化访问和运行时 setter：

| 方法 | 类型 | 描述 |
|---|---|---|
| `.rows()` | `&[Row]` | 当前数据模型。 |
| `.selected_row()` | `usize` | 键盘行光标（默认 0）。 |
| `.visible_rows()` | `(usize, usize)` | 发布的视口范围，末尾为开区间。 |
| `.scroll_offset()` | `f32` | 当前滚动偏移（DIP）。 |
| `.scroll_to_row(usize)` | `bool` | 将行滚动到顶部，带限制。 |
| `.ensure_row_visible(usize)` | `bool` | 显示该行的最小滚动。 |
| `.set_row_height(f32)` | `bool` | 运行时行高（`>= 24`），并保持光标可见。 |
| `.set_alternating(bool)` / `.is_alternating()` | `bool` | 运行时条纹。 |
| `.set_empty_text(text)` / `.empty_text_ref()` | text | 运行时空消息。 |
| `.scrollbar()` | `&ScrollBarStyle` | 当前样式。 |

行为：行是虚拟化的，因此一千行的列表仅需一个视口的工作量。滚轮输入会使列表滚动几行，列表会从外层 `ScrollView` 接管该滚动刻度。在调整模式下，Up/Down 移动光标，PageUp/PageDown 移动一页，Home/End 跳转，Enter 激活。无障碍角色为 `List`；仅发布视口内的行，且仅一行的前瞻行会被标记为屏幕外。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `.on_activate` 的 `Fn(usize) -> M` | 某一行被点击（在释放时）或用 Enter/Activate 激活；参数为行索引。 |

没有选择回调：移动键盘光标是静默的。

### 通用观察器

`ListView` 安装共享的观察器构建器（通过 `observer_methods!` / `observer_builders!`）。所有这些都是叠加式的：内置行处理在处理器返回后仍会运行，处理器的消息会与之一并入队。

| 构建器 | 处理器 | 触发时机 |
|---|---|---|
| `.on_click(f)` | `Fn(Point) -> Option<M>` | 控件上的左键点击。 |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | 右键点击，在上下文菜单之前。 |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | 平台双击。 |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | 控件获得焦点时的按键按下。 |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | 控件获得焦点时的按键释放。 |
| `.on_focus_gained(f)` | `Fn(FocusEvent) -> Option<M>` | 焦点移入该控件。 |
| `.on_focus_lost(f)` | `Fn(FocusEvent) -> Option<M>` | 焦点离开该控件（在编辑器提交之后）。 |

### 控件专属回调

| 方法 | 处理器 | 触发时机 |
|---|---|---|
| `.on_activate(handler)` | `impl Fn(usize) -> M + 'static` | 在某一行上按下并释放（禁用的行会被忽略）、调整模式下按 Enter，或 UIA Invoke；消息在该行被高亮之后入队。 |

该回调返回消息：它构建应用程序的 `M`，并在控件更新其光标/按下状态之后运行。

### Ui 触发器

任何控件 id 都可以通过 `Ui::bind` 进行绑定：

```rust
ui.bind(WidgetId::new("queue"), Trigger::Click, |event, ctx| {
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

它响应 `Click` 和 `Activate`（行激活）、`Wheel`（滚动）以及 `ValueChange`（选中行变化）。`DoubleClick`、`RightClick` 以及拖动触发器没有内置效果；滚动条拖动改用按下/移动/释放输入。

## XML 定义

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。

几何布局遵循常规的 `<widget>` 规则（一个水平形式和一个垂直形式）。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`、`expose`、焦点策略、光标）均适用。`font-size` 和 `font-family` 属于绘制层。列表从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported` 并跳过。

局部属性：

| 属性 | 类型 | 描述 |
|---|---|---|
| `row-height` | number | 行高（DIP）；必须 `>= 24`。 |
| `alternating` | bool | 交替行上的面板色着色。 |
| `empty-text` | text | 列表为空时居中的消息。 |

子元素：

| 元素 | 属性 | 描述 |
|---|---|---|
| `<item>` | `title`（必需，非空）、`subtitle`、`trailing`、`current`（bool）、`disabled`（bool） | 一行；列表会替换每一行。 |

子元素列表属于内容：仅在文档生成变化时写入，与变体无关，且静默。当列表变化时请声明每个 `<item>`。

## 另见

- `table.md`
- `tree-view.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
