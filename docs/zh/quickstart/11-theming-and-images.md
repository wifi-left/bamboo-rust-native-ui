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
- 对于第 4 和第 5 步，启用主题解析器。编辑 crate 清单
  （`bamboo-hello/bamboo-hello/Cargo.toml`）：

```toml
[dependencies]
# `theme-xml` 增加 `.buitheme` 解析；不需要 `layout-xml`。
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["theme-xml"] }
```

`theme-xml` 添加 `ThemeDocument::parse` 和 `ThemeDocumentExt::palette`；它
独立于 `layout-xml`。

## 本节新术语

- **主题设置** —— 应用选择的少数几项输入：强调色、字体家族、基础字号
  和列表密度。`ThemeSettings` 是一个普通值。
- **调色板** —— 控件在绘制时读取的完整颜色表
  （`background`、`text`、`button-hover`……）。它是由设置和浅色/深色模式
  *派生*出来的，而不是手写的。
- **调色板 token** —— 那张表里的一个具名条目。覆盖一个 token 只改变
  一种颜色，派生出的其余部分保持完整。
- **浅色/深色** —— 派生时使用的模式；深色模式让表面变暗，
  浅色模式让表面变亮。
- **强调色** —— 那个品牌色，其他由强调色派生的 token（选区、焦点、
  活动标签页）都由它着色而来。
- **`.buitheme`** —— Bamboo 的小型 XML 主题文档：一个基础模式、一个可选的
  强调色，以及一组 token 覆盖。
- **图像存储** —— `Ui` 的像素表。`ImageId` 是用来引用其中一个条目的数字；
  它不是文件名。
- **RGBA** —— 每个像素四个字节：红、绿、蓝和 alpha（不透明度）。
- **渐变** —— 两种颜色之间的平滑过渡；`UiImage::gradient`
  从顶部颜色插值到底部颜色。

本页所有 Rust 代码都写入 crate 的 `src/main.rs`
（`bamboo-hello/bamboo-hello/src/main.rs`）。

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
    ToggleDark, // 浅色 <-> 深色
    NextAccent, // 在六个预设之间逐步切换
    NightTheme, // 应用 .buitheme 文档
}

struct App {
    ui: Ui<Msg>,
    settings: ThemeSettings, // 输入：强调色、字体、字号、密度
    dark: bool,              // 传给调色板派生的模式
}
```

`new` 添加一个标签和三个按钮，设置 `dark: true` 和
`settings.list_density = ListDensity::Compact`。`ThemeSettings` 是
`{ accent, font_family, font_size, list_density }`；行高为 34、40 和
48 DIP。

### 2. 派生并安装调色板

`Palette::new(&settings, dark)` 从强调色和模式派生每个 token。
`set_theme` 存储设置、重建字体和菜单颜色并
失效；`set_palette` 安装一个已派生的表。

```rust
impl App {
    fn apply_theme(&mut self, window: &mut WindowHandle<'_>) {
        // 根据设置和模式重建每一种颜色。
        let palette = Palette::new(&self.settings, self.dark);
        // 设置驱动字体和菜单颜色；调色板驱动控件。
        window.set_theme(self.settings.clone(), self.dark);
        window.set_palette(palette);
    }
}
```

`paint` 是第 03 页的：用
`ctx.painter.palette().background` 填充每个脏矩形，然后调用 `ui.paint(ctx)`。每个 setter
使窗口失效。

**刚才发生了什么** —— `apply_theme` 读取当前设置和模式，
派生出一张完整的颜色表，把设置存到窗口上，并安装
这张表。控件不需要逐个告知颜色：它们重绘时会从调色板里
查找，而两个 setter 已经请求了重绘。

### 3. 切换浅色/深色并循环强调色

`ACCENT_PRESETS` 是六个 `(label, [u8; 3])` 对；`ACCENT_NAMES` 重复
标签。第一个条目是默认绿色。

```rust
impl App {
    fn apply(&mut self, message: Msg, window: &mut WindowHandle<'_>) {
        match message {
            Msg::ToggleDark => {
                self.dark = !self.dark;
                self.apply_theme(window); // 重新派生并重绘
            }
            Msg::NextAccent => {
                // 找到当前预设，然后回绕到下一个。
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

创建 `bamboo-hello/themes/night.buitheme` —— 放在 crate 文件夹里，紧挨
`src/`：

```xml
<?xml version="1.0" encoding="utf-8"?>
<!-- `base` 是必需的；`accent` 在派生之前替换设置中的强调色。 -->
<theme version="1" name="Night Blue" base="dark" accent="#7aa2f7">
  <!-- 每个 color 在派生之后覆盖一个调色板 token。 -->
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
  <color name="card" value="#22262e"/>
</theme>
```

`base` 是必需的。`accent` 在派生之前替换设置中的强调色；
每个 `<color>` 覆盖一个 token。派生是 `Palette::new` 加覆盖。

```rust
use bamboo_native_ui::theme_xml::{ThemeDocument, ThemeDocumentExt};

impl App {
    fn apply_night(&mut self, window: &mut WindowHandle<'_>) {
        // 在编译时嵌入；路径相对于 `src/main.rs`。
        let text = include_str!("../themes/night.buitheme");
        let Ok(document) = ThemeDocument::parse(text) else {
            return; // 文档有误时保持当前调色板不变
        };
        // 先从设置派生，再应用文档的覆盖。
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

**刚才发生了什么** —— 点击 **Night theme** 解析了嵌入的 XML，
以文档的深色基础和蓝色强调色派生出一个调色板，并叠加了
那三个 `<color>` 条目。随后应用把得到的模式和强调色
复制回自己的状态，这样下一次切换会从屏幕上当前的样子开始；
未被覆盖的 token 保留了派生出的值。

### 5. 覆盖单个 token

token 名是 kebab-case，与布局 schema 中一样：`background`、`nav-active`、`button-hover`……

```rust
use bamboo_native_ui::theme_xml::set_palette_token;

impl App {
    fn tint_card(&self, window: &mut WindowHandle<'_>) {
        // 从派生出的调色板开始，然后替换一个 token。
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
        // 一张 96x96 的纯色图像：红、绿、蓝、alpha 各 8 位。
        ui.set_image(UiImage::solid(ImageId(1), 96, 96, Color::from_hex(0x4C6EF5)));
        // 一张 160x160 的图像，从顶部颜色过渡到底部颜色。
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
ui.add(Avatar::new("me", "Bamboo Fan").image(ImageId(1))); // 居中裁剪到圆形
ui.add(
    CoverWall::new("wall")
        .covers(vec![
            // 第一个封面使用注册为 ImageId(10) 的渐变。
            Cover::new("Nocturne").subtitle("Jay Chou").image(ImageId(10)),
            // 没有图像：封面改为绘制它的回退瓦片。
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
  <!-- `image` 是数字，不是文件名：它必须与某个 ImageId 匹配。 -->
  <cover title="Nocturne" subtitle="Jay Chou" image="10"/>
  <cover title="Morning Mist" subtitle="Lin Xiangwan"/>
</widget>
```

`image` 被解析为数字并变成 `ImageId(10)`。XML 不能加载文件
或命名图像；未注册的 id 绘制回退瓦片。

### 7. 运行它

从 crate 文件夹运行（`bamboo-hello/bamboo-hello`，也就是
`src/main.rs` 旁边的那个）。

```powershell
# 工作目录：bamboo-hello/bamboo-hello
cargo run
```

**Next accent** 移动每个由强调色派生的颜色；浅色/深色
切换页面、菜单和对话框；**Night theme** 应用该文档。

如果构建停在 `include_str!`，说明 `.buitheme` 文件不在
`src/` 旁边的 `themes/night.buitheme`；编译器会打印它尝试过的路径。

## 预期结果

窗口以页面初始的深色模式和绿色强调色打开。此后：

- 浅色/深色切换页面、所有者绘制的菜单栏和工具提示。
- 循环强调色无需重启即可重新着色控件。
- Night 文档加载并保持其未覆盖的 token 不变。
- 头像显示纯色图像；第一个封面显示渐变，
  第二个显示回退瓦片。

## 故障排查

- **编辑 `self.settings` 后没有任何变化** —— `ThemeSettings` 是一个
  值。调用 `set_theme` 和 `set_palette`。
- **点击 Light/dark 后页面看起来仍然是深色** —— 只替换了调色板。
  `set_theme` 还会刷新字体和菜单颜色；像 `apply_theme` 那样两个都调用。
- **强调色按钮改了标签但没改颜色** —— 调色板没有重新派生。
  修改 `settings.accent`，然后再次调用 `Palette::new` 和
  `set_palette`。
- **一个 `.buixml` 页面保持旧颜色** —— token 在布局
  应用时解析。用新调色板重新应用它。
- **`include_str!` 找不到 `themes/night.buitheme`** —— 该路径
  相对于源文件解析。把 `themes/` 放在 `src/` 旁边，而不是放在
  工作区清单旁边。
- **封面空白或全是回退瓦片** —— `ImageId` 从未注册；
  id 是普通数字，不是文件名。
- **头像是普通圆形** —— `Avatar::image` 收到一个没有用
  `Ui::set_image` 注册的 id；未知的 id 不会绘制任何图像。
- **替换的图像仍显示旧像素** —— `set_image` 不会失效。
  重绘覆盖的矩形。
- **文本使用了意料之外的字体** —— `ThemeSettings` 中的字体家族
  没有安装，于是 Windows 替换成了另一个。挑一个已安装的家族或保留
  默认值。
- **`ThemeDocument::parse` 失败** —— `version="1"` 是必需的，`base` 必须是
  `light` 或 `dark`，未知的 `<color>` 名称会被拒绝。

## 下一步

[12 — 文本编辑与输入法](12-text-editing-and-ime.md)：原生和绘制的文本
字段、数字和搜索输入，以及中文/日文输入。
