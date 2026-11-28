# 架构

> crate、运行时层次以及两种布局消费模式如何衔接在一起。

## crate 地图

该仓库是一个包含五个成员的 Cargo 工作区。

| crate | 包 | 作用 |
|---|---|---|
| `crates/native-ui` | `bamboo-native-ui` | 框架：控件、`Ui<M>` 容器、绘制器与后端 trait，以及 Windows 宿主（`WindowBuilder`、`WindowHandle`）。 |
| `crates/layout-core` | `bamboo-native-ui-layout-core` | 平台中立的 `.buixml` / `.buitheme` 模型、值类型、规范写入器和生成哈希。XML 读取器位于其 `parse` 特性之后。 |
| `crates/layout-codegen` | `bamboo-native-ui-layout-codegen` | 库加 CLI，将 `.buixml` 文档转换为普通 Rust 模块（一个 `CompiledLayout` 静态量和 `install` 函数）。 |
| `crates/layout-macros` | `bamboo-native-ui-layout-macros` | `layout_xml!` 过程宏：在编译期展开代码生成输出，无需单独的生成步骤。 |
| `crates/showcase` | `bamboo-native-ui-showcase` | 全控件演示二进制 `showcase`；完整 API 的参考接线。 |

依赖方向是单向的：
`native-ui` → `layout-core`；`layout-codegen` → `layout-core`；
`layout-macros` → `layout-codegen`。`layout-core` 不链接任何 UI 代码，因此它可以在构建脚本或工具中使用。

## 运行时层次

```
  WM_* messages, WM_PAINT
            │
  ┌─────────▼──────────────────────────┐
  │ WindowBuilder / WindowHandle       │  platform/window.rs  (Windows only)
  │ message loop, DPI, timers,         │  normalizes input, owns one back
  │ dirty-rect merge, back-buffer blit │  buffer and the native surfaces
  └───────┬──────────────────▲─────────┘
          │ Event,           │ paint(&mut PaintCtx)
          │ WindowHandle     │
  ┌───────▼──────────────────┴─────────┐
  │ Host  (your application)           │  host.rs
  │ computes rectangles, fills the page│  implements paint/event/resize
  └───────┬──────────────────▲─────────┘
          │ ui.pointer_*(),  │ ui.paint(ctx)
          │ ui.key(), ...    │
  ┌───────▼──────────────────┴─────────┐
  │ Ui<M>                              │  ui.rs
  │ Box<dyn Widget<M>>, hit-testing,   │  hover/press/focus, scroll views,
  │ damage merge, message queue        │  drag and drop, a11y tree
  └───────┬──────────────────▲─────────┘
          │ Widget::input    │ Widget::paint(ctx, state)
  ┌───────▼──────────────────┴─────────┐
  │ Painter + dyn Canvas               │  painter.rs, backend/mod.rs
  │ platform-neutral draw calls        │  text, fills, rounded rects, images
  └───────┬──────────────────▲─────────┘
          │                  │
  ┌───────▼──────────────────┴─────────┐
  │ GDI / Direct2D backend             │  #[cfg(windows)]  backend/gdi.rs,
  │ cached brushes, fonts, masks       │  backend/d2d.rs
  └────────────────────────────────────┘
```

宿主边界仅限 Windows：`crate::host`、`crate::platform` 以及 `windows` crate 依赖都在 `#[cfg(windows)]` 之后编译。`Painter`/`Canvas` 之上的所有内容都能在任何平台上编译，这正是控件测试可以在没有窗口的情况下运行的原因。

## 消息流程

1. 窗口过程接收到 `WM_*` 消息，宿主将其转换为规范化后的 `Event`（`PointerMove`、`PointerDown`、`Wheel`、`Key`、`Char`、`PointerKind`……）。应用永远不会看到原始消息。
2. 调用 `Host::event(&event, &mut WindowHandle)`。应用将指针/键盘事件转发给 `Ui` 方法，例如 `pointer_move`、`pointer_down_with`、`pointer_up_with`、`wheel_xy`、`key` 和 `char`。
3. `Ui` 执行命中测试，更新悬停/按下/焦点状态，并调用目标控件的 `Widget::input`。控件返回 `Option<M>`，即应用的消息类型。
4. 消息汇集到队列中。应用用 `ui.drain_messages()` 取出它们并更新自身状态。
5. 通过 `Ui` 的 setter（`set_label`、`set_progress`、`edit_widget`……）所做的程序化更改会将受影响的矩形记录为待处理损坏，而不是立即重绘。应用在回调末尾调用一次 `ui.flush_damage(window)`，因此一连串 setter 只耗费一次重绘请求。
6. 宿主合并失效矩形（最多 32 个独立片段）并使客户区失效。随后 `WM_PAINT` 运行一次绘制过程。

生命周期回调（`resize`、`dpi_changed`、`started`、`loaded`、`close_requested`）在同一线程上、在绘制过程之间运行。`resize` 是唯一可以重新计算布局的地方。

## 绘制流程

1. 宿主将累积的损坏合并为一个独立脏矩形列表（DIP），超过阈值后将其合并为一个边界框。
2. 它构建一个 `PaintCtx`，包含借用的脏列表、客户区 `Size`、DPI、编辑状态以及任何叠加标志。
3. `Host::paint` 在每个脏片段下填充页面背景，并调用 `ui.paint(ctx)`。
4. `Ui::paint` 按插入顺序遍历控件，跳过绘制边界不与任何脏矩形相交的控件，并调用 `Widget::paint`。
5. 控件通过 `Painter` 和 `Canvas` 绘制到后台缓冲中。
6. 宿主将每个脏片段从后台缓冲 blit 到窗口。片段之外的像素永远不会被触及，因此空闲控件不耗费任何代价。

没有测量过程：控件的矩形来自应用（或来自所应用的布局文档），永远不来自其内容。

## 特性门控

特性在 `crates/native-ui/Cargo.toml` 中声明。

| 特性 | 默认 | 启用的内容 | 额外依赖 |
|---|---|---|---|
| `gdi` | 是 | Windows GDI 后端（以及 Direct2D 渲染器选项）。关闭时公共 API 相同，但运行中的窗口需要它。 | `windows` |
| `layout-xml` | 否 | 运行时 `.buixml` 解析（`LayoutDocument::parse`）和应用器（`Ui::apply_layout`）。 | 带 `parse` 特性的 `layout-core`（`quick-xml`） |
| `layout-compiled` | 否 | 对生成的 Rust 的运行时支持：`layout_xml::compiled::CompiledLayout` 和 `Ui::apply_compiled`。不链接 XML 解析器。 | 无 |
| `theme-xml` | 否 | `.buitheme` 读取器，通过 `ThemeDocumentExt` 映射到 `Palette`。 | 带 `parse` 的 `layout-core` |
| `debug-form` | 否 | 只读调试快照（`Ui::debug_snapshot`、`DebugSnapshot`）和绘制统计（`PaintStats`）。 | 启用 `layout-xml` |

两个布局特性相互独立，可以同时启用；showcase 就是这样做的，因此一次构建就能对两种模式进行类型检查。

## XML 布局的定位

`.buixml` 主要是一种**皮肤**：文档为 Rust 侧已经创建的控件 id 分配矩形、属性覆盖、样式令牌和内容初始值。它不能删除、重排或重新挂接这些被寻址的控件。`type="..."` 条目也可以通过已安装的 `WidgetRegistry` *创建*控件（12 种标准类型；第三方类型从 Rust 注册）；创建的控件归文档所有，因此重新加载会移除新文档不再声明的控件。XML 永远不绑定事件处理器。
两种消费模式共享同一个 schema：

- **动态**（`feature = "layout-xml"`）。应用读取文件，调用 `LayoutDocument::parse`，然后调用 `ui.apply_layout(&document, &palette, size, window)`。这是开发中使用的热重载路径。
- **编译期**（`feature = "layout-compiled"`）。`bamboo-native-ui-layout-codegen` 或 `layout_xml!` 宏将同一文件转换为 Rust 模块。应用调用 `ui.apply_compiled(&module::LAYOUT, &palette, size, window)` 或生成的 `install` 辅助函数。运行时不再链接解析器。

两条路径运行相同的内部应用器（`Ui::install_prepared`），因此同一文档在两种方式下产生相同的矩形、属性和内容。共享模型位于 `layout-core`；应用器位于 `crates/native-ui/src/layout_xml/`。

主题文档（`.buitheme`，`feature = "theme-xml"`）是一个单独的读取器，它产生一个 `Palette`；布局的颜色令牌针对传给应用调用的调色板进行解析。

## 另见

- [设计原则](design-principles.md)
- [宿主与窗口 API](../api/host-and-window.md)
- [主题指南](../guides/theming.md)
- [快速上手](../quickstart/index.md)
- [动态 XML 布局](../quickstart/05-xml-dynamic.md)
