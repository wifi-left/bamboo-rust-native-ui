# Painting and Canvas

> The theme-aware `Painter` and the `Canvas` drawing backend traits used during one paint pass.

## Overview

Painting has two layers. `Canvas` is the backend: pixels, text, images and
clipping. It is a trait object (`&mut dyn Canvas`) owned by the window for its
whole lifetime, so its font, brush, geometry and image caches survive between
frames. `Painter` holds the resolved `Palette`, the `ThemeSettings` and the text
styles; it has no per-frame state and never allocates while painting, and every
method takes the canvas as its first argument.

Widgets receive both through `WidgetPaint` (`ctx.painter`, `ctx.canvas`), which
adds the per-instance font override and text-role helpers — see
[The widget trait](./widget-trait.md). Dispatch happens once per draw call (tens
to a few hundred per frame), not per widget.

## Key types

### `Painter`

```rust
pub struct Painter { /* private */ }
impl Default for Painter { /* Painter::new() */ }

impl Painter {
    pub fn new() -> Self;
    pub fn caret_width(&self) -> f32;              // default 1.0
    pub fn set_caret_width(&mut self, width: f32); // clamped to 0.5..=6.0
    pub fn settings(&self) -> &ThemeSettings;
    pub fn palette(&self) -> &Palette;
    pub fn dark(&self) -> bool;

    /// Install a palette without touching the text styles.
    pub fn set_palette(&mut self, palette: Palette);
    /// Rebuild palette and styles; `true` when the text styles changed.
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

The role styles scale with `ThemeSettings::font_scale()`: `body` 13.5, `small`
11.5, `title` 20.0 bold, `brand` 12.0 semibold, glyphs 15/18/24. `ui_style`
returns a one-off body style whose given size is multiplied by the theme scale.
`set_theme` returns whether the text styles changed so the backend can drop
cached fonts.

### `Canvas`

```rust
pub trait Canvas {
    // Required
    fn scale(&self) -> f32;          fn dpi(&self) -> f32;
    fn fill_rect(&mut self, rect: Rect, color: Color);
    fn text(&mut self, rect: Rect, text: &str, style: &TextStyle, align: TextAlign, color: Color);
    fn measure_text(&mut self, text: &str, style: &TextStyle) -> Size;
    fn ellipse(&mut self, rect: Rect, color: Color);
    fn vertical_gradient(&mut self, rect: Rect, top: Color, bottom: Color);
    fn image(&mut self, rect: Rect, crop_square: bool, id: ImageId, data: ImageData<'_>);
    fn push_clip(&mut self, rect: Rect);   fn pop_clip(&mut self);

    // Provided fallbacks
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

The fallbacks are correct but plain: rounded shapes compose rectangles and
ellipses (no anti-aliasing), translucent fills paint opaquely, and clipped image
variants paint the square image. The bundled GDI backend overrides them with
cached coverage masks, `AlphaBlend` and clipped images; a new backend only has
to implement the required methods.

### Text types

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

`FontFace::Ui` is the theme family (falling back to Microsoft YaHei UI),
`FontFace::Glyph` is Segoe MDL2 Assets, and `Named` is an application family.
`TextStyle::size` is already theme-scaled, in DIPs.

### Images and `Renderer`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u64);

#[derive(Clone, Copy)]
pub struct ImageData<'a> { pub rgba: &'a [u8], pub width: u32, pub height: u32 }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Renderer { #[default] Gdi, D2dSoftware, D2dHardware }
```

`ImageData` is borrowed pixels; the backend caches them by `ImageId` and reads
the data only on a cache miss. `Renderer` selects the `Canvas` implementation —
see [Host and window](./host-and-window.md) for the selection order and
`BAMBOO_UI_RENDERER`.

## Example

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

## Notes and limits

- A `Canvas` implementation must live for the lifetime of the window: it owns
  the caches. Never construct one per frame.
- The paint path forbids allocation. Resolve styles once (role styles,
  `ui_style`), borrow text, and let the backend cache fonts, brushes,
  geometries and images.
- The provided `Canvas` methods are fallbacks, not the preferred look. GDI
  paints anti-aliased rounded shapes, rings, circles, translucent fills and
  clipped images; another backend may be plainer.
- `fill_round_rect` clamps the radius to half the shorter side, so
  `rect.height() / 2.0` is a pill. Mixed-radius corner fills fall back to the
  largest radius on a backend without per-corner masks.
- `text_overflow` defaults to the plain `text` behavior (an ellipsis); only a
  backend that distinguishes the two honors `TextOverflow::Clip` exactly.
- `measure_text` takes `&mut self` (it fills a cache).
- `ImageId` is a stable cache key: repaint with the same id and new pixels to
  replace an image, or use `Ui::set_image`.
- `Renderer` and the D2D modules are compiled only with
  `feature = "gdi"` on Windows (`#[cfg(all(windows, feature = "gdi"))]`); the
  `Canvas` and `Painter` API itself is platform-neutral.

## See also

- [The widget trait](./widget-trait.md)
- [Theme and palette](./theme-and-palette.md)
- [Geometry and color](./geometry-and-color.md)
- [Host and window](./host-and-window.md)
- [The Ui container](./ui-container.md)
- [Label](../widgets/content/label.md)
