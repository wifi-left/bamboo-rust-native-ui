# 11 — 主题与图像

> 切换浅色和深色、循环强调色预设、加载 `.buitheme` 调色板，并将
> 应用拥有的渐变馈送给头像和封面墙。

## 你将构建

一个带三个按钮 —— **Light / dark**、**Next accent** 和 **Night
theme** —— 以及一个头像和一个封面墙的窗口。一个按钮重新派生整个调色板，
将其安装到窗口并重绘；控件随之更新，因为它们在绘制时
从调色板读取颜色。Night 主题是一个 `.buitheme` 文档。

图像是独立的：页面用
`Ui::set_image` 注册两个 RGBA 图像，并将其数字 `ImageId` 交给 `Avatar` 和 `CoverWall`。
不涉及图像文件。

## 前置条件

- [03](03-first-window.md) 中的项目可以构建并运行。
- 对于第 4 和第 5 步，启用主题解析器：

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["theme-xml"] }
```

`theme-xml` 添加 `ThemeDocument::parse` 和 `ThemeDocumentExt::palette`；它
独立于 `layout-xml`。

## 步骤

### 1. 将主题输入保存在应用状态中

`ThemeSettings` 是一个普通值。默认值是 Bamboo 绿色强调色、
`Microsoft YaHei UI`、13.5 DIP 基础字号和舒适的行高。

```rust
use bamboo_native_ui::widgets::{Avatar, Cover, CoverWall};
use bamboo_native_ui::{
    ACCENT_NAMES, ACCENT_PRESETS, Color, ImageId, ListDensity, Palette, ThemeSettings, Ui,
    UiImage, WidgetId, WindowHandle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    ToggleDark,
    NextAccent,
    NightTheme,
}

struct App {
    ui: Ui<Msg>,
    settings: ThemeSettings,
    dark: bool,
}
```

`new` 添加一个标签和三个按钮，设置 `dark: true` 和
`settings.list_density = ListDensity::Compact`。`ThemeSettings` 是
`{ accent, font_family, font_size, list_density }`；行高为 34、40 和
48 DIP。

### 2. 派生并安装调色板

`Palette::new(&settings, dark)` 从强调色和模式派生每个令牌。
`set_theme` 存储设置、重建字体和菜单颜色并
失效；`set_palette` 安装一个已派生的表。

```rust
impl App {
    fn apply_theme(&mut self, window: &mut WindowHandle<'_>) {
        let palette = Palette::new(&self.settings, self.dark);
        window.set_theme(self.settings.clone(), self.dark);
        window.set_palette(palette);
    }
}
```

`paint` 是第 03 页的：用
`ctx.painter.palette().background` 填充每个脏矩形，然后调用 `ui.paint(ctx)`。每个 setter
使窗口失效。

### 3. 切换浅色/深色并循环强调色

`ACCENT_PRESETS` 是六个 `(label, [u8; 3])` 对；`ACCENT_NAMES` 重复
标签。第一个条目是默认绿色。

```rust
impl App {
    fn apply(&mut self, message: Msg, window: &mut WindowHandle<'_>) {
        match message {
            Msg::ToggleDark => {
                self.dark = !self.dark;
                self.apply_theme(window);
            }
            Msg::NextAccent => {
                let current = ACCENT_PRESETS.iter()
                    .position(|(_, rgb)| *rgb == self.settings.accent)
                    .unwrap_or(0);
                let next = (current + 1) % ACCENT_PRESETS.len();
                self.settings.accent = ACCENT_PRESETS[next].1;
                self.ui.set_label(WidgetId::new("accent"), ACCENT_NAMES[next]);
                self.apply_theme(window);
            }
            Msg::NightTheme => self.apply_night(window),
        }
    }
}
```

预设标签是设置页面显示的中文名称；RGB 值
才是调色板关心的。任何其他 `[u8; 3]` 都可以作为强调色。

### 4. 加载 `.buitheme` 文档

创建 `bamboo-hello/themes/night.buitheme`：

```xml
<?xml version="1.0" encoding="utf-8"?>
<theme version="1" name="Night Blue" base="dark" accent="#7aa2f7">
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
  <color name="card" value="#22262e"/>
</theme>
```

`base` 是必需的。`accent` 在派生之前替换设置中的强调色；
每个 `<color>` 覆盖一个令牌。派生是 `Palette::new` 加覆盖。

```rust
use bamboo_native_ui::theme_xml::{ThemeDocument, ThemeDocumentExt};

impl App {
    fn apply_night(&mut self, window: &mut WindowHandle<'_>) {
        let text = include_str!("../themes/night.buitheme");
        let Ok(document) = ThemeDocument::parse(text) else {
            return; // a bad document leaves the current palette alone
        };
        let palette = document.palette(&self.settings);
        self.dark = palette.dark;
        self.settings.accent = palette.accent.to_rgb();
        window.set_theme(self.settings.clone(), self.dark);
        window.set_palette(palette);
    }
}
```

`palette()` 来自 `ThemeDocumentExt`，因此该 trait 必须在作用域内。当
`parse` 返回诊断时保留旧调色板。

### 5. 覆盖单个令牌

令牌名是 kebab-case，与布局 schema 中一样：`background`、`nav-active`、`button-hover`……

```rust
use bamboo_native_ui::theme_xml::set_palette_token;

impl App {
    fn tint_card(&self, window: &mut WindowHandle<'_>) {
        let mut palette = Palette::new(&self.settings, self.dark);
        if set_palette_token(&mut palette, "card", Color::from_hex(0x23262e)) {
            window.set_palette(palette);
        }
    }
}
```

名称未知时，`set_palette_token` 返回 `false` 且不写入任何内容。

### 6. 注册图像并通过 id 引用它们

图像是应用拥有的 RGBA 像素。`UiImage::solid` 填充一种颜色；
`UiImage::gradient` 从上到下插值。`UiImage::new` 接受真实
解码的像素。

```rust
impl App {
    fn register_images(ui: &mut Ui<Msg>) {
        ui.set_image(UiImage::solid(ImageId(1), 96, 96, Color::from_hex(0x4C6EF5)));
        ui.set_image(UiImage::gradient(
            ImageId(10), 160, 160,
            Color::from_hex(0x5B6BA8), Color::from_hex(0x222B4C),
        ));
    }
}
```

在添加控件之前从 `new` 调用 `register_images`。`Avatar::image`
居中裁剪到圆形；`Cover::image` 回退到着色瓦片。

```rust
ui.add(Avatar::new("me", "Bamboo Fan").image(ImageId(1)));
ui.add(
    CoverWall::new("wall")
        .covers(vec![
            Cover::new("Nocturne").subtitle("Jay Chou").image(ImageId(10)),
            Cover::new("Morning Mist").subtitle("Lin Xiangwan"),
        ])
        .tile(148.0).gap(14.0).corner_radius(6.0),
);
```

页面拥有存储：`UiImage` 持有一个 `Arc<[u8]>`，因此墙、头像和
播放卡片共用的封面只占一份副本。在同一个 `ImageId` 下注册另一个图像
会为每个引用替换像素。`set_image`
不会失效，因此要重绘显示它的控件。

文档使用相同的数字 id；没有图像名注册表。在应用布局之前
在 Rust 中注册像素。

```xml
<widget id="wall" x="16" y="120" w="360" h="320">
  <cover title="Nocturne" subtitle="Jay Chou" image="10"/>
  <cover title="Morning Mist" subtitle="Lin Xiangwan"/>
</widget>
```

`image` 被解析为数字并变成 `ImageId(10)`。XML 不能加载文件
或命名图像；未注册的 id 绘制回退瓦片。

### 7. 运行它

`cargo run`。**Next accent** 移动每个由强调色派生的颜色；浅色/深色
切换页面、菜单和对话框；**Night theme** 应用该文档。

## 预期结果

- 浅色/深色切换页面、所有者绘制的菜单栏和工具提示。
- 循环强调色无需重启即可重新着色控件。
- Night 文档加载并保持其未覆盖的令牌不变。
- 头像显示纯色图像；第一个封面显示渐变，
  第二个显示回退瓦片。

## 故障排查

- **编辑 `self.settings` 后没有任何变化** —— `ThemeSettings` 是一个
  值。调用 `set_theme` 和 `set_palette`。
- **一个 `.buixml` 页面保持旧颜色** —— 令牌在布局
  应用时解析。用新调色板重新应用它。
- **封面空白或全是回退瓦片** —— `ImageId` 从未注册；
  id 是普通数字，不是文件名。
- **替换的图像仍显示旧像素** —— `set_image` 不会失效。
  重绘覆盖的矩形。
- **`ThemeDocument::parse` 失败** —— `version="1"` 是必需的，`base` 必须是
  `light` 或 `dark`，未知的 `<color>` 名称会被拒绝。

## 下一步

[12 — 文本编辑与输入法](12-text-editing-and-ime.md)：原生和绘制的文本
字段、数字和搜索输入，以及中文/日文输入。
