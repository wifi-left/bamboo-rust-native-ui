// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! GDI backend: persistent double buffer, brush/font/image caches.
//!
//! This is the only module that talks to Win32 drawing APIs. It is deliberately
//! stateful: brushes, fonts and converted image pixels are cached for the
//! lifetime of the window, so a frame reuses them instead of recreating GDI
//! objects.

use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::hash::{Hash, Hasher};

use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, AlphaBlend, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap,
    CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush,
    DEFAULT_CHARSET, DIB_RGB_COLORS, DRAW_TEXT_FORMAT, DT_CALCRECT, DT_CENTER, DT_END_ELLIPSIS,
    DT_LEFT, DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW,
    Ellipse, FillRect, GRADIENT_FILL_RECT_V, GRADIENT_RECT, GetStockObject, GradientFill,
    HALFTONE, HBRUSH, HBITMAP, HDC, HFONT, HGDIOBJ, IntersectClipRect, NULL_PEN, OUT_DEFAULT_PRECIS,
    RestoreDC, SRCCOPY, SaveDC, SelectObject, SetBkMode, SetStretchBltMode,
    SetTextColor, StretchDIBits, TRANSPARENT, TRIVERTEX,
};
use windows::core::PCWSTR;

use crate::backend::{
    composite_round_rect, composite_round_stroke, Canvas, FontFace, ImageData, ImageId, TextAlign,
    TextStyle, TextOverflow,
};
use crate::color::Color;
use crate::geometry::{Corners, PxRect, Rect, Size};

/// Upper bound for converted image pixels (same budget as the player).
const IMAGE_CACHE_BUDGET: usize = 16 * 1024 * 1024;
/// Budget for anti-aliased rounded-corner masks (premultiplied BGRA).
const ROUND_MASK_BUDGET: usize = 8 * 1024 * 1024;
/// One byte per pixel, shared by every color of a geometry.
const COVERAGE_BUDGET: usize = 4 * 1024 * 1024;
/// Above this pixel *area* a rounded fill composes rects and ellipses instead
/// of building a mask. The budget is per area, not per dimension: a wide, short
/// shape is cheap (a 1900×56 field is 106k pixels), while a full-window rounded
/// rectangle is not a real use case. A per-dimension cap used to send every
/// widget wider than 1024 px down the aliased fallback, which drew a solid
/// border disc in each corner instead of an arc.
const MAX_ROUND_MASK_AREA: i64 = 1 << 20;

/// Whether a mask of `width × height` device pixels is too expensive to build.
fn mask_too_large(width: i32, height: i32) -> bool {
    width <= 0 || height <= 0 || (width as i64) * (height as i64) > MAX_ROUND_MASK_AREA
}

/// Cached GDI objects, owned for the lifetime of the window.
pub struct GdiResources {
    brushes: HashMap<u32, HBRUSH>,
    fonts: HashMap<FontKey, HFONT>,
    images: HashMap<u64, CachedImage>,
    image_order: VecDeque<u64>,
    image_bytes: usize,
    /// Anti-aliased rounded fills, keyed by pixel size, radius and color.
    masks: HashMap<MaskKey, CachedMask>,
    mask_order: VecDeque<MaskKey>,
    mask_bytes: usize,
    /// Coverage maps shared by every color of one rounded geometry. The mask
    /// build is the expensive part (supersampling); colorizing a cached map is
    /// O(pixels) with no geometry work, so a color change (a hover tint, a
    /// second badge tone) no longer pays the supersampling cost.
    coverages: HashMap<CoverageKey, std::rc::Rc<Vec<u8>>>,
    coverage_order: VecDeque<CoverageKey>,
    coverage_bytes: usize,
    /// Reused UTF-16 buffer for `DrawTextW`, so text drawing does not allocate.
    text_buf: Vec<u16>,
    /// One 1x1 premultiplied DIB/DC for translucent fills. Reusing it avoids
    /// creating and deleting a DC and bitmap for every scrim paint.
    alpha: Option<AlphaSource>,
    /// Scratch pixels for masked images (an avatar circle, rounded cover art):
    /// the image is resampled here with the anti-aliased coverage as alpha and
    /// then blended through `image_surface`. Both are reused, so an idle cover
    /// repaint pays no allocation.
    masked: Vec<u8>,
    image_surface: Option<ImageSurface>,
}

impl Default for GdiResources {
    fn default() -> Self {
        Self::new()
    }
}

impl GdiResources {
    pub fn new() -> Self {
        Self {
            brushes: HashMap::new(),
            fonts: HashMap::new(),
            images: HashMap::new(),
            image_order: VecDeque::new(),
            image_bytes: 0,
            masks: HashMap::new(),
            mask_order: VecDeque::new(),
            mask_bytes: 0,
            coverages: HashMap::new(),
            coverage_order: VecDeque::new(),
            coverage_bytes: 0,
            text_buf: Vec::new(),
            alpha: None,
            masked: Vec::new(),
            image_surface: None,
        }
    }

    /// Anti-aliased coverage for one shape, shared by every color.
    fn coverage(
        &mut self,
        width: usize,
        height: usize,
        shape: CoverageShape,
    ) -> std::rc::Rc<Vec<u8>> {
        let key = CoverageKey {
            width,
            height,
            shape,
        };
        if let Some(coverage) = self.coverages.get(&key) {
            return coverage.clone();
        }
        let coverage = std::rc::Rc::new(shape_coverage(width, height, shape, 4));
        self.coverages.insert(key, coverage.clone());
        self.coverage_order.push_back(key);
        self.coverage_bytes += coverage.len();
        while self.coverage_bytes > COVERAGE_BUDGET || self.coverages.len() > 256 {
            let Some(old_key) = self.coverage_order.pop_front() else {
                break;
            };
            if let Some(old) = self.coverages.remove(&old_key) {
                self.coverage_bytes = self.coverage_bytes.saturating_sub(old.len());
            }
        }
        coverage
    }

    /// Lazily build (and cache) the premultiplied BGRA mask for one rounded
    /// shape. Keys include the pixel size, the radius and the color, so a
    /// repeated checkbox/badge/thumb costs no mask work on idle repaints.
    fn round_mask(&mut self, reference: HDC, key: MaskKey) -> Option<&CachedMask> {
        if !self.masks.contains_key(&key) {
            let width = key.width as usize;
            let height = key.height as usize;
            if width == 0 || height == 0 {
                return None;
            }
            let coverage = self.coverage(width, height, key.shape);
            let colorref = key.color;
            let (r, g, b) = (
                (colorref & 0xFF) as u32,
                ((colorref >> 8) & 0xFF) as u32,
                ((colorref >> 16) & 0xFF) as u32,
            );
            // A frame bakes the border and the inner fill into one mask: the
            // outer coverage is the alpha, the inner rounded coverage tells
            // which pixels take the fill color.
            let inner = match (key.fill, key.shape) {
                (Some(_), CoverageShape::Frame { corners, thickness }) => {
                    let inner_corners = corners.map(|radius| (radius - thickness).max(0));
                    Some(self.coverage(
                        width,
                        height,
                        CoverageShape::CornersInset {
                            corners: inner_corners,
                            inset: thickness,
                        },
                    ))
                }
                _ => None,
            };
            let (fr, fg, fb) = match key.fill {
                Some(fill) => (
                    (fill & 0xFF) as u32,
                    ((fill >> 8) & 0xFF) as u32,
                    ((fill >> 16) & 0xFF) as u32,
                ),
                None => (0, 0, 0),
            };
            let mut bgra = vec![0u8; width * height * 4];
            for (index, &outer) in coverage.iter().enumerate() {
                let alpha = outer as u32;
                if alpha == 0 {
                    continue;
                }
                let pixel = &mut bgra[index * 4..index * 4 + 4];
                if let Some(inner) = inner.as_ref() {
                    let inner = (inner[index] as u32).min(alpha);
                    let ring = alpha - inner;
                    pixel[0] = ((b * ring + fb * inner) / 255) as u8;
                    pixel[1] = ((g * ring + fg * inner) / 255) as u8;
                    pixel[2] = ((r * ring + fr * inner) / 255) as u8;
                } else {
                    pixel[0] = ((b * alpha) / 255) as u8;
                    pixel[1] = ((g * alpha) / 255) as u8;
                    pixel[2] = ((r * alpha) / 255) as u8;
                }
                pixel[3] = outer;
            }
            drop(coverage);
            let dc = unsafe { CreateCompatibleDC(Some(reference)) };
            if dc.is_invalid() {
                return None;
            }
            let mut info = BITMAPINFO::default();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = key.width;
            info.bmiHeader.biHeight = -key.height;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB.0;
            let mut bits: *mut c_void = std::ptr::null_mut();
            let Ok(bitmap) = (unsafe {
                CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
            }) else {
                unsafe {
                    let _ = DeleteDC(dc);
                }
                return None;
            };
            if bits.is_null() {
                unsafe {
                    let _ = DeleteObject(HGDIOBJ(bitmap.0));
                    let _ = DeleteDC(dc);
                }
                return None;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());
            }
            let old = unsafe { SelectObject(dc, HGDIOBJ(bitmap.0)) };
            let bytes = bgra.len();
            self.masks.insert(
                key,
                CachedMask {
                    dc,
                    bitmap,
                    old,
                    bytes,
                },
            );
            self.mask_order.push_back(key);
            self.mask_bytes += bytes;
            while self.mask_bytes > ROUND_MASK_BUDGET || self.masks.len() > 128 {
                let Some(old_key) = self.mask_order.pop_front() else {
                    break;
                };
                if let Some(old) = self.masks.remove(&old_key) {
                    self.mask_bytes = self.mask_bytes.saturating_sub(old.bytes);
                    unsafe {
                        let _ = SelectObject(old.dc, old.old);
                        let _ = DeleteObject(HGDIOBJ(old.bitmap.0));
                        let _ = DeleteDC(old.dc);
                    }
                }
            }
        }
        self.masks.get(&key)
    }

    /// Lazily created 1x1 alpha source, reused by every `fill_translucent`.
    fn alpha_source(&mut self, reference: HDC) -> Option<&mut AlphaSource> {
        if self.alpha.is_none() {
            unsafe {
                let dc = CreateCompatibleDC(Some(reference));
                if dc.is_invalid() {
                    return None;
                }
                let mut info = BITMAPINFO::default();
                info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = 1;
                info.bmiHeader.biHeight = -1;
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB.0;
                let mut bits: *mut c_void = std::ptr::null_mut();
                let Ok(bitmap) =
                    CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                else {
                    let _ = DeleteDC(dc);
                    return None;
                };
                if bits.is_null() {
                    let _ = DeleteObject(HGDIOBJ(bitmap.0));
                    let _ = DeleteDC(dc);
                    return None;
                }
                let old = SelectObject(dc, HGDIOBJ(bitmap.0));
                self.alpha = Some(AlphaSource {
                    dc,
                    bitmap,
                    old,
                    bits: bits as *mut u8,
                    color: u32::MAX,
                    alpha: 0,
                });
            }
        }
        self.alpha.as_mut()
    }

    fn brush(&mut self, color: Color) -> HBRUSH {
        let key = color.to_colorref();
        if let Some(brush) = self.brushes.get(&key) {
            return *brush;
        }
        let brush = unsafe { CreateSolidBrush(COLORREF(key)) };
        self.brushes.insert(key, brush);
        brush
    }

    /// Cached solid brush for owner-drawn backgrounds (`WM_CTLCOLOREDIT`).
    pub(crate) fn area_brush(&mut self, color: Color) -> HBRUSH {
        self.brush(color)
    }

    fn font(&mut self, style: &TextStyle, size_px: i32, family: &str) -> HFONT {
        let key = FontKey {
            size_px,
            weight: style.weight.to_windows(),
            italic: style.italic,
            glyph: matches!(style.face, FontFace::Glyph),
            family_hash: hash_str(family),
        };
        if let Some(font) = self.fonts.get(&key) {
            return *font;
        }
        let wide: Vec<u16> = family.encode_utf16().chain([0]).collect();
        let font = unsafe {
            CreateFontW(
                -size_px.max(1),
                0,
                0,
                0,
                key.weight,
                if key.italic { 1 } else { 0 },
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                PCWSTR(wide.as_ptr()),
            )
        };
        self.fonts.insert(key, font);
        font
    }

    /// Convert RGBA to BGRA once per image id and keep it within the budget.
    fn cached_image(&mut self, id: ImageId, data: ImageData<'_>) -> Option<&CachedImage> {
        if !self.images.contains_key(&id.0) {
            let pixels = (data.width as usize) * (data.height as usize);
            if pixels == 0 || data.rgba.len() < pixels.saturating_mul(4) {
                return None;
            }
            let mut bgra = Vec::with_capacity(pixels * 4);
            for chunk in data.rgba[..pixels * 4].chunks_exact(4) {
                bgra.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
            }
            let image = CachedImage {
                bgra,
                width: data.width as i32,
                height: data.height as i32,
            };
            self.image_bytes += image.bgra.len();
            self.images.insert(id.0, image);
            self.image_order.push_back(id.0);
            while self.image_bytes > IMAGE_CACHE_BUDGET {
                let Some(old) = self.image_order.pop_front() else {
                    break;
                };
                if let Some(old) = self.images.remove(&old) {
                    self.image_bytes = self.image_bytes.saturating_sub(old.bgra.len());
                }
            }
        }
        self.images.get(&id.0)
    }

    /// A `width × height` BGRA image surface, recreated when the size changes.
    fn image_surface(&mut self, width: i32, height: i32) -> Option<&ImageSurface> {
        if width <= 0 || height <= 0 {
            return None;
        }
        if !self
            .image_surface
            .as_ref()
            .is_some_and(|surface| surface.width == width && surface.height == height)
        {
            if let Some(surface) = self.image_surface.take() {
                unsafe {
                    let _ = SelectObject(surface.dc, surface.old);
                    let _ = DeleteObject(HGDIOBJ(surface.bitmap.0));
                    let _ = DeleteDC(surface.dc);
                }
            }
            let surface = unsafe {
                let dc = CreateCompatibleDC(None);
                if dc.is_invalid() {
                    return None;
                }
                let mut info = BITMAPINFO::default();
                info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = width;
                info.bmiHeader.biHeight = -height;
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB.0;
                let mut bits: *mut c_void = std::ptr::null_mut();
                let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                else {
                    let _ = DeleteDC(dc);
                    return None;
                };
                if bits.is_null() {
                    let _ = DeleteObject(HGDIOBJ(bitmap.0));
                    let _ = DeleteDC(dc);
                    return None;
                }
                let old = SelectObject(dc, HGDIOBJ(bitmap.0));
                ImageSurface {
                    dc,
                    bitmap,
                    old,
                    bits: bits as *mut u8,
                    width,
                    height,
                }
            };
            self.image_surface = Some(surface);
        }
        self.image_surface.as_ref()
    }

    pub fn clear_image_cache(&mut self) {
        self.images.clear();
        self.image_order.clear();
        self.image_bytes = 0;
    }

    /// Drop cached fonts; call on DPI or theme font changes.
    pub fn clear_fonts(&mut self) {
        for (_, font) in self.fonts.drain() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(font.0));
            }
        }
    }
}

impl Drop for GdiResources {
    fn drop(&mut self) {
        unsafe {
            if let Some(source) = self.alpha.take() {
                let _ = SelectObject(source.dc, source.old);
                let _ = DeleteObject(HGDIOBJ(source.bitmap.0));
                let _ = DeleteDC(source.dc);
            }
            if let Some(surface) = self.image_surface.take() {
                let _ = SelectObject(surface.dc, surface.old);
                let _ = DeleteObject(HGDIOBJ(surface.bitmap.0));
                let _ = DeleteDC(surface.dc);
            }
            for (_, brush) in self.brushes.drain() {
                let _ = DeleteObject(HGDIOBJ(brush.0));
            }
            for (_, font) in self.fonts.drain() {
                let _ = DeleteObject(HGDIOBJ(font.0));
            }
            for (_, mask) in self.masks.drain() {
                let _ = SelectObject(mask.dc, mask.old);
                let _ = DeleteObject(HGDIOBJ(mask.bitmap.0));
                let _ = DeleteDC(mask.dc);
            }
        }
    }
}

/// One reusable 1x1 premultiplied BGRA source for `AlphaBlend`.
struct AlphaSource {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    /// `COLORREF` and alpha currently written into the pixel.
    color: u32,
    alpha: u8,
}

/// One reusable 32-bit DIB/DC that masked images are blended from. Recreated
/// only when the requested pixel size changes (all covers in a row share one).
struct ImageSurface {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    width: i32,
    height: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct FontKey {
    size_px: i32,
    weight: i32,
    italic: bool,
    glyph: bool,
    family_hash: u64,
}

struct CachedImage {
    bgra: Vec<u8>,
    width: i32,
    height: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MaskKey {
    width: i32,
    height: i32,
    shape: CoverageShape,
    color: u32,
    /// Inner fill color for `CoverageShape::Frame`; `None` for a plain shape.
    fill: Option<u32>,
}

/// The geometry a coverage map describes. The mask cache is keyed by it, so a
/// spinner's dot costs one coverage map per pixel size, not one per frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum CoverageShape {
    /// Rounded rectangle with `[top-left, top-right, bottom-right, bottom-left]`
    /// radii in pixels.
    Corners([i32; 4]),
    /// A rounded outline: uniformly rounded rect minus its inset.
    Ring { radius: i32, thickness: i32 },
    /// A rounded outline with independent corner radii.
    RingCorners { corners: [i32; 4], thickness: i32 },
    /// The inset rounded rectangle used as a frame's inner coverage.
    CornersInset { corners: [i32; 4], inset: i32 },
    /// A rounded frame: the outer coverage is used for the mask alpha, the
    /// inner rounded coverage separates the border color from the fill color
    /// (the two colors are baked into one mask, see `round_mask`).
    Frame { corners: [i32; 4], thickness: i32 },
    /// The circle inscribed in the bitmap.
    Circle,
}

/// Geometry (no color) of a cached coverage map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct CoverageKey {
    width: usize,
    height: usize,
    shape: CoverageShape,
}

fn shape_coverage(
    width: usize,
    height: usize,
    shape: CoverageShape,
    samples: usize,
) -> Vec<u8> {
    match shape {
        CoverageShape::Corners(corners) => corners_coverage(
            width,
            height,
            [
                corners[0] as f32,
                corners[1] as f32,
                corners[2] as f32,
                corners[3] as f32,
            ],
            samples,
        ),
        CoverageShape::Circle => circle_coverage(width, height, samples),
        CoverageShape::Ring { radius, thickness } => ring_coverage(
            width,
            height,
            radius as f32,
            thickness as f32,
            samples,
        ),
        CoverageShape::RingCorners { corners, thickness } => ring_corners_coverage(
            width,
            height,
            [
                corners[0] as f32,
                corners[1] as f32,
                corners[2] as f32,
                corners[3] as f32,
            ],
            thickness as f32,
            samples,
        ),
        // The outer shape's coverage: the frame mask keeps the inner coverage
        // separately for the fill color.
        CoverageShape::Frame { corners, thickness: _ } => corners_coverage(
            width,
            height,
            [
                corners[0] as f32,
                corners[1] as f32,
                corners[2] as f32,
                corners[3] as f32,
            ],
            samples,
        ),
        CoverageShape::CornersInset { corners, inset } => {
            let inset = inset.max(0) as f32;
            let inner_width = (width as f32 - inset * 2.0).max(0.0);
            let inner_height = (height as f32 - inset * 2.0).max(0.0);
            let corners = [
                corners[0] as f32,
                corners[1] as f32,
                corners[2] as f32,
                corners[3] as f32,
            ];
            supersampled(width, height, samples, |x, y| {
                if inner_width <= 0.0 || inner_height <= 0.0 {
                    return false;
                }
                rounded_contains_corners(
                    x - inset,
                    y - inset,
                    inner_width,
                    inner_height,
                    corners,
                )
            })
        }
    }
}

/// One premultiplied BGRA rounded-rectangle mask plus the DC that owns it.
struct CachedMask {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bytes: usize,
}

/// Anti-aliased coverage (0..=255) of a rounded rectangle, row-major and
/// top-down. Each pixel is supersampled `samples × samples` times; the radius
/// is clamped to half the shorter side. This is the uniform-radius helper the
/// tests use; the painter goes through [`shape_coverage`].
#[cfg(test)]
fn rounded_coverage(width: usize, height: usize, radius: f32, samples: usize) -> Vec<u8> {
    corners_coverage(width, height, [radius; 4], samples)
}

/// Coverage of a rounded rectangle with independent corner radii
/// (`[top-left, top-right, bottom-right, bottom-left]`). A convex shape is
/// fully covered wherever its four pixel corners agree, so only the arcs are
/// supersampled.
fn corners_coverage(width: usize, height: usize, corners: [f32; 4], samples: usize) -> Vec<u8> {
    let width_f = width as f32;
    let height_f = height as f32;
    let limit = (width_f.min(height_f) / 2.0).max(0.0);
    let corners = corners.map(|radius| radius.clamp(0.0, limit));
    supersampled(width, height, samples, |x, y| {
        rounded_contains_corners(x, y, width_f, height_f, corners)
    })
}

/// Anti-aliased coverage of the circle inscribed in the bitmap.
fn circle_coverage(width: usize, height: usize, samples: usize) -> Vec<u8> {
    let width_f = width as f32;
    let height_f = height as f32;
    let radius = width_f.min(height_f) / 2.0;
    let cx = width_f / 2.0;
    let cy = height_f / 2.0;
    supersampled(width, height, samples, |x, y| {
        let dx = x - cx;
        let dy = y - cy;
        dx * dx + dy * dy <= radius * radius
    })
}

/// Anti-aliased coverage of a rounded outline: `thickness` DIPs of the border
/// are opaque, the inset area is clear.
fn ring_coverage(
    width: usize,
    height: usize,
    radius: f32,
    thickness: f32,
    samples: usize,
) -> Vec<u8> {
    let width_f = width as f32;
    let height_f = height as f32;
    let thickness = thickness.max(0.0);
    let radius = radius.max(0.0).min(width_f.min(height_f) / 2.0);
    let inner_width = (width_f - thickness * 2.0).max(0.0);
    let inner_height = (height_f - thickness * 2.0).max(0.0);
    let inner_radius = (radius - thickness).max(0.0);
    supersampled(width, height, samples, |x, y| {
        if !rounded_contains_corners(x, y, width_f, height_f, [radius; 4]) {
            return false;
        }
        if inner_width <= 0.0 || inner_height <= 0.0 {
            return true;
        }
        !rounded_contains_corners(
            x - thickness,
            y - thickness,
            inner_width,
            inner_height,
            [inner_radius; 4],
        )
    })
}

/// Anti-aliased coverage of a rounded outline with independent corner radii.
fn ring_corners_coverage(
    width: usize,
    height: usize,
    corners: [f32; 4],
    thickness: f32,
    samples: usize,
) -> Vec<u8> {
    let width_f = width as f32;
    let height_f = height as f32;
    let thickness = thickness.max(0.0);
    let limit = width_f.min(height_f) / 2.0;
    let corners = corners.map(|radius| radius.clamp(0.0, limit));
    let inner_width = (width_f - thickness * 2.0).max(0.0);
    let inner_height = (height_f - thickness * 2.0).max(0.0);
    let inner_corners = corners.map(|radius| (radius - thickness).max(0.0));
    supersampled(width, height, samples, |x, y| {
        if !rounded_contains_corners(x, y, width_f, height_f, corners) {
            return false;
        }
        if inner_width <= 0.0 || inner_height <= 0.0 {
            return true;
        }
        !rounded_contains_corners(
            x - thickness,
            y - thickness,
            inner_width,
            inner_height,
            inner_corners,
        )
    })
}

/// Rasterize a coverage predicate with the convex fast path and `samples²`
/// supersampling on edge pixels.
fn supersampled(
    width: usize,
    height: usize,
    samples: usize,
    contains: impl Fn(f32, f32) -> bool,
) -> Vec<u8> {
    let mut coverage = vec![0u8; width * height];
    if width == 0 || height == 0 {
        return coverage;
    }
    let samples = samples.max(1);
    let total = (samples * samples) as f32;
    let step = 1.0 / samples as f32;
    for y in 0..height {
        let y0 = y as f32;
        let y1 = y0 + 1.0;
        for x in 0..width {
            let x0 = x as f32;
            let x1 = x0 + 1.0;
            if contains(x0, y0) && contains(x1, y0) && contains(x0, y1) && contains(x1, y1) {
                coverage[y * width + x] = 255;
                continue;
            }
            let mut inside = 0u32;
            for sy in 0..samples {
                for sx in 0..samples {
                    let px = x as f32 + (sx as f32 + 0.5) * step;
                    let py = y as f32 + (sy as f32 + 0.5) * step;
                    if contains(px, py) {
                        inside += 1;
                    }
                }
            }
            coverage[y * width + x] = ((inside as f32 / total) * 255.0).round() as u8;
        }
    }
    coverage
}

fn rounded_contains_corners(x: f32, y: f32, width: f32, height: f32, r: [f32; 4]) -> bool {
    if x < 0.0 || y < 0.0 || x > width || y > height {
        return false;
    }
    let [tl, tr, br, bl] = r;
    if x < tl && y < tl {
        return (x - tl) * (x - tl) + (y - tl) * (y - tl) <= tl * tl;
    }
    if x > width - tr && y < tr {
        return (x - (width - tr)) * (x - (width - tr)) + (y - tr) * (y - tr) <= tr * tr;
    }
    if x > width - br && y > height - br {
        return (x - (width - br)) * (x - (width - br))
            + (y - (height - br)) * (y - (height - br))
            <= br * br;
    }
    if x < bl && y > height - bl {
        return (x - bl) * (x - bl) + (y - (height - bl)) * (y - (height - bl)) <= bl * bl;
    }
    true
}

fn hash_str(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// A persistent GDI back buffer. Only the dirty bounding box is blitted.
pub struct GdiDoubleBuffer {
    dc: Option<HDC>,
    bitmap: Option<HBITMAP>,
    old: Option<HGDIOBJ>,
    size: (i32, i32),
}

impl Default for GdiDoubleBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl GdiDoubleBuffer {
    pub fn new() -> Self {
        Self {
            dc: None,
            bitmap: None,
            old: None,
            size: (0, 0),
        }
    }

    /// Create or resize the back buffer to `width` x `height` physical pixels.
    pub fn ensure(&mut self, reference: HDC, width: i32, height: i32) {
        if self.dc.is_some() && self.size == (width, height) {
            return;
        }
        self.release();
        unsafe {
            let dc = CreateCompatibleDC(Some(reference));
            if dc.is_invalid() {
                return;
            }
            let bitmap = CreateCompatibleBitmap(reference, width, height);
            if bitmap.is_invalid() {
                let _ = DeleteDC(dc);
                return;
            }
            let old = SelectObject(dc, HGDIOBJ(bitmap.0));
            self.dc = Some(dc);
            self.bitmap = Some(bitmap);
            self.old = Some(old);
            self.size = (width, height);
        }
    }

    pub fn dc(&self) -> Option<HDC> {
        self.dc
    }

    pub fn release(&mut self) {
        unsafe {
            if let Some(dc) = self.dc.take() {
                if let Some(old) = self.old.take() {
                    let _ = SelectObject(dc, old);
                }
                let _ = DeleteDC(dc);
            }
            if let Some(bitmap) = self.bitmap.take() {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
            self.size = (0, 0);
        }
    }

    /// Copy the dirty bounding box from the back buffer to the window DC.
    pub fn blit(&self, target: HDC, bounds: PxRect) {
        if bounds.is_empty() {
            return;
        }
        let Some(back) = self.dc else {
            return;
        };
        unsafe {
            let _ = BitBlt(
                target,
                bounds.left,
                bounds.top,
                bounds.width(),
                bounds.height(),
                Some(back),
                bounds.left,
                bounds.top,
                SRCCOPY,
            );
        }
    }
}

impl Drop for GdiDoubleBuffer {
    fn drop(&mut self) {
        self.release();
    }
}

/// A `Canvas` over one HDC (the back buffer during painting).
pub struct GdiCanvas<'a> {
    hdc: HDC,
    res: &'a mut GdiResources,
    scale: f32,
    dpi: f32,
    clip_depth: usize,
}

impl<'a> GdiCanvas<'a> {
    pub fn new(hdc: HDC, res: &'a mut GdiResources, scale: f32, dpi: f32) -> Self {
        unsafe {
            SetBkMode(hdc, TRANSPARENT);
        }
        Self {
            hdc,
            res,
            scale,
            dpi,
            clip_depth: 0,
        }
    }

    fn px(&self, rect: Rect) -> RECT {
        let px = rect.to_px(self.scale);
        RECT {
            left: px.left,
            top: px.top,
            right: px.right,
            bottom: px.bottom,
        }
    }

    /// AlphaBlend a cached coverage mask of `shape` into `rect`. The mask
    /// (BGRA, premultiplied) is cached per pixel size, shape and color, so a
    /// repeated dot costs one GDI call and no rasterization. Returns whether
    /// the mask was available; oversized shapes are left to the caller's
    /// fallback.
    fn blend_shape(&mut self, rect: Rect, shape: CoverageShape, color: Color) -> bool {
        let px = self.px(rect);
        let width = px.right - px.left;
        let height = px.bottom - px.top;
        if mask_too_large(width, height) {
            return false;
        }
        let key = MaskKey {
            width,
            height,
            shape,
            color: color.to_colorref(),
            fill: None,
        };
        self.blend_mask(rect, key)
    }

    /// AlphaBlend a cached mask keyed by `key` into `rect`.
    fn blend_mask(&mut self, rect: Rect, key: MaskKey) -> bool {
        if mask_too_large(key.width, key.height) {
            return false;
        }
        let px = self.px(rect);
        let width = key.width;
        let height = key.height;
        let hdc = self.hdc;
        let Some(mask) = self.res.round_mask(hdc, key) else {
            return false;
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        unsafe {
            let _ = AlphaBlend(
                hdc,
                px.left,
                px.top,
                width,
                height,
                mask.dc,
                0,
                0,
                width,
                height,
                blend,
            );
        }
        true
    }

    /// Resample an image into a premultiplied BGRA buffer whose alpha is the
    /// image alpha times the anti-aliased `shape` coverage, then AlphaBlend it
    /// (so the outside really is transparent, not painted over). The coverage
    /// map comes from the shared cache, and the pixels go through one reused
    /// DIB, so a repaint costs no geometry work and no GDI object churn.
    fn image_masked(
        &mut self,
        rect: Rect,
        shape: CoverageShape,
        crop_square: bool,
        id: ImageId,
        data: ImageData<'_>,
    ) {
        let px = self.px(rect);
        let width = px.right - px.left;
        let height = px.bottom - px.top;
        if mask_too_large(width, height) {
            return;
        }
        let coverage = self.res.coverage(width as usize, height as usize, shape);
        if self.res.cached_image(id, data).is_none() {
            return;
        }
        let GdiResources {
            images, masked, ..
        } = &mut *self.res;
        let Some(pixels) = images.get(&id.0) else {
            return;
        };
        let side = pixels.width.min(pixels.height).max(1);
        let (src_x, src_y, src_w, src_h) = if crop_square {
            (
                (pixels.width - side) / 2,
                (pixels.height - side) / 2,
                side,
                side,
            )
        } else {
            (0, 0, pixels.width, pixels.height)
        };
        let len = (width as usize) * (height as usize) * 4;
        masked.clear();
        masked.resize(len, 0);
        for y in 0..height as usize {
            let sy = src_y as usize + y * src_h as usize / height as usize;
            for x in 0..width as usize {
                let sx = src_x as usize + x * src_w as usize / width as usize;
                let source = (sy * pixels.width as usize + sx) * 4;
                let destination = (y * width as usize + x) * 4;
                let mask = coverage[y * width as usize + x] as u32;
                let alpha = pixels.bgra[source + 3] as u32 * mask / 255;
                if alpha == 0 {
                    continue;
                }
                for channel in 0..3 {
                    masked[destination + channel] =
                        (pixels.bgra[source + channel] as u32 * alpha / 255) as u8;
                }
                masked[destination + 3] = alpha as u8;
            }
        }
        self.blend_masked(px, width, height);
    }

    /// AlphaBlend `res.masked` (premultiplied BGRA, `width × height`) into
    /// `px` through the reused image surface. Shared by masked images and the
    /// rounded gradient placeholder.
    fn blend_masked(&mut self, px: RECT, width: i32, height: i32) {
        let res = &mut *self.res;
        if res.image_surface(width, height).is_none() {
            return;
        }
        let GdiResources {
            image_surface,
            masked,
            ..
        } = res;
        let Some(surface) = image_surface.as_ref() else {
            return;
        };
        unsafe {
            std::ptr::copy_nonoverlapping(masked.as_ptr(), surface.bits, masked.len());
        }
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let hdc = self.hdc;
        unsafe {
            let _ = AlphaBlend(
                hdc, px.left, px.top, width, height, surface.dc, 0, 0, width, height, blend,
            );
        }
    }

    /// Vertical gradient masked to a rounded rectangle (the cover-wall
    /// placeholder). Same pixel route as [`Self::image_masked`]: the gradient
    /// is lerped per row, premultiplied by the coverage, then blended.
    fn gradient_round(&mut self, rect: Rect, radius: f32, top: Color, bottom: Color) {
        let px = self.px(rect);
        let width = px.right - px.left;
        let height = px.bottom - px.top;
        let radius_px = (radius * self.scale).round().max(0.0) as i32;
        if radius_px == 0 || mask_too_large(width, height) {
            self.vertical_gradient(rect, top, bottom);
            return;
        }
        let coverage = self
            .res
            .coverage(width as usize, height as usize, CoverageShape::Corners([radius_px; 4]));
        let len = (width as usize) * (height as usize) * 4;
        let masked = &mut self.res.masked;
        masked.clear();
        masked.resize(len, 0);
        for y in 0..height as usize {
            let t = if height > 1 {
                y as f32 / (height - 1) as f32
            } else {
                0.0
            };
            let r = top.r as f32 + (bottom.r as f32 - top.r as f32) * t;
            let g = top.g as f32 + (bottom.g as f32 - top.g as f32) * t;
            let b = top.b as f32 + (bottom.b as f32 - top.b as f32) * t;
            for x in 0..width as usize {
                let coverage = coverage[y * width as usize + x] as u32;
                if coverage == 0 {
                    continue;
                }
                let destination = (y * width as usize + x) * 4;
                masked[destination] = (b as u32 * coverage / 255) as u8;
                masked[destination + 1] = (g as u32 * coverage / 255) as u8;
                masked[destination + 2] = (r as u32 * coverage / 255) as u8;
                masked[destination + 3] = coverage as u8;
            }
        }
        self.blend_masked(px, width, height);
    }
}

impl Canvas for GdiCanvas<'_> {
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
        let brush = self.res.brush(color);
        let px = self.px(rect);
        unsafe {
            FillRect(self.hdc, &px, brush);
        }
    }

    fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        let px = rect.to_px(self.scale);
        let width = px.width();
        let height = px.height();
        if width <= 0 || height <= 0 {
            return;
        }
        let radius_px = (radius * self.scale)
            .round()
            .clamp(0.0, (width.min(height) as f32) / 2.0);
        if radius_px < 0.5 {
            self.fill_rect(rect, color);
            return;
        }
        let shape = CoverageShape::Corners([radius_px as i32; 4]);
        if !self.blend_shape(rect, shape, color) {
            composite_round_rect(self, rect, radius, color);
        }
    }

    fn fill_round_rect_corners(&mut self, rect: Rect, corners: Corners, color: Color) {
        let px = rect.to_px(self.scale);
        let width = px.width();
        let height = px.height();
        if width <= 0 || height <= 0 {
            return;
        }
        let limit = (width.min(height) as f32) / 2.0;
        let radii = corners
            .to_array()
            .map(|radius| (radius * self.scale).round().clamp(0.0, limit) as i32);
        if radii.iter().all(|radius| *radius == 0) {
            self.fill_rect(rect, color);
            return;
        }
        if !self.blend_shape(rect, CoverageShape::Corners(radii), color) {
            self.fill_round_rect(rect, corners.max(), color);
        }
    }

    fn stroke_round_rect(&mut self, rect: Rect, radius: f32, thickness: f32, color: Color) {
        let px = rect.to_px(self.scale);
        let width = px.width();
        let height = px.height();
        if width <= 0 || height <= 0 {
            return;
        }
        let thickness_px = (thickness * self.scale).round().max(1.0);
        let limit = (width.min(height) as f32) / 2.0;
        let radius_px = (radius * self.scale).round().clamp(0.0, limit);
        if thickness_px * 2.0 >= width.min(height) as f32 {
            self.fill_round_rect(rect, radius, color);
            return;
        }
        let shape = CoverageShape::Ring {
            radius: radius_px as i32,
            thickness: thickness_px as i32,
        };
        if !self.blend_shape(rect, shape, color) {
            composite_round_stroke(self, rect, radius, thickness, color);
        }
    }

    fn stroke_round_rect_corners(
        &mut self,
        rect: Rect,
        corners: Corners,
        thickness: f32,
        color: Color,
    ) {
        let px = rect.to_px(self.scale);
        let width = px.width();
        let height = px.height();
        if width <= 0 || height <= 0 {
            return;
        }
        let limit = (width.min(height) as f32) / 2.0;
        let radii = corners
            .to_array()
            .map(|radius| (radius * self.scale).round().clamp(0.0, limit) as i32);
        let thickness_px = (thickness * self.scale).round().max(1.0) as i32;
        if thickness_px * 2 >= width.min(height) {
            self.fill_round_rect_corners(rect, corners, color);
            return;
        }
        let shape = CoverageShape::RingCorners {
            corners: radii,
            thickness: thickness_px,
        };
        if !self.blend_shape(rect, shape, color) {
            composite_round_stroke(self, rect, corners.max(), thickness, color);
        }
    }

    fn fill_round_frame(
        &mut self,
        rect: Rect,
        corners: Corners,
        thickness: f32,
        border: Color,
        fill: Color,
    ) {
        let px = rect.to_px(self.scale);
        let width = px.width();
        let height = px.height();
        if width <= 0 || height <= 0 {
            return;
        }
        let limit = (width.min(height) as f32) / 2.0;
        let radii = corners
            .to_array()
            .map(|radius| (radius * self.scale).round().clamp(0.0, limit) as i32);
        let thickness_px = (thickness * self.scale).round().max(1.0) as i32;
        if thickness_px * 2 >= width.min(height) {
            self.fill_round_rect_corners(rect, corners, border);
            return;
        }
        let key = MaskKey {
            width,
            height,
            shape: CoverageShape::Frame {
                corners: radii,
                thickness: thickness_px,
            },
            color: border.to_colorref(),
            fill: Some(fill.to_colorref()),
        };
        if !self.blend_mask(rect, key) {
            // Mask unavailable (oversized): the portable two-pass shape.
            let inner = rect.inset(thickness, thickness);
            let inner_corners = corners.map(|radius| {
                (radius - thickness).max(0.0)
            });
            self.fill_round_rect_corners(inner, inner_corners, fill);
            self.stroke_round_rect_corners(rect, corners, thickness, border);
        }
    }

    fn fill_circle(&mut self, rect: Rect, color: Color) {
        let px = rect.to_px(self.scale);
        if px.width() <= 0 || px.height() <= 0 {
            return;
        }
        if !self.blend_shape(rect, CoverageShape::Circle, color) {
            self.ellipse(rect, color);
        }
    }

    fn fill_translucent(&mut self, rect: Rect, color: Color, alpha: f32) {
        if rect.is_empty() {
            return;
        }
        let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        if alpha == 0 {
            return;
        }
        if alpha == 255 {
            self.fill_rect(rect, color);
            return;
        }
        let px = self.px(rect);
        let width = px.right - px.left;
        let height = px.bottom - px.top;
        if width <= 0 || height <= 0 {
            return;
        }
        let premultiply = |channel: u8| (channel as f32 * alpha as f32 / 255.0).round() as u8;
        // 1x1 premultiplied BGRA source, stretched over the destination. The
        // source DC and bitmap live in the resource cache; only the pixel
        // changes between calls.
        let pixel = [
            premultiply(color.b),
            premultiply(color.g),
            premultiply(color.r),
            alpha,
        ];
        let key = color.to_colorref();
        let source_dc = {
            let Some(source) = self.res.alpha_source(self.hdc) else {
                self.fill_rect(rect, color);
                return;
            };
            if source.color != key || source.alpha != alpha {
                unsafe {
                    std::ptr::copy_nonoverlapping(pixel.as_ptr(), source.bits, 4);
                }
                source.color = key;
                source.alpha = alpha;
            }
            source.dc
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        unsafe {
            let _ = AlphaBlend(
                self.hdc,
                px.left,
                px.top,
                width,
                height,
                source_dc,
                0,
                0,
                1,
                1,
                blend,
            );
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
        let family = family_name(&style.face);
        let size_px = (style.size * self.scale).round().max(1.0) as i32;
        let font = self.res.font(style, size_px, family);
        self.res.text_buf.clear();
        self.res.text_buf.extend(text.encode_utf16());
        let mut px = self.px(rect);
        unsafe {
            SelectObject(self.hdc, HGDIOBJ(font.0));
            SetTextColor(self.hdc, COLORREF(color.to_colorref()));
            let mut base = DT_SINGLELINE.0 | DT_VCENTER.0 | DT_NOPREFIX.0;
            if overflow == TextOverflow::Ellipsis {
                base |= DT_END_ELLIPSIS.0;
            }
            let align = match align {
                TextAlign::Left => DT_LEFT.0,
                TextAlign::Center => DT_CENTER.0,
                TextAlign::Right => DT_RIGHT.0,
            };
            // `text_buf` and `px` are disjoint fields of different owners.
            let mut wide = std::mem::take(&mut self.res.text_buf);
            DrawTextW(self.hdc, &mut wide, &mut px, DRAW_TEXT_FORMAT(base | align));
            self.res.text_buf = wide;
        }
    }

    fn measure_text(&mut self, text: &str, style: &TextStyle) -> Size {
        if text.is_empty() {
            return Size::ZERO;
        }
        let family = family_name(&style.face);
        let size_px = (style.size * self.scale).round().max(1.0) as i32;
        let font = self.res.font(style, size_px, family);
        self.res.text_buf.clear();
        self.res.text_buf.extend(text.encode_utf16());
        let mut px = RECT::default();
        unsafe {
            SelectObject(self.hdc, HGDIOBJ(font.0));
            let mut wide = std::mem::take(&mut self.res.text_buf);
            DrawTextW(
                self.hdc,
                &mut wide,
                &mut px,
                DRAW_TEXT_FORMAT(DT_CALCRECT.0 | DT_SINGLELINE.0 | DT_NOPREFIX.0),
            );
            self.res.text_buf = wide;
        }
        Size::new(
            (px.right - px.left) as f32 / self.scale,
            (px.bottom - px.top) as f32 / self.scale,
        )
    }

    fn ellipse(&mut self, rect: Rect, color: Color) {
        if rect.width() <= 1.0 || rect.height() <= 1.0 {
            return;
        }
        let brush = self.res.brush(color);
        let px = self.px(rect);
        unsafe {
            let old_brush = SelectObject(self.hdc, HGDIOBJ(brush.0));
            let old_pen = SelectObject(self.hdc, GetStockObject(NULL_PEN));
            let _ = Ellipse(self.hdc, px.left, px.top, px.right, px.bottom);
            let _ = SelectObject(self.hdc, old_pen);
            let _ = SelectObject(self.hdc, old_brush);
        }
    }

    fn vertical_gradient(&mut self, rect: Rect, top: Color, bottom: Color) {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        let px = rect.to_px(self.scale);
        let vertex = |color: Color, x: i32, y: i32| TRIVERTEX {
            x,
            y,
            Red: (color.r as u16) << 8,
            Green: (color.g as u16) << 8,
            Blue: (color.b as u16) << 8,
            Alpha: 0xff00,
        };
        let mut vertices = [
            vertex(top, px.left, px.top),
            vertex(bottom, px.right, px.bottom),
        ];
        let gradient = [GRADIENT_RECT {
            UpperLeft: 0,
            LowerRight: 1,
        }];
        unsafe {
            let _ = GradientFill(
                self.hdc,
                &mut vertices,
                gradient.as_ptr() as *const c_void,
                1,
                GRADIENT_FILL_RECT_V,
            );
        }
    }

    fn vertical_gradient_round(&mut self, rect: Rect, radius: f32, top: Color, bottom: Color) {
        self.gradient_round(rect, radius, top, bottom);
    }

    fn image(&mut self, rect: Rect, crop_square: bool, id: ImageId, data: ImageData<'_>) {
        if rect.is_empty() {
            return;
        }
        let Some(pixels) = self.res.cached_image(id, data) else {
            return;
        };
        let side = pixels.width.min(pixels.height).max(1);
        let (src_x, src_y, src_w, src_h) = if crop_square {
            (
                (pixels.width - side) / 2,
                (pixels.height - side) / 2,
                side,
                side,
            )
        } else {
            (0, 0, pixels.width, pixels.height)
        };
        let px = rect.to_px(self.scale);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: pixels.width,
                // Top-down DIB: rows arrive in image order.
                biHeight: -pixels.height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            let saved = SaveDC(self.hdc);
            SetStretchBltMode(self.hdc, HALFTONE);
            let _ = StretchDIBits(
                self.hdc,
                px.left,
                px.top,
                px.right - px.left,
                px.bottom - px.top,
                src_x,
                src_y,
                src_w,
                src_h,
                Some(pixels.bgra.as_ptr() as *const c_void),
                &info,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
            let _ = RestoreDC(self.hdc, saved);
        }
    }

    /// Circle-masked image (an avatar).
    fn image_circle(&mut self, rect: Rect, crop_square: bool, id: ImageId, data: ImageData<'_>) {
        self.image_masked(rect, CoverageShape::Circle, crop_square, id, data);
    }

    /// Rounded-corner image: the same resample as [`Self::image`], but the
    /// anti-aliased rounded-rect coverage multiplies the alpha, so the corners
    /// reveal whatever is behind the picture (a card, a gradient placeholder).
    fn image_round(
        &mut self,
        rect: Rect,
        radius: f32,
        crop_square: bool,
        id: ImageId,
        data: ImageData<'_>,
    ) {
        let radius_px = (radius * self.scale).round().max(0.0) as i32;
        if radius_px == 0 {
            self.image(rect, crop_square, id, data);
            return;
        }
        self.image_masked(rect, CoverageShape::Corners([radius_px; 4]), crop_square, id, data);
    }

    fn push_clip(&mut self, rect: Rect) {
        let px = rect.to_px(self.scale);
        unsafe {
            let saved = SaveDC(self.hdc);
            self.clip_depth += 1;
            let _ = IntersectClipRect(self.hdc, px.left, px.top, px.right, px.bottom);
            // The saved DC value is stashed in the GDI resource stack implicitly:
            // RestoreDC is called with -1 in `pop_clip`.
            let _ = saved;
        }
    }
    fn pop_clip(&mut self) {
        if self.clip_depth > 0 {
            self.clip_depth -= 1;
            unsafe {
                let _ = RestoreDC(self.hdc, -1);
            }
        }
    }
}

impl Drop for GdiCanvas<'_> {
    fn drop(&mut self) {
        unsafe {
            while self.clip_depth > 0 {
                self.clip_depth -= 1;
                let _ = RestoreDC(self.hdc, -1);
            }
        }
    }
}

/// Resolve the typeface for a style. `Ui` means "the theme family", which the
/// painter encodes as `Named`; when it does not, Microsoft YaHei UI is used.
pub(crate) fn family_name(face: &FontFace) -> &str {
    match face {
        FontFace::Ui => "Microsoft YaHei UI",
        FontFace::Glyph => "Segoe MDL2 Assets",
        FontFace::Named(name) => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 32×32 BGRA target DIB plus the canvas drawing into it; returns the
    /// bits pointer and the canvas' DC handle for cleanup.
    struct Target {
        dc: HDC,
        bitmap: HBITMAP,
        old: HGDIOBJ,
        bits: *mut u8,
    }

    fn target(size: i32) -> Target {
        rect_target(size, size)
    }

    fn rect_target(width: i32, height: i32) -> Target {
        unsafe {
            let dc = CreateCompatibleDC(None);
            assert!(!dc.is_invalid());
            let mut info = BITMAPINFO::default();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = width;
            info.bmiHeader.biHeight = -height;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB.0;
            let mut bits: *mut c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                .expect("target DIB");
            let old = SelectObject(dc, HGDIOBJ(bitmap.0));
            Target {
                dc,
                bitmap,
                old,
                bits: bits as *mut u8,
            }
        }
    }

    impl Target {
        /// `(r, g, b, a)` of one pixel; `stride` is the bitmap width in pixels.
        fn at(&self, x: usize, y: usize, stride: usize) -> (u8, u8, u8, u8) {
            unsafe {
                let i = (y * stride + x) * 4;
                let pixels = std::slice::from_raw_parts(self.bits.add(i), 4);
                (pixels[2], pixels[1], pixels[0], pixels[3])
            }
        }
    }

    impl Drop for Target {
        fn drop(&mut self) {
            unsafe {
                let _ = SelectObject(self.dc, self.old);
                let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
                let _ = DeleteDC(self.dc);
            }
        }
    }

    #[test]
    fn image_round_keeps_the_corners_clear_and_anti_aliased() {
        const SIZE: i32 = 32;
        let target = target(SIZE);
        let mut res = GdiResources::new();
        let rgba = [255u8, 0, 0, 255].repeat(SIZE as usize * SIZE as usize);
        {
            let mut canvas = GdiCanvas::new(target.dc, &mut res, 1.0, 96.0);
            canvas.image_round(
                Rect::from_xywh(0.0, 0.0, SIZE as f32, SIZE as f32),
                8.0,
                true,
                ImageId(1),
                ImageData {
                    rgba: &rgba,
                    width: SIZE as u32,
                    height: SIZE as u32,
                },
            );
        }
        let size = SIZE as usize;
        assert_eq!(target.at(16, 16, size), (255, 0, 0, 255), "the centre is opaque red");
        assert_eq!(target.at(0, 0, size).3, 0, "the top-left corner stays clear");
        assert_eq!(target.at(size - 1, 0, size).3, 0, "the top-right corner stays clear");
        assert_eq!(target.at(0, size - 1, size).3, 0, "the bottom-left corner stays clear");
        assert_eq!(target.at(16, 0, size).3, 255, "the solid top edge keeps the image");
        assert!(
            (0..size).any(|y| (0..size).any(|x| {
                let alpha = target.at(x, y, size).3;
                alpha > 0 && alpha < 255
            })),
            "the corner must be anti-aliased, not a hard region edge"
        );
    }

    #[test]
    fn vertical_gradient_round_clips_the_placeholder_corners() {
        const SIZE: i32 = 24;
        let target = target(SIZE);
        let mut res = GdiResources::new();
        {
            let mut canvas = GdiCanvas::new(target.dc, &mut res, 1.0, 96.0);
            canvas.vertical_gradient_round(
                Rect::from_xywh(0.0, 0.0, SIZE as f32, SIZE as f32),
                6.0,
                Color::new(255, 0, 0),
                Color::new(0, 0, 255),
            );
        }
        let size = SIZE as usize;
        let top = target.at(12, 1, size);
        assert_eq!(top.3, 255, "the top edge is opaque");
        assert!(top.0 > 200 && top.2 < 60, "the top of the gradient is red: {top:?}");
        let bottom = target.at(12, size - 2, size);
        assert!(bottom.2 > 200 && bottom.0 < 60, "the bottom is blue: {bottom:?}");
        assert_eq!(target.at(0, 0, size).3, 0, "the corner stays clear");
        assert_eq!(target.at(size - 1, size - 1, size).3, 0, "the far corner stays clear");
    }

    #[test]
    fn image_cache_respects_the_budget() {
        let mut res = GdiResources::new();
        // 1024 x 1024 RGBA = 4 MB per image; five images exceed 16 MB.
        let pixels = vec![0u8; 1024 * 1024 * 4];
        for id in 0..5u64 {
            let data = ImageData {
                rgba: &pixels,
                width: 1024,
                height: 1024,
            };
            assert!(res.cached_image(ImageId(id), data).is_some());
        }
        assert!(res.image_bytes <= IMAGE_CACHE_BUDGET);
        assert!(res.images.len() < 5);
    }

    #[test]
    fn image_cache_reuses_the_same_id() {
        let mut res = GdiResources::new();
        let pixels = vec![0u8; 4 * 4 * 4];
        let data = ImageData {
            rgba: &pixels,
            width: 4,
            height: 4,
        };
        assert!(res.cached_image(ImageId(7), data).is_some());
        let before = res.image_bytes;
        assert!(res.cached_image(ImageId(7), data).is_some());
        assert_eq!(res.image_bytes, before);
    }

    #[test]
    fn a_transparent_rounded_frame_paints_only_the_outline() {
        use crate::surface::SurfaceStyle;
        const SIZE: usize = 32;
        let target = target(SIZE as i32);
        let mut res = GdiResources::new();
        let mut canvas = GdiCanvas::new(target.dc, &mut res, 1.0, 96.0);
        let rect = Rect::from_xywh(0.0, 0.0, SIZE as f32, SIZE as f32);
        canvas.fill_rect(rect, Color::new(255, 255, 255));
        let style = SurfaceStyle::new().with_radius(8.0).with_transparent(true);
        style.frame(&mut canvas, rect, Color::new(0, 0, 0), Color::new(255, 255, 255));
        drop(canvas);

        let centre = target.at(16, 16, SIZE);
        assert_eq!(
            (centre.0, centre.1, centre.2),
            (255, 255, 255),
            "the transparent interior keeps the page pixels"
        );
        let edge = target.at(16, 0, SIZE);
        assert!(edge.0 < 40 && edge.1 < 40 && edge.2 < 40, "top edge is the border: {edge:?}");
        assert_eq!(target.at(0, 0, SIZE).0, 255, "outside the rounded corner");
        assert_eq!(target.at(16, 3, SIZE).0, 255, "just inside the border");
    }

    /// A field wider than the old 1024 px per-dimension cap used to fall back
    /// to the aliased composer, which filled every corner square with a solid
    /// border disc. The cap is per area now, so a wide, short shape keeps the
    /// cached anti-aliased mask and its corners are thin arcs.
    #[test]
    fn a_wide_rounded_shape_keeps_its_anti_aliased_arcs() {
        const W: f32 = 1500.0;
        const H: f32 = 40.0;
        let target = rect_target(W as i32, H as i32);
        let mut res = GdiResources::new();
        {
            let mut canvas = GdiCanvas::new(target.dc, &mut res, 1.0, 96.0);
            let rect = Rect::from_xywh(0.0, 0.0, W, H);
            canvas.fill_rect(rect, Color::new(255, 255, 255));
            canvas.fill_round_rect(rect, 8.0, Color::new(20, 120, 60));
            canvas.stroke_round_rect(rect, 8.0, 2.0, Color::new(0, 0, 0));
        }
        let size = W as usize;
        let corner = target.at(0, 0, size);
        assert_eq!(
            (corner.0, corner.1, corner.2),
            (255, 255, 255),
            "the corner stays page-coloured: {corner:?}"
        );
        let edge = target.at(700, 0, size);
        assert!(edge.0 < 40 && edge.1 < 40, "the top arc is the border: {edge:?}");
        // Somewhere along the corner the mask is partially covered.
        assert!(
            (0..24).any(|y| (0..16).any(|x| {
                let pixel = target.at(x, y, size);
                pixel.0 < 250 && pixel.0 > 40
            })),
            "the corner must be anti-aliased"
        );
    }

    /// The portable composer is the last-resort fallback; its corner arcs must
    /// be quarter rings, not filled discs.
    #[test]
    fn the_portable_stroke_fallback_draws_arcs_not_corner_discs() {
        const SIZE: i32 = 48;
        let target = target(SIZE);
        let mut res = GdiResources::new();
        {
            let mut canvas = GdiCanvas::new(target.dc, &mut res, 1.0, 96.0);
            let rect = Rect::from_xywh(0.0, 0.0, SIZE as f32, SIZE as f32);
            canvas.fill_rect(rect, Color::new(255, 255, 255));
            composite_round_stroke(
                &mut canvas,
                Rect::from_xywh(4.0, 4.0, 40.0, 40.0),
                10.0,
                2.0,
                Color::new(0, 0, 0),
            );
        }
        let size = SIZE as usize;
        // Deep inside the corner square but inside the inner quarter: the
        // inner fill shows through, it is not painted by the ring.
        for (x, y) in [(10, 10), (12, 12), (9, 11)] {
            let pixel = target.at(x, y, size);
            assert_eq!(
                (pixel.0, pixel.1, pixel.2),
                (255, 255, 255),
                "corner ({x},{y}) must stay open: {pixel:?}"
            );
        }
        // The arc itself is painted: 45° into the corner, between the outer
        // radius (10) and the inner radius (8) around the corner centre (14,14).
        let (x, y) = (8usize, 8usize);
        let pixel = target.at(x, y, size);
        assert!(
            pixel.0 < 60,
            "the corner arc must be painted at ({x},{y}): {pixel:?}"
        );
        // The straight edges are intact.
        assert!(target.at(24, 4, size).0 < 60, "top edge");
        assert!(target.at(24, 43, size).0 < 60, "bottom edge");
    }

    #[test]
    fn a_rounded_frame_uses_the_fill_inside_and_the_border_on_the_rim() {
        use crate::surface::SurfaceStyle;
        const SIZE: usize = 32;
        let target = target(SIZE as i32);
        let mut res = GdiResources::new();
        let mut canvas = GdiCanvas::new(target.dc, &mut res, 1.0, 96.0);
        let rect = Rect::from_xywh(0.0, 0.0, SIZE as f32, SIZE as f32);
        canvas.fill_rect(rect, Color::new(255, 255, 255));
        let style = SurfaceStyle::new().with_radius(8.0);
        style.frame(&mut canvas, rect, Color::new(0, 0, 0), Color::new(20, 160, 60));
        drop(canvas);

        let centre = target.at(16, 16, SIZE);
        assert!(
            centre.0 < 60 && centre.1 > 120 && centre.2 < 120,
            "the interior is the fill color: {centre:?}"
        );
        assert!(
            target.at(16, 0, SIZE).1 < 60,
            "the rim is the border color"
        );
        assert_eq!(target.at(0, 0, SIZE).0, 255, "outside the rounded corner");
    }
}

#[cfg(test)]
mod round_mask_tests {
    use super::*;

    #[test]
    fn rounded_coverage_is_opaque_inside_and_clear_at_the_corners() {
        let mask = rounded_coverage(20, 20, 6.0, 4);
        let alpha = |x: usize, y: usize| mask[y * 20 + x];
        assert_eq!(alpha(0, 0), 0);
        assert_eq!(alpha(19, 0), 0);
        assert_eq!(alpha(0, 19), 0);
        assert_eq!(alpha(10, 10), 255);
        assert_eq!(alpha(10, 0), 255, "the top edge is solid");
        assert_eq!(alpha(0, 10), 255, "the left edge is solid");
        // Some corner pixel is partially covered (anti-aliased).
        let partial = (0..20).any(|y| (0..20).any(|x| {
            let value = alpha(x, y);
            value > 0 && value < 255
        }));
        assert!(partial, "the corner must have partial coverage");
    }

    #[test]
    fn a_zero_radius_is_a_full_rectangle() {
        let mask = rounded_coverage(8, 6, 0.0, 2);
        assert!(mask.iter().all(|alpha| *alpha == 255));
    }

    #[test]
    fn radius_clamps_to_half_the_shorter_side() {
        // A radius beyond half the height is the same pill as radius 2.
        assert_eq!(
            rounded_coverage(10, 4, 100.0, 4),
            rounded_coverage(10, 4, 2.0, 4)
        );
        // The pill's ends are anti-aliased, not square.
        assert!(rounded_coverage(10, 4, 100.0, 4)
            .iter()
            .any(|alpha| *alpha > 0 && *alpha < 255));
    }

    #[test]
    fn radius_masks_stay_antialiased_across_dpi_scales() {
        // The canvas rounds `radius * scale` before picking the cache entry;
        // the mask must be solid inside and anti-aliased at the corner at
        // every scale the showcase runs at.
        for scale in [1.0f32, 1.25, 1.5, 1.75, 2.0] {
            let radius = (6.0 * scale).round();
            let mask = rounded_coverage(40, 40, radius, 4);
            let alpha = |x: usize, y: usize| mask[y * 40 + x];
            assert_eq!(alpha(20, 20), 255, "scale {scale}: centre");
            assert_eq!(alpha(0, 0), 0, "scale {scale}: corner");
            assert_eq!(alpha(20, 0), 255, "scale {scale}: solid edge");
            assert!(
                (0..40).any(|y| (0..40).any(|x| {
                    let value = alpha(x, y);
                    value > 0 && value < 255
                })),
                "scale {scale}: the corner must be anti-aliased"
            );
            // A per-corner mix keeps its sharp and round corners at scale.
            let corners = corners_coverage(40, 40, [radius, 0.0, radius, 0.0], 4);
            let corner_alpha = |x: usize, y: usize| corners[y * 40 + x];
            assert_eq!(corner_alpha(0, 0), 0, "scale {scale}: rounded TL");
            assert_eq!(corner_alpha(39, 0), 255, "scale {scale}: sharp TR");
            assert_eq!(corner_alpha(39, 39), 0, "scale {scale}: rounded BR");
            assert_eq!(corner_alpha(0, 39), 255, "scale {scale}: sharp BL");
        }
    }
}

#[cfg(test)]
mod alpha_bench {
    use super::*;

    /// Manual micro-benchmark: `cargo test -p bamboo-native-ui --lib
    /// alpha_bench -- --ignored --nocapture`. Confirms how expensive a
    /// full-window translucent fill is on the current GDI implementation.
    #[test]
    #[ignore]
    fn full_window_translucent_cost() {
        use windows::Win32::Graphics::Gdi::{CreateCompatibleBitmap, GetDC};
        unsafe {
            let screen = GetDC(None);
            if screen.is_invalid() {
                return;
            }
            let dc = CreateCompatibleDC(Some(screen));
            let bitmap = CreateCompatibleBitmap(screen, 1479, 930);
            let old = SelectObject(dc, HGDIOBJ(bitmap.0));
            let mut res = GdiResources::new();
            let mut canvas = GdiCanvas::new(dc, &mut res, 1.0, 96.0);
            let rect = Rect::from_xywh(0.0, 0.0, 1479.0, 930.0);
            let start = std::time::Instant::now();
            for _ in 0..8 {
                canvas.fill_translucent(rect, Color::new(0, 0, 0), 0.45);
            }
            eprintln!("8 x 1479x930 translucent fills: {:?}", start.elapsed());
            let small = Rect::from_xywh(0.0, 0.0, 96.0, 36.0);
            let start = std::time::Instant::now();
            for _ in 0..64 {
                canvas.fill_translucent(small, Color::new(0, 0, 0), 0.45);
            }
            eprintln!("64 x 96x36 translucent fills: {:?}", start.elapsed());
            drop(canvas);
            let _ = SelectObject(dc, old);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, screen);
        }
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn per_corner_radii_round_only_their_corners() {
        let mask = corners_coverage(20, 20, [0.0, 8.0, 0.0, 8.0], 4);
        let alpha = |x: usize, y: usize| mask[y * 20 + x];
        assert_eq!(alpha(0, 0), 255, "the square top-left corner is solid");
        assert_eq!(alpha(19, 19), 255, "the square bottom-right is solid");
        assert_eq!(alpha(19, 0), 0, "the rounded top-right is clear");
        assert_eq!(alpha(0, 19), 0, "the rounded bottom-left is clear");
        assert_eq!(alpha(10, 10), 255);
    }

    #[test]
    fn circle_coverage_is_round_and_anti_aliased() {
        let mask = circle_coverage(16, 16, 4);
        let alpha = |x: usize, y: usize| mask[y * 16 + x];
        assert_eq!(alpha(8, 8), 255, "the centre is opaque");
        assert_eq!(alpha(0, 0), 0, "the box corner is outside the circle");
        assert_eq!(alpha(1, 1), 0, "near-corner pixels are outside too");
        // The inscribed circle touches the box edges, so edge midpoints are
        // inside; only the corners are clear.
        assert_eq!(alpha(0, 8), 255);
        assert!(
            mask.iter().any(|value| *value > 0 && *value < 255),
            "the rim must be anti-aliased"
        );
    }

    #[test]
    fn ring_coverage_has_an_opaque_edge_and_a_clear_centre() {
        let mask = ring_coverage(20, 20, 9.0, 2.0, 4);
        let alpha = |x: usize, y: usize| mask[y * 20 + x];
        assert_eq!(alpha(10, 10), 0, "the hole is clear");
        assert!(alpha(10, 1) > 200, "inside the two-pixel top band");
        assert_eq!(alpha(10, 2), 0, "the band ends at the inset");
        assert_eq!(alpha(0, 0), 0, "the box corner is outside the outline");
        assert!(
            mask.iter().any(|value| *value > 0 && *value < 255),
            "the ring edges are anti-aliased"
        );
    }

    #[test]
    fn per_corner_ring_follows_its_own_radii() {
        let mask = ring_corners_coverage(20, 20, [8.0, 0.0, 0.0, 8.0], 2.0, 4);
        let alpha = |x: usize, y: usize| mask[y * 20 + x];
        assert_eq!(alpha(10, 10), 0, "the hole is clear");
        assert!(alpha(10, 1) > 200, "top band");
        assert!(alpha(1, 10) > 200, "left band");
        assert_eq!(alpha(0, 0), 0, "the rounded corner is outside");
        assert_eq!(alpha(19, 0), 255, "the square top-right corner is solid");
        assert_eq!(alpha(19, 19), 255, "the square bottom-right corner is solid");
        assert_eq!(alpha(0, 19), 0, "the rounded bottom-left is outside");
    }
}

#[cfg(test)]
mod circle_bench {
    use super::*;

    /// Manual micro-benchmark: `cargo test -p bamboo-native-ui --lib
    /// circle_bench -- --ignored --nocapture`. Compares anti-aliased circle
    /// masks against GDI's `Ellipse` for the dot counts a spinner or progress
    /// ring paints per frame (roughly 12 and 45).
    #[test]
    #[ignore]
    fn circle_dot_cost() {
        use windows::Win32::Graphics::Gdi::{
            CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, ReleaseDC,
            SelectObject,
        };
        unsafe {
            let screen = GetDC(None);
            if screen.is_invalid() {
                return;
            }
            let dc = CreateCompatibleDC(Some(screen));
            let bitmap = CreateCompatibleBitmap(screen, 400, 400);
            let old = SelectObject(dc, HGDIOBJ(bitmap.0));
            let mut res = GdiResources::new();
            let mut canvas = GdiCanvas::new(dc, &mut res, 1.0, 96.0);
            let tints: Vec<Color> = (0..12)
                .map(|index| Color::new(index * 20, 80, 200 - index * 10))
                .collect();
            let dots: Vec<Rect> = (0..45)
                .map(|index| {
                    let x = 10.0 + (index % 20) as f32 * 18.0;
                    let y = 10.0 + (index / 20) as f32 * 18.0;
                    Rect::from_xywh(x, y, 6.0, 6.0)
                })
                .collect();
            let start = std::time::Instant::now();
            for frame in 0..60 {
                for (index, dot) in dots.iter().enumerate() {
                    canvas.fill_circle(*dot, tints[(index + frame) % tints.len()]);
                }
            }
            let aa = start.elapsed();
            let start = std::time::Instant::now();
            for frame in 0..60 {
                for (index, dot) in dots.iter().enumerate() {
                    canvas.ellipse(*dot, tints[(index + frame) % tints.len()]);
                }
            }
            let gdi = start.elapsed();
            eprintln!(
                "60 frames x 45 dots: aa {:?} / ellipse {:?} (per dot {:?} / {:?})",
                aa,
                gdi,
                aa / (60 * 45),
                gdi / (60 * 45)
            );
            drop(canvas);
            let _ = SelectObject(dc, old);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
        }
    }
}

#[cfg(test)]
mod round_rect_bench {
    use super::*;

    /// Manual micro-benchmark: `cargo test -p bamboo-native-ui --lib
    /// round_rect_bench -- --ignored --nocapture`. Compares the cached
    /// coverage+mask path (`fill_round_rect` reusing one coverage map across
    /// frames) against forced re-rasterization (`shape_coverage` every paint)
    /// for the rounded fills a page of buttons paints. The cached number also
    /// pays the per-fill blend into the DC; the raw number is the
    /// rasterization cost the cache removes.
    #[test]
    #[ignore]
    fn cached_coverage_against_re_rasterization() {
        use windows::Win32::Graphics::Gdi::{
            CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, ReleaseDC,
            SelectObject,
        };
        unsafe {
            let screen = GetDC(None);
            if screen.is_invalid() {
                return;
            }
            let dc = CreateCompatibleDC(Some(screen));
            let bitmap = CreateCompatibleBitmap(screen, 1200, 900);
            let old = SelectObject(dc, HGDIOBJ(bitmap.0));
            let mut res = GdiResources::new();
            let mut canvas = GdiCanvas::new(dc, &mut res, 1.0, 96.0);
            // A 240x36 button with a 6 DIP radius, painted 19 times per frame.
            let (width, height, radius) = (240usize, 36usize, 6i32);
            let rect = Rect::from_xywh(0.0, 0.0, width as f32, height as f32);
            let color = Color::new(80, 140, 200);
            let frames = 60;
            let per_frame = 19;
            let start = std::time::Instant::now();
            for _ in 0..frames {
                for _ in 0..per_frame {
                    canvas.fill_round_rect(rect, radius as f32, color);
                }
            }
            let cached = start.elapsed();
            let start = std::time::Instant::now();
            let mut sink = 0usize;
            for _ in 0..frames {
                for _ in 0..per_frame {
                    // `shape_coverage` allocates and rasterizes every call: the
                    // work the coverage cache skips after the first fill.
                    sink += shape_coverage(
                        width,
                        height,
                        CoverageShape::Corners([radius; 4]),
                        4,
                    )
                    .len();
                }
            }
            let raw = start.elapsed();
            eprintln!(
                "{} frames x {} round fills: cached {:?} / re-rasterized {:?} (sink {sink})",
                frames, per_frame, cached, raw
            );
            drop(canvas);
            let _ = SelectObject(dc, old);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
        }
    }
}

#[cfg(test)]
mod frame_bench {
    use super::*;

    /// Manual micro-benchmark: `cargo test -p bamboo-native-ui --lib
    /// frame_bench -- --ignored --nocapture`. Compares the one-mask frame
    /// (`fill_round_frame`, both colors baked into one AlphaBlend) with the
    /// two-pass shape it replaced (inner fill + separate ring), at a typical
    /// field size and at 175 % DPI.
    #[test]
    #[ignore]
    fn rounded_frame_cost() {
        use windows::Win32::Graphics::Gdi::{CreateCompatibleBitmap, GetDC};
        unsafe {
            let screen = GetDC(None);
            if screen.is_invalid() {
                return;
            }
            let dc = CreateCompatibleDC(Some(screen));
            let bitmap = CreateCompatibleBitmap(screen, 1024, 1024);
            let old = SelectObject(dc, HGDIOBJ(bitmap.0));
            let mut res = GdiResources::new();
            let mut canvas = GdiCanvas::new(dc, &mut res, 1.75, 168.0);
            let rect = Rect::from_xywh(8.0, 8.0, 220.0, 36.0);
            let corners = Corners::uniform(8.0);
            let border = Color::new(80, 90, 100);
            let fill = Color::new(30, 34, 40);
            const FRAMES: usize = 1000;
            for _ in 0..8 {
                canvas.fill_round_frame(rect, corners, 1.0, border, fill);
            }
            let start = std::time::Instant::now();
            for _ in 0..FRAMES {
                canvas.fill_round_frame(rect, corners, 1.0, border, fill);
            }
            let one = start.elapsed();
            for _ in 0..8 {
                let inner = rect.inset(1.0, 1.0);
                canvas.fill_round_rect_corners(inner, corners.map(|r| (r - 1.0).max(0.0)), fill);
                canvas.stroke_round_rect(rect, corners.top_left, 1.0, border);
            }
            let start = std::time::Instant::now();
            for _ in 0..FRAMES {
                let inner = rect.inset(1.0, 1.0);
                canvas.fill_round_rect_corners(inner, corners.map(|r| (r - 1.0).max(0.0)), fill);
                canvas.stroke_round_rect(rect, corners.top_left, 1.0, border);
            }
            let two = start.elapsed();
            eprintln!("{FRAMES} rounded frames: one-mask {one:?} / two-pass {two:?}");
            drop(canvas);
            let _ = SelectObject(dc, old);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, screen);
        }
    }
}
