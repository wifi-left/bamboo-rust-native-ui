// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Windows notification platform (WinRT `ToastNotificationManager`).
//!
//! The window thread initializes WinRT next to OLE (`RoInitialize`); a thread
//! whose apartment cannot host it reports `false` from [`initialize`] and the
//! application falls back to a tray balloon.

use windows::core::{HSTRING, Interface, Result};
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::System::Com::{
    CoCreateInstance, CoTaskMemAlloc, IPersistFile, CLSCTX_INPROC_SERVER,
};
use windows::Win32::System::Com::StructuredStorage::{
    PropVariantClear, PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_SINGLETHREADED};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{
    IShellLinkW, SHGetFolderPathW, ShellLink, CSIDL_PROGRAMS, SHGFP_TYPE_CURRENT,
};

use crate::notification::Notification;

/// `PKEY_AppUserModel_ID` (`{9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}, 5`), the
/// property a Start-menu shortcut carries to register its AUMID for toasts.
const PKEY_APP_USER_MODEL_ID: PROPERTYKEY = PROPERTYKEY {
    fmtid: windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
    pid: 5,
};

pub(crate) fn initialize() -> bool {
    unsafe { RoInitialize(RO_INIT_SINGLETHREADED).is_ok() }
}

pub(crate) fn uninitialize() {
    unsafe { RoUninitialize() };
}

/// Show one toast. `app_id` is the AppUserModelID (`None` uses the process
/// one). Returns whether the shell accepted the notification.
pub(crate) fn show(notification: &Notification, app_id: Option<&str>) -> bool {
    let Ok(document) = toast_document(notification) else {
        return false;
    };
    let Ok(toast) = ToastNotification::CreateToastNotification(&document) else {
        return false;
    };
    if let Some(tag) = notification.tag_text() {
        let _ = toast.SetTag(&HSTRING::from(tag));
    }
    if let Some(group) = notification.group_text() {
        let _ = toast.SetGroup(&HSTRING::from(group));
    }
    let notifier = match app_id {
        Some(id) => ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(id)),
        None => ToastNotificationManager::CreateToastNotifier(),
    };
    match notifier {
        Ok(notifier) => notifier.Show(&toast).is_ok(),
        Err(_) => false,
    }
}

fn toast_document(notification: &Notification) -> Result<XmlDocument> {
    let document = XmlDocument::new()?;
    document.LoadXml(&HSTRING::from(notification.to_toast_xml()))?;
    Ok(document)
}

/// Set the process AUMID (taskbar grouping and the default toast identity).
pub(crate) fn set_process_app_id(app_id: &str) -> bool {
    unsafe {
        windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(app_id))
            .is_ok()
    }
}

/// Create (or refresh) the Start-menu shortcut that registers `app_id` with
/// the shell, which is what an unpackaged desktop application needs before
/// `CreateToastNotifierWithId` displays anything. The shortcut points at this
/// executable and carries `System.AppUserModel.ID`.
///
/// This writes one `.lnk` under the user's Programs folder; applications call
/// it once (settings/first run), not on every startup.
pub(crate) fn register_shortcut(app_id: &str, display_name: &str) -> bool {
    unsafe { register_shortcut_inner(app_id, display_name).is_ok() }
}

unsafe fn register_shortcut_inner(app_id: &str, display_name: &str) -> Result<()> {
    let executable = std::env::current_exe()?;
    let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }?;
    unsafe {
        link.SetPath(&HSTRING::from(executable.to_string_lossy().as_ref()))?;
        if let Some(directory) = executable.parent() {
            link.SetWorkingDirectory(&HSTRING::from(directory.to_string_lossy().as_ref()))?;
        }
        link.SetDescription(&HSTRING::from(display_name))?;
    }

    let store: IPropertyStore = link.cast()?;
    let mut value = string_prop(app_id);
    let stored = unsafe { store.SetValue(&PKEY_APP_USER_MODEL_ID, &value) }
        .and_then(|_| unsafe { store.Commit() });
    unsafe {
        let _ = PropVariantClear(&mut value);
    }
    stored?;

    let mut programs = [0u16; 260];
    unsafe {
        SHGetFolderPathW(
            None,
            CSIDL_PROGRAMS as i32,
            None,
            SHGFP_TYPE_CURRENT.0 as u32,
            &mut programs,
        )?;
    }
    let length = programs
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(programs.len());
    let folder = String::from_utf16_lossy(&programs[..length]);
    let file = std::path::Path::new(&folder).join(format!("{app_id}.lnk"));
    let persist: IPersistFile = link.cast()?;
    unsafe { persist.Save(&HSTRING::from(file.to_string_lossy().as_ref()), true) }?;
    Ok(())
}

/// A `VT_LPWSTR` PROPVARIANT owning a CoTaskMem copy of `value`.
fn string_prop(value: &str) -> PROPVARIANT {
    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let memory = CoTaskMemAlloc(wide.len() * 2) as *mut u16;
        if memory.is_null() {
            return PROPVARIANT::default();
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), memory, wide.len());
        PROPVARIANT {
            Anonymous: PROPVARIANT_0 {
                Anonymous: std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                    vt: VT_LPWSTR,
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: PROPVARIANT_0_0_0 {
                        pwszVal: windows::core::PWSTR(memory),
                    },
                }),
            },
        }
    }
}

