# 主题文档（`.buitheme`）

> 将整个调色板作为数据：`ThemeDocument` 读取器、其调色板推导以及令牌写入器。

## 概述

`.buitheme` 文件是 `.buixml` 的姊妹语法：一个根 `<theme>`，带有一个必需的浅色/深色
`base`、一个可选的 `accent`，以及任意数量的 `<color>` 覆盖。模型和读取器位于
`bamboo_native_ui_layout_core::theme`，无需 Win32，也无需 `Palette`。`bamboo-native-ui` 的
`theme-xml` 特性重新导出它们，并添加映射到 window crate 的 `Palette` 上的功能。

应用主题就是交换调色板并重绘。布局模式的颜色令牌（`background="accent"`、`card`、
`text` 等）随后会在下一次 `apply_layout` 时针对新调色板解析。推导过程恰好是
`Palette::new(settings, base)` 后接文档的覆盖，因此一个主题可以只改一个令牌，也可以改整张表。

`theme-xml` 特性仅启用 `layout-core/parse`；它独立于 `layout-xml` 和 `layout-compiled`。
一次构建可以在不加载布局的情况下加载主题。

## 关键类型

### `ThemeDocument`

```rust
pub struct ThemeDocument {
    pub version: u32,
    pub name: Option<String>,
    pub base: ThemeBase,
    pub accent: Option<Color>,
    pub colors: Vec<ThemeColor>,
    pub canonical: String,
    pub generation: u64,
}

impl ThemeDocument {
    pub fn parse(text: &str) -> Result<Self, Vec<LayoutDiagnostic>>; // 特性 "theme-xml"
    pub fn version(&self) -> u32;
    pub fn name(&self) -> Option<&str>;
    pub fn base(&self) -> ThemeBase;
    pub fn accent(&self) -> Option<Color>;
    pub fn colors(&self) -> &[ThemeColor];
    pub fn generation(&self) -> u64;
    pub fn to_xml(&self) -> String;
    pub fn color(&self, name: &str) -> Option<Color>;
}
```

- `version` 必须为 `1`；`name` 是人类可读的标签，在规范文本中保持修剪状态。
- `canonical` 是规范化后的文档；`generation` 是其 FNV-1a 哈希，因此同一模型两次保存的
  字节完全相同，哈希仅随内容变化。
- `colors()` 按声明顺序枚举覆盖；`color(name)` 按令牌查找其中一个。

### `ThemeBase`、`ThemeColor`、`ThemeDiagnostics`

```rust
pub enum ThemeBase { Light, Dark }

impl ThemeBase {
    pub fn is_dark(self) -> bool;
    pub fn name(self) -> &'static str;          // "light" / "dark"
    pub fn from_name(name: &str) -> Option<Self>;
}

pub struct ThemeColor {
    pub name: String,
    pub color: Color,
    pub line: usize,
    pub column: usize,
}

pub type ThemeDiagnostics = Vec<LayoutDiagnostic>;
```

`ThemeColor` 是一个 `<color name="…" value="…"/>` 覆盖。`ThemeDiagnostics` 是 `parse` 返回
的错误类型；其代码是稳定的，并以 `theme.*` 为前缀。

### 映射到 `Palette`

```rust
pub trait ThemeDocumentExt {
    fn palette(&self, settings: &ThemeSettings) -> Palette;
}

impl ThemeDocumentExt for ThemeDocument { /* … */ }

pub fn set_palette_token(palette: &mut Palette, name: &str, color: Color) -> bool;
```

`palette(&settings)` 克隆设置，文档存在 accent 时用其替换设置中的 accent，推导
`Palette::new(&settings, base.is_dark())`，然后按声明顺序应用每个 `<color>` 覆盖。
`set_palette_token` 写入一个 kebab-case 调色板字段，并返回该名称是否为令牌；该函数是全函数，
因此应用可以直接使用它。令牌名称与布局 `background` 属性接受的 `PALETTE_TOKENS` 表相同。

## `.buitheme` 语法

```text
theme              根；version="1" 与 base 必需；name 与 accent 可选
└── color*         name="<palette-token>" value="<color literal>"
```

- 属性区分大小写且为 kebab-case；元素顺序无关紧要。
- `base` 必需，且必须为 `light` 或 `dark`（`theme.base.value`）。文档不跟随应用当前的模式。
- `accent` 在推导**之前**供给 `Palette::new`，因此由 accent 推导出的令牌（`accent-text`、
  `row-selected` 等）会跟随它。随后显式的 `<color name="accent">` 再设置该单个字段。
- `value` 使用唯一的颜色语法：`#rgb`、`#rrggbb`、`#rgba`、`#rrggbbaa`、`rgb(...)`、
  `rgba(...)` 或十进制通道列表。Alpha 会被接受并丢弃——调色板颜色是不透明的。
- 覆盖是字面量，而非令牌引用：文档*定义*调色板。重复和未知的令牌名称是错误。

showcase 附带 `crates/showcase/themes/night.buitheme`：

```xml
<?xml version="1.0" encoding="utf-8"?>
<theme version="1" name="夜蓝" base="dark" accent="#7aa2f7">
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
  <color name="card" value="#22262e"/>
</theme>
```

## 示例

```rust
use bamboo_native_ui::theme::ThemeSettings;
use bamboo_native_ui::theme_xml::{ThemeDocument, ThemeDocumentExt};
use bamboo_native_ui::WindowHandle;

fn apply_theme(window: &mut WindowHandle<'_>, text: &str) -> Result<(), String> {
    let document = ThemeDocument::parse(text).map_err(|d| d[0].to_string())?;
    let settings = ThemeSettings::default();
    let palette = document.palette(&settings);
    window.set_theme(settings, palette.dark);
    window.set_palette(palette); // 安装该表并重绘一次
    Ok(())
}
```

`WindowHandle::set_palette` 将调色板存储到窗口的绘制器中，更新原生 chrome/菜单并使窗口失效；
`WindowHandle::palette()` 将其读回。随后使用该调色板应用布局：

```rust
ui.apply_layout(&layout, &palette, client, sink)?; // 颜色令牌在此解析
```

## 说明与限制

- 特性门控：`ThemeDocument::parse` 和 `theme_xml` 模块需要 `feature = "theme-xml"`。该模型
  也可通过 `bamboo_native_ui_layout_core::theme` 访问；其读取器需要该 crate 的 `parse`
  特性。
- 解析错误不会改变任何内容；安装调色板是应用自己的步骤。解析器将文件上限设为 1 MiB、
  512 种颜色和 100 条诊断。
- Base 优先于应用的模式：没有 `auto`。请显式使用浅色或深色文档。
- accent 推导先发生；覆盖后发生，因此 `<color name="accent">` 不会重新推导 `accent-text`。
  如果你希望两者都更改，请同时设置它们。
- 代码：`theme.syntax`、`theme.version`、`theme.structure`、`theme.unknown-element`、
  `theme.unknown-attribute`、`theme.missing-attribute`、`theme.base.value`、
  `theme.color.value`、`theme.color.unknown`、`theme.color.duplicate`。
- 带有 `WidgetProps::palette` 的控件无论窗口设置如何都会保留自己的调色板。

## 另见

- [theme-and-palette.md](theme-and-palette.md) — `ThemeSettings`、`Palette`、预设
- [layout-documents.md](layout-documents.md) — `background` 使用的调色板令牌
- [Theming guide](../guides/theming.md)
- [05-xml-dynamic.md](../quickstart/05-xml-dynamic.md) — 加载使用令牌的布局
