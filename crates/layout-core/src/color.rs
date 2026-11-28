// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Platform-neutral colors.
//!
//! Colors are stored as 8-bit RGB. A host converts to its own color type at
//! the boundary; no platform type appears in this module's public API.

/// An opaque RGB color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn rgb(rgb: [u8; 3]) -> Self {
        Self::new(rgb[0], rgb[1], rgb[2])
    }

    /// From `0xRRGGBB`.
    pub const fn from_hex(hex: u32) -> Self {
        Self::new((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    /// As `0xRRGGBB`.
    pub const fn to_hex(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | self.b as u32
    }

    pub const fn to_rgb(self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }

    /// Perceptual luminance in 0..=1 (Rec. 601 weights).
    pub fn luminance(self) -> f32 {
        (0.299 * self.r as f32 + 0.587 * self.g as f32 + 0.114 * self.b as f32) / 255.0
    }

    /// Linear blend toward `other`; `t` is clamped to 0..=1.
    pub fn mix(self, other: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color::new(lerp(self.r, other.r), lerp(self.g, other.g), lerp(self.b, other.b))
    }

    /// Move toward white by `t`.
    pub fn lighten(self, t: f32) -> Color {
        self.mix(Color::new(255, 255, 255), t)
    }

    /// Move toward black by `t`.
    pub fn darken(self, t: f32) -> Color {
        self.mix(Color::new(0, 0, 0), t)
    }

    /// `COLORREF` value (`0x00BBGGRR`) for the Windows backend.
    #[cfg(windows)]
    pub fn to_colorref(self) -> u32 {
        ((self.b as u32) << 16) | ((self.g as u32) << 8) | self.r as u32
    }

    #[cfg(windows)]
    pub const fn from_colorref(value: u32) -> Self {
        Self::new(value as u8, (value >> 8) as u8, (value >> 16) as u8)
    }
}

/// A color plus a per-pixel alpha byte, for widgets that accept transparency
/// (the in-window color picker's alpha row). `alpha` is `0` (invisible) to
/// `255` (opaque).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub color: Color,
    pub alpha: u8,
}

impl Rgba {
    pub const fn new(color: Color, alpha: u8) -> Self {
        Self { color, alpha }
    }

    pub const fn opaque(color: Color) -> Self {
        Self { color, alpha: 255 }
    }

    /// Alpha in `0..=1` for canvas blending.
    pub fn alpha_unit(self) -> f32 {
        self.alpha as f32 / 255.0
    }

    /// `#RRGGBB` for an opaque color, `#RRGGBBAA` otherwise.
    pub fn display(self) -> String {
        if self.alpha == 255 {
            format!("#{:06X}", self.color.to_hex())
        } else {
            format!("#{:06X}{:02X}", self.color.to_hex(), self.alpha)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let color = Color::from_hex(0x7fd3a8);
        assert_eq!(color, Color::new(127, 211, 168));
        assert_eq!(color.to_hex(), 0x7fd3a8);
    }

    #[test]
    fn mix_hits_the_endpoints() {        let black = Color::new(0, 0, 0);
        let white = Color::new(255, 255, 255);
        assert_eq!(black.mix(white, 0.0), black);
        assert_eq!(black.mix(white, 1.0), white);
        assert_eq!(black.mix(white, 0.5), Color::new(128, 128, 128));
    }

    #[test]
    fn lighten_and_darken() {
        let base = Color::new(100, 100, 100);
        assert!(base.lighten(0.5).r > base.r);
        assert!(base.darken(0.5).r < base.r);
    }

    #[cfg(windows)]
    #[test]
    fn colorref_is_bgr() {
        let color = Color::new(0x11, 0x22, 0x33);
        assert_eq!(color.to_colorref(), 0x00332211);
        assert_eq!(Color::from_colorref(0x00332211), color);
    }

    #[test]
    fn rgba_display_drops_an_opaque_alpha() {
        let color = Color::from_hex(0x1971C2);
        assert_eq!(Rgba::opaque(color).display(), "#1971C2");
        assert_eq!(Rgba::new(color, 0x80).display(), "#1971C280");
        assert_eq!(Rgba::new(color, 0).alpha_unit(), 0.0);
        assert_eq!(Rgba::new(color, 255).alpha_unit(), 1.0);
    }
}
