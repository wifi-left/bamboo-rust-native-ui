# 01 — 创建项目

> 设置一个空的 Cargo 工作区，用来承载 Bamboo Native UI 示例。

## 你将构建

一个名为 `bamboo-hello` 的二进制 crate，位于一个小型 Cargo 工作区内。到本页结束时，`cargo run` 会打印 `Hello, world!`；窗口将在 [03 — 第一个窗口](03-first-window.md) 中到来。

## 本节新术语

- **Rust** —— 编程语言。它的编译器把整个程序变成一个原生 `.exe`，并拒绝它无法检查的代码。
- **Cargo** —— 随 Rust 一起提供的构建工具。它创建项目、读取 `Cargo.toml`、下载依赖、编译并启动程序。
- **crate** —— 一个作为整体编译的 Rust 包。*库 crate* 供其他代码使用；*二进制 crate* 变成可执行文件。
- **二进制 crate（binary crate）** —— 生成你的 `.exe` 的 crate；它有一个 `main` 函数，`cargo run` 启动的就是它。
- **工作区（workspace）** —— 把一到多个 crate 组织在一起的 Cargo 项目，共用一个 `Cargo.lock` 和 `target/` 目录。
- **清单（manifest）** —— 描述一个 crate 或一个工作区的 `Cargo.toml` 文件。
- **edition** —— crate 所依据的 Rust 语言版本；`2024` 是最新的，需要 Rust 1.85 或更新。
- **resolver** —— Cargo 选择依赖特性的算法；`"3"` 是与 edition 2024 配套的解析器。
- **构建（build）** —— 把代码编译成可执行文件，但不启动它。
- **运行（run）** —— 必要时先构建，然后启动可执行文件。
- **链接器（linker，`link.exe`）** —— 把编译后的 Rust 代码与 Windows 库连接起来、生成 `.exe` 的工具；MSVC 构建工具提供它。
- **`target/`** —— Cargo 写入构建产物的目录。它可以删除，Cargo 会重新生成。

## 前置条件

### 操作系统

Windows 10 或 Windows 11。框架的窗口宿主仅限 Windows（`#[cfg(windows)]`）；框架本身可在其他平台编译以便测试，但本教程运行真实窗口。

### Rust 工具链

从 <https://rustup.rs> 安装 Rust。框架使用 edition 2024 和 `resolver = "3"` 工作区，因此你需要 Rust 1.85 或更新；任何当前稳定版均可。

在任意 PowerShell 窗口中运行下面三条命令。它们修改的是用户级的工具链，而不是某个项目文件夹。

```powershell
# 下载稳定工具链的 Windows MSVC 版本。
rustup toolchain install stable-x86_64-pc-windows-msvc
# 把它设为新终端的默认工具链。
rustup default stable-x86_64-pc-windows-msvc
# 打印当前活动的宿主机和工具链。
rustup show
```

预期输出（版本号取决于你的工具链）：

```
Default host: x86_64-pc-windows-msvc
active toolchain
----------------
stable-x86_64-pc-windows-msvc (default)
rustc 1.91.0
```

`rustup show` 必须报告 **msvc** 宿主机和工具链；这是框架测试所用的配置。`windows-gnu` 工具链不在覆盖范围内。

### C++ 构建工具与 Windows SDK

安装 **Visual Studio 2022 Build Tools** 并选择
**使用 C++ 的桌面开发**工作负载。它提供 `link.exe`、C/C++
运行时以及 Windows 10/11 SDK。`windows` crate 在链接时需要它们；
无需单独下载 SDK。

要验证配置，请在普通 PowerShell 窗口中运行下面的命令；在哪个项目文件夹里运行无关紧要：

```powershell
# 如果 C++ 链接器在 PATH 上，打印它的完整路径。
where.exe link.exe
```

如果没有任何输出，请打开 **Developer PowerShell for VS 2022**（构建工具快捷方式）并在那里继续，或者添加该工作负载。已经打开的终端会保留旧的 `PATH`；安装完成后请打开一个新终端。

## 步骤

### 1. 创建工作区文件夹和 crate

在应当包含项目的文件夹中运行这些命令 —— 示例中是 `F:\codes`。第一条命令创建 `F:\codes\bamboo-hello`；后两条在它里面执行。

```powershell
# 创建外层项目文件夹。
mkdir bamboo-hello
# 进入该文件夹；本页后续所有命令都在这里运行。
cd bamboo-hello
# 在子文件夹中创建 crate，不创建嵌套的 .git 仓库。
cargo new bamboo-hello --vcs none
```

预期输出：

```
    Creating binary (application) `bamboo-hello` package
```

`--vcs none` 避免创建嵌套的 git 仓库。如果你想要一个，去掉该标志。

`cargo new` 现在创建了两层结构：

```
bamboo-hello/               outer folder; you are here
└── bamboo-hello/           the crate
    ├── Cargo.toml          package manifest
    └── src/
        └── main.rs         prints "Hello, world!"
```

**刚才发生了什么。** `cargo new` 写出了一个完整的最小程序：一个写明包名的清单，以及带 `main` 函数、会打印 `Hello, world!` 的 `src/main.rs`。外层文件夹此时还不是 crate；第 2 步才会把它变成工作区。到目前为止还没有编译任何东西。

### 2. 添加工作区清单

一个项目有两个 `Cargo.toml` 文件是正常的。**外层**清单描述工作区：哪些 crate 属于它，以及它们的依赖如何解析。**内层**清单描述你的 crate：名称、版本、edition 和依赖。Cargo 从当前文件夹向上查找最近的清单，因此在外层文件夹中运行的命令会影响它的成员。

在外层文件夹中创建 `bamboo-hello/Cargo.toml` —— 按示例位置就是 `F:\codes\bamboo-hello\Cargo.toml`，与 crate 文件夹同级：

```toml
[workspace]
# edition 2024 的特性解析器，与框架使用的一致。
resolver = "3"
# 由 cargo new 创建的 crate 文件夹。
members = ["bamboo-hello"]
```

`resolver = "3"` 与框架自身的工作区及 edition-2024 特性解析器一致。将 crate 列为成员意味着外层文件夹中的每条 `cargo` 命令都会构建它。这个清单没有 `[package]` 段，所以它本身不构建任何东西。

**刚才发生了什么。** 外层文件夹变成了工作区根目录。它仍然没有 `src/`，也不产生可执行文件；它只协调 `members` 中列出的 crate。

### 3. 确认 crate 清单

`bamboo-hello/bamboo-hello/Cargo.toml` 已为你生成，应如下所示。这就是用 `[package]` 命名你程序的清单；后续页面把依赖加在这里，而不是工作区清单。

```toml
[package]
# crate 名称；Cargo 也把它用作可执行文件名。
name = "bamboo-hello"
version = "0.1.0"
# 仅对本 crate 生效的语言 edition；需要 Rust 1.85 或更新。
edition = "2024"

[dependencies]
# 暂时为空；第 02 页会在这里加入 bamboo-native-ui。
```

保留 `edition = "2024"` 不变。edition 并不是另一个编译器：它启用更新的语法和规则，而旧的 crate 仍能继续编译。框架本身也使用 edition 2024，但依赖的 edition 是独立的，因此这个设置只针对你的代码。工作区里的 `resolver = "3"` 与它配套。

### 4. 构建并运行

在外层 `bamboo-hello` 文件夹（含工作区 `Cargo.toml` 和 `bamboo-hello` crate 文件夹的那个）中运行这些命令。

```powershell
# 以 debug 配置编译；生成 target\debug\bamboo-hello.exe。
cargo build
# 如有改动就重新编译，然后启动可执行文件。
cargo run
```

`build` 在生成可执行文件后停止。`run` 执行同样的编译步骤，然后启动程序，因此下面的输出会同时显示两者。

预期输出：

```
   Compiling bamboo-hello v0.1.0 (C:\path\to\bamboo-hello\bamboo-hello)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.6s
     Running `target\debug\bamboo-hello.exe`
Hello, world!
```

耗时和 `Compiling` 行中的路径取决于你的机器。关键部分是 `Finished`（编译成功）和 `Hello, world!`（程序已启动）。

**刚才发生了什么。** Cargo 编译了你的 crate，把它链接成
`target\debug\bamboo-hello.exe`，随后 `cargo run` 启动了这个文件。文本出现在运行命令的终端里；本页还没有创建窗口。
`Cargo.lock` 和 `target/` 文件夹在工作区清单旁生成。

## 项目结构

```
bamboo-hello/
├── Cargo.toml              workspace manifest (resolver 3, members)
├── Cargo.lock              created by the first build
└── bamboo-hello/
    ├── Cargo.toml          the binary crate
    └── src/
        └── main.rs         "Hello, world!" for now
```

你在后续页面中添加的所有内容都放在 `bamboo-hello/bamboo-hello/` 下。Cargo 把构建产物写在外层文件夹的 `target/` 中；该目录是生成物，可以安全删除。native-ui 仓库是一个同级文件夹；第 02 页通过路径链接它。

## 预期结果

`cargo run` 从生成的 `main.rs` 中打印 `Hello, world!`。程序会自行退出，因此提示符立刻返回。尚未创建窗口；如果你期待看到一个窗口，那是第 03 页的内容。

## 故障排查

- **链接步骤失败并提示 "link.exe not found"** —— 缺少 MSVC 构建工具，或它不在 `PATH` 上。安装 **使用 C++ 的桌面开发**
  工作负载，或使用 Developer PowerShell。安装工作负载不会更新已经打开的终端；请打开一个新终端，再次检查 `where.exe link.exe`。
- **Cargo 因目标已存在而拒绝** —— 你已经在名为 `bamboo-hello` 的文件夹内。在父文件夹里运行它，或换一个名字。
- **`feature \`edition2024\` is required`** —— 工具链早于 Rust
  1.85。运行 `rustup update stable`。
- **`rustup show` 报告 `windows-gnu`** —— 安装 MSVC 工具链
  （`rustup toolchain install stable-x86_64-pc-windows-msvc`）并将其设为默认。
- **`cargo` 不被识别为命令** —— 没有安装 Rust，或这个终端在安装完成之前就已打开。从
  <https://rustup.rs> 安装，打开一个新的 PowerShell 窗口，然后检查
  `cargo --version`。如果 `rustup --version` 可用而 `cargo` 不行，说明上面安装工具链的步骤没有完成。
- **`error: could not find \`Cargo.toml\``** —— 你在错误的文件夹中。请在外层 `bamboo-hello` 文件夹中运行
  `cargo build` 和 `cargo run`。在该目录下列出文件，应能看到 `Cargo.toml` 和内层的 `bamboo-hello` 文件夹；`src/` 在内层文件夹里，不在这里。
- **修改后运行的仍是旧程序** —— 编辑器没有保存文件，或你修改的是别处的副本。检查编辑器标签页上是否有未保存标记，按 Ctrl+S，并确认路径是
  `...\bamboo-hello\bamboo-hello\src\main.rs`。
- **`cargo run` 没有任何输出就返回** —— `main.rs` 里可能已不再包含生成的 `println!("Hello, world!");`。空的 `main` 能编译，但会直接退出，不打印任何内容。
- **控制台一闪而过** —— 你从资源管理器中启动了 `.exe`。请在终端里运行 `cargo run`，这样输出才保持可见。
- **构建成功但没有出现窗口** —— 本页就是这个预期。生成的程序只打印文本；第一个窗口在第 03 页。

## 下一步

[02 — 添加 crate](02-add-the-crate.md)：通过路径链接 `bamboo-native-ui` 并
选择你需要的特性。
