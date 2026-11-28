# 02 — 添加 crate

> 通过路径链接 `bamboo-native-ui`，选择你需要的特性，并构建一次。

## 你将构建

第 01 页中的 `bamboo-hello` crate，将框架作为依赖。构建会编译框架及其 Windows 依赖；在第 03 页之前，`main.rs` 仍打印 `Hello, world!`。

## 本节新术语

- **依赖（dependency）** —— 你的 crate 所调用的另一个 crate；它列在你的包清单的
  `[dependencies]` 下。
- **特性开关（feature flag）** —— crate 内部一个有名字的可选开关。只有列进
  `features = [...]` 时，开关后面的代码才会被编译。
- **默认特性（default features）** —— 除非你主动关闭，否则 crate 会打开的开关；
  `gdi` 是框架唯一的默认特性。
- **路径依赖（path dependency）** —— 由文件夹路径定位的依赖；Cargo 直接使用那
  份检出，因此本地的修改会立即生效。
- **git 依赖（git dependency）** —— Cargo 从 Git URL 克隆并缓存的依赖；不需要
  本地检出。
- **子模块（submodule）** —— 以固定提交嵌套在另一个仓库中的 Git 仓库；
  与路径依赖配合，可锁定精确的框架修订版本。
- **虚拟工作区（virtual workspace）** —— 只有 `[workspace]`、没有 `[package]` 的
  工作区清单；它协调各 crate，但本身不是库。
- **crates.io** —— Rust 的公开包注册表。本框架没有发布到那里，因此依赖要按路径
  或 Git 引入。
- **`Cargo.lock`** —— 记录 Cargo 实际解析出的精确版本的文件；应用应当提交它。
- **修订版本（`rev`）** —— 一个固定的 Git 提交哈希。
- **代码生成（code generation）** —— 预先把 `.buixml` 文件转成 Rust 源码；
  代码生成 CLI 做的就是这件事。
- **过程宏（proc macro）** —— 在编译期展开为 Rust 代码的宏；
  `layout_xml!` 就是其中之一。

## 前置条件

完成 [01 — 创建项目](01-create-project.md)。你需要 `bamboo-hello` 工作区、可用的 MSVC 工具链和 Windows SDK，以及本地检出
<https://github.com/wifi-left/bamboo-rust-native-ui>，或克隆它的网络访问。配置好 Git 代理即可；仓库是公开的。

## 步骤

### 1. 从 GitHub 获取框架

仓库公开在
<https://github.com/wifi-left/bamboo-rust-native-ui>。把它克隆到工作区旁边。请在包含 `bamboo-hello` 工作区的文件夹中运行下面的命令（示例中是 `F:\codes`），这样 `native-ui/` 就成为同级目录：

```powershell
# 公开仓库；克隆到工作区旁边的 native-ui/。
git clone https://github.com/wifi-left/bamboo-rust-native-ui.git native-ui
```

结果是：

```
F:\codes\
├── bamboo-hello\          your workspace from page 01
└── native-ui\             the framework repository
    └── crates\native-ui\  the library crate
```

如果框架属于你项目的版本历史，把它添加为子模块，这样提交会固定精确的版本。请在你自己的 Git 仓库根目录（包含其 `.git` 的文件夹）运行这两条命令：

```powershell
# 在添加时的那个修订版本上把框架记录为子模块。
git submodule add https://github.com/wifi-left/bamboo-rust-native-ui.git native-ui
# 在这个克隆中检出子模块，包括嵌套的子模块。
git submodule update --init --recursive
```

两种方式都会得到下面要用的 `native-ui/` 文件夹。crate 本身位于
`native-ui/crates/native-ui`；仓库根目录是虚拟工作区，不是库。

**刚才发生了什么。** Git 下载了仓库。`crates/native-ui` 是你依赖的 crate；`crates/` 下的其他文件夹是同一个工作区的组成部分（布局模型、代码生成、宏和 showcase）。此时还没有编译任何东西。

### 2. 指向框架

框架不在 crates.io 上，因此依赖需要指明来源：指向本地检出的**路径依赖**，或由 Cargo 拉取的 **git 依赖**。子模块就是一种路径依赖，只是检出由你自己的仓库固定。

一条依赖声明由三部分组成：你的代码导入的名称
（`bamboo-native-ui`）、来源（`path` 或 `git`），以及可选的 `features`。把它加到**crate 清单**中 —— 带 `[package]` 段的那个 `Cargo.toml`，第 01 页把它创建在 `bamboo-hello/bamboo-hello/Cargo.toml`。工作区清单（`bamboo-hello/Cargo.toml`）只有 `[workspace]`；在那里写 `[dependencies]` 会被拒绝，因为虚拟清单没有可应用的包。

如果你的项目与仓库同级：

```
F:\codes\
├── bamboo-hello\          your workspace from page 01
└── native-ui\             the framework repository
    └── crates\native-ui\
```

那么编辑 crate 清单：

```toml
[dependencies]
# 指向 crate 文件夹，而不是仓库根目录。
bamboo-native-ui = { path = "../../native-ui/crates/native-ui" }
```

相对路径从清单文件算起，而不是从运行 `git clone` 的文件夹算起。在第 01 页的布局中，crate 清单是
`bamboo-hello/bamboo-hello/Cargo.toml`，位于 `F:\codes` 下两层，因此
`../../native-ui/crates/native-ui` 到达同级仓库。每多一层文件夹就多加一个 `..`；清单缺失时，Cargo 会打印它尝试过的路径。

当框架作为 git 子模块检入你的仓库时，路径取决于子模块的位置。如果它在工作区外层、与 crate 文件夹同级：

```toml
# 子模块位于 bamboo-hello/native-ui，从 crate 清单看过去。
bamboo-native-ui = { path = "../native-ui/crates/native-ui" }
```

如果 crate 清单位于仓库根目录，而子模块是它旁边的 `native-ui/`（播放器的布局），则改用
`native-ui/crates/native-ui`。

使用指向 **crate**（`crates/native-ui`）的路径，而不是工作区根目录。指向根目录的路径会解析到虚拟工作区清单，它没有
`[package]`，也没有库目标，因此 Cargo 无法链接它。

#### 或者直接依赖 GitHub 仓库

如果你不想本地检出，Cargo 也可以拉取整个工作区：

```toml
[dependencies]
# package = "..." 选择 crate；URL 指向工作区根目录。
bamboo-native-ui = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui" }
```

必须写 `package`：URL 指向工作区根目录，Cargo 按名称选择 `bamboo-native-ui`
成员。用 `rev = "<commit>"` 固定版本以保证可复现，或用 `branch = "main"`
跟随默认分支。特性的写法相同：

```toml
# 同一个 Git 源，并编译进可选的 .buixml 解析。
bamboo-native-ui = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui", features = ["layout-xml"] }
```

git 形式会在首次构建时拉取整个工作区，并缓存到 `~/.cargo/git`；本地迭代时
path 形式更快。代码生成 CLI 是一个二进制，因此要运行它请克隆仓库。
`layout_xml!` 过程宏也可以作为 git 依赖：

```toml
# 提供 layout_xml! 的过程宏 crate；同一个 Git 仓库。
bamboo-native-ui-layout-macros = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui-layout-macros" }
```

### 3. 选择特性

特性开关是编译期的选择，不是运行时设置：它决定二进制里存在哪些代码。`bamboo-native-ui` 有五个特性。默认只开启 `gdi`，这对第 03 页足够。

| 特性 | 默认 | 启用 |
|---|---|---|
| `gdi` | 是 | GDI 后端（以及 Direct2D 渲染器选项）。保持开启。 |
| `layout-xml` | 否 | `.buixml` 解析（`LayoutDocument::parse`）和用于热重载的 `Ui::apply_layout` |
| `layout-compiled` | 否 | 生成的布局（`CompiledLayout`、`Ui::apply_compiled`）；无解析器 |
| `theme-xml` | 否 | 映射到 `Palette` 的 `.buitheme` 主题文档 |
| `debug-form` | 否 | 只读调试快照（`Ui::debug_snapshot`）和绘制统计；蕴含 `layout-xml` |

如果你确定需要它们，现在就启用，或者在用到它们的页面上再添加：

```toml
[dependencies]
# 加入可选的 .buixml 解析；默认的 gdi 仍然开启。
bamboo-native-ui = { path = "../../native-ui/crates/native-ui", features = ["layout-xml"] }
```

`gdi` 是累加的，因此 `features = ["layout-xml"]` 会保持它启用。要构建第 05 和 06 页中的示例，你分别需要 `layout-xml` 和 `layout-compiled`。

### 4. 构建工作区

在外层 `bamboo-hello` 文件夹（工作区根目录，即含工作区 `Cargo.toml` 的那个）中运行：

```powershell
# 解析、下载并编译 bamboo-hello 的每个依赖。
cargo build
```

首次构建会下载 `windows` crate 的源码，把它和生成的绑定一起编译，然后编译框架各 crate，最后编译你的 crate，因此需要一分钟或更久。后续构建会复用 `target/` 中的结果。预期输出（路径和耗时会有变化）：

```
   Compiling windows v0.62.2
   Compiling bamboo-native-ui v0.1.0 (F:\codes\native-ui\crates\native-ui)
   Compiling bamboo-hello v0.1.0 (F:\codes\bamboo-hello\bamboo-hello)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 12s
```

**如何判断框架已经链接。** `Compiling bamboo-native-ui v0.1.0 (...)`
这一行直接证明 Cargo 到达了该依赖。后续构建中它可能不再出现，因为结果已被缓存，Cargo 只打印 `Finished`。要查看依赖图，在同一个外层文件夹中运行：

```powershell
# 列出 bamboo-hello 依赖的 crate。
cargo tree
```

`bamboo-native-ui` 会出现在 `bamboo-hello` 之下，`windows` 系列 crate 在它下面。如果没有，说明 `[dependencies]` 行写在了错误的清单里，或文件没有保存。

**刚才发生了什么。** Cargo 解析了新的依赖，下载并编译了它及其 Windows 支持 crate，然后编译了你未改动的
`main.rs`。程序仍打印 `Hello, world!`；第 03 页会把它换成窗口。

无警告的成功构建是框架自身 `docs/HANDOFF.md` 在提交前要求的基线；如果你看到来自*你的* crate 的警告，请在继续之前修复它们。

### 5. 关于子模块

如果框架作为 git 子模块存在于你的仓库中，全新的克隆必须初始化它。第一条命令在你存放仓库的位置运行；第二条在已有的克隆中运行：

```powershell
# 一步克隆你的仓库及其子模块。
git clone --recursive <your-repo>
# 或者，在已有的克隆中，获取固定的子模块修订版本：
git submodule update --init --recursive
```

不要将子模块文件夹添加到 `.gitignore`；父仓库必须跟踪 gitlink，否则全新克隆无法解析路径依赖。

**刚才发生了什么。** 在子模块初始化之前，全新克隆只会得到一个空的 `native-ui/` 文件夹；只有初始化之后，路径依赖才指向一份真实的检出。这也解释了为什么构建在你的机器上能成功，却可能在同事的全新克隆上失败。

## 预期结果

`cargo build` 以 `Finished` 结束且无错误。首次构建会打印
`Compiling bamboo-native-ui ...` 行，`cargo tree` 会在 `bamboo-hello`
下列出框架。`cargo run` 仍打印 `Hello, world!` 并退出；框架已链接并可使用，但还不会绘制任何东西。

## 故障排查

- **Cargo 报告 "failed to load manifest for dependency bamboo-native-ui"** ——
  路径错误。它必须以 `crates/native-ui` 结尾。检查相对深度：
  从 `bamboo-hello/bamboo-hello/Cargo.toml` 出发，与工作区同级的仓库是 `../../native-ui/crates/native-ui`。
- **Cargo 报告 "no matching package named bamboo-native-ui-layout-core
  found"** —— 检出不完整，或你指向了一个被复制的、没有 `crates/` 兄弟目录的 `native-ui`
  文件夹。使用完整检出（或
  `git submodule update --init`）。
- **未知特性警告** —— 特性名是精确的：
  `layout-xml`、`layout-compiled`、`theme-xml`、`debug-form`、`gdi`。拼写错误会
  由 Cargo 报告，而不是被静默忽略。
- **Cargo 无法拉取 git 依赖** —— 仓库是公开的，因此这几乎总是网络或代理问题。
  `git ls-remote https://github.com/wifi-left/bamboo-rust-native-ui` 可验证访问；
  没有网络时，使用本地克隆和 path 形式。
- **编译出两份框架副本** —— 即使版本相同，path 依赖和 git 依赖也是不同的
  Cargo 源。每个工作区只选一种形式。
- **首次构建非常慢** —— 属预期；`windows` 绑定很大。
  后续构建会复用它。
- **`cargo` 不被识别为命令** —— 没有安装 Rust，或这个终端在安装之前就已打开。从
  <https://rustup.rs> 安装并打开一个新的 PowerShell 窗口。
- **`error: could not find \`Cargo.toml\``** —— `cargo build` 在错误的文件夹中运行。
  请使用包含工作区清单的外层 `bamboo-hello` 文件夹。
- **"this virtual manifest specifies a `dependencies` section"** —— 依赖被加到了
  工作区清单（`[workspace]`，没有 `[package]`）。把 `[dependencies]` 行移到 crate 清单
  （`bamboo-hello/bamboo-hello/Cargo.toml`）。
- **构建成功，但从未编译 `bamboo-native-ui`** —— 改动进了另一个 `Cargo.toml`，或文件没有保存。
  确认 crate 清单包含这一行，并在外层文件夹中运行 `cargo tree`。
- **修改后使用的仍是旧清单** —— 编辑器没有保存，或另一个终端运行在不同的检出中。按 Ctrl+S 保存，
  并在错误信息或 `cargo tree` 输出中核对路径。
- **`git` 不被识别为命令** —— 安装 Git for Windows，打开一个新终端，然后检查
  `git --version`。
- **`destination path 'native-ui' already exists`** —— 该位置已有一份检出。复用它，或在再次克隆前删除或重命名它。
- **链接步骤失败并提示 "link.exe not found"** —— 缺少 C++ 构建工具或
  Windows SDK。安装 **使用 C++ 的桌面开发**
  工作负载并打开一个新终端；详见第 01 页。
- **构建成功但没有出现窗口** —— 本页就是这个预期。
  `main.rs` 仍打印 `Hello, world!`；第 03 页会把它换成窗口。

## 下一步

[03 — 第一个窗口](03-first-window.md)：编写完整的 `Host`
实现并打开一个窗口。
