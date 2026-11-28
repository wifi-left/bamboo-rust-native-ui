# 特性开关

> 选择框架链接什么：Windows 后端、运行时 XML
> 读取器、编译布局模式，或调试 API。

## 五个特性

所有特性都在 `crates/native-ui/Cargo.toml` 中声明。`gdi` 是唯一的
默认特性。

| 特性 | 默认 | 启用的内容 | 额外依赖 |
|---|---|---|---|
| `gdi` | 是 | Windows GDI 后端和 Direct2D 渲染器选项（`Renderer::D2dSoftware`、`D2dHardware`）。关闭时公共 API 相同，但运行中的窗口需要它。 | `windows`、`windows-core`（仅 Windows） |
| `layout-xml` | 否 | 运行时 `.buixml` 读取器（`LayoutDocument::parse`）和应用器（`Ui::apply_layout`），以及应用器中的控件皮肤协议。 | 带 `parse` 的 `layout-core`（`quick-xml`） |
| `theme-xml` | 否 | `.buitheme` 读取器（`ThemeDocument::parse`）、`ThemeDocumentExt::palette` 和 `set_palette_token`。 | 带 `parse` 的 `layout-core` |
| `layout-compiled` | 否 | 对生成 Rust 的运行时支持：`layout_xml::compiled::CompiledLayout`、`Ui::apply_compiled`。无 XML 解析器。 | 无 |
| `debug-form` | 否 | `Ui::debug_snapshot`、`DebugSnapshot`、`PaintStats` 和诊断环形缓冲区。 | 无直接依赖；**隐含 `layout-xml`** |

控件声明的皮肤类型（`SkinAttrDecl`、`SkinValue`、`SkinElement`）
以及 `Widget` 皮肤方法是**无条件**编译的。第三方
控件无需任何特性开关即可发布其词汇；只有读取一份
文档才需要某个布局特性。

## 能力矩阵

| 能力 | 默认 | `layout-xml` | `theme-xml` | `layout-compiled` | `debug-form` |
|---|---|---|---|---|---|
| 控件、容器、绘制器、GDI 窗口 | 是 | 是 | 是 | 是 | 是 |
| 运行时解析 `.buixml` | — | 是 | — | — | 是 |
| `Ui::apply_layout` | — | 是 | — | — | 是 |
| `.buixml` 文件的热重载 | — | 是 | — | — | 是 |
| 解析 `.buitheme` | — | — | 是 | — | — |
| `Ui::apply_compiled` | — | — | — | 是 | — |
| `Ui::debug_snapshot` | — | — | — | — | 是 |
| 链接 XML 解析器（`quick-xml`） | 否 | 是 | 是 | 否 | 是 |

`layout-xml` 和 `layout-compiled` 相互独立，可以一起
启用；showcase 就是这样做的，以便一次构建对两种模式进行类型检查。同一份
文档无论哪种方式都会产生相同的矩形、属性和内容。

`theme-xml` 仅启用 `layout-core/parse`。它不会打开布局
读取器，而只含主题的构建仍可应用 `layout-compiled` 编译出的布局。

## 启用特性

运行时 XML，开发：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

仅主题文档：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["theme-xml"] }
```

编译布局，发布：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-compiled", "theme-xml"] }
```

生成的模块由
`bamboo-native-ui-layout-codegen`（CLI 或构建脚本）或
`bamboo-native-ui-layout-macros` 的 `layout_xml!` 过程宏生成。参见
[06 — 编译期 XML](../quickstart/06-xml-compiled.md)。

调试 API（开发构建；它会引入解析器）：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["debug-form"] }
```

可移植库（非 Windows 目标、控件测试、工具）：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", default-features = false }
```

`platform` 模块是 `#[cfg(windows)]` 且会导入 GDI 后端，因此
**Windows** 构建需要 `gdi`；禁用默认特性适用于非 Windows
目标（一个编译可移植层并运行控件测试的 Linux CI runner）。
在 Windows 上，启用 `gdi` 时仍可使用可移植层。

## 构建与验证

```powershell
cargo build                                       # 默认：仅 gdi，0 警告
cargo build -p bamboo-native-ui --features layout-xml
cargo build -p bamboo-native-ui --features theme-xml
cargo build -p bamboo-native-ui --features layout-compiled
cargo build -p bamboo-native-ui --features debug-form
cargo build -p bamboo-native-ui --features layout-xml,layout-compiled
cargo test  -p bamboo-native-ui --features layout-xml
cargo test  -p bamboo-native-ui --features theme-xml
cargo test  -p bamboo-native-ui --features layout-compiled
cargo test  -p bamboo-native-ui --features debug-form
cargo test  -p bamboo-native-ui-layout-core --features parse
cargo test  -p bamboo-native-ui-layout-codegen
cargo test  -p bamboo-native-ui-layout-macros
```

showcase 的 `m7` 特性同时启用全部四个可选特性，因此在工作区层面
一次 `cargo test`/`cargo build` 就能对应用可能使用的
组合进行类型检查：

```powershell
cargo run -p bamboo-native-ui-showcase -- --self-test
cargo run -p bamboo-native-ui-showcase --no-default-features -- --self-test
```

## 权衡

| 如果你…… | 选择 | 因为 |
|---|---|---|
| 在运行中的应用里迭代布局 | `layout-xml` | 编辑并重新加载，无需重建。 |
| 发布不带 XML 解析器的版本 | `layout-compiled` | 二进制不含解析器，也不含表达式解释器。 |
| 只使用 `.buitheme` 文件 | `theme-xml` | 它不会引入布局读取器。 |
| 在开发构建中诊断有效状态 | `debug-form` | 快照和绘制统计；不要让它进入发布版。 |
| 构建控件库或工具 | 默认 | 无 XML 代码；可移植类型保持平台中立。 |
| 在一个二进制中支持两种 XML 模式 | `layout-xml,layout-compiled` | 它们共享应用器。 |

## 常见错误

- **`LayoutDocument` 无法解析** —— `layout-xml` 特性未启用。
- **`ThemeDocument` 无法解析** —— `theme-xml` 特性未启用；它
  与 `layout-xml` 相互独立。
- **`apply_compiled` 无法解析** —— `layout-compiled` 特性未启用，
  或生成的模块来自较旧版本的框架。
- **发布版仍链接 `quick-xml`** —— `debug-form` 隐含 `layout-xml`。
  为发布版禁用 `debug-form`。
- **`cargo build` 失败并报 `cannot find gdi in backend`** —— `default-features
  = false` 在 Windows 构建上移除了 `gdi`；在那里保留默认特性。
- **编辑 `.buixml` 不会重建 `layout_xml!` 模块** —— 稳定的过程
  宏无法注册文件依赖；请用 CLI 重新生成，或 touch
  包含它的源文件。

## 另请参阅

- [Crate 与特性](../overview/architecture.md)
- [06 — 编译期 XML](../quickstart/06-xml-compiled.md)
- [XML 布局编写](xml-layout-authoring.md)
- [主题](theming.md)
- [测试](testing.md)
- [调试](debugging.md)
