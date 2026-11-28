# TreeView

> 虚拟化、可展开的层次结构，支持重命名和拖放移动。

## 介绍

`TreeView<M>` 将可展开的 `TreeItem` 层次结构扁平化为可见行，并且只绘制与视口相交的那些行。一行可以显示展开/折叠箭头、可选的前导字形、其标签和弱化的辅助文本。分支行在结构变化时扁平化，而非在绘制时，因此即使是很深的树，每帧也只需一个视口的工作量。

消息携带节点的**路径**（`&[usize]`），而非其标签：`[0, 2, 1]` 表示第一个根节点的第三个子节点的第二个子节点。只要节点存在，路径在重建后仍然有效；选择、激活、切换展开、重命名和拖动移动都会报告路径。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::tree::TreeDragRules;
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::{TreeItem, TreeView};

enum Msg {
    Select(Vec<usize>),
    Open(Vec<usize>),
    Toggle(Vec<usize>, bool),
    Rename(Vec<usize>, String),
    Move(Vec<usize>, Vec<usize>),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    TreeView::new("library")
        .items(vec![TreeItem::new("Music").expanded(true).child(
            TreeItem::new("Albums")
                .glyph('\u{E8FD}')
                .expanded(true)
                .child(TreeItem::new("Nocturne").glyph('\u{E8D6}').secondary("3:32"))
                .child(TreeItem::new("Bamboo").glyph('\u{E8D6}').secondary("4:05")),
        )])
        .row_height(28.0)
        .indent(18.0)
        .renameable(true)
        .drag_rules(TreeDragRules::new().nest(false))
        .on_select(|path: &[usize]| Msg::Select(path.to_vec()))
        .on_activate(|path: &[usize]| Msg::Open(path.to_vec()))
        .on_toggle(|path: &[usize], open| Msg::Toggle(path.to_vec(), open))
        .on_rename(|path: &[usize], label| Msg::Rename(path.to_vec(), label.to_string()))
        .on_move(|from: &[usize], to: &[usize]| Msg::Move(from.to_vec(), to.to_vec())),
);
ui.set_rect(WidgetId::new("library"), Rect::from_xywh(16.0, 56.0, 320.0, 280.0));
```

`TreeItem::child` 追加一个子树，`children` 替换该列表；`on_move` 运行时，被移动的节点已经就位；`on_rename` 运行时，已提交的重命名也已应用，因此回调只需持久化。

## 用法（XML）

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。嵌套的 `<node>` 元素会替换层次结构：

```xml
<widget id="library" x="16" y="56" w="320" h="280"
        row-height="28" indent="18" renameable="true" draggable="true"
        selected="1">
  <node label="Music" glyph="&#xE8FD;" expanded="true">
    <node label="Albums" expanded="true">
      <node label="Nocturne" glyph="&#xE8D6;" secondary="3:32"/>
      <node label="Bamboo" glyph="&#xE8D6;" secondary="4:05"/>
    </node>
    <node label="Playlists" disabled="true"/>
  </node>
</widget>
```

共享的 `selected` 属性是**可见行索引**（不是路径），在子元素列表之后应用；它只移动光标。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id)` | `&'static str` | — | 创建行高为 28 DIP 的空树。 |
| `.items(Vec<TreeItem>)` | items | empty | 替换层次结构（静默）。 |
| `.row_height(f32)` | `f32` | `28.0` | 行高，限制为 `>= 16`。 |
| `.indent(f32)` | `f32` | `18.0` | 每层深度的缩进，限制为 `>= 8`。 |
| `.chevrons(collapsed, expanded)` | `char, char` | MDL2 `U+E76C`、`U+E70D` | 箭头标记。 |
| `.empty_text(impl Into<String>)` | text | `""` | 没有可见行时的消息。 |
| `.renameable(bool)` | `bool` | `false` | 启用 F2 就地标签编辑。 |
| `.draggable(bool)` | `bool` | `true` | 启用按下拖动移动。 |
| `.drag_rules(TreeDragRules)` | rules | all allowed | 限制放置类型。 |
| `.on_select(impl Fn(&[usize]) -> M)` | path handler | `None` | 某一行被选中。 |
| `.on_activate(impl Fn(&[usize]) -> M)` | path handler | `None` | 在某一行上按 Enter 或双击。 |
| `.on_toggle(impl Fn(&[usize], bool) -> M)` | path, expanded | `None` | 某节点被展开或折叠。 |
| `.on_rename(impl Fn(&[usize], &str) -> M)` | path, label | `None` | 标签被就地提交。 |
| `.on_move(impl Fn(&[usize], &[usize]) -> M)` | from, to | `None` | 一次拖动移动了节点（已应用）。 |
| `.on_drag_start(impl Fn(&[usize]) -> bool)` | path | `None` | 否决此手势；`false` 会让按下保持为点击。 |
| `.on_drag_over` / `.on_drag_end` | `impl Fn(&[usize], &[usize]) -> bool` | `None` | 过滤实时目标或取消释放。 |
| `.scrollbar_style(ScrollBarStyle)` | style | theme default | 滚动条外观。 |

`TreeDragRules::new()` 允许所有放置类型；`.reorder(bool)`、`.nest(bool)`、`.detach(bool)` 和 `.reparent(bool)` 会限制它们，`.allows(TreeDropKind)` 会查询策略（`Reorder`、`Nest`、`Detach` 或 `Reparent`）。

类型化访问和运行时 setter：

| 方法 | 类型 | 描述 |
|---|---|---|
| `.items_ref()` / `.item(path)` / `.visible_row_count()` / `.visible_rows()` | mixed | 层次结构、单个节点、扁平化计数、视口范围。 |
| `.is_expanded(path)` / `.set_expanded(path, bool)` | `bool` | 静默读取或更改展开状态。 |
| `.selected_path()` / `.set_selection_path(path)` / `.selected_row()` | mixed | 按路径或行索引的键盘光标。 |
| `.row_label(row)` / `.row_rect(row)` | `Option<&str>` / `Rect` | 可见行的标签和客户区矩形。 |
| `.rename(path, label)` / `.begin_rename(path)` / `.is_renaming()` | mixed | 无会话重命名或打开一个会话。 |
| `.move_item(from, parent, index)` | `bool` | 在没有指针的情况下移动节点。 |
| `.set_items`、`.set_row_height`、`.set_indent`、`.set_renameable`、`.set_draggable`、`.set_drag_rules`、`.set_empty_text` | mixed | 运行时 setter。 |
| `.scroll_to_row`、`.ensure_row_visible`、`.scroll_offset` | mixed | 滚动。 |

行为：Up/Down 移动光标并跳过禁用行；PageUp/PageDown 移动一页；Home/End 跳到边界。Right 展开或进入分支；Left 折叠或上溯到父节点。Enter 激活，空格切换，F2 重命名（Enter/Tab 提交，Escape 还原）。按下拖动移动 6 DIP 后生效：上/下四分之一处插入到前/后，中间则嵌套，离开视口会自动滚动。角色为 `Tree`。

## 事件

### 消息

五个回调构建应用程序的 `M`；三个拖动钩子返回 `bool` 用于否决，而不发出消息。全部在构建器上设置；`new` 只接收 id。

| 消息 | 发出时机 |
|---|---|
| 传给 `.on_select` 的 `Fn(&[usize]) -> M` | 某一行成为光标（点击或键盘）。 |
| 传给 `.on_activate` 的 `Fn(&[usize]) -> M` | 在已选中/被点击的节点上按 Enter 或双击。 |
| 传给 `.on_toggle` 的 `Fn(&[usize], bool) -> M` | 某节点被展开（`true`）或折叠（`false`）。 |
| 传给 `.on_rename` 的 `Fn(&[usize], &str) -> M` | 就地标签编辑被提交（已应用）。 |
| 传给 `.on_move` 的 `Fn(&[usize], &[usize]) -> M` | 一次拖动移动了节点（已应用）。 |

### 通用观察器

该控件不安装共享的观察器构建器（`on_click`、`on_right_click`、`on_double_click`、`on_key_down`、`on_key_up`、`on_focus_gained`、`on_focus_lost`）。下面的回调是它的事件接缝；其他任何内容都通过 `Ui::bind` 处理。

### 控件专属回调

路径是从根开始的 `&[usize]`，例如 `[0, 2, 1]`；除拖动否决外，每个处理器都返回消息。

| 方法 | 处理器 | 触发时机 |
|---|---|---|
| `.on_select(handler)` | `impl Fn(&[usize]) -> M + 'static` | 光标通过指针或键盘移动到某一行。 |
| `.on_activate(handler)` | `impl Fn(&[usize]) -> M + 'static` | 按 Enter、UIA invoke，或双击某一行。 |
| `.on_toggle(handler)` | `impl Fn(&[usize], bool) -> M + 'static` | 箭头点击、Left/Right、空格或 UIA 展开请求改变了某节点。 |
| `.on_rename(handler)` | `impl Fn(&[usize], &str) -> M + 'static` | F2 会话提交；控件会先应用标签。 |
| `.on_move(handler)` | `impl Fn(&[usize], &[usize]) -> M + 'static` | 拖放已应用（`from`、`to`）。 |
| `.on_drag_start(handler)` | `impl Fn(&[usize]) -> bool + 'static` | 指针越过拖动阈值；`false` 会让按下保持为点击。 |
| `.on_drag_over(handler)` | `impl Fn(&[usize], &[usize]) -> bool + 'static` | 预期的放置目标发生变化；`false` 将其标记为无效。 |
| `.on_drag_end(handler)` | `impl Fn(&[usize], &[usize]) -> bool + 'static` | 在有效目标上释放，在树变化之前；`false` 取消放置。 |

### Ui 触发器

任何控件 id 都可以通过 `Ui::bind` 进行绑定：

```rust
ui.bind(WidgetId::new("library"), Trigger::DragEnd, |event, ctx| {
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

它响应 `Click`（选择）、`DoubleClick`/`Activate`（激活）、`Wheel`（滚动）以及 `ValueChange`（选择）。移动响应按下/移动/释放输入，而非拖动触发器，`RightClick` 没有内置效果。

## XML 定义

该控件不由标准注册表创建。请在 Rust 中创建它，并在文档中通过 `id` 引用。

几何布局遵循常规的 `<widget>` 规则。通用属性适用，包括 `selected`（可见行索引）和 `read-only`（锁定重命名）。`font-size`/`font-family` 属于绘制层。树从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported` 并跳过。

| 属性 | 类型 | 描述 |
|---|---|---|
| `row-height` | number | 行高；必须 `>= 16`。 |
| `indent` | number | 每层缩进；必须 `>= 8`。 |
| `renameable` | bool | 启用 F2 重命名。 |
| `draggable` | bool | 启用按下拖动移动。 |
| `empty-text` | text | 树没有可见行时的消息。 |

子元素：

| 元素 | 属性 | 描述 |
|---|---|---|
| `<node>` | `label`（必需，非空）、`glyph`（单个字符）、`secondary`、`expanded`（bool）、`disabled`（bool） | 一个节点；`<node>` 子元素会递归嵌套。 |

子元素列表属于内容（在生成变化时写入）；回调保留在 Rust 中。

## 另见

- `table.md`
- `list-view.md`
- `../index.md`
- `../../api/widget-trait.md`
