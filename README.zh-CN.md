# Bamboo Native UI

一个小巧的、声明式的原生 Windows UI 框架，为 Bamboo Music
Player 构建。使用 GDI + 脏矩形渲染、一个后端抽象的绘制器，且不依赖
egui、wgpu 或 WebView。

> **AI 辅助项目。** 代码、测试和文档大部分由 AI 编码助手
> 生成或共同编写，可能包含错误。
> 依赖之前请先审查并测试。

## 为什么

立即模式 UI 每帧都会重新布局并重绘整个窗口。本框架
保持相反的契约：

- 应用计算控件矩形；框架只绘制脏矩形
  并分发规范化后的事件。
- 每个窗口一个持久化后备缓冲区；只 blit 脏包围盒。
- 空闲即零绘制。没有显式计时器就不会周期性重绘。
- 禁止绘制路径上的分配；画刷、字体和转换后的图像在
  窗口的整个生命周期内被缓存。
- 公共类型是平台中立的；Win32/GDI/Direct2D 宿主是唯一的
  平台层。

## 特性

- 62 个控件：文本/内容、输入、控件、选择器、数据视图（虚拟化
  列表/表格/树/属性网格）、导航、反馈、播放器组件、
  窗口装饰（菜单栏、功能区、工具栏、状态栏）、浮层和自定义控件。
- 保留式 `Ui<M>` 容器，支持焦点、命中测试、悬停/按下、拖动、
  滚动、逐实例属性和触发器绑定。
- UI Automation、原生菜单、对话框、托盘图标、通知、全局和
  窗口热键、系统拖放、手柄导航。
- `.buixml` 布局文档：运行时动态（`layout-xml`）或编译为
  Rust（`layout-compiled` / `layout_xml!`）。
- `.buitheme` 主题文档，用于声明整个调色板（`theme-xml`）。
- GDI 默认，Direct2D 软件/硬件后端。

## 快速上手

```powershell
cargo test
cargo run -p bamboo-native-ui-showcase
cargo run -p bamboo-native-ui-showcase -- --self-test   # 绘制一次，退出 0
cargo run -p bamboo-native-ui-showcase -- --a11y-dump   # UIA 客户端检查 + 树
cargo run -p bamboo-native-ui-showcase -- --light
```

从另一个 crate 使用它：

```toml
[dependencies]
bamboo-native-ui = { path = "native-ui/crates/native-ui" }
# 可选特性：
# features = ["layout-xml", "layout-compiled", "theme-xml", "debug-form"]
```

最小程序形态是一个 `Host` 实现加上一个 `Ui<M>`：

```rust
use bamboo_native_ui::{Host, PaintCtx, WindowBuilder, WindowHandle};

struct App { /* Ui<Msg> 和应用状态 */ }

impl Host for App {
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        for rect in ctx.dirty {
            ctx.canvas.fill_rect(*rect, ctx.painter.palette().background);
        }
        // self.ui.paint(ctx);
    }
    // resize()：用 self.ui.set_rect(..) 定位控件；window.invalidate_all()
}

fn main() -> windows::core::Result<()> {
    WindowBuilder::new(App { /* .. */ })
        .title("Hello")
        .size(800.0, 600.0)
        .run()
}
```

完整程序参见[第一个窗体教程](docs/zh/quickstart/03-first-window.md)。

## 文档

| 资源 | 内容 |
|---|---|
| [English docs](docs/en/index.md) | 综述、十三个快速上手（六个核心、七个实战）、API 参考、62 个控件页面、指南、查找表。 |
| [中文文档](docs/zh/index.md) | 完整中文文档；英文文档是完整来源，`zh/` 树与之镜像对应。 |
| [llms.txt](docs/llms.txt) | 面向 AI 的索引，遵循 llmstxt.org 约定；中文版见 [llms.zh.txt](docs/llms.zh.txt)。 |
| [AI reference](docs/en/ai/reference.md) | 一个自包含的上下文文件：crate 地图、特性、签名、控件目录、XML/主题语法；中文 AI 索引入口见 [llms.zh.txt](docs/llms.zh.txt)。 |
| 本地设计记录 | `docs/archive/`（未跟踪）：`API.md`、`DECISIONS.md`、`M7-XML.md`、`CUSTOM-WIDGETS.md`。 |

使用 mkdocs-material 构建 HTML 站点：

```powershell
pip install -r requirements-docs.txt
mkdocs build              # English -> site/
mkdocs build -f mkdocs.zh.yml
```

`.github/workflows/docs.yml` 将两者都发布到 `gh-pages` 分支。

## 仓库布局

```
crates/native-ui/        框架本体（bamboo-native-ui）
crates/layout-core/      .buixml/.buitheme 模型、解析器、写入器
crates/layout-codegen/   .buixml -> Rust 生成器（CLI + 库）
crates/layout-macros/    layout_xml! 过程宏
crates/showcase/         全控件演示（bin: showcase）
docs/                    中英文文档与归档的设计记录
```

## 状态

控件、逐实例属性、触发器、UI Automation、工具提示、上下文
菜单、对话框、托盘、热键、M6.5 组件组、M7 布局皮肤
以及 M7.8 主题文档均已实现。showcase 覆盖了每个
区域。未完成项记录在本地 `TODO.md` 中；`docs/HANDOFF.md` 是
下一次会话的恢复点。两者都刻意不纳入版本控制。

## 作为子模块使用

播放器仓库在 `native-ui/` 跟踪本仓库。全新克隆必须
使用 `git clone --recursive`（或 `git submodule update --init --recursive`），以便
路径依赖能够解析。不要把 `native-ui/` 添加到父仓库
`.gitignore`。

## 授权

Apache License 2.0。参见 [`LICENSE`](LICENSE) 和 [`NOTICE`](NOTICE)。允许商业
和闭源使用；在任何分发中保留版权声明和 `NOTICE`，
如果你修改源码，请说明重大更改。
