# 指南

> 围绕主题、布局文档、性能、调试、测试、特性选择与辅助功能的任务导向式
> 讲解。

每一页都从一个可运行的窗口开始（参见[快速上手](../quickstart/index.md)），
以一个你能看到的结果或你能运行的命令结束。参考表格位于[参考](../reference/index.md)；
API 契约位于 [API](../api/theme-and-palette.md)。

## 选择一份指南

| 指南 | 适用场景 | 最终得到 |
|---|---|---|
| [主题](theming.md) | 你需要强调色、浅色/深色模式、字体设置、列表密度、圆角半径，或一份 `.buitheme` 文档。 | 一个在启动时安装、运行时可切换的设置结构体，外加一个可重新加载的主题文件。 |
| [XML 布局编写](xml-layout-authoring.md) | 你希望用 `.buixml` 而不是 Rust 来表达矩形、属性、样式和初始内容。 | 一份在 `resize` 中应用的文档，带有变体、常量和一个热重载循环。 |
| [性能](performance.md) | 你希望让空闲绘制保持为零、精确地使正确的矩形失效，并理解各类缓存。 | 一个只绘制变化区域的页面，以及一个会停止的动画计时器。 |
| [调试](debugging.md) | 布局被拒绝、控件不绘制、UIA 看起来不对，或你想要调试快照。 | 一个带有文件/行/代码的诊断失败，或一个能正常工作的 `--a11y-dump`。 |
| [测试](testing.md) | 你正在添加控件、修改解析器，或围绕工作区测试矩阵搭建 CI。 | 一个通过的特性矩阵，以及一个无需窗口的单元测试。 |
| [特性开关](feature-flags.md) | 你正在默认构建、`layout-xml`、`layout-compiled`、`theme-xml` 和 `debug-form` 之间做选择。 | 每种模式对应的一行依赖和一条构建命令。 |
| [辅助功能](accessibility.md) | 你正在编写 `A11yInfo`/`Narration`、接入 UIA，或用 `--a11y-dump` 检查树。 | 带有角色和名称的控件、一个能响应 UIA 的宿主，以及一棵经过验证的树。 |
| [常见问题](faq.md) | 你想要关于 egui、DPI、字体、中文文本、图像、热重载或授权的简短答案。 | 结论，以及指向详细页面的链接。 |

## 阅读路径

### 路径 1：第一个应用

1. [01 — 创建项目](../quickstart/01-create-project.md)
2. [02 — 引入 crate](../quickstart/02-add-the-crate.md)
3. [03 — 第一个窗体](../quickstart/03-first-window.md)
4. [主题](theming.md) —— 选择强调色和深色模式
5. [XML 布局编写](xml-layout-authoring.md) —— 将矩形从 Rust 中移出
6. [性能](performance.md) —— 让空闲窗口保持安静

### 路径 2：布局密集型应用

1. [05 — 运行时 XML](../quickstart/05-xml-dynamic.md)
2. [XML 布局编写](xml-layout-authoring.md)
3. [XML 元素](../reference/xml-elements.md) —— 查询某个属性
4. [调色板 token](../reference/palette-tokens.md) —— 查询某个颜色名称
5. [06 — 编译期 XML](../quickstart/06-xml-compiled.md) —— 在不带解析器的情况下发布
6. [特性开关](feature-flags.md) —— 选择发布组合

### 路径 3：框架贡献者

1. [架构](../overview/architecture.md)
2. [设计原则](../overview/design-principles.md)
3. [Widget trait](../api/widget-trait.md)
4. [04 — 自定义控件](../quickstart/04-custom-widgets.md)
5. [测试](testing.md)
6. [调试](debugging.md)

## 常见任务

| 任务 | 从这里开始 |
|---|---|
| 打开一个窗口并绘制一个标签 | [03 — 第一个窗体](../quickstart/03-first-window.md) |
| 实现一个自定义控件 | [04 — 自定义控件](../quickstart/04-custom-widgets.md) |
| 设置强调色、字号或深色模式 | [主题](theming.md) |
| 跟随 Windows 的浅色/深色设置 | [主题](theming.md) —— 自行解析模式，然后调用 `set_theme` |
| 将布局移入文件 | [XML 布局编写](xml-layout-authoring.md) |
| 在应用运行时重新加载布局 | [XML 布局编写](xml-layout-authoring.md) |
| 发布不带 XML 解析器的版本 | [06 — 编译期 XML](../quickstart/06-xml-compiled.md) |
| 查询 `background=` 的颜色名称 | [调色板 token](../reference/palette-tokens.md) |
| 查询 `<widget>` 的某个属性 | [XML 元素](../reference/xml-elements.md) |
| 将长页面放入滚动视图 | [滚动](../api/scrolling.md) |
| 停止重绘整个窗口 | [性能](performance.md) |
| 弄清布局为何被拒绝 | [调试](debugging.md) |
| 检查屏幕阅读器看到的内容 | [辅助功能](accessibility.md) 和 `--a11y-dump` |
| 添加一个无需窗口的控件测试 | [测试](testing.md) |
| 为开发版与发布版选择特性 | [特性开关](feature-flags.md) |

## 本文使用的约定

- **DIP** 指设备无关像素。公共 API 中的每个矩形、半径和字号都以 DIP 为单位；
  平台会乘以 `dpi / 96`。
- **控件 id** 是由 `&'static str` 构建的 `WidgetId`。文档只能寻址 Rust 侧
  已经创建的 id（除非它通过 `WidgetRegistry` 创建它们；参见
  [XML 布局编写](xml-layout-authoring.md)）。
- **布局文档**是一份 `.buixml` 文件。运行时读取器是
  `feature = "layout-xml"`；构建期形式是
  `feature = "layout-compiled"`。**主题文档**是一份 `.buitheme` 文件，
  由 `feature = "theme-xml"` 提供。
- 特性开关在首次出现时写作 `feature = "layout-xml"`，之后仅用名称。
- 框架 crate 是 `bamboo-native-ui`；容器是 `Ui<M>`；应用消息类型是 `M`。

## 特性一览

| 特性 | 默认 | 增加的内容 |
|---|---|---|
| `gdi` | 是 | Windows GDI 后端和 Direct2D 渲染器选项。 |
| `layout-xml` | 否 | 运行时 `.buixml` 解析和 `Ui::apply_layout`。 |
| `theme-xml` | 否 | 运行时 `.buitheme` 解析和 `ThemeDocumentExt`。 |
| `layout-compiled` | 否 | 用于生成 Rust 的 `Ui::apply_compiled`；不带解析器。 |
| `debug-form` | 否 | `Ui::debug_snapshot` 和绘制统计；隐含 `layout-xml`。 |

权衡与构建命令见[特性开关](feature-flags.md)。

## 各类页面所在位置

| 文件夹 | 回答的问题 |
|---|---|
| [快速上手](../quickstart/index.md) | “给我看一个完整的程序，一步一步来。” |
| [指南](index.md) | “我如何完成这项任务，代价是什么？” |
| [API](../api/theme-and-palette.md) | “这个模块的确切签名和行为是什么？” |
| [控件](../widgets/custom/custom-widget.md) | “这个控件做什么，我如何配置它？” |
| [参考](../reference/index.md) | “这个 token、属性或键的确切拼写是什么？” |
| [综述](../overview/introduction.md) | “我应该使用这个框架吗，它是如何构建的？” |

## 另请参阅

- [项目介绍](../overview/introduction.md)
- [架构](../overview/architecture.md)
- [设计原则](../overview/design-principles.md)
- [快速上手](../quickstart/index.md)
- [参考](../reference/index.md)
- [常见问题](faq.md)
