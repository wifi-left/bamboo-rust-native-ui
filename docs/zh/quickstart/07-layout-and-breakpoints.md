# 07 — 布局与断点

> 两次计算响应式外壳：先在 `Host::resize` 中使用
> `bamboo_native_ui::layout` 辅助函数，然后用按客户区
> 尺寸选择的 `.buixml` 断点变体。

## 你将构建

一个窗口，带侧边栏、含两个按钮的标题栏、内容窗格和状态
行。当客户区窄于 900 DIP 时，侧边栏从 220
DIP 折叠到 56 DIP；高于阈值时再次展开。本页的
前半部分用 `VStack`、`HStack`、`split_h`、`split_v`、`grid` 和
`padded` 完成。后半部分将同样的六个控件移入带
`wide` 和 `base` 变体的 `.buixml` 文档，并让 `Ui::apply_layout` 选择一个。

## 前置条件

- [03](03-first-window.md) 中的窗口可以构建并运行。
- 对于后半部分，像 [05](05-xml-dynamic.md) 那样启用 `layout-xml`：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

`bamboo_native_ui::layout` 中的辅助函数无需特性开关。

## 步骤

### 1. 创建外壳控件

一次创建六个控件。它们与文档稍后将要寻址的 id 相同，
因此本页的两半只交换放置代码。

```rust
use bamboo_native_ui::widgets::{Button, Label, LabelStyle};
use bamboo_native_ui::{Host, Insets, Rect, Size, Ui, WidgetId, WindowHandle};

#[derive(Clone, Copy, Debug)]
enum Msg {
    Open,
    Save,
}

struct App {
    ui: Ui<Msg>,
}

impl App {
    fn new() -> Self {
        let mut ui = Ui::new();
        ui.add(Label::new("title", "Bamboo Player").style(LabelStyle::Title));
        ui.add(Button::new("open", "Open", Msg::Open));
        ui.add(Button::new("save", "Save", Msg::Save));
        ui.add(Label::new("sidebar", "Playlists · Favorites").wrap(true));
        ui.add(Label::new("body", "Select a track from the sidebar.").wrap(true));
        ui.add(Label::new("hint", "Ready"));
        Self { ui }
    }
}
```

控件在不知道将位于何处的情况下创建；同一个 `ui` 可以
手动布局或由文档布局。

### 2. 在 `resize` 中计算布局

[布局辅助函数](../api/layout-helpers.md)是作用于 `Rect` 的纯函数，
因此只在客户区尺寸变化时运行它们。将 03 的 `Host` 实现的 `resize` 方法
替换为此版本，并添加 `layout`
函数。

```rust
use bamboo_native_ui::layout::{grid, padded, split_h, split_v, HStack, VStack};

fn layout(ui: &mut Ui<Msg>, size: Size) {
    // One page inset; every other rectangle is derived from it.
    let page = padded(
        Rect::from_xywh(0.0, 0.0, size.width, size.height),
        Insets::uniform(12.0),
    );

    // Header row, then everything below it.
    let mut rows = VStack::new(page, 10.0);
    let header = rows.next(44.0);
    let body = rows.remaining();

    // Header: title on the left, two equal button cells on the right.
    let header_cells = split_h(header, &[3.0, 1.0], 10.0);
    ui.set_rect(WidgetId::new("title"), header_cells[0]);
    let actions = grid(header_cells[1], 2, 1, 8.0);
    ui.set_rect(WidgetId::new("open"), actions[0]);
    ui.set_rect(WidgetId::new("save"), actions[1]);

    // The sidebar width follows the breakpoint; content takes the rest.
    let mut columns = HStack::new(body, 10.0);
    let sidebar_width = if size.width < 900.0 { 56.0 } else { 220.0 };
    let sidebar = columns.next(sidebar_width);
    let content = columns.remaining();
    ui.set_rect(WidgetId::new("sidebar"), sidebar);

    // Content: a main pane and a footer strip, split by weight.
    let content_cells = split_v(content, &[5.0, 1.0], 10.0);
    ui.set_rect(WidgetId::new("body"), content_cells[0]);
    ui.set_rect(WidgetId::new("hint"), content_cells[1]);
}

impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        layout(&mut self.ui, size);
        window.invalidate_all();
    }
}
```

`VStack::next` 返回一个全宽行并推进游标；`remaining`
报告它下方剩余的空间（尾部间隙被排除，因此它从
前一个单元格的边缘开始）。`split_h`/`split_v` 在
减去 `gap * (n - 1)` 后划分空间，并按 `usable * weight / total` 给每个单元格；
权重无需总和为一。`grid` 按行优先平铺区域并将两个
计数钳制为至少一。`padded` 即 `rect.inset_by(insets)`。当你想要沟槽时使用拆分
辅助函数，就像这里的标题栏和内容列那样。

### 3. 编写断点文档

[`.buixml` 布局文档](../api/layout-documents.md)为每个变体保存一个 `<layout>`。
每个变体都需要为它移动的每个 id 提供一个矩形；它还可以
隐藏控件或更改字体。解析器要求恰好一个 `0,0` 变体；
`LayoutDocument::select(size)` 在最小值都
适配的变体中，选择最大的 `(min-width, min-height)` 对。最小值是客户区 DIP。

创建 `layouts/shell.buixml`：

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <const name="pad" value="16"/>
  <const name="collapsed" value="56"/>
  <const name="expanded" value="220"/>
  <const name="header" value="56"/>
  <const name="footer" value="64"/>

  <layout id="wide" min-width="900">
    <widget id="sidebar" x="0" y="0" w="expanded" h="ch" background="sidebar"/>
    <widget id="title" x="expanded+pad" y="pad" w="cw-expanded-2*pad-272" h="30" font-size="20"/>
    <widget id="open" right="pad+132" y="pad" w="120" h="36" radius="medium" background="accent"/>
    <widget id="save" right="pad" y="pad" w="120" h="36" radius="medium"/>
    <widget id="body" x="expanded+pad" y="header" right="pad" bottom="footer"/>
    <widget id="hint" x="expanded+pad" right="pad" bottom="pad" h="24"/>
  </layout>

  <layout id="base" min-width="0" min-height="0">
    <widget id="sidebar" x="0" y="0" w="collapsed" h="ch" background="sidebar"/>
    <widget id="title" x="collapsed+pad" y="pad" w="cw-collapsed-2*pad" h="30" font-size="16"/>
    <widget id="open" x="collapsed+pad" y="header" right="cx-collapsed/2" h="36"
            radius="medium" background="accent"/>
    <widget id="save" x="cx+collapsed/2" y="header" right="pad" h="36" radius="medium"/>
    <widget id="body" x="collapsed+pad" y="header+52" right="pad" bottom="footer"/>
    <widget id="hint" x="collapsed+pad" right="pad" bottom="pad" h="24"/>
  </layout>
</layouts>
```

wide 变体将按钮放在标题栏右侧。base 变体
将它们堆叠在标题下方并平均划分内容宽度：`x` 加
`right` 拉伸每个矩形，`cx` 定位分隔处。侧边栏
只改变 `w`。

### 4. 在 `resize` 中应用文档

向 app 添加一个 `layout: LayoutDocument` 字段，在
`new` 中从此辅助函数赋值：

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;

fn load_layout() -> LayoutDocument {
    LayoutDocument::parse(include_str!("../layouts/shell.buixml"))
        .expect("the embedded layout is valid")
}
```

然后将程序化的 `layout` 调用替换为 `apply_layout`。调色板
来自窗口，因此 `background="sidebar"` 和 `background="accent"`
针对活动主题解析。

```rust
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

`apply_layout` 在改变任何内容之前解析每个 id、矩形和表达式，因此失败时会在屏幕上保留先前的布局。同一
生成上的尺寸改变只重写矩形：内容和皮肤数据每个
文档生成只写入一次，因此用户编辑的文本不会被断点重置。

### 5. 理解表达式语言

每个几何属性都是一个表达式。它被解析一次，并在每次应用时
针对当前空间（客户区，或滚动画板的内容区域）
求值。

| 名称 | 值 |
|---|---|
| `cw`、`right` | 空间宽度，以 DIP 为单位。 |
| `ch`、`bottom` | 空间高度，以 DIP 为单位。 |
| `cx`、`cy` | 宽度 / 2、高度 / 2。 |
| `left`、`top` | `0`。 |
| `N%` | 该轴的 `N/100`：`x`、`w`、`right` 使用宽度；`y`、`h`、`bottom` 使用高度。 |

运算符是 `+`、`-`、`*` 和 `/`，具有优先级和括号。一个
`<const>` 是按声明顺序解析的文档作用域数字；常量可以使用
更早的常量，但不能使用空间变量或 `%`。水平形式
恰好是 `x`+`w`、`right`+`w` 或 `x`+`right` 之一；垂直形式是
`y`+`h`、`bottom`+`h` 或 `y`+`bottom` 之一。

## 预期结果

- 在 1100 DIP 时侧边栏为 220 DIP，标题位于两个按钮旁边，
  提示行位于内容下方。
- 低于 900 DIP 时 base 变体显示 56 DIP 的侧边栏和位于标题
  下方的按钮；六个控件在其他方面相同。
- 文档版本无需重新构建即可更改矩形。

## 故障排查

- **`xml.id.unknown-widget`** —— 文档命名了一个 `App::new` 未
  添加的 id。应用是原子的，因此什么都不会移动。添加控件或修正 id。
- **`xml.layout.base`** —— 没有变体具有 `min-width="0" min-height="0"`，或两个
  变体共享相同的最小值。保留恰好一个 base 变体。
- **`xml.geom.underdetermined`** —— 一个条目在某一轴上既没有尺寸也没有两条边。
  每个条目确定自己的矩形；没有测量过程。
- **缩小后控件保持陈旧矩形** —— 它在 `wide` 中声明
  但在 `base` 中缺失。在每个变体中声明每个 id（如有需要
  用 `visible="false"` 隐藏它），否则其最后矩形会保留。
- **布局从不随窗口变化** —— `select` 比较客户区 DIP；
  `min-width` 高于客户区的变体永远无法被选中。检查
  阈值以及 `resize` 确实运行。

## 下一步

[08 — 列表、表格与树](08-lists-tables-trees.md)：用虚拟化数据视图填充内容窗格。
