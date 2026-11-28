# XML 布局编写

> 编写一份 `.buixml` 文档，用来定位控件、设置运行时属性、
> 应用样式 token、提供初始内容和数据列表，甚至能
> 创建控件 —— 然后在运行时应用它或编译它。

## `.buixml` 文档是什么

`.buixml` 文档主要是一层**皮肤（skin）**：它把矩形、属性、
样式 token 和初始内容分配给 Rust 侧已经创建的控件 id。它
不能移除、重排或改变这些被寻址控件的父级，也不能绑定事件。有两个明确的例外是：`type="..."` 通过已安装的 `WidgetRegistry` 创建一个控件
（12 种标准类型；第三方类型从 Rust 注册；
创建的 id 属于文档，一次重新加载会移除或替换它们），以及子
元素（`<item>`、`<row>` 等）通过控件的皮肤
协议替换其数据模型。

运行时读取器是 `feature = "layout-xml"`；同一个文件可以用
`feature = "layout-compiled"` 编译（参见 [06](../quickstart/06-xml-compiled.md)）。

## 步骤

### 1. 启用特性并创建文件

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

创建 `layouts/main.buixml`：

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <const name="pad" value="16"/>
  <layout id="base" min-width="0" min-height="0">
    <widget id="title" x="pad" y="pad" w="cw-2*pad" h="28" label="Library"/>
    <widget id="search" right="pad" y="pad" w="220" h="28"/>
  </layout>
</layouts>
```

`version` 是必需的，且必须为 `1`。文件为 UTF-8（接受 BOM），
名称区分大小写且为 kebab-case，注释会被忽略，非空白
文本节点会被拒绝。

### 2. 在 Rust 中创建控件

文件中的 id 必须在首次应用之前存在，除非条目会
创建它们。

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;
use bamboo_native_ui::widgets::{Label, SearchBox};
use bamboo_native_ui::Ui;

#[derive(Clone)] enum Msg { Save }

struct App { ui: Ui<Msg>, layout: LayoutDocument }

impl App {
    fn new() -> Self {
        let mut ui = Ui::new();
        ui.add(Label::new("title", "Library"));
        ui.add(SearchBox::new("search", "Search"));
        let layout = LayoutDocument::parse(include_str!("../layouts/main.buixml"))
            .expect("the embedded layout is valid");
        Self { ui, layout }
    }
}
```

未知 id 会产生 `xml.id.unknown-widget`，并且应用是原子的：什么都不会移动。

### 3. 在 `resize` 中应用它

```rust
use bamboo_native_ui::{Host, Size, WindowHandle};

impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let palette = *window.palette();
        if let Err(failure) = self.ui.apply_layout(&self.layout, &palette, size, window) {
            eprintln!("layout rejected: {failure}");
        }
        window.invalidate_all();
    }
}
```

`apply_layout` 先解析每一个 id、矩形和表达式。任何错误都会
返回 `LayoutFailure` 且不修改任何内容；警告写入
`LayoutReport::warnings`。`WindowHandle` 是 `UiHost` 接收端，调色板
负责解析 `background` token。

### 4. 结构：根、变体、常量

根 `<layouts version="1">` 持有文档作用域的 `<const>` 定义和
断点 `<layout>` 变体；每个变体持有 `<scroll>` 面板、
`<widget>` 条目以及编译模式下的 `<rust>` 绑定。变体需要 `id`
加上可选的 `min-width` 和 `min-height`。恰好必须有一个变体为
`(0, 0)`，且最小值对必须唯一。选择是纯函数式的：在最小值
都适合客户端尺寸的变体中，最大的 `min-width`，然后
最大的 `min-height` 胜出。变体只在几何、属性和
样式上可以不同；内容与局部皮肤数据在每个声明它们的变体中
必须相同（`xml.content.conflict`、`xml.skin.conflict`）。

`<const>` 声明一个文档作用域的数字；`<scroll>` 寻址一个通过
`Ui::set_scroll_view` 注册的面板；`<rust>` 是编译模式的绑定。

### 5. 几何表达式

每个矩形都是一个横向和一个纵向形式：`x`+`w`、`right`+`w`
或 `x`+`right`（拉伸）；`y`+`h`、`bottom`+`h` 或 `y`+`bottom`（拉伸）。

```text
expr   := term (('+' | '-') term)*
term   := factor (('*' | '/') factor)*
factor := number | number '%' | variable | '(' expr ')' | '-' factor
```

`left`/`top` 为 `0`；`right`/`bottom` 是空间边缘；`cw`/`ch` 是
空间宽度/高度；`cx`/`cy` 是其中心。`N%` 是该属性轴向的 `N/100`
（`x`/`w`/`right` 使用 `cw`；`y`/`h`/`bottom` 使用 `ch`）。
常量在求值之前解析。`w`/`h` 必须 `>= 0`；`right`/`bottom`
必须 `>= 0`；零尺寸是警告。

以 1000×700 的窗口、`pad = 16` 为例，这些写法就很具体：

| 目标 | 几何写法 |
|---|---|
| 通栏页眉 | `x="pad" y="pad" w="cw-2*pad" h="48"` |
| 侧栏宽 240，页眉下方占满高度 | `x="pad" y="80" w="240" bottom="pad"` |
| 侧栏右侧的内容区 | `x="272" y="80" right="pad" bottom="pad"` |
| 两列等宽，间距 16 DIP | `x="pad" w="(cw-3*pad)/2"` 与 `x="2*pad+(cw-3*pad)/2" w="(cw-3*pad)/2"` |
| 居中的 400×280 卡片 | `x="cx-200" y="cy-140" w="400" h="280"` |
| 高 40 的底栏 | `x="pad" bottom="pad" w="cw-2*pad" h="40"` |

同一个外壳写成文档：

```xml
<const name="pad" value="16"/>
<layout id="base" min-width="0" min-height="0">
  <widget id="header"  x="pad" y="pad" w="cw-2*pad" h="48"/>
  <widget id="sidebar" x="pad" y="80" w="240" bottom="pad"/>
  <widget id="content" x="272" y="80" right="pad" bottom="pad"/>
</layout>
```

由已注册面板滚动的控件定位在该面板的**内容**
空间中，`cw`/`ch` 表示该内容的范围。`auto`
内容高度面板中的 `cw` 会晚一次应用；对于
底部锚定的子项，请使用显式的 `content-height`。

### 6. 属性

可选且运行时安全：`visible`、`enabled`、`interactive`、`focusable`、
`focus-order`、`read-only`、`tooltip`、`name`、`description`、`automation-id`、
`role`、`expose`、`cursor`。布尔值接受 `true`/`false`/`1`/`0`；`role` 接受
一个 kebab-case 的 `A11yRole` 名称；`cursor` 接受
`arrow`/`hand`/`i-beam`/`size-we`/`size-ns`。它们走现有的
容器 setter，因此文档永远不会重建控件。省略意味着“保留
应用设置的值”；`name`/`description` 只改变朗读，不改变
绘制的文本。

### 7. 样式 token

| 属性 | 接受的值 |
|---|---|
| `radius` | 非负数、`small`（4）、`medium`（6）、`large`（10）、`pill`（高度的一半） |
| `corners` | 四个数字 `"tl tr br bl"`（空格或逗号） |
| `background` | 一个调色板 token、一个颜色字面量，或 `none` |
| `font-size` | 大于零的数字（DIP） |
| `font-family` | 非空字符串 |

`radius` 和 `corners` 互斥。`background="none"` 保留
边框并去掉填充。没有 `SurfaceStyle` 的控件会报告
`xml.style.unsupported` 并跳过表面 token；`font-*` 适用于每个
控件。token 列表见[调色板 token](../reference/palette-tokens.md)。

### 8. 内容初始值

`text` → `TEXT`、`label` → `LABEL`、`value` → `VALUE`、`progress` → `PROGRESS`
（钳制到 `0..=1`）、`number` → `NUMBER`、`checked` → `TOGGLED`、`selected` →
`SELECTION`。`text`/`label` 冲突，`value`/`progress`/`number` 也是如此。

内容是**初始值**，仅在文档 generation
改变时写入。调整尺寸、切换断点、更改主题或重新应用同一份
文档都不会覆盖用户输入的文本。不发布
该能力的控件会报告 `xml.content.unsupported`，并且该属性被跳过。

### 9. 子列表

子元素替换控件的数据模型；它们不是实时绑定。`PlaylistView` 接受 `<track title subtitle current disabled>` 子元素，`Table` 接受 `<column>`/`<row>`（外加行内的 `<cell>`）。常见的子元素
名称有 `item`、`option`、`row`、`cell`、`column`、`node`、`section`、
`cover`、`track`、`segment`、`step`、`page`、`run` 和 `point`。每个控件的
词汇和接受的属性见
[XML 元素](../reference/xml-elements.md)。

### 10. 创建控件（`type`、`command`、`parent`）

带有 `type` 的条目会创建一个尚不存在的 id。交互类型
需要 `command`，通过注册表的 `CommandMap` 解析。

```xml
<widget id="save" type="button" command="101" x="16" y="8" w="96" h="32" label="Save"/>
<widget id="row-1" type="label" parent="list" x="0" y="250" w="120" h="20" label="Row"/>
```

```rust
use bamboo_native_ui::creation::WidgetRegistry;

let mut registry = WidgetRegistry::<Msg>::standard();
registry.set_commands(|command| (command == 101).then_some(Msg::Save));
ui.set_widget_registry(registry);
```

`WidgetRegistry::standard()` 注册 `label`、`button`、`toggle`、`slider`、
`progress`、`icon`、`badge`、`card`、`divider`、`kbd`、`avatar` 和 `spinner`。
`parent` 把一个创建的控件加入一个已注册面板，并在该面板的内容空间中
解析其矩形。创建的控件属于文档：重新加载
若丢弃它就会移除它，而 `Ui::clear_layout` 会移除每一个创建的控件。
每个变体都必须以相同的 `type`/`command`/`parent` 声明每一个创建的 id。

<a id="custom-widgets"></a>
### 11. 自定义控件

crate 之外的控件会发布它接受的属性和子元素名称。

```rust
use bamboo_native_ui::{SkinAttrDecl, SkinValue, SkinValueKind, Widget};

static LEVEL_ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("level", SkinValueKind::Number)];

impl Widget<Msg> for LevelBar {
    fn skin_attrs(&self) -> &'static [SkinAttrDecl] { LEVEL_ATTRS }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        matches!(
            (name, value),
            ("level", SkinValue::Number(level)) if (0.0..=1.0).contains(level)
        )
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        if let ("level", SkinValue::Number(level)) = (name, value) {
            self.level = *level as f32;
        }
    }
}
```

该代码块只展示三个皮肤方法；必需的 `id`、`rect`、
`set_rect`、`as_any`、`as_any_mut`、`paint` 和 `input` 方法就是
任何手写控件的方法（参见 [04 — 自定义控件](../quickstart/04-custom-widgets.md)）。
然后文档可以写入
`<widget id="level" x="16" y="120" w="240" h="12" level="0.8"/>`。

应用器在修改之前验证每一个声明的属性和子列表，
然后按顺序应用子列表、内容和属性，并一次性把控件
标记为脏。`x-` 名称保留给应用代码；共享的内容
名称保留给 schema。局部值是内容：仅在
generation 改变时写入，且不会被 `clear_layout` 恢复。完整协议参见
[CustomWidget](../widgets/custom/custom-widget.md)。

### 12. 热重载

库不执行任何文件 IO，也不安装任何监视器。轮询文件并保留
最后一个可用的文档。

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;
use bamboo_native_ui::{Palette, Size, Ui, UiHost};

fn reload(text: &str, current: &LayoutDocument, ui: &mut Ui<Msg>,
          palette: &Palette, size: Size, sink: &mut impl UiHost) -> Option<LayoutDocument> {
    let document = LayoutDocument::parse(text).ok()?;
    if document.generation() == current.generation() {
        return None;
    }
    ui.apply_layout(&document, palette, size, sink).ok()?;
    Some(document)
}
```

在
`window.set_animation_timer(Some(Duration::from_millis(250)))` 之后从 `Host::timer` 驱动它；
一次轮询只在 generation 改变时绘制。

### 13. 限制与失败行为

- 文件限制为 4 MiB、50 000 个条目，以及每个控件 10 000 个子元素。
  每次解析最多收集 100 条诊断。
- 表达式限制为 32 层嵌套；除以零和
  非有限结果是错误。
- 不支持 DTD、CDATA、命名空间、include 或监视器。样式或
  内容值中不允许表达式。不支持事件绑定：点击、按键和拖动保留在 Rust 中
  （`Widget::input`、`Ui::bind`）。
- 错误会拒绝整个文档并保留先前安装的布局；
  警告（`xml.style.unsupported`、`xml.content.unsupported`、
  `xml.value.clamped` 等）会应用其余部分。
- `Ui::clear_layout(window)` 归还每个已声明的字段并移除创建的
  控件；矩形和内容不会被恢复。完整的诊断
  代码列表见[XML 元素](../reference/xml-elements.md)。

## 权衡

| 选择 | 好处 | 代价 |
|---|---|---|
| 动态 `layout-xml` | 无需重建即可编辑和重新加载 | 链接 XML 解析器；运行时解析 |
| 编译 `layout-compiled` | 无解析器；值由 `rustc` 类型检查 | 一个生成步骤或宏重建 |
| `type` + 注册表 | 页面完全由数据描述 | 需要安装一个注册表/命令表 |

## 另请参阅

- [XML 元素](../reference/xml-elements.md)
- [05 — 运行时 XML](../quickstart/05-xml-dynamic.md)
- [06 — 编译期 XML](../quickstart/06-xml-compiled.md)
- [Ui 容器](../api/ui-container.md)
- [CustomWidget](../widgets/custom/custom-widget.md)
- [滚动](../api/scrolling.md)
- [调试](debugging.md)
