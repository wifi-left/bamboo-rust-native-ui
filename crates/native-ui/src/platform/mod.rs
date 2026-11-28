// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Platform backends.

#[cfg(windows)]
pub mod window;

#[cfg(windows)]
pub mod keyboard;

#[cfg(windows)]
pub(crate) mod uia;

#[cfg(windows)]
pub(crate) mod dialog;

#[cfg(windows)]
pub(crate) mod color_window;

#[cfg(windows)]
pub(crate) mod menu;

#[cfg(windows)]
pub(crate) mod dark_theme;

#[cfg(windows)]
pub(crate) mod drag_drop;

#[cfg(windows)]
pub(crate) mod notification;

#[cfg(windows)]
pub(crate) mod tooltip;

#[cfg(windows)]
pub use window::{WindowBuilder, WindowHandle};

/// Lines a wheel notch scrolls, from the system setting
/// (`SPI_GETWHEELSCROLLLINES`, 3 by default). The "scroll a page" choice is
/// reported as [`WHEEL_PAGE_LINES`] and a failed query falls back to 3.
#[cfg(windows)]
pub fn wheel_lines() -> f32 {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETWHEELSCROLLLINES, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    let mut lines: u32 = 3;
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETWHEELSCROLLLINES,
            0,
            Some(&mut lines as *mut u32 as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if result.is_err() || lines == 0 {
        return 3.0;
    }
    // WHEEL_PAGESCROLL (MAXDWORD) asks for a page per notch; the scrollable
    // widgets turn the marker into "the whole range".
    if lines >= WHEEL_PAGE_LINES as u32 {
        WHEEL_PAGE_LINES
    } else {
        lines as f32
    }
}

/// Marker for "a notch scrolls a page" in [`wheel_lines`].
#[cfg(windows)]
pub const WHEEL_PAGE_LINES: f32 = 100.0;
