# 主题

> 在启动时和运行时更改强调色、浅色/深色模式、字体、列表密度和圆角半径，
> 然后将整个调色板移入一份 `.buitheme` 文档。

## 主题是什么

一个主题包含三个层次：

1. **`ThemeSettings`** —— 应用编辑的输入：强调色、字体系列、
   基础字号、列表密度。
2. **`Palette`** —— 控件绘制时使用的、已解析的颜色。`Palette::new`
   根据设置和一个浅色/深色决定派生每一个字段。
3. **`Painter`** —— 拥有已安装的调色板和文本样式。窗口安装它；
   控件通过 `WidgetPaint::palette` 读取它。

浅色/深色的决定由应用来做。`ThemeSettings` 没有模式字段：自行解析
“跟随系统”，并将得到的 `bool` 传给 `Palette::new` 或
`WindowBuilder::theme`。

## 步骤

### 1. 构建设置

```rust
use bamboo_native_ui::{ListDensity, ThemeSettings, ACCENT_PRESETS};

fn settings() -> ThemeSettings {
    ThemeSettings {
        accent: ACCENT_PRESETS[1].1,          // 海蓝
        font_family: Some("Microsoft YaHei UI".to_string()),
        font_size: 15.0,
        list_density: ListDensity::Compact,
    }
}
```

`ThemeSettings::default()` 使用第一个强调色预设（Bamboo 绿，
`[127, 211, 168]`）、`Microsoft YaHei UI`、`13.5` DIP 以及
`ListDensity::Comfortable`。

| 字段 | 默认值 | 被谁使用 |
|---|---|---|
| `accent` | `ACCENT_PRESETS[0].1` | `Palette::new` 由它派生悬停、按下、选中和播放颜色。 |
| `font_family` | `Some("Microsoft YaHei UI")` | `Painter` 构建 body/small/title/brand 样式。 |
| `font_size` | `13.5` | `font_scale() = (font_size / 13.5).clamp(0.85, 1.4)`。 |
| `list_density` | `ListDensity::Comfortable` | 仅通过 `row_height()`；控件不会隐式读取它。 |

`ListDensity::row_height()` 返回 `34.0`（`Compact`）、`40.0`
（`Comfortable`）或 `48.0`（`Spacious`）。将它传给应当遵循
该密度的列表：

```rust
use bamboo_native_ui::widgets::ListView;
use bamboo_native_ui::{ThemeSettings, Ui};

fn add_list(ui: &mut Ui<()>, settings: &ThemeSettings) {
    ui.add(ListView::new("library").row_height(settings.row_height()));
}
```

若不调用它，列表会保留自己的默认值（`ListView` 和
`PlaylistView` 使用 `44.0`）。

### 2. 在启动时安装主题

```rust
use bamboo_native_ui::{ThemeSettings, WindowBuilder};

fn theme() -> (ThemeSettings, bool) {
    (ThemeSettings::default(), true)
}

fn main() -> windows::core::Result<()> {
    let (settings, dark) = theme();
    WindowBuilder::new(App::new(settings.clone()))
        .title("Bamboo")
        .theme(settings, dark)
        .run()?;
    Ok(())
}
```

`WindowBuilder::theme(settings, dark)` 在首次绘制之前应用，因此
窗口、原生菜单栏和自绘窗口装饰从第一帧起就保持一致。

### 3. 在运行时切换

`WindowHandle::set_theme(settings, dark)` 重建调色板和文本
样式，在样式改变时丢弃缓存的字体，重新为主题化原生菜单，并
将整个窗口重绘一次。

```rust
use bamboo_native_ui::{ListDensity, ThemeSettings, WindowHandle};

fn toggle_density(&mut self, window: &mut WindowHandle<'_>) {
    self.settings.list_density = match self.settings.list_density {
        ListDensity::Compact => ListDensity::Comfortable,
        ListDensity::Comfortable => ListDensity::Spacious,
        ListDensity::Spacious => ListDensity::Compact,
    };
    window.set_theme(self.settings.clone(), self.dark);
    // 重新计算的矩形和内容属于应用：调整它们
    // 并使失效一次（set_theme 已经重绘了窗口）。
    self.relayout(window.size());
}
```

切换浅色/深色是相同的调用，只是 `bool` 不同：

```rust
use bamboo_native_ui::{ThemeSettings, WindowHandle};

fn set_dark(dark: bool, settings: &ThemeSettings, window: &mut WindowHandle<'_>) {
    window.set_theme(settings.clone(), dark);
}
```

如果你需要当前调色板（用于页面填充、自定义控件或布局
应用），用 `WindowHandle::palette()` 读回它，它返回
`&Palette`。

### 4. 预设与半径

```rust
use bamboo_native_ui::theme::radius;
use bamboo_native_ui::{Color, ACCENT_NAMES, ACCENT_PRESETS};

let labels = ACCENT_NAMES;                  // 与预设顺序相同
let accent = Color::rgb(ACCENT_PRESETS[3].1); // 珊瑚
let _soft = radius::SMALL;                  // 4.0
let _card = radius::MEDIUM;                 // 6.0
let _panel = radius::LARGE;                 // 10.0
```

`radius::pill(height)` 返回 `height.max(0.0) / 2.0`。布局 schema 的
`radius="small|medium|large|pill"` 解析为相同的值，因此
`.buitheme` 和 `.buixml` 文件共享同一套圆角词汇。`ACCENT_NAMES`
中的名称以中文提供；如需另一种语言，请构建你自己的标签数组。

### 5. 将调色板移入 `.buitheme` 文档

启用该特性：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["theme-xml"] }
```

`theme-xml` 启用 `layout-core` 主题读取器；它不要求
`layout-xml`，并且两个特性都不启用的构建不会链接任何 XML 解析器。

创建 `themes/night.buitheme`：

```xml
<?xml version="1.0" encoding="utf-8"?>
<theme version="1" name="Night" base="dark" accent="#7aa2f7">
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
  <color name="card" value="#1d2027"/>
</theme>
```

`base` 是必需的（`light` 或 `dark`）；它决定派生方式，并忽略
应用当前的模式。`accent` 是可选的，它会在派生之前馈送给
`Palette::new`，因此由强调色派生的字段（`accent-text`、
`row-selected`、按钮按下状态、`play_*`）会跟随它。每个 `<color>`
在派生之后按声明顺序覆盖一个调色板字段。一个
`<color name="accent">` 显式设置该字段而不重新派生其余字段。

### 6. 应用文档

```rust
use bamboo_native_ui::theme_xml::{ThemeDocument, ThemeDocumentExt};
use bamboo_native_ui::{Palette, ThemeSettings, WindowHandle};

fn apply_theme(
    text: &str,
    settings: &ThemeSettings,
    window: &mut WindowHandle<'_>,
) -> Option<(ThemeDocument, Palette)> {
    let document = ThemeDocument::parse(text).ok()?;
    let palette = document.palette(settings);
    window.set_theme(settings.clone(), palette.dark);
    window.set_palette(palette);
    Some((document, palette))
}
```

`palette(settings)` 复制设置，应用文档的强调色（若有），
用 `base` 调用 `Palette::new`，然后应用覆盖项。`set_theme` 设置
模式并重建文本样式；`set_palette` 安装派生的表并
重绘一次 —— 当文档处于活动状态时，两者都要做。

如果安装了布局文档，用新调色板重新应用它，以便其
`background` token 依据它解析：

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;
use bamboo_native_ui::{Palette, Size, Ui, UiHost};

fn reapply(
    ui: &mut Ui<()>,
    document: &LayoutDocument,
    palette: &Palette,
    client: Size,
    sink: &mut impl UiHost,
) {
    if let Err(failure) = ui.apply_layout(document, palette, client, sink) {
        eprintln!("layout rejected: {failure}");
    }
}
```

调色板是 `Copy`，因此将它传给 `apply_layout` 不会移动它。

### 7. 重新加载主题文档

主题文档是纯数据；库不会监视文件。

```rust
use bamboo_native_ui::theme_xml::ThemeDocument;
use bamboo_native_ui::{ThemeSettings, WindowHandle};

fn reload_theme(
    text: &str,
    current: Option<&ThemeDocument>,
    settings: &ThemeSettings,
    window: &mut WindowHandle<'_>,
) -> Option<ThemeDocument> {
    let document = ThemeDocument::parse(text).ok()?;
    if current.is_some_and(|old| old.generation() == document.generation()) {
        return None;
    }
    let palette = document.palette(settings);
    window.set_theme(settings.clone(), palette.dark);
    window.set_palette(palette);
    Some(document)
}
```

用 `WindowHandle::set_animation_timer(Some(Duration::from_millis(250)))`
安装计时器之后，从 `Host::timer` 轮询它。一次轮询
不会绘制，除非 generation 改变。

### 8. 不使用文档写入单个 token

```rust
use bamboo_native_ui::theme_xml::set_palette_token;
use bamboo_native_ui::{Color, Palette};

fn recolor(mut palette: Palette) -> Palette {
    let _ = set_palette_token(&mut palette, "accent", Color::from_hex(0x68_8c_ff));
    assert!(set_palette_token(&mut palette, "divider", Color::new(60, 64, 72)));
    palette
}
```

`set_palette_token` 返回该名称是否为调色板 token。可接受的
名称是[调色板 token](../reference/palette-tokens.md)中的 31 个 token。

## 权衡

| 方式 | 得到 | 代价 |
|---|---|---|
| 仅 `ThemeSettings` | 强调色、字体、缩放、密度；无文件 | 新颜色需要重新编译或应用代码 |
| `.buitheme` 文档 | 整个调色板作为数据；热重载 | `theme-xml`、要随附的文件、一个应用步骤 |
| 逐控件 `WidgetProps::palette` | 一张卡片或面板脱离主题 | 手工的逐控件状态；非全局 |
| 逐控件 `background` token | 一个表面跟随主题 | 仅表面填充，不含文本 |

主题文档只定义颜色。字体、密度和半径保留在
`ThemeSettings` 以及控件/表面构建器中。

## 常见错误

- **调用 `set_palette` 却期望字体改变** —— 它从不触碰
  文本样式。用 `set_theme` 设置模式和字体。
- **一个浅色的 `.buitheme` 看起来是深色** —— `base` 决定模式；
  不会参考应用当前的 `dark` 标志。
- **覆盖项不生效** —— `<color>` 名称是调色板 token
  名称（`nav-active`、`accent-text`），而不是 Rust 字段名
  （`nav_active`、`accent_text`）。
- **未知 token 被静默跳过** —— 解析器以
  `theme.color.unknown` 拒绝它；应检查解析结果而不是忽略它。
- **布局保留了旧颜色** —— 调色板在
  `apply_layout` 运行时被解析；在安装新调色板之后重新应用它。

## 另请参阅

- [主题与调色板 API](../api/theme-and-palette.md)
- [绘制与 Canvas](../api/painting-and-canvas.md)
- [调色板 token](../reference/palette-tokens.md)
- [XML 元素](../reference/xml-elements.md)
- [XML 布局编写](xml-layout-authoring.md)
- [Host 与窗口](../api/host-and-window.md)
