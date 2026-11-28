// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The Windows on-screen keyboard, for text entered with a gamepad.
//!
//! A desktop window has no text-input host of its own, so the shell keyboard
//! has to be asked for explicitly. The paths are tried in the order Windows
//! itself uses on a controller:
//!
//! 1. `CoreInputView::TryShowWithKind(Gamepad)` — the controller-navigable
//!    layout Windows 11 24H2 exposes to handhelds. It needs a current view,
//!    which an unpackaged desktop process usually has none of; the call stays
//!    because a packaged or hosted window does, and the failure is harmless.
//! 2. `ITipInvocation` on the undocumented `CLSID_UIHostNoLaunch` — the
//!    interface the shell uses to show the regular touch keyboard. Launching
//!    `TabTip.exe` on its own is unreliable on Windows 11 because the host is
//!    often already running hidden and refuses to surface itself for a
//!    non-shell caller.
//! 3. Launching `TabTip.exe` / `osk.exe`, for the first-ever use when the
//!    shell class is not registered yet.
//!
//! Both shell calls are state-flipping in one direction or the other: `Toggle`
//! always inverts the keyboard, and `TryShowWithKind` can be ignored when
//! nothing has focus. So visibility is measured, not assumed —
//! `IFrameworkInputPane::Location` returns the input pane's screen rectangle
//! (empty while hidden) and works for the shell keyboard in an ordinary
//! desktop process. Window-class probing does *not* work: the Windows 11 input
//! service keeps a visible full-screen `Windows.UI.Core.CoreWindow` around
//! whether or not the keyboard is up, and the visible window's title is
//! localized ("Windows 输入体验").

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::{CLSCTX_ALL, CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::WinRT::{RO_INIT_SINGLETHREADED, RoInitialize};
use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;
use windows::UI::ViewManagement::Core::{CoreInputView, CoreInputViewKind};
use windows::core::{GUID, HRESULT};

/// Which layout to ask for. A gamepad asks for the controller layout first;
/// `Touch` goes straight to the touch keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Gamepad,
    Touch,
}

/// Undocumented shell interface Windows uses to show the touch keyboard.
#[windows::core::interface("37c994e7-432b-4834-a2f7-dce1f13b834b")]
unsafe trait ITipInvocation: windows::core::IUnknown {
    /// Named in snake case only to satisfy the lint; the vtable slot is the
    /// COM `Toggle` method.
    fn toggle(&self, wnd: HWND) -> windows::core::HRESULT;
}

const CLSID_UIHOST_NO_LAUNCH: GUID = GUID::from_u128(0x4ce576fa_83dc_4f88_951c_9d0782b4e376);

/// `IFrameworkInputPane` (documented): the shell's input-pane location, the
/// same signal a touch-aware app uses to move its content out of the way.
/// `Location` reports the keyboard's screen rectangle and an empty rectangle
/// while it is hidden. The vtable order includes `AdviseWithHWND` and
/// `LocationWithHWND`; declaring them keeps the slots aligned.
#[windows::core::interface("5752238b-24f0-495a-82f1-2fd593056796")]
unsafe trait IFrameworkInputPane: windows::core::IUnknown {
    fn advise(&self, handler: *mut core::ffi::c_void, cookie: *mut u32) -> HRESULT;
    fn advise_with_hwnd(&self, hwnd: HWND, handler: *mut core::ffi::c_void, cookie: *mut u32)
    -> HRESULT;
    fn unadvise(&self, cookie: u32) -> HRESULT;
    fn location(&self, rect: *mut RECT) -> HRESULT;
    fn location_with_hwnd(&self, hwnd: HWND, rect: *mut RECT) -> HRESULT;
}

const CLSID_FRAMEWORK_INPUT_PANE: GUID = GUID::from_u128(0xd5120aa3_46ba_44c5_822d_ca8092c1fc72);

/// How long an accepted show request counts as "open" when neither visibility
/// API answers (a shell without an input pane or an input view).
const SHOW_GRACE: Duration = Duration::from_secs(4);

/// When a show request was last accepted, in microseconds since `EPOCH`.
static SHOWN_AT: AtomicU64 = AtomicU64::new(0);
static EPOCH: Mutex<Option<Instant>> = Mutex::new(None);

fn mark_shown() {
    let now = Instant::now();
    let mut epoch = EPOCH.lock().unwrap_or_else(|err| err.into_inner());
    let base = *epoch.get_or_insert(now);
    SHOWN_AT.store(now.duration_since(base).as_micros() as u64, Ordering::Relaxed);
}

fn mark_hidden() {
    SHOWN_AT.store(0, Ordering::Relaxed);
}

/// Whether an accepted show request is still in its grace window.
fn recently_shown() -> bool {
    let stamp = SHOWN_AT.load(Ordering::Relaxed);
    if stamp == 0 {
        return false;
    }
    let epoch = EPOCH.lock().unwrap_or_else(|err| err.into_inner());
    let Some(base) = *epoch else {
        return false;
    };
    base + Duration::from_micros(stamp) + SHOW_GRACE > Instant::now()
}

/// WinRT/COM apartment initialization for this thread. `RoInitialize` is
/// already-initialized when COM was set up before and reports
/// `RPC_E_CHANGED_MODE` otherwise; both are fine because the activation
/// factory and `CoCreateInstance` work either way.
fn ensure_apartment() {
    std::thread_local! {
        static INIT: () = {
            let _ = unsafe { RoInitialize(RO_INIT_SINGLETHREADED) };
        };
    }
    INIT.with(|_| ());
}

/// Whether the shell's input pane says a keyboard is on screen. `None` when
/// the input pane object is unavailable.
fn input_pane_shown() -> Option<bool> {
    ensure_apartment();
    let pane = unsafe {
        CoCreateInstance::<_, IFrameworkInputPane>(
            &CLSID_FRAMEWORK_INPUT_PANE,
            None,
            CLSCTX_INPROC_SERVER,
        )
    }
    .ok()?;
    let mut rect = RECT::default();
    let hr = unsafe { pane.location(&mut rect) };
    if hr.is_err() {
        return None;
    }
    Some(rect.right > rect.left && rect.bottom > rect.top)
}

/// The input view of this window, when the process has one (a packaged or
/// hosted window). Used for the gamepad layout and for the documented
/// occlusion test; `None` on an ordinary desktop window.
fn input_view() -> Option<CoreInputView> {
    ensure_apartment();
    match CoreInputView::GetForCurrentView() {
        Ok(view) => Some(view),
        Err(err) => {
            debug!("CoreInputView unavailable: {err}");
            None
        }
    }
}

/// Whether the system keyboard is currently covering the window. The input
/// pane's rectangle is the direct answer; the input view's occlusions cover a
/// hosted window; the grace window after an accepted request covers the shell
/// taking a moment to put its keyboard up (or a shell without either API).
pub fn is_open() -> bool {
    if let Some(shown) = input_pane_shown() {
        return shown;
    }
    if let Some(view) = input_view() {
        if let Ok(occlusions) = view.GetCoreInputViewOcclusions() {
            if let Ok(count) = occlusions.Size() {
                if count > 0 {
                    return true;
                }
            }
        }
    }
    recently_shown()
}

/// Show the system keyboard. Returns whether some path accepted the request;
/// an already-open keyboard is left alone. `layout` only selects the first
/// attempt — a touch keyboard is a working answer to a gamepad request too.
pub fn show(layout: Layout) -> bool {
    if is_open() {
        mark_shown();
        return true;
    }
    if let Some(view) = input_view() {
        let kinds: &[CoreInputViewKind] = match layout {
            Layout::Gamepad => &[CoreInputViewKind::Gamepad, CoreInputViewKind::Keyboard],
            Layout::Touch => &[CoreInputViewKind::Keyboard],
        };
        for kind in kinds {
            match view.TryShowWithKind(*kind) {
                Ok(true) => {
                    debug!("system keyboard shown with kind {kind:?}");
                    mark_shown();
                    return true;
                }
                Ok(false) => debug!("system keyboard kind {kind:?} unavailable"),
                Err(err) => debug!("TryShowWithKind({kind:?}) failed: {err}"),
            }
        }
    }
    show_touch()
}

fn show_touch() -> bool {
    ensure_apartment();
    if let Ok(host) =
        unsafe { CoCreateInstance::<_, ITipInvocation>(&CLSID_UIHOST_NO_LAUNCH, None, CLSCTX_ALL) }
    {
        let result = unsafe { host.toggle(GetDesktopWindow()) };
        if result.is_ok() {
            debug!("system keyboard shown via ITipInvocation");
            mark_shown();
            return true;
        }
        warn!("ITipInvocation.Toggle failed: 0x{:08X}", result.0 as u32);
    } else {
        debug!("UIHostNoLaunch is not registered (the keyboard never started?)");
    }
    let launched = launch_process();
    debug!("system keyboard process launch: {launched}");
    if launched {
        mark_shown();
    }
    launched
}

fn launch_process() -> bool {
    let tabtip =
        std::path::Path::new(r"C:\Program Files\Common Files\microsoft shared\ink\TabTip.exe");
    if tabtip.exists() && std::process::Command::new(tabtip).spawn().is_ok() {
        return true;
    }
    std::process::Command::new("osk.exe").spawn().is_ok()
}

/// Dismiss the system keyboard. A measured-visible keyboard is closed with the
/// shell's toggle (the one call that reliably takes down what the shell put
/// up); the input view is the fallback when the toggle is unavailable.
pub fn hide() -> bool {
    mark_hidden();
    if input_pane_shown() == Some(true) {
        ensure_apartment();
        if let Ok(host) = unsafe {
            CoCreateInstance::<_, ITipInvocation>(&CLSID_UIHOST_NO_LAUNCH, None, CLSCTX_ALL)
        } {
            let result = unsafe { host.toggle(GetDesktopWindow()) };
            if result.is_ok() {
                debug!("system keyboard hidden via ITipInvocation");
                return true;
            }
            warn!("ITipInvocation.Toggle failed: 0x{:08X}", result.0 as u32);
        }
    }
    // The input view can report success without closing a keyboard another
    // caller opened; it is the last resort, not the first.
    if let Some(view) = input_view() {
        match view.TryHide() {
            Ok(true) => return true,
            Ok(false) => {}
            Err(err) => debug!("TryHide failed: {err}"),
        }
        if let Ok(true) = view.TryHidePrimaryView() {
            return true;
        }
    }
    false
}
