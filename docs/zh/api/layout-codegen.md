# 布局代码生成与编译布局

> 在构建时将 `.buixml` 文档转换为纯 Rust，并通过共享的应用器安装它。

## 概述

编译模式是消费布局文档三种方式中的第二种。生成器读取 `.buixml`，将常量折叠进原生算术，
并输出一个普通的 Rust 模块：一个 `static LAYOUT: CompiledLayout`、每个矩形一个
`fn(Size) -> Result<Rect, RectError>`、用于属性和样式的 `LazyLock` 静态项，以及用于内容和
数据子项的 `&'static` 表。二进制中既没有 XML 解析器，也没有表达式解释器。

运行时部分位于 `bamboo_native_ui::layout_xml::compiled` 中，受 `feature = "layout-compiled"`
门控。`Ui::apply_compiled` 将描述符解析为 `Ui::apply_layout` 构建的同一内部计划，然后运行
共享的安装步骤——认领、内容时序、子项/内容/属性顺序以及损伤刷新在构造上完全相同。
`layout_parity` 断言两条路径产生相同的矩形、属性和内容。

| 路径 | 何时读取文件 | 运行时需要 |
|---|---|---|
| 动态 | 安装时以及每次重载时 | `layout-xml`（解析器 + 应用器） |
| 代码生成 CLI / 库 | 构建时一次，输出纳入版本控制 | `layout-compiled` |
| `layout_xml!` 过程宏 | 编译时，就地展开 | `layout-compiled` |

在编写、主题化或热重载时使用动态路径。在发布构建和大型静态页面中使用编译路径：启动时无需
解析、无需验证遍历、无需模型分配，且 `rustc` 会对每个拼接的值进行类型检查。

## 关键类型

### 生成器库与 CLI

Crate `bamboo-native-ui-layout-codegen`（库 + 二进制）：

```rust
pub fn module_name(source_name: &str) -> String;
// `layout-demo.buixml` -> `layout_demo`

pub fn generate(xml: &str, source_name: &str, module: &str)
    -> Result<String, Vec<LayoutDiagnostic>>;

#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    pub module: &'a str,
    pub context: Option<&'a str>,
}

pub fn generate_with(xml: &str, source_name: &str, options: Options<'_>)
    -> Result<String, Vec<LayoutDiagnostic>>;
```

```text
bamboo-native-ui-layout-codegen <input.buixml> [-o out.rs] [--module name] [--context Type]
```

`--module` 默认为 `module_name`；`--context Type` 选择下文的上下文模式。当文档出错时，CLI
会打印每条诊断及其 `.buixml` 行号和列号，并以状态 1 退出，不写入任何内容。

### 生成的模块结构

输出是普通源代码，包裹在一个模块中：

```rust
pub mod parity {
    pub static LAYOUT: CompiledLayout; // name, generation, variants, targets

    pub fn install<M: 'static, H: UiHost>(
        ui: &mut Ui<M>,
        sink: &mut H,
        palette: &Palette,
        client: Size,
    ) -> Result<LayoutReport, LayoutFailure>;
}
```

使用 `context = Type` 时，`install` 多接收一个参数（`context: &Type`）。`LAYOUT` 命名该
文件、携带 FNV-1a generation 和完整的预检目标列表，并持有各个变体。`install` 调用
`Ui::apply_compiled`（对于上下文模块则调用 `apply_compiled_with`）。

### 运行时词汇表

```rust
pub type RectFn = fn(Size) -> Result<Rect, RectError>;

pub enum RectError {
    Negative(&'static str),  // 某个宽/高边低于零
    NotFinite,               // 例如在此尺寸下除以零
    Context,                 // 仅通过生成的 `install` 存在
}

pub fn rect(x: f32, y: f32, width: f32, height: f32) -> Result<Rect, RectError>;
pub fn edges(left: f64, top: f64, right: f64, bottom: f64) -> Result<Rect, RectError>;

pub struct CompiledLayout {
    pub name: &'static str,
    pub generation: u64,
    pub variants: &'static [CompiledVariant],
    pub targets: &'static [CompiledTarget],
}

pub struct CompiledVariant {
    pub id: &'static str,
    pub min_width: f32,
    pub min_height: f32,
    pub entries: &'static [CompiledEntry],
    pub scrolls: &'static [CompiledScroll],
}

pub enum TargetKind { Widget, Scroll }
pub enum CompiledContent {
    Text(&'static str), Label(&'static str), Value(f64), Progress(f64),
    Number(i64), Checked(bool), Selected(u32),
}
pub enum CompiledHeight { Keep, Exact(fn(Size) -> Option<f32>), Auto }

impl CompiledLayout {
    pub fn select_index(&self, client: Size) -> usize;
    pub fn select(&self, client: Size) -> &CompiledVariant;
}
```

`CompiledEntry` 携带 id、源代码行/路径、`rect: RectFn`、`type_name`/`command`/`parent`、
`props`/`style` 静态项、内容、皮肤属性以及子项（以及一个可选的 `rust_children` 静态项）。
`edges` 将 `f64` 转换为 `f32`，与动态解析器完全相同，因此两条路径的舍入也一致。

### 应用

```rust
impl<M> Ui<M> {
    pub fn apply_compiled(
        &mut self,
        layout: &CompiledLayout,
        palette: &Palette,
        client: Size,
        sink: &mut impl UiHost,
    ) -> Result<LayoutReport, LayoutFailure> where M: 'static;

    pub fn apply_compiled_with(
        &mut self,
        layout: &CompiledLayout,
        values: &CompiledValues<'_>,
        palette: &Palette,
        client: Size,
        sink: &mut impl UiHost,
    ) -> Result<LayoutReport, LayoutFailure> where M: 'static;
}
```

两者都要求 `feature = "layout-compiled"`（`compiled` 模块仅在那时编译）。`apply_compiled`
传递空的 `CompiledValues`。每个目标 id 都会在第一次修改前被校验；失败会返回
`LayoutFailure` 且不改变任何内容。

### `layout_xml!`

```rust
use bamboo_native_ui_layout_macros::layout_xml;

layout_xml!("layouts/main.buixml");                        // mod main
layout_xml!("layouts/main.buixml", module = app_layout);   // mod app_layout
layout_xml!("layouts/main.buixml", context = AppLayout);   // install(.., &AppLayout)
```

该宏相对于 `CARGO_MANIFEST_DIR` 读取路径（使用正斜杠；不使用转义），运行相同的生成器并就地
展开模块。文档错误会变成命名 `.buixml` 行号和列号的 `compile_error!`。宏 crate 本身没有
特性标志；它发出的模块需要在依赖 crate 中启用 `feature = "layout-compiled"`。

## 示例

```rust
// 构建步骤：cargo run -p bamboo-native-ui-layout-codegen -- \
//   layouts/main.buixml -o src/main_layout.rs --module main_layout
include!("main_layout.rs");

use bamboo_native_ui::{Host, Size, WindowHandle};

impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let palette = *window.palette();
        if let Err(failure) = self
            .ui
            .apply_compiled(&main_layout::LAYOUT, &palette, size, window)
        {
            eprintln!("layout rejected: {failure}");
            return;
        }
        window.invalidate_all();
    }
}
```

`install` 执行相同的调用，但接收 `(ui, sink, palette, client)`——注意参数顺序与
`apply_compiled` 不同。

## 一致性与内联 Rust

- 两条路径共享 `Ui::install_prepared`，因此页面可以从动态切换到编译而无需编辑 XML。
  `cargo test -p bamboo-native-ui --features layout-xml,layout-compiled layout_parity`
  会断言这一点。
- `rust:` 值仅编译模式支持：几何代码被输出到生成的矩形函数内部，其中 `cw`、`ch`、`cx`、
  `cy` 以 `f64` 局部变量存在；属性/样式/内容代码进入条目的 `LazyLock`（或在有上下文时
  进入每次应用的值）。模块以 `use super::*;` 开头，因此包含它的模块中的常量和类型可按名
  解析。
- `context = Type` 在每个内联 Rust 值中绑定 `context`，并使生成的 `install` 接收 `&Type`。
  上下文矩形会从静态 `RectFn` 报告 `RectError::Context`，因此应通过
  `install` / `apply_compiled_with` 应用此类模块：
  `CompiledValues { variants: Vec<Vec<CompiledEntryValues<'_>>> }`。
- `<rust target="id" code="expr"/>` 替换一个控件的子项列表；该表达式必须生成
  `Vec<SkinElement>`。参见 `binding.buixml` 和 `context.buixml` 测试夹具。
- 动态应用器会以 `xml.rust.unsupported` 拒绝其中任何一项；运行时的 `.buixml` 保持为可移植
  数据。

## 说明与限制

- 特性门控：生成的输出需要 `feature = "layout-compiled"` 才能编译；解析器不会被链接。
  `layout-xml` 是独立的——一次构建可以拥有其中之一、两者，或都不拥有。
- 生成的代码会纳入版本控制（测试夹具的 `*_generated.rs` 是黄金测试），因此过期的模块会
  使代码生成测试套件失败。请重新生成，而不要手动编辑。
- 过程宏在稳定版 Rust 上无法注册文件依赖：编辑 `.buixml` 本身不会触发重建。当文档独立
  变化时，请使用 CLI 或构建脚本。
- 内联 Rust 使该文件成为源代码：`rustc` 在包含处进行类型检查。请勿从不受信任的位置加载
  此类文件。
- 二进制大小会随生成的代码增长；这就是换取启动速度的代价。绘制路径中不发生任何分配。

## 另见

- [layout-documents.md](layout-documents.md) — 两条路径共享的模型
- [widget-registry-and-creation.md](widget-registry-and-creation.md) — 编译文档中的 `type` 条目
- [diagnostics-and-debug.md](diagnostics-and-debug.md) — `LayoutReport` 与 `LayoutFailure`
- [06-xml-compiled.md](../quickstart/06-xml-compiled.md) — 端到端教程
- [XML layout authoring](../guides/xml-layout-authoring.md) — 文档语法
