# 06 — 编译期 XML

> 将 `.buixml` 文档编译为 Rust 模块，并用
> `feature = "layout-compiled"` 应用它。不链接任何 XML 解析器。

## 你将构建

与 [05](05-xml-dynamic.md) 相同的窗口和相同的 `layouts/main.buixml`，
但该文档在构建时变为一个 Rust 模块。
应用调用 `ui.apply_compiled`，或生成的 `install` 辅助函数，
而不是在运行时解析文本。你可以在其旁边保持 `layout-xml` 启用；
两种模式共享应用器并产生相同的结果。

**本节新术语**

- **代码生成** —— 一个工具读取 `.buixml` 文件并写出普通的
  Rust 源码。然后你像编译任何其他模块一样编译该源码。
- **过程宏** —— 在编译期间运行的 Rust 宏（`layout_xml!` 就是
  一个），它会展开为代码。它是自己运行生成器的另一种方式。
- **构建期与运行时** —— 构建期是 Cargo 编译的时候；运行时是
  窗口打开的时候。动态模式在运行时转换 XML；编译期模式
  在构建时转换它。
- **`layout-compiled`** —— 添加静态布局类型
  （`CompiledLayout`、`Ui::apply_compiled`）的 Cargo 特性。它不添加 XML 解析器。
- **`CompiledLayout`** —— 文档生成的静态描述：
  变体、作为 Rust 函数的矩形、内容值。不解析任何文本。
- **生成的模块** —— 这里的 `main_layout`：一个包含 `LAYOUT` 和
  `install` 的 `pub mod`。
- **`include!`** —— 在编译期把 `.rs` 文件粘贴进当前模块。
  它是文本包含，不同于 `mod`（后者打开一个嵌套模块）。
- **重建** —— 再次运行 `cargo build`/`cargo run`。编译期模式需要
  一次重建（或重新生成再重建），XML 编辑才能生效。

## 前置条件

- [05](05-xml-dynamic.md) 中的项目，包括 `layouts/main.buixml`。
- 在 `bamboo-hello/Cargo.toml` 中启用编译期模式：

```toml
[dependencies]
# 编译期模式：生成的布局，二进制中不含 XML 解析器。
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-compiled"] }
```

这一行属于 `bamboo-hello/bamboo-hello/Cargo.toml`，也就是你在第 05 页
编辑过的同一个 crate 清单。它替换第 05 页的那一行，所以如果你仍然希望在同一个
二进制中保留运行时解析器，请在 `"layout-compiled"` 旁边加上
`"layout-xml"`。保持你在第 02 页使用的 `../native-ui` 深度；
对于那里展示的兄弟文件夹，从 crate 文件夹出发的路径是
`../../native-ui/crates/native-ui`。

`layout-compiled` 添加生成代码所命名的运行时类型
（`layout_xml::compiled::CompiledLayout` 等）。它**不**引入任何 XML
解析器：在此配置下 `quick-xml` 不会被编译或链接。

下面命令中的路径相对你运行它们的文件夹，因此每个
步骤都会先说明该文件夹。

## 步骤

### 1. 使用 CLI 生成模块

生成器位于框架工作区中。从 `native-ui` 检出的根目录 —— 也就是
包含框架工作区 `Cargo.toml` 的文件夹 —— 运行下一条命令，
因为 `-p` 会从该工作区中选一个包。
两个文件路径都从你运行的文件夹解析。

```powershell
# 在 native-ui 工作区根目录运行；两个路径都相对该目录。
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

输入路径必须相对你运行的文件夹存在。如果不存在，
CLI 会打印 `cannot read layouts/main.buixml: ...` 并以状态 1 退出，不写入
任何内容。框架检出中不包含本教程的
`layouts/main.buixml`，所以这种形式适用于文档已被放入
该工作区（或你传入其实际路径）的情况。本教程把
文档放在应用中，因此你通常使用的是下一种形式。在
应用 crate 文件夹 `bamboo-hello/bamboo-hello/`（包含
`layouts/` 和 `src/` 的文件夹）中运行它，并让 Cargo
指向框架清单：

```powershell
# 在应用 crate 文件夹运行；把 ../native-ui 调整为你框架的位置。
cargo run --manifest-path ../native-ui/Cargo.toml `
  -p bamboo-native-ui-layout-codegen -- `
  layouts/main.buixml -o src/main_layout.rs --module main_layout
```

`--manifest-path` 也从你当前的文件夹解析。使用第 02 页的兄弟
布局（`F:\codes\bamboo-hello` 与 `F:\codes\native-ui` 相邻）时，
从 crate 文件夹出发的路径是 `../../native-ui/Cargo.toml`。让它指向
框架的工作区 `Cargo.toml`，而不是 `crates/native-ui` 库
清单。

输出是普通的 Rust：一个 `pub mod main_layout`，包含
`pub static LAYOUT: CompiledLayout` 和
`pub fn install<M, H>(ui, sink, palette, client)`。将其检入，或从构建脚本生成它（
`bamboo-native-ui-layout-codegen` 库暴露
`generate` 和 `generate_with`）。文档错误会使 CLI 以
状态 1 退出，并打印带 `.buixml` 行号和列号的诊断；不写入任何文件。
可选的 `--context Type` 使生成的 `install` 接受一个
额外的 `&Type` 参数，文档中的内联 Rust 值可以读取它。

**刚才发生了什么。** CLI 只解析并校验了一次文档，然后在
`main.rs` 旁边写出了 `src/main_layout.rs`。应用运行时不再解析任何内容：
矩形已经是 Rust 值。如果命令打印了一行带
`.buixml` 位置的输出并以状态 1 退出，说明没有写入文件，之前
生成的文件（如果有）保持不变。

### 2. 包含生成的模块并应用它

在 `src/main.rs` 顶部添加 include。该文件已经把内容
包进一个模块，因此包含它，而不是声明 `mod main_layout;`（一个 `mod`
会嵌套它并产生 `main_layout::main_layout`）：

```rust
// 文本包含进本模块：文件本身已经声明了 `mod main_layout`。
include!("main_layout.rs");

use bamboo_native_ui::widgets::{Button, Label};
use bamboo_native_ui::{Host, Size, Ui, WindowHandle};
```

替换第 05 页中的 `apply_layout` 调用：

```rust
impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let palette = *window.palette();
        // LAYOUT 是静态数据；这个调用不解析任何文本。
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
// 同样的应用，参数顺序不同：(ui, sink, palette, client)。
main_layout::install(&mut self.ui, window, &palette, size)
```

二者均返回 `Result<LayoutReport, LayoutFailure>`，并遵循动态路径的
规则：在变更前验证所有内容，末尾刷新一次损坏，内容
只在生成变化时写入，用 `ui.clear_layout(window)` 交还
字段。

`include!` 直接把生成的 `pub mod main_layout { ... }` 粘贴进
`main.rs`，因此类型是 `main_layout::LAYOUT`。而 `mod main_layout;`
会去找 `main_layout.rs` 并把其内容包进一个外层模块，
产生 `main_layout::main_layout::LAYOUT` —— 一个在成功生成之后
令人困惑的错误。include 路径相对 `main.rs` 解析，这就是
CLI 写入 `src/` 的原因。

**刚才发生了什么。** 二进制现在把布局作为生成的 Rust
数据携带。没有文件 IO，也没有解析器：`apply_compiled` 遍历
动态路径从 XML 文本构建的同一批内存描述符，并产生
相同的矩形和相同的错误码。

### 3. 替代方案：`layout_xml!` 过程宏

该宏在编译期展开相同的生成模块，无需单独的
步骤。它位于自己的 crate 中；将其添加到框架旁边：

```toml
[dependencies]
# 编译期模式，同上。
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-compiled"] }
# 过程宏 crate，在编译期把 layout_xml! 变成生成的 Rust。
bamboo-native-ui-layout-macros = { path = "../native-ui/crates/layout-macros" }
```

然后，在模块作用域：

```rust
use bamboo_native_ui_layout_macros::layout_xml;

// 路径相对 CARGO_MANIFEST_DIR；使用正斜杠。
layout_xml!("layouts/main.buixml", module = main_layout);
```

这定义了 `main_layout::LAYOUT` 和 `main_layout::install`，与
CLI 输出完全一样。文档错误变成带 `.buixml`
行号和列号的 `compile_error!`。

这条路没有命令要运行：下一次 `cargo build`（或
`cargo run`）会展开该宏。宏读取的 `layouts/main.buixml` 相对
crate 文件夹 —— 即含 `Cargo.toml` 的文件夹 —— 因此第 05 页的同一个文件
无需改动即可使用。生成的模块只存在于编译后的
二进制内部；磁盘上不会出现任何 `.rs` 文件。

**刚才发生了什么。** 编译期间，宏读取了文档、
校验了它，并把生成的模块粘贴到调用所在的位置。源码中唯一的线索是
`use` 语句；结果与 CLI 的输出完全相同。

**重建注意事项。** 稳定版过程宏无法注册文件依赖，因此
编辑 `layouts/main.buixml` 本身**不会**触发重建。当文档独立变化时，
使用 CLI（或构建脚本），或 touch
包含它的 `.rs` 文件。`cargo clean -p <your-crate>` 是粗暴的兜底方案。

实际中：保存 XML，运行 `cargo run`，窗口仍然显示旧
布局 —— 没有错误，没有警告。这是本注意事项造成的，而不是宏坏了。touch
`src/main.rs`（加一行空行再删掉），或用 CLI 重新生成，然后
再次构建。

### 4. 选择一种消费路径

下面三个模式列回答同一个问题 —— *XML 什么
时候被变成矩形？* —— 表中其余条目由此答案推导出来。

| | 运行时（`layout-xml`） | CLI / 构建脚本（`layout-compiled`） | `layout_xml!`（`layout-compiled`） |
|---|---|---|---|
| 文档何时被读取 | 运行时，在安装和重新加载时 | 构建时，显式步骤 | 编译时，宏展开 |
| 二进制中的 XML 解析器 | 是 | 否 | 否 |
| 错误 | 诊断；保留先前的布局 | CLI 以 1 退出并带位置 | 带位置的 `compile_error!` |
| 热重载 | 是 | 否；需重建 | 否；需重建 |
| 需要管理的产物 | `.buixml` 文件 | 检入或生成的 `.rs` 文件 | 无 |
| 最适合 | 开发、主题、调试表单 | 发布构建、大型静态页面 | 小型应用、一步构建 |

把最后一行读作默认建议：布局仍在变化时用运行时
模式，文档稳定并且你想把解析器移出二进制之后用
编译期模式。CLI 形式更容易检查、也更容易
在 CI 中重新生成；宏形式对小应用来说活动部件更少。

页面可以在运行时和编译期模式之间移动，而无需编辑 XML。

## 预期结果

`cargo build` 成功，窗口与第 05 页相同：标签
和按钮位于相同的矩形。差异在运行时不可见 —— 不
读取文件、不链接解析器，无效文档是构建错误
而不是日志行。编辑 `.buixml` 在你重新生成
或重新构建之前都没有效果。

**运行它。** 使用工作区文件夹 `bamboo-hello/` 或 crate 文件夹
`bamboo-hello/bamboo-hello/`；Cargo 从任一位置解析同一个
项目：

```powershell
cargo run
```

第 05 页的 520×320 窗口出现，标签和按钮在相同的
位置。窗口是生成矩形等价的证明；
构建日志是无需解析器的证明。

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
- **`error: package ID specification 'bamboo-native-ui-layout-codegen' did not match any packages`** —— 你不在框架工作区内。在 `native-ui` 根目录运行
  `cargo run -p ...`，或从应用文件夹使用
  `--manifest-path` 形式。
- **`include!("main_layout.rs")` 失败并报 "file not found"** —— 路径
  相对 `main.rs`，因此文件必须在 `src/` 中。如果 CLI 把它写到了
  别处，说明 CLI 的工作目录与你的预期不同；
  从 crate 文件夹用 `-o src/main_layout.rs` 重新生成。
- **生成的模块缺少你期望的函数** —— `--module` 设置
  模块名；`layout_xml!(..., module = ...)` 必须使用你在 Rust 中
  调用的同一个名字。
- **对 `.buixml` 的编辑被忽略** —— 宏的预期行为。用
  CLI 重新生成、添加构建脚本，或 touch 包含它的源文件。
  使用 CLI 形式时，记住只重新生成而不重建仍然运行旧的
  编译副本。
- **XML 中的拼写错误在运行时没有消息** —— 编译期模式没有
  运行时解析。错误在你运行生成器或展开宏时出现，
  作为带位置的诊断或 `compile_error!`，而不是在窗口中。
- **`apply_compiled` 缺失** —— `layout-compiled` 特性关闭，或你
  用早于 M7.6 的框架版本生成了该模块。
- **`no matching package named bamboo-native-ui-layout-macros`** —— 宏
  crate 路径错误。它在框架检出中是 `crates/layout-macros`，
  是 `crates/native-ui` 的兄弟。

## 下一步

这就是快速上手的结尾。继续阅读
[控件参考](../widgets/index.md)、
[宿主与窗口 API](../api/host-and-window.md)，或
[XML 编写指南](../guides/xml-layout-authoring.md)。
