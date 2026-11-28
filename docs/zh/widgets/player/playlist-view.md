# PlaylistView

> 虚拟化、可重排的曲目列表：选择、播放并拖动以重排。

## 介绍

`PlaylistView<M>` 从 `Vec<Row>` 绘制一个垂直列表，复用共享的行模型：标题（曲目）、副标题（艺术家）、尾部（时长）、`current`（正在播放的曲目）和 `disabled`。只绘制、命中测试并发布视口行，并且该列表拥有自己的滚动偏移、滚轮处理和主题滚动条。

单击会选中并上报 `on_select`；双击或 Enter 通过 `on_activate` 播放。按下某行并拖动它会移动曲目：控件绘制一条强调色插入线，在视口边缘附近自动滚动，并通过 `on_reorder` 上报移动。应用拥有数据，并自行执行该移动；控件从不重排其副本，因此模型和控件不会漂移。`Alt+上/下` 是等效的键盘操作。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::PlaylistView;
use bamboo_native_ui::{Rect, Row, Ui, WidgetId};

enum Msg {
    Select(usize),
    Play(usize),
    Reorder(usize, usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    PlaylistView::new("queue")
        .rows(vec![
            Row::new("Nocturne").subtitle("Jay Chou").trailing("3:58").current(true),
            Row::new("Sunny Day").subtitle("Jay Chou").trailing("4:29"),
            Row::new("Rice Field").subtitle("Jay Chou").trailing("3:43"),
        ])
        .row_height(40.0)
        .alternating(true)
        .empty_text("The queue is empty")
        .on_select(Msg::Select)
        .on_activate(Msg::Play)
        .on_reorder(Msg::Reorder),
);
ui.set_rect(WidgetId::new("queue"), Rect::from_xywh(552.0, 56.0, 320.0, 360.0));
```

`on_reorder(from, to)` 上报的 `to` 是**移除** `from` 之后的最终索引，因此应用按 `let track = rows.remove(from); rows.insert(to, track);` 执行它，并用 `set_rows` 将结果推回。拖放到同一槽位不发出任何消息。

## 用法（XML）

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。其子列表为 `<track>`，会替换所有行：

```xml
<widget id="queue" x="552" y="56" w="320" h="360"
        row-height="40" alternating="true" empty-text="The queue is empty"
        selected="0">
  <track title="Nocturne" subtitle="Jay Chou" trailing="3:58" current="true"/>
  <track title="Sunny Day" subtitle="Jay Chou" trailing="4:29"/>
  <track title="Rice Field" subtitle="Jay Chou" trailing="3:43" disabled="true"/>
</widget>
```

`title` 为必填且必须非空。`current` 和 `disabled` 是 bool 属性。共享的 `selected` 属性在子列表之后应用，并移动光标而不上报消息。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建一个空列表；`320 × 360` DIP。 |
| `.rows(Vec<Row>)` / `.set_rows` | `Vec<Row>` | 空 | 替换曲目、钳制光标并清除拖动。 |
| `.row_height(f32)` | `f32` | `44.0` | 行高（DIP），钳制为 `>= 24`。 |
| `.alternating(bool)` | `bool` | `false` | 用面板颜色为每隔一行着色。 |
| `.empty_text(text)` | text | `""` | 没有曲目时居中显示的消息。 |
| `.on_select(handler)` | `Fn(usize) -> M` | 无 | 一次点击改变了当前曲目。 |
| `.on_activate(handler)` | `Fn(usize) -> M` | 无 | 双击、Enter 或 `Activate` 播放曲目。 |
| `.on_reorder(handler)` | `Fn(usize, usize) -> M` | 无 | 曲目被移动；`to` 是移除后的索引。 |
| `.scrollbar_style(ScrollBarStyle)` | style | 主题默认 | 滚动条粗细、内边距和颜色。 |

类型化访问和运行时 setter：

| 方法 | 类型 | 描述 |
|---|---|---|
| `.rows_ref()` | `&[Row]` | 当前模型。 |
| `.selected_index()` | `usize` | 光标索引（为空时为 `0`）。 |
| `.visible_range()` | `(usize, usize)` | 发布的视口范围，右端不含。 |
| `.row_rect(usize)` / `.scroll_offset()` | `Option<Rect>` / `f32` | 行矩形和滚动偏移。 |
| `.drag_state()` | `Option<(usize, usize)>` | 拖动进行中的 `(from, 插入槽位)`。 |
| `.scroll_to_row(usize)` / `.ensure_row_visible(usize)` | `bool` | 将行顶部对齐，或以最小幅度滚动到可见。 |
| `.set_row_height(f32)` | `bool` | 运行时行高（`>= 24`），保持光标可见。 |
| `.set_alternating(bool)` / `.is_alternating()` | `bool` | 运行时斑马纹。 |
| `.set_empty_text(text)` / `.empty_text_ref()` | text | 运行时空消息。 |
| `.scrollbar()` | `&ScrollBarStyle` | 当前滚动条样式。 |

### 行为

- **指针。** 在按下的、已启用的行上释放会选中它，而不会滚动到它。按下后拖动开始重排；将指针保持在顶部/底部边缘 24 DIP 以内，每次移动会滚动半行。插入槽位是最近的行间隙，并钳制到两端。
- **键盘。** 上/下、PageUp/PageDown、Home/End 移动光标并显示它；Enter 播放；`Alt+上/下` 移动选中的曲目并上报 `on_reorder`。重排不会在两端循环。
- **行。** `current` 行绘制选中的填充和一条 3-DIP 强调色条；`disabled` 行绘制为暗淡，且无法被选中、播放或拖动。尾部文本在一个 64-DIP 区域内右对齐。
- **无障碍。** 角色为 `List`，包含可见行，选中的行被标记，一个预读行被标记为屏幕外，并带有垂直滚动状态。调用子项会选中并显示它。

## 事件

### 消息

| 消息 | 发出条件 |
|---|---|
| 传给 `.on_select` 的 `impl Fn(usize) -> M` | 在按下的、已启用的行上释放指针、键盘光标移动（方向键、PageUp/PageDown、Home/End）或 UIA 子调用会选中一行。 |
| 传给 `.on_activate` 的 `impl Fn(usize) -> M` | 双击已启用的行、在选中行上按 `Enter` 或 `Activate` 会播放它。 |
| 传给 `.on_reorder` 的 `impl Fn(usize, usize) -> M` | 拖动在不同槽位上释放，或 `Alt+上`/`Alt+下` 移动选中行；`to` 是移除 `from` 之后的索引。 |

从文档设置 `rows`、`row_height` 或 `selected`，以及将拖动放回其自身槽位，都不发出任何消息。

### 通用观察器

此控件不安装任何共享的观察器构建器。请使用上述三个回调或 `Ui::bind`。

### 控件专属回调

| 回调 | 签名 | 触发条件 |
|---|---|---|
| `.on_select(handler)` | `impl Fn(usize) -> M` | 通过点击、键盘或 UIA 调用改变了当前行。 |
| `.on_activate(handler)` | `impl Fn(usize) -> M` | 通过双击、`Enter` 或 `Activate` 播放已启用的行。 |
| `.on_reorder(handler)` | `impl Fn(usize, usize) -> M` | 拖动释放或 `Alt+上`/`Alt+下` 移动了一行。 |

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("queue"), Trigger::DragEnd, |event, ctx| {
    // 返回 Some(Msg::...) 以排队一条消息；调用 ctx.default() 以保留
    // 内置动作，随后会在处理函数之后运行。
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

该列表响应 `Click`、`DoubleClick`、`HoverEnter`/`HoverLeave`、`Wheel`、`Activate`、`DragStart`/`DragEnd` 和 `ValueChange`；右键点击没有内置效果。

## XML 定义

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。

几何遵循常规的 `<widget>` 规则。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`、`expose`、焦点策略）适用。内容支持仅为 `selected`。该列表从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported`。

控件局部属性：

| 属性 | 类型 | 描述 |
|---|---|---|
| `row-height` | 数字 `>= 24` | 行高（DIP）。 |
| `alternating` | bool | 交替行上的面板着色。 |
| `empty-text` | 文本 | 列表为空时居中显示的消息。 |

子元素：

| 元素 | 属性 | 描述 |
|---|---|---|
| `<track>` | `title`（必填，非空）、`subtitle`、`trailing`、`current`（bool）、`disabled`（bool） | 一条曲目；该列表替换所有行。 |

## 另见

- `./cover-wall.md`
- `./playback-transport.md`
- `../data/list-view.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
