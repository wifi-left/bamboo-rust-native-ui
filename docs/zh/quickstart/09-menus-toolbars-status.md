# 09 — 菜单、工具栏与状态栏

> 将页面包裹在应用界面框架中：一个菜单栏、页内菜单按钮、一个
> 带溢出功能的工具栏、一条状态带和上下文菜单 —— 全部通过
> 一个命令处理器路由。

## 你将构建

一个包含绘制的 `MenuBar`、`MenuButton`、`SplitButton`、两种 `Toolbar`
方向的 `Toolbar`、一个 `StatusBar` 以及一个上下文菜单的外壳。`Msg::Command(u32)` 将
每个命令承载到 `Host::menu_command` 或 `menu_bar_command`。

## 前置条件

- [03](03-first-window.md) 中的窗口可以构建并运行。
- 无需特性开关；界面框架控件总是被编译。

## 步骤

### 1. 一条消息，一个菜单模型

命令 id 非零；一个 `Msg::Command` 和一个共享的 `Menu` 模型。

```rust
use bamboo_native_ui::{Menu, MenuItem, Submenu};

#[derive(Clone, Copy, Debug)]
enum Msg {
    Command(u32),
}

fn file_menu() -> Menu {
    Menu::new()
        .item(500, "New playlist")
        .item(501, "Open library...")
        .separator()
        .push(Submenu::new("Recent", Menu::new().item(504, "Nocturne")))
        .push(MenuItem::new(506, "Export...").shortcut("Ctrl+E"))
}
```

`item` 追加一个命令，`checked_item`/`disabled_item` 设置初始状态，
`separator` 绘制一条分隔线，`push(Submenu::new(label, menu))` 嵌套一个子菜单。

### 2. 添加绘制的菜单栏

[`MenuBar`](../widgets/chrome/menu-bar.md) 绘制顶级单元格；点击、
Enter/Space 或 `Alt+key` 将单元格的菜单打开为原生弹出菜单。

```rust
use bamboo_native_ui::widgets::MenuBar;
use bamboo_native_ui::{Rect, Ui, WidgetId};

fn add_menu_bar(ui: &mut Ui<Msg>) {
    ui.add(
        MenuBar::new("menubar", Msg::Command)
            .item("&File", file_menu())
            .item("&Edit", Menu::new().item(520, "Undo").item(521, "Redo")
                .checked_item(522, "Shuffle", false)),
    );
    ui.set_rect(WidgetId::new("menubar"), Rect::from_xywh(0.0, 0.0, 420.0, 26.0));
}
```

标签以 `&File` 或 `File(&F)` 携带其访问键；Alt 模式开启时，标记
被移除并加下划线。转发该点击：

```rust
if let Event::Alt(show) = *event {
    self.ui.set_mnemonic_underlines(show);
}
```

只有具有标记标签的界面框架控件会响应；`flush_damage` 重绘它们。

### 3. 路由命令

每个界面框架和上下文弹出菜单都通过
[`Host::menu_command`](../api/host-and-window.md) 报告；控件 id 命名该控件。

```rust
impl Host for App {
    fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>) {
        if self.ui.menu_command(widget, command, window) {
            self.ui.flush_damage(window);
            return;
        }
        self.apply(Msg::Command(command));
    }
}
```

`Ui::menu_command` 处理绘制编辑器内置的 Undo/Cut/Copy/Paste 菜单，否则
返回 `false`。`Host::menu_command` 在弹出菜单
关闭后、在其模态循环之外运行。

返回 `MenuDecision::Handled` 以取消 `Opening` 或抑制 `Selected` 重放。

```rust
use bamboo_native_ui::{MenuDecision, MenuEvent};

impl Host for App {
    fn menu_event(&mut self, event: &mut MenuEvent<'_>, _window: &mut WindowHandle<'_>) -> MenuDecision {
        if let MenuEvent::Opening { menu, .. } = event {
            menu.set_enabled(520, self.can_undo);
        }
        MenuDecision::Pass
    }
}
```

`Opening` 修改模型，`Opened` 编辑活动的弹出菜单，`Selected` 改写
命令，`Closed` 报告最终命令或 `None`。

### 4. 原生窗口菜单栏

绘制的 `MenuBar` 是一个你定位的控件。`MenuBarModel` 安装到
窗口框架中：系统绘制它、处理 `Alt`、勾选标记和
无障碍，并缩减客户区。

```rust
use bamboo_native_ui::{MenuBarModel, WindowBuilder};

let result = WindowBuilder::new(App::new())
    .menu_bar(MenuBarModel::new().item("&File", file_menu())
        .item("&Help", Menu::new().item(610, "About")))
    .run();

impl Host for App {
    fn menu_bar_command(&mut self, command: u32, _window: &mut WindowHandle<'_>) {
        self.apply(Msg::Command(command));
    }
}
```

`menu_bar_command` 没有控件 id；`MenuEvent` 首先以源 `MenuBar` 运行。

### 5. 页内菜单按钮

[`MenuButton`](../widgets/chrome/menu-button.md) 在其下方打开一个菜单；
[`SplitButton`](../widgets/chrome/split-button.md) 运行一个主操作并从其箭头
打开替代项。

```rust
use bamboo_native_ui::widgets::{MenuButton, SplitButton};

ui.add(MenuButton::new("actions", "Actions(&A)",
    Menu::new().item(700, "Play").checked_item(701, "Shuffle", false)
        .separator().submenu("Add to", Menu::new().item(702, "Queue").item(703, "Favorites")),
    Msg::Command).glyph('≡'));

ui.add(SplitButton::new("export", "Export(&E)",
    Menu::new().item(720, "MP3").item(721, "FLAC"),
    || Msg::Command(799), Msg::Command));
```

二者都打开相同的原生弹出菜单；选择会带着
按钮的 id 到达 `Host::menu_command`。

### 6. 工具栏、溢出和垂直模式

[`Toolbar`](../widgets/chrome/toolbar.md) 是一条固定宽度单元格的带子；
多余的项移入山形符溢出菜单，因此拆分不需要文本
测量。

```rust
use bamboo_native_ui::widgets::{Toolbar, ToolbarItem};

ui.add(Toolbar::new("toolbar", vec![
    ToolbarItem::new(600, "Play").glyph('▶'),
    ToolbarItem::new(601, "Pause").glyph('❚'),
    ToolbarItem::separator(),
    ToolbarItem::new(602, "Favorite").checked(true),
    ToolbarItem::new(603, "").glyph('\u{E721}').icon_only().name("Search"),
], Msg::Command).item_width(104.0));

ui.add(Toolbar::new("side", vec![
    ToolbarItem::new(620, "").glyph('▶').icon_only().name("Play"),
    ToolbarItem::new(621, "").glyph('\u{E713}').icon_only().name("Settings"),
], Msg::Command).vertical().item_height(40.0));
```

`separator()` 分隔组，不占用焦点或溢出槽位。`icon_only()`
生成一个方形单元格；给它一个 `.name`。`.checked(true)` 绘制一个切换。
`visible_count()` 报告有多少个放得下；`vertical()` 堆叠单元格并将
溢出移到底部。命令带着工具栏的 id 到达。

### 7. 状态栏

[`StatusBar`](../widgets/chrome/status-bar.md) 是一条只读的段带，
可选的进度单元格和右下角对角尺寸手柄。

```rust
use bamboo_native_ui::widgets::{StatusBar, StatusSegment};
use bamboo_native_ui::TextAlign;

ui.add(StatusBar::new("status")
    .segment(StatusSegment::new("Ready").icon('♪'))
    .segment(StatusSegment::new("600 tracks").align(TextAlign::Right).width(160.0))
    .progress(0.62).size_grip(true)
    .label("Status bar"));

let changed = ui.edit_widget::<StatusBar>(WidgetId::new("status"), |bar| bar.set_progress(0.75));
if changed { ui.flush_damage(window); }
```

带 `.width` 的段保持固定；`set_progress` 钳制到 `0..=1`。

### 8. 上下文菜单

用 [`WidgetProps::context_menu`](../api/props-and-narration.md) 附加一个菜单。
右键、Menu 键和 `Shift+F10` 打开它；命令通过
`Host::menu_command` 返回。

```rust
use bamboo_native_ui::widgets::Button;
use bamboo_native_ui::WidgetProps;

ui.add_with(Button::new("primary", "Play", Msg::Command(600)),
    WidgetProps::new().context_menu(Menu::new()
        .item(800, "Play now").item(801, "Add to queue")
        .separator().disabled_item(802, "Remove")));
```

该模型是静态的；为实时状态用
`ui.set_context_menu(WidgetId::new("primary"), Some(menu))` 重建它。

## 预期结果

- 菜单栏和两个按钮打开原生弹出菜单；选择以控件的 id 调用
  `Host::menu_command`。
- 窗口变窄时工具栏溢出到山形符菜单。
- 状态带显示段、进度和尺寸手柄。
- 右键点击按钮打开其上下文菜单。

## 故障排查

- **命令从不到达** —— 实现 `Host::menu_command`。对于框架中的
  `MenuBarModel`，改为实现 `menu_bar_command`。
- **访问键从不加下划线** —— 宿主没有转发 `Event::Alt`。
  调用 `Ui::set_mnemonic_underlines(show)`。
- **工具栏项似乎缺失** —— 它们移入了溢出。加宽该栏、
  减小 `item_width`，或检查 `visible_count()`。
- **进度段不移动** —— 在 `set_progress` 之后调用 `flush_damage(window)`。

## 下一步

[控件目录](../widgets/index.md)：每个控件、其 XML 定义及其
事件。完整的菜单 API 见[菜单](../api/menus.md)。
