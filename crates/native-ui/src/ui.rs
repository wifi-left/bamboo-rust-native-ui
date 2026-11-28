// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Retained widget container.
//!
//! `Ui` owns hit-testing, hover/press/focus state and invalidation. Widgets are
//! created once per page rebuild; state changes update values in place and only
//! the affected rectangles are invalidated.

use std::collections::HashMap;

use crate::a11y::{A11yAction, A11yActionKind, A11yNode, A11yRole, A11yScroll};
use crate::backend::Canvas;
use crate::drag::{DragData, DropEffect};
use crate::event::{Cursor, KeyCode, KeyEvent, PointerKind};
use crate::geometry::{Point, Rect};
use crate::host::PaintCtx;
use crate::layout::VStack;
use crate::menu::{Menu, MenuPath, MenuSource};
use crate::platform::WindowHandle;
use crate::props::{Narration, WidgetProps};
use crate::scroll::{self, ScrollView};
#[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
use crate::surface::SurfaceStyle;
use crate::widget::{
    EditRequest, Row, SessionSnapshot, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState,
};
use crate::widgets::scrollbar::{self, BarHit};

/// Anything that can accept dirty rectangles. `WindowHandle` implements this;
/// unit tests use a recording sink to assert invalidation counts.
pub trait Invalidator {
    fn invalidate(&mut self, rect: Rect);
}

#[cfg(windows)]
impl Invalidator for WindowHandle<'_> {
    fn invalidate(&mut self, rect: Rect) {
        WindowHandle::invalidate(self, rect);
    }
}

/// Where the popup should sit. Hover tips follow the pointer; keyboard/a11y
/// focus tips hang below the widget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TooltipAnchor {
    Pointer(Point),
    Widget(Rect),
}

/// A tooltip the host may show after its dwell time. `widget` identifies the
/// target (so moving the pointer inside it does not restart the dwell), `item`
/// is the hovered row/option/cell for composite controls (moving between rows
/// is a new target), and `anchor` is the latest placement request, in client
/// DIPs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TooltipRequest<'a> {
    pub text: &'a str,
    pub widget: Rect,
    pub item: Option<usize>,
    pub anchor: TooltipAnchor,
}

/// A native popup menu the host should show. `anchor` is the menu's top-left
/// corner in client DIPs: the pointer for a secondary click, the focused
/// widget's lower-left corner for the menu key / Shift+F10.
pub struct ContextMenuRequest<'a> {
    /// Widget whose menu was invoked; echoed back with the selected command.
    pub widget: WidgetId,
    pub menu: &'a Menu,
    pub anchor: Point,
    /// Control rectangle the popup must not cover (client DIPs). A menu shown
    /// for a widget (a menu button, a menu bar cell) flips above this
    /// rectangle instead of covering it; a pointer context menu passes `None`.
    pub avoid: Option<Rect>,
    /// What opened the menu; carried through the host's `MenuEvent` hook.
    pub source: MenuSource,
}

/// The sink `Ui` talks to besides invalidation: it opens, moves and closes the
/// native text editor. The platform host implements it on `WindowHandle`; tests
/// record the calls. Every method has a no-op default so a plain invalidator
/// only needs `impl UiHost for Sink {}`.
pub trait UiHost: Invalidator {
    /// Open a native editor for `request`, replacing any active session.
    fn begin_edit(&mut self, _request: &EditRequest<'_>) {}

    /// Commit and close the active editor because focus left its widget.
    fn end_edit(&mut self) {}

    /// The edited widget moved; keep the native editor aligned.
    fn move_edit(&mut self, _rect: Rect) {}

    /// Id of the widget whose native editor is currently open.
    fn editing_id(&self) -> Option<WidgetId> {
        None
    }

    /// The hovered or keyboard-focused widget changed. `Some` asks the host to
    /// show a tooltip after its dwell (450 ms); `None` hides any visible one.
    /// Hosts deduplicate repeated identical requests, so reporting the same
    /// target on every pointer move does not restart the dwell.
    fn set_tooltip(&mut self, _target: Option<TooltipRequest<'_>>) {}

    /// Secondary click / menu key on a widget that has a context menu. The
    /// host shows it after the current callback returns and reports the
    /// selection through [`crate::Host::menu_command`]; the modal menu loop
    /// must never run inside `Host::event`. The tooltip has been dismissed by
    /// the time this is called.
    fn show_context_menu(&mut self, _request: ContextMenuRequest<'_>) {}

    /// Read the clipboard as text (painted editor paste). `None` when the
    /// clipboard is empty, holds another format or is owned by another
    /// process.
    fn clipboard_text(&mut self) -> Option<String> {
        None
    }

    /// Write text to the clipboard (painted editor copy/cut).
    fn copy_to_clipboard(&mut self, _text: &str) {}

    /// Tell the host where the focused painted editor's caret is, so the IME
    /// composition/candidate window follows it and the blink timer knows which
    /// rectangle to toggle. `None` stops the caret timer.
    fn set_caret_rect(&mut self, _rect: Option<Rect>) {}

    /// Raise (`true`) or dismiss (`false`) the system on-screen keyboard for a
    /// text session that [`Ui::pad_key`] opened. The framework never reports
    /// pointer or hardware-keyboard text entry here, so a host maps this
    /// straight to the platform keyboard and desktop users never see one.
    ///
    /// Returns whether the request was accepted; `false` (the default, and
    /// what a machine without the keyboard reports) makes `pad_key` continue
    /// ordinary key routing instead of waiting for a keyboard that never
    /// appears.
    ///
    /// `WindowHandle` forwards it to [`crate::soft_keyboard`] with the gamepad
    /// layout; plain invalidators and tests keep the no-op default.
    fn set_soft_keyboard(&mut self, _visible: bool) -> bool {
        false
    }

    /// Whether the system on-screen keyboard is currently up. [`Ui::pad_key`]
    /// uses it to re-raise the keyboard on **A** after the user dismissed it
    /// with its own close button. The default is `false` (never open).
    fn soft_keyboard_open(&self) -> bool {
        false
    }

    /// Raise (`true`) or dismiss (`false`) the system on-screen keyboard for a
    /// text session a *touch* press opened (the pointer kind was
    /// [`PointerKind::Touch`]). A painted field is not a real edit control, so
    /// Windows does not raise the keyboard by itself.
    ///
    /// The default accepts nothing (`false`); `WindowHandle` forwards it to
    /// [`crate::soft_keyboard`] with the touch layout. A dismissed session
    /// (focus loss, commit, removal) asks for `false`.
    fn set_touch_keyboard(&mut self, _visible: bool) -> bool {
        false
    }

    /// Start a **system** drag (OLE) carrying `data`. Called when a registered
    /// drag source crosses the threshold. `WindowHandle` queues the drag and
    /// starts it after the current callback returns (never re-entering the
    /// application from inside `Ui`); a host without system drags returns
    /// `false`, and the container falls back to its in-window session.
    fn start_drag(&mut self, _data: DragData) -> bool {
        false
    }
}

/// High-level interaction a binding can observe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Press and release landed on the same widget.
    Click,
    /// The platform reported a double click (`CS_DBLCLKS`). A double click
    /// also produces the ordinary clicks around it.
    DoubleClick,
    /// Secondary-button press and release landed on the widget. The binding
    /// runs, then the widget's `context_menu` (when installed) is shown unless
    /// a native editor owns the field.
    RightClick,
    HoverEnter,
    HoverLeave,
    Wheel,
    /// Enter/Space on a focused widget, or a UIA Invoke/Toggle request.
    Activate,
    /// The pointer moved past the drag threshold while the button was held.
    DragStart,
    /// The button was released after a drag.
    DragEnd,
    /// The widget's accessibility value changed as a result of this input
    /// (checked, selected option/row, range, text). Fires after the widget
    /// handled the input, so the custom message is queued after the default
    /// one. Editor commits and `Ui::set_*` calls are application-known and do
    /// not fire it.
    ValueChange,
}

/// Information handed to a trigger handler.
#[derive(Clone, Copy, Debug)]
pub struct TriggerEvent {
    pub trigger: Trigger,
    /// Pointer position in client DIPs when the trigger came from the mouse.
    pub pos: Option<Point>,
    /// Wheel delta; zero for the other triggers.
    pub wheel_delta: f32,
}

/// Escape hatch passed to every binding. Two ways to keep the widget's normal
/// behavior, and neither leaves it suppressed:
///
/// * [`TriggerContext::default`] sets a flag: the default runs *after* the
///   handler returns (the original form, kept for compatibility).
/// * [`TriggerContext::run_default`] runs the default *now* and returns its
///   message, so the handler can wrap or post-process the built-in result.
///   The default does not run again afterwards.
///
/// Call neither to suppress the default entirely.
pub struct TriggerContext<'a, M = ()> {
    /// Flag form: run the default after the handler returns.
    requested: bool,
    /// The handler already ran the default through `run_default`.
    ran: bool,
    /// The widget's own action, invocable from inside the handler.
    run: &'a mut dyn FnMut() -> Option<M>,
}

impl<M> TriggerContext<'_, M> {
    /// Run the widget's original behavior (message, state, edit session,
    /// repaint) after the handler returns. The backward-compatible flag form.
    pub fn default(&mut self) {
        self.requested = true;
    }

    /// Run the widget's original behavior *now* and return its message, so the
    /// handler can wrap or post-process it. The default will not run a second
    /// time. Returns `None` when called more than once in one handler.
    pub fn run_default(&mut self) -> Option<M> {
        if self.ran {
            return None;
        }
        self.ran = true;
        (self.run)()
    }
}

type TriggerHandler<M> = Box<dyn FnMut(TriggerEvent, &mut TriggerContext<'_, M>) -> Option<M>>;

struct Binding<M> {
    id: WidgetId,
    trigger: Trigger,
    handler: TriggerHandler<M>,
}

/// Result of one widget input dispatch: the trigger's custom message, the
/// widget's own message, a post-change message (value-change bindings) and
/// whether the widget's default behavior ran.
struct Outcome<M> {
    custom: Option<M>,
    default: Option<M>,
    /// An additive message the widget reported alongside its own (a drag-end
    /// callback next to a change report).
    extra: Option<M>,
    after: Option<M>,
    ran: bool,
}

/// Accessible state compared before/after one input to detect a value change.
#[derive(PartialEq)]
struct ValueFingerprint {
    checked: Option<bool>,
    value: Option<String>,
    selected: Option<bool>,
    range: Option<(f64, f64, f64)>,
    child: Option<usize>,
}

/// Why focus moved; passed to the focus lifecycle hooks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusReason {
    Pointer,
    Keyboard,
    Programmatic,
    /// UIA `SetFocus` / Invoke on a field.
    A11y,
    /// Relocated because the previous owner was hidden, disabled or made
    /// non-focusable.
    Relocated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusEvent {
    pub id: WidgetId,
    pub reason: FocusReason,
}

/// Returned by a focus hook. `Cancel` on a lost hook aborts the move; `Redirect`
/// sends focus to another widget instead (one redirect per move).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusOutcome {
    #[default]
    Allow,
    Redirect(WidgetId),
    Cancel,
}

struct FocusHooks {
    id: WidgetId,
    gained: Option<Box<dyn FnMut(FocusEvent) -> FocusOutcome>>,
    lost: Option<Box<dyn FnMut(FocusEvent) -> FocusOutcome>>,
}

pub struct Ui<M> {
    widgets: Vec<Box<dyn Widget<M>>>,
    hover: Option<usize>,
    press: Option<usize>,
    /// Last pointer position in client DIPs (tooltip placement, hit hints).
    pointer: Option<Point>,
    /// Where the primary button went down; the drag threshold is measured
    /// from here.
    press_pos: Option<Point>,
    dragging: bool,
    /// Widget under the secondary-button press, used to pair down and up.
    right_press: Option<usize>,
    focus: Option<usize>,
    adjusting: bool,
    /// Draw the focus ring. Pointer focus stays invisible (a mouse click is not
    /// an accessibility navigation); Tab/arrow/Enter turn it on.
    focus_visible: bool,
    messages: Vec<M>,
    /// Per-instance focus policy: tab weight and a `focusable` override that
    /// leaves the widget's own default untouched when unset.
    focus_policies: Vec<(WidgetId, FocusPolicy)>,
    /// Per-instance cursor override; an entry here wins over the widget's
    /// suggested shape.
    cursors: HashMap<WidgetId, Cursor>,
    /// Container-owned visibility. Widgets that track their own (Toast) still
    /// get `set_visible`, but the container is the source of truth for input,
    /// painting and the a11y tree.
    visibility: HashMap<WidgetId, bool>,
    /// Fast id → index lookup. Rebuilt after a structural change; a pointer
    /// move performs enough lookups (hit-testing, scroll ownership) that a
    /// linear scan per widget is a measurable per-event cost.
    widget_index: HashMap<WidgetId, usize>,
    /// Indices of the currently visible widgets, in paint order. Rebuilt
    /// lazily when `visible_dirty` is set (a page switch hides/shows dozens of
    /// widgets at once); hit-testing and painting then visit only the active
    /// page instead of every widget in every page.
    visible_indices: Vec<usize>,
    visible_dirty: bool,
    /// Scroll-bar/corner state under the last pointer position, shared by
    /// hit-testing, bar hover and the cursor so one move scans the panels once.
    bar_hover: Option<(usize, bool)>,
    bar_corner: bool,
    chrome_valid: bool,
    /// Per-panel "has visible content" results, rebuilt after a visibility or
    /// panel change. `scroll_shown` is called several times per pointer move
    /// and per paint; the recursive child scan is not repeated while clean.
    scroll_shown_cache: Vec<bool>,
    scroll_shown_dirty: bool,
    /// Presentation and narration properties that are not container state
    /// (palette, role, name/description/row overrides, automation id).
    props: HashMap<WidgetId, WidgetProps>,
    /// Trigger bindings; one handler per (widget, trigger) pair.
    bindings: Vec<Binding<M>>,
    /// Focus gained/lost hooks, one entry per widget.
    focus_hooks: Vec<FocusHooks>,
    /// Page images (covers, avatars) referenced by widgets through
    /// `WidgetPaint::image`. Owned here so a cover shared by several widgets
    /// is stored once.
    images: crate::image::ImageStore,
    /// When set, Tab/arrow navigation and programmatic focus are confined to
    /// these widgets (an in-window modal shell).
    focus_scope: Option<Vec<WidgetId>>,
    /// A gamepad raised the system on-screen keyboard for the current text
    /// session ([`Ui::pad_key`]); leaving the session dismisses it. Pointer
    /// and hardware-keyboard sessions leave this false.
    pad_soft: bool,
    /// A touch press opened a text session and the host raised its on-screen
    /// keyboard; the session's end dismisses it (D121).
    touch_soft: bool,
    /// Device driving the pointer (the host reports a change through
    /// [`Event::PointerKind`]). Touch selects the painted editor's touch
    /// model: long-press word selection and selection handles (D121).
    pointer_kind: PointerKind,
    /// Scrollable viewports. A child listed here is positioned by the panel
    /// (`content rect - offset`) and clipped to its viewport; the container
    /// paints and drags the bars. See D65.
    scroll_views: Vec<ScrollView>,
    /// Scroll-view child → owning panel (the first panel listing it), so
    /// hit-testing and painting do not rescan every child list per widget.
    scroll_children: HashMap<WidgetId, usize>,
    /// Thumb drag in flight: which panel/axis and the press grab offset.
    scroll_drag: Option<ScrollDrag>,
    /// Sessions captured by [`Ui::remove_preserving_session`], waiting for a
    /// widget with the same id to be inserted (D111).
    pending_sessions: Vec<(WidgetId, SessionSnapshot)>,
    /// Cross-widget drag sources: a widget id and the data it offers when a
    /// drag starts on it. The data is what OLE transports (text/files), so the
    /// same drag can be dropped into another window or application.
    drag_sources: Vec<(WidgetId, Box<dyn Fn() -> Option<DragData>>)>,
    /// Text drag sources: a painted field whose selected text a drag copies
    /// out (D122). No closure: the widget supplies the text.
    text_sources: Vec<WidgetId>,
    /// Drop targets: a widget id, the payload kinds it accepts, and the
    /// handler that consumes the data released over it (returns the
    /// application message). Text, files and a system drag from another
    /// application all arrive here.
    drop_targets: Vec<(WidgetId, DragKind, Box<dyn Fn(DragData, Point) -> Option<M>>)>,
    /// Optional hover feedback for a target: `(data, pos, over) -> repaint`.
    drop_hovers: Vec<(WidgetId, Box<dyn Fn(&DragData, Point, bool) -> bool>)>,
    /// The drag in flight, if any: an OLE system drag (the host runs it) or
    /// the container's own fallback when the host has no system drags.
    drag: Option<DragSession>,
    /// Pending paint damage: every widget rectangle a programmatic
    /// display-property change touched since the last [`Ui::flush_damage`]
    /// (the `set_*` setters, [`Ui::edit_widget`], [`Ui::set_props`],
    /// [`Ui::set_enabled`], [`Ui::set_message`] and [`Ui::mark_dirty`]).
    /// Pieces are kept separate (and merged only when they overlap or touch),
    /// so repainting a few widgets spread over the page stays a few small
    /// rectangles instead of one page-sized bounding box. Code-driven changes
    /// never call the widget's change callback; this is what repaints them,
    /// once per batch (D120/D123).
    damage: Vec<Rect>,
    /// The installed layout skin (`layout-xml`): generation, resolved entries
    /// and the baselines a claimed field is restored to.
    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    xml_layout: Option<crate::layout_xml::AppliedLayout>,
    /// Widget types a layout document may create (M7.7/D148). `None` keeps the
    /// address-only model: a `type` entry is `xml.type.unknown`.
    widget_registry: Option<crate::creation::WidgetRegistry<M>>,
    /// Bounded diagnostics ring the debug API exposes (`debug-form` only).
    #[cfg(feature = "debug-form")]
    debug_log: crate::debug::DebugLog,
    /// Per-frame and per-widget paint measurements (`debug-form` only).
    #[cfg(feature = "debug-form")]
    paint_recorder: crate::debug::PaintRecorder,
    /// The pieces the last `flush_damage` handed to the host.
    #[cfg(feature = "debug-form")]
    debug_last_damage: Vec<Rect>,
}

/// A bar drag owned by the container (bars are painted by `Ui`, not a widget).
#[derive(Clone, Copy, Debug)]
struct ScrollDrag {
    panel: usize,
    vertical: bool,
    /// Pointer position minus thumb start at press time.
    grab: f32,
}

/// A drag in flight: the data the source produced, the widget currently under
/// the pointer (a registered drop target) and the last pointer position.
///
/// `system` separates the two owners: a host with OLE runs the gesture and
/// reports `DragEnter`/`DragOver`/`Drop`, so the container only mirrors the
/// target for hover and accessors. Without OLE (a test sink, a platform
/// without drag) the container tracks the pointer itself.
struct DragSession {
    source: Option<WidgetId>,
    data: DragData,
    target: Option<WidgetId>,
    pos: Point,
    system: bool,
}

/// What a registered drop target accepts. The window's `DragOver` effect is
/// derived from this, so a file dragged over a text-only target shows the
/// shell's "no drop" cursor instead of accepting and then ignoring it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DragKind {
    Any,
    Text,
    Files,
}

impl DragKind {
    fn accepts(self, data: &DragData) -> bool {
        match (self, data) {
            (Self::Any, _) => true,
            (Self::Text, DragData::Text(_)) => true,
            (Self::Files, DragData::Files(_)) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Copy)]
struct FocusPolicy {
    /// Ascending Tab order; 0 means "insertion order".
    weight: i32,
    focusable: Option<bool>,
}

/// Pointer travel (DIPs) before a held press becomes a drag.
const DRAG_THRESHOLD: f32 = 4.0;

/// Most separate rectangles one pending damage batch keeps before it collapses
/// to a single bounding box. Kept well above the handful of widgets a timer
/// usually touches, so a normal batch stays partial.
const MAX_DAMAGE_PIECES: usize = 32;

impl<M> Default for Ui<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Ui<M> {
    pub fn new() -> Self {
        Self {
            widgets: Vec::new(),
            damage: Vec::new(),
            #[cfg(feature = "debug-form")]
            debug_log: crate::debug::DebugLog::default(),
            #[cfg(feature = "debug-form")]
            paint_recorder: crate::debug::PaintRecorder::default(),
            #[cfg(feature = "debug-form")]
            debug_last_damage: Vec::new(),
            hover: None,
            press: None,
            pointer: None,
            press_pos: None,
            dragging: false,
            right_press: None,
            focus: None,
            adjusting: false,
            focus_visible: false,
            messages: Vec::new(),
            focus_policies: Vec::new(),
            cursors: HashMap::new(),
            visibility: HashMap::new(),
            widget_index: HashMap::new(),
            visible_indices: Vec::new(),
            visible_dirty: true,
            bar_hover: None,
            bar_corner: false,
            chrome_valid: false,
            scroll_shown_cache: Vec::new(),
            scroll_shown_dirty: true,
            props: HashMap::new(),
            bindings: Vec::new(),
            focus_hooks: Vec::new(),
            images: crate::image::ImageStore::new(),
            focus_scope: None,
            pad_soft: false,
            touch_soft: false,
            pointer_kind: PointerKind::Mouse,
            scroll_views: Vec::new(),
            scroll_children: HashMap::new(),
            scroll_drag: None,
            pending_sessions: Vec::new(),
            drag_sources: Vec::new(),
            text_sources: Vec::new(),
            drop_targets: Vec::new(),
            drop_hovers: Vec::new(),
            drag: None,
            #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
            xml_layout: None,
            widget_registry: None,
        }
    }

    /// Drop every widget; call on page switch / rebuild, then invalidate all.
    /// Close any open native editor first (`WindowHandle::end_edit`): this
    /// method has no host access, so it cannot do that itself.
    pub fn clear(&mut self) {
        self.widgets.clear();
        self.widget_index.clear();
        self.visible_indices.clear();
        self.visible_dirty = true;
        self.scroll_shown_cache.clear();
        self.scroll_shown_dirty = true;
        self.bar_hover = None;
        self.bar_corner = false;
        self.chrome_valid = false;
        self.hover = None;
        self.press = None;
        self.pointer = None;
        self.press_pos = None;
        self.dragging = false;
        self.right_press = None;
        self.focus = None;
        self.adjusting = false;
        self.focus_visible = false;
        self.focus_policies.clear();
        self.cursors.clear();
        self.visibility.clear();
        self.props.clear();
        self.bindings.clear();
        self.focus_hooks.clear();
        self.images.clear();
        self.focus_scope = None;
        self.scroll_views.clear();
        self.scroll_children.clear();
        self.scroll_drag = None;
        self.pending_sessions.clear();
        self.drag_sources.clear();
        self.text_sources.clear();
        self.drop_targets.clear();
        #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
        {
            self.xml_layout = None;
        }
        self.drop_hovers.clear();
        self.drag = None;
        // Pending damage belongs to the old tree; a rebuilt page repaints
        // through its own layout pass.
        self.damage.clear();
    }

    pub fn add(&mut self, widget: impl Widget<M> + 'static) -> &mut Self {
        self.add_boxed(Box::new(widget))
    }

    /// [`Ui::add`] for a widget the caller already holds boxed (a factory
    /// produces `Box<dyn Widget<M>>`, M7.7).
    pub fn add_boxed(&mut self, widget: Box<dyn Widget<M>>) -> &mut Self {
        self.widgets.push(widget);
        let index = self.widgets.len() - 1;
        self.widget_index
            .entry(self.widgets[index].id())
            .or_insert(index);
        self.invalidate_visible();
        self.restore_pending_session(index);
        self
    }

    /// Register a closure-painted custom widget in one call (see
    /// [`crate::widgets::CustomWidget`] for the builder options):
    ///
    /// ```ignore
    /// ui.add_custom("meter", rect, |ctx, rect, state| {
    ///     ctx.painter.round_rect(ctx.canvas, *rect, 8.0, ctx.palette.accent);
    /// });
    /// ```
    pub fn add_custom(
        &mut self,
        id: &'static str,
        rect: Rect,
        paint: impl for<'a, 'b> Fn(
            &'a mut crate::widget::WidgetPaint<'b>,
            Rect,
            &'a crate::widget::WidgetState,
        ) + 'static,
    ) -> &mut Self
    where
        M: 'static,
    {
        self.add(crate::widgets::CustomWidget::new(id, paint).rect(rect))
    }

    /// Add a widget with creation-time properties in one expression:
    /// `ui.add_with(Card::new(..), WidgetProps::new().mouse(true).name("Details"))`.
    /// Focus/visibility/enabled overrides are recorded before the first layout,
    /// so they never need focus relocation or a repaint of another widget.
    pub fn add_with(
        &mut self,
        widget: impl Widget<M> + 'static,
        props: WidgetProps,
    ) -> &mut Self {
        self.add(widget);
        let id = self.widgets[self.widgets.len() - 1].id();
        self.apply_props(id, props);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }

    /// Whether a widget with this id is in the tree.
    pub fn contains(&self, id: WidgetId) -> bool {
        self.index_of(id).is_some()
    }

    /// Widget ids in insertion (paint/hit-test) order. Useful for a dynamic
    /// panel that tracks its own members.
    pub fn ids(&self) -> impl Iterator<Item = WidgetId> + '_ {
        self.widgets.iter().map(|widget| widget.id())
    }

    /// Insert a widget at a tree position. Later positions paint on top and
    /// win hit-testing, and equal focus weights keep insertion order (D1/D58),
    /// so this also reorders z-order and Tab ties. `at` is clamped to the end.
    ///
    /// Unlike [`Ui::add`] the container's pointer/focus indices shift with the
    /// insertion; the widget keeps its own state. Widget ids stay unique
    /// (`WidgetId` is a static name), so inserting an id that is already
    /// present leaves two widgets with one identity — do not do that.
    pub fn insert(&mut self, at: usize, widget: impl Widget<M> + 'static) -> &mut Self {
        self.insert_boxed(at, Box::new(widget))
    }

    /// [`Ui::insert`] for an already boxed widget.
    pub fn insert_boxed(&mut self, at: usize, widget: Box<dyn Widget<M>>) -> &mut Self {
        let at = at.min(self.widgets.len());
        self.widgets.insert(at, widget);
        self.refresh_widget_index();
        self.invalidate_visible();
        self.shift_state_after_insert(at);
        self.restore_pending_session(at);
        self
    }

    /// Remove a widget but keep a live framework-painted editing session for
    /// its id, so a widget inserted later with the same id inherits the draft,
    /// caret and selection (D111). The draft is closed here, not committed;
    /// nothing reaches `on_change`/`on_commit`. A native `EDIT` session and a
    /// composite's embedded field cannot be handed over and still commit on
    /// removal; a painted [`crate::widgets::TextEdit`] is the supported case.
    /// The snapshot is dropped when the id is inserted again, removed again or
    /// the page is cleared.
    pub fn remove_preserving_session(&mut self, id: WidgetId, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let snapshot = self.widgets[index].take_session();
        let removed = self.remove(id, sink);
        if removed && let Some(snapshot) = snapshot {
            self.pending_sessions.push((id, snapshot));
        }
        removed
    }

    /// Whether a captured session is waiting for a widget with this id.
    pub fn has_pending_session(&self, id: WidgetId) -> bool {
        self.pending_sessions
            .iter()
            .any(|(pending, _)| *pending == id)
    }

    /// Drop every captured session (call when the page is being discarded).
    pub fn discard_pending_sessions(&mut self) {
        self.pending_sessions.clear();
    }

    fn restore_pending_session(&mut self, index: usize) {
        let id = self.widgets[index].id();
        let Some(position) = self
            .pending_sessions
            .iter()
            .position(|(pending, _)| *pending == id)
        else {
            return;
        };
        let (_, snapshot) = self.pending_sessions.remove(position);
        // A widget that cannot take it drops it; it will not apply elsewhere.
        let _ = self.widgets[index].restore_session(snapshot);
    }

    // ---- System drag and drop (OLE) -----------------------------------------

    /// Register a drag source: when the pointer is dragged past the threshold
    /// on this widget, `source` produces the data the drag carries (or `None`
    /// to fall back to the ordinary `Trigger::DragStart` path).
    ///
    /// The data is a [`DragData`] (text or a file list), which is exactly what
    /// a system drag can transport: the host starts an OLE drag, so the same
    /// gesture can be dropped into another window or another application.
    /// Registering the same id again replaces the source.
    pub fn set_drag_source(
        &mut self,
        id: WidgetId,
        source: impl Fn() -> Option<DragData> + 'static,
    ) -> &mut Self {
        if self.index_of(id).is_none() {
            return self;
        }
        self.drag_sources.retain(|(candidate, _)| *candidate != id);
        self.drag_sources.push((id, Box::new(source)));
        self
    }

    pub fn clear_drag_source(&mut self, id: WidgetId) {
        self.drag_sources.retain(|(candidate, _)| *candidate != id);
    }

    /// Register a drop target: `drop` consumes the data released over this
    /// widget and returns the application message (or `None`). It runs for a
    /// drag that started in this window *and* for text or files dragged in
    /// from another window or application (OLE). Registering the same id again
    /// replaces the target.
    pub fn set_drop_target(
        &mut self,
        id: WidgetId,
        drop: impl Fn(DragData, Point) -> Option<M> + 'static,
    ) -> &mut Self {
        self.set_drop_target_kind(id, DragKind::Any, drop)
    }

    fn set_drop_target_kind(
        &mut self,
        id: WidgetId,
        kind: DragKind,
        drop: impl Fn(DragData, Point) -> Option<M> + 'static,
    ) -> &mut Self {
        if self.index_of(id).is_none() {
            return self;
        }
        self.drop_targets.retain(|(candidate, _, _)| *candidate != id);
        self.drop_targets.push((id, kind, Box::new(drop)));
        self
    }

    pub fn clear_drop_target(&mut self, id: WidgetId) {
        self.drop_targets.retain(|(candidate, _, _)| *candidate != id);
    }

    /// Optional hover feedback for a drop target: called when the pointer
    /// enters (`over = true`) or leaves (`over = false`) this widget during a
    /// drag, with the data and pointer position. Return `true` when the
    /// widget's visuals changed so the container repaints it.
    pub fn set_drop_hover(
        &mut self,
        id: WidgetId,
        hover: impl Fn(&DragData, Point, bool) -> bool + 'static,
    ) -> &mut Self {
        if self.index_of(id).is_none() {
            return self;
        }
        self.drop_hovers.retain(|(candidate, _)| *candidate != id);
        self.drop_hovers.push((id, Box::new(hover)));
        self
    }

    pub fn clear_drop_hover(&mut self, id: WidgetId) {
        self.drop_hovers.retain(|(candidate, _)| *candidate != id);
    }

    /// Register a painted field as a *text* drag source (D122): pressing
    /// inside its selection and dragging past the threshold copies the
    /// selected text into a system drag (the field keeps its text). The drag
    /// can leave the window like any other.
    ///
    /// A press that does not start inside the selection, a collapsed
    /// selection, a password field and a press that grabbed a touch selection
    /// handle never start the drag; the field goes on with its normal
    /// selection behavior.
    pub fn set_text_drag_source(&mut self, id: WidgetId) -> &mut Self {
        if self.index_of(id).is_some() {
            self.text_sources.retain(|candidate| *candidate != id);
            self.text_sources.push(id);
        }
        self
    }

    pub fn clear_text_drag_source(&mut self, id: WidgetId) {
        self.text_sources.retain(|candidate| *candidate != id);
    }

    /// Register a target that accepts text only (a convenience wrapper around
    /// [`Ui::set_drop_target`]). When the target is a painted field the
    /// container inserts the text at the release point first (a user edit, so
    /// its `on_change` also reports) unless the field is read-only.
    pub fn set_text_drop_target(
        &mut self,
        id: WidgetId,
        drop: impl Fn(String, Point) -> Option<M> + 'static,
    ) -> &mut Self {
        self.set_drop_target_kind(id, DragKind::Text, move |data, pos| match data {
            DragData::Text(text) => drop(text, pos),
            DragData::Files(_) => None,
        })
    }

    pub fn clear_text_drop_target(&mut self, id: WidgetId) {
        self.clear_drop_target(id);
    }

    /// Hover feedback for a text-only target: `(text, pos, over) -> repaint`.
    pub fn set_text_drop_hover(
        &mut self,
        id: WidgetId,
        hover: impl Fn(&str, Point, bool) -> bool + 'static,
    ) -> &mut Self {
        self.set_drop_hover(id, move |data, pos, over| match data {
            DragData::Text(text) => hover(text, pos, over),
            DragData::Files(_) => false,
        })
    }

    pub fn clear_text_drop_hover(&mut self, id: WidgetId) {
        self.clear_drop_hover(id);
    }

    /// Register a target that accepts file lists only (dropped from Explorer
    /// or another application).
    pub fn set_files_drop_target(
        &mut self,
        id: WidgetId,
        drop: impl Fn(Vec<std::path::PathBuf>, Point) -> Option<M> + 'static,
    ) -> &mut Self {
        self.set_drop_target_kind(id, DragKind::Files, move |data, pos| match data {
            DragData::Files(paths) => drop(paths, pos),
            DragData::Text(_) => None,
        })
    }

    /// Whether a drag is in flight (a system drag or the in-window fallback).
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The data the drag in flight carries, for a hover/drop handler that wants
    /// to peek (a typed-access read).
    pub fn drag_data(&self) -> Option<&DragData> {
        self.drag.as_ref().map(|drag| &drag.data)
    }

    /// The text a drag is carrying, when it is a text drag.
    pub fn drag_text(&self) -> Option<&str> {
        self.drag.as_ref()?.data.as_text()
    }

    /// The widget the drag started on, when one is in flight. `None` for a
    /// drag that entered from another window or application.
    pub fn drag_source(&self) -> Option<WidgetId> {
        self.drag.as_ref().and_then(|drag| drag.source)
    }

    /// The registered target currently under the pointer, when one is in
    /// flight.
    pub fn drag_target(&self) -> Option<WidgetId> {
        self.drag.as_ref().and_then(|drag| drag.target)
    }

    /// Abort a drag that started in this window (Escape, window deactivation,
    /// page change). The data is dropped without a drop handler; the source is
    /// repainted. A system drag is OLE's; use this to clear the mirror state,
    /// the shell itself cancels on Escape.
    pub fn cancel_drag(&mut self, sink: &mut impl UiHost) {
        let Some(session) = self.drag.take() else {
            return;
        };
        self.dragging = false;
        self.press = None;
        self.press_pos = None;
        if let Some(target) = session.target {
            self.notify_drag_hover(target, &session.data, session.pos, false, sink);
        }
        if let Some(index) = session.source.and_then(|id| self.index_of(id)) {
            sink.invalidate(self.bounds(index));
        }
    }

    /// Start a drag from the widget at `index` if it registered a source and
    /// the source produced data. Returns whether the drag started. A text
    /// source takes precedence and lets the widget supply the text.
    fn begin_drag(&mut self, index: usize, pos: Point, sink: &mut impl UiHost) -> bool {
        let id = self.widgets[index].id();
        let data = if self.text_sources.contains(&id) {
            match self.widgets[index].take_drag_text(pos) {
                Some(text) => DragData::Text(text),
                None => return false,
            }
        } else {
            let Some((_, source)) = self
                .drag_sources
                .iter()
                .find(|(candidate, _)| *candidate == id)
            else {
                return false;
            };
            let Some(data) = source() else {
                return false;
            };
            data
        };
        if data.is_empty() {
            return false;
        }
        // A host with OLE (WindowHandle) takes the gesture over; a test sink
        // or a platform without system drags reports false and the container
        // keeps tracking the pointer itself.
        let system = sink.start_drag(data.clone());
        self.drag = Some(DragSession {
            source: Some(id),
            data,
            target: None,
            pos,
            system,
        });
        true
    }

    /// Track the widget under the pointer for the in-window fallback (a system
    /// drag reports its own hover through `drag_enter`/`drag_over`).
    fn update_drag_hover(&mut self, pos: Point, sink: &mut impl UiHost) {
        let Some((source, current, system)) = self
            .drag
            .as_ref()
            .map(|drag| (drag.source, drag.target, drag.system))
        else {
            return;
        };
        if system {
            return;
        }
        let new_target = self
            .hit_index(pos)
            .map(|index| self.widgets[index].id())
            .filter(|id| Some(*id) != source);
        if let Some(drag) = self.drag.as_mut() {
            drag.pos = pos;
        }
        if new_target == current {
            return;
        }
        if let Some(old) = current
            && let Some(data) = self.drag.as_ref().map(|drag| &drag.data)
        {
            self.notify_drag_hover(old, data, pos, false, sink);
        }
        if let Some(new) = new_target
            && let Some(data) = self.drag.as_ref().map(|drag| &drag.data)
        {
            self.notify_drag_hover(new, data, pos, true, sink);
        }
        if let Some(drag) = self.drag.as_mut() {
            drag.target = new_target;
        }
    }

    /// Call one target's hover callback and repaint it when it asked to.
    fn notify_drag_hover(
        &self,
        target: WidgetId,
        data: &DragData,
        pos: Point,
        over: bool,
        sink: &mut impl UiHost,
    ) {
        let changed = self
            .drop_hovers
            .iter()
            .find(|(candidate, _)| *candidate == target)
            .map(|(_, hover)| hover(data, pos, over))
            .unwrap_or(false);
        if changed
            && let Some(index) = self.index_of(target)
        {
            sink.invalidate(self.bounds(index));
        }
    }

    /// The widget under `pos` that registered a drop handler for this data
    /// kind (never `source`).
    fn drop_target_at(
        &mut self,
        pos: Point,
        source: Option<WidgetId>,
        data: &DragData,
    ) -> Option<WidgetId> {
        let index = self.hit_index(pos)?;
        let id = self.widgets[index].id();
        if Some(id) == source {
            return None;
        }
        self.drop_targets
            .iter()
            .any(|(candidate, kind, _)| *candidate == id && kind.accepts(data))
            .then_some(id)
    }

    /// Run `target`'s drop handler with `data`. A painted text target inserts
    /// the text at `pos` first (a normal user edit), then the application
    /// handler reports the drop. Returns whether a handler consumed the data.
    fn drop_on_target(&mut self, target: WidgetId, data: DragData, pos: Point) -> bool {
        let Some(position) = self
            .drop_targets
            .iter()
            .position(|(candidate, kind, _)| *candidate == target && kind.accepts(&data))
        else {
            return false;
        };
        if let DragData::Text(text) = &data
            && let Some(index) = self.index_of(target)
            && self.widgets[index].insert_dropped_text(text, pos)
            && let Some(message) = self.widgets[index].take_change_message()
        {
            self.messages.push(message);
        }
        if let Some(message) = (self.drop_targets[position].2)(data, pos) {
            self.messages.push(message);
        }
        true
    }

    /// Resolve the release of the in-window fallback drag.
    fn finish_drag(&mut self, pos: Point, sink: &mut impl UiHost) -> bool {
        let Some(session) = self.drag.take() else {
            return false;
        };
        self.dragging = false;
        self.press = None;
        self.press_pos = None;
        if let Some(target) = session.target {
            self.notify_drag_hover(target, &session.data, pos, false, sink);
        }
        let target = self.drop_target_at(pos, session.source, &session.data);
        if let Some(target) = target {
            self.drop_on_target(target, session.data.clone(), pos);
        }
        // A drag is visual state on the source and target alike; repaint both.
        if let Some(index) = session.source.and_then(|id| self.index_of(id)) {
            sink.invalidate(self.bounds(index));
        }
        if let Some(target) = target
            && let Some(index) = self.index_of(target)
        {
            sink.invalidate(self.bounds(index));
        }
        true
    }

    // ---- System drag callbacks (forwarded by `Host`) ------------------------

    /// A system drag entered this window: route it to the widget under `pos`,
    /// show its hover feedback and report the effect it accepts. Called by
    /// `Host::drag_enter` (with `WindowHandle` as the sink).
    pub fn drag_enter(
        &mut self,
        data: &DragData,
        pos: Point,
        sink: &mut impl UiHost,
    ) -> DropEffect {
        let source = self.drag.as_ref().and_then(|drag| drag.source);
        self.drag = Some(DragSession {
            source,
            data: data.clone(),
            target: None,
            pos,
            system: true,
        });
        self.drag_over(data, pos, sink)
    }

    /// A system drag moved over this window. Only a target change repaints.
    pub fn drag_over(
        &mut self,
        data: &DragData,
        pos: Point,
        sink: &mut impl UiHost,
    ) -> DropEffect {
        let source = self.drag.as_ref().and_then(|drag| drag.source);
        let current = self.drag.as_ref().and_then(|drag| drag.target);
        if let Some(drag) = self.drag.as_mut() {
            drag.pos = pos;
            drag.data = data.clone();
        }
        let new_target = self
            .hit_index(pos)
            .map(|index| self.widgets[index].id())
            .filter(|id| Some(*id) != source);
        if new_target != current {
            if let Some(old) = current {
                self.notify_drag_hover(old, data, pos, false, sink);
            }
            if let Some(new) = new_target {
                self.notify_drag_hover(new, data, pos, true, sink);
            }
            if let Some(drag) = self.drag.as_mut() {
                drag.target = new_target;
            }
        }
        match self.drop_target_at(pos, source, data) {
            Some(_) => DropEffect::Copy,
            None => DropEffect::None,
        }
    }

    /// A system drag left this window. Clear hover feedback and the mirror
    /// state.
    pub fn drag_leave(&mut self, sink: &mut impl UiHost) {
        let Some(session) = self.drag.take() else {
            return;
        };
        if let Some(target) = session.target {
            self.notify_drag_hover(target, &session.data, session.pos, false, sink);
        }
    }

    /// A system drag was released over this window. Run the target's handler
    /// and report the effect.
    pub fn drag_drop(
        &mut self,
        data: DragData,
        pos: Point,
        sink: &mut impl UiHost,
    ) -> DropEffect {
        let source = self.drag.as_ref().and_then(|drag| drag.source);
        let current = self.drag.as_ref().and_then(|drag| drag.target);
        if let Some(target) = current {
            self.notify_drag_hover(target, &data, pos, false, sink);
        }
        self.drag = None;
        let target = self.drop_target_at(pos, source, &data);
        let consumed = target
            .map(|target| self.drop_on_target(target, data.clone(), pos))
            .unwrap_or(false);
        if let Some(target) = target
            && let Some(index) = self.index_of(target)
        {
            sink.invalidate(self.bounds(index));
        }
        if let Some(index) = source.and_then(|id| self.index_of(id)) {
            sink.invalidate(self.bounds(index));
        }
        if consumed {
            DropEffect::Copy
        } else {
            DropEffect::None
        }
    }

    /// The system drag this window started finished (dropped or cancelled).
    /// OLE consumed the pointer release, so clear the press/drag state and
    /// repaint the source. Called by `Host::drag_finished`.
    pub fn drag_finished(&mut self, sink: &mut impl UiHost) {
        self.dragging = false;
        self.press = None;
        self.press_pos = None;
        let Some(session) = self.drag.take() else {
            return;
        };
        if let Some(target) = session.target {
            self.notify_drag_hover(target, &session.data, session.pos, false, sink);
        }
        if let Some(index) = session.source.and_then(|id| self.index_of(id)) {
            sink.invalidate(self.bounds(index));
        }
    }

    /// Insert a widget directly below `anchor` in paint order (i.e. before it
    /// in the tree). An unknown anchor appends, like [`Ui::add`].
    pub fn insert_before(
        &mut self,
        anchor: WidgetId,
        widget: impl Widget<M> + 'static,
    ) -> &mut Self {
        let at = self.index_of(anchor).unwrap_or(self.widgets.len());
        self.insert(at, widget)
    }

    /// Move a widget to the top of the paint/hit-test order. Returns whether
    /// the tree changed (unknown ids and an already-topmost widget report
    /// `false`). Nothing is invalidated: z-order does not change pixels unless
    /// widgets overlap, so the caller repaints the affected union.
    pub fn raise(&mut self, id: WidgetId) -> bool {
        let Some(from) = self.index_of(id) else {
            return false;
        };
        let to = self.widgets.len() - 1;
        if from == to {
            return false;
        }
        self.move_widget(from, to);
        true
    }

    /// Move a widget to the bottom of the paint/hit-test order.
    pub fn lower(&mut self, id: WidgetId) -> bool {
        let Some(from) = self.index_of(id) else {
            return false;
        };
        if from == 0 {
            return false;
        }
        self.move_widget(from, 0);
        true
    }

    /// Remove a widget and everything the container stored for its id: focus
    /// (relocated to the next widget in the Tab order, with the lost hook
    /// run), hover/press/drag state, focus policy, cursor override,
    /// visibility, properties (including its context menu), trigger bindings,
    /// focus hooks and focus-scope membership. Returns whether a widget was
    /// removed.
    ///
    /// An open editor is closed the same way a focus loss closes it: a painted
    /// draft commits and its message is queued for [`Ui::drain_messages`], and
    /// a native child (only when it belongs to this widget) is ended with
    /// commit, so the text reaches `Host::edit_committed` after the widget is
    /// gone — an application that needs it reads it there, because
    /// [`Ui::commit_edit`] finds no widget for the id any more. The removed
    /// widget's rectangle (including an open overlay) is invalidated. The
    /// accessibility snapshot is not owned here: call
    /// `WindowHandle::a11y_refresh` when a screen reader should see the new
    /// tree.
    pub fn remove(&mut self, id: WidgetId, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let bounds_before = self.bounds(index);
        // The panel's bars are chrome over the whole viewport; removing a
        // child can hide the panel, so repaint the viewport too.
        let owner_viewport = self.scroll_owner(id).map(|(_, viewport)| viewport);
        if self.widgets[index].editing() {
            let message = self.widgets[index].finish_edit(true);
            if let Some(message) = message {
                self.messages.push(message);
            }
            sink.set_caret_rect(None);
        }
        if sink.editing_id() == Some(id) {
            sink.end_edit();
        }
        // The removed widget owned the pad-raised session, if any.
        self.end_soft_keyboard(sink);
        // Where focus goes: the widget at the same Tab-order position after the
        // removal (the next row slides up), not the next insertion-order
        // widget.
        let keyboard = self.focus_visible;
        let focus_position = (self.focus == Some(index)).then(|| {
            self.focus_order()
                .iter()
                .position(|candidate| *candidate == index)
        });
        if self.focus == Some(index) {
            // The removed widget still gets its lost hook; since it cannot
            // redirect anywhere useful, the outcome is ignored.
            let _ = self.run_focus_hook(id, false, FocusReason::Relocated);
            self.focus = None;
            self.adjusting = false;
        }
        // Drop the widget's drag registrations, and abort a drag that involves
        // it (D113).
        self.drag_sources.retain(|(candidate, _)| *candidate != id);
        self.text_sources.retain(|candidate| *candidate != id);
        self.drop_targets.retain(|(candidate, _, _)| *candidate != id);
        self.drop_hovers.retain(|(candidate, _)| *candidate != id);
        if self
            .drag
            .as_ref()
            .is_some_and(|drag| drag.source == Some(id) || drag.target == Some(id))
        {
            self.cancel_drag(sink);
        }
        self.widgets.remove(index);
        self.refresh_widget_index();
        self.invalidate_visible();
        self.shift_state_after_remove(index);
        for view in &mut self.scroll_views {
            view.forget(id);
        }
        self.refresh_scroll_children();
        self.focus_policies.retain(|(candidate, _)| *candidate != id);
        self.cursors.remove(&id);
        self.visibility.remove(&id);
        self.props.remove(&id);
        self.bindings.retain(|binding| binding.id != id);
        self.focus_hooks.retain(|hooks| hooks.id != id);
        // A fresh removal supersedes any captured session for the same id;
        // `remove_preserving_session` pushes its own after this call.
        self.pending_sessions.retain(|(pending, _)| *pending != id);
        if let Some(scope) = &mut self.focus_scope {
            scope.retain(|candidate| *candidate != id);
        }
        if let Some(Some(position)) = focus_position {
            let order = self.focus_order();
            if !order.is_empty() {
                let next = order[position.min(order.len() - 1)];
                self.focus_on(Some(next), sink, keyboard, FocusReason::Relocated);
            }
        }
        sink.invalidate(bounds_before);
        if let Some(viewport) = owner_viewport {
            sink.invalidate(viewport);
        }
        self.refresh_tooltip(sink);
        true
    }

    /// Rebuild the whole page in one pass: commit an open editor, drop the
    /// tree (page images survive, so re-registering covers is unnecessary),
    /// run `build` and invalidate `area` once. Returns the committed editor's
    /// message, if any.
    ///
    /// This is the sequence the application used to spell out by hand
    /// (`end_edit` → `clear` → build → invalidate). The a11y snapshot is
    /// refreshed by the host afterwards (`WindowHandle::a11y_refresh`).
    pub fn rebuild(
        &mut self,
        area: Rect,
        sink: &mut impl UiHost,
        build: impl FnOnce(&mut Self),
    ) -> Option<M> {
        let message = self.finish_inline_edit(true, sink);
        if sink.editing_id().is_some() {
            sink.end_edit();
            sink.set_caret_rect(None);
        }
        self.end_soft_keyboard(sink);
        let images = std::mem::take(&mut self.images);
        self.clear();
        self.images = images;
        build(self);
        sink.invalidate(area);
        self.refresh_tooltip(sink);
        message
    }

    /// Move a widget and repaint both the old and the new rectangle. Unlike
    /// the deliberately silent [`Ui::set_rect`] (layout passes assign many
    /// rects at once) this also keeps an open native editor aligned and
    /// refreshes the tooltip target.
    pub fn move_rect(&mut self, id: WidgetId, rect: Rect, sink: &mut impl UiHost) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let before = self.bounds(index);
        self.widgets[index].set_rect(rect);
        sink.invalidate(before.union(self.bounds(index)));
        if sink.editing_id() == Some(id) {
            sink.move_edit(rect);
        }
        self.refresh_tooltip(sink);
    }

    /// Shift the container's index-based pointer/focus state after removing a
    /// widget from the vector.
    fn shift_state_after_remove(&mut self, removed: usize) {
        fn shift(slot: &mut Option<usize>, removed: usize) {
            match *slot {
                Some(value) if value == removed => *slot = None,
                Some(value) if value > removed => *slot = Some(value - 1),
                _ => {}
            }
        }
        shift(&mut self.hover, removed);
        shift(&mut self.press, removed);
        shift(&mut self.focus, removed);
        shift(&mut self.right_press, removed);
    }

    /// Shift the container's index-based pointer/focus state after inserting a
    /// widget at `at`.
    fn shift_state_after_insert(&mut self, at: usize) {
        fn shift(slot: &mut Option<usize>, at: usize) {
            if let Some(value) = *slot
                && value >= at
            {
                *slot = Some(value + 1);
            }
        }
        shift(&mut self.hover, at);
        shift(&mut self.press, at);
        shift(&mut self.focus, at);
        shift(&mut self.right_press, at);
    }

    /// Move a widget position in the vector and follow the pointer/focus state
    /// that pointed at any widget between the two positions.
    fn move_widget(&mut self, from: usize, to: usize) {
        let widget = self.widgets.remove(from);
        self.widgets.insert(to, widget);
        fn remap(slot: &mut Option<usize>, from: usize, to: usize) {
            let Some(value) = *slot else {
                return;
            };
            let moved = if value == from {
                to
            } else if from < to && value > from && value <= to {
                value - 1
            } else if to < from && value >= to && value < from {
                value + 1
            } else {
                value
            };
            *slot = Some(moved);
        }
        remap(&mut self.hover, from, to);
        remap(&mut self.press, from, to);
        remap(&mut self.focus, from, to);
        remap(&mut self.right_press, from, to);
        self.refresh_widget_index();
        self.invalidate_visible();
    }

    /// Register (or replace) an image for this page. Widgets reference it by
    /// `ImageId` through `WidgetPaint::image`; replacing pixels under the same
    /// id keeps every reference valid. Nothing is invalidated: the caller
    /// repaints the widgets that show it.
    pub fn set_image(&mut self, image: crate::image::UiImage) {
        self.images.insert(image);
    }

    /// Drop an image. Widgets that still reference the id paint nothing (or
    /// their placeholder) until it is registered again.
    pub fn remove_image(&mut self, id: crate::backend::ImageId) -> Option<crate::image::UiImage> {
        self.images.remove(id)
    }

    pub fn image(&self, id: crate::backend::ImageId) -> Option<&crate::image::UiImage> {
        self.images.get(id)
    }

    pub fn images(&self) -> &crate::image::ImageStore {
        &self.images
    }

    // ---- Scroll views -------------------------------------------------------

    /// Register (or replace) a scrollable viewport over a set of child widgets.
    ///
    /// The children are laid out once, in *content* coordinates (the viewport's
    /// top-left corner is the origin); the container positions each of them at
    /// `content rect - offset`, clips them to the viewport and skips painting
    /// and hit-testing for the ones scrolled out. Call this again after a
    /// resize or a child-layout change — an application never moves a scrolled
    /// child itself. Re-registering the same panel keeps its live offset (call
    /// [`Ui::set_scroll_offset`] to change it).
    ///
    /// A panel is shown while at least one of its children is visible, so
    /// hiding the children (a page switch) hides the bars too. See D65.
    pub fn set_scroll_view(&mut self, mut view: ScrollView, sink: &mut impl UiHost) {
        let panel = match self.scroll_index(view.id()) {
            Some(index) => {
                view.set_offset(self.scroll_views[index].offset_of());
                self.scroll_views[index] = view;
                index
            }
            None => {
                self.scroll_views.push(view);
                self.scroll_views.len() - 1
            }
        };
        // The bar geometry changed with the viewport/children, so a drag in
        // flight has no valid thumb any more; drop it and clear the held look
        // too, or the bar stays "active" until the next release.
        if let Some(drag) = self.scroll_drag.take() {
            let state = self.scroll_views[drag.panel].bar_state_mut(drag.vertical);
            state.active = false;
            state.hovered = false;
        }
        self.refresh_scroll_children();
        self.scroll_shown_dirty = true;
        self.scroll_layout(panel, sink);
    }

    /// Read a registered panel (viewport, offset, content extent, axes and the
    /// children in content coordinates).
    pub fn scroll_view(&self, id: WidgetId) -> Option<&ScrollView> {
        self.scroll_index(id).map(|index| &self.scroll_views[index])
    }

    /// Registered panels in registration order; a later panel wins hit-testing
    /// where two viewports overlap.
    pub fn scroll_views(&self) -> impl Iterator<Item = &ScrollView> {
        self.scroll_views.iter()
    }

    /// Drop a panel. Its children keep their current rectangles (they are
    /// ordinary widgets again).
    pub fn remove_scroll_view(&mut self, id: WidgetId, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.scroll_index(id) else {
            return false;
        };
        let viewport = self.scroll_views[index].viewport();
        self.scroll_views.remove(index);
        // A nested panel whose parent went away becomes a root panel with its
        // configured viewport.
        for view in &mut self.scroll_views {
            if view.parent() == Some(id) {
                view.set_parent(None);
                let configured = view.configured_viewport();
                view.set_viewport(configured);
            }
        }
        self.scroll_drag = None;
        self.bar_hover = None;
        self.chrome_valid = false;
        self.refresh_scroll_children();
        self.scroll_shown_dirty = true;
        sink.invalidate(viewport);
        true
    }

    /// Current scroll offset of a panel.
    pub fn scroll_offset(&self, id: WidgetId) -> Option<Point> {
        self.scroll_index(id)
            .map(|index| self.scroll_views[index].offset_of())
    }

    /// Scroll a panel to `offset` (clamped). Returns whether it moved.
    pub fn set_scroll_offset(
        &mut self,
        id: WidgetId,
        offset: Point,
        sink: &mut impl UiHost,
    ) -> bool {
        let Some(index) = self.scroll_index(id) else {
            return false;
        };
        let before = self.scroll_views[index].offset_of();
        self.scroll_views[index].set_offset(offset);
        self.scroll_views[index].clamp();
        let moved = self.scroll_views[index].offset_of() != before;
        if moved {
            self.scroll_layout(index, sink);
        }
        moved
    }

    /// Scroll a panel by a delta (clamped). Returns whether it moved.
    pub fn scroll_by(&mut self, id: WidgetId, dx: f32, dy: f32, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.scroll_index(id) else {
            return false;
        };
        let before = self.scroll_views[index].offset_of();
        self.scroll_views[index].set_offset(Point::new(before.x + dx, before.y + dy));
        self.scroll_views[index].clamp();
        let moved = self.scroll_views[index].offset_of() != before;
        if moved {
            self.scroll_layout(index, sink);
        }
        moved
    }

    /// Scroll the panel that owns `child` so the child is fully visible.
    pub fn scroll_to_child(&mut self, child: WidgetId, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.index_of(child) else {
            return false;
        };
        self.reveal_chain(index, sink)
    }

    /// The panel that owns a widget, if any.
    pub fn scroll_owner_of(&self, child: WidgetId) -> Option<WidgetId> {
        self.scroll_owner(child)
            .map(|(panel, _)| self.scroll_views[panel].id())
    }

    /// Move/resize a panel's viewport (a window resize) and re-clamp.
    pub fn set_scroll_viewport(
        &mut self,
        id: WidgetId,
        viewport: Rect,
        sink: &mut impl UiHost,
    ) -> bool {
        let Some(index) = self.scroll_index(id) else {
            return false;
        };
        let before = self.scroll_views[index].viewport();
        self.scroll_views[index].set_viewport(viewport);
        self.scroll_layout(index, sink);
        if before != viewport {
            sink.invalidate(before);
        }
        true
    }

    fn scroll_index(&self, id: WidgetId) -> Option<usize> {
        self.scroll_views
            .iter()
            .position(|view| view.id() == id)
    }

    /// The panel and its viewport that own a widget. The viewport is the clip
    /// rectangle: content is painted, hit-tested and reported inside the whole
    /// viewport (it may slide under an overlay bar mid-scroll), while the
    /// *range* ends with the padding gap — see `ScrollView::content_rect`.
    fn scroll_owner(&self, id: WidgetId) -> Option<(usize, Rect)> {
        let panel = *self.scroll_children.get(&id)?;
        Some((panel, self.scroll_views[panel].clip_rect()))
    }

    /// Panel visibility is derived: a panel shows while at least one of its
    /// children is visible. The result is cached; pointer moves and paints
    /// read it many times per event.
    fn scroll_shown(&self, panel: usize) -> bool {
        if !self.scroll_shown_dirty {
            return self.scroll_shown_cache.get(panel).copied().unwrap_or(false);
        }
        self.scroll_shown_at(panel, 0)
    }

    fn ensure_scroll_shown(&mut self) {
        if !self.scroll_shown_dirty {
            return;
        }
        self.scroll_shown_dirty = false;
        let cache: Vec<bool> = (0..self.scroll_views.len())
            .map(|panel| self.scroll_shown_at(panel, 0))
            .collect();
        self.scroll_shown_cache = cache;
    }

    fn scroll_shown_at(&self, panel: usize, depth: usize) -> bool {
        // Visibility is "this panel's subtree has visible content". A page
        // whose only content is a nested panel is shown exactly when that
        // panel is; no ancestor walk is needed, because hiding a page hides
        // its widgets and therefore the nested content too.
        if depth > 32 {
            return false;
        }
        let own = self.scroll_views[panel].children_ref().iter().any(|child| {
            self.index_of(child.id)
                .map(|index| self.effective_visible(index))
                .unwrap_or(false)
        });
        if own {
            return true;
        }
        let id = self.scroll_views[panel].id();
        self.scroll_views.iter().enumerate().any(|(index, view)| {
            index != panel
                && view.parent() == Some(id)
                && self.scroll_shown_at(index, depth + 1)
        })
    }

    /// Resolve the content extent, clamp the offset, position every child at
    /// `viewport origin + content rect - offset` and repaint the viewport.
    fn scroll_layout(&mut self, panel: usize, sink: &mut impl UiHost) {
        self.scroll_resolve_viewport(panel, 0);
        self.scroll_views[panel].resolve();
        self.scroll_views[panel].clamp();
        let offset = self.scroll_views[panel].offset_of();
        let area = self.scroll_views[panel].content_rect();
        let count = self.scroll_views[panel].children_ref().len();
        for position in 0..count {
            let child = self.scroll_views[panel].children_ref()[position];
            if let Some(index) = self.index_of(child.id) {
                let rect = child
                    .rect
                    .offset_into(area, Point::new(-offset.x, -offset.y));
                self.widgets[index].set_rect(rect);
            }
        }
        let id = self.scroll_views[panel].id();
        let nested: Vec<usize> = self
            .scroll_views
            .iter()
            .enumerate()
            .filter(|(index, view)| *index != panel && view.parent() == Some(id))
            .map(|(index, _)| index)
            .collect();
        for child in nested {
            self.scroll_layout(child, sink);
        }
        sink.invalidate(self.scroll_views[panel].viewport());
    }

    fn scroll_resolve_viewport(&mut self, panel: usize, depth: usize) {
        let parent = self.scroll_views[panel]
            .parent()
            .and_then(|id| self.scroll_index(id))
            .filter(|parent| *parent != panel && depth < 32);
        let Some(parent) = parent else {
            let configured = self.scroll_views[panel].configured_viewport();
            self.scroll_views[panel].set_effective(configured, configured);
            return;
        };
        self.scroll_resolve_viewport(parent, depth + 1);
        let (area, offset, parent_clip) = {
            let view = &self.scroll_views[parent];
            (view.content_rect(), view.offset_of(), view.clip_rect())
        };
        let configured = self.scroll_views[panel].configured_viewport();
        let effective = configured.offset_into(area, Point::new(-offset.x, -offset.y));
        let clip = effective
            .intersect(parent_clip)
            .unwrap_or(Rect::new(effective.left, effective.top, effective.left, effective.top));
        self.scroll_views[panel].set_effective(effective, clip);
    }

    fn scroll_depth(&self, panel: usize) -> usize {
        let mut depth = 0;
        let mut current = self.scroll_views[panel].parent();
        while let Some(id) = current {
            let Some(index) = self.scroll_index(id) else {
                break;
            };
            depth += 1;
            current = self.scroll_views[index].parent();
            if depth > 32 {
                break;
            }
        }
        depth
    }

    /// Set one axis of a panel's offset and re-lay it out when it moved.
    fn scroll_axis(
        &mut self,
        panel: usize,
        vertical: bool,
        value: f32,
        sink: &mut impl UiHost,
    ) -> bool {
        let before = self.scroll_views[panel].offset_of();
        let mut offset = before;
        if vertical {
            offset.y = value;
        } else {
            offset.x = value;
        }
        self.scroll_views[panel].set_offset(offset);
        self.scroll_views[panel].clamp();
        if self.scroll_views[panel].offset_of() == before {
            return false;
        }
        self.scroll_layout(panel, sink);
        true
    }

    /// Which panel bar is under `pos` (thumb or track).
    fn scroll_bar_at(&self, pos: Point) -> Option<(usize, bool, BarHit)> {
        for panel in (0..self.scroll_views.len()).rev() {
            if !self.scroll_shown(panel) {
                continue;
            }
            let view = &self.scroll_views[panel];
            if !view.clip_rect().contains_point(pos) {
                continue;
            }
            for vertical in [true, false] {
                if let Some(bar) = scroll::bar(view, vertical)
                    && let Some(hit) = scrollbar::hit(bar.track, Some(bar.thumb), pos)
                {
                    return Some((panel, vertical, hit));
                }
            }
        }
        None
    }

    /// Whether `pos` lands in a panel's bar junction (the masked square where a
    /// vertical and a horizontal bar meet): container chrome, not content.
    fn scroll_corner_at(&self, pos: Point) -> bool {
        (0..self.scroll_views.len()).rev().any(|panel| {
            self.scroll_shown(panel)
                && scroll::corner_rect(&self.scroll_views[panel])
                    .is_some_and(|corner| corner.contains_point(pos))
        })
    }

    /// One scan for both chrome kinds: the bar under `pos` and whether the
    /// point is in a bar junction. The pointer path shares the result with bar
    /// hover and the cursor instead of rescanning every panel three times.
    fn scroll_chrome_at(&self, pos: Point) -> (Option<(usize, bool)>, bool) {
        let mut bar_hover = None;
        let mut corner = false;
        for panel in (0..self.scroll_views.len()).rev() {
            if !self.scroll_shown(panel) {
                continue;
            }
            let view = &self.scroll_views[panel];
            if !view.clip_rect().contains_point(pos) {
                continue;
            }
            if !corner
                && scroll::corner_rect(view).is_some_and(|corner| corner.contains_point(pos))
            {
                corner = true;
            }
            if bar_hover.is_none() {
                for vertical in [true, false] {
                    if let Some(bar) = scroll::bar(view, vertical)
                        && scrollbar::hit(bar.track, Some(bar.thumb), pos).is_some()
                    {
                        bar_hover = Some((panel, vertical));
                        break;
                    }
                }
            }
            if bar_hover.is_some() && corner {
                break;
            }
        }
        (bar_hover, corner)
    }

    /// Set one bar's hover flag and repaint its track when it changed.
    fn set_bar_hover(
        &mut self,
        panel: usize,
        vertical: bool,
        on: bool,
        sink: &mut impl UiHost,
    ) {
        if self.scroll_views[panel].bar_state(vertical).hovered == on {
            return;
        }
        self.scroll_views[panel].bar_state_mut(vertical).hovered = on;
        if let Some(bar) = scroll::bar(&self.scroll_views[panel], vertical) {
            sink.invalidate(bar.track);
        }
    }

    /// Move bar hover to `target` (at most the previous and the new bar are
    /// touched; only one bar can be hovered at a time).
    fn scroll_hover_to(&mut self, target: Option<(usize, bool)>, sink: &mut impl UiHost) {
        let old = self.bar_hover;
        if old == target {
            return;
        }
        if let Some((panel, vertical)) = old {
            self.set_bar_hover(panel, vertical, false, sink);
        }
        if let Some((panel, vertical)) = target {
            self.set_bar_hover(panel, vertical, true, sink);
        }
        self.bar_hover = target;
    }

    /// Track bar hover for every panel; repaints a track whose state changed.
    fn scroll_hover(&mut self, pos: Option<Point>, sink: &mut impl UiHost) -> bool {
        let hovered = pos.and_then(|pos| {
            self.scroll_bar_at(pos)
                .map(|(panel, vertical, _)| (panel, vertical))
        });
        let mut changed = false;
        for panel in 0..self.scroll_views.len() {
            if !self.scroll_shown(panel) {
                continue;
            }
            for vertical in [true, false] {
                let on = hovered == Some((panel, vertical));
                if self.scroll_views[panel].bar_state(vertical).hovered == on {
                    continue;
                }
                self.scroll_views[panel].bar_state_mut(vertical).hovered = on;
                if let Some(bar) = scroll::bar(&self.scroll_views[panel], vertical) {
                    sink.invalidate(bar.track);
                }
                changed = true;
            }
        }
        changed
    }

    /// Clear bar hover/active on every panel (pointer leave, page switch).
    fn scroll_release_bars(&mut self, sink: &mut impl UiHost) -> bool {
        self.scroll_drag = None;
        let mut changed = false;
        for panel in 0..self.scroll_views.len() {
            for vertical in [true, false] {
                let state = *self.scroll_views[panel].bar_state(vertical);
                if !state.hovered && !state.active {
                    continue;
                }
                if let Some(bar) = scroll::bar(&self.scroll_views[panel], vertical) {
                    sink.invalidate(bar.track);
                }
                let state = self.scroll_views[panel].bar_state_mut(vertical);
                state.hovered = false;
                state.active = false;
                changed = true;
            }
        }
        changed
    }

    /// A press on a bar: start a thumb drag or jump/page the track. Returns
    /// whether the press was consumed (it never reaches a widget).
    fn scroll_press(&mut self, pos: Point, sink: &mut impl UiHost) -> bool {
        let Some((panel, vertical, hit)) = self.scroll_bar_at(pos) else {
            return false;
        };
        let Some(bar) = scroll::bar(&self.scroll_views[panel], vertical) else {
            return false;
        };
        let view = &self.scroll_views[panel];
        // The drag and track-jump math must map to the same visible size the
        // clamp uses (the content area, not the viewport), or the pointer can
        // only reach part of the scroll range and the bar stops short.
        let area = view.content_rect();
        let (content, visible, offset) = if vertical {
            (view.content_size().height, area.height(), view.offset_of().y)
        } else {
            (view.content_size().width, area.width(), view.offset_of().x)
        };
        let style = *view.style_ref();
        match hit {
            BarHit::Thumb => {
                let grab = if vertical {
                    pos.y - bar.thumb.top
                } else {
                    pos.x - bar.thumb.left
                };
                self.scroll_drag = Some(ScrollDrag {
                    panel,
                    vertical,
                    grab,
                });
            }
            BarHit::Track => {
                let next = scrollbar::track_click_offset(
                    pos, bar.track, bar.thumb, content, visible, offset, vertical, &style,
                );
                self.scroll_axis(panel, vertical, next, sink);
                // A track press also starts a drag, grabbing the thumb's
                // centre at its new place: holding and moving keeps scrolling
                // and the release clears the active state (a track press with
                // no drag state used to leave the thumb stuck "held").
                let grab = if vertical {
                    bar.thumb.height() * 0.5
                } else {
                    bar.thumb.width() * 0.5
                };
                self.scroll_drag = Some(ScrollDrag {
                    panel,
                    vertical,
                    grab,
                });
            }
        }
        let state = self.scroll_views[panel].bar_state_mut(vertical);
        state.hovered = true;
        state.active = true;
        if let Some(bar) = scroll::bar(&self.scroll_views[panel], vertical) {
            sink.invalidate(bar.track);
        }
        self.press = None;
        self.press_pos = None;
        self.dragging = false;
        true
    }

    /// Continue a thumb drag; keeps the press grab offset.
    fn scroll_drag_to(&mut self, pos: Point, sink: &mut impl UiHost) -> bool {
        let Some(drag) = self.scroll_drag else {
            return false;
        };
        let Some(bar) = scroll::bar(&self.scroll_views[drag.panel], drag.vertical) else {
            return false;
        };
        let view = &self.scroll_views[drag.panel];
        let area = view.content_rect();
        let (content, visible) = if drag.vertical {
            (view.content_size().height, area.height())
        } else {
            (view.content_size().width, area.width())
        };
        let next = scrollbar::drag_offset(
            pos,
            bar.track,
            bar.thumb,
            drag.grab,
            content,
            visible,
            drag.vertical,
        );
        self.scroll_axis(drag.panel, drag.vertical, next, sink);
        true
    }

    /// End a bar press or drag (hover is re-evaluated at the release position).
    /// Any bar still marked active is cleared, so a press can never leave a
    /// thumb stuck in the held state.
    fn scroll_release(&mut self, pos: Point, sink: &mut impl UiHost) -> bool {
        let drag = self.scroll_drag.take();
        let mut released = drag.is_some();
        for panel in 0..self.scroll_views.len() {
            for vertical in [true, false] {
                if !self.scroll_views[panel].bar_state(vertical).active {
                    continue;
                }
                if drag.is_some_and(|drag| drag.panel == panel && drag.vertical == vertical) {
                    // Repainted below, after the hover update.
                    self.scroll_views[panel].bar_state_mut(vertical).active = false;
                    continue;
                }
                self.scroll_views[panel].bar_state_mut(vertical).active = false;
                if let Some(bar) = scroll::bar(&self.scroll_views[panel], vertical) {
                    sink.invalidate(bar.track);
                }
                released = true;
            }
        }
        if let Some(drag) = drag {
            if let Some(bar) = scroll::bar(&self.scroll_views[drag.panel], drag.vertical) {
                sink.invalidate(bar.track);
            }
        }
        self.scroll_hover(Some(pos), sink);
        released
    }

    /// Scroll the focused widget's panel so the widget is fully visible. Tab
    /// order visits scrolled-out children (a panel's content is a page, not a
    /// virtualized list), so focusing one has to bring it into view.
    fn reveal_focused(&mut self, sink: &mut impl UiHost) {
        if let Some(index) = self.focus {
            self.reveal_chain(index, sink);
        }
    }

    /// Scroll every panel on the widget's ancestor chain just enough to show
    /// it: the innermost first, then outwards (a nested list inside a page
    /// reveals in both, D92). Returns whether any panel moved.
    fn reveal_chain(&mut self, index: usize, sink: &mut impl UiHost) -> bool {
        let id = self.widgets[index].id();
        let mut panel = self.scroll_owner(id).map(|(panel, _)| panel);
        let mut moved = false;
        let mut depth = 0;
        while let Some(current) = panel {
            let rect = self.widgets[index].rect();
            if self.scroll_views[current].reveal(rect) {
                self.scroll_layout(current, sink);
                moved = true;
            }
            panel = self.scroll_views[current]
                .parent()
                .and_then(|id| self.scroll_index(id));
            depth += 1;
            if depth > 32 {
                break;
            }
        }
        moved
    }

    /// Bind a high-level interaction for a widget. The handler may emit a
    /// message and/or call `ctx.default()` to keep the widget's own behavior.
    /// Binding the same (widget, trigger) pair again replaces the handler.
    pub fn bind(
        &mut self,
        id: WidgetId,
        trigger: Trigger,
        handler: impl FnMut(TriggerEvent, &mut TriggerContext<'_, M>) -> Option<M> + 'static,
    ) -> &mut Self {
        if self.index_of(id).is_none() {
            return self;
        }
        self.unbind(id, trigger);
        self.bindings.push(Binding {
            id,
            trigger,
            handler: Box::new(handler),
        });
        self
    }

    /// Shorthand for [`Ui::bind`] with [`Trigger::Click`].
    pub fn on_click(
        &mut self,
        id: WidgetId,
        handler: impl FnMut(TriggerEvent, &mut TriggerContext<'_, M>) -> Option<M> + 'static,
    ) -> &mut Self {
        self.bind(id, Trigger::Click, handler)
    }

    /// Shorthand for [`Ui::bind`] with [`Trigger::Activate`].
    pub fn on_activate(
        &mut self,
        id: WidgetId,
        handler: impl FnMut(TriggerEvent, &mut TriggerContext<'_, M>) -> Option<M> + 'static,
    ) -> &mut Self {
        self.bind(id, Trigger::Activate, handler)
    }

    pub fn unbind(&mut self, id: WidgetId, trigger: Trigger) {
        self.bindings
            .retain(|binding| binding.id != id || binding.trigger != trigger);
    }

    /// Called with focus moving onto `id`. Runs after the widget has focus;
    /// returning [`FocusOutcome::Cancel`] reverts the move.
    pub fn on_focus_gained(
        &mut self,
        id: WidgetId,
        handler: impl FnMut(FocusEvent) -> FocusOutcome + 'static,
    ) -> &mut Self {
        if self.index_of(id).is_some() {
            self.hooks_mut(id).gained = Some(Box::new(handler));
        }
        self
    }

    /// Called with focus leaving `id`, after an open native editor commits.
    /// `Cancel` aborts the move; `Redirect` focuses another widget instead.
    pub fn on_focus_lost(
        &mut self,
        id: WidgetId,
        handler: impl FnMut(FocusEvent) -> FocusOutcome + 'static,
    ) -> &mut Self {
        if self.index_of(id).is_some() {
            self.hooks_mut(id).lost = Some(Box::new(handler));
        }
        self
    }

    fn hooks_mut(&mut self, id: WidgetId) -> &mut FocusHooks {
        if let Some(position) = self.focus_hooks.iter().position(|hooks| hooks.id == id) {
            return &mut self.focus_hooks[position];
        }
        self.focus_hooks.push(FocusHooks {
            id,
            gained: None,
            lost: None,
        });
        let last = self.focus_hooks.len() - 1;
        &mut self.focus_hooks[last]
    }

    pub fn len(&self) -> usize {
        self.widgets.len()
    }

    /// Everything the widget paints, including an open overlay. Invalidation
    /// uses this so a combo drop-down outside the layout rect is repainted. A
    /// widget inside a scroll view is clipped to its viewport, so only that
    /// part can change on screen.
    fn bounds(&self, index: usize) -> Rect {
        let bounds = self.widgets[index].paint_bounds();
        match self.scroll_owner(self.widgets[index].id()) {
            Some((_, viewport)) => bounds.intersect(viewport).unwrap_or(Rect::new(
                bounds.left,
                bounds.top,
                bounds.left,
                bounds.top,
            )),
            None => bounds,
        }
    }

    fn in_scope(&self, index: usize) -> bool {
        match &self.focus_scope {
            Some(scope) => scope.contains(&self.widgets[index].id()),
            None => true,
        }
    }

    /// Confine keyboard navigation to `scope` (an in-window modal shell).
    /// `None` removes the trap. When the focused widget falls outside the new
    /// scope, focus moves to the first widget inside it.
    pub fn set_focus_scope(&mut self, scope: Option<Vec<WidgetId>>, sink: &mut impl UiHost) {
        self.focus_scope = scope;
        if let Some(index) = self.focus
            && (!self.in_scope(index) || !self.effective_visible(index))
        {
            let next = self.focus_order().first().copied();
            self.focus_on(next, sink, true, FocusReason::Programmatic);
        }
    }

    pub fn focus_scope(&self) -> Option<&[WidgetId]> {
        self.focus_scope.as_deref()
    }

    fn index_of(&self, id: WidgetId) -> Option<usize> {
        self.widget_index.get(&id).copied()
    }

    /// Rebuild the id → index map after an insertion, removal or reorder.
    fn refresh_widget_index(&mut self) {
        self.widget_index.clear();
        for (index, widget) in self.widgets.iter().enumerate() {
            self.widget_index.entry(widget.id()).or_insert(index);
        }
    }

    /// Rebuild the scroll-child lookup after a panel changes. The first panel
    /// listing a child owns it, exactly like the old linear `.position` scan.
    fn refresh_scroll_children(&mut self) {
        self.scroll_children.clear();
        for (panel, view) in self.scroll_views.iter().enumerate() {
            for child in view.children_ref() {
                self.scroll_children.entry(child.id).or_insert(panel);
            }
        }
    }

    /// The visible set changed (page switch, show/hide, add/remove).
    fn invalidate_visible(&mut self) {
        self.visible_dirty = true;
        self.scroll_shown_dirty = true;
    }

    /// Rebuild the visible index list once per visibility change. A page
    /// switch hides every other page's widgets in one pass; without this
    /// cache every pointer move (and every paint) would still visit all of
    /// them through `effective_visible`.
    fn ensure_visible(&mut self) {
        if !self.visible_dirty {
            return;
        }
        self.visible_dirty = false;
        let list: Vec<usize> = (0..self.widgets.len())
            .filter(|index| self.effective_visible(*index))
            .collect();
        self.visible_indices = list;
    }

    fn policy_mut(&mut self, id: WidgetId) -> &mut FocusPolicy {
        if let Some(position) = self
            .focus_policies
            .iter()
            .position(|(candidate, _)| *candidate == id)
        {
            return &mut self.focus_policies[position].1;
        }
        self.focus_policies
            .push((id, FocusPolicy { weight: 0, focusable: None }));
        let last = self.focus_policies.len() - 1;
        &mut self.focus_policies[last].1
    }

    fn focus_weight(&self, index: usize) -> i32 {
        let id = self.widgets[index].id();
        self.focus_policies
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .map(|(_, policy)| policy.weight)
            .unwrap_or(0)
    }

    /// Effective focusability: the per-instance override when set, otherwise
    /// the widget type's own default.
    fn effective_focusable(&self, index: usize) -> bool {
        let widget = &self.widgets[index];
        self.focus_policies
            .iter()
            .find(|(candidate, _)| *candidate == widget.id())
            .and_then(|(_, policy)| policy.focusable)
            .unwrap_or_else(|| widget.focusable())
    }

    /// Container visibility: the per-instance override, otherwise the
    /// widget's own state (Toast keeps one).
    fn effective_visible(&self, index: usize) -> bool {
        let widget = &self.widgets[index];
        self.visibility
            .get(&widget.id())
            .copied()
            .unwrap_or_else(|| widget.visible())
    }

    /// Clear hover/press state that pointed at a widget that just became
    /// hidden or disabled.
    fn release_pointer(&mut self, index: usize) {
        if self.hover == Some(index) {
            self.hover = None;
            let _ = self.widgets[index].input(WidgetInput::HoverLeave);
        }
        if self.press == Some(index) {
            self.press = None;
            self.press_pos = None;
            self.dragging = false;
        }
        if self.right_press == Some(index) {
            self.right_press = None;
        }
    }

    /// Focus target after `index` stopped being eligible: the next widget in
    /// the new order, wrapping to the first.
    fn next_focus(&self, before: &[usize], index: usize) -> Option<usize> {
        let position = before.iter().position(|candidate| *candidate == index);
        let order = self.focus_order();
        if order.is_empty() {
            return None;
        }
        match position {
            Some(position) => Some(order[position.min(order.len() - 1)]),
            None => Some(order[0]),
        }
    }

    /// Stack widgets vertically in insertion order using their preferred
    /// heights. Only called on layout changes.
    pub fn layout_vertical(&mut self, area: Rect, gap: f32) {
        let mut stack = VStack::new(area, gap);
        for widget in &mut self.widgets {
            let height = widget.preferred_height(area.width());
            widget.set_rect(stack.next(height));
        }
    }

    pub fn set_rect(&mut self, id: WidgetId, rect: Rect) {
        if let Some(index) = self.index_of(id) {
            self.widgets[index].set_rect(rect);
        }
    }

    pub fn rect(&self, id: WidgetId) -> Option<Rect> {
        self.widgets
            .iter()
            .find(|widget| widget.id() == id)
            .map(|widget| widget.rect())
    }

    /// Read a widget through its concrete type:
    /// `ui.with_widget::<ListView<Msg>, _>(id, |list| list.visible_rows())`.
    /// Returns `None` when the id is unknown or the widget is not a `W`.
    pub fn with_widget<W, R>(&self, id: WidgetId, f: impl FnOnce(&W) -> R) -> Option<R>
    where
        W: Widget<M> + 'static,
    {
        let index = self.index_of(id)?;
        let widget = self.widgets[index].as_any().downcast_ref::<W>()?;
        Some(f(widget))
    }

    /// Modify a widget through its concrete type (its own display-property
    /// setters: a label's text, a picker's color, a transport's flags). The
    /// closure reports whether anything changed; a change records the widget's
    /// rectangle (before and after, so a setter that moves an overlay repaints
    /// both) as pending damage instead of invalidating at once. The host hands
    /// that damage to the window with [`Ui::flush_damage`] at the end of the
    /// event, so editing several widgets in a row costs one repaint request
    /// (D120, superseding D28's immediate invalidation).
    ///
    /// The caller marks the a11y snapshot stale (`WindowHandle::a11y_refresh`)
    /// when the change is visible to a screen reader, because this seam cannot
    /// know that from a boolean.
    ///
    /// Returns false when the id is unknown, the widget is not a `W`, or the
    /// closure reported no change.
    pub fn edit_widget<W>(&mut self, id: WidgetId, f: impl FnOnce(&mut W) -> bool) -> bool
    where
        W: Widget<M> + 'static,
    {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let bounds_before = self.bounds(index);
        let was_visible = self.widgets[index].visible();
        let changed = {
            let Some(widget) = self.widgets[index].as_any_mut().downcast_mut::<W>() else {
                return false;
            };
            f(widget)
        };
        if self.widgets[index].visible() != was_visible {
            // A typed-access setter can hide a widget directly; the visible
            // index list must not keep it.
            self.invalidate_visible();
        }
        if changed {
            self.mark_damage_rect(bounds_before.union(self.bounds(index)));
        }
        changed
    }

    pub fn set_enabled(&mut self, id: WidgetId, enabled: bool, sink: &mut impl UiHost) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        if self.widgets[index].enabled() == enabled {
            return;
        }
        let bounds_before = self.widgets[index].paint_bounds();
        let before = (!enabled && self.focus == Some(index)).then(|| self.focus_order());
        self.widgets[index].set_enabled(enabled);
        if !enabled {
            // A disabled widget must not keep an overlay (drop-down, flyout)
            // open, and its old rectangle must be repainted.
            self.widgets[index].dismiss();
            self.release_pointer(index);
        }
        if let Some(before) = before {
            let keyboard = self.focus_visible;
            let next = self.next_focus(&before, index);
            self.focus_on(next, sink, keyboard, FocusReason::Relocated);
        }
        if self.focus == Some(index) {
            // A focus hook vetoed relocation; a hidden/disabled widget must
            // not keep keyboard focus.
            self.focus = None;
            self.adjusting = false;
        }
        // The disabled look is a content change like any other: record it and
        // let the host flush it with the rest of the batch.
        self.mark_damage_rect(bounds_before.union(self.widgets[index].paint_bounds()));
        self.refresh_tooltip(sink);
    }

    /// Set a widget's per-instance focusability. Unlike the widget's own
    /// default this can change at runtime; disabling it relocates focus when
    /// the widget owned it.
    pub fn set_focusable(&mut self, id: WidgetId, focusable: bool, sink: &mut impl UiHost) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let before = (!focusable && self.focus == Some(index)).then(|| self.focus_order());
        self.policy_mut(id).focusable = Some(focusable);
        if let Some(before) = before {
            let keyboard = self.focus_visible;
            let next = self.next_focus(&before, index);
            self.focus_on(next, sink, keyboard, FocusReason::Relocated);
        }
        if self.focus == Some(index) {
            self.focus = None;
            self.adjusting = false;
        }
        self.refresh_tooltip(sink);
    }

    /// Tab-order weight; smaller values come first and equal weights keep
    /// insertion order. Default `0`.
    pub fn set_focus_order(&mut self, id: WidgetId, weight: i32) {
        if self.index_of(id).is_some() {
            self.policy_mut(id).weight = weight;
        }
    }

    /// Apply properties to an existing widget. Unlike [`Ui::add_with`] this
    /// runs on a live page, so making the focused widget invisible or
    /// non-focusable relocates focus and both rectangles are repainted.
    pub fn set_props(&mut self, id: WidgetId, props: WidgetProps, sink: &mut impl UiHost) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let bounds_before = self.widgets[index].paint_bounds();
        let order_before = self.focus_order();
        let eligible_before = self.effective_eligible(index);
        self.apply_props(id, props);
        let eligible = self.effective_eligible(index);
        if !self.effective_visible(index) || !self.widgets[index].enabled() {
            self.widgets[index].dismiss();
            self.release_pointer(index);
        }
        if eligible_before && !eligible && self.focus == Some(index) {
            let keyboard = self.focus_visible;
            let next = self.next_focus(&order_before, index);
            self.focus_on(next, sink, keyboard, FocusReason::Relocated);
        }
        if self.focus == Some(index) && !eligible {
            self.focus = None;
            self.adjusting = false;
        }
        // Palette/font/visibility props change what the widget paints; record
        // the rectangle and leave the repaint to the batch flush.
        self.mark_damage_rect(bounds_before.union(self.widgets[index].paint_bounds()));
        self.refresh_tooltip(sink);
    }

    /// Record properties in the container state. Focus policy, cursor and
    /// visibility are folded into the dedicated tables so the rest of the
    /// container keeps reading one source of truth.
    fn apply_props(&mut self, id: WidgetId, props: WidgetProps) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        // A composite widget that edits its contents itself needs the flag as
        // well: the container only gates the editors it opens. `None` leaves
        // the widget's own state alone (like `visible`/`enabled` below).
        if let Some(read_only) = props.read_only {
            self.widgets[index].set_read_only(read_only);
        }
        if let Some(focusable) = props.focusable {
            self.policy_mut(id).focusable = Some(focusable);
        }
        if let Some(weight) = props.focus_order {
            self.policy_mut(id).weight = weight;
        }
        if let Some(cursor) = props.cursor {
            self.set_cursor(id, Some(cursor));
        }
        if let Some(visible) = props.visible {
            self.visibility.insert(id, visible);
            self.widgets[index].set_visible(visible);
            self.invalidate_visible();
        }
        if let Some(enabled) = props.enabled {
            self.widgets[index].set_enabled(enabled);
        }
        self.props.insert(id, props);
    }

    fn props_for(&self, id: WidgetId) -> Option<&WidgetProps> {
        self.props.get(&id)
    }

    fn props_at(&self, index: usize) -> Option<&WidgetProps> {
        self.props_for(self.widgets[index].id())
    }

    /// Eligible for keyboard focus: visible, enabled and focusable.
    fn effective_eligible(&self, index: usize) -> bool {
        self.effective_visible(index)
            && self.widgets[index].enabled()
            && self.effective_focusable(index)
    }

    /// Per-instance mouse opt-in for widgets that are display-only by default.
    fn effective_interactive(&self, index: usize) -> bool {
        self.props_at(index)
            .and_then(|props| props.interactive)
            .unwrap_or_else(|| self.widgets[index].interactive())
    }

    /// Per-instance read-only state; editors do not open a native session.
    /// A widget that edits its contents in place can declare itself read-only
    /// too (`Widget::read_only`), so the property is the union of both.
    fn effective_read_only(&self, index: usize) -> bool {
        self.widgets[index].read_only()
            || self
                .props_at(index)
                .and_then(|props| props.read_only)
                .unwrap_or(false)
    }

    /// Narration for a widget, if the application installed one.
    pub fn narration(&self, id: WidgetId) -> Option<&Narration> {
        self.props_for(id).and_then(|props| props.narration.as_ref())
    }

    // -----------------------------------------------------------------------
    // Layout-skin seams (`layout-xml` / `layout-compiled`, M7). The applier lives in
    // `crate::layout_xml`; these keep its access to the container state
    // explicit instead of downcasting widgets.
    // -----------------------------------------------------------------------

    /// The installed layout, for the debug form and tests.
    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub fn installed_layout(&self) -> Option<&crate::layout_xml::AppliedLayout> {
        self.xml_layout.as_ref()
    }

    /// Install the widget-type registry a layout document creates widgets from
    /// (M7.7, [`crate::creation`]). Replacing it affects the next apply;
    /// document-owned widgets already on screen stay until a reload removes
    /// them.
    pub fn set_widget_registry(&mut self, registry: crate::creation::WidgetRegistry<M>) {
        self.widget_registry = Some(registry);
    }

    /// The installed creation registry, if any.
    pub fn widget_registry(&self) -> Option<&crate::creation::WidgetRegistry<M>> {
        self.widget_registry.as_ref()
    }

    /// Mutate the installed registry (register a type after the first apply).
    pub fn widget_registry_mut(&mut self) -> Option<&mut crate::creation::WidgetRegistry<M>> {
        self.widget_registry.as_mut()
    }

    /// Drop the registry: subsequent `type` entries are `xml.type.unknown`.
    pub fn clear_widget_registry(&mut self) {
        self.widget_registry = None;
    }

    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn props_snapshot(&self, id: WidgetId) -> Option<WidgetProps> {
        self.props_for(id).cloned()
    }

    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn visibility_override(&self, id: WidgetId) -> Option<bool> {
        self.visibility.get(&id).copied()
    }

    /// Restore "the application owns visibility": apply `own` to the widget and
    /// drop the container override, so `Widget::visible` is the source again.
    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn clear_visibility_override(
        &mut self,
        id: WidgetId,
        own: bool,
        sink: &mut impl UiHost,
    ) {
        self.set_visible(id, own, sink);
        self.visibility.remove(&id);
    }

    /// The widget's own visible/enabled state (the container override is
    /// separate, see [`Ui::visibility_override`]).
    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn widget_flags(&self, id: WidgetId) -> Option<(bool, bool)> {
        let index = self.index_of(id)?;
        Some((
            self.widgets[index].visible(),
            self.widgets[index].enabled(),
        ))
    }

    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn focus_policy_of(&self, id: WidgetId) -> (i32, Option<bool>) {
        self.focus_policies
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .map(|(_, policy)| (policy.weight, policy.focusable))
            .unwrap_or((0, None))
    }

    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn set_focus_policy(
        &mut self,
        id: WidgetId,
        focusable: Option<bool>,
        weight: i32,
    ) {
        if self.index_of(id).is_none() {
            return;
        }
        let policy = self.policy_mut(id);
        policy.focusable = focusable;
        policy.weight = weight;
    }

    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn cursor_override_of(&self, id: WidgetId) -> Option<Cursor> {
        self.cursors.get(&id).copied()
    }

    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn content_caps_of(&self, id: WidgetId) -> Option<crate::props::ContentCaps> {
        self.index_of(id)
            .map(|index| self.widgets[index].content_caps())
    }

    /// One widget by id, without downcasting (the layout applier reads the
    /// skin/content declarations of an entry's target).
    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn widget_ref(&self, id: WidgetId) -> Option<&dyn crate::widget::Widget<M>> {
        self.index_of(id)
            .map(|index| self.widgets[index].as_ref())
    }

    /// Where a document-created widget is inserted: before the first backdrop
    /// widget (the page layer, so a modal scrim keeps painting above it), or at
    /// the end when there is none (M7.7).
    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn page_insert_index(&self) -> usize {
        self.widgets
            .iter()
            .position(|widget| widget.is_backdrop())
            .unwrap_or(self.widgets.len())
    }

    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn surface_style_of(&self, id: WidgetId) -> Option<SurfaceStyle> {
        self.index_of(id)
            .and_then(|index| self.widgets[index].surface_style().copied())
    }

    /// Write the surface of a widget that adopts one; `false` when it does not.
    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn set_surface_style_of(&mut self, id: WidgetId, style: SurfaceStyle) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let Some(slot) = self.widgets[index].surface_style_mut() else {
            return false;
        };
        *slot = style;
        self.mark_damage(index);
        true
    }

    /// Write one widget's local skin data: the widget applies children and
    /// attributes; the caller sequences it (children first, shared content,
    /// then attributes). One damage mark for the whole write.
    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn apply_skin_of(
        &mut self,
        id: WidgetId,
        attrs: &[(String, String)],
        children: &[crate::skin::SkinElement],
        palette: &crate::theme::Palette,
    ) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let widget = &mut self.widgets[index];
        if !children.is_empty() {
            widget.apply_skin_children(children);
        }
        if !attrs.is_empty() {
            let declarations = widget.skin_attrs();
            for (name, raw) in attrs {
                let Some(declaration) = declarations
                    .iter()
                    .find(|declaration| declaration.name == name.as_str())
                else {
                    continue;
                };
                if let Some(value) = declaration
                    .kind
                    .parse_with(raw, |token| crate::layout_xml::palette_token(palette, token))
                {
                    widget.apply_skin_attr(name, &value);
                }
            }
        }
        self.mark_damage(index);
        true
    }

    /// Whether an editing session (painted or native) is open on `id`.
    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn is_editing_on(&self, id: WidgetId, sink: &impl UiHost) -> bool {
        if sink.editing_id() == Some(id) {
            return true;
        }
        self.index_of(id)
            .map(|index| self.widgets[index].editing())
            .unwrap_or(false)
    }

    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn take_layout_state(&mut self) -> Option<crate::layout_xml::AppliedLayout> {
        self.xml_layout.take()
    }

    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn put_layout_state(&mut self, layout: crate::layout_xml::AppliedLayout) {
        self.xml_layout = Some(layout);
    }

    /// Override the cursor suggested by the widget; `None` restores it.
    pub fn set_cursor(&mut self, id: WidgetId, cursor: Option<Cursor>) {
        if self.index_of(id).is_none() {
            return;
        }
        match cursor {
            Some(cursor) => {
                self.cursors.insert(id, cursor);
            }
            None => {
                self.cursors.remove(&id);
            }
        }
    }

    /// Cursor for the widget under the pointer: per-instance override, then
    /// the widget's position-aware suggestion, then `Hand` for interactive
    /// widgets.
    pub fn hover_cursor(&self) -> Cursor {
        // A scroll-view bar is container chrome: the arrow, like every other
        // scrollbar in the library. The pointer path cached the answer; a
        // caller outside it (a direct read) falls back to scanning.
        let chrome = if self.chrome_valid {
            self.bar_hover.is_some() || self.bar_corner
        } else {
            self.pointer.is_some_and(|pos| {
                self.scroll_bar_at(pos).is_some() || self.scroll_corner_at(pos)
            })
        };
        if chrome {
            return Cursor::Arrow;
        }
        let Some(index) = self.hover else {
            return Cursor::Arrow;
        };
        let widget = &self.widgets[index];
        if let Some(cursor) = self.cursors.get(&widget.id()) {
            return *cursor;
        }
        let at_pointer = self
            .pointer
            .and_then(|pos| widget.cursor_at(pos))
            .or_else(|| widget.cursor());
        at_pointer.unwrap_or_else(|| {
            if self.effective_interactive(index) {
                Cursor::Hand
            } else {
                Cursor::Arrow
            }
        })
    }

    /// Install or clear a widget's context menu at runtime. The menu is not
    /// painted, so nothing is invalidated; a menu that mirrors live state is
    /// rebuilt here and its commands keep their ids.
    pub fn set_context_menu(&mut self, id: WidgetId, menu: Option<Menu>) {
        if self.index_of(id).is_none() {
            return;
        }
        if let Some(props) = self.props.get_mut(&id) {
            props.context_menu = menu;
        } else if let Some(menu) = menu {
            let mut props = WidgetProps::new();
            props.context_menu = Some(menu);
            self.props.insert(id, props);
        }
    }

    /// The widget's context menu, when one was installed.
    pub fn context_menu(&self, id: WidgetId) -> Option<&Menu> {
        self.props_for(id)?.context_menu.as_ref()
    }

    /// Install, replace or remove (`None`) a widget's menu at runtime.
    ///
    /// This is the per-widget menu registry: the same model feeds the
    /// secondary-click/menu-key path, [`Ui::open_menu`] and
    /// [`Ui::open_menu_for`]. A widget that owns a menu model of its own (a
    /// menu button or split button) is updated too, so its click, its
    /// keyboard chord and a programmatic open cannot disagree. Nothing is
    /// repainted — a menu has no on-screen part while it is closed — and an
    /// already-open popup keeps the model it was built from: edits apply to
    /// the next open.
    pub fn set_menu(&mut self, id: WidgetId, menu: Option<Menu>) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let model = menu.clone().unwrap_or_default();
        self.widgets[index].set_native_menu(model);
        self.set_context_menu(id, menu);
    }

    /// Remove a widget's menu (registry entry and its own model).
    pub fn remove_menu(&mut self, id: WidgetId) {
        self.set_menu(id, None);
    }

    /// The menu registered for a widget (what a secondary click would show).
    pub fn menu(&self, id: WidgetId) -> Option<&Menu> {
        self.context_menu(id)
    }

    /// Open a widget's registered menu from application code, anchored at
    /// `anchor` (client DIPs). Returns whether a popup was requested: a
    /// missing/empty menu, an unknown widget or an open native editor (the
    /// child owns the secondary click) request nothing. The popup itself is
    /// deferred by the host exactly like a context menu; the selection is
    /// replayed through `Host::menu_command` after the `MenuEvent` hook.
    pub fn open_menu(&mut self, id: WidgetId, anchor: Point, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        self.show_registered_menu(index, anchor, None, MenuSource::Programmatic, sink)
    }

    /// Open the menu a widget would show for its current state: a chrome
    /// widget's own model (a menu button, the menu bar's cursor cell, a
    /// toolbar's overflow) or, when it has none, its registered menu. This is
    /// the imperative equivalent of the widget's keyboard chord. The anchor
    /// is the widget's own (below the control) so the popup never covers it.
    pub fn open_menu_for(&mut self, id: WidgetId, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        if sink.editing_id() == Some(id) {
            return false;
        }
        if let Some(request) = self.widgets[index].native_menu() {
            if request.menu.is_empty() {
                return false;
            }
            sink.set_tooltip(None);
            sink.show_context_menu(ContextMenuRequest {
                widget: id,
                menu: &request.menu,
                anchor: request.anchor,
                avoid: request.avoid,
                source: MenuSource::Chrome,
            });
            return true;
        }
        let rect = self.widgets[index].rect();
        self.show_registered_menu(
            index,
            Point::new(rect.left, rect.bottom),
            Some(rect),
            MenuSource::Chrome,
            sink,
        )
    }

    /// Reopen the submenu at `path` as its own root popup, anchored at the
    /// widget's lower-left corner. This is an explicit reopen at that level
    /// (the row of a live popup cannot be measured from here), not a hover
    /// into an open menu. Returns whether a popup was requested.
    pub fn expand_menu(
        &mut self,
        id: WidgetId,
        path: &MenuPath,
        sink: &mut impl UiHost,
    ) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        if sink.editing_id() == Some(id) {
            return false;
        }
        let Some(menu) = self.props_at(index).and_then(|props| props.context_menu.clone()) else {
            return false;
        };
        let Some(submenu) = menu.submenu_at(path) else {
            return false;
        };
        if submenu.is_empty() {
            return false;
        }
        let rect = self.widgets[index].rect();
        sink.set_tooltip(None);
        sink.show_context_menu(ContextMenuRequest {
            widget: id,
            menu: submenu,
            anchor: Point::new(rect.left, rect.bottom),
            avoid: Some(rect),
            source: MenuSource::Programmatic,
        });
        true
    }

    fn show_registered_menu(
        &mut self,
        index: usize,
        anchor: Point,
        avoid: Option<Rect>,
        source: MenuSource,
        sink: &mut impl UiHost,
    ) -> bool {
        let id = self.widgets[index].id();
        if sink.editing_id() == Some(id) {
            return false;
        }
        let Some(menu) = self.props_at(index).and_then(|props| props.context_menu.clone()) else {
            return false;
        };
        if menu.is_empty() {
            return false;
        }
        sink.set_tooltip(None);
        sink.show_context_menu(ContextMenuRequest {
            widget: id,
            menu: &menu,
            anchor,
            avoid,
            source,
        });
        true
    }

    /// Programmatic display-property setters.
    ///
    /// They never call a widget's change callback (`TextEdit::on_change`,
    /// `NumberInput`/`Slider`'s `on_change`): those report *user* edits —
    /// typing, paste, cut, delete, undo, a stepper click, a drag. A setter only
    /// stores the value and marks its own widget for a repaint, collected here
    /// and handed to the sink once by [`Ui::flush_damage`], so setting N values
    /// in a row (a dialog syncing its controls) costs one repaint request.
    pub fn set_value(&mut self, id: WidgetId, value: f32) {
        if let Some(index) = self.index_of(id) {
            self.widgets[index].set_value(value);
            self.mark_damage(index);
        }
    }

    pub fn set_toggle(&mut self, id: WidgetId, on: bool) {
        if let Some(index) = self.index_of(id) {
            self.widgets[index].set_toggle(on);
            self.mark_damage(index);
        }
    }

    pub fn set_selected(&mut self, id: WidgetId, selected: usize) {
        if let Some(slot) = self.index_of(id) {
            self.widgets[slot].set_selected(selected);
            self.mark_damage(slot);
        }
    }

    /// Mark a widget for a repaint in the pending batch without touching it.
    ///
    /// Every `Ui` setter and [`Ui::edit_widget`] calls this for the widget they
    /// changed, so an application only needs it when it repaints a widget for a
    /// reason the container cannot observe (an application-owned overlay drawn
    /// beside it) or wants one extra rectangle in the same flush. Hidden and
    /// zero-sized widgets record nothing and return `false`.
    pub fn mark_dirty(&mut self, id: WidgetId) -> bool {
        match self.index_of(id) {
            Some(index) => self.mark_damage(index),
            None => false,
        }
    }

    /// Record the widget's paint bounds as pending damage. A visible widget
    /// only; a hidden one repaints through `set_visible`.
    fn mark_damage(&mut self, index: usize) -> bool {
        if !self.effective_visible(index) {
            return false;
        }
        let rect = self.bounds(index);
        if rect.is_empty() {
            return false;
        }
        crate::geometry::merge_damage(&mut self.damage, rect, MAX_DAMAGE_PIECES);
        true
    }

    /// Record an explicit rectangle as pending damage. Setters that change a
    /// widget's shape (open/close an overlay, disable it) pass the before and
    /// after bounds; a hidden widget still needs its old pixels repainted.
    fn mark_damage_rect(&mut self, rect: Rect) {
        crate::geometry::merge_damage(&mut self.damage, rect, MAX_DAMAGE_PIECES);
    }

    /// Hand the programmatic display changes since the last call to `sink` as
    /// one rectangle per changed region and clear them. Returns whether
    /// anything was pending.
    ///
    /// A host calls this once at the end of the event/callback that changed
    /// widget content through code (the showcase does it for every host
    /// callback). A burst of setters therefore repaints once, and the widget's
    /// change callback stays silent. Distant widgets stay separate pieces, so
    /// a timer updating a label and a spinner does not repaint the page
    /// between them.
    pub fn flush_damage(&mut self, sink: &mut impl Invalidator) -> bool {
        if self.damage.is_empty() {
            return false;
        }
        #[cfg(feature = "debug-form")]
        {
            self.debug_last_damage = self.damage.clone();
        }
        for rect in self.damage.drain(..) {
            if !rect.is_empty() {
                sink.invalidate(rect);
            }
        }
        true
    }

    /// Whether programmatic display changes are waiting for
    /// [`Ui::flush_damage`].
    pub fn has_damage(&self) -> bool {
        !self.damage.is_empty()
    }

    /// Select an option/row through the widget's own selection path and return
    /// the message it emits (the same path a click or a UIA `SelectionItem`
    /// action takes). Unlike [`Ui::set_selected`] this respects per-item rules
    /// — a disabled segmented option or an out-of-range index selects nothing.
    /// The widget rectangle is invalidated. `None` also covers widgets that
    /// select without emitting (a list row).
    pub fn select_child(
        &mut self,
        id: WidgetId,
        index: usize,
        sink: &mut impl Invalidator,
    ) -> Option<M> {
        let slot = self.index_of(id)?;
        let bounds_before = self.bounds(slot);
        let message = self.widgets[slot].a11y_select_child(index);
        sink.invalidate(bounds_before.union(self.bounds(slot)));
        message
    }

    pub fn set_rows(&mut self, id: WidgetId, rows: Vec<Row>) {
        if let Some(index) = self.index_of(id) {
            self.widgets[index].set_rows(rows);
            self.mark_damage(index);
        }
    }

    pub fn set_label(&mut self, id: WidgetId, label: impl Into<String>) {
        if let Some(index) = self.index_of(id) {
            self.widgets[index].set_label(label.into());
            self.mark_damage(index);
        }
    }

    pub fn set_progress(&mut self, id: WidgetId, value: f32) {
        if let Some(index) = self.index_of(id) {
            self.widgets[index].set_progress(value);
            self.mark_damage(index);
        }
    }

    /// Show or hide access-key underlines (the application-level Alt mode).
    ///
    /// Only chrome widgets that paint `&`-marked labels respond; hidden ones
    /// keep the new state so it applies when they appear. The usual policy is
    /// "underlines on after an Alt tap, off on the next key/click"; a host
    /// forwards `Event::Alt` here. Flipping the mode touches every chrome
    /// label at once, so each changed widget records itself in the pending
    /// damage and the host's end-of-event flush repaints them in one request.
    pub fn set_mnemonic_underlines(&mut self, show: bool) {
        for index in 0..self.widgets.len() {
            if self.widgets[index].set_mnemonic_underlines(show) && self.effective_visible(index)
            {
                self.mark_damage(index);
            }
        }
    }

    /// Show or hide a widget. A hidden widget stops taking input, leaves the
    /// Tab order and the a11y tree, and the widgets under its rectangle are
    /// exposed again. Hiding the focused widget moves focus to the next one.
    pub fn set_visible(&mut self, id: WidgetId, visible: bool, sink: &mut impl UiHost) {
        let Some(index) = self.index_of(id) else {
            // Remember the override for an id that does not exist yet: a
            // document-created widget (M7.7) inherits the page visibility the
            // application set before the layout created it.
            self.visibility.insert(id, visible);
            return;
        };
        if self.effective_visible(index) == visible {
            return;
        }
        let bounds_before = self.widgets[index].paint_bounds();
        let before = (!visible && self.focus == Some(index)).then(|| self.focus_order());
        self.visibility.insert(id, visible);
        self.widgets[index].set_visible(visible);
        self.invalidate_visible();
        if !visible {
            // Hiding a widget closes its overlay, so showing it again does not
            // resurrect a drop-down that was open when it disappeared.
            self.widgets[index].dismiss();
            self.release_pointer(index);
        }
        if let Some(before) = before {
            let keyboard = self.focus_visible;
            let next = self.next_focus(&before, index);
            self.focus_on(next, sink, keyboard, FocusReason::Relocated);
        }
        if self.focus == Some(index) {
            self.focus = None;
            self.adjusting = false;
        }
        // A scroll-view child hides the panel's chrome with it: the bars span
        // the whole viewport, which the children don't cover, so repaint the
        // viewport or the part below the last row keeps the old bar pixels.
        let owner = self.scroll_owner(id).map(|(_, viewport)| viewport);
        sink.invalidate(bounds_before.union(self.widgets[index].paint_bounds()));
        if let Some(viewport) = owner {
            sink.invalidate(viewport);
        }
        self.refresh_tooltip(sink);
    }

    /// Widget under the pointer, if any.
    pub fn hover_id(&self) -> Option<WidgetId> {
        self.hover.map(|index| self.widgets[index].id())
    }

    pub fn set_message(&mut self, id: WidgetId, text: impl Into<String>, error: bool) {
        if let Some(slot) = self.index_of(id) {
            self.widgets[slot].set_message(text.into(), error);
            // A message shows a widget that was hidden (Toast): drop the
            // container override so the widget's own state applies again.
            self.visibility.remove(&id);
            self.invalidate_visible();
            // The visible-state check inside `mark_damage` now sees the widget
            // again; a message that stays hidden records nothing.
            self.mark_damage(slot);
        }
    }

    pub fn set_text(&mut self, id: WidgetId, text: impl Into<String>) {
        if let Some(index) = self.index_of(id) {
            self.widgets[index].set_text(text.into());
            self.mark_damage(index);
        }
    }

    pub fn set_number(&mut self, id: WidgetId, value: i64) {
        if let Some(index) = self.index_of(id) {
            self.widgets[index].set_number(value);
            self.mark_damage(index);
        }
    }

    /// Widget holding the primary button, if any. A scroll-view bar drag is
    /// the container's own state and reports `None`.
    pub fn press_id(&self) -> Option<WidgetId> {
        self.press.map(|index| self.widgets[index].id())
    }

    pub fn focus_id(&self) -> Option<WidgetId> {
        self.focus.map(|index| self.widgets[index].id())
    }

    /// One read-only snapshot of the layout, widget tree, interaction state,
    /// damage and paint measurements (`debug-form`).
    ///
    /// The library does not paint an inspector: an application shows this value
    /// however it likes (the showcase's `调试` menu writes summaries into a
    /// dialog). Narration is resolved into each widget's accessibility name and
    /// description, and the returned data owns everything, so it stays valid
    /// while the tree changes.
    #[cfg(feature = "debug-form")]
    pub fn debug_snapshot(&self) -> crate::debug::DebugSnapshot {
        use crate::debug::{
            DebugClaim, DebugEntry, DebugLayout, DebugScroll, DebugSnapshot, DebugWidget,
        };

        let layout = self.installed_layout().map(|installed| DebugLayout {
            generation: installed.generation(),
            variant: installed.variant_id().to_string(),
            client: installed.client(),
            content_writes: installed.content_writes(),
            skin_writes: installed.skin_writes(),
            entries: installed
                .entries()
                .iter()
                .map(|entry| DebugEntry {
                    id: entry.id,
                    rect: entry.rect,
                    space: entry.space,
                })
                .collect(),
            claims: installed
                .claims
                .iter()
                .map(|claim| {
                    let fields = claim.fields;
                    let mut names = Vec::new();
                    if fields.visible {
                        names.push("visible");
                    }
                    if fields.enabled {
                        names.push("enabled");
                    }
                    if fields.focusable {
                        names.push("focusable");
                    }
                    if fields.focus_order {
                        names.push("focus-order");
                    }
                    if fields.cursor {
                        names.push("cursor");
                    }
                    if fields.props_any {
                        names.push("props");
                    }
                    if fields.corners {
                        names.push("corners");
                    }
                    if fields.background {
                        names.push("background");
                    }
                    DebugClaim {
                        id: claim.id,
                        baseline: claim.rect,
                        fields: names,
                    }
                })
                .collect(),
            warnings: installed
                .warnings()
                .iter()
                .map(ToString::to_string)
                .collect(),
            scrolls: self
                .scroll_views
                .iter()
                .map(|view| DebugScroll {
                    id: view.id(),
                    parent: view.parent(),
                    viewport: view.viewport(),
                    offset: view.offset_of(),
                    content: view.content_size(),
                })
                .collect(),
        });

        let widgets = self
            .widgets
            .iter()
            .enumerate()
            .map(|(index, widget)| {
                let props = self.props_at(index);
                let mut a11y = widget.a11y();
                if let (Some(info), Some(narration)) = (
                    a11y.as_mut(),
                    props.and_then(|props| props.narration.as_ref()),
                ) {
                    if let Some(name) = narration.resolve_name(info) {
                        info.name = name;
                    }
                    if let Some(description) = narration.resolve_description(info) {
                        info.description = Some(description);
                    }
                    narration.apply_rows(&mut info.children);
                }
                DebugWidget {
                    id: widget.id(),
                    rect: widget.rect(),
                    visible: self.effective_visible(index),
                    enabled: widget.enabled(),
                    focusable: self.effective_focusable(index),
                    read_only: self.effective_read_only(index),
                    focused: self.focus == Some(index),
                    hovered: self.hover == Some(index),
                    pressed: self.press == Some(index),
                    overlay: widget.overlay(),
                    scroll_parent: self.scroll_owner_of(widget.id()),
                    automation_id: props.and_then(|props| props.automation_id.clone()),
                    a11y,
                    skin_attrs: widget
                        .skin_attrs()
                        .iter()
                        .map(|decl| (decl.name, decl.kind))
                        .collect(),
                    skin_children: widget.skin_children().to_vec(),
                }
            })
            .collect();

        let (widget_types, has_commands) = match self.widget_registry() {
            Some(registry) => (registry.type_names().collect(), registry.has_commands()),
            None => (Vec::new(), false),
        };

        DebugSnapshot {
            layout,
            widgets,
            widget_types,
            has_commands,
            focus: self.focus_id(),
            hover: self.hover_id(),
            press: self.press_id(),
            pending_damage: self.damage.clone(),
            last_damage: self.debug_last_damage.clone(),
            paint: self.paint_recorder.snapshot(),
            log: self.debug_log.events().cloned().collect(),
        }
    }

    /// Append an application note to the debug diagnostics ring (`debug-form`).
    /// The ring keeps its most recent entries and refuses nothing; a hot reload
    /// can record its state here.
    #[cfg(feature = "debug-form")]
    pub fn debug_event(&mut self, kind: &'static str, text: impl Into<String>) {
        self.debug_log.push(kind, text);
    }

    /// Drop the diagnostics ring contents (`debug-form`). Measurements keep
    /// accumulating.
    #[cfg(feature = "debug-form")]
    pub fn clear_debug_log(&mut self) {
        self.debug_log.clear();
    }

    pub fn adjusting(&self) -> bool {
        self.adjusting
    }

    pub fn set_focus(&mut self, id: WidgetId, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        if !self.effective_visible(index) || !self.in_scope(index) {
            return false;
        }
        self.focus_on(Some(index), sink, false, FocusReason::Programmatic);
        true
    }

    /// Whether the focus ring is currently drawn (keyboard navigation).
    pub fn focus_visible(&self) -> bool {
        self.focus_visible
    }

    /// Force the focus ring on or off (a11y focus events should force it on).
    /// Turning it on also arms the focused widget's tooltip, the same dwell a
    /// keyboard move gets.
    pub fn show_focus_ring(&mut self, visible: bool, sink: &mut impl UiHost) {
        if self.focus_visible == visible {
            return;
        }
        self.focus_visible = visible;
        if let Some(index) = self.focus {
            sink.invalidate(self.bounds(index));
        }
        self.refresh_tooltip(sink);
    }

    /// Re-align an open native editor with the widget rectangle. Layout code
    /// calls this after assigning rects on resize or DPI change; the platform
    /// host ignores it when no session is open or nothing moved.
    pub fn sync_edit(&mut self, sink: &mut impl UiHost) {
        let Some(id) = sink.editing_id() else {
            return;
        };
        if let Some(rect) = self.rect(id) {
            sink.move_edit(rect);
        }
    }

    /// Apply text committed by the native editor: store it in the widget,
    /// repaint the widget and return the message the application should handle.
    pub fn commit_edit(
        &mut self,
        id: WidgetId,
        text: String,
        sink: &mut impl Invalidator,
    ) -> Option<M> {
        let index = self.index_of(id)?;
        let message = self.widgets[index].commit_text(text);
        sink.invalidate(self.bounds(index));
        message
    }

    /// Close an open framework-painted editing session. `commit` keeps the
    /// draft and returns the widget's commit message; otherwise the value
    /// reverts to what it was when editing began. Call before
    /// [`Ui::clear`]/page rebuild so a draft is not dropped silently.
    pub fn finish_inline_edit(
        &mut self,
        commit: bool,
        sink: &mut impl UiHost,
    ) -> Option<M> {
        let index = self.focus.filter(|index| self.widgets[*index].editing())?;
        let message = self.widgets[index].finish_edit(commit);
        sink.invalidate(self.bounds(index));
        sink.set_caret_rect(None);
        self.end_soft_keyboard(sink);
        message
    }

    /// Handle one translated character (keyboard or IME-independent input).
    /// A focused painted editor starts editing on the first printable
    /// character and inserts it. Returns whether the character was consumed.
    pub fn char(&mut self, ch: char, sink: &mut impl UiHost) -> bool {
        if ch.is_control() {
            return false;
        }
        let Some(index) = self.focus else {
            return false;
        };
        if !self.widgets[index].enabled()
            || self.effective_read_only(index)
            || !self.widgets[index].inline_edit()
        {
            return false;
        }
        if !self.widgets[index].editing() && !self.widgets[index].begin_inline_edit(None) {
            return false;
        }
        let observed = self.widgets[index].observe_char(ch);
        let mut buffer = [0u8; 4];
        if self.widgets[index].insert_text(ch.encode_utf8(&mut buffer)) {
            sink.invalidate(self.bounds(index));
        }
        // The `on_char` message wins over `on_change` for the same keystroke;
        // either way the pending change is consumed.
        let change = self.widgets[index].take_change_message();
        if let Some(message) = observed {
            self.messages.push(message);
        } else if let Some(message) = change {
            self.messages.push(message);
        }
        self.sync_caret(index, sink);
        true
    }

    /// Update the focused painted editor's IME composition string. Returns
    /// whether anything was consumed.
    pub fn ime_compose(&mut self, text: &str, cursor: usize, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.focus else {
            return false;
        };
        if !self.widgets[index].inline_edit() || self.effective_read_only(index) {
            return false;
        }
        // IME input can start before the first WM_CHAR (the composition *is*
        // the input); open the session so the composition is visible and the
        // final result has somewhere to go.
        if !self.widgets[index].editing() && !self.widgets[index].begin_inline_edit(None) {
            return false;
        }
        if self.widgets[index].set_composition(text, cursor) {
            sink.invalidate(self.bounds(index));
        }
        self.sync_caret(index, sink);
        true
    }

    /// Insert a committed IME result string into the focused painted editor.
    pub fn ime_commit(&mut self, text: &str, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.focus else {
            return false;
        };
        if !self.widgets[index].inline_edit() || self.effective_read_only(index) {
            return false;
        }
        if !self.widgets[index].editing() && !self.widgets[index].begin_inline_edit(None) {
            return false;
        }
        if !text.is_empty() && self.widgets[index].insert_text(text) {
            sink.invalidate(self.bounds(index));
        }
        if let Some(message) = self.widgets[index].take_change_message() {
            self.messages.push(message);
        }
        self.widgets[index].set_composition("", 0);
        self.sync_caret(index, sink);
        true
    }

    /// Route a context-menu command to a painted editor's built-in menu.
    /// Returns false when the id is not one of the reserved
    /// [`crate::widget::edit_command`] ids or no widget handled it, so the
    /// application can run its own menu command.
    pub fn menu_command(
        &mut self,
        id: WidgetId,
        command: u32,
        sink: &mut impl UiHost,
    ) -> bool {
        if !crate::widget::edit_command::is_edit(command) {
            return false;
        }
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let handled = if command == crate::widget::edit_command::PASTE {
            match sink.clipboard_text() {
                Some(text) if !text.is_empty() => {
                    let inserted = self.widgets[index].insert_text(&text);
                    if inserted && let Some(message) = self.widgets[index].take_change_message() {
                        self.messages.push(message);
                    }
                    inserted
                }
                _ => false,
            }
        } else {
            let handled = self.widgets[index].edit_command(command);
            if handled && let Some(message) = self.widgets[index].take_change_message() {
                self.messages.push(message);
            }
            handled
        };
        if let Some(text) = self.widgets[index].take_clipboard() {
            sink.copy_to_clipboard(&text);
        }
        sink.invalidate(self.bounds(index));
        self.sync_caret(index, sink);
        handled
    }

    fn sync_caret(&mut self, index: usize, sink: &mut impl UiHost) {
        if self.widgets[index].editing() {
            let rect = self.widgets[index].caret_rect();
            sink.set_caret_rect(rect);
        } else {
            sink.set_caret_rect(None);
        }
    }

    /// Whether the hovered widget opens a text editor (cursor shape hint).
    pub fn hover_wants_text(&self) -> bool {
        self.hover
            .map(|index| self.widgets[index].wants_edit())
            .unwrap_or(false)
    }

    /// Report the tooltip for the current hover/focus target. Hovering wins
    /// even when the hovered widget has no tooltip, so a keyboard-dwell tip
    /// does not linger while the pointer is over other content.
    fn refresh_tooltip(&self, sink: &mut impl UiHost) {
        // A held button (drag, slider) suppresses the tip until release.
        if self.press.is_some() {
            sink.set_tooltip(None);
            return;
        }
        let index = self
            .hover
            .or_else(|| self.focus_visible.then_some(self.focus).flatten());
        let target = index.and_then(|index| {
            let hovering = self.hover == Some(index);
            // Moving to another row/cell inside one container is a new
            // target, so the visible tip hides and the dwell restarts. Focus
            // resolves the selected item instead.
            let item = if hovering {
                self.pointer
                    .and_then(|point| self.widgets[index].child_at(point))
            } else {
                self.widgets[index].focus_item()
            };
            let text = self.props_at(index)?.tooltip_for(item)?;
            let widget = self.widgets[index].rect();
            // A hover tip follows the pointer; a keyboard/a11y-focus tip has
            // no pointer position and hangs below the focused element (a list
            // row, a selected tab cell, otherwise the widget itself).
            let anchor = match self.pointer {
                Some(point) if hovering && widget.contains_point(point) => {
                    TooltipAnchor::Pointer(point)
                }
                _ => TooltipAnchor::Widget(self.widgets[index].tooltip_rect()),
            };
            Some(TooltipRequest {
                text,
                widget,
                item,
                anchor,
            })
        });
        sink.set_tooltip(target);
    }

    /// Ask the host for `index`'s context menu, anchored at `anchor` (client
    /// DIPs). A widget without a menu, an empty menu, or a widget whose native
    /// editor is open (the child owns the secondary click) requests nothing.
    fn open_context_menu(&mut self, index: usize, anchor: Point, sink: &mut impl UiHost) -> bool {
        let id = self.widgets[index].id();
        if sink.editing_id() == Some(id) {
            return false;
        }
        let installed = self.props_at(index).and_then(|props| props.context_menu.clone());
        let owned;
        let menu = match installed.as_ref() {
            Some(menu) => menu,
            None => {
                // A painted editor supplies the standard edit menu when the
                // application installed none of its own.
                let Some(mut menu) = self.widgets[index].edit_menu() else {
                    return false;
                };
                // Paste depends on the clipboard, which only the host can
                // read; the widget leaves the item enabled while editing and
                // the container narrows it here.
                let has_text = sink
                    .clipboard_text()
                    .is_some_and(|text| !text.is_empty());
                menu.set_enabled(crate::widget::edit_command::PASTE, has_text);
                owned = menu;
                &owned
            }
        };
        if menu.is_empty() {
            return false;
        }
        // The menu covers the pointer; a lingering tip would sit on top of it.
        sink.set_tooltip(None);
        sink.show_context_menu(ContextMenuRequest {
            widget: id,
            menu,
            anchor,
            avoid: None,
            source: MenuSource::ContextMenu,
        });
        true
    }

    /// Ask the host to show a native menu a widget requested during its own
    /// input (a menu button, a menu bar, a toolbar overflow). The request is
    /// deferred by the host, exactly like a context menu.
    fn drain_native_menu(&mut self, index: usize, sink: &mut impl UiHost) {
        let Some(request) = self.widgets[index].take_native_menu() else {
            return;
        };
        if request.menu.is_empty() {
            return;
        }
        sink.set_tooltip(None);
        let id = self.widgets[index].id();
        sink.show_context_menu(ContextMenuRequest {
            widget: id,
            menu: &request.menu,
            anchor: request.anchor,
            avoid: request.avoid,
            source: MenuSource::Chrome,
        });
    }

    /// Menu key / Shift+F10: show the focused widget's menu at its lower-left
    /// corner. Returns false when nothing has focus or the widget has no menu,
    /// so the key stays unhandled.
    fn open_context_menu_for_focus(&mut self, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.focus else {
            return false;
        };
        let rect = self.widgets[index].rect();
        self.open_context_menu(index, Point::new(rect.left, rect.bottom), sink)
    }

    /// Flatten the widget tree into the accessibility model. One node per
    /// exposed widget, followed by its options/tabs/visible rows. Only the
    /// visible list rows are published, so a 600-row queue is cheap.
    ///
    /// A focused container presents its selected option/row as the element
    /// with keyboard focus, the way native list and tab controls do: a screen
    /// reader then announces the item instead of only "contains N items".
    pub fn a11y_nodes(&self) -> Vec<A11yNode> {
        let mut nodes: Vec<A11yNode> = Vec::new();
        // A named scroll view publishes one `Group` node per viewport, first,
        // so its children can point at it. A child scrolled out of the viewport
        // stays in the tree with `offscreen`, the way a virtualized list's
        // look-ahead row does; a partially visible one reports its clipped
        // on-screen rectangle.
        let mut panes: Vec<Option<usize>> = vec![None; self.scroll_views.len()];
        for panel in 0..self.scroll_views.len() {
            let Some(name) = self.scroll_views[panel].name_ref() else {
                continue;
            };
            if !self.scroll_shown(panel) {
                continue;
            }
            let index = nodes.len();
            // The scroll range ends at the `content_rect` (viewport minus the
            // bars that are showing), which is what `max_offset` measures
            // against; the content extent is the resolved one.
            let view = &self.scroll_views[panel];
            let area = view.content_rect();
            let content = view.content_size();
            let offset = view.offset_of();
            nodes.push(A11yNode {
                id: view.id(),
                role: A11yRole::Group,
                child: None,
                name: name.to_string(),
                automation_id: None,
                description: None,
                value: None,
                enabled: true,
                focusable: false,
                focused: false,
                checked: None,
                selected: None,
                range: None,
                read_only: true,
                text_selection: None,
                rect: view.viewport(),
                offscreen: false,
                expanded: None,
                scroll: Some(
                    A11yScroll::new()
                        .horizontal(offset.x, area.width(), content.width)
                        .vertical(offset.y, area.height(), content.height),
                ),
                grid_row: None,
                grid_column: None,
                grid: None,
                parent: None,
                children: Vec::new(),
            });
            panes[panel] = Some(index);
        }
        for (widget_index, widget) in self.widgets.iter().enumerate() {
            let Some(mut info) = widget.a11y() else {
                continue;
            };
            if !self.effective_visible(widget_index) {
                continue;
            }
            let props = self.props_at(widget_index);
            // Display-only widgets (titles, cards, progress bars) stay out of
            // the tree because Narrator would otherwise read decoration on
            // every Tab. The instance opts in by becoming focusable, carrying
            // narration or a role override, or by asking to be exposed.
            let expose = props.and_then(|props| props.exposed);
            if expose == Some(false) {
                continue;
            }
            if info.display_only
                && expose != Some(true)
                && !self.effective_focusable(widget_index)
                && !self.effective_interactive(widget_index)
                && props.map_or(true, |props| {
                    props.narration.is_none() && props.role.is_none()
                })
            {
                continue;
            }
            if let Some(role) = props.and_then(|props| props.role) {
                info.role = role;
            }
            if let Some(narration) = props.and_then(|props| props.narration.as_ref()) {
                if let Some(name) = narration.resolve_name(&info) {
                    info.name = name;
                }
                if let Some(description) = narration.resolve_description(&info) {
                    info.description = Some(description);
                }
                narration.apply_rows(&mut info.children);
            }
            let focused = self.focus == Some(widget_index);
            let focused_child = if focused {
                info.children.iter().position(|child| child.selected)
            } else {
                None
            };
            let automation_id = props.and_then(|props| props.automation_id.clone());
            let read_only =
                info.read_only || props.and_then(|props| props.read_only).unwrap_or(false);
            let index = nodes.len();
            let (parent, rect, offscreen) = match self.scroll_owner(widget.id()) {
                Some((panel, viewport)) => match widget.rect().intersect(viewport) {
                    Some(clipped) => (panes[panel], clipped, false),
                    None => (panes[panel], widget.rect(), true),
                },
                None => (None, widget.rect(), false),
            };
            nodes.push(A11yNode {
                id: widget.id(),
                role: info.role,
                child: None,
                name: info.name,
                automation_id: automation_id.clone(),
                description: info.description,
                value: info.value,
                enabled: widget.enabled(),
                focusable: self.effective_focusable(widget_index),
                focused: focused && focused_child.is_none(),
                checked: info.checked,
                selected: info.selected,
                range: info.range,
                read_only,
                text_selection: info.text_selection,
                rect,
                offscreen,
                expanded: info.expanded,
                scroll: info.scroll,
                grid_row: None,
                grid_column: None,
                grid: info.grid,
                parent,
                children: Vec::new(),
            });
            if let Some(parent) = parent {
                nodes[parent].children.push(index);
            }
            for (position, child) in info.children.into_iter().enumerate() {
                let node_index = nodes.len();
                nodes.push(A11yNode {
                    id: widget.id(),
                    role: child.role.unwrap_or_else(|| info.role.child_role()),
                    child: Some(child.index),
                    name: child.name,
                    automation_id: automation_id.clone(),
                    description: None,
                    value: child.value,
                    enabled: widget.enabled() && child.enabled,
                    focusable: true,
                    focused: focused_child == Some(position),
                    checked: child.checked,
                    selected: Some(child.selected),
                    range: None,
                    read_only: child.read_only,
                    text_selection: None,
                    rect: child.rect,
                    offscreen: child.offscreen,
                    expanded: child.expanded,
                    scroll: None,
                    grid_row: child.row,
                    grid_column: child.column,
                    grid: None,
                    parent: Some(index),
                    children: Vec::new(),
                });
                nodes[index].children.push(node_index);
            }
        }
        nodes
    }

    /// Apply an assistive-technology request through the normal widget input
    /// path. Returns the message the application should handle, if any.
    pub fn apply_a11y(
        &mut self,
        action: &A11yAction,
        sink: &mut impl UiHost,
    ) -> Option<M> {
        // A scroll view is a container region, not a widget, so its viewport
        // moves before the widget lookup below; every other scrollable widget
        // owns its offset and clamps the request itself.
        if let A11yActionKind::Scroll {
            horizontal,
            vertical,
        } = action.kind
            && let Some(panel) = self.scroll_index(action.id)
        {
            let before = self.scroll_views[panel].offset_of();
            let offset =
                Point::new(horizontal.unwrap_or(before.x), vertical.unwrap_or(before.y));
            self.scroll_views[panel].set_offset(offset);
            self.scroll_views[panel].clamp();
            if self.scroll_views[panel].offset_of() != before {
                self.scroll_layout(panel, sink);
            }
            return None;
        }
        let index = self.index_of(action.id)?;
        match action.kind {
            A11yActionKind::Focus => {
                self.focus_on(Some(index), sink, true, FocusReason::A11y);
                None
            }
            A11yActionKind::Scroll {
                horizontal,
                vertical,
            } => {
                let before = self.bounds(index);
                if self.widgets[index].a11y_scroll(horizontal, vertical) {
                    sink.invalidate(before);
                }
                None
            }
            A11yActionKind::Invoke | A11yActionKind::Toggle => {
                // A published child (a painted menu item) is activated through
                // the widget's own child path, the same one a click takes.
                if let Some(child) = action.child {
                    let bounds_before = self.bounds(index);
                    let message = self.widgets[index].a11y_select_child(child);
                    sink.invalidate(bounds_before.union(self.bounds(index)));
                    self.drain_native_menu(index, sink);
                    return message;
                }
                if self.widgets[index].inline_edit() {
                    self.focus_on(Some(index), sink, true, FocusReason::A11y);
                    if !self.effective_read_only(index) {
                        self.widgets[index].begin_inline_edit(None);
                        sink.invalidate(self.bounds(index));
                        self.sync_caret(index, sink);
                    }
                    None
                } else if self.widgets[index].wants_edit() {
                    self.focus_on(Some(index), sink, true, FocusReason::A11y);
                    let outcome = self.interact(index, WidgetInput::Activate, sink, true);
                    if self.emit(outcome) {
                        self.begin_edit_at(index, sink);
                    }
                    None
                } else {
                    let outcome = self.interact(index, WidgetInput::Activate, sink, true);
                    outcome.custom.or(outcome.default).or(outcome.after)
                }
            }
            A11yActionKind::Select => {
                let child = action.child?;
                let bounds_before = self.bounds(index);
                let message = self.widgets[index].a11y_select_child(child);
                sink.invalidate(bounds_before.union(self.bounds(index)));
                self.drain_native_menu(index, sink);
                message
            }
            A11yActionKind::Expand | A11yActionKind::Collapse => {
                let expanded = action.kind == A11yActionKind::Expand;
                let bounds_before = self.bounds(index);
                let message = self.widgets[index].a11y_expand(action.child, expanded);
                sink.invalidate(bounds_before.union(self.bounds(index)));
                self.drain_native_menu(index, sink);
                message
            }
            A11yActionKind::SetValue => {
                let text = action
                    .value
                    .clone()
                    .or_else(|| action.number.map(|number| number.to_string()))?;
                let bounds_before = self.bounds(index);
                let message = self.widgets[index].a11y_set_value_at(action.child, &text);
                sink.invalidate(bounds_before.union(self.bounds(index)));
                message
            }
        }
    }

    /// Drain application messages produced since the last call.
    pub fn drain_messages(&mut self) -> Vec<M> {
        std::mem::take(&mut self.messages)
    }

    /// Whether a visible full-window backdrop (a modal [`crate::Scrim`]) is
    /// up. The host reports this through [`crate::Host::overlay_paint`] so the
    /// window composites the overlay over a cached page instead of repainting
    /// the page (D87).
    pub fn overlay_active(&self) -> bool {
        self.overlay_split().is_some()
    }

    /// Index of the first visible backdrop widget; widgets before it form the
    /// page and the ones from it on form the overlay layer.
    pub fn overlay_split(&self) -> Option<usize> {
        (0..self.widgets.len())
            .find(|&index| self.effective_visible(index) && self.widgets[index].is_backdrop())
    }

    pub fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        let palette = *ctx.painter.palette();
        self.ensure_visible();
        self.ensure_scroll_shown();
        let images = &self.images;
        // Overlay composite: the back buffer already holds the page, so only
        // the backdrop and the widgets added after it paint (D87).
        let overlay_split = if ctx.overlay {
            self.overlay_split()
        } else {
            None
        };
        // Diagnostic only (see HANDOFF): logs any widget slower than 1.5 ms so
        // a slow page can be attributed without a profiler. With `debug-form`
        // the same timing also feeds the queryable paint statistics.
        let paint_log = std::env::var_os("BAMBOO_UI_PAINT_LOG").is_some();
        #[cfg(feature = "debug-form")]
        let frame_started = std::time::Instant::now();
        #[cfg(feature = "debug-form")]
        let mut frame_stats: Vec<(WidgetId, u64)> = Vec::new();
        // Two passes: ordinary widgets, then overlays (an open drop-down or
        // flyout) so a transient popup is never painted under a sibling that
        // happens to come later in insertion order. Hidden pages are already
        // excluded from `visible_indices`.
        for overlay_pass in [false, true] {
            for &index in self.visible_indices.iter() {
                if self.widgets[index].overlay() != overlay_pass {
                    continue;
                }
                if let Some(split) = overlay_split {
                    if index < split {
                        continue;
                    }
                    // The backdrop already dimmed the cached page on the first
                    // overlay frame; later frames leave those pixels alone.
                    if index == split && !ctx.overlay_first {
                        continue;
                    }
                }
                let layout = self.widgets[index].rect();
                let mut bounds = self.widgets[index].paint_bounds();
                // A scroll-view child is clipped to the viewport and skipped
                // entirely while it is scrolled out, so its pixels stay cheap.
                let clip = self
                    .scroll_owner(self.widgets[index].id())
                    .map(|(_, viewport)| viewport);
                if let Some(viewport) = clip {
                    if !bounds.intersects(viewport) {
                        continue;
                    }
                    bounds = bounds.intersect(viewport).unwrap_or(bounds);
                }
                if layout.is_empty()
                    || !self.effective_visible(index)
                    || !ctx.hits(bounds)
                {
                    continue;
                }
                let state = WidgetState {
                    hovered: self.hover == Some(index),
                    pressed: self.press == Some(index),
                    focused: self.focus == Some(index),
                    adjusting: self.adjusting && self.focus == Some(index),
                    enabled: self.widgets[index].enabled(),
                    focus_visible: self.focus == Some(index) && self.focus_visible,
                    editing: ctx.editing == Some(self.widgets[index].id()),
                    caret_on: ctx.caret_on,
                };
                // A per-instance palette only affects this widget's paint; the
                // brush cache in the backend is keyed by color, so this
                // allocates nothing.
                let widget_palette = self
                    .props_at(index)
                    .and_then(|props| props.palette)
                    .unwrap_or(palette);
                // Push the viewport clip before the paint context borrows the
                // canvas.
                if let Some(viewport) = clip {
                    ctx.canvas.push_clip(viewport);
                }
                let mut paint = WidgetPaint {
                    painter: ctx.painter,
                    canvas: &mut *ctx.canvas,
                    palette: widget_palette,
                    font: self.props_at(index).and_then(|props| props.font.clone()),
                    images,
                    viewport: Rect::from_size(ctx.size),
                    dirty: ctx.dirty,
                };
                #[cfg(feature = "debug-form")]
                let started = std::time::Instant::now();
                #[cfg(not(feature = "debug-form"))]
                let started = paint_log.then(std::time::Instant::now);
                self.widgets[index].paint(&mut paint, &state);
                #[cfg(feature = "debug-form")]
                {
                    let micros = started.elapsed().as_micros() as u64;
                    if paint_log && micros > 1500 {
                        eprintln!(
                            "  widget {:?} paint={}us rect={:?}",
                            self.widgets[index].id().0,
                            micros,
                            layout
                        );
                    }
                    frame_stats.push((self.widgets[index].id(), micros));
                }
                #[cfg(not(feature = "debug-form"))]
                if let Some(started) = started {
                    let micros = started.elapsed().as_micros();
                    if micros > 1500 {
                        eprintln!(
                            "  widget {:?} paint={}us rect={:?}",
                            self.widgets[index].id().0,
                            micros,
                            layout
                        );
                    }
                }
                drop(paint);
                // A display widget opted into the mouse gets feedback the
                // widget itself does not paint: a tinted outline on hover,
                // accent while pressed. Widgets that paint their own hover
                // state never set `mouse(true)`, so this does not double up.
                let opted_mouse =
                    self.props_at(index).and_then(|props| props.interactive) == Some(true);
                if opted_mouse && (state.hovered || state.pressed) {
                    let color = if state.pressed {
                        widget_palette.accent
                    } else {
                        widget_palette
                            .accent
                            .mix(widget_palette.background, 0.55)
                    };
                    outline(&mut *ctx.canvas, layout.inset(1.0, 1.0), color, 1.0);
                }
                if clip.is_some() {
                    ctx.canvas.pop_clip();
                }
            }
        }
        // Scroll-view bars are the container's chrome: they paint above the
        // content, clipped to their viewport, with the shared themed colors.
        // An overlay composite leaves the page (and therefore its bars) in the
        // cached buffer.
        if !ctx.overlay {
            for panel in 0..self.scroll_views.len() {
            if !self.scroll_shown(panel) {
                continue;
            }
            // Clip to the visible part: a nested panel's viewport may extend
            // past its parent, and its bars must not paint outside it (D92).
            let viewport = self.scroll_views[panel].clip_rect();
            if viewport.is_empty() || !ctx.hits(viewport) {
                continue;
            }
            let bars = scroll::bars_layout(&self.scroll_views[panel]);
            let style = *self.scroll_views[panel].style_ref();
            let vertical = *self.scroll_views[panel].bar_state(true);
            let horizontal = *self.scroll_views[panel].bar_state(false);
            ctx.canvas.push_clip(viewport);
            if let Some(bar) = bars.vertical {
                scroll::paint_bar(
                    ctx.painter,
                    &mut *ctx.canvas,
                    &palette,
                    bar,
                    &vertical,
                    &style,
                );
            }
            if let Some(bar) = bars.horizontal {
                scroll::paint_bar(
                    ctx.painter,
                    &mut *ctx.canvas,
                    &palette,
                    bar,
                    &horizontal,
                    &style,
                );
            }
            // Mask the junction: neither track covers that square, and the
            // content underneath would otherwise show through it.
            if let Some(corner) = scroll::corner_rect(&self.scroll_views[panel]) {
                let colors = style.resolved_colors(&palette);
                ctx.canvas.fill_rect(corner, colors.track);
            }
            ctx.canvas.pop_clip();
        }
        }
        if let Some(index) = self.focus
            && (self.focus_visible || self.adjusting)
            && overlay_split.map_or(true, |split| index >= split)
        {
            let rect = self.widgets[index].rect();
            if !rect.is_empty() && ctx.hits(rect) {
                // The ring follows the widget's visual bounds so painted
                // content (list rows, the switch track) never shows outside it.
                let ring = self.widgets[index].focus_ring_rect();
                let width = if self.adjusting { 2.0 } else { 1.0 };
                let accent = self
                    .props_at(index)
                    .and_then(|props| props.palette)
                    .map(|palette| palette.accent)
                    .unwrap_or(palette.accent);
                let clip = self
                    .scroll_owner(self.widgets[index].id())
                    .map(|(_, viewport)| viewport);
                if let Some(viewport) = clip {
                    ctx.canvas.push_clip(viewport);
                }
                if let Some(radius) = self.widgets[index].focus_ring_radius() {
                    // Circular/pill controls get a ring that follows their
                    // shape instead of a rectangle cutting the corners.
                    ctx.canvas.stroke_round_rect(ring, radius, width, accent);
                } else {
                    outline(&mut *ctx.canvas, ring, accent, width);
                }
                if clip.is_some() {
                    ctx.canvas.pop_clip();
                }
            }
        }
        // Report the painted editor's caret to the host: it drives the blink
        // rectangle and the IME composition/candidate window.
        if let Some(index) = self.focus
            && self.widgets[index].editing()
        {
            ctx.caret = self.widgets[index].caret_rect();
        }
        #[cfg(feature = "debug-form")]
        self.paint_recorder.end_frame(
            frame_started.elapsed().as_micros() as u64,
            &frame_stats,
        );
    }

    pub fn pointer_move(&mut self, pos: Point, sink: &mut impl UiHost) -> bool {
        self.pointer = Some(pos);
        // A thumb drag owns the pointer until the release, like every other
        // bar in the widget library.
        if self.scroll_drag_to(pos, sink) {
            return true;
        }
        // A cross-widget drag owns the pointer: track the target under it and
        // leave the ordinary hover/press path alone (D113).
        if self.drag.is_some() {
            self.update_drag_hover(pos, sink);
            self.refresh_tooltip(sink);
            return true;
        }
        self.ensure_scroll_shown();
        let (bar, corner) = self.scroll_chrome_at(pos);
        self.chrome_valid = true;
        self.bar_corner = corner;
        self.scroll_hover_to(bar, sink);
        self.ensure_visible();
        let hit = self.hit_index_after_chrome(pos, bar.is_some() || corner);
        let mut changed = false;
        if hit != self.hover {
            if let Some(old) = self.hover {
                // The widget's own hint narrows a bar-chrome change to the
                // scrollbar strip instead of the whole list.
                let repaint = self.widgets[old].paints_pointer_state();
                let outcome = self.interact(old, WidgetInput::HoverLeave, sink, repaint);
                self.emit(outcome);
            }
            self.hover = hit;
            if let Some(new) = hit {
                let repaint = self.widgets[new].paints_pointer_state();
                let outcome = self.interact(new, WidgetInput::HoverEnter, sink, repaint);
                self.emit(outcome);
            }
            changed = true;
        }
        if let Some(index) = self.press {
            if !self.dragging
                && let Some(start) = self.press_pos
            {
                let dx = pos.x - start.x;
                let dy = pos.y - start.y;
                if dx * dx + dy * dy >= DRAG_THRESHOLD * DRAG_THRESHOLD {
                    self.dragging = true;
                    // A registered drag source owns the gesture; otherwise the
                    // widget sees the ordinary DragStart (D113).
                    if self.begin_drag(index, start, sink) {
                        self.update_drag_hover(pos, sink);
                        self.refresh_tooltip(sink);
                        return true;
                    }
                    let outcome =
                        self.interact(index, WidgetInput::DragStart(start), sink, false);
                    self.emit(outcome);
                }
            }
            self.dispatch(index, WidgetInput::DragTo(pos), sink);
        }
        if let Some(index) = hit {
            self.dispatch(index, WidgetInput::HoverAt(pos), sink);
        }
        self.refresh_tooltip(sink);
        changed
    }

    pub fn pointer_down(&mut self, pos: Point, sink: &mut impl UiHost) -> bool {
        self.pointer_down_button(pos, crate::event::MouseButton::Left, sink)
    }

    /// Pointer press with an explicit button and no keyboard modifiers.
    pub fn pointer_down_button(
        &mut self,
        pos: Point,
        button: crate::event::MouseButton,
        sink: &mut impl UiHost,
    ) -> bool {
        self.pointer_down_with(pos, button, crate::event::PointerModifiers::NONE, sink)
    }

    /// Pointer press with an explicit button. Left performs the normal
    /// press/focus/edit path; right tracks the target so the release can raise
    /// [`Trigger::RightClick`]. A secondary click on a text field additionally
    /// focuses it and opens the native editor (paste/context-menu workflows);
    /// other widgets keep the "no focus on right click" rule. Middle is
    /// ignored. `modifiers` is the keyboard state at press time; the release
    /// carries its own, which is what a multi-select list reads.
    pub fn pointer_down_with(
        &mut self,
        pos: Point,
        button: crate::event::MouseButton,
        _modifiers: crate::event::PointerModifiers,
        sink: &mut impl UiHost,
    ) -> bool {
        use crate::event::MouseButton;
        self.pointer = Some(pos);
        self.ensure_scroll_shown();
        self.ensure_visible();
        // A scroll-view bar owns its presses: they never reach a widget
        // underneath and never move focus (native bars behave the same way).
        if button == MouseButton::Left && self.scroll_press(pos, sink) {
            sink.set_tooltip(None);
            return true;
        }
        let hit = self.hit_index(pos);
        if hit != self.hover {
            if let Some(old) = self.hover {
                sink.invalidate(self.bounds(old));
            }
            self.hover = hit;
        }
        // A press anywhere else closes a transient overlay owned by the
        // focused widget (an open combo drop-down, a flyout).
        if let Some(focused) = self.focus
            && Some(focused) != hit
        {
            let bounds_before = self.bounds(focused);
            if self.widgets[focused].dismiss() {
                sink.invalidate(bounds_before.union(self.bounds(focused)));
            }
        }
        match button {
            MouseButton::Left => {
                self.press = hit;
                self.press_pos = Some(pos);
                self.dragging = false;
                let Some(index) = hit else {
                    sink.set_tooltip(None);
                    return false;
                };
                if self.effective_focusable(index) {
                    self.focus_on(Some(index), sink, false, FocusReason::Pointer);
                }
                self.dispatch(index, WidgetInput::PressAt(pos), sink);
                if self.widgets[index].inline_edit() {
                    // A read-only painted field still opens the session: the
                    // session owns caret movement, pointer/Shift selection
                    // and copy. The widget itself refuses every mutation.
                    if self.widgets[index].inline_edit_at(pos) {
                        // A touch press on a selection handle grabs it: the
                        // caret must stay where it is, or the selection the
                        // handle belongs to disappears under the finger. Any
                        // other press goes through the widget's own rule,
                        // which keeps a selection the press landed in (so it
                        // can be dragged out, D122) or places the caret.
                        if !self.widgets[index].grab_selection_handle(pos) {
                            self.widgets[index].begin_inline_edit(Some(pos));
                        }
                        sink.invalidate(self.bounds(index));
                        self.sync_caret(index, sink);
                        // A painted field is not a real edit control, so the
                        // shell never raises the touch keyboard by itself
                        // (D121). A read-only session has nothing to type
                        // into, so it leaves the keyboard alone.
                        if self.pointer_kind != PointerKind::Mouse
                            && !self.effective_read_only(index)
                            && !self.touch_soft
                        {
                            self.touch_soft = sink.set_touch_keyboard(true);
                        }
                    }
                } else if self.widgets[index].wants_edit_at(pos) {
                    self.begin_edit_at(index, sink);
                }
                // A click dismisses the tooltip; moving or re-entering the
                // widget starts a fresh dwell.
                sink.set_tooltip(None);
                true
            }
            MouseButton::Right => {
                self.right_press = hit;
                sink.set_tooltip(None);
                if let Some(index) = hit {
                    if self.widgets[index].inline_edit() {
                        // A painted field: the secondary button only focuses
                        // and opens the edit menu. It never dispatches a press
                        // or repositions the caret, so an existing selection
                        // survives for Copy/Cut. A read-only field opens the
                        // selection session too (copy stays available).
                        if self.effective_focusable(index) {
                            self.focus_on(Some(index), sink, false, FocusReason::Pointer);
                        }
                        if !self.widgets[index].editing()
                            && self.widgets[index].begin_inline_edit(None)
                        {
                            sink.invalidate(self.bounds(index));
                            self.sync_caret(index, sink);
                        }
                    } else if self.widgets[index].wants_edit_at(pos) {
                        if self.effective_focusable(index) {
                            self.focus_on(Some(index), sink, false, FocusReason::Pointer);
                        }
                        self.begin_edit_at(index, sink);
                    }
                }
                hit.is_some()
            }
            _ => false,
        }
    }

    pub fn pointer_up(&mut self, pos: Point, sink: &mut impl UiHost) -> bool {
        self.pointer_up_button(pos, crate::event::MouseButton::Left, sink)
    }

    /// Pointer release with an explicit button and no keyboard modifiers.
    pub fn pointer_up_button(
        &mut self,
        pos: Point,
        button: crate::event::MouseButton,
        sink: &mut impl UiHost,
    ) -> bool {
        self.pointer_up_with(pos, button, crate::event::PointerModifiers::NONE, sink)
    }

    /// Pointer release with an explicit button. A Ctrl/Shift release on a
    /// widget that opted into modified selection (`Widget::multi_selectable`)
    /// is delivered as [`WidgetInput::SelectWith`]; every other widget sees the
    /// ordinary click.
    pub fn pointer_up_with(
        &mut self,
        pos: Point,
        button: crate::event::MouseButton,
        modifiers: crate::event::PointerModifiers,
        sink: &mut impl UiHost,
    ) -> bool {
        use crate::event::MouseButton;
        self.pointer = Some(pos);
        // A drop resolves the cross-widget drag instead of a release/click on
        // the source (D113).
        if self.drag.is_some() {
            return self.finish_drag(pos, sink);
        }
        if button == MouseButton::Left && self.scroll_release(pos, sink) {
            return true;
        }
        if button == MouseButton::Right {
            let Some(pressed) = self.right_press.take() else {
                return false;
            };
            let hit = self.hit_index(pos);
            if hit == Some(pressed) {
                self.dispatch(pressed, WidgetInput::RightClickAt(pos), sink);
                // The binding has run; the widget's own menu (if any) opens at
                // the pointer after it.
                self.open_context_menu(pressed, pos, sink);
            }
            return hit.is_some();
        }
        if button != MouseButton::Left {
            return false;
        }
        let Some(index) = self.press.take() else {
            return false;
        };
        let dragged = std::mem::replace(&mut self.dragging, false);
        self.press_pos = None;
        let hit = self.hit_index(pos);
        self.dispatch(index, WidgetInput::ReleaseAt(pos), sink);
        if dragged {
            // A drag is not a click: the release ends the drag instead.
            self.dispatch(index, WidgetInput::DragEnd(pos), sink);
        } else if hit == Some(index) {
            let input = if modifiers.any() && self.widgets[index].multi_selectable() {
                WidgetInput::SelectWith(pos, modifiers)
            } else {
                WidgetInput::ClickAt(pos)
            };
            self.dispatch(index, input, sink);
        }
        true
    }

    /// The platform reported a double click (`CS_DBLCLKS`). Bindings observe
    /// it; widgets have no default double-click behavior.
    pub fn pointer_double_click(&mut self, pos: Point, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.hit_index(pos) else {
            return false;
        };
        self.dispatch(index, WidgetInput::DoubleClickAt(pos), sink);
        true
    }

    pub fn pointer_leave(&mut self, sink: &mut impl UiHost) {
        self.pointer = None;
        self.scroll_release_bars(sink);
        self.bar_hover = None;
        self.bar_corner = false;
        self.chrome_valid = false;
        if let Some(index) = self.hover.take() {
            let repaint = self.widgets[index].paints_pointer_state();
            let _ = self.widgets[index].input(WidgetInput::HoverLeave);
            let hint = self.widgets[index].take_repaint_hint();
            if repaint {
                sink.invalidate(hint.unwrap_or_else(|| self.bounds(index)));
            }
        }
        self.refresh_tooltip(sink);
    }

    pub fn wheel(&mut self, pos: Point, delta: f32, sink: &mut impl UiHost) -> bool {
        self.wheel_xy(pos, 0.0, delta, sink)
    }

    /// Wheel with both axes (Shift+wheel reports a horizontal delta). A widget
    /// that scrolls its own content (a list, table, tree, multiline field)
    /// consumes the notch; otherwise the scroll view under the pointer scrolls.
    pub fn wheel_xy(
        &mut self,
        pos: Point,
        delta_x: f32,
        delta_y: f32,
        sink: &mut impl UiHost,
    ) -> bool {
        // A visible backdrop (the in-window modal shell) swallows the wheel:
        // the page under the scrim must never scroll, or the cached page and
        // the overlay would disagree and leave ghost pixels.
        if let Some(split) = self.overlay_split() {
            if let Some(index) = self.hit_index(pos).filter(|index| *index >= split) {
                self.dispatch(index, WidgetInput::Wheel(delta_y), sink);
            }
            return true;
        }
        let hit = self.hit_index(pos);
        if let Some(index) = hit {
            self.dispatch(index, WidgetInput::Wheel(delta_y), sink);
        }
        let inner_scrolls = hit
            .map(|index| self.widgets[index].scrolls_with_wheel())
            .unwrap_or(false);
        if !inner_scrolls {
            // The innermost panel under the pointer scrolls first; a panel
            // already at its end passes the notch to its ancestors (D92), so a
            // nested list hands the wheel to the page it sits in.
            let mut chain: Vec<usize> = (0..self.scroll_views.len())
                .filter(|panel| {
                    self.scroll_shown(*panel)
                        && self.scroll_views[*panel].clip_rect().contains_point(pos)
                })
                .collect();
            chain.sort_by_key(|panel| {
                (std::cmp::Reverse(self.scroll_depth(*panel)), std::cmp::Reverse(*panel))
            });
            for panel in chain {
                // The DIP notch is calibrated for the default three lines; the
                // system's wheel setting scales it like the row/line widgets.
                let step = self.scroll_views[panel].wheel_step_dips()
                    * scrollbar::wheel_lines()
                    / 3.0;
                let axes = self.scroll_views[panel].axes();
                let mut dx = if axes.horizontal() { -delta_x * step } else { 0.0 };
                let dy = if axes.vertical() { -delta_y * step } else { 0.0 };
                // A horizontal-only panel still moves with the plain wheel.
                if dx == 0.0 && dy == 0.0 && delta_y != 0.0 && axes.horizontal() {
                    dx = -delta_y * step;
                }
                if (dx != 0.0 || dy != 0.0) && self.scroll_by_panel(panel, dx, dy, sink) {
                    return true;
                }
            }
        }
        hit.is_some()
    }

    /// Scroll one panel by a delta along the axes it accepts.
    fn scroll_by_panel(
        &mut self,
        panel: usize,
        dx: f32,
        dy: f32,
        sink: &mut impl UiHost,
    ) -> bool {
        let before = self.scroll_views[panel].offset_of();
        self.scroll_views[panel].set_offset(Point::new(before.x + dx, before.y + dy));
        self.scroll_views[panel].clamp();
        if self.scroll_views[panel].offset_of() == before {
            return false;
        }
        self.scroll_layout(panel, sink);
        true
    }

    /// Handle one key produced by [`crate::gamepad::Gamepad`]: the ordinary
    /// [`Ui::key`] path plus the platform courtesy a controller needs for text
    /// entry. When the key opens a text session, the host is asked to raise the
    /// system on-screen keyboard ([`UiHost::set_soft_keyboard`]); the keyboard
    /// is dismissed as soon as that session ends. Only a keyboard the pad
    /// itself raised is ever touched, so pointer and hardware-keyboard users
    /// never see it and mouse text entry keeps working untouched.
    ///
    /// While the shell keyboard is up, the pad belongs to it and keys are not
    /// forwarded: on the Windows gamepad layout the user is choosing keys on
    /// the keyboard, and forwarding **A** would commit the field underneath.
    /// **A** re-raises the keyboard when the user dismissed it with its own
    /// close button, and **B** closes it and leaves the field (committing, the
    /// same as clicking out of it). Returns whether the key was consumed.
    pub fn pad_key(&mut self, key: KeyEvent, sink: &mut impl UiHost) -> bool {
        if self.pad_soft && self.text_entry_active(sink) {
            if key.code == KeyCode::Escape {
                sink.set_soft_keyboard(false);
                self.pad_soft = false;
                self.finish_text_entry(sink);
                return true;
            }
            if sink.soft_keyboard_open() {
                // The shell keyboard is up: it owns the pad, so nothing is
                // forwarded (A would commit the field under the selection).
                return true;
            }
            // The user closed the keyboard with its own close button; A
            // brings it back instead of committing the field. Another key, or
            // a platform that refuses to show it, falls back to normal routing
            // rather than trapping the pad in a session it cannot type into.
            if key.code == KeyCode::Enter
                && !key.ctrl
                && !key.alt
                && sink.set_soft_keyboard(true)
            {
                return true;
            }
            self.pad_soft = false;
        }
        let was_editing = self.text_entry_active(sink);
        let handled = self.key(key, sink);
        if !was_editing && self.text_entry_active(sink) {
            self.pad_soft = sink.set_soft_keyboard(true);
        } else if self.pad_soft && !self.text_entry_active(sink) {
            self.end_soft_keyboard(sink);
        }
        handled
    }

    /// Report what is driving the pointer (the host forwards
    /// [`Event::PointerKind`]). Touch and pen input selects the painted
    /// editor's touch model: a touch press raises the shell keyboard, a
    /// long-press picks a word and the selection shows draggable handles.
    /// A mouse press keeps the desktop model.
    pub fn set_pointer_kind(&mut self, kind: PointerKind) {
        self.pointer_kind = kind;
    }

    /// The device last reported through [`Ui::set_pointer_kind`].
    pub fn pointer_kind(&self) -> PointerKind {
        self.pointer_kind
    }

    /// A touch press was held in place without moving (the host owns the
    /// ~500 ms dwell and forwards [`Event::LongPress`]): the painted editor
    /// under `pos` selects the word there and shows its selection handles,
    /// like a mobile text field. A press outside a text field does nothing.
    /// Returns whether a widget handled it.
    pub fn long_press(&mut self, pos: Point, sink: &mut impl UiHost) -> bool {
        let Some(index) = self.hit_index(pos) else {
            return false;
        };
        if !self.widgets[index].inline_edit() || !self.widgets[index].inline_edit_at(pos) {
            return false;
        }
        if !self.widgets[index].editing() && !self.widgets[index].begin_inline_edit(None) {
            return false;
        }
        if !self.widgets[index].long_press(pos) {
            return false;
        }
        sink.invalidate(self.bounds(index));
        self.sync_caret(index, sink);
        true
    }

    /// Whether a text-entry session is open: a native editor the host holds or
    /// a framework-painted draft on the focused widget. A read-only painted
    /// field keeps a selection session open but never counts as text entry (a
    /// gamepad must not raise the on-screen keyboard for it).
    fn text_entry_active(&self, sink: &impl UiHost) -> bool {
        sink.editing_id().is_some()
            || self.focus.is_some_and(|index| {
                self.widgets[index].editing() && !self.effective_read_only(index)
            })
    }

    /// Dismiss an on-screen keyboard the framework raised for a text session
    /// (gamepad or touch), once the session is gone.
    fn end_soft_keyboard(&mut self, sink: &mut impl UiHost) {
        if std::mem::take(&mut self.pad_soft) {
            sink.set_soft_keyboard(false);
        }
        if std::mem::take(&mut self.touch_soft) {
            sink.set_touch_keyboard(false);
        }
    }

    /// Commit and close whichever text session is open, the way leaving the
    /// field does: a painted draft's message is queued for
    /// [`Ui::drain_messages`], a native child is ended with commit so the text
    /// reaches `Host::edit_committed`.
    fn finish_text_entry(&mut self, sink: &mut impl UiHost) {
        if let Some(message) = self.finish_inline_edit(true, sink) {
            self.messages.push(message);
        }
        if sink.editing_id().is_some() {
            sink.end_edit();
            sink.set_caret_rect(None);
        }
    }

    /// Handle navigation and activation. Returns false when the key is not
    /// consumed, so the host can apply global shortcuts.
    pub fn key(&mut self, key: KeyEvent, sink: &mut impl UiHost) -> bool {
        // Access keys run before every other key path: Alt+F must open a menu
        // even while an editor owns the keyboard.
        if key.alt
            && !key.ctrl
            && let Some(ch) = access_char(key.code)
            && self.run_mnemonic(ch, sink)
        {
            return true;
        }
        // VK_APPS (0x5D) and Shift+F10 open the focused widget's context menu,
        // like the native controls do. Unhandled when there is no menu.
        if (matches!(key.code, KeyCode::Other(0x5D))
            || (key.code == KeyCode::Function(10) && key.shift))
            && self.open_context_menu_for_focus(sink)
        {
            return true;
        }
        // A framework-painted editor owns every key while it is open. Tab and
        // Escape commit/cancel and leave the session; Enter commits a
        // single-line field; everything else (arrows, Home/End, Backspace,
        // Delete, Ctrl chords) is editing input and must not move focus.
        if let Some(index) = self.focus
            && self.widgets[index].editing()
        {
            return self.key_for_editor(index, key, sink);
        }
        // Additive key observation (D109): the focused widget sees every
        // key-down before the container interprets it, including keys it does
        // not act on (Tab, arrows outside adjust mode, plain characters). A
        // widget that owns the keyboard (an open painted field) observes
        // through its own editor path, so this is skipped while editing.
        if let Some(index) = self.focus
            && self.widgets[index].enabled()
            && let Some(message) = self.widgets[index].observe_key_down(key)
        {
            self.messages.push(message);
        }
        // Enter opens a closed painted field; Space would otherwise activate
        // it and start an unwanted session. A read-only field opens too, so
        // the keyboard can select and copy its text.
        if key.code == KeyCode::Enter
            && !key.ctrl
            && !key.alt
            && let Some(index) = self.focus
            && self.widgets[index].inline_edit()
        {
            self.widgets[index].begin_inline_edit(None);
            sink.invalidate(self.bounds(index));
            self.sync_caret(index, sink);
            return true;
        }
        // Alt+Up/Down is the native drop-down chord; deliver it to the focused
        // widget instead of moving focus.
        if key.alt
            && matches!(key.code, KeyCode::Down | KeyCode::Up)
            && let Some(index) = self.focus
        {
            self.dispatch(index, WidgetInput::Key(key), sink);
            return true;
        }
        // An open transient overlay owns navigation: arrows move the open
        // drop-down's highlight and Enter/Space select it. Tab still moves
        // focus and Escape still dismisses the overlay.
        if let Some(index) = self.focus
            && self.widgets[index].owns_navigation_key(key.code)
            && matches!(
                key.code,
                KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Home
                    | KeyCode::End
                    | KeyCode::PageUp
                    | KeyCode::PageDown
                    | KeyCode::Enter
                    | KeyCode::Space
            )
        {
            self.dispatch(index, WidgetInput::Key(key), sink);
            return true;
        }
        match key.code {
            KeyCode::Tab => {
                self.move_focus(if key.shift { -1 } else { 1 }, sink);
                true
            }
            KeyCode::Up => {
                if self.adjusting && let Some(index) = self.focus {
                    self.dispatch(index, WidgetInput::Key(key), sink);
                } else {
                    self.move_focus(-1, sink);
                }
                true
            }
            KeyCode::Down => {
                if self.adjusting && let Some(index) = self.focus {
                    self.dispatch(index, WidgetInput::Key(key), sink);
                } else {
                    self.move_focus(1, sink);
                }
                true
            }
            KeyCode::Left => {
                if self.adjusting && let Some(index) = self.focus {
                    self.dispatch(index, WidgetInput::Key(key), sink);
                } else {
                    self.move_focus(-1, sink);
                }
                true
            }
            KeyCode::Right => {
                if self.adjusting && let Some(index) = self.focus {
                    self.dispatch(index, WidgetInput::Key(key), sink);
                } else {
                    self.move_focus(1, sink);
                }
                true
            }
            KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End => {
                // Only meaningful inside adjust mode (list navigation).
                if self.adjusting && let Some(index) = self.focus {
                    self.dispatch(index, WidgetInput::Key(key), sink);
                    true
                } else {
                    false
                }
            }
            KeyCode::Backspace => {
                // Outside a native editor (which owns the key and deletes
                // text), Backspace cancels the focused widget: it leaves
                // adjust mode, otherwise it releases keyboard focus.
                if self.adjusting {
                    self.adjusting = false;
                    if let Some(index) = self.focus {
                        sink.invalidate(self.bounds(index));
                    }
                    true
                } else if self.focus.is_some() {
                    self.focus_on(None, sink, false, FocusReason::Keyboard);
                    true
                } else {
                    false
                }
            }
            KeyCode::Enter | KeyCode::Space => {
                let Some(index) = self.focus else {
                    return false;
                };
                if self.adjusting {
                    self.dispatch(index, WidgetInput::Key(key), sink);
                    return true;
                }
                if self.widgets[index].adjustable() {
                    self.adjusting = true;
                    self.focus_visible = true;
                    sink.invalidate(self.bounds(index));
                } else {
                    let ran = self.dispatch(index, WidgetInput::Activate, sink);
                    if ran {
                        sink.invalidate(self.bounds(index));
                        if key.code == KeyCode::Enter && self.widgets[index].wants_edit() {
                            self.begin_edit_at(index, sink);
                        }
                    }
                }
                true
            }
            KeyCode::Escape => {
                // Escape aborts a cross-widget drag before any other meaning.
                if self.drag.is_some() {
                    self.cancel_drag(sink);
                    return true;
                }
                // A transient overlay (combo drop-down, flyout) closes first;
                // leaving adjust mode is the second meaning of Escape.
                if let Some(index) = self.focus {
                    let bounds_before = self.bounds(index);
                    if self.widgets[index].dismiss() {
                        sink.invalidate(bounds_before.union(self.bounds(index)));
                        return true;
                    }
                }
                if self.adjusting {
                    self.adjusting = false;
                    if let Some(index) = self.focus {
                        sink.invalidate(self.bounds(index));
                    }
                    true
                } else {
                    false
                }
            }
            _ => {
                if self.adjusting && let Some(index) = self.focus {
                    self.dispatch(index, WidgetInput::Key(key), sink);
                    return true;
                }
                false
            }
        }
    }

    /// Deliver a key release to the focused widget. Releases carry no
    /// navigation or editing meaning, so the container only forwards them (the
    /// edit-box `on_key_up` hook); nothing is repainted unless the application
    /// reacts to the message. Returns whether a focused widget received it.
    pub fn key_up(&mut self, key: KeyEvent, _sink: &mut impl UiHost) -> bool {
        let Some(index) = self.focus else {
            return false;
        };
        if !self.in_scope(index) || !self.widgets[index].enabled() {
            return false;
        }
        // The painted editor owns its own release path (`input(KeyUp)`); other
        // widgets report through the additive observer (D109).
        let mut message = self.widgets[index].input(WidgetInput::KeyUp(key));
        if message.is_none() {
            message = self.widgets[index].observe_key_up(key);
        }
        if let Some(message) = message {
            self.messages.push(message);
        }
        true
    }

    /// Run an `Alt+key` access key. A widget may claim per-item keys (a menu
    /// bar's `Alt+F`) through [`Widget::mnemonic_key`]; otherwise the first
    /// visible widget whose own mnemonic matches is focused and activated.
    /// Returns whether the chord was handled, so the key is not also typed.
    fn run_mnemonic(&mut self, ch: char, sink: &mut impl UiHost) -> bool {
        for index in 0..self.widgets.len() {
            if !self.effective_visible(index)
                || !self.in_scope(index)
                || !self.widgets[index].enabled()
            {
                continue;
            }
            if self.widgets[index].mnemonic_key(ch) {
                self.focus_on(Some(index), sink, true, FocusReason::Keyboard);
                sink.invalidate(self.bounds(index));
                self.drain_native_menu(index, sink);
                return true;
            }
        }
        let target = (0..self.widgets.len()).find(|&index| {
            self.effective_visible(index)
                && self.in_scope(index)
                && self.widgets[index].enabled()
                && self.widgets[index]
                    .mnemonic()
                    .is_some_and(|key| key.eq_ignore_ascii_case(&ch))
        });
        if let Some(index) = target {
            self.focus_on(Some(index), sink, true, FocusReason::Keyboard);
            self.dispatch(index, WidgetInput::Activate, sink);
            sink.invalidate(self.bounds(index));
            return true;
        }
        false
    }

    /// Key routing for an open framework-painted editor. It owns the keyboard:
    /// Tab and Escape close the session, Enter commits a single-line field and
    /// everything else is editing input. A multiline field keeps Enter (it
    /// inserts a newline) and commits on Ctrl+Enter, Tab or focus loss.
    fn key_for_editor(&mut self, index: usize, key: KeyEvent, sink: &mut impl UiHost) -> bool {
        let multiline = self.widgets[index].multiline_edit();
        match key.code {
            KeyCode::Tab => {
                let message = self.widgets[index].finish_edit(true);
                if let Some(message) = message {
                    self.messages.push(message);
                }
                sink.invalidate(self.bounds(index));
                sink.set_caret_rect(None);
                self.end_soft_keyboard(sink);
                self.move_focus(if key.shift { -1 } else { 1 }, sink);
                return true;
            }
            KeyCode::Escape => {
                let message = self.widgets[index].finish_edit(false);
                if let Some(message) = message {
                    self.messages.push(message);
                }
                sink.invalidate(self.bounds(index));
                sink.set_caret_rect(None);
                self.end_soft_keyboard(sink);
                return true;
            }
            KeyCode::Enter if !multiline => {
                let message = self.widgets[index].finish_edit(true);
                if let Some(message) = message {
                    self.messages.push(message);
                }
                sink.invalidate(self.bounds(index));
                sink.set_caret_rect(None);
                self.end_soft_keyboard(sink);
                return true;
            }
            // Multiline: Enter is a newline; Ctrl+Enter is the commit chord.
            KeyCode::Enter if multiline && key.ctrl && !key.alt => {
                let message = self.widgets[index].finish_edit(true);
                if let Some(message) = message {
                    self.messages.push(message);
                }
                sink.invalidate(self.bounds(index));
                sink.set_caret_rect(None);
                self.end_soft_keyboard(sink);
                return true;
            }
            _ => {}
        }
        // Paste needs the host clipboard; every other chord is handled by the
        // widget itself (copy/cut report their text through `take_clipboard`).
        if key.ctrl
            && !key.alt
            && matches!(key.code, KeyCode::Letter(21))
        {
            if let Some(text) = sink.clipboard_text()
                && !text.is_empty()
            {
                if self.widgets[index].insert_text(&text) {
                    sink.invalidate(self.bounds(index));
                    if let Some(message) = self.widgets[index].take_change_message() {
                        self.messages.push(message);
                    }
                }
            }
            self.sync_caret(index, sink);
            return true;
        }
        self.dispatch(index, WidgetInput::Key(key), sink);
        if let Some(text) = self.widgets[index].take_clipboard() {
            sink.copy_to_clipboard(&text);
        }
        self.sync_caret(index, sink);
        true
    }

    /// Ask the host for a native editor on `index`; no-op unless the widget is
    /// editable, enabled and not read-only.
    fn begin_edit_at(&mut self, index: usize, sink: &mut impl UiHost) {
        if !self.widgets[index].enabled()
            || !self.widgets[index].wants_edit()
            || self.widgets[index].inline_edit()
            || self.effective_read_only(index)
        {
            return;
        }
        if let Some(mut request) = self.widgets[index].edit_request() {
            // A per-instance font override applies to the native control too,
            // so the open editor matches the closed paint.
            if let Some(font) = self.props_at(index).and_then(|props| props.font.as_ref()) {
                if let Some(size) = font.size {
                    request.font_size = Some(size);
                }
                if let Some(family) = &font.family {
                    request.font_family = Some(family.clone());
                }
            }
            sink.begin_edit(&request);
        }
    }

    fn dispatch(&mut self, index: usize, input: WidgetInput, sink: &mut impl UiHost) -> bool {
        let outcome = self.interact(index, input, sink, true);
        self.emit(outcome)
    }

    /// Deliver widget input, running a matching trigger first and a
    /// value-change trigger afterwards when the widget's a11y state moved.
    fn interact(
        &mut self,
        index: usize,
        input: WidgetInput,
        sink: &mut impl UiHost,
        repaint: bool,
    ) -> Outcome<M> {
        let id = self.widgets[index].id();
        let mut custom = None;
        let mut pos = Self::input_pos(input);
        // Additive observers run first (D109): the event reached the widget,
        // even if a trigger binding then replaces the built-in action. The
        // message is additive (queued after the widget's own), so the built-in
        // message is never displaced.
        let observed = match input {
            WidgetInput::ClickAt(pos) | WidgetInput::SelectWith(pos, _) => {
                self.widgets[index].observe_click(pos)
            }
            WidgetInput::RightClickAt(pos) => self.widgets[index].observe_right_click(pos),
            WidgetInput::DoubleClickAt(pos) => self.widgets[index].observe_double_click(pos),
            _ => None,
        };
        // Snapshot value-change and repaint intent *before* the handler, which
        // may run the default itself through `TriggerContext::run_default`.
        // The snapshot costs a visit only when someone asked for it and only
        // for inputs that can change a value; hover motion and drag previews
        // would otherwise snapshot on every move.
        let can_change = matches!(
            input,
            WidgetInput::ClickAt(_)
                | WidgetInput::SelectWith(_, _)
                | WidgetInput::Activate
                | WidgetInput::Key(_)
                | WidgetInput::Step(_)
                | WidgetInput::ReleaseAt(_)
        );
        let watching = can_change
            && self
                .bindings
                .iter()
                .any(|binding| binding.id == id && binding.trigger == Trigger::ValueChange);
        let before = watching.then(|| self.value_fingerprint(index)).flatten();
        let wants_repaint = repaint
            && match input {
                WidgetInput::HoverAt(pos) | WidgetInput::DragTo(pos) => {
                    self.widgets[index].repaint_on_move_at(pos)
                }
                WidgetInput::PressAt(_)
                | WidgetInput::DragStart(_)
                | WidgetInput::ReleaseAt(_)
                | WidgetInput::DragEnd(_)
                | WidgetInput::ClickAt(_)
                | WidgetInput::SelectWith(_, _)
                | WidgetInput::DoubleClickAt(_)
                | WidgetInput::RightClickAt(_)
                | WidgetInput::Wheel(_) => self.widgets[index].paints_pointer_state(),
                _ => true,
            };
        // The widget may open or close an overlay (a combo drop-down). Its
        // bounds after the call then describe only the new shape, so the union
        // with the old bounds is what repaints the pixels it just vacated.
        let bounds_before = self.widgets[index].paint_bounds();

        let mut run_default = true;
        let mut default_ran = false;
        if let Some((trigger, trigger_pos, wheel_delta)) = Self::trigger_of(input) {
            pos = trigger_pos;
            if let Some((message, keep_after, ran)) =
                self.call_binding_input(index, trigger, trigger_pos, wheel_delta, input)
            {
                custom = message;
                // The handler may have run the default itself (nothing left to
                // do) or asked for it afterwards through the flag form.
                run_default = keep_after && !ran;
                default_ran = ran;
            }
        }
        if !run_default && !default_ran {
            return Outcome {
                custom,
                default: None,
                extra: observed,
                after: None,
                ran: false,
            };
        }
        let message = if default_ran {
            // The handler ran the default inside the binding and owns the
            // resulting message; it already decided what to return.
            None
        } else {
            self.widgets[index].input(input)
        };
        // An additive notification the widget reported next to its own message
        // (a drag-end callback beside a change report). Drained on every input
        // so it can never leak; an observer message from this input wins the
        // slot (the two cannot come from the same input).
        let widget_extra = self.widgets[index].take_extra_message();
        let extra = observed.or(widget_extra);
        // A widget may narrow the repaint to the part that changed (scrollbar
        // chrome); the hint is consumed on every input so it cannot leak.
        let hint = self.widgets[index].take_repaint_hint();
        if wants_repaint {
            let rect = hint.unwrap_or_else(|| bounds_before.union(self.bounds(index)));
            sink.invalidate(rect);
        }
        self.drain_native_menu(index, sink);
        let after = match before {
            Some(before) if self.value_fingerprint(index).as_ref() != Some(&before) => {
                self.call_binding(id, Trigger::ValueChange, pos, 0.0)
                    .and_then(|(message, _)| message)
            }
            _ => None,
        };
        Outcome {
            custom,
            default: message,
            extra,
            after,
            ran: true,
        }
    }

    /// Run one (widget, trigger) handler with no invocable default (the
    /// value-change notification, which has no widget input behind it);
    /// returns its message and whether the (nonexistent) default was requested.
    fn call_binding(
        &mut self,
        id: WidgetId,
        trigger: Trigger,
        pos: Option<Point>,
        wheel_delta: f32,
    ) -> Option<(Option<M>, bool)> {
        let position = self
            .bindings
            .iter()
            .position(|binding| binding.id == id && binding.trigger == trigger)?;
        let mut binding = self.bindings.remove(position);
        let mut no_default = || -> Option<M> { None };
        let mut context = TriggerContext {
            requested: false,
            ran: false,
            run: &mut no_default,
        };
        let message = (binding.handler)(
            TriggerEvent {
                trigger,
                pos,
                wheel_delta,
            },
            &mut context,
        );
        let keep_default = context.requested;
        self.bindings.push(binding);
        Some((message, keep_default))
    }

    /// Run one (widget, trigger) handler with the widget's default action
    /// available through [`TriggerContext::run_default`]. Returns the handler
    /// message, whether it asked for the default *after* returning
    /// ([`TriggerContext::default`]) and whether it already ran the default
    /// itself.
    fn call_binding_input(
        &mut self,
        index: usize,
        trigger: Trigger,
        pos: Option<Point>,
        wheel_delta: f32,
        input: WidgetInput,
    ) -> Option<(Option<M>, bool, bool)> {
        let id = self.widgets[index].id();
        let position = self
            .bindings
            .iter()
            .position(|binding| binding.id == id && binding.trigger == trigger)?;
        let mut binding = self.bindings.remove(position);
        let message;
        let requested;
        let ran;
        {
            // The default closure owns the widget for the handler's lifetime;
            // the binding was taken out of the vector first, so no other field
            // of `self` is borrowed here.
            let widget = &mut *self.widgets[index];
            let mut run = move || widget.input(input);
            let mut context = TriggerContext {
                requested: false,
                ran: false,
                run: &mut run,
            };
            message = (binding.handler)(
                TriggerEvent {
                    trigger,
                    pos,
                    wheel_delta,
                },
                &mut context,
            );
            requested = context.requested;
            ran = context.ran;
        }
        self.bindings.push(binding);
        Some((message, requested, ran))
    }

    /// The parts of a widget's accessible state that make a value change:
    /// checked, the value text, an explicit selected flag, the range, and the
    /// selected option/row of a composite control.
    fn value_fingerprint(&self, index: usize) -> Option<ValueFingerprint> {
        let info = self.widgets[index].a11y()?;
        Some(ValueFingerprint {
            checked: info.checked,
            value: info.value,
            selected: info.selected,
            range: info.range,
            child: info.children.iter().position(|child| child.selected),
        })
    }

    fn emit(&mut self, outcome: Outcome<M>) -> bool {
        if let Some(message) = outcome.custom {
            self.messages.push(message);
        }
        if let Some(message) = outcome.default {
            self.messages.push(message);
        }
        if let Some(message) = outcome.extra {
            self.messages.push(message);
        }
        if let Some(message) = outcome.after {
            self.messages.push(message);
        }
        outcome.ran
    }

    fn trigger_of(input: WidgetInput) -> Option<(Trigger, Option<Point>, f32)> {
        match input {
            WidgetInput::ClickAt(pos) | WidgetInput::SelectWith(pos, _) => {
                Some((Trigger::Click, Some(pos), 0.0))
            }
            WidgetInput::DoubleClickAt(pos) => Some((Trigger::DoubleClick, Some(pos), 0.0)),
            WidgetInput::RightClickAt(pos) => Some((Trigger::RightClick, Some(pos), 0.0)),
            WidgetInput::DragStart(pos) => Some((Trigger::DragStart, Some(pos), 0.0)),
            WidgetInput::DragEnd(pos) => Some((Trigger::DragEnd, Some(pos), 0.0)),
            WidgetInput::HoverEnter => Some((Trigger::HoverEnter, None, 0.0)),
            WidgetInput::HoverLeave => Some((Trigger::HoverLeave, None, 0.0)),
            WidgetInput::Wheel(delta) => Some((Trigger::Wheel, None, delta)),
            WidgetInput::Activate => Some((Trigger::Activate, None, 0.0)),
            _ => None,
        }
    }

    fn input_pos(input: WidgetInput) -> Option<Point> {
        match input {
            WidgetInput::SelectWith(pos, _)
            | WidgetInput::HoverAt(pos)
            | WidgetInput::PressAt(pos)
            | WidgetInput::DragTo(pos)
            | WidgetInput::ReleaseAt(pos)
            | WidgetInput::ClickAt(pos)
            | WidgetInput::DoubleClickAt(pos)
            | WidgetInput::RightClickAt(pos)
            | WidgetInput::DragStart(pos)
            | WidgetInput::DragEnd(pos) => Some(pos),
            _ => None,
        }
    }

    /// Topmost visible, interactive widget under the point. Display-only
    /// widgets are transparent to the pointer, so a label or card never blocks
    /// the control underneath it. An open overlay (a combo drop-down) is
    /// checked first because it paints above every sibling. A scroll panel's
    /// bar strip owns its own presses and a child scrolled out of the viewport
    /// is not under the pointer at all.
    fn hit_index(&mut self, pos: Point) -> Option<usize> {
        self.ensure_scroll_shown();
        let (bar, corner) = self.scroll_chrome_at(pos);
        self.bar_hover = bar;
        self.bar_corner = corner;
        self.chrome_valid = true;
        self.ensure_visible();
        self.hit_index_after_chrome(pos, bar.is_some() || corner)
    }

    /// Hit-test when the caller already resolved the scroll-bar/corner chrome
    /// for this position (the pointer path shares it with bar hover and the
    /// cursor, so the panels are scanned once per move).
    fn hit_index_after_chrome(&self, pos: Point, chrome: bool) -> Option<usize> {
        if chrome {
            return None;
        }
        // Most pages have no overlay at all; scanning only the visible widgets
        // once is half the work of the two overlay passes.
        let has_overlay = self
            .visible_indices
            .iter()
            .any(|index| self.widgets[*index].overlay());
        let passes: &[bool] = if has_overlay { &[true, false] } else { &[false] };
        for overlay_pass in passes {
            for &index in self.visible_indices.iter().rev() {
                let widget = &self.widgets[index];
                if widget.overlay() != *overlay_pass {
                    continue;
                }
                if widget.enabled()
                    && self.effective_interactive(index)
                    && self.scroll_hit_visible(index, widget.rect(), pos)
                    && widget.hit_test(pos)
                {
                    return Some(index);
                }
            }
        }
        None
    }

    /// Whether a widget can be hit at all: an ordinary widget is clipped by
    /// nothing, a scroll-view child only while it intersects the viewport.
    fn scroll_hit_visible(&self, index: usize, rect: Rect, pos: Point) -> bool {
        match self.scroll_owner(self.widgets[index].id()) {
            Some((_, viewport)) => rect.intersects(viewport) && viewport.contains_point(pos),
            None => true,
        }
    }

    /// Tab/arrow order: ascending focus weight, insertion order as the
    /// tie-break. Hidden, disabled, empty and non-focusable widgets are
    /// skipped.
    fn focus_order(&self) -> Vec<usize> {
        let mut order: Vec<(i32, usize)> = self
            .widgets
            .iter()
            .enumerate()
            .filter(|(index, widget)| {
                self.effective_visible(*index)
                    && self.in_scope(*index)
                    && widget.enabled()
                    && !widget.rect().is_empty()
                    && self.effective_focusable(*index)
            })
            .map(|(index, _)| (self.focus_weight(index), index))
            .collect();
        order.sort_by_key(|(weight, index)| (*weight, *index));
        order.into_iter().map(|(_, index)| index).collect()
    }

    fn move_focus(&mut self, delta: i32, sink: &mut impl UiHost) {
        let order = self.focus_order();
        if order.is_empty() {
            return;
        }
        let position = self
            .focus
            .and_then(|index| order.iter().position(|candidate| *candidate == index));
        let next = match position {
            Some(position) => {
                let len = order.len() as i32;
                ((position as i32 + delta).rem_euclid(len)) as usize
            }
            None => {
                if delta < 0 {
                    order.len() - 1
                } else {
                    0
                }
            }
        };
        self.focus_on(Some(order[next]), sink, true, FocusReason::Keyboard);
    }

    fn focus_on(
        &mut self,
        index: Option<usize>,
        sink: &mut impl UiHost,
        keyboard: bool,
        reason: FocusReason,
    ) {
        self.focus_move(index, sink, keyboard, reason, true);
    }

    fn focus_move(
        &mut self,
        index: Option<usize>,
        sink: &mut impl UiHost,
        keyboard: bool,
        reason: FocusReason,
        allow_redirect: bool,
    ) {
        let index = index.filter(|index| self.effective_visible(*index) && self.in_scope(*index));
        if self.focus == index {
            self.focus_visible |= keyboard;
            self.refresh_tooltip(sink);
            return;
        }
        let previous = self.focus;
        if let Some(old) = previous {
            // Leaving an editing widget is the same as kill-focus: commit and
            // close the session (native child or painted draft) before the
            // lost hook runs.
            if self.widgets[old].editing() {
                let message = self.widgets[old].finish_edit(true);
                if let Some(message) = message {
                    self.messages.push(message);
                }
                sink.invalidate(self.bounds(old));
            }
            sink.set_caret_rect(None);
            if sink.editing_id().is_some() {
                sink.end_edit();
            }
            // Focus left the field, so a gamepad-raised shell keyboard goes
            // with it (the session above is already committed).
            self.end_soft_keyboard(sink);
            let id = self.widgets[old].id();
            match self.run_focus_hook(id, false, reason) {
                FocusOutcome::Cancel => return,
                FocusOutcome::Redirect(target) if allow_redirect => {
                    if let Some(target_index) = self.index_of(target) {
                        self.focus_move(
                            Some(target_index),
                            sink,
                            keyboard,
                            reason,
                            false,
                        );
                    }
                    return;
                }
                _ => {}
            }
        }
        // Focus is definitely leaving `previous` now (the container hook did
        // not cancel or redirect): offer its additive lost observer (D109).
        if let Some(old) = previous {
            let id = self.widgets[old].id();
            if let Some(message) =
                self.widgets[old].observe_focus_lost(FocusEvent { id, reason })
            {
                self.messages.push(message);
            }
        }
        if let Some(old) = previous {
            let bounds_before = self.bounds(old);
            if self.widgets[old].dismiss() {
                sink.invalidate(bounds_before.union(self.bounds(old)));
            }
        }
        let was_visible = self.focus_visible;
        let was_adjusting = self.adjusting;
        self.focus_visible = keyboard;
        if let Some(old) = previous {
            sink.invalidate(self.bounds(old));
        }
        self.focus = index;
        self.adjusting = false;
        if let Some(new) = index {
            sink.invalidate(self.bounds(new));
        }
        if let Some(new) = index {
            let id = self.widgets[new].id();
            if self.run_focus_hook(id, true, reason) == FocusOutcome::Cancel {
                // The gained hook vetoed the move: revert without re-running
                // hooks so a pair of hooks cannot ping-pong.
                self.focus = previous;
                self.focus_visible = was_visible;
                self.adjusting = was_adjusting;
                if let Some(old) = previous {
                    sink.invalidate(self.bounds(old));
                }
                sink.invalidate(self.bounds(new));
            } else if let Some(message) =
                self.widgets[new].observe_focus_gained(FocusEvent { id, reason })
            {
                self.messages.push(message);
            }
        }
        // Keyboard/a11y focus may have landed on a widget inside a scroll view:
        // the panel scrolls the least distance that shows it. A pointer press
        // never scrolls: the user clicked the part that is already visible, and
        // moving the content under the cursor would undo the click.
        if reason != FocusReason::Pointer {
            self.reveal_focused(sink);
        }
        // Keyboard/a11y focus gets the same dwell as hover; pointer focus is
        // already covered by the hover target.
        self.refresh_tooltip(sink);
    }

    fn run_focus_hook(&mut self, id: WidgetId, gained: bool, reason: FocusReason) -> FocusOutcome {
        let event = FocusEvent { id, reason };
        let Some(hooks) = self.focus_hooks.iter_mut().find(|hooks| hooks.id == id) else {
            return FocusOutcome::Allow;
        };
        let handler = if gained {
            hooks.gained.as_mut()
        } else {
            hooks.lost.as_mut()
        };
        handler.map(|handler| handler(event)).unwrap_or_default()
    }
}

/// The character an `Alt+key` access key stands for.
fn access_char(code: KeyCode) -> Option<char> {
    match code {
        KeyCode::Letter(letter) if letter < 26 => Some((b'A' + letter) as char),
        KeyCode::Digit(digit) if digit < 10 => Some((b'0' + digit) as char),
        _ => None,
    }
}

fn outline(canvas: &mut dyn Canvas, rect: Rect, color: crate::Color, width: f32) {
    if rect.is_empty() {
        return;
    }
    canvas.fill_rect(
        Rect::new(rect.left, rect.top, rect.right, rect.top + width),
        color,
    );
    canvas.fill_rect(
        Rect::new(rect.left, rect.bottom - width, rect.right, rect.bottom),
        color,
    );
    canvas.fill_rect(
        Rect::new(rect.left, rect.top, rect.left + width, rect.bottom),
        color,
    );
    canvas.fill_rect(
        Rect::new(rect.right - width, rect.top, rect.right, rect.bottom),
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y::A11yRole;
    use crate::event::MouseButton;
    use crate::menu::{Menu, MenuSource};
    use crate::scroll::{ScrollChild, ScrollView};
    use crate::widgets::{
        Button, Card, ComboBox, ListView, MenuButton, NumberInput, Pager, ProgressBar, Scrim,
        SectionTitle, Segmented, Slider, Tabs, TextEdit, Time, TimePicker, Toast, Toggle,
    };

    #[derive(Default)]
    struct Sink {
        rects: Vec<Rect>,
        began: Vec<(WidgetId, Rect, String)>,
        began_font: Vec<(Option<f32>, Option<String>)>,
        ended: usize,
        editing: Option<WidgetId>,
        moves: Vec<Rect>,
        tooltips: Vec<Option<(String, Rect, Option<usize>, TooltipAnchor)>>,
        menus: Vec<(WidgetId, Point, Menu, MenuSource)>,
        caret: Option<Rect>,
        clipboard: Option<String>,
        copied: Vec<String>,
        /// `set_soft_keyboard` calls, in order; the fake system state follows
        /// the last request unless a test changes `soft_open` by hand.
        soft: Vec<bool>,
        soft_open: bool,
        /// The platform has no on-screen keyboard (the request is refused).
        refuse_keyboard: bool,
        /// `set_touch_keyboard` calls, in order (a touch-opened session).
        soft_touch: Vec<bool>,
    }

    impl Sink {
        fn take(&mut self) -> Vec<Rect> {
            std::mem::take(&mut self.rects)
        }
    }

    impl Invalidator for Sink {
        fn invalidate(&mut self, rect: Rect) {
            self.rects.push(rect);
        }
    }

    impl UiHost for Sink {
        fn begin_edit(&mut self, request: &EditRequest<'_>) {
            self.began
                .push((request.id, request.rect, request.text.to_string()));
            self.began_font
                .push((request.font_size, request.font_family.clone()));
            self.editing = Some(request.id);
        }

        fn end_edit(&mut self) {
            self.ended += 1;
            self.editing = None;
        }

        fn move_edit(&mut self, rect: Rect) {
            self.moves.push(rect);
        }

        fn editing_id(&self) -> Option<WidgetId> {
            self.editing
        }

        fn set_tooltip(&mut self, target: Option<TooltipRequest<'_>>) {
            self.tooltips.push(target.map(|target| {
                (
                    target.text.to_string(),
                    target.widget,
                    target.item,
                    target.anchor,
                )
            }));
        }

        fn show_context_menu(&mut self, request: ContextMenuRequest<'_>) {
            self.menus.push((
                request.widget,
                request.anchor,
                request.menu.clone(),
                request.source,
            ));
        }

        fn clipboard_text(&mut self) -> Option<String> {
            self.clipboard.clone()
        }

        fn copy_to_clipboard(&mut self, text: &str) {
            self.copied.push(text.to_string());
        }

        fn set_caret_rect(&mut self, rect: Option<Rect>) {
            self.caret = rect;
        }

        fn set_soft_keyboard(&mut self, visible: bool) -> bool {
            self.soft.push(visible);
            if !self.refuse_keyboard {
                self.soft_open = visible;
            }
            !self.refuse_keyboard
        }

        fn soft_keyboard_open(&self) -> bool {
            self.soft_open
        }

        fn set_touch_keyboard(&mut self, visible: bool) -> bool {
            self.soft_touch.push(visible);
            if !self.refuse_keyboard {
                self.soft_open = visible;
            }
            !self.refuse_keyboard
        }
    }

    fn two_buttons() -> (Ui<u32>, Rect, Rect) {
        let a = Rect::from_xywh(0.0, 0.0, 100.0, 36.0);
        let b = Rect::from_xywh(0.0, 50.0, 100.0, 86.0);
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("a", "A", 1u32));
        ui.add(Button::new("b", "B", 2u32));
        ui.set_rect(WidgetId::new("a"), a);
        ui.set_rect(WidgetId::new("b"), b);
        (ui, a, b)
    }

    /// A page of plain widgets for the transition budget: touching one of
    /// them may repaint that widget (and the focus ring it moves), never the
    /// page.
    fn budget_page() -> (Ui<u32>, Vec<Rect>) {
        const IDS: [&str; 12] = [
            "t0", "t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8", "t9", "t10", "t11",
        ];
        let mut ui: Ui<u32> = Ui::new();
        let mut areas = Vec::new();
        for (index, id) in IDS.iter().enumerate() {
            ui.add(Button::new(id, "b", index as u32));
            let rect = Rect::from_xywh(
                (index % 4) as f32 * 120.0,
                (index / 4) as f32 * 50.0,
                100.0,
                36.0,
            );
            ui.set_rect(WidgetId::new(id), rect);
            areas.push(rect);
        }
        (ui, areas)
    }

    #[test]
    fn hover_and_press_stay_inside_the_transition_budget() {
        let (mut ui, areas) = budget_page();
        // One widget's area is the most a transition may repaint; a page-wide
        // invalidation (a union of several widgets) is far above it.
        let largest = areas
            .iter()
            .map(|rect| rect.width() * rect.height())
            .fold(0.0, f32::max);
        let mut sink = Sink::default();
        let mut spent = 0.0f32;
        let mut transitions = 0u32;
        for (index, rect) in areas.iter().enumerate() {
            let pos = rect.center();
            ui.pointer_move(pos, &mut sink);
            let hovered = sink.take();
            assert!(
                hovered.len() <= 2,
                "hover step {index} repainted {} rects: {hovered:?}",
                hovered.len()
            );
            for piece in &hovered {
                assert!(
                    piece.width() * piece.height() <= largest + 0.5,
                    "hover step {index} repainted beyond one widget: {piece:?}"
                );
            }
            spent += hovered
                .iter()
                .map(|piece| piece.width() * piece.height())
                .sum::<f32>();
            transitions += 1;

            // Moving inside the hovered widget changes no state.
            ui.pointer_move(Point::new(pos.x + 8.0, pos.y + 2.0), &mut sink);
            assert!(
                sink.take().is_empty(),
                "moving inside step {index} repainted the page"
            );

            // Press and release repaint the widget, its pressed state and
            // the focus ring it moves (all widget-sized), nothing else.
            ui.pointer_down(pos, &mut sink);
            let pressed = sink.take();
            assert!(pressed.len() <= 3, "press step {index}: {pressed:?}");
            for piece in &pressed {
                assert!(
                    piece.width() * piece.height() <= largest + 0.5,
                    "press step {index} repainted beyond one widget: {piece:?}"
                );
            }
            spent += pressed
                .iter()
                .map(|piece| piece.width() * piece.height())
                .sum::<f32>();
            transitions += 1;

            ui.pointer_up(pos, &mut sink);
            let released = sink.take();
            assert!(released.len() <= 2, "release step {index}: {released:?}");
            for piece in &released {
                assert!(
                    piece.width() * piece.height() <= largest + 0.5,
                    "release step {index} repainted beyond one widget: {piece:?}"
                );
            }
            spent += released
                .iter()
                .map(|piece| piece.width() * piece.height())
                .sum::<f32>();
            transitions += 1;
            ui.drain_messages();
        }
        // The budget: at most three widget-sized rects per transition over a
        // whole hover/press tour (hover: two; press: old ring, new ring and the
        // pressed face). A page repaint is 12 widgets and fails here.
        let budget = transitions as f32 * 3.0 * largest;
        assert!(
            spent <= budget + 0.5,
            "repainted {spent} DIP² over {transitions} transitions (budget {budget})"
        );
    }

    #[test]
    fn hover_change_invalidates_exactly_two_rects() {
        let (mut ui, a, b) = two_buttons();
        let mut sink = Sink::default();
        assert!(ui.pointer_move(Point::new(10.0, 10.0), &mut sink));
        assert_eq!(sink.take(), vec![a]);
        // Moving inside the same widget repaints nothing.
        ui.pointer_move(Point::new(20.0, 15.0), &mut sink);
        assert!(sink.take().is_empty());
        // Entering another widget repaints the one left and the one entered.
        ui.pointer_move(Point::new(10.0, 60.0), &mut sink);
        let rects = sink.take();
        assert_eq!(rects.len(), 2);
        assert!(rects.contains(&a));
        assert!(rects.contains(&b));
    }

    #[test]
    fn moving_inside_a_tab_or_segmented_cell_repaints_nothing() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Tabs::new("tabs", &["a", "b", "c", "d"], 0, |index| index as u32));
        ui.add(Segmented::new("seg", &["a", "b", "c", "d"], 0, |index| {
            index as u32
        }));
        let tabs = Rect::from_xywh(0.0, 0.0, 400.0, 32.0);
        let seg = Rect::from_xywh(0.0, 40.0, 400.0, 32.0);
        ui.set_rect(WidgetId::new("tabs"), tabs);
        ui.set_rect(WidgetId::new("seg"), seg);
        let mut sink = Sink::default();

        // Entering repaints the strip once.
        ui.pointer_move(Point::new(10.0, 16.0), &mut sink);
        assert!(!sink.take().is_empty());
        // Moving inside the same tab is not a visual change.
        ui.pointer_move(Point::new(20.0, 18.0), &mut sink);
        assert!(sink.take().is_empty());
        // Crossing into the next tab repaints only the two cells, not 0..400.
        ui.pointer_move(Point::new(150.0, 16.0), &mut sink);
        let rects = sink.take();
        assert_eq!(rects.len(), 1);
        assert!(
            rects[0].right <= 200.5,
            "the hint narrows to the old/new cell: {rects:?}"
        );

        // The same rule for a segmented control.
        ui.pointer_move(Point::new(10.0, 56.0), &mut sink);
        assert!(!sink.take().is_empty());
        ui.pointer_move(Point::new(20.0, 58.0), &mut sink);
        assert!(sink.take().is_empty());
        ui.pointer_move(Point::new(150.0, 56.0), &mut sink);
        let rects = sink.take();
        assert_eq!(rects.len(), 1);
        assert!(
            rects[0].right <= 200.5,
            "the segmented hint narrows to two cells: {rects:?}"
        );
    }

    fn scrollable_list() -> (Ui<usize>, Rect) {
        let area = Rect::from_xywh(0.0, 0.0, 200.0, 200.0);
        let mut ui: Ui<usize> = Ui::new();
        ui.add(ListView::new("list").row_height(40.0).on_activate(|index| index));
        ui.set_rect(WidgetId::new("list"), area);
        ui.set_rows(
            WidgetId::new("list"),
            (0..50).map(|index| Row::new(format!("r{index}"))).collect(),
        );
        (ui, area)
    }

    #[test]
    fn moving_inside_a_list_row_repaints_nothing() {
        let (mut ui, _) = scrollable_list();
        let mut sink = Sink::default();
        // Enter row 2; the first hover repaints the widget.
        ui.pointer_move(Point::new(20.0, 90.0), &mut sink);
        assert!(!sink.take().is_empty());
        // Moving inside the same row changes no hover state.
        ui.pointer_move(Point::new(120.0, 100.0), &mut sink);
        assert!(sink.take().is_empty());
        // Moving to the next row repaints (the row highlight moved).
        ui.pointer_move(Point::new(120.0, 130.0), &mut sink);
        assert!(!sink.take().is_empty());
    }

    #[test]
    fn hiding_a_scroll_panel_child_repaints_the_whole_viewport() {
        // The panel's bars span the viewport, while the two children cover only
        // its top: hiding them must repaint the bar area below them too, or the
        // old bar stays on screen after the page switches away.
        let viewport = Rect::from_xywh(0.0, 0.0, 200.0, 200.0);
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("a", "A", 1u32));
        ui.add(Button::new("b", "B", 2u32));
        ui.set_rect(WidgetId::new("a"), Rect::from_xywh(0.0, 0.0, 200.0, 30.0));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 34.0, 200.0, 30.0));
        let mut sink = Sink::default();
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                viewport,
                crate::widget::ScrollBars::Vertical,
            )
            .content(crate::geometry::Size::new(200.0, 400.0))
            .child(WidgetId::new("a"), Rect::from_xywh(0.0, 0.0, 200.0, 30.0))
            .child(WidgetId::new("b"), Rect::from_xywh(0.0, 34.0, 200.0, 60.0)),
            &mut sink,
        );
        sink.take();
        ui.set_visible(WidgetId::new("a"), false, &mut sink);
        ui.set_visible(WidgetId::new("b"), false, &mut sink);
        let rects = sink.take();
        assert!(
            rects.contains(&viewport),
            "hiding the last child must repaint the panel viewport: {rects:?}"
        );
        // The same holds when the child is removed instead of hidden.
        ui.set_visible(WidgetId::new("a"), true, &mut sink);
        ui.set_visible(WidgetId::new("b"), true, &mut sink);
        sink.take();
        ui.remove(WidgetId::new("a"), &mut sink);
        ui.remove(WidgetId::new("b"), &mut sink);
        assert!(
            sink.take().contains(&viewport),
            "removing the last child must repaint the panel viewport"
        );
    }

    #[test]
    fn list_bar_press_and_release_repaint_only_the_bar() {
        let (mut ui, area) = scrollable_list();
        // The list is already focused, so a press does not also repaint for a
        // focus move.
        ui.set_focus(WidgetId::new("list"), &mut Sink::default());
        let mut sink = Sink::default();
        // The vertical track is the last 10 DIPs; the thumb starts at the top.
        let bar = Rect::new(area.right - 10.0, area.top, area.right, area.bottom);
        let thumb = Point::new(bar.center().x, 10.0);
        ui.pointer_move(thumb, &mut sink);
        sink.take();
        ui.pointer_down(thumb, &mut sink);
        let pressed = sink.take();
        assert!(!pressed.is_empty());
        assert!(pressed.iter().all(|rect| *rect == bar), "{pressed:?}");
        ui.pointer_up(thumb, &mut sink);
        let released = sink.take();
        assert!(!released.is_empty());
        assert!(released.iter().all(|rect| *rect == bar), "{released:?}");
    }

    #[test]
    fn click_emits_one_message() {
        let (mut ui, _, _) = two_buttons();
        let mut sink = Sink::default();
        ui.pointer_down(Point::new(10.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(10.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec![1]);
        assert!(ui.drain_messages().is_empty());
    }

    #[test]
    fn right_click_release_requests_the_widget_menu_at_the_pointer() {
        let (mut ui, _, _) = two_buttons();
        ui.set_context_menu(
            WidgetId::new("a"),
            Some(Menu::new().item(1, "Open").separator().item(2, "Close")),
        );
        let mut sink = Sink::default();
        assert!(ui.pointer_down_button(
            Point::new(10.0, 10.0),
            MouseButton::Right,
            &mut sink
        ));
        assert!(sink.menus.is_empty(), "the menu opens on release");
        ui.pointer_up_button(Point::new(12.0, 11.0), MouseButton::Right, &mut sink);
        assert_eq!(sink.menus.len(), 1);
        let (widget, anchor, menu, source) = &sink.menus[0];
        assert_eq!(*widget, WidgetId::new("a"));
        assert_eq!(*anchor, Point::new(12.0, 11.0));
        assert_eq!(*source, crate::menu::MenuSource::ContextMenu);
        assert_eq!(menu.len(), 3);
        assert_eq!(menu.command(2).map(|item| item.label()), Some("Close"));
        // The menu replaced any pending tooltip.
        assert_eq!(sink.tooltips.last(), Some(&None));
    }

    #[test]
    fn right_click_without_a_menu_or_while_editing_requests_nothing() {
        // No menu installed: the secondary click still runs its binding path
        // but asks the host for nothing.
        let (mut ui, _, _) = two_buttons();
        let mut sink = Sink::default();
        ui.pointer_down_button(Point::new(10.0, 10.0), MouseButton::Right, &mut sink);
        ui.pointer_up_button(Point::new(10.0, 10.0), MouseButton::Right, &mut sink);
        assert!(sink.menus.is_empty());

        // With a menu, but the native editor owns the field: the child shows
        // its own EDIT menu, so the widget menu stays closed.
        ui.set_context_menu(WidgetId::new("a"), Some(Menu::new().item(1, "Open")));
        sink.editing = Some(WidgetId::new("a"));
        ui.pointer_down_button(Point::new(10.0, 10.0), MouseButton::Right, &mut sink);
        ui.pointer_up_button(Point::new(10.0, 10.0), MouseButton::Right, &mut sink);
        assert!(sink.menus.is_empty());

        // An empty menu is treated like no menu.
        sink.editing = None;
        ui.set_context_menu(WidgetId::new("a"), Some(Menu::new()));
        ui.pointer_down_button(Point::new(10.0, 10.0), MouseButton::Right, &mut sink);
        ui.pointer_up_button(Point::new(10.0, 10.0), MouseButton::Right, &mut sink);
        assert!(sink.menus.is_empty());
    }

    #[test]
    fn menu_key_and_shift_f10_open_the_focused_widgets_menu() {
        let (mut ui, _, b) = two_buttons();
        ui.set_context_menu(WidgetId::new("b"), Some(Menu::new().item(7, "Rename")));
        let mut sink = Sink::default();
        ui.key(key_event(KeyCode::Tab), &mut sink);
        ui.key(key_event(KeyCode::Tab), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("b")));

        let apps = KeyEvent {
            code: KeyCode::Other(0x5D),
            vk: 0x5D,
            ..key_event(KeyCode::Other(0x5D))
        };
        assert!(ui.key(apps, &mut sink));
        assert_eq!(sink.menus.len(), 1);
        assert_eq!(sink.menus[0].0, WidgetId::new("b"));
        assert_eq!(sink.menus[0].1, Point::new(b.left, b.bottom));

        let shift_f10 = KeyEvent {
            code: KeyCode::Function(10),
            vk: 0x79,
            shift: true,
            ..key_event(KeyCode::Function(10))
        };
        assert!(ui.key(shift_f10, &mut sink));
        assert_eq!(sink.menus.len(), 2);

        // Focus a widget without a menu: the key is not consumed.
        ui.key(key_event(KeyCode::Tab), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("a")));
        assert!(!ui.key(apps, &mut sink));
        assert_eq!(sink.menus.len(), 2);
    }

    #[test]
    fn context_menu_can_be_replaced_and_cleared_at_runtime() {
        let (mut ui, _, _) = two_buttons();
        let id = WidgetId::new("a");
        assert!(ui.context_menu(id).is_none());
        ui.set_context_menu(id, Some(Menu::new().item(1, "One")));
        assert_eq!(ui.context_menu(id).map(Menu::len), Some(1));
        ui.set_context_menu(id, Some(Menu::new().item(2, "Two")));
        assert!(ui.context_menu(id).and_then(|menu| menu.command(1)).is_none());
        assert!(ui.context_menu(id).and_then(|menu| menu.command(2)).is_some());
        ui.set_context_menu(id, None);
        assert!(ui.context_menu(id).is_none());
        // Unknown ids are a no-op.
        ui.set_context_menu(WidgetId::new("missing"), Some(Menu::new().item(3, "X")));
    }

    #[test]
    fn set_menu_installs_the_registry_and_a_menu_buttons_own_model() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(MenuButton::new(
            "btn",
            "文件",
            Menu::new().item(1, "打开"),
            |command| command,
        ));
        let id = WidgetId::new("btn");
        ui.set_rect(id, Rect::from_xywh(10.0, 20.0, 120.0, 30.0));
        // The button owns its model; the registry starts empty.
        assert!(ui.menu(id).is_none());
        let own = ui.with_widget::<MenuButton<u32>, _>(id, |button| button.menu().clone());
        assert_eq!(own.map(|menu| menu.len()), Some(1));

        // A single call updates the registry and the button's own model, so
        // the click path and a programmatic open cannot disagree.
        let edited = Menu::new().item(2, "保存").checked_item(3, "自动保存", true);
        ui.set_menu(id, Some(edited.clone()));
        assert_eq!(ui.menu(id), Some(&edited));
        let own = ui
            .with_widget::<MenuButton<u32>, _>(id, |button| button.menu().clone());
        assert_eq!(own, Some(edited));

        // `open_menu_for` uses the widget's own menu and its own anchor.
        let mut sink = Sink::default();
        assert!(ui.open_menu_for(id, &mut sink));
        let (widget, anchor, menu, source) = &sink.menus[0];
        assert_eq!(*widget, id);
        assert_eq!(*anchor, Point::new(10.0, 50.0));
        assert_eq!(*source, MenuSource::Chrome);
        assert_eq!(menu.command(3).map(|item| item.is_checked()), Some(true));

        // Removing the menu also clears the widget's model.
        ui.remove_menu(id);
        assert!(ui.menu(id).is_none());
        let own = ui
            .with_widget::<MenuButton<u32>, _>(id, |button| button.menu().clone());
        assert!(own.is_some_and(|menu| menu.is_empty()));
        assert!(!ui.open_menu_for(id, &mut sink));
        assert_eq!(sink.menus.len(), 1);
    }

    #[test]
    fn open_menu_shows_a_registered_menu_from_application_code() {
        let (mut ui, _, b) = two_buttons();
        let mut sink = Sink::default();
        let id = WidgetId::new("b");
        assert!(!ui.open_menu(id, Point::new(1.0, 2.0), &mut sink), "no menu yet");

        ui.set_menu(id, Some(Menu::new().item(7, "重命名")));
        assert!(ui.open_menu(id, Point::new(1.0, 2.0), &mut sink));
        let (widget, anchor, menu, source) = &sink.menus[0];
        assert_eq!(*widget, id);
        assert_eq!(*anchor, Point::new(1.0, 2.0));
        assert_eq!(menu.len(), 1);
        assert_eq!(*source, MenuSource::Programmatic);

        // An empty menu, an unknown widget and an open native editor ask for
        // nothing.
        ui.set_menu(id, Some(Menu::new()));
        assert!(!ui.open_menu(id, Point::new(1.0, 2.0), &mut sink));
        assert!(!ui.open_menu(WidgetId::new("missing"), Point::new(1.0, 2.0), &mut sink));
        ui.set_menu(id, Some(Menu::new().item(7, "重命名")));
        sink.editing = Some(id);
        assert!(!ui.open_menu(id, Point::new(1.0, 2.0), &mut sink));
        assert_eq!(sink.menus.len(), 1);

        // `open_menu_for` falls back to the registry for a plain widget and
        // anchors below the control without covering it.
        sink.editing = None;
        assert!(ui.open_menu_for(id, &mut sink));
        let (_, anchor, _, source) = &sink.menus[1];
        assert_eq!(*anchor, Point::new(b.left, b.bottom));
        assert_eq!(*source, MenuSource::Chrome);
    }

    #[test]
    fn expand_menu_reopens_a_submenu_as_its_own_root() {
        let (mut ui, _, b) = two_buttons();
        let id = WidgetId::new("b");
        ui.set_menu(
            id,
            Some(
                Menu::new()
                    .item(1, "打开")
                    .submenu("最近", Menu::new().item(2, "A.mp3").item(3, "B.mp3")),
            ),
        );
        let mut sink = Sink::default();
        let path = crate::menu::MenuPath::root().label("最近");
        assert!(ui.expand_menu(id, &path, &mut sink));
        let (widget, anchor, menu, source) = &sink.menus[0];
        assert_eq!(*widget, id);
        assert_eq!(menu.len(), 2);
        assert!(menu.command(2).is_some());
        assert_eq!(*anchor, Point::new(b.left, b.bottom));
        assert_eq!(*source, MenuSource::Programmatic);

        // A path through an item, a missing label or an empty menu is refused.
        let through_item = crate::menu::MenuPath::root().index(0);
        assert!(!ui.expand_menu(id, &through_item, &mut sink));
        let missing = crate::menu::MenuPath::root().label("缺失");
        assert!(!ui.expand_menu(id, &missing, &mut sink));
        assert_eq!(sink.menus.len(), 1);
    }

    #[test]
    fn tab_focuses_the_first_widget_and_enter_activates() {
        let (mut ui, a, _) = two_buttons();
        let mut sink = Sink::default();
        let tab = KeyEvent {
            code: KeyCode::Tab,
            vk: 0x09,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        };
        assert!(ui.key(tab, &mut sink));
        assert_eq!(ui.focus_id(), Some(WidgetId::new("a")));
        assert!(sink.take().contains(&a));
        let enter = KeyEvent {
            code: KeyCode::Enter,
            vk: 0x0D,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        };
        assert!(ui.key(enter, &mut sink));
        assert_eq!(ui.drain_messages(), vec![1]);
    }

    #[test]
    fn escape_leaves_adjust_mode_without_a_message() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Slider::new("s", 0.5, 0.0..=1.0, |value| value.to_bits()));
        ui.set_rect(WidgetId::new("s"), Rect::from_xywh(0.0, 0.0, 200.0, 32.0));
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("s"), &mut sink);
        let key = |code| KeyEvent {
            code,
            vk: 0,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        };
        assert!(ui.key(key(KeyCode::Enter), &mut sink));
        assert!(ui.adjusting());
        ui.key(key(KeyCode::Escape), &mut sink);
        assert!(!ui.adjusting());
        assert!(ui.drain_messages().is_empty());
    }

    #[test]
    fn add_and_state_setters_dispatch_by_id() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("btn", "Go", 1u32));
        ui.set_value(WidgetId::new("btn"), 0.5);
        ui.set_rows(WidgetId::new("btn"), Vec::new());
        assert!(ui.len() == 1);
        assert!(ui.focus_id().is_none());
    }

    #[test]
    fn vertical_layout_stacks_widgets() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("a", "A", 1u32));
        ui.add(Button::new("b", "B", 2u32));
        ui.layout_vertical(Rect::new(0.0, 0.0, 300.0, 200.0), 8.0);
        let a = ui.rect(WidgetId::new("a")).unwrap();
        let b = ui.rect(WidgetId::new("b")).unwrap();
        assert_eq!(a.top, 0.0);
        assert_eq!(b.top, a.bottom + 8.0);
    }

    #[test]
    fn drain_messages_is_empty_without_input() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("btn", "Go", 1u32));
        assert!(ui.drain_messages().is_empty());
    }

    fn text_and_button() -> (Ui<String>, Rect, Rect) {
        let field = Rect::from_xywh(0.0, 0.0, 200.0, 34.0);
        let button = Rect::from_xywh(0.0, 50.0, 120.0, 86.0);
        let mut ui: Ui<String> = Ui::new();
        ui.add(TextEdit::new("name", "Bamboo", |text| text));
        ui.add(Button::new("go", "Go", "go".to_string()));
        ui.set_rect(WidgetId::new("name"), field);
        ui.set_rect(WidgetId::new("go"), button);
        (ui, field, button)
    }

    #[test]
    fn clicking_a_text_edit_opens_a_native_editor() {
        let (mut ui, field, _) = text_and_button();
        let mut sink = Sink::default();
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(
            sink.began,
            vec![(WidgetId::new("name"), field, "Bamboo".to_string())]
        );
        // No application message: the native editor owns input now.
        assert!(ui.drain_messages().is_empty());
    }

    #[test]
    fn enter_on_a_focused_text_edit_opens_a_native_editor() {
        let (mut ui, field, _) = text_and_button();
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("name"), &mut sink);
        sink.began.clear();
        let enter = KeyEvent {
            code: KeyCode::Enter,
            vk: 0x0D,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        };
        assert!(ui.key(enter, &mut sink));
        assert_eq!(
            sink.began,
            vec![(WidgetId::new("name"), field, "Bamboo".to_string())]
        );
    }

    #[test]
    fn a_pad_enter_raises_the_soft_keyboard_and_b_dismisses_it() {
        let (mut ui, _, _) = text_and_button();
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("name"), &mut sink);
        assert!(ui.pad_key(key_event(KeyCode::Enter), &mut sink));
        assert_eq!(sink.soft, vec![true], "A on a text field raises it");
        assert!(sink.editing.is_some());
        // The shell keyboard owns the pad now, so A does not commit the field.
        assert!(ui.pad_key(key_event(KeyCode::Enter), &mut sink));
        assert_eq!(sink.ended, 0);
        assert_eq!(sink.soft, vec![true]);
        // B leaves the field: the native session commits and the keyboard goes.
        assert!(ui.pad_key(key_event(KeyCode::Escape), &mut sink));
        assert_eq!(sink.ended, 1);
        assert!(sink.editing.is_none());
        assert_eq!(sink.soft, vec![true, false]);
    }

    #[test]
    fn a_dismissed_system_keyboard_comes_back_on_a() {
        let (mut ui, _, _) = text_and_button();
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("name"), &mut sink);
        ui.pad_key(key_event(KeyCode::Enter), &mut sink);
        // The user closed the keyboard with its own close button.
        sink.soft_open = false;
        assert!(ui.pad_key(key_event(KeyCode::Enter), &mut sink));
        assert_eq!(sink.ended, 0, "A re-raises instead of committing");
        assert_eq!(sink.soft, vec![true, true]);
    }

    #[test]
    fn hardware_keyboard_text_entry_never_touches_the_soft_keyboard() {
        let (mut ui, _, _) = text_and_button();
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("name"), &mut sink);
        assert!(ui.key(key_event(KeyCode::Enter), &mut sink));
        assert!(sink.editing.is_some());
        assert!(sink.soft.is_empty(), "a keyboard session asks for no keyboard");
        // Tabbing away still ends the session the ordinary way.
        ui.key(key_event(KeyCode::Tab), &mut sink);
        assert!(sink.editing.is_none());
        assert!(sink.soft.is_empty());
    }

    #[test]
    fn clicking_elsewhere_dismisses_a_pad_raised_keyboard() {
        let (mut ui, _, _) = text_and_button();
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("name"), &mut sink);
        ui.pad_key(key_event(KeyCode::Enter), &mut sink);
        ui.pointer_down(Point::new(10.0, 60.0), &mut sink);
        assert_eq!(sink.soft, vec![true, false]);
    }

    #[test]
    fn pad_enter_on_a_painted_field_commits_and_dismisses_on_b() {
        let field = Rect::from_xywh(0.0, 0.0, 120.0, 36.0);
        let mut ui: Ui<String> = Ui::new();
        ui.add(TextEdit::new("name", "Bamboo", |text| text).painted());
        ui.set_rect(WidgetId::new("name"), field);
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("name"), &mut sink);
        assert!(ui.pad_key(key_event(KeyCode::Enter), &mut sink));
        assert_eq!(sink.soft, vec![true]);
        let editing = |ui: &Ui<String>| {
            ui.with_widget::<TextEdit<String>, _>(WidgetId::new("name"), |field| field.is_editing())
                .unwrap()
        };
        assert!(editing(&ui));
        assert!(ui.pad_key(key_event(KeyCode::Escape), &mut sink));
        assert!(!editing(&ui));
        assert_eq!(sink.soft, vec![true, false]);
    }

    #[test]
    fn a_machine_without_the_keyboard_falls_back_to_normal_key_routing() {
        let (mut ui, _, _) = text_and_button();
        let mut sink = Sink::default();
        sink.refuse_keyboard = true;
        ui.set_focus(WidgetId::new("name"), &mut sink);
        assert!(ui.pad_key(key_event(KeyCode::Enter), &mut sink));
        assert!(sink.editing.is_some(), "the field still opens");
        assert_eq!(sink.soft, vec![true]);
        // The request was refused, so the next key is ordinary routing — the
        // pad is never trapped in a session it cannot type into.
        assert!(!sink.soft_open);
        assert!(ui.pad_key(key_event(KeyCode::Enter), &mut sink));
        assert_eq!(sink.ended, 0, "no swallow, no commit either");
        assert_eq!(sink.soft, vec![true]);
    }

    #[test]
    fn leaving_an_editing_widget_commits_the_session() {
        let (mut ui, _, button) = text_and_button();
        let mut sink = Sink::default();
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        assert!(sink.editing.is_some());
        ui.pointer_down(Point::new(10.0, 60.0), &mut sink);
        assert_eq!(sink.ended, 1);
        assert!(sink.editing.is_none());
        assert_eq!(ui.focus_id(), Some(WidgetId::new("go")));
        assert!(sink.rects.contains(&button));
    }

    #[test]
    fn commit_edit_stores_text_and_returns_the_message() {
        let (mut ui, field, _) = text_and_button();
        let mut sink = Sink::default();
        sink.rects.clear();
        let message = ui.commit_edit(WidgetId::new("name"), "New".to_string(), &mut sink);
        assert_eq!(message, Some("New".to_string()));
        assert_eq!(sink.take(), vec![field]);
        // The widget now holds the committed value: reopening reports it.
        assert!(ui.drain_messages().is_empty());
        let mut reopened = Sink::default();
        ui.pointer_down(Point::new(20.0, 10.0), &mut reopened);
        assert_eq!(reopened.began.len(), 1);
        assert_eq!(reopened.began[0].2, "New");
    }

    #[test]
    fn the_number_input_steppers_stay_clickable_while_typing() {
        let mut ui: Ui<i64> = Ui::new();
        ui.add(NumberInput::new("count", 8, 1..=500, |value| value).steppers());
        let rect = Rect::from_xywh(0.0, 0.0, 120.0, 34.0);
        ui.set_rect(WidgetId::new("count"), rect);
        let count = WidgetId::new("count");
        let mut sink = Sink::default();
        // Clicking the text opens the session; the arrows stay drawn and the
        // editor only covers the text area beside them.
        ui.pointer_down(Point::new(40.0, 17.0), &mut sink);
        ui.pointer_up(Point::new(40.0, 17.0), &mut sink);
        assert!(ui
            .with_widget::<NumberInput<i64>, _>(count, |input| input.is_editing())
            .unwrap());
        assert!(ui.key(key_event(KeyCode::End), &mut sink));
        assert!(ui.char('3', &mut sink));
        // A press on the upper arrow commits "83" and steps to 84.
        ui.pointer_down(Point::new(110.0, 6.0), &mut sink);
        ui.pointer_up(Point::new(110.0, 6.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec![84]);
        assert!(!ui
            .with_widget::<NumberInput<i64>, _>(count, |input| input.is_editing())
            .unwrap());
        assert_eq!(
            ui.with_widget::<NumberInput<i64>, _>(count, |input| input.value()),
            Some(84)
        );
    }

    #[test]
    fn number_input_commit_clamps_and_emits_only_on_change() {
        let mut ui: Ui<i64> = Ui::new();
        ui.add(NumberInput::new("count", 5, 1..=10, |value| value));
        ui.set_rect(WidgetId::new("count"), Rect::from_xywh(0.0, 0.0, 120.0, 34.0));
        let mut sink = Sink::default();
        assert_eq!(
            ui.commit_edit(WidgetId::new("count"), "42".to_string(), &mut sink),
            Some(10)
        );
        assert_eq!(
            ui.commit_edit(WidgetId::new("count"), "10".to_string(), &mut sink),
            None
        );
    }

    #[test]
    fn sync_edit_moves_with_the_widget_rect() {
        struct MovingSink {
            rects: Vec<Rect>,
            editing: Option<WidgetId>,
        }
        impl Invalidator for MovingSink {
            fn invalidate(&mut self, _rect: Rect) {}
        }
        impl UiHost for MovingSink {
            fn move_edit(&mut self, rect: Rect) {
                self.rects.push(rect);
            }
            fn editing_id(&self) -> Option<WidgetId> {
                self.editing
            }
        }
        let (mut ui, _, _) = text_and_button();
        let mut sink = MovingSink {
            rects: Vec::new(),
            editing: Some(WidgetId::new("name")),
        };
        let moved = Rect::from_xywh(10.0, 10.0, 200.0, 34.0);
        ui.set_rect(WidgetId::new("name"), moved);
        ui.sync_edit(&mut sink);
        assert_eq!(sink.rects, vec![moved]);
        sink.editing = None;
        ui.sync_edit(&mut sink);
        assert_eq!(sink.rects, vec![moved]);
    }

    /// Records how many `fill_rect` calls used the palette accent, which is
    /// exactly what the focus ring draws.
    struct AccentCanvas {
        accent: crate::Color,
        fills: usize,
    }

    impl Canvas for AccentCanvas {
        fn scale(&self) -> f32 {
            1.0
        }
        fn dpi(&self) -> f32 {
            96.0
        }
        fn fill_rect(&mut self, _rect: Rect, color: crate::Color) {
            if color == self.accent {
                self.fills += 1;
            }
        }
        fn text(
            &mut self,
            _rect: Rect,
            _text: &str,
            _style: &crate::backend::TextStyle,
            _align: crate::backend::TextAlign,
            _color: crate::Color,
        ) {
        }
        fn measure_text(&mut self, text: &str, _style: &crate::backend::TextStyle) -> crate::Size {
            crate::Size::new(text.chars().count() as f32 * 8.0, 16.0)
        }
        fn ellipse(&mut self, _rect: Rect, _color: crate::Color) {}
        fn vertical_gradient(&mut self, _rect: Rect, _top: crate::Color, _bottom: crate::Color) {}
        fn image(
            &mut self,
            _rect: Rect,
            _crop: bool,
            _id: crate::backend::ImageId,
            _data: crate::backend::ImageData<'_>,
        ) {
        }
        fn push_clip(&mut self, _rect: Rect) {}
        fn pop_clip(&mut self) {}
    }

    fn paint_and_count_accent(ui: &mut Ui<u32>) -> usize {
        let painter = crate::painter::Painter::new();
        let accent = painter.palette().accent;
        let mut canvas = AccentCanvas { accent, fills: 0 };
        let dirty = vec![Rect::from_xywh(0.0, 0.0, 200.0, 100.0)];
        let mut ctx = PaintCtx {
            painter: &painter,
            canvas: &mut canvas,
            dirty: &dirty,
            size: crate::Size::new(200.0, 100.0),
            dpi: 96.0,
            editing: None,
            caret_on: true,
            overlay: false,
            overlay_first: false,
            caret: None,
        };
        ui.paint(&mut ctx);
        canvas.fills
    }

    /// Records every string painted, so a test can tell which widgets painted.
    #[derive(Default)]
    struct TextCanvas {
        texts: Vec<String>,
    }

    impl Canvas for TextCanvas {
        fn scale(&self) -> f32 {
            1.0
        }
        fn dpi(&self) -> f32 {
            96.0
        }
        fn fill_rect(&mut self, _rect: Rect, _color: crate::Color) {}
        fn fill_translucent(&mut self, _rect: Rect, _color: crate::Color, _alpha: f32) {}
        fn text(
            &mut self,
            _rect: Rect,
            text: &str,
            _style: &crate::backend::TextStyle,
            _align: crate::backend::TextAlign,
            _color: crate::Color,
        ) {
            self.texts.push(text.to_string());
        }
        fn measure_text(&mut self, text: &str, _style: &crate::backend::TextStyle) -> crate::Size {
            crate::Size::new(text.chars().count() as f32 * 8.0, 16.0)
        }
        fn ellipse(&mut self, _rect: Rect, _color: crate::Color) {}
        fn vertical_gradient(&mut self, _rect: Rect, _top: crate::Color, _bottom: crate::Color) {}
        fn image(
            &mut self,
            _rect: Rect,
            _crop: bool,
            _id: crate::backend::ImageId,
            _data: crate::backend::ImageData<'_>,
        ) {
        }
        fn push_clip(&mut self, _rect: Rect) {}
        fn pop_clip(&mut self) {}
    }

    fn paint_texts(ui: &mut Ui<u32>, overlay: bool, first: bool) -> Vec<String> {
        let painter = crate::painter::Painter::new();
        let mut canvas = TextCanvas::default();
        let dirty = vec![Rect::from_xywh(0.0, 0.0, 300.0, 200.0)];
        let mut ctx = PaintCtx {
            painter: &painter,
            canvas: &mut canvas,
            dirty: &dirty,
            size: crate::Size::new(300.0, 200.0),
            dpi: 96.0,
            editing: None,
            caret_on: true,
            overlay,
            overlay_first: first,
            caret: None,
        };
        ui.paint(&mut ctx);
        canvas.texts
    }

    #[test]
    fn an_overlay_paint_skips_the_page_and_the_backdrop_after_the_first_frame() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("page_btn", "页面按钮", 1));
        ui.add(Scrim::new("scrim"));
        ui.add(Button::new("panel_btn", "面板按钮", 2));
        for (id, rect) in [
            ("page_btn", Rect::from_xywh(0.0, 0.0, 100.0, 40.0)),
            ("scrim", Rect::from_xywh(0.0, 0.0, 300.0, 200.0)),
            ("panel_btn", Rect::from_xywh(40.0, 60.0, 120.0, 40.0)),
        ] {
            ui.set_rect(WidgetId::new(id), rect);
        }
        assert!(ui.overlay_active());
        let split = ui.overlay_split().expect("a backdrop is up");
        assert_eq!(ui.widgets[split].id(), WidgetId::new("scrim"));
        // A normal paint draws the page and the panel.
        let texts = paint_texts(&mut ui, false, false);
        assert!(texts.iter().any(|text| text == "页面按钮"), "{texts:?}");
        assert!(texts.iter().any(|text| text == "面板按钮"), "{texts:?}");
        // The first overlay frame draws the backdrop and the panel only.
        let texts = paint_texts(&mut ui, true, true);
        assert!(!texts.iter().any(|text| text == "页面按钮"), "{texts:?}");
        assert!(texts.iter().any(|text| text == "面板按钮"), "{texts:?}");
        // Later frames leave the dimmed page AND the backdrop alone.
        let texts = paint_texts(&mut ui, true, false);
        assert!(!texts.iter().any(|text| text == "页面按钮"), "{texts:?}");
        assert!(texts.iter().any(|text| text == "面板按钮"), "{texts:?}");
        // Without a visible backdrop there is no split.
        let scrim = WidgetId::new("scrim");
        let mut sink = Sink::default();
        ui.set_visible(scrim, false, &mut sink);
        assert!(!ui.overlay_active());
    }

    /// Paint a painted-field page so its layout/caret cache is populated, and
    /// return the caret the paint reported.
    /// Paint one full page through the test canvas, so the painted editors'
    /// layout caches exist (their geometry drives pointer hit-testing).
    fn paint_page<M>(ui: &mut Ui<M>) -> Option<Rect> {
        let painter = crate::painter::Painter::new();
        let mut canvas = AccentCanvas {
            accent: crate::Color::new(0, 0, 0),
            fills: 0,
        };
        let dirty = vec![Rect::from_xywh(0.0, 0.0, 400.0, 300.0)];
        let mut ctx = PaintCtx {
            painter: &painter,
            canvas: &mut canvas,
            dirty: &dirty,
            size: crate::Size::new(400.0, 300.0),
            dpi: 96.0,
            editing: None,
            caret_on: true,
            overlay: false,
            overlay_first: false,
            caret: None,
        };
        ui.paint(&mut ctx);
        ctx.caret
    }

    fn paint_painted_page(ui: &mut Ui<String>) -> Option<Rect> {
        paint_page(ui)
    }

    #[test]
    fn focus_ring_is_keyboard_only() {
        let (mut ui, _, _) = two_buttons();
        let mut sink = Sink::default();
        // Pointer focus must not draw the accessibility ring.
        ui.pointer_down(Point::new(10.0, 10.0), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("a")));
        assert!(!ui.focus_visible());
        assert_eq!(paint_and_count_accent(&mut ui), 0);
        // Tab turns it on: four 1 DIP accent bars.
        let tab = KeyEvent {
            code: KeyCode::Tab,
            vk: 0x09,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        };
        ui.key(tab, &mut sink);
        assert!(ui.focus_visible());
        assert_eq!(paint_and_count_accent(&mut ui), 4);
    }

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            vk: 0,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        }
    }

    fn key_event_mod(code: KeyCode, ctrl: bool) -> KeyEvent {
        KeyEvent {
            ctrl,
            ..key_event(code)
        }
    }

    fn combo_ui() -> (Ui<usize>, Rect) {
        let mut ui: Ui<usize> = Ui::new();
        ui.add(ComboBox::new("combo", &["甲", "乙", "丙"], 0, |index| index));
        let box_rect = Rect::from_xywh(0.0, 0.0, 200.0, 32.0);
        ui.set_rect(WidgetId::new("combo"), box_rect);
        (ui, box_rect)
    }

    fn combo_open(ui: &Ui<usize>) -> bool {
        ui.with_widget(WidgetId::new("combo"), |combo: &ComboBox<usize>| {
            combo.is_open()
        })
        .unwrap()
    }

    fn open_combo(ui: &mut Ui<usize>, sink: &mut Sink) {
        ui.pointer_down(Point::new(100.0, 16.0), sink);
        ui.pointer_up(Point::new(100.0, 16.0), sink);
        assert!(combo_open(ui));
    }

    #[test]
    fn escape_closes_an_open_combo_and_repaints_the_dropdown() {
        let (mut ui, box_rect) = combo_ui();
        let mut sink = Sink::default();
        open_combo(&mut ui, &mut sink);
        sink.take();

        assert!(ui.key(key_event(KeyCode::Escape), &mut sink));
        assert!(!combo_open(&ui));
        let rects = sink.take();
        assert!(
            rects
                .iter()
                .any(|rect| rect.bottom >= box_rect.bottom + 70.0),
            "the vacated drop-down area must be repainted: {rects:?}"
        );
    }

    #[test]
    fn arrows_move_an_open_combos_highlight_and_enter_selects() {
        let (mut ui, _) = combo_ui();
        let mut sink = Sink::default();
        open_combo(&mut ui, &mut sink);

        // Without the overlay owning the keys these would move focus instead.
        ui.key(key_event(KeyCode::Down), &mut sink);
        ui.key(key_event(KeyCode::Down), &mut sink);
        assert!(ui.key(key_event(KeyCode::Enter), &mut sink));
        assert_eq!(ui.drain_messages(), vec![2]);
        assert!(!combo_open(&ui));
    }

    #[test]
    fn clicking_the_box_again_closes_the_list_and_repaints_it() {
        let (mut ui, box_rect) = combo_ui();
        let mut sink = Sink::default();
        open_combo(&mut ui, &mut sink);
        sink.take();

        ui.pointer_down(Point::new(100.0, 16.0), &mut sink);
        ui.pointer_up(Point::new(100.0, 16.0), &mut sink);
        assert!(!combo_open(&ui));
        let rects = sink.take();
        assert!(
            rects
                .iter()
                .any(|rect| rect.bottom >= box_rect.bottom + 70.0),
            "the vacated drop-down area must be repainted: {rects:?}"
        );
    }

    #[test]
    fn hiding_or_disabling_an_open_combo_closes_it_and_repaints_it() {
        let (mut ui, box_rect) = combo_ui();
        let mut sink = Sink::default();
        open_combo(&mut ui, &mut sink);
        sink.take();

        ui.set_visible(WidgetId::new("combo"), false, &mut sink);
        assert!(!combo_open(&ui), "hiding must close the drop-down");
        assert!(
            sink.take()
                .iter()
                .any(|rect| rect.bottom >= box_rect.bottom + 70.0),
            "hiding must repaint the drop-down area"
        );
        // Showing it again must not resurrect the list.
        ui.set_visible(WidgetId::new("combo"), true, &mut sink);
        assert!(!combo_open(&ui));

        open_combo(&mut ui, &mut sink);
        ui.set_enabled(WidgetId::new("combo"), false, &mut sink);
        assert!(!combo_open(&ui), "disabling must close the drop-down");

        ui.set_enabled(WidgetId::new("combo"), true, &mut sink);
        open_combo(&mut ui, &mut sink);
        ui.set_props(
            WidgetId::new("combo"),
            WidgetProps::new().visible(false),
            &mut sink,
        );
        assert!(!combo_open(&ui), "hidden props must close the drop-down");
    }

    #[test]
    fn list_arrows_work_only_in_adjust_mode() {
        let list = Rect::from_xywh(0.0, 0.0, 200.0, 200.0);
        let button = Rect::from_xywh(0.0, 220.0, 100.0, 256.0);
        let mut ui: Ui<usize> = Ui::new();
        ui.add(ListView::new("list").on_activate(|index| index));
        ui.add(Button::new("b", "B", 99usize));
        ui.set_rect(WidgetId::new("list"), list);
        ui.set_rect(WidgetId::new("b"), button);
        ui.set_rows(
            WidgetId::new("list"),
            (0..50).map(|i| Row::new(format!("row {i}"))).collect(),
        );
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("list"), &mut sink);
        // Not adjustable yet: arrows move focus like any other widget.
        assert!(ui.key(key_event(KeyCode::Down), &mut sink));
        assert_eq!(ui.focus_id(), Some(WidgetId::new("b")));
        // Enter enters row navigation; arrows then stay in the list.
        ui.set_focus(WidgetId::new("list"), &mut sink);
        assert!(ui.key(key_event(KeyCode::Enter), &mut sink));
        assert!(ui.adjusting());
        ui.key(key_event(KeyCode::Down), &mut sink);
        ui.key(key_event(KeyCode::Down), &mut sink);
        ui.key(key_event(KeyCode::PageDown), &mut sink);
        assert!(ui.adjusting());
        assert_eq!(ui.focus_id(), Some(WidgetId::new("list")));
        // Enter again activates the row cursor (two Down + one PageDown = 4).
        assert!(ui.key(key_event(KeyCode::Enter), &mut sink));
        assert_eq!(ui.drain_messages(), vec![6]);
        // Escape leaves adjust mode; the next Tab moves focus.
        ui.key(key_event(KeyCode::Escape), &mut sink);
        assert!(!ui.adjusting());
        ui.key(key_event(KeyCode::Tab), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("b")));
    }

    #[test]
    fn segment_arrows_work_only_in_adjust_mode() {
        let mut ui: Ui<usize> = Ui::new();
        ui.add(Segmented::new("seg", &["a", "b", "c"], 0, |index| index));
        ui.add(Button::new("b", "B", 99usize));
        ui.set_rect(WidgetId::new("seg"), Rect::from_xywh(0.0, 0.0, 300.0, 32.0));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 40.0, 100.0, 76.0));
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("seg"), &mut sink);
        // A plain arrow must not change the selection by accident.
        assert!(ui.key(key_event(KeyCode::Right), &mut sink));
        assert_eq!(ui.focus_id(), Some(WidgetId::new("b")));
        assert!(ui.drain_messages().is_empty());
        // Enter enters modify mode; arrows then change the selection.
        ui.set_focus(WidgetId::new("seg"), &mut sink);
        assert!(ui.key(key_event(KeyCode::Enter), &mut sink));
        assert!(ui.adjusting());
        assert!(ui.key(key_event(KeyCode::Right), &mut sink));
        assert_eq!(ui.focus_id(), Some(WidgetId::new("seg")));
        assert_eq!(ui.drain_messages(), vec![1]);
        assert!(ui.key(key_event(KeyCode::Left), &mut sink));
        assert_eq!(ui.drain_messages(), vec![0]);
        // Backspace leaves modify mode, then Tab moves focus.
        assert!(ui.key(key_event(KeyCode::Backspace), &mut sink));
        assert!(!ui.adjusting());
        ui.key(
            KeyEvent {
                code: KeyCode::Tab,
                vk: 0x09,
                ctrl: false,
                shift: false,
                alt: false,
                repeat: false,
            },
            &mut sink,
        );
        assert_eq!(ui.focus_id(), Some(WidgetId::new("b")));
    }

    #[test]
    fn backspace_cancels_adjustment_or_releases_focus() {
        let (mut ui, _, _) = two_buttons();
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("a"), &mut sink);
        assert!(ui.key(key_event(KeyCode::Backspace), &mut sink));
        assert_eq!(ui.focus_id(), None);
        // Nothing focused: the key is not consumed.
        assert!(!ui.key(key_event(KeyCode::Backspace), &mut sink));

        let mut slider_ui: Ui<u32> = Ui::new();
        slider_ui.add(Slider::new("s", 0.5, 0.0..=1.0, |value| value.to_bits()));
        slider_ui.set_rect(WidgetId::new("s"), Rect::from_xywh(0.0, 0.0, 200.0, 32.0));
        slider_ui.set_focus(WidgetId::new("s"), &mut sink);
        slider_ui.key(key_event(KeyCode::Enter), &mut sink);
        assert!(slider_ui.adjusting());
        assert!(slider_ui.key(key_event(KeyCode::Backspace), &mut sink));
        assert!(!slider_ui.adjusting());
        // Adjust-mode cancel keeps focus; a second Backspace releases it.
        assert_eq!(slider_ui.focus_id(), Some(WidgetId::new("s")));
        assert!(slider_ui.key(key_event(KeyCode::Backspace), &mut sink));
        assert_eq!(slider_ui.focus_id(), None);
    }

    #[test]
    fn a11y_list_rows_keep_absolute_indices_after_scrolling() {
        let mut ui: Ui<usize> = Ui::new();
        ui.add(ListView::new("list").row_height(44.0).on_activate(|index| index));
        ui.set_rect(WidgetId::new("list"), Rect::from_xywh(0.0, 0.0, 200.0, 200.0));
        ui.set_rows(
            WidgetId::new("list"),
            (0..50).map(|index| Row::new(format!("row {index}"))).collect(),
        );
        let mut sink = Sink::default();
        // Scroll down so the viewport no longer starts at row 0.
        ui.wheel(Point::new(100.0, 100.0), -3.0, &mut sink);

        let nodes = ui.a11y_nodes();
        let list = nodes
            .iter()
            .position(|node| node.id == WidgetId::new("list") && node.child.is_none())
            .expect("list node");
        let first_row = nodes[nodes[list].children[0]].clone();
        let row = first_row.child.expect("row index");
        assert!(row > 0, "published row {row} should be scroll-adjusted");

        // Selecting the published row must select that row, not row 0.
        let action = A11yAction {
            kind: A11yActionKind::Select,
            id: WidgetId::new("list"),
            child: Some(row),
            value: None,
            number: None,
        };
        ui.apply_a11y(&action, &mut sink);
        let nodes = ui.a11y_nodes();
        let selected = nodes
            .iter()
            .find(|node| node.id == WidgetId::new("list") && node.selected == Some(true))
            .expect("selected row");
        assert_eq!(selected.child, Some(row));
    }

    #[test]
    fn focused_container_reports_focus_on_the_selected_option() {
        let mut ui: Ui<usize> = Ui::new();
        ui.add(Segmented::new("seg", &["a", "b", "c"], 1, |index| index));
        ui.set_rect(WidgetId::new("seg"), Rect::from_xywh(0.0, 0.0, 300.0, 32.0));
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("seg"), &mut sink);

        let nodes = ui.a11y_nodes();
        let parent = nodes
            .iter()
            .find(|node| node.id == WidgetId::new("seg") && node.child.is_none())
            .expect("segmented node");
        // The selected option, not the group, carries keyboard focus so a
        // screen reader announces the option it would otherwise only count.
        assert!(!parent.focused);
        let focused: Vec<_> = nodes.iter().filter(|node| node.focused).collect();
        assert_eq!(focused.len(), 1);
        assert_eq!(focused[0].id, WidgetId::new("seg"));
        assert_eq!(focused[0].child, Some(1));
    }

    #[test]
    fn hidden_and_display_only_widgets_do_not_steal_input() {
        let area = Rect::from_xywh(0.0, 0.0, 100.0, 40.0);
        let mut sink = Sink::default();
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("bottom", "B", "bottom"));
        ui.add(Toast::new("toast"));
        ui.set_rect(WidgetId::new("bottom"), area);
        ui.set_rect(WidgetId::new("toast"), area);
        // A display-only widget is transparent to the pointer even when shown.
        ui.set_message(WidgetId::new("toast"), "toast", false);
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["bottom"]);

        // A hidden interactive widget over another one passes clicks through.
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("bottom", "B", "bottom"));
        ui.add(Button::new("top", "T", "top"));
        ui.set_rect(WidgetId::new("bottom"), area);
        ui.set_rect(WidgetId::new("top"), area);
        ui.set_visible(WidgetId::new("top"), false, &mut sink);
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["bottom"]);
    }

    #[test]
    fn hiding_a_pressed_widget_releases_press_and_hover() {
        let area = Rect::from_xywh(0.0, 0.0, 100.0, 40.0);
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("bottom", "B", "bottom"));
        ui.add(Button::new("top", "T", "top"));
        ui.set_rect(WidgetId::new("bottom"), area);
        ui.set_rect(WidgetId::new("top"), area);
        let mut sink = Sink::default();

        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.hover_id(), Some(WidgetId::new("top")));
        ui.set_visible(WidgetId::new("top"), false, &mut sink);
        assert_eq!(ui.hover_id(), None);
        assert_eq!(sink.rects.last(), Some(&area));
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        // The press was released by hiding: no click is delivered to either.
        assert!(ui.drain_messages().is_empty());
    }

    #[test]
    fn overlapping_widgets_pick_the_topmost_visible_one() {
        let area = Rect::from_xywh(0.0, 0.0, 100.0, 40.0);
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("bottom", "B", "bottom"));
        ui.add(Button::new("middle", "M", "middle"));
        ui.add(Button::new("top", "T", "top"));
        for id in ["bottom", "middle", "top"] {
            ui.set_rect(WidgetId::new(id), area);
        }
        let mut sink = Sink::default();
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["top"]);

        // Hiding the top of the stack exposes the widget under it.
        ui.set_visible(WidgetId::new("top"), false, &mut sink);
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["middle"]);
    }

    #[test]
    fn focus_order_uses_weight_then_insertion_order() {
        let mut ui: Ui<&'static str> = Ui::new();
        for (position, id) in ["a", "b", "c"].iter().enumerate() {
            ui.add(Button::new(id, *id, *id));
            ui.set_rect(
                WidgetId::new(id),
                Rect::from_xywh(0.0, position as f32 * 50.0, 100.0, 40.0),
            );
        }
        // Default weight 0 keeps insertion order.
        let mut sink = Sink::default();
        let tab = key_event(KeyCode::Tab);
        ui.key(tab, &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("a")));
        ui.key(tab, &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("b")));

        // Ascending weight: b(-1), c(0), a(10).
        let mut ui2: Ui<&'static str> = Ui::new();
        for (position, id) in ["a", "b", "c"].iter().enumerate() {
            ui2.add(Button::new(id, *id, *id));
            ui2.set_rect(
                WidgetId::new(id),
                Rect::from_xywh(0.0, position as f32 * 50.0, 100.0, 40.0),
            );
        }
        ui2.set_focus_order(WidgetId::new("a"), 10);
        ui2.set_focus_order(WidgetId::new("b"), -1);
        ui2.key(tab, &mut sink);
        assert_eq!(ui2.focus_id(), Some(WidgetId::new("b")));
        ui2.key(tab, &mut sink);
        assert_eq!(ui2.focus_id(), Some(WidgetId::new("c")));
        ui2.key(tab, &mut sink);
        assert_eq!(ui2.focus_id(), Some(WidgetId::new("a")));
    }

    #[test]
    fn focusable_override_removes_widget_and_relocates_focus() {
        let mut ui: Ui<&'static str> = Ui::new();
        for (position, id) in ["a", "b", "c"].iter().enumerate() {
            ui.add(Button::new(id, *id, *id));
            ui.set_rect(
                WidgetId::new(id),
                Rect::from_xywh(0.0, position as f32 * 50.0, 100.0, 40.0),
            );
        }
        let mut sink = Sink::default();
        ui.set_focusable(WidgetId::new("b"), false, &mut sink);
        ui.set_focus(WidgetId::new("a"), &mut sink);
        ui.key(key_event(KeyCode::Tab), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("c")));

        // Turning off the focused widget moves focus to the next one.
        ui.set_focus(WidgetId::new("c"), &mut sink);
        ui.set_focusable(WidgetId::new("c"), false, &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("a")));
    }

    #[test]
    fn hiding_or_disabling_the_focused_widget_moves_focus() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("a", "A", "a"));
        ui.add(Button::new("b", "B", "b"));
        ui.set_rect(WidgetId::new("a"), Rect::from_xywh(0.0, 0.0, 100.0, 40.0));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 50.0, 100.0, 40.0));
        let mut sink = Sink::default();

        ui.set_focus(WidgetId::new("a"), &mut sink);
        ui.set_visible(WidgetId::new("a"), false, &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("b")));
        // Hidden widgets are out of the a11y tree and cannot be focused.
        assert!(!ui.a11y_nodes().iter().any(|node| node.id == WidgetId::new("a")));
        assert!(!ui.set_focus(WidgetId::new("a"), &mut sink));

        ui.set_focus(WidgetId::new("b"), &mut sink);
        ui.set_enabled(WidgetId::new("b"), false, &mut sink);
        assert_eq!(ui.focus_id(), None);
    }

    #[test]
    fn hover_cursor_comes_from_the_widget_and_override() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("b", "B", 1u32));
        ui.add(TextEdit::new("t", "x", |_| 2u32));
        ui.add(SectionTitle::new("title", "Title"));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 0.0, 100.0, 40.0));
        ui.set_rect(WidgetId::new("t"), Rect::from_xywh(0.0, 50.0, 100.0, 40.0));
        ui.set_rect(WidgetId::new("title"), Rect::from_xywh(0.0, 100.0, 100.0, 24.0));
        let mut sink = Sink::default();

        ui.pointer_move(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.hover_cursor(), Cursor::Hand);
        ui.pointer_move(Point::new(20.0, 60.0), &mut sink);
        assert_eq!(ui.hover_cursor(), Cursor::IBeam);
        // Display-only widgets are transparent to the pointer.
        ui.pointer_move(Point::new(20.0, 110.0), &mut sink);
        assert_eq!(ui.hover_id(), None);
        assert_eq!(ui.hover_cursor(), Cursor::Arrow);

        // Composite fields report the cursor per position: an arrow over a
        // number stepper, an I-beam over the editable area.
        ui.add(NumberInput::new("n", 5, 1..=9, |_| 3u32).steppers());
        ui.set_rect(WidgetId::new("n"), Rect::from_xywh(0.0, 150.0, 120.0, 34.0));
        ui.pointer_move(Point::new(40.0, 160.0), &mut sink);
        assert_eq!(ui.hover_cursor(), Cursor::IBeam);
        ui.pointer_move(Point::new(116.0, 160.0), &mut sink);
        assert_eq!(ui.hover_cursor(), Cursor::Arrow);

        ui.set_cursor(WidgetId::new("b"), Some(Cursor::Arrow));
        ui.pointer_move(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.hover_cursor(), Cursor::Arrow);
        ui.set_cursor(WidgetId::new("b"), None);
        assert_eq!(ui.hover_cursor(), Cursor::Hand);
    }

    #[test]
    fn click_binding_can_replace_or_wrap_the_default() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("b", "B", "default"));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 0.0, 100.0, 40.0));
        let mut sink = Sink::default();

        // Suppress the default: only the custom message is emitted.
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let counter = calls.clone();
        ui.on_click(WidgetId::new("b"), move |_event, _ctx| {
            counter.set(counter.get() + 1);
            Some("custom")
        });
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["custom"]);
        assert_eq!(calls.get(), 1);

        // Calling ctx.default() emits the custom message and keeps the widget
        // behavior (custom first).
        ui.on_click(WidgetId::new("b"), |_event, ctx| {
            ctx.default();
            Some("custom")
        });
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["custom", "default"]);

        // Calling ctx.run_default() runs the default *now* and hands its
        // message to the handler, which can wrap or post-process it. The
        // default does not run again (even if the flag form is also used).
        ui.on_click(WidgetId::new("b"), |_event, ctx| {
            let default = ctx.run_default();
            assert_eq!(default, Some("default"));
            // A second call in the same handler yields nothing.
            assert_eq!(ctx.run_default(), None);
            ctx.default();
            default.map(|_| "wrapped")
        });
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["wrapped"]);

        // A handler that runs the default but returns nothing still runs the
        // built-in behavior (it just discards the message).
        ui.on_click(WidgetId::new("b"), |_event, ctx| {
            let _ = ctx.run_default();
            None
        });
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert!(ui.drain_messages().is_empty());
    }

    #[test]
    fn wheel_binding_can_suppress_or_keep_scrolling() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(ListView::new("list").row_height(40.0).on_activate(|_| "row"));
        ui.set_rect(WidgetId::new("list"), Rect::from_xywh(0.0, 0.0, 200.0, 200.0));
        ui.set_rows(
            WidgetId::new("list"),
            (0..50).map(|index| Row::new(format!("row {index}"))).collect(),
        );
        let mut sink = Sink::default();
        sink.rects.clear();

        // Without default, nothing scrolls and the list is not repainted.
        ui.bind(WidgetId::new("list"), Trigger::Wheel, |event, _ctx| {
            assert!(event.wheel_delta < 0.0);
            Some("wheel")
        });
        assert!(ui.wheel(Point::new(50.0, 50.0), -1.0, &mut sink));
        assert_eq!(ui.drain_messages(), vec!["wheel"]);
        assert!(sink.rects.is_empty());

        // With default the list scrolls and repaints.
        ui.bind(WidgetId::new("list"), Trigger::Wheel, |_event, ctx| {
            ctx.default();
            Some("wheel")
        });
        sink.rects.clear();
        ui.wheel(Point::new(50.0, 50.0), -1.0, &mut sink);
        assert_eq!(ui.drain_messages(), vec!["wheel"]);
        assert_eq!(sink.rects, vec![Rect::from_xywh(0.0, 0.0, 200.0, 200.0)]);
    }

    #[test]
    fn activate_binding_fires_for_keyboard_and_uia() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("b", "B", "default"));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 0.0, 100.0, 40.0));
        let mut sink = Sink::default();
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let counter = calls.clone();
        ui.on_activate(WidgetId::new("b"), move |_event, ctx| {
            counter.set(counter.get() + 1);
            ctx.default();
            None
        });

        ui.set_focus(WidgetId::new("b"), &mut sink);
        ui.key(key_event(KeyCode::Enter), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["default"]);

        let action = A11yAction {
            kind: A11yActionKind::Invoke,
            id: WidgetId::new("b"),
            child: None,
            value: None,
            number: None,
        };
        assert_eq!(ui.apply_a11y(&action, &mut sink), Some("default"));
        assert_eq!(calls.get(), 2);
        assert!(ui.drain_messages().is_empty());
    }

    #[test]
    fn focus_hooks_run_lost_before_gained_with_the_reason() {
        type Log = std::rc::Rc<std::cell::RefCell<Vec<(WidgetId, FocusReason, bool)>>>;
        let log: Log = Default::default();
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("a", "A", "a"));
        ui.add(Button::new("b", "B", "b"));
        ui.set_rect(WidgetId::new("a"), Rect::from_xywh(0.0, 0.0, 100.0, 40.0));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 50.0, 100.0, 40.0));

        let lost = log.clone();
        ui.on_focus_lost(WidgetId::new("a"), move |event| {
            lost.borrow_mut().push((event.id, event.reason, false));
            FocusOutcome::Allow
        });
        let gained = log.clone();
        ui.on_focus_gained(WidgetId::new("b"), move |event| {
            gained.borrow_mut().push((event.id, event.reason, true));
            FocusOutcome::Allow
        });

        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("a"), &mut sink);
        ui.set_focus(WidgetId::new("b"), &mut sink);
        assert_eq!(
            *log.borrow(),
            vec![
                (WidgetId::new("a"), FocusReason::Programmatic, false),
                (WidgetId::new("b"), FocusReason::Programmatic, true),
            ]
        );
    }

    #[test]
    fn focus_lost_hook_can_cancel_or_redirect() {
        let mut ui: Ui<&'static str> = Ui::new();
        for (position, id) in ["a", "b", "c"].iter().enumerate() {
            ui.add(Button::new(id, *id, *id));
            ui.set_rect(
                WidgetId::new(id),
                Rect::from_xywh(0.0, position as f32 * 50.0, 100.0, 40.0),
            );
        }
        let mut sink = Sink::default();

        ui.on_focus_lost(WidgetId::new("a"), |_event| FocusOutcome::Cancel);
        ui.set_focus(WidgetId::new("a"), &mut sink);
        ui.set_focus(WidgetId::new("b"), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("a")));

        // Redirect sends focus to another widget instead of the requested one.
        ui.on_focus_lost(WidgetId::new("a"), |_event| {
            FocusOutcome::Redirect(WidgetId::new("c"))
        });
        ui.set_focus(WidgetId::new("b"), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("c")));
    }

    #[test]
    fn hiding_the_focus_owner_reports_relocation_and_commits_first() {
        struct HookSink {
            editing: Option<WidgetId>,
            ended_before_hook: std::rc::Rc<std::cell::Cell<bool>>,
        }
        impl Invalidator for HookSink {
            fn invalidate(&mut self, _rect: Rect) {}
        }
        impl UiHost for HookSink {
            fn begin_edit(&mut self, request: &EditRequest<'_>) {
                self.editing = Some(request.id);
            }
            fn end_edit(&mut self) {
                self.ended_before_hook
                    .set(self.editing.is_some());
                self.editing = None;
            }
            fn editing_id(&self) -> Option<WidgetId> {
                self.editing
            }
        }

        let committed = std::rc::Rc::new(std::cell::Cell::new(false));
        let reason = std::rc::Rc::new(std::cell::Cell::new(None));
        let mut ui: Ui<u32> = Ui::new();
        ui.add(TextEdit::new("field", "x", |_| 1u32));
        ui.add(Button::new("b", "B", 2u32));
        ui.set_rect(WidgetId::new("field"), Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 50.0, 100.0, 40.0));
        let mut sink = HookSink {
            editing: None,
            ended_before_hook: committed.clone(),
        };

        ui.set_focus(WidgetId::new("field"), &mut sink);
        ui.key(key_event(KeyCode::Enter), &mut sink);
        assert!(sink.editing.is_some(), "native editor should be open");

        let observed = committed.clone();
        let observed_reason = reason.clone();
        ui.on_focus_lost(WidgetId::new("field"), move |event| {
            observed.set(observed.get() || event.reason == FocusReason::Relocated);
            observed_reason.set(Some(event.reason));
            FocusOutcome::Allow
        });
        ui.set_visible(WidgetId::new("field"), false, &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("b")));
        assert_eq!(reason.get(), Some(FocusReason::Relocated));
        assert!(committed.get(), "the editor must commit before the lost hook");
    }

    #[test]
    fn vetoing_relocation_still_removes_focus_from_the_hidden_widget() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("a", "A", "a"));
        ui.add(Button::new("b", "B", "b"));
        ui.set_rect(WidgetId::new("a"), Rect::from_xywh(0.0, 0.0, 100.0, 40.0));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 50.0, 100.0, 40.0));
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("a"), &mut sink);
        ui.on_focus_lost(WidgetId::new("a"), |_event| FocusOutcome::Cancel);
        ui.set_visible(WidgetId::new("a"), false, &mut sink);
        // The hook vetoed the move; a hidden widget still must not keep focus.
        assert_eq!(ui.focus_id(), None);
    }

    #[test]
    fn display_widgets_stay_out_of_the_tree_until_opted_in() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(SectionTitle::new("title", "播放队列"));
        ui.add(Card::new("card", "详情", "曲目信息"));
        ui.add(ProgressBar::new("progress", 0.25).label("缓存"));
        ui.set_rect(
            WidgetId::new("title"),
            Rect::from_xywh(0.0, 0.0, 200.0, 24.0),
        );
        ui.set_rect(
            WidgetId::new("card"),
            Rect::from_xywh(0.0, 30.0, 240.0, 96.0),
        );
        ui.set_rect(
            WidgetId::new("progress"),
            Rect::from_xywh(0.0, 140.0, 200.0, 24.0),
        );
        assert!(ui.a11y_nodes().is_empty());

        // Focusability puts the heading in Tab and in the tree with a role a
        // screen reader understands (not a button).
        let mut sink = Sink::default();
        ui.set_focusable(WidgetId::new("title"), true, &mut sink);
        let nodes = ui.a11y_nodes();
        let title = nodes
            .iter()
            .find(|node| node.id == WidgetId::new("title"))
            .expect("focusable heading is exposed");
        assert_eq!(title.role, A11yRole::Heading);
        assert_eq!(title.name, "播放队列");
        // The card and progress bar are still decoration.
        assert_eq!(nodes.len(), 1);

        // An explicit opt-in exposes the progress bar as a range value.
        ui.set_props(
            WidgetId::new("progress"),
            WidgetProps::new().expose(true),
            &mut sink,
        );
        let nodes = ui.a11y_nodes();
        let progress = nodes
            .iter()
            .find(|node| node.id == WidgetId::new("progress"))
            .expect("exposed progress bar");
        assert_eq!(progress.role, A11yRole::ProgressBar);
        assert_eq!(progress.range, Some((0.0, 1.0, 0.25)));
    }

    #[test]
    fn narration_and_automation_id_feed_the_tree() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add_with(
            SectionTitle::new("title", "队列"),
            WidgetProps::new()
                .narrate(Narration::new().name("播放队列").description("600 首"))
                .automation_id("queue-heading"),
        );
        ui.set_rect(
            WidgetId::new("title"),
            Rect::from_xywh(0.0, 0.0, 200.0, 24.0),
        );
        let nodes = ui.a11y_nodes();
        let title = nodes
            .iter()
            .find(|node| node.id == WidgetId::new("title"))
            .expect("narrated display widget is exposed");
        assert_eq!(title.name, "播放队列");
        assert_eq!(title.description.as_deref(), Some("600 首"));
        assert_eq!(title.automation_id.as_deref(), Some("queue-heading"));
    }

    #[test]
    fn narrated_rows_keep_absolute_indices() {
        let mut ui: Ui<usize> = Ui::new();
        ui.add_with(
            ListView::new("list").row_height(40.0).on_activate(|index| index),
            WidgetProps::new().narrate(
                Narration::new().row(|index, child| format!("{} {}", index + 1, child.name)),
            ),
        );
        ui.set_rect(WidgetId::new("list"), Rect::from_xywh(0.0, 0.0, 200.0, 200.0));
        ui.set_rows(
            WidgetId::new("list"),
            (0..50).map(|index| Row::new(format!("曲目 {index}"))).collect(),
        );
        let mut sink = Sink::default();
        ui.wheel(Point::new(100.0, 100.0), -3.0, &mut sink);

        let nodes = ui.a11y_nodes();
        let list = nodes
            .iter()
            .find(|node| node.id == WidgetId::new("list") && node.child.is_none())
            .expect("list node");
        let first = &nodes[list.children[0]];
        let index = first.child.expect("row index");
        assert!(index > 0, "row {index} should be scroll-adjusted");
        assert_eq!(first.name, format!("{} 曲目 {index}", index + 1));
    }

    #[test]
    fn a_mouse_enabled_display_widget_takes_clicks() {
        let area = Rect::from_xywh(0.0, 0.0, 120.0, 40.0);
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("bottom", "B", "bottom"));
        ui.add(Card::new("card", "详情", "body"));
        ui.set_rect(WidgetId::new("bottom"), area);
        ui.set_rect(WidgetId::new("card"), area);
        let mut sink = Sink::default();

        // Default cards are transparent to the pointer: the button is hit.
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["bottom"]);

        // With mouse(true) the card is the topmost interactive widget, its
        // hover cursor changes and a trigger binding fires.
        ui.add_with(
            Card::new("card2", "详情", "body"),
            WidgetProps::new().mouse(true).cursor(Cursor::Hand),
        );
        ui.set_rect(WidgetId::new("card2"), area);
        ui.on_click(WidgetId::new("card2"), |_event, _ctx| Some("card"));
        ui.pointer_move(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.hover_cursor(), Cursor::Hand);
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["card"]);
        // A clickable display widget is in the tree; a plain card is not.
        let nodes = ui.a11y_nodes();
        assert!(nodes.iter().any(|node| node.id == WidgetId::new("card2")));
        assert!(!nodes.iter().any(|node| node.id == WidgetId::new("card")));
    }

    #[test]
    fn read_only_fields_do_not_open_an_editor() {
        let (mut ui, _, _) = text_and_button();
        let mut sink = Sink::default();
        ui.set_props(
            WidgetId::new("name"),
            WidgetProps::new().read_only(true),
            &mut sink,
        );
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        assert!(sink.began.is_empty(), "read-only field must not open an editor");
        let node = ui
            .a11y_nodes()
            .into_iter()
            .find(|node| node.id == WidgetId::new("name"))
            .expect("field node");
        assert!(node.read_only);
    }

    #[test]
    fn read_only_painted_fields_keep_a_selection_session() {
        let id = WidgetId::new("ro");
        let mut ui: Ui<String> = Ui::new();
        ui.add_with(
            TextEdit::new("ro", "Bamboo", |text| text).painted(),
            WidgetProps::new().read_only(true),
        );
        ui.set_rect(id, Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        let mut sink = Sink::default();

        // Enter opens a session even though the value is locked: the caret
        // and selection work, typing does not.
        ui.set_focus(id, &mut sink);
        assert!(ui.key(key_event(KeyCode::Enter), &mut sink));
        assert!(
            ui.with_widget::<TextEdit<String>, _>(id, |field| field.is_editing())
                .unwrap(),
            "a locked painted field must still open its selection session"
        );

        let mut shift_left = key_event(KeyCode::Left);
        shift_left.shift = true;
        assert!(ui.key(shift_left, &mut sink));
        assert_eq!(
            ui.with_widget::<TextEdit<String>, _>(id, |field| field.selection()),
            Some(Some((5, 6)))
        );

        // Typing and deleting are refused; the copy chord still reaches the
        // clipboard.
        assert!(!ui.char('x', &mut sink));
        assert!(ui.key(key_event(KeyCode::Backspace), &mut sink));
        assert_eq!(
            ui.with_widget::<TextEdit<String>, _>(id, |field| field.text().to_string()),
            Some("Bamboo".to_string())
        );
        assert!(ui.key(key_event_mod(KeyCode::Letter(2), true), &mut sink));
        assert_eq!(sink.copied.last().map(String::as_str), Some("o"));

        let node = ui
            .a11y_nodes()
            .into_iter()
            .find(|node| node.id == id)
            .expect("field node");
        assert!(node.read_only);
    }

    #[test]
    fn key_up_reaches_the_focused_widget() {
        let id = WidgetId::new("field");
        let mut ui: Ui<String> = Ui::new();
        ui.add(
            TextEdit::new("field", "", |text| text)
                .painted()
                .on_key_up(|key| (key.code == KeyCode::Left).then(|| "up".to_string())),
        );
        ui.set_rect(id, Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        let mut sink = Sink::default();
        assert!(!ui.key_up(key_event(KeyCode::Left), &mut sink), "nothing focused");
        ui.set_focus(id, &mut sink);
        assert!(ui.key_up(key_event(KeyCode::Left), &mut sink));
        assert_eq!(ui.drain_messages(), vec!["up".to_string()]);
        assert!(ui.key_up(key_event(KeyCode::Right), &mut sink));
        assert!(ui.drain_messages().is_empty(), "the observer stays silent");
    }

    #[test]
    fn typed_characters_and_pastes_reach_the_edit_box_observers() {
        let id = WidgetId::new("field");
        let mut ui: Ui<String> = Ui::new();
        ui.add(
            TextEdit::new("field", "", |text| text)
                .painted()
                .on_char(|ch| (ch == '!').then(|| "bang".to_string()))
                .on_change(|text| Some(format!("change:{text}"))),
        );
        ui.set_rect(id, Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        let mut sink = Sink::default();
        ui.set_focus(id, &mut sink);

        // A plain character reports the change (the `on_char` hook stays
        // silent); an interesting one wins over it.
        assert!(ui.char('a', &mut sink));
        assert_eq!(ui.drain_messages(), vec!["change:a".to_string()]);
        assert!(ui.char('!', &mut sink));
        assert_eq!(ui.drain_messages(), vec!["bang".to_string()]);
        assert_eq!(
            ui.with_widget::<TextEdit<String>, _>(id, |field| field.text().to_string()),
            Some("a!".to_string())
        );

        // Paste is a direct insertion too: its change is reported.
        sink.clipboard = Some("P".to_string());
        assert!(ui.key(key_event_mod(KeyCode::Letter(21), true), &mut sink));
        assert_eq!(ui.drain_messages(), vec!["change:a!P".to_string()]);

        // A cut follows the same path.
        assert!(ui.key(key_event_mod(KeyCode::Letter(0), true), &mut sink));
        assert!(ui.key(key_event_mod(KeyCode::Letter(23), true), &mut sink));
        assert_eq!(ui.drain_messages(), vec!["change:".to_string()]);
    }

    #[test]
    fn read_only_props_are_pushed_into_a_composite_widget() {
        // A composite that edits its own contents (the property grid) locks
        // through the same property: the container pushes the flag and ORs it
        // into the effective state.
        let mut ui: Ui<()> = Ui::new();
        let mut sink = Sink::default();
        ui.add_with(
            crate::widgets::PropertyGrid::<()>::new("grid"),
            WidgetProps::new().read_only(true),
        );
        ui.set_rect(
            WidgetId::new("grid"),
            Rect::from_xywh(0.0, 0.0, 200.0, 200.0),
        );
        assert!(ui.effective_read_only(0));
        assert!(ui.widgets[0].read_only());
        ui.set_props(
            WidgetId::new("grid"),
            WidgetProps::new().read_only(false),
            &mut sink,
        );
        assert!(!ui.effective_read_only(0));
        assert!(!ui.widgets[0].read_only());
    }

    #[test]
    fn setting_props_can_relocate_focus_and_repaints() {
        let (mut ui, a, b) = two_buttons();
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("a"), &mut sink);
        sink.rects.clear();
        ui.set_props(
            WidgetId::new("a"),
            WidgetProps::new().focusable(false),
            &mut sink,
        );
        assert_eq!(ui.focus_id(), Some(WidgetId::new("b")));
        // Focus leaves a and lands on b; both rings repaint.
        let rects = sink.take();
        assert!(rects.contains(&a));
        assert!(rects.contains(&b));
    }

    #[test]
    fn typed_access_scrolls_a_list_and_invalidates_once() {
        let area = Rect::from_xywh(0.0, 0.0, 200.0, 200.0);
        let list = WidgetId::new("list");
        let mut ui: Ui<usize> = Ui::new();
        ui.add(ListView::new("list").row_height(40.0).on_activate(|index| index));
        ui.set_rect(list, area);
        ui.set_rows(list, (0..50).map(|index| Row::new(format!("r{index}"))).collect());
        let mut sink = Sink::default();

        assert_eq!(
            ui.with_widget::<ListView<usize>, _>(list, |view| view.visible_rows()),
            Some((0, 6))
        );
        assert!(ui.edit_widget::<ListView<usize>>(list, |view| {
            view.scroll_to_row(25)
        }));
        assert!(ui.flush_damage(&mut sink));
        assert_eq!(sink.take(), vec![area]);
        assert_eq!(
            ui.with_widget::<ListView<usize>, _>(list, |view| view.visible_rows().0),
            Some(25)
        );
        // A no-op scroll reports false and repaints nothing.
        assert!(!ui.edit_widget::<ListView<usize>>(list, |view| {
            view.scroll_to_row(25)
        }));
        assert!(sink.take().is_empty());
        assert!(ui.edit_widget::<ListView<usize>>(list, |view| {
            view.ensure_row_visible(0)
        }));
        assert_eq!(
            ui.with_widget::<ListView<usize>, _>(list, |view| view.scroll_offset()),
            Some(0.0)
        );
    }

    #[test]
    fn typed_access_rejects_the_wrong_type() {
        let (mut ui, _, _) = two_buttons();
        let mut sink = Sink::default();
        assert!(
            ui.with_widget::<ListView<u32>, _>(WidgetId::new("a"), |view| view.visible_rows())
                .is_none()
        );
        assert!(!ui.edit_widget::<ListView<u32>>(
            WidgetId::new("a"),
            |_| true,
        ));
        assert!(sink.take().is_empty());
    }

    #[test]
    fn typed_range_and_page_changes_clamp_and_report() {
        let area = Rect::from_xywh(0.0, 0.0, 200.0, 32.0);
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Slider::new("s", 0.5, 0.0..=1.0, |value| value.to_bits()));
        ui.add(Pager::new("p", 4, 5, |delta| delta as u32));
        ui.set_rect(WidgetId::new("s"), area);
        ui.set_rect(WidgetId::new("p"), area);
        let mut sink = Sink::default();

        // Raising the minimum clamps the current value into the new range.
        assert!(ui.edit_widget::<Slider<u32>>(WidgetId::new("s"), |slider| {
            slider.set_range(2.0, 4.0)
        }));
        assert_eq!(
            ui.with_widget::<Slider<u32>, _>(WidgetId::new("s"), |slider| {
                (slider.value(), slider.range())
            }),
            Some((2.0, (2.0, 4.0)))
        );
        assert!(ui.flush_damage(&mut sink));
        assert_eq!(sink.take(), vec![area]);

        // Fewer pages clamps the current page.
        assert!(ui.edit_widget::<Pager<u32>>(WidgetId::new("p"), |pager| {
            pager.set_pages(2)
        }));
        assert_eq!(
            ui.with_widget::<Pager<u32>, _>(WidgetId::new("p"), |pager| {
                (pager.page(), pager.pages())
            }),
            Some((1, 2))
        );
        assert!(ui.flush_damage(&mut sink));
        assert_eq!(sink.take(), vec![area]);
    }

    #[test]
    fn typed_text_and_number_edits_stay_consistent() {
        let (mut ui, _, _) = text_and_button();
        let name = WidgetId::new("name");
        assert!(ui.edit_widget::<TextEdit<String>>(name, |edit| edit.clear()));
        assert_eq!(
            ui.with_widget::<TextEdit<String>, _>(name, |edit| edit.text().to_string()),
            Some(String::new())
        );
        assert!(ui.edit_widget::<TextEdit<String>>(name, |edit| {
            edit.insert_text("x")
        }));
        assert_eq!(
            ui.with_widget::<TextEdit<String>, _>(name, |edit| edit.text().to_string()),
            Some("x".to_string())
        );

        let mut numbers: Ui<i64> = Ui::new();
        numbers.add(NumberInput::new("count", 5, 1..=10, |value| value));
        let count = WidgetId::new("count");
        numbers.set_rect(count, Rect::from_xywh(0.0, 0.0, 120.0, 34.0));
        // Clamped at the maximum and reported as unchanged the second time.
        assert!(numbers.edit_widget::<NumberInput<i64>>(count, |input| {
            input.step(100)
        }));
        assert_eq!(
            numbers.with_widget::<NumberInput<i64>, _>(count, |input| input.value()),
            Some(10)
        );
        assert!(!numbers.edit_widget::<NumberInput<i64>>(count, |input| {
            input.step(1)
        }));
    }

    #[test]
    fn double_and_right_click_bindings_fire_without_a_click() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("b", "B", "click"));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 0.0, 200.0, 40.0));
        ui.bind(WidgetId::new("b"), Trigger::DoubleClick, |event, _ctx| {
            assert!(event.pos.is_some());
            Some("double")
        });
        ui.bind(WidgetId::new("b"), Trigger::RightClick, |_event, _ctx| {
            Some("right")
        });
        let mut sink = Sink::default();

        assert!(ui.pointer_double_click(Point::new(20.0, 10.0), &mut sink));
        assert_eq!(ui.drain_messages(), vec!["double"]);
        // A double click does not change focus by itself.
        assert_eq!(ui.focus_id(), None);

        // The secondary button raises its trigger on release and neither
        // focuses nor clicks.
        assert!(ui.pointer_down_button(Point::new(20.0, 10.0), MouseButton::Right, &mut sink));
        assert!(ui.pointer_up_button(Point::new(20.0, 10.0), MouseButton::Right, &mut sink));
        assert_eq!(ui.drain_messages(), vec!["right"]);
        assert_eq!(ui.focus_id(), None);
        // Releasing without a matching right press does nothing.
        assert!(!ui.pointer_up_button(Point::new(20.0, 10.0), MouseButton::Right, &mut sink));
    }

    #[test]
    fn widget_observers_fire_beside_the_built_in_action() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(
            Button::new("b", "B", "default")
                .on_click(|_| Some("click"))
                .on_right_click(|_| Some("right"))
                .on_double_click(|_| Some("double"))
                .on_focus_gained(|_| Some("gained"))
                .on_focus_lost(|_| Some("lost"))
                .on_key_down(|key| (key.code == KeyCode::Letter(0)).then_some("down"))
                .on_key_up(|key| (key.code == KeyCode::Letter(0)).then_some("up")),
        );
        // A second widget is a focus destination for the lost observer.
        ui.add(Button::new("c", "C", "other"));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 0.0, 200.0, 40.0));
        ui.set_rect(WidgetId::new("c"), Rect::from_xywh(0.0, 50.0, 200.0, 40.0));
        let mut sink = Sink::default();

        // The press focuses the button (gained), then the release runs the
        // built-in message; the click observer is queued after it.
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["gained"]);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["default", "click"]);

        ui.pointer_double_click(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["double"]);
        ui.pointer_down_button(Point::new(20.0, 10.0), MouseButton::Right, &mut sink);
        ui.pointer_up_button(Point::new(20.0, 10.0), MouseButton::Right, &mut sink);
        assert_eq!(ui.drain_messages(), vec!["right"]);

        // Keys: the observer sees the key the container leaves to the
        // application, and releases reach it too (focus is still on "b").
        let letter = key_event(KeyCode::Letter(0));
        assert!(!ui.key(letter, &mut sink), "the container leaves A unhandled");
        assert_eq!(ui.drain_messages(), vec!["down"]);
        assert!(ui.key_up(letter, &mut sink));
        assert_eq!(ui.drain_messages(), vec!["up"]);

        // Moving focus away reports the lost observer.
        ui.set_focus(WidgetId::new("c"), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["lost"]);
    }

    #[test]
    fn drag_fires_once_and_suppresses_the_click() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("b", "B", "click"));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 0.0, 200.0, 40.0));
        ui.bind(WidgetId::new("b"), Trigger::DragStart, |_event, _ctx| {
            Some("start")
        });
        ui.bind(WidgetId::new("b"), Trigger::DragEnd, |_event, _ctx| Some("end"));
        let mut sink = Sink::default();

        ui.pointer_down(Point::new(10.0, 10.0), &mut sink);
        // Below the threshold nothing fires...
        ui.pointer_move(Point::new(12.0, 10.0), &mut sink);
        assert!(ui.drain_messages().is_empty());
        // ...past it, DragStart fires exactly once.
        ui.pointer_move(Point::new(60.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["start"]);
        ui.pointer_move(Point::new(90.0, 10.0), &mut sink);
        assert!(ui.drain_messages().is_empty());
        // The release ends the drag and must not become a click.
        ui.pointer_up(Point::new(90.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["end"]);
    }

    #[test]
    fn a_short_press_still_clicks() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("b", "B", "click"));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 0.0, 200.0, 40.0));
        ui.bind(WidgetId::new("b"), Trigger::DragStart, |_event, _ctx| {
            Some("start")
        });
        let mut sink = Sink::default();
        ui.pointer_down(Point::new(10.0, 10.0), &mut sink);
        ui.pointer_move(Point::new(12.0, 11.0), &mut sink);
        ui.pointer_up(Point::new(12.0, 11.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["click"]);
    }

    #[test]
    fn value_change_binding_runs_after_the_default_message() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Toggle::new("t", "T", false, |on| if on { "on" } else { "off" }));
        ui.set_rect(WidgetId::new("t"), Rect::from_xywh(0.0, 0.0, 200.0, 40.0));
        ui.bind(WidgetId::new("t"), Trigger::ValueChange, |_event, _ctx| {
            Some("changed")
        });
        let mut sink = Sink::default();

        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        // Default message first, then the observed change.
        assert_eq!(ui.drain_messages(), vec!["on", "changed"]);
        // Flipping back fires again with the other default message.
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["off", "changed"]);
    }

    #[test]
    fn value_change_needs_an_actual_change() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Segmented::new("seg", &["a", "b"], 0, |index| {
            if index == 0 { "a" } else { "b" }
        }));
        ui.set_rect(WidgetId::new("seg"), Rect::from_xywh(0.0, 0.0, 200.0, 32.0));
        ui.bind(WidgetId::new("seg"), Trigger::ValueChange, |_event, _ctx| {
            Some("changed")
        });
        let mut sink = Sink::default();
        // Clicking the already-selected cell emits its default message only.
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["a"]);
        // Clicking the other cell changes the selection: default + change.
        ui.pointer_down(Point::new(150.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(150.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["b", "changed"]);
    }

    #[test]
    fn clickable_display_widgets_show_press_feedback() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add_with(
            Card::new("card", "标题", "内容"),
            WidgetProps::new().mouse(true),
        );
        ui.set_rect(WidgetId::new("card"), Rect::from_xywh(0.0, 0.0, 120.0, 40.0));
        let mut sink = Sink::default();
        // Pressed: an accent outline (four 1 DIP bars).
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(paint_and_count_accent(&mut ui), 4);
        // Released and merely hovered: the outline is a blend, not the accent.
        ui.pointer_up(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(paint_and_count_accent(&mut ui), 0);
    }

    #[test]
    fn typed_access_selects_and_replaces_closed_field_text() {
        let id = WidgetId::new("field");
        let rect = Rect::from_xywh(0.0, 0.0, 200.0, 34.0);
        let mut ui: Ui<String> = Ui::new();
        ui.add(TextEdit::new("field", "Bamboo", |text| text));
        ui.set_rect(id, rect);
        let mut sink = Sink::default();

        assert!(ui.edit_widget::<TextEdit<String>>(id, |field| {
            field.select_all()
        }));
        // Typed access records the widget's rectangle; the host's flush hands
        // it to the window once.
        assert!(sink.take().is_empty(), "the change is pending, not painted");
        assert!(ui.flush_damage(&mut sink));
        assert_eq!(sink.take(), vec![rect]);
        assert_eq!(ui.a11y_nodes()[0].text_selection, Some((0, 6)));
        // Selecting the same range again is a no-op.
        assert!(!ui.edit_widget::<TextEdit<String>>(id, |field| {
            field.select_all()
        }));
        assert!(!ui.has_damage());
        assert!(sink.take().is_empty());

        assert!(ui.edit_widget::<TextEdit<String>>(id, |field| {
            field.insert_text("竹")
        }));
        assert_eq!(
            ui.with_widget::<TextEdit<String>, _>(id, |field| field.text().to_string()),
            Some("竹".to_string())
        );
        assert_eq!(ui.a11y_nodes()[0].text_selection, Some((3, 3)));
    }

    /// A widget whose hit area is a circle inside its layout rectangle.
    struct RoundButton {
        id: WidgetId,
        rect: Rect,
    }

    impl Widget<u32> for RoundButton {
        fn id(&self) -> WidgetId {
            self.id
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn rect(&self) -> Rect {
            self.rect
        }

        fn set_rect(&mut self, rect: Rect) {
            self.rect = rect;
        }

        fn hit_test(&self, pos: Point) -> bool {
            let cx = (self.rect.left + self.rect.right) / 2.0;
            let cy = (self.rect.top + self.rect.bottom) / 2.0;
            let rx = (self.rect.width() / 2.0).max(1.0);
            let ry = (self.rect.height() / 2.0).max(1.0);
            let dx = (pos.x - cx) / rx;
            let dy = (pos.y - cy) / ry;
            dx * dx + dy * dy <= 1.0
        }

        fn paint(&self, _ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {}

        fn input(&mut self, input: WidgetInput) -> Option<u32> {
            matches!(input, WidgetInput::ClickAt(_)).then_some(7)
        }
    }

    #[test]
    fn custom_hit_test_lets_empty_corners_fall_through() {
        let area = Rect::from_xywh(0.0, 0.0, 100.0, 100.0);
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("under", "Under", 1u32));
        ui.add(RoundButton {
            id: WidgetId::new("round"),
            rect: Rect::default(),
        });
        ui.set_rect(WidgetId::new("under"), area);
        ui.set_rect(WidgetId::new("round"), area);
        let mut sink = Sink::default();

        // The corner is inside the rectangle but outside the circle: the
        // button underneath receives the click.
        ui.pointer_down(Point::new(4.0, 4.0), &mut sink);
        ui.pointer_up(Point::new(4.0, 4.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec![1]);
        // The circle body takes the click.
        ui.pointer_down(Point::new(50.0, 50.0), &mut sink);
        ui.pointer_up(Point::new(50.0, 50.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec![7]);
    }

    #[test]
    fn tooltips_follow_hover_and_keyboard_focus() {
        let a = Rect::from_xywh(0.0, 0.0, 100.0, 36.0);
        let b = Rect::from_xywh(0.0, 50.0, 100.0, 86.0);
        let c = Rect::from_xywh(0.0, 100.0, 100.0, 136.0);
        let mut ui: Ui<u32> = Ui::new();
        ui.add_with(
            Button::new("a", "A", 1u32),
            WidgetProps::new().tooltip("a tip"),
        );
        ui.add_with(
            Button::new("b", "B", 2u32),
            WidgetProps::new().tooltip("b tip"),
        );
        ui.add(Button::new("c", "C", 3u32));
        for (id, rect) in [("a", a), ("b", b), ("c", c)] {
            ui.set_rect(WidgetId::new(id), rect);
        }
        let mut sink = Sink::default();
        let last = |sink: &Sink| sink.tooltips.last().cloned().flatten();

        ui.pointer_move(Point::new(10.0, 10.0), &mut sink);
        assert_eq!(
            last(&sink),
            Some((
                "a tip".to_string(),
                a,
                None,
                TooltipAnchor::Pointer(Point::new(10.0, 10.0)),
            ))
        );
        // Moving inside the same widget reports the same target; the host
        // deduplicates it and only updates the pointer position.
        ui.pointer_move(Point::new(30.0, 10.0), &mut sink);
        assert_eq!(
            last(&sink),
            Some((
                "a tip".to_string(),
                a,
                None,
                TooltipAnchor::Pointer(Point::new(30.0, 10.0)),
            ))
        );
        // A press dismisses the visible tip.
        ui.pointer_down(Point::new(30.0, 10.0), &mut sink);
        assert_eq!(last(&sink), None);
        ui.pointer_up(Point::new(30.0, 10.0), &mut sink);
        ui.pointer_leave(&mut sink);

        // Tab focuses A then B; the focused widget owns the tip, anchored to
        // the widget because there is no pointer over it.
        let tab = KeyEvent {
            code: KeyCode::Tab,
            vk: 0x09,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        };
        ui.key(tab, &mut sink);
        assert_eq!(
            last(&sink),
            Some(("b tip".to_string(), b, None, TooltipAnchor::Widget(b)))
        );
        // Hover wins even when the hovered widget has no tooltip.
        ui.pointer_move(Point::new(10.0, 110.0), &mut sink);
        assert_eq!(last(&sink), None);
        // Leaving the widget returns to the keyboard target.
        ui.pointer_leave(&mut sink);
        assert_eq!(
            last(&sink),
            Some(("b tip".to_string(), b, None, TooltipAnchor::Widget(b)))
        );
    }

    #[test]
    fn right_click_opens_a_text_field_but_does_not_focus_buttons() {
        let field = WidgetId::new("field");
        let area = Rect::from_xywh(0.0, 0.0, 200.0, 34.0);
        let mut ui: Ui<()> = Ui::new();
        ui.add(TextEdit::new("field", "abc", |_| ()));
        ui.set_rect(field, area);
        let mut sink = Sink::default();
        assert!(ui.pointer_down_button(Point::new(10.0, 10.0), MouseButton::Right, &mut sink));
        assert_eq!(ui.focus_id(), Some(field));
        assert_eq!(sink.began.len(), 1);
        assert_eq!(sink.began[0].0, field);
        ui.pointer_up_button(Point::new(10.0, 10.0), MouseButton::Right, &mut sink);

        // A plain button keeps the "right click does not focus" rule.
        let (mut ui, _, _) = two_buttons();
        let mut sink = Sink::default();
        ui.pointer_down_button(Point::new(10.0, 10.0), MouseButton::Right, &mut sink);
        assert!(ui.focus_id().is_none());
        assert!(sink.began.is_empty());
    }

    #[test]
    fn tooltip_target_changes_between_list_rows() {
        let list = WidgetId::new("list");
        let area = Rect::from_xywh(0.0, 0.0, 200.0, 200.0);
        let mut ui: Ui<usize> = Ui::new();
        ui.add_with(
            ListView::new("list").row_height(40.0),
            WidgetProps::new()
                .tooltip("list tip")
                .tooltip_at(1, "row one"),
        );
        ui.set_rect(list, area);
        ui.set_rows(
            list,
            (0..5).map(|index| Row::new(format!("r{index}"))).collect(),
        );
        let mut sink = Sink::default();
        let last = |sink: &Sink| sink.tooltips.last().cloned().flatten();

        ui.pointer_move(Point::new(10.0, 10.0), &mut sink);
        assert_eq!(
            last(&sink),
            Some((
                "list tip".to_string(),
                area,
                Some(0),
                TooltipAnchor::Pointer(Point::new(10.0, 10.0)),
            ))
        );
        // Another row is another target with its own text; the host hides and
        // re-dwells.
        ui.pointer_move(Point::new(10.0, 60.0), &mut sink);
        assert_eq!(
            last(&sink),
            Some((
                "row one".to_string(),
                area,
                Some(1),
                TooltipAnchor::Pointer(Point::new(10.0, 60.0)),
            ))
        );
        // Moving inside the same row keeps the item and text.
        ui.pointer_move(Point::new(30.0, 62.0), &mut sink);
        let target = last(&sink).expect("still over the list");
        assert_eq!(target.0, "row one");
        assert_eq!(target.2, Some(1));
    }

    #[test]
    fn select_child_runs_the_widget_path_and_reports_the_message() {
        let mut ui: Ui<usize> = Ui::new();
        ui.add(
            Segmented::new("seg", &["a", "b", "c"], 0, |index| index).disable(1),
        );
        ui.set_rect(WidgetId::new("seg"), Rect::from_xywh(0.0, 0.0, 300.0, 32.0));
        let mut sink = Sink::default();

        // A normal option selects and emits, invalidating exactly its rect.
        assert_eq!(ui.select_child(WidgetId::new("seg"), 2, &mut sink), Some(2));
        assert_eq!(sink.take(), vec![Rect::from_xywh(0.0, 0.0, 300.0, 32.0)]);
        // A disabled option and the current one emit nothing.
        assert_eq!(ui.select_child(WidgetId::new("seg"), 1, &mut sink), None);
        assert_eq!(ui.select_child(WidgetId::new("seg"), 2, &mut sink), None);
        // An unknown widget is a no-op.
        assert_eq!(ui.select_child(WidgetId::new("nope"), 0, &mut sink), None);
    }

    #[test]
    fn per_widget_font_reaches_the_native_editor_request() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add_with(
            TextEdit::new("field", "x", |_| 1u32),
            WidgetProps::new()
                .font_size(22.0)
                .font_family("Consolas"),
        );
        ui.set_rect(WidgetId::new("field"), Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("field"), &mut sink);
        ui.key(key_event(KeyCode::Enter), &mut sink);
        assert_eq!(
            sink.began_font.last(),
            Some(&(Some(22.0), Some("Consolas".to_string())))
        );
    }

    #[test]
    fn focus_scope_traps_tab_and_programmatic_focus() {
        let a = Rect::from_xywh(0.0, 0.0, 100.0, 36.0);
        let b = Rect::from_xywh(0.0, 50.0, 100.0, 86.0);
        let c = Rect::from_xywh(0.0, 100.0, 100.0, 136.0);
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("a", "A", 1u32));
        ui.add(Button::new("b", "B", 2u32));
        ui.add(Button::new("c", "C", 3u32));
        ui.set_rect(WidgetId::new("a"), a);
        ui.set_rect(WidgetId::new("b"), b);
        ui.set_rect(WidgetId::new("c"), c);
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("c"), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("c")));

        // Entering a scope that does not contain the focused widget moves
        // focus to the first member.
        ui.set_focus_scope(
            Some(vec![WidgetId::new("a"), WidgetId::new("b")]),
            &mut sink,
        );
        assert_eq!(ui.focus_id(), Some(WidgetId::new("a")));
        assert!(ui.focus_scope().is_some());
        // Programmatic focus outside the scope is refused.
        assert!(!ui.set_focus(WidgetId::new("c"), &mut sink));
        // Tab wraps inside the scope only.
        ui.key(key_event(KeyCode::Tab), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("b")));
        ui.key(key_event(KeyCode::Tab), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("a")));

        // Clearing the trap makes the third widget reachable again.
        ui.set_focus_scope(None, &mut sink);
        assert!(ui.set_focus(WidgetId::new("c"), &mut sink));
        assert_eq!(ui.focus_id(), Some(WidgetId::new("c")));
    }

    #[test]
    fn widget_paint_style_applies_the_font_override() {
        let painter = crate::painter::Painter::new();
        let mut canvas = AccentCanvas {
            accent: crate::Color::new(0, 0, 0),
            fills: 0,
        };
        let images = crate::image::ImageStore::new();
        let ctx = WidgetPaint {
            painter: &painter,
            canvas: &mut canvas,
            palette: *painter.palette(),
            font: Some(crate::props::FontSpec::new().size(20.0)),
            images: &images,
            viewport: Rect::from_xywh(0.0, 0.0, 200.0, 100.0),
        dirty: &[],
        };
        let style = ctx.style(painter.body());
        assert_eq!(style.size, 20.0);
        // Glyph runs are never restyled.
        let glyph = ctx.style(painter.glyph());
        assert_eq!(glyph, *painter.glyph());
    }

    fn painted_ui() -> (Ui<String>, Rect) {
        let rect = Rect::from_xywh(0.0, 0.0, 200.0, 34.0);
        let mut ui: Ui<String> = Ui::new();
        ui.add(TextEdit::new("field", "ab", |text| text).painted());
        ui.set_rect(WidgetId::new("field"), rect);
        (ui, rect)
    }

    fn widget_text(ui: &Ui<String>, id: &'static str) -> String {
        ui.with_widget::<TextEdit<String>, _>(WidgetId::new(id), |field| field.text().to_string())
            .unwrap()
    }

    fn is_editing(ui: &Ui<String>, id: &'static str) -> bool {
        ui.with_widget::<TextEdit<String>, _>(WidgetId::new(id), |field| field.is_editing())
            .unwrap()
    }

    #[test]
    fn a_multiline_painted_field_keeps_enter_and_commits_on_ctrl_enter() {
        let rect = Rect::from_xywh(0.0, 0.0, 200.0, 90.0);
        let mut ui: Ui<String> = Ui::new();
        ui.add(TextEdit::new("field", "ab", |text| text).painted().multiline());
        ui.set_rect(WidgetId::new("field"), rect);
        let mut sink = Sink::default();
        ui.pointer_down(Point::new(20.0, 17.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 17.0), &mut sink);
        ui.key(key_event(KeyCode::End), &mut sink);
        // Enter inserts a line break and keeps the session open.
        assert!(ui.key(key_event(KeyCode::Enter), &mut sink));
        assert!(is_editing(&ui, "field"));
        assert_eq!(widget_text(&ui, "field"), "ab\n");
        assert!(ui.char('c', &mut sink));
        assert_eq!(widget_text(&ui, "field"), "ab\nc");
        // Ctrl+Enter is the commit chord; the message carries both lines.
        assert!(ui.key(key_event_mod(KeyCode::Enter, true), &mut sink));
        assert_eq!(ui.drain_messages(), vec!["ab\nc".to_string()]);
        assert!(!is_editing(&ui, "field"));
        assert_eq!(ui.focus_id(), Some(WidgetId::new("field")));
    }

    #[test]
    fn clicking_a_painted_field_edits_in_place_and_escape_reverts() {
        let (mut ui, _) = painted_ui();
        let mut sink = Sink::default();
        ui.pointer_down(Point::new(20.0, 17.0), &mut sink);
        assert!(is_editing(&ui, "field"));
        assert!(!ui.hover_wants_text(), "a painted field opens no native child");
        assert!(sink.began.is_empty());
        // One paint populates the layout/caret cache the host receives.
        assert!(paint_painted_page(&mut ui).is_some());
        assert!(ui.char('c', &mut sink));
        assert_eq!(widget_text(&ui, "field"), "abc");
        assert!(sink.caret.is_some(), "the caret reaches the IME host");
        // Escape cancels the draft but keeps focus.
        assert!(ui.key(key_event(KeyCode::Escape), &mut sink));
        assert_eq!(widget_text(&ui, "field"), "ab");
        assert!(!is_editing(&ui, "field"));
        assert_eq!(ui.focus_id(), Some(WidgetId::new("field")));
    }

    #[test]
    fn tab_commits_and_moves_focus_and_a_click_commits() {
        let rect = Rect::from_xywh(0.0, 0.0, 200.0, 34.0);
        let button = Rect::from_xywh(0.0, 50.0, 100.0, 86.0);
        let mut ui: Ui<String> = Ui::new();
        ui.add(TextEdit::new("field", "ab", |text| text).painted());
        ui.add(Button::new("next", "Next", "next".to_string()));
        ui.set_rect(WidgetId::new("field"), rect);
        ui.set_rect(WidgetId::new("next"), button);
        let mut sink = Sink::default();
        ui.pointer_down(Point::new(20.0, 17.0), &mut sink);
        ui.char('!', &mut sink);
        assert!(ui.key(key_event(KeyCode::Tab), &mut sink));
        assert_eq!(ui.drain_messages(), vec!["ab!".to_string()]);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("next")));
        assert!(!is_editing(&ui, "field"));

        // Clicking another widget commits the draft too.
        ui.set_focus(WidgetId::new("field"), &mut sink);
        ui.pointer_down(Point::new(20.0, 17.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 17.0), &mut sink);
        ui.char('?', &mut sink);
        ui.pointer_down(Point::new(10.0, 60.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["ab!?".to_string()]);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("next")));
        assert_eq!(widget_text(&ui, "field"), "ab!?");
    }

    #[test]
    fn painted_field_copy_paste_and_ime_route_through_the_host() {
        let (mut ui, _) = painted_ui();
        let mut sink = Sink::default();
        ui.pointer_down(Point::new(20.0, 17.0), &mut sink);
        // Ctrl+A, Ctrl+C: the widget reports the text, the host writes it.
        assert!(ui.key(key_event_mod(KeyCode::Letter(0), true), &mut sink));
        assert!(ui.key(key_event_mod(KeyCode::Letter(2), true), &mut sink));
        assert_eq!(sink.copied, vec!["ab".to_string()]);

        // Ctrl+V reads the clipboard through the sink.
        sink.clipboard = Some("XY".to_string());
        assert!(ui.key(key_event_mod(KeyCode::Letter(21), true), &mut sink));
        assert_eq!(widget_text(&ui, "field"), "XY");
        // Enter commits the single-line draft and closes the session.
        assert!(ui.key(key_event(KeyCode::Enter), &mut sink));
        assert_eq!(ui.drain_messages(), vec!["XY".to_string()]);
        assert!(!is_editing(&ui, "field"));

        // IME composition starts a session when the field was only focused.
        assert!(ui.ime_compose("にほ", 1, &mut sink));
        assert!(is_editing(&ui, "field"));
        let composing = ui
            .with_widget::<TextEdit<String>, _>(WidgetId::new("field"), |field| {
                field.composition().to_string()
            })
            .unwrap();
        assert_eq!(composing, "にほ");
        assert!(ui.ime_commit("日", &mut sink));
        assert_eq!(widget_text(&ui, "field"), "XY日");
        assert!(is_editing(&ui, "field"));
    }

    #[test]
    fn painted_field_has_a_builtin_edit_menu() {
        let (mut ui, _) = painted_ui();
        let mut sink = Sink::default();
        ui.pointer_down_button(Point::new(20.0, 17.0), MouseButton::Right, &mut sink);
        assert!(is_editing(&ui, "field"));
        ui.pointer_up_button(Point::new(20.0, 17.0), MouseButton::Right, &mut sink);
        let (widget, _, menu, _) = sink.menus.last().expect("the edit menu opened");
        assert_eq!(*widget, WidgetId::new("field"));
        assert_eq!(
            menu.command(crate::widget::edit_command::SELECT_ALL)
                .map(|item| item.label().to_string()),
            Some("全选(&A)".to_string())
        );
        // The Unicode submenu is published in the built-in menu.
        let submenu = menu
            .entries()
            .iter()
            .find_map(|entry| match entry {
                crate::menu::MenuEntry::Submenu(sub) => Some(sub),
                _ => None,
            })
            .expect("the insert-Unicode submenu");
        assert_eq!(submenu.label(), "插入 Unicode 控制字符(&I)");
        assert_eq!(submenu.menu().len(), 6);
        // Paste follows the host clipboard: empty now, so disabled.
        assert!(
            menu.command(crate::widget::edit_command::PASTE)
                .is_some_and(|item| item.is_disabled()),
            "paste is disabled while the clipboard is empty"
        );
        // The application forwards the command id to `Ui`; the widget selects.
        assert!(ui.menu_command(
            WidgetId::new("field"),
            crate::widget::edit_command::SELECT_ALL,
            &mut sink
        ));
        ui.with_widget::<TextEdit<String>, _>(WidgetId::new("field"), |field| {
            assert_eq!(field.selection(), Some((0, 2)));
        });
        // A paste command uses the host clipboard.
        sink.clipboard = Some("Z".to_string());
        assert!(ui.menu_command(
            WidgetId::new("field"),
            crate::widget::edit_command::PASTE,
            &mut sink
        ));
        assert_eq!(widget_text(&ui, "field"), "Z");
        // Reopening the menu with text on the clipboard enables paste.
        ui.pointer_down_button(Point::new(20.0, 17.0), MouseButton::Right, &mut sink);
        ui.pointer_up_button(Point::new(20.0, 17.0), MouseButton::Right, &mut sink);
        let (_, _, menu, _) = sink.menus.last().expect("the edit menu reopened");
        assert!(
            menu.command(crate::widget::edit_command::PASTE)
                .is_some_and(|item| !item.is_disabled()),
            "paste is enabled when the clipboard holds text"
        );
        // Foreign ids are left to the application.
        assert!(!ui.menu_command(WidgetId::new("field"), 101, &mut sink));
    }

    #[test]
    fn right_click_keeps_a_painted_selection_and_only_opens_the_menu() {
        let (mut ui, _) = painted_ui();
        let mut sink = Sink::default();
        paint_painted_page(&mut ui);
        ui.pointer_down(Point::new(60.0, 17.0), &mut sink);
        ui.pointer_up(Point::new(60.0, 17.0), &mut sink);
        assert!(is_editing(&ui, "field"));
        ui.edit_widget::<TextEdit<String>>(WidgetId::new("field"), |field| {
            field.set_selection(0, 2)
        });
        let before = ui
            .with_widget::<TextEdit<String>, _>(WidgetId::new("field"), |field| {
                (field.caret(), field.selection())
            })
            .unwrap();
        sink.menus.clear();
        ui.pointer_down_button(Point::new(20.0, 17.0), MouseButton::Right, &mut sink);
        ui.pointer_up_button(Point::new(20.0, 17.0), MouseButton::Right, &mut sink);
        let after = ui
            .with_widget::<TextEdit<String>, _>(WidgetId::new("field"), |field| {
                (field.caret(), field.selection())
            })
            .unwrap();
        assert_eq!(before, after, "a secondary click never moves the caret");
        assert_eq!(after.1, Some((0, 2)));
        assert_eq!(sink.menus.len(), 1, "the edit menu still opens");
        // The preserved selection is what Copy takes.
        assert!(ui.menu_command(
            WidgetId::new("field"),
            crate::widget::edit_command::COPY,
            &mut sink
        ));
        assert_eq!(sink.copied, vec!["ab".to_string()]);
    }

    #[test]
    fn painted_field_starts_editing_on_enter_or_the_first_character() {
        let (mut ui, _) = painted_ui();
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("field"), &mut sink);
        assert!(ui.key(key_event(KeyCode::Enter), &mut sink));
        assert!(is_editing(&ui, "field"));
        assert!(ui.key(key_event(KeyCode::Escape), &mut sink));

        // A printable character on a focused field opens it and types.
        assert!(ui.char('!', &mut sink));
        assert!(is_editing(&ui, "field"));
        assert_eq!(widget_text(&ui, "field"), "ab!");
        // Control characters never insert.
        assert!(!ui.char('\u{8}', &mut sink));
    }

    #[test]
    fn scrim_pointer_state_never_repaints_the_page() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("a", "A", 1u32));
        ui.set_rect(WidgetId::new("a"), Rect::from_xywh(0.0, 0.0, 100.0, 36.0));
        ui.add(Scrim::new("scrim"));
        ui.set_rect(WidgetId::new("scrim"), Rect::from_xywh(0.0, 0.0, 300.0, 200.0));
        let mut sink = Sink::default();
        // Entering, pressing, releasing, wheeling and leaving a full-window
        // backdrop must not invalidate it: its pixels are static.
        ui.pointer_move(Point::new(10.0, 10.0), &mut sink);
        assert!(sink.take().is_empty());
        ui.pointer_down(Point::new(10.0, 10.0), &mut sink);
        assert!(sink.take().is_empty());
        ui.pointer_up(Point::new(10.0, 10.0), &mut sink);
        assert!(sink.take().is_empty());
        ui.wheel(Point::new(10.0, 10.0), 1.0, &mut sink);
        assert!(sink.take().is_empty());
        ui.pointer_leave(&mut sink);
        assert!(sink.take().is_empty());
    }

    #[test]
    fn painted_field_press_and_drag_selects_and_repaints() {        let rect = Rect::from_xywh(0.0, 0.0, 200.0, 34.0);
        let mut ui: Ui<String> = Ui::new();
        ui.add(TextEdit::new("field", "abcdef", |text| text).painted());
        ui.set_rect(WidgetId::new("field"), rect);
        let mut sink = Sink::default();
        paint_painted_page(&mut ui);
        ui.pointer_down(Point::new(20.0, 17.0), &mut sink);
        sink.take();
        // The drag is a selection change: it must repaint on every move, not
        // only when the button is released.
        ui.pointer_move(Point::new(60.0, 17.0), &mut sink);
        assert!(!sink.take().is_empty(), "a selection drag repaints the field");
        ui.pointer_up(Point::new(60.0, 17.0), &mut sink);
        let selection = ui
            .with_widget::<TextEdit<String>, _>(WidgetId::new("field"), |field| field.selection())
            .unwrap();
        assert_eq!(selection, Some((1, 6)));
    }

    fn field_and_button() -> (Ui<String>, WidgetId, Rect, WidgetId) {
        let field = WidgetId::new("field");
        let next = WidgetId::new("next");
        let mut ui: Ui<String> = Ui::new();
        ui.add(TextEdit::new("field", "alpha beta gamma", |value| value).painted());
        ui.add(Button::new("next", "Next", "next".to_string()));
        let rect = Rect::from_xywh(0.0, 0.0, 200.0, 34.0);
        ui.set_rect(field, rect);
        ui.set_rect(next, Rect::from_xywh(0.0, 60.0, 100.0, 90.0));
        (ui, field, rect, next)
    }

    #[test]
    fn a_touch_press_raises_the_touch_keyboard_and_leaving_dismisses_it() {
        let (mut ui, field, _, next) = field_and_button();
        paint_page(&mut ui);
        let mut sink = Sink::default();
        // The desktop model never asks for a system keyboard.
        ui.pointer_down(Point::new(20.0, 17.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 17.0), &mut sink);
        assert!(sink.soft_touch.is_empty());
        // A touch press opens the painted session and asks for the keyboard.
        ui.set_pointer_kind(PointerKind::Touch);
        ui.pointer_down(Point::new(20.0, 17.0), &mut sink);
        assert_eq!(sink.soft_touch, vec![true]);
        assert!(
            ui.with_widget::<TextEdit<String>, _>(field, |edit| edit.is_editing())
                .unwrap()
        );
        // Leaving the field commits and dismisses it.
        ui.set_focus(next, &mut sink);
        assert_eq!(sink.soft_touch, vec![true, false]);

        // A read-only session has nothing to type into: no keyboard.
        ui.set_props(field, WidgetProps::new().read_only(true), &mut sink);
        ui.pointer_down(Point::new(20.0, 17.0), &mut sink);
        assert_eq!(sink.soft_touch, vec![true, false]);
    }

    #[test]
    fn a_long_press_selects_the_word_under_the_finger() {
        let (mut ui, field, _, next) = field_and_button();
        paint_page(&mut ui);
        let mut sink = Sink::default();
        ui.set_pointer_kind(PointerKind::Touch);
        ui.pointer_down(Point::new(20.0, 17.0), &mut sink);
        sink.take();
        assert!(ui.long_press(Point::new(20.0, 17.0), &mut sink));
        let selection = ui
            .with_widget::<TextEdit<String>, _>(field, |edit| edit.selection())
            .unwrap();
        assert_eq!(selection, Some((0, 5)), "the word under the finger");
        assert!(!sink.take().is_empty(), "the word selection repaints");
        // Lifting the finger keeps the selection (the handles need it).
        ui.pointer_up(Point::new(20.0, 17.0), &mut sink);
        let selection = ui
            .with_widget::<TextEdit<String>, _>(field, |edit| edit.selection())
            .unwrap();
        assert_eq!(selection, Some((0, 5)));
        // Outside a text field there is nothing to pick.
        assert!(!ui.long_press(Point::new(20.0, 70.0), &mut sink));
        assert!(ui.set_focus(next, &mut sink));
    }

    #[test]
    fn dragging_inside_a_selection_into_another_field_copies_the_text() {
        let from = WidgetId::new("from");
        let to = WidgetId::new("to");
        let mut ui: Ui<u32> = Ui::new();
        ui.add(TextEdit::new("from", "alpha beta", |_| 0u32).painted());
        ui.add(TextEdit::new("to", "", |_| 0u32).painted());
        ui.set_rect(from, Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        ui.set_rect(to, Rect::from_xywh(0.0, 60.0, 200.0, 34.0));
        ui.set_text_drag_source(from);
        ui.set_text_drop_target(to, |text, _pos| Some(text.len() as u32));
        assert!(ui.edit_widget::<TextEdit<u32>>(from, |edit| edit.select_all()));
        paint_page(&mut ui);
        let mut sink = Sink::default();

        // A press inside the selection keeps it instead of collapsing it...
        ui.pointer_down(Point::new(60.0, 17.0), &mut sink);
        assert!(ui.drag_text().is_none());
        // ...and the drag threshold starts a text drag carrying the selection.
        ui.pointer_move(Point::new(120.0, 40.0), &mut sink);
        assert_eq!(ui.drag_text(), Some("alpha beta"));
        // Releasing over the other field inserts the text there.
        ui.pointer_up(Point::new(20.0, 77.0), &mut sink);
        assert!(ui.drag_text().is_none());
        assert_eq!(ui.drain_messages(), vec![10u32], "the drop handler reports");
        let into = ui
            .with_widget::<TextEdit<u32>, _>(to, |edit| edit.text().to_string())
            .unwrap();
        assert_eq!(into, "alpha beta");
        // A copy drag: the source keeps its text.
        let source = ui
            .with_widget::<TextEdit<u32>, _>(from, |edit| edit.text().to_string())
            .unwrap();
        assert_eq!(source, "alpha beta");
    }

    #[test]
    fn a_text_drag_only_reaches_registered_targets() {
        let a = WidgetId::new("a");
        let b = WidgetId::new("b");
        let plain = WidgetId::new("plain");
        let mut ui: Ui<u32> = Ui::new();
        ui.add(TextEdit::new("a", "hello", |_| 1u32).painted());
        ui.add(Button::new("b", "B", 2u32));
        ui.add(TextEdit::new("plain", "", |_| 3u32).painted());
        ui.set_rect(a, Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        ui.set_rect(b, Rect::from_xywh(0.0, 60.0, 200.0, 96.0));
        ui.set_rect(plain, Rect::from_xywh(0.0, 170.0, 200.0, 204.0));
        ui.set_text_drag_source(a);
        // A text target sees the carried text.
        let seen = std::rc::Rc::new(std::cell::Cell::new(0));
        let hits = seen.clone();
        ui.set_text_drop_target(b, move |text, _pos| {
            hits.set(hits.get() + if text == "hello" { 1 } else { 99 });
            None
        });
        assert!(ui.edit_widget::<TextEdit<u32>>(a, |edit| edit.select_all()));
        paint_page(&mut ui);
        let mut sink = Sink::default();
        ui.pointer_down(Point::new(60.0, 17.0), &mut sink);
        ui.pointer_move(Point::new(80.0, 40.0), &mut sink);
        assert_eq!(ui.drag_text(), Some("hello"));
        assert_eq!(ui.drag_data(), Some(&DragData::Text("hello".to_string())));
        ui.pointer_up(Point::new(20.0, 70.0), &mut sink);
        assert_eq!(seen.get(), 1);
        assert!(ui.drain_messages().is_empty());

        // A painted field that was not registered as a text target takes
        // nothing: dropping on widgets is explicit.
        ui.edit_widget::<TextEdit<u32>>(a, |edit| edit.select_all());
        ui.pointer_down(Point::new(60.0, 17.0), &mut sink);
        ui.pointer_move(Point::new(80.0, 40.0), &mut sink);
        ui.pointer_up(Point::new(20.0, 187.0), &mut sink);
        let plain_text = ui
            .with_widget::<TextEdit<u32>, _>(plain, |edit| edit.text().to_string())
            .unwrap();
        assert!(plain_text.is_empty());
    }

    #[test]
    fn remove_relocates_focus_and_drops_every_per_id_table() {
        let (mut ui, a, _) = two_buttons();
        let id_a = WidgetId::new("a");
        let id_b = WidgetId::new("b");
        ui.set_context_menu(id_a, Some(Menu::new().item(1, "x")));
        ui.set_cursor(id_a, Some(Cursor::IBeam));
        ui.on_click(id_a, |_, _| Some(99u32));
        ui.set_focus_order(id_a, 5);
        let mut sink = Sink::default();
        ui.set_focus_scope(Some(vec![id_a, id_b]), &mut sink);
        assert!(ui.set_focus(id_a, &mut sink));
        assert!(ui.remove(id_a, &mut sink));
        assert!(!ui.contains(id_a));
        assert_eq!(ui.len(), 1);
        assert_eq!(ui.focus_id(), Some(id_b), "focus slides to the next widget");
        assert!(ui.context_menu(id_a).is_none());
        assert_eq!(ui.focus_scope(), Some(&[id_b][..]));
        assert!(sink.take().contains(&a), "the vacated rectangle repaints");

        // The id is reusable and starts clean: no stale click binding.
        ui.insert_before(id_b, Button::new("a", "A", 1u32));
        let a_rect = Rect::from_xywh(0.0, 0.0, 100.0, 36.0);
        ui.set_rect(id_a, a_rect);
        ui.pointer_down(Point::new(10.0, 10.0), &mut sink);
        ui.pointer_up(Point::new(10.0, 10.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec![1]);
        // Removing twice (or an unknown id) is a no-op.
        assert!(!ui.remove(WidgetId::new("missing"), &mut sink));
    }

    #[test]
    fn remove_runs_the_lost_hook_of_the_focused_widget() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let (mut ui, _, _) = two_buttons();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        ui.on_focus_lost(WidgetId::new("a"), move |event| {
            log.borrow_mut().push((event.id, event.reason));
            FocusOutcome::Allow
        });
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("a"), &mut sink);
        assert!(ui.remove(WidgetId::new("a"), &mut sink));
        assert_eq!(
            *seen.borrow(),
            vec![(WidgetId::new("a"), FocusReason::Relocated)]
        );
    }

    #[test]
    fn remove_commits_an_open_painted_edit_and_leaves_other_sessions_alone() {
        let mut ui: Ui<String> = Ui::new();
        ui.add(TextEdit::new("field", "abc", |text| text).painted());
        ui.add(Button::new("go", "Go", "go".to_string()));
        ui.set_rect(WidgetId::new("field"), Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        ui.set_rect(WidgetId::new("go"), Rect::from_xywh(0.0, 60.0, 100.0, 96.0));
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("field"), &mut sink);
        assert!(ui.char('d', &mut sink));
        assert!(ui.remove(WidgetId::new("field"), &mut sink));
        assert_eq!(ui.drain_messages(), vec!["abcd".to_string()]);
        assert!(sink.caret.is_none(), "the caret timer is stopped");

        // A removal must not close a native session that belongs to another
        // widget.
        let mut ui: Ui<String> = Ui::new();
        ui.add(TextEdit::new("field", "abc", |text| text));
        ui.add(Button::new("go", "Go", "go".to_string()));
        ui.set_rect(WidgetId::new("field"), Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        ui.set_rect(WidgetId::new("go"), Rect::from_xywh(0.0, 60.0, 100.0, 96.0));
        let mut sink = Sink::default();
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        assert_eq!(sink.editing, Some(WidgetId::new("field")));
        assert!(ui.remove(WidgetId::new("go"), &mut sink));
        assert_eq!(sink.ended, 0);
        assert_eq!(sink.editing, Some(WidgetId::new("field")));
    }

    #[test]
    fn insert_raise_and_lower_change_paint_and_hit_testing_order() {
        let area = Rect::from_xywh(0.0, 0.0, 100.0, 36.0);
        let mut ui: Ui<u32> = Ui::new();
        for (id, message) in [("a", 1u32), ("b", 2)] {
            ui.add(Button::new(id, id, message));
            ui.set_rect(WidgetId::new(id), area);
        }
        let mut sink = Sink::default();
        // Later widgets paint on top and win the pointer.
        ui.pointer_move(Point::new(10.0, 10.0), &mut sink);
        assert_eq!(ui.hover_id(), Some(WidgetId::new("b")));

        // Raising a keeps the hover on the widget under the pointer (the
        // container's index moved with it).
        assert!(ui.raise(WidgetId::new("a")));
        assert_eq!(ui.hover_id(), Some(WidgetId::new("b")));
        ui.pointer_move(Point::new(50.0, 20.0), &mut sink);
        assert_eq!(ui.hover_id(), Some(WidgetId::new("a")));
        assert!(!ui.raise(WidgetId::new("a")), "already topmost");

        // insert_before puts the new widget below the anchor.
        ui.insert_before(WidgetId::new("a"), Button::new("c", "C", 3u32));
        ui.set_rect(WidgetId::new("c"), area);
        assert_eq!(
            ui.ids().collect::<Vec<_>>(),
            vec![
                WidgetId::new("b"),
                WidgetId::new("c"),
                WidgetId::new("a")
            ]
        );
        ui.pointer_move(Point::new(90.0, 30.0), &mut sink);
        assert_eq!(ui.hover_id(), Some(WidgetId::new("a")));

        // Lowering a leaves it at the bottom, so the last-added widget wins.
        assert!(ui.lower(WidgetId::new("a")));
        assert_eq!(
            ui.ids().collect::<Vec<_>>(),
            vec![
                WidgetId::new("a"),
                WidgetId::new("b"),
                WidgetId::new("c")
            ]
        );
        ui.pointer_move(Point::new(10.0, 10.0), &mut sink);
        assert_eq!(ui.hover_id(), Some(WidgetId::new("c")));
        assert!(!ui.lower(WidgetId::new("a")), "already bottom");
        assert!(!ui.raise(WidgetId::new("missing")));
    }

    #[test]
    fn insert_keeps_the_focused_widgets_identity() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("a", "A", 1u32));
        ui.add(Button::new("b", "B", 2u32));
        ui.set_rect(WidgetId::new("a"), Rect::from_xywh(0.0, 0.0, 100.0, 36.0));
        ui.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 50.0, 100.0, 86.0));
        let mut sink = Sink::default();
        assert!(ui.set_focus(WidgetId::new("b"), &mut sink));
        ui.insert(0, Button::new("c", "C", 3u32));
        ui.set_rect(WidgetId::new("c"), Rect::from_xywh(0.0, 100.0, 100.0, 136.0));
        assert_eq!(ui.focus_id(), Some(WidgetId::new("b")));
    }

    #[test]
    fn move_rect_invalidates_the_union_and_moves_the_open_editor() {
        let (mut ui, a, _) = two_buttons();
        let id = WidgetId::new("a");
        let mut sink = Sink::default();
        sink.editing = Some(id);
        let target = Rect::from_xywh(40.0, 40.0, 100.0, 76.0);
        ui.move_rect(id, target, &mut sink);
        assert_eq!(sink.take(), vec![a.union(target)]);
        assert_eq!(sink.moves, vec![target]);
        assert_eq!(ui.rect(id), Some(target));
        // An unknown id is a no-op.
        sink.moves.clear();
        ui.move_rect(WidgetId::new("missing"), target, &mut sink);
        assert!(sink.moves.is_empty());
    }

    // ---- scroll views ----------------------------------------------------

    /// Records fills and clip pushes so a paint test can assert what was drawn
    /// and that it was clipped to the viewport.
    struct ClipRecorder {
        fills: Vec<Rect>,
        clips: Vec<Rect>,
        depth: usize,
    }

    impl ClipRecorder {
        fn new() -> Self {
            Self {
                fills: Vec::new(),
                clips: Vec::new(),
                depth: 0,
            }
        }
    }

    impl Canvas for ClipRecorder {
        fn scale(&self) -> f32 {
            1.0
        }
        fn dpi(&self) -> f32 {
            96.0
        }
        fn fill_rect(&mut self, rect: Rect, _color: crate::Color) {
            self.fills.push(rect);
        }
        fn text(
            &mut self,
            _rect: Rect,
            _text: &str,
            _style: &crate::backend::TextStyle,
            _align: crate::backend::TextAlign,
            _color: crate::Color,
        ) {
        }
        fn measure_text(&mut self, text: &str, _style: &crate::backend::TextStyle) -> crate::Size {
            crate::Size::new(text.chars().count() as f32 * 8.0, 16.0)
        }
        fn ellipse(&mut self, rect: Rect, _color: crate::Color) {
            self.fills.push(rect);
        }
        fn vertical_gradient(&mut self, _rect: Rect, _top: crate::Color, _bottom: crate::Color) {}
        fn image(
            &mut self,
            _rect: Rect,
            _crop: bool,
            _id: crate::backend::ImageId,
            _data: crate::backend::ImageData<'_>,
        ) {
        }
        fn push_clip(&mut self, rect: Rect) {
            self.clips.push(rect);
            self.depth += 1;
        }
        fn pop_clip(&mut self) {
            self.depth = self.depth.saturating_sub(1);
        }
    }

    fn paint_into_recorder(ui: &mut Ui<u32>, size: crate::Size) -> ClipRecorder {
        let painter = crate::painter::Painter::new();
        let mut canvas = ClipRecorder::new();
        let dirty = vec![Rect::from_size(size)];
        let mut ctx = PaintCtx {
            painter: &painter,
            canvas: &mut canvas,
            dirty: &dirty,
            size,
            dpi: 96.0,
            editing: None,
            caret_on: true,
            overlay: false,
            overlay_first: false,
            caret: None,
        };
        ui.paint(&mut ctx);
        canvas
    }

    /// A 100x100 viewport with three 30 DIP buttons stacked at 0/40/80.
    fn scroll_page() -> (Ui<u32>, Sink) {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("s0", "0", 10u32));
        ui.add(Button::new("s1", "1", 11u32));
        ui.add(Button::new("s2", "2", 12u32));
        let mut sink = Sink::default();
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                crate::widget::ScrollBars::Vertical,
            )
            .children([
                ScrollChild::new(WidgetId::new("s0"), Rect::from_xywh(0.0, 0.0, 100.0, 30.0)),
                ScrollChild::new(WidgetId::new("s1"), Rect::from_xywh(0.0, 40.0, 100.0, 30.0)),
                ScrollChild::new(WidgetId::new("s2"), Rect::from_xywh(0.0, 80.0, 100.0, 30.0)),
            ]),
            &mut sink,
        );
        (ui, sink)
    }

    #[test]
    fn a11y_scroll_moves_a_scroll_view_and_publishes_the_axis() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("s0", "0", 10u32));
        let mut sink = Sink::default();
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                crate::widget::ScrollBars::Vertical,
            )
            .named("panel")
            .children([ScrollChild::new(
                WidgetId::new("s0"),
                Rect::from_xywh(0.0, 0.0, 100.0, 300.0),
            )]),
            &mut sink,
        );
        let panel = WidgetId::new("panel");
        let action = |vertical| A11yAction {
            kind: A11yActionKind::Scroll {
                horizontal: None,
                vertical,
            },
            id: panel,
            child: None,
            value: None,
            number: None,
        };
        // Registration painted the viewport once; the scroll repaints it once.
        sink.take();
        assert!(ui.apply_a11y(&action(Some(120.0)), &mut sink).is_none());
        assert_eq!(ui.scroll_offset(panel), Some(Point::new(0.0, 120.0)));
        assert_eq!(sink.take(), vec![Rect::from_xywh(0.0, 0.0, 100.0, 100.0)]);

        let nodes = ui.a11y_nodes();
        let pane = nodes
            .iter()
            .find(|node| node.id == panel && node.child.is_none())
            .expect("named scroll pane node");
        let axis = pane.scroll.expect("scroll state").vertical.expect("vertical axis");
        let view = ui.scroll_view(panel).unwrap();
        let max = 300.0 - view.content_rect().height();
        assert_eq!(axis.viewport, view.content_rect().height());
        assert_eq!(axis.content, 300.0);
        assert_eq!(axis.offset, 120.0);
        assert!((axis.percent().unwrap() - (120.0 / max * 100.0) as f64).abs() < 0.01);

        // A target past the end clamps; a request at the same offset is a
        // no-op and repaints nothing.
        ui.apply_a11y(&action(Some(9_000.0)), &mut sink);
        assert!((ui.scroll_offset(panel).unwrap().y - max).abs() < 0.01);
        sink.take();
        ui.apply_a11y(&action(Some(max)), &mut sink);
        assert!(sink.take().is_empty());
    }

    #[test]
    fn scroll_view_positions_children_and_clamps_the_offset() {
        let (mut ui, mut sink) = scroll_page();
        assert_eq!(
            ui.rect(WidgetId::new("s2")),
            Some(Rect::from_xywh(0.0, 80.0, 100.0, 30.0))
        );
        // Registering repaints the viewport once.
        assert_eq!(sink.take(), vec![Rect::from_xywh(0.0, 0.0, 100.0, 100.0)]);
        // Content is 110 DIPs tall, so only 10 DIPs of travel exist.
        assert_eq!(
            ui.scroll_view(WidgetId::new("panel"))
                .unwrap()
                .content_size()
                .height,
            110.0
        );
        assert!(ui.scroll_by(WidgetId::new("panel"), 0.0, 50.0, &mut sink));
        assert_eq!(
            ui.scroll_offset(WidgetId::new("panel")),
            Some(Point::new(0.0, 10.0))
        );
        assert_eq!(
            ui.rect(WidgetId::new("s2")),
            Some(Rect::from_xywh(0.0, 70.0, 100.0, 30.0))
        );
        // The whole viewport repaints on a scroll.
        assert_eq!(sink.take(), vec![Rect::from_xywh(0.0, 0.0, 100.0, 100.0)]);
        // Already at the end: no move, no repaint.
        assert!(!ui.scroll_by(WidgetId::new("panel"), 0.0, 50.0, &mut sink));
        assert!(sink.take().is_empty());
        // Re-registering keeps the live offset (a resize does not jump home).
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                crate::widget::ScrollBars::Vertical,
            )
            .children([ScrollChild::new(
                WidgetId::new("s2"),
                Rect::from_xywh(0.0, 80.0, 100.0, 30.0),
            )]),
            &mut sink,
        );
        assert_eq!(
            ui.rect(WidgetId::new("s2")),
            Some(Rect::from_xywh(0.0, 70.0, 100.0, 30.0))
        );
    }

    #[test]
    fn the_content_origin_is_the_viewport_corner() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("only", "Only", 1u32));
        let mut sink = Sink::default();
        let viewport = Rect::from_xywh(40.0, 20.0, 100.0, 60.0);
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                viewport,
                crate::widget::ScrollBars::Vertical,
            )
            .child(WidgetId::new("only"), Rect::from_xywh(0.0, 10.0, 100.0, 30.0)),
            &mut sink,
        );
        assert_eq!(
            ui.rect(WidgetId::new("only")),
            Some(Rect::new(40.0, 30.0, 140.0, 60.0))
        );
        // Content (40 DIPs) fits the 60 DIP viewport: nothing scrolls.
        assert!(!ui.scroll_by(WidgetId::new("panel"), 0.0, 10.0, &mut sink));
    }

    #[test]
    fn padding_keeps_the_content_off_the_bars() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("wide", "Wide", 1u32));
        let mut sink = Sink::default();
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                crate::widget::ScrollBars::Vertical,
            )
            .padding(8.0)
            .child(WidgetId::new("wide"), Rect::from_xywh(0.0, 0.0, 300.0, 200.0)),
            &mut sink,
        );
        // Content starts at the padding, not the viewport corner.
        assert_eq!(
            ui.rect(WidgetId::new("wide")),
            Some(Rect::new(8.0, 8.0, 308.0, 208.0))
        );
        // At the end of the range the row stops 8 DIPs before the bar's inner
        // edge (viewport 100 - 10 bar - 8 padding), so the bar never covers it.
        assert!(ui.set_scroll_offset(
            WidgetId::new("panel"),
            Point::new(1000.0, 0.0),
            &mut sink
        ));
        let rect = ui.rect(WidgetId::new("wide")).unwrap();
        assert!((rect.right - 82.0).abs() < 0.01, "right edge {rect:?}");
        // A press on the bar strip belongs to the bar (the container scrolls),
        // never to the child underneath.
        ui.pointer_down(Point::new(95.0, 20.0), &mut sink);
        assert!(ui.press_id().is_none());
        ui.pointer_up(Point::new(95.0, 20.0), &mut sink);
        assert!(ui.drain_messages().is_empty());
    }

    #[test]
    fn wheel_scrolls_the_panel_unless_a_child_owns_the_wheel() {
        let (mut ui, mut sink) = scroll_page();
        sink.take();
        // Over the first button: the button ignores the wheel, the panel moves
        // by the system's notch (three lines ≈ 48 DIPs), clamped to the 10 DIP
        // range.
        assert!(ui.wheel(Point::new(50.0, 10.0), -1.0, &mut sink));
        assert_eq!(
            ui.scroll_offset(WidgetId::new("panel")),
            Some(Point::new(0.0, 10.0))
        );
        // Up again reaches the top.
        assert!(ui.wheel(Point::new(50.0, 10.0), 1.0, &mut sink));
        assert_eq!(
            ui.scroll_offset(WidgetId::new("panel")),
            Some(Point::new(0.0, 0.0))
        );
        // A list inside the panel consumes its own wheel (D65).
        let mut ui: Ui<u32> = Ui::new();
        ui.add(ListView::new("rows"));
        let mut sink = Sink::default();
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                crate::widget::ScrollBars::Vertical,
            )
            .child(WidgetId::new("rows"), Rect::from_xywh(0.0, 0.0, 100.0, 400.0)),
            &mut sink,
        );
        ui.wheel(Point::new(50.0, 50.0), -1.0, &mut sink);
        assert_eq!(
            ui.scroll_offset(WidgetId::new("panel")),
            Some(Point::new(0.0, 0.0))
        );
    }

    /// An outer page containing a nested panel. The outer's content is 400
    /// DIPs tall in a 150 DIP viewport; the inner viewport is a child rectangle
    /// in the outer's content coordinates with six 30 DIP rows in a 100 DIP
    /// viewport.
    fn nested_scroll_page() -> (Ui<u32>, Sink) {
        let mut ui: Ui<u32> = Ui::new();
        let outer = WidgetId::new("outer");
        let inner = WidgetId::new("inner");
        for index in 0..6 {
            ui.add(Button::new(
                match index {
                    0 => "n0",
                    1 => "n1",
                    2 => "n2",
                    3 => "n3",
                    4 => "n4",
                    _ => "n5",
                },
                "row",
                20u32 + index,
            ));
        }
        let mut sink = Sink::default();
        ui.set_scroll_view(
            ScrollView::new(
                outer,
                Rect::from_xywh(0.0, 0.0, 200.0, 150.0),
                crate::widget::ScrollBars::Vertical,
            )
            .content(crate::Size::new(200.0, 400.0)),
            &mut sink,
        );
        ui.set_scroll_view(
            ScrollView::new(
                inner,
                Rect::from_xywh(10.0, 100.0, 180.0, 100.0),
                crate::widget::ScrollBars::Vertical,
            )
            .nested_in(outer)
            .children((0..6).map(|index| {
                let id = WidgetId::new(ROW_IDS[index]);
                ScrollChild::new(id, Rect::from_xywh(0.0, index as f32 * 30.0, 180.0, 30.0))
            })),
            &mut sink,
        );
        (ui, sink)
    }

    const ROW_IDS: [&str; 6] = ["n0", "n1", "n2", "n3", "n4", "n5"];

    #[test]
    fn a_nested_panel_moves_and_clips_with_its_parent() {
        let (mut ui, mut sink) = nested_scroll_page();
        let outer = WidgetId::new("outer");
        let inner = WidgetId::new("inner");
        // The inner viewport is resolved in the outer's content area; its clip
        // never leaves the outer viewport.
        let viewport = ui.scroll_view(inner).unwrap().viewport();
        assert!((viewport.left - 10.0).abs() < 0.01);
        let clip = ui.scroll_view(inner).unwrap().clip_rect();
        assert!(clip.top >= 0.0);
        assert!(clip.bottom <= 150.0 + 0.01);
        assert_eq!(ui.rect(WidgetId::new("n0")), Some(Rect::from_xywh(viewport.left, viewport.top, 180.0, 30.0)));
        // The outer scroll moves the nested viewport and its content.
        sink.take();
        assert!(ui.set_scroll_offset(outer, Point::new(0.0, 40.0), &mut sink));
        let moved = ui.scroll_view(inner).unwrap().viewport();
        assert!((moved.top - (viewport.top - 40.0)).abs() < 0.01);
        assert_eq!(ui.rect(WidgetId::new("n0")), Some(Rect::from_xywh(moved.left, moved.top, 180.0, 30.0)));
        // A nested child is owned by the inner panel; a point outside the
        // outer viewport is not part of either.
        assert_eq!(ui.scroll_owner_of(WidgetId::new("n0")), Some(inner));
        assert!(!ui
            .scroll_view(inner)
            .unwrap()
            .clip_rect()
            .contains_point(Point::new(100.0, 20.0)));
    }

    #[test]
    fn a_nested_panels_bars_are_clipped_to_its_visible_part() {
        let (mut ui, _sink) = nested_scroll_page();
        let inner = WidgetId::new("inner");
        // The inner viewport (100 DIPs tall at y=100) extends past the outer
        // viewport (150 tall), so only its top 50 DIPs are visible.
        let clip = ui.scroll_view(inner).unwrap().clip_rect();
        let viewport = ui.scroll_view(inner).unwrap().viewport();
        assert!(clip.height() < viewport.height() - 1.0);
        let recorder = paint_into_recorder(&mut ui, crate::Size::new(200.0, 150.0));
        assert!(
            recorder.clips.contains(&clip),
            "the nested bars clip to the intersection, not the raw viewport"
        );
        assert!(
            !recorder.clips.contains(&viewport),
            "a nested viewport outside its parent is never a paint clip"
        );
    }

    #[test]
    fn the_wheel_scrolls_the_inner_panel_then_hands_over_to_the_outer() {
        let (mut ui, mut sink) = nested_scroll_page();
        let outer = WidgetId::new("outer");
        let inner = WidgetId::new("inner");
        let pos = ui.scroll_view(inner).unwrap().clip_rect().center();
        // The inner panel scrolls first.
        assert!(ui.wheel(pos, -1.0, &mut sink));
        assert!(ui.scroll_offset(inner).unwrap().y > 0.0);
        assert_eq!(ui.scroll_offset(outer), Some(Point::new(0.0, 0.0)));
        // Wheel until the inner reaches its end, then one more notch bubbles
        // to the outer page.
        for _ in 0..40 {
            ui.wheel(pos, -1.0, &mut sink);
        }
        let inner_max = ui.scroll_view(inner).unwrap().max_offset().y;
        assert!((ui.scroll_offset(inner).unwrap().y - inner_max).abs() < 0.01);
        assert!(ui.scroll_offset(outer).unwrap().y > 0.0);
        // With the pointer outside the inner viewport the outer page scrolls
        // directly.
        let outside = Point::new(5.0, 20.0);
        assert!(ui.wheel(outside, 1.0, &mut sink));
        assert!(ui.scroll_offset(outer).unwrap().y < 400.0);
    }

    #[test]
    fn removing_the_outer_panel_returns_the_nested_one_to_a_root() {
        let (mut ui, mut sink) = nested_scroll_page();
        let outer = WidgetId::new("outer");
        let inner = WidgetId::new("inner");
        ui.set_scroll_offset(outer, Point::new(0.0, 40.0), &mut sink);
        assert!(ui.remove_scroll_view(outer, &mut sink));
        assert_eq!(ui.scroll_view(inner).unwrap().parent(), None);
        assert_eq!(
            ui.scroll_view(inner).unwrap().viewport(),
            Rect::from_xywh(10.0, 100.0, 180.0, 100.0)
        );
    }

    #[test]
    fn scrolled_out_children_are_not_hit_and_paint_nothing() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("in", "In", 1u32));
        ui.add(Button::new("out", "Out", 2u32));
        let mut sink = Sink::default();
        let viewport = Rect::from_xywh(0.0, 0.0, 200.0, 100.0);
        let inside = Rect::from_xywh(0.0, 0.0, 200.0, 40.0);
        let outside = Rect::from_xywh(0.0, 100.0, 200.0, 40.0);
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                viewport,
                crate::widget::ScrollBars::Vertical,
            )
            .children([
                ScrollChild::new(WidgetId::new("in"), inside),
                ScrollChild::new(WidgetId::new("out"), outside),
            ]),
            &mut sink,
        );
        // The press lands on the child just below the fold: nothing is hit.
        ui.pointer_down(Point::new(50.0, 120.0), &mut sink);
        ui.pointer_up(Point::new(50.0, 120.0), &mut sink);
        assert!(ui.drain_messages().is_empty());
        // A press inside the viewport reaches the visible child.
        ui.pointer_down(Point::new(50.0, 20.0), &mut sink);
        ui.pointer_up(Point::new(50.0, 20.0), &mut sink);
        assert_eq!(ui.drain_messages(), vec![1]);
        // The visible child paints under the viewport clip; the scrolled-out
        // one is skipped before it draws anything.
        let canvas = paint_into_recorder(&mut ui, crate::Size::new(240.0, 200.0));
        assert!(canvas.clips.contains(&viewport));
        assert!(!canvas.fills.iter().any(|rect| *rect == outside));
        assert!(canvas.fills.iter().any(|rect| rect.intersects(inside)));
        assert_eq!(canvas.depth, 0, "clips are balanced");
    }

    #[test]
    fn bar_drag_and_track_click_never_press_the_child() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("full", "Full", 1u32));
        let mut sink = Sink::default();
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                crate::widget::ScrollBars::Vertical,
            )
            .child(WidgetId::new("full"), Rect::from_xywh(0.0, 0.0, 100.0, 300.0)),
            &mut sink,
        );
        // Thumb: 100/300 of the 100 DIP track at offset 0.
        assert!(ui.pointer_down(Point::new(95.0, 10.0), &mut sink));
        assert!(ui.press_id().is_none(), "the bar does not press the button");
        assert!(ui.pointer_move(Point::new(95.0, 60.0), &mut sink));
        let dragged = ui.scroll_offset(WidgetId::new("panel")).unwrap();
        assert!((dragged.y - 150.0).abs() < 0.01, "grab offset is preserved");
        assert!(ui.pointer_up(Point::new(95.0, 60.0), &mut sink));
        assert!(ui.drain_messages().is_empty());
        assert!(!ui
            .scroll_view(WidgetId::new("panel"))
            .unwrap()
            .bar_state(true)
            .active);
        // Track click below the thumb jumps (native jump behavior).
        ui.pointer_down(Point::new(95.0, 95.0), &mut sink);
        ui.pointer_up(Point::new(95.0, 95.0), &mut sink);
        let jumped = ui.scroll_offset(WidgetId::new("panel")).unwrap();
        assert!((jumped.y - 200.0).abs() < 0.01, "track click jumps to the end");
        assert!(ui.drain_messages().is_empty());
        assert_eq!(ui.hover_cursor(), Cursor::Arrow);
    }

    #[test]
    fn keyboard_focus_reveals_a_scrolled_out_child() {
        let (mut ui, mut sink) = scroll_page();
        // A tighter viewport makes s2 (80..110) start below the fold.
        ui.set_scroll_viewport(
            WidgetId::new("panel"),
            Rect::from_xywh(0.0, 0.0, 100.0, 70.0),
            &mut sink,
        );
        assert_eq!(
            ui.scroll_offset(WidgetId::new("panel")),
            Some(Point::new(0.0, 0.0))
        );
        ui.set_focus(WidgetId::new("s2"), &mut sink);
        // Reveal compares against the padded viewport: the bar overlays the
        // edge, so a full-width row is not scrolled sideways for it.
        assert_eq!(
            ui.scroll_offset(WidgetId::new("panel")),
            Some(Point::new(0.0, 40.0))
        );
        let rect = ui.rect(WidgetId::new("s2")).unwrap();
        assert!(rect.bottom <= 70.0 && rect.top >= 0.0);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("s2")));
    }

    #[test]
    fn clicking_a_partly_visible_child_does_not_scroll_the_panel() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("wide", "Wide", 1u32));
        let mut sink = Sink::default();
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                crate::widget::ScrollBars::Both,
            )
            .child(WidgetId::new("wide"), Rect::from_xywh(80.0, 0.0, 120.0, 40.0)),
            &mut sink,
        );
        // Only the left 20 DIPs are inside the viewport; clicking there focuses
        // the button but must not scroll under the cursor.
        ui.pointer_down(Point::new(90.0, 20.0), &mut sink);
        ui.pointer_up(Point::new(90.0, 20.0), &mut sink);
        assert_eq!(ui.focus_id(), Some(WidgetId::new("wide")));
        assert_eq!(
            ui.scroll_offset(WidgetId::new("panel")),
            Some(Point::new(0.0, 0.0))
        );
        assert_eq!(ui.drain_messages(), vec![1]);
        // Keyboard/a11y focus does reveal it: focus starts elsewhere, then
        // moves into the partly visible child like a Tab step.
        ui.add(Button::new("other", "Other", 2u32));
        ui.set_rect(WidgetId::new("other"), Rect::from_xywh(0.0, 120.0, 100.0, 36.0));
        ui.set_focus(WidgetId::new("other"), &mut sink);
        assert_eq!(
            ui.scroll_offset(WidgetId::new("panel")),
            Some(Point::new(0.0, 0.0))
        );
        ui.set_focus(WidgetId::new("wide"), &mut sink);
        assert_eq!(
            ui.scroll_offset(WidgetId::new("panel")),
            Some(Point::new(100.0, 0.0))
        );
    }

    #[test]
    fn a11y_groups_the_panel_and_marks_offscreen_children() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add_with(
            Button::new("top", "Top", 1u32),
            WidgetProps::new().automation_id("scroll-top"),
        );
        ui.add_with(
            Button::new("below", "Below", 2u32),
            WidgetProps::new().automation_id("scroll-below"),
        );
        let mut sink = Sink::default();
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                crate::widget::ScrollBars::Vertical,
            )
            .children([
                ScrollChild::new(WidgetId::new("top"), Rect::from_xywh(0.0, 0.0, 100.0, 40.0)),
                ScrollChild::new(WidgetId::new("below"), Rect::from_xywh(0.0, 160.0, 100.0, 40.0)),
            ])
            .name("面板"),
            &mut sink,
        );
        let nodes = ui.a11y_nodes();
        let pane = nodes
            .iter()
            .position(|node| node.role == A11yRole::Group && node.name == "面板")
            .expect("named group");
        assert_eq!(nodes[pane].rect, Rect::from_xywh(0.0, 0.0, 100.0, 100.0));
        let top = nodes
            .iter()
            .position(|node| node.id == WidgetId::new("top"))
            .unwrap();
        let below = nodes
            .iter()
            .position(|node| node.id == WidgetId::new("below"))
            .unwrap();
        assert_eq!(nodes[top].parent, Some(pane));
        assert_eq!(nodes[below].parent, Some(pane));
        assert_eq!(nodes[pane].children, vec![top, below]);
        assert!(!nodes[top].offscreen);
        assert!(nodes[below].offscreen, "scrolled-out child is offscreen");
        assert_eq!(nodes[below].rect, Rect::from_xywh(0.0, 160.0, 100.0, 40.0));
        // A partially visible child reports its clipped rectangle.
        ui.scroll_by(WidgetId::new("panel"), 0.0, 100.0, &mut sink);
        let nodes = ui.a11y_nodes();
        let below = nodes
            .iter()
            .position(|node| node.id == WidgetId::new("below"))
            .unwrap();
        assert!(!nodes[below].offscreen);
        // Clipped to the viewport (the clip rectangle is never shrunk by a bar).
        assert_eq!(nodes[below].rect, Rect::from_xywh(0.0, 60.0, 100.0, 40.0));
    }

    #[test]
    fn a_track_press_releases_its_held_state_and_keeps_scrolling() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("full", "Full", 1u32));
        let mut sink = Sink::default();
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                crate::widget::ScrollBars::Vertical,
            )
            .child(WidgetId::new("full"), Rect::from_xywh(0.0, 0.0, 100.0, 300.0)),
            &mut sink,
        );
        // A track press jumps and marks the bar held.
        assert!(ui.pointer_down(Point::new(95.0, 95.0), &mut sink));
        assert!(
            ui.scroll_view(WidgetId::new("panel"))
                .unwrap()
                .bar_state(true)
                .active
        );
        // Holding and moving keeps scrolling (the press grabbed the thumb).
        assert!(ui.pointer_move(Point::new(95.0, 40.0), &mut sink));
        let dragged = ui.scroll_offset(WidgetId::new("panel")).unwrap().y;
        assert!(dragged < 200.0, "dragged to {dragged}");
        // The release clears the held state: no stuck thumb.
        assert!(ui.pointer_up(Point::new(95.0, 40.0), &mut sink));
        assert!(
            !ui.scroll_view(WidgetId::new("panel"))
                .unwrap()
                .bar_state(true)
                .active
        );
        assert!(ui.pointer_up(Point::new(95.0, 40.0), &mut sink) == false);
        assert!(ui.drain_messages().is_empty());
    }

    #[test]
    fn ctrl_and_shift_clicks_reach_a_multi_select_list() {
        use crate::event::PointerModifiers;
        use crate::widgets::MultiSelectList;
        let mut ui: Ui<Vec<usize>> = Ui::new();
        ui.add(MultiSelectList::new("multi", &["a", "b", "c", "d"], |rows| rows));
        ui.set_rect(WidgetId::new("multi"), Rect::from_xywh(0.0, 0.0, 220.0, 160.0));
        let mut sink = Sink::default();
        let click = |ui: &mut Ui<Vec<usize>>, y: f32, modifiers: PointerModifiers, sink: &mut Sink| {
            let pos = Point::new(20.0, y);
            ui.pointer_down_with(pos, MouseButton::Left, modifiers, sink);
            ui.pointer_up_with(pos, MouseButton::Left, modifiers, sink);
            ui.drain_messages()
        };
        // A plain click toggles and anchors.
        assert_eq!(click(&mut ui, 10.0, PointerModifiers::NONE, &mut sink), vec![vec![0]]);
        // Shift+click extends from the anchor.
        assert_eq!(
            click(&mut ui, 66.0, PointerModifiers::shift(true), &mut sink),
            vec![vec![0, 1, 2]]
        );
        // Ctrl+click toggles one row and is not sent to other widgets.
        assert_eq!(
            click(&mut ui, 38.0, PointerModifiers::ctrl(true), &mut sink),
            vec![vec![0, 2]]
        );
        // A modified click on an ordinary widget is still a plain click.
        let mut plain: Ui<u32> = Ui::new();
        plain.add(Button::new("b", "B", 1u32));
        plain.set_rect(WidgetId::new("b"), Rect::from_xywh(0.0, 0.0, 100.0, 36.0));
        let mut sink = Sink::default();
        plain.pointer_down_with(Point::new(10.0, 10.0), MouseButton::Left, PointerModifiers::shift(true), &mut sink);
        plain.pointer_up_with(Point::new(10.0, 10.0), MouseButton::Left, PointerModifiers::shift(true), &mut sink);
        assert_eq!(plain.drain_messages(), vec![1]);
    }

    #[test]
    fn dragging_the_bar_reaches_the_same_end_as_the_setter() {
        // A padded panel with a visible bar: the pointer drag has to map to the
        // same range the clamp uses, or the bar stops short of the end and the
        // content never leaves the bar's strip.
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("wide", "Wide", 1u32));
        let mut sink = Sink::default();
        let panel = WidgetId::new("panel");
        ui.set_scroll_view(
            ScrollView::new(
                panel,
                Rect::from_xywh(0.0, 0.0, 250.0, 214.0),
                crate::widget::ScrollBars::Both,
            )
            .padding(12.0)
            .child(WidgetId::new("wide"), Rect::from_xywh(0.0, 0.0, 300.0, 268.0)),
            &mut sink,
        );
        let max = ui
            .scroll_view(panel)
            .map(|view| view.max_offset())
            .unwrap();
        assert!(max.x > 0.0 && max.y > 0.0);
        assert!(ui.set_scroll_offset(panel, max, &mut sink));
        assert_eq!(ui.scroll_offset(panel), Some(max));
        assert!(ui.set_scroll_offset(panel, Point::new(0.0, 0.0), &mut sink));

        // Drag the horizontal thumb all the way right with the pointer.
        let bar = crate::scroll::bar(ui.scroll_view(panel).unwrap(), false).unwrap();
        let press = Point::new(bar.thumb.center().x, bar.thumb.center().y);
        assert!(ui.pointer_down(press, &mut sink));
        let released = ui.pointer_move(Point::new(bar.track.right + 200.0, press.y), &mut sink);
        assert!(released);
        assert!((ui.scroll_offset(panel).unwrap().x - max.x).abs() < 0.01);
        ui.pointer_up(Point::new(bar.track.right + 200.0, press.y), &mut sink);
        // The last column of the row is now visible with the padding gap.
        let rect = ui.rect(WidgetId::new("wide")).unwrap();
        let area = ui.scroll_view(panel).unwrap().content_rect();
        assert!((rect.right - area.right).abs() < 0.01);

        // The same for the vertical thumb.
        let bar = crate::scroll::bar(ui.scroll_view(panel).unwrap(), true).unwrap();
        let press = Point::new(bar.thumb.center().x, bar.thumb.center().y);
        assert!(ui.pointer_down(press, &mut sink));
        ui.pointer_move(Point::new(press.x, bar.track.bottom + 200.0), &mut sink);
        assert!((ui.scroll_offset(panel).unwrap().y - max.y).abs() < 0.01);
        ui.pointer_up(Point::new(press.x, bar.track.bottom + 200.0), &mut sink);
    }

    #[test]
    fn the_bar_junction_is_masked_and_does_not_press_the_content() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("full", "Full", 1u32));
        let mut sink = Sink::default();
        let panel = WidgetId::new("panel");
        ui.set_scroll_view(
            ScrollView::new(
                panel,
                Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                crate::widget::ScrollBars::Both,
            )
            .child(WidgetId::new("full"), Rect::from_xywh(0.0, 0.0, 300.0, 300.0)),
            &mut sink,
        );
        let corner = crate::scroll::corner_rect(ui.scroll_view(panel).unwrap()).unwrap();
        assert_eq!(corner, Rect::new(90.0, 90.0, 100.0, 100.0));
        // Painted with the track color, above the content.
        let canvas = paint_into_recorder(&mut ui, crate::Size::new(200.0, 200.0));
        assert!(canvas.fills.contains(&corner));
        // A press there is chrome: no widget under it, and the bar does not
        // jump either (the junction is dead space, like a native corner).
        ui.pointer_down(corner.center(), &mut sink);
        ui.pointer_up(corner.center(), &mut sink);
        assert!(ui.press_id().is_none());
        assert!(ui.drain_messages().is_empty());
        assert_eq!(ui.hover_cursor(), Cursor::Arrow);
    }

    #[test]
    fn removing_a_child_drops_it_from_the_panel() {
        let (mut ui, mut sink) = scroll_page();
        assert!(ui.remove(WidgetId::new("s1"), &mut sink));
        let view = ui.scroll_view(WidgetId::new("panel")).unwrap();
        assert!(!view.contains(WidgetId::new("s1")));
        assert_eq!(view.children_ref().len(), 2);
        // The panel still tracks the remaining children.
        assert_eq!(
            ui.scroll_owner_of(WidgetId::new("s2")),
            Some(WidgetId::new("panel"))
        );
        assert!(ui.remove_scroll_view(WidgetId::new("panel"), &mut sink));
        assert!(ui.scroll_views().next().is_none());
        assert_eq!(ui.scroll_owner_of(WidgetId::new("s2")), None);
    }

    #[test]
    fn rebuild_commits_the_editor_and_builds_a_fresh_tree_once() {
        let mut ui: Ui<String> = Ui::new();
        ui.add(TextEdit::new("field", "abc", |text| text).painted());
        ui.add(Button::new("old", "Old", "old".to_string()));
        ui.set_rect(WidgetId::new("field"), Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("field"), &mut sink);
        assert!(ui.char('x', &mut sink));
        let area = Rect::from_xywh(0.0, 0.0, 300.0, 200.0);
        let message = ui.rebuild(area, &mut sink, |ui| {
            ui.add(Button::new("only", "Only", "only".to_string()));
        });
        assert_eq!(message, Some("abcx".to_string()));
        assert_eq!(ui.len(), 1);
        assert_eq!(ui.ids().collect::<Vec<_>>(), vec![WidgetId::new("only")]);
        assert!(sink.take().contains(&area));
        assert!(sink.caret.is_none());
    }

    #[test]
    fn a_preserved_session_is_handed_to_a_reinserted_widget() {
        let id = WidgetId::new("field");
        let mut ui: Ui<String> = Ui::new();
        ui.add(TextEdit::new("field", "old", |text| text).painted());
        ui.set_rect(id, Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        let mut sink = Sink::default();
        // Open the painted session and type a draft.
        ui.set_focus(id, &mut sink);
        assert!(ui.key(key_event(KeyCode::Enter), &mut sink));
        assert!(ui.char('X', &mut sink));
        assert_eq!(
            ui.with_widget::<TextEdit<String>, _>(id, |field| (
                field.is_editing(),
                field.text().to_string()
            )),
            Some((true, "oldX".to_string()))
        );

        // Removing it with the session preserved commits nothing.
        assert!(ui.remove_preserving_session(id, &mut sink));
        assert!(ui.has_pending_session(id));
        assert!(ui.drain_messages().is_empty(), "the draft was not committed");

        // A widget re-inserted with the same id inherits the draft and the
        // open session; the stored snapshot is consumed.
        ui.add(TextEdit::new("field", "", |text| text).painted());
        ui.set_rect(id, Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        assert!(!ui.has_pending_session(id));
        assert_eq!(
            ui.with_widget::<TextEdit<String>, _>(id, |field| (
                field.is_editing(),
                field.text().to_string()
            )),
            Some((true, "oldX".to_string()))
        );

        // Escape after the restore reverts to the value the session opened with.
        ui.set_focus(id, &mut sink);
        assert!(ui.key(key_event(KeyCode::Escape), &mut sink));
        assert_eq!(
            ui.with_widget::<TextEdit<String>, _>(id, |field| (
                field.is_editing(),
                field.text().to_string()
            )),
            Some((false, "old".to_string()))
        );
    }

    #[test]
    fn a_cross_widget_drag_drops_the_payload_on_the_target() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("source", "S", "source"));
        ui.add(Button::new("target", "T", "target"));
        ui.set_rect(WidgetId::new("source"), Rect::from_xywh(0.0, 0.0, 100.0, 40.0));
        ui.set_rect(WidgetId::new("target"), Rect::from_xywh(0.0, 50.0, 100.0, 40.0));
        let mut sink = Sink::default();

        ui.set_drag_source(WidgetId::new("source"), || Some(DragData::text("payload")));
        ui.set_drop_target(WidgetId::new("target"), |payload, pos| {
            (payload.as_text() == Some("payload") && pos.y >= 50.0).then_some("dropped")
        });
        let hovers = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let log = hovers.clone();
        ui.set_drop_hover(WidgetId::new("target"), move |payload, _pos, over| {
            log.borrow_mut().push((payload.as_text().unwrap_or("").to_string(), over));
            true
        });

        // Press the source, cross the threshold (the drag starts), move over
        // the target, and release there.
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_move(Point::new(60.0, 10.0), &mut sink);
        assert!(ui.is_dragging());
        assert_eq!(ui.drag_data(), Some(&DragData::text("payload")));
        assert_eq!(ui.drag_source(), Some(WidgetId::new("source")));
        assert_eq!(ui.drag_target(), None);
        sink.take();
        ui.pointer_move(Point::new(20.0, 60.0), &mut sink);
        assert_eq!(ui.drag_target(), Some(WidgetId::new("target")));
        assert_eq!(hovers.borrow().as_slice(), &[("payload".to_string(), true)]);
        assert!(!sink.take().is_empty(), "the hover repainted the target");

        assert!(ui.pointer_up(Point::new(20.0, 60.0), &mut sink));
        assert_eq!(ui.drain_messages(), vec!["dropped"]);
        assert!(!ui.is_dragging());
        assert_eq!(ui.drag_data(), None);
        // The release leaves the target: the hover callback saw the leave.
        assert_eq!(
            hovers.borrow().as_slice(),
            &[("payload".to_string(), true), ("payload".to_string(), false)]
        );
    }

    #[test]
    fn a_drag_cancels_on_escape_and_a_sourceless_widget_keeps_drag_start() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("source", "S", "source"));
        ui.add(Button::new("target", "T", "target"));
        ui.set_rect(WidgetId::new("source"), Rect::from_xywh(0.0, 0.0, 100.0, 40.0));
        ui.set_rect(WidgetId::new("target"), Rect::from_xywh(0.0, 50.0, 100.0, 40.0));
        let mut sink = Sink::default();

        // No drag source: the ordinary DragStart path still fires.
        let fired = std::rc::Rc::new(std::cell::Cell::new(false));
        let flag = fired.clone();
        ui.bind(WidgetId::new("source"), Trigger::DragStart, move |_event, _ctx| {
            flag.set(true);
            None
        });
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_move(Point::new(60.0, 10.0), &mut sink);
        assert!(fired.get());
        assert!(!ui.is_dragging());
        ui.pointer_up(Point::new(60.0, 10.0), &mut sink);

        // With a source, Escape aborts the drag without a drop or a click.
        ui.set_drag_source(WidgetId::new("source"), || Some(DragData::text("payload")));
        ui.pointer_down(Point::new(20.0, 10.0), &mut sink);
        ui.pointer_move(Point::new(60.0, 10.0), &mut sink);
        assert!(ui.is_dragging());
        assert!(ui.key(key_event(KeyCode::Escape), &mut sink));
        assert!(!ui.is_dragging());
        ui.pointer_up(Point::new(60.0, 10.0), &mut sink);
        assert!(ui.drain_messages().is_empty());
    }

    #[test]
    fn a_system_drag_routes_text_and_files_to_registered_targets() {
        let mut ui: Ui<String> = Ui::new();
        let target = WidgetId::new("target");
        ui.add(Button::new("target", "T", "dropped".to_string()));
        ui.set_rect(target, Rect::from_xywh(0.0, 0.0, 120.0, 40.0));
        ui.set_drop_target(target, |data, pos| match data {
            DragData::Text(text) => Some(format!("{text}@{}", pos.y as i32)),
            DragData::Files(paths) => Some(format!("{} files", paths.len())),
        });
        let mut sink = Sink::default();

        // A drag from another application: enter over the target accepts and
        // shows its hover, moving within the same widget repaints nothing.
        let text = DragData::text("外部文字");
        assert_eq!(
            ui.drag_enter(&text, Point::new(10.0, 10.0), &mut sink),
            DropEffect::Copy
        );
        assert_eq!(ui.drag_text(), Some("外部文字"));
        assert_eq!(ui.drag_source(), None);
        assert_eq!(ui.drag_target(), Some(target));
        sink.take();
        ui.drag_over(&text, Point::new(20.0, 20.0), &mut sink);
        assert!(sink.take().is_empty(), "a target change is what repaints");

        // Releasing runs the handler and clears the session.
        assert_eq!(
            ui.drag_drop(text, Point::new(20.0, 20.0), &mut sink),
            DropEffect::Copy
        );
        assert_eq!(ui.drain_messages(), vec!["外部文字@20".to_string()]);
        assert!(!ui.is_dragging());
        assert_eq!(ui.drag_data(), None);

        // Outside every target the effect is None, and a leave clears the
        // hover without a drop.
        assert_eq!(
            ui.drag_enter(&DragData::text("x"), Point::new(200.0, 200.0), &mut sink),
            DropEffect::None
        );
        ui.drag_leave(&mut sink);
        assert!(!ui.is_dragging());

        // A file list reaches the same generic target.
        assert_eq!(
            ui.drag_enter(
                &DragData::files(["C:\\a.mp3", "C:\\b.mp3"]),
                Point::new(10.0, 10.0),
                &mut sink
            ),
            DropEffect::Copy
        );
        assert_eq!(
            ui.drag_drop(
                DragData::files(["C:\\a.mp3", "C:\\b.mp3"]),
                Point::new(10.0, 10.0),
                &mut sink
            ),
            DropEffect::Copy
        );
        assert_eq!(ui.drain_messages(), vec!["2 files".to_string()]);
    }

    #[test]
    fn a_kind_filtered_target_refuses_the_other_payload() {
        let target = WidgetId::new("target");
        let mut ui: Ui<String> = Ui::new();
        ui.add(Button::new("target", "T", "text".to_string()));
        ui.set_rect(target, Rect::from_xywh(0.0, 0.0, 120.0, 40.0));
        ui.set_text_drop_target(target, |text, _pos| Some(text));
        let mut sink = Sink::default();
        let files = DragData::files(["C:\\a.mp3"]);
        assert_eq!(
            ui.drag_enter(&files, Point::new(10.0, 10.0), &mut sink),
            DropEffect::None,
            "a text-only target shows no-drop for files"
        );
        ui.drag_leave(&mut sink);
        assert_eq!(
            ui.drag_drop(files, Point::new(10.0, 10.0), &mut sink),
            DropEffect::None
        );
        assert!(ui.drain_messages().is_empty());
        // The matching kind is accepted.
        assert_eq!(
            ui.drag_enter(&DragData::text("ok"), Point::new(10.0, 10.0), &mut sink),
            DropEffect::Copy
        );
    }

    #[test]
    fn a_system_text_drop_inserts_into_a_registered_painted_field() {
        let field = WidgetId::new("field");
        let mut ui: Ui<u32> = Ui::new();
        ui.add(TextEdit::new("field", "abc", |_| 1u32).painted());
        ui.set_rect(field, Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        paint_page(&mut ui);
        ui.set_drop_target(field, |data, _pos| data.as_text().map(|_| 2u32));
        let mut sink = Sink::default();
        let text = DragData::text("XY");
        assert_eq!(
            ui.drag_enter(&text, Point::new(10.0, 17.0), &mut sink),
            DropEffect::Copy
        );
        assert_eq!(
            ui.drag_drop(text, Point::new(150.0, 17.0), &mut sink),
            DropEffect::Copy
        );
        let value = ui
            .with_widget::<TextEdit<u32>, _>(field, |edit| edit.text().to_string())
            .unwrap();
        assert!(value.starts_with("abc"), "inserted, not replaced: {value}");
        assert!(value.contains("XY"), "the dropped text landed: {value}");
        assert_eq!(ui.drain_messages(), vec![2u32]);
    }

    #[test]
    fn programmatic_content_changes_are_silent_and_repaint_once() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(
            TextEdit::new("field", "", |_text| 1u32)
                .painted()
                .on_change(|_text| Some(2u32)),
        );
        ui.add(NumberInput::new("number", 0, 0..=100, |_value| 3u32));
        ui.add(Slider::new("slider", 0.0, 0.0..=1.0, |_value| 4u32));
        ui.set_rect(WidgetId::new("field"), Rect::from_xywh(10.0, 10.0, 200.0, 34.0));
        ui.set_rect(WidgetId::new("number"), Rect::from_xywh(10.0, 60.0, 120.0, 34.0));
        ui.set_rect(WidgetId::new("slider"), Rect::from_xywh(10.0, 110.0, 240.0, 32.0));

        // Content set through code never reports a change event.
        ui.set_text(WidgetId::new("field"), "hello");
        ui.set_number(WidgetId::new("number"), 42);
        ui.set_value(WidgetId::new("slider"), 0.5);
        assert!(
            ui.drain_messages().is_empty(),
            "code changes must stay silent"
        );

        // The three changes are pending, and one flush hands them to the sink
        // as three small pieces — distant widgets never share a bounding box.
        assert!(ui.has_damage());
        let mut sink = Sink::default();
        assert!(ui.flush_damage(&mut sink));
        let rects = sink.take();
        assert_eq!(rects.len(), 3, "one piece per changed widget");
        let covers = |point: Point| rects.iter().any(|rect| rect.contains_point(point));
        assert!(covers(Point::new(20.0, 20.0)));
        assert!(covers(Point::new(30.0, 70.0)));
        assert!(covers(Point::new(30.0, 120.0)));
        assert!(!ui.flush_damage(&mut sink));
        assert!(sink.take().is_empty());
        assert!(!ui.has_damage());
    }

    #[test]
    fn widget_setters_through_typed_access_coalesce_into_one_flush() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Slider::new("slider", 0.5, 0.0..=1.0, |value| value.to_bits()));
        ui.add(Pager::new("pager", 0, 5, |delta| delta as u32));
        ui.set_rect(
            WidgetId::new("slider"),
            Rect::from_xywh(10.0, 10.0, 200.0, 32.0),
        );
        ui.set_rect(
            WidgetId::new("pager"),
            Rect::from_xywh(10.0, 60.0, 200.0, 32.0),
        );
        let mut sink = Sink::default();

        // The widgets' own setters are display changes like the `Ui::set_*`
        // ones: they record their rectangle instead of invalidating per call.
        assert!(ui.edit_widget::<Slider<u32>>(WidgetId::new("slider"), |slider| {
            slider.set_range(0.0, 2.0)
        }));
        assert!(ui.edit_widget::<Pager<u32>>(WidgetId::new("pager"), |pager| {
            pager.set_pages(3)
        }));
        assert!(
            sink.take().is_empty(),
            "typed access records damage; it does not repaint per setter"
        );
        assert!(ui.flush_damage(&mut sink));
        let rects = sink.take();
        assert_eq!(rects.len(), 2, "both widgets repaint, one piece each");
        let covers = |point: Point| rects.iter().any(|rect| rect.contains_point(point));
        assert!(covers(Point::new(20.0, 20.0)));
        assert!(covers(Point::new(20.0, 70.0)));

        // A closure that reports no change records nothing.
        assert!(!ui.edit_widget::<Pager<u32>>(WidgetId::new("pager"), |pager| {
            pager.set_pages(3)
        }));
        assert!(!ui.has_damage());
        assert!(sink.take().is_empty());
    }

    #[test]
    fn props_and_enabled_changes_wait_for_the_flush() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("button", "按钮", 1u32));
        ui.set_rect(
            WidgetId::new("button"),
            Rect::from_xywh(0.0, 0.0, 160.0, 36.0),
        );
        let mut sink = Sink::default();

        // Palette/name props and the disabled look are display changes; the
        // batched flush repaints the widget once, not once per property.
        ui.set_props(
            WidgetId::new("button"),
            WidgetProps::new().name("主按钮"),
            &mut sink,
        );
        ui.set_enabled(WidgetId::new("button"), false, &mut sink);
        assert!(
            sink.take().is_empty(),
            "props/enabled record damage instead of invalidating"
        );
        assert!(ui.flush_damage(&mut sink));
        assert_eq!(
            sink.take(),
            vec![Rect::from_xywh(0.0, 0.0, 160.0, 36.0)],
            "both changes share one rect"
        );
    }

    #[test]
    fn mark_dirty_schedules_a_repaint_for_the_next_flush() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("button", "按钮", 1u32));
        let area = Rect::from_xywh(10.0, 10.0, 120.0, 30.0);
        ui.set_rect(WidgetId::new("button"), area);
        let mut sink = Sink::default();

        // An application-owned display change can join the code-driven batch.
        assert!(ui.mark_dirty(WidgetId::new("button")));
        assert!(ui.has_damage());
        assert!(sink.take().is_empty());
        assert!(ui.flush_damage(&mut sink));
        assert_eq!(sink.take(), vec![area]);
        // Unknown ids and hidden widgets record nothing.
        assert!(!ui.mark_dirty(WidgetId::new("missing")));
        ui.set_visible(WidgetId::new("button"), false, &mut sink);
        sink.take();
        assert!(!ui.mark_dirty(WidgetId::new("button")));
        assert!(!ui.flush_damage(&mut sink));
    }

    #[test]
    fn a_new_toast_message_is_marked_and_flushed() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Toast::new("toast"));
        let area = Rect::from_xywh(20.0, 20.0, 320.0, 44.0);
        ui.set_rect(WidgetId::new("toast"), area);
        let mut sink = Sink::default();

        // The message setter shows a hidden toast; the paint comes from the
        // same pending-damage batch as every other code change.
        ui.set_message(WidgetId::new("toast"), "已保存", false);
        assert!(sink.take().is_empty());
        assert!(ui.flush_damage(&mut sink));
        assert_eq!(sink.take(), vec![area]);
    }

    #[test]
    fn user_edits_report_the_content_change() {
        let mut ui: Ui<String> = Ui::new();
        ui.add(
            TextEdit::new("field", "", |text| format!("commit:{text}"))
                .painted()
                .on_change(|text| Some(format!("change:{text}"))),
        );
        ui.set_rect(WidgetId::new("field"), Rect::from_xywh(0.0, 0.0, 240.0, 34.0));
        let mut sink = Sink::default();
        ui.set_focus(WidgetId::new("field"), &mut sink);
        ui.key(key_event(KeyCode::Enter), &mut sink);
        assert!(ui.drain_messages().is_empty(), "opening is not a change");

        // Typing, deleting, cutting and pasting are user content changes.
        ui.char('a', &mut sink);
        assert_eq!(ui.drain_messages(), vec!["change:a".to_string()]);
        ui.char('b', &mut sink);
        assert_eq!(ui.drain_messages(), vec!["change:ab".to_string()]);
        ui.key(key_event(KeyCode::Backspace), &mut sink);
        assert_eq!(ui.drain_messages(), vec!["change:a".to_string()]);
        ui.key(key_event_mod(KeyCode::Letter(0), true), &mut sink); // Ctrl+A
        assert!(ui.drain_messages().is_empty(), "selecting is not a change");
        ui.key(key_event_mod(KeyCode::Letter(23), true), &mut sink); // Ctrl+X
        assert_eq!(ui.drain_messages(), vec!["change:".to_string()]);
        assert_eq!(sink.copied.last().map(String::as_str), Some("a"));
        sink.clipboard = Some("XYZ".to_string());
        ui.key(key_event_mod(KeyCode::Letter(21), true), &mut sink); // Ctrl+V
        assert_eq!(ui.drain_messages(), vec!["change:XYZ".to_string()]);
    }

    #[test]
    fn the_time_picker_popup_scrolls_with_the_wheel() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(TimePicker::new("time", Time::new(9, 30), |time| time.hour));
        ui.set_rect(WidgetId::new("time"), Rect::from_xywh(20.0, 20.0, 110.0, 34.0));
        let mut sink = Sink::default();
        let value = |ui: &Ui<u32>| {
            ui.with_widget::<TimePicker<u32>, _>(WidgetId::new("time"), |picker| {
                (picker.is_open(), picker.value())
            })
            .unwrap()
        };
        // Clicking the field opens the popup, which drops below it.
        ui.pointer_down(Point::new(60.0, 30.0), &mut sink);
        ui.pointer_up(Point::new(60.0, 30.0), &mut sink);
        assert_eq!(value(&ui), (true, Time::new(9, 30)));

        // A notch over the minute column: the hovered unit follows the value.
        let popup = Point::new(130.0, 150.0);
        ui.pointer_move(popup, &mut sink);
        ui.wheel(popup, -1.0, &mut sink);
        assert_eq!(value(&ui), (true, Time::new(9, 31)));
        ui.wheel(popup, 1.0, &mut sink);
        assert_eq!(value(&ui), (true, Time::new(9, 30)));
    }

    #[test]
    fn a_backdrop_blocks_the_wheel_from_scrolling_the_page() {
        let mut ui: Ui<u32> = Ui::new();
        ui.add(Button::new("row_a", "A", 1u32));
        ui.add(Button::new("row_b", "B", 2u32));
        let mut sink = Sink::default();
        ui.set_scroll_view(
            ScrollView::new(
                WidgetId::new("panel"),
                Rect::from_xywh(0.0, 0.0, 200.0, 100.0),
                crate::widget::ScrollBars::Vertical,
            )
            .child(WidgetId::new("row_a"), Rect::from_xywh(0.0, 0.0, 200.0, 30.0))
            .child(WidgetId::new("row_b"), Rect::from_xywh(0.0, 300.0, 200.0, 30.0)),
            &mut sink,
        );
        // Without a backdrop the wheel scrolls the page.
        assert!(ui.wheel(Point::new(50.0, 50.0), -1.0, &mut sink));
        let scrolled = ui.scroll_offset(WidgetId::new("panel")).unwrap().y;
        assert!(scrolled > 0.0, "the page scrolled: {scrolled}");

        // A visible scrim swallows the wheel; the page stays put.
        ui.add_with(
            Scrim::new("scrim"),
            crate::props::WidgetProps::new().visible(false),
        );
        ui.set_visible(WidgetId::new("scrim"), true, &mut sink);
        ui.set_rect(WidgetId::new("scrim"), Rect::from_xywh(0.0, 0.0, 200.0, 100.0));
        assert!(ui.wheel(Point::new(50.0, 50.0), -1.0, &mut sink));
        assert_eq!(
            ui.scroll_offset(WidgetId::new("panel")).unwrap().y,
            scrolled,
            "the page under the modal must not scroll"
        );
    }
}
