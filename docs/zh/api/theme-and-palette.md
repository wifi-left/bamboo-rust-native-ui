# 主题与调色板

> 运行时主题设置、解析后的调色板、强调色预设、列表密度和具名圆角半径。

## 概述

`bamboo_native_ui::theme` 掌管应用在运行时重设 UI 外观所需改动的一切：
`ThemeSettings`（输入）、`Palette`（解析后的颜色）和
`ListDensity`。它还重导出 layout-core 的 `radius` 模块，使各控件共享同一套具名圆角半径。

调色板不是某个控件拥有的单例：窗口根据
`ThemeSettings` 加上解析后的浅色/深色决策构建一个 `Palette`，将其安装进
`Painter`，每个控件都用自己收到的调色板绘制
（`WidgetPaint::palette`）。`WidgetProps::palette` 可为单个控件覆盖它。

`.buitheme` 文档格式受特性开关控制：启用
`feature = "theme-xml"` 时，`theme_xml` 模块会重导出 `ThemeDocument`
读取器，并提供 `ThemeDocumentExt::palette(&settings)`——先 `Palette::new`
再应用文档中的覆盖——以及 `set_palette_token`。布局
特性与之相互独立。

## 关键类型

### `ThemeSettings` 与 `ListDensity`

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeSettings {
    pub accent: [u8; 3],
    pub font_family: Option<String>,
    /// UI 基准尺寸，单位为 DIP；13.5 是参考值。
    pub font_size: f32,
    pub list_density: ListDensity,
}

impl Default for ThemeSettings { /* 见表格 */ }

impl ThemeSettings {
    pub fn font_scale(&self) -> f32;   // (font_size / 13.5).clamp(0.85, 1.4)
    pub fn row_height(&self) -> f32;   // 按密度为 34 / 40 / 48
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListDensity {
    Compact,
    #[default]
    Comfortable,
    Spacious,
}
```

| 字段 | 默认值 | 含义 |
|---|---|---|
| `accent` | `ACCENT_PRESETS[0].1` = `[127, 211, 168]` | RGB 强调色；由强调色派生的调色板字段跟随它。 |
| `font_family` | `Some("Microsoft YaHei UI")` | UI 字体族；空字符串回退到主题字体。 |
| `font_size` | `13.5` | UI 基准尺寸，单位为 DIP。 |
| `list_density` | `ListDensity::Comfortable` | `Compact` 34.0、`Comfortable` 40.0、`Spacious` 48.0 DIP 行高。 |

### `Palette`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub dark: bool,
    pub background: Color, pub sidebar: Color, pub player: Color, pub cover: Color,
    pub nav_active: Color, pub accent: Color, pub text: Color, pub muted: Color,
    pub divider: Color, pub row_selected: Color, pub scroll_track: Color,
    pub scroll_thumb: Color, pub accent_text: Color, pub search_bg: Color,
    pub button_bg: Color, pub button_hover: Color, pub button_pressed: Color,
    pub button_active: Color, pub button_active_hover: Color,
    pub explicit_border: Color, pub nav_hover: Color, pub row_hover: Color,
    pub nav_pressed: Color, pub row_pressed: Color, pub scroll_thumb_hover: Color,
    pub card: Color, pub panel: Color, pub button_disabled: Color,
    pub text_disabled: Color, pub play_button: Color, pub play_hover: Color,
}

impl Palette {
    /// `dark` 是解析后的浅色/深色决策（"跟随系统"由调用方
    /// 解析）。
    pub fn new(settings: &ThemeSettings, dark: bool) -> Self;
}
```

| 分组 | 字段 |
|---|---|
| 表面 | `background`、`sidebar`、`player`、`cover`、`card`、`panel`、`search_bg` |
| 导航与行 | `nav_active`、`nav_hover`、`nav_pressed`、`row_hover`、`row_pressed`、`row_selected` |
| 文本 | `text`、`muted`、`accent_text`、`text_disabled` |
| 强调色 | `accent`、`play_button`、`play_hover` |
| 按钮 | `button_bg`、`button_hover`、`button_pressed`、`button_active`、`button_active_hover`、`button_disabled` |
| 边框与镶边 | `divider`、`explicit_border`、`scroll_track`、`scroll_thumb`、`scroll_thumb_hover` |
| 模式 | `dark` |

由强调色派生的字段（`nav_active`、`row_selected`、`button_pressed`、
`button_active*`、`nav_pressed`、`row_pressed`、`play_*`、`accent_text`）在
构造时由强调色混合而成，因此根据新的设置重建调色板也会一致地重新设定悬停和按下状态。

### 强调色预设与圆角半径

```rust
pub const ACCENT_PRESETS: [(&str, [u8; 3]); 6] = [
    ("竹绿", [127, 211, 168]), ("海蓝", [104, 140, 255]),
    ("紫罗兰", [167, 139, 250]), ("珊瑚", [251, 146, 120]),
    ("琥珀", [245, 190, 90]), ("青碧", [120, 200, 210]),
];
pub const ACCENT_NAMES: [&str; 6] = ["竹绿", "海蓝", "紫罗兰", "珊瑚", "琥珀", "青碧"];

// bamboo_native_ui::theme::radius
pub const SMALL: f32 = 4.0;
pub const MEDIUM: f32 = 6.0;
pub const LARGE: f32 = 10.0;
pub fn pill(height: f32) -> f32;   // height.max(0.0) / 2.0
```

第一个预设是历史悠久的 Bamboo 绿，也是默认强调色。
`ACCENT_NAMES` 是按相同顺序排列的标签数组，供分段选择器使用。这些
半径常量是布局 schema 的 `radius` token 所解析到的值；`pill` 是一个辅助函数，不是 token。

## 示例

```rust
use bamboo_native_ui::theme::{radius, ACCENT_PRESETS, Palette, ThemeSettings};

fn install_theme() -> Palette {
    let settings = ThemeSettings {
        accent: ACCENT_PRESETS[1].1,
        font_size: 15.0,
        ..ThemeSettings::default()
    };
    assert_eq!(settings.row_height(), 40.0);
    assert!(settings.font_scale() > 1.0);
    let _corner = radius::MEDIUM;

    let palette = Palette::new(&settings, true);
    assert!(palette.dark);
    assert_eq!(palette.accent, bamboo_native_ui::Color::rgb(settings.accent));
    palette
}
```

`Palette::new` 只推导颜色；字体和文本样式来自
`Painter::set_theme`，它返回文本样式是否发生变化，以便
后端丢弃缓存的字体（见
[绘制与 Canvas](./painting-and-canvas.md)）。

## 说明与限制

- `Palette` 是 `Copy`；按值传递。控件读取 `WidgetPaint::palette`，
  而 `WidgetProps::palette` 可按控件覆盖它。
- 浅色/深色决策由调用方负责：`ThemeSettings` 不读取
  系统主题，因此"跟随系统"要在 `Palette::new` 之前解析。
- `ACCENT_NAMES` 是预设随附语言（中文）下的显示字符串；
  对其他本地化的应用，请使用你自己的标签。
- `font_scale` 限制在 `0.85..=1.4`，因此非常小或非常大的 `font_size`
  值不会线性缩放。
- `.buitheme` 解析及其调色板映射需要 `feature = "theme-xml"`；
  没有它时 `Palette::new` 仍可工作，也不会链接任何 XML 读取器。
- 半径常量是 DIP，并且绘制时还会额外限制到较短边的一半，
  因此过大的常量会变成胶囊，而不是报错。

## 另见

- [几何与颜色](./geometry-and-color.md)
- [绘制与 Canvas](./painting-and-canvas.md)
- [Ui 容器](./ui-container.md)
- [控件 trait](./widget-trait.md)
- [Button](../widgets/controls/button.md)
- [主题指南](../guides/theming.md)
