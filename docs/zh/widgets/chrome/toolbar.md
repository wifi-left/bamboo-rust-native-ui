# Toolbar

> 一条命令按钮条带，带分隔符、仅图标单元格和一个溢出菜单。

## 介绍

`Toolbar<M>` 绘制一行（或一列）`ToolbarItem` 命令单元格。放不下的项目会移入一个尖角符溢出菜单，而不是被裁剪。单元格具有固定尺寸，因此溢出边界纯粹是几何计算：无障碍、命中测试和 UIA 动作在首次绘制之前就看到相同的布局。

`ToolbarItem::separator` 将条带分成可视组；它不拥有命令，会被键盘光标跳过，不接受悬停，并且永远不会出现在溢出菜单中。带有 `.icon_only()`（或字形加空标签）的项目仅绘制其字形，并在水平条带上变为正方形。`.checked(true)` 绘制一个强调色着色的单元格，并在溢出菜单中发布勾选标记。

水平条带使用左/右作为其光标，垂直条带使用上/下；Enter/Space 运行当前项目（或打开溢出列表）。工具栏是一个 Tab 停靠点，并发布 `ToolBar`，每个命令一个子项。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{Toolbar, ToolbarItem};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Command(u32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(Toolbar::new(
    "tools",
    vec![
        ToolbarItem::new(600, "Play").glyph('▶'),
        ToolbarItem::new(601, "Pause").glyph('❚'),
        ToolbarItem::separator(),
        ToolbarItem::new(602, "Favorite").checked(true),
        ToolbarItem::new(603, "")
            .glyph('\u{E721}')
            .icon_only()
            .name("Search"),
    ],
    Msg::Command,
));
ui.set_rect(WidgetId::new("tools"), Rect::from_xywh(0.0, 32.0, 420.0, 32.0));
```

调用 `.vertical()` 以堆叠单元格（应用工具栏或任务窗格）：溢出按钮移至底部，上/下驱动光标，`preferred_height` 变为 `8 + 单元格尺寸之和`。

## 用法（XML）

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。它是仅用于构造的窗口装饰：不声明任何控件局部属性，也没有子元素。

```xml
<widget id="tools" x="0" y="32" w="420" h="32"/>
```

项目、字形、命令和 vertical 标志保留在 Rust 中。

## 属性

`Toolbar`：

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, items, on_command)` | `&'static str, Vec<ToolbarItem>, impl Fn(u32) -> M` | — | 创建该条带；`420 × 32` DIP。 |
| `.vertical()` | — | 水平 | 堆叠单元格；溢出移至底部。 |
| `.item_width(f32)` | `f32` | `104.0` | 水平单元格宽度，钳制为 `>= 40`。 |
| `.item_height(f32)` | `f32` | `32.0` | 垂直单元格高度，钳制为 `>= 24`。 |
| `.disabled(bool)` | `bool` | `false` | 以禁用状态启动。 |
| `.items()` / `.set_items(Vec<ToolbarItem>)` | items | — | 读取或替换模型（重置光标）。 |
| `.insert_item(index, item)` / `.remove_item(index)` | index | — | 无需重建的运行时编辑。 |
| `.set_item_label(index, &str)` | index | — | 重命名一个非分隔符单元格；返回其是否发生变化。 |
| `.set_checked(index, bool)` / `.set_enabled_item(index, bool)` | index | — | 运行时项目状态；返回其是否发生变化。 |
| `.visible_count()` | `usize` | — | 能放在溢出按钮之外的项目。 |
| `.is_vertical()` / `.mnemonic_underlines()` | `bool` | — | 当前标志。 |

`ToolbarItem`：

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(command, label)` | `u32, impl AsRef<str>` | — | 一个命令单元格；标签可以携带访问键。 |
| `separator()` | — | — | 一条 9-DIP 的组分隔线，不带命令。 |
| `.glyph(char)` | `char` | 无 | 前导 MDL2 码点或文本字符。 |
| `.icon_only()` | — | 关闭 | 仅绘制字形；在水平条带上为正方形。 |
| `.name(text)` | text | 绘制的标签 | 仅图标单元格的无障碍/工具提示名称。 |
| `.disabled(bool)` / `.checked(bool)` | `bool` | `false` | 项目状态。 |

### 行为

- **几何。** 水平条带从左侧 4 DIP 处开始放置单元格；命令单元格为 `item_width`，分隔符为 9 DIP，仅图标单元格为正方形并使用条带高度。溢出按钮为 32 DIP，位于右侧（水平）或底部（垂直）。
- **溢出。** 当总尺寸超过可用空间时，尾部项目被隐藏，尖角符打开一个由它们构建的菜单（省略分隔符，保留禁用状态）。隐藏的命令仍保留在无障碍树中。
- **键盘。** 轴向方向键在命令单元格和溢出按钮之间移动光标，Home/End 跳转，Enter/Space 运行光标项。`owns_navigation_key` 在两端释放轴向按键，以便焦点可以离开。
- **无障碍。** 角色为 `ToolBar`；每个可见命令一个子项，外加一个用于溢出的“更多”子项，隐藏的命令作为菜单项发布。调用子项会运行其命令；调用溢出项会打开它。

## 事件

### 消息

| 消息 | 发出条件 |
|---|---|
| 作为 `on_command` 传给 `new` 的 `impl Fn(u32) -> M` | 点击命令单元格、在光标单元格上按 `Enter`/`Space` 运行、通过无障碍或 UIA 调用，或从溢出菜单中选择。 |

分隔符不拥有命令；移动光标不发出任何消息。

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
| `new(id, items, on_command)` | `&'static str, Vec<ToolbarItem>, impl Fn(u32) -> M` | 任何可见或溢出命令运行。 |

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("tools"), Trigger::Click, |event, ctx| {
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

该工具栏响应 `Click`、`HoverEnter`/`HoverLeave` 和 `Activate`；滚轮、拖动和右键点击没有内置效果。

## XML 定义

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。

几何遵循常规的 `<widget>` 规则。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`、`expose`、enabled、焦点策略）适用。工具栏从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported`。它不声明任何共享内容和子列表。

控件局部属性：无。子元素：无。项目列表仅用于构造，因此 `<cell>` 子元素会被拒绝为 `xml.skin.unknown`。

## 另见

- `./ribbon.md`
- `./menu-bar.md`
- `./status-bar.md`
- `../player/playback-transport.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
