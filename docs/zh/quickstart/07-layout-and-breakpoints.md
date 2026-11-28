# 07 — 布局与断点

> 两次计算响应式外壳：先在 `Host::resize` 中使用
> `bamboo_native_ui::layout` 辅助函数，然后用按客户区
> 尺寸选择的 `.buixml` 断点变体。

## 你将构建

一个窗口，带侧边栏、含两个按钮的标题栏、内容窗格和状态
行。客户区窄于 900 DIP 时，侧边栏从 220
DIP 折叠到 56 DIP；高于阈值时再次展开。本页的
前半部分用 `VStack`、`HStack`、`split_h`、`split_v`、`grid` 和
`padded` 完成。后半部分将同样的六个控件移入带
`wide` 和 `base` 变体的 `.buixml` 文档，并让 `Ui::apply_layout` 选择一个。

**本节新术语**

- **DIP** —— 设备无关像素。框架以 DIP 布局；宿主把它们缩放为物理
  像素，因此这些数字在 100%、150% 或 200% 显示缩放下
  仍然有意义。
- **客户区** —— 窗口的可绘制内部区域，不含边框、
  标题栏和任何原生菜单栏。`resize` 中的 `size` 就是客户区。
- **布局辅助函数** —— 一个纯函数，接受一个 `Rect` 并返回一个或
  多个更小的 `Rect`。它不绘制任何东西，也不拥有任何东西。
- **`VStack` / `HStack`** —— 区域上的一个游标：`next(n)` 取下一行
  或单元格，`remaining()` 返回剩余部分。
- **`split_h` / `split_v`** —— 把区域划分为带沟槽的加权单元格。
- **`grid`** —— 按行优先把区域平铺为相等单元格。
- **`padded`** —— 按内边距收缩矩形；这里就是单行的页面边距。
- **断点** —— 客户区尺寸阈值，到达它时换用不同的布局
  （这里是：900 DIP 宽）。
- **变体** —— 文档中的一个 `<layout>` 块。文档为当前客户区尺寸
  选择最合适的变体。
- **表达式** —— 像 `cw-expanded-2*pad` 这样的几何属性值。它
  解析一次，并在每次应用时按当前空间求值。

## 前置条件

- [03](03-first-window.md) 中的窗口可以构建并运行。
- 对于后半部分，像 [05](05-xml-dynamic.md) 那样启用 `layout-xml`：

```toml
[dependencies]
# 只有后半部分需要它；第 1-2 步使用纯 Rust。
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

`bamboo_native_ui::layout` 中的辅助函数无需特性开关。

本页所有文件都位于 crate 文件夹 `bamboo-hello/bamboo-hello/`：
在那里编辑 `src/main.rs`，并在 `src` 旁边的 `layouts`
文件夹中创建 `layouts/shell.buixml`。从该 crate 文件夹或上一级的
工作区文件夹运行 `cargo run`。

## 步骤

### 1. 创建外壳控件

一次创建六个控件。它们与文档稍后要寻址的 id 相同，
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
        // 六个控件，只创建一次；无论辅助函数代码还是下面的
        // 文档，都不会创建或移除它们。
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
    // 一个页面内边距；其余每个矩形都由它推导。
    let page = padded(
        Rect::from_xywh(0.0, 0.0, size.width, size.height),
        Insets::uniform(12.0),
    );

    // 标题行，然后是它下方的所有内容。
    let mut rows = VStack::new(page, 10.0);
    let header = rows.next(44.0);
    let body = rows.remaining();

    // 标题栏：标题在左，两个等宽按钮单元格在右。
    let header_cells = split_h(header, &[3.0, 1.0], 10.0);
    ui.set_rect(WidgetId::new("title"), header_cells[0]);
    let actions = grid(header_cells[1], 2, 1, 8.0);
    ui.set_rect(WidgetId::new("open"), actions[0]);
    ui.set_rect(WidgetId::new("save"), actions[1]);

    // 侧边栏宽度跟随断点；内容占用其余部分。
    let mut columns = HStack::new(body, 10.0);
    let sidebar_width = if size.width < 900.0 { 56.0 } else { 220.0 };
    let sidebar = columns.next(sidebar_width);
    let content = columns.remaining();
    ui.set_rect(WidgetId::new("sidebar"), sidebar);

    // 内容：一个主窗格和一条页脚带，按权重拆分。
    let content_cells = split_v(content, &[5.0, 1.0], 10.0);
    ui.set_rect(WidgetId::new("body"), content_cells[0]);
    ui.set_rect(WidgetId::new("hint"), content_cells[1]);
}

impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        // `size` 是以 DIP 为单位的客户区；布局只在尺寸调整时运行。
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
计数钳制为至少一。`padded` 即 `rect.inset_by(insets)`。想要沟槽时用拆分
辅助函数，就像这里的标题栏和内容列那样。

跟随一次计算，看看矩形是如何得出的。在 1100×700 的客户区上，12 DIP 的内边距
留下 1076×676 的页面。标题栏取前 44 DIP，
主体得到其余部分。权重为 `[3.0, 1.0]`、间隙 10 DIP 的 `split_h` 让
标题单元格宽 799.5 DIP，动作单元格宽 266.5；`grid` 再把
动作单元格切成两个各 129.25 DIP 的按钮。主体的 `HStack` 给
侧边栏 220 DIP，内容得到剩余的 856。

**刚才发生了什么。** 现在每个控件都有由同一个页面
内边距推导出的矩形。把窗口拖得比 900 客户区 DIP 更窄，`resize` 会以更小的 `size`
再次运行：同一个 `if` 为侧边栏选择 56，标题栏、
按钮和内容都从新的页面重新计算。没有控件被重建，
`invalidate_all` 重绘之外不绘制任何东西。

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
  <!-- 文档作用域的数字：下面表达式使用的名称。 -->
  <const name="pad" value="16"/>
  <const name="collapsed" value="56"/>
  <const name="expanded" value="220"/>
  <const name="header" value="56"/>
  <const name="footer" value="64"/>

  <!-- 在 900 客户区 DIP 及以上时选用。 -->
  <layout id="wide" min-width="900">
    <widget id="sidebar" x="0" y="0" w="expanded" h="ch" background="sidebar"/>
    <widget id="title" x="expanded+pad" y="pad" w="cw-expanded-2*pad-272" h="30" font-size="20"/>
    <widget id="open" right="pad+132" y="pad" w="120" h="36" radius="medium" background="accent"/>
    <widget id="save" right="pad" y="pad" w="120" h="36" radius="medium"/>
    <widget id="body" x="expanded+pad" y="header" right="pad" bottom="footer"/>
    <widget id="hint" x="expanded+pad" right="pad" bottom="pad" h="24"/>
  </layout>

  <!-- 必需的 0,0 回退：低于 900 DIP 的一切。 -->
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

选择依据是最小值，而不是顺序。在 1100 DIP 宽的客户区上，
`LayoutDocument::select` 返回 `wide`（它适配且最小值最大）；在 700 DIP 时
唯一适配的变体是 `base`。文件里除了 `min-width="900"` 之外
没有命名任何像素断点。每个 id 都必须在
应该移动它的变体中声明 —— 缺失的条目会保留它先前的矩形。

### 4. 在 `resize` 中应用文档

向 app 添加一个 `layout: LayoutDocument` 字段，在
`new` 中从此辅助函数赋值：

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;

// 解析构建时嵌入的文本；文件缺失会阻止构建。
fn load_layout() -> LayoutDocument {
    LayoutDocument::parse(include_str!("../layouts/shell.buixml"))
        .expect("the embedded layout is valid")
}
```

然后将程序化的 `layout` 调用替换为 `apply_layout`。调色板
来自窗口，因此 `background="sidebar"` 和 `background="accent"`
按活动主题解析。

```rust
impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        // 文档的颜色令牌按这个调色板解析。
        let palette = *window.palette();
        // 校验全部六个 id 和表达式，然后移动它们。
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

启动时应用的是文档嵌入的副本。本页没有文件
监视器，不同于 [05](05-xml-dynamic.md)，因此在应用运行期间编辑 `shell.buixml`
不会有任何变化，直到你重新构建；调整尺寸只会重新应用
编译进来的副本。

**刚才发生了什么。** 第 1 步的六个控件现在由文档放置，
而不是手工放置。越过 900 客户区 DIP 会重新运行 `resize`，因此
`select` 切换变体，`apply_layout` 把同样的六个 id 移到
`base` 矩形。手写的 `layout` 函数不再被调用；
删除它（或保留它并接受你的 crate 产生的
`dead_code`/`unused_imports` 警告），让构建保持无警告。

### 5. 理解表达式语言

每个几何属性都是一个表达式。它被解析一次，并在每次应用时
按当前空间（客户区，或滚动画板的内容区域）
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

在 1100 DIP 宽客户区上，`shell.buixml` 的两个演算示例：

- wide 标题的 `w="cw-expanded-2*pad-272"` 是
  `1100 - 220 - 32 - 272 = 576` DIP。
- 在 base 变体中，`cx` 是 `1100 / 2 = 550`。第一个按钮结束于
  `1100 - (cx-collapsed/2) = 1100 - 522 = 578`，第二个开始于
  `cx+collapsed/2 = 578`，因此它们在分隔处相遇。在
  `x` 轴属性上写 `w="50%"` 也以同样方式表示空间宽度的一半。

`right` 和 `bottom` 属性是从右边缘和下边缘量起的内边距，
而不是绝对坐标；这就是为什么正的 `right` 会把控件
向左移。`cw`/`ch`/`cx`/`cy` 描述条目求值所在的空间，
因此在滚动画板内部它们指面板的内容区域，而不是
窗口。

## 预期结果

- 在 1100 DIP 时侧边栏为 220 DIP，标题位于两个按钮旁边，
  提示行位于内容下方。
- 低于 900 DIP 时 base 变体显示 56 DIP 的侧边栏和位于标题
  下方的按钮；六个控件在其他方面相同。
- 调整尺寸会在不重新构建的情况下重新应用已解析的文档；而更改
  `.buixml` 文件本身在本页确实需要重新构建，因为这里没有文件
  监视器。

**运行它。** 从工作区文件夹 `bamboo-hello/` 或 crate 文件夹
`bamboo-hello/bamboo-hello/` 运行：

```powershell
cargo run
```

会出现一个窗口，左侧是深色侧边栏，标题栏中是标题和两个按钮，
还有内容窗格和状态行。如果窗口仍然是
第 03 页的 520×320 那个，先把它加宽（拖动边缘，或在
`WindowBuilder` 上设置 `.size(1100.0, 700.0)`）—— 低于 900 客户区 DIP 时你只会
看到 `base`。从宽处开始，然后把右边缘向内拖过阈值：
侧边栏变窄，按钮移到标题下方。向外拖，宽
布局又回来。关键的数值是客户区宽度（以 DIP 计），
不是物理像素 —— 在 150% 显示缩放下，1350 像素宽的客户区
恰好是 900 DIP。

## 故障排查

- **`LayoutDocument` 或 `layout_xml` 无法解析** —— `layout-xml`
  特性关闭。第 1-2 步不需要它即可编译；第 4 步不行。
- **带位置的 `xml.syntax`** —— XML 格式错误（标签未闭合、
  属性未加引号、空元素缺少 `/`）。因为本页
  用 `.expect(...)` 解析一次，程序会在启动时 panic，窗口
  不会出现；修好文件并重新构建。
- **`xml.id.unknown-widget`** —— 文档命名了一个 `App::new` 未
  添加的 id。应用是原子的，因此什么都不会移动。添加控件或修正 id；
  第 1 步的六个 id 必须完全匹配。
- **`xml.layout.base`** —— 没有变体具有 `min-width="0" min-height="0"`，或两个
  变体共享相同的最小值。保留恰好一个 base 变体。
- **`xml.geom.underdetermined`** —— 一个条目在某一轴上既没有尺寸也没有两条边。
  每个条目确定自己的矩形；没有测量过程。
- **`xml.geom.overdetermined`** —— 一个条目在某一轴上给了太多，例如
  `x`、`right` 和 `w`。恰好使用 `x`+`w`、`right`+`w` 或
  `x`+`right` 之一（以及匹配的垂直形式）。
- **`xml.expr.syntax` 或 `xml.const.undefined`** —— 表达式或
  常量名拼写错误。常量先声明后使用；`undefined` 表示该名字
  从未在此文档中声明。
- **`xml.const.vars`** —— 一个 `<const>` 使用了 `cw`、`ch`、`cx`、`cy` 或 `%`。常量
  是固定数字；只有控件属性可以读取空间。
- **`error: couldn't read ...\layouts\shell.buixml`** —— `include_str!` 相对
  `src/main.rs` 解析，因此构建时文件必须存在。在
  `src` 旁边创建 `layouts/shell.buixml`，而不是在 `src` 内部。
- **编辑 XML 没有可见效果** —— 本页没有热重载。文本
  在构建时被嵌入；再次运行 `cargo build`/`cargo run`。调整尺寸
  只会重新应用旧的嵌入文档。
- **侧边栏在 900 时从不折叠** —— `resize` 收到的是客户区 DIP，因此
  高 DPI 窗口在更大的像素宽度处才到达阈值。检查实际
  传给 `layout`/`apply_layout` 的值，并确认 `resize` 确实在
  运行。
- **缩小后控件保持陈旧矩形** —— 它在 `wide` 中声明
  但在 `base` 中缺失。在每个变体中声明每个 id（如有需要
  用 `visible="false"` 隐藏它），否则会保留它最后的矩形。
- **布局从不随窗口变化** —— `select` 比较客户区 DIP；
  `min-width` 高于客户区的变体永远无法被选中。检查
  阈值，并确认 `resize` 确实在运行。
- **窗口空白，或控件堆叠在原点** —— 每次
  应用都被拒绝，所以手写的矩形没有了，控件
  停在构造函数的默认位置。读第一条 `layout rejected:` 行。

## 下一步

[08 — 列表、表格与树](08-lists-tables-trees.md)：用虚拟化数据视图填充内容窗格。
