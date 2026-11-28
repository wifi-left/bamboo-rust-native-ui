# 调色板 token

> `.buixml` 文档中 `background=` 接受的 31 个颜色名称、`.buitheme`
> 文档中的 `<color name>` 以及 `set_palette_token` 接受的名称。

## token 表

token 名称是 kebab-case。它们由 `bamboo-native-ui-layout-core` 中的
`PALETTE_TOKENS` 定义，并与 `bamboo-native_ui::theme` 中 `Palette` 结构体的
字段一一对应。

| Token | `Palette` 字段 | 分组 | 强调色派生 |
|---|---|---|---|
| `background` | `background` | 表面 | |
| `sidebar` | `sidebar` | 表面 | |
| `player` | `player` | 表面 | |
| `cover` | `cover` | 表面 | |
| `card` | `card` | 表面 | |
| `panel` | `panel` | 表面 | |
| `search-bg` | `search_bg` | 表面 | |
| `nav-active` | `nav_active` | 导航与行 | 是 |
| `nav-hover` | `nav_hover` | 导航与行 | |
| `nav-pressed` | `nav_pressed` | 导航与行 | 是 |
| `row-hover` | `row_hover` | 导航与行 | |
| `row-pressed` | `row_pressed` | 导航与行 | 是 |
| `row-selected` | `row_selected` | 导航与行 | 是 |
| `text` | `text` | 文本 | |
| `muted` | `muted` | 文本 | |
| `accent-text` | `accent_text` | 文本 | 是 |
| `text-disabled` | `text_disabled` | 文本 | |
| `accent` | `accent` | 强调 | 输入 |
| `play-button` | `play_button` | 强调 | 是 |
| `play-hover` | `play_hover` | 强调 | 是 |
| `button-bg` | `button_bg` | 按钮 | |
| `button-hover` | `button_hover` | 按钮 | |
| `button-pressed` | `button_pressed` | 按钮 | 是 |
| `button-active` | `button_active` | 按钮 | 是 |
| `button-active-hover` | `button_active_hover` | 按钮 | 是 |
| `button-disabled` | `button_disabled` | 按钮 | |
| `divider` | `divider` | 边框与窗口装饰 | |
| `explicit-border` | `explicit_border` | 边框与窗口装饰 | |
| `scroll-track` | `scroll_track` | 边框与窗口装饰 | |
| `scroll-thumb` | `scroll_thumb` | 边框与窗口装饰 | |
| `scroll-thumb-hover` | `scroll_thumb_hover` | 边框与窗口装饰 | |

“强调色派生”意味着 `Palette::new` 将强调色混入基础表面
颜色。确切的混合取决于浅色/深色，因此主题只有在想要偏离
强调色时才应覆盖这些项。`accent` 本身是
`ThemeSettings::accent` 输入。

`Palette::dark: bool` 不是颜色，也没有 token；它跟随
`.buitheme` 的 `base` 或传给 `Palette::new` 的 `bool`。

## 强调色预设

`ACCENT_PRESETS` 和 `ACCENT_NAMES` 共享此顺序。第一个条目是
默认强调色。

| 索引 | `ACCENT_NAMES` | RGB | 十六进制 |
|---|---|---|---|
| 0 | 竹绿 | `[127, 211, 168]` | `#7fd3a8` |
| 1 | 海蓝 | `[104, 140, 255]` | `#688cff` |
| 2 | 紫罗兰 | `[167, 139, 250]` | `#a78bfa` |
| 3 | 珊瑚 | `[251, 146, 120]` | `#fb9278` |
| 4 | 琥珀 | `[245, 190, 90]` | `#f5be5a` |
| 5 | 青碧 | `[120, 200, 210]` | `#78c8d2` |

`ACCENT_PRESETS` 是 `[(&str, [u8; 3]); 6]`；这些标签是预设自带语言中的
显示字符串，框架不会本地化它们。

## 这些名称在哪里被接受

| 上下文 | 形式 | 备注 |
|---|---|---|
| 布局表面 | `<widget background="accent"/>` | 一个 token、一个颜色字面量，或 `none`。 |
| 布局局部颜色 | `<widget tint="accent"/>`、`color="text"` | 任何局部属性具有 `SkinValueKind::Color` 的控件。 |
| 主题文档 | `<color name="accent-text" value="#7aa2f7"/>` | token 是 `name`；值必须是字面量，而不是另一个 token。 |
| Rust | `set_palette_token(&mut palette, "nav-active", color)` | 未知名称返回 `false`。 |
| Rust | `palette.nav_active` | 直接访问 `Palette` 的字段。 |

颜色字面量在各处使用同一套语法：`#rgb`、`#rgba`、`#rrggbb`、
`#rrggbbaa`、`r,g,b`、`r,g,b,a`、`rgb(r,g,b)`、`rgba(r,g,b,a)`，或
带空格的函数形式。表面填充会丢弃 alpha；支持
透明度的控件读取 `SkinValue::as_rgba()`。

## `.buitheme` 根

| 属性 | 必需 | 接受的值 |
|---|---|---|
| `version` | 是 | `1` |
| `base` | 是 | `light`、`dark` |
| `name` | 否 | 任意文本；在规范模型中保持去除首尾空白。 |
| `accent` | 否 | 一个颜色字面量；在派生之前馈送给 `Palette::new`。 |

一个 `<color>` 子元素同时需要 `name`（上面的一个 token）和 `value`（一个字面量
颜色）。重复项和未知名称是错误。带有任何错误的文件不改变
任何内容。

## `.buitheme` 诊断

| 代码 | 严重程度 | 原因 |
|---|---|---|
| `theme.syntax` | 错误 | XML 格式错误或文件超过 1 MiB。 |
| `theme.version` | 错误 | `version` 缺失或不受支持。 |
| `theme.structure` | 错误 | 嵌套错误、杂散文本、DTD、缺少根。 |
| `theme.unknown-element` | 错误 | 除 `<theme>`/`<color>` 之外的元素。 |
| `theme.unknown-attribute` | 错误 | schema 之外的属性。 |
| `theme.missing-attribute` | 错误 | 缺少必需属性。 |
| `theme.base.value` | 错误 | `base` 不是 `light`/`dark`。 |
| `theme.color.value` | 错误 | 某个颜色值不是有效的字面量。 |
| `theme.color.unknown` | 错误 | `<color name>` 不是调色板 token。 |
| `theme.color.duplicate` | 错误 | 同一个 token 被声明两次。 |

每份文档最多接受 512 个 `<color>` 覆盖和 100 条诊断。

## 另请参阅

- [主题与调色板 API](../api/theme-and-palette.md)
- [主题指南](../guides/theming.md)
- [XML 元素](xml-elements.md)
- [几何与颜色](../api/geometry-and-color.md)
