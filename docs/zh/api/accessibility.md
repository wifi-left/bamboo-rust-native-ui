# 辅助功能

> 平台无关的辅助功能模型、控件如何描述自身，以及 Windows 宿主如何通过 UI Automation 公开它。

## 概述

控件每次快照描述自身一次（`Widget::a11y`），`Ui` 将其展平为 `A11yNode` 树，Windows
平台层则通过 UI Automation（UIA）公开该树。字符串是自有的，因此快照的生命周期可以超出
绘制阶段并传入屏幕阅读器。

编写发生在两处：控件返回 `A11yInfo` 描述其角色、名称、值和已发布的子项；应用则从
`Host::a11y_nodes` 返回该快照，并用 `Ui::apply_a11y` 应用传入的 `A11yAction`，接通
宿主。UIA 提供程序仅限 Windows（`platform::uia`），但 `a11y` 中的模型以及 `A11yRole`
词汇表跨平台。

## 关键类型

### `A11yRole`

```rust
pub enum A11yRole {
    Button, Toggle, Slider, TextField, NumberField,
    RadioGroup, TabList, List, MultiSelectList,
    RadioItem, TabItem, ListItem,
    Tree, TreeItem, Grid, GridCell, ColumnHeader,
    Heading, Group, Separator, Text, ComboBox, ProgressBar,
    MenuItem, Menu, MenuBar, ToolBar, StatusBar,
}
```

每个角色映射到一个 UIA 控件类型和一组固定的模式（例如 `Button` → Invoke，`Toggle` →
Toggle，`Slider`/`ProgressBar` → RangeValue，`TreeItem` → SelectionItem +
ExpandCollapse）。`A11yRole::child_role()` 返回容器为其子项发布的角色。

### `A11yInfo`

```rust
pub struct A11yInfo {
    pub role: A11yRole,
    pub name: String,
    pub description: Option<String>,
    pub value: Option<String>,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub range: Option<(f64, f64, f64)>,
    pub read_only: bool,
    pub text_selection: Option<(usize, usize)>,
    pub children: Vec<A11yChild>,
    pub display_only: bool,
    pub expanded: Option<bool>,
    pub scroll: Option<A11yScroll>,
    pub grid: Option<(usize, usize)>,
}
```

使用 `A11yInfo::new(role, name)` 构建，另加：
`value`、`description`、`checked`、`range(min, max, value)`、
`text_selection(start, end)`、`read_only`、`children(Vec<A11yChild>)`、
`display_only()`、`expanded`、`scroll(A11yScroll)`、`grid(rows, columns)`。

`display_only()` 使装饰性内容脱离树且不进入 Tab 顺序，除非该实例自行选择加入——标题、卡片
和进度条都使用它。

### `A11yChild`

```rust
pub struct A11yChild {
    pub index: usize,
    pub name: String,
    pub selected: bool,
    pub enabled: bool,
    pub rect: Rect,
    pub offscreen: bool,
    pub checked: Option<bool>,
    pub expanded: Option<bool>,
    pub role: Option<A11yRole>,
    pub value: Option<String>,
    pub read_only: bool,
    pub row: Option<usize>,
    pub column: Option<usize>,
}
```

`A11yChild::new(index, name, rect)` 另加 `selected`、`enabled`、`offscreen`、
`checked`、`expanded`、`role`、`grid_cell(row, column)`、`value`、`editable`。
一个子项即一个选项、一个选项卡或一个可见行。

### `A11yNode` 与 `A11yScroll`

`A11yNode` 是展平后的快照元素：`id: WidgetId`、`role`、`child: Option<usize>`、`name`、
`automation_id`、`description`、`value`、`enabled`、`focusable`、`focused`、`checked`、
`selected`、`range`、`read_only`、`text_selection`、`rect`、`offscreen`、`expanded`、
`scroll`、`grid_row`、`grid_column`、`grid`、`parent`、`children`。`parent`/`children`
是 `Ui::a11y_nodes` 返回的向量中的索引。

`A11yScroll { horizontal, vertical }` 每个轴持有一个 `A11yScrollAxis { offset, viewport,
content }`。使用 `A11yScroll::new().horizontal(o, v, c)` 与 `.vertical(o, v, c)` 构建；
`scrollable()` 决定 UIA 是否声明 Scroll 模式。

### `A11yAction` 与优先级

```rust
pub struct A11yAction {
    pub kind: A11yActionKind,
    pub id: WidgetId,
    pub child: Option<usize>,
    pub value: Option<String>,
    pub number: Option<f64>,
}

pub enum A11yActionKind {
    Invoke, Toggle, Select, SetValue, Focus, Expand, Collapse,
    Scroll { horizontal: Option<f32>, vertical: Option<f32> },
}
```

`AnnouncementPriority` 为 `Polite`（默认；仅保留最新消息）或 `Assertive`（打断当前朗读）。

### 接线

| API | 签名 |
|---|---|
| `Widget::a11y` | `fn a11y(&self) -> Option<A11yInfo>` |
| `Ui::a11y_nodes` | `a11y_nodes(&self) -> Vec<A11yNode>` |
| `Ui::apply_a11y` | `apply_a11y(&mut self, action: &A11yAction, sink) -> Option<M>` |
| `Host::a11y_nodes` | `fn a11y_nodes(&mut self) -> Vec<A11yNode>` |
| `Host::a11y_action` | `fn a11y_action(&mut self, action: A11yAction, window: &mut WindowHandle<'_>)` |
| `WindowHandle::a11y_refresh` | `a11y_refresh(&mut self)` |
| `WindowHandle::a11y_focus` | `a11y_focus(&mut self, id: Option<WidgetId>)` |
| `WindowHandle::a11y_announce` | `a11y_announce(&mut self, text: impl Into<String>)` |
| `WindowHandle::a11y_announce_with` | `a11y_announce_with(&mut self, text, priority: AnnouncementPriority)` |

## 示例

编写自定义控件并为内置控件添加叙述：

```rust
use bamboo_native_ui::{A11yInfo, A11yRole, Narration, Widget, WidgetProps};

impl<M> Widget<M> for Rating {
    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Slider, "Rating")
                .value(format!("{} of 5", self.value))
                .range(0.0, 5.0, self.value as f64),
        )
    }
}

fn annotate(ui: &mut bamboo_native_ui::Ui<Msg>, sink: &mut impl bamboo_native_ui::UiHost) {
    let props = WidgetProps::new().narrate(
        Narration::new()
            .name("Play")
            .description("Starts playback of the selected track"),
    );
    ui.set_props(bamboo_native_ui::WidgetId::new("play"), props, sink);
}
```

宿主接线：

```rust
use bamboo_native_ui::{A11yAction, A11yNode, WindowHandle};

fn a11y_nodes(&mut self) -> Vec<bamboo_native_ui::A11yNode> {
    self.ui.a11y_nodes()
}

fn a11y_action(&mut self, action: A11yAction, window: &mut WindowHandle<'_>) {
    if let Some(message) = self.ui.apply_a11y(&action, window) {
        self.apply(message, window);
    }
    window.a11y_focus(self.ui.focus_id());
}
```

`apply_a11y` 把屏幕阅读器请求转换成用户本来也会产生的控件输入，因此不存在单独的代码路径。

## 说明与限制

- UIA 提供程序仅限 Windows（`#[cfg(windows)]`，`platform::uia`）。模型与角色词汇表在所有
  平台都能编译；只有提供程序和 `WindowHandle::a11y_*` 受 Windows 门控。
- `Host::a11y_nodes` 默认返回空树；未实现它的宿主不会公开任何内容。`Ui::a11y_nodes` 是
  常规实现体。
- 在页面重建或布局变化后调用 `WindowHandle::a11y_refresh`，以便下一次屏幕阅读器请求重建
  快照。`a11y_focus(Some(id))` 会在刷新后朗读焦点；`a11y_focus(None)` 会清除它。
- 告知（`a11y_announce`）是临时的，不与元素绑定；`Assertive` 会打断，`Polite` 会替换较早
  的状态文本。
- 仅发布可见的列表行，因此 600 行的队列开销很低。滚动出视野的行保留其矩形并设置
  `offscreen`。
- 模式方法从不直接接触应用：它们将 `A11yRequest` 投递回窗口，动作通过控件输入路径在 UI
  线程上运行。

## 另见

- `scrolling.md`（面板上的 Scroll 模式状态）
- `menus.md`（Menu/MenuItem 角色）
- `../widgets/controls/slider.md`
- `../widgets/data/list-view.md`
- `../overview/architecture.md`
