# 快速上手

> 十三个教程，带你从一个空文件夹走到一个运行中的窗口 ——
> 先是六个核心步骤，然后是七个实用配方，分别涉及布局、数据、
> 界面框架、桌面集成、主题、文本输入和播放器页面。

六个核心教程是线性的：每个都建立在前一个项目之上，但每个页面都会重复它所需的命令，因此你也可以跳到关心的那一篇，并复制它所假设的项目状态。实用教程复用同一个项目，并添加一个完整的功能领域。

## 本教程用到的术语

你不需要背下这份清单。第 01–04 页各自以一个简短小节开头，只解释该页用到的术语；最后一列说明每个术语首次在上下文中解释的位置。

| 术语 | 含义 | 首次出现 |
|---|---|---|
| Rust | 你的应用和这个框架所使用的编程语言；编译器会在程序运行之前检查整个程序。 | 01 |
| Cargo | Rust 的构建工具和包管理器：创建项目、获取依赖、编译它们并运行结果。 | 01 |
| crate | 一个作为整体编译的 Rust 包；框架是库 crate，你的程序是二进制 crate。 | 01 |
| 工作区（workspace） | 把多个 crate 组织在一起的 Cargo 项目，共用一个 `Cargo.lock` 和一个 `target/` 构建目录。 | 01 |
| 依赖（dependency） | 你的 crate 所依赖的另一个 crate，列在 `Cargo.toml` 的 `[dependencies]` 下。 | 02 |
| 特性开关（feature flag） | crate 内部一个有名字的可选开关，只有列进 `features = [...]` 时才会编译相关代码。 | 02 |
| 路径依赖（path dependency） | 通过磁盘上的文件夹路径找到的依赖，用于你可以修改的本地检出。 | 02 |
| git 依赖（git dependency） | Cargo 从 Git URL 克隆的依赖，用于你不保留本地检出的情形。 | 02 |
| 控件（widget） | 页面上的一个元素——按钮、标签或你自己的条——带有稳定 id、矩形、绘制代码和输入处理。 | 03 |
| `Host` | 你的应用实现的 trait（Rust 对接口的叫法），窗口通过它回调绘制、尺寸改变和输入。 | 03 |
| `Ui<M>` | 拥有一个页面全部控件的容器，并把输入路由给它们；`M` 是你定义的消息类型。 | 03 |
| 消息（message，`M`） | 控件返回的、用来说明发生了什么的值，例如“按钮被点击了”；你用一个 `enum` 定义它。 | 03 |
| 事件（event） | 一种规范化后的输入或窗口事件（指针移动、点击、按键），交给 `Host`；事件不是消息。 | 03 |
| DIP | 设备无关像素：1/96 英寸，宿主按显示器 DPI 缩放，因此同一尺寸在任何屏幕上看起来都一样大。 | 03 |
| 脏矩形（dirty rectangle） | 像素发生变化、必须重绘的矩形；框架会把它们合并，只把这些片段复制到窗口。 | 03 |
| 布局文档（layout document，`.buixml`） | 用 XML 文件声明控件矩形和属性，而不在 Rust 代码里写。 | 05 |
| 主题文档（theme document，`.buitheme`） | 一份包含调色板和主题值的 XML 文件，应用到运行中的应用。 | 11 |
| 代码生成（code generation） | 从 `.buixml` 文件生成 Rust 源码的构建步骤，因此发布的程序不含 XML 解析器。 | 06 |
| 过程宏（proc macro） | 在编译期运行并展开为代码的 Rust 宏；`layout_xml!` 就是其中之一。 | 06 |

## 开始之前

- **操作系统：** x64 上的 Windows 10 或 Windows 11，这是框架测试所用的配置。窗口宿主仅限 Windows；其他教程仍需它来运行结果。
- **Rust：** 支持 edition 2024 的稳定工具链（Rust 1.85 或更新）。从 <https://rustup.rs> 安装，并确保 MSVC 目标已激活：
  `rustup show`。
- **C++ 构建工具：** 带有 **使用 C++ 的桌面开发** 工作负载的 Visual Studio 2022 Build Tools，它提供 `link.exe` 和 Windows SDK。
- 一个终端。示例使用 PowerShell 记法；在 cmd 中命令相同。

## 核心教程

| 页面 | 主题 | 新增内容 |
|---|---|---|
| [01 — 创建项目](01-create-project.md) | `cargo new`、工作区、Windows 前置条件 | 一个可编译的二进制 |
| [02 — 添加 crate](02-add-the-crate.md) | GitHub 克隆、路径/git 依赖、特性开关、首次构建 | 链接好的框架 |
| [03 — 第一个窗口](03-first-window.md) | `Host`、`Ui<M>`、`WindowBuilder`、绘制/尺寸改变/事件 | 一个带标签和按钮的窗口 |
| [04 — 自定义控件](04-custom-widgets.md) | `CustomWidget`、实现 `Widget<M>`、皮肤协议 | 一个发出消息的绘制条 |
| [05 — 动态 XML](05-xml-dynamic.md) | 运行时 `.buixml`（`feature = "layout-xml"`） | 来自文件的矩形、属性和创建的控件，支持热重载 |
| [06 — 编译期 XML](06-xml-compiled.md) | 代码生成 CLI 和 `layout_xml!`（`feature = "layout-compiled"`） | 相同的布局作为生成的 Rust，无解析器 |

## 实用教程

| 页面 | 主题 | 新增内容 |
|---|---|---|
| [07 — 布局与断点](07-layout-and-breakpoints.md) | `VStack`/`HStack`/`split_h`/`grid`、`.buixml` 变体和表达式 | 一个响应式侧边栏外壳 |
| [08 — 列表、表格与树](08-lists-tables-trees.md) | `ListView`、`Table`、`TreeView`、XML 子列表 | 一个虚拟化的曲目浏览器 |
| [09 — 菜单、工具栏与状态栏](09-menus-toolbars-status.md) | `MenuBar`、菜单按钮、`Toolbar`、`StatusBar`、上下文菜单 | 按命令 id 路由的界面框架 |
| [10 — 对话框、托盘与热键](10-dialogs-tray-hotkeys.md) | 原生对话框、托盘图标、通知、全局热键与捕获 | 桌面集成 |
| [11 — 主题与图像](11-theming-and-images.md) | `ThemeSettings`/`Palette`、`.buitheme`（`theme-xml`）、RGBA 图像 | 运行时主题和媒体资源 |
| [12 — 文本编辑与输入法](12-text-editing-and-ime.md) | 原生/绘制的 `TextEdit`、`SearchBox`、`NumberInput`、输入法组字 | 文本输入 |
| [13 — 播放器 UI](13-player-ui.md) | `CoverWall`、`PlaylistView`、`PlaybackTransport`、`WaveformSeekBar` | 一个媒体播放器页面 |

## 每页你需要了解的内容

- 第 01 和 02 页是设置；代码出现在第 03 页。
- 第 03 页是完整的参考程序。大多数后续页面修改它。
- 第 05 和 06 页是定位控件的替代方法。它们替换第 03 页的
  `ui.set_rect` 调用，而不是 `Host` 实现。
- 第 07–13 页是独立的配方；每一页都指出它所假设项目的更早页面。

## 接下来去哪里

- [架构](../overview/architecture.md) 了解各层如何衔接。
- [设计原则](../overview/design-principles.md) 了解脏矩形
  契约。
- [宿主与窗口](../api/host-and-window.md) 了解完整的 `Host` 和
  `WindowHandle` 表面。
