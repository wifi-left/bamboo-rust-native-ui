# 菜单

> 声明式弹出菜单、原生菜单栏模型，以及观察每次菜单生命周期的钩子。

## 概述

`Menu` 描述菜单项、分隔符和子菜单。在 Windows 上，宿主将其展平为原生 `HMENU` 并使用
`TrackPopupMenu` 显示，因此选择会以菜单项的命令 id 通过 `Host::menu_command` 返回——应用
从不接触 `WM_COMMAND`。命令是应用定义的正整数 id；`0` 保留给被取消的弹出菜单。

`MenuBarModel` 建模系统绘制在标题栏下方的原生窗口菜单栏。`MenuEvent` 是框架打开的每个
弹出菜单（上下文菜单、chrome 菜单、托盘菜单、程序化打开以及菜单栏）的通用生命周期钩子。

`Menu`、`MenuItem`、`MenuEntry`、`Submenu`、`MenuPath` 和 `MnemonicLabel` 模型是跨平台的
纯数据；原生弹出菜单、系统菜单栏以及 `WindowHandle` 方法仅限 Windows。

## 关键类型

### `Menu`

```rust
pub struct Menu { /* private */ }

impl Menu {
    pub fn new() -> Self;
}
```

构建方法（消耗 `self`）：

| 方法 | 签名 | 描述 |
|---|---|---|
| `item` | `item(self, command: u32, label: impl Into<String>) -> Self` | 一个普通菜单项。 |
| `shortcut_item` | `shortcut_item(self, command, label, shortcut) -> Self` | 带有原生右列加速键文本的菜单项。 |
| `checked_item` | `checked_item(self, command, label, checked: bool) -> Self` | 镜像某个开关状态的菜单项。 |
| `disabled_item` | `disabled_item(self, command, label) -> Self` | 可见但不可选。 |
| `separator` | `separator(self) -> Self` | 一条水平分隔符。 |
| `submenu` | `submenu(self, label, menu: Menu) -> Self` | 一个嵌套菜单。 |
| `push` | `push(self, entry: impl Into<MenuEntry>) -> Self` | 追加 `MenuItem`、`Submenu` 或 `MenuEntry`。 |

运行时编辑（接收 `&mut self`）：`insert(index, entry)`、`remove(index)`、
`remove_command(command)`、`item_mut(command)`、`set_label`、`set_checked`、
`set_enabled`、`set_command(old, new)`、`submenu_at(&MenuPath)`、
`submenu_at_mut`、`entry_mut(&MenuPath)`。读取方法：`entries()`、`len()`、
`is_empty()`、`command(u32) -> Option<&MenuItem>`、`visit_items(&mut impl
FnMut(&MenuItem))`。

### `MenuItem` 与 `MenuIcon`

```rust
pub struct MenuItem { /* private */ }
impl MenuItem { pub fn new(command: u32, label: impl Into<String>) -> Self; }
```

`new` 断言 `command != 0`。构建方法：`checked(bool)`、`disabled(bool)`、
`shortcut(text)`、`icon(MenuIcon)`。设置方法：`set_label`、`set_shortcut`、
`set_icon`、`set_checked`、`set_disabled`、`set_command`（忽略 `0`）。读取方法包括
`command()`、`label()`、`shortcut_text() -> Option<&str>`、
`hotkey() -> Option<Hotkey>`（解析加速键文本）、`icon_image()`、
`is_checked()`、`is_disabled()`。

`MenuIcon::from_rgba(width: u16, height: u16, rgba: impl Into<Vec<u8>>)` 根据自顶向下的
RGBA 构建一个惯例为 16×16 的位图；读取方法为 `width`、`height`、`rgba`。

### `MenuEntry` 与 `Submenu`

`MenuEntry` 为 `Item(MenuItem) | Separator | Submenu(Submenu)`，辅助方法有
`MenuEntry::item(command, label)`、`separator()`、`submenu(label, menu)`、
`disabled(bool)`、`checked(bool)`、`set_label`、`command()`。`Submenu::new(label,
menu)` 另加 `disabled(bool)`、`label()`、`menu()`、`is_disabled()`。

### `MenuPath` / `MenuStep`

`MenuPath::root()`（或 `new()`）、`.index(usize)`、`.label(&str)`、`.then(step)`、
`.steps()`、`.is_root()`。`MenuStep` 为 `Index(usize)` 或 `Label(String)`。

### `MenuBarModel`

```rust
pub struct MenuBarModel { /* private */ }

impl MenuBarModel {
    pub fn new() -> Self;
    pub fn item(self, label: impl Into<String>, menu: Menu) -> Self;
}
```

还有 `push(entry)`、`separator()`、`entries()`、`len()`、`is_empty()`、
`insert(index, label, menu)`、`remove(index)`、`set_menu(index, menu)`、
`item_mut(command)`、`set_checked`、`set_enabled`、`set_label`、
`mnemonic_index(ch) -> Option<usize>`、`mnemonic_indices(ch) -> Vec<usize>`，以及
`hotkeys() -> Vec<(Hotkey, u32)>`（能解析为组合键的已启用菜单项加速键）。

### `MenuEvent` / `MenuDecision` / `MenuSource`

`MenuEvent<'a>` 为 `Opening { widget, source, anchor, menu }`、
`Opened { widget, source, menu }`、`Selected { widget, source, command:
&mut u32 }`、`Closed { widget, source, command: Option<u32> }`。
`MenuDecision` 为 `Pass`（默认）或 `Handled`。`MenuSource` 为 `ContextMenu`、
`Chrome`、`Programmatic`、`Tray`、`MenuBar`。

### 接线

| API | 签名 |
|---|---|
| `Ui::set_menu` | `set_menu(&mut self, id: WidgetId, menu: Option<Menu>)` |
| `Ui::open_menu` | `open_menu(&mut self, id, anchor: Point, sink) -> bool` |
| `Ui::open_menu_for` | `open_menu_for(&mut self, id, sink) -> bool` |
| `WindowHandle::set_menu_bar` | `set_menu_bar(&mut self, menu_bar: Option<MenuBarModel>)` |
| `WindowHandle::open_menu` | `open_menu(&mut self, widget, menu: Menu, anchor: Point, avoid: Option<Rect>)` |
| `Host::menu_command` | `fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>)` |
| `Host::menu_event` | `fn menu_event(&mut self, event: &mut MenuEvent<'_>, window) -> MenuDecision` |
| `Host::menu_bar_command` | `fn menu_bar_command(&mut self, command: u32, window)` |

## 示例

```rust
use bamboo_native_ui::{Menu, Ui, WidgetId};

fn install_player_menu(ui: &mut Ui<Msg>) {
    let menu = Menu::new()
        .shortcut_item(101, "Play", "Ctrl+P")
        .item(102, "Pause")
        .separator()
        .checked_item(103, "Shuffle", true)
        .submenu("More", Menu::new().item(104, "Properties"))
        .disabled_item(105, "Equalizer");
    ui.set_menu(WidgetId::new("player"), Some(menu));
}
```

选择以 id 的形式到达：

```rust
use bamboo_native_ui::{WidgetId, WindowHandle};

fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>) {
    if widget != WidgetId::new("player") {
        return;
    }
    match command {
        101 => self.play(window),
        102 => self.pause(window),
        103 => self.toggle_shuffle(window),
        _ => {}
    }
}
```

要在弹出菜单出现前镜像实时状态，请在钩子中编辑模型：

```rust
use bamboo_native_ui::{MenuDecision, MenuEvent, WindowHandle};

fn menu_event(
    &mut self,
    event: &mut MenuEvent<'_>,
    _window: &mut WindowHandle<'_>,
) -> MenuDecision {
    if let MenuEvent::Opening { menu, .. } = event {
        menu.set_checked(103, self.shuffle);
    }
    MenuDecision::Pass
}
```

## 说明与限制

- 命令 id 非零；`MenuItem::new` 在 `0` 时 panic，而 `0` 保留给被取消的弹出菜单。在 `Opening`
  上返回 `MenuDecision::Handled` 会取消弹出菜单；在 `Selected` 上则抑制默认回放。
- 打开的弹出菜单保留其构建时所用的模型；`Ui::set_menu` 的编辑应用于下一次打开。在 `Opened`
  期间调用 `Host::menu_event` 以及 `WindowHandle::edit_open_menu` 会关闭并重建活动弹出菜单。
- 原生弹出菜单和系统菜单栏仅限 Windows，并遵循系统颜色/DPI。菜单栏的 `&` 访问键由 Shell
  处理；`MenuBarModel::mnemonic_index` 为所有者绘制的标题解析 `Alt+键`。
- 绘制的标签（按钮、菜单栏控件）使用 `MnemonicLabel::parse("&File")`，它会去掉标记并报告
  该键；`Widget::mnemonic` 公开它，`Ui` 负责路由 `Alt+键`。
- 在模型安装期间，菜单栏菜单项的加速键会自动成为窗口加速键（`MenuBarModel::hotkeys`）；
  `WindowHandle::register_window_hotkey` 可以覆盖其中一个。
- 构建原生弹出菜单时会丢弃边缘和重复的分隔符。空菜单永远不会显示。

## 另见

- `hotkeys.md`（加速键解析和窗口快捷键）
- `tray-and-notifications.md`（托盘菜单）
- `accessibility.md`（Menu/MenuBar 角色）
- `../widgets/controls/button.md`
