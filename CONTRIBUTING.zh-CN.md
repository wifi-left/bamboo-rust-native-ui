# 为 Bamboo Native UI 做贡献

感谢你考虑贡献。本项目是一个小巧且坚持取舍的原生 Windows UI 框架，依靠
谨慎、经过测试的改动成长。Bug 修复、新功能、文档、翻译、辅助功能改进和
跨平台移植都欢迎。

English: [CONTRIBUTING.md](CONTRIBUTING.md)

## 开始之前

- 先搜索[已有 issue](https://github.com/wifi-left/bamboo-rust-native-ui/issues)。
- 较大的改动、新的公共 API、新的依赖或跨平台移植，请先开 issue 讨论方向。
- 阅读
  [`docs/zh/overview/design-principles.md`](docs/zh/overview/design-principles.md)
  中的设计契约，以及 [`docs/zh/`](docs/zh/index.md) 中的控件/API 参考。
- 保持尊重和具体。批评代码，而不是人。

## 贡献方式

### 修复 Bug

一个好的 Bug 报告应包含最小复现、准确命令、操作系统与 Windows 版本、Rust
版本、启用的特性、渲染器（`gdi`、`d2d-software`、`d2d-hardware`），以及
实际行为与预期行为。视觉问题请附截图或短录屏。

修复本身：

- 能在无窗口环境测试的行为，请补回归测试（`layout-core` 的大部分逻辑、
  布局应用、控件输入/状态、几何）。
- 保持改动聚焦；不要把重构和修复混在一起。
- 运行下面的检查。

### 新功能

请先开功能 issue。说明要解决的问题、建议的 API 或 XML 表面、替代方案，以及
改动带来的复杂度成本。框架刻意避免“每帧重新布局/重绘”的模型、运行时依赖
和绘制路径上的分配；需要这些的功能要先做设计讨论，而不是直接提交补丁。

有价值的功能方向包括控件、布局文档词汇、主题、辅助功能、性能、示例和工具。

### 文档

文档位于 `docs/en/`（英文是源）和 `docs/zh/`（简体中文镜像）。面向用户的
改动应同时更新两者。

- 遵循 [`docs/DOC-GUIDE.md`](docs/DOC-GUIDE.md)：页面模板、术语和链接规则。
- 控件页必须包含 Introduction、Usage (Rust)、Usage (XML)、Properties、
  Events、XML definition 和 See also。
- 用 Zensical 构建并保持无警告：
  `zensical build -s -f mkdocs.yml` 和 `zensical build -s -f mkdocs.zh.yml`。

### 翻译

翻译是一等贡献。`docs/en/` 是源；翻译在 `docs/<lang>/` 下按相同相对路径镜像。

- 代码、标识符、XML/TOML、命令、路径和链接目标保持不变；翻译正文、标题、
  表格和代码注释。
- 使用 `docs/zh/` 中已有的术语（控件、宿主、消息、事件、触发器、注册表、
  布局文档……）。
- 新增或更新 `mkdocs.<lang>.yml`，并在其他配置中更新语言切换。
- 运行下面的链接检查。翻译 PR 不应悄悄改变技术内容；如果英文页有误，先在
  英文页修正。

欢迎新增语言。请在翻译 issue 中说明语言代码和范围；Pages 工作流目前只构建
英文和中文，因此新语言还需要增加工作流条目。

### 跨平台支持

公共类型是平台中立的，但窗口宿主和平台层目前仅支持 Windows
（`#[cfg(windows)]`）。移植是工作量较大但范围清晰的项目：

- **后端**：实现 `Canvas` trait（填充、圆角矩形、文本、裁剪、图像、渐变），
  并为目标平台提供 `Renderer` 选择。
- **窗口宿主**：消息循环、`Host` 回调、DPI、计时器、脏区跟踪、每个窗口一个
  持久后台缓冲，以及脏包围盒 blit。
- **平台模块**：窗口创建、键盘、菜单、托盘、通知、对话框、拖放、辅助功能、
  软键盘和 IME。
- **保持契约**：应用负责矩形、只绘制脏区、空闲零绘制、绘制路径无分配、
  公共 API 平台中立。

请开一个跨平台 issue，说明目标系统、想先做的层，以及构建/测试计划。控件层和
布局模型保持共享，不要按平台分叉。新目标需要 CI 和一个冒烟测试
（相当于 `showcase --self-test`）。

### 辅助功能、性能、示例

辅助功能修复（UI Automation 角色、名称、键盘路径、Narrator）、有测量的性能
改进和新展示示例都欢迎。性能改动请附 `--bench` 或绘制日志的前后数据；辅助
功能改动请附 `--a11y-dump` 输出或 Narrator 复现步骤。

## 开发环境

前置条件：Windows 10/11 x64、Rust 1.85+（MSVC 目标）、Visual Studio 2022
Build Tools 的 Desktop C++ 工作负载，以及 Windows SDK。

```powershell
git clone https://github.com/wifi-left/bamboo-rust-native-ui.git native-ui
cd native-ui
cargo build
cargo test
cargo run -p bamboo-native-ui-showcase -- --self-test
cargo run -p bamboo-native-ui-showcase -- --a11y-dump
```

文档：

```powershell
pip install -r requirements-docs.txt
zensical build -s -f mkdocs.yml
zensical build -s -f mkdocs.zh.yml
```

仓库布局和特性开关见 [`README.zh-CN.md`](README.zh-CN.md) 与
[`docs/zh/guides/feature-flags.md`](docs/zh/guides/feature-flags.md)。

## 必须遵守的设计规则

这些不是风格偏好，而是框架赖以成立的契约。破坏其中一条的改动需要明确的
设计决策。

- 应用计算控件矩形。框架只绘制脏矩形并分发规范化事件；状态变化从不触发
  重新布局或整窗重绘。
- 每个窗口一个持久后台缓冲；只 blit 脏包围盒。空闲即零绘制；没有显式计时器
  就不会周期性重绘。
- 绘制路径上禁止分配。借用文本和脏矩形；字体、画刷和转换后的图像在窗口
  生命周期内缓存。
- 公共类型保持平台中立；Win32/WinUI/Direct2D 类型留在平台层内。
- 新依赖需要理由和讨论；默认构建不链接 XML 解析器。
- 布局应用是原子的：验证失败的文档不改变任何东西。
- 框架 setter 是静默的并记录脏区；由宿主统一 flush。

## Pull Request

1. Fork 仓库并创建聚焦的分支。
2. 完成改动，添加测试，并更新 `docs/en/` 与 `docs/zh/`。
3. 运行下面的检查。
4. 使用模板开 PR，并链接它关闭的 issue。
5. PR 保持在可审查的规模。机械重构与行为改动分开。

提交和 PR 标题使用 `type: summary` —— `feat:`、`fix:`、`docs:`、`i18n:`、
`refactor:`、`test:`、`perf:`、`chore:`。正文说明改了什么、为什么。

### 必须运行的检查

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets
cargo build                     # 0 warnings
cargo test
cargo run -p bamboo-native-ui-showcase -- --self-test
cargo run -p bamboo-native-ui-showcase -- --a11y-dump
zensical build -s -f mkdocs.yml
zensical build -s -f mkdocs.zh.yml
```

仅文档改动且没有改代码/特性文件时，可以跳过 Rust 检查，但要在 PR 中说明。
文档构建和链接检查始终必须通过。

### AI 辅助贡献

欢迎 AI 辅助的改动。请在 PR 中说明，保持 diff 可审查，并自行验证结果——生成
的代码和文档看起来合理，也可能是错的。你需要对提交的改动负责。

## 后续会考虑的方向

以下是项目可能发展的方向；**不是承诺**，也不代表时间表。

- 跨平台后端（更多 `Canvas` 实现和窗口宿主）。
- 更多翻译和更顺畅的多语言工作流。
- 更深入的辅助功能覆盖和自动化 UI Automation 检查。
- 性能工作：绘制统计、脏区合并、文本缓存。
- 更多控件、布局文档词汇和主题灵活性。
- 更好的示例，以及兼作视觉测试的组件画廊。
- 第三方控件和 XML 类型注册表的扩展方案。

如果你想做其中一项，请开 issue 说明范围。小而经过测试的第一步，比庞大但
未完成的分支更有用。

## 许可证

项目使用 Apache License 2.0。贡献即表示你同意以相同条款授权你的贡献。
没有 CLA。修改或再分发源码时，请保留现有的版权和 `NOTICE` 要求。
