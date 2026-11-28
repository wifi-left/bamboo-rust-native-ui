# API 参考

公共 API 按主题分组。每个页面都是一份查阅参考：真实签名、简短示例和真正重要的限制。如果你是新手，请先从
[Ui 容器](ui-container.md)和[控件 trait](widget-trait.md)开始；其余部分是你按需取用的能力。

每个页面都会注明特性开关（feature gate）。默认构建只启用 `gdi`；
`layout-xml`、`layout-compiled`、`theme-xml` 和 `debug-form` 都是选择性启用的（opt-in）。

## 基础

| 页面 | 概要 |
|---|---|
| [几何与颜色](geometry-and-color.md) | DIP 矩形、点、尺寸、内边距、圆角、损伤合并，以及平台无关的颜色。 |
| [主题与调色板](theme-and-palette.md) | 主题设置、解析后的调色板、强调色预设、列表密度和具名圆角半径。 |
| [Ui 容器](ui-container.md) | 保留式控件树：构建、路由、绑定、滚动、损伤、菜单和无障碍。 |
| [控件 trait](widget-trait.md) | `Widget<M>` 契约、输入/绘制规则、状态类型和可选的皮肤协议。 |
| [事件与输入](events-and-input.md) | 规范化的数据事件、按键码、鼠标按钮和修饰键，以及宿主如何路由它们。 |
| [绘制与 Canvas](painting-and-canvas.md) | 感知主题的绘制、绘图后端 trait、文本样式和渲染器选择类型。 |
| [宿主与窗口](host-and-window.md) | Windows 宿主回调、绘制上下文、窗口构建器/句柄以及渲染器选择。 |
| [布局辅助函数](layout-helpers.md) | 用于 `Host::resize` 的栈游标、内边距、带权分割和网格。 |

## 能力

| 页面 | 概要 |
|---|---|
| [图像](images.md) | 注册应用持有的 RGBA 像素，并通过 `ImageId` 引用它们。 |
| [滚动](scrolling.md) | 基于普通控件的可滚动视口，以及共享的滚动条模型。 |
| [无障碍](accessibility.md) | 编写 `A11yInfo`，并通过 UI Automation 暴露控件树。 |
| [对话框](dialogs.md) | 带有异步结果的原生消息、文件、字体和颜色对话框。 |
| [菜单](menus.md) | 声明式弹出菜单、原生菜单栏和菜单事件钩子。 |
| [托盘图标与通知](tray-and-notifications.md) | 通知区域图标和系统 Toast，并以气泡提示作为回退。 |
| [热键](hotkeys.md) | 解析、注册和捕获全局或每窗口键盘组合键。 |
| [拖放](drag-and-drop.md) | 文本和文件列表的系统级拖放。 |
| [游戏手柄](gamepad.md) | 将 XInput 控制器映射到键盘导航和软键盘。 |

## 布局文档、注册表与诊断

| 页面 | 概要 |
|---|---|
| [布局文档（`.buixml`）](layout-documents.md) | 解析后的模型、变体/选择、几何形式、表达式语法、常量和诊断。 |
| [布局代码生成与编译布局](layout-codegen.md) | 代码生成 CLI/库、生成的 `LAYOUT`/`install` 模块、`layout_xml!` 和 `apply_compiled`。 |
| [主题文档（`.buitheme`）](theme-documents.md) | `ThemeDocument`、调色板推导、`set_palette_token` 和主题文档语法。 |
| [控件注册表与创建](widget-registry-and-creation.md) | `WidgetRegistry`/工厂类型、`type`/`command`/`parent`、`STANDARD_TYPES`，以及注册 `x-*` 类型。 |
| [属性与朗读](props-and-narration.md) | `WidgetProps` 构建器、`FontSpec`、`Narration`、`ContentCaps` 和 `Ui::add_with`。 |
| [诊断与调试](diagnostics-and-debug.md) | 诊断类型/代码、`LayoutReport`/`LayoutFailure`、`debug-form` 快照和 `BAMBOO_UI_DEBUG`。 |

## 另见

- [控件目录](../widgets/index.md)
- [特性开关](../guides/feature-flags.md)
- [架构](../overview/architecture.md)
