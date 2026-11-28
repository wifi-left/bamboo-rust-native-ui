# MultiSelectList

> 支持 Ctrl/Shift 多选的虚拟化可勾选行列表。

## 介绍

`MultiSelectList<M>` 绘制一个可滚动的行列表，每行带一个复选框和一个标签。选择是每行的
勾选标志，独立于其他行。只有与视口相交的行才会被绘制和发布，因此长列表的开销仍然很低。

普通点击会切换被点击的行并移动锚点。Ctrl+点击切换一行并将锚点移到那里；Shift+点击会选择
从锚点到该行的包含范围并保持它（在任何先前点击之前的 Shift+点击行为类似普通切换）。
滚轮按小数 DIP 滚动（使行和滑块一起移动），主题滚动条可以悬停、点击和拖动。
键盘：上/下移动光标，Home/End 跳转，Space/Enter 切换光标所在行。

每次更改都会发出 `on_change(selected)`，包含完整排序的勾选索引列表；没有改变任何内容的
Shift 范围不会发出任何消息。无障碍为 `A11yRole::MultiSelectList`，每个可见行一个子节点
（携带 `selected`）外加一个垂直滚动范围。`M` 是 `on_change` 返回的消息类型。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::MultiSelectList;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Formats(Vec<usize>) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        MultiSelectList::new(
            "formats",
            &["Lossless", "Local library", "Auto cover"],
            Msg::Formats,
        )
        .checked(&[0, 2])
        .row_height(28.0),
    );
    ui.set_rect(WidgetId::new("formats"), Rect::from_xywh(16.0, 16.0, 220.0, 160.0));
}
```

`.checked(&[usize])` 播种选择时不发出消息。默认矩形为 220×160。
`set_selection(&[usize])`、`set_checked(index, bool)` 和
`set_rows(Vec<String>)` 静默地从模型同步控件。

## 用法（XML）

`MultiSelectList` 不在标准注册表中。在 Rust 中创建它，并从文档中通过 `id` 引用：

```xml
<widget id="formats" x="16" y="16" w="220" h="160" selected="0"/>
```

`selected` 是唯一共享内容（`content_caps` 公开 `SELECTION`）；它移动键盘光标，
而不是勾选标志。行标签和勾选集合是应用数据，不是 XML 子元素——此控件未声明
`skin_children`。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, rows, on_change)` | `&'static str, &[&str], impl Fn(Vec<usize>) -> M + 'static` | — | 创建列表。 |
| `.checked(&[usize])` | `&[usize]` | 无勾选 | 创建时将行标记为勾选（不发消息）。 |
| `.check(char)` | `char` | `'\u{E73E}'` | 勾选状态的标记字形。 |
| `.square()` | — | 圆角 | 直角行复选框（半径为 0）。 |
| `.corner_radius(f32)` | `f32` | `4.0` | 行复选框半径，钳制为框的一半。 |
| `.row_height(f32)` | `f32` | `28.0` | 行高度，单位为 DIP（最小 18）。 |
| `.scrollbar_style(ScrollBarStyle)` | `ScrollBarStyle` | 默认主题滚动条 | 滚动条粗细、内边距和颜色。 |

Getter 与 setter：`selected() -> Vec<usize>`、`is_checked(index)`、
`set_checked(index, bool)`、`set_selection(&[usize])`、`set_rows(Vec<String>)`、
`corner_radius_value()`、`set_corner_radius(f32)` 和 `scrollbar()`。观察器钩子
（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、
`on_key_up`、`on_focus_gained`、`on_focus_lost`）在内置行为之前运行。

行为说明：

- `multi_selectable()` 和 `adjustable()` 为 true；Ctrl/Shift 点击以 `SelectWith` 到达。
- 在滚动条上按下是滚动条拖动，永远不会切换行。
- 列表将其行裁剪到矩形内，并在光标移动时保持其可见。

## 事件

### 消息

| 消息 | 发出时机 |
|---|---|
| 传给 `new`（`on_change`）的 `Fn(Vec<usize>) -> M` | 勾选集合发生变化；参数是完整升序的勾选索引列表。没有改变任何内容的 Shift 范围不会发出任何消息。 |

### 通用观察器

此控件安装共享观察器构建器。它们是累加的：处理器返回后，内置行为仍会运行。

| 构建器 | 处理器 | 触发条件 |
|---|---|---|
| `.on_click(f)` | `Fn(Point) -> Option<M>` | 在控件上左键点击。 |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | 右键点击，发生在上下文菜单之前。 |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | 平台双击。 |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | 控件拥有焦点时按下按键。 |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | 控件拥有焦点时释放按键。 |
| `.on_focus_gained(f)` | `Fn(FocusEvent) -> Option<M>` | 焦点移动到控件上。 |
| `.on_focus_lost(f)` | `Fn(FocusEvent) -> Option<M>` | 焦点离开控件（在编辑器提交之后）。 |

### 控件专属回调

无。构造函数的 `on_change` 消息回调是唯一的控件回调。

### Ui 触发器

任意控件 id 都可以用 `Ui::bind(id, Trigger, handler)`（或 `Ui::on_click` /
`Ui::on_activate` 简写）绑定。返回 `Some(M)` 的处理器会将消息入队；除非处理器调用
`ctx.run_default()`，否则内置操作仍会执行。

| 触发器 | 触发条件 |
|---|---|
| `Click` | 在同一控件上按下并释放。 |
| `DoubleClick` | 平台报告双击。 |
| `RightClick` | 次按键点击，发生在上下文菜单之前。 |
| `HoverEnter` / `HoverLeave` | 指针进入 / 离开控件。 |
| `Wheel` | 控件上发生滚轮事件（`event.wheel_delta`）。 |
| `Activate` | 焦点控件上按 Enter/Space，或 UIA Invoke/Toggle。 |
| `DragStart` / `DragEnd` | 指针越过拖动阈值 / 释放按键。 |
| `ValueChange` | 控件的无障碍值发生变化（在内置更新之后）。 |

点击某行会切换它，`Activate` 会切换光标所在行；`HoverEnter`/`HoverLeave` 高亮某行，
`Wheel` 滚动，`DragStart`/`DragEnd` 驱动滚动条。`ValueChange` 跟随勾选集合。

## XML 定义

此控件不由标准注册表创建。在 Rust 中创建它，并从文档中通过 `id` 引用。

- 几何：适用常规 `widget` 规则；高度决定滚动前可见的行数。
- 共享内容：`selected`（静默移动光标）。
- 本地属性：无。
- 样式 token：无 `SurfaceStyle`；`radius`、`corners` 和 `background` 不受支持
  （`xml.style.unsupported`，一个警告）。
- 子元素：无。

## 另见

- `../index.md`
- `combo-box.md`
- `checkbox.md`
- `../data/list-view.md`
- `../../guides/xml-layout-authoring.md`
