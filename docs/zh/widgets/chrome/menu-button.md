# MenuButton

> 按钮，把声明式菜单作为原生弹出菜单打开。

## 介绍

`MenuButton<M>` 绘制一个按钮，带标签、可选的前导字形和尾部尖角符。点击它、按 Enter/Space 或 `Alt+键` 访问键，会请求宿主把其 `Menu` 作为原生弹出菜单显示，并锚定在按钮下方。该弹出菜单使用与上下文菜单相同的 `TrackPopupMenu` 路径，因此遵循系统菜单颜色；按钮本身是页面内容并使用调色板。

菜单是声明式的，可以携带分隔符、勾选项、禁用项、快捷键、图标和子菜单。选中的命令会作为 `u32` 应用 id 传递给构造函数的 `on_command` 处理函数；宿主还会通过 `Host::menu_command` 以该控件的 id 重放它。控件不解释命令。

关闭的菜单没有屏幕上的行，因此其无障碍子项会报告按钮的矩形；调用其中一个会直接运行命令。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::MenuButton;
use bamboo_native_ui::{Menu, MenuItem, Rect, Ui, WidgetId};

enum Msg {
    Command(u32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    MenuButton::new(
        "actions",
        "Actions(&A)",
        Menu::new()
            .item(700, "Play")
            .checked_item(701, "Shuffle", true)
            .separator()
            .submenu("Add to...", Menu::new().item(710, "Queue").item(711, "Favorites"))
            .push(MenuItem::new(720, "Track Info").shortcut("F2")),
        Msg::Command,
    )
    .glyph('≡'),
);
ui.set_rect(WidgetId::new("actions"), Rect::from_xywh(16.0, 16.0, 140.0, 30.0));
```

`MenuButton::new` 接收标签、菜单和命令处理函数。标签可以携带访问键（`"文件(&F)"` / `"&File"`）；该标记会从绘制的文本中剥离，并在 Alt 模式开启时加下划线，`Alt+键` 会打开菜单。

## 用法（XML）

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。它是仅用于构造的窗口装饰：不声明任何控件局部属性，也没有子元素。

```xml
<widget id="actions" x="16" y="16" w="140" h="30"/>
```

菜单及其命令保留在 Rust 中；文档无法构造控件实例或消息值。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, label, menu, on_command)` | `&'static str, impl AsRef<str>, Menu, impl Fn(u32) -> M` | — | 创建该按钮；`140 × 30` DIP，已启用。 |
| `.glyph(char)` | `char` | 无 | 前导 MDL2 码点或文本字符。 |
| `.disabled(bool)` | `bool` | `false` | 以禁用状态启动；禁用的按钮不会打开任何内容。 |
| `.menu()` | `&Menu` | — | 当前菜单模型。 |
| `.set_menu(Menu)` | `Menu` | — | 替换菜单并丢弃待处理的请求。 |
| `.set_label(&str)` | `&str` | — | 替换标签（重新解析其访问键）。 |

`preferred_height` 为 30 DIP。按钮绘制 `button_bg`，具有 `button_hover` / `button_pressed` 状态，并在右边缘绘制一个柔和的尖角符；字形条带宽 20 DIP，从左侧 10 DIP 处开始。

### 行为

- **指针。** 点击会打开菜单，锚点位于按钮左下角；控件的矩形作为避让矩形传入。悬停和按下状态遵循正常的按钮调色板。
- **键盘。** Enter/Space（`Activate`）打开菜单；与标签访问键匹配的 `Alt+键` 也会打开它。禁用的按钮会拒绝两者。
- **无障碍。** 角色为 `Menu`，以标签命名；其子项是菜单条目，带有菜单项角色、勾选/禁用状态和按钮的矩形。`a11y_expand(true)` 打开菜单，`a11y_select_child(index)` 通过 `on_command` 运行一个已启用的叶子项。

## 事件

### 消息

| 消息 | 发出条件 |
|---|---|
| 作为 `on_command` 传给 `new` 的 `impl Fn(u32) -> M` | 在原生弹出菜单中选择了命令、调用了无障碍子项（叶子菜单项），或宿主通过 `Host::menu_command` 重放了选择。 |

`u32` 是来自 `Menu` 的应用命令 id；打开菜单不发出任何消息。

### 通用观察器

此控件安装了共享的观察器构建器。它们都是附加型的：处理函数返回后，内置行为仍会执行。

| 构建器 | 处理函数 | 触发条件 |
|---|---|---|
| `.on_click(f)` | `Fn(Point) -> Option<M>` | 在控件上的左键点击。 |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | 右键点击，在上下文菜单之前。 |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | 平台双击。 |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | 控件获得焦点时的按键按下。 |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | 控件获得焦点时的按键释放。 |
| `.on_focus_gained(f)` | `Fn(FocusEvent) -> Option<M>` | 焦点移入该控件。 |
| `.on_focus_lost(f)` | `Fn(FocusEvent) -> Option<M>` | 焦点离开该控件。 |

### 控件专属回调

| 回调 | 签名 | 触发条件 |
|---|---|---|
| `new(id, label, menu, on_command)` | `&'static str, impl AsRef<str>, Menu, impl Fn(u32) -> M` | 选择或调用了菜单命令。 |

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("actions"), Trigger::Click, |event, ctx| {
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

该按钮响应 `Click`、`HoverEnter`/`HoverLeave` 和 `Activate`；其他触发器会到达绑定，但不会打开任何内容。

## XML 定义

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。

几何遵循常规的 `<widget>` 规则。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`、`expose`、enabled、焦点策略）适用。按钮从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported`。它不声明任何共享内容和子列表。

控件局部属性：无。子元素：无。

## 另见

- `./menu-bar.md`
- `./split-button.md`
- `../controls/button.md`
- `../overlay/popover.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
