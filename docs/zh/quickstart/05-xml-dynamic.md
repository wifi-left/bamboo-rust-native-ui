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

**本节新术语**

- **XML** —— 一种用于嵌套数据的纯文本格式。*元素*是像 `<widget>` 这样的
  标签；*属性*是标签内的名值对，例如 `id="status"`。*注释*写作
  `<!-- ... -->`，解析器会忽略它。
- **根元素** —— XML 文件中唯一的最外层元素。这里是
  `<layouts>`；其他所有元素都位于它内部。
- **`.buixml` 布局文档** —— 遵循框架布局模式的 XML 文件：
  常量、变体、控件及其几何。
- **Cargo 特性开关** —— `Cargo.toml` 中的一个按需启用开关。位于
  特性之后的代码只有在列出该特性时才会被编译，一个 crate 借此
  提供可选的 XML 解析器。
- **`LayoutDocument`** —— 文件在内存中、已经校验过的形式。
  解析把文本变成这个值；应用把它变成矩形。
- **应用（apply）** —— 把文档中的矩形、属性和初始内容
  复制到 Rust 已经创建的控件上。
- **热重载** —— 程序运行期间重新读取文件，无需重新构建。
- **变体** —— 文档中的一个 `<layout>` 块；应用器按
  客户区尺寸选一个。第 07 页用两个变体；本页只需要 `0,0`
  回退变体。
- **表达式** —— 像 `cw-2*pad` 这样的几何值，由客户区尺寸
  和文档常量计算得到。
- **`WidgetRegistry`** —— 一张把 `type="..."` 名称映射到创建该控件的
  代码的表，只在可选步骤中使用。

## 前置条件

- [03](03-first-window.md) 中的项目可以构建并运行。
- 在 `bamboo-hello/Cargo.toml` 中启用布局特性：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

`layout-xml` 添加解析器（`LayoutDocument::parse`）和应用器
（`Ui::apply_layout`）。它不改变任何现有 API。

这段代码替换第 02 页 `bamboo-hello/bamboo-hello/Cargo.toml` 中的依赖行 ——
也就是同时包含 `src/` 的 crate 文件夹，而不是它上面的
工作区文件夹。*特性* 在编译期按需启用：没有 `layout-xml`，
解析器和应用器就不在编译后的 crate 中，因此编译器会把它们
报告为缺失，而不是在运行时失败。

本页所有路径都相对该 crate 文件夹。`cargo build` 和
`cargo run` 在 crate 文件夹或上一级的工作区文件夹中都能工作，
因为 Cargo 会自行找到工作区清单。

## 步骤

### 1. 创建文档

创建 `bamboo-hello/bamboo-hello/layouts/main.buixml`。该路径从第 01 页的
工作区文件夹算起；如果你的终端已经在
crate 文件夹中，就在 `src` 旁边创建 `layouts/main.buixml`。输入完整名称：如果你的编辑器自动追加
`.txt`，请去掉它，因为构建期的
`include_str!` 和重载定时器都只认 `main.buixml`。

```xml
<?xml version="1.0" encoding="utf-8"?>
<!-- 唯一的根元素容纳整个文档。 -->
<layouts version="1">
  <!-- 文档作用域的数字，供下面的几何使用。 -->
  <const name="pad" value="24"/>

  <!-- 必需的 0,0 回退变体。 -->
  <layout id="base" min-width="0" min-height="0">
    <!-- x/y/w/h 以 DIP 为单位；"pad" 和 "cw" 是表达式。 -->
    <widget id="status"
            x="pad" y="24" w="cw-2*pad" h="28"
            label="No clicks yet"/>

    <!-- 右边缘的带样式按钮；id 与一个 Rust 控件匹配。 -->
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

把文件读作一棵树：`<layouts>` 是根元素，每个 `<const>`、
`<layout>` 和 `<widget>` 都是元素，`id="status"` 是属性。
空元素以 `/>` 结尾；注释以 `<!--` 开始、以 `-->` 结束。
XML 区分大小写，每个属性值都必须加引号。

保存文件暂时不会改变任何东西：在下一步 Rust 读取它之前，
它仍是一段惰性文本。

### 1b. 几何表达式详解

`x`、`y`、`w`、`h`、`right`、`bottom` 的值都是表达式。它们在**布局空间**里求值——
通常是窗口客户区，或已注册滚动面板的内容区——结果是一个 DIP 数值。

#### 六种形式

一个矩形需要恰好一个横向形式和一个纵向形式：

| 轴 | 写法 | 含义 |
|---|---|---|
| 横向 | `x` + `w` | 左边缘和宽度 |
| 横向 | `right` + `w` | 距右边缘的距离和宽度 |
| 横向 | `x` + `right` | 左右边缘都给；宽度拉伸 |
| 纵向 | `y` + `h` | 上边缘和高度 |
| 纵向 | `bottom` + `h` | 距下边缘的距离和高度 |
| 纵向 | `y` + `bottom` | 上下边缘都给；高度拉伸 |

“恰好一个”是字面意思：每个轴两个值。三个（`x`、`w`、`right`）是过定，
一个是不定。两者都是文档错误，不是警告。

#### 表达式的组成部分

| 成分 | 例子 | 含义 |
|---|---|---|
| 数字 | `24`、`1.5` | DIP |
| 百分比 | `50%` | 该属性所在轴的百分比 |
| 常量 | `pad` | 由 `<const name="pad" value="24"/>` 声明的值 |
| 空间变量 | `cw`、`ch`、`cx`、`cy` | 客户区（或内容区）的宽、高、中心 |
| 边缘变量 | `left`、`top`、`right`、`bottom` | `0` 和空间的远端边缘 |
| 运算符 | `+ - * /` | 算术；`*` 和 `/` 优先级更高 |
| 括号 | `(cw-48)/2` | 先算括号里的 |

百分比的轴向最容易搞错：横向属性（`x`、`w`、`right`）里的百分比按 `cw` 算；
同一个数字写在纵向属性（`y`、`h`、`bottom`）里就按 `ch` 算。所以
`w="50%"` 是宽度的一半，`h="50%"` 是高度的一半。

`cw`、`ch`、`cx`、`cy` 都从布局空间算，不从父控件算：这里是扁平坐标系，
不是嵌套的 flexbox。表达式里的 `right`、`bottom` 也指空间的右边缘和下边缘。

#### 在教程窗口上算一遍

第 03 页的窗口客户区是 520×320 DIP。取 `pad = 24`，下面这些写法会得到表中
的结果：

| 目标 | 写法 | 解析结果 |
|---|---|---|
| 全宽，左右各 24 DIP | `x="24" w="cw-48"` | `x = 24`，`w = 472` |
| 右对齐，宽 140 | `right="24" w="140"` | `x = 356`，`w = 140` |
| 居中，宽 140 | `x="cx-70" w="140"` | `x = 190`，`w = 140` |
| 半宽 | `x="24" w="50%"` | `x = 24`，`w = 260` |
| 带偏移的百分比 | `x="25%+8" w="50%-16"` | `x = 138`，`w = 244` |
| 64 DIP 页眉下方拉伸，底部留 24 | `y="64" bottom="24"` | `y = 64`，`h = 232` |
| 底栏高 36，距底部 24 | `bottom="24" h="36"` | `y = 260`，`h = 36` |
| 两列等宽，间距 16 | `x="24" w="(cw-64)/2"` 与 `x="40+(cw-64)/2" w="(cw-64)/2"` | `w = 228`；`x = 24` 和 `x = 268` |
| 居中的 200×100 卡片 | `x="cx-100" y="cy-50" w="200" h="100"` | `x = 160`，`y = 110` |

为什么是 `(cw-64)/2`？64 是左右各 24 DIP 的边距加上 16 DIP 的列间距；
剩下的宽度由两列平分。第二列从第一列右边再隔一个间距开始：
`24 + 228 + 16 = 268`。

#### 运算符优先级

`*` 和 `/` 的优先级高于 `+` 和 `-`，括号可以改变顺序。下面两行结果不同：

- `w="cw-2*pad"` → `cw - (2*pad)` = 472。
- `w="(cw-2)*pad"` → `(520 - 2) * 24` = 12432，远超窗口。

拿不准就加括号；解析器不会猜你的意思。

#### 常见错误

| 现象 | 原因 | 改法 |
|---|---|---|
| `w="50"` 得到 50 DIP 的按钮 | `50` 是数字，不是百分比 | `w="50%"` |
| “半宽”控件从中间开始 | `x="50%"` 移动的是左边缘，不是宽度 | 用 `w="50%"`；要居中就用 `x="25%" w="50%"` |
| `xml.const.undefined` | 常量名拼错，或在使用之后才声明 | 把 `<const>` 放到使用它的 `<layout>` 之前 |
| `xml.geom.overdetermined` | `x`/`right`/`w` 给了三个（纵向同理） | 删掉一个 |
| `xml.geom.underdetermined` | 横向或纵向只给了一个值 | 补上缺失的那一对 |
| 控件跑到屏幕外 | `cx` 是中心，不是右边缘；`cx+100` 已经越过中心 | 减去一半宽度：`x="cx-100" w="200"` |

#### 滚动内容

在已注册的滚动面板里，同一套表达式在面板的**内容**空间中求值：`cw`/`ch`
是内容宽高，不是可见视口。宽表格或长列表因此可以得到比窗口更高的矩形。
使用 `content-height="auto"` 时，`ch` 会晚一次应用，底部锚定的子项可能在
第一帧跳动；在意这一点时请显式写 `content-height`。

[XML 元素参考](../reference/xml-elements.md)用 1000×700 的空间列出了同一组
配方；第 07 页用这些表达式做断点。试着把 `greet` 按钮的 `w="140"` 改成
`w="200"` 再重载：它用 `right` 锚定，所以右边缘不动，左边缘会向左移动。

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
    // 当前已安装的文档；一次重载会替换它。
    layout: LayoutDocument,
    // 上次轮询看到的时间戳；`None` 表示文件还没被读过。
    modified: Option<std::time::SystemTime>,
}

fn build() -> App {
    let mut ui = Ui::new();
    // 控件必须先存在，文档才能寻址它们的 id。
    ui.add(Label::new("status", "No clicks yet"));
    ui.add(Button::new("greet", "Say hello", Msg::Greet));
    // include_str! 在构建时把文件文本烘焙进二进制，因此
    // 首次解析从不访问磁盘。下面的重载会再次读取它。
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

XML 是声明式的：它描述控件坐在*哪里*、*长什么样*，却不能凭空
造出消息处理器或应用状态。`Button` 仍然拥有它的 `Msg::Greet`
行为，因为它是 Rust 创建的。这就是本页在 Rust 中创建控件、只把
矩形、样式令牌和初始标签移入文档的原因。

`include_str!` 是编译期文本包含；路径相对包含它的源文件
（`src/main.rs`），而不是你运行 Cargo 的文件夹。如果构建时文件缺失，
编译器会以 `couldn't read ../layouts/main.buixml` 停下。保持该路径
不变，并且第 1 步的 crate 文件夹位置就足够了。

**刚才发生了什么。** `App` 现在携带一个已解析的 `LayoutDocument`，
两个控件都存在但没有可用的矩形。在第 3 步 `resize` 应用文档之前，
屏幕上看不出任何变化。

### 2b. 可选 —— 让文档创建控件

安装一个 `WidgetRegistry`，让 `type="…"` 条目创建 id
尚不存在的控件。标准注册表知道十二种类型（`label`、`button`、
`toggle`、`slider`、`progress`、`icon`、`badge`、`card`、`divider`、`kbd`、
`avatar`、`spinner`）；第三方控件从 Rust 注册自己的类型名。
交互式类型需要一个 `command`，应用将其映射到自己的
消息：

```rust
use bamboo_native_ui::creation::WidgetRegistry;

// 一张工厂表：`type="label"` 等会变成构建控件的代码。
let mut registry = WidgetRegistry::<Msg>::standard();
// 创建出的交互式控件会报告命令 id；把它们映射到你的消息。
registry.set_commands(|id| match id {
    101 => Some(Msg::Greet),
    _ => None,
});
// 安装到 Ui 上；每次应用都会为缺失的 id 查询它。
ui.set_widget_registry(registry);
```

将创建的条目添加到文档（Rust 中无匹配的 `ui.add`）：

```xml
<!-- 这个 id 没有 ui.add：type="label" 让注册表创建它。 -->
<widget id="xml_hint" type="label"
        x="pad" y="112" w="cw-2*pad" h="24"
        label="Created by the document"/>

<!-- command="101" 由上面的 set_commands 变成 Msg::Greet。 -->
<widget id="xml_button" type="button" command="101"
        right="pad" y="112" w="120" h="32"
        label="XML button"/>
```

在重新加载时，新文件仍声明的一个已创建 id 会继续运行；一个
消失的则被移除；更改其 `type`、`command` 或 `parent`
会替换它。创建的控件归文档所有，`clear_layout`
会移除它们。其他内置控件和你自己的 `Widget<M>` 类型仍需要
一个 Rust 工厂 —— `type` 名称只会创建注册表知道的内容。

注册表只会为 id 尚不存在的 `type` 条目而查询；
第 2 步的控件永远不会被替换。`command` 是一个普通数字，
`set_commands` 是从这些数字到你的类型化消息的桥梁。没有
安装注册表时，同样的条目会以 `xml.type.unknown` 失败，并且
应用不作任何更改。

**刚才发生了什么。** 这个可选步骤给文档赋予了有限的
构造能力：两个 id 只存在于 XML 中的控件。它们的几何、标签
和命令仍然走同一个应用器，因此重载可以在不重新编译 Rust 的情况下
添加、替换或移除它们。

### 3. 在 `resize` 中应用它

```rust
impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        // 文档的颜色令牌按这个调色板解析。
        let palette = *window.palette();
        // 先校验，再移动文档命名的每个 id。
        if let Err(failure) = self.ui.apply_layout(&self.layout, &palette, size, window) {
            eprintln!("layout rejected: {failure}");
        }
        // 一次重绘请求，覆盖发生变化的矩形。
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

`resize` 由框架在启动时调用，之后每当客户区
尺寸变化时再次调用，因此这一个方法同时覆盖首次布局和后续
尺寸调整。`size` 参数是 DIP 为单位的客户区，与
文档数字的单位相同。

**刚才发生了什么。** 在首次 `resize` 时，文档替换了第 03 页的控件
矩形：标签横跨宽度减去两个 `pad` 边距，
按钮距离右边缘 `pad` DIP。`radius="medium"` 和
`background="accent"` 重新给按钮设置了样式。如果出现以
`layout rejected:` 开头的行，说明控件保留了构造函数给的默认矩形，
这通常会让窗口看起来为空或错位。

### 4. 在变化时重新加载

库不执行任何文件 IO，也不启动任何监视器。轮询文件并调用
`parse`，然后调用 `apply_layout`，在任一步骤失败时保留旧文档：

```rust
fn reload(&mut self, text: &str, window: &mut WindowHandle<'_>) {
    let document = match LayoutDocument::parse(text) {
        Ok(document) => document,
        Err(_) => return, // 保留可用的布局
    };
    if document.generation() == self.layout.generation() {
        return; // 逐字节相同的规范模型
    }
    let palette = *window.palette();
    let size = window.size();
    match self.ui.apply_layout(&document, &palette, size, window) {
        Ok(_) => self.layout = document, // 只记住成功应用的文档
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
    // 在编译期固定到 crate 文件夹；shell 的 cwd 无关紧要。
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("layouts/main.buixml")
}

impl Host for App {
    fn started(&mut self, window: &mut WindowHandle<'_>) {
        // 250 ms 的心跳：每秒检查四次文件，本身不绘制。
        window.set_animation_timer(Some(std::time::Duration::from_millis(250)));
    }

    fn timer(&mut self, window: &mut WindowHandle<'_>) {
        // metadata() 很便宜；只有时间戳移动了才会去读取。
        let Ok(modified) = std::fs::metadata(layout_path())
            .and_then(|meta| meta.modified())
        else {
            return; // 文件被移走或不可读：保留运行中的布局
        };
        if self.modified == Some(modified) {
            return; // 与上一 tick 的时间戳相同
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

**刚才发生了什么。** 现在一次保存就能到达运行中的窗口：时间戳
变化，文本被解析，新的生成被应用，发生变化的
矩形重绘。`generation()` 会过滤噪声，因此保存但布局未变
（或只编辑了注释）不会引起重绘。

**运行它。** 在 `bamboo-hello/`（包含外层 `Cargo.toml` 的工作区文件夹）
或 crate 文件夹 `bamboo-hello/bamboo-hello/` 中打开终端；
两者都可以。然后运行：

```powershell
cargo run
```

会出现一个 520×320 的窗口，标签在顶部，按钮在
右侧。保持它打开，把 `layouts/main.buixml` 中的 `w="140"` 改成 `w="200"`，
保存，按钮会在约四分之一秒内变宽。无需重新构建，也无需重启。

### 5. XML 能做什么和不能做什么

把文档看作已存在控件的配置，再加上第 2b 步里范围很窄的
创建例外。这份清单就是契约；未列出的所有内容都留在应用代码中。

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

磁盘上的文件和构建时嵌入的文本是两份副本：首次应用使用
嵌入的副本，定时器读取的是磁盘。因此重启程序会从
编译进来的版本开始，而不是从最后一次运行时编辑开始。运行时重载
永远不会把内容写回文件。

## 故障排查

- **`LayoutDocument` 或 `layout_xml` 无法解析** —— 未启用 `layout-xml`
  特性。检查 `Cargo.toml` 和拼写是否完全正确；Cargo 会拒绝未知
  特性，而不是忽略它。
- **`xml.id.unknown-widget`** —— 文档命名了一个未用
  `ui.add` 添加的 id。应用是原子的，因此什么都不会移动。将 `id` 属性
  与传给 `new` 的字符串比较；id 区分大小写。
- **`xml.layout.base`** —— 没有 `<layout>` 具有 `min-width="0" min-height="0"`，或
  两个变体共享最小值。保留一个 `0,0` 回退变体。
- **`xml.geom.underdetermined`** —— 一个条目既没有 `w`/`h` 也没有两条边。
  每个条目必须确定一个矩形；没有测量过程。
- **带行号和列号的 `xml.syntax`** —— XML 本身格式错误：
  标签未闭合、属性值缺少引号，或空元素丢了 `/`。解析器在
  第一个错误处停止。重新加载期间运行中的窗口会保留上一个好的
  布局；在首次解析时 `build` 中的 `.expect` 会 panic，所以在 `cargo run`
  之前先修好文件。
- **`xml.unknown-attribute` 或 `xml.unknown-element`** —— 名字拼写错误。两个
  名字都区分大小写；是 `background`，不是 `Background`。
- **编辑文件后什么都没变** —— 先确认编辑器已保存。
  许多编辑器会把文本留在内存中，直到你按 Ctrl+S，还有一些在
  扩展名不熟悉时把文件改名为 `main.buixml.txt`。查看
  终端：那里的解析错误能解释为什么旧布局仍然保留。
- **编辑永远到不了窗口** —— 定时器读取
  `<crate>/layouts/main.buixml`。保存到 `src/` 或工作区
  文件夹中的是另一个文件。构建期的 `include_str!` 路径和
  运行时路径必须指向同一个文件。
- **窗口为空或错位，而不是显示标签** —— 每次
  应用都失败了，所以控件保留了构造函数的矩形。读第一条 `layout rejected:` 行；它会给出错误码和文档
  位置。
- **编辑后文本回退** —— 一次保存改变了生成，内容
  初始值被重写。这是预期的时机；如果应用拥有文本，移除
  内容属性。
- **点击按钮没有任何反应** —— 文档可能设置了
  `enabled="false"` 或 `interactive="false"`，而且你第 03 页的 `apply` 仍然
  必须把 `Msg::Greet` 变成标签变化。两者都要检查。

## 下一步

[06 — 编译期 XML](06-xml-compiled.md)：将相同的布局生成为 Rust 并
去掉运行时解析器。
