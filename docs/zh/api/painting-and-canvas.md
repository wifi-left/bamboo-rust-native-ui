# 绘制与 Canvas

> 在一次绘制过程中使用的、感知主题的 `Painter` 以及 `Canvas` 绘图后端 trait。

## 概述

绘制分为两层。`Canvas` 是后端：像素、文本、图像和裁剪。它是一个
trait 对象（`&mut dyn Canvas`），由窗口在其整个生命周期内持有，
因此它的字体、画刷、几何和图像缓存能跨帧保留。`Painter` 持有已解析的 `Palette`、
`ThemeSettings` 和文本
样式；它没有逐帧状态，绘制时从不分配，并且每个
方法都将 canvas 作为其第一个参数。

控件通过 `WidgetPaint`（`ctx.painter`、`ctx.canvas`）接收两者，它
添加了每个实例的字体覆盖和文本角色辅助方法——见
[控件 trait](./widget-trait.md)。每次绘制调用只分派一次（每帧数十
到几百次），而不是每个控件一次。

## 关键类型

### `Painter`

```rust
pub struct Painter { /* private */ }
impl Default for Painter { /* Painter::new() */ }

impl Painter {
    pub fn new() -> Self;
    pub fn caret_width(&self) -> f32;              // 默认 1.0
    pub fn set_caret_width(&mut self, width: f32); // 限制在 0.5..=6.0
    pub fn settings(&self) -> &ThemeSettings;
    pub fn palette(&self) -> &Palette;
    pub fn dark(&self) -> bool;

    /// 安装一个调色板而不改动文本样式。
    pub fn set_palette(&mut self, palette: Palette);
    /// 重建调色板和样式；文本样式发生变化时返回 `true`。
    pub fn set_theme(&mut self, settings: &ThemeSettings, dark: bool) -> bool;

    pub fn body(&self) -> &TextStyle;   pub fn small(&self) -> &TextStyle;
    pub fn title(&self) -> &TextStyle;  pub fn brand(&self) -> &TextStyle;
    pub fn glyph(&self) -> &TextStyle;  pub fn glyph_large(&self) -> &TextStyle;
    pub fn glyph_huge(&self) -> &TextStyle;
    pub fn ui_style(&self, size: f32) -> TextStyle;

    pub fn fill(&self, canvas: &mut dyn Canvas, rect: Rect, color: Color);
    pub fn frame(&self, canvas: &mut dyn Canvas, rect: Rect, border: Color, background: Color);
    pub fn round_rect(&self, canvas: &mut dyn Canvas, rect: Rect, radius: f32, color: Color);
    pub fn round_rect_corners(&self, canvas: &mut dyn Canvas, rect: Rect, corners: Corners, color: Color);
    pub fn round_rect_stroke(&self, canvas: &mut dyn Canvas, rect: Rect, radius: f32, thickness: f32, color: Color);
    pub fn circle(&self, canvas: &mut dyn Canvas, rect: Rect, color: Color);
    pub fn text(&self, canvas: &mut dyn Canvas, rect: Rect, text: &str, style: &TextStyle, align: TextAlign, color: Color);
}
```

角色样式会随 `ThemeSettings::font_scale()` 缩放：`body` 13.5、`small`
11.5、`title` 20.0 粗体、`brand` 12.0 半粗体、glyph 为 15/18/24。`ui_style`
返回一个一次性的正文样式，其给定尺寸会乘以主题缩放比例。
`set_theme` 返回文本样式是否发生变化，以便后端丢弃
缓存的字体。

### `Canvas`

```rust
pub trait Canvas {
    // 必需
    fn scale(&self) -> f32;          fn dpi(&self) -> f32;
    fn fill_rect(&mut self, rect: Rect, color: Color);
    fn text(&mut self, rect: Rect, text: &str, style: &TextStyle, align: TextAlign, color: Color);
    fn measure_text(&mut self, text: &str, style: &TextStyle) -> Size;
    fn ellipse(&mut self, rect: Rect, color: Color);
    fn vertical_gradient(&mut self, rect: Rect, top: Color, bottom: Color);
    fn image(&mut self, rect: Rect, crop_square: bool, id: ImageId, data: ImageData<'_>);
    fn push_clip(&mut self, rect: Rect);   fn pop_clip(&mut self);

    // 提供的回退实现
    fn fill_translucent(&mut self, rect: Rect, color: Color, alpha: f32);
    fn frame(&mut self, rect: Rect, border: Color, background: Color);
    fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: Color);
    fn fill_round_rect_corners(&mut self, rect: Rect, corners: Corners, color: Color);
    fn stroke_round_rect(&mut self, rect: Rect, radius: f32, thickness: f32, color: Color);
    fn stroke_round_rect_corners(&mut self, rect: Rect, corners: Corners, thickness: f32, color: Color);
    fn fill_round_frame(&mut self, rect: Rect, corners: Corners, thickness: f32, border: Color, fill: Color);
    fn fill_circle(&mut self, rect: Rect, color: Color);
    fn text_overflow(&mut self, rect: Rect, text: &str, style: &TextStyle, align: TextAlign, color: Color, overflow: TextOverflow);
    fn vertical_gradient_round(&mut self, rect: Rect, radius: f32, top: Color, bottom: Color);
    fn image_circle(&mut self, rect: Rect, crop_square: bool, id: ImageId, data: ImageData<'_>);
    fn image_round(&mut self, rect: Rect, radius: f32, crop_square: bool, id: ImageId, data: ImageData<'_>);
}
```

这些回退实现正确但朴素：圆角形状由矩形和
椭圆组合而成（无抗锯齿），半透明填充会不透明地绘制，裁剪图像
变体绘制方形图像。内置的 GDI 后端用
缓存的覆盖掩码、`AlphaBlend` 和裁剪图像覆盖它们；新的后端只需
实现必需的方法。

### 文本类型

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFace { Ui, Glyph, Named(Arc<str>) }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontWeight { Normal, Medium, Semibold, Bold }
impl FontWeight { pub const fn to_windows(self) -> i32; }   // 400/500/600/700

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle { pub size: f32, pub weight: FontWeight, pub italic: bool, pub face: FontFace }
impl TextStyle {
    pub fn ui(size: f32) -> Self;
    pub fn glyph(size: f32) -> Self;
    pub fn with_weight(mut self, weight: FontWeight) -> Self;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign { #[default] Left, Center, Right }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextOverflow { Clip, #[default] Ellipsis }
```

`FontFace::Ui` 是主题字体族（回退到 Microsoft YaHei UI），
`FontFace::Glyph` 是 Segoe MDL2 Assets，而 `Named` 是应用字体族。
`TextStyle::size` 已经是主题缩放后的，单位为 DIP。

### 图像与 `Renderer`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u64);

#[derive(Clone, Copy)]
pub struct ImageData<'a> { pub rgba: &'a [u8], pub width: u32, pub height: u32 }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Renderer { #[default] Gdi, D2dSoftware, D2dHardware }
```

`ImageData` 是借用的像素；后端按 `ImageId` 缓存它们，并且只在
缓存未命中时读取数据。`Renderer` 选择 `Canvas` 实现——
关于选择顺序和 `BAMBOO_UI_RENDERER`，见[宿主与窗口](./host-and-window.md)。

## 示例

```rust
use bamboo_native_ui::{Color, Corners, FontWeight, Painter, Rect, TextAlign, TextStyle};

fn paint_card(painter: &Painter, canvas: &mut dyn bamboo_native_ui::Canvas, rect: Rect) {
    let palette = painter.palette();
    painter.round_rect_corners(&mut *canvas, rect, Corners::uniform(6.0), palette.card);
    painter.round_rect_stroke(&mut *canvas, rect, 6.0, 1.0, palette.divider);

    let inner = rect.inset(12.0, 12.0);
    painter.text(&mut *canvas, inner, "Now playing", painter.title(), TextAlign::Left, palette.text);

    let style = TextStyle::ui(13.5).with_weight(FontWeight::Semibold);
    let accent = Color::from_hex(0x7FD3A8);
    painter.text(&mut *canvas, inner.translate(0.0, 28.0), "Bamboo", &style, TextAlign::Left, accent);
}
```

## 说明与限制

- `Canvas` 实现的存活期必须与窗口一致：它拥有缓存。
  绝不要逐帧构造一个。
- 绘制路径禁止分配。一次解析样式（角色样式、
  `ui_style`），借用文本，并让后端缓存字体、画刷、
  几何和图像。
- 提供的 `Canvas` 方法是回退实现，而不是首选的观感。GDI
  会绘制抗锯齿的圆角形状、圆环、圆形、半透明填充和
  裁剪图像；其他后端可能更朴素。
- `fill_round_rect` 将半径限制到较短边的一半，因此
  `rect.height() / 2.0` 就是一个胶囊。在缺少逐角掩码的后端上，混合半径的圆角填充会回退到
  最大半径。
- `text_overflow` 默认采用普通 `text` 行为（省略号）；只有
  区分两者的后端才会精确遵从 `TextOverflow::Clip`。
- `measure_text` 接收 `&mut self`（它会填充一个缓存）。
- `ImageId` 是稳定的缓存键：用相同的 id 和新像素重绘即可
  替换图像，或使用 `Ui::set_image`。
- `Renderer` 和 D2D 模块仅在 Windows 上以
  `feature = "gdi"` 编译（`#[cfg(all(windows, feature = "gdi"))]`）；`Canvas`
  和 `Painter` API 本身是平台无关的。

## 另见

- [控件 trait](./widget-trait.md)
- [主题与调色板](./theme-and-palette.md)
- [几何与颜色](./geometry-and-color.md)
- [宿主与窗口](./host-and-window.md)
- [Ui 容器](./ui-container.md)
- [Label](../widgets/content/label.md)
