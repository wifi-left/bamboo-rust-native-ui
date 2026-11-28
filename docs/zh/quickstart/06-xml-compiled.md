# 06 — 编译期 XML

> 将 `.buixml` 文档编译为 Rust 模块，并用
> `feature = "layout-compiled"` 应用它。不链接任何 XML 解析器。

## 你将构建

与 [05](05-xml-dynamic.md) 相同的窗口和相同的 `layouts/main.buixml`，
但该文档在构建时变为一个 Rust 模块。
应用调用 `ui.apply_compiled`，或生成的 `install` 辅助函数，
而不是在运行时解析文本。你可以在其旁边保持 `layout-xml` 启用；
两种模式共享应用器并产生相同的结果。

## 前置条件

- [05](05-xml-dynamic.md) 中的项目，包括 `layouts/main.buixml`。
- 在 `bamboo-hello/Cargo.toml` 中启用编译期模式：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-compiled"] }
```

`layout-compiled` 添加生成代码所命名的运行时类型
（`layout_xml::compiled::CompiledLayout` 等）。它**不**引入任何 XML
解析器：在此配置下 `quick-xml` 不会被编译或链接。

## 步骤

### 1. 使用 CLI 生成模块

生成器位于框架工作区中。从
`native-ui` 的检出中：

```powershell
cargo run -p bamboo-native-ui-layout-codegen -- `
  layouts/main.buixml -o src/main_layout.rs --module main_layout
```

命令形态为：

```
bamboo-native-ui-layout-codegen <input.buixml> [-o out.rs] [--module name] [--context Type]
```

预期输出：

```
src/main_layout.rs: main.buixml -> mod main_layout
```

如果你的应用是一个独立的仓库，让 Cargo 指向框架
清单，并给出绝对路径或相对应用路径：

```powershell
cargo run --manifest-path ../native-ui/Cargo.toml `
  -p bamboo-native-ui-layout-codegen -- `
  layouts/main.buixml -o src/main_layout.rs --module main_layout
```

输出是普通的 Rust：一个 `pub mod main_layout`，包含
`pub static LAYOUT: CompiledLayout` 和
`pub fn install<M, H>(ui, sink, palette, client)`。将其检入，或从构建脚本生成它（
`bamboo-native-ui-layout-codegen` 库暴露
`generate` 和 `generate_with`）。文档错误会使 CLI 以
状态 1 退出，并打印带有其 `.buixml` 行和列的诊断；不写入任何文件。
可选的 `--context Type` 使生成的 `install` 接受一个
额外的 `&Type` 参数，文档中的内联 Rust 值可以读取它。

### 2. 包含生成的模块并应用它

在 `src/main.rs` 顶部添加 include。该文件已经将其内容
包裹在一个模块中，因此包含它，而不是声明 `mod main_layout;`（一个 `mod`
会嵌套它并产生 `main_layout::main_layout`）：

```rust
include!("main_layout.rs");

use bamboo_native_ui::widgets::{Button, Label};
use bamboo_native_ui::{Host, Size, Ui, WindowHandle};
```

替换第 05 页中的 `apply_layout` 调用：

```rust
impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let palette = *window.palette();
        if let Err(failure) = self
            .ui
            .apply_compiled(&main_layout::LAYOUT, &palette, size, window)
        {
            eprintln!("layout rejected: {failure}");
        }
        window.invalidate_all();
    }
}
```

生成的 `install` 辅助函数做同样的调用；注意其参数顺序是
`(ui, sink, palette, client)`，而 `apply_compiled` 接受
`(layout, palette, client, sink)`：

```rust
main_layout::install(&mut self.ui, window, &palette, size)
```

二者均返回 `Result<LayoutReport, LayoutFailure>`，并遵循动态路径的
规则：在变更前验证所有内容，末尾刷新一次损坏，内容
只在生成变化时写入，用 `ui.clear_layout(window)` 交还
字段。

### 3. 替代方案：`layout_xml!` 过程宏

该宏在编译期展开相同的生成模块，无需单独的
步骤。它位于自己的 crate 中；将其添加到框架旁边：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-compiled"] }
bamboo-native-ui-layout-macros = { path = "../native-ui/crates/layout-macros" }
```

然后，在模块作用域：

```rust
use bamboo_native_ui_layout_macros::layout_xml;

// Paths are relative to CARGO_MANIFEST_DIR; use forward slashes.
layout_xml!("layouts/main.buixml", module = main_layout);
```

这定义了 `main_layout::LAYOUT` 和 `main_layout::install`，与
CLI 输出完全一样。文档错误变成带 `.buixml`
行和列的 `compile_error!`。

**重建注意事项。** 稳定版过程宏无法注册文件依赖，因此
编辑 `layouts/main.buixml` 本身**不会**触发重建。当文档独立变化时，
使用 CLI（或构建脚本），或 touch
包含它的 `.rs` 文件。`cargo clean -p <your-crate>` 是粗暴的兜底方案。

### 4. 选择一种消费路径

| | 运行时（`layout-xml`） | CLI / 构建脚本（`layout-compiled`） | `layout_xml!`（`layout-compiled`） |
|---|---|---|---|
| 文档何时被读取 | 运行时，在安装和重新加载时 | 构建时，显式步骤 | 编译时，宏展开 |
| 二进制中的 XML 解析器 | 是 | 否 | 否 |
| 错误 | 诊断；保留先前的布局 | CLI 以 1 退出并带位置 | 带位置的 `compile_error!` |
| 热重载 | 是 | 否；需重建 | 否；需重建 |
| 需要管理的产物 | `.buixml` 文件 | 检入或生成的 `.rs` 文件 | 无 |
| 最适合 | 开发、主题、调试表单 | 发布构建、大型静态页面 | 小型应用、一步构建 |

页面可以在运行时和编译期模式之间移动，而无需编辑 XML。

## 预期结果

`cargo build` 成功，窗口与第 05 页相同：相同的标签
和按钮位于相同的矩形。差异在运行时不可见 —— 不
读取文件、不链接解析器，无效文档是构建错误
而不是日志行。编辑 `.buixml` 在你重新生成
或重新构建之前都没有效果。

## 故障排查

- **`layout_xml!` 无法读取文件** —— 路径相对于
  `CARGO_MANIFEST_DIR`，而不是源文件，且必须使用正斜杠。
  `layout_xml!("layouts/main.buixml")` 读取
  `<crate>/layouts/main.buixml`。
- **`main_layout::main_layout::LAYOUT` 不存在** —— 生成的文件
  已经包含 `pub mod main_layout`。使用 `include!("main_layout.rs")`，而不是
  `mod main_layout;`。
- **CLI 找不到生成的文件** —— 从
  crate 目录运行命令，或为框架传递 `--manifest-path` 并给出正确的
  相对输入路径。
- **对 `.buixml` 的编辑被忽略** —— 宏的预期行为。用
  CLI 重新生成、添加构建脚本，或 touch 包含它的源文件。
- **`apply_compiled` 缺失** —— `layout-compiled` 特性关闭，或你
  用早于 M7.6 的框架版本生成了该模块。

## 下一步

这就是快速上手的结尾。继续阅读
[控件参考](../widgets/index.md)、
[宿主与窗口 API](../api/host-and-window.md)，或
[XML 编写指南](../guides/xml-layout-authoring.md)。
