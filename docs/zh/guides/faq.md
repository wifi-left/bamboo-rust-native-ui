# 常见问题

> 对采用该框架之前的常见问题的简短回答。

## 为什么不用 egui 或其他立即模式库？

立即模式 UI 每帧都会重新布局并重绘整个窗口。桌面
音乐播放器大部分时间处于空闲：一个静态页面、一首曲目、一个每秒变化一次的进度条。
Bamboo Native UI 保留控件、记录
损坏、只将脏矩形绘制到持久化后备缓冲区，并只
blit 那些片段。空闲即零绘制。

代价是应用要计算矩形，并在
状态变化之后使失效。完整的权衡表见[设计原则](../overview/design-principles.md)。

## 它跨平台吗？

不。窗口宿主、GDI/Direct2D 后端、原生菜单、托盘、对话框和
UIA provider 都是仅 Windows 的（`#[cfg(windows)]`）。可移植层（`Ui`、
`Widget`、`Painter`、`Canvas`、`Event`、几何、主题和布局模型）
可以在任何平台编译，所以控件测试才能在没有窗口的情况下
运行。

## DPI 如何处理？

进程是按显示器 DPI 感知 v2 的
（`DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`）。公共 API 中的每个矩形、字号和
半径都以 DIP 为单位；平台按 `dpi / 96` 缩放。DPI
变化会在字体缓存丢弃之后调用 `Host::dpi_changed`，然后
常规的调整尺寸路径重新计算矩形。`.buixml` 文档中的百分比和 `pill`
值会重新解析。

## 我可以使用哪些字体？

主题字体系列默认为 `Microsoft YaHei UI`，字形运行使用
`Segoe MDL2 Assets`。`ThemeSettings::font_family` 选择另一个**已安装的
字体系列名称**；`WidgetProps::font`/`font_size`/`font_family` 覆盖单个
控件。不支持加载字体文件：请使用系统已有的字体系列。
`ThemeSettings::font_size` 缩放角色样式，钳制在 13.5 DIP 参考值的 `0.85..=1.4` 范围内。

## 中文文本能用吗？

能。默认字体系列覆盖中文，一行没有空格时，换行会在字符之间
断行，因此 CJK 段落能正确换行。源文件是
UTF-8。某些库默认值（强调色预设标签、播放重复标签）以中文
提供，因为它们与播放器一致；在你需要另一种语言的地方使用你自己的标签。

## 图像如何工作？

创建一个 `UiImage`，用 `Ui::set_image` 注册它，让控件通过
`ImageId` 显示它。存储是页面作用域的；注册相同的 id 会替换
像素。没有路径、没有文件加载、也没有图像名称注册表：
应用解码或生成像素并将它们交出。在 `.buixml` 中，
子元素上的 `image` 属性是数字 `ImageId`（`image="7"`）。
参见[图像](../api/images.md)。

## `.buixml` 文档能把事件绑定到 Rust 函数吗？

不能。文档分配几何、属性、样式和初始内容。点击、
按键、拖动和焦点保留在 Rust 中：实现 `Widget::input`、使用
`on_click`/`on_key_down` 观察者，或用 `Ui::bind` 绑定。文档可以通过
`WidgetRegistry` 创建一个控件并给它一个 `command`，但该
命令仍然通过你的 `CommandMap` 解析为应用消息。

## 热重载如何工作？

使用 `layout-xml` 时，轮询文件，调用 `LayoutDocument::parse`，比较
`generation()` 并调用 `Ui::apply_layout`。解析或应用出错时，
先前的文档保持活动。库不执行文件 IO，也不启动
监视器。编译布局（`layout-compiled`、`layout_xml!`）没有热
重载：重新生成并重建。`layout_xml!` 宏也无法注册
文件依赖，因此仅仅编辑 `.buixml` 不会触发重建。

## 原生文本编辑器还是绘制式编辑器？

`TextEdit`（以及 `SearchBox`、`TextArea`、`PasswordBox`）默认使用原生 `EDIT`
子控件：平台拥有会话、光标、选择和上下文
菜单，应用接收 `edit_changed`/`edit_committed`。调用
`.painted()` 保留相同的控件 API，但用框架绘制就地编辑：
垂直居中的单行文本、可配置的光标/选择
颜色、带候选框定位的内联 IME 组合、撤销/剪贴板和
实时 UIA 文本，且没有原生子控件。选择原生以获得最大的平台
保真度；选择绘制式以获得一致的样式和内联组合。参见
[TextEdit](../widgets/inputs/text-edit.md)。

## 它有自动布局吗？

没有测量阶段：应用分配矩形。使用 `Ui::set_rect`、
`bamboo_native_ui::layout` 辅助函数（`VStack`、`HStack`、`padded`、`split_h`、
`split_v`、`grid`）或一份 `.buixml` 文档。控件有一个
`preferred_height`，只有可选的垂直堆叠辅助函数会用到它，从不用
于页面布局。这是有意为之：调整尺寸是算术，而不是约束
求解。

## 为什么我的 `apply_layout` 什么也不做？

它是原子的。任何错误都会返回 `LayoutFailure` 且不修改任何内容；先前的
布局保持不变。打印 `failure`（或遍历 `failure.diagnostics`）并
检查报告的行/代码。解析成功但 generation 没有
改变也会跳过内容写入。参见[调试](debugging.md)。

## 我可以做动画吗？

可以，但要显式。安装 `WindowHandle::set_animation_timer(Some(duration))`，
在 `Host::timer` 中推进状态，只使动画矩形失效，并调用
`ui.flush_damage(window)`。动画结束时停止计时器。没有
帧循环，空闲时不重绘。参见[性能](performance.md)。

## 授权是什么？

Apache License 2.0。只要你保留
版权声明和 `NOTICE` 文件，并在修改源码时说明
重大更改，就允许商业和闭源使用。你不必在应用 UI 中显示署名。参见
仓库根目录的 `LICENSE` 和 `NOTICE`。

## 我如何将它用作 git 子模块？

播放器在 `native-ui/` 跟踪此仓库，并通过路径依赖它：

```toml
bamboo-native-ui = { path = "native-ui/crates/native-ui" }
```

全新克隆必须使用 `git clone --recursive` 或
`git submodule update --init --recursive`。不要把 `native-ui/` 添加到
父仓库的 `.gitignore`：父仓库必须跟踪该 gitlink，否则全新克隆
无法构建。按需在依赖上添加特性开关（参见
[特性开关](feature-flags.md)）。

## 另请参阅

- [项目介绍](../overview/introduction.md)
- [设计原则](../overview/design-principles.md)
- [架构](../overview/architecture.md)
- [主题](theming.md)
- [XML 布局编写](xml-layout-authoring.md)
- [特性开关](feature-flags.md)
