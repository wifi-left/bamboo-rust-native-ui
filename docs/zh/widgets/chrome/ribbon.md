# Ribbon

> 位于分组大/小命令图块上方的选项卡带。

## 介绍

`Ribbon<M>` 是应用的命令界面：一行选项卡（`RibbonTab`）位于一组带标签的分组（`RibbonGroup`）主体之上，每个分组持有命令（`RibbonItem`）。大项目是全高的图块，字形位于标签上方；连续的小项目按每列三行填充，字形位于标签旁边。分组绘制一条分隔线和一个底部标签。

点击选项卡会选中它并上报 `on_tab`；点击已启用的项目会通过 `on_command` 运行其 `u32` 命令。在键盘调整模式下，光标位于选项卡带上时，左/右切换选项卡（一旦进入主体则在项目之间移动），下进入主体，方向键按几何方式移动，Enter/Space 运行光标所在项目。选项卡标题可以携带 `&` 访问键（`Alt+K`）；标签遵循应用的 Alt 下划线模式。

模型由 Rust 拥有；功能区无法从文档重建。

## 用法（Rust）

```rust
use bamboo_native_ui::widgets::{
    Ribbon, RibbonGroup, RibbonItem, RibbonItemSize, RibbonTab,
};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Command(u32),
    Tab(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Ribbon::new(
        "ribbon",
        vec![
            RibbonTab::new("Home(&H)")
                .group(
                    RibbonGroup::new("Clipboard")
                        .item(RibbonItem::large(900, "Paste").glyph('\u{E77F}'))
                        .item(RibbonItem::small(901, "Cut"))
                        .item(RibbonItem::small(902, "Copy")),
                )
                .group(
                    RibbonGroup::new("Playback")
                        .item(RibbonItem::large(903, "Play").glyph('\u{E768}'))
                        .item(RibbonItem::small(904, "Next")),
                ),
            RibbonTab::new("Insert(&I)")
                .group(RibbonGroup::new("Cover").item(RibbonItem::large(910, "Add Cover"))),
        ],
        Msg::Command,
    )
    .label("Ribbon")
    .on_tab(Msg::Tab),
);
ui.set_rect(WidgetId::new("ribbon"), Rect::from_xywh(0.0, 0.0, 720.0, 140.0));
```

`RibbonItem::large` 和 `RibbonItem::small` 是两个构造函数；`RibbonItemSize` 是它们设置的枚举（`Large` 是默认变体）。大项目之后的小项目会开始新的一列，且任何一列最多容纳三个小项目。

## 用法（XML）

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。它是仅用于构造的窗口装饰：不声明任何控件局部属性，也没有子元素。

```xml
<widget id="ribbon" x="0" y="0" w="720" h="140"/>
```

选项卡、分组、命令和命令表保留在 Rust 中；文档无法构造控件实例或消息值。

## 属性

`Ribbon`：

| 方法 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `new(id, tabs, on_command)` | `&'static str, Vec<RibbonTab>, impl Fn(u32) -> M` | — | 创建该功能区；`720 × 140` DIP。 |
| `.label(text)` | text | 控件 id | 无障碍名称。 |
| `.on_tab(handler)` | `Fn(usize) -> M` | 无 | 通过点击、访问键或 UIA 改变了选中的选项卡。 |
| `.disabled(bool)` | `bool` | `false` | 以禁用状态启动。 |
| `.tab_height(f32)` | `f32` | `28.0` | 选项卡带高度，钳制为 `>= 20`。 |
| `.small_height(f32)` | `f32` | `22.0` | 一行小项目的高度，钳制为 `>= 18`。 |
| `.tabs()` / `.set_tabs(Vec<RibbonTab>)` | tabs | — | 读取或替换模型（重置主体光标）。 |
| `.tab_count()` / `.selected_tab()` | `usize` | — | 选项卡数量和选中索引。 |
| `.select_tab(index)` | `bool` | — | 选中一个选项卡（钳制）；返回其是否发生变化。 |
| `.tab_rect(index)` / `.item_rect(group, item)` | `Option<Rect>` | — | 选项卡标题或所选选项卡上某个项目的几何。 |
| `.set_checked(tab, group, item, bool)` | `bool` | — | 运行时勾选标记。 |
| `.set_enabled_item(tab, group, item, bool)` | `bool` | — | 运行时项目启用。 |

`RibbonTab`、`RibbonGroup`、`RibbonItem`：

| 方法 | 类型 | 描述 |
|---|---|---|
| `RibbonTab::new(title)` | `impl AsRef<str>` | 一个选项卡；标题可以携带访问键。 |
| `.group(RibbonGroup)` | `RibbonGroup` | 追加一个分组。 |
| `RibbonGroup::new(label)` | `impl Into<String>` | 一个带标签的分组。 |
| `.item(RibbonItem)` | `RibbonItem` | 追加一个命令。 |
| `RibbonItem::large(command, label)` / `::small(...)` | `u32, impl AsRef<str>` | 全高图块或一行小项目。 |
| `.glyph(char)` / `.disabled(bool)` / `.checked(bool)` | — | 图标和项目状态。 |

### 行为

- **几何。** 选项卡在 `tab_height` 带中宽 88 DIP。大项目宽 58 DIP 并跨越主体；小项目宽 96 DIP、高 22 DIP，每列最多三个。每个分组以 16-DIP 的标签带收尾。
- **指针。** 悬停选项卡或项目时，仅在目标变化时重绘；点击会选中选项卡或运行项目。选中的选项卡与主体融合，并绘制一条 2-DIP 的强调色顶边。
- **键盘。** 光标位于带上时，左/右切换选项卡，Home/End 跳转；下进入第一个项目。在主体中，左/右线性移动，上/下按几何方式选择最近的项目，从第一个项目向上返回带。Enter/Space 运行光标所在项目。
- **无障碍。** 角色为 `TabList`；选项卡是子项，所选选项卡的命令作为 Button 子项跟随其后（偏移 100）。调用选项卡会选中它；调用命令会运行它。

## 事件

### 消息

| 消息 | 发出条件 |
|---|---|
| 作为 `on_command` 传给 `new` 的 `impl Fn(u32) -> M` | 点击已启用的项目、在光标项目上按 `Enter`/`Space` 运行，或通过无障碍或 UIA 调用。 |
| 传给 `.on_tab` 的 `impl Fn(usize) -> M` | 通过点击、`Alt+键` 访问键或 UIA 调用选中选项卡；索引是新的选项卡。 |

悬停、移动光标和 `select_tab`（Rust setter）不发出任何消息。

### 通用观察器

此控件不安装任何共享的观察器构建器。请使用 `on_command` / `on_tab` 或 `Ui::bind`。

### 控件专属回调

| 回调 | 签名 | 触发条件 |
|---|---|---|
| `new(id, tabs, on_command)` | `&'static str, Vec<RibbonTab>, impl Fn(u32) -> M` | 运行一个命令项目。 |
| `.on_tab(handler)` | `impl Fn(usize) -> M` | 选中的选项卡因用户输入而改变。 |

### Ui 触发器

除上述构建器外，任何控件 id 都可以通过 `Ui::bind` 绑定：

```rust
ui.bind(WidgetId::new("ribbon"), Trigger::Click, |event, ctx| {
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

该功能区响应 `Click`、`HoverEnter`/`HoverLeave` 和 `Activate`；滚轮、拖动和右键点击没有内置效果。

## XML 定义

此控件不由标准注册表创建。请在 Rust 中创建它，并通过文档中的 `id` 引用它。

几何遵循常规的 `<widget>` 规则。通用属性（`visible`、`tooltip`、`name`、`description`、`automation-id`、`role`、`expose`、enabled、焦点策略）适用。功能区从调色板绘制，没有 `SurfaceStyle`，因此 `radius`、`corners` 和 `background` 会被报告为 `xml.style.unsupported`。它不声明任何共享内容和子列表。

控件局部属性：无。子元素：无（`<tab>` 和 `<command>` 会被拒绝为 `xml.skin.unknown`）。

## 另见

- `./toolbar.md`
- `./menu-bar.md`
- `./status-bar.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
