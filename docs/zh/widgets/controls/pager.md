# Pager

> 一个带有居中页面标签的上一页/下一页导航栏。

## 介绍

`Pager<M>` 在前端和后端绘制两个相等的点击区域，并在它们之间绘制一个居中的标签：当前页和页数。这些区域最宽为 80 DIP（控件的 30%），可用时填充 `button_bg`，不可用时填充 `button_disabled`，并携带内置标题 `上一页`（“previous”）和 `下一页`（“next”）。

该控件通过 `on_nav(i32)` 上报导航增量，且从不自行更改页面。应用更新自己的状态，并用 `Ui::set_selected(id, page)` 将页面写回；控件会钳制它并刷新标签。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::Pager;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Page(i32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Pager::new("pager", 0, 5, Msg::Page)
        .label_with(|page, pages| format!("Page {page} of {pages}")),
);
ui.set_rect(WidgetId::new("pager"), Rect::from_xywh(16.0, 16.0, 360.0, 32.0));
```

`on_nav` 对“上一页”接收 `-1`，对“下一页”接收 `+1`。`label_with` 接受 `Fn(usize, usize) -> String`，其中页是**从 1 开始**的，外加页数；该闭包替换内置的 `第 N / M 页` 标签。在你的更新循环中应用上报的增量，例如使用 `ui.set_selected(WidgetId::new("pager"), page + 1)`。

## 用法（XML）

`Pager` 不由标准注册表创建。在 Rust 中创建它，并通过 `id` 定位；每页标签来自 `<page>` 子元素：

```xml
<widget id="pager" x="16" y="16" w="360" h="32" selected="0">
  <page label="Overview"/>
  <page label="Tracks"/>
  <page label="Credits"/>
</widget>
```

一个 `<page>` 要求一个非空的 `label`，不接受其他属性，也没有子元素。该列表安装每页标签，丢弃任何自定义格式，并将页数设为列表长度；然后现有的 `selected` 从中选中一个。没有 `<page>` 子元素时，中心标签是内置的中文 `第 N / M 页`，因此使用英文文案的应用应调用 `label_with`。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, page, pages, on_nav)` | `&'static str, usize, usize, impl Fn(i32) -> M` | — | 创建该分页器；`360 × 32` DIP，`pages` 钳制为至少 1。 |
| `.label_with(format)` | `impl Fn(usize, usize) -> String` | 内置 `第 N / M 页` | 替换中心标签；清除每页标签。 |
| `.set_label_format(format)` | 闭包 | — | 运行时交换格式；始终返回 `true`。 |
| `.clear_label_format()` | — | — | 恢复为内置标签；返回是否移除了一个格式。 |
| `.set_page_labels(Vec<String>)` | `Vec<String>` | — | 安装每页标签，丢弃格式，设置 `pages = len`；返回是否发生了更改。 |
| `.page_labels()` | — | — | 已安装的每页标签。 |
| `.label_text()` | — | — | 当前绘制的标签。 |
| `.page()` / `.pages()` | — | — | 当前页（从 0 开始）和页数。 |
| `.set_pages(usize)` | `usize` | — | 更改页数，钳制页面，截断标签；不发出任何消息。 |

### 行为

- **点击边界。** “上一页”仅在 `page > 0` 时上报；“下一页”仅在 `page + 1 < pages` 时上报。`Activate`（Enter/Space）始终上报 `+1` 而不做边界检查，因此应用必须在最后一页忽略它。
- **状态。** `page` 仅通过 `Widget::set_selected`（因而通过 `Ui::set_selected` 或文档的 `selected`）更改。`set_pages` 和 `set_page_labels` 会静默钳制它。
- **标题。** 区域的标题固定为 `上一页`/`下一页`；只有中心标签可替换。绘制使用 `Small` 文本，标签使用 `muted`。
- **无障碍性。** 角色为 `button`，名称来自标签（当标签为空时来自 id），值为 `"current/total"`。

## 事件

### 消息

| 消息 | 触发条件 |
|---|---|
| 作为 `on_nav` 传给 `Pager::new` 的 `Fn(i32) -> M` | 某个区域被点击：`-1` 表示“上一页”（仅在 `page > 0` 时），或 `+1` 表示“下一页”（仅在 `page + 1 < pages` 时）。`Enter`/`Space` 始终上报 `+1`。 |

### 通用观察器

该控件**不**安装共享的观察器构建器。它没有 `Observers` 字段，自身也没有焦点/按键处理；导航就是上面的构造函数回调。当需要应用侧钩子时，绑定 `Ui::bind`。

### 控件专属回调

除 `on_nav` 外没有。控件从不自行更改页面；应用增量并用 `Ui::set_selected` 将页面写回。

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("pager"), Trigger::Click, |event, ctx| {
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

分页器响应其两个区域内的 `Click` 以及 `Activate`；它忽略 `Wheel`、`DragStart`/`DragEnd` 和 `ValueChange`。

## XML 定义

- **创建：** 无 `type`；该控件仅可被定位。
- **几何：** 一种水平形式（`x`+`w`、`right`+`w`、`x`+`right`）和一种垂直形式（`y`+`h`、`bottom`+`h`、`y`+`bottom`）。这些区域各需要约 80 DIP，外加标签所需的空间。
- **内容：** `selected`（从 0 开始的页面）。`label`/`text`/`value` 不适用；绘制的文本是当前页面标签。
- **样式：** `font-size`、`font-family`。Pager 没有表面，因此 `radius`、`corners` 和 `background` 会报 `xml.style.unsupported`。
- **属性：** 通用运行时集合（`visible`、`tooltip`、`name`、……）。`enabled` 会被接受，但 `Pager` 没有禁用接缝，因此它没有效果；改为隐藏分页器或忽略其消息。
- **子元素：** 仅 `<page label="…"/>` 元素，如上所示。

## 另见

- `./tabs.md`
- `./segmented.md`
- `./progress-bar.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
