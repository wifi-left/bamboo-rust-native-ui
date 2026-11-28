# 滚动

> 普通控件之上的可滚动视口，以及共享的滚动条模型。

## 概述

`Ui` 是一张扁平的控件映射，因此滚动视图是**容器区域**，而不是拥有子项的 `Widget`。应用提供
视口，并在*内容*坐标中一次性布局其子项（视口的左上角即内容原点）。`Ui` 将每个子项的矩形
保持在 `content rect - offset`，因此命中测试、绘制、类型化访问、原生编辑器、工具提示和
辅助功能树都继续读取普通的客户区矩形。

滚动条由容器用共享的 `widgets::scrollbar` 数学绘制和拖动；面板只存储样式和每个滚动条
的指针状态。该模块跨平台（滚轮行数在 Windows 上来自系统，其他平台默认为 3）。

## 关键类型

### `ScrollView`

```rust
pub struct ScrollView { /* private */ }

impl ScrollView {
    pub const DEFAULT_WHEEL_STEP: f32 = 48.0;
    pub fn new(id: WidgetId, viewport: Rect, axes: ScrollBars) -> Self;
}
```

构建方法（均消耗 `self`）：

| 方法 | 签名 | 描述 |
|---|---|---|
| `child` | `child(self, id: WidgetId, rect: Rect) -> Self` | 以内容坐标追加一个子项。 |
| `children` | `children(self, impl IntoIterator<Item = ScrollChild>) -> Self` | 替换子项列表。 |
| `content` | `content(self, content: Size) -> Self` | 显式内容范围（以子项为下限）。 |
| `offset` | `offset(self, offset: Point) -> Self` | 初始偏移，注册时会被钳制。 |
| `style` | `style(self, style: ScrollBarStyle) -> Self` | 滚动条几何与颜色。 |
| `wheel_step` | `wheel_step(self, dips: f32) -> Self` | 每格滚轮行程，最小钳制为 1 DIP。 |
| `padding` | `padding(self, dips: f32) -> Self` | 视口边缘与内容之间的统一边距。 |
| `padding_by` | `padding_by(self, insets: Insets) -> Self` | 每边各自的边距。 |
| `name` / `named` | `name(self, impl Into<String>) -> Self` | 辅助功能 `Group` 名称（也可用 `named`）。 |
| `nested_in` | `nested_in(self, parent: WidgetId) -> Self` | 嵌套在另一个面板内（D92）。 |

读取方法包括 `id()`、`viewport()`、`configured_viewport()`、`clip_rect()`、`axes()`、
`style_ref()`、`children_ref()`、`child_rect(id)`、`offset_of()`、`content_size()`、
`max_offset()`、`wheel_step_dips()`、`padding_insets()`、`padded_rect()`、
`content_rect()`、`name_ref()`、`parent()` 和 `contains(id)`。
修改方法包括 `set_offset`、`set_viewport`、`set_axes`、`set_style`、`set_children`、
`set_content_override`。

`ScrollBars`（来自 `bamboo_native_ui`）为 `None | Vertical（默认） | Horizontal | Both`。

### `ScrollChild`

```rust
pub struct ScrollChild { pub id: WidgetId, pub rect: Rect }
impl ScrollChild { pub fn new(id: WidgetId, rect: Rect) -> Self; }
```

`rect` 使用内容坐标；容器将控件定位在 `rect - offset`。

### `ScrollBarStyle` / `ScrollBarColors`

来自 `bamboo_native_ui::widgets::scrollbar`（重新导出为
`widgets::{ScrollBarStyle, ScrollBarColors}`）。`ScrollBarStyle` 的字段与默认值：

| 字段 | 类型 | 默认值 |
|---|---|---|
| `thickness` | `f32` | `10.0` |
| `inset` | `f32` | `0.0` |
| `thumb_inset` | `f32` | `2.0` |
| `min_thumb` | `f32` | `24.0` |
| `page_on_track_click` | `bool` | `false` |
| `colors` | `Option<ScrollBarColors>` | `None`（从调色板解析） |

构建方法：`thickness`（最小 2.0）、`inset`、`thumb_inset`、`min_thumb`（最小 4.0）、
`page_on_track_click`、`colors`。`ScrollBarColors { track, thumb, thumb_hover,
thumb_active }` 是纯数据；`ScrollBarColors::from_palette(&Palette)` 可推导出它。该模块还
公开 `wheel_lines() -> f32`（系统格数，非 Windows 上为 3）以及 `PAGE_LINES: f32 = 100.0`。

### `Ui` 方法

| 方法 | 签名 |
|---|---|
| `set_scroll_view` | `set_scroll_view(&mut self, view: ScrollView, sink: &mut impl UiHost)` |
| `scroll_view` | `scroll_view(&self, id: WidgetId) -> Option<&ScrollView>` |
| `remove_scroll_view` | `remove_scroll_view(&mut self, id: WidgetId, sink: &mut impl UiHost) -> bool` |
| `scroll_offset` | `scroll_offset(&self, id: WidgetId) -> Option<Point>` |
| `set_scroll_offset` | `set_scroll_offset(&mut self, id, offset: Point, sink) -> bool` |
| `scroll_by` | `scroll_by(&mut self, id, dx: f32, dy: f32, sink) -> bool` |
| `scroll_to_child` | `scroll_to_child(&mut self, child: WidgetId, sink) -> bool` |
| `scroll_owner_of` | `scroll_owner_of(&self, child: WidgetId) -> Option<WidgetId>` |
| `set_scroll_viewport` | `set_scroll_viewport(&mut self, id, viewport: Rect, sink) -> bool` |

## 示例

```rust
use bamboo_native_ui::{Rect, ScrollView, Size, Ui, WidgetId};
use bamboo_native_ui::ScrollBars;

fn register_queue(ui: &mut Ui<Msg>, sink: &mut impl bamboo_native_ui::UiHost) {
    let view = ScrollView::new(
        WidgetId::new("queue"),
        Rect::from_xywh(16.0, 48.0, 320.0, 480.0),
        ScrollBars::Vertical,
    )
    .padding(8.0)
    .child(WidgetId::new("row-0"), Rect::from_xywh(0.0, 0.0, 320.0, 56.0))
    .child(WidgetId::new("row-1"), Rect::from_xywh(0.0, 56.0, 320.0, 56.0))
    .name("Play queue");
    ui.set_scroll_view(view, sink);
    ui.scroll_to_child(WidgetId::new("row-1"), sink);
}
```

`row-0` 和 `row-1` 是用 `ui.add(...)` 添加的普通控件；面板在尺寸调整或子项布局变化后会
被重新注册，并保持其活动偏移。`scroll_to_child` 会滚动子项所属的面板，距离恰好是将其
完全带入视野所需的最小值。

## 说明与限制

- `ScrollView` 是由 `Ui` 管理的容器原语，而非 `Widget`。它没有 builder-props，在
  `.buixml` 中没有 `type=`，并且通过 `set_scroll_view` 而非 `add` 注册。
- 在尺寸调整或子项重新布局后需再次调用 `set_scroll_view`。重新注册同一 id 会保留活动偏移；
  如需更改请使用 `set_scroll_offset`。
- 只有某个轴的溢出实际超出带内边距的视口时，才显示对应滚动条，因此较短的面板不会显示
  空轨道。`page_on_track_click`（默认 `false`）使点击轨道时跳转至滑块居中；`true` 则按约
  90% 翻页。
- 滚轮行程为每格 `wheel_step` DIP（默认 `ScrollView::DEFAULT_WHEEL_STEP`，48 DIP = 三行
  文本）。系统的“滚动一页”设置以较大的标记 `PAGE_LINES` 传入。
- 有名称的面板会发布一个 `A11yRole::Group` 节点以及 Scroll 模式；参见 `accessibility.md`。
- 嵌套面板（`nested_in`）随其父级移动和裁剪；移除父级会将其嵌套面板恢复为位于其配置视口
  的根面板。

## 另见

- `accessibility.md`（Scroll 模式状态）
- `drag-and-drop.md`（滚动列表内的放置目标）
- `../widgets/data/list-view.md`
- `../widgets/data/tree-view.md`
- `../widgets/inputs/text-edit.md`
