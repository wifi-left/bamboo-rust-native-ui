# 05 — 动态 XML

> 将矩形、属性和初始内容移入 `.buixml` 文件，并在运行时用
> `feature = "layout-xml"` 应用它。

## 你将构建

[03](03-first-window.md) 中的窗口，其布局由
`layouts/main.buixml` 描述。在本教程中，Rust 侧创建标签和
按钮；文档定位它们、给按钮设置样式并设置初始
标签。一个 250 ms 的定时器轮询该文件并在变化时重新加载，因此编辑
XML 会更新运行中的窗口。一个可选步骤还展示了文档
如何通过 `WidgetRegistry` 自行创建控件。

## 前置条件

- [03](03-first-window.md) 中的项目可以构建并运行。
- 在 `bamboo-hello/Cargo.toml` 中启用布局特性：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

`layout-xml` 添加解析器（`LayoutDocument::parse`）和应用器
（`Ui::apply_layout`）。它不改变任何现有 API。

## 步骤

### 1. 创建文档

创建 `bamboo-hello/bamboo-hello/layouts/main.buixml`：

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <const name="pad" value="24"/>

  <layout id="base" min-width="0" min-height="0">
    <widget id="status"
            x="pad" y="24" w="cw-2*pad" h="28"
            label="No clicks yet"/>

    <widget id="greet"
            right="pad" y="64" w="140" h="36"
            label="Say hello"
            radius="medium" background="accent"/>
  </layout>
</layouts>
```

每个矩形都以 DIP 为单位。`cw` 和 `ch` 是客户区宽度和高度；
`<const>` 是文档作用域的数字。水平形式是 `x`+`w`、
`right`+`w` 或 `x`+`right` 之一，垂直形式是 `y`+`h`、
`bottom`+`h` 或 `y`+`bottom` 之一。文档必须包含一个最小值为
`0,0` 的变体；匹配的最宽变体胜出。

### 2. 在 Rust 中创建控件，但不带矩形

默认情况下，皮肤只会*寻址*应用已经创建的控件。
像之前一样创建控件，让文档分配矩形：

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;
use bamboo_native_ui::widgets::{Button, Label};
use bamboo_native_ui::Ui;

struct App {
    ui: Ui<Msg>,
    clicks: u32,
    layout: LayoutDocument,
    modified: Option<std::time::SystemTime>,
}

fn build() -> App {
    let mut ui = Ui::new();
    ui.add(Label::new("status", "No clicks yet"));
    ui.add(Button::new("greet", "Say hello", Msg::Greet));
    let layout = LayoutDocument::parse(include_str!("../layouts/main.buixml"))
        .expect("the embedded layout is valid");
    App {
        ui,
        clicks: 0,
        layout,
        modified: None,
    }
}
```

文件中的 id（`status`、`greet`）必须与传给 `new` 的 id 匹配。如果
某个不存在，应用会报告 `xml.id.unknown-widget` 并且不作任何更改。

### 2b. 可选 —— 让文档创建控件

安装 `WidgetRegistry`，让 `type="…"` 条目创建 id
尚不存在的控件。标准注册表知道十二种类型（`label`、`button`、
`toggle`、`slider`、`progress`、`icon`、`badge`、`card`、`divider`、`kbd`、
`avatar`、`spinner`）；第三方控件从 Rust 注册自己的类型名。
交互式类型需要一个 `command`，应用将其映射到自己的
消息：

```rust
use bamboo_native_ui::creation::WidgetRegistry;

let mut registry = WidgetRegistry::<Msg>::standard();
registry.set_commands(|id| match id {
    101 => Some(Msg::Greet),
    _ => None,
});
ui.set_widget_registry(registry);
```

将创建的条目添加到文档（Rust 中无匹配的 `ui.add`）：

```xml
<widget id="xml_hint" type="label"
        x="pad" y="112" w="cw-2*pad" h="24"
        label="Created by the document"/>

<widget id="xml_button" type="button" command="101"
        right="pad" y="112" w="120" h="32"
        label="XML button"/>
```

在重新加载时，新文件仍声明的一个已创建 id 会继续运行；一个
消失的则被移除；更改其 `type`、`command` 或 `parent`
会替换它。创建的控件归文档所有，`clear_layout`
会移除它们。其他内置控件和你自己的 `Widget<M>` 类型仍需要
一个 Rust 工厂 —— `type` 名称只会创建注册表知道的内容。

### 3. 在 `resize` 中应用它

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

`apply_layout(&document, &palette, client_size, sink)` 首先解析并验证
每个 id、矩形和表达式。出现任何错误时它不做任何更改，并且
先前安装的布局保持生效。成功时它记录一次损坏
并集并保存结果，`ui.installed_layout()` 暴露该结果。

`sink` 是任意 `UiHost`；`WindowHandle` 实现了它。`window.palette()` 是
窗口正在绘制所用的调色板，文档的 `background` 令牌
按它解析。

### 4. 在变化时重新加载

库不执行任何文件 IO，也不启动任何监视器。轮询文件并调用
`parse`，然后调用 `apply_layout`，在任一步骤失败时保留旧文档：

```rust
fn reload(&mut self, text: &str, window: &mut WindowHandle<'_>) {
    let document = match LayoutDocument::parse(text) {
        Ok(document) => document,
        Err(_) => return, // keep the working layout
    };
    if document.generation() == self.layout.generation() {
        return; // byte-for-byte the same canonical model
    }
    let palette = *window.palette();
    let size = window.size();
    match self.ui.apply_layout(&document, &palette, size, window) {
        Ok(_) => self.layout = document,
        Err(failure) => eprintln!("layout rejected: {failure}"),
    }
}
```

`generation()` 是规范模型的哈希，因此只改注释的编辑
什么都不做。只在生成变化时才写入内容初始值；
尺寸改变或断点切换永远不会覆盖用户输入的文本。

用定时器驱动它，并且只在文件修改时间变动时读取文件。
从 crate 目录解析路径，这样工作目录
无关紧要：

```rust
fn layout_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("layouts/main.buixml")
}

impl Host for App {
    fn started(&mut self, window: &mut WindowHandle<'_>) {
        window.set_animation_timer(Some(std::time::Duration::from_millis(250)));
    }

    fn timer(&mut self, window: &mut WindowHandle<'_>) {
        let Ok(modified) = std::fs::metadata(layout_path())
            .and_then(|meta| meta.modified())
        else {
            return;
        };
        if self.modified == Some(modified) {
            return;
        }
        self.modified = Some(modified);
        if let Ok(text) = std::fs::read_to_string(layout_path()) {
            self.reload(&text, window);
        }
    }
}
```

定时器每秒唤醒消息循环四次；除非重新加载改变了某些内容，
否则它不绘制。这保持了“空闲即零绘制”规则。

要丢弃文档并回到应用拥有的矩形，调用
`ui.clear_layout(window)`。它交还每个被认领的字段，并移除
文档创建的控件；矩形和内容不会恢复。

### 5. XML 能做什么和不能做什么

文档**能**：

- 为现有控件 id 分配矩形：锚点、百分比、算术、
  常量，以及按客户区尺寸选择的断点变体；
- 设置运行时属性：`visible`、`enabled`、`interactive`、`focusable`、
  `focus-order`、`read-only`、`tooltip`、`name`、`description`、
  `automation-id`、`role`、`expose`、`cursor`；
- 设置样式令牌：`radius`、`corners`、`background`（一个调色板令牌、
  `#rrggbb` 或 `none`）、`font-size`、`font-family`；
- 设置内容初始值：`text`、`label`、`value`、`progress`、`number`、
  `checked`、`selected`；
- 描述已注册的滚动画板（`<scroll>` 条目）；
- 通过皮肤协议设置控件声明的局部属性；
- 以子元素形式，为声明了那些列表的控件提供控件数据（`<item>`、`<row>`、`<option>`、`<track>`）；
- 创建控件，但仅当应用安装了
  `WidgetRegistry`（`ui.set_widget_registry`）且条目声明了 `type`
  （交互式类型还需 `command`）。重新加载可以在不重新编译 Rust 的情况下添加、替换或移除
  那些创建的 id。

它**不能**：

- 在该注册表之外创建、移除、重排或重新挂接控件；
- 实时绑定控件的数据：子列表在生成
  变化时替换数据模型，它们不是双向绑定；
- 安装整个调色板（那是一个 `.buitheme` 主题文档）；逐条目文本
  颜色只存在于文本控件上由控件声明并发布的 `color` 属性中
  （`Label` 及其同类、文本字段），而不是通用令牌；
- 在样式或内容值中使用表达式（内容是字面量）；
- 实时绑定内容：值是初始值，而不是绑定；
- 包含其他文件、使用 DTD，或自行运行监视器。

## 预期结果

运行程序会显示相同的窗口，标签在顶部，
按钮在右侧，二者均由文件定位。编辑
`layouts/main.buixml`，例如将按钮的 `w` 改为 `200`，会在四分之一秒内
重绘窗口，无需重新构建。写错（错误的表达式、未知 id）会记录失败并在屏幕上保留上一个可用的
布局。

## 故障排查

- **`LayoutDocument` 或 `layout_xml` 无法解析** —— 未启用 `layout-xml`
  特性。检查 `Cargo.toml`。
- **`xml.id.unknown-widget`** —— 文档命名了一个未用
  `ui.add` 添加的 id。应用是原子的，因此什么都不会移动。
- **`xml.layout.base`** —— 没有 `<layout>` 具有 `min-width="0" min-height="0"`，或
  两个变体共享最小值。保留一个 `0,0` 回退变体。
- **`xml.geom.underdetermined`** —— 一个条目既没有 `w`/`h` 也没有两条边。
  每个条目必须确定一个矩形；没有测量过程。
- **编辑后文本回退** —— 一次保存改变了生成，内容
  初始值被重写。这是预期的时机；如果应用拥有文本，移除
  内容属性。

## 下一步

[06 — 编译期 XML](06-xml-compiled.md)：将相同的布局生成为 Rust 并
去掉运行时解析器。
