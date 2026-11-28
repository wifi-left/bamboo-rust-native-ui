# 辅助功能

> 为每个控件描述一次，从应用覆盖名称和描述，
> 接入 UIA provider，并用 `--a11y-dump` 验证树。

## 模型如何工作

1. 控件从 `Widget::a11y` 返回 `A11yInfo`：角色、名称、值、范围、
   选中状态、发布的子项以及可选的滚动/网格状态。
2. `Ui::a11y_nodes()` 将每个可见控件展平为 `Vec<A11yNode>` 并
   应用应用的属性（角色覆盖、朗读、暴露、
   可见性、焦点）。
3. Windows 宿主从 `Host::a11y_nodes` 返回该向量；平台
   层通过 UI Automation 暴露它。
4. 一个屏幕阅读器请求会以 `A11yAction` 返回，`Ui::apply_a11y`
   将其转成与用户操作相同的输入。

该模型是平台中立的；只有 UIA provider 和
`WindowHandle::a11y_*` 仅限 Windows。

## 步骤

### 1. 为每个控件指定角色和名称

内置控件已经这样做了。自定义控件返回一个 `A11yInfo`：

```rust
use bamboo_native_ui::{A11yInfo, A11yRole, Widget};

impl<M> Widget<M> for Rating {
    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Slider, "Rating")
                .value(format!("{} of 5", self.value))
                .range(0.0, 5.0, f64::from(self.value)),
        )
    }
}
```

从词汇中选择角色（`Button`、`Toggle`、`Slider`、
`TextField`、`NumberField`、`RadioGroup`、`TabList`、`List`、
`MultiSelectList`、`RadioItem`、`TabItem`、`ListItem`、`Tree`、`TreeItem`、
`Grid`、`GridCell`、`ColumnHeader`、`Heading`、`Group`、`Separator`、`Text`、
`ComboBox`、`ProgressBar`、`MenuItem`、`Menu`、`MenuBar`、`ToolBar`、
`StatusBar`）。角色决定 UIA 控件类型以及所提供的模式
（例如 `Button` → Invoke、`Toggle` → Toggle、`Slider` → RangeValue、
`TreeItem` → SelectionItem + ExpandCollapse）。

完整的 `A11yInfo` 构建器集合：

| 构建器 | 含义 |
|---|---|
| `new(role, name)` | 角色和无障碍名称。 |
| `.value(text)` | 当前值（编辑器、滑块、进度）。 |
| `.description(text)` | UIA HelpText / FullDescription。 |
| `.checked(bool)` | 复选框/切换状态。 |
| `.range(min, max, value)` | RangeValue 范围和位置。 |
| `.text_selection(start, end)` | 已关闭文本字段中 `value` 的字节偏移。 |
| `.read_only(bool)` | Value 模式报告为只读。 |
| `.children(Vec<A11yChild>)` | 选项、标签页或可见行。 |
| `.display_only()` | 装饰：不进树，也不进 Tab，除非显式选择加入。 |
| `.expanded(bool)` | ExpandCollapse 状态（菜单、树行、手风琴标题）。 |
| `.scroll(A11yScroll)` | Scroll 模式状态。 |
| `.grid(rows, columns)` | 网格范围；单元格是子项。 |

`A11yInfo::new` 默认 `read_only` 为 `false`，因此接受
输入的编辑器不需要额外调用；只读文本块设置
`.read_only(true)`。

### 2. 为复合控件发布子元素

一个 `A11yChild` 就是一个选项、标签页或可见行。索引是控件级
索引，因此滚动的列表保持绝对行号。

```rust
use bamboo_native_ui::{A11yChild, A11yInfo, A11yRole, Rect};

fn tabs_info(labels: &[&str], selected: usize, rect: Rect) -> A11yInfo {
    let children = labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let cell = Rect::from_xywh(
                rect.left + index as f32 * 80.0,
                rect.top,
                80.0,
                rect.height(),
            );
            A11yChild::new(index, *label, cell).selected(index == selected)
        })
        .collect();
    A11yInfo::new(A11yRole::TabList, "Views").children(children)
}
```

`A11yChild` 构建器：`.selected`、`.enabled`、`.offscreen`、`.checked`、
`.expanded`、`.role`（覆盖容器的子角色）、`.grid_cell(row,
column)`、`.value`、`.editable`。只发布已实现的行，并将
预读行标记为 `.offscreen(true)`；600 行的队列也能保持低成本。

对于滚动容器，发布 `A11yScroll`：

```rust
use bamboo_native_ui::{A11yInfo, A11yRole, A11yScroll};

fn list_info(offset: f32, viewport: f32, content: f32) -> A11yInfo {
    A11yInfo::new(A11yRole::List, "Library")
        .scroll(A11yScroll::new().vertical(offset, viewport, content))
}
```

UIA 只在一个轴实际可以移动时才宣告 Scroll 模式。

### 3. 从应用使用 `WidgetProps` 覆盖

朗读改变屏幕阅读器所说的内容，而不改变控件
绘制的文本。

```rust
use bamboo_native_ui::{Narration, Ui, UiHost, WidgetId, WidgetProps};

fn describe_play(ui: &mut Ui<()>, sink: &mut impl UiHost) {
    let props = WidgetProps::new().narrate(
        Narration::new()
            .name("Play")
            .description("Starts playback of the selected track"),
    );
    ui.set_props(WidgetId::new("play"), props, sink);
}
```

| 构建器 | 效果 |
|---|---|
| `Narration::name(text)` | 固定的无障碍名称。 |
| `Narration::name_with(f)` | 由当前 `A11yInfo` 构建名称（例如实时计数器）。 |
| `Narration::description(text)` | 固定描述。 |
| `Narration::description_with(f)` | 由当前信息构建描述。 |
| `Narration::row(f)` | 重命名复合控件的子项（`Fn(usize, &A11yChild) -> String`）。 |
| `WidgetProps::role(role)` | 覆盖控件自身的角色。 |
| `WidgetProps::automation_id(id)` | 稳定的 UIA `AutomationId`。 |
| `WidgetProps::expose(bool)` | 强制将仅显示控件加入或移出树。 |

`display_only()` 控件不进树，也不进 Tab，除非它是
可聚焦、可交互、被朗读、被覆盖角色或 `expose(true)`。所以
标题、卡片和进度条不会每次 Tab 都被朗读。

### 4. 接入宿主

```rust
use bamboo_native_ui::{A11yAction, A11yNode, Host, WindowHandle};

impl Host for App {
    fn a11y_nodes(&mut self) -> Vec<A11yNode> {
        self.ui.a11y_nodes()
    }

    fn a11y_action(&mut self, action: A11yAction, window: &mut WindowHandle<'_>) {
        if let Some(message) = self.ui.apply_a11y(&action, window) {
            self.apply(message, window);
        }
        window.a11y_focus(self.ui.focus_id());
    }
}
```

要有可用的树，两个钩子都必需。默认的 `a11y_nodes` 返回
空向量，默认的 `a11y_action` 什么也不做：只读检查
可以工作，但 Invoke/Toggle/Select/SetValue 什么也不做。

在页面重建或布局应用之后，调用 `WindowHandle::a11y_refresh()`，以便
下一次 provider 请求重建快照。`a11y_focus(Some(id))`
宣告焦点变化；`a11y_focus(None)` 清除它。

`Ui::apply_a11y` 处理 `Invoke`、`Toggle`、`Select`、`SetValue`、`Focus`、
`Expand`、`Collapse` 和 `Scroll { horizontal, vertical }`。它通过
控件的常规输入路径路由，因此应用消息和观察者会
与鼠标或键盘完全一样地触发。

### 5. 焦点与快捷键模式

- 首选：让控件可聚焦，并让 Tab 能到达它。使用
  `Ui::set_focusable(id, true, sink)` 或 `WidgetProps::focusable(true)`；
  `Ui::set_focus_order(id, weight)` 设置顺序（较小的在前，相同则保持
  插入顺序）。
- 聚焦控件上的 Enter/Space 变为 `WidgetInput::Activate`。UIA 的
  Invoke/Toggle 以相同的输入到达，因此不需要第二条代码路径。
- 复合控件（列表、标签页、树）拥有方向键导航并发布
  选择变化；不要添加并行的键盘处理器。
- 窗口装饰标签（菜单栏、菜单按钮、工具栏、拆分按钮）将 `&` 解析为
  访问键：`"&File"` 绘制 `File` 并响应 `Alt+F`；`"&&"` 是字面
  和号。普通控件使用 Tab 加 Space/Enter，而不是访问键。
- 窗口快捷键（`register_window_hotkey`）仅在窗口拥有
  焦点时触发；全局热键（`register_hotkey`）是全系统范围的。两者都在
  [键码与热键](../reference/key-codes-and-hotkeys.md)中介绍。
- `Ui::bind(id, Trigger::Activate, handler)` 在控件自身行为之外
  观察激活；返回 `Some(message)` 以排队一条消息，或调用
  `ctx.run_default()` 来自己运行内置操作。

### 6. 宣告状态和错误

```rust
use bamboo_native_ui::{AnnouncementPriority, WindowHandle};

fn imported(count: usize, window: &mut WindowHandle<'_>) {
    window.a11y_announce(format!("{count} files imported"));
}

fn failed(reason: &str, window: &mut WindowHandle<'_>) {
    window.a11y_announce_with(reason, AnnouncementPriority::Assertive);
}
```

`Polite`（默认）只保留最新消息，适合状态 tick
和重复的 toast；`Assertive` 为错误和破坏性操作中断
当前朗读。宣告是临时的，不与
元素关联。

### 7. 检查树

```powershell
cargo run -p bamboo-native-ui-showcase -- --a11y-dump
```

`--a11y-dump` 在工作线程上运行一个真实的 UIA **客户端**：它向窗口
请求其 provider，遍历控件视图并打印名称、角色、模式、
值和选择。如果探针失败，它以非零退出，因此它既是 CI 检查
也是调试工具。

对于交互式检查，请使用 Windows SDK 工具（Inspect、Accessibility
Insights for Windows）针对正在运行的应用。验证与
dump 相同的项目：

- 每个控件都有非空名称和预期的控件类型；
- 按钮暴露 Invoke，切换暴露 Toggle，滑块暴露 RangeValue，列表
  暴露 Selection + Scroll，表格暴露 Grid + GridItem；
- 聚焦元素正是应用认为已聚焦的那个；
- 选择变化引发 `SelectionItem_ElementSelected`，切换状态
  变化引发 `ToggleState`（dump 监听两者）；
- 滚出视图的行报告 `IsOffscreen`，而不是消失。

## 常见错误

- **空树** —— 未实现 `Host::a11y_nodes`。
- **可读但不可操作** —— 未实现 `Host::a11y_action`，或
  控件的角色没有为该操作提供任何模式。
- **装饰在每次 Tab 时都被朗读** —— 控件是 `display_only`，但
  应用让它可聚焦或被朗读；去掉该选择加入或设置
  `expose(false)`。
- **重建后名称陈旧** —— 在重建之后调用 `a11y_refresh()`。
- **滚动后朗读了错误的行** —— 子索引必须是
  控件级索引，而不是视口索引。
- **列表检查缓慢** —— 只发布已实现的行外加一个
  预读行；将它标记为 `offscreen`。
- **`--a11y-dump` 只在 CI 中失败** —— 桌面会话必须允许一个窗口；
  请交互运行它，或在有交互式桌面的会话中运行。

## 另请参阅

- [辅助功能 API](../api/accessibility.md)
- [Widget trait](../api/widget-trait.md)
- [Host 与窗口](../api/host-and-window.md)
- [滚动](../api/scrolling.md)
- [XML 元素](../reference/xml-elements.md)（`role`、`name`、`description`、`automation-id`、`expose`）
- [键码与热键](../reference/key-codes-and-hotkeys.md)
- [调试](debugging.md)
