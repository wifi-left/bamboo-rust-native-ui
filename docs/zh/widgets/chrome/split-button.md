# SplitButton

> 一个按钮，其主体运行主要动作，箭头打开备选菜单。

## 介绍

`SplitButton<M>` 绘制一个具有两个区域的控件：主体（最后 26 DIP 的左侧）直接运行主要动作，右边缘的尖角符区域打开一个原生备选菜单。一条 1-DIP 分隔线分隔两个区域，悬停在箭头上时仅着色该区域。

点击主体，或按 Enter/Space/`Activate`，会上报 `on_primary`；点击箭头、`Alt+键` 访问键或无障碍展开请求会打开菜单。菜单命令使用与 `MenuButton` 相同的原生弹出菜单和命令 id，`on_command` 将它们映射为应用消息。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::SplitButton;
use bamboo_native_ui::{Menu, Rect, Ui, WidgetId};

enum Msg {
    ExportNow,
    Command(u32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    SplitButton::new(
        "export",
        "Export(&E)",
        Menu::new()
            .item(800, "Export as MP3")
            .item(801, "Export as FLAC")
            .separator()
            .item(802, "Export Settings..."),
        || Msg::ExportNow,
        Msg::Command,
    )
    .glyph('⇩'),
);
ui.set_rect(WidgetId::new("export"), Rect::from_xywh(16.0, 16.0, 170.0, 30.0));
```

主体处理函数不接受参数；菜单处理函数接收 `u32` 命令。标签可以携带访问键（`"Export(&E)"`），该访问键会打开菜单而非运行主要动作。

## 用法（XML）

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。它是仅用于构造的窗口装饰：不声明任何控件局部属性，也没有子元素。

```xml
<widget id="export" x="16" y="16" w="170" h="30"/>
```

菜单、主要动作和命令表保留在 Rust 中。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, label, menu, on_primary, on_command)` | `&'static str, impl AsRef<str>, Menu, impl Fn() -> M, impl Fn(u32) -> M` | — | 创建该按钮；`170 × 30` DIP，已启用。 |
| `.glyph(char)` | `char` | 无 | 前导 MDL2 码点或文本字符。 |
| `.disabled(bool)` | `bool` | `false` | 以禁用状态启动；两个区域都不响应。 |
| `.set_menu(Menu)` | `Menu` | — | 替换备选菜单并丢弃待处理的请求。 |

`preferred_height` 为 30 DIP。右侧 26 DIP 是箭头区域；分隔线位于其左边缘。主体使用 `button_bg`、`button_hover` 和 `button_pressed`；尖角符为柔和色。

### 行为

- **指针。** 悬停会跟踪指针所在的区域，仅在区域变化时重绘。在主体中点击会发出主要消息；在箭头区域中点击会打开菜单，锚定在箭头的底边，整个按钮作为避让矩形。
- **键盘。** `Activate` 和 Enter/Space 运行主要动作。与标签访问键匹配的 `Alt+键` 打开备选菜单。
- **无障碍。** 角色为 `Menu`，以标签命名，描述为主要动作加菜单，菜单条目作为子项。展开会打开菜单；调用已启用的叶子项会运行 `on_command`。

## 事件

### 消息

| 消息 | 发出条件 |
|---|---|
| 作为 `on_primary` 传给 `new` 的 `impl Fn() -> M` | 点击主体区域，或 `Enter`/`Space`/`Activate` 运行主要动作。 |
| 作为 `on_command` 传给 `new` 的 `impl Fn(u32) -> M` | 在备选菜单中选择了命令，或调用了 UIA 菜单子项。 |

打开菜单和悬停不发出任何消息。

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
| `new(id, label, menu, on_primary, on_command)` | `&'static str, impl AsRef<str>, Menu, impl Fn() -> M, impl Fn(u32) -> M` | 主体运行主要动作，或选择了菜单命令。 |

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("export"), Trigger::Click, |event, ctx| {
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

该按钮响应 `Click`、`HoverEnter`/`HoverLeave` 和 `Activate`；其他触发器会到达绑定，但不改变任何内容。

## XML 定义

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。

几何遵循常规的 `<widget>` 规则。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`、`expose`、enabled、焦点策略）适用。按钮从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported`。它不声明任何共享内容和子列表。

控件局部属性：无。子元素：无。

## 另见

- `./menu-button.md`
- `./menu-bar.md`
- `./toolbar.md`
- `../controls/button.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
