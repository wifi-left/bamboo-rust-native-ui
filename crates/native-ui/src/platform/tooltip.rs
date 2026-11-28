// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Borderless tooltip popup.
//!
//! The rest of the framework never knows this window exists: [`crate::Ui`]
//! reports the hovered/focused widget through [`crate::UiHost::set_tooltip`],
//! and the host owns the dwell (450 ms), the 5 s auto-hide and the popup.
//!
//! The popup is an owned, topmost `WS_POPUP` window with
//! `WS_EX_NOACTIVATE | WS_EX_TRANSPARENT`: it never takes focus and mouse
//! messages pass through to the window below, so hovering it cannot trigger a
//! `WM_MOUSELEAVE` on the host. Painting reuses the framework's [`Canvas`] and
//! palette, so the tooltip follows the theme; `WS_EX_LAYERED` alpha gives the
//! native fade-in/fade-out that the host drives with a timer.

use std::sync::OnceLock;

use windows::core::{PCWSTR, Result, w};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, GetDC, InvalidateRect, ReleaseDC, HDC, PAINTSTRUCT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetWindowLongPtrW,
    GetWindowRect, HTTRANSPARENT, LWA_ALPHA, MA_NOACTIVATE, RegisterClassW, SWP_NOACTIVATE,
    SWP_NOOWNERZORDER, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SetLayeredWindowAttributes,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_NCHITTEST,
    WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::backend::gdi::GdiCanvas;
use crate::backend::Canvas;
use crate::backend::TextStyle;
use crate::geometry::{Rect, Size};
use crate::theme::Palette;
use crate::ui::TooltipAnchor;

/// Padding inside the popup, in DIPs.
const PAD_X: f32 = 8.0;
const PAD_Y: f32 = 5.0;

fn class_name() -> PCWSTR {
    w!("BambooNativeUiTooltip")
}

static CLASS_REGISTERED: OnceLock<bool> = OnceLock::new();

/// Register the tooltip window class once per process.
pub(crate) fn ensure_class(instance: HINSTANCE) -> Result<()> {
    if CLASS_REGISTERED.get().copied().unwrap_or(false) {
        return Ok(());
    }
    let class = WNDCLASSW {
        hInstance: instance,
        lpfnWndProc: Some(tooltip_proc),
        lpszClassName: class_name(),
        ..Default::default()
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err(windows::core::Error::from_thread());
    }
    let _ = CLASS_REGISTERED.set(true);
    Ok(())
}

/// One reusable tooltip window with its own theme palette and GDI caches.
pub(crate) struct TooltipWindow {
    hwnd: HWND,
    text: String,
    palette: Palette,
    style: TextStyle,
    /// Client size in DIPs (painted; also the positioner's input).
    size: Size,
    scale: f32,
    dpi: f32,
    resources: crate::backend::gdi::GdiResources,
}

impl TooltipWindow {
    pub(crate) fn create(instance: HINSTANCE, owner: HWND) -> Result<Self> {
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST,
                class_name(),
                w!(""),
                WS_POPUP,
                0,
                0,
                10,
                10,
                Some(owner),
                None,
                Some(instance),
                None,
            )
        }?;
        // A layered window without attributes is not drawn; start fully
        // transparent so the host can fade it in.
        set_alpha(hwnd, 0);
        Ok(Self {
            hwnd,
            text: String::new(),
            palette: Palette::new(&crate::theme::ThemeSettings::default(), true),
            style: TextStyle::ui(11.5),
            size: Size::ZERO,
            scale: 1.0,
            dpi: 96.0,
            resources: crate::backend::gdi::GdiResources::new(),
        })
    }

    pub(crate) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Update the contents/theme, size the popup to the text and place it next
    /// to `anchor`: the pointer for a hover tip, or below/above the widget for
    /// a keyboard/a11y-focus tip. `client_origin` is the screen pixel position
    /// of client (0,0).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn show(
        &mut self,
        text: &str,
        anchor: TooltipAnchor,
        client_origin: (i32, i32),
        owner_rect: [i32; 4],
        scale: f32,
        dpi: f32,
        palette: Palette,
        style: TextStyle,
        gap_px: i32,
        edge_px: i32,
    ) {
        self.text.clear();
        self.text.push_str(text);
        self.palette = palette;
        self.style = style;
        self.scale = scale;
        self.dpi = dpi;

        let measured = self.measure();
        let text_px = (measured.width * scale).ceil() as i32;
        let pad_x = (PAD_X * scale).round() as i32;
        let pad_y = (PAD_Y * scale).round() as i32;
        let max_width = (owner_rect[2] - owner_rect[0] - edge_px * 2).max(40);
        let width = (text_px + pad_x * 2).min(max_width).max(40);
        let height = ((measured.height * scale).ceil() as i32 + pad_y * 2).max(20);
        self.size = Size::new(width as f32 / scale, height as f32 / scale);

        let position = match anchor {
            TooltipAnchor::Pointer(point) => {
                let px = client_origin.0 + (point.x * scale).round() as i32;
                let py = client_origin.1 + (point.y * scale).round() as i32;
                place_pointer(
                    [px, py],
                    [width, height],
                    owner_rect,
                    (8.0 * scale).round() as i32,
                    (14.0 * scale).round() as i32,
                    edge_px,
                )
            }
            TooltipAnchor::Widget(rect) => {
                let anchor_px = [
                    client_origin.0 + (rect.left * scale).round() as i32,
                    client_origin.1 + (rect.top * scale).round() as i32,
                    client_origin.0 + (rect.right * scale).round() as i32,
                    client_origin.1 + (rect.bottom * scale).round() as i32,
                ];
                place(anchor_px, [width, height], owner_rect, gap_px, edge_px)
            }
        };
        set_alpha(self.hwnd, 0);
        unsafe {
            // `ShowWindow(SW_SHOWNOACTIVATE)` is unreliable for an owned popup:
            // it can activate the owner. SDL switched to this flag set for the
            // same reason. `SWP_NOZORDER | SWP_NOOWNERZORDER` also keep the
            // owner's z-order untouched.
            let _ = SetWindowPos(
                self.hwnd,
                None,
                position[0],
                position[1],
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOZORDER | SWP_NOOWNERZORDER,
            );
            let _ = InvalidateRect(Some(self.hwnd), None, true);
        }
    }

    pub(crate) fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub(crate) fn destroy(&mut self) {
        if !self.hwnd.is_invalid() {
            unsafe {
                // Drop the back-pointer before the window goes away, so the
                // final messages cannot reach a freed `TooltipWindow`.
                SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                let _ = DestroyWindow(self.hwnd);
            }
            self.hwnd = HWND::default();
        }
    }

    fn measure(&mut self) -> Size {
        unsafe {
            let dc = GetDC(None);
            if dc.is_invalid() {
                return Size::ZERO;
            }
            let size = {
                let mut canvas = GdiCanvas::new(dc, &mut self.resources, self.scale, self.dpi);
                canvas.measure_text(&self.text, &self.style)
            };
            let _ = ReleaseDC(None, dc);
            size
        }
    }

    fn paint(&mut self, hdc: HDC) {
        let full = Rect::from_size(self.size);
        let mut canvas = GdiCanvas::new(hdc, &mut self.resources, self.scale, self.dpi);
        canvas.frame(full, self.palette.divider, self.palette.panel);
        canvas.text(
            full.inset(PAD_X, PAD_Y),
            &self.text,
            &self.style,
            crate::backend::TextAlign::Left,
            self.palette.text,
        );
    }
}

/// Set the layered window alpha (0 = transparent, 255 = opaque).
pub(crate) fn set_alpha(hwnd: HWND, alpha: u8) {
    unsafe {
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
    }
}

/// Place a `size` popup below `anchor` (screen pixels), flipping above when it
/// would leave the owner window and clamping to an `edge` margin.
pub(crate) fn place(
    anchor: [i32; 4],
    size: [i32; 2],
    owner: [i32; 4],
    gap: i32,
    edge: i32,
) -> [i32; 2] {
    let [a_left, a_top, a_right, a_bottom] = anchor;
    let [width, height] = size;
    let [o_left, o_top, o_right, o_bottom] = owner;

    let mut x = a_left + (a_right - a_left - width) / 2;
    let mut y = a_bottom + gap;
    if y + height > o_bottom - edge {
        y = a_top - gap - height;
    }
    let max_x = (o_right - edge - width).max(o_left + edge);
    x = x.clamp(o_left + edge, max_x);
    let max_y = (o_bottom - edge - height).max(o_top + edge);
    y = y.clamp(o_top + edge, max_y);
    [x, y]
}

/// Place a `size` popup near the pointer (screen pixels): `dx`/`dy` to the
/// lower right, flipping to the other side when it would leave the owner
/// window, then clamping to the `edge` margin.
pub(crate) fn place_pointer(
    point: [i32; 2],
    size: [i32; 2],
    owner: [i32; 4],
    dx: i32,
    dy: i32,
    edge: i32,
) -> [i32; 2] {
    let [px, py] = point;
    let [width, height] = size;
    let [o_left, o_top, o_right, o_bottom] = owner;

    let mut x = px + dx;
    if x + width > o_right - edge {
        x = px - width - dx;
    }
    let mut y = py + dy;
    if y + height > o_bottom - edge {
        y = py - height - dy;
    }
    let max_x = (o_right - edge - width).max(o_left + edge);
    x = x.clamp(o_left + edge, max_x);
    let max_y = (o_bottom - edge - height).max(o_top + edge);
    y = y.clamp(o_top + edge, max_y);
    [x, y]
}

unsafe extern "system" fn tooltip_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut TooltipWindow;
            let mut paint = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
            if !ptr.is_null() {
                unsafe { (*ptr).paint(hdc) };
            }
            unsafe {
                let _ = EndPaint(hwnd, &paint);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        // The popup sits under the pointer; let every mouse message reach the
        // window below and never take activation.
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// Screen rectangle of `hwnd` in pixels (`[left, top, right, bottom]`).
pub(crate) fn window_rect(hwnd: HWND) -> [i32; 4] {
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return [0, 0, 0, 0];
    }
    [rect.left, rect.top, rect.right, rect.bottom]
}

#[cfg(test)]
mod tests {
    use super::{place, place_pointer};

    #[test]
    fn places_below_and_centers() {
        let position = place([100, 100, 200, 140], [120, 30], [0, 0, 800, 600], 8, 4);
        assert_eq!(position, [90, 148]);
    }

    #[test]
    fn flips_above_near_the_bottom() {
        let position = place([100, 540, 200, 590], [120, 30], [0, 0, 800, 600], 8, 4);
        assert_eq!(position, [90, 502]);
    }

    #[test]
    fn clamps_to_the_owner_edges() {
        let left = place([0, 100, 40, 140], [120, 30], [0, 0, 800, 600], 8, 4);
        assert_eq!(left, [4, 148]);
        let right = place([760, 100, 800, 140], [120, 30], [0, 0, 800, 600], 8, 4);
        assert_eq!(right, [676, 148]);
        let top = place([100, 3, 200, 30], [120, 30], [0, 0, 800, 40], 8, 4);
        assert_eq!(top[1], 4);
    }

    #[test]
    fn pointer_tip_sits_to_the_lower_right() {
        let position = place_pointer([100, 100], [120, 30], [0, 0, 800, 600], 12, 18, 4);
        assert_eq!(position, [112, 118]);
    }

    #[test]
    fn pointer_tip_flips_near_the_edges() {
        let right = place_pointer([750, 100], [120, 30], [0, 0, 800, 600], 12, 18, 4);
        assert_eq!(right, [618, 118]);
        let bottom = place_pointer([100, 580], [120, 30], [0, 0, 800, 600], 12, 18, 4);
        assert_eq!(bottom, [112, 532]);
        let corner = place_pointer([-50, -50], [120, 30], [0, 0, 800, 600], 12, 18, 4);
        assert_eq!(corner, [4, 4]);
    }
}
