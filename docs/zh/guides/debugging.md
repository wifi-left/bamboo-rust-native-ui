# 调试

> 打开框架的 stderr 诊断、运行 showcase 探针、检查一份
> 被拒绝的布局，并读取可选的调试快照。

## 可用的工具

| 工具 | 回答的问题 |
|---|---|
| `--self-test` | 窗口是否绘制、宿主路径是否工作？ |
| `--a11y-dump` | 一个真实的 UI Automation 客户端是否看到预期的树？ |
| `BAMBOO_UI_DEBUG` | 窗口在平台层面正在做什么？ |
| `BAMBOO_UI_PAINT_LOG` | 哪些控件绘制了，哪些很慢？ |
| `debug-form` 快照 | 每个控件和布局条目的有效状态是什么？ |
| `LayoutFailure` | 一份布局文档为何被拒绝？ |

showcase 探针是最快的起点，因为它们不需要修改代码。
环境变量适用于任何基于该框架构建的应用。

## 步骤

### 1. 运行 showcase 探针

```powershell
cargo run -p bamboo-native-ui-showcase -- --self-test
cargo run -p bamboo-native-ui-showcase -- --a11y-dump
cargo run -p bamboo-native-ui-showcase -- --light
cargo run -p bamboo-native-ui-showcase -- --renderer d2d-software
cargo run -p bamboo-native-ui-showcase -- --bench 2
```

- `--self-test` 创建窗口、绘制它、驱动一次原生编辑会话以
  走通宿主文本路径，并以状态 0 退出。非零退出码
  意味着某个探针失败；原因会被打印。
- `--a11y-dump` 启动窗口并在工作线程上运行一个真实的 UIA **客户端**。
  它向窗口请求其 provider，遍历控件视图并
  打印名称、角色、模式、值和选择。仅当
  探针成功时它才以 0 退出。
- `--light` 以浅色模式启动（否则 showcase 以深色启动）。
- `--renderer gdi|d2d-software|d2d-hardware` 在创建窗口之前设置 `BAMBOO_UI_RENDERER`。
- `--bench N` 运行 N 秒并打印帧数、tick 数、失效次数、
  平均绘制时间、私有字节数以及实际活动的渲染器。

showcase 的菜单栏还有一个构建于 `debug-form` API 的调试菜单
（快照、控件树、帧统计、诊断、重新加载/保存/重置）。那是
showcase 代码，不是库检查器。

### 2. 打开 `BAMBOO_UI_DEBUG`

`BAMBOO_UI_DEBUG` 通过 `std::env::var_os` 读取，因此任何非空值
都能启用它。诊断输出到 stderr：

```powershell
$env:BAMBOO_UI_DEBUG=1
cargo run -p bamboo-native-ui-showcase
```

典型消息：

- uxtheme 深色模式导出是否可用（菜单跟随页面）；
- 带有客户端尺寸和 DPI 的 `WM_SIZE`；
- 显露与遮挡决策（客户端被另一个窗口覆盖、显露
  区域增大、整窗重绘）；
- 屏幕阅读器请求树时的 `WM_GETOBJECT` 及节点数；
- 原生编辑器和 Direct2D 回退失败。

无法启动的 Direct2D 模式会在此记录日志并永久切换到 GDI；
此后 `WindowHandle::renderer()` 报告实际活动的那个。

### 3. 打开绘制日志

```powershell
$env:BAMBOO_UI_PAINT_LOG=1
cargo run -p bamboo-native-ui-showcase
```

启用 `debug-form` 时，`Ui::paint` 会打印任何绘制耗时超过 1.5 ms 的控件，
连同其 id 和矩形。showcase 会额外增加一行逐帧信息
（脏计数、合并矩形、控件计数、微秒数）。这仅是
诊断；它不改变绘制的内容。

### 4. 启用调试快照

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["debug-form"] }
```

`debug-form` 隐含 `layout-xml`；默认构建及其二进制文件保持
不含调试代码。快照是一个值：`Ui::debug_snapshot()` 遍历控件
树一次，解析有效状态并返回拥有所有权的数据。不会修改任何内容。

```rust
#[cfg(feature = "debug-form")]
fn report(ui: &bamboo_native_ui::Ui<()>) {
    let snapshot = ui.debug_snapshot();
    if let Some(layout) = &snapshot.layout {
        eprintln!(
            "variant={} client={:?} entries={} warnings={}",
            layout.variant,
            layout.client,
            layout.entries.len(),
            layout.warnings.len()
        );
    }
    for widget in &snapshot.widgets {
        eprintln!(
            "{} visible={} enabled={} focused={} a11y={:?}",
            widget.id.name(),
            widget.visible,
            widget.enabled,
            widget.focused,
            widget.a11y.as_ref().map(|info| info.name.as_str())
        );
    }
    eprintln!("pending damage: {:?}", snapshot.pending_damage);
    eprintln!("frames={} last={}us", snapshot.paint.frames, snapshot.paint.last_frame_micros);
}
```

`DebugSnapshot` 携带已安装的布局（条目、声明、滚动、
警告）、每个控件的有效可见/启用/焦点/只读状态、
经朗读解析的 `A11yInfo` 和皮肤词汇、交互 id、待处理和
上次损坏、绘制统计，以及一个 128 条目的诊断环形缓冲区。

相关调用：

| 调用 | 用途 |
|---|---|
| `Ui::debug_event("note", text)` | 向环形缓冲区追加一条应用备注。 |
| `Ui::clear_debug_log()` | 清空环形缓冲区。 |
| `Ui::hover_id()` | 指针下的控件。 |
| `Ui::installed_layout()` | 已应用的布局记录（`AppliedLayout`）。 |
| `snapshot.widget_types` | 已注册的创建类型，按注册顺序。 |
| `snapshot.has_commands` | 注册表是否携带命令表。 |

`PaintStats::per_widget` 按累计微秒数排序，最长的在前，因此
昂贵的控件是第一行。

### 5. 读取一份被拒绝的布局

`apply_layout` 和 `apply_compiled` 返回 `Result<LayoutReport, LayoutFailure>`。
错误意味着**没有应用任何内容**；先前的布局仍然活动。

```rust
use bamboo_native_ui::{Host, Size, WindowHandle};

impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let palette = *window.palette();
        match self.ui.apply_layout(&self.layout, &palette, size, window) {
            Ok(report) => {
                if !report.warnings.is_empty() {
                    eprintln!("layout warnings: {:?}", report.warnings);
                }
            }
            Err(failure) => {
                eprintln!("layout failed: {failure}");
                for diagnostic in &failure.diagnostics {
                    eprintln!(
                        "{}:{} {} {}",
                        diagnostic.line, diagnostic.column, diagnostic.code, diagnostic.message
                    );
                }
            }
        }
        window.invalidate_all();
    }
}
```

每个 `LayoutDiagnostic` 都有 `severity`（`Error`/`Warning`）、稳定的 `code`、
从 1 开始的 `line` 和 `column`、尽力而为的 `path`（`layout/wide/save`）以及
`message`。`LayoutFailure::first()` 返回第一个错误（或当只有警告时
返回第一条诊断）。常见代码列在
[XML 元素](../reference/xml-elements.md)中；修复通常如下：

| 代码 | 修复 |
|---|---|
| `xml.geom.underdetermined` | 给出恰好两个横向和两个纵向值。 |
| `xml.geom.overdetermined` | 移除 `x`/`right`/`w` 之一（或对应的纵向量）。 |
| `xml.layout.base` | 保留一个带 `min-width="0" min-height="0"` 的变体。 |
| `xml.id.unknown-widget` | 先在 Rust 中创建该控件，或添加 `type` 和一个注册表。 |
| `xml.attr.value` | 在报告的行检查 bool/数字/角色/光标/颜色拼写。 |
| `xml.content.unsupported` | 该控件不发布该内容能力（警告）。 |
| `xml.skin.unknown` | 该控件未声明该局部属性或子元素。 |

### 6. 检查 UI Automation

`--a11y-dump` 是内置检查。它验证 provider 能响应一个
真实客户端，因此它能捕获空树、缺失的模式、错误的
角色，或从未到达 UIA 的名称。

对于交互式检查，请使用 Windows SDK 工具（Inspect、
Accessibility Insights for Windows）针对正在运行的 showcase 或你自己的
应用。框架自己的 provider 是 `#[cfg(windows)]`；可移植的
模型是 `bamboo_native_ui::a11y`。

两个宿主侧的失误会使树为空：

- 未实现 `Host::a11y_nodes`（默认返回空向量）；
- 未实现 `Host::a11y_action`，因此屏幕阅读器可以读取树，
  但无法调用或切换任何内容。

`WindowHandle::a11y_refresh()` 在页面重建后重建快照；
`a11y_focus(Some(id))` 宣告焦点变化。

### 7. 使用应用日志

该 crate 依赖 `tracing`，并发出少量 `tracing::warn!` 事件（例如
一个原生工具提示窗口失败）。框架不安装任何订阅者，
因此如果你想要这些事件，请在应用中安装一个。库自己的
调试输出（`BAMBOO_UI_DEBUG`、绘制日志）使用 stderr，且无需
订阅者即可工作。

对于应用层诊断，当 `debug-form` 开启时优先使用 `Ui::debug_event`：
这些条目会与布局应用和警告落入同一个环形缓冲区，
调试菜单可以将它们一起显示。

## 常见错误

- **`--a11y-dump` 以非零退出** —— 探针失败，而非窗口。阅读
  打印的错误；缺失 `Host::a11y_nodes` 是常见原因。
- **没有 `BAMBOO_UI_DEBUG` 输出** —— 该变量必须在创建窗口的进程中
  设置；在 IDE 的测试运行器中设置它可能无法到达
  构建出的二进制文件。
- **快照为空** —— `debug-form` 未启用，或应用
  在空的 `Ui` 上调用 `debug_snapshot`。
- **布局编辑被忽略** —— `LayoutDocument::parse` 成功但
  generation 没有改变（只改了注释/格式），或应用失败而
  旧文档仍然安装。
- **绘制日志每帧都命名同一个控件** —— 它每帧都是脏的；
  找出在 `timer` 中运行的 setter 或失效调用。
- **渲染器回退到 GDI** —— 阅读启动时的 `BAMBOO_UI_DEBUG` 行；
  驱动或 Direct2D 目标无法创建。

## 另请参阅

- [性能](performance.md)
- [测试](testing.md)
- [辅助功能](accessibility.md)
- [Host 与窗口](../api/host-and-window.md)
- [XML 布局编写](xml-layout-authoring.md)
- [XML 元素](../reference/xml-elements.md)
