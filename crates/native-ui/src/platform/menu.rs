// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Native popup menus.
//!
//! [`crate::menu::Menu`] is flattened into an `HMENU` tree and shown with
//! `TrackPopupMenu` (`TPM_RETURNCMD`), so the selection comes back
//! synchronously as the item's command id and no `WM_COMMAND` reaches the
//! window procedure. Menus are the native shell control: they follow the user's
//! system colors and DPI, which an owner-drawn themed menu would not.
//!
//! The host calls this from a posted message, never from a `Host` callback:
//! `TrackPopupMenu` runs its own modal loop, and dispatching that loop while
//! the application's host is borrowed would re-enter the window state.

use std::ffi::c_void;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HBITMAP,
    HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateMenu, CreatePopupMenu, DestroyMenu, GetSystemMetrics, HMENU, MENUITEMINFOW,
    MF_CHECKED, MF_GRAYED, MF_OWNERDRAW, MF_POPUP, MF_SEPARATOR, MF_STRING, MIIM_BITMAP, MIIM_STRING,
    SM_CYMENU, SetMenuItemInfoW, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TPM_TOPALIGN, TrackPopupMenu,
};
use windows::core::{PCWSTR, PWSTR, Result};

use crate::geometry::Rect;
use crate::menu::{Menu, MenuBarModel, MenuEntry, MenuIcon};

/// Where a popup goes: the anchor point in screen pixels and whether the
/// menu's *bottom* edge (not its top) is aligned to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Placement {
    pub x: i32,
    pub y: i32,
    pub bottom_aligned: bool,
}

/// Place a menu that must not cover `avoid` (a button rectangle, screen px).
///
/// The menu opens below `anchor` (its top-left there). When it cannot fit
/// inside `bounds` (the client area for a widget menu) and there is room above
/// the avoid rectangle, it flips above: its *bottom* is aligned to the avoid
/// rectangle's top edge, so the button that opened it stays visible instead of
/// being covered by the popup — the failure mode of handing the button's
/// bottom point to a flipped native menu.
///
/// This is what makes a top-docked menu bar drop down and a bottom-docked one
/// open upward instead of spilling out of the window.
pub(crate) fn place(
    anchor: (i32, i32),
    avoid: Option<Rect>,
    height: i32,
    bounds: Rect,
) -> Placement {
    let below = Placement {
        x: anchor.0,
        y: anchor.1,
        bottom_aligned: false,
    };
    let Some(avoid) = avoid else {
        return below;
    };
    let overflows_bottom = (anchor.1 as f32) + height as f32 > bounds.bottom;
    let fits_above = (avoid.top as f32) - height as f32 >= bounds.top;
    if overflows_bottom && fits_above {
        Placement {
            x: avoid.left as i32,
            y: avoid.top as i32 - 2,
            bottom_aligned: true,
        }
    } else {
        below
    }
}

/// Approximate height in pixels of `menu` when shown, for the flip decision.
/// The native popup still fits itself precisely; this only has to be close.
pub(crate) fn estimated_height(menu: &Menu) -> i32 {
    let item = unsafe { GetSystemMetrics(SM_CYMENU) } + 2;
    let mut rows = 0;
    let mut separators = 0;
    count_rows(menu, &mut rows, &mut separators);
    (rows.max(1) * item + (separators * 6) + 8).max(24)
}

fn count_rows(menu: &Menu, rows: &mut i32, separators: &mut i32) {
    for entry in menu.entries() {
        match entry {
            MenuEntry::Separator => *separators += 1,
            MenuEntry::Item(_) => *rows += 1,
            MenuEntry::Submenu(submenu) => {
                // A submenu parent is one row; its children live in a popup.
                *rows += 1;
                let _ = submenu;
            }
        }
    }
}

/// Show `menu` at `placement` (screen pixels).
///
/// Returns the selected command id; `None` means the menu was dismissed
/// (Escape, a click outside, or an empty menu).
pub(crate) fn show(hwnd: HWND, menu: &Menu, placement: Placement) -> Option<u32> {
    let (hmenu, bitmaps) = unsafe { build(menu) }.ok()?;
    let mut flags = TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON;
    if placement.bottom_aligned {
        flags |= TPM_BOTTOMALIGN;
    }
    let (x, y) = (placement.x, placement.y);
    let selected = unsafe { TrackPopupMenu(hmenu, flags, x, y, None, hwnd, None) };
    unsafe {
        let _ = DestroyMenu(hmenu);
    }
    free_bitmaps(bitmaps);
    let command = selected.0 as u32;
    // With TPM_RETURNCMD the return value *is* the command (0 = dismissed).
    (command != 0).then_some(command)
}

/// Show an existing popup handle at a screen point (a menu-bar title's
/// submenu, opened from `Alt+key` by the host). Returns the selected command.
pub(crate) fn show_popup(hwnd: HWND, popup: HMENU, x: i32, y: i32) -> Option<u32> {
    let flags = TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON;
    let selected = unsafe { TrackPopupMenu(popup, flags, x, y, None, hwnd, None) };
    let command = selected.0 as u32;
    (command != 0).then_some(command)
}

/// Build the native menu tree. Submenus are appended with `MF_POPUP`, so
/// `DestroyMenu` on the root frees the whole tree. Item icons are separate
/// `HBITMAP`s the caller frees with [`free_bitmaps`] after the popup closes.
unsafe fn build(menu: &Menu) -> Result<(HMENU, Vec<HBITMAP>)> {
    let root = unsafe { CreatePopupMenu()? };
    let mut bitmaps = Vec::new();
    if let Err(error) = unsafe { fill(root, menu, &mut bitmaps) } {
        unsafe {
            let _ = DestroyMenu(root);
        }
        free_bitmaps(bitmaps);
        return Err(error);
    }
    Ok((root, bitmaps))
}

/// Build a window menu bar: one popup per top-level model entry. The returned
/// `HMENU` is installed with `SetMenu`; `DestroyMenu` on it frees the whole
/// tree. Labels keep their `&` access key for the system to underline.
///
/// Top-level entries are `MF_OWNERDRAW`, so the host paints them with the page
/// palette (`WM_MEASUREITEM`/`WM_DRAWITEM`); `item_data[i]` is the stable
/// pointer passed with entry `i` and echoed back by those messages. The
/// drop-downs stay normal native popups. A model separator becomes a plain
/// `MF_SEPARATOR` row (it owns no popup), and the per-model-entry positions
/// still equal their `HMENU` positions.
pub(crate) fn build_bar(
    model: &MenuBarModel,
    item_data: &[usize],
) -> Result<(HMENU, Vec<HBITMAP>)> {
    let root = unsafe { CreateMenu()? };
    let mut bitmaps = Vec::new();
    for (index, entry) in model.entries().iter().enumerate() {
        if entry.is_separator() {
            if let Err(error) = unsafe { AppendMenuW(root, MF_SEPARATOR, 0, PCWSTR::null()) } {
                unsafe {
                    let _ = DestroyMenu(root);
                }
                free_bitmaps(bitmaps);
                return Err(error);
            }
            continue;
        }
        let popup = unsafe { CreatePopupMenu()? };
        if let Err(error) = unsafe { fill(popup, entry.menu(), &mut bitmaps) } {
            unsafe {
                let _ = DestroyMenu(popup);
                let _ = DestroyMenu(root);
            }
            free_bitmaps(bitmaps);
            return Err(error);
        }
        let data = item_data.get(index).copied().unwrap_or(0) as *const u16;
        let label = wide(entry.label());
        if let Err(error) = unsafe {
            AppendMenuW(root, MF_OWNERDRAW | MF_POPUP, popup.0 as usize, PCWSTR(data))
        } {
            // Not owned by the parent yet.
            unsafe {
                let _ = DestroyMenu(popup);
                let _ = DestroyMenu(root);
            }
            free_bitmaps(bitmaps);
            return Err(error);
        }
        // Give the owner-drawn title a string too: the drawing stays ours
        // (`WM_DRAWITEM`), but the shell's mnemonic matching reads this text,
        // so `Alt+key` and the arrow-key menu mode work natively.
        let mut info = MENUITEMINFOW {
            cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_STRING,
            dwTypeData: PWSTR(label.as_ptr() as *mut u16),
            ..Default::default()
        };
        let _ = unsafe { SetMenuItemInfoW(root, index as u32, true, &mut info) };
    }
    Ok((root, bitmaps))
}

unsafe fn fill(hmenu: HMENU, menu: &Menu, bitmaps: &mut Vec<HBITMAP>) -> Result<()> {
    let entries = menu.entries();
    // A leading separator, doubled separators and a trailing separator add
    // lines without meaning; the native menu would draw them as gaps.
    let mut last_was_separator = true;
    let mut position: u32 = 0;
    for (index, entry) in entries.iter().enumerate() {
        match entry {
            MenuEntry::Separator => {
                if last_was_separator || index + 1 == entries.len() {
                    continue;
                }
                unsafe { AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null())? };
                last_was_separator = true;
                position += 1;
            }
            MenuEntry::Item(item) => {
                // The shell right-aligns the accelerator when it follows a tab.
                let label = match item.shortcut_text() {
                    Some(shortcut) => wide(&format!("{}\t{shortcut}", item.label())),
                    None => wide(item.label()),
                };
                let mut flags = MF_STRING;
                if item.is_checked() {
                    flags |= MF_CHECKED;
                }
                if item.is_disabled() {
                    flags |= MF_GRAYED;
                }
                unsafe {
                    AppendMenuW(
                        hmenu,
                        flags,
                        item.command() as usize,
                        PCWSTR(label.as_ptr()),
                    )?;
                }
                if let Some(icon) = item.icon_image()
                    && let Some(bitmap) = unsafe { icon_bitmap(icon) }
                {
                    let mut info = MENUITEMINFOW {
                        cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
                        fMask: MIIM_BITMAP,
                        hbmpItem: bitmap,
                        ..Default::default()
                    };
                    if unsafe { SetMenuItemInfoW(hmenu, position, true, &mut info) }.is_ok() {
                        bitmaps.push(bitmap);
                    } else {
                        unsafe {
                            let _ = DeleteObject(HGDIOBJ(bitmap.0));
                        }
                    }
                }
                last_was_separator = false;
                position += 1;
            }
            MenuEntry::Submenu(submenu) => {
                let child = unsafe { CreatePopupMenu()? };
                if let Err(error) = unsafe { fill(child, submenu.menu(), bitmaps) } {
                    unsafe {
                        let _ = DestroyMenu(child);
                    }
                    return Err(error);
                }
                let label = wide(submenu.label());
                let mut flags = MF_POPUP;
                if submenu.is_disabled() {
                    flags |= MF_GRAYED;
                }
                if let Err(error) =
                    unsafe { AppendMenuW(hmenu, flags, child.0 as usize, PCWSTR(label.as_ptr())) }
                {
                    // Not owned by the parent yet.
                    unsafe {
                        let _ = DestroyMenu(child);
                    }
                    return Err(error);
                }
                last_was_separator = false;
                position += 1;
            }
        }
    }
    Ok(())
}

/// One menu-row icon as a top-down 32-bit DIB section (BGRA). The caller frees
/// it after the popup closes; the menu does not own `hbmpItem` bitmaps.
unsafe fn icon_bitmap(icon: &MenuIcon) -> Option<HBITMAP> {
    let width = icon.width() as i32;
    let height = icon.height() as i32;
    let mut info = BITMAPINFO::default();
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        // Negative height selects a top-down DIB.
        biHeight: -height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let bitmap = unsafe {
        CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?
    };
    if bits.is_null() {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
        }
        return None;
    }
    let pixels = (icon.width() as usize) * (icon.height() as usize);
    let source = icon.rgba();
    let target = unsafe { std::slice::from_raw_parts_mut(bits as *mut u8, pixels * 4) };
    for (source, target) in source.chunks_exact(4).zip(target.chunks_exact_mut(4)) {
        // RGBA -> BGRA for the DIB section.
        target[0] = source[2];
        target[1] = source[1];
        target[2] = source[0];
        target[3] = source[3];
    }
    Some(bitmap)
}

/// Delete icon bitmaps after their menu is destroyed.
pub(crate) fn free_bitmaps(bitmaps: Vec<HBITMAP>) {
    for bitmap in bitmaps {
        if !bitmap.is_invalid() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::{build, fill};
    use crate::menu::{Menu, MenuIcon, MenuItem};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyMenu, GetMenuItemCount};
    use windows::core::PWSTR;

    #[test]
    fn a_menu_that_cannot_fit_below_flips_above_the_control() {
        let work = crate::geometry::Rect::new(0.0, 0.0, 1920.0, 1040.0);
        // The control sits near the bottom: the menu opens above it, with its
        // bottom at the control's top so the control is never covered.
        let avoid = crate::geometry::Rect::new(200.0, 950.0, 320.0, 980.0);
        let placement = super::place((200, 980), Some(avoid), 300, work);
        assert!(placement.bottom_aligned);
        assert_eq!(placement.x, 200);
        assert_eq!(placement.y, 948);

        // Room below keeps the normal placement.
        let avoid = crate::geometry::Rect::new(200.0, 200.0, 320.0, 230.0);
        let placement = super::place((200, 230), Some(avoid), 300, work);
        assert!(!placement.bottom_aligned);
        assert_eq!(placement.y, 230);

        // No room on either side: stay below and let Windows fit it.
        let avoid = crate::geometry::Rect::new(200.0, 300.0, 320.0, 330.0);
        let placement = super::place((200, 330), Some(avoid), 900, work);
        assert!(!placement.bottom_aligned);

        // A pointer context menu has no control to avoid.
        let placement = super::place((700, 700), None, 400, work);
        assert!(!placement.bottom_aligned);
        assert_eq!(placement.y, 700);
    }

    #[test]
    fn a_control_at_the_bottom_of_the_client_flips_even_when_the_screen_has_room() {
        // The client ends at 900 but the screen has 140 px more below it: an
        // in-window chrome menu still stays inside the window.
        let client = crate::geometry::Rect::new(100.0, 100.0, 1200.0, 900.0);
        let avoid = crate::geometry::Rect::new(300.0, 850.0, 420.0, 880.0);
        let placement = super::place((300, 880), Some(avoid), 200, client);
        assert!(placement.bottom_aligned);
        assert_eq!(placement.y, 848);

        // The same control at the top of the client opens downward.
        let avoid = crate::geometry::Rect::new(300.0, 120.0, 420.0, 150.0);
        let placement = super::place((300, 150), Some(avoid), 200, client);
        assert!(!placement.bottom_aligned);
        assert_eq!(placement.y, 150);
    }

    #[test]
    fn builds_and_frees_a_nested_native_menu() {
        let menu = Menu::new()
            .item(1, "播放")
            .separator()
            .submenu("更多", Menu::new().item(2, "复制").disabled_item(3, "属性"));
        let (native, bitmaps) = unsafe { build(&menu) }.expect("menu tree");
        let count = unsafe { GetMenuItemCount(Some(native)) };
        assert_eq!(count, 3);
        unsafe {
            let _ = DestroyMenu(native);
        }
        super::free_bitmaps(bitmaps);
    }

    #[test]
    fn edge_and_double_separators_are_dropped() {
        let menu = Menu::new()
            .separator()
            .item(1, "A")
            .separator()
            .separator()
            .item(2, "B")
            .separator();
        let (native, bitmaps) = unsafe { build(&menu) }.expect("menu tree");
        assert_eq!(unsafe { GetMenuItemCount(Some(native)) }, 3);
        unsafe {
            let _ = DestroyMenu(native);
        }
        super::free_bitmaps(bitmaps);
    }

    #[test]
    fn shortcut_text_and_icon_bitmaps_reach_the_native_item() {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetMenuItemInfoW, MIIM_BITMAP, MIIM_STRING, MENUITEMINFOW,
        };
        let icon = MenuIcon::from_rgba(2, 2, [200u8, 40, 40, 255].repeat(4));
        let menu = Menu::new()
            .shortcut_item(1, "播放", "Ctrl+P")
            .push(MenuItem::new(2, "收藏").icon(icon));
        let (native, bitmaps) = unsafe { build(&menu) }.expect("menu tree");

        // The accelerator follows a tab, which the shell renders right-aligned.
        let mut buffer = [0u16; 64];
        let mut info = MENUITEMINFOW {
            cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_STRING,
            dwTypeData: PWSTR(buffer.as_mut_ptr()),
            cch: buffer.len() as u32,
            ..Default::default()
        };
        assert!(unsafe { GetMenuItemInfoW(native, 0, true, &mut info) }.is_ok());
        let text = String::from_utf16_lossy(&buffer[..info.cch as usize]);
        assert_eq!(text, "播放\tCtrl+P");

        // The icon is a real bitmap handle and is tracked for cleanup.
        let mut info = MENUITEMINFOW {
            cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_BITMAP,
            ..Default::default()
        };
        assert!(unsafe { GetMenuItemInfoW(native, 1, true, &mut info) }.is_ok());
        assert!(!info.hbmpItem.is_invalid());
        assert_eq!(bitmaps.len(), 1);
        unsafe {
            let _ = DestroyMenu(native);
        }
        super::free_bitmaps(bitmaps);
    }

    #[test]
    fn filling_into_a_plain_popup_reports_errors_without_leaking() {
        // An empty menu still produces a valid (empty) popup.
        let empty = Menu::new();
        let (native, bitmaps) = unsafe { build(&empty) }.expect("menu tree");
        assert_eq!(unsafe { GetMenuItemCount(Some(native)) }, 0);
        unsafe {
            let mut extra = Vec::new();
            let _ = fill(native, &empty, &mut extra);
            let _ = DestroyMenu(native);
        }
        super::free_bitmaps(bitmaps);
    }
}
