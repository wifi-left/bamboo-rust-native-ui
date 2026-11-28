# The Ui container

> The retained widget tree that owns hit-testing, focus, input routing, damage tracking and painting.

## Overview

`Ui<M>` is the application-facing container: it owns every widget of one page
as `Box<dyn Widget<M>>` in paint and hit-test order, and it — not the widgets —
owns hover, press, focus, drag and scroll state. Widgets are added once per
page rebuild and persist until the next one; state changes update them in
place.

The container never computes layout. The application assigns rectangles with
`set_rect` (or `layout_vertical`), and a state change only invalidates the
affected rectangles. Messages arrive either from input routing (queued
internally, drained with `drain_messages`) or as the return value of the method
that caused them.

Most methods take a sink: `&mut impl Invalidator` accepts dirty rectangles
(`WindowHandle` implements it), and `&mut impl UiHost` adds the native-editor,
tooltip, clipboard, context-menu and drag hooks. Every `UiHost` method except
`invalidate` has a no-op default, so a test sink is `impl UiHost for Sink {}`.
`Ui` is platform-neutral; `UiHost`, `WindowHandle` and the window are
Windows-only. `paint` needs a `PaintCtx` (see
[Host and window](./host-and-window.md)).

## Key types

### `Ui<M>`, `Invalidator`, `UiHost`

```rust
pub struct Ui<M> { /* private */ }

impl<M> Ui<M> { pub fn new() -> Self; }
impl<M> Default for Ui<M> { /* Ui::new() */ }

pub trait Invalidator {
    fn invalidate(&mut self, rect: Rect);
}

/// Sink for native editors, tooltips, clipboard and menus; all methods except
/// `invalidate` have defaults.
pub trait UiHost: Invalidator {
    fn begin_edit(&mut self, request: &EditRequest<'_>);
    fn end_edit(&mut self);
    fn move_edit(&mut self, rect: Rect);
    fn editing_id(&self) -> Option<WidgetId>;
    fn set_tooltip(&mut self, target: Option<TooltipRequest<'_>>);
    fn show_context_menu(&mut self, request: ContextMenuRequest<'_>);
    fn clipboard_text(&mut self) -> Option<String>;
    fn copy_to_clipboard(&mut self, text: &str);
    fn set_caret_rect(&mut self, rect: Option<Rect>);
    // Also: set_soft_keyboard, soft_keyboard_open, set_touch_keyboard, start_drag.
}
```

### `Trigger` and `TriggerEvent`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Click, DoubleClick, RightClick, HoverEnter, HoverLeave, Wheel, Activate,
    DragStart, DragEnd, ValueChange,
}

#[derive(Clone, Copy, Debug)]
pub struct TriggerEvent {
    pub trigger: Trigger,
    pub pos: Option<Point>,
    pub wheel_delta: f32,
}

pub struct TriggerContext<'a, M = ()> { /* private */ }

impl<M> TriggerContext<'_, M> {
    /// Run the widget's own behaviour after the handler returns.
    pub fn default(&mut self);
    /// Run the widget's own behaviour now and return its message.
    pub fn run_default(&mut self) -> Option<M>;
}
```

### Focus types

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusReason { Pointer, Keyboard, Programmatic, A11y, Relocated }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusEvent { pub id: WidgetId, pub reason: FocusReason }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusOutcome { #[default] Allow, Redirect(WidgetId), Cancel }
```

`Cancel` from a focus-lost hook aborts the move; `Redirect` sends focus to
another widget instead (one redirect per move).

## Methods

### Construction and tree

| Methods | Behaviour |
|---|---|
| `new` | Empty tree and damage list. |
| `add`, `add_boxed`, `add_custom` | Append a widget (later widgets paint on top, win hit-testing and keep Tab order on ties); `add_custom` wraps a paint closure. |
| `add_with` | Adds with creation-time `WidgetProps` applied before the first layout. |
| `insert`, `insert_boxed`, `insert_before` | Insert at a position (reorders z-order and Tab ties); `insert_before` appends for an unknown anchor. |
| `raise`, `lower` | Move to the top/bottom of the paint order; `false` when already there. |
| `remove` | Close an open editor, relocate focus and remove; `false` when unknown. |
| `remove_preserving_session`, `has_pending_session`, `discard_pending_sessions` | Capture a painted editing session for a re-insert, inspect or drop it. |
| `clear`, `rebuild(area, sink, build)` | Drop everything; `rebuild` ends editors, clears, runs `build`, invalidates and returns the edit commit message. |
| `contains`, `is_empty`, `len`, `ids` | Query the tree; `ids` is in paint/hit order. |

### Geometry and typed access

| Methods | Behaviour |
|---|---|
| `set_rect`, `rect` | Silent assignment for layout passes; current rectangle. |
| `move_rect` | Move and repaint old and new rectangles, keep a native editor aligned. |
| `layout_vertical(area, gap)` | Stack in insertion order using `preferred_height`. |
| `with_widget::<W, R>`, `edit_widget::<W>` | Typed read/mutable access by downcast; `edit_widget` returns whether to repaint. |

### Bindings, focus and properties

| Methods | Behaviour |
|---|---|
| `bind`, `on_click`, `on_activate`, `unbind` | Bind a high-level `Trigger` (rebinding replaces); convenience wrappers for Click and Activate. |
| `on_focus_gained`, `on_focus_lost` | Focus hooks returning `FocusOutcome`. |
| `set_focus`, `focus_id`, `focus_visible`, `show_focus_ring` | Programmatic focus and ring state. |
| `set_focus_scope`, `focus_scope` | Confine Tab navigation (`None` removes the trap). |
| `set_focusable`, `set_focus_order` | Runtime focusability; Tab weight (smaller first). |
| `set_props`, `narration` | Apply `WidgetProps` at runtime; read installed narration. |
| `set_cursor`, `hover_cursor` | Cursor override; cursor the hovered widget requests. |
| `adjusting` | Focused widget is in keyboard adjust mode. |

### Pointer, wheel, keyboard and editing

| Methods | Behaviour |
|---|---|
| `pointer_move`, `pointer_leave` | Hover, drag tracking, hover clear. |
| `pointer_down`, `pointer_down_button`, `pointer_down_with` | Press with optional button and press-time modifiers. |
| `pointer_up`, `pointer_up_button`, `pointer_up_with` | Release with optional button and release-time modifiers. |
| `pointer_double_click` | Platform double click. |
| `wheel`, `wheel_xy` | Vertical notch; both axes (a self-scrolling widget wins over a view). |
| `key`, `key_up`, `pad_key` | Navigation/activation/mnemonics/editing (`false` lets the host try global shortcuts); release forwarding; gamepad-mapped key. |
| `char`, `ime_compose`, `ime_commit` | Translated characters and IME composition/result. |
| `set_pointer_kind`, `pointer_kind`, `long_press`, `hover_id`, `press_id`, `hover_wants_text` | Device kind, touch dwell, and interaction state queries. |
| `sync_edit`, `commit_edit`, `finish_inline_edit` | Re-align an open native editor, deliver committed text, close a painted draft. |

### Drag and drop

| Methods | Behaviour |
|---|---|
| `set_drag_source`, `clear_drag_source`, `set_drop_target`, `clear_drop_target`, `set_drop_hover`, `clear_drop_hover` | Payload, drop and hover callbacks for widget drags. |
| `set_text_drag_source`, `clear_text_drag_source`, `set_text_drop_target`, `clear_text_drop_target`, `set_text_drop_hover`, `clear_text_drop_hover` | The selected-text variants. |
| `set_files_drop_target` | Accept shell file drops. |
| `is_dragging`, `drag_data`, `drag_text`, `drag_source`, `drag_target`, `cancel_drag` | Session queries and cancellation. |
| `drag_enter`, `drag_over`, `drag_leave`, `drag_drop`, `drag_finished` | System (OLE) drag lifecycle; return `DropEffect`. |

### Scrolling

| Methods | Behaviour |
|---|---|
| `set_scroll_view`, `scroll_view`, `scroll_views`, `remove_scroll_view` | Register, read or drop a `ScrollView` panel. |
| `scroll_offset`, `set_scroll_offset`, `scroll_by` | Read or change the clamped offset. |
| `scroll_to_child`, `scroll_owner_of` | Reveal a child in every enclosing panel; find its innermost owner. |
| `set_scroll_viewport` | Move or resize a panel's viewport. |

A scroll view is a container region, not a widget: children are laid out once
in content coordinates, the container positions each at `rect - offset`, clips
painting and hit-testing, and paints the bars itself. See
[Scrolling](./scrolling.md).

### Images and content setters

| Methods | Behaviour |
|---|---|
| `set_image`, `remove_image`, `image`, `images` | Page image store, keyed by `ImageId`. |
| `set_enabled`, `set_visible` | Runtime state; a disabled widget loses focus, hidden widgets leave input, Tab, paint and a11y. |
| `set_value`, `set_toggle`, `set_selected`, `select_child` | Value, toggle and selection; `select_child` goes through the widget's own path and returns its message. |
| `set_rows`, `set_label`, `set_text`, `set_number`, `set_progress`, `set_message` | Content setters; a setter the widget does not implement is a no-op. |
| `set_mnemonic_underlines` | Show or hide `&` access-key underlines. |

### Menus

| Methods | Behaviour |
|---|---|
| `set_menu`, `remove_menu`, `menu` | The widget's own menu model. |
| `set_context_menu`, `context_menu` | Right-click / menu-key menu. |
| `open_menu`, `open_menu_for`, `expand_menu` | Open at an anchor, open what the widget would open, reopen at a submenu path. |
| `menu_command(id, command)` | Replay a selection; reserved editor ids are dispatched to the widget. |

### Painting, damage, messages and accessibility

| Methods | Behaviour |
|---|---|
| `paint(ctx)` | Paint widgets intersecting `ctx.dirty`; fills `ctx.caret`. |
| `mark_dirty`, `flush_damage`, `has_damage` | Queue one widget, emit one rectangle per changed region, query pending damage. |
| `overlay_active`, `overlay_split` | A modal backdrop is up; index where the overlay layer starts. |
| `drain_messages` | Take every queued message. |
| `debug_snapshot`, `debug_event`, `clear_debug_log` | `feature = "debug-form"` snapshots and log. |
| `a11y_nodes`, `apply_a11y` | Snapshot for UIA; apply a screen-reader action. |
| `installed_layout` (`layout-xml` / `layout-compiled`), `set_widget_registry`, `widget_registry`, `widget_registry_mut`, `clear_widget_registry` | Installed layout; the document widget factory registry (always compiled). |

## Example

```rust
use bamboo_native_ui::widgets::Button;
use bamboo_native_ui::{Invalidator, Point, Rect, Ui, UiHost, WidgetId};

enum Msg { Play }

struct Sink { damage: Vec<Rect> }

impl Invalidator for Sink {
    fn invalidate(&mut self, rect: Rect) { self.damage.push(rect); }
}

impl UiHost for Sink {}

fn click_play() -> Vec<Msg> {
    let mut ui: Ui<Msg> = Ui::new();
    ui.add(Button::new("play", "Play", Msg::Play));
    ui.set_rect(WidgetId::new("play"), Rect::from_xywh(16.0, 16.0, 120.0, 36.0));

    let mut sink = Sink { damage: Vec::new() };
    ui.pointer_down(Point::new(40.0, 30.0), &mut sink);
    ui.pointer_up(Point::new(40.0, 30.0), &mut sink);
    ui.flush_damage(&mut sink);
    assert!(!sink.damage.is_empty());
    ui.drain_messages()
}
```

## Notes and limits

- The application owns layout: `set_rect` and `layout_vertical` are the only
  entry points, and `set_rect` is silent so a pass assigns many rectangles and
  the host invalidates once. `paint` must run from `Host::paint` with a
  `PaintCtx`; it reads `dirty`, `overlay`, `editing`, `caret_on` and writes
  `caret`.
- A burst of `set_*` calls queues damage; call `flush_damage` once at the end of
  the host callback so distant widgets stay separate repaint pieces. Input
  routing invalidates directly and needs no flush.
- A binding may keep the widget's built-in behaviour with `ctx.default()`
  (runs after the handler) or `ctx.run_default()` (runs now and returns the
  message). Calling neither suppresses the default.
- `clear` has no host access, so end a native editor first; `rebuild` does it
  and `remove` closes or commits as needed. `remove` does not refresh the
  accessibility snapshot — call `WindowHandle::a11y_refresh`.

## See also

- [The widget trait](./widget-trait.md)
- [Events and input](./events-and-input.md)
- [Painting and Canvas](./painting-and-canvas.md)
- [Host and window](./host-and-window.md)
- [Scrolling](./scrolling.md)
- [Button](../widgets/controls/button.md)
- [First window quickstart](../quickstart/03-first-window.md)
