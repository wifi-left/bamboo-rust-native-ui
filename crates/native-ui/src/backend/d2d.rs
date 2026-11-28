// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Direct2D renderer over the same persistent GDI back buffer.
//!
//! The window keeps one back buffer and blits the dirty bounding box, exactly
//! like the GDI path; only the drawing half of the frame changes. A
//! `ID2D1DCRenderTarget` bound to that buffer renders with the GPU when the
//! driver offers one (`D2D1_RENDER_TARGET_TYPE_HARDWARE`), with a software
//! rasterizer on request (`…_SOFTWARE`), and Direct2D itself falls back to its
//! software target when hardware creation fails. Because the target is created
//! at `96 × scale` DPI, every geometry is in DIPs — the same coordinates every
//! widget already uses.
//!
//! What is cached across frames (and dropped together with the target when the
//! device is lost): solid brushes, image bitmaps and one layer. What is cached
//! for the life of the window (device-independent): text formats and geometry
//! paths (rounded rectangles, rings, ellipse masks). Nothing here touches the
//! GDI resources, so switching renderers never invalidates fonts or masks.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use windows_numerics::{Matrix3x2, Vector2};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
    D2D1_FILL_MODE_ALTERNATE, D2D1_GRADIENT_STOP, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_F,
    D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ARC_SEGMENT,
    D2D1_ARC_SIZE_SMALL, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BITMAP_PROPERTIES,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT, D2D1_GAMMA_2_2,
    D2D1_LAYER_OPTIONS_NONE, D2D1_LAYER_PARAMETERS, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
    D2D1_RENDER_TARGET_TYPE_HARDWARE, D2D1_RENDER_TARGET_TYPE_SOFTWARE,
    D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT, D2D1_SWEEP_DIRECTION_CLOCKWISE,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory, ID2D1Bitmap, ID2D1Brush,
    ID2D1EllipseGeometry, ID2D1Factory, ID2D1Geometry, ID2D1Layer,
    ID2D1PathGeometry, ID2D1RenderTarget, ID2D1RoundedRectangleGeometry, ID2D1SolidColorBrush,
    ID2D1DCRenderTarget,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT, DWRITE_MEASURING_MODE_NATURAL,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TEXT_METRICS, DWRITE_TRIMMING,
    DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory,
    IDWriteFactory, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::HDC;
use windows::core::{Interface, PCWSTR};

use crate::backend::{
    Canvas, FontFace, ImageData, ImageId, Renderer, TextAlign, TextStyle, TextOverflow,
};
use crate::color::Color;
use crate::geometry::{Corners, Rect, Size};

/// Upper bound for image bitmaps (RGBA bytes, same budget as the GDI cache).
const IMAGE_BUDGET: usize = 16 * 1024 * 1024;
/// Above this count the solid-brush cache is left to grow no further (a color
/// that misses while full is created per draw instead).
const MAX_BRUSHES: usize = 512;
/// Above this count the geometry cache is dropped wholesale; widget geometry
/// is stable, so this only guards a pathological caller.
const MAX_GEOMETRIES: usize = 512;
const MAX_FORMATS: usize = 64;

/// Device-independent resources (factory, DirectWrite, caches) plus the
/// device-dependent target, which is recreated after a device loss.
pub struct D2dResources {
    factory: Option<ID2D1Factory>,
    dwrite: Option<IDWriteFactory>,
    target: Option<Target>,
    formats: HashMap<FormatKey, IDWriteTextFormat>,
    geometries: HashMap<GeometryKey, Geometry>,
}

/// One device-dependent render target and everything bound to it.
struct Target {
    rt: ID2D1DCRenderTarget,
    kind: Renderer,
    dpi: f32,
    brushes: HashMap<u32, ID2D1SolidColorBrush>,
    images: HashMap<u64, CachedBitmap>,
    image_order: VecDeque<u64>,
    image_bytes: usize,
    layer: Option<ID2D1Layer>,
}

struct CachedBitmap {
    bitmap: ID2D1Bitmap,
    bytes: usize,
}

/// A cached geometry. Every variant is built at the rectangle's position, so
/// the key includes the origin (positions are stable across frames).
enum Geometry {
    Rounded(ID2D1RoundedRectangleGeometry),
    Ellipse(ID2D1EllipseGeometry),
    Path(ID2D1PathGeometry),
}

impl Geometry {
    fn geometry(&self) -> &ID2D1Geometry {
        match self {
            Geometry::Rounded(geometry) => &**geometry,
            Geometry::Ellipse(geometry) => &**geometry,
            Geometry::Path(geometry) => &**geometry,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum GeometryKey {
    Rounded {
        left: u32,
        top: u32,
        width: u32,
        height: u32,
        radius: u32,
    },
    Corners {
        left: u32,
        top: u32,
        width: u32,
        height: u32,
        corners: [u32; 4],
    },
    Ring {
        left: u32,
        top: u32,
        width: u32,
        height: u32,
        radius: u32,
        thickness: u32,
    },
    RingCorners {
        left: u32,
        top: u32,
        width: u32,
        height: u32,
        corners: [u32; 4],
        thickness: u32,
    },
    Ellipse {
        left: u32,
        top: u32,
        width: u32,
        height: u32,
    },
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct FormatKey {
    family: Arc<str>,
    size_x10: u32,
    weight: i32,
    italic: bool,
    align: i32,
    trim: bool,
}

/// The drawing half of a frame on a Direct2D DC target.
pub struct D2dCanvas<'a> {
    res: &'a mut D2dResources,
    scale: f32,
    dpi: f32,
    clip_depth: usize,
    text_buf: Vec<u16>,
    /// `EndDraw` runs in `Drop`; this tracks a successful `BeginDraw`.
    drawing: bool,
    /// The target failed; the next frame recreates it (device loss).
    broken: bool,
}

impl D2dResources {
    pub fn new() -> Self {
        Self {
            factory: None,
            dwrite: None,
            target: None,
            formats: HashMap::new(),
            geometries: HashMap::new(),
        }
    }

    fn factory(&mut self) -> Option<&ID2D1Factory> {
        if self.factory.is_none() {
            self.factory = unsafe {
                D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).ok()
            };
        }
        self.factory.as_ref()
    }

    fn dwrite(&mut self) -> Option<&IDWriteFactory> {
        if self.dwrite.is_none() {
            self.dwrite = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok() };
        }
        self.dwrite.as_ref()
    }

    /// Drop the device-dependent target so the next frame rebuilds it (device
    /// loss, resize of the back buffer, or a renderer switch).
    fn drop_target(&mut self) {
        self.target = None;
    }

    /// Create (or reuse) the DC render target bound to `hdc` and start a frame.
    fn begin(
        &mut self,
        hdc: HDC,
        width: i32,
        height: i32,
        dpi: f32,
        kind: Renderer,
    ) -> Option<&mut Target> {
        let recreate = match &self.target {
            Some(target) => target.kind != kind || (target.dpi - dpi).abs() > 0.01,
            None => true,
        };
        if recreate {
            self.drop_target();
            let factory = self.factory()?.clone();
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: match kind {
                    Renderer::D2dHardware => D2D1_RENDER_TARGET_TYPE_HARDWARE,
                    Renderer::D2dSoftware => D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                    _ => D2D1_RENDER_TARGET_TYPE_DEFAULT,
                },
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: dpi,
                dpiY: dpi,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let rt = unsafe { factory.CreateDCRenderTarget(&properties) }.ok()?;
            self.target = Some(Target {
                rt,
                kind,
                dpi,
                brushes: HashMap::new(),
                images: HashMap::new(),
                image_order: VecDeque::new(),
                image_bytes: 0,
                layer: None,
            });
        }
        let mut failed = false;
        {
            let target = self.target.as_mut()?;
            let bounds = RECT {
                left: 0,
                top: 0,
                right: width.max(1),
                bottom: height.max(1),
            };
            unsafe {
                if target.rt.BindDC(hdc, &bounds).is_err() {
                    failed = true;
                } else {
                    target.rt.BeginDraw();
                    // The back buffer is written with premultiplied alpha;
                    // ClearType is unavailable there, so ask for grayscale.
                    let _ = target.rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
                    let _ = target.rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                }
            }
        }
        if failed {
            self.target = None;
            return None;
        }
        self.target.as_mut()
    }

    /// The text format for one style/alignment; cached for the window's life.
    fn format(
        &mut self,
        style: &TextStyle,
        align: TextAlign,
        overflow: TextOverflow,
    ) -> Option<IDWriteTextFormat> {
        let align_key = match align {
            TextAlign::Left => 0,
            TextAlign::Center => 1,
            TextAlign::Right => 2,
        };
        let family: Arc<str> = match &style.face {
            FontFace::Ui => Arc::from(family_name(&FontFace::Ui)),
            FontFace::Glyph => Arc::from(family_name(&FontFace::Glyph)),
            FontFace::Named(name) => name.clone(),
        };
        let key = FormatKey {
            family,
            size_x10: (style.size.max(0.5) * 10.0).round() as u32,
            weight: style.weight.to_windows(),
            italic: style.italic,
            align: align_key,
            trim: overflow == TextOverflow::Ellipsis,
        };
        if let Some(format) = self.formats.get(&key) {
            return Some(format.clone());
        }
        let dwrite = self.dwrite()?.clone();
        let family = utf16(key.family.as_ref());
        let locale = utf16("");
        let format = unsafe {
            dwrite.CreateTextFormat(
                PCWSTR(family.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT(key.weight),
                if key.italic {
                    DWRITE_FONT_STYLE_ITALIC
                } else {
                    DWRITE_FONT_STYLE_NORMAL
                },
                DWRITE_FONT_STRETCH_NORMAL,
                key.size_x10 as f32 / 10.0,
                PCWSTR(locale.as_ptr()),
            )
        }
        .ok()?;
        unsafe {
            let _ = format.SetTextAlignment(match align {
                TextAlign::Left => DWRITE_TEXT_ALIGNMENT_LEADING,
                TextAlign::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
                TextAlign::Right => DWRITE_TEXT_ALIGNMENT_TRAILING,
            });
            let _ = format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            let _ = format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
            if key.trim
                && let Ok(sign) = dwrite.CreateEllipsisTrimmingSign(&format)
            {
                let trimming = DWRITE_TRIMMING {
                    granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                    delimiter: 0,
                    delimiterCount: 0,
                };
                let _ = format.SetTrimming(&trimming, &sign);
            }
        }
        if self.formats.len() >= MAX_FORMATS {
            self.formats.clear();
        }
        self.formats.insert(key, format.clone());
        Some(format)
    }
}

impl Default for D2dResources {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> D2dCanvas<'a> {
    /// Bind the back buffer `hdc` (a `width × height` physical-pixel surface)
    /// and start a frame. `None` means "use the GDI path": the renderer is
    /// compiled in but Direct2D could not start.
    pub fn new(
        hdc: HDC,
        res: &'a mut D2dResources,
        scale: f32,
        dpi: f32,
        width: i32,
        height: i32,
        kind: Renderer,
    ) -> Option<Self> {
        res.begin(hdc, width, height, 96.0 * scale, kind)?;
        Some(Self {
            res,
            scale,
            dpi,
            clip_depth: 0,
            text_buf: Vec::new(),
            drawing: true,
            broken: false,
        })
    }

    fn rt(&self) -> Option<&ID2D1RenderTarget> {
        self.res.target.as_ref().map(|target| {
            let rt: &ID2D1RenderTarget = &target.rt;
            rt
        })
    }

    /// The cached text format for one style and alignment.
    fn text_format(
        &mut self,
        style: &TextStyle,
        align: TextAlign,
        overflow: TextOverflow,
    ) -> Option<IDWriteTextFormat> {
        self.res.format(style, align, overflow)
    }

    fn brush_with_alpha(&mut self, color: Color, alpha: u8) -> Option<ID2D1SolidColorBrush> {
        let key = (color.to_hex() << 8) | alpha as u32;
        let target = self.res.target.as_mut()?;
        if let Some(brush) = target.brushes.get(&key) {
            return Some(brush.clone());
        }
        let color = D2D1_COLOR_F {
            r: color.r as f32 / 255.0,
            g: color.g as f32 / 255.0,
            b: color.b as f32 / 255.0,
            a: alpha as f32 / 255.0,
        };
        let brush = unsafe { target.rt.CreateSolidColorBrush(&color, None) }.ok()?;
        if target.brushes.len() < MAX_BRUSHES {
            target.brushes.insert(key, brush.clone());
        }
        Some(brush)
    }

    fn rect_f(rect: Rect) -> D2D_RECT_F {
        D2D_RECT_F {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }

    /// A rounded-rect geometry for `rect`, cached by position and radius.
    fn rounded_geometry(&mut self, rect: Rect, radius: f32) -> Option<Geometry> {
        let radius = radius.max(0.0).min(rect.width().min(rect.height()) / 2.0);
        let key = GeometryKey::Rounded {
            left: rect.left.to_bits(),
            top: rect.top.to_bits(),
            width: rect.width().to_bits(),
            height: rect.height().to_bits(),
            radius: radius.to_bits(),
        };
        if let Some(geometry) = self.res.geometries.get(&key) {
            return Some(match geometry {
                Geometry::Rounded(geometry) => Geometry::Rounded(geometry.clone()),
                _ => return None,
            });
        }
        let factory = self.res.factory()?.clone();
        let rounded = D2D1_ROUNDED_RECT {
            rect: Self::rect_f(rect),
            radiusX: radius,
            radiusY: radius,
        };
        let geometry = unsafe { factory.CreateRoundedRectangleGeometry(&rounded) }.ok()?;
        self.insert_geometry(key, Geometry::Rounded(geometry.clone()));
        Some(Geometry::Rounded(geometry))
    }

    /// A per-corner rounded-rect geometry (a path), cached like the rounded
    /// variant.
    fn corners_geometry(&mut self, rect: Rect, corners: Corners) -> Option<Geometry> {
        let limit = rect.width().min(rect.height()) / 2.0;
        let corners = Corners::new(
            corners.top_left.clamp(0.0, limit),
            corners.top_right.clamp(0.0, limit),
            corners.bottom_right.clamp(0.0, limit),
            corners.bottom_left.clamp(0.0, limit),
        );
        let key = GeometryKey::Corners {
            left: rect.left.to_bits(),
            top: rect.top.to_bits(),
            width: rect.width().to_bits(),
            height: rect.height().to_bits(),
            corners: [
                corners.top_left.to_bits(),
                corners.top_right.to_bits(),
                corners.bottom_right.to_bits(),
                corners.bottom_left.to_bits(),
            ],
        };
        if let Some(Geometry::Path(geometry)) = self.res.geometries.get(&key) {
            return Some(Geometry::Path(geometry.clone()));
        }
        let factory = self.res.factory()?.clone();
        let geometry = rounded_path(&factory, rect, corners).ok()?;
        self.insert_geometry(key, Geometry::Path(geometry.clone()));
        Some(Geometry::Path(geometry))
    }

    /// The rounded ring (outer minus inset) used by `stroke_round_rect`.
    fn ring_geometry(&mut self, rect: Rect, radius: f32, thickness: f32) -> Option<Geometry> {
        let thickness = thickness.max(0.0).min(rect.width().min(rect.height()) / 2.0);
        let radius = radius.max(0.0).min(rect.width().min(rect.height()) / 2.0);
        let key = GeometryKey::Ring {
            left: rect.left.to_bits(),
            top: rect.top.to_bits(),
            width: rect.width().to_bits(),
            height: rect.height().to_bits(),
            radius: radius.to_bits(),
            thickness: thickness.to_bits(),
        };
        if let Some(Geometry::Path(geometry)) = self.res.geometries.get(&key) {
            return Some(Geometry::Path(geometry.clone()));
        }
        let factory = self.res.factory()?.clone();
        let inner = rect.inset(thickness, thickness);
        let shift = thickness;
        let inner_corners = Corners::new(
            (radius - thickness).max(0.0),
            (radius - thickness).max(0.0),
            (radius - thickness).max(0.0),
            (radius - thickness).max(0.0),
        );
        let geometry = ring_path(&factory, rect, radius, inner, inner_corners, shift).ok()?;
        self.insert_geometry(key, Geometry::Path(geometry.clone()));
        Some(Geometry::Path(geometry))
    }

    /// The per-corner rounded ring used by `stroke_round_rect_corners`.
    fn ring_corners_geometry(
        &mut self,
        rect: Rect,
        corners: Corners,
        thickness: f32,
    ) -> Option<Geometry> {
        let thickness = thickness.max(0.0).min(rect.width().min(rect.height()) / 2.0);
        let limit = rect.width().min(rect.height()) / 2.0;
        let corners = corners.map(|radius| radius.clamp(0.0, limit));
        let key = GeometryKey::RingCorners {
            left: rect.left.to_bits(),
            top: rect.top.to_bits(),
            width: rect.width().to_bits(),
            height: rect.height().to_bits(),
            corners: [
                corners.top_left.to_bits(),
                corners.top_right.to_bits(),
                corners.bottom_right.to_bits(),
                corners.bottom_left.to_bits(),
            ],
            thickness: thickness.to_bits(),
        };
        if let Some(Geometry::Path(geometry)) = self.res.geometries.get(&key) {
            return Some(Geometry::Path(geometry.clone()));
        }
        let factory = self.res.factory()?.clone();
        let inner = rect.inset(thickness, thickness);
        let inner_corners = corners.map(|radius| (radius - thickness).max(0.0));
        let geometry =
            ring_corners_path(&factory, rect, corners, inner, inner_corners).ok()?;
        self.insert_geometry(key, Geometry::Path(geometry.clone()));
        Some(Geometry::Path(geometry))
    }

    /// The ellipse inscribed in `rect` (a mask geometry for circular images).
    fn ellipse_geometry(&mut self, rect: Rect) -> Option<Geometry> {
        let key = GeometryKey::Ellipse {
            left: rect.left.to_bits(),
            top: rect.top.to_bits(),
            width: rect.width().to_bits(),
            height: rect.height().to_bits(),
        };
        if let Some(Geometry::Ellipse(geometry)) = self.res.geometries.get(&key) {
            return Some(Geometry::Ellipse(geometry.clone()));
        }
        let factory = self.res.factory()?.clone();
        let ellipse = D2D1_ELLIPSE {
            point: Vector2 {
                X: rect.center().x,
                Y: rect.center().y,
            },
            radiusX: rect.width() / 2.0,
            radiusY: rect.height() / 2.0,
        };
        let geometry = unsafe { factory.CreateEllipseGeometry(&ellipse) }.ok()?;
        self.insert_geometry(key, Geometry::Ellipse(geometry.clone()));
        Some(Geometry::Ellipse(geometry))
    }

    fn insert_geometry(&mut self, key: GeometryKey, geometry: Geometry) {
        if self.res.geometries.len() >= MAX_GEOMETRIES {
            self.res.geometries.clear();
        }
        self.res.geometries.insert(key, geometry);
    }

    /// Draw inside a geometry mask (rounded or circular images).
    fn masked_image(
        &mut self,
        rect: Rect,
        mask: Geometry,
        crop_square: bool,
        id: ImageId,
        data: ImageData<'_>,
    ) {
        let Some(bitmap) = self.image_bitmap(id, data) else {
            return;
        };
        let Some(rt) = self.rt().cloned() else {
            return;
        };
        let layer = match self.res.target.as_mut().and_then(|target| {
            if target.layer.is_none() {
                target.layer = unsafe { target.rt.CreateLayer(None) }.ok();
            }
            target.layer.clone()
        }) {
            Some(layer) => layer,
            None => return,
        };
        let geometry: ID2D1Geometry = mask.geometry().clone();
        let parameters = D2D1_LAYER_PARAMETERS {
            contentBounds: Self::rect_f(rect),
            geometricMask: std::mem::ManuallyDrop::new(Some(geometry)),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: 1.0,
            opacityBrush: std::mem::ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS_NONE,
        };
        unsafe { rt.PushLayer(&parameters, &layer) };
        // The mask interfaces are kept alive by the layer parameters until the
        // end of the scope; dropping them here is the windows-rs contract for
        // `ManuallyDrop` COM fields.
        let _ = std::mem::ManuallyDrop::into_inner(parameters.geometricMask);
        self.draw_bitmap(&bitmap, rect, crop_square);
        unsafe {
            rt.PopLayer();
        }
    }

    fn draw_bitmap(&mut self, bitmap: &ID2D1Bitmap, rect: Rect, crop_square: bool) {
        let Some(rt) = self.rt().cloned() else {
            return;
        };
        let source = unsafe { bitmap.GetSize() };
        let side = source.width.min(source.height).max(1.0);
        let source_rect = if crop_square {
            D2D_RECT_F {
                left: (source.width - side) / 2.0,
                top: (source.height - side) / 2.0,
                right: (source.width + side) / 2.0,
                bottom: (source.height + side) / 2.0,
            }
        } else {
            D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: source.width,
                bottom: source.height,
            }
        };
        let destination = Self::rect_f(rect);
        unsafe {
            rt.DrawBitmap(
                bitmap,
                Some(&destination),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                Some(&source_rect),
            );
        }
    }

    fn image_bitmap(&mut self, id: ImageId, data: ImageData<'_>) -> Option<ID2D1Bitmap> {
        let Some(target) = self.res.target.as_mut() else {
            return None;
        };
        if let Some(cached) = target.images.get(&id.0) {
            return Some(cached.bitmap.clone());
        }
        if data.width == 0 || data.height == 0 {
            return None;
        }
        let width = data.width as usize;
        let height = data.height as usize;
        if data.rgba.len() < width * height * 4 {
            return None;
        }
        // Direct2D premultiplied BGRA; the RGBA store is straight alpha.
        let mut bgra = vec![0u8; width * height * 4];
        for (source, destination) in data.rgba.chunks_exact(4).zip(bgra.chunks_exact_mut(4)) {
            let alpha = source[3] as u32;
            destination[0] = (source[2] as u32 * alpha / 255) as u8;
            destination[1] = (source[1] as u32 * alpha / 255) as u8;
            destination[2] = (source[0] as u32 * alpha / 255) as u8;
            destination[3] = source[3];
        }
        let properties = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
        };
        let size = D2D_SIZE_U {
            width: data.width,
            height: data.height,
        };
        let bitmap = match unsafe {
            target.rt.CreateBitmap(
                size,
                Some(bgra.as_ptr() as *const core::ffi::c_void),
                (width * 4) as u32,
                &properties,
            )
        } {
            Ok(bitmap) => bitmap,
            Err(error) => {
                eprintln!("d2d: CreateBitmap failed: {error:?}");
                return None;
            }
        };
        let bytes = bgra.len();
        target.images.insert(
            id.0,
            CachedBitmap {
                bitmap: bitmap.clone(),
                bytes,
            },
        );
        target.image_order.push_back(id.0);
        target.image_bytes += bytes;
        while target.image_bytes > IMAGE_BUDGET || target.images.len() > 64 {
            let Some(old) = target.image_order.pop_front() else {
                break;
            };
            if let Some(old) = target.images.remove(&old) {
                target.image_bytes = target.image_bytes.saturating_sub(old.bytes);
            }
        }
        Some(bitmap)
    }
}

impl D2dCanvas<'_> {
    /// Finish the frame (`EndDraw`). A failure — most often a lost device —
    /// drops the target so the next frame rebuilds it. Called by `Drop`; a
    /// test can call it early and inspect the result.
    pub fn finish(&mut self) -> windows::core::Result<()> {
        while self.clip_depth > 0 {
            self.clip_depth -= 1;
            if let Some(rt) = self.rt() {
                unsafe { rt.PopAxisAlignedClip() };
            }
        }
        let result = match self.rt() {
            Some(rt) => unsafe { rt.EndDraw(None, None) },
            None => Ok(()),
        };
        self.drawing = false;
        if result.is_err() {
            self.broken = true;
            self.res.drop_target();
        }
        result
    }
}

impl Drop for D2dCanvas<'_> {
    fn drop(&mut self) {
        if self.drawing {
            let _ = self.finish();
        }
    }
}

impl Canvas for D2dCanvas<'_> {
    fn scale(&self) -> f32 {
        self.scale
    }

    fn dpi(&self) -> f32 {
        self.dpi
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        if rect.is_empty() {
            return;
        }
        let Some(brush) = self.brush_with_alpha(color, 255) else {
            return;
        };
        if let Some(rt) = self.rt() {
            let rect = Self::rect_f(rect);
            unsafe { rt.FillRectangle(&rect, &brush) };
        }
    }

    fn fill_translucent(&mut self, rect: Rect, color: Color, alpha: f32) {
        if rect.is_empty() {
            return;
        }
        let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        let Some(brush) = self.brush_with_alpha(color, alpha) else {
            return;
        };
        if let Some(rt) = self.rt() {
            let rect = Self::rect_f(rect);
            unsafe { rt.FillRectangle(&rect, &brush) };
        }
    }

    fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        if rect.is_empty() {
            return;
        }
        let Some(geometry) = self.rounded_geometry(rect, radius) else {
            self.fill_rect(rect, color);
            return;
        };
        let Some(brush) = self.brush_with_alpha(color, 255) else {
            return;
        };
        if let Some(rt) = self.rt() {
            unsafe { rt.FillGeometry(geometry.geometry(), &brush, None::<&ID2D1Brush>) };
        }
    }

    fn fill_round_rect_corners(&mut self, rect: Rect, corners: Corners, color: Color) {
        if rect.is_empty() {
            return;
        }
        if corners.top_left == corners.top_right
            && corners.top_right == corners.bottom_right
            && corners.bottom_right == corners.bottom_left
        {
            self.fill_round_rect(rect, corners.top_left, color);
            return;
        }
        let Some(geometry) = self.corners_geometry(rect, corners) else {
            self.fill_round_rect(rect, corners.max(), color);
            return;
        };
        let Some(brush) = self.brush_with_alpha(color, 255) else {
            return;
        };
        if let Some(rt) = self.rt() {
            unsafe { rt.FillGeometry(geometry.geometry(), &brush, None::<&ID2D1Brush>) };
        }
    }

    fn stroke_round_rect(&mut self, rect: Rect, radius: f32, thickness: f32, color: Color) {
        if rect.is_empty() || thickness <= 0.0 {
            return;
        }
        let Some(geometry) = self.ring_geometry(rect, radius, thickness) else {
            return;
        };
        let Some(brush) = self.brush_with_alpha(color, 255) else {
            return;
        };
        if let Some(rt) = self.rt() {
            unsafe { rt.FillGeometry(geometry.geometry(), &brush, None::<&ID2D1Brush>) };
        }
    }

    fn stroke_round_rect_corners(
        &mut self,
        rect: Rect,
        corners: Corners,
        thickness: f32,
        color: Color,
    ) {
        if rect.is_empty() || thickness <= 0.0 {
            return;
        }
        let Some(geometry) = self.ring_corners_geometry(rect, corners, thickness) else {
            return;
        };
        let Some(brush) = self.brush_with_alpha(color, 255) else {
            return;
        };
        if let Some(rt) = self.rt() {
            unsafe { rt.FillGeometry(geometry.geometry(), &brush, None::<&ID2D1Brush>) };
        }
    }

    fn fill_circle(&mut self, rect: Rect, color: Color) {
        if rect.is_empty() {
            return;
        }
        let Some(brush) = self.brush_with_alpha(color, 255) else {
            return;
        };
        let ellipse = D2D1_ELLIPSE {
            point: Vector2 {
                X: rect.center().x,
                Y: rect.center().y,
            },
            radiusX: rect.width() / 2.0,
            radiusY: rect.height() / 2.0,
        };
        if let Some(rt) = self.rt() {
            unsafe { rt.FillEllipse(&ellipse, &brush) };
        }
    }

    fn text(&mut self, rect: Rect, text: &str, style: &TextStyle, align: TextAlign, color: Color) {
        self.text_overflow(rect, text, style, align, color, TextOverflow::Ellipsis);
    }

    fn text_overflow(
        &mut self,
        rect: Rect,
        text: &str,
        style: &TextStyle,
        align: TextAlign,
        color: Color,
        overflow: TextOverflow,
    ) {
        if rect.width() <= 1.0 || text.is_empty() {
            return;
        }
        let Some(format) = self.text_format(style, align, overflow) else {
            return;
        };
        let Some(brush) = self.brush_with_alpha(color, 255) else {
            return;
        };
        let Some(rt) = self.rt().cloned() else {
            return;
        };
        self.text_buf.clear();
        self.text_buf.extend(text.encode_utf16());
        let layout = Self::rect_f(rect);
        unsafe {
            let _ = rt.DrawText(
                &self.text_buf,
                &format,
                &layout,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    fn measure_text(&mut self, text: &str, style: &TextStyle) -> Size {
        if text.is_empty() {
            return Size::ZERO;
        }
        let Some(format) = self.text_format(style, TextAlign::Left, TextOverflow::Clip) else {
            return Size::ZERO;
        };
        let Some(dwrite) = self.res.dwrite.clone() else {
            return Size::ZERO;
        };
        let wide: Vec<u16> = text.encode_utf16().collect();
        let layout = unsafe { dwrite.CreateTextLayout(&wide, &format, 1_000_000.0, 1_000_000.0) };
        let Ok(layout) = layout else {
            return Size::ZERO;
        };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        let _ = unsafe { layout.GetMetrics(&mut metrics) };
        Size::new(metrics.widthIncludingTrailingWhitespace, metrics.height)
    }

    fn ellipse(&mut self, rect: Rect, color: Color) {
        self.fill_circle(rect, color);
    }

    fn vertical_gradient(&mut self, rect: Rect, top: Color, bottom: Color) {
        self.gradient(rect, None, top, bottom);
    }

    fn vertical_gradient_round(
        &mut self,
        rect: Rect,
        radius: f32,
        top: Color,
        bottom: Color,
    ) {
        let Some(geometry) = self.rounded_geometry(rect, radius) else {
            self.vertical_gradient(rect, top, bottom);
            return;
        };
        self.gradient(rect, Some(geometry), top, bottom);
    }

    fn image(&mut self, rect: Rect, crop_square: bool, id: ImageId, data: ImageData<'_>) {
        if rect.is_empty() {
            return;
        }
        let Some(bitmap) = self.image_bitmap(id, data) else {
            return;
        };
        self.draw_bitmap(&bitmap, rect, crop_square);
    }

    fn image_circle(&mut self, rect: Rect, crop_square: bool, id: ImageId, data: ImageData<'_>) {
        let Some(mask) = self.ellipse_geometry(rect) else {
            self.image(rect, crop_square, id, data);
            return;
        };
        self.masked_image(rect, mask, crop_square, id, data);
    }

    fn image_round(
        &mut self,
        rect: Rect,
        radius: f32,
        crop_square: bool,
        id: ImageId,
        data: ImageData<'_>,
    ) {
        if radius <= 0.0 {
            self.image(rect, crop_square, id, data);
            return;
        }
        let Some(mask) = self.rounded_geometry(rect, radius) else {
            self.image(rect, crop_square, id, data);
            return;
        };
        self.masked_image(rect, mask, crop_square, id, data);
    }

    fn push_clip(&mut self, rect: Rect) {
        let Some(rt) = self.rt().cloned() else {
            return;
        };
        let clip = Self::rect_f(rect);
        unsafe {
            rt.PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_ALIASED);
            self.clip_depth += 1;
        }
    }

    fn pop_clip(&mut self) {
        if self.clip_depth == 0 {
            return;
        }
        self.clip_depth -= 1;
        if let Some(rt) = self.rt() {
            unsafe { rt.PopAxisAlignedClip() };
        }
    }
}

impl D2dCanvas<'_> {
    /// A vertical gradient with an optional geometry mask (the rounded cover
    /// placeholder).
    fn gradient(&mut self, rect: Rect, mask: Option<Geometry>, top: Color, bottom: Color) {
        if rect.is_empty() {
            return;
        }
        let Some(rt) = self.rt().cloned() else {
            return;
        };
        let color = |color: Color| D2D1_COLOR_F {
            r: color.r as f32 / 255.0,
            g: color.g as f32 / 255.0,
            b: color.b as f32 / 255.0,
            a: 1.0,
        };
        let stops = [
            D2D1_GRADIENT_STOP {
                position: 0.0,
                color: color(top),
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: color(bottom),
            },
        ];
        let Ok(collection) =
            (unsafe { rt.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP) })
        else {
            return;
        };
        let properties = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
            startPoint: Vector2 {
                X: rect.left,
                Y: rect.top,
            },
            endPoint: Vector2 {
                X: rect.left,
                Y: rect.bottom,
            },
        };
        let Ok(linear) = (unsafe {
            rt.CreateLinearGradientBrush(&properties, None, &collection)
        }) else {
            return;
        };
        let Ok(brush): windows::core::Result<ID2D1Brush> = linear.cast() else {
            return;
        };
        match mask {
            Some(mask) => {
                let Some(layer) = self.res.target.as_mut().and_then(|target| {
                    if target.layer.is_none() {
                        target.layer = unsafe { target.rt.CreateLayer(None) }.ok();
                    }
                    target.layer.clone()
                }) else {
                    return;
                };
                let geometry: ID2D1Geometry = mask.geometry().clone();
                let parameters = D2D1_LAYER_PARAMETERS {
                    contentBounds: Self::rect_f(rect),
                    geometricMask: std::mem::ManuallyDrop::new(Some(geometry)),
                    maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                    maskTransform: Matrix3x2::identity(),
                    opacity: 1.0,
                    opacityBrush: std::mem::ManuallyDrop::new(None),
                    layerOptions: D2D1_LAYER_OPTIONS_NONE,
                };
                unsafe { rt.PushLayer(&parameters, &layer) };
                let _ = std::mem::ManuallyDrop::into_inner(parameters.geometricMask);
                let rect_f = Self::rect_f(rect);
                unsafe {
                    rt.FillRectangle(&rect_f, &brush);
                    rt.PopLayer();
                }
            }
            None => {
                let rect_f = Self::rect_f(rect);
                unsafe { rt.FillRectangle(&rect_f, &brush) };
            }
        }
    }
}

/// The theme family for a face, matching the GDI backend.
fn family_name(face: &FontFace) -> &'static str {
    match face {
        FontFace::Ui => "Microsoft YaHei UI",
        FontFace::Glyph => "Segoe MDL2 Assets",
        FontFace::Named(_) => "Microsoft YaHei UI",
    }
}

fn utf16(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A rounded rectangle path with independent corner radii, at `rect`.
fn rounded_path(factory: &ID2D1Factory, rect: Rect, corners: Corners) -> windows::core::Result<ID2D1PathGeometry> {
    let geometry = unsafe { factory.CreatePathGeometry() }?;
    let sink = unsafe { geometry.Open() }?;
    let [tl, tr, br, bl] = [
        corners.top_left,
        corners.top_right,
        corners.bottom_right,
        corners.bottom_left,
    ];
    let point = |x: f32, y: f32| Vector2 { X: x, Y: y };
    let arc = |to_x: f32, to_y: f32, radius: f32| D2D1_ARC_SEGMENT {
        point: point(to_x, to_y),
        size: D2D_SIZE_F {
            width: radius,
            height: radius,
        },
        rotationAngle: 0.0,
        sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
        arcSize: D2D1_ARC_SIZE_SMALL,
    };
    unsafe {
        sink.SetFillMode(D2D1_FILL_MODE_ALTERNATE);
        sink.BeginFigure(point(rect.left + tl, rect.top), D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(point(rect.right - tr, rect.top));
        if tr > 0.0 {
            sink.AddArc(&arc(rect.right, rect.top + tr, tr));
        }
        sink.AddLine(point(rect.right, rect.bottom - br));
        if br > 0.0 {
            sink.AddArc(&arc(rect.right - br, rect.bottom, br));
        }
        sink.AddLine(point(rect.left + bl, rect.bottom));
        if bl > 0.0 {
            sink.AddArc(&arc(rect.left, rect.bottom - bl, bl));
        }
        sink.AddLine(point(rect.left, rect.top + tl));
        if tl > 0.0 {
            sink.AddArc(&arc(rect.left + tl, rect.top, tl));
        }
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
    }
    Ok(geometry)
}

/// A rounded-ring path (outer rounded rect plus its inset, alternate fill).
fn ring_path(
    factory: &ID2D1Factory,
    outer: Rect,
    radius: f32,
    inner: Rect,
    inner_corners: Corners,
    _shift: f32,
) -> windows::core::Result<ID2D1PathGeometry> {
    let radius = radius.min(outer.width().min(outer.height()) / 2.0);
    ring_corners_path(factory, outer, Corners::uniform(radius), inner, inner_corners)
}

/// A rounded-ring path with independent outer corner radii.
fn ring_corners_path(
    factory: &ID2D1Factory,
    outer: Rect,
    outer_corners: Corners,
    inner: Rect,
    inner_corners: Corners,
) -> windows::core::Result<ID2D1PathGeometry> {
    let geometry = unsafe { factory.CreatePathGeometry() }?;
    let sink = unsafe { geometry.Open() }?;
    let point = |x: f32, y: f32| Vector2 { X: x, Y: y };
    let arc = |to_x: f32, to_y: f32, radius: f32| D2D1_ARC_SEGMENT {
        point: point(to_x, to_y),
        size: D2D_SIZE_F {
            width: radius,
            height: radius,
        },
        rotationAngle: 0.0,
        sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
        arcSize: D2D1_ARC_SIZE_SMALL,
    };
    let limit = outer.width().min(outer.height()) / 2.0;
    let outer_corners = outer_corners.map(|radius| radius.clamp(0.0, limit));
    unsafe {
        sink.SetFillMode(D2D1_FILL_MODE_ALTERNATE);
        let [tl, tr, br, bl] = [
            outer_corners.top_left,
            outer_corners.top_right,
            outer_corners.bottom_right,
            outer_corners.bottom_left,
        ];
        sink.BeginFigure(point(outer.left + tl, outer.top), D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(point(outer.right - tr, outer.top));
        if tr > 0.0 {
            sink.AddArc(&arc(outer.right, outer.top + tr, tr));
        }
        sink.AddLine(point(outer.right, outer.bottom - br));
        if br > 0.0 {
            sink.AddArc(&arc(outer.right - br, outer.bottom, br));
        }
        sink.AddLine(point(outer.left + bl, outer.bottom));
        if bl > 0.0 {
            sink.AddArc(&arc(outer.left, outer.bottom - bl, bl));
        }
        sink.AddLine(point(outer.left, outer.top + tl));
        if tl > 0.0 {
            sink.AddArc(&arc(outer.left + tl, outer.top, tl));
        }
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);

        let [tl, tr, br, bl] = [
            inner_corners.top_left,
            inner_corners.top_right,
            inner_corners.bottom_right,
            inner_corners.bottom_left,
        ];
        sink.BeginFigure(point(inner.left + tl, inner.top), D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(point(inner.right - tr, inner.top));
        if tr > 0.0 {
            sink.AddArc(&arc(inner.right, inner.top + tr, tr));
        }
        sink.AddLine(point(inner.right, inner.bottom - br));
        if br > 0.0 {
            sink.AddArc(&arc(inner.right - br, inner.bottom, br));
        }
        sink.AddLine(point(inner.left + bl, inner.bottom));
        if bl > 0.0 {
            sink.AddArc(&arc(inner.left, inner.bottom - bl, bl));
        }
        sink.AddLine(point(inner.left, inner.top + tl));
        if tl > 0.0 {
            sink.AddArc(&arc(inner.left + tl, inner.top, tl));
        }
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
    }
    Ok(geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::{
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, HGDIOBJ, SelectObject,
    };

    /// A `size × size` BGRA DIB plus the DC a canvas renders into.
    struct Surface {
        dc: HDC,
        bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
        old: HGDIOBJ,
        bits: *mut u8,
        size: i32,
    }

    impl Surface {
        fn new(size: i32) -> Self {
            unsafe {
                let dc = CreateCompatibleDC(None);
                assert!(!dc.is_invalid());
                let mut info = BITMAPINFO::default();
                info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = size;
                info.bmiHeader.biHeight = -size;
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB.0;
                let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
                let bitmap =
                    CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                        .expect("d2d test DIB");
                let old = SelectObject(dc, HGDIOBJ(bitmap.0));
                Self {
                    dc,
                    bitmap,
                    old,
                    bits: bits as *mut u8,
                    size,
                }
            }
        }

        fn pixel(&self, x: usize, y: usize) -> (u8, u8, u8, u8) {
            unsafe {
                let pixels =
                    std::slice::from_raw_parts(self.bits, (self.size * self.size * 4) as usize);
                let i = (y * self.size as usize + x) * 4;
                (pixels[i + 2], pixels[i + 1], pixels[i], pixels[i + 3])
            }
        }
    }

    impl Drop for Surface {
        fn drop(&mut self) {
            unsafe {
                let _ = SelectObject(self.dc, self.old);
                let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
                let _ = DeleteDC(self.dc);
            }
        }
    }

    /// The D2D factory needs COM; ignore S_FALSE (already initialized).
    fn com() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            );
        }
    }

    /// `true` when Direct2D could start here. The software DC target needs no
    /// GPU; a machine without d2d1.dll skips the pixel checks instead of
    /// failing the suite.
    fn canvas<'a>(
        res: &'a mut D2dResources,
        surface: &Surface,
    ) -> Option<D2dCanvas<'a>> {
        D2dCanvas::new(
            surface.dc,
            res,
            1.0,
            96.0,
            surface.size,
            surface.size,
            Renderer::D2dSoftware,
        )
    }

    #[test]
    fn fills_circles_and_gradients_land_in_the_back_buffer() {
        com();
        let surface = Surface::new(48);
        let mut res = D2dResources::new();
        let Some(mut canvas) = canvas(&mut res, &surface) else {
            eprintln!("Direct2D unavailable; skipping");
            return;
        };
        canvas.fill_rect(Rect::new(4.0, 4.0, 20.0, 20.0), Color::new(255, 0, 0));
        canvas.fill_round_rect(
            Rect::new(24.0, 4.0, 44.0, 24.0),
            6.0,
            Color::new(0, 0, 255),
        );
        canvas.fill_circle(Rect::new(4.0, 26.0, 20.0, 42.0), Color::new(0, 255, 0));
        canvas.vertical_gradient(
            Rect::new(24.0, 28.0, 44.0, 44.0),
            Color::new(255, 0, 0),
            Color::new(0, 0, 255),
        );
        canvas.finish().expect("EndDraw after fills");

        assert_eq!(surface.pixel(12, 12), (255, 0, 0, 255), "rect fill");
        assert_eq!(surface.pixel(34, 14), (0, 0, 255, 255), "rounded fill");
        assert_eq!(surface.pixel(24, 4).3, 0, "rounded corner stays clear");
        assert_eq!(surface.pixel(12, 34), (0, 255, 0, 255), "circle fill");
        assert_eq!(surface.pixel(4, 27).3, 0, "circle corner stays clear");
        let top = surface.pixel(34, 29);
        let bottom = surface.pixel(34, 42);
        assert!(top.0 > 200 && top.2 < 60, "gradient top is red: {top:?}");
        assert!(bottom.2 > 200 && bottom.0 < 60, "gradient bottom is blue: {bottom:?}");
    }

    #[test]
    fn a_rounded_frame_draws_an_outline_and_a_two_color_frame() {
        com();
        let surface = Surface::new(48);
        let mut res = D2dResources::new();
        let Some(mut canvas) = canvas(&mut res, &surface) else {
            eprintln!("Direct2D unavailable; skipping");
            return;
        };
        let rect = Rect::new(0.0, 0.0, 48.0, 48.0);
        canvas.fill_rect(rect, Color::new(255, 255, 255));
        // Transparent frame: a 1 DIP outline only, the interior keeps the page.
        canvas.stroke_round_rect_corners(
            rect,
            Corners::uniform(8.0),
            1.0,
            Color::new(0, 0, 0),
        );
        // Opaque frame below the outline's area: fill + rim.
        canvas.fill_round_frame(
            Rect::new(4.0, 4.0, 44.0, 44.0),
            Corners::new(8.0, 8.0, 0.0, 0.0),
            1.0,
            Color::new(0, 0, 0),
            Color::new(20, 160, 60),
        );
        canvas.finish().expect("EndDraw after frames");

        assert_eq!(surface.pixel(24, 2).0, 255, "outline interior is untouched");
        assert_eq!(surface.pixel(24, 0).0, 0, "top edge is the outline");
        assert_eq!(surface.pixel(0, 0).0, 255, "the rounded corner is untouched");
        let fill = surface.pixel(24, 24);
        assert!(
            fill.0 < 60 && fill.1 > 120 && fill.2 < 120,
            "inner fill: {fill:?}"
        );
        assert!(surface.pixel(24, 4).1 < 60, "the frame rim is the border");
        assert_eq!(surface.pixel(4, 4).0, 255, "the cut corner stays untouched");
    }

    #[test]
    fn clips_text_and_masked_images_render() {
        com();
        let surface = Surface::new(48);
        let mut res = D2dResources::new();
        let Some(mut canvas) = canvas(&mut res, &surface) else {
            eprintln!("Direct2D unavailable; skipping");
            return;
        };
        // Clip: a red fill under a 10..20 clip only paints inside it.
        canvas.push_clip(Rect::new(10.0, 10.0, 20.0, 20.0));
        canvas.fill_rect(Rect::new(0.0, 0.0, 48.0, 48.0), Color::new(255, 0, 0));
        canvas.pop_clip();

        // A rounded image: corners clear, centre the image color.
        let rgba = [0u8, 128, 255, 255].repeat(16 * 16);
        canvas.image_round(
            Rect::new(24.0, 24.0, 40.0, 40.0),
            4.0,
            true,
            ImageId(7),
            ImageData {
                rgba: &rgba,
                width: 16,
                height: 16,
            },
        );
        // Text: something must light up inside a reasonable band. GDI and D2D
        // shape glyphs differently, so only "drew something" is asserted.
        let style = TextStyle::ui(13.0);
        canvas.text(
            Rect::new(24.0, 2.0, 46.0, 22.0),
            "Ag",
            &style,
            TextAlign::Left,
            Color::new(0, 0, 0),
        );
        canvas.finish().expect("EndDraw after text");
        assert_eq!(surface.pixel(15, 15), (255, 0, 0, 255), "inside the clip");
        assert_eq!(surface.pixel(5, 5).3, 0, "outside the clip");
        let centre = surface.pixel(32, 32);
        assert!(
            centre.0 < 60 && centre.1 > 100 && centre.2 > 200,
            "image colour: {centre:?}"
        );
        assert_eq!(surface.pixel(24, 24).3, 0, "image corner stays clear");
        let painted = (0..48).any(|y| (24..46).any(|x| surface.pixel(x, y).3 > 0));
        assert!(painted, "text must paint into the back buffer");
    }

    #[test]
    fn text_and_measurement_are_close_to_gdi() {
        com();
        let surface = Surface::new(160);
        let mut res = D2dResources::new();
        let Some(mut canvas) = canvas(&mut res, &surface) else {
            eprintln!("Direct2D unavailable; skipping");
            return;
        };
        let style = TextStyle::ui(13.5).with_weight(crate::backend::FontWeight::Semibold);
        let rect = Rect::new(0.0, 0.0, 160.0, 160.0);
        canvas.fill_rect(rect, Color::new(255, 255, 255));
        canvas.text(
            Rect::new(8.0, 8.0, 152.0, 40.0),
            "Bamboo 竹韵",
            &style,
            TextAlign::Left,
            Color::new(0, 0, 0),
        );
        let width = canvas.measure_text("Bamboo 竹韵", &style).width;
        canvas.finish().expect("EndDraw after text");

        let ink = |surface: &Surface| {
            let mut count = 0;
            let mut right = 0;
            for y in 0..160usize {
                for x in 0..160usize {
                    let pixel = surface.pixel(x, y);
                    if pixel.0 < 128 && pixel.1 < 128 && pixel.2 < 128 {
                        count += 1;
                        right = right.max(x);
                    }
                }
            }
            (count, right)
        };
        let (count, right) = ink(&surface);
        assert!(count > 60, "D2D must paint text ({count} dark pixels)");
        assert!(right > 40, "the run must advance across the rect");

        // The GDI painter is the reference; both use the same family/size, so
        // the ink amount and the horizontal extent stay in the same ballpark
        // (ClearType and grayscale AA differ pixel by pixel).
        let gdi_surface = Surface::new(160);
        let mut gdi = crate::backend::gdi::GdiResources::new();
        {
            use crate::backend::gdi::GdiCanvas;
            let mut gdi_canvas = GdiCanvas::new(gdi_surface.dc, &mut gdi, 1.0, 96.0);
            gdi_canvas.fill_rect(rect, Color::new(255, 255, 255));
            gdi_canvas.text(
                Rect::new(8.0, 8.0, 152.0, 40.0),
                "Bamboo 竹韵",
                &style,
                TextAlign::Left,
                Color::new(0, 0, 0),
            );
        }
        let (gdi_count, gdi_right) = ink(&gdi_surface);
        assert!(gdi_count > 60, "GDI reference text ({gdi_count})");
        let ratio = count as f32 / gdi_count as f32;
        assert!(
            (0.5..2.0).contains(&ratio),
            "ink ratio D2D/GDI out of range: {count} vs {gdi_count}"
        );
        assert!(
            (right as i32 - gdi_right as i32).abs() < 24,
            "text extent differs: {right} vs {gdi_right}"
        );
        let gdi_width = {
            use crate::backend::gdi::GdiCanvas;
            let mut gdi_canvas = GdiCanvas::new(gdi_surface.dc, &mut gdi, 1.0, 96.0);
            gdi_canvas.measure_text("Bamboo 竹韵", &style).width
        };
        assert!(
            (width - gdi_width).abs() < gdi_width * 0.2 + 4.0,
            "measured width differs: {width} vs {gdi_width}"
        );
    }

    #[test]
    fn renderer_names_round_trip() {
        for renderer in [Renderer::Gdi, Renderer::D2dSoftware, Renderer::D2dHardware] {
            assert_eq!(Renderer::parse(renderer.name()), Some(renderer));
        }
        assert_eq!(Renderer::parse(""), Some(Renderer::Gdi));
        assert_eq!(Renderer::parse("D2D"), Some(Renderer::D2dHardware));
        assert_eq!(Renderer::parse("metal"), None);
    }
}
