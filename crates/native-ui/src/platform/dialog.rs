// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Native dialog bodies, run on a worker thread.
//!
//! Every dialog is shown from its own thread. A modal dialog disables the
//! owner window first, so the user must close it before using the application;
//! a parallel dialog passes no owner, so the application stays enabled. Either
//! way the UI thread keeps pumping its own message loop — a nested modal loop
//! never runs inside a `Host` callback and no window state is borrowed twice.
//!
//! The caller (`WindowState`) posts the returned [`DialogResult`] back to the
//! UI thread.

use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::{COLORREF, HWND, S_FALSE, S_OK};
use windows::Win32::Graphics::Gdi::LOGFONTW;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Controls::{
    TASKDIALOGCONFIG, TASKDIALOGCONFIG_0, TASKDIALOG_BUTTON, TaskDialogIndirect,
    TDCBF_CANCEL_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION, TDF_POSITION_RELATIVE_TO_WINDOW,
    TDF_USE_HICON_MAIN, TD_ERROR_ICON, TD_INFORMATION_ICON, TD_WARNING_ICON,
};
use windows::Win32::UI::Controls::Dialogs::{
    CC_ANYCOLOR, CC_FULLOPEN, CC_RGBINIT, CF_EFFECTS, CF_INITTOLOGFONTSTRUCT, CF_NOVERTFONTS,
    CF_SCREENFONTS, CHOOSECOLORW, CHOOSEFONTW, ChooseColorW, ChooseFontW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, HICON, IMAGE_ICON, LR_DEFAULTSIZE, LR_LOADFROMFILE, LoadImageW,
};
use windows::Win32::UI::Shell::{
    FILEOPENDIALOGOPTIONS, FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
    FOS_NOCHANGEDIR, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog,
    FileSaveDialog, IFileDialog, IFileOpenDialog, IShellItem, IShellItemArray,
    SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ABORTRETRYIGNORE, MB_DEFBUTTON1, MB_DEFBUTTON2, MB_DEFBUTTON3, MB_ICONERROR,
    MB_ICONINFORMATION, MB_ICONQUESTION, MB_ICONWARNING, MB_OK, MB_OKCANCEL, MB_RETRYCANCEL,
    MB_SETFOREGROUND, MB_YESNO, MB_YESNOCANCEL, MESSAGEBOX_STYLE, MessageBoxW,
};
use windows::core::{Interface, PCWSTR, Result};

use crate::color::Color;
use crate::dialog::{
    ColorDialog, Dialog, DialogResult, FileDialog, FileDialogMode, FontChoice, FontDialog,
    MessageAnswer, MessageButtons, MessageDefault, MessageDialog, MessageIcon,
};

// `MessageBox` result ids (`IDOK` and friends are typed as MESSAGEBOX_RESULT in
// the bindings; plain numbers keep the mapping readable).
const ID_OK: i32 = 1;
const ID_CANCEL: i32 = 2;
const ID_ABORT: i32 = 3;
const ID_RETRY: i32 = 4;
const ID_IGNORE: i32 = 5;
const ID_YES: i32 = 6;
const ID_NO: i32 = 7;

/// Show `dialog` on the current thread. Blocks until the user closes it.
///
/// `modal` disables `hwnd` (the owner) for the lifetime of the dialog. The
/// dialog itself is given the owner too, so it stays above the application.
pub(crate) fn show(
    hwnd: HWND,
    dialog: &Dialog,
    modal: bool,
    theme: &crate::theme::ThemeSettings,
    dark: bool,
) -> DialogResult {
    let owner = modal.then_some(hwnd);
    let guard = modal.then(|| OwnerGuard::new(hwnd));
    let result = match dialog {
        Dialog::Message(dialog) => show_message(owner, dialog),
        Dialog::File(dialog) => {
            let paths = show_file(owner, dialog);
            if dialog.returns_multiple() {
                DialogResult::Files(paths)
            } else {
                DialogResult::File(paths.and_then(|paths| paths.into_iter().next()))
            }
        }
        Dialog::Font(dialog) => DialogResult::Font(show_font(owner, dialog)),
        Dialog::Color(dialog) => DialogResult::Color(show_color(owner, dialog)),
        Dialog::ColorWindow(dialog) => {
            DialogResult::ColorWindow(super::color_window::show(owner, dialog, theme, dark))
        }
    };
    drop(guard);
    result
}

/// Re-enables the owner even if the dialog body returns early.
struct OwnerGuard {
    hwnd: HWND,
}

impl Drop for OwnerGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = EnableWindow(self.hwnd, true);
        }
    }
}

impl OwnerGuard {
    fn new(hwnd: HWND) -> Self {
        unsafe {
            let _ = EnableWindow(hwnd, false);
        }
        Self { hwnd }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn show_message(owner: Option<HWND>, dialog: &MessageDialog) -> DialogResult {
    // Custom buttons or a custom icon need the task-dialog shell.
    if !dialog.custom_buttons.is_empty() || dialog.icon_file.is_some() {
        return show_task_message(owner, dialog);
    }
    let text = wide(&dialog.text);
    let title = wide(&dialog.title);
    let flags = message_flags(dialog);
    let answer =
        unsafe { MessageBoxW(owner, PCWSTR(text.as_ptr()), PCWSTR(title.as_ptr()), flags).0 };
    DialogResult::Message(Some(answer_from_button(answer)))
}

/// `MessageBox`/task-dialog button id → the reported answer.
fn answer_from_button(id: i32) -> MessageAnswer {
    match id {
        ID_OK => MessageAnswer::Ok,
        ID_CANCEL => MessageAnswer::Cancel,
        ID_YES => MessageAnswer::Yes,
        ID_NO => MessageAnswer::No,
        ID_RETRY => MessageAnswer::Retry,
        ID_ABORT => MessageAnswer::Abort,
        ID_IGNORE => MessageAnswer::Ignore,
        other => MessageAnswer::Custom(other.max(0) as u32),
    }
}

/// The task-dialog form: application button labels (reported as
/// [`MessageAnswer::Custom`]), an optional bold instruction and an optional
/// `.ico` main icon. Needs comctl32 v6 (an application manifest); without it
/// the call falls back to the standard `MessageBox` set.
fn show_task_message(owner: Option<HWND>, dialog: &MessageDialog) -> DialogResult {
    let text = wide(&dialog.text);
    let title = wide(&dialog.title);
    let instruction = wide(&dialog.instruction);
    let labels: Vec<Vec<u16>> = dialog
        .custom_buttons
        .iter()
        .map(|button| wide(button.label()))
        .collect();
    let buttons: Vec<TASKDIALOG_BUTTON> = dialog
        .custom_buttons
        .iter()
        .zip(&labels)
        .map(|(button, label)| TASKDIALOG_BUTTON {
            nButtonID: button.command() as i32,
            pszButtonText: PCWSTR(label.as_ptr()),
        })
        .collect();
    let custom_icon = dialog.icon_file.as_ref().and_then(|path| {
        let path = wide(path);
        unsafe {
            LoadImageW(
                None,
                PCWSTR(path.as_ptr()),
                IMAGE_ICON,
                0,
                0,
                LR_LOADFROMFILE | LR_DEFAULTSIZE,
            )
        }
        .ok()
        .map(|handle| HICON(handle.0))
    });
    let mut flags = TDF_ALLOW_DIALOG_CANCELLATION;
    if owner.is_some() {
        flags |= TDF_POSITION_RELATIVE_TO_WINDOW;
    }
    let icon = match custom_icon {
        Some(icon) => {
            flags |= TDF_USE_HICON_MAIN;
            TASKDIALOGCONFIG_0 { hMainIcon: icon }
        }
        None => TASKDIALOGCONFIG_0 {
            pszMainIcon: match dialog.icon {
                MessageIcon::Info | MessageIcon::Question => TD_INFORMATION_ICON,
                MessageIcon::Warning => TD_WARNING_ICON,
                MessageIcon::Error => TD_ERROR_ICON,
            },
        },
    };
    let default_index = match dialog.default_button {
        MessageDefault::First => 0,
        MessageDefault::Second => 1,
        MessageDefault::Third => 2,
    };
    let default_button = buttons
        .get(default_index.min(buttons.len().saturating_sub(1)))
        .map(|button| button.nButtonID)
        .unwrap_or(0);
    let config = TASKDIALOGCONFIG {
        cbSize: size_of::<TASKDIALOGCONFIG>() as u32,
        hwndParent: owner.unwrap_or_default(),
        dwFlags: flags,
        // The Cancel common button (and Esc/close) always reports Cancel.
        dwCommonButtons: TDCBF_CANCEL_BUTTON,
        pszWindowTitle: PCWSTR(title.as_ptr()),
        Anonymous1: icon,
        pszMainInstruction: PCWSTR(instruction.as_ptr()),
        pszContent: PCWSTR(text.as_ptr()),
        cButtons: buttons.len() as u32,
        pButtons: buttons.as_ptr(),
        nDefaultButton: default_button,
        ..Default::default()
    };
    let mut pressed = ID_CANCEL;
    let result = unsafe { TaskDialogIndirect(&config, Some(&mut pressed), None, None) };
    if let Some(icon) = custom_icon {
        unsafe {
            let _ = DestroyIcon(icon);
        }
    }
    if result.is_err() {
        // No comctl32 v6 activation context: show the standard set instead of
        // failing (the custom labels/ids cannot be represented there).
        let text = wide(&dialog.text);
        let title = wide(&dialog.title);
        let answer = unsafe {
            MessageBoxW(
                owner,
                PCWSTR(text.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_SETFOREGROUND,
            )
            .0
        };
        return DialogResult::Message(Some(answer_from_button(answer)));
    }
    DialogResult::Message(Some(answer_from_button(pressed)))
}

fn message_flags(dialog: &MessageDialog) -> MESSAGEBOX_STYLE {
    let buttons = match dialog.buttons {
        MessageButtons::Ok => MB_OK,
        MessageButtons::OkCancel => MB_OKCANCEL,
        MessageButtons::YesNo => MB_YESNO,
        MessageButtons::YesNoCancel => MB_YESNOCANCEL,
        MessageButtons::RetryCancel => MB_RETRYCANCEL,
        MessageButtons::AbortRetryIgnore => MB_ABORTRETRYIGNORE,
    };
    let icon = match dialog.icon {
        MessageIcon::Info => MB_ICONINFORMATION,
        MessageIcon::Warning => MB_ICONWARNING,
        MessageIcon::Error => MB_ICONERROR,
        MessageIcon::Question => MB_ICONQUESTION,
    };
    let default = match dialog.default_button {
        MessageDefault::First => MB_DEFBUTTON1,
        MessageDefault::Second => MB_DEFBUTTON2,
        MessageDefault::Third => MB_DEFBUTTON3,
    };
    // MB_SETFOREGROUND matters for a parallel dialog: it has no owner, so it
    // has to bring itself up instead of inheriting the owner's activation.
    buttons | icon | default | MB_SETFOREGROUND
}

fn show_file(owner: Option<HWND>, dialog: &FileDialog) -> Option<Vec<String>> {
    // The shell dialogs are COM objects; a worker thread starts uninitialized.
    // Single-threaded apartment is what the shell expects.
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let initialized = hr.0 == S_OK.0 || hr.0 == S_FALSE.0;
    let result = match unsafe { file_dialog(owner, dialog) } {
        Ok(paths) => paths,
        Err(_) => None,
    };
    if initialized {
        unsafe { CoUninitialize() };
    }
    result
}

unsafe fn file_dialog(owner: Option<HWND>, dialog: &FileDialog) -> Result<Option<Vec<String>>> {
    let folder = dialog.mode == FileDialogMode::Folder;
    let save = dialog.mode == FileDialogMode::Save;
    let clsid = if save {
        &FileSaveDialog
    } else {
        &FileOpenDialog
    };
    let fd: IFileDialog = unsafe { CoCreateInstance(clsid, None, CLSCTX_INPROC_SERVER)? };

    if !dialog.title.is_empty() {
        let title = wide(&dialog.title);
        unsafe { fd.SetTitle(PCWSTR(title.as_ptr()))? };
    }
    if !dialog.filters.is_empty() {
        // The wide buffers must outlive the SetFileTypes call.
        let labels: Vec<Vec<u16>> = dialog.filters.iter().map(|f| wide(&f.label)).collect();
        let patterns: Vec<Vec<u16>> = dialog.filters.iter().map(|f| wide(&f.pattern)).collect();
        let specs: Vec<COMDLG_FILTERSPEC> = labels
            .iter()
            .zip(&patterns)
            .map(|(label, pattern)| COMDLG_FILTERSPEC {
                pszName: PCWSTR(label.as_ptr()),
                pszSpec: PCWSTR(pattern.as_ptr()),
            })
            .collect();
        unsafe { fd.SetFileTypes(&specs)? };
    }

    let mut options = unsafe { fd.GetOptions()? };
    options |= FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR;
    options |= if folder {
        FOS_PICKFOLDERS | FOS_PATHMUSTEXIST
    } else if save {
        FOS_OVERWRITEPROMPT | FOS_PATHMUSTEXIST
    } else {
        FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST
    };
    if dialog.returns_multiple() {
        options |= FOS_ALLOWMULTISELECT;
    }
    let options = FILEOPENDIALOGOPTIONS(options.0);
    unsafe { fd.SetOptions(options)? };

    if let Some(name) = &dialog.initial_name {
        let name = wide(name);
        unsafe { fd.SetFileName(PCWSTR(name.as_ptr()))? };
    }
    if let Some(dir) = &dialog.initial_dir {
        let dir = wide(dir);
        if let Ok(item) =
            unsafe { SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(dir.as_ptr()), None) }
        {
            let _ = unsafe { fd.SetFolder(&item) };
        }
    }

    // Cancellation is an error from Show (ERROR_CANCELLED); so is any other
    // failure, and to the caller both mean "no selection".
    if unsafe { fd.Show(owner) }.is_err() {
        return Ok(None);
    }
    if dialog.returns_multiple() {
        let open: IFileOpenDialog = fd.cast()?;
        let results: IShellItemArray = unsafe { open.GetResults()? };
        let count = unsafe { results.GetCount()? };
        let mut paths = Vec::with_capacity(count as usize);
        for index in 0..count {
            let item = unsafe { results.GetItemAt(index)? };
            if let Some(path) = shell_item_path(&item) {
                paths.push(path);
            }
        }
        return Ok((!paths.is_empty()).then_some(paths));
    }
    let item = unsafe { fd.GetResult()? };
    Ok(shell_item_path(&item).map(|path| vec![path]))
}

/// The filesystem path of a shell item, freeing the COM string.
fn shell_item_path(item: &IShellItem) -> Option<String> {
    let path = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }.ok()?;
    let text = unsafe { path.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(path.0 as *const c_void)) };
    text
}

fn show_font(owner: Option<HWND>, dialog: &FontDialog) -> Option<FontChoice> {
    let mut logfont = LOGFONTW::default();
    let mut flags = CF_SCREENFONTS | CF_EFFECTS | CF_NOVERTFONTS;
    if let Some(initial) = &dialog.initial {
        flags |= CF_INITTOLOGFONTSTRUCT;
        // 96 DPI is the dialog's own reference; the returned point size is
        // what the picker wrote into `iPointSize`.
        logfont.lfHeight = -((initial.size * 96.0 / 72.0).round() as i32).max(1);
        logfont.lfWeight = if initial.bold { 700 } else { 400 };
        logfont.lfItalic = initial.italic as u8;
        let face: Vec<u16> = initial.family.encode_utf16().take(31).collect();
        logfont.lfFaceName[..face.len()].copy_from_slice(&face);
    }
    let mut chooser = CHOOSEFONTW {
        lStructSize: size_of::<CHOOSEFONTW>() as u32,
        hwndOwner: owner.unwrap_or_default(),
        lpLogFont: &mut logfont,
        iPointSize: dialog
            .initial
            .as_ref()
            .map(|initial| (initial.size * 10.0).round() as i32)
            .unwrap_or(0),
        Flags: flags,
        ..Default::default()
    };
    if !unsafe { ChooseFontW(&mut chooser) }.as_bool() {
        return None;
    }
    let end = logfont
        .lfFaceName
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(logfont.lfFaceName.len());
    Some(FontChoice {
        family: String::from_utf16_lossy(&logfont.lfFaceName[..end]),
        size: chooser.iPointSize as f32 / 10.0,
        bold: logfont.lfWeight >= 700,
        italic: logfont.lfItalic != 0,
    })
}

fn show_color(owner: Option<HWND>, dialog: &ColorDialog) -> Option<Color> {
    // The custom-color wells are process-wide: the shell writes a mixed color
    // back into the array, and the next picker (on any worker thread) starts
    // from it.
    let mut custom = match dialog.custom {
        Some(colors) => colors.map(|color| COLORREF(color.to_colorref())),
        None => crate::dialog::custom_colors().map(|color| COLORREF(color.to_colorref())),
    };
    let mut flags = CC_FULLOPEN | CC_ANYCOLOR;
    let mut result = COLORREF(0);
    if let Some(initial) = dialog.initial {
        flags |= CC_RGBINIT;
        result = COLORREF(initial.to_colorref());
    }
    let mut chooser = CHOOSECOLORW {
        lStructSize: size_of::<CHOOSECOLORW>() as u32,
        hwndOwner: owner.unwrap_or_default(),
        rgbResult: result,
        lpCustColors: custom.as_mut_ptr(),
        Flags: flags,
        ..Default::default()
    };
    if !unsafe { ChooseColorW(&mut chooser) }.as_bool() {
        return None;
    }
    crate::dialog::set_custom_colors(custom.map(|colorref| Color::from_colorref(colorref.0)));
    Some(Color::from_colorref(chooser.rgbResult.0))
}

