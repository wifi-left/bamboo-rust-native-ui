# Host and window

> The Windows-only application side: `Host` callbacks, the paint context, and the `WindowBuilder` / `WindowHandle` API.

## Overview

Everything here is Windows-only: `host` and `platform` are `#[cfg(windows)]`
modules, and `Host`, `PaintCtx`, `CloseReason`, `WindowVisualState`,
`WindowBuilder` and `WindowHandle` are re-exported from the crate root only on
Windows. The widget, painter and event layers stay platform-neutral; Win32 is
confined to the backend and this host layer.

The application implements `Host` and never sees a `WM_*` message. The window
turns messages into normalized `Event`s and lifecycle callbacks, calls
`Host::paint` with a borrowed dirty-rect list, and hands every callback a
`WindowHandle` — which can invalidate, open menus and dialogs, adjust the tray
icon or start a drag. `WindowHandle` implements `Invalidator` and `UiHost`, so
`&mut window` is exactly the sink the `Ui` methods expect. One persistent back
buffer exists per window and only the dirty bounding box is blitted, so idle
means zero paints.

## Key types

### `PaintCtx`

```rust
pub struct PaintCtx<'a> {
    pub painter: &'a Painter,
    pub canvas: &'a mut dyn Canvas,
    /// Merged dirty rectangles in DIPs.
    pub dirty: &'a [Rect],
    pub size: Size,
    pub dpi: f32,
    /// Widget whose native edit child is open; its closed-field caret is suppressed.
    pub editing: Option<WidgetId>,
    /// Caret blink phase for framework-painted editors.
    pub caret_on: bool,
    /// Only the overlay layer changed; paint widgets from the backdrop on.
    pub overlay: bool,
    /// This frame blends the backdrop over the cached page.
    pub overlay_first: bool,
    /// Output: the focused painted editor's caret rect, filled by `Ui::paint`.
    pub caret: Option<Rect>,
}

impl PaintCtx<'_> {
    pub fn scale(&self) -> f32;      // dpi / 96.0
    pub fn hits(&self, rect: Rect) -> bool;
    pub fn full(&self) -> Rect;      // Rect::from_size(self.size)
}
```

`hits` is the cheap "does this widget need painting" check; `full` is the client
area. `overlay_first` is true only for the frame that blends the backdrop over
the cached page, so later overlay frames leave the dimmed page alone.

### `CloseReason` and `WindowVisualState`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// Title-bar close, Alt+F4, the system menu, Task Manager's "End task".
    User,
    /// The system is logging off or shutting down.
    System,
    /// `WindowHandle::request_close`.
    Programmatic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowVisualState { #[default] Normal, Minimized, Maximized }
```

### `Renderer`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Renderer { #[default] Gdi, D2dSoftware, D2dHardware }
```

`Renderer::parse` maps `gdi`/`d2d-software`/`d2d-hardware` (and a few aliases);
`from_env` reads `BAMBOO_UI_RENDERER`; `name` returns the selector string.

## `Host` hooks

`paint` is the only required method; every other hook has a default that does
nothing (or vetoes nothing).

### Painting, timing and layout

| Hook | Signature and notes |
|---|---|
| `paint` | `fn paint(&mut self, ctx: &mut PaintCtx<'_>)` — only rectangles in `ctx.dirty` are visible. |
| `overlay_paint` | `fn overlay_paint(&self) -> bool { false }` — `true` keeps the cached page and composites only the overlay. |
| `timer` | `fn timer(&mut self, window: &mut WindowHandle<'_>)` — animation/heartbeat tick. |
| `resize` | `fn resize(&mut self, size: Size, dpi: f32, window: &mut WindowHandle<'_>)` — the only place layout may be recomputed. |
| `dpi_changed` | `fn dpi_changed(&mut self, dpi: f32, window: &mut WindowHandle<'_>)` — font caches are already dropped. |

### Lifecycle

| Hook | Signature and notes |
|---|---|
| `started` | `fn started(&mut self, window: &mut WindowHandle<'_>)` — created and visible; install timers, request focus. |
| `loaded` | `fn loaded(&mut self, window: &mut WindowHandle<'_>)` — first frame is on screen. |
| `visibility_changed` | `fn visibility_changed(&mut self, visible: bool, window: &mut WindowHandle<'_>)` — `WM_SHOWWINDOW`, after `started`. |
| `activated` | `fn activated(&mut self, active: bool, window: &mut WindowHandle<'_>)` — foreground/inactive. |
| `visual_state_changed` | `fn visual_state_changed(&mut self, state: WindowVisualState, window: &mut WindowHandle<'_>)` — real minimize/maximize/restore. |
| `moved` | `fn moved(&mut self, position: Point, window: &mut WindowHandle<'_>)` — frame top-left in screen DIPs. |
| `close_requested` | `fn close_requested(&mut self, reason: CloseReason, window: &mut WindowHandle<'_>) -> bool { true }` — `false` vetoes the close. |
| `destroying`, `destroy` | `fn destroying(&mut self, window: &mut WindowHandle<'_>)`; `fn destroy(&mut self) {}`. |

### Input, text, drag and shell

| Hook | Signature and notes |
|---|---|
| `event` | `fn event(&mut self, event: &Event, window: &mut WindowHandle<'_>) -> bool { false }` — every normalized input; `true` when consumed. |
| `edit_changed` | `fn edit_changed(&mut self, id: WidgetId, text: &str, window: &mut WindowHandle<'_>)` — native editor text changed while typing. |
| `edit_committed` | `fn edit_committed(&mut self, id: WidgetId, text: &str, window: &mut WindowHandle<'_>)` — forward to `Ui::commit_edit`. |
| `edit_canceled` | `fn edit_canceled(&mut self, id: WidgetId, window: &mut WindowHandle<'_>)` — closed without committing. |
| `ime_compose` | `fn ime_compose(&mut self, text: &str, cursor: usize, window: &mut WindowHandle<'_>)` — forward to `Ui::ime_compose`. |
| `ime_commit` | `fn ime_commit(&mut self, text: &str, window: &mut WindowHandle<'_>)` — forward to `Ui::ime_commit`. |
| `files_dropped` | `fn files_dropped(&mut self, paths: Vec<String>, window: &mut WindowHandle<'_>)`. |
| `drag_enter` | `fn drag_enter(&mut self, data: &DragData, pos: Point, window: &mut WindowHandle<'_>) -> DropEffect`. |
| `drag_over` | `fn drag_over(&mut self, data: &DragData, pos: Point, window: &mut WindowHandle<'_>) -> DropEffect`. |
| `drag_leave` | `fn drag_leave(&mut self, window: &mut WindowHandle<'_>)`. |
| `drag_drop` | `fn drag_drop(&mut self, data: DragData, pos: Point, window: &mut WindowHandle<'_>) -> DropEffect`. |
| `drag_finished` | `fn drag_finished(&mut self, effect: DropEffect, window: &mut WindowHandle<'_>)`. |
| `menu_event` | `fn menu_event(&mut self, event: &mut MenuEvent<'_>, window: &mut WindowHandle<'_>) -> MenuDecision` — `Handled` cancels/consumes. |
| `menu_command` | `fn menu_command(&mut self, widget: WidgetId, command: u32, window: &mut WindowHandle<'_>)`. |
| `menu_bar_command` | `fn menu_bar_command(&mut self, command: u32, window: &mut WindowHandle<'_>)`. |
| `dialog_finished` | `fn dialog_finished(&mut self, id: DialogId, result: DialogResult, window: &mut WindowHandle<'_>)`. |
| `tray_event` | `fn tray_event(&mut self, event: TrayEvent, window: &mut WindowHandle<'_>)`. |
| `hotkey` | `fn hotkey(&mut self, hotkey: Hotkey, window: &mut WindowHandle<'_>)`. |
| `hotkey_captured` | `fn hotkey_captured(&mut self, hotkey: Option<Hotkey>, window: &mut WindowHandle<'_>)`. |
| `a11y_nodes` | `fn a11y_nodes(&mut self) -> Vec<A11yNode>` — default exposes nothing. |
| `a11y_action` | `fn a11y_action(&mut self, action: A11yAction, window: &mut WindowHandle<'_>)` — apply via `Ui::apply_a11y`. |

## `WindowBuilder`

```rust
impl WindowBuilder {
    pub fn new(host: impl Host + 'static) -> Self;
    pub fn title(self, title: impl Into<String>) -> Self;
    pub fn size(self, width: f32, height: f32) -> Self;
    pub fn min_size(self, width: f32, height: f32) -> Self;
    pub fn theme(self, settings: ThemeSettings, dark: bool) -> Self;
    pub fn renderer(self, renderer: Renderer) -> Self;
    pub fn menu_bar(self, menu_bar: MenuBarModel) -> Self;
    // Also: owner, app_id, window_hotkey, window_hotkeys.
    /// Create the window, run the message loop, return the exit code.
    pub fn run(self) -> windows::core::Result<i32>;
}
```

Defaults set by `new`: title `"Bamboo Native UI"`, size 900 × 620 DIPs, minimum
320 × 240, `ThemeSettings::default()`, dark mode `true`, no menu bar, no
renderer override. `menu_bar` installs the system-drawn window bar (commands
arrive through `Host::menu_bar_command` after the `MenuEvent` hook), `owner`
makes an owned dialog centered on another `HWND`, and `window_hotkey` registers
a chord that fires `Host::menu_bar_command` while the window has focus.

### Renderer selection

1. `WindowBuilder::renderer(...)`, when called.
2. Otherwise `BAMBOO_UI_RENDERER` (`gdi`, `d2d-software`, `d2d-hardware`).
3. Otherwise `Renderer::Gdi`, the default.

All modes draw into the same persistent back buffer and blit the dirty box. A
Direct2D mode draws through `ID2D1DCRenderTarget` — hardware when the driver
offers a target, Direct2D's software rasterizer otherwise — and a mode that
cannot start logs under `BAMBOO_UI_DEBUG` and switches to GDI permanently.
`WindowHandle::renderer` reports the active one. The GDI/D2D backends need
`feature = "gdi"` on Windows (the default feature).

## `WindowHandle`

`WindowHandle<'_>` implements `Invalidator` and `UiHost`, so `&mut window` is
the sink for `Ui` calls. Frequently used operations:

| Area | Methods |
|---|---|
| Painting | `invalidate`, `invalidate_all`, `invalidate_force`, `invalidate_all_force`, `flush`, `invalidations`, `resync_overlay` |
| Geometry | `size`, `dpi`, `scale`, `frame_position`, `renderer`, `set_cursor`, `set_animation_timer` |
| Lifecycle | `request_close`, `is_visible`, `is_active`, `visual_state`, `raise`, `hide`, `minimize`, `maximize`, `restore` |
| Editing / a11y | `end_edit`, `a11y_refresh`, `a11y_focus`, `a11y_announce`, `a11y_announce_with` |
| Menus | `open_menu`, `menu_click`, `set_menu_bar`, `menu_bar_installed`, `edit_menu_bar`, `dismiss_menu`, `is_menu_open`, `active_menu_widget` |
| Drag | `start_drag`, `set_system_drag_enabled`, `system_drag_enabled` |
| Shell | `show_dialog`, `show_color_window`, `set_tray_icon`, `notify`, `show_tray_balloon`, `copy_text` |
| Hotkeys and misc | `register_hotkey`, `unregister_hotkey`, `register_window_hotkey`, `clear_window_hotkeys`, `begin_hotkey_capture`, `cancel_hotkey_capture`, `tooltip_visible`, `is_foreground`, `hwnd` |

## Example

```rust
use bamboo_native_ui::widgets::Label;
use bamboo_native_ui::{
    Event, Host, PaintCtx, Rect, Size, Ui, WidgetId, WindowBuilder, WindowHandle,
};

struct App { ui: Ui<Msg> }

enum Msg { Quit }

impl Host for App {
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        let full = ctx.full();
        let background = ctx.painter.palette().background;
        ctx.painter.fill(ctx.canvas, full, background);
        self.ui.paint(ctx);
    }

    fn resize(&mut self, size: Size, _dpi: f32, _window: &mut WindowHandle<'_>) {
        if self.ui.is_empty() {
            self.ui.add(Label::new("title", "Hello, Bamboo"));
        }
        let rect = Rect::from_xywh(16.0, 16.0, size.width - 32.0, 28.0);
        self.ui.set_rect(WidgetId::new("title"), rect);
    }

    fn event(&mut self, event: &Event, window: &mut WindowHandle<'_>) -> bool {
        match *event {
            Event::PointerDown { pos, button, modifiers } => {
                self.ui.pointer_down_with(pos, button, modifiers, window)
            }
            Event::PointerUp { pos, button, modifiers } => {
                self.ui.pointer_up_with(pos, button, modifiers, window)
            }
            _ => false,
        }
    }
}

fn main() -> windows::core::Result<()> {
    let _exit_code = WindowBuilder::new(App { ui: Ui::new() })
        .title("Hello")
        .size(640.0, 360.0)
        .run()?;
    Ok(())
}
```

## Notes and limits

- Windows-only: `Host`, `PaintCtx`, `WindowBuilder`, `WindowHandle`,
  `CloseReason` and `WindowVisualState` do not exist in a non-Windows build;
  the portable part (`Ui`, `Widget`, `Painter`, `Canvas`, `Event`) does.
- `paint` may draw only inside `ctx.dirty`; use `ctx.hits(rect)` to skip
  widgets cheaply.
- Do not run a modal loop inside `Host::event`: menus and dialogs are deferred,
  the framework shows them after the callback returns and reports the result
  through `menu_command`, `menu_event` or `dialog_finished`.
- Closing is a request: returning `false` from `close_requested` keeps the
  window open and suppresses the destroy callbacks.
- `WindowBuilder::renderer` wins over `BAMBOO_UI_RENDERER`; invalid values are
  ignored (`from_env` returns `None`). `run` blocks until the message loop
  exits and returns the exit code as a `windows::core::Result<i32>`. A
  `WindowHandle` borrow is valid only inside the callback that received it.

## See also

- [Events and input](./events-and-input.md)
- [The Ui container](./ui-container.md)
- [Painting and Canvas](./painting-and-canvas.md)
- [Tray icons and notifications](./tray-and-notifications.md)
- [Button](../widgets/controls/button.md)
- [First window quickstart](../quickstart/03-first-window.md)
