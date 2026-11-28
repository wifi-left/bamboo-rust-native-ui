// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Dark/light app mode for native popups (menus and the shell controls that
//! follow it).
//!
//! Windows has no supported switch for the popup menu colors. The shell and
//! most modern applications use undocumented `uxtheme.dll` exports:
//! `SetPreferredAppMode` (ordinal 135) decides the process app mode,
//! `FlushMenuThemes` (ordinal 136) drops the cached menu theme and
//! `AllowDarkModeForWindow` (ordinal 104) opts one window (the popup menu
//! HWND) into dark mode. They are resolved dynamically, so a system without
//! them (Windows 10 before 1903) keeps its default menus.
//!
//! The page palette stays the source of truth: the framework calls
//! [`follow_theme`] before the first window exists and on every theme change,
//! and [`install_popup_hook`] themes the `#32768` popup windows the shell
//! creates for `TrackPopupMenu` on this thread.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetClassNameW, HCBT_CREATEWND, SetWindowsHookExW, WH_CBT,
};
use windows::core::{BOOL, PCSTR, w};

/// Values of the undocumented `PreferredAppMode` enum used here. Forcing the
/// mode makes the page decision win over the system setting.
const FORCE_DARK: i32 = 2;
const FORCE_LIGHT: i32 = 3;

const ORDINAL_ALLOW_DARK_MODE_FOR_WINDOW: usize = 104;
const ORDINAL_SET_PREFERRED_APP_MODE: usize = 135;
const ORDINAL_FLUSH_MENU_THEMES: usize = 136;

type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
type FlushMenuThemes = unsafe extern "system" fn();
type AllowDarkModeForWindow = unsafe extern "system" fn(HWND, BOOL) -> BOOL;

struct Api {
    set_mode: SetPreferredAppMode,
    flush: Option<FlushMenuThemes>,
    allow_window: Option<AllowDarkModeForWindow>,
}

/// The page's resolved dark decision, read by the popup hook.
static DARK: AtomicBool = AtomicBool::new(true);

fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(|| unsafe {
        let module = LoadLibraryW(w!("uxtheme.dll")).ok()?;
        // Ordinal imports: `PCSTR` carries a small integer as the "pointer"
        // when the high word is zero, which is how the loader asks for an
        // ordinal export.
        let set_mode = GetProcAddress(module, PCSTR(ORDINAL_SET_PREFERRED_APP_MODE as *const u8))?;
        let set_mode: SetPreferredAppMode = std::mem::transmute(set_mode);
        let flush = GetProcAddress(module, PCSTR(ORDINAL_FLUSH_MENU_THEMES as *const u8)).map(
            |address| {
                let flush: FlushMenuThemes = std::mem::transmute(address);
                flush
            },
        );
        let allow_window = GetProcAddress(
            module,
            PCSTR(ORDINAL_ALLOW_DARK_MODE_FOR_WINDOW as *const u8),
        )
        .map(|address| {
            let allow: AllowDarkModeForWindow = std::mem::transmute(address);
            allow
        });
        Some(Api {
            set_mode,
            flush,
            allow_window,
        })
    })
    .as_ref()
}

/// Whether the undocumented exports are available on this system.
pub(crate) fn available() -> bool {
    api().is_some()
}

/// Make native menus (and the shell controls that follow the app mode) match
/// the page: dark pages get dark menus, light pages light ones. Safe to call
/// repeatedly and before a window exists; a no-op on systems without the
/// exports.
pub(crate) fn follow_theme(dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
    let Some(api) = api() else {
        return;
    };
    unsafe {
        (api.set_mode)(if dark { FORCE_DARK } else { FORCE_LIGHT });
        // Drop the cached menu theme so the next popup is drawn with the mode
        // set above; without it Windows can keep the previous theme.
        if let Some(flush) = api.flush {
            flush();
        }
    }
    if std::env::var_os("BAMBOO_UI_DEBUG").is_some() {
        eprintln!(
            "bamboo-ui: native popup app mode -> {}",
            if dark { "dark" } else { "light" }
        );
    }
}

/// Opt one window (the main window, so its native scrollbars follow, or a
/// popup menu) into the current app mode.
pub(crate) fn apply_to_window(hwnd: HWND) {
    let dark = DARK.load(Ordering::Relaxed);
    // The DWM frame is separate from the uxtheme popup mode: a dark window
    // with the default frame keeps a light 1px border (the "white shadow" on
    // the color dialog). Immersive dark mode picks the dark caption/border;
    // the border is black explicitly because some builds still draw a light
    // one in dark mode.
    unsafe {
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
        };
        let flag = BOOL::from(dark);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &flag as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<BOOL>() as u32,
        );
        let border: u32 = if dark { 0x0000_0000 } else { 0xFFFF_FFFF };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
    }
    let Some(api) = api() else {
        return;
    };
    let Some(allow) = api.allow_window else {
        return;
    };
    unsafe {
        let _ = allow(hwnd, BOOL::from(dark));
    }
}

/// Theme the `#32768` popup windows the shell creates for `TrackPopupMenu` on
/// this thread. `TrackPopupMenu` builds the menu window internally, so there is
/// no handle to theme before the call; the CBT hook sees it at creation.
pub(crate) fn install_popup_hook() {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    if api().is_none() {
        return;
    }
    INSTALLED.get_or_init(|| unsafe {
        let _ = SetWindowsHookExW(WH_CBT, Some(cbt_hook), None, current_thread_id());
    });
}

fn current_thread_id() -> u32 {
    unsafe { windows::Win32::System::Threading::GetCurrentThreadId() }
}

unsafe extern "system" fn cbt_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // HCBT_CREATEWND: `wParam` is the new HWND. The menu popup class is
    // `#32768`; everything else is left alone. This callback runs on our own
    // thread inside `CreateWindowEx`, so it must stay allocation- and
    // panic-free: two FFI calls and an array comparison.
    if code == HCBT_CREATEWND as i32 {
        let hwnd = HWND(wparam.0 as *mut core::ffi::c_void);
        if is_menu_window(hwnd) {
            apply_to_window(hwnd);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn is_menu_window(hwnd: HWND) -> bool {
    let mut buffer = [0u16; 16];
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if length <= 0 {
        return false;
    }
    let name = &buffer[..length as usize];
    name == [b'#' as u16, b'3' as u16, b'2' as u16, b'7' as u16, b'6' as u16, b'8' as u16]
}
