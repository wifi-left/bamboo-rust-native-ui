// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Window host contract.
//!
//! The application implements [`Host`] and never sees `WM_*` messages. The
//! host returns normalized events; painting receives a borrowed dirty-rect
//! list and must only draw intersecting widgets.

use crate::a11y::{A11yAction, A11yNode};
use crate::backend::Canvas;
use crate::drag::{DragData, DropEffect};
use crate::event::Event;
use crate::geometry::{Point, Rect, Size};
use crate::painter::Painter;
use crate::platform::WindowHandle;
use crate::widget::WidgetId;

/// Why a close request arrived (`Host::close_requested`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// The user asked: the title-bar close button, Alt+F4, the system menu,
    /// Task Manager's "End task" (a bare `WM_CLOSE`).
    User,
    /// The system is logging off or shutting down (`WM_QUERYENDSESSION`).
    System,
    /// The application asked with [`crate::WindowHandle::request_close`].
    Programmatic,
}

/// The window's visual state as the shell reports it (`WM_SIZE`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowVisualState {
    #[default]
    Normal,
    Minimized,
    Maximized,
}

/// One paint pass.
pub struct PaintCtx<'a> {
    pub painter: &'a Painter,
    pub canvas: &'a mut dyn Canvas,
    /// Merged dirty rectangles in DIPs. Draw only widgets that intersect them.
    pub dirty: &'a [Rect],
    pub size: Size,
    pub dpi: f32,
    /// Widget whose native edit child is currently open. Its closed-field
    /// caret/selection are suppressed (the child paints its own).
    pub editing: Option<WidgetId>,
    /// Caret blink phase for framework-painted editors. The host toggles it
    /// on its blink timer and repaints the caret rectangle.
    pub caret_on: bool,
    /// The back buffer already holds the page and only the overlay layer
    /// changed (a modal scrim is up). [`crate::Ui::paint`] then paints only
    /// widgets from the backdrop on, skips the scroll-view bars, and the
    /// application must skip its own page background/sidebar fills.
    pub overlay: bool,
    /// The frame that blends the backdrop over the cached page. Later overlay
    /// frames leave the already-dimmed page alone.
    pub overlay_first: bool,
    /// Output: the focused inline editor's caret rectangle, filled by
    /// [`crate::Ui::paint`] after the widgets painted. The host stores it for
    /// IME candidate placement and the blink timer.
    pub caret: Option<Rect>,
}

impl PaintCtx<'_> {
    pub fn scale(&self) -> f32 {
        self.dpi / 96.0
    }

    /// Whether `rect` intersects any dirty rectangle.
    pub fn hits(&self, rect: Rect) -> bool {
        self.dirty.iter().any(|dirty| dirty.intersects(rect))
    }

    pub fn full(&self) -> Rect {
        Rect::from_size(self.size)
    }
}

/// The application side of a window.
pub trait Host {
    /// Draw. Only widgets intersecting `ctx.dirty` need to be painted.
    fn paint(&mut self, ctx: &mut PaintCtx<'_>);

    /// Whether a full-window backdrop (a modal scrim) owns the page this
    /// frame. When true the window keeps the page pixels already in its back
    /// buffer, forces a full-window overlay composite and sets
    /// [`PaintCtx::overlay`]: the host skips its page background/sidebar and
    /// [`crate::Ui::paint`] paints only the overlay layer. Closing the overlay
    /// (returning `false`) repaints the page normally.
    fn overlay_paint(&self) -> bool {
        false
    }

    /// Handle one normalized event. Return `true` when consumed.
    fn event(&mut self, event: &Event, window: &mut WindowHandle<'_>) -> bool {
        let _ = (event, window);
        false
    }

    /// Animation / heartbeat tick (installed by `set_animation_timer`).
    fn timer(&mut self, window: &mut WindowHandle<'_>) {
        let _ = window;
    }

    /// Client size changed (DIPs). Recompute layout and invalidate everything
    /// once; this is the only place layout may be recomputed.
    fn resize(&mut self, size: Size, dpi: f32, window: &mut WindowHandle<'_>) {
        let _ = (size, dpi, window);
    }

    /// DPI changed. Font caches are already dropped by the framework.
    fn dpi_changed(&mut self, dpi: f32, window: &mut WindowHandle<'_>) {
        let _ = (dpi, window);
    }

    /// The window is created and visible. Install timers, request initial
    /// focus, and invalidate once if needed.
    fn started(&mut self, window: &mut WindowHandle<'_>) {
        let _ = window;
    }

    /// A close request. Return `false` to veto it (e.g. hide to the tray
    /// instead of exiting); the window stays open and no destroy callback
    /// follows. `reason` separates a user request (title-bar close, Alt+F4,
    /// Task Manager), a system shutdown/logoff, and
    /// [`crate::WindowHandle::request_close`].
    fn close_requested(&mut self, reason: CloseReason, window: &mut WindowHandle<'_>) -> bool {
        let _ = (reason, window);
        true
    }

    /// The window finished its first paint after startup (the first frame is
    /// on screen). Use it to dismiss a splash screen or start work that needs
    /// a painted window.
    fn loaded(&mut self, window: &mut WindowHandle<'_>) {
        let _ = window;
    }

    /// The window was shown or hidden (`WM_SHOWWINDOW`), after `started`.
    fn visibility_changed(&mut self, visible: bool, window: &mut WindowHandle<'_>) {
        let _ = (visible, window);
    }

    /// The window became active or inactive (`WM_ACTIVATE`). Activation is
    /// the foreground window state; per-widget focus is not involved.
    fn activated(&mut self, active: bool, window: &mut WindowHandle<'_>) {
        let _ = (active, window);
    }

    /// The window was minimized, maximized or restored. Fires only on a real
    /// state change (`IsIconic`/`IsZoomed`), not on every resize.
    fn visual_state_changed(
        &mut self,
        state: WindowVisualState,
        window: &mut WindowHandle<'_>,
    ) {
        let _ = (state, window);
    }

    /// The window was moved. `position` is the frame's top-left corner in
    /// screen DIPs (persist it and restore it with the window placement).
    fn moved(&mut self, position: Point, window: &mut WindowHandle<'_>) {
        let _ = (position, window);
    }

    /// Files/folders were dropped on the window from the shell (`WM_DROPFILES`).
    fn files_dropped(&mut self, paths: Vec<String>, window: &mut WindowHandle<'_>) {
        let _ = (paths, window);
    }

    /// A system drag (from this window or another application) entered the
    /// window's client area. `pos` is in client DIPs. Return the effect the
    /// target offers: `None` refuses the drop (the shell shows a "no" cursor).
    /// `Ui::drag_enter` routes it to the widget under the pointer and is the
    /// usual implementation.
    fn drag_enter(
        &mut self,
        data: &DragData,
        pos: Point,
        window: &mut WindowHandle<'_>,
    ) -> DropEffect {
        let _ = (data, pos, window);
        DropEffect::None
    }

    /// The pointer moved while a system drag is over the window. Return the
    /// effect for the widget now under `pos`; called continuously, so it must
    /// only re-damage widgets whose hover state actually changed.
    fn drag_over(
        &mut self,
        data: &DragData,
        pos: Point,
        window: &mut WindowHandle<'_>,
    ) -> DropEffect {
        let _ = (data, pos, window);
        DropEffect::None
    }

    /// The system drag left the window (cancelled, dropped elsewhere or moved
    /// to another window). Clear hover feedback.
    fn drag_leave(&mut self, window: &mut WindowHandle<'_>) {
        let _ = window;
    }

    /// The user released a system drag over the window. Consume the data and
    /// return the effect that was applied (`Copy`, `Move` or `None`).
    fn drag_drop(
        &mut self,
        data: DragData,
        pos: Point,
        window: &mut WindowHandle<'_>,
    ) -> DropEffect {
        let _ = (data, pos, window);
        DropEffect::None
    }

    /// A drag this window started finished (dropped or cancelled). `effect` is
    /// what the receiving target reported. Clear the source widget's drag
    /// state; the pointer release was consumed by OLE, so no `PointerUp`
    /// follows.
    fn drag_finished(&mut self, effect: DropEffect, window: &mut WindowHandle<'_>) {
        let _ = (effect, window);
    }

    /// The window is being destroyed (`WM_DESTROY`): the user closed it and
    /// `close_requested` allowed it, or the system is ending the session. The
    /// handle is still valid (you may read sizes/flags), but any painting
    /// would be discarded.
    fn destroying(&mut self, window: &mut WindowHandle<'_>) {
        let _ = window;
    }

    /// Native editor text changed while the user types (every keystroke).
    fn edit_changed(&mut self, id: WidgetId, text: &str, window: &mut WindowHandle<'_>) {
        let _ = (id, text, window);
    }

    /// The native editor closed after losing focus, Enter or Tab. `text` is
    /// the control contents; the application usually forwards it to
    /// [`crate::Ui::commit_edit`].
    fn edit_committed(&mut self, id: WidgetId, text: &str, window: &mut WindowHandle<'_>) {
        let _ = (id, text, window);
    }

    /// The native editor closed without committing (Escape).
    fn edit_canceled(&mut self, id: WidgetId, window: &mut WindowHandle<'_>) {
        let _ = (id, window);
    }

    /// IME composition string changed. `text` is the uncommitted composition
    /// (empty when it ended); `cursor` is the composition cursor in
    /// characters. Framework-painted editors draw it inline; forward it with
    /// [`crate::Ui::ime_compose`]. The host hides the system composition
    /// window, so an application that ignores this callback would show no
    /// composition at all.
    fn ime_compose(&mut self, text: &str, cursor: usize, window: &mut WindowHandle<'_>) {
        let _ = (text, cursor, window);
    }

    /// IME committed a result string (a converted candidate, a kana run, a
    /// Han character). Forward it with [`crate::Ui::ime_commit`].
    fn ime_commit(&mut self, text: &str, window: &mut WindowHandle<'_>) {
        let _ = (text, window);
    }

    /// Accessibility tree snapshot, rebuilt when a screen reader asks for the
    /// window's provider. Default: nothing is exposed.
    fn a11y_nodes(&mut self) -> Vec<A11yNode> {
        Vec::new()
    }

    /// A screen reader invoked/toggled/selected/set a value. Apply it through
    /// [`crate::Ui::apply_a11y`] and handle the returned message.
    fn a11y_action(&mut self, action: A11yAction, window: &mut WindowHandle<'_>) {
        let _ = (action, window);
    }

    /// The user picked a context-menu command. `widget` is the widget whose
    /// menu was open, `command` the id from its [`crate::Menu`]. Called after
    /// the popup has closed, never inside its modal loop, so it is safe to
    /// touch the UI, open dialogs or invalidate. `command` is nonzero; a menu
    /// dismissed without a selection does not call this.
    ///
    /// This is the replay step; [`Host::menu_event`] observes every menu
    /// before the command reaches it. Return nothing — a command consumed by
    /// the hook never arrives here.
    fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>) {
        let _ = (widget, command, window);
    }

    /// Every native menu's lifecycle, for all menu paths: context menus,
    /// chrome widgets (menu bar, menu button, split button, toolbar overflow),
    /// the tray menu and programmatic opens (`Ui::open_menu`,
    /// `WindowHandle::open_menu`). Runs on the UI thread outside every other
    /// callback.
    ///
    /// Return [`MenuDecision::Handled`] to cancel an `Opening` event (the
    /// popup is not shown) or to consume a `Selected` command (its replay —
    /// `menu_command` or `tray_event` — is skipped). An `Opening` event hands
    /// out the menu model mutably, so a hook can rebuild it from live state
    /// ("rebuild before show") or install a per-user command set right before
    /// the popup appears. `Selected` hands out the command mutably for menu
    /// aliases. This is the one place to log, rewrite or route menu ids; the
    /// default passes everything through.
    fn menu_event(
        &mut self,
        event: &mut crate::menu::MenuEvent<'_>,
        window: &mut WindowHandle<'_>,
    ) -> crate::menu::MenuDecision {
        let _ = (event, window);
        crate::menu::MenuDecision::Pass
    }

    /// A command from the native window menu bar (`WindowBuilder::menu_bar` /
    /// `WindowHandle::set_menu_bar`). Runs on the UI thread outside every
    /// other callback. The [`crate::menu::MenuEvent`] hook sees the selection
    /// first with source `MenuBar`; a command it consumes never arrives here.
    fn menu_bar_command(&mut self, command: u32, window: &mut WindowHandle<'_>) {
        let _ = (command, window);
    }

    /// A native dialog opened with [`crate::WindowHandle::show_dialog`] closed.
    /// `id` is the handle returned by that call; `result` carries the chosen
    /// value (message answer, path, font or color) or `None` when dismissed.
    /// The callback runs on the UI thread after the dialog's thread is gone,
    /// so the UI is safe to update; a modal dialog has already re-enabled the
    /// window.
    fn dialog_finished(
        &mut self,
        id: crate::dialog::DialogId,
        result: crate::dialog::DialogResult,
        window: &mut WindowHandle<'_>,
    ) {
        let _ = (id, result, window);
    }

    /// The tray icon was activated (left click, double click or keyboard
    /// select) or one of its menu commands was chosen. Runs on the UI thread
    /// outside every other callback, so showing or focusing the window is
    /// safe. A modal dialog opened from here would be modal to this window.
    fn tray_event(&mut self, event: crate::tray::TrayEvent, window: &mut WindowHandle<'_>) {
        let _ = (event, window);
    }

    /// A registered global hotkey fired. Runs on the UI thread, outside every
    /// other callback, while the window exists.
    fn hotkey(&mut self, hotkey: crate::hotkey::Hotkey, window: &mut WindowHandle<'_>) {
        let _ = (hotkey, window);
    }

    /// Shortcut capture finished. `Some` is the chord the user pressed (with a
    /// modifier or a function key); `None` means it was canceled with Esc, by
    /// focus loss or through `WindowHandle::cancel_hotkey_capture`. The key is
    /// not registered automatically.
    fn hotkey_captured(
        &mut self,
        hotkey: Option<crate::hotkey::Hotkey>,
        window: &mut WindowHandle<'_>,
    ) {
        let _ = (hotkey, window);
    }

    /// The window is being destroyed; release resources.
    fn destroy(&mut self) {}
}
