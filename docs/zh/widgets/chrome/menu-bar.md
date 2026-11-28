# MenuBar

> 绘制的顶层菜单栏（File / Edit / View），其项目会打开原生菜单。

## 介绍

`MenuBar<M>` 绘制一条水平顶层项目条。每个项目拥有一个声明式 `Menu`；点击项目、按 Enter/Space 或 `Alt+键` 访问键，会请求宿主将该项目的菜单作为原生弹出菜单显示在其单元格下方。该原生弹出菜单与上下文菜单使用相同的延迟 `TrackPopupMenu` 路径，因此菜单保持即时、键盘/模型正确且可访问，而无需第二套绘制的菜单系统；它遵循系统菜单颜色而非页面调色板。

聚焦且关闭的菜单栏仅在某一方向还有另一个项目时，才为其顶层光标保留左/右方向键；在两端以及上/下方向键时，该按键会移动焦点而不是被吞掉。选中的命令以应用 id 形式经构造函数处理函数到达；宿主还会通过 `Host::menu_command` 以该控件的 id 重放它们。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::MenuBar;
use bamboo_native_ui::{Menu, Rect, Ui, WidgetId};

enum Msg {
    Command(u32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    MenuBar::new("menubar", Msg::Command)
        .item(
            "File(&F)",
            Menu::new()
                .item(500, "New Playlist")
                .separator()
                .item(501, "Open Library...")
                .disabled_item(502, "Export"),
        )
        .item(
            "View(&V)",
            Menu::new()
                .item(540, "Show Lyrics")
                .item(541, "Full Screen"),
        ),
);
ui.set_rect(WidgetId::new("menubar"), Rect::from_xywh(0.0, 0.0, 420.0, 26.0));
```

标签可以以任一顺序携带访问键（`"文件(&F)"` 或 `"&File"`）；该标记会从绘制的文本中移除，并在应用的 Alt 模式开启时加下划线。`.item` 追加；可以一次链式调用构建整个菜单栏，或稍后用 `set_items` 替换它。

## 用法（XML）

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。它是仅用于构造的窗口装饰：不声明任何控件局部属性，也没有子元素。

```xml
<widget id="menubar" x="0" y="0" w="420" h="26"/>
```

菜单、标签和命令保留在 Rust 中，因为文档无法构造控件或消息值。

## 属性

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, on_command)` | `&'static str, impl Fn(u32) -> M` | — | 创建一个空菜单栏；`420 × 26` DIP。 |
| `.item(label, menu)` | `impl AsRef<str>, Menu` | — | 追加一个顶层项目。 |
| `.disabled(bool)` | `bool` | `false` | 以禁用状态启动（忽略输入，清除待处理的菜单）。 |
| `.len()` / `.is_empty()` | `usize` / `bool` | — | 顶层项目的数量。 |
| `.cursor()` | `usize` | `0` | 覆盖顶层项目的键盘/选择光标。 |
| `.set_items(Vec<(String, Menu)>)` | pairs | — | 替换所有项目并钳制光标。 |
| `.set_item_menu(index, menu)` | `usize, Menu` | — | 替换某个单元格的菜单；返回它是否存在。 |
| `.insert_item(index, label, menu)` | `usize, &str, Menu` | — | 插入一个单元格（钳制到末尾）；返回 `&mut Self`。 |
| `.remove_item(index)` | `usize` | — | 移除一个单元格；返回其 `(label, menu)` 或 `None`。 |
| `.set_cursor(index)` | `usize` | — | 当 `index` 在范围内时移动光标。 |

### 行为

- **布局。** 菜单栏绘制 `palette.panel`，底部有 1-DIP 分隔线，按每个单元格的正文文本加两侧各 12 DIP 内边距来测量，最小为 42 DIP。单元格几何会被缓存，并在宽度、项目集或字体大小变化时失效。
- **指针。** 悬停和聚焦光标单元格绘制 `nav_hover`；点击会打开被点击单元格的菜单并将光标移到那里。
- **键盘。** 菜单栏拥有该按键时，左/右会在菜单栏内循环光标；Home/End 跳转；下/上和 Enter/Space 打开光标所在单元格。`mnemonic_key` 会打开匹配标签的单元格。
- **无障碍。** 角色为 `MenuBar`；每个顶层项目是一个子项，光标项目的菜单项发布在它们之后（偏移 1000），以便屏幕阅读器无需打开原生弹出菜单即可访问命令。调用已发布的条目会运行 `on_command`。

## 事件

### 消息

| 消息 | 发出条件 |
|---|---|
| 作为 `on_command` 传给 `new` 的 `impl Fn(u32) -> M` | 在项目的原生弹出菜单中选择了命令、调用了无障碍条目（发布在顶层项目之后），或宿主通过 `Host::menu_command` 重放了选择。 |

`u32` 是来自 `Menu` 的应用命令 id。

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
| `new(id, on_command)` | `&'static str, impl Fn(u32) -> M` | 选择或调用了菜单命令。 |

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("menubar"), Trigger::Click, |event, ctx| {
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

该菜单栏响应 `Click`、`HoverEnter`/`HoverLeave` 和 `Activate`；滚轮、拖动和右键点击没有内置效果。

## XML 定义

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。

几何遵循常规的 `<widget>` 规则。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`、`expose`、enabled、焦点策略）适用。菜单栏从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported`。它不声明任何共享内容和子列表。

控件局部属性：无。子元素：无。由于项目集仅用于构造，`<cell>` 或 `<tab>` 子元素会被拒绝为 `xml.skin.unknown`。

## 另见

- `./menu-button.md`
- `./split-button.md`
- `./toolbar.md`
- `./ribbon.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
