// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Bamboo Native UI — a small, declarative native Windows UI framework.
//!
//! Design contract (see `docs/DECISIONS.md`):
//!
//! - The application computes widget rectangles; the framework only paints the
//!   dirty rectangles and routes normalized events. State changes never trigger
//!   a relayout or a full-window repaint.
//! - Public types are platform-neutral. The GDI backend lives behind
//!   `#[cfg(windows)]` and is the only place that touches Win32.
//! - One persistent back buffer per window; only the dirty bounding box is
//!   blitted. Idle means zero paints.
//! - Paint-path allocations are forbidden: dirty rectangles are borrowed, text
//!   is borrowed, caches live in the backend for the lifetime of the window.

pub mod a11y;
pub mod backend;
pub mod color;
pub mod dialog;
pub mod drag;
pub mod event;
pub mod gamepad;
pub use bamboo_native_ui_layout_core::geometry;
pub mod hotkey;
pub mod image;
pub mod layout;
/// Read-only debugging snapshots (feature `debug-form`); the application
/// decides how to present them.
#[cfg(feature = "debug-form")]
pub mod debug;
/// Document-created widgets (M7.7): the type registry a layout document uses
/// to create widgets it addresses. Always compiled; creation itself needs a
/// layout feature.
pub mod creation;
/// M7 layout skins (`.buixml`, XML syntax): model, applier and — with
/// `layout-compiled` — the runtime that generated Rust installs through.
#[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
pub mod layout_xml;
pub mod menu;
pub mod mnemonic;
pub mod notification;
pub mod painter;
pub mod props;
pub mod scroll;
/// The widget-declared skin protocol (M7 §6.2–6.4). Always compiled: a
/// third-party widget can declare support without `layout-xml`.
pub mod skin;
pub mod surface;
pub mod text;
pub mod theme;
/// M7.8 theme documents (`.buitheme`, XML syntax): the reader re-export and
/// the mapping onto [`theme::Palette`].
#[cfg(feature = "theme-xml")]
pub mod theme_xml;
pub mod tray;
pub mod ui;
pub mod widget;
pub mod widgets;

#[cfg(windows)]
pub mod host;
#[cfg(windows)]
pub mod platform;

#[cfg(windows)]
pub mod diagnostics;

pub use a11y::{
    A11yAction, A11yActionKind, A11yChild, A11yInfo, A11yNode, A11yRole, A11yScroll,
    A11yScrollAxis, AnnouncementPriority,
};
pub use backend::{
    Canvas, FontFace, FontWeight, ImageData, ImageId, Renderer, TextAlign, TextStyle, TextOverflow,
};
pub use color::{Color, Rgba};
pub use dialog::{
    ColorDialog, ColorWindow, Dialog, DialogId, DialogResult, FileDialog, FileDialogMode, FileFilter,
    FontChoice, FontDialog, MessageAnswer, MessageButtons, MessageDefault, MessageDialog,
    MessageIcon, custom_colors, set_custom_colors,
};
pub use drag::{DragData, DropEffect};
pub use event::{Cursor, Event, KeyCode, KeyEvent, MouseButton, PointerKind, PointerModifiers};
pub use gamepad::{Gamepad, PadButtons, PadState, read_state as read_gamepad};
pub use geometry::{Corners, Insets, Point, Rect, Size};
pub use hotkey::{Hotkey, HotkeyModifiers};
pub use image::{ImageStore, UiImage};
pub use menu::{
    Menu, MenuBarEntry, MenuBarModel, MenuDecision, MenuEntry, MenuEvent, MenuIcon, MenuItem,
    MenuPath, MenuSource, MenuStep, Submenu,
};
pub use mnemonic::MnemonicLabel;
pub use notification::{Notification, NotificationResult};
pub use painter::Painter;
pub use props::{ContentCaps, FontSpec, Narration, WidgetProps};
pub use scroll::{ScrollChild, ScrollView};
pub use skin::{SkinAttrDecl, SkinElement, SkinValue, SkinValueKind};
pub use surface::SurfaceStyle;
pub use theme::{ACCENT_NAMES, ACCENT_PRESETS, ListDensity, Palette, ThemeSettings};
pub use tray::{TrayAction, TrayEvent, TrayIcon, TrayImage};
pub use ui::{
    ContextMenuRequest, FocusEvent, FocusOutcome, FocusReason, Invalidator, TooltipAnchor,
    TooltipRequest, Trigger, TriggerContext, TriggerEvent, Ui, UiHost,
};
pub use widgets::CustomWidget;
pub use widgets::observers::Observers;
pub use widget::{
    EditRequest, NativeMenuRequest, Row, ScrollBars, SessionSnapshot, TextStyleRole, Widget,
    WidgetId, WidgetInput, WidgetPaint, WidgetState,
};

#[cfg(windows)]
pub use host::{CloseReason, Host, PaintCtx, WindowVisualState};
#[cfg(windows)]
pub use platform::keyboard as soft_keyboard;
#[cfg(windows)]
pub use platform::{WindowBuilder, WindowHandle};
