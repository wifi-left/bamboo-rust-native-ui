# 快速上手

> 十三个教程，带你从一个空文件夹走到一个运行中的窗口 ——
> 先是六个核心步骤，然后是七个实用配方，分别涉及布局、数据、
> 界面框架、桌面集成、主题、文本输入和播放器页面。

六个核心教程是线性的：每个都建立在前一个项目之上，但每个页面都会重复它所需的命令，因此你也可以跳到关心的那一篇，并复制它所假设的项目状态。实用教程复用同一个项目，并添加一个完整的功能领域。

## 开始之前

- **操作系统：** x64 上的 Windows 10 或 Windows 11，这是框架测试所用的配置。窗口宿主仅限 Windows；其他教程仍需它来运行结果。
- **Rust：** 支持 edition 2024 的稳定工具链（Rust 1.85 或更新）。从 <https://rustup.rs> 安装，并确保 MSVC 目标是激活的：
  `rustup show`。
- **C++ 构建工具：** 带有 **使用 C++ 的桌面开发** 工作负载的 Visual Studio 2022 Build Tools，它提供 `link.exe` 和 Windows SDK。
- 一个终端。示例使用 PowerShell 记法；在 cmd 中命令相同。

## 核心教程

| 页面 | 主题 | 新增内容 |
|---|---|---|
| [01 — 创建项目](01-create-project.md) | `cargo new`、工作区、Windows 前置条件 | 一个可编译的二进制 |
| [02 — 添加 crate](02-add-the-crate.md) | 路径依赖、特性开关、首次构建 | 链接好的框架 |
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
