# 01 — 创建项目

> 设置一个空的 Cargo 工作区，用来承载 Bamboo Native UI 示例。

## 你将构建

一个名为 `bamboo-hello` 的二进制 crate，位于一个小型 Cargo 工作区内。到本页结束时，`cargo run` 会打印 `Hello, world!`；窗口将在 [03 — 第一个窗口](03-first-window.md) 中到来。

## 前置条件

### 操作系统

Windows 10 或 Windows 11。框架的窗口宿主仅限 Windows（`#[cfg(windows)]`）；框架本身可在其他平台编译以便测试，但本教程运行真实窗口。

### Rust 工具链

从 <https://rustup.rs> 安装 Rust。框架使用 edition 2024 和 `resolver = "3"` 工作区，因此你需要 Rust 1.85 或更新；任何当前稳定版均可。

```powershell
rustup toolchain install stable-x86_64-pc-windows-msvc
rustup default stable-x86_64-pc-windows-msvc
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

要在普通终端中验证：

```powershell
where.exe link.exe
```

如果没有任何输出，请打开 **Developer PowerShell for VS 2022**（构建工具快捷方式）并在那里继续，或者添加该工作负载。

## 步骤

### 1. 创建工作区文件夹和 crate

```powershell
mkdir bamboo-hello
cd bamboo-hello
cargo new bamboo-hello --vcs none
```

预期输出：

```
    Creating binary (application) `bamboo-hello` package
```

`--vcs none` 避免创建嵌套的 git 仓库。如果你想要一个，去掉该标志。

### 2. 添加工作区清单

创建 `bamboo-hello/Cargo.toml`（外层文件夹，与 crate 文件夹同级）：

```toml
[workspace]
resolver = "3"
members = ["bamboo-hello"]
```

`resolver = "3"` 与框架自身的工作区及 edition-2024 特性解析器一致。将 crate 列为成员意味着外层文件夹中的每条 `cargo` 命令都会构建它。

### 3. 确认 crate 清单

`bamboo-hello/bamboo-hello/Cargo.toml` 已为你生成，应如下所示：

```toml
[package]
name = "bamboo-hello"
version = "0.1.0"
edition = "2024"

[dependencies]
```

保留 `edition = "2024"` 不变。框架本身也使用 edition 2024，但依赖的 edition 是独立的，所以这只是针对你的代码。

### 4. 构建并运行

```powershell
cargo build
cargo run
```

预期输出：

```
   Compiling bamboo-hello v0.1.0 (C:\path\to\bamboo-hello\bamboo-hello)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.6s
     Running `target\debug\bamboo-hello.exe`
Hello, world!
```

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

你在后续页面中添加的所有内容都放在 `bamboo-hello/bamboo-hello/` 下。native-ui 仓库是一个同级文件夹；第 02 页通过路径链接它。

## 预期结果

`cargo run` 从生成的 `main.rs` 中打印 `Hello, world!`。尚未创建窗口。

## 故障排查

- **链接步骤失败并提示 "link.exe not found"** —— 缺少 MSVC 构建工具，或它不在 `PATH` 上。安装 **使用 C++ 的桌面开发**
  工作负载，或使用 Developer PowerShell。
- **Cargo 因目标已存在而拒绝** —— 你已经在名为 `bamboo-hello` 的文件夹内。从父文件夹运行它，或换一个名字。
- **`feature \`edition2024\` is required`** —— 工具链早于 Rust
  1.85。运行 `rustup update stable`。
- **`rustup show` 报告 `windows-gnu`** —— 安装 MSVC 工具链
  （`rustup toolchain install stable-x86_64-pc-windows-msvc`）并将其设为默认。

## 下一步

[02 — 添加 crate](02-add-the-crate.md)：通过路径链接 `bamboo-native-ui` 并
选择你需要的特性。
