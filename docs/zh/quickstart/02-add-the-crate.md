# 02 — 添加 crate

> 通过路径链接 `bamboo-native-ui`，选择你需要的特性，并构建一次。

## 你将构建

第 01 页中的 `bamboo-hello` crate，将框架作为依赖。构建会编译框架及其 Windows 依赖；在第 03 页之前，`main.rs` 仍打印 `Hello, world!`。

## 前置条件

完成 [01 — 创建项目](01-create-project.md)。你需要 `bamboo-hello` 工作区、可用的 MSVC 工具链和 Windows SDK，以及本地检出
<https://github.com/wifi-left/bamboo-rust-native-ui>，或克隆它的网络访问。配置好 Git 代理即可；仓库是公开的。

## 步骤

### 1. 从 GitHub 获取框架

仓库公开在
<https://github.com/wifi-left/bamboo-rust-native-ui>。把它克隆到工作区旁边：

```powershell
git clone https://github.com/wifi-left/bamboo-rust-native-ui.git native-ui
```

如果框架属于你项目的版本历史，把它添加为子模块，这样提交会固定精确的版本：

```powershell
git submodule add https://github.com/wifi-left/bamboo-rust-native-ui.git native-ui
git submodule update --init --recursive
```

两种方式都会得到下面要用的 `native-ui/` 文件夹。crate 本身位于
`native-ui/crates/native-ui`；仓库根目录是虚拟工作区，不是库。

### 2. 指向框架

框架不在 crates.io 上；引用 crate 文件夹。如果你的项目与仓库同级：

```
F:\codes\
├── bamboo-hello\          your workspace from page 01
└── native-ui\             the framework repository
    └── crates\native-ui\
```

然后编辑 `bamboo-hello/Cargo.toml`：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui" }
```

当框架作为 git 子模块检入你的仓库时，播放器的布局是参考：

```toml
bamboo-native-ui = { path = "native-ui/crates/native-ui" }
```

使用指向 **crate**（`crates/native-ui`）的路径，而不是工作区根目录。指向根目录的路径会解析到虚拟工作区清单，而它不是库。

#### 或者直接依赖 GitHub 仓库

如果你不想本地检出，Cargo 可以拉取整个工作区：

```toml
[dependencies]
bamboo-native-ui = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui" }
```

必须写 `package`：URL 指向工作区根目录，Cargo 按名称选择 `bamboo-native-ui`
成员。用 `rev = "<commit>"` 固定版本以保证可复现，或用 `branch = "main"`
跟随默认分支。特性的写法相同：

```toml
bamboo-native-ui = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui", features = ["layout-xml"] }
```

git 形式会在首次构建时拉取整个工作区，并缓存到 `~/.cargo/git`；本地迭代时
path 形式更快。代码生成 CLI 是一个二进制，因此要运行它请克隆仓库；
`layout_xml!` 过程宏也可以作为 git 依赖：

```toml
bamboo-native-ui-layout-macros = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui-layout-macros" }
```

### 3. 选择特性

`bamboo-native-ui` 有五个特性。默认只开启 `gdi`，这对第 03 页足够。

| 特性 | 默认 | 启用 |
|---|---|---|
| `gdi` | 是 | GDI 后端（以及 Direct2D 渲染器选项）。保持开启。 |
| `layout-xml` | 否 | `.buixml` 解析（`LayoutDocument::parse`）和用于热重载的 `Ui::apply_layout` |
| `layout-compiled` | 否 | 生成的布局（`CompiledLayout`、`Ui::apply_compiled`）；无解析器 |
| `theme-xml` | 否 | 映射到 `Palette` 的 `.buitheme` 主题文档 |
| `debug-form` | 否 | 只读调试快照（`Ui::debug_snapshot`）和绘制统计；蕴含 `layout-xml` |

如果你知道需要它们，现在启用，或者在用到它们的页面上添加：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

`gdi` 是累加的，因此 `features = ["layout-xml"]` 会保持它启用。要构建第 05 和 06 页中的示例，你分别需要 `layout-xml` 和 `layout-compiled`。

### 4. 构建工作区

```powershell
cargo build
```

首次构建会编译 `windows` crate 及其生成的绑定，因此需要一分钟或更久。预期输出（路径和耗时会有变化）：

```
   Compiling windows v0.62.2
   Compiling bamboo-native-ui v0.1.0 (F:\codes\native-ui\crates\native-ui)
   Compiling bamboo-hello v0.1.0 (F:\codes\bamboo-hello\bamboo-hello)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 12s
```

无警告的成功构建是框架自身 `docs/HANDOFF.md` 在提交前要求的基线；如果你看到来自*你的* crate 的警告，请在继续之前修复它们。

### 5. 关于子模块

如果框架作为 git 子模块存在于你的仓库中，全新的克隆必须初始化它：

```powershell
git clone --recursive <your-repo>
# or, in an existing clone:
git submodule update --init --recursive
```

不要将子模块文件夹添加到 `.gitignore`；父仓库必须跟踪 gitlink，否则全新克隆无法解析路径依赖。

## 预期结果

`cargo build` 以 `Finished` 结束且无错误。`cargo run` 仍打印
`Hello, world!`。框架已链接并可使用。

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

## 下一步

[03 — 第一个窗口](03-first-window.md)：编写完整的 `Host`
实现并打开一个窗口。
