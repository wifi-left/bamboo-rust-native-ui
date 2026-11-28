// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Win32 window host: class registration, message loop, DPI, timers and the
//! `WM_PAINT` double-buffer pipeline.
//!
//! The host owns one persistent back buffer and merges the dirty rectangles
//! handed to it by the application. Only their bounding box is blitted, so a
//! hover change that invalidates two small rectangles costs two small blits.

use std::ffi::c_void;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, ClientToScreen, CreateFontIndirectW,
    CreateFontW, DEFAULT_CHARSET, DeleteObject, EndPaint, FillRect, GetDC, GetMonitorInfoW,
    GetStockObject, GetTextExtentPoint32W, GetTextMetricsW, GetWindowDC, HDC, HFONT, HGDIOBJ,
    InvalidateRect,
    LOGFONTW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, OUT_DEFAULT_PRECIS,
    PAINTSTRUCT, ReleaseDC, ScreenToClient, SelectObject, SetBkColor, SetBkMode, SetTextColor,
    TextOutW, TRANSPARENT, TEXTMETRICW, UpdateWindow, DEFAULT_GUI_FONT,
};
use windows::Win32::System::DataExchange::{GetClipboardData, IsClipboardFormatAvailable};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::UI::Accessibility::{
    UiaRootObjectId,
    IRawElementProviderSimple, UiaClientsAreListening, UiaRaiseAutomationEvent,
    UiaReturnRawElementProvider, UIA_AutomationFocusChangedEventId,
};
use windows::Win32::UI::Controls::{
    DRAWITEMSTRUCT, MEASUREITEMSTRUCT, ODT_MENU, ODS_NOACCEL, ODS_SELECTED,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, GetDpiForWindow,
    GetSystemMetricsForDpi, SystemParametersInfoForDpi,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::Ime::{
    CANDIDATEFORM, CFS_CANDIDATEPOS, CFS_POINT, COMPOSITIONFORM, GCS_COMPSTR, GCS_CURSORPOS,
    GCS_RESULTSTR, ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext, ImmSetCandidateWindow,
    ImmSetCompositionFontW, ImmSetCompositionWindow, ISC_SHOWUICOMPOSITIONWINDOW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    RegisterHotKey, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT,
    TrackMouseEvent, UnregisterHotKey, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_LEFT, VK_LWIN, VK_MENU,
    VK_RETURN, VK_RIGHT, VK_RWIN, VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::Shell::{
    DragAcceptFiles, DragFinish, DragQueryFileW, HDROP,
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIIF_ERROR, NIIF_INFO, NIIF_NONE,
    NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NIN_SELECT, NOTIFYICONDATAW,
    NOTIFYICON_VERSION_4, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    CS_DBLCLKS, DispatchMessageW, EN_CHANGE, EN_KILLFOCUS, ES_AUTOHSCROLL, ES_AUTOVSCROLL,
    ES_CENTER, ES_LEFT, ES_MULTILINE, ES_NUMBER,
    ES_PASSWORD, ES_RIGHT, ES_WANTRETURN, GWL_EXSTYLE, GWLP_USERDATA, GWLP_WNDPROC, GW_HWNDPREV,
    GetClassNameW, GetForegroundWindow, GetMessageExtraInfo,
    GUITHREADINFO, GUI_INMENUMODE, GUI_POPUPMENUMODE, GUI_SYSTEMMENUMODE, GetClientRect,
    GetCaretBlinkTime, GetGUIThreadInfo, GetMenuItemRect, GetMessageW, GetSubMenu, GetWindow,
    GetWindowLongPtrW, GetWindowRect,
    GetWindowTextLengthW,
    GetWindowTextW, HWND_TOP, IDC_ARROW, IDC_HAND, IDC_IBEAM, IDC_SIZENS, IDC_SIZEWE, IDI_APPLICATION,
    IMAGE_ICON, IsIconic,
    IsWindowVisible, KillTimer, LR_LOADFROMFILE, LoadCursorW, LoadIconW, LoadImageW,
    MINMAXINFO, MSG, NONCLIENTMETRICSW, OBJID_CLIENT, PostMessageW,
    PostQuitMessage, RegisterClassW, SW_HIDE, SW_RESTORE, SW_SHOW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    GetSystemMetrics, SM_CXMENUCHECK, SM_CXSMICON, SM_CYMENU, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetCursor,
    SetForegroundWindow, SetTimer,
    SetWindowLongPtrW,
    SetWindowPos, SetWindowTextW, ShowWindow, SPI_GETCARETWIDTH, SPI_GETNONCLIENTMETRICS,
    SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    SystemParametersInfoW, TranslateMessage, WM_CANCELMODE,
    WM_CAPTURECHANGED,
    WM_ACTIVATE, WM_CHAR, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_CTLCOLOREDIT, WM_DESTROY,
    WM_DPICHANGED, WM_DROPFILES,
    WM_ERASEBKGND,
    WM_GETMINMAXINFO, WM_GETOBJECT, WM_HOTKEY, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION,
    WM_QUERYENDSESSION,
    WM_IME_SETCONTEXT, WM_IME_STARTCOMPOSITION, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS, WM_LBUTTONDBLCLK,
    WM_LBUTTONDOWN,
    WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MEASUREITEM, WM_MENUSELECT, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_MOVE, WM_NCCREATE,
    WM_NCACTIVATE, WM_NCDESTROY, WM_NCPAINT, WM_NULL, WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SETFOCUS,
    WM_SETFONT, WM_SETCURSOR,
    WM_SETTINGCHANGE,
    WM_SHOWWINDOW, WM_SYSCHAR, WM_SYSCOMMAND,
    WM_DRAWITEM, WM_SIZE, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WM_UNINITMENUPOPUP,
    WM_WINDOWPOSCHANGED,
    WNDCLASSW,
    WNDCLASS_STYLES, WNDPROC, WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD, WS_CLIPCHILDREN,
    WS_CAPTION, WS_EX_LAYERED, WS_EX_TRANSPARENT, WS_HSCROLL, WS_OVERLAPPED, WS_OVERLAPPEDWINDOW,
    WS_SYSMENU, WS_VSCROLL, CallWindowProcW,
    DestroyIcon, GetCursorPos, HICON, HMENU, RegisterWindowMessageW, SetMenu, DrawMenuBar,
    DestroyMenu, IsZoomed, SC_CLOSE, SW_MAXIMIZE, SW_MINIMIZE,
};
use windows::Win32::System::Ole::IDropTarget;
use windows::core::{PCWSTR, Result, w};

use crate::a11y::{A11yAction, A11yActionKind, AnnouncementPriority};
use crate::color::Rgba;
use crate::backend::TextAlign;
use crate::backend::Renderer;
use crate::backend::d2d::{D2dCanvas, D2dResources};
use crate::backend::gdi::{GdiCanvas, GdiDoubleBuffer, GdiResources};
use crate::dialog::{Dialog, DialogId, DialogResult, MessageIcon};
use crate::drag::{DragData, DropEffect};
use crate::event::{Cursor, Event, KeyCode, KeyEvent, MouseButton, PointerKind};
use crate::geometry::{Point, PxRect, Rect, Size};
use crate::host::{CloseReason, Host, PaintCtx, WindowVisualState};
use crate::hotkey::{Hotkey, HotkeyModifiers, is_modifier_vk};
use crate::menu::{Menu, MenuBarModel, MenuDecision, MenuEvent, MenuSource};
use crate::notification::{Notification, NotificationResult};
use crate::painter::Painter;
use crate::platform::dark_theme;
use crate::platform::dialog as native_dialog;
use crate::platform::drag_drop;
use crate::platform::notification as native_notification;
use crate::platform::menu as native_menu;
use crate::platform::tooltip::{self, TooltipWindow};
use crate::platform::uia::{
    A11yRequest, A11yTree, UiaProvider, WM_APP_A11Y_ACTION, announce, raise_changes,
};
use crate::tray::{TrayAction, TrayEvent, TrayIcon, TrayImage};
use crate::ui::{ContextMenuRequest, TooltipAnchor, TooltipRequest, UiHost};
use crate::widget::{EditRequest, WidgetId};

/// `WM_MOUSELEAVE` is declared in the controls module; keep the numeric value
/// local so this crate does not need that feature.
const WM_MOUSELEAVE: u32 = 0x02A3;
const ANIMATION_TIMER: usize = 1;
const OCCLUSION_TIMER: usize = 2;
const TOOLTIP_DWELL_TIMER: usize = 3;
const TOOLTIP_HIDE_TIMER: usize = 4;
const TOOLTIP_ANIM_TIMER: usize = 5;
const CARET_TIMER: usize = 6;
/// Touch long-press dwell timer (a press held without moving picks a word).
const LONG_PRESS_TIMER: usize = 7;
/// Windows' own touch selection waits about half a second.
const LONG_PRESS_MS: u32 = 500;
/// Pointer travel (DIPs) that turns a touch press into a selection drag and
/// cancels the pending long press.
const LONG_PRESS_SLOP: f32 = 8.0;
/// `MI_WP_SIGNATURE`: touch and pen input promoted to mouse messages carries
/// this in the high 24 bits of `GetMessageExtraInfo`; bit `0x80` of the low
/// byte is set for touch, clear for pen (Win32 "System Events and Mouse
/// Messages").
const POINTER_SIGNATURE: u32 = 0xFF51_5700;
const POINTER_SIGNATURE_MASK: u32 = 0xFFFF_FF00;
const POINTER_TOUCH_BIT: u32 = 0x80;
/// Most separate dirty rectangles the host keeps for one paint. Distant
/// pieces stay separate so a few changed widgets never pull the page between
/// them into a repaint; only past this many does it collapse to a bounding
/// box.
const MAX_DIRTY_PIECES: usize = 32;
const TOOLTIP_DWELL_MS: u32 = 450;
const TOOLTIP_HIDE_MS: u32 = 5000;
/// Layered-alias step per ~15 ms tick: about 95 ms for a full fade.
const TOOLTIP_FADE_STEP: u8 = 40;
const HTCLIENT_CODE: u32 = 1;

// Edit-control messages; kept local alongside `WM_MOUSELEAVE` so the crate
// does not need the `Win32_UI_Controls` feature.
const EM_SETSEL: u32 = 0x00B1;
const EM_SETMARGINS: u32 = 0x00D3;
const EM_SETCUEBANNER: u32 = 0x1501;
const EM_SETLIMITTEXT: u32 = 0x00C5;
const EC_LEFTMARGIN: usize = 0x0001;
const EC_RIGHTMARGIN: usize = 0x0002;
/// Posted to the window once an editor is hidden so the child window can be
/// destroyed outside its own `WM_KILLFOCUS` handler.
const WM_APP_EDIT_TRASH: u32 = 0x8000 + 1;
/// Posted when `Ui` asks for a context menu; the popup is shown from the
/// message loop, never inside a `Host` callback. (0x8002 belongs to UIA.)
const WM_APP_SHOW_MENU: u32 = 0x8000 + 3;
/// Posted by a dialog worker thread once its dialog closed.
const WM_APP_DIALOG_DONE: u32 = 0x8000 + 4;
/// Notification message for the tray icon (`uCallbackMessage`).
const WM_APP_TRAY: u32 = 0x8000 + 5;
/// Posted by `WindowHandle::menu_click`; the replay runs from the message loop.
const WM_APP_MENU_CLICK: u32 = 0x8000 + 6;
/// Posted when `Alt+key` names a window-menu-bar title; the popup is shown
/// from the message loop like every other menu.
const WM_APP_SHOW_BAR_MENU: u32 = 0x8000 + 7;
/// Posted immediately before a popup's blocking loop starts, so
/// `MenuEvent::Opened` reaches the application *while* the popup is up. The
/// menu's own nested loop dispatches it to this window.
const WM_APP_MENU_OPENED: u32 = 0x8000 + 8;
/// A popup is rebuilt at most this many times before a hook that edits it on
/// every open is considered a spin.
const MAX_MENU_REOPENS: u8 = 8;
/// `NIN_KEYSELECT` is not exported by the bindings; its value is stable.
const NIN_KEYSELECT: u32 = 0x0401;

/// The character of a letter/digit virtual key, for `Alt+key` menu mnemonics.
fn mnemonic_char(vk: u16) -> Option<char> {
    match vk {
        0x30..=0x39 => char::from_u32(vk as u32),
        0x41..=0x5A => char::from_u32((vk as u32) + 32),
        _ => None,
    }
}

fn class_name() -> PCWSTR {
    w!("BambooNativeUiWindow")
}

static DPI_AWARE: OnceLock<()> = OnceLock::new();
static CLASS_REGISTERED: OnceLock<bool> = OnceLock::new();

/// Owned copy of a widget's [`EditRequest`]; the request borrows widget data
/// and cannot outlive the callback that produced it.
struct EditParams {
    id: WidgetId,
    rect: Rect,
    text: String,
    cue: String,
    password: bool,
    multiline: bool,
    wrap: bool,
    scrollbars: crate::widget::ScrollBars,
    numbers_only: bool,
    align: TextAlign,
    font_size: Option<f32>,
    font_family: Option<String>,
    max_length: u32,
}

impl EditParams {
    fn from_request(request: &EditRequest<'_>) -> Self {
        Self {
            id: request.id,
            rect: request.rect,
            text: request.text.to_string(),
            cue: request.cue.to_string(),
            password: request.password,
            multiline: request.multiline,
            wrap: request.wrap,
            scrollbars: request.scrollbars,
            numbers_only: request.numbers_only,
            align: request.align,
            font_size: request.font_size,
            font_family: request.font_family.clone(),
            max_length: request.max_length,
        }
    }
}

/// One live native `EDIT` child.
struct EditSession {
    hwnd: HWND,
    id: WidgetId,
    /// Widget rectangle in client DIPs (for hit-testing and commits).
    rect: Rect,
    /// Child rectangle in client DIPs. Single-line editors are a vertically
    /// centered text strip, so the caret and text sit in the middle of the
    /// field instead of hugging its top edge.
    child: Rect,
    font: HFONT,
    /// Per-widget font overrides, kept so a DPI change recreates the same font.
    font_size: Option<f32>,
    font_family: Option<String>,
    /// Multiline editors keep Enter as a newline instead of committing.
    multiline: bool,
    dpi: f32,
}

/// Deferred work requested while a host callback was running. Editing must not
/// create, focus or destroy windows inside `Host::event`: those calls re-enter
/// the window procedure synchronously.
enum EditOp {
    Begin(EditParams),
    Move(Rect),
    End { commit: bool },
}

/// One reported tooltip target. `widget` + `item` identify it (pointer moves
/// inside the same row only update the anchor); `anchor` is the latest
/// placement.
struct TipTarget {
    text: String,
    widget: Rect,
    item: Option<usize>,
    anchor: TooltipAnchor,
}

/// Subclass state for a native `EDIT` child. The edit control owns its context
/// menu, so when the menu closes Windows invalidates the child (a text-height
/// strip), not the parent area the popup actually covered. `WM_UNINITMENUPOPUP`
/// lets us invalidate the whole parent instead.
struct EditSubclass {
    parent: HWND,
    old_proc: WNDPROC,
}

unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut EditSubclass;
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    if message == WM_UNINITMENUPOPUP {
        // Deferred to the message loop: painting synchronously here would
        // re-enter the parent's window procedure mid menu.
        unsafe {
            let _ = InvalidateRect(Some((*ptr).parent), None, false);
        }
    }
    if message == WM_NCDESTROY {
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
        let subclass = unsafe { Box::from_raw(ptr) };
        let result = match subclass.old_proc {
            Some(old) => unsafe { CallWindowProcW(Some(old), hwnd, message, wparam, lparam) },
            None => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        };
        drop(subclass);
        return result;
    }
    match unsafe { (*ptr).old_proc } {
        Some(old) => unsafe { CallWindowProcW(Some(old), hwnd, message, wparam, lparam) },
        None => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn install_edit_subclass(child: HWND, parent: HWND) {
    let state = Box::new(EditSubclass {
        parent,
        old_proc: None,
    });
    let ptr = Box::into_raw(state);
    unsafe {
        SetWindowLongPtrW(child, GWLP_USERDATA, ptr as isize);
        let old = SetWindowLongPtrW(
            child,
            GWLP_WNDPROC,
            edit_subclass_proc as *const () as isize,
        );
        if old == 0 {
            SetWindowLongPtrW(child, GWLP_USERDATA, 0);
            drop(Box::from_raw(ptr));
        } else {
            (*ptr).old_proc = std::mem::transmute::<isize, WNDPROC>(old);
        }
    }
}

/// Tooltip timing and popup owned by the host.
#[derive(Default)]
struct TooltipState {
    window: Option<Box<TooltipWindow>>,
    target: Option<TipTarget>,
    /// The dwell timer is running for `target`.
    armed: bool,
    /// The popup window is shown (possibly mid-fade).
    visible: bool,
    /// Current layered alpha and the value the animation is heading to.
    alpha: u8,
    target_alpha: u8,
    animating: bool,
}

/// One owner-drawn menu bar item: the label with its `&` access key parsed
/// once. A `Box` keeps its address stable while the native menu holds the
/// pointer as item data.
struct BarItemPaint {
    label: crate::mnemonic::MnemonicLabel,
    /// Model position (it matches the `HMENU` position, separators included).
    index: usize,
    separator: bool,
}

/// A native popup menu waiting for the message loop. The menu is cloned
/// because the request borrows widget properties that do not outlive the
/// current callback.
struct PendingMenu {
    widget: WidgetId,
    menu: Menu,
    /// Top-left corner in client DIPs.
    anchor: Point,
    /// Control rectangle the popup must not cover (client DIPs).
    avoid: Option<Rect>,
    /// Which path requested the popup (context menu, chrome, programmatic).
    source: MenuSource,
}

/// A menu selection chosen without a popup (`WindowHandle::menu_click`),
/// posted so the replay never runs inside another `Host` callback.
struct PendingMenuClick {
    widget: WidgetId,
    command: u32,
}

/// The popup that is currently up, kept in the window state so the application
/// can rebuild its model from a timer (`WindowHandle::edit_open_menu`) and so
/// `MenuEvent::Opened` can hand out the model while the user is looking at it.
struct OpenMenu {
    widget: Option<WidgetId>,
    source: MenuSource,
    menu: Menu,
    placement: native_menu::Placement,
    /// An edit closed the popup: rebuild and show it again.
    dirty: bool,
    /// Rebuild count, capped by [`MAX_MENU_REOPENS`].
    reopens: u8,
}

/// A closed dialog's result, posted from its worker thread.
struct DialogDone {
    id: DialogId,
    /// A modal dialog disabled this window; restoring it is the UI thread's
    /// job (the dialog thread may no longer have foreground rights).
    modal: bool,
    result: DialogResult,
}

/// One registered tray icon.
struct TrayState {
    /// Application id: the shell `NOTIFYICONDATAW.uID` and the value carried by
    /// [`TrayEvent`]. The primary icon is id 0.
    id: u32,
    tooltip: String,
    menu: Menu,
    /// Kept so a DPI change can reload the icon at the new small-icon size
    /// (a multi-size `.ico` picks the closest image to the request).
    image: TrayImage,
    icon: HICON,
    /// The icon came from a file and must be destroyed when replaced.
    owned: bool,
    /// `Shell_NotifyIcon(NIM_ADD)` succeeded; removed on destroy.
    added: bool,
}

pub(crate) struct WindowState {
    host: Option<Box<dyn Host>>,
    painter: Painter,
    gdi: GdiResources,
    /// Direct2D resources (factory, formats, geometry paths and the DC target
    /// bound to the back buffer). Only used when `renderer` is a D2D mode.
    d2d: D2dResources,
    /// Which renderer draws the back buffer (the buffer and dirty blit are
    /// the same in every mode).
    renderer: Renderer,
    buffer: GdiDoubleBuffer,
    dirty: Vec<Rect>,
    hwnd: HWND,
    instance: HINSTANCE,
    size: Size,
    min_size: Size,
    dpi: f32,
    /// Exposed client pieces; empty while fully covered or minimized.
    visible: Vec<Rect>,
    exposed: bool,
    cursor: Cursor,
    capturing: bool,
    /// Device driving the pointer (mouse, touch or pen), reported to the host
    /// whenever it changes so a painted editor can pick its touch model.
    pointer_kind: PointerKind,
    /// A touch press waiting out its long-press dwell, in client DIPs. Any
    /// travel past `LONG_PRESS_SLOP` or the release cancels it.
    long_press: Option<Point>,
    /// OLE is initialized on this thread for system drag and drop.
    ole: bool,
    /// WinRT is initialized on this thread for system notifications.
    winrt: bool,
    /// AppUserModelID for taskbar grouping and notifications.
    app_id: Option<String>,
    /// The window's registered `IDropTarget`; the system holds its own
    /// reference until `RevokeDragDrop`.
    drop_target: Option<IDropTarget>,
    /// The data the last `DragEnter` brought in, cleared by a drop/leave.
    ole_drag: Option<DragData>,
    /// A drag the application asked for, started from the message loop after
    /// the callback returns (never re-entering the host from inside `Ui`).
    pending_drag: Option<DragData>,
    /// Whether [`UiHost::start_drag`] may hand a gesture to OLE. The showcase
    /// turns it off for automated runs, where a synthetic pointer has no real
    /// mouse button to end the modal drag loop.
    system_drag: bool,
    /// Number of rectangle invalidations (diagnostics / bench).
    invalidations: u64,
    timer_ms: u32,
    tracking_leave: bool,
    /// Set by `flush`; the host paints synchronously after its callback.
    flush_requested: bool,
    /// The last paint was an overlay composite (the page is in the buffer).
    overlay_painted: bool,
    /// A resize/DPI change while an overlay is up needs one normal page
    /// repaint before compositing can resume (the back buffer was recreated).
    overlay_resync: bool,
    quit_code: i32,
    edit: Option<EditSession>,
    edit_ops: Vec<EditOp>,
    /// Hidden editors waiting for destruction plus the font they were using.
    edit_trash: Vec<(HWND, HFONT)>,
    trash_posted: bool,
    /// The gamepad raised the system on-screen keyboard for text entry. The
    /// keyboard is dismissed when the session or the window's activation goes
    /// away, so it never outlives the field it was opened for.
    soft_keyboard: bool,
    /// UI Automation snapshot built on `WM_GETOBJECT`.
    a11y: Option<Arc<A11yTree>>,
    /// The snapshot needs rebuilding after a callback returns.
    a11y_dirty: bool,
    /// Focus to announce once the snapshot is current.
    pending_a11y_focus: Option<Option<WidgetId>>,
    /// Last announced focus target (widget id + option/row index) so repeated
    /// `a11y_focus` calls do not re-announce the same element.
    a11y_focus_key: Option<(WidgetId, Option<usize>)>,
    /// Transient status text waiting for a UIA notification.
    pending_a11y_announce: Option<(String, AnnouncementPriority)>,
    /// Hover/focus tooltip: dwell, popup window and auto-hide.
    tooltip: TooltipState,
    /// Context menu waiting for the posted message, and whether it was posted.
    pending_menu: Option<PendingMenu>,
    menu_posted: bool,
    /// `TrackPopupMenu` is running (read by the self-test's timer).
    menu_open: bool,
    /// The popup currently up (its model can be edited and rebuilt live).
    open_menu: Option<OpenMenu>,
    /// `Opened` already reached the application for the current popup loop.
    menu_opened_fired: bool,
    /// The last popup's owner (`None` for the tray menu) and path, kept after
    /// it closes so `active_menu_widget` and `menu_click` can name it.
    menu_target: Option<(Option<WidgetId>, MenuSource)>,
    /// The native window menu bar model (under the title bar) and its
    /// installed `HMENU`. The window owns the menu; replacing it destroys the
    /// old tree.
    menu_bar: Option<MenuBarModel>,
    window_menu: Option<HMENU>,
    /// Application-registered window shortcuts (not system-wide): the chord
    /// fires while this window receives the key, before the page sees it.
    window_hotkeys: Vec<(Hotkey, u32)>,
    /// Accelerators derived from the installed menu bar's shortcut labels,
    /// rebuilt with the menu. A chord in `window_hotkeys` wins over the same
    /// chord here, so an application can override a bar accelerator.
    menu_hotkeys: Vec<(Hotkey, u32)>,
    /// Icon bitmaps referenced by the installed menu tree (`hbmpItem` is not
    /// owned by the menu); freed when the menu is replaced.
    bar_bitmaps: Vec<windows::Win32::Graphics::Gdi::HBITMAP>,
    /// Owner-drawn bar item data (stable addresses held as menu item data).
    bar_items: Vec<Box<BarItemPaint>>,
    /// Cached menu font for the owner-drawn bar: `(dpi, font, owned)`.
    bar_font: Option<(u32, HFONT, bool)>,
    /// Bar item the shell currently reports as hot/active (`ODS_SELECTED`).
    /// The strip painter highlights it; the whole strip is redrawn on every
    /// such change so the system's own hover rendering never survives.
    bar_selected: Option<usize>,
    /// The shell asked the last bar item draw to hide access keys
    /// (`ODS_NOACCEL`). The strip painter uses it for every label, exactly the
    /// per-item signal the shell gives a normal owner-drawn bar item.
    bar_no_accel: bool,
    /// Last `Alt+key` mnemonic used on the bar, so a repeated letter cycles
    /// between titles that share it (native menu-bar behavior).
    bar_mnemonic: Option<(char, usize)>,
    /// The user tapped Alt: title mnemonics stay active until a key that is
    /// not a mnemonic or a focus change leaves the mode. Mirrors the shell's
    /// menu mode for owner-drawn titles, whose text the shell cannot match.
    bar_armed: bool,
    /// An Alt press was forwarded to the shell and has not been released or
    /// followed by another key. Only a *tap* arms the titles; a chord held with
    /// Alt (`Alt+P`) is left to the application.
    bar_alt_down: bool,
    /// An Alt press without a chord is in flight; the release decides whether
    /// it was a tap that shows access-key underlines.
    alt_tap_pending: bool,
    /// Access-key underlines are showing because of an Alt tap. The next key,
    /// pointer press or focus change turns them off (`Event::Alt(false)`).
    alt_underlines: bool,
    /// Next [`DialogId`]; starts at 1 so ids are never 0.
    next_dialog_id: u32,
    /// Notification-area icons, keyed by application id (D93).
    trays: Vec<TrayState>,
    /// `Host::started` has run (window callbacks are gated until then).
    started: bool,
    /// `Host::loaded` has run (the first frame painted).
    loaded: bool,
    /// A close request is in flight: its reason for `WM_CLOSE`.
    close_reason: Option<CloseReason>,
    /// Last reported visual state (change detection for `WM_SIZE`).
    visual_state: WindowVisualState,
    /// Last reported show/hide and activation state.
    shown: bool,
    active: bool,
    /// Last reported frame position in screen DIPs (`WM_MOVE`).
    frame_pos: Point,
    /// Lifecycle events observed while a host callback held the host (a
    /// `hide()`/`minimize()` from inside `Host::timer` makes the shell send
    /// `WM_SHOWWINDOW`/`WM_SIZE` synchronously). They are dispatched right
    /// after the callback returns instead of being dropped.
    pending_lifecycle: Vec<LifecycleEvent>,    /// `TaskbarCreated` broadcast id; Explorer sends it after a restart so the
    /// icon can be re-registered.
    taskbar_created: u32,
    /// Registered global hotkeys: (`RegisterHotKey` id, hotkey).
    hotkeys: Vec<(i32, Hotkey)>,
    next_hotkey_id: i32,
    /// The next non-modifier key press becomes a captured shortcut instead of
    /// reaching the application.
    hotkey_capture: bool,
    /// A consumed key press still produces `WM_CHAR`/`WM_SYSCHAR`; drop it.
    swallow_char: bool,
    /// Caret rectangle of the focused framework-painted editor, in client
    /// DIPs. Drives the blink timer and the IME composition/candidate window.
    caret_rect: Option<Rect>,
    /// Current blink phase (true = caret drawn).
    caret_on: bool,
    /// The caret blink timer is running.
    caret_timer: bool,
}

/// One `Host` lifecycle notification deferred past a running callback.
#[derive(Clone, Debug)]
enum LifecycleEvent {
    Visibility(bool),
    VisualState(WindowVisualState),
    Moved(Point),
}

impl WindowState {
    pub(crate) fn scale(&self) -> f32 {
        self.dpi / 96.0
    }

    fn invalidate(&mut self, rect: Rect) {
        self.invalidate_impl(rect, false);
    }

    /// Invalidate even while covered. Used when a covering child window hides
    /// (the pixels must be repainted) and by the benchmark harness.
    fn invalidate_force(&mut self, rect: Rect) {
        self.invalidate_impl(rect, true);
    }

    fn invalidate_impl(&mut self, rect: Rect, force: bool) {
        if !force && !self.exposed {
            return;
        }
        let Some(rect) = rect.intersect(Rect::from_size(self.size)) else {
            return;
        };
        let pieces = if force || self.visible.is_empty() {
            vec![rect]
        } else {
            let mut pieces = Vec::new();
            for visible in &self.visible {
                if let Some(piece) = rect.intersect(*visible) {
                    pieces.push(piece);
                }
            }
            pieces
        };
        for piece in pieces {
            if piece.is_empty() {
                continue;
            }
            self.invalidations += 1;
            crate::geometry::merge_damage(&mut self.dirty, piece, MAX_DIRTY_PIECES);
            let px = piece.to_px(self.scale());
            let device = RECT {
                left: px.left - 1,
                top: px.top - 1,
                right: px.right + 1,
                bottom: px.bottom + 1,
            };
            unsafe {
                let _ = InvalidateRect(Some(self.hwnd), Some(&device), false);
            }
        }
    }

    fn invalidate_all(&mut self) {
        self.invalidate(Rect::from_size(self.size));
    }

    /// Invalidate the whole client even while `visible` excludes parts of it.
    /// Used when a popup (menu, tooltip) or another window uncovers pixels
    /// that ordinary invalidation would clip away.
    fn invalidate_all_force(&mut self) {
        self.invalidate_force(Rect::from_size(self.size));
    }

    /// Re-scan z-order and repaint when the exposed area grows.
    ///
    /// A partially covering popup (the native edit context menu) removes its
    /// area from `visible`; ordinary invalidations are clipped to that set, so
    /// when the popup closes there may be no queued repaint for the newly
    /// visible pixels. A shrinking exposed set (something moved on top) needs
    /// no repaint; a growing one forces a full frame.
    fn refresh_visibility(&mut self) {
        let previous = std::mem::take(&mut self.visible);
        self.scan_visibility();
        let grew = exposed_area(&self.visible) > exposed_area(&previous) + 0.5;
        if self.exposed && grew {
            if std::env::var_os("BAMBOO_UI_DEBUG").is_some() {
                eprintln!("bamboo-ui: exposed area grew, full repaint");
            }
            self.dirty.clear();
            self.invalidate_all_force();
        }
    }

    /// Exposed = client pieces not covered by opaque windows above us.
    ///
    /// Layered and click-through windows are never treated as opaque: their
    /// pixels may be partly transparent, so skipping what is below them would
    /// leave stale content.
    fn scan_visibility(&mut self) {
        let full = Rect::from_size(self.size);
        let visible = unsafe { IsWindowVisible(self.hwnd).as_bool() };
        let iconic = unsafe { IsIconic(self.hwnd).as_bool() };
        if !visible || iconic || full.is_empty() {
            if std::env::var_os("BAMBOO_UI_DEBUG").is_some() {
                eprintln!(
                    "bamboo-ui: scan cleared (visible={visible} iconic={iconic} size={:?})",
                    full.size()
                );
            }
            self.visible.clear();
            self.exposed = false;
            return;
        }
        let mut pieces = vec![full];
        let mut above = unsafe { GetWindow(self.hwnd, GW_HWNDPREV) }.ok();
        let mut guard = 0;
        while let Some(window) = above {
            guard += 1;
            if guard > 128 {
                break;
            }
            if let Some(cover) = occluder_rect(self.hwnd, window, self.scale(), full) {
                let mut next = Vec::new();
                for piece in &pieces {
                    next.extend(subtract(*piece, cover));
                }
                pieces = next;
                if pieces.is_empty() {
                    if std::env::var_os("BAMBOO_UI_DEBUG").is_some() {
                        let mut class = [0u16; 128];
                        let class_len = unsafe { GetClassNameW(window, &mut class) }.max(0) as usize;
                        let mut title = [0u16; 256];
                        let title_len = unsafe { GetWindowTextW(window, &mut title) }.max(0) as usize;
                        eprintln!(
                            "bamboo-ui: client covered by {window:?} rect={cover:?} class={} title={}",
                            String::from_utf16_lossy(&class[..class_len]),
                            String::from_utf16_lossy(&title[..title_len])
                        );
                    }
                    break;
                }
                // Over-drawing is safe, under-drawing is not: if the exposure
                // set fragments too much, repaint the whole client instead.
                if pieces.len() > 64 {
                    pieces = vec![full];
                    break;
                }
            }
            above = unsafe { GetWindow(window, GW_HWNDPREV) }.ok();
        }
        self.visible = pieces;
        self.exposed = !self.visible.is_empty();
    }

    fn set_timer(&mut self, interval: Option<Duration>) {
        let millis = interval
            .map(|value| value.as_millis().clamp(1, u32::MAX as u128) as u32)
            .unwrap_or(0);
        if millis == self.timer_ms {
            return;
        }
        unsafe {
            if millis > 0 {
                SetTimer(Some(self.hwnd), ANIMATION_TIMER, millis, None);
            } else {
                let _ = KillTimer(Some(self.hwnd), ANIMATION_TIMER);
            }
        }
        self.timer_ms = millis;
    }

    fn paint(&mut self, hdc: windows::Win32::Graphics::Gdi::HDC, rc: RECT) {
        let scale = self.scale();
        let full = Rect::from_size(self.size);
        let debug = std::env::var_os("BAMBOO_UI_DEBUG").is_some();
        let mut dirty = std::mem::take(&mut self.dirty);
        for rect in &mut dirty {
            *rect = rect.intersect(full).unwrap_or_default();
        }
        dirty.retain(|rect| !rect.is_empty());
        // An overlay (modal scrim) composites over the page pixels already in
        // the back buffer: nothing of the page is repainted and the whole
        // window is blitted. The first overlay frame blends the backdrop;
        // later ones only repaint the overlay widgets (D87). A resize/DPI
        // change recreates the buffer, so one normal page frame runs first.
        let host_overlay = self
            .host
            .as_ref()
            .map(|host| host.overlay_paint())
            .unwrap_or(false);
        let overlay = host_overlay && !self.overlay_resync;
        let overlay_first = overlay && !self.overlay_painted;
        if self.overlay_resync {
            self.overlay_resync = false;
        }
        // The resync frame paints the page *and* the scrim normally, so the
        // buffer is dimmed again and later frames continue from there.
        self.overlay_painted = overlay || host_overlay;
        if overlay {
            dirty.clear();
            dirty.push(full);
        }
        let update = (rc.right > rc.left && rc.bottom > rc.top)
            .then(|| {
                Rect::new(
                    rc.left as f32 / scale,
                    rc.top as f32 / scale,
                    rc.right as f32 / scale,
                    rc.bottom as f32 / scale,
                )
                .intersect(full)
                .unwrap_or_default()
            })
            .filter(|rect| !rect.is_empty());
        if dirty.is_empty() {
            if debug {
                eprintln!(
                    "bamboo-ui: paint with an empty queue (rcPaint={:?})",
                    (rc.left, rc.top, rc.right, rc.bottom)
                );
            }
            dirty.push(update.unwrap_or(full));
        } else if let Some(update) = update {
            // `InvalidateRect` inflates every queued piece by one pixel, so the
            // system's update rectangle should be contained in the queued
            // bounding box. Anything larger is an exposure we never queued (a
            // popup menu or another window just uncovered this area); repaint
            // once instead of leaving stale pixels.
            //
            // The tolerance is two pixels, not one: `Rect::to_px` rounds each
            // edge and at a fractional DPI scale (1.75, 1.5…) the system's
            // one-pixel inflation can land one pixel further out. With a
            // one-pixel tolerance every small invalidation (a caret blink, a
            // selection drag) looked like an exposure and escalated to a
            // full-window repaint, which is a repaint storm while selecting
            // text.
            let queued = dirty
                .iter()
                .copied()
                .reduce(Rect::union)
                .unwrap_or_default()
                .to_px(scale);
            let queued = PxRect {
                left: queued.left - 2,
                top: queued.top - 2,
                right: queued.right + 2,
                bottom: queued.bottom + 2,
            };
            let update = update.to_px(scale);
            let exposed = update.left < queued.left
                || update.top < queued.top
                || update.right > queued.right
                || update.bottom > queued.bottom;
            if exposed {
                if debug {
                    eprintln!(
                        "bamboo-ui: unqueued exposure -> full frame (update={:?} queued={:?})",
                        (update.left, update.top, update.right, update.bottom),
                        (
                            queued.left,
                            queued.top,
                            queued.right,
                            queued.bottom
                        )
                    );
                }
                dirty.clear();
                dirty.push(full);
            }
        }
        // Many overlapping pieces cost more blits than one frame, but the
        // threshold is deliberately high: partial repaints are the point.
        // Rectangle count is already bounded by `MAX_DIRTY_PIECES` and merges
        // keep overlapping pieces together, so the sum is a good area proxy.
        let covered: f32 = dirty
            .iter()
            .map(|rect| rect.width() * rect.height())
            .sum();
        let total = (full.width() * full.height()).max(1.0);
        if covered > total * 0.55 {
            dirty.clear();
            dirty.push(full);
        }
        // Keep the pieces. `Ui::paint` skips every widget that intersects none
        // of them, so two small changes far apart repaint two small regions
        // instead of the page-sized bounding box between them. Every widget is
        // painted in full (the back buffer and widgets agree), and only the
        // dirty pieces are copied to the window below, which is safe because a
        // pixel inside a dirty piece is covered only by widgets that intersect
        // that piece.
        let client = full.to_px(scale);
        let boxes: Vec<PxRect> = dirty
            .iter()
            .filter_map(|rect| rect.to_px(scale).intersect(client))
            .filter(|rect| !rect.is_empty())
            .collect();
        if boxes.is_empty() {
            return;
        }

        self.buffer
            .ensure(hdc, client.width().max(1), client.height().max(1));
        let Some(back) = self.buffer.dc() else {
            return;
        };

        let renderer = self.renderer;
        let client_w = client.width().max(1);
        let client_h = client.height().max(1);
        let mut d2d_failed = false;
        let caret = {
            let painter = &self.painter;
            // The back buffer and the dirty blit are identical in every mode;
            // only the drawing half of the frame changes.
            let mut canvas: Box<dyn crate::backend::Canvas> = match renderer {
                Renderer::Gdi => {
                    Box::new(GdiCanvas::new(back, &mut self.gdi, scale, self.dpi))
                }
                kind => match D2dCanvas::new(
                    back,
                    &mut self.d2d,
                    scale,
                    self.dpi,
                    client_w,
                    client_h,
                    kind,
                ) {
                    Some(canvas) => Box::new(canvas),
                    None => {
                        d2d_failed = true;
                        Box::new(GdiCanvas::new(back, &mut self.gdi, scale, self.dpi))
                    }
                },
            };
            let mut ctx = PaintCtx {
                painter,
                canvas: &mut *canvas,
                dirty: &dirty,
                size: self.size,
                dpi: self.dpi,
                editing: self.edit.as_ref().map(|session| session.id),
                caret_on: self.caret_on,
                overlay,
                overlay_first,
                caret: None,
            };
            let Some(mut host) = self.host.take() else {
                return;
            };
            host.paint(&mut ctx);
            self.host = Some(host);
            ctx.caret
        };
        if d2d_failed {
            if std::env::var_os("BAMBOO_UI_DEBUG").is_some() {
                eprintln!(
                    "bamboo-ui: {} renderer unavailable, falling back to GDI",
                    renderer.name()
                );
            }
            self.renderer = Renderer::Gdi;
        }
        self.update_caret_after_paint(caret);
        for piece in &boxes {
            self.buffer.blit(hdc, *piece);
        }
    }

    /// Run one host callback with the host temporarily taken, exactly like
    /// `dispatch`. Returns `None` when no host is installed or the host is
    /// already inside another callback (a system drag cannot be delivered
    /// re-entrantly; the drop target then reports `None`).
    fn with_host<R>(
        &mut self,
        callback: impl FnOnce(&mut dyn Host, &mut WindowHandle<'_>) -> R,
    ) -> Option<R> {
        let mut host = self.host.take()?;
        let hwnd = self.hwnd;
        let result = {
            let mut window = WindowHandle {
                state: self,
                hwnd,
            };
            callback(host.as_mut(), &mut window)
        };
        self.host = Some(host);
        Some(result)
    }

    pub(crate) fn ole_drag_enter(&mut self, data: &DragData, pos: Point) -> DropEffect {
        self.ole_drag = Some(data.clone());
        let data = data.clone();
        self.with_host(|host, window| host.drag_enter(&data, pos, window))
            .unwrap_or(DropEffect::None)
    }

    pub(crate) fn ole_drag_over(&mut self, pos: Point) -> DropEffect {
        let Some(data) = self.ole_drag.clone() else {
            return DropEffect::None;
        };
        self.with_host(|host, window| host.drag_over(&data, pos, window))
            .unwrap_or(DropEffect::None)
    }

    pub(crate) fn ole_drag_leave(&mut self) {
        self.ole_drag = None;
        let _ = self.with_host(|host, window| host.drag_leave(window));
    }

    pub(crate) fn ole_drag_drop(&mut self, data: DragData, pos: Point) -> DropEffect {
        self.ole_drag = None;
        self.with_host(|host, window| host.drag_drop(data, pos, window))
            .unwrap_or(DropEffect::None)
    }

    /// Start a drag the application requested. Runs from the message loop,
    /// never inside the callback that asked for it: OLE's modal loop calls the
    /// drop target back, and that must be able to reach the host.
    pub(crate) fn run_pending_drag(&mut self) {
        let Some(data) = self.pending_drag.take() else {
            return;
        };
        if !self.system_drag || !self.ole {
            return;
        }
        let effect = crate::platform::drag_drop::begin_drag(self.hwnd, data);
        self.ole_drag = None;
        let _ = self.with_host(|host, window| host.drag_finished(effect, window));
    }

    fn dispatch(&mut self, event: Event) -> bool {
        let Some(mut host) = self.host.take() else {
            return false;
        };
        let hwnd = self.hwnd;
        let handled = {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.event(&event, &mut window)
        };
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
        handled
    }

    fn dispatch_timer(&mut self) {
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.timer(&mut window);
        }
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
        if std::mem::take(&mut self.flush_requested) {
            unsafe {
                let _ = UpdateWindow(hwnd);
            }
        }
    }

    /// Run `f` with the host and a window handle (the same borrow rules as
    /// every callback: the host is taken out of the state).
    fn call_host<R>(
        &mut self,
        f: impl FnOnce(&mut Box<dyn Host>, &mut WindowHandle<'_>) -> R,
    ) -> Option<R> {
        let mut host = self.host.take()?;
        let hwnd = self.hwnd;
        let result = {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            f(&mut host, &mut window)
        };
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
        Some(result)
    }

    fn dispatch_close_request(&mut self, reason: CloseReason) -> bool {
        self.call_host(|host, window| host.close_requested(reason, window))
            .unwrap_or(true)
    }

    fn dispatch_loaded(&mut self) {
        self.call_host(|host, window| host.loaded(window));
    }

    fn dispatch_visibility(&mut self, visible: bool) {
        if self.host.is_none() {
            self.pending_lifecycle.push(LifecycleEvent::Visibility(visible));
            return;
        }
        self.call_host(|host, window| host.visibility_changed(visible, window));
    }

    fn dispatch_activation(&mut self, active: bool) {
        self.call_host(|host, window| host.activated(active, window));
    }

    fn dispatch_visual_state(&mut self, state: WindowVisualState) {
        if self.host.is_none() {
            self.pending_lifecycle.push(LifecycleEvent::VisualState(state));
            return;
        }
        self.call_host(|host, window| host.visual_state_changed(state, window));
    }

    fn dispatch_moved(&mut self, position: Point) {
        if self.host.is_none() {
            self.pending_lifecycle.push(LifecycleEvent::Moved(position));
            return;
        }
        self.call_host(|host, window| host.moved(position, window));
    }

    fn dispatch_files(&mut self, paths: Vec<String>) {
        self.call_host(|host, window| host.files_dropped(paths, window));
    }

    /// Create the Direct2D target before the first frame. Device creation can
    /// take hundreds of milliseconds; doing it here keeps the cost out of the
    /// first exposed frame (and out of a native menu's modal loop, where a
    /// lazily created device used to stall the UIA probe). A failure switches
    /// the window to GDI for good.
    fn prewarm_renderer(&mut self) {
        if self.renderer == Renderer::Gdi {
            return;
        }
        let client = Rect::from_size(self.size).to_px(self.scale());
        let width = client.width().max(1);
        let height = client.height().max(1);
        let hdc = unsafe { GetDC(Some(self.hwnd)) };
        if hdc.is_invalid() {
            return;
        }
        self.buffer.ensure(hdc, width, height);
        unsafe {
            let _ = ReleaseDC(Some(self.hwnd), hdc);
        }
        let Some(back) = self.buffer.dc() else {
            return;
        };
        let kind = self.renderer;
        let (scale, dpi) = (self.scale(), self.dpi);
        let ready = D2dCanvas::new(back, &mut self.d2d, scale, dpi, width, height, kind).is_some();
        if !ready {
            if std::env::var_os("BAMBOO_UI_DEBUG").is_some() {
                eprintln!(
                    "bamboo-ui: {} renderer unavailable, falling back to GDI",
                    kind.name()
                );
            }
            self.renderer = Renderer::Gdi;
        }
    }

    /// Deliver lifecycle events that arrived while a host callback held the
    /// host. Runs outside callbacks (the message loop calls it after each
    /// message), so the host is available again.
    fn flush_lifecycle(&mut self) {
        if self.host.is_none() {
            return;
        }
        let delayed = std::mem::take(&mut self.pending_lifecycle);
        for event in delayed {
            match event {
                LifecycleEvent::Visibility(visible) => self.dispatch_visibility(visible),
                LifecycleEvent::VisualState(state) => self.dispatch_visual_state(state),
                LifecycleEvent::Moved(position) => self.dispatch_moved(position),
            }
        }
    }

    fn dispatch_destroying(&mut self) {
        self.call_host(|host, window| host.destroying(window));
    }

    fn dispatch_resize(&mut self) {
        let size = self.size;
        let dpi = self.dpi;
        // The back buffer is recreated for the new size, so an overlay must
        // repaint its page once before it can composite again.
        self.overlay_resync = true;
        let Some(mut host) = self.host.take() else {
            return;
        };
        let hwnd = self.hwnd;
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.resize(size, dpi, &mut window);
        }
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
    }

    fn dispatch_dpi(&mut self) {
        let dpi = self.dpi;
        self.overlay_resync = true;
        // The notification area asks for a small-icon size that follows the
        // monitor DPI; reload a file icon so a multi-size .ico stays crisp.
        self.reload_tray_icons();
        let Some(mut host) = self.host.take() else {
            return;
        };
        let hwnd = self.hwnd;
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.dpi_changed(dpi, &mut window);
        }
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
    }
}

/// Caret blink and IME placement for framework-painted editors.
///
/// The `Ui` reports the focused editor's caret rectangle after input and after
/// every paint. The host owns the blink timer and tells IMM where the caret is,
/// so the candidate window opens next to it instead of at the window origin.
impl WindowState {
    fn set_caret_rect(&mut self, rect: Option<Rect>) {
        self.caret_rect = rect;
        self.caret_on = true;
        match rect {
            Some(rect) => {
                self.start_caret_timer();
                self.invalidate(rect);
                self.apply_ime_position();
            }
            None => self.stop_caret_timer(),
        }
    }

    /// Store the caret rectangle a paint just produced. Does not restart the
    /// blink phase (that would freeze it on the visible frame).
    fn update_caret_after_paint(&mut self, rect: Option<Rect>) {
        if self.caret_rect == rect {
            return;
        }
        self.caret_rect = rect;
        match rect {
            Some(_) => {
                if !self.caret_timer {
                    self.start_caret_timer();
                }
                self.apply_ime_position();
            }
            None => self.stop_caret_timer(),
        }
    }

    fn blink_interval_ms() -> u32 {
        let millis = unsafe { GetCaretBlinkTime() };
        if millis == 0 || millis == u32::MAX {
            500
        } else {
            millis.max(100)
        }
    }

    fn start_caret_timer(&mut self) {
        unsafe {
            SetTimer(Some(self.hwnd), CARET_TIMER, Self::blink_interval_ms(), None);
        }
        self.caret_timer = true;
    }

    /// Show or dismiss the system on-screen keyboard a gamepad asked for. The
    /// flag makes the dismissal idempotent and lets the native editor's close
    /// path (which the framework does not see) put the keyboard away too.
    /// Returns whether the request was accepted, so a machine without the
    /// touch keyboard does not leave the pad waiting for one.
    fn set_soft_keyboard(&mut self, visible: bool) -> bool {
        if visible {
            // Never short-circuit a show request on the local flag: the shell
            // keyboard can be dismissed by its own close button while the
            // flag still says "ours", and re-showing it is idempotent.
            let shown = crate::platform::keyboard::show(crate::platform::keyboard::Layout::Gamepad);
            self.soft_keyboard = shown;
            shown
        } else {
            if self.soft_keyboard {
                self.soft_keyboard = false;
                crate::platform::keyboard::hide();
            }
            true
        }
    }

    fn dismiss_soft_keyboard(&mut self) {
        if std::mem::take(&mut self.soft_keyboard) {
            crate::platform::keyboard::hide();
        }
    }

    fn stop_caret_timer(&mut self) {
        if self.caret_timer {
            unsafe {
                let _ = KillTimer(Some(self.hwnd), CARET_TIMER);
            }
            self.caret_timer = false;
        }
        self.caret_on = true;
    }

    fn blink_caret(&mut self) {
        if self.caret_rect.is_none() {
            return;
        }
        self.caret_on = !self.caret_on;
        if let Some(rect) = self.caret_rect {
            self.invalidate(rect);
        }
    }

    /// Report the device driving the pointer when it changes. Touch and pen
    /// input is promoted to mouse messages by the shell; `GetMessageExtraInfo`
    /// tags those with `MI_WP_SIGNATURE`, and bit `0x80` separates touch from
    /// pen. A plain mouse stays silent (no event, no repaint).
    fn sync_pointer_kind(&mut self) {
        let kind = pointer_kind_now();
        if kind == self.pointer_kind {
            return;
        }
        self.pointer_kind = kind;
        self.dispatch(Event::PointerKind(kind));
    }

    /// A touch pressed: wait out the long-press dwell. The timer fires
    /// [`Event::LongPress`] if the finger stays put; movement or the release
    /// cancels it, so a drag stays a selection drag.
    fn arm_long_press(&mut self, pos: Point) {
        if self.pointer_kind != PointerKind::Touch {
            return;
        }
        self.long_press = Some(pos);
        unsafe {
            SetTimer(Some(self.hwnd), LONG_PRESS_TIMER, LONG_PRESS_MS, None);
        }
    }

    fn cancel_long_press(&mut self) {
        if self.long_press.take().is_some() {
            unsafe {
                let _ = KillTimer(Some(self.hwnd), LONG_PRESS_TIMER);
            }
        }
    }

    /// Move tracking while a touch press is down: past the slop it is a
    /// selection drag, not a word pick.
    fn long_press_moved(&mut self, pos: Point) {
        let Some(start) = self.long_press else {
            return;
        };
        let dx = pos.x - start.x;
        let dy = pos.y - start.y;
        if dx * dx + dy * dy > LONG_PRESS_SLOP * LONG_PRESS_SLOP {
            self.cancel_long_press();
        }
    }

    /// The dwell expired: hand the held position to the host.
    fn fire_long_press(&mut self) {
        let Some(pos) = self.long_press.take() else {
            return;
        };
        unsafe {
            let _ = KillTimer(Some(self.hwnd), LONG_PRESS_TIMER);
        }
        self.dispatch(Event::LongPress { pos });
    }

    /// Point the IME's composition window and candidate list at the caret.
    /// Coordinates are client pixels, like every IMM form.
    fn apply_ime_position(&self) {
        let Some(rect) = self.caret_rect else {
            return;
        };
        let scale = self.scale();
        let point = POINT {
            x: ((rect.left * scale).round() as i32).max(0),
            y: ((rect.bottom * scale).round() as i32).max(0),
        };
        unsafe {
            let himc = ImmGetContext(self.hwnd);
            if himc.0.is_null() {
                return;
            }
            let form = COMPOSITIONFORM {
                dwStyle: CFS_POINT,
                ptCurrentPos: point,
                rcArea: RECT::default(),
            };
            let _ = ImmSetCompositionWindow(himc, &form);
            let candidate = CANDIDATEFORM {
                dwIndex: 0,
                dwStyle: CFS_CANDIDATEPOS,
                ptCurrentPos: point,
                rcArea: RECT::default(),
            };
            let _ = ImmSetCandidateWindow(himc, &candidate);
            self.apply_ime_font(himc, scale);
            let _ = ImmReleaseContext(self.hwnd, himc);
        }
    }

    /// Give the IME the same face as the field so the candidate list looks
    /// like the text being edited.
    fn apply_ime_font(&self, himc: windows::Win32::UI::Input::Ime::HIMC, scale: f32) {
        let body = self.painter.body();
        let family = crate::backend::gdi::family_name(&body.face);
        let mut logfont = LOGFONTW {
            lfHeight: -((body.size * scale).round().max(1.0) as i32),
            lfWeight: body.weight.to_windows(),
            lfCharSet: DEFAULT_CHARSET,
            lfQuality: CLEARTYPE_QUALITY,
            ..Default::default()
        };
        for (index, unit) in family.encode_utf16().take(31).enumerate() {
            logfont.lfFaceName[index] = unit;
        }
        unsafe {
            let _ = ImmSetCompositionFontW(himc, &logfont);
        }
    }

    /// Read the current composition or result string (UTF-16 bytes from IMM).
    fn ime_composition_string(
        &self,
        himc: windows::Win32::UI::Input::Ime::HIMC,
        kind: windows::Win32::UI::Input::Ime::IME_COMPOSITION_STRING,
    ) -> String {
        unsafe {
            let bytes = ImmGetCompositionStringW(himc, kind, None, 0);
            if bytes <= 0 {
                return String::new();
            }
            let mut buffer = vec![0u16; bytes as usize / 2 + 1];
            let read = ImmGetCompositionStringW(
                himc,
                kind,
                Some(buffer.as_mut_ptr() as *mut std::ffi::c_void),
                (buffer.len() * 2) as u32,
            );
            if read <= 0 {
                return String::new();
            }
            let mut text =
                String::from_utf16_lossy(&buffer[..(read as usize / 2).min(buffer.len())]);
            // IMEs occasionally hand back CRLF; editors store LF.
            if text.contains('\r') {
                text = crate::text::normalize_newlines(&text).into_owned();
            }
            text
        }
    }

    /// Read `CF_UNICODETEXT` from the clipboard. Called from a host callback,
    /// so the owner window is this one and no nested loop runs.
    fn read_clipboard(&self) -> Option<String> {
        use windows::Win32::Foundation::HGLOBAL;
        use windows::Win32::System::DataExchange::{CloseClipboard, OpenClipboard};
        use windows::Win32::System::Ole::CF_UNICODETEXT;
        unsafe {
            if OpenClipboard(Some(self.hwnd)).is_err() {
                return None;
            }
            let text = (|| {
                IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).ok()?;
                let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
                if handle.0.is_null() {
                    return None;
                }
                let global = HGLOBAL(handle.0);
                let units = GlobalSize(global) / std::mem::size_of::<u16>();
                if units == 0 {
                    return None;
                }
                let ptr = GlobalLock(global) as *const u16;
                if ptr.is_null() {
                    return None;
                }
                let mut len = 0;
                while len < units && *ptr.add(len) != 0 {
                    len += 1;
                }
                let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
                let _ = GlobalUnlock(global);
                Some(text)
            })();
            let _ = CloseClipboard();
            text
        }
    }

    fn dispatch_ime_compose(&mut self, text: String, cursor: usize) {
        self.dispatch_ime(Some((text, cursor)), None);
    }

    fn dispatch_ime_commit(&mut self, text: String) {
        self.dispatch_ime(None, Some(text));
    }

    fn dispatch_ime(&mut self, compose: Option<(String, usize)>, commit: Option<String>) {
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            if let Some((text, cursor)) = compose.as_ref() {
                host.ime_compose(text, *cursor, &mut window);
            }
            if let Some(text) = commit.as_ref() {
                host.ime_commit(text, &mut window);
            }
        }
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
        if std::mem::take(&mut self.flush_requested) {
            unsafe {
                let _ = UpdateWindow(hwnd);
            }
        }
    }
}

/// Native `EDIT` session management.
///
/// The session is a borderless child control aligned to the widget rectangle.
/// The widget keeps painting its own text underneath, so nothing depends on
/// the control being present; `WS_CLIPCHILDREN` keeps blits out of its area.
impl WindowState {
    fn queue_edit(&mut self, op: EditOp) {
        self.edit_ops.push(op);
    }

    /// Apply queued edit work. Host callbacks can run here (commit/cancel
    /// notifications), which may queue more work; a small guard bounds the
    /// cascade in case an application re-opens an editor from its callback.
    fn process_edit_ops(&mut self) {
        let mut guard = 0;
        while !self.edit_ops.is_empty() {
            guard += 1;
            if guard > 16 {
                self.edit_ops.clear();
                break;
            }
            let ops = std::mem::take(&mut self.edit_ops);
            for op in ops {
                match op {
                    EditOp::Begin(params) => self.open_edit(params),
                    EditOp::Move(rect) => self.move_edit(rect),
                    EditOp::End { commit } => self.close_edit(commit),
                }
            }
        }
    }

    /// Id of the editor that is open or about to open, including queued work.
    fn editing_id(&self) -> Option<WidgetId> {
        let mut id = self.edit.as_ref().map(|session| session.id);
        for op in &self.edit_ops {
            match op {
                EditOp::Begin(params) => id = Some(params.id),
                EditOp::End { .. } => id = None,
                EditOp::Move(_) => {}
            }
        }
        id
    }

    fn open_edit(&mut self, params: EditParams) {
        // The native child owns the caret from now on.
        self.set_caret_rect(None);
        if std::env::var_os("BAMBOO_UI_DEBUG").is_some() {
            eprintln!(
                "bamboo-ui: open_edit id={:?} rect={:?} scale={}",
                params.id,
                params.rect,
                self.scale()
            );
        }
        if let Some(old) = self.edit.take() {
            self.retire_edit(old);
        }
        let scale = self.scale();
        let font = self.edit_font(scale, params.font_size, params.font_family.as_deref());
        let child = edit_child_rect(params.rect, params.multiline, scale, font);
        let px = child.to_px(scale);
        if px.width() < 4 || px.height() < 4 {
            delete_font(font);
            return;
        }
        let mut style = WS_CHILD.0;
        style |= match params.align {
            TextAlign::Left => ES_LEFT as u32,
            TextAlign::Center => ES_CENTER as u32,
            TextAlign::Right => ES_RIGHT as u32,
        };
        if params.multiline {
            // Multiline fields keep hard newlines; wrap decides whether long
            // lines break or scroll horizontally, and the requested bars
            // become real WS_VSCROLL/WS_HSCROLL chrome. Enter inserts a newline
            // (the message loop only commits on Tab/Escape/focus loss).
            style |= ES_MULTILINE as u32 | ES_WANTRETURN as u32;
            let vbar = params.scrollbars.vertical();
            let hbar = params.scrollbars.horizontal() && !params.wrap;
            if !params.wrap {
                style |= ES_AUTOHSCROLL as u32;
            }
            if vbar {
                style |= ES_AUTOVSCROLL as u32 | WS_VSCROLL.0;
            }
            if hbar {
                style |= WS_HSCROLL.0;
            }
        } else {
            style |= ES_AUTOHSCROLL as u32;
        }
        if params.password {
            style |= ES_PASSWORD as u32;
        }
        if params.numbers_only {
            style |= ES_NUMBER as u32;
        }
        // The multiline EDIT control stores line breaks as CRLF; convert so
        // the initial text round-trips identically.
        let text = wide(&crate::text::to_crlf(&params.text));
        let created = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("EDIT"),
                PCWSTR(text.as_ptr()),
                WINDOW_STYLE(style),
                px.left,
                px.top,
                px.width(),
                px.height(),
                Some(self.hwnd),
                None,
                Some(self.instance),
                None,
            )
        };
        let Ok(hwnd) = created else {
            if std::env::var_os("BAMBOO_UI_DEBUG").is_some() {
                eprintln!("bamboo-ui: open_edit CreateWindowExW failed");
            }
            delete_font(font);
            return;
        };
        install_edit_subclass(hwnd, self.hwnd);
        unsafe {
            SendMessageW(hwnd, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(1)));
            let margin = (8.0 * scale).round().max(2.0) as i32;
            let packed = ((margin & 0xFFFF) | ((margin & 0xFFFF) << 16)) as isize;
            SendMessageW(
                hwnd,
                EM_SETMARGINS,
                Some(WPARAM(EC_LEFTMARGIN | EC_RIGHTMARGIN)),
                Some(LPARAM(packed)),
            );
            if params.max_length > 0 {
                SendMessageW(
                    hwnd,
                    EM_SETLIMITTEXT,
                    Some(WPARAM(params.max_length as usize)),
                    Some(LPARAM(0)),
                );
            }
            if !params.cue.is_empty() {
                let cue = wide(&params.cue);
                SendMessageW(
                    hwnd,
                    EM_SETCUEBANNER,
                    Some(WPARAM(1)),
                    Some(LPARAM(cue.as_ptr() as isize)),
                );
            }
            SendMessageW(hwnd, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        self.edit = Some(EditSession {
            hwnd,
            id: params.id,
            rect: params.rect,
            child,
            font,
            font_size: params.font_size,
            font_family: params.font_family.clone(),
            multiline: params.multiline,
            dpi: self.dpi,
        });
        unsafe {
            let _ = SetFocus(Some(hwnd));
        }
        // Repaint the frame around the child (its border turns accent).
        self.invalidate(params.rect);
    }

    /// Child rectangle for the editor, in client DIPs. Multiline editors fill
    /// the field; single-line editors are a text-height strip centered
    /// vertically, because the classic `EDIT` class top-aligns its text.
    fn move_edit(&mut self, rect: Rect) {
        let Some(session) = self.edit.as_ref() else {
            return;
        };
        let dpi = self.dpi;
        let refont = session.dpi != dpi;
        if session.rect == rect && !refont {
            return;
        }
        let scale = dpi / 96.0;
        let multiline = session.multiline;
        // Create the replacement font before touching the session so a failed
        // layout change never leaves the control with a dead handle.
        let font = if refont {
            self.edit_font(scale, session.font_size, session.font_family.as_deref())
        } else {
            session.font
        };
        let child = edit_child_rect(rect, multiline, scale, font);
        let px = child.to_px(scale);
        let Some(session) = self.edit.as_mut() else {
            return;
        };
        if refont {
            unsafe {
                SendMessageW(
                    session.hwnd,
                    WM_SETFONT,
                    Some(WPARAM(font.0 as usize)),
                    Some(LPARAM(1)),
                );
            }
            delete_font(session.font);
            session.font = font;
        }
        session.rect = rect;
        session.child = child;
        session.dpi = dpi;
        unsafe {
            let _ = SetWindowPos(
                session.hwnd,
                None,
                px.left,
                px.top,
                px.width().max(1),
                px.height().max(1),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        self.invalidate(rect);
    }

    fn close_edit(&mut self, commit: bool) {
        let Some(session) = self.edit.take() else {
            return;
        };
        // The field the keyboard was raised for is closing (committed by its
        // own Enter, a focus change, or the application); put the keyboard
        // away even when the framework did not observe the transition.
        self.dismiss_soft_keyboard();
        let EditSession {
            hwnd,
            id,
            rect,
            font,
            multiline,
            ..
        } = session;
        let mut text = String::new();
        if commit {
            text = read_text(hwnd);
            if !multiline {
                // A single-line field must not smuggle a pasted line break in.
                text.retain(|ch| ch != '\r' && ch != '\n');
            }
        }
        // Move keyboard focus out first; hiding a focused window is legal but
        // leaves the parent without a keyboard focus target.
        if unsafe { GetFocus() } == hwnd {
            unsafe {
                let _ = SetFocus(Some(self.hwnd));
            }
        }
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        self.edit_trash.push((hwnd, font));
        self.post_edit_trash();
        self.invalidate(rect);
        if commit {
            self.notify_edit_committed(id, &text);
        } else {
            self.notify_edit_canceled(id);
        }
    }

    /// Hide a session that is being replaced without telling the application.
    fn retire_edit(&mut self, session: EditSession) {
        let EditSession { hwnd, font, .. } = session;
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        self.edit_trash.push((hwnd, font));
        self.post_edit_trash();
    }

    fn post_edit_trash(&mut self) {
        if self.trash_posted || self.edit_trash.is_empty() {
            return;
        }
        self.trash_posted = true;
        unsafe {
            let _ = PostMessageW(Some(self.hwnd), WM_APP_EDIT_TRASH, WPARAM(0), LPARAM(0));
        }
    }

    fn drop_edit_trash(&mut self) {
        self.trash_posted = false;
        for (hwnd, font) in std::mem::take(&mut self.edit_trash) {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            delete_font(font);
        }
    }

    /// Release native editors and their fonts when the window goes away.
    fn shutdown_edits(&mut self) {
        self.dismiss_soft_keyboard();
        if let Some(session) = self.edit.take() {
            delete_font(session.font);
        }
        for (_, font) in std::mem::take(&mut self.edit_trash) {
            delete_font(font);
        }
    }

    /// Font for the native editor: the theme body style at the current DPI.
    fn edit_font(&self, scale: f32, font_size: Option<f32>, family: Option<&str>) -> HFONT {
        let body = self.painter.body();
        let size_px = (font_size.unwrap_or(body.size) * scale).round().max(1.0) as i32;
        let family = match family {
            Some(name) => wide(name),
            None => wide(crate::backend::gdi::family_name(&body.face)),
        };
        unsafe {
            CreateFontW(
                -size_px,
                0,
                0,
                0,
                body.weight.to_windows(),
                if body.italic { 1 } else { 0 },
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                PCWSTR(family.as_ptr()),
            )
        }
    }

    fn notify_edit_changed(&mut self, id: WidgetId, text: &str) {
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.edit_changed(id, text, &mut window);
        }
        self.host = Some(host);
    }

    fn notify_edit_committed(&mut self, id: WidgetId, text: &str) {
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.edit_committed(id, text, &mut window);
        }
        self.host = Some(host);
    }

    fn notify_edit_canceled(&mut self, id: WidgetId) {
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.edit_canceled(id, &mut window);
        }
        self.host = Some(host);
    }
}

/// Tooltip timing and popup.
impl WindowState {
    /// The hovered/focused target changed. A new target hides the old popup
    /// and arms the dwell timer; identical repeats are ignored (the container
    /// reports the hover target on every pointer move).
    fn update_tooltip(&mut self, request: Option<TooltipRequest<'_>>) {
        let Some(request) = request else {
            self.tooltip.target = None;
            self.hide_tooltip();
            return;
        };
        if let Some(target) = self.tooltip.target.as_mut() {
            let same_kind = matches!(
                (target.anchor, request.anchor),
                (TooltipAnchor::Pointer(_), TooltipAnchor::Pointer(_))
                    | (TooltipAnchor::Widget(_), TooltipAnchor::Widget(_))
            );
            if target.text == request.text
                && target.widget == request.widget
                && target.item == request.item
                && same_kind
            {
                // Follow the pointer until the popup appears; once it is on
                // screen it stays where the dwell placed it.
                if !self.tooltip.visible {
                    target.anchor = request.anchor;
                }
                return;
            }
        }
        self.hide_tooltip();
        self.tooltip.target = Some(TipTarget {
            text: request.text.to_string(),
            widget: request.widget,
            item: request.item,
            anchor: request.anchor,
        });
        self.tooltip.armed = true;
        unsafe {
            let _ = KillTimer(Some(self.hwnd), TOOLTIP_HIDE_TIMER);
            SetTimer(
                Some(self.hwnd),
                TOOLTIP_DWELL_TIMER,
                TOOLTIP_DWELL_MS,
                None,
            );
        }
    }

    /// The dwell elapsed: size, place and show the popup for the pending
    /// target. Creating the window happens here, not inside a `Host` callback,
    /// so a callback never re-enters window creation.
    fn show_tooltip(&mut self) {
        unsafe {
            let _ = KillTimer(Some(self.hwnd), TOOLTIP_DWELL_TIMER);
        }
        if !self.tooltip.armed {
            return;
        }
        let Some((text, anchor)) = self
            .tooltip
            .target
            .as_ref()
            .map(|target| (target.text.clone(), target.anchor))
        else {
            return;
        };
        if self.tooltip.window.is_none() {
            match TooltipWindow::create(self.instance, self.hwnd) {
                Ok(window) => {
                    let mut window = Box::new(window);
                    let hwnd = window.hwnd();
                    unsafe {
                        SetWindowLongPtrW(
                            hwnd,
                            GWLP_USERDATA,
                            &mut *window as *mut TooltipWindow as isize,
                        );
                    }
                    self.tooltip.window = Some(window);
                }
                Err(error) => {
                    tracing::warn!("tooltip window: {error}");
                    return;
                }
            }
        }
        let scale = self.scale();
        let owner_rect = tooltip::window_rect(self.hwnd);
        let origin = client_origin(self.hwnd);
        let gap = (8.0 * scale).round() as i32;
        let edge = (4.0 * scale).round() as i32;
        let palette = *self.painter.palette();
        let style = self.painter.small().clone();
        if let Some(window) = self.tooltip.window.as_mut() {
            window.show(
                &text, anchor, origin, owner_rect, scale, self.dpi, palette, style, gap, edge,
            );
        }
        self.tooltip.armed = false;
        self.tooltip.visible = true;
        self.tooltip.alpha = 0;
        self.fade_tooltip_to(u8::MAX);
        unsafe {
            SetTimer(
                Some(self.hwnd),
                TOOLTIP_HIDE_TIMER,
                TOOLTIP_HIDE_MS,
                None,
            );
        }
    }

    /// Hide the popup and cancel its timers but keep the current target, so
    /// moving inside the same widget does not restart the dwell.
    fn hide_tooltip(&mut self) {
        unsafe {
            let _ = KillTimer(Some(self.hwnd), TOOLTIP_DWELL_TIMER);
            let _ = KillTimer(Some(self.hwnd), TOOLTIP_HIDE_TIMER);
        }
        self.tooltip.armed = false;
        self.hide_tooltip_now();
    }

    /// Hide the popup without touching the dwell/auto-hide timers. Used when
    /// the window loses focus: a pending hover tip may still appear, and an
    /// already-shown one fades out.
    fn hide_tooltip_now(&mut self) {
        if self.tooltip.window.is_some() && self.tooltip.alpha > 0 {
            self.fade_tooltip_to(0);
            return;
        }
        self.stop_tooltip_animation();
        self.tooltip.visible = false;
        if let Some(window) = self.tooltip.window.as_ref() {
            window.hide();
        }
    }

    /// Start (or retarget) the layered alpha animation.
    fn fade_tooltip_to(&mut self, alpha: u8) {
        self.tooltip.target_alpha = alpha;
        if !self.tooltip.animating {
            self.tooltip.animating = true;
            unsafe {
                SetTimer(
                    Some(self.hwnd),
                    TOOLTIP_ANIM_TIMER,
                    15,
                    None,
                );
            }
        }
    }

    /// One fade step: move alpha toward the target and hide at zero.
    fn animate_tooltip(&mut self) {
        let Some(hwnd) = self.tooltip.window.as_ref().map(|window| window.hwnd()) else {
            self.stop_tooltip_animation();
            return;
        };
        let target = self.tooltip.target_alpha;
        let current = self.tooltip.alpha;
        let next = if current < target {
            current.saturating_add(TOOLTIP_FADE_STEP).min(target)
        } else {
            current.saturating_sub(TOOLTIP_FADE_STEP).max(target)
        };
        self.tooltip.alpha = next;
        tooltip::set_alpha(hwnd, next);
        if next == target {
            self.stop_tooltip_animation();
            if target == 0 {
                if let Some(window) = self.tooltip.window.as_ref() {
                    window.hide();
                }
                self.tooltip.visible = false;
            }
        }
    }

    fn stop_tooltip_animation(&mut self) {
        self.tooltip.animating = false;
        unsafe {
            let _ = KillTimer(Some(self.hwnd), TOOLTIP_ANIM_TIMER);
        }
    }

    fn shutdown_tooltip(&mut self) {
        self.hide_tooltip();
        self.stop_tooltip_animation();
        self.tooltip.target = None;
        if let Some(mut window) = self.tooltip.window.take() {
            window.destroy();
        }
    }
}

/// Context menus requested by `Ui`.
///
/// `TrackPopupMenu` runs a modal loop. Showing it inside `Host::event` would
/// dispatch that loop with the host taken (no paints, no timer work) and with
/// the window state borrowed twice. The request is therefore queued and the
/// popup opens from a posted message; the selection is replayed to the host
/// afterwards, when the menu no longer owns the mouse.
impl WindowState {
    fn show_pending_menu(&mut self) {
        // Clear the posted flag first: a dismissed request leaves no work for
        // the next message, and a later request must be able to post again.
        self.menu_posted = false;
        let Some(mut pending) = self.pending_menu.take() else {
            return;
        };
        self.update_tooltip(None);
        self.hide_tooltip_now();
        let widget = pending.widget;
        let source = pending.source;
        // The one menu hook: the model can still be edited or the popup
        // canceled before it is built (D61).
        let opening = self.raise_menu_event(&mut MenuEvent::Opening {
            widget: Some(widget),
            source,
            anchor: pending.anchor,
            menu: &mut pending.menu,
        });
        if opening == MenuDecision::Handled || pending.menu.is_empty() {
            self.raise_menu_event(&mut MenuEvent::Closed {
                widget: Some(widget),
                source,
                command: None,
            });
            return;
        }
        let scale = self.scale();
        let (origin_x, origin_y) = client_origin(self.hwnd);
        let x = origin_x + (pending.anchor.x * scale).round() as i32;
        let y = origin_y + (pending.anchor.y * scale).round() as i32;
        // A menu opened from a control flips above it instead of covering it.
        let avoid = pending.avoid.map(|rect| {
            Rect::new(
                origin_x as f32 + rect.left * scale,
                origin_y as f32 + rect.top * scale,
                origin_x as f32 + rect.right * scale,
                origin_y as f32 + rect.bottom * scale,
            )
        });
        // Widget menus stay inside the window (a bottom-docked menu bar opens
        // upward); the monitor work area keeps them on screen too.
        let client = client_area(self.hwnd);
        let work = work_area(self.hwnd);
        let bounds = Rect::new(
            client.left.max(work.left),
            client.top.max(work.top),
            client.right.min(work.right),
            client.bottom.min(work.bottom),
        );
        let bounds = if bounds.width() > 40.0 && bounds.height() > 40.0 {
            bounds
        } else {
            work
        };
        let placement = native_menu::place(
            (x, y),
            avoid,
            native_menu::estimated_height(&pending.menu),
            bounds,
        );
        // The popup's model moves into the window state so `Opened` can hand it
        // out while the user sees the menu, and so a timer-side edit can
        // rebuild it (`WindowHandle::edit_open_menu`).
        self.open_menu = Some(OpenMenu {
            widget: Some(widget),
            source,
            menu: pending.menu,
            placement,
            dirty: false,
            reopens: 0,
        });
        self.menu_open = true;
        self.menu_target = Some((Some(widget), source));
        let command = self.run_open_menu_loop();
        self.menu_open = false;
        self.open_menu = None;
        // The popup covered client pixels and the occlusion scan never removed
        // them (menu windows are not occluders), so nothing queued a repaint
        // for the uncovered area.
        self.invalidate_all_force();
        let mut command = command;
        let mut handled = false;
        if let Some(choice) = command.as_mut() {
            handled = self.raise_menu_event(&mut MenuEvent::Selected {
                widget: Some(widget),
                source,
                command: choice,
            }) == MenuDecision::Handled;
        }
        self.raise_menu_event(&mut MenuEvent::Closed {
            widget: Some(widget),
            source,
            command,
        });
        if let Some(command) = command
            && !handled
        {
            self.dispatch_menu_command(widget, command);
        }
    }

    /// Show the popup in `self.open_menu`, applying a live rebuild when the
    /// application edited the model. Returns the chosen command.
    ///
    /// Each pass posts [`WM_APP_MENU_OPENED`] first: `TrackPopupMenu`'s own
    /// modal loop dispatches it, so `MenuEvent::Opened` reaches the application
    /// *while* the popup is up (and can edit the model it is handed). An edit
    /// (detected by comparing the model, or requested through
    /// `WindowHandle::edit_open_menu`) posts `WM_CANCELMODE`, the popup closes
    /// with no selection and the next pass rebuilds it from the edited model.
    fn run_open_menu_loop(&mut self) -> Option<u32> {
        loop {
            let Some(open) = self.open_menu.as_ref() else {
                return None;
            };
            let placement = open.placement;
            self.menu_opened_fired = false;
            unsafe {
                let _ = PostMessageW(Some(self.hwnd), WM_APP_MENU_OPENED, WPARAM(0), LPARAM(0));
            }
            let selected = native_menu::show(self.hwnd, &open.menu, placement);
            // A popup that closed before its loop dispatched the posted message
            // still reports `Opened` (the lifecycle stays complete).
            if !self.menu_opened_fired {
                self.fire_menu_opened();
            }
            let Some(open) = self.open_menu.as_ref() else {
                return selected;
            };
            if !open.dirty || selected.is_some() || open.reopens >= MAX_MENU_REOPENS {
                return selected;
            }
            if let Some(open) = self.open_menu.as_mut() {
                open.dirty = false;
                open.reopens += 1;
            }
        }
    }

    /// Deliver `MenuEvent::Opened` with the live model. Runs from the posted
    /// message inside the popup's loop; a model edit closes the popup so the
    /// loop rebuilds it.
    fn fire_menu_opened(&mut self) {
        if self.menu_opened_fired {
            return;
        }
        self.menu_opened_fired = true;
        let Some(open) = self.open_menu.as_mut() else {
            return;
        };
        let (widget, source) = (open.widget, open.source);
        // The hook needs `&mut WindowHandle`, which borrows the whole state,
        // so the model leaves briefly. `edit_open_menu` from inside `Opened`
        // is therefore a no-op; edit the `menu` argument instead.
        let mut menu = std::mem::take(&mut open.menu);
        let before = menu.clone();
        self.raise_menu_event(&mut MenuEvent::Opened {
            widget,
            source,
            menu: &mut menu,
        });
        let changed = menu != before;
        if let Some(open) = self.open_menu.as_mut() {
            open.menu = menu;
            if changed {
                open.dirty = true;
            }
        }
        if changed {
            // Close the current popup; the loop rebuilds the edited model.
            unsafe {
                let _ = PostMessageW(Some(self.hwnd), WM_CANCELMODE, WPARAM(0), LPARAM(0));
            }
        }
    }

    /// Deliver one menu lifecycle event to the application hook.
    ///
    /// Mirrors [`WindowState::dispatch_menu_command`]: the host is taken out so
    /// the hook can call back into the window without a double borrow, and the
    /// deferred edit/a11y/paint work runs after it returns. Defaults to
    /// `Pass` when no host is installed.
    fn raise_menu_event(&mut self, event: &mut MenuEvent<'_>) -> MenuDecision {
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return MenuDecision::Pass;
        };
        let decision = {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.menu_event(event, &mut window)
        };
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
        if std::mem::take(&mut self.flush_requested) {
            unsafe {
                let _ = UpdateWindow(hwnd);
            }
        }
        decision
    }

    /// Queue a native popup. The `WM_APP_SHOW_MENU` message is enough for one
    /// pending request; a second request replaces the first.
    fn request_menu(&mut self, pending: PendingMenu) {
        if pending.menu.is_empty() {
            return;
        }
        self.pending_menu = Some(pending);
        if !self.menu_posted {
            self.menu_posted = true;
            unsafe {
                let _ = PostMessageW(Some(self.hwnd), WM_APP_SHOW_MENU, WPARAM(0), LPARAM(0));
            }
        }
    }

    /// Replay a menu selection chosen without the popup (a shortcut, a gamepad
    /// binding, a test). Reports `Selected` to the hook and, unless it is
    /// consumed, runs the normal `Host::menu_command` replay. No `Closed`
    /// event is raised: there was no popup.
    fn dispatch_menu_click(&mut self, widget: WidgetId, command: u32) -> bool {
        let source = self
            .menu_target
            .filter(|(target, _)| *target == Some(widget))
            .map(|(_, source)| source)
            .unwrap_or(MenuSource::Programmatic);
        let mut command = command;
        if self.raise_menu_event(&mut MenuEvent::Selected {
            widget: Some(widget),
            source,
            command: &mut command,
        }) == MenuDecision::Handled
        {
            return false;
        }
        self.menu_target = Some((Some(widget), source));
        self.dispatch_menu_command(widget, command);
        true
    }

    fn dispatch_menu_command(&mut self, widget: WidgetId, command: u32) {
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.menu_command(widget, command, &mut window);
        }
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
        if std::mem::take(&mut self.flush_requested) {
            unsafe {
                let _ = UpdateWindow(hwnd);
            }
        }
    }
}

/// First command whose chord matches `vk` and the exact modifier state. The
/// window host owns two tables (application shortcuts, then menu accelerators)
/// and this is the one matching rule both use.
pub(crate) fn hotkey_command(
    hotkeys: &[(Hotkey, u32)],
    vk: u16,
    modifiers: HotkeyModifiers,
) -> Option<u32> {
    hotkeys
        .iter()
        .find(|(hotkey, _)| hotkey.vk == vk && hotkey.modifiers == modifiers)
        .map(|(_, command)| *command)
}

/// Dispatch a command picked from the native window menu bar.
///
/// The selection goes through the same `MenuEvent` hook as every other menu
/// (source `MenuBar`, no owning widget), then to
/// [`crate::Host::menu_bar_command`] unless the hook consumed it.
impl WindowState {
    /// Command of the first registered chord matching this key press. The
    /// application's window shortcuts win over the menu bar's accelerator
    /// labels.
    fn window_hotkey_command(&self, vk: u16, modifiers: HotkeyModifiers) -> Option<u32> {
        hotkey_command(&self.window_hotkeys, vk, modifiers)
            .or_else(|| hotkey_command(&self.menu_hotkeys, vk, modifiers))
    }

    fn dispatch_menu_bar_command(&mut self, command: u32) {
        if command == 0 {
            return;
        }
        let mut command = command;
        if self.raise_menu_event(&mut MenuEvent::Selected {
            widget: None,
            source: MenuSource::MenuBar,
            command: &mut command,
        }) == MenuDecision::Handled
        {
            return;
        }
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.menu_bar_command(command, &mut window);
        }
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
        if std::mem::take(&mut self.flush_requested) {
            unsafe {
                let _ = UpdateWindow(hwnd);
            }
        }
    }

    /// The bar title a mnemonic selects. A repeated press of the same letter
    /// cycles through titles that share it, like a native menu bar.
    fn bar_mnemonic_pick(&mut self, ch: char) -> Option<usize> {
        let matches = self.menu_bar.as_ref()?.mnemonic_indices(ch);
        if matches.is_empty() {
            return None;
        }
        let next = match self.bar_mnemonic {
            Some((last, index)) if last == ch => {
                let position = matches.iter().position(|i| *i == index).unwrap_or(0);
                matches[(position + 1) % matches.len()]
            }
            _ => matches[0],
        };
        self.bar_mnemonic = Some((ch, next));
        Some(next)
    }

    /// Open one bar title's drop-down at its screen rectangle (the `Alt+key`
    /// path; the mouse path is the shell's own). The popup handle belongs to
    /// the root menu, so it is not destroyed here; the selection is replayed
    /// through the same `MenuEvent`/`menu_bar_command` path.
    fn show_bar_menu(&mut self, index: usize) {
        let Some(root) = self.window_menu else {
            return;
        };
        if index >= self.bar_items.len() {
            return;
        }
        let popup = unsafe { GetSubMenu(root, index as i32) };
        if popup.is_invalid() {
            return;
        }
        let mut rect = RECT::default();
        if unsafe { GetMenuItemRect(Some(self.hwnd), root, index as u32, &mut rect) }.is_err() {
            return;
        }
        self.update_tooltip(None);
        self.hide_tooltip_now();
        self.menu_open = true;
        self.menu_target = Some((None, MenuSource::MenuBar));
        let command = native_menu::show_popup(self.hwnd, popup, rect.left, rect.bottom);
        self.menu_open = false;
        self.invalidate_all_force();
        if let Some(command) = command {
            self.dispatch_menu_bar_command(command);
        }
    }

    /// Install, replace or remove (`None`) the window menu bar. The system
    /// draws it in the non-client area, so the client rectangle changes: the
    /// window is resized by the OS and a `WM_SIZE`/layout pass follows.
    fn apply_menu_bar(&mut self, model: Option<MenuBarModel>) {
        self.menu_bar = model;
        // The accelerator labels in the popups become live window shortcuts
        // while this model is installed.
        self.menu_hotkeys = self
            .menu_bar
            .as_ref()
            .map(MenuBarModel::hotkeys)
            .unwrap_or_default();
        unsafe {
            let _ = SetMenu(self.hwnd, None);
        }
        // Destroy the menu before the item data it points at.
        if let Some(old) = self.window_menu.take() {
            unsafe {
                let _ = DestroyMenu(old);
            }
        }
        // Icon bitmaps are not owned by the menu; free them with it.
        native_menu::free_bitmaps(std::mem::take(&mut self.bar_bitmaps));
        self.bar_items.clear();
        self.bar_selected = None;
        self.bar_no_accel = true;
        self.bar_mnemonic = None;
        self.bar_armed = false;
        self.bar_alt_down = false;
        if let Some(model) = &self.menu_bar
            && !model.is_empty()
        {
            self.bar_items = model
                .entries()
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    Box::new(BarItemPaint {
                        label: crate::mnemonic::MnemonicLabel::parse(entry.label()),
                        index,
                        separator: entry.is_separator(),
                    })
                })
                .collect();
            let data: Vec<usize> = self
                .bar_items
                .iter()
                .map(|item| &**item as *const BarItemPaint as usize)
                .collect();
            if let Ok((hmenu, bitmaps)) = native_menu::build_bar(model, &data) {
                if unsafe { SetMenu(self.hwnd, Some(hmenu)) }.is_ok() {
                    self.window_menu = Some(hmenu);
                    self.bar_bitmaps = bitmaps;
                } else {
                    unsafe {
                        let _ = DestroyMenu(hmenu);
                    }
                    native_menu::free_bitmaps(bitmaps);
                }
            }
        }
        unsafe {
            let _ = DrawMenuBar(self.hwnd);
        }
        // The non-client area changed height. Post `WM_SIZE` so the layout
        // pass runs after the current callback returns (this may be called
        // from a `Host` callback, where the host is already borrowed).
        unsafe {
            let _ = PostMessageW(Some(self.hwnd), WM_SIZE, WPARAM(0), LPARAM(0));
        }
    }

    /// The bar item data pointer, only when it is one of ours. `WM_DRAWITEM` /
    /// `WM_MEASUREITEM` can name any owner-drawn control, so the pointer is
    /// never dereferenced without this check.
    fn bar_item(&self, data: usize) -> Option<&BarItemPaint> {
        if data == 0 {
            return None;
        }
        self.bar_items
            .iter()
            .map(|item| &**item as *const BarItemPaint)
            .find(|item| *item as usize == data)
            .map(|item| unsafe { &*item })
    }

    /// Drop the cached menu font (DPI change / destroy). The system font is
    /// not owned and is left alone.
    fn release_bar_font(&mut self) {
        if let Some((_, font, owned)) = self.bar_font.take()
            && owned
        {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(font.0));
            }
        }
    }

    /// The menu font at the window's DPI: the system menu font (DPI-scaled),
    /// cached until the DPI changes.
    fn bar_font(&mut self) -> HFONT {
        let dpi = self.dpi.round().max(96.0) as u32;
        if let Some((cached, font, _)) = self.bar_font
            && cached == dpi
        {
            return font;
        }
        if let Some((_, old, owned)) = self.bar_font.take()
            && owned
        {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(old.0));
            }
        }
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        let mut font = HFONT::default();
        let mut owned = false;
        if unsafe {
            SystemParametersInfoForDpi(
                SPI_GETNONCLIENTMETRICS.0,
                metrics.cbSize,
                Some(&mut metrics as *mut NONCLIENTMETRICSW as *mut c_void),
                0,
                dpi,
            )
        }
        .is_ok()
        {
            font = unsafe { CreateFontIndirectW(&metrics.lfMenuFont) };
            owned = !font.is_invalid();
        }
        if font.is_invalid() {
            font = HFONT(unsafe { GetStockObject(DEFAULT_GUI_FONT) }.0);
        }
        self.bar_font = Some((dpi, font, owned));
        font
    }

    /// Handle `WM_MEASUREITEM` for one of our bar items.
    fn measure_bar_item(&mut self, item: &mut MEASUREITEMSTRUCT) -> bool {
        if item.CtlType != ODT_MENU {
            return false;
        }
        let Some(label) = self.bar_item(item.itemData).map(|paint| paint.label.text().to_string())
        else {
            return false;
        };
        let scale = self.scale();
        let font = self.bar_font();
        let pad = (8.0 * scale).round() as u32;
        let mut size = crate::Size::new(0.0, 0.0);
        unsafe {
            let dc = GetDC(Some(self.hwnd));
            if !dc.is_invalid() {
                let old = SelectObject(dc, HGDIOBJ(font.0));
                let wide = wide(&label);
                let mut extent = windows::Win32::Foundation::SIZE::default();
                if GetTextExtentPoint32W(dc, &wide[..wide.len() - 1], &mut extent).as_bool() {
                    size = crate::Size::new(extent.cx as f32, extent.cy as f32);
                }
                SelectObject(dc, old);
                let _ = ReleaseDC(Some(self.hwnd), dc);
            }
        }
        // The shell reserves a check-mark column on an owner-drawn bar item
        // even though a top-level item never carries a checkmark. Without
        // subtracting it every item is ~15 DIP wider than its label, which
        // reads as a large blank gap between menu titles.
        let dpi = self.dpi.round().max(96.0) as u32;
        let check_column = unsafe { GetSystemMetricsForDpi(SM_CXMENUCHECK, dpi) }
            + (2.0 * scale).round() as i32;
        let check_column = check_column.max(0) as u32;
        let text_width = size.width.max(8.0) as u32;
        item.itemWidth = (text_width + pad * 2).saturating_sub(check_column).max(text_width);
        item.itemHeight = (size.height + 10.0 * scale).round().max(20.0 * scale) as u32;
        true
    }

    /// `WM_DRAWITEM` for one of our bar items. The shell tells us which item
    /// is hot/active through `ODS_SELECTED`; the whole strip is then repainted
    /// with the page palette. Painting the entire strip (not just the item
    /// rectangle) is deliberate: the shell paints its own bar background and
    /// rounded hot/focus cues around an owner item, and those leftovers
    /// otherwise survive a full frame repaint as light slivers. A hover or a
    /// menu-mode move asks for one item and repaints the strip, which also
    /// clears whatever the previous state left behind.
    fn draw_bar_item(&mut self, item: &DRAWITEMSTRUCT) -> bool {
        if item.CtlType != ODT_MENU {
            return false;
        }
        let Some(index) = self.bar_item(item.itemData).map(|paint| paint.index) else {
            return false;
        };
        if item.itemState.0 & ODS_SELECTED.0 != 0 {
            self.bar_selected = Some(index);
        } else if self.bar_selected == Some(index) {
            self.bar_selected = None;
        }
        // The shell decides whether access keys are underlined; this is the
        // same per-item signal a normal owner-drawn item gets.
        self.bar_no_accel = item.itemState.0 & ODS_NOACCEL.0 != 0;
        self.paint_menu_bar_strip();
        true
    }

    /// Whether the shell is driving this thread's menu bar (Alt/F10 pressed,
    /// a bar menu or the window menu is open). Those keys belong to the menu,
    /// not to the page's focused widget.
    fn menu_mode_active() -> bool {
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetGUIThreadInfo(0, &mut info) }.is_err() {
            return false;
        }
        let flags = info.flags.0;
        flags & (GUI_INMENUMODE.0 | GUI_POPUPMENUMODE.0 | GUI_SYSTEMMENUMODE.0) != 0
    }

    /// Repaint the whole menu bar strip with the page palette: background,
    /// themed separators, the hot item and every label. Runs after
    /// `DefWindowProc` painted the frame and whenever the shell asks for one
    /// bar pixel, so the system's own bar background and its rounded
    /// hot/focus decorations never survive on a dark page.
    fn paint_menu_bar_strip(&mut self) {
        let Some(hmenu) = self.window_menu else {
            return;
        };
        let count = self.bar_items.len();
        if count == 0 {
            return;
        }
        let mut first = RECT::default();
        if unsafe { GetMenuItemRect(Some(self.hwnd), hmenu, 0, &mut first) }.is_err() {
            return;
        }
        let mut last = RECT::default();
        if unsafe { GetMenuItemRect(Some(self.hwnd), hmenu, (count - 1) as u32, &mut last) }.is_err()
        {
            return;
        }
        let mut window = RECT::default();
        if unsafe { GetWindowRect(self.hwnd, &mut window) }.is_err() {
            return;
        }
        let width = window.right - window.left;
        // The band is the strip between the caption and the client, not the
        // item rects: those are unavailable on the very first frame, and the
        // shell paints its own light rules into the strip's edges. The client
        // origin gives the exact bottom; the system menu height gives the top.
        let mut client_origin = POINT::default();
        let client_top = if unsafe { ClientToScreen(self.hwnd, &mut client_origin) }.as_bool() {
            client_origin.y - window.top
        } else {
            last.bottom - window.top
        };
        let dpi = self.dpi.round().max(96.0) as u32;
        let menu_height = unsafe { GetSystemMetricsForDpi(SM_CYMENU, dpi) };
        let (band_top, band_bottom) = if menu_height > 0 && client_top - menu_height > window.top {
            (client_top - menu_height, client_top)
        } else {
            (first.top - window.top, last.bottom - window.top)
        };
        let bottom = last.bottom - window.top;
        if width <= 0 || band_bottom <= band_top {
            return;
        }
        let palette = *self.painter.palette();
        let scale = self.scale();
        let line = (1.0 * scale).round().max(1.0) as i32;
        let pad = (14.0 * scale).round() as i32;
        // Access-key underlines follow the shell's per-item `ODS_NOACCEL`
        // signal, which is clear only while the menu mode shows accelerators.
        let accents_visible = !self.bar_no_accel;
        let font = self.bar_font();
        unsafe {
            let dc = GetWindowDC(Some(self.hwnd));
            if dc.is_invalid() {
                return;
            }
            let panel = self.gdi.area_brush(palette.panel);
            FillRect(
                dc,
                &RECT {
                    left: 0,
                    top: band_top,
                    right: width,
                    bottom: band_bottom,
                },
                panel,
            );
            let divider = self.gdi.area_brush(palette.divider);
            let old_font = SelectObject(dc, HGDIOBJ(font.0));
            SetBkMode(dc, TRANSPARENT);
            let mut metrics = TEXTMETRICW::default();
            let _ = GetTextMetricsW(dc, &mut metrics);
            for index in 0..count {
                let item = &self.bar_items[index];
                let mut rect = RECT::default();
                let rect_ok = GetMenuItemRect(Some(self.hwnd), hmenu, index as u32, &mut rect).is_ok();
                let left = if rect_ok { rect.left - window.left } else { 0 };
                let right = if rect_ok { rect.right - window.left } else { 0 };
                if item.separator {
                    // Bar groups keep their spacing but no vertical rule; the
                    // page-theme bar reads cleaner without one.
                    continue;
                }
                if !rect_ok || right <= left {
                    continue;
                }
                if self.bar_selected == Some(index) {
                    let hover = self.gdi.area_brush(palette.nav_hover);
                    FillRect(
                        dc,
                        &RECT {
                            left,
                            top: band_top,
                            right,
                            bottom: band_bottom,
                        },
                        hover,
                    );
                }
                SetTextColor(dc, COLORREF(palette.text.to_colorref()));
                let text = item.label.text();
                let wide_label = wide(text);
                let mut label_size = windows::Win32::Foundation::SIZE::default();
                let label_width = if GetTextExtentPoint32W(
                    dc,
                    &wide_label[..wide_label.len() - 1],
                    &mut label_size,
                )
                .as_bool()
                {
                    label_size.cx
                } else {
                    0
                };
                let text_left = left + pad.min((right - left - label_width).max(0) / 2);
                let text_top = (band_top + band_bottom - metrics.tmHeight as i32) / 2;
                let _ = TextOutW(
                    dc,
                    text_left,
                    text_top,
                    &wide_label[..wide_label.len() - 1],
                );
                if accents_visible
                    && let Some((key_byte, key_len)) =
                        item.label.key_byte().map(|byte| (byte, item.label.key_len()))
                    && let Some(prefix) = text.get(..key_byte)
                    && let Some(key_text) = text.get(key_byte..key_byte + key_len)
                {
                    let wide_prefix = wide(prefix);
                    let wide_key = wide(key_text);
                    let mut prefix_size = windows::Win32::Foundation::SIZE::default();
                    if GetTextExtentPoint32W(
                        dc,
                        &wide_prefix[..wide_prefix.len() - 1],
                        &mut prefix_size,
                    )
                    .as_bool()
                    {
                        let mut key_size = windows::Win32::Foundation::SIZE::default();
                        if GetTextExtentPoint32W(dc, &wide_key[..wide_key.len() - 1], &mut key_size)
                            .as_bool()
                        {
                            let underline = bottom - (4.0 * scale) as i32;
                            let text_brush = self.gdi.area_brush(palette.text);
                            FillRect(
                                dc,
                                &RECT {
                                    left: text_left + prefix_size.cx,
                                    top: underline,
                                    right: text_left + prefix_size.cx + key_size.cx,
                                    bottom: underline + line,
                                },
                                text_brush,
                            );
                        }
                    }
                }
            }
            for rule_top in [band_top, band_bottom - line] {
                FillRect(
                    dc,
                    &RECT {
                        left: 0,
                        top: rule_top,
                        right: width,
                        bottom: rule_top + line,
                    },
                    divider,
                );
            }
            SelectObject(dc, old_font);
            let _ = ReleaseDC(Some(self.hwnd), dc);
        }
    }
}

/// Native dialogs.
///
/// Each dialog runs on its own thread. A modal dialog disables this window
/// from that thread (the dialog also gets it as owner, so it stays above the
/// application); a parallel dialog passes no owner, leaving the window enabled
/// and usable. The UI thread therefore never runs the dialog's nested loop: it
/// keeps painting and dispatching while the dialog is up, and receives the
/// result as a posted message. This avoids the dual borrow a nested loop inside
/// a `Host` callback would create, and mirrors the deferred-menu rule (D37).
impl WindowState {
    fn show_dialog(&mut self, dialog: Dialog) -> DialogId {
        let id = DialogId(self.next_dialog_id.max(1));
        self.next_dialog_id = self.next_dialog_id.wrapping_add(1).max(1);
        self.update_tooltip(None);
        self.hide_tooltip_now();
        let modal = dialog.modal();
        let canceled = dialog.canceled_result();
        let raw = self.hwnd.0 as isize;
        // The library color window follows the page's theme; capture it on the
        // UI thread (the worker has no access to this window's painter).
        let theme = self.painter.settings().clone();
        let dark = self.painter.dark();
        let spawned = std::thread::Builder::new()
            .name("bamboo-ui-dialog".to_string())
            .spawn(move || {
                let hwnd = HWND(raw as *mut c_void);
                let result = native_dialog::show(hwnd, &dialog, modal, &theme, dark);
                post_dialog_done(
                    hwnd,
                    DialogDone {
                        id,
                        modal,
                        result,
                    },
                );
            });
        if spawned.is_err() {
            // No worker: report a dismissal so the application is not left
            // waiting for a callback that will never come.
            post_dialog_done(
                self.hwnd,
                DialogDone {
                    id,
                    modal,
                    result: canceled,
                },
            );
        }
        id
    }

    fn finish_dialog(&mut self, done: DialogDone) {
        if done.modal {
            // Re-enable on the UI thread as well: it owns the activation and
            // can restore foreground after the dialog thread is gone.
            unsafe {
                let _ = EnableWindow(self.hwnd, true);
                let _ = SetForegroundWindow(self.hwnd);
            }
        }
        // The dialog overlapped client pixels; the occlusion scan may not have
        // re-exposed them yet.
        self.refresh_visibility();
        self.invalidate_all();
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.dialog_finished(done.id, done.result, &mut window);
        }
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
        if std::mem::take(&mut self.flush_requested) {
            unsafe {
                let _ = UpdateWindow(hwnd);
            }
        }
    }
}

/// Notification-area icons.
///
/// Each icon is registered with `Shell_NotifyIconW` under its application id
/// (the primary icon is id 0). The icons are re-registered when Explorer
/// broadcasts `TaskbarCreated`, and removed in `WM_NCDESTROY`. Clicks and menu
/// selections are dispatched as [`TrayEvent`]s carrying the icon id, from the
/// notification message, which runs outside every `Host` callback.
impl WindowState {
    fn set_tray(&mut self, tray: Option<TrayIcon>) {
        self.set_tray_at(0, tray);
    }

    /// Add, update (same id) or remove one icon.
    fn set_tray_at(&mut self, id: u32, tray: Option<TrayIcon>) -> bool {
        let Some(config) = tray else {
            self.remove_tray(id);
            return false;
        };
        let size = self.tray_icon_size();
        let (icon, owned) = load_tray_icon(&config.image, size);
        let replacement = TrayState {
            id,
            tooltip: config.tooltip,
            menu: config.menu,
            image: config.image,
            icon,
            owned,
            added: false,
        };
        if let Some(index) = self.trays.iter().position(|tray| tray.id == id) {
            let was_added = self.trays[index].added;
            let previous = std::mem::replace(&mut self.trays[index], replacement);
            if was_added {
                self.modify_tray(index);
                self.trays[index].added = true;
            } else {
                self.add_tray(index);
            }
            // Only after the shell has the new handle.
            destroy_tray_icon(previous.icon, previous.owned);
        } else {
            self.trays.push(replacement);
            self.add_tray(self.trays.len() - 1);
        }
        self.trays.iter().any(|tray| tray.id == id && tray.added)
    }

    /// The notification-area small-icon size at the current window DPI.
    fn tray_icon_size(&self) -> i32 {
        let dpi = self.dpi.round().max(96.0) as u32;
        let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) };
        if size > 0 {
            size
        } else {
            unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16)
        }
    }

    /// Reload every tray icon for the current DPI. A multi-size `.ico` lets
    /// the loader pick the closest image to the requested small-icon size.
    fn reload_tray_icons(&mut self) {
        let size = self.tray_icon_size();
        for index in 0..self.trays.len() {
            let image = self.trays[index].image.clone();
            let (icon, owned) = load_tray_icon(&image, size);
            let old_icon = std::mem::replace(&mut self.trays[index].icon, icon);
            let old_owned = std::mem::replace(&mut self.trays[index].owned, owned);
            if self.trays[index].added {
                self.modify_tray(index);
            }
            destroy_tray_icon(old_icon, old_owned);
        }
    }

    fn tray_data(&self, tray: &TrayState) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: tray.id,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: WM_APP_TRAY,
            hIcon: tray.icon,
            ..Default::default()
        };
        let tooltip: Vec<u16> = tray.tooltip.encode_utf16().take(127).collect();
        data.szTip[..tooltip.len()].copy_from_slice(&tooltip);
        data
    }

    fn add_tray(&mut self, index: usize) {
        let Some(tray) = self.trays.get(index) else {
            return;
        };
        let data = self.tray_data(tray);
        let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool();
        if added {
            let mut version = self.tray_data(tray);
            // NOTIFYICON_VERSION_4: wParam carries (notification << 16) | id
            // and lParam the cursor coordinates.
            version.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            unsafe {
                let _ = Shell_NotifyIconW(NIM_SETVERSION, &version);
            }
        }
        self.trays[index].added = added;
    }

    fn modify_tray(&mut self, index: usize) {
        let Some(tray) = self.trays.get(index) else {
            return;
        };
        let data = self.tray_data(tray);
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
        }
    }

    /// Re-add every icon after Explorer restarted (the shell drops them).
    fn readd_trays(&mut self) {
        for index in 0..self.trays.len() {
            self.trays[index].added = false;
            self.add_tray(index);
        }
    }

    fn remove_tray(&mut self, id: u32) {
        let Some(index) = self.trays.iter().position(|tray| tray.id == id) else {
            return;
        };
        let tray = self.trays.remove(index);
        self.delete_tray(tray);
    }

    fn remove_all_trays(&mut self) {
        while let Some(tray) = self.trays.pop() {
            self.delete_tray(tray);
        }
    }

    fn delete_tray(&self, tray: TrayState) {
        if tray.added {
            let data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: tray.id,
                ..Default::default()
            };
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &data);
            }
        }
        destroy_tray_icon(tray.icon, tray.owned);
    }

    fn shutdown_tray(&mut self) {
        self.remove_all_trays();
    }

    fn tray_icon_visible(&self) -> bool {
        self.trays.iter().any(|tray| tray.added)
    }

    /// Show a notification balloon on one icon (id 0 is the primary). The
    /// shell owns its look and lifetime; the return value only says the
    /// request was accepted.
    fn show_tray_balloon(&mut self, id: u32, title: &str, text: &str, icon: MessageIcon) -> bool {
        let Some(tray) = self.trays.iter().find(|tray| tray.id == id) else {
            return false;
        };
        if !tray.added {
            return false;
        }
        let mut data = self.tray_data(tray);
        data.uFlags = NIF_INFO;
        data.dwInfoFlags = match icon {
            MessageIcon::Info | MessageIcon::Question => NIIF_INFO,
            MessageIcon::Warning => NIIF_WARNING,
            MessageIcon::Error => NIIF_ERROR,
        };
        if title.is_empty() && text.is_empty() {
            data.dwInfoFlags = NIIF_NONE;
        }
        // The native fields are fixed-size, NUL-terminated buffers.
        let title: Vec<u16> = title.encode_utf16().take(63).collect();
        data.szInfoTitle[..title.len()].copy_from_slice(&title);
        let text: Vec<u16> = text.encode_utf16().take(255).collect();
        data.szInfo[..text.len()].copy_from_slice(&text);
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) }.as_bool()
    }

    fn handle_tray(&mut self, wparam: WPARAM, lparam: LPARAM) {
        // With version 4 wParam is (notification << 16) | icon id; before the
        // SETVERSION call takes effect Windows sends the legacy form where the
        // low word is the id and lParam is the mouse message.
        let icon = (wparam.0 & 0xFFFF) as u32;
        let event = if (wparam.0 >> 16) != 0 {
            (wparam.0 >> 16) as u32
        } else {
            lparam.0 as u32
        };
        match event {
            NIN_SELECT | WM_LBUTTONUP => {
                self.dispatch_tray(TrayEvent::Action {
                    icon,
                    action: TrayAction::Click,
                });
            }
            NIN_KEYSELECT => {
                self.dispatch_tray(TrayEvent::Action {
                    icon,
                    action: TrayAction::KeySelect,
                });
            }
            WM_LBUTTONDBLCLK => {
                self.dispatch_tray(TrayEvent::Action {
                    icon,
                    action: TrayAction::DoubleClick,
                });
            }
            WM_RBUTTONUP | WM_CONTEXTMENU => {
                // Notify first, so a hook can refresh the menu model before it
                // is shown, then open it.
                self.dispatch_tray(TrayEvent::Action {
                    icon,
                    action: TrayAction::RightClick,
                });
                self.show_tray_menu(icon);
            }
            WM_MBUTTONUP => {
                self.dispatch_tray(TrayEvent::Action {
                    icon,
                    action: TrayAction::MiddleClick,
                });
            }
            _ => {}
        }
    }

    fn show_tray_menu(&mut self, icon: u32) {
        let Some(menu) = self
            .trays
            .iter()
            .find(|tray| tray.id == icon)
            .map(|tray| tray.menu.clone())
        else {
            return;
        };
        if menu.is_empty() {
            return;
        }
        let mut menu = menu;
        let mut point = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut point);
        }
        // The hook's anchor is in client DIPs, like every other menu event.
        let scale = self.scale();
        let (origin_x, origin_y) = client_origin(self.hwnd);
        let anchor = Point::new(
            (point.x - origin_x) as f32 / scale,
            (point.y - origin_y) as f32 / scale,
        );
        let opening = self.raise_menu_event(&mut MenuEvent::Opening {
            widget: None,
            source: MenuSource::Tray,
            anchor,
            menu: &mut menu,
        });
        if opening == MenuDecision::Handled || menu.is_empty() {
            self.raise_menu_event(&mut MenuEvent::Closed {
                widget: None,
                source: MenuSource::Tray,
                command: None,
            });
            return;
        }
        unsafe {
            // Required for the popup to close when the user clicks elsewhere;
            // the trailing WM_NULL is the documented companion.
            let _ = SetForegroundWindow(self.hwnd);
        }
        self.open_menu = Some(OpenMenu {
            widget: None,
            source: MenuSource::Tray,
            menu,
            placement: native_menu::Placement {
                x: point.x,
                y: point.y,
                bottom_aligned: false,
            },
            dirty: false,
            reopens: 0,
        });
        self.menu_open = true;
        self.menu_target = Some((None, MenuSource::Tray));
        let mut command = self.run_open_menu_loop();
        self.menu_open = false;
        self.open_menu = None;
        unsafe {
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        }
        let mut handled = false;
        if let Some(choice) = command.as_mut() {
            handled = self.raise_menu_event(&mut MenuEvent::Selected {
                widget: None,
                source: MenuSource::Tray,
                command: choice,
            }) == MenuDecision::Handled;
        }
        self.raise_menu_event(&mut MenuEvent::Closed {
            widget: None,
            source: MenuSource::Tray,
            command,
        });
        if let Some(command) = command
            && !handled
        {
            self.dispatch_tray(TrayEvent::Command { icon, command });
        }
    }

    fn dispatch_tray(&mut self, event: TrayEvent) {
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.tray_event(event, &mut window);
        }
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
        if std::mem::take(&mut self.flush_requested) {
            unsafe {
                let _ = UpdateWindow(hwnd);
            }
        }
    }
}

/// Global hotkeys (`RegisterHotKey`) and shortcut capture.
///
/// Registrations are owned by the host: ids are allocated internally, duplicate
/// registrations are deduplicated, a conflict with another process simply
/// reports `false`, and everything is unregistered on destroy. Capture
/// consumes the next chord before the application sees it, including its
/// translated `WM_CHAR`, and reports through `Host::hotkey_captured`.
impl WindowState {
    fn register_hotkey(&mut self, hotkey: Hotkey) -> bool {
        if self
            .hotkeys
            .iter()
            .any(|(_, registered)| *registered == hotkey)
        {
            return true;
        }
        let id = self.next_hotkey_id;
        let mut modifiers = MOD_NOREPEAT;
        if hotkey.modifiers.alt {
            modifiers |= MOD_ALT;
        }
        if hotkey.modifiers.ctrl {
            modifiers |= MOD_CONTROL;
        }
        if hotkey.modifiers.shift {
            modifiers |= MOD_SHIFT;
        }
        if hotkey.modifiers.win {
            modifiers |= MOD_WIN;
        }
        match unsafe { RegisterHotKey(Some(self.hwnd), id, modifiers, hotkey.vk as u32) } {
            Ok(()) => {
                self.hotkeys.push((id, hotkey));
                self.next_hotkey_id = self.next_hotkey_id.wrapping_add(1).max(1);
                true
            }
            Err(_) => false,
        }
    }

    fn unregister_hotkey(&mut self, hotkey: Hotkey) -> bool {
        let Some(position) = self
            .hotkeys
            .iter()
            .position(|(_, registered)| *registered == hotkey)
        else {
            return false;
        };
        let (id, _) = self.hotkeys.remove(position);
        unsafe {
            let _ = UnregisterHotKey(Some(self.hwnd), id);
        }
        true
    }

    fn shutdown_hotkeys(&mut self) {
        for (id, _) in self.hotkeys.drain(..) {
            unsafe {
                let _ = UnregisterHotKey(Some(self.hwnd), id);
            }
        }
    }

    fn begin_hotkey_capture(&mut self) {
        self.hotkey_capture = true;
        self.swallow_char = false;
        self.update_tooltip(None);
        self.hide_tooltip_now();
    }

    fn capture_hotkey(&mut self, vk: u16) {
        if vk == VK_ESCAPE.0 {
            self.finish_hotkey_capture(None);
            return;
        }
        if is_modifier_vk(vk) {
            // Keep waiting for the actual key; the modifiers are sampled when
            // it arrives.
            return;
        }
        // The key is consumed either way, so is its translated character.
        self.swallow_char = true;
        let hotkey = Hotkey::from_vk(self.current_modifiers(), vk);
        if hotkey.is_global_safe() {
            self.finish_hotkey_capture(Some(hotkey));
        }
        // A bare printable key is not a valid global shortcut; keep capturing
        // and wait for a chord with a modifier or a function key.
    }

    fn current_modifiers(&self) -> HotkeyModifiers {
        let down = |vk: u16| unsafe { GetKeyState(vk as i32) } < 0;
        HotkeyModifiers {
            ctrl: down(VK_CONTROL.0),
            shift: down(VK_SHIFT.0),
            alt: down(VK_MENU.0),
            win: down(VK_LWIN.0) || down(VK_RWIN.0),
        }
    }

    fn finish_hotkey_capture(&mut self, hotkey: Option<Hotkey>) {
        self.hotkey_capture = false;
        // `swallow_char` is deliberately left as the key handler set it: the
        // chord's WM_CHAR/WM_SYSCHAR is still queued and must be dropped.
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.hotkey_captured(hotkey, &mut window);
        }
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
        if std::mem::take(&mut self.flush_requested) {
            unsafe {
                let _ = UpdateWindow(hwnd);
            }
        }
    }

    fn dispatch_hotkey(&mut self, hotkey: Hotkey) {
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.hotkey(hotkey, &mut window);
        }
        self.host = Some(host);
        self.process_edit_ops();
        self.flush_a11y();
        if std::mem::take(&mut self.flush_requested) {
            unsafe {
                let _ = UpdateWindow(hwnd);
            }
        }
    }
}

fn load_tray_icon(image: &TrayImage, size: i32) -> (HICON, bool) {
    match image {
        TrayImage::Default => (default_tray_icon(), false),
        TrayImage::File(path) => {
            let path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            match unsafe {
                LoadImageW(
                    None,
                    PCWSTR(path.as_ptr()),
                    IMAGE_ICON,
                    size.max(16),
                    size.max(16),
                    LR_LOADFROMFILE,
                )
            } {
                Ok(handle) if !handle.0.is_null() => (HICON(handle.0), true),
                _ => (default_tray_icon(), false),
            }
        }
    }
}

fn default_tray_icon() -> HICON {
    unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or_default() }
}

fn destroy_tray_icon(icon: HICON, owned: bool) {
    if owned && !icon.0.is_null() {
        unsafe {
            let _ = DestroyIcon(icon);
        }
    }
}

fn post_dialog_done(hwnd: HWND, done: DialogDone) {
    let ptr = Box::into_raw(Box::new(done));
    let posted = unsafe {
        PostMessageW(Some(hwnd), WM_APP_DIALOG_DONE, WPARAM(0), LPARAM(ptr as isize))
    };
    if posted.is_err() {
        // The window is gone; nothing will consume the box.
        unsafe { drop(Box::from_raw(ptr)) };
    }
}

/// Screen position of a window's client origin, in pixels.
/// The client area in screen pixels.
fn client_area(hwnd: HWND) -> Rect {
    let (origin_x, origin_y) = client_origin(hwnd);
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut client);
    }
    Rect::new(
        origin_x as f32,
        origin_y as f32,
        (origin_x + client.right) as f32,
        (origin_y + client.bottom) as f32,
    )
}

/// The work area (screen pixels) of the monitor nearest `hwnd`, for keeping a
/// popup on screen. Falls back to the primary work area, then to a generous
/// rectangle; the native menu still fits itself on the monitor.
fn work_area(hwnd: HWND) -> Rect {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if !monitor.is_invalid() {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            let work = info.rcWork;
            return Rect::new(
                work.left as f32,
                work.top as f32,
                work.right as f32,
                work.bottom as f32,
            );
        }
    }
    let mut rect = RECT::default();
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut RECT as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if result.is_err() || rect.right <= rect.left || rect.bottom <= rect.top {
        return Rect::new(0.0, 0.0, 8192.0, 8192.0);
    }
    Rect::new(
        rect.left as f32,
        rect.top as f32,
        rect.right as f32,
        rect.bottom as f32,
    )
}

fn client_origin(hwnd: HWND) -> (i32, i32) {
    let mut origin = POINT::default();
    unsafe {
        let _ = ClientToScreen(hwnd, &mut origin);
    }
    (origin.x, origin.y)
}

/// UI Automation integration.
impl WindowState {
    /// Snapshot the widget tree for a screen reader that asked for it.
    fn build_a11y_tree(&mut self) {
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        let nodes = host.a11y_nodes();
        self.host = Some(host);
        let mut origin = POINT::default();
        unsafe {
            let _ = ClientToScreen(hwnd, &mut origin);
        }
        let mut title = [0u16; 256];
        let length = unsafe { GetWindowTextW(hwnd, &mut title) }.max(0) as usize;
        let root_name = String::from_utf16_lossy(&title[..length.min(title.len())]);
        let root_rect = Rect::from_size(self.size);
        let scale = self.scale();
        let changes = match self.a11y.as_ref() {
            // Refresh in place: screen readers keep the provider objects.
            Some(tree) => tree.refresh(origin.x, origin.y, scale, root_name, root_rect, nodes),
            None => {
                self.a11y = Some(A11yTree::new(
                    hwnd,
                    origin.x,
                    origin.y,
                    scale,
                    root_name,
                    root_rect,
                    nodes,
                ));
                Vec::new()
            }
        };
        if let Some(tree) = self.a11y.clone() {
            raise_changes(&tree, &changes);
        }
    }

    /// Rebuild the snapshot (if any) and announce the pending focus. Runs
    /// after a host callback, when the host is available again.
    fn flush_a11y(&mut self) {
        if self.a11y.is_none() {
            self.a11y_dirty = false;
            self.pending_a11y_focus = None;
            return;
        }
        if std::mem::take(&mut self.a11y_dirty) {
            self.build_a11y_tree();
        }
        if let Some(id) = self.pending_a11y_focus.take() {
            self.raise_a11y_focus(id);
        }
        if let Some((text, priority)) = self.pending_a11y_announce.take()
            && let Some(tree) = self.a11y.clone()
        {
            announce(&tree, &text, priority);
        }
    }

    fn raise_a11y_focus(&mut self, id: Option<WidgetId>) {
        let Some(tree) = self.a11y.clone() else {
            return;
        };
        if unsafe { !UiaClientsAreListening().as_bool() } {
            return;
        }
        let index = match id {
            Some(id) => match tree.focus_index(id) {
                Some(index) => Some(index),
                None => {
                    self.a11y_focus_key = None;
                    return;
                }
            },
            None => None,
        };
        // Hosts call `a11y_focus` after every event, including pointer moves;
        // announce only when the focus target actually changed.
        if let Some(id) = id {
            let key = index
                .and_then(|index| tree.node(index))
                .map(|node| (id, node.child));
            if key.is_some() && key == self.a11y_focus_key {
                return;
            }
            self.a11y_focus_key = key;
        } else {
            self.a11y_focus_key = None;
        }
        let provider: IRawElementProviderSimple = UiaProvider::at(tree, index).into();
        unsafe {
            let _ = UiaRaiseAutomationEvent(&provider, UIA_AutomationFocusChangedEventId);
        }
    }

    /// Run a queued assistive-technology request on the UI thread.
    fn dispatch_a11y(&mut self, request: A11yRequest) {
        let action = match request {
            A11yRequest::Invoke(id, child) => A11yAction {
                kind: A11yActionKind::Invoke,
                id,
                child,
                value: None,
                number: None,
            },
            A11yRequest::Toggle(id, child) => A11yAction {
                kind: A11yActionKind::Toggle,
                id,
                child,
                value: None,
                number: None,
            },
            A11yRequest::Expand(id, child, expanded) => A11yAction {
                kind: if expanded {
                    A11yActionKind::Expand
                } else {
                    A11yActionKind::Collapse
                },
                id,
                child,
                value: None,
                number: None,
            },
            A11yRequest::Select(id, child) => A11yAction {
                kind: A11yActionKind::Select,
                id,
                child,
                value: None,
                number: None,
            },
            A11yRequest::Focus(id, child) => A11yAction {
                kind: A11yActionKind::Focus,
                id,
                child,
                value: None,
                number: None,
            },
            A11yRequest::SetText(id, child, text) => A11yAction {
                kind: A11yActionKind::SetValue,
                id,
                child,
                value: Some(text),
                number: None,
            },
            A11yRequest::SetNumber(id, child, number) => A11yAction {
                kind: A11yActionKind::SetValue,
                id,
                child,
                value: None,
                number: Some(number),
            },
            A11yRequest::Scroll(id, child, horizontal, vertical) => A11yAction {
                kind: A11yActionKind::Scroll {
                    horizontal,
                    vertical,
                },
                id,
                child,
                value: None,
                number: None,
            },
        };
        let hwnd = self.hwnd;
        let Some(mut host) = self.host.take() else {
            return;
        };
        {
            let mut window = WindowHandle {
                state: &mut *self,
                hwnd,
            };
            host.a11y_action(action, &mut window);
        }
        self.host = Some(host);
        // An accessibility Invoke on a text field opens a native editor.
        self.process_edit_ops();
        // Names, values and states changed; refresh before the next read.
        self.a11y_dirty = true;
        self.flush_a11y();
    }
}

/// Builds and runs one native window.
pub struct WindowBuilder {
    title: String,
    size: Size,
    min_size: Size,
    theme: crate::theme::ThemeSettings,
    dark: bool,
    menu_bar: Option<MenuBarModel>,
    window_hotkeys: Vec<(Hotkey, u32)>,
    renderer: Option<Renderer>,
    /// Owner handle: a raw `HWND` from another window (`WindowHandle::hwnd`).
    /// An owned window is created as a caption-only dialog window centered on
    /// its owner, so a library dialog window (the color picker) behaves like a
    /// modal chooser.
    owner: Option<isize>,
    /// AppUserModelID for taskbar grouping and system notifications.
    app_id: Option<String>,
    host: Box<dyn Host>,
}

/// Center `window` over `owner`'s frame (both in physical pixels).
fn center_over_owner(owner: HWND, window: HWND) {
    unsafe {
        let mut owner_rect = RECT::default();
        let mut window_rect = RECT::default();
        if GetWindowRect(owner, &mut owner_rect).is_err()
            || GetWindowRect(window, &mut window_rect).is_err()
        {
            return;
        }
        let width = window_rect.right - window_rect.left;
        let height = window_rect.bottom - window_rect.top;
        let x = owner_rect.left + ((owner_rect.right - owner_rect.left) - width) / 2;
        let y = owner_rect.top + ((owner_rect.bottom - owner_rect.top) - height) / 2;
        let _ = SetWindowPos(
            window,
            Some(HWND_TOP),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

impl WindowBuilder {
    pub fn new(host: impl Host + 'static) -> Self {
        Self {
            title: "Bamboo Native UI".to_string(),
            size: Size::new(900.0, 620.0),
            min_size: Size::new(320.0, 240.0),
            theme: crate::theme::ThemeSettings::default(),
            dark: true,
            menu_bar: None,
            window_hotkeys: Vec::new(),
            renderer: None,
            owner: None,
            app_id: None,
            host: Box::new(host),
        }
    }

    /// Set the AppUserModelID used for taskbar grouping and system
    /// notifications. It is pushed to the process
    /// (`SetCurrentProcessExplicitAppUserModelID`) and used as the toast
    /// identity when [`WindowHandle::notify`] has no per-notification id.
    ///
    /// An unpackaged desktop app still needs a Start-menu shortcut carrying
    /// the same AppUserModelID for toasts to be accepted; without one the
    /// window falls back to a tray-icon balloon.
    pub fn app_id(mut self, id: impl Into<String>) -> Self {
        self.app_id = Some(id.into());
        self
    }

    /// Make this an owned dialog window of `owner` (a raw `HWND` from
    /// [`WindowHandle::hwnd`]). The window gets a caption and a close button
    /// (no resize, no maximize box) and is centered on its owner. What makes
    /// it *modal* is the caller disabling the owner while it runs, exactly as
    /// the native dialog bodies do.
    pub fn owner(mut self, owner: isize) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Install a native window menu bar under the title bar (the system-drawn
    /// bar, not the painted widget). Labels keep their `&` access key; the
    /// system handles `Alt` activation, keyboard navigation, checkmarks and
    /// accessibility. Commands arrive through
    /// [`crate::Host::menu_bar_command`] after the [`MenuEvent`] hook.
    ///
    /// The bar is part of the window frame, so it follows the shell's colors
    /// (not the page palette) and reduces the client area by its height.
    pub fn menu_bar(mut self, menu_bar: MenuBarModel) -> Self {
        self.menu_bar = Some(menu_bar);
        self
    }

    /// Select the renderer that paints the window's back buffer: GDI (the
    /// default), Direct2D hardware or Direct2D software (see
    /// [`crate::backend::Renderer`]). The `BAMBOO_UI_RENDERER` environment
    /// variable (`gdi`, `d2d-software`, `d2d-hardware`) wins only when this is
    /// not called. A Direct2D mode that cannot start logs under
    /// `BAMBOO_UI_DEBUG` and falls back to GDI.
    pub fn renderer(mut self, renderer: Renderer) -> Self {
        self.renderer = Some(renderer);
        self
    }

    /// Register a window shortcut: the chord fires [`Host::menu_bar_command`]
    /// with `command` while this window has focus (not system-wide). The
    /// native menu bar already registers the chord shown in each item's
    /// accelerator column; this adds commands that have no menu row. Chords
    /// without a modifier must be function keys, like global hotkeys.
    pub fn window_hotkey(mut self, hotkey: Hotkey, command: u32) -> Self {
        if command != 0
            && hotkey.is_global_safe()
            && !self
                .window_hotkeys
                .iter()
                .any(|(chord, _)| *chord == hotkey)
        {
            self.window_hotkeys.push((hotkey, command));
        }
        self
    }

    /// Register several window shortcuts at once (see
    /// [`WindowBuilder::window_hotkey`]).
    pub fn window_hotkeys(mut self, hotkeys: impl IntoIterator<Item = (Hotkey, u32)>) -> Self {
        for (hotkey, command) in hotkeys {
            self = self.window_hotkey(hotkey, command);
        }
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Size::new(width, height);
        self
    }

    pub fn min_size(mut self, width: f32, height: f32) -> Self {
        self.min_size = Size::new(width, height);
        self
    }

    pub fn theme(mut self, settings: crate::theme::ThemeSettings, dark: bool) -> Self {
        self.theme = settings;
        self.dark = dark;
        self
    }

    /// Create the window, run the message loop, and return the exit code.
    pub fn run(self) -> Result<i32> {
        ensure_process_dpi_awareness();
        // Native popups (menus, some shell dialogs) follow the page's
        // dark/light decision; this must run before the first window exists so
        // the shell themes its menu class with the right mode.
        dark_theme::follow_theme(self.dark);
        if let Some(app_id) = self.app_id.as_deref() {
            // Taskbar grouping and the default toast identity; harmless when
            // the package/shortcut that registers it is absent.
            native_notification::set_process_app_id(app_id);
        }
        if std::env::var_os("BAMBOO_UI_DEBUG").is_some() {
            eprintln!(
                "bamboo-ui: uxtheme dark-mode exports available: {}",
                dark_theme::available()
            );
        }
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None)?.0 });
        register_class(instance)?;
        tooltip::ensure_class(instance)?;

        let system_dpi = unsafe { GetDpiForSystem() };
        let system_scale = if system_dpi > 0 {
            system_dpi as f32 / 96.0
        } else {
            1.0
        };

        let mut painter = Painter::new();
        painter.set_theme(&self.theme, self.dark);
        painter.set_caret_width(system_caret_width(system_scale));
        let state = Box::new(WindowState {
            host: Some(self.host),
            painter,
            gdi: GdiResources::new(),
            d2d: D2dResources::new(),
            renderer: self.renderer.or_else(Renderer::from_env).unwrap_or_default(),
            buffer: GdiDoubleBuffer::new(),
            dirty: Vec::new(),
            hwnd: HWND::default(),
            instance,
            size: self.size,
            min_size: self.min_size,
            dpi: system_dpi.max(96) as f32,
            visible: vec![Rect::from_size(self.size)],
            exposed: true,
            cursor: Cursor::Arrow,
            capturing: false,
            pointer_kind: PointerKind::Mouse,
            long_press: None,
            ole: false,
            winrt: false,
            app_id: self.app_id,
            drop_target: None,
            ole_drag: None,
            pending_drag: None,
            system_drag: true,
            invalidations: 0,
            timer_ms: 0,
            tracking_leave: false,
            flush_requested: false,
            overlay_painted: false,
            overlay_resync: false,
            quit_code: 0,
            edit: None,
            edit_ops: Vec::new(),
            edit_trash: Vec::new(),
            trash_posted: false,
            soft_keyboard: false,
            a11y: None,
            a11y_dirty: false,
            pending_a11y_focus: None,
            a11y_focus_key: None,
            pending_a11y_announce: None,
            tooltip: TooltipState::default(),
            pending_menu: None,
            menu_posted: false,
            menu_open: false,
            open_menu: None,
            menu_opened_fired: false,
            menu_target: None,
            menu_bar: self.menu_bar,
            window_hotkeys: self.window_hotkeys,
            menu_hotkeys: Vec::new(),
            window_menu: None,
            bar_bitmaps: Vec::new(),
            bar_items: Vec::new(),
            bar_font: None,
            bar_selected: None,
            bar_no_accel: true,
            bar_mnemonic: None,
            bar_armed: false,
            bar_alt_down: false,
            alt_tap_pending: false,
            alt_underlines: false,
            next_dialog_id: 1,
            trays: Vec::new(),
            started: false,
            loaded: false,
            close_reason: None,
            visual_state: WindowVisualState::Normal,
            shown: false,
            active: false,
            frame_pos: Point::new(0.0, 0.0),
            pending_lifecycle: Vec::new(),
            taskbar_created: 0,
            hotkeys: Vec::new(),
            next_hotkey_id: 1,
            hotkey_capture: false,
            swallow_char: false,
            caret_rect: None,
            caret_on: true,
            caret_timer: false,
        });
        let ptr = Box::into_raw(state);
        let title = wide(&self.title);
        // An owned window is a dialog window: caption + close, no resize frame,
        // centered on its owner after creation.
        let parent = self.owner.map(|raw| HWND(raw as *mut c_void));
        let style = if parent.is_some() {
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN
        } else {
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN
        };
        let hwnd = match unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name(),
                PCWSTR(title.as_ptr()),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                (self.size.width * system_scale).round().max(1.0) as i32,
                (self.size.height * system_scale).round().max(1.0) as i32,
                parent,
                None,
                Some(instance),
                Some(ptr as *const c_void),
            )
        } {
            Ok(hwnd) => hwnd,
            Err(error) => {
                drop(unsafe { Box::from_raw(ptr) });
                return Err(error);
            }
        };

        unsafe {
            (*ptr).hwnd = hwnd;
            if let Some(owner) = parent {
                center_over_owner(owner, hwnd);
            }
            // Opt the main window into the page's app mode so its native
            // scrollbars follow, and theme `#32768` popups as they are created.
            dark_theme::apply_to_window(hwnd);
            dark_theme::install_popup_hook();
            // The menu bar belongs to the frame: install it before the client
            // size is read so layout starts with the reduced client area.
            (*ptr).apply_menu_bar((*ptr).menu_bar.take());
            // Explorer restarts broadcast this; the tray icon is re-added then.
            (*ptr).taskbar_created = RegisterWindowMessageW(w!("TaskbarCreated"));
            let dpi = normalize_dpi(GetDpiForWindow(hwnd));
            let mut client = RECT::default();
            let _ = GetClientRect(hwnd, &mut client);
            (*ptr).dpi = dpi;
            (*ptr).painter.set_caret_width(system_caret_width((*ptr).scale()));
            (*ptr).size = client_size(client, dpi);
            (*ptr).visible = vec![Rect::from_size((*ptr).size)];
            (*ptr).exposed = true;
            let _ = ShowWindow(hwnd, SW_SHOW);
            // The first frame's bar is painted by the shell with its own light
            // rules before the menu item rects are laid out. Redraw the bar
            // and then paint the whole strip, so the first visible frame
            // already carries the page-palette edges.
            let _ = DrawMenuBar(hwnd);
            (*ptr).paint_menu_bar_strip();
            SetTimer(Some(hwnd), OCCLUSION_TIMER, 500, None);
            (*ptr).scan_visibility();
            // Accept shell file drops (WM_DROPFILES) from the first frame.
            DragAcceptFiles(hwnd, true);
            // Register as an OLE drop target so text and files can be dragged
            // in from other windows and applications. A thread whose COM
            // apartment cannot host OLE simply keeps the in-window fallback.
            (*ptr).ole = drag_drop::initialize();
            if (*ptr).ole {
                (*ptr).drop_target = drag_drop::register(hwnd);
            }
            // WinRT for the notification platform (same STA as OLE).
            (*ptr).winrt = native_notification::initialize();
        }

        // `started` runs after the window is visible so hosts can install the
        // animation timer without racing the first WM_SIZE.
        {
            let state = unsafe { &mut *ptr };
            state.invalidate_all();
            state.started = true;
            state.prewarm_renderer();
            state.shown = true;
            let mut frame = RECT::default();
            let _ = unsafe { GetWindowRect(hwnd, &mut frame) };
            let scale = state.scale();
            state.frame_pos = Point::new(frame.left as f32 / scale, frame.top as f32 / scale);
            let Some(mut host) = state.host.take() else {
                unreachable!("host present");
            };
            {
                let mut window = WindowHandle {
                    state: &mut *state,
                    hwnd,
                };
                host.started(&mut window);
            }
            state.host = Some(host);
            state.process_edit_ops();
        }

        let mut message = MSG::default();
        loop {
            let status = unsafe { GetMessageW(&mut message, None, 0, 0) };
            if status.0 == -1 || !status.as_bool() {
                break;
            }
            // Keyboard messages go to the focused child, so the native editor
            // is the only place Tab/Enter/Escape can be intercepted. This runs
            // outside any host callback, so closing the session is safe here.
            let state = unsafe { &mut *ptr };
            if message.message == WM_KEYDOWN
                && let Some(session) = state.edit.as_ref()
                && message.hwnd == session.hwnd
            {
                let vk = message.wParam.0 as u16;
                let multiline = session.multiline;
                match vk {
                    vk if vk == VK_ESCAPE.0 => {
                        state.queue_edit(EditOp::End { commit: false });
                        state.process_edit_ops();
                        continue;
                    }
                    vk if vk == VK_RETURN.0 && !multiline => {
                        state.queue_edit(EditOp::End { commit: true });
                        state.process_edit_ops();
                        continue;
                    }
                    vk if vk == VK_TAB.0 => {
                        state.queue_edit(EditOp::End { commit: true });
                        state.process_edit_ops();
                        // Hand focus navigation back to the application.
                        state.dispatch(Event::Key(KeyEvent {
                            code: KeyCode::Tab,
                            vk,
                            ctrl: key_down(VK_CONTROL.0 as i32),
                            shift: key_down(VK_SHIFT.0 as i32),
                            alt: key_down(VK_MENU.0 as i32),
                            repeat: (message.lParam.0 & (1isize << 30)) != 0,
                        }));
                        continue;
                    }
                    _ => {}
                }
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            // A callback that hid/minimized/moved the window made the shell
            // send the notification synchronously, while the host was taken;
            // deliver it now that the callback has returned.
            unsafe { &mut *ptr }.flush_lifecycle();
            // A drag requested from a callback starts here, outside the host
            // borrow, so OLE's drop-target callbacks can reach the application.
            unsafe { &mut *ptr }.run_pending_drag();
        }
        let code = message.wParam.0 as i32;

        // DestroyWindow frees the state in WM_NCDESTROY; if the window is
        // already gone, reclaim explicitly so nothing leaks.
        unsafe {
            let _ = DestroyWindow(hwnd);
            reclaim(hwnd);
        }
        Ok(code)
    }
}

/// Operations available from inside `Host` callbacks.
pub struct WindowHandle<'a> {
    state: &'a mut WindowState,
    hwnd: HWND,
}

impl WindowHandle<'_> {
    /// Invalidate one widget rectangle (DIPs).
    pub fn invalidate(&mut self, rect: Rect) {
        self.state.invalidate(rect);
    }

    /// Invalidate the whole client area once (page switch, resize, theme).
    pub fn invalidate_all(&mut self) {
        self.state.invalidate_all();
    }

    /// Invalidate a rectangle even while the window is covered or minimized.
    /// Use when something that covered this area just disappeared.
    pub fn invalidate_force(&mut self, rect: Rect) {
        self.state.invalidate_force(rect);
    }

    /// Invalidate the whole client even while covered.
    pub fn invalidate_all_force(&mut self) {
        let full = Rect::from_size(self.state.size);
        self.state.invalidate_force(full);
    }

    pub fn size(&self) -> Size {
        self.state.size
    }

    /// Start a **system** drag carrying `data` (text or files). The drag runs
    /// after the current callback returns; it can be dropped into this window,
    /// another window or another application. Returns whether OLE accepted the
    /// request (a window built without OLE, or one that turned system drags
    /// off, reports `false`).
    pub fn start_drag(&mut self, data: DragData) -> bool {
        UiHost::start_drag(self, data)
    }

    /// Enable or disable system (OLE) drags for this window. Automated runs
    /// turn them off: a synthetic pointer has no real button release to end
    /// OLE's modal drag loop. `Ui` then keeps its in-window drag session.
    pub fn set_system_drag_enabled(&mut self, enabled: bool) {
        self.state.system_drag = enabled;
    }

    /// Whether system (OLE) drags are available and enabled.
    pub fn system_drag_enabled(&self) -> bool {
        self.state.system_drag && self.state.ole
    }

    /// Raw `HWND` as an `isize` (diagnostics, UIA client probes).
    pub fn hwnd(&self) -> isize {
        self.hwnd.0 as isize
    }

    /// Whether this window is the foreground window. Background polling (a
    /// gamepad, a media key) should gate on this so a focused window keeps
    /// ownership of navigation, like a console dashboard.
    pub fn is_foreground(&self) -> bool {
        unsafe { GetForegroundWindow() == self.hwnd }
    }

    pub fn dpi(&self) -> f32 {
        self.state.dpi
    }

    pub fn scale(&self) -> f32 {
        self.state.scale()
    }

    /// Install or stop the animation timer. `None` means no periodic paints.
    pub fn set_animation_timer(&mut self, interval: Option<Duration>) {
        self.state.set_timer(interval);
    }

    /// Request a cursor shape for the client area.
    pub fn set_cursor(&mut self, cursor: Cursor) {
        if self.state.cursor == cursor {
            return;
        }
        self.state.cursor = cursor;
        unsafe {
            SetCursor(Some(cursor_handle(cursor)));
        }
    }

    /// Whether a tooltip popup is currently shown (diagnostics / self-test).
    pub fn tooltip_visible(&self) -> bool {
        self.state.tooltip.visible
    }

    /// Whether a native popup menu (context, chrome, tray or programmatic) is
    /// currently open (diagnostics / self-test).
    pub fn menu_active(&self) -> bool {
        self.state.menu_open
    }

    /// Alias for [`WindowHandle::menu_active`] with controller-style naming.
    pub fn is_menu_open(&self) -> bool {
        self.state.menu_open
    }

    /// The widget whose menu was the last one shown; `None` before the first
    /// menu and for the tray menu (which has no widget).
    pub fn active_menu_widget(&self) -> Option<WidgetId> {
        self.state.menu_target.and_then(|(widget, _)| widget)
    }

    /// Which path produced the last menu (context, chrome, programmatic or
    /// tray); `None` before the first menu.
    pub fn active_menu_source(&self) -> Option<MenuSource> {
        self.state.menu_target.map(|(_, source)| source)
    }

    /// Open a native popup menu from application code for `widget` (a
    /// shortcut handler, a gamepad binding, a command palette). `anchor` is
    /// the popup's top-left in client DIPs; `avoid` is a control rectangle the
    /// popup must not cover (it flips above it). Deferred exactly like a
    /// context menu: the popup opens from a posted message, the `MenuEvent`
    /// hook sees it and the selection is replayed through
    /// [`crate::Host::menu_command`]. An empty menu requests nothing; a second
    /// request before the loop runs replaces the first.
    pub fn open_menu(
        &mut self,
        widget: WidgetId,
        menu: Menu,
        anchor: Point,
        avoid: Option<Rect>,
    ) {
        self.state.request_menu(PendingMenu {
            widget,
            menu,
            anchor,
            avoid,
            source: MenuSource::Programmatic,
        });
    }

    /// Deliver a menu selection without showing the popup: the `MenuEvent`
    /// hook sees `Selected` (and may rewrite or consume the command) and the
    /// command then takes the normal `Host::menu_command` replay path. This is
    /// what a shortcut, a gamepad binding or a test uses to "choose" an item.
    ///
    /// The replay is posted to the message loop (like every menu path), so it
    /// is safe to call from a `Host` callback; `true` means it was queued.
    /// There is no `Closed` event: no popup was involved.
    pub fn menu_click(&mut self, widget: WidgetId, command: u32) -> bool {
        let payload = Box::new(PendingMenuClick { widget, command });
        let posted = unsafe {
            PostMessageW(
                Some(self.hwnd),
                WM_APP_MENU_CLICK,
                WPARAM(0),
                LPARAM(Box::into_raw(payload) as isize),
            )
        };
        posted.is_ok()
    }

    /// Install, replace or remove (`None`) the native window menu bar. The
    /// system draws it in the non-client area, so the client rectangle shrinks
    /// or grows and a layout pass follows. Commands arrive through
    /// [`crate::Host::menu_bar_command`]; the [`MenuEvent`] hook sees them
    /// first with source `MenuBar`.
    pub fn set_menu_bar(&mut self, menu_bar: Option<MenuBarModel>) {
        self.state.apply_menu_bar(menu_bar);
    }

    /// The installed menu bar model.
    pub fn menu_bar(&self) -> Option<&MenuBarModel> {
        self.state.menu_bar.as_ref()
    }

    /// Whether the window currently has a native menu bar installed.
    pub fn menu_bar_installed(&self) -> bool {
        self.state.window_menu.is_some()
    }

    /// Edit the installed model in place (toggle a checkmark, enable an item)
    /// and rebuild the native bar only when the closure reports a change.
    /// Returns whether a rebuild happened.
    pub fn edit_menu_bar(&mut self, edit: impl FnOnce(&mut MenuBarModel) -> bool) -> bool {
        let Some(model) = self.state.menu_bar.as_mut() else {
            return false;
        };
        if !edit(model) {
            return false;
        }
        let model = self.state.menu_bar.clone();
        self.state.apply_menu_bar(model);
        true
    }

    /// Close an open menu. Deferred to the menu's own modal loop; a no-op when
    /// no menu is open. Applications use it when the page changes or a
    /// background event makes the menu's commands stale.
    pub fn dismiss_menu(&mut self) {
        // A request that has not reached the message loop yet is dropped too.
        self.state.pending_menu = None;
        if !self.state.menu_open {
            return;
        }
        unsafe {
            let _ = PostMessageW(Some(self.hwnd), WM_CANCELMODE, WPARAM(0), LPARAM(0));
        }
    }

    /// Rebuild the popup that is currently up. `edit` runs on the live model
    /// (from a timer, typically, since `Host::menu_event` already gets it
    /// during `Opened`); if it changed anything the host closes the popup and
    /// reopens it with the new items at the same place. Returns whether a
    /// rebuild was requested — `false` when no popup is up or the model was
    /// left unchanged.
    pub fn edit_open_menu(&mut self, edit: impl FnOnce(&mut crate::menu::Menu)) -> bool {
        let Some(open) = self.state.open_menu.as_mut() else {
            return false;
        };
        let before = open.menu.clone();
        edit(&mut open.menu);
        if open.menu == before {
            return false;
        }
        open.dirty = true;
        unsafe {
            let _ = PostMessageW(Some(self.hwnd), WM_CANCELMODE, WPARAM(0), LPARAM(0));
        }
        true
    }

    /// Alias for [`WindowHandle::dismiss_menu`] (the name the context-menu
    /// host introduced).
    pub fn dismiss_context_menu(&mut self) {
        self.dismiss_menu();
    }

    /// Show a native dialog and return immediately.
    ///
    /// A modal dialog (the default) disables this window until it closes, so
    /// the user must dismiss it before using the application; a parallel
    /// dialog (`.parallel()` / `.modal(false)`) leaves the window enabled and
    /// usable. The result arrives through [`crate::Host::dialog_finished`] on
    /// the UI thread, correlated by the returned [`DialogId`]. Any number of
    /// dialogs may be open at once.
    /// Open the library's own color window (D118): an owned, theme-following
    /// dialog with an HSV field, hex editor, RGB/HSV slider + number rows and
    /// presets. `allow_alpha` adds the transparency row; the picked value
    /// arrives as [`DialogResult::ColorWindow`] in `Host::dialog_finished`.
    /// The system chooser stays available through
    /// `show_dialog(ColorDialog::new())`.
    pub fn show_color_window(&mut self, initial: Rgba, allow_alpha: bool) -> DialogId {
        self.show_dialog(crate::dialog::ColorWindow::new(initial).allow_alpha(allow_alpha))
    }

    pub fn show_dialog(&mut self, dialog: impl Into<Dialog>) -> DialogId {
        self.state.show_dialog(dialog.into())
    }

    /// Install, update or remove (`None`) this window's primary
    /// notification-area icon (id 0). Repeated calls modify it in place. A
    /// null tooltip/menu is allowed; the menu opens on right click or the
    /// keyboard menu key and its selection arrives as
    /// [`TrayEvent::Command`]. Icons are removed when the window is destroyed
    /// and re-added if Explorer restarts.
    pub fn set_tray_icon(&mut self, tray: Option<TrayIcon>) {
        self.state.set_tray(tray);
    }

    /// Install, update or remove (`None`) one tray icon by application id
    /// (D93). Id 0 is the primary icon [`WindowHandle::set_tray_icon`] manages;
    /// other ids keep independent tooltips, menus and images. The id comes
    /// back in every [`TrayEvent`].
    pub fn set_tray_icon_at(&mut self, id: u32, tray: Option<TrayIcon>) -> bool {
        self.state.set_tray_at(id, tray)
    }

    /// Whether the shell accepted the primary tray icon (diagnostics /
    /// self-test).
    pub fn tray_icon_visible(&self) -> bool {
        self.state.tray_icon_visible()
    }

    /// Register the window's [`WindowBuilder::app_id`] with the shell by
    /// creating/refreshing a Start-menu shortcut that carries
    /// `System.AppUserModel.ID`. An unpackaged desktop app needs this once
    /// before its toasts are displayed; `notify` does not write it implicitly.
    /// Returns whether the window has an app id and the shortcut was written.
    pub fn register_notification_shortcut(&self, display_name: &str) -> bool {
        let Some(app_id) = self.state.app_id.as_deref() else {
            return false;
        };
        let name = if display_name.is_empty() {
            app_id
        } else {
            display_name
        };
        native_notification::register_shortcut(app_id, name)
    }

    /// Show a system notification through the Windows notification platform
    /// (a WinRT toast). The per-notification AppUserModelID wins over the
    /// window's [`WindowBuilder::app_id`], then the process identity. When the
    /// shell refuses the toast (an unpackaged app without a registered AUMID,
    /// or the platform is unavailable) the call falls back to a tray-icon
    /// balloon if one is installed. The returned [`NotificationResult`] says
    /// which path was used.
    ///
    /// Call it from a window callback or the message loop; WinRT is initialized
    /// on the window thread.
    pub fn notify(&mut self, notification: &Notification) -> NotificationResult {
        let app_id = notification
            .app_id_str()
            .or(self.state.app_id.as_deref());
        if self.state.winrt && native_notification::show(notification, app_id) {
            return NotificationResult::Toast;
        }
        if self.state.show_tray_balloon(
            0,
            notification.title_text(),
            notification.body_text(),
            MessageIcon::Info,
        ) {
            NotificationResult::Balloon
        } else {
            NotificationResult::Failed
        }
    }

    /// Show a notification balloon on the primary tray icon. The shell draws
    /// it (and may route it to the notification center instead); `false` means
    /// no icon is installed or the request was refused.
    pub fn show_tray_balloon(
        &mut self,
        title: &str,
        text: &str,
        icon: crate::dialog::MessageIcon,
    ) -> bool {
        self.state.show_tray_balloon(0, title, text, icon)
    }

    /// Show a notification balloon on the icon with `id` (see
    /// [`WindowHandle::set_tray_icon_at`]).
    pub fn show_tray_balloon_on(
        &mut self,
        id: u32,
        title: &str,
        text: &str,
        icon: crate::dialog::MessageIcon,
    ) -> bool {
        self.state.show_tray_balloon(id, title, text, icon)
    }

    /// Register a system-wide hotkey. `false` means the system refused it —
    /// usually because another window or process already owns the chord.
    /// Registering the same hotkey again is a no-op that returns `true`. The
    /// key arrives as [`crate::Host::hotkey`]; registrations are released when
    /// the window is destroyed.
    pub fn register_hotkey(&mut self, hotkey: Hotkey) -> bool {
        self.state.register_hotkey(hotkey)
    }

    /// Every global hotkey currently registered, in registration order. Persist
    /// `hotkey.label()` strings and restore with
    /// [`WindowHandle::register_hotkey`] after checking the bool result (the
    /// system may already own the chord).
    pub fn registered_hotkeys(&self) -> Vec<Hotkey> {
        self.state.hotkeys.iter().map(|(_, hotkey)| *hotkey).collect()
    }

    /// Application window shortcuts (`register_window_hotkey`) only — the
    /// menu bar's accelerator labels are derived and excluded. The chord and
    /// its bar command persist through `label()`/`parse`.
    pub fn registered_window_hotkeys(&self) -> &[(Hotkey, u32)] {
        &self.state.window_hotkeys
    }

    /// Remove a registration installed with [`WindowHandle::register_hotkey`].
    /// `false` when it was not registered.
    pub fn unregister_hotkey(&mut self, hotkey: Hotkey) -> bool {
        self.state.unregister_hotkey(hotkey)
    }

    /// Register a window shortcut: while this window receives the key,
    /// `hotkey` fires [`crate::Host::menu_bar_command`] with `command` (after
    /// the `MenuEvent` hook) and the page never sees the chord. Unlike
    /// [`WindowHandle::register_hotkey`] this needs no system registration, so
    /// it cannot conflict with another process. `false` when `command` is 0,
    /// the chord has no modifier and is not a function key, or the chord is
    /// already registered here. The native menu bar's accelerator labels are
    /// registered automatically; registering the same chord here overrides
    /// them, and every command arrives through the same hook.
    pub fn register_window_hotkey(&mut self, hotkey: Hotkey, command: u32) -> bool {
        if command == 0 || !hotkey.is_global_safe() {
            return false;
        }
        if self
            .state
            .window_hotkeys
            .iter()
            .any(|(chord, _)| *chord == hotkey)
        {
            return false;
        }
        self.state.window_hotkeys.push((hotkey, command));
        true
    }

    /// Remove a window shortcut installed with
    /// [`WindowHandle::register_window_hotkey`]. `false` when it was not
    /// registered. Menu bar accelerators are not affected.
    pub fn unregister_window_hotkey(&mut self, hotkey: Hotkey) -> bool {
        let before = self.state.window_hotkeys.len();
        self.state
            .window_hotkeys
            .retain(|(chord, _)| *chord != hotkey);
        self.state.window_hotkeys.len() != before
    }

    /// Remove every application window shortcut. The installed menu bar's
    /// accelerator labels stay live.
    pub fn clear_window_hotkeys(&mut self) {
        self.state.window_hotkeys.clear();
    }

    /// Active window shortcuts: application registrations first, then the
    /// menu bar's accelerator labels (a chord registered for the application
    /// shadows the menu's entry). Read-only snapshot for diagnostics.
    pub fn window_hotkeys(&self) -> Vec<(Hotkey, u32)> {
        let mut all = self.state.window_hotkeys.clone();
        for entry in &self.state.menu_hotkeys {
            if !all.iter().any(|(chord, _)| *chord == entry.0) {
                all.push(*entry);
            }
        }
        all
    }

    /// Accelerators derived from the installed menu bar's shortcut labels.
    pub fn menu_hotkeys(&self) -> Vec<(Hotkey, u32)> {
        self.state.menu_hotkeys.clone()
    }

    /// Capture the next chord for a shortcut editor. Until it finishes, key
    /// presses are consumed before the application sees them (including their
    /// translated character); modifier-only presses keep waiting. The result
    /// arrives as [`crate::Host::hotkey_captured`]: `Some` for a chord with a
    /// modifier or a function key, `None` when Esc, focus loss or
    /// [`WindowHandle::cancel_hotkey_capture`] ends it.
    pub fn begin_hotkey_capture(&mut self) {
        self.state.begin_hotkey_capture();
    }

    /// Abort an active capture, reporting `None`.
    pub fn cancel_hotkey_capture(&mut self) {
        if self.state.hotkey_capture {
            self.state.finish_hotkey_capture(None);
        }
    }

    /// Whether a shortcut capture is in progress (diagnostics and tests).
    pub fn hotkey_capture_active(&self) -> bool {
        self.state.hotkey_capture
    }

    /// Whether a native text editor is open (diagnostics and tests).
    pub fn edit_active(&self) -> bool {
        self.state.editing_id().is_some()
    }

    /// Commit and close an open native text editor, if any. Use before a page
    /// switch or when the application drops the widget that owns it.
    pub fn end_edit(&mut self) {
        self.state.queue_edit(EditOp::End { commit: true });
    }

    /// Drop the cached UI Automation snapshot (page rebuild or layout change).
    /// The next screen-reader request rebuilds it.
    pub fn a11y_refresh(&mut self) {
        if self.state.a11y.is_some() {
            self.state.a11y_dirty = true;
        }
    }

    /// Put `text` on the clipboard as `CF_UNICODETEXT`. Returns false when the
    /// clipboard is owned by another process (it retries nothing) or the
    /// allocation failed. `CopyableText` applications call this from the
    /// message their copy handler emits.
    pub fn copy_text(&mut self, text: &str) -> bool {
        use windows::Win32::Foundation::{HANDLE, GlobalFree};
        use windows::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
        };
        use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
        use windows::Win32::System::Ole::CF_UNICODETEXT;
        unsafe {
            if OpenClipboard(Some(self.hwnd)).is_err() {
                return false;
            }
            let mut copied = false;
            if EmptyClipboard().is_ok() {
                let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                let bytes = units.len() * std::mem::size_of::<u16>();
                if let Ok(handle) = GlobalAlloc(GMEM_MOVEABLE, bytes) {
                    let ptr = GlobalLock(handle) as *mut u16;
                    if ptr.is_null() {
                        let _ = GlobalFree(Some(handle));
                    } else {
                        std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
                        let _ = GlobalUnlock(handle);
                        if SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(handle.0))).is_ok()
                        {
                            copied = true;
                        } else {
                            let _ = GlobalFree(Some(handle));
                        }
                    }
                }
            }
            let _ = CloseClipboard();
            copied
        }
    }

    /// Announce keyboard focus so a screen reader follows Tab/arrow movement.
    /// Deferred until the snapshot is refreshed after the callback.
    pub fn a11y_focus(&mut self, id: Option<WidgetId>) {
        if self.state.a11y.is_none() {
            return;
        }
        self.state.a11y_dirty = true;
        self.state.pending_a11y_focus = Some(id);
    }

    /// Raise a UIA notification for transient status text (a toast, a finished
    /// background task). Coordinates with nothing on screen: screen readers
    /// announce it without moving focus. Dropped when no client has ever
    /// requested the accessibility tree. Polite (the default) keeps only the
    /// newest message.
    pub fn a11y_announce(&mut self, text: impl Into<String>) {
        self.a11y_announce_with(text, AnnouncementPriority::Polite);
    }

    /// Like [`WindowHandle::a11y_announce`], with an explicit priority:
    /// `Assertive` interrupts the current utterance (errors, destructive
    /// actions), `Polite` replaces older status announcements.
    pub fn a11y_announce_with(
        &mut self,
        text: impl Into<String>,
        priority: AnnouncementPriority,
    ) {
        if self.state.a11y.is_none() {
            return;
        }
        self.state.pending_a11y_announce = Some((text.into(), priority));
    }

    /// Cumulative rectangle invalidations (diagnostics and benchmarks).
    pub fn invalidations(&self) -> u64 {
        self.state.invalidations
    }

    /// Paint pending invalidations immediately (synchronous `WM_PAINT`).
    /// Requested from a callback, it runs after the callback returns so the
    /// host is not re-entered.
    pub fn flush(&mut self) {
        self.state.flush_requested = true;
    }

    /// Request one normal page frame before overlay compositing continues.
    /// Call this when an application shows a modal backdrop: the frame paints
    /// the page and the visible scrim normally, so the cached page is complete
    /// and the scrim is blended exactly once (a stale page would show through
    /// the overlay as ghost pixels).
    pub fn resync_overlay(&mut self) {
        self.state.overlay_resync = true;
    }

    /// Bring the window to the top of the non-topmost z-order without
    /// activating it, then re-scan exposure. Useful right after startup when
    /// the launching console may still be above the new window.
    pub fn raise(&mut self) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        self.state.refresh_visibility();
        if self.state.exposed {
            self.state.invalidate_all();
        }
    }

    /// Hide the window (it keeps running; tray/hotkey callbacks still fire).
    /// `Host::visibility_changed(false, ..)` follows.
    pub fn hide(&mut self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Minimize the window. `Host::visual_state_changed(Minimized, ..)` follows.
    pub fn minimize(&mut self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_MINIMIZE);
        }
    }

    /// Maximize the window. `Host::visual_state_changed(Maximized, ..)`
    /// follows.
    pub fn maximize(&mut self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_MAXIMIZE);
        }
    }

    /// Restore the window without activating it.
    pub fn restore(&mut self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_RESTORE);
        }
    }

    /// Ask the window to close as if the user had. The request runs through
    /// `Host::close_requested(CloseReason::Programmatic, ..)`; a veto keeps
    /// the window open.
    pub fn request_close(&mut self) {
        self.state.close_reason = Some(CloseReason::Programmatic);
        unsafe {
            let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }

    /// Whether the window is visible (`IsWindowVisible`).
    pub fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd) }.as_bool()
    }

    /// Whether the window is the foreground/active window.
    pub fn is_active(&self) -> bool {
        self.state.active
    }

    /// The window's visual state (normal/minimized/maximized).
    pub fn visual_state(&self) -> WindowVisualState {
        self.state.visual_state
    }

    /// Which renderer paints this window (`Gdi` after a Direct2D fallback).
    pub fn renderer(&self) -> Renderer {
        self.state.renderer
    }

    /// The frame's top-left corner in screen DIPs.
    pub fn frame_position(&self) -> Point {
        self.state.frame_pos
    }

    /// Restore the window if it is minimized or hidden, bring it to the
    /// foreground and re-scan exposure. This is the usual reaction to a tray
    /// icon activation.
    pub fn show(&mut self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(self.hwnd);
        }
        self.state.refresh_visibility();
        self.state.invalidate_all();
    }

    /// Whether any client pixels are currently exposed.
    pub fn exposed(&self) -> bool {
        self.state.exposed
    }

    /// Number of exposed client pieces (1 = unobstructed, 0 = covered).
    pub fn visible_pieces(&self) -> usize {
        self.state.visible.len()
    }

    pub fn set_title(&mut self, title: &str) {
        let title = wide(title);
        unsafe {
            let _ = SetWindowTextW(self.hwnd, PCWSTR(title.as_ptr()));
        }
    }

    /// Install a new theme. Drops cached fonts when the text styles change and
    /// repaints the whole window once.
    pub fn set_theme(&mut self, settings: crate::theme::ThemeSettings, dark: bool) {
        if self.state.painter.set_theme(&settings, dark) {
            self.state.gdi.clear_fonts();
        }
        // Menus and the shell dialogs follow the same decision as the page.
        dark_theme::follow_theme(dark);
        dark_theme::apply_to_window(self.state.hwnd);
        // The owner-drawn window menu bar is non-client: redraw it with the
        // new palette in both the light and the dark direction.
        unsafe {
            let _ = DrawMenuBar(self.state.hwnd);
        }
        // `DrawMenuBar` may repaint only the changed items; repaint the whole
        // strip so it follows the new palette in both directions.
        self.state.paint_menu_bar_strip();
        self.state.update_tooltip(None);
        self.state.invalidate_all();
        // The native editor is a separate window; repaint it with the new
        // background and text colors too.
        if let Some(session) = self.state.edit.as_ref() {
            unsafe {
                let _ = InvalidateRect(Some(session.hwnd), None, true);
            }
        }
    }

    /// Install a whole palette (for example derived from a `.buitheme`
    /// document, D150) and repaint the window once. Text styles are unchanged;
    /// use [`WindowHandle::set_theme`] for the light/dark decision and fonts.
    /// The native menus and the native editor follow the new colours too.
    pub fn set_palette(&mut self, palette: crate::theme::Palette) {
        let dark = palette.dark;
        self.state.painter.set_palette(palette);
        dark_theme::follow_theme(dark);
        dark_theme::apply_to_window(self.state.hwnd);
        unsafe {
            let _ = DrawMenuBar(self.state.hwnd);
        }
        self.state.paint_menu_bar_strip();
        self.state.update_tooltip(None);
        self.state.invalidate_all();
        if let Some(session) = self.state.edit.as_ref() {
            unsafe {
                let _ = InvalidateRect(Some(session.hwnd), None, true);
            }
        }
    }

    /// The palette the window paints with (chrome, page fills, native menus).
    pub fn palette(&self) -> &crate::theme::Palette {
        self.state.painter.palette()
    }

    /// Exit the message loop with `code`; the window is destroyed afterwards.
    pub fn quit(&mut self, code: i32) {
        self.state.quit_code = code;
        unsafe { PostQuitMessage(code) };
    }
}

impl UiHost for WindowHandle<'_> {
    fn begin_edit(&mut self, request: &EditRequest<'_>) {
        self.state
            .queue_edit(EditOp::Begin(EditParams::from_request(request)));
    }

    fn end_edit(&mut self) {
        WindowHandle::end_edit(self);
    }

    fn move_edit(&mut self, rect: Rect) {
        self.state.queue_edit(EditOp::Move(rect));
    }

    fn editing_id(&self) -> Option<WidgetId> {
        self.state.editing_id()
    }

    fn set_tooltip(&mut self, target: Option<TooltipRequest<'_>>) {
        self.state.update_tooltip(target);
    }

    fn clipboard_text(&mut self) -> Option<String> {
        self.state.read_clipboard()
    }

    fn copy_to_clipboard(&mut self, text: &str) {
        let _ = self.copy_text(text);
    }

    fn set_caret_rect(&mut self, rect: Option<Rect>) {
        self.state.set_caret_rect(rect);
    }

    fn set_soft_keyboard(&mut self, visible: bool) -> bool {
        self.state.set_soft_keyboard(visible)
    }

    fn set_touch_keyboard(&mut self, visible: bool) -> bool {
        if !visible {
            return self.state.set_soft_keyboard(false);
        }
        // The touch layout: the shell picks the touch keyboard (the gamepad
        // one an application may have raised is the same window, so the flag
        // stays shared and one dismissal path covers both).
        let shown = crate::platform::keyboard::show(crate::platform::keyboard::Layout::Touch);
        self.state.soft_keyboard = shown;
        shown
    }

    fn soft_keyboard_open(&self) -> bool {
        crate::platform::keyboard::is_open()
    }

    fn start_drag(&mut self, data: DragData) -> bool {
        if !self.state.system_drag || !self.state.ole || data.is_empty() {
            return false;
        }
        // Queued for the message loop: OLE's DoDragDrop calls the drop target
        // back, which must be free to enter the host.
        self.state.pending_drag = Some(data);
        true
    }

    fn show_context_menu(&mut self, request: ContextMenuRequest<'_>) {
        // Deferred: `TrackPopupMenu` runs a modal loop that would otherwise
        // dispatch paints and timers while the host callback is in progress.
        self.state.request_menu(PendingMenu {
            widget: request.widget,
            menu: request.menu.clone(),
            anchor: request.anchor,
            avoid: request.avoid,
            source: request.source,
        });
    }
}

fn ensure_process_dpi_awareness() {
    DPI_AWARE.get_or_init(|| unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    });
}

fn register_class(instance: HINSTANCE) -> Result<()> {
    if CLASS_REGISTERED.get().copied().unwrap_or(false) {
        return Ok(());
    }
    let class = WNDCLASSW {
        // CS_DBLCLKS: report WM_*BUTTONDBLCLK; the preceding down/up pair is
        // still delivered as ordinary clicks, so both triggers can fire.
        style: WNDCLASS_STYLES(CS_DBLCLKS.0),
        hInstance: instance,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        lpszClassName: class_name(),
        lpfnWndProc: Some(window_proc),
        ..Default::default()
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err(windows::core::Error::from_thread());
    }
    let _ = CLASS_REGISTERED.set(true);
    Ok(())
}

unsafe fn reclaim(hwnd: HWND) {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowState;
    if !ptr.is_null() {
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(ptr));
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowState;
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    let state = unsafe { &mut *ptr };
    if state.taskbar_created != 0 && message == state.taskbar_created {
        // The shell (Explorer) restarted and forgot every tray icon.
        state.readd_trays();
        return LRESULT(0);
    }
    match message {
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
            state.paint(hdc, paint.rcPaint);
            unsafe {
                let _ = EndPaint(hwnd, &paint);
            }
            if state.started && !state.loaded {
                state.loaded = true;
                state.dispatch_loaded();
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_CTLCOLOREDIT => {
            // wParam is the control's HDC, lParam is the control handle.
            let hdc = HDC(wparam.0 as *mut c_void);
            let child = HWND(lparam.0 as *mut c_void);
            let is_editor = state
                .edit
                .as_ref()
                .map(|session| session.hwnd == child)
                .unwrap_or(false);
            if !is_editor {
                return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            }
            let pal = *state.painter.palette();
            unsafe {
                SetTextColor(hdc, COLORREF(pal.text.to_colorref()));
                SetBkColor(hdc, COLORREF(pal.search_bg.to_colorref()));
            }
            LRESULT(state.gdi.area_brush(pal.search_bg).0 as isize)
        }
        WM_COMMAND => {
            let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
            let child = HWND(lparam.0 as *mut c_void);
            // A menu-bar command arrives with no child window and code 0
            // (editor notifications carry the child HWND and a notification
            // code instead).
            if lparam.0 == 0 && code == 0 {
                state.dispatch_menu_bar_command((wparam.0 & 0xFFFF) as u32);
                return LRESULT(0);
            }
            let is_editor = state
                .edit
                .as_ref()
                .map(|session| session.hwnd == child)
                .unwrap_or(false);
            if is_editor {
                match code {
                    EN_CHANGE => {
                        if let Some(session) = state.edit.as_ref() {
                            let id = session.id;
                            let text = read_text(child);
                            state.notify_edit_changed(id, &text);
                        }
                    }
                    EN_KILLFOCUS => {
                        state.queue_edit(EditOp::End { commit: true });
                        state.process_edit_ops();
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_APP_EDIT_TRASH => {
            state.drop_edit_trash();
            LRESULT(0)
        }
        WM_APP_SHOW_MENU => {
            state.show_pending_menu();
            LRESULT(0)
        }
        WM_APP_MENU_OPENED => {
            // Dispatched by the popup's own loop, so this runs while the menu
            // is up and its model can still be edited.
            state.fire_menu_opened();
            LRESULT(0)
        }
        WM_MEASUREITEM => {
            if let Some(item) = unsafe { (lparam.0 as *mut MEASUREITEMSTRUCT).as_mut() }
                && state.measure_bar_item(item)
            {
                return LRESULT(1);
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_DRAWITEM => {
            if let Some(item) = unsafe { (lparam.0 as *const DRAWITEMSTRUCT).as_ref() }
                && state.draw_bar_item(item)
            {
                return LRESULT(1);
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_NCPAINT | WM_NCACTIVATE => {
            // The system paints the frame and the owner-drawn bar items; fill
            // the strip outside the items with the page palette so the native
            // bar does not stay light on a dark page. `WM_NCACTIVATE` redraws
            // the frame without a following `WM_NCPAINT`, so it needs the
            // same pass (otherwise switching away and back leaves a light
            // gap).
            let result = unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            state.paint_menu_bar_strip();
            result
        }
        WM_APP_MENU_CLICK => {
            let click = unsafe { *Box::from_raw(lparam.0 as *mut PendingMenuClick) };
            state.dispatch_menu_click(click.widget, click.command);
            LRESULT(0)
        }
        WM_APP_SHOW_BAR_MENU => {
            state.show_bar_menu(wparam.0);
            LRESULT(0)
        }
        WM_APP_DIALOG_DONE => {
            let done = unsafe { *Box::from_raw(lparam.0 as *mut DialogDone) };
            state.finish_dialog(done);
            LRESULT(0)
        }
        WM_APP_TRAY => {
            state.handle_tray(wparam, lparam);
            LRESULT(0)
        }
        WM_HOTKEY => {
            let id = wparam.0 as i32;
            if let Some(&(_, hotkey)) = state
                .hotkeys
                .iter()
                .find(|(registered, _)| *registered == id)
            {
                state.dispatch_hotkey(hotkey);
            }
            LRESULT(0)
        }
        WM_GETOBJECT => {
            // UIA asks for the fragment root with UiaRootObjectId; MSAA and
            // older clients ask for OBJID_CLIENT. Both get the same provider.
            let object_id = lparam.0 as i32;
            if object_id == UiaRootObjectId || object_id == OBJID_CLIENT.0 {
                state.build_a11y_tree();
                if let Some(tree) = state.a11y.clone() {
                    let root = UiaProvider::root(tree);
                    let result = unsafe { UiaReturnRawElementProvider(hwnd, wparam, lparam, &root) };
                    if std::env::var_os("BAMBOO_UI_DEBUG").is_some() {
                        eprintln!(
                            "bamboo-ui: WM_GETOBJECT id={object_id} nodes={} result={:?}",
                            state.a11y.as_ref().map(|tree| tree.node_count()).unwrap_or(0),
                            result
                        );
                    }
                    return result;
                }
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_APP_A11Y_ACTION => {
            let request = unsafe { *Box::from_raw(lparam.0 as *mut A11yRequest) };
            state.dispatch_a11y(request);
            LRESULT(0)
        }
        WM_SIZE => {
            let mut client = RECT::default();
            let _ = unsafe { GetClientRect(hwnd, &mut client) };
            let dpi = normalize_dpi(unsafe { GetDpiForWindow(hwnd) });
            state.dpi = dpi;
            state.size = client_size(client, dpi);
            if std::env::var_os("BAMBOO_UI_DEBUG").is_some() {
                eprintln!(
                    "bamboo-ui: WM_SIZE client={}x{} dpi={dpi}",
                    state.size.width, state.size.height
                );
            }
            state.refresh_visibility();
            state.invalidate_all();
            state.update_tooltip(None);
            if state.started {
                let visual = if unsafe { IsIconic(hwnd) }.as_bool() {
                    WindowVisualState::Minimized
                } else if unsafe { IsZoomed(hwnd) }.as_bool() {
                    WindowVisualState::Maximized
                } else {
                    WindowVisualState::Normal
                };
                if visual != state.visual_state {
                    state.visual_state = visual;
                    state.dispatch_visual_state(visual);
                }
            }
            state.dispatch_resize();
            LRESULT(0)
        }
        WM_DPICHANGED => {
            if let Some(suggested) = unsafe { (lparam.0 as *const RECT).as_ref() } {
                let _ = unsafe {
                    SetWindowPos(
                        hwnd,
                        None,
                        suggested.left,
                        suggested.top,
                        suggested.right - suggested.left,
                        suggested.bottom - suggested.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                };
            }
            state.dpi = normalize_dpi(unsafe { GetDpiForWindow(hwnd) });
            state.painter.set_caret_width(system_caret_width(state.scale()));
            state.gdi.clear_fonts();
            // The bar's cached menu font is DPI-specific; rebuild it and
            // repaint the non-client bar at the new scale.
            state.release_bar_font();
            unsafe {
                let _ = DrawMenuBar(hwnd);
            }
            state.invalidate_all();
            state.update_tooltip(None);
            state.dispatch_dpi();
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
            let scale = state.scale();
            info.ptMinTrackSize = POINT {
                x: (state.min_size.width * scale).round() as i32,
                y: (state.min_size.height * scale).round() as i32,
            };
            LRESULT(0)
        }
        WM_SHOWWINDOW => {
            let visible = wparam.0 != 0;
            if visible {
                state.refresh_visibility();
                state.invalidate_all();
            } else {
                state.visible.clear();
                state.exposed = false;
                state.update_tooltip(None);
            }
            if state.started && state.shown != visible {
                state.shown = visible;
                state.dispatch_visibility(visible);
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            let active = (wparam.0 & 0xFFFF) != 0; // WA_INACTIVE is 0
            if !active {
                // A deactivated window loses the field focus the keyboard was
                // opened for; handing focus to the native EDIT child is not a
                // deactivation, so this never hides a working keyboard.
                state.dismiss_soft_keyboard();
            }
            if state.started && state.active != active {
                state.active = active;
                state.dispatch_activation(active);
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_WINDOWPOSCHANGED => {
            // DefWindowProc turns this into WM_SIZE / WM_MOVE. Swallowing it
            // would leave the client size stale after every resize, including
            // maximize, so the layout and paints would cover only the old area.
            let result = unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            state.refresh_visibility();
            result
        }
        WM_MOVE => {
            state.refresh_visibility();
            state.update_tooltip(None);
            if state.started {
                let mut frame = RECT::default();
                let _ = unsafe { GetWindowRect(hwnd, &mut frame) };
                let scale = state.scale();
                let position = Point::new(frame.left as f32 / scale, frame.top as f32 / scale);
                if (position.x - state.frame_pos.x).abs() > 0.01
                    || (position.y - state.frame_pos.y).abs() > 0.01
                {
                    state.frame_pos = position;
                    state.dispatch_moved(position);
                }
            }
            LRESULT(0)
        }
        WM_DROPFILES => {
            let drop = HDROP(wparam.0 as *mut c_void);
            let count = unsafe { DragQueryFileW(drop, u32::MAX, None) };
            let mut paths = Vec::with_capacity(count as usize);
            for index in 0..count {
                let length = unsafe { DragQueryFileW(drop, index, None) } as usize;
                let mut buffer = vec![0u16; length + 1];
                let written = unsafe { DragQueryFileW(drop, index, Some(&mut buffer)) } as usize;
                paths.push(String::from_utf16_lossy(&buffer[..written]));
            }
            unsafe { DragFinish(drop) };
            state.dispatch_files(paths);
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let target = HWND(wparam.0 as *mut c_void);
            // wParam is the window under the cursor; a child (the native
            // editor) must keep its own class cursor (I-beam), so only the
            // client area of this window uses the stored shape.
            if (lparam.0 as u32 & 0xFFFF) == HTCLIENT_CODE && target == hwnd {
                unsafe {
                    SetCursor(Some(cursor_handle(state.cursor)));
                }
                return LRESULT(1);
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_CAPTURECHANGED => {
            state.capturing = false;
            state.cancel_long_press();
            LRESULT(0)
        }
        WM_TIMER => {
            match wparam.0 {
                OCCLUSION_TIMER => state.refresh_visibility(),
                TOOLTIP_DWELL_TIMER => state.show_tooltip(),
                TOOLTIP_HIDE_TIMER => state.hide_tooltip(),
                TOOLTIP_ANIM_TIMER => state.animate_tooltip(),
                CARET_TIMER => state.blink_caret(),
                LONG_PRESS_TIMER => state.fire_long_press(),
                _ => state.dispatch_timer(),
            }
            LRESULT(0)
        }
        WM_SETTINGCHANGE => {
            // Accessibility settings can change while the app runs; the caret
            // bar width is the one this framework mirrors.
            let width = system_caret_width(state.scale());
            if (state.painter.caret_width() - width).abs() > 0.01 {
                state.painter.set_caret_width(width);
                state.invalidate_all_force();
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_SETFOCUS => {
            if state.caret_rect.is_some() {
                state.caret_on = true;
                state.start_caret_timer();
                if let Some(rect) = state.caret_rect {
                    state.invalidate(rect);
                }
            }
            state.dispatch(Event::Focus(true));
            LRESULT(0)
        }
        WM_KILLFOCUS => {
            state.bar_armed = false;
            state.alt_tap_pending = false;
            if std::mem::take(&mut state.alt_underlines) {
                state.dispatch(Event::Alt(false));
            }
            // Leaving the window (or handing focus to the native editor)
            // dismisses a visible tip. A pending hover/focus dwell is left
            // armed, so an inactive window can still show a hover tip.
            if state.hotkey_capture {
                // A shortcut is meaningless without focus (and Alt+Tab would
                // otherwise be captured as `Alt+Tab`).
                state.finish_hotkey_capture(None);
            }
            state.hide_tooltip_now();
            // A painted editor keeps its session and value, but the caret is
            // meaningless while the window is inactive.
            state.stop_caret_timer();
            state.caret_on = false;
            if let Some(rect) = state.caret_rect {
                state.invalidate(rect);
            }
            state.dispatch(Event::Focus(false));
            LRESULT(0)
        }
        WM_IME_SETCONTEXT => {
            // The composition is drawn inline by the focused field, so the
            // system composition window would duplicate it. Keep the candidate
            // window (and the other IME UI).
            let flags = lparam.0 & !(ISC_SHOWUICOMPOSITIONWINDOW as isize);
            unsafe { DefWindowProcW(hwnd, message, wparam, LPARAM(flags)) }
        }
        WM_IME_STARTCOMPOSITION => {
            // Re-assert the caret position: some IMEs open their candidate
            // window on start rather than on the first composition update.
            state.apply_ime_position();
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_IME_COMPOSITION => {
            let flags = lparam.0 as u32;
            let himc = unsafe { ImmGetContext(hwnd) };
            if himc.0.is_null() {
                return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            }
            let result = if flags & GCS_RESULTSTR.0 != 0 {
                state.ime_composition_string(himc, GCS_RESULTSTR)
            } else {
                String::new()
            };
            let (composition, cursor) = if flags & GCS_COMPSTR.0 != 0 {
                let text = state.ime_composition_string(himc, GCS_COMPSTR);
                let cursor = unsafe { ImmGetCompositionStringW(himc, GCS_CURSORPOS, None, 0) };
                (text, cursor.max(0) as usize)
            } else {
                (String::new(), 0)
            };
            unsafe {
                let _ = ImmReleaseContext(hwnd, himc);
            }
            if !result.is_empty() {
                state.flush_requested = true;
                state.dispatch_ime_commit(result);
            }
            if flags & GCS_COMPSTR.0 != 0 {
                state.flush_requested = true;
                state.dispatch_ime_compose(composition, cursor);
            }
            LRESULT(0)
        }
        WM_IME_ENDCOMPOSITION => {
            state.flush_requested = true;
            state.dispatch_ime_compose(String::new(), 0);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            state.sync_pointer_kind();
            if !state.tracking_leave {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                unsafe {
                    let _ = TrackMouseEvent(&mut track);
                }
                state.tracking_leave = true;
            }
            let pos = point_from_lparam(lparam, state.scale());
            state.long_press_moved(pos);
            state.dispatch(Event::PointerMove {
                pos,
                buttons: buttons_from_wparam(wparam.0 as u32),
            });
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            state.tracking_leave = false;
            state.cancel_long_press();
            state.dispatch(Event::PointerLeave);
            LRESULT(0)
        }
        WM_LBUTTONDBLCLK => {
            // Windows replaces the second click's `WM_LBUTTONDOWN` with this
            // message. Dispatch it as a press as well, so rapid clicking (a
            // numeric stepper, a repeat button) counts every click instead of
            // dropping every other one; the real `WM_LBUTTONUP` follows and
            // turns it into a normal release/click.
            if state.edit.is_some() {
                if forward_edit_click(state, lparam, wparam) {
                    return LRESULT(0);
                }
                state.queue_edit(EditOp::End { commit: true });
                state.process_edit_ops();
            }
            if !state.capturing {
                unsafe {
                    SetCapture(hwnd);
                }
                state.capturing = true;
            }
            state.sync_pointer_kind();
            let pos = point_from_lparam(lparam, state.scale());
            state.arm_long_press(pos);
            state.dispatch(Event::PointerDown {
                pos,
                button: MouseButton::Left,
                modifiers: pointer_modifiers(),
            });
            state.dispatch(Event::PointerDoubleClick {
                pos,
                button: MouseButton::Left,
            });
            LRESULT(0)
        }
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
            state.bar_armed = false;
            // A press means the Alt-tap underline mode is over.
            if std::mem::take(&mut state.alt_underlines) {
                state.dispatch(Event::Alt(false));
            }
            if state.edit.is_some() {
                // A click inside the field but outside the text strip belongs
                // to the editor; forward it so the caret lands under the
                // pointer instead of closing and re-opening the session.
                if message == WM_LBUTTONDOWN && forward_edit_click(state, lparam, wparam) {
                    return LRESULT(0);
                }
                // A secondary click inside the field keeps the session and
                // shows the native EDIT context menu (paste/undo/select all).
                if message == WM_RBUTTONDOWN && forward_edit_context(state, lparam) {
                    return LRESULT(0);
                }
                // Anything else is outside the field: commit like kill-focus.
                state.queue_edit(EditOp::End { commit: true });
                state.process_edit_ops();
            }
            if !state.capturing {
                unsafe {
                    SetCapture(hwnd);
                }
                state.capturing = true;
            }
            state.sync_pointer_kind();
            let pos = point_from_lparam(lparam, state.scale());
            if message == WM_LBUTTONDOWN {
                state.arm_long_press(pos);
            }
            state.dispatch(Event::PointerDown {
                pos,
                button: button_from_message(message),
                modifiers: pointer_modifiers(),
            });
            if message == WM_RBUTTONDOWN && state.edit.is_some() {
                // The secondary click just opened a field (or kept one open):
                // show the native EDIT context menu too, like right-clicking a
                // closed native control. Drop the parent's capture first so
                // the modal menu owns the mouse.
                if state.capturing {
                    unsafe {
                        let _ = ReleaseCapture();
                    }
                    state.capturing = false;
                }
                forward_edit_context(state, lparam);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
            state.sync_pointer_kind();
            state.cancel_long_press();
            state.dispatch(Event::PointerUp {
                pos: point_from_lparam(lparam, state.scale()),
                button: button_from_message(message),
                modifiers: pointer_modifiers(),
            });
            if state.capturing {
                unsafe {
                    let _ = ReleaseCapture();
                }
                state.capturing = false;
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let mut point = POINT {
                x: lparam.0 as i16 as i32,
                y: (lparam.0 >> 16) as i16 as i32,
            };
            unsafe {
                let _ = ScreenToClient(hwnd, &mut point);
            }
            let scale = state.scale();
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as f32 / 120.0;
            // Shift+wheel is the horizontal notch, like a native scroll view;
            // a plain wheel keeps the vertical axis only.
            let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
            state.dispatch(Event::Wheel {
                pos: Point::new(point.x as f32 / scale, point.y as f32 / scale),
                delta_x: if shift { delta } else { 0.0 },
                delta_y: if shift { 0.0 } else { delta },
            });
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            // A new key press means any translated character of the previous
            // one was already delivered; clear a stale suppression flag.
            state.swallow_char = false;
            let vk = wparam.0 as u16;
            if state.hotkey_capture {
                // Swallow the chord: the application must not also act on it.
                state.capture_hotkey(vk);
                return LRESULT(0);
            }
            // Access-key underlines: an Alt press with no chord in flight is a
            // candidate tap; any other key means it was a chord modifier and
            // also leaves the "underlines shown" mode.
            if vk == VK_MENU.0 {
                if (lparam.0 & (1 << 30)) == 0 {
                    state.alt_tap_pending = true;
                }
            } else {
                state.alt_tap_pending = false;
                if std::mem::take(&mut state.alt_underlines) {
                    state.dispatch(Event::Alt(false));
                }
            }
            // Window-scoped shortcuts: the menu bar's accelerator labels and
            // the application's registered chords fire before the page sees
            // the key. The shell owns the keys while its menu is up; capture
            // (above) owns everything else. The translated character of a
            // matched chord is dropped.
            if (!state.window_hotkeys.is_empty() || !state.menu_hotkeys.is_empty())
                && !is_modifier_vk(vk)
                && !WindowState::menu_mode_active()
            {
                let modifiers = HotkeyModifiers {
                    ctrl: key_down(VK_CONTROL.0 as i32),
                    shift: key_down(VK_SHIFT.0 as i32),
                    alt: key_down(VK_MENU.0 as i32),
                    win: key_down(VK_LWIN.0 as i32) || key_down(VK_RWIN.0 as i32),
                };
                if let Some(command) = state.window_hotkey_command(vk, modifiers) {
                    state.swallow_char = true;
                    state.dispatch_menu_bar_command(command);
                    return LRESULT(0);
                }
            }
            // The bar titles are owner-drawn, so they carry no text the shell
            // could parse for `Alt+key` mnemonics. Resolve the chord here and
            // open that title's menu; the translated character is dropped.
            // The bar titles are owner-drawn, so they carry no text the shell
            // could parse for mnemonics. Resolve `Alt+key` here — and a plain
            // key while the shell is in menu mode (Alt pressed and released),
            // which is how a native bar is driven — and open that title's
            // menu; the translated character is dropped.
            // Only a *tap* of Alt enables mnemonics; a chord held with Alt
            // (`Alt+P`) keeps its application meaning.
            let menu_mode = WindowState::menu_mode_active();
            let mnemonic_ready = state.window_menu.is_some()
                && !state.bar_alt_down
                && (menu_mode || state.bar_armed);
            if mnemonic_ready
                && let Some(ch) = mnemonic_char(vk)
                && let Some(index) = state.bar_mnemonic_pick(ch)
            {
                state.swallow_char = true;
                unsafe {
                    let _ = PostMessageW(
                        Some(hwnd),
                        WM_APP_SHOW_BAR_MENU,
                        WPARAM(index),
                        LPARAM(0),
                    );
                }
                return LRESULT(0);
            }
            // Any other key means the Alt press was not a tap; a key that is
            // not a title mnemonic also leaves the Alt-tap mode. The shell's
            // own menu keys (arrows/Enter/Escape) keep it.
            if vk != VK_MENU.0 {
                state.bar_alt_down = false;
            }
            if vk != VK_MENU.0
                && state.bar_armed
                && vk != VK_UP.0
                && vk != VK_DOWN.0
                && vk != VK_LEFT.0
                && vk != VK_RIGHT.0
                && vk != VK_RETURN.0
            {
                state.bar_armed = false;
            }
            // While the shell drives the native menu bar (Alt/F10 pressed or
            // a bar menu open) the keys belong to the menu, not to the page.
            if state.window_menu.is_some() && WindowState::menu_mode_active() {
                return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            }
            let consumed = state.dispatch(Event::Key(KeyEvent {
                code: KeyCode::from_vk(vk),
                vk,
                ctrl: key_down(VK_CONTROL.0 as i32),
                shift: key_down(VK_SHIFT.0 as i32),
                alt: key_down(VK_MENU.0 as i32),
                repeat: (lparam.0 & (1 << 30)) != 0,
            }));
            // Alt/F10/access keys activate the native bar through the default
            // window procedure (it also handles Alt+F4/Alt+Space). A key the
            // application consumed never reaches it.
            if !consumed && message == WM_SYSKEYDOWN && state.window_menu.is_some() {
                if vk == VK_MENU.0 {
                    // Remember the press; the release decides whether it was
                    // a tap (mnemonics on) or a chord modifier.
                    state.bar_alt_down = true;
                }
                return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            }
            LRESULT(0)
        }
        WM_KEYUP | WM_SYSKEYUP => {
            let vk = wparam.0 as u16;
            if message == WM_SYSKEYUP && vk == VK_MENU.0 {
                if state.alt_tap_pending {
                    // Released without a chord: show access-key underlines until
                    // the next key, pointer press or focus change.
                    state.alt_tap_pending = false;
                    if !state.alt_underlines {
                        state.alt_underlines = true;
                        state.dispatch(Event::Alt(true));
                    }
                }
                if state.bar_alt_down {
                    state.bar_alt_down = false;
                    state.bar_armed = state.window_menu.is_some();
                }
            }
            // Observation only: the application hands it to the focused widget
            // (the edit-box `on_key_up` hook). A native child editor owns its
            // own key releases.
            state.dispatch(Event::KeyUp(KeyEvent {
                code: KeyCode::from_vk(vk),
                vk,
                ctrl: key_down(VK_CONTROL.0 as i32),
                shift: key_down(VK_SHIFT.0 as i32),
                alt: key_down(VK_MENU.0 as i32),
                repeat: false,
            }));

            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_MENUSELECT => {
            // No item selected (`0xFFFF` in the high word) means the menu
            // mode ended: drop the Alt tap state and repaint the bar so stale
            // access-key underlines never outlive their menu.
            let cleared = (wparam.0 >> 16) & 0xFFFF == 0xFFFF;
            if cleared {
                state.bar_armed = false;
                state.bar_alt_down = false;
                // The menu lost focus: access keys are hidden again, and the
                // painted chrome's Alt-tap mode ends even when the shell
                // swallowed the Escape key.
                state.bar_no_accel = true;
                if std::mem::take(&mut state.alt_underlines) {
                    state.dispatch(Event::Alt(false));
                }
                state.paint_menu_bar_strip();
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_CHAR | WM_SYSCHAR => {
            // A key consumed by hotkey capture still translated into a
            // character; drop exactly that one.
            if std::mem::take(&mut state.swallow_char) {
                return LRESULT(0);
            }
            if message == WM_CHAR {
                let value = wparam.0 as u32;
                let printable = value >= 0x20 || matches!(value, 0x08 | 0x09 | 0x0D);
                if printable && let Some(ch) = char::from_u32(value) {
                    state.dispatch(Event::Char(ch));
                }
                LRESULT(0)
            } else {
                unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
            }
        }
        WM_SYSCOMMAND => {
            // A title-bar close, Alt+F4 and the system menu all arrive as
            // SC_CLOSE; remember who asked so WM_CLOSE can report it.
            if (wparam.0 & 0xFFF0) == SC_CLOSE as usize {
                state.close_reason = Some(CloseReason::User);
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_CLOSE => {
            // WM_CLOSE has no source of its own: a SC_CLOSE request is the
            // user, request_close flags itself, and a bare WM_CLOSE (Task
            // Manager, a tool) counts as a user request.
            let reason = state.close_reason.take().unwrap_or(CloseReason::User);
            if state.dispatch_close_request(reason) {
                // DefWindowProc performs the default close (DestroyWindow).
                return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            }
            LRESULT(0)
        }
        WM_QUERYENDSESSION => {
            // Logoff/shutdown: the application may still veto.
            let allow = state.dispatch_close_request(CloseReason::System);
            LRESULT(if allow { 1 } else { 0 })
        }
        WM_DESTROY => {
            state.started = false;
            state.dispatch_destroying();
            unsafe { PostQuitMessage(state.quit_code) };
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowState;
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                if !ptr.is_null() {
                    let mut state = Box::from_raw(ptr);
                    if let Some(host) = state.host.as_mut() {
                        host.destroy();
                    }
                    state.release_bar_font();
                    state.shutdown_edits();
                    state.shutdown_tooltip();
                    state.shutdown_tray();
                    state.shutdown_hotkeys();
                    if state.ole {
                        drag_drop::revoke(hwnd);
                        state.drop_target = None;
                        drag_drop::uninitialize();
                    }
                    if state.winrt {
                        native_notification::uninitialize();
                    }
                    drop(state);
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
        }
        WM_UNINITMENUPOPUP => {
            // The native EDIT context menu may be owned by the child or by
            // this window depending on the control; either way the popup can
            // have covered more than the owner. Force a full repaint because
            // `visible` still excludes the menu area (the occlusion scan has
            // not caught up), so a clipped invalidation would do nothing.
            state.invalidate_all_force();
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn normalize_dpi(dpi: u32) -> f32 {
    if dpi > 0 { dpi as f32 } else { 96.0 }
}

/// The caret width from the system accessibility setting, in DIPs. The setting
/// is a physical pixel width; a failed query falls back to 1.0 DIP.
fn system_caret_width(scale: f32) -> f32 {
    let mut width: i32 = 0;
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETCARETWIDTH,
            0,
            Some(&mut width as *mut i32 as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if result.is_err() || width <= 0 {
        1.0
    } else {
        (width as f32 / scale.max(0.5)).clamp(0.5, 6.0)
    }
}

fn client_size(client: RECT, dpi: f32) -> Size {
    let scale = dpi / 96.0;
    Size::new(
        (client.right - client.left).max(0) as f32 / scale,
        (client.bottom - client.top).max(0) as f32 / scale,
    )
}

fn point_from_lparam(lparam: LPARAM, scale: f32) -> Point {
    let x = (lparam.0 & 0xFFFF) as u16 as i16 as f32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as f32;
    Point::new(x / scale, y / scale)
}

fn buttons_from_wparam(wparam: u32) -> u32 {
    let left = (wparam & 0x0001 != 0) as u32;
    let right = ((wparam & 0x0002 != 0) as u32) << 1;
    let middle = ((wparam & 0x0010 != 0) as u32) << 2;
    left | right | middle
}

/// What produced the pointer message being handled: a touch screen and a pen
/// both arrive as mouse messages tagged in `GetMessageExtraInfo`.
fn pointer_kind_now() -> PointerKind {
    let extra = unsafe { GetMessageExtraInfo() }.0 as u32;
    if extra & POINTER_SIGNATURE_MASK != POINTER_SIGNATURE {
        PointerKind::Mouse
    } else if extra & POINTER_TOUCH_BIT != 0 {
        PointerKind::Touch
    } else {
        PointerKind::Pen
    }
}

fn button_from_message(message: u32) -> MouseButton {
    match message {
        WM_LBUTTONDOWN | WM_LBUTTONUP => MouseButton::Left,
        WM_RBUTTONDOWN | WM_RBUTTONUP => MouseButton::Right,
        _ => MouseButton::Middle,
    }
}

/// Ctrl/Shift held right now, for a pointer event. Sampled with `GetKeyState`
/// (the message flags lag a modifier pressed in the same chord).
fn pointer_modifiers() -> crate::event::PointerModifiers {
    crate::event::PointerModifiers {
        ctrl: unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0,
        shift: unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0,
    }
}

fn key_down(vk: i32) -> bool {
    (unsafe { GetKeyState(vk) } as u16 & 0x8000) != 0
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

/// Current text of a native edit control, with CRLF normalized to `\n` so
/// the widget, its accessibility value and the painted lines agree.
fn read_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut buf = vec![0u16; len + 1];
        let read = GetWindowTextW(hwnd, &mut buf).max(0) as usize;
        let raw = String::from_utf16_lossy(&buf[..read.min(len)]);
        crate::text::normalize_newlines(&raw).into_owned()
    }
}

/// Delete a font handle; tolerates the null handle used for "none".
fn delete_font(font: HFONT) {
    if !font.0.is_null() {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
}

/// Line height in device pixels for `font` (1 when measuring fails).
fn line_height_px(font: HFONT) -> i32 {
    unsafe {
        let dc = GetDC(None);
        if dc.is_invalid() {
            return 1;
        }
        let old = SelectObject(dc, HGDIOBJ(font.0));
        let mut metrics = TEXTMETRICW::default();
        let ok = GetTextMetricsW(dc, &mut metrics).as_bool();
        let _ = SelectObject(dc, old);
        let _ = ReleaseDC(None, dc);
        if ok { metrics.tmHeight.max(1) } else { 1 }
    }
}

/// Child rectangle for the editor, in client DIPs.
fn edit_child_rect(rect: Rect, multiline: bool, scale: f32, font: HFONT) -> Rect {
    let inner = rect.inset(1.0, 1.0);
    if multiline {
        return inner;
    }
    let line = line_height_px(font) as f32 / scale;
    let pad = ((inner.height() - line) * 0.5).max(0.0);
    if pad * 2.0 >= inner.height() {
        return inner;
    }
    Rect::new(inner.left, inner.top + pad, inner.right, inner.bottom - pad)
}

/// Forward a click that landed in the field's padding to the editor child.
/// Returns false when there is no open editor or the click is outside it.
fn forward_edit_click(state: &WindowState, lparam: LPARAM, wparam: WPARAM) -> bool {
    let Some(session) = state.edit.as_ref() else {
        return false;
    };
    let scale = state.scale();
    let pos = point_from_lparam(lparam, scale);
    if !session.rect.contains_point(pos) {
        return false;
    }
    let width = (session.child.width() * scale).round().max(1.0) as i32;
    let height = (session.child.height() * scale).round().max(1.0) as i32;
    let x = (((pos.x - session.child.left) * scale) as i32).clamp(0, width - 1);
    let y = (((pos.y - session.child.top) * scale) as i32).clamp(0, height - 1);
    let local = LPARAM(((y << 16) | (x & 0xFFFF)) as isize);
    let child = session.hwnd;
    unsafe {
        let _ = SetFocus(Some(child));
        SendMessageW(child, WM_LBUTTONDOWN, Some(wparam), Some(local));
        SendMessageW(child, WM_LBUTTONUP, Some(WPARAM(0)), Some(local));
    }
    true
}

/// Right click inside an open editor's field: keep the session and show the
/// native `EDIT` context menu. The child only covers a text-height strip, so a
/// click in the field padding would otherwise close and commit the editor.
fn forward_edit_context(state: &WindowState, lparam: LPARAM) -> bool {
    let Some(session) = state.edit.as_ref() else {
        return false;
    };
    let scale = state.scale();
    let pos = point_from_lparam(lparam, scale);
    if !session.rect.contains_point(pos) {
        return false;
    }
    let mut screen = POINT {
        x: (pos.x * scale).round() as i32,
        y: (pos.y * scale).round() as i32,
    };
    unsafe {
        let _ = ClientToScreen(state.hwnd, &mut screen);
        let packed = LPARAM(((screen.y << 16) | (screen.x & 0xFFFF)) as isize);
        let child = session.hwnd;
        let _ = SetFocus(Some(child));
        SendMessageW(
            child,
            WM_CONTEXTMENU,
            Some(WPARAM(child.0 as usize)),
            Some(packed),
        );
    }
    true
}

fn cursor_handle(cursor: Cursor) -> windows::Win32::UI::WindowsAndMessaging::HCURSOR {
    let id = match cursor {
        Cursor::Arrow => IDC_ARROW,
        Cursor::Hand => IDC_HAND,
        Cursor::IBeam => IDC_IBEAM,
        Cursor::SizeWE => IDC_SIZEWE,
        Cursor::SizeNS => IDC_SIZENS,
    };
    unsafe { LoadCursorW(None, id).unwrap_or_default() }
}

/// Client-space rectangle covered by `window`, if it is an opaque occluder.
fn occluder_rect(target: HWND, window: HWND, scale: f32, client: Rect) -> Option<Rect> {
    if window == target {
        return None;
    }
    unsafe {
        if !IsWindowVisible(window).as_bool() || IsIconic(window).as_bool() {
            return None;
        }
        let ex = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32;
        if ex & (WS_EX_LAYERED.0 as u32) != 0 || ex & (WS_EX_TRANSPARENT.0 as u32) != 0 {
            return None;
        }
        // Menus (`#32768`) are transient popups: the menu's own dismissal
        // invalidates what it covered. Treating one as an occluder would drop
        // every invalidation under it while it is open, so the parent would
        // have nothing to paint when the menu closes.
        let mut class = [0u16; 16];
        let length = GetClassNameW(window, &mut class).max(0) as usize;
        if length == 6 && class[..6] == [0x23, 0x33, 0x32, 0x37, 0x36, 0x38] {
            return None;
        }
        let mut rect = RECT::default();
        if GetWindowRect(window, &mut rect).is_err() {
            return None;
        }
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return None;
        }
        let mut top_left = POINT {
            x: rect.left,
            y: rect.top,
        };
        let mut bottom_right = POINT {
            x: rect.right,
            y: rect.bottom,
        };
        let _ = ScreenToClient(target, &mut top_left);
        let _ = ScreenToClient(target, &mut bottom_right);
        let area = Rect::new(
            top_left.x as f32 / scale,
            top_left.y as f32 / scale,
            bottom_right.x as f32 / scale,
            bottom_right.y as f32 / scale,
        );
        area.intersect(client)
    }
}

/// Total area of a disjoint exposed-rectangle list, in DIPs².
fn exposed_area(rects: &[Rect]) -> f32 {
    rects
        .iter()
        .map(|rect| rect.width() * rect.height())
        .sum()
}

/// `area` minus `cover`, split into at most four rectangles.
fn subtract(area: Rect, cover: Rect) -> Vec<Rect> {
    let Some(inter) = area.intersect(cover) else {
        return vec![area];
    };
    let mut out = Vec::with_capacity(4);
    if inter.top > area.top {
        out.push(Rect::new(area.left, area.top, area.right, inter.top));
    }
    if inter.bottom < area.bottom {
        out.push(Rect::new(
            area.left,
            inter.bottom,
            area.right,
            area.bottom,
        ));
    }
    if inter.left > area.left {
        out.push(Rect::new(area.left, inter.top, inter.left, inter.bottom));
    }
    if inter.right < area.right {
        out.push(Rect::new(
            inter.right,
            inter.top,
            area.right,
            inter.bottom,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    /// The one matching rule: exact chord, application table first.
    #[test]
    fn hotkey_command_matches_the_exact_chord_and_takes_the_first_table() {
        use super::hotkey_command;
        let ctrl_p = Hotkey::parse("Ctrl+P").unwrap();
        let app = vec![(ctrl_p, 10), (Hotkey::parse("F5").unwrap(), 11)];
        let menu = vec![(ctrl_p, 20), (Hotkey::parse("Ctrl+Shift+P").unwrap(), 21)];
        assert_eq!(
            hotkey_command(&app, ctrl_p.vk, ctrl_p.modifiers),
            Some(10),
            "the application table wins"
        );
        // A chord only the menu carries still matches.
        let about = Hotkey::parse("Ctrl+Shift+P").unwrap();
        assert_eq!(
            hotkey_command(&menu, about.vk, about.modifiers),
            Some(21)
        );
        // Extra or missing modifiers do not match.
        let mut extra = ctrl_p.modifiers;
        extra.alt = true;
        assert_eq!(hotkey_command(&menu, ctrl_p.vk, extra), None);
        assert_eq!(hotkey_command(&menu, ctrl_p.vk, HotkeyModifiers::NONE), None);
        // A different key with the same modifiers does not match.
        assert_eq!(
            hotkey_command(&menu, Hotkey::parse("Ctrl+O").unwrap().vk, ctrl_p.modifiers),
            None
        );
    }

    use super::*;

    fn rect(l: f32, t: f32, r: f32, b: f32) -> Rect {
        Rect::new(l, t, r, b)
    }

    #[test]
    fn subtract_none_returns_the_original() {
        let pieces = subtract(rect(0.0, 0.0, 10.0, 10.0), rect(20.0, 20.0, 30.0, 30.0));
        assert_eq!(pieces, vec![rect(0.0, 0.0, 10.0, 10.0)]);
    }

    #[test]
    fn subtract_top_half_leaves_the_bottom() {
        let pieces = subtract(rect(0.0, 0.0, 10.0, 10.0), rect(0.0, 0.0, 10.0, 5.0));
        assert_eq!(pieces, vec![rect(0.0, 5.0, 10.0, 10.0)]);
    }

    #[test]
    fn subtract_full_cover_leaves_nothing() {
        let pieces = subtract(rect(0.0, 0.0, 10.0, 10.0), rect(-5.0, -5.0, 20.0, 20.0));
        assert!(pieces.is_empty());
    }

    #[test]
    fn subtract_center_leaves_four_bands() {
        let pieces = subtract(rect(0.0, 0.0, 10.0, 10.0), rect(3.0, 3.0, 7.0, 7.0));
        assert_eq!(pieces.len(), 4);
        let total: f32 = pieces.iter().map(|piece| piece.width() * piece.height()).sum();
        // 100 - 16 = 84, no overlap and no gap.
        assert!((total - 84.0).abs() < 0.01);
    }

    #[test]
    fn button_mask_maps_virtual_keys() {
        assert_eq!(buttons_from_wparam(0x0001), 1);
        assert_eq!(buttons_from_wparam(0x0002), 2);
        assert_eq!(buttons_from_wparam(0x0010), 4);
        assert_eq!(buttons_from_wparam(0x0001 | 0x0002 | 0x0010), 7);
    }
}
