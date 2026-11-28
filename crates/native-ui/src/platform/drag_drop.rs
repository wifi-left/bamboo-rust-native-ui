// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! System-wide drag and drop (OLE `DoDragDrop`).
//!
//! A drag started here is a real shell drag: it leaves the window and the
//! process, Explorer and other applications can accept it, and the same window
//! can accept text and file lists dragged in from anywhere. The retained widget
//! tree sees nothing of COM: the host translates `IDropTarget` callbacks into
//! [`crate::Host`] methods (`drag_enter`/`drag_over`/`drag_leave`/`drag_drop`)
//! with client DIP coordinates, exactly like pointer events.

use std::mem::ManuallyDrop;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::ptr;

use windows::core::{implement, BOOL, Error, HRESULT, Ref};
use windows::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, DV_E_DVASPECT,
    DV_E_FORMATETC, DV_E_TYMED, E_NOTIMPL, GlobalFree, HGLOBAL, HWND, POINT, POINTL, S_OK,
};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::Com::{
    IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, DVASPECT_CONTENT,
    FORMATETC, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::System::Ole::{
    DoDragDrop, IDropSource, IDropSource_Impl, IDropTarget, IDropTarget_Impl, OleInitialize,
    OleUninitialize, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop, CF_HDROP, CF_UNICODETEXT,
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE, DROPEFFECT_NONE,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::{DragQueryFileW, DROPFILES, SHCreateStdEnumFmtEtc, HDROP};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWLP_USERDATA};

use crate::drag::{DragData, DropEffect};
use crate::geometry::Point;
use crate::platform::window::WindowState;

/// Initialize OLE for this thread. Repeated calls are reference counted and
/// balanced by [`uninitialize`]; a thread already in a conflicting apartment
/// gets `false` and simply cannot drag.
pub(crate) fn initialize() -> bool {
    unsafe { OleInitialize(None).is_ok() }
}

pub(crate) fn uninitialize() {
    unsafe { OleUninitialize() };
}

/// Register `hwnd` as a drop target. The returned object must be kept alive
/// until [`revoke`].
pub(crate) fn register(hwnd: HWND) -> Option<IDropTarget> {
    let target: IDropTarget = WindowDropTarget {
        hwnd: hwnd.0 as isize,
    }
    .into();
    match unsafe { RegisterDragDrop(hwnd, &target) } {
        Ok(()) => Some(target),
        Err(_) => None,
    }
}

pub(crate) fn revoke(hwnd: HWND) {
    unsafe {
        let _ = RevokeDragDrop(hwnd);
    }
}

/// Run a system drag for `data` and return what the drop reported. Blocks in
/// OLE's own message loop (the window keeps painting and receiving drop-target
/// callbacks there).
pub(crate) fn begin_drag(_hwnd: HWND, data: DragData) -> DropEffect {
    if data.is_empty() {
        return DropEffect::None;
    }
    let object: IDataObject = DataObject { data: data.clone() }.into();
    let source: IDropSource = DataObject { data }.into();
    let mut effect = DROPEFFECT_NONE;
    let hr = unsafe {
        DoDragDrop(
            &object,
            &source,
            DROPEFFECT_COPY | DROPEFFECT_MOVE,
            &mut effect,
        )
    };
    if hr == DRAGDROP_S_DROP {
        from_ole(effect)
    } else {
        DropEffect::None
    }
}

fn from_ole(effect: DROPEFFECT) -> DropEffect {
    if effect.contains(DROPEFFECT_MOVE) {
        DropEffect::Move
    } else if effect.contains(DROPEFFECT_COPY) {
        DropEffect::Copy
    } else if effect.0 & 0x4 != 0 {
        DropEffect::Link
    } else {
        DropEffect::None
    }
}

fn to_ole(effect: DropEffect) -> DROPEFFECT {
    match effect {
        DropEffect::None => DROPEFFECT_NONE,
        DropEffect::Copy => DROPEFFECT_COPY,
        DropEffect::Move => DROPEFFECT_MOVE,
        DropEffect::Link => DROPEFFECT_LINK,
    }
}

/// The `FORMATETC` this data object offers.
fn format_for(data: &DragData) -> FORMATETC {
    FORMATETC {
        cfFormat: match data {
            DragData::Text(_) => CF_UNICODETEXT.0,
            DragData::Files(_) => CF_HDROP.0,
        },
        ptd: ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

fn supported(data: &DragData, format: &FORMATETC) -> bool {
    let expected = format_for(data);
    format.cfFormat == expected.cfFormat
        && format.dwAspect == expected.dwAspect
        && format.lindex == expected.lindex
        && (format.tymed & expected.tymed) != 0
}

fn allocate(bytes: &[u8]) -> Option<HGLOBAL> {
    unsafe {
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).ok()?;
        let pointer = GlobalLock(handle);
        if pointer.is_null() {
            let _ = GlobalFree(Some(handle));
            return None;
        }
        ptr::copy_nonoverlapping(bytes.as_ptr(), pointer as *mut u8, bytes.len());
        let _ = GlobalUnlock(handle);
        Some(handle)
    }
}

fn text_handle(text: &str) -> Option<HGLOBAL> {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    let bytes = unsafe {
        std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2)
    };
    allocate(bytes)
}

fn files_handle(paths: &[PathBuf]) -> Option<HGLOBAL> {
    let mut wide: Vec<u16> = Vec::new();
    for path in paths {
        wide.extend(path.as_os_str().encode_wide());
        wide.push(0);
    }
    wide.push(0);
    let header = std::mem::size_of::<DROPFILES>();
    let mut bytes = vec![0u8; header + wide.len() * 2];
    let drop_files = bytes.as_mut_ptr() as *mut DROPFILES;
    unsafe {
        (*drop_files).pFiles = header as u32;
        (*drop_files).pt = POINT::default();
        (*drop_files).fNC = BOOL(0);
        (*drop_files).fWide = BOOL(1);
        ptr::copy_nonoverlapping(
            wide.as_ptr() as *const u8,
            bytes.as_mut_ptr().add(header),
            wide.len() * 2,
        );
    }
    allocate(&bytes)
}

fn medium(handle: HGLOBAL) -> STGMEDIUM {
    STGMEDIUM {
        tymed: TYMED_HGLOBAL.0 as u32,
        u: STGMEDIUM_0 { hGlobal: handle },
        pUnkForRelease: ManuallyDrop::new(None),
    }
}

/// A drag source's payload. Only what OLE can transport is offered: Unicode
/// text or a file list.
#[implement(IDataObject, IDropSource)]
struct DataObject {
    data: DragData,
}

impl IDataObject_Impl for DataObject_Impl {
    fn GetData(&self, format: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
        let format = unsafe { *format };
        if !supported(&self.data, &format) {
            return Err(Error::from(DV_E_FORMATETC));
        }
        let handle = match &self.data {
            DragData::Text(text) => text_handle(text).ok_or_else(|| Error::from(DV_E_TYMED))?,
            DragData::Files(paths) => files_handle(paths).ok_or_else(|| Error::from(DV_E_TYMED))?,
        };
        Ok(medium(handle))
    }

    fn GetDataHere(
        &self,
        _format: *const FORMATETC,
        _medium: *mut STGMEDIUM,
    ) -> windows::core::Result<()> {
        Err(Error::from(E_NOTIMPL))
    }

    fn QueryGetData(&self, format: *const FORMATETC) -> HRESULT {
        let format = unsafe { *format };
        if supported(&self.data, &format) {
            S_OK
        } else if format.dwAspect != DVASPECT_CONTENT.0 {
            DV_E_DVASPECT
        } else if (format.tymed & (TYMED_HGLOBAL.0 as u32)) == 0 {
            DV_E_TYMED
        } else {
            DV_E_FORMATETC
        }
    }

    fn GetCanonicalFormatEtc(
        &self,
        _input: *const FORMATETC,
        _output: *mut FORMATETC,
    ) -> HRESULT {
        E_NOTIMPL
    }

    fn SetData(
        &self,
        _format: *const FORMATETC,
        _medium: *const STGMEDIUM,
        _release: BOOL,
    ) -> windows::core::Result<()> {
        Err(Error::from(E_NOTIMPL))
    }

    fn EnumFormatEtc(&self, direction: u32) -> windows::core::Result<IEnumFORMATETC> {
        const DATADIR_GET: u32 = 1;
        if direction != DATADIR_GET {
            return Err(Error::from(E_NOTIMPL));
        }
        let format = format_for(&self.data);
        unsafe { SHCreateStdEnumFmtEtc(&[format]) }
    }

    fn DAdvise(
        &self,
        _format: *const FORMATETC,
        _flags: u32,
        _sink: Ref<IAdviseSink>,
    ) -> windows::core::Result<u32> {
        Err(Error::from(E_NOTIMPL))
    }

    fn DUnadvise(&self, _connection: u32) -> windows::core::Result<()> {
        Err(Error::from(E_NOTIMPL))
    }

    fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> {
        Err(Error::from(E_NOTIMPL))
    }
}

impl IDropSource_Impl for DataObject_Impl {
    fn QueryContinueDrag(
        &self,
        escape: BOOL,
        keys: MODIFIERKEYS_FLAGS,
    ) -> HRESULT {
        if escape.as_bool() {
            DRAGDROP_S_CANCEL
        } else if (keys.0 & MK_LBUTTON.0) == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _effect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

/// The window's drop target. COM objects live on the UI thread; every callback
/// recovers the window state from the HWND's user data and forwards a
/// normalized event to the application.
#[implement(IDropTarget)]
struct WindowDropTarget {
    hwnd: isize,
}

fn state<R>(hwnd: isize, callback: impl FnOnce(&mut WindowState) -> R) -> Option<R> {
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    if hwnd.0.is_null() {
        return None;
    }
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowState;
    if pointer.is_null() {
        return None;
    }
    Some(callback(unsafe { &mut *pointer }))
}

fn client_position(hwnd: isize, point: POINTL) -> Point {
    let window = HWND(hwnd as *mut core::ffi::c_void);
    let mut client = POINT { x: point.x, y: point.y };
    unsafe {
        let _ = ScreenToClient(window, &mut client);
    }
    let scale = state(hwnd, |state| state.scale()).unwrap_or(1.0);
    Point::new(client.x as f32 / scale, client.y as f32 / scale)
}

impl IDropTarget_Impl for WindowDropTarget_Impl {
    fn DragEnter(
        &self,
        data_object: Ref<IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let Ok(object) = data_object.ok() else {
            unsafe { *effect = DROPEFFECT_NONE };
            return Ok(());
        };
        let Some(data) = read_data(object) else {
            unsafe { *effect = DROPEFFECT_NONE };
            return Ok(());
        };
        let pos = client_position(self.hwnd, *point);
        let result = state(self.hwnd, |state| state.ole_drag_enter(&data, pos))
            .unwrap_or(DropEffect::None);
        unsafe { *effect = to_ole(result) };
        Ok(())
    }

    fn DragOver(
        &self,
        _keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let pos = client_position(self.hwnd, *point);
        let result = state(self.hwnd, |state| state.ole_drag_over(pos))
            .unwrap_or(DropEffect::None);
        unsafe { *effect = to_ole(result) };
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        state(self.hwnd, WindowState::ole_drag_leave);
        Ok(())
    }

    fn Drop(
        &self,
        data_object: Ref<IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let pos = client_position(self.hwnd, *point);
        let object = data_object.ok().ok();
        let result = match object.and_then(read_data) {
            Some(data) => state(self.hwnd, |state| state.ole_drag_drop(data, pos))
                .unwrap_or(DropEffect::None),
            None => {
                state(self.hwnd, WindowState::ole_drag_leave);
                DropEffect::None
            }
        };
        unsafe { *effect = to_ole(result) };
        Ok(())
    }
}

/// Read a system drag's payload, preferring a file list (Explorer drags carry
/// both a list and text; the files are what the user means).
fn read_data(object: &IDataObject) -> Option<DragData> {
    read_files(object)
        .map(DragData::Files)
        .or_else(|| read_text(object).map(DragData::Text))
}

fn get_medium(object: &IDataObject, format: FORMATETC) -> Option<STGMEDIUM> {
    unsafe { object.GetData(&format).ok() }
}

fn read_text(object: &IDataObject) -> Option<String> {
    let mut medium = get_medium(
        object,
        FORMATETC {
            cfFormat: CF_UNICODETEXT.0,
            ptd: ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        },
    )?;
    let handle = unsafe { medium.u.hGlobal };
    let pointer = unsafe { GlobalLock(handle) } as *const u16;
    let mut text = None;
    if !pointer.is_null() {
        let mut length = 0usize;
        unsafe {
            while *pointer.add(length) != 0 && length < (1 << 24) {
                length += 1;
            }
            text = Some(String::from_utf16_lossy(std::slice::from_raw_parts(
                pointer, length,
            )));
            let _ = GlobalUnlock(handle);
        }
    }
    unsafe { ReleaseStgMedium(&mut medium) };
    text
}

fn read_files(object: &IDataObject) -> Option<Vec<PathBuf>> {
    let mut medium = get_medium(
        object,
        FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        },
    )?;
    let handle = unsafe { medium.u.hGlobal };
    let drop = HDROP(handle.0);
    let count = unsafe { DragQueryFileW(drop, u32::MAX, None) };
    let mut paths = Vec::new();
    for index in 0..count {
        let length = unsafe { DragQueryFileW(drop, index, None) } as usize;
        if length == 0 {
            continue;
        }
        let mut buffer = vec![0u16; length + 1];
        let written = unsafe { DragQueryFileW(drop, index, Some(&mut buffer)) } as usize;
        if written > 0 {
            paths.push(PathBuf::from(std::ffi::OsString::from_wide(
                &buffer[..written],
            )));
        }
    }
    unsafe { ReleaseStgMedium(&mut medium) };
    (!paths.is_empty()).then_some(paths)
}
