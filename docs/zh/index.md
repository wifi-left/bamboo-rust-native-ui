# Bamboo Native UI

一个小巧的声明式原生 Windows UI 框架：用 GDI + 脏矩形渲染，配一个后端抽象的绘制器，不依赖 egui、wgpu 或 WebView。你负责计算控件矩形；框架只绘制脏矩形并分发规范化后的输入。空闲即零绘制。

本文档覆盖 `bamboo-native-ui` 工作区：框架 crate、布局模型/代码生成，以及全控件演示 showcase。

## 从这里开始

| 如果你想…… | 阅读 |
|---|---|
| 了解这是什么以及为何存在 | [简介](overview/introduction.md) |
| 了解各部分如何衔接 | [架构](overview/architecture.md) |
| 了解渲染与所有权契约 | [设计原则](overview/design-principles.md) |
| 十分钟内构建一个应用 | [快速上手](quickstart/index.md) |
| 查询某个控件 | [控件目录](widgets/index.md) |
| 查询某个类型或函数 | [API 参考](api/index.md) |
| 解决某个具体问题 | [指南](guides/index.md) |

## 两分钟版本

```rust
use bamboo_native_ui::{
    Color, Host, PaintCtx, Rect, Size, ThemeSettings, Ui, WidgetId, WindowBuilder, WindowHandle,
};

enum Msg { Play }

struct App { ui: Ui<Msg> }

impl App {
    fn new() -> Self {
        let mut ui = Ui::new();
        ui.add(bamboo_native_ui::widgets::Button::new("play", "Play", Msg::Play));
        Self { ui }
    }
}

impl Host for App {
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        for rect in ctx.dirty {
            ctx.canvas.fill_rect(*rect, ctx.painter.palette().background);
        }
        self.ui.paint(ctx);
    }

    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        self.ui.set_rect(
            WidgetId::new("play"),
            Rect::from_xywh((size.width - 120.0) / 2.0, (size.height - 36.0) / 2.0, 120.0, 36.0),
        );
        window.invalidate_all();
    }
}

fn main() -> windows::core::Result<()> {
    WindowBuilder::new(App::new())
        .title("Hello")
        .size(640.0, 400.0)
        .theme(ThemeSettings::default(), true)
        .run()
}
```

完整的演练见[你的第一个窗口](quickstart/03-first-window.md)。

## 文档地图

### 概览

- [简介](overview/introduction.md) —— 它是什么、状态、适用人群。
- [架构](overview/architecture.md) —— crate、层次、消息与绘制流程。
- [设计原则](overview/design-principles.md) —— 脏矩形契约及其权衡。

### 快速上手

核心教程：

1. [创建项目](quickstart/01-create-project.md)
2. [添加 crate](quickstart/02-add-the-crate.md)
3. [你的第一个窗口](quickstart/03-first-window.md)
4. [自定义控件](quickstart/04-custom-widgets.md)
5. [运行时 XML 布局](quickstart/05-xml-dynamic.md)
6. [编译期 XML 布局](quickstart/06-xml-compiled.md)

实用教程：

7. [布局与断点](quickstart/07-layout-and-breakpoints.md)
8. [列表、表格与树](quickstart/08-lists-tables-trees.md)
9. [菜单、工具栏与状态栏](quickstart/09-menus-toolbars-status.md)
10. [对话框、托盘与热键](quickstart/10-dialogs-tray-hotkeys.md)
11. [主题与图像](quickstart/11-theming-and-images.md)
12. [文本编辑与输入法](quickstart/12-text-editing-and-ime.md)
13. [播放器 UI](quickstart/13-player-ui.md)

### API 参考

基础：[几何与颜色](api/geometry-and-color.md)、
[主题](api/theme-and-palette.md)、[Ui](api/ui-container.md)、
[控件 trait](api/widget-trait.md)、[事件](api/events-and-input.md)、
[绘制](api/painting-and-canvas.md)、[宿主/窗口](api/host-and-window.md)、
[布局辅助函数](api/layout-helpers.md)。

能力：[图像](api/images.md)、[滚动](api/scrolling.md)、
[无障碍](api/accessibility.md)、[对话框](api/dialogs.md)、
[菜单](api/menus.md)、[托盘/通知](api/tray-and-notifications.md)、
[热键](api/hotkeys.md)、[拖放](api/drag-and-drop.md)、
[手柄](api/gamepad.md)。

XML 与工具：[布局文档](api/layout-documents.md)、
[代码生成](api/layout-codegen.md)、[主题文档](api/theme-documents.md)、
[注册表](api/widget-registry-and-creation.md)、
[属性/叙述](api/props-and-narration.md)、
[诊断](api/diagnostics-and-debug.md)。

### 控件

[目录](widgets/index.md)按类别列出全部 62 个控件。每个控件页面都包含 Rust 示例、XML 示例、属性表、事件一节（消息、共享观察者、控件专属回调、`Ui` 触发器），以及精确的 XML 定义规则。

### 指南与参考

- [主题](guides/theming.md)
- [XML 布局编写](guides/xml-layout-authoring.md)
- [性能](guides/performance.md)
- [调试](guides/debugging.md)
- [测试](guides/testing.md)
- [特性开关](guides/feature-flags.md)
- [无障碍](guides/accessibility.md)
- [常见问题](guides/faq.md)
- [调色板令牌](reference/palette-tokens.md)
- [XML 元素](reference/xml-elements.md)
- [键码与热键](reference/key-codes-and-hotkeys.md)

## 面向 AI 助手

一个紧凑、自包含的上下文文件在
[`ai/reference.md`](ai/reference.md)，另有一页
[控件查询表](ai/widgets.md)。仓库级别的 `docs/llms.txt` 是遵循 llmstxt.org 约定的工具的入口。

## 状态

框架正在积极开发中。控件、按实例属性、触发器、UI 自动化、工具提示、菜单、对话框、托盘和热键均已就位；M6.5 组件组已完成；M7 布局皮肤（`.buixml`）和 M7.8 主题文档（`.buitheme`）已实现。showcase 覆盖了每一个领域。详情见[简介](overview/introduction.md)。

## 贡献

欢迎 Bug 修复、新功能、文档、翻译和跨平台工作。请阅读
[CONTRIBUTING.zh-CN.md](https://github.com/wifi-left/bamboo-rust-native-ui/blob/main/CONTRIBUTING.zh-CN.md)
（English:
[CONTRIBUTING.md](https://github.com/wifi-left/bamboo-rust-native-ui/blob/main/CONTRIBUTING.md)），
并使用 issue 模板提交 Bug 报告、功能建议、翻译或跨平台方案。

## 许可证

Apache License 2.0。参见仓库根目录的 `LICENSE` 和 `NOTICE` 文件。
