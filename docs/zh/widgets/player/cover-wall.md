# CoverWall

> 虚拟化、可滚动的正方形专辑封面网格，带标题说明。

## 介绍

`CoverWall<M>` 是播放器的浏览界面。它把 `Cover` 图块排成垂直网格：封面按仍能容纳目标 `.tile(size)` 的最大图块宽度铺满整行，实际单元格会扩展，分摊剩余空间，并且只绘制和命中测试可见行。该墙使用共享的主题滚动条和滚轮滚动。

单击会选中一个封面并上报 `on_select`；双击或 Enter 通过 `on_open` 打开它，因此单击永远不会意外开始播放。选择框绘制在每个图块保留的 2-DIP 带内，因此选中封面永远不会改变画面尺寸或移动网格。

画面是可选页面图像，通过 `Ui::set_image` 注册。图像未设置或无法识别时，绘制带标题首字符的哈希着色渐变图块。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{Cover, CoverWall};
use bamboo_native_ui::{ImageId, Rect, Ui, WidgetId};

enum Msg {
    Select(usize),
    Open(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    CoverWall::new("wall")
        .covers(vec![
            Cover::new("Nocturne").subtitle("Jay Chou").image(ImageId(7)),
            Cover::new("Bamboo").subtitle("Playlist").corner_radius(4.0),
            Cover::new("Sunny Day").subtitle("Jay Chou"),
        ])
        .tile(148.0)
        .gap(14.0)
        .caption_height(42.0)
        .corner_radius(6.0)
        .empty_text("No covers")
        .on_select(Msg::Select)
        .on_open(Msg::Open),
);
ui.set_rect(WidgetId::new("wall"), Rect::from_xywh(16.0, 56.0, 520.0, 320.0));
```

`Cover::image` 接受一个 `ImageId`（传给 `Ui::set_image` 的数字）；没有图像名称注册表。`Cover::corner_radius` 会为单个图块覆盖墙的默认值，`Some(0.0)` 表示方形裁剪。使用 `set_covers`（或 `Ui::edit_widget`）在运行时替换数据，它会将光标保持在范围内、清除悬停/按下状态并显示所选封面。

## 用法（XML）

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。其子列表为 `<cover>`，会替换所有封面：

```xml
<widget id="wall" x="16" y="56" w="520" h="320"
        gap="14" corner-radius="6" caption-height="42"
        empty-text="No covers" selected="0">
  <cover title="Nocturne" subtitle="Jay Chou" image="7" corner-radius="4"/>
  <cover title="Bamboo" subtitle="Playlist"/>
  <cover title="Sunny Day" subtitle="Jay Chou"/>
</widget>
```

`title` 为必填且必须非空；`image` 是原始数值 `ImageId`；`corner-radius` 钳制为 `>= 0`。共享的 `selected` 属性是封面索引，并在子列表之后应用。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建一个空墙；`520 × 320` DIP。 |
| `.covers(Vec<Cover>)` / `.set_covers` | `Vec<Cover>` | 空 | 替换模型并将光标保持在范围内。 |
| `.tile(f32)` | `f32` | `132.0` | 目标图块宽度（DIP），钳制为 `>= 48`。 |
| `.gap(f32)` | `f32` | `14.0` | 图块之间的间距，钳制为 `>= 0`。 |
| `.caption_height(f32)` | `f32` | `42.0` | 画面下方的标题带，钳制为 `>= 0`。 |
| `.corner_radius(f32)` | `f32` | `0.0` | 墙默认的画面圆角，钳制为 `>= 0`。 |
| `.empty_text(text)` | text | `""` | 墙为空时居中显示的消息。 |
| `.on_select(handler)` | `Fn(usize) -> M` | 无 | 一次点击改变了当前封面。 |
| `.on_open(handler)` | `Fn(usize) -> M` | 无 | 双击、Enter 或 `Activate` 打开了封面。 |
| `.scrollbar_style(ScrollBarStyle)` | style | 主题默认 | 滚动条的粗细、内边距和颜色。 |

类型化访问和运行时 setter：

| 方法 | 类型 | 描述 |
|---|---|---|
| `.covers_ref()` | `&[Cover]` | 当前模型。 |
| `.selected_index()` | `usize` | 选中的封面（钳制到范围内）。 |
| `.columns()` | `usize` | 当前行的列数（至少一个）。 |
| `.visible_range()` | `(usize, usize)` | 绘制的视口范围，右端不含。 |
| `.cell_rect(usize)` / `.scroll_offset()` | `Option<Rect>` / `f32` | 图块矩形和滚动偏移。 |
| `.scroll_to_index(usize)` | `bool` | 将索引所在行滚动到顶部（钳制）。 |
| `.set_gap` / `.set_caption_height` / `.set_empty_text` | `bool` | 运行时局部值；返回它们是否发生变化。 |
| `.gap_value()` / `.corner_radius_value()` / `.caption_height_value()` / `.empty_text_value()` | values | 读取局部值。 |
| `.scrollbar()` | `&ScrollBarStyle` | 当前滚动条样式。 |

`Cover` 字段是公开的：`title`、`subtitle`、`image: Option<ImageId>` 和 `corner_radius: Option<f32>`。构建器：`new(title)`、`subtitle`、`image`、`corner_radius`。

### 行为

- **布局。** 列根据可用宽度、目标图块和间距计算；显示的滚动条会从行宽中扣除其条带。图块和内容的几何形状不随选择而改变。标题会截断并显示省略号，副标题仅在 `caption_height >= 34` 时绘制。
- **指针。** 在按下的图块上释放会选中它，而不会滚动到它；滚轮每格约滚动一行并进行钳制。滚动条滑块可拖动，点击轨道会跳转。
- **键盘。** 方向键按图块或行移动，PageUp/PageDown 按页移动，Home/End 跳转，Enter 打开。键盘选择会显示该行。
- **无障碍。** 角色为 `List`，可见封面作为子项，选中的子项被标记，一个预读行被标记为屏幕外，并带有垂直滚动状态。调用子项会选中它。

## 事件

### 消息

| 消息 | 发出条件 |
|---|---|
| 传给 `.on_select` 的 `impl Fn(usize) -> M` | 在按下的图块上释放指针会选中它，或 UIA 子调用会选中它。键盘方向键/Home/End 移动会静默改变选择。 |
| 传给 `.on_open` 的 `impl Fn(usize) -> M` | 双击图块、在选中图块上按 `Enter` 或 `Activate` 会打开它。 |

两个处理函数都接收封面索引。通过 `set_covers` 替换模型或从文档设置 `selected` 不发出任何消息。

### 通用观察器

此控件不安装任何共享的观察器构建器。请使用 `on_select` / `on_open` 或 `Ui::bind`。

### 控件专属回调

| 回调 | 签名 | 触发条件 |
|---|---|---|
| `.on_select(handler)` | `impl Fn(usize) -> M` | 通过点击或 UIA 调用改变了当前封面。 |
| `.on_open(handler)` | `impl Fn(usize) -> M` | 通过双击、`Enter` 或 `Activate` 打开封面。 |

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("wall"), Trigger::DoubleClick, |event, ctx| {
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

该墙响应 `Click`、`DoubleClick`、`HoverEnter`/`HoverLeave`、`Wheel`、`Activate` 和 `ValueChange`；右键点击没有内置效果。

## XML 定义

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。

几何遵循常规的 `<widget>` 规则。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`、`expose`、焦点策略）适用。内容支持仅为 `selected`。该墙从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported`。

控件局部属性：

| 属性 | 类型 | 描述 |
|---|---|---|
| `gap` | 数字 `>= 0` | 图块之间的间距。 |
| `corner-radius` | 数字 `>= 0` | 画面圆角默认值。 |
| `caption-height` | 数字 `>= 0` | 标题带高度。 |
| `empty-text` | 文本 | 没有封面时绘制的消息。 |

子元素：

| 元素 | 属性 | 描述 |
|---|---|---|
| `<cover>` | `title`（必填，非空）、`subtitle`、`image`（数值 `ImageId`）、`corner-radius` | 一个图块；该列表替换所有封面。 |

未知的子属性会被原子地拒绝；先前的墙保持不变。

## 另见

- `./playlist-view.md`
- `./waveform-seek-bar.md`
- `../data/list-view.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
