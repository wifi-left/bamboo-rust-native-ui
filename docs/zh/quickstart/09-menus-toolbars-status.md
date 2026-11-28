# 09 — 菜单、工具栏与状态栏

> 将页面包裹在应用界面框架中：一个菜单栏、页内菜单按钮、一个
> 带溢出功能的工具栏、一条状态带和上下文菜单 —— 全部通过
> 一个命令处理器路由。

## 你将构建

一个外壳，包含绘制的 `MenuBar`、`MenuButton`、`SplitButton`、两个方向的 `Toolbar`、一个 `StatusBar` 和一个上下文菜单。`Msg::Command(u32)` 把每个命令送到 `Host::menu_command` 或 `menu_bar_command`。

**本节新术语**

- **命令 id** —— 一个非零整数，用来命名一个动作（“play”、“export”）。
  菜单、工具栏和上下文菜单报告 id；你的应用把它们映射到
  消息。`0` 保留给关闭。
- **命令处理器** —— 控件弹出菜单和上下文菜单用 `Host::menu_command`，
  原生框架菜单用 `Host::menu_bar_command`。前者还会收到打开
  该弹出菜单的控件 id。
- **菜单模型** —— 你用 `.item(...)` 构建的 `Menu` 值。它描述
  标签、子菜单、勾选状态和分隔符；它不是活动的 UI 对象。
- **菜单栏（绘制的）** —— `MenuBar`：一个你定位的普通控件，它
  绘制自己的单元格并打开原生弹出菜单。
- **原生菜单栏** —— `MenuBarModel`：安装在窗口框架中，由系统
  绘制，并且会缩减客户区。
- **菜单按钮 / 拆分按钮** —— 打开菜单的页内按钮；拆分按钮还有一个
  主操作。
- **工具栏** —— 一条固定宽度命令单元格的带子。放不下的单元格会移入
  山形符**溢出**菜单。
- **状态栏** —— 一条只读的段带，外加一个可选的进度单元格
  和一个缩放手柄。
- **上下文菜单** —— 附加到控件上的菜单，通过右键、Menu 键
  或 `Shift+F10` 打开。
- **访问键（助记符）** —— `&File` 中带下划线的字母，用
  Alt 激活。`Alt+F` 打开 File。
- **`MenuEvent` / `MenuDecision`** —— 对弹出菜单生命周期的钩子，以及它的
  回答：`Pass`（继续）或 `Handled`（取消/抑制）。

## 前置条件

- [03](03-first-window.md) 中的窗口可以构建并运行。
- 无需特性开关；界面框架控件总是被编译。

这些片段扩展 crate 文件夹 `bamboo-hello/bamboo-hello/` 中的
`src/main.rs`。从该文件夹或上一级的工作区文件夹运行
`cargo run`；不涉及任何新文件或特性开关。

## 步骤

### 1. 一条消息，一个菜单模型

命令 id 非零；一个 `Msg::Command` 和一个共享的 `Menu` 模型。

```rust
use bamboo_native_ui::{Menu, MenuItem, Submenu};

#[derive(Clone, Copy, Debug)]
enum Msg {
    // 一个变体承载每个命令；数字标识动作。
    Command(u32),
}

fn file_menu() -> Menu {
    Menu::new()
        // id 是你自己的编号（非零）；标签是显示文本。
        .item(500, "New playlist")
        .item(501, "Open library...")
        // 组之间仅用于显示的分隔线。
        .separator()
        // 子菜单把一个完整的 Menu 嵌套在一个标签下。
        .push(Submenu::new("Recent", Menu::new().item(504, "Nocturne")))
        // 快捷键文本绘制在原生加速键列中。
        .push(MenuItem::new(506, "Export...").shortcut("Ctrl+E"))
}
```

`item` 追加一个命令，`checked_item`/`disabled_item` 设置初始状态，
`separator` 绘制一条分隔线，`push(Submenu::new(label, menu))` 嵌套一个子菜单。

这个文件中的 id 是任意的；重要的是应用在它的 `Msg::Command`
处理器中使用相同的数字。标签永远不会和代码做匹配，因此把 "Export..." 重命名为 "Export audio..." 只改变
可见文本，不改变其他任何东西。

**刚才发生了什么。** 目前还没有绘制任何东西：`file_menu()` 只构建了一个
描述。同一个值被绘制的菜单栏、原生菜单栏和下面的菜单按钮复用，
因此 File 菜单的内容和 id 只有一个事实来源。

### 2. 添加绘制的菜单栏

[`MenuBar`](../widgets/chrome/menu-bar.md) 绘制顶级单元格；点击、
Enter/Space 或 `Alt+key` 把单元格的菜单作为原生弹出菜单打开。

```rust
use bamboo_native_ui::widgets::MenuBar;
use bamboo_native_ui::{Rect, Ui, WidgetId};

fn add_menu_bar(ui: &mut Ui<Msg>) {
    ui.add(
        // 第二个参数把选中的命令 id 映射到消息。
        MenuBar::new("menubar", Msg::Command)
            // `&` 标记访问键（Alt+F）；它不会被绘制。
            .item("&File", file_menu())
            .item("&Edit", Menu::new().item(520, "Undo").item(521, "Redo")
                .checked_item(522, "Shuffle", false)),
    );
    // 绘制的菜单栏是普通控件：它需要一个矩形。
    ui.set_rect(WidgetId::new("menubar"), Rect::from_xywh(0.0, 0.0, 420.0, 26.0));
}
```

标签以 `&File` 或 `File(&F)` 携带其访问键；Alt 模式开启时，标记
被移除并加下划线。转发该点击：

```rust
if let Event::Alt(show) = *event {
    // 按住 Alt：给访问键加下划线；松开：恢复正常。
    self.ui.set_mnemonic_underlines(show);
}
```

只有具有标记标签的界面框架控件会响应；`flush_damage` 重绘它们。

绘制的菜单栏和其他控件一样：它位于客户区内
你给它的矩形处，并且不会缩减页面其余部分可用的
空间。`MenuBar::new` 以 420×26 的矩形开始，因此忘记
`set_rect` 会让菜单栏停在左上角，并可能盖在其他控件
上面。

**刚才发生了什么。** 现在有两个顶级单元格（“File” 和 “Edit”），它们
可以打开原生弹出菜单。选择一项会通过 `Host::menu_command` 带着
这个菜单栏的 id 和命令号报告；第 3 步的路由把它变成
`Msg::Command`。菜单栏本身不知道 “Undo” 做什么。

### 3. 路由命令

每个界面框架和上下文弹出菜单都通过
[`Host::menu_command`](../api/host-and-window.md) 报告；控件 id 命名该控件。

```rust
impl Host for App {
    fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>) {
        // 先处理绘制编辑器内置的 Undo/Cut/Copy/Paste；true = 已处理。
        if self.ui.menu_command(widget, command, window) {
            self.ui.flush_damage(window);
            return;
        }
        // 其余一切都是应用命令。
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
        // Opening 在弹出菜单构建之前运行：修改 enabled/checked 状态。
        if let MenuEvent::Opening { menu, .. } = event {
            menu.set_enabled(520, self.can_undo);
        }
        // Pass 让选择通过 menu_command 重放。
        MenuDecision::Pass
    }
}
```

`Opening` 修改模型，`Opened` 编辑活动的弹出菜单，`Selected` 改写
命令，`Closed` 报告最终命令或 `None`。

**刚才发生了什么。** 现在选择一项只有一条路进入你的代码：弹出菜单
通过 `Host::menu_command` 带着控件的 id 报告，该处理器把
数字变成 `Msg::Command`，`apply` 执行动作。`MenuEvent` 是一个
可选的更早钩子，用于在用户看到菜单之前修改它。

### 4. 原生窗口菜单栏

绘制的 `MenuBar` 是一个你定位的控件。`MenuBarModel` 安装到
窗口框架中：系统绘制它、处理 `Alt`、勾选标记和
无障碍，并缩减客户区。

```rust
use bamboo_native_ui::{MenuBarModel, WindowBuilder};

let result = WindowBuilder::new(App::new())
    // 在 run() 之前安装到窗口框架中：系统绘制这条栏。
    .menu_bar(MenuBarModel::new().item("&File", file_menu())
        .item("&Help", Menu::new().item(610, "About")))
    .run();

impl Host for App {
    // 框架栏没有控件 id，因此这就是完整签名。
    fn menu_bar_command(&mut self, command: u32, _window: &mut WindowHandle<'_>) {
        self.apply(Msg::Command(command));
    }
}
```

`menu_bar_command` 没有控件 id；`MenuEvent` 先以 `MenuBar` 为源运行。

原生菜单栏属于框架，而不是你的页面：Windows 在客户区
上方绘制它并减去它的高度，因此你的 `resize` 收到的 `size`
已经排除了它。你无法用 `set_rect` 定位它，放在
`y="0"` 的控件位于它下方。第 2 步的绘制 `MenuBar` 是另一回事；
二者选其一使用。

**刚才发生了什么。** 窗口现在拥有一个系统菜单栏，带有自己的 Alt、
键盘和无障碍行为，它的命令到达一个单独的回调。`MenuEvent`
仍然先运行，`source` 会告诉你哪个菜单栏打开了弹出菜单。

### 5. 页内菜单按钮

[`MenuButton`](../widgets/chrome/menu-button.md) 在其下方打开一个菜单；
[`SplitButton`](../widgets/chrome/split-button.md) 运行一个主操作并从其箭头
打开替代项。

```rust
use bamboo_native_ui::widgets::{MenuButton, SplitButton};

// MenuButton：一个按钮；"Actions(&A)" 给出标签 "Actions" 和 Alt+A。
ui.add(MenuButton::new("actions", "Actions(&A)",
    Menu::new().item(700, "Play").checked_item(701, "Shuffle", false)
        .separator().submenu("Add to", Menu::new().item(702, "Queue").item(703, "Favorites")),
    Msg::Command).glyph('≡'));

// SplitButton：一个主操作（799）加一个打开菜单的箭头。
ui.add(SplitButton::new("export", "Export(&E)",
    Menu::new().item(720, "MP3").item(721, "FLAC"),
    || Msg::Command(799), Msg::Command));
```

二者都打开相同的原生弹出菜单；选择会带着
按钮的 id 到达 `Host::menu_command`。

拆分按钮的主半边不打开弹出菜单：它直接运行闭包，
因此普通点击发送 `Msg::Command(799)`，只有箭头才询问
格式。两个控件仍然需要来自 `resize` 或文档的矩形，和
任何其他控件一样；没有矩形时它们保留构造函数的矩形，可能
重叠页面。

**刚才发生了什么。** 两个控件都复用第 1 步的菜单值。菜单按钮
通过 `Host::menu_command` 报告 `700`-`703`；拆分按钮的
主半边立即报告 `799`，它的箭头报告 `720` 或 `721`。

### 6. 工具栏、溢出和垂直模式

[`Toolbar`](../widgets/chrome/toolbar.md) 是一条固定宽度单元格的带子；
多余的项移入山形符溢出菜单，因此拆分不需要文本
测量。

```rust
use bamboo_native_ui::widgets::{Toolbar, ToolbarItem};

ui.add(Toolbar::new("toolbar", vec![
    // 每个单元格固定宽度；多余的移入山形符溢出。
    ToolbarItem::new(600, "Play").glyph('▶'),
    ToolbarItem::new(601, "Pause").glyph('❚'),
    // 分隔符不占焦点，也不占溢出槽位。
    ToolbarItem::separator(),
    // checked(true) 让该项初始绘制为已勾选的切换。
    ToolbarItem::new(602, "Favorite").checked(true),
    // icon_only() 生成方形单元格；name 是它的无障碍标签。
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

工具栏不会换行：可用宽度用完时，整个单元格移入
溢出菜单，因此一个单元格要么完全可见，要么在山形符中。
`vertical()` 用高度而不是宽度做同样的判断。两个
工具栏都需要一个矩形；`item_width`/`item_height` 设置单元格尺寸，而不是
栏的尺寸。

**刚才发生了什么。** 水平带子布局尽可能多的 104 DIP 单元格，
把其余的放在山形符后面。垂直带子向下做同样的事。
选择任何单元格（可见的或溢出的）都会产生带
该单元格 id 的 `Msg::Command`。

### 7. 状态栏

[`StatusBar`](../widgets/chrome/status-bar.md) 是一条只读的段带，
可选的进度单元格和右下角对角尺寸手柄。

```rust
use bamboo_native_ui::widgets::{StatusBar, StatusSegment};
use bamboo_native_ui::TextAlign;

ui.add(StatusBar::new("status")
    // 一个带字形和文本的段。
    .segment(StatusSegment::new("Ready").icon('♪'))
    // 一个固定宽度、右对齐的段。
    .segment(StatusSegment::new("600 tracks").align(TextAlign::Right).width(160.0))
    // 初始进度和右下角的尺寸手柄。
    .progress(0.62).size_grip(true)
    // 该带的无障碍名称（默认为控件 id）。
    .label("Status bar"));

// set_progress 钳制到 0..=1，并返回是否改变了任何东西。
let changed = ui.edit_widget::<StatusBar>(WidgetId::new("status"), |bar| bar.set_progress(0.75));
if changed { ui.flush_damage(window); }
```

带 `.width` 的段保持固定；`set_progress` 钳制到 `0..=1`。

状态栏是只读的：它没有命令 id，也不报告任何消息。
进度是推入的应用状态，不是用户可以拖动的滑块。和
本页其他控件一样，状态栏需要先有矩形，才能出现在你预期的
位置。

**刚才发生了什么。** 这条带绘制了两个段、一个 62% 的进度单元格和
手柄，然后 `set_progress(0.75)` 移动了填充，并通过 `flush_damage`
调度一次重绘。

### 8. 上下文菜单

用 [`WidgetProps::context_menu`](../api/props-and-narration.md) 附加一个菜单。
右键、Menu 键和 `Shift+F10` 打开它；命令通过
`Host::menu_command` 返回。

```rust
use bamboo_native_ui::widgets::Button;
use bamboo_native_ui::WidgetProps;

ui.add_with(Button::new("primary", "Play", Msg::Command(600)),
    // 菜单通过 props 附加，而不是通过按钮类型。
    WidgetProps::new().context_menu(Menu::new()
        .item(800, "Play now").item(801, "Add to queue")
        .separator().disabled_item(802, "Remove")));
```

该模型是静态的；需要实时状态时用
`ui.set_context_menu(WidgetId::new("primary"), Some(menu))` 重建它。

按钮自己的点击仍然发送 `Msg::Command(600)`；上下文菜单用
按钮的 id 通过 `Host::menu_command` 发送
`800`、`801` 或 `802`。两条路径互不干扰，这就是为什么上下文菜单可以在
不改变主操作的情况下提供替代项。

**刚才发生了什么。** 按钮现在会用原生弹出菜单响应右键（或 Menu 键，
或 `Shift+F10`），而普通点击保留其原始
消息。关闭菜单不发送任何内容。

## 预期结果

- 菜单栏和两个按钮打开原生弹出菜单；选择以控件的 id 调用
  `Host::menu_command`。
- 窗口变窄时工具栏溢出到山形符菜单。
- 状态带显示段、进度和尺寸手柄。
- 右键点击按钮打开其上下文菜单。

**运行它。** 从工作区文件夹 `bamboo-hello/` 或 crate 文件夹
`bamboo-hello/bamboo-hello/` 运行：

```powershell
cargo run
```

窗口出现，界面框架就位：顶部是菜单栏，页内
按钮、两个工具栏和底部的状态带。用
鼠标或 `Alt+F` 打开 File；选择一项会到达你的处理器。在你接线时想让结果
更明显，可以在 `apply` 中打印命令，或把它写进
一个标签。

## 故障排查

- **窗口打开时没有界面框架** —— 控件被添加了但从未
  被赋予矩形。每个界面框架控件都会保留构造函数的矩形，直到
  `set_rect`（来自 `resize`）或文档移动它，因此栏和工具栏可能
  堆叠在原点并互相遮挡。
- **命令从不到达** —— 实现 `Host::menu_command`。对于框架中的
  `MenuBarModel`，改为实现 `menu_bar_command`。
- **控件菜单能用但原生栏什么都不做** —— 它们使用不同的
  回调。绘制的 `MenuBar`、`MenuButton`、`SplitButton`、工具栏和
  上下文菜单报告给 `menu_command(widget, command)`；只有框架
  `MenuBarModel` 报告给 `menu_bar_command(command)`。实现一个并不
  覆盖另一个，缺少回调会丢弃命令。
- **程序在构建菜单时 panic** —— 某个命令 id 是 `0`。
  `Menu::item(0, ...)` 在调试构建中会 panic，因为 `0` 保留给
  关闭；从 1 开始给你的命令编号。
- **访问键从不加下划线** —— 宿主没有转发 `Event::Alt`。
  调用 `Ui::set_mnemonic_underlines(show)`。
- **工具栏项似乎缺失** —— 它们移入了溢出。加宽该栏、
  减小 `item_width`，或检查 `visible_count()`。一个单元格永远不会
  只显示一半。
- **绘制的菜单栏不在你预期的位置** —— 它是一个控件；用 `set_rect`
  给它一个矩形。否则它会保留其在客户区原点的 420×26 构造函数
  矩形。
- **原生菜单栏不出现** —— 它必须在 `.run()` 之前通过 `.menu_bar(...)`
  安装在 `WindowBuilder` 上。添加一个绘制的 `MenuBar` 控件是另一种菜单；
  它不会变成框架栏。
- **某个菜单项永久变灰** —— 该项用 `disabled_item` 构建，
  或 `MenuEvent::Opening` 调用了 `set_enabled(false)`。检查
  那里使用的实时条件（示例中的 `self.can_undo`）。
- **上下文菜单不打开** —— 控件被禁用、它的矩形为空或
  在屏幕外（因此点击落在它外面），或它的 props 从未
  用 `add_with` 注册。右键、Menu 键和 `Shift+F10` 都使用
  同一个模型。
- **进度段不移动** —— 在 `set_progress` 之后调用 `flush_damage(window)`。
  构建器 `.progress(0.62)` 只设置初始值。

## 下一步

[控件目录](../widgets/index.md)：每个控件、其 XML 定义及其
事件。完整的菜单 API 见[菜单](../api/menus.md)。
