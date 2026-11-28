# 诊断与调试

> 稳定的诊断代码、应用报告/失败这一对类型，以及只读的 `debug-form` 快照。

## 概述

诊断分为两层。解析器和应用器产生带有稳定字符串代码和源位置的 `LayoutDiagnostic` 值；
一次应用要么返回 `LayoutReport`（已应用，可能带有警告），要么返回 `LayoutFailure`（被拒绝，
未做任何更改）。`debug-form` 特性添加对活动容器的只读快照——布局、控件、损伤、绘制耗时以及
一个有界日志环——并且设置 `BAMBOO_UI_DEBUG` 时，平台层会将内部信息记录到 stderr。

错误会拒绝整个文档；警告会被报告，其余部分仍照常应用。这种划分正是原子性承诺：一次糟糕的
热重载会让之前的布局保持在屏幕上，并交回其原因。

| 层 | 类型 | 特性 |
|---|---|---|
| 解析 / 应用 | `LayoutSeverity`、`LayoutDiagnostic`、`Diagnostics` | `layout-xml`（模型类型也在 core 中） |
| 应用结果 | `LayoutReport`、`LayoutFailure`、`AppliedLayout` | `layout-xml` 或 `layout-compiled` |
| 调试快照 | `DebugSnapshot` 系列、`Ui::debug_snapshot` | `debug-form`（隐含 `layout-xml`） |
| 进程指标 | `private_bytes` | 仅 Windows，`#[cfg(windows)]` |

## 关键类型

### `LayoutDiagnostic` 与 `LayoutSeverity`

```rust
pub enum LayoutSeverity { Error, Warning }

pub struct LayoutDiagnostic {
    pub severity: LayoutSeverity,
    pub code: &'static str,   // 稳定、可测试（`xml.geom.negative` 等）
    pub line: usize,          // 从 1 开始，指向该元素
    pub column: usize,
    pub path: String,         // `layout/wide/action`，尽力而为
    pub message: String,
}

impl LayoutDiagnostic {
    pub fn error(code: &'static str, line: usize, column: usize,
                 path: impl Into<String>, message: impl Into<String>) -> Self;
    pub fn warning(code: &'static str, line: usize, column: usize,
                   path: impl Into<String>, message: impl Into<String>) -> Self;
    pub fn is_error(&self) -> bool;
}
```

`Display` 打印 `line:column: severity[code] (path): message`，因此可以直接记录或
显示一条诊断。

### `Diagnostics` 接收器

```rust
pub struct Diagnostics { /* private */ }

impl Diagnostics {
    pub fn new(limit: usize) -> Self;
    pub fn push(&mut self, diagnostic: LayoutDiagnostic);
    pub fn error(&mut self, code: &'static str, line: usize, column: usize,
                 path: impl Into<String>, message: impl Into<String>);
    pub fn warning(&mut self, code: &'static str, line: usize, column: usize,
                   path: impl Into<String>, message: impl Into<String>);
    pub fn has_errors(&self) -> bool;
    pub fn len(&self) -> usize;
    pub fn into_vec(self) -> Vec<LayoutDiagnostic>;
}
```

解析器和应用器各自最多收集 100 条诊断；后续的会被丢弃，而不是无界增长。

### `LayoutReport` 与 `LayoutFailure`

```rust
pub struct LayoutReport {
    pub generation: u64,
    pub variant_id: String,
    pub applied_entries: usize,
    pub content_writes: usize,
    pub skin_writes: usize,
    pub created: usize,
    pub warnings: Vec<LayoutDiagnostic>,
}

impl LayoutReport {
    pub fn summary(&self) -> String;   // 一行，适合状态栏
}

pub struct LayoutFailure {
    pub diagnostics: Vec<LayoutDiagnostic>,
}

impl LayoutFailure {
    pub fn first(&self) -> Option<&LayoutDiagnostic>;
}
```

`summary()` 命名变体、条目/内容/皮肤计数以及第一条警告（或无警告标记）。`LayoutFailure`
实现 `Display` 和 `std::error::Error`；`first()` 返回第一条错误，只有警告时回退到第一条
诊断。`diagnostics` 按错误在前排序。

`Ui::installed_layout()` 返回一个 `AppliedLayout` 供检查：generation、client、变体 id、
已解析条目、认领 id、已创建的控件以及警告。

### 调试快照（`debug-form`）

```rust
impl<M> Ui<M> {
    #[cfg(feature = "debug-form")]
    pub fn debug_snapshot(&self) -> DebugSnapshot;
    #[cfg(feature = "debug-form")]
    pub fn debug_event(&mut self, kind: &'static str, text: impl Into<String>);
    #[cfg(feature = "debug-form")]
    pub fn clear_debug_log(&mut self);
}

pub struct DebugSnapshot {
    pub layout: Option<DebugLayout>,
    pub widgets: Vec<DebugWidget>,
    pub widget_types: Vec<&'static str>,
    pub has_commands: bool,
    pub focus: Option<WidgetId>,
    pub hover: Option<WidgetId>,
    pub press: Option<WidgetId>,
    pub pending_damage: Vec<Rect>,
    pub last_damage: Vec<Rect>,
    pub paint: PaintStats,
    pub log: Vec<DebugEvent>,
}
```

该系列是自有数据，可以在树变化时安全保留：

- `DebugLayout` — `generation`、`variant`、`client`、`content_writes`、`skin_writes`、
  `entries: Vec<DebugEntry>`、`claims: Vec<DebugClaim>`、`warnings: Vec<String>`、
  `scrolls: Vec<DebugScroll>`。
- `DebugEntry` — `id`、`rect`、`space`（`EntrySpace`）。
- `DebugClaim` — `id`、`baseline` 矩形以及已认领的字段名（`visible`、`enabled`、
  `focusable`、`focus-order`、`cursor`、`props`、`corners`、`background`）。
- `DebugWidget` — 生效的 `visible`/`enabled`/`focusable`/`read_only`、交互标志、
  已解析叙述的 `a11y: Option<A11yInfo>`、`skin_attrs` 和 `skin_children` 词汇表。
- `PaintStats` — 累计帧数/`widget_paints`、上一帧的 `last_frame_micros`，以及按开销排序的
  `per_widget`。
- `DebugEvent` — `sequence`、`kind`（`layout`、`warning`、`error`、`note`）以及 `text`；
  环形缓冲保留最近的 128 条条目。

库不绘制任何检查器；应用可以自行呈现这些值（showcase 会把摘要写入对话框）。

### 环境变量与进程指标

- `BAMBOO_UI_DEBUG`（任意值）使平台层将内部决策记录到 stderr：重绘/可见性变化、渲染器
  启动失败与 GDI 回退、chrome 解析。
- `BAMBOO_UI_PAINT_LOG` 使 `Ui::paint` 打印每次绘制的诊断（脏区数、控件数、微秒数；慢于
  1.5 ms 的控件及其矩形）。
- `bamboo_native_ui::diagnostics::private_bytes() -> Option<u64>` 在 Windows 上返回进程的
  私有提交字节数（`PagefileUsage`），在其他平台返回 `None`。`diagnostics` 是一个
  `#[cfg(windows)]` 模块。

## 示例

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;
use bamboo_native_ui::{Palette, Size, Ui, UiHost};

fn apply<M: 'static>(
    ui: &mut Ui<M>, text: &str, palette: &Palette,
    client: Size, sink: &mut impl UiHost,
) -> String {
    let document = LayoutDocument::parse(text).expect("parse before applying");
    match ui.apply_layout(&document, palette, client, sink) {
        Ok(report) => {
            if let Some(warning) = report.warnings.first() {
                eprintln!("layout warning: {warning}");
            }
            report.summary()
        }
        Err(failure) => {
            // 未做任何更改：之前的布局仍处于已安装状态。
            let first = failure.first().map(ToString::to_string).unwrap_or_default();
            format!("layout rejected: {first}")
        }
    }
}
```

读取快照：

```rust
#[cfg(feature = "debug-form")]
fn dump<M>(ui: &bamboo_native_ui::Ui<M>) {
    let snapshot = ui.debug_snapshot();
    if let Some(layout) = &snapshot.layout {
        for entry in &layout.entries {
            let _ = (entry.id, entry.rect, entry.space);
        }
        for claim in &layout.claims {
            let _ = (claim.id, claim.baseline, &claim.fields);
        }
    }
    for widget in &snapshot.widgets {
        let _ = (widget.id, widget.visible, widget.a11y.as_ref());
    }
    if let Some(event) = snapshot.log.last() {
        let _ = (event.sequence, event.kind, &event.text);
    }
}
```

## 说明与限制

- 特性门控：诊断类型需要 `layout-xml`（模型/诊断类型也在 `bamboo-native-ui-layout-core`
  中存在）；`LayoutReport`/`LayoutFailure` 在任一种布局特性下都存在；`debug` 和
  `Ui::debug_snapshot` 需要 `debug-form`，它隐含 `layout-xml`。
- 一次失败的应用不会更改任何内容，包括已创建的控件和认领。
- 代表性代码：`xml.syntax`、`xml.structure`、`xml.unknown-attribute`、`xml.attr.value`、
  `xml.const.*`、`xml.expr.*`、`xml.geom.negative`、`xml.geom.overdetermined`、
  `xml.geom.underdetermined`、`xml.id.unknown-widget`、`xml.content.conflict`、
  `xml.skin.unknown`、`xml.skin.value`、`xml.rust.unsupported`、`xml.type.unknown`、
  `xml.command.missing`。警告：`xml.style.unsupported`、`xml.content.unsupported`、
  `xml.content.session-open`、`xml.value.clamped`、`xml.value.zero-size`。
- 主题文档使用自己的 `theme.*` 代码；参见
  [theme-documents.md](theme-documents.md)。
- 调试环形缓冲为每次应用记录一个 `layout` 事件（`variant=…`）、被拒绝应用的第一条错误，
  以及传给 `debug_event` 的应用备注。
- `private_bytes` 是 Windows 进程指标；它不包含共享 DLL。

## 另见

- [layout-documents.md](layout-documents.md) — 模型与解析错误
- [layout-codegen.md](layout-codegen.md) — 相同的报告/失败类型
- [host-and-window.md](host-and-window.md) — 宿主回调与绘制
- [Debugging guide](../guides/debugging.md)
- [05-xml-dynamic.md](../quickstart/05-xml-dynamic.md) — 加载与重载
