// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Text and number fields.
//!
//! Both widgets paint their own frame and text, so a page looks complete while
//! the field is not being edited. Clicking or pressing Enter asks the platform
//! host for a native `EDIT` session — see [`crate::widget::EditRequest`] and
//! [`crate::UiHost`]. Committed text comes back through
//! [`crate::Ui::commit_edit`], which updates the widget and turns the value
//! into an application message.

use std::cell::{Cell, RefCell};

use crate::a11y::{A11yInfo, A11yRole};
use crate::backend::{TextAlign, TextStyle, TextOverflow};
use crate::event::{Cursor, KeyCode, KeyEvent};
use crate::geometry::{Insets, Point, Rect};
use crate::menu::{Menu, MenuEntry, MenuItem};
use crate::widget::{
    EditRequest, ScrollBars, SessionSnapshot, TextStyleRole, Widget, WidgetId, WidgetInput,
    WidgetPaint, WidgetState, edit_command,
};
use crate::widgets::editor::{self, EditColors, LineMetrics, ResolvedEditColors};
use crate::widgets::inline::PaintedField;
use crate::widgets::scrollbar::{self, BarHit, ScrollBarState, ScrollBarStyle};
use crate::skin::{SkinAttrDecl, SkinValue, SkinValueKind};
use crate::surface::{SurfaceStyle, surface_builders};

static EDIT_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("cue", SkinValueKind::Text),
    SkinAttrDecl::new("color", SkinValueKind::Color),
];

static NUMBER_SKIN_ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("prefix", SkinValueKind::Text),
    SkinAttrDecl::new("suffix", SkinValueKind::Text),
];

/// Enough bullets for any reasonable password; longer values are clipped.
const MASK: &str = "••••••••••••••••••••••••••••••••";

fn field_colors(
    pal: &crate::theme::Palette,
    enabled: bool,
    focused: bool,
) -> (crate::Color, crate::Color) {
    let border = if !enabled {
        pal.divider
    } else if focused {
        pal.accent
    } else {
        pal.explicit_border
    };
    let background = if enabled {
        pal.search_bg
    } else {
        pal.button_disabled
    };
    (border, background)
}

/// Cached line layout for the closed/painted paint. Recomputed only when the
/// width, font, wrapping or text changes, so a caret blink or hover repaint
/// allocates nothing.
#[derive(Default)]
struct TextLayout {
    stale: bool,
    width: f32,
    font_size: f32,
    wrap: bool,
    line_height: f32,
    text_rev: u64,
    lines: Vec<LineMetrics>,
    max_width: f32,
    visible_lines: usize,
}

/// One undo/redo step.
#[derive(Clone, Debug)]
struct EditSnapshot {
    text: String,
    caret: usize,
    anchor: Option<usize>,
}

/// Which end of the selection a touch is dragging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionEdge {
    Start,
    End,
}

/// Radius of a painted selection handle (DIPs).
const HANDLE_RADIUS: f32 = 6.0;
/// How far (DIPs) a touch may land from a handle's centre and still grab it.
const HANDLE_HIT_RADIUS: f32 = 20.0;
/// Space between the text line and a handle's centre stem end (DIPs).
const HANDLE_GAP: f32 = 4.0;

/// Consecutive mutations of the same kind coalesce into one undo step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum EditRun {
    #[default]
    None,
    Insert,
    DeleteBack,
    DeleteForward,
}

pub struct TextEdit<M> {
    id: WidgetId,
    rect: Rect,
    text: String,
    /// Caret position in characters; the closed field keeps its own caret so
    /// programmatic select-all/insert-replace and UIA TextPattern work without
    /// an open native editor.
    caret: usize,
    /// Selection anchor in characters. `None` means the selection is collapsed
    /// at `caret` (no selection).
    anchor: Option<usize>,
    /// Pre-computed paint text for password fields (allocated off the paint
    /// path; the paint path only borrows it).
    masked: String,
    /// Bumped on every text/mask change; keys the layout cache.
    text_rev: u64,
    cue: String,
    password: bool,
    multiline: bool,
    /// Word-wrap long lines (multiline only).
    wrap: bool,
    /// Which scrollbars the closed paint and the native editor get.
    scrollbars: ScrollBars,
    /// Vertical scroll offset in DIPs (wheel, thumb drag, caret reveal).
    /// Cells because the caret reveal runs inside `paint(&self)` and must
    /// survive to the next paint (a blink or a rejected keystroke otherwise
    /// redraws from the top). Fractional on purpose: a notch or a thumb drag
    /// scrolls smoothly instead of snapping to a whole line.
    scroll_y: std::cell::Cell<f32>,
    /// Horizontal scroll in DIPs for no-wrap and single-line fields.
    scroll_x: std::cell::Cell<f32>,
    /// Cached line layout for the closed/painted paint.
    layout: RefCell<TextLayout>,
    align: TextAlign,
    /// Per-widget text size (theme-scaled DIPs); `None` uses the theme body.
    font_size: Option<f32>,
    /// Content padding; defaults to `(9, 2)` single-line and `(9, 6)`
    /// multiline.
    padding: Option<Insets>,
    /// Extra space between lines in DIPs (default 2.0).
    line_spacing: Option<f32>,
    /// Single-line vertical centering; defaults on for single-line fields.
    vertical_center: Option<bool>,
    /// Framework-painted editing instead of the native child.
    painted: bool,
    /// Paint the field's own frame/background. Off for a painted session the
    /// composite widget draws itself (a search box, a number input with its
    /// prefix/suffix and stepper chrome).
    chrome: bool,
    /// An inline editing session is open.
    editing: bool,
    /// Value/selection when the session opened, for Escape.
    previous: Option<EditSnapshot>,
    undo: Vec<EditSnapshot>,
    redo: Vec<EditSnapshot>,
    last_run: EditRun,
    /// Preferred x while moving vertically.
    goal_x: Option<f32>,
    /// Inline IME composition and its cursor, in characters.
    composition: String,
    composition_cursor: usize,
    /// Pointer selection is in progress.
    selecting: bool,
    /// The touch selection model is active: the selection paints draggable
    /// handles. A long press turns it on (D121); a mouse press turns it off.
    touch_handles: bool,
    /// The selection edge a touch is currently dragging.
    handle_drag: Option<SelectionEdge>,
    /// The press landed inside the selection and kept it; a click (release
    /// without a drag) collapses it to the click point.
    press_kept: bool,
    /// Shared scrollbar state/style (hover, active, grab) for both bars.
    vbar: ScrollBarState,
    hbar: ScrollBarState,
    bar_style: ScrollBarStyle,
    /// Text copied by Ctrl+C/X, written to the clipboard by the container.
    clipboard: Option<String>,
    /// Caret rectangle from the last paint (IME placement, blink).
    caret_rect: Cell<Option<Rect>>,
    /// Caret bar width override; `None` follows the system accessibility
    /// setting (via the painter).
    caret_width: Option<f32>,
    /// A caret move/edit asked the paint path to scroll the caret into view.
    reveal: Cell<bool>,
    /// Editor colours; unset entries come from the theme.
    colors: EditColors,
    max_length: u32,
    enabled: bool,
    /// Container-level lock: the field still opens a session for caret
    /// movement, selection and copy, but every input that would mutate the
    /// buffer (typing, paste, cut, delete, IME, undo) is refused.
    read_only: bool,
    /// Application observers, run before the built-in behavior (which still
    /// runs). Each returns the message the application should handle; `None`
    /// stays silent. At most one message per input is delivered: an earlier
    /// observer wins over a later one, and either wins over `on_change`.
    on_change: Option<Box<dyn Fn(&str) -> Option<M>>>,
    on_char: Option<Box<dyn Fn(char) -> Option<M>>>,
    on_click: Option<Box<dyn Fn(Point) -> Option<M>>>,
    on_right_click: Option<Box<dyn Fn(Point) -> Option<M>>>,
    on_double_click: Option<Box<dyn Fn(Point) -> Option<M>>>,
    on_key_down: Option<Box<dyn Fn(KeyEvent) -> Option<M>>>,
    on_key_up: Option<Box<dyn Fn(KeyEvent) -> Option<M>>>,
    /// A user edit changed the text since the last report (input, paste, IME,
    /// cut, delete, undo). The container asks for the `on_change` message
    /// through [`Widget::take_change_message`] after a direct insertion.
    change_pending: Cell<bool>,
    /// Commit handler; `None` for a field the composite widgets drive
    /// themselves (they read [`TextEdit::text`] when the session closes).
    on_commit: Option<Box<dyn Fn(String) -> M>>,
    /// Background surface (corner radius / fill override / transparency).
    surface: SurfaceStyle,
}

impl<M> TextEdit<M> {
    pub fn new(id: &'static str, text: impl Into<String>, on_commit: impl Fn(String) -> M + 'static) -> Self {
        Self::with_commit(id, text.into(), Some(Box::new(on_commit)))
    }

    /// The editing field the composite widgets (`Table`, `TreeView`,
    /// `PropertyGrid`) embed instead of the old one-line draft: a painted
    /// field with no commit handler, no scrollbar chrome and a padding the
    /// composite sets per session. The composite owns the session lifetime and
    /// reads [`TextEdit::text`] when it applies the value.
    pub(crate) fn inline_field(id: &'static str) -> Self {
        let mut field = Self::with_commit(id, String::new(), None);
        field.painted = true;
        field.scrollbars = ScrollBars::None;
        field
    }

    fn with_commit(
        id: &'static str,
        text: String,
        on_commit: Option<Box<dyn Fn(String) -> M>>,
    ) -> Self {
        let text = crate::text::normalize_newlines(&text).into_owned();
        let caret = text.chars().count();
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 280.0, 34.0),
            text,
            caret,
            anchor: None,
            masked: String::new(),
            text_rev: 1,
            cue: String::new(),
            password: false,
            multiline: false,
            wrap: true,
            scrollbars: ScrollBars::Vertical,
            scroll_y: std::cell::Cell::new(0.0),
            scroll_x: std::cell::Cell::new(0.0),
            layout: RefCell::new(TextLayout::default()),
            align: TextAlign::Left,
            font_size: None,
            padding: None,
            line_spacing: None,
            vertical_center: None,
            painted: false,
            chrome: true,
            editing: false,
            previous: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_run: EditRun::None,
            goal_x: None,
            composition: String::new(),
            composition_cursor: 0,
            selecting: false,
            touch_handles: false,
            handle_drag: None,
            press_kept: false,
            vbar: ScrollBarState::default(),
            hbar: ScrollBarState::default(),
            bar_style: ScrollBarStyle {
                // A slim field bar: 7 DIP track, a 5 DIP pill thumb.
                thickness: 7.0,
                inset: 0.0,
                thumb_inset: 1.0,
                min_thumb: 12.0,
                page_on_track_click: false,
                colors: None,
            },
            clipboard: None,
            caret_rect: Cell::new(None),
            caret_width: None,
            reveal: Cell::new(false),
            colors: EditColors::default(),
            max_length: 0,
            enabled: true,
            read_only: false,
            on_change: None,
            on_char: None,
            on_click: None,
            on_right_click: None,
            on_double_click: None,
            on_key_down: None,
            on_key_up: None,
            change_pending: Cell::new(false),
            on_commit,
            surface: SurfaceStyle::new(),
        }
    }

    /// Horizontal alignment of the painted text and the native editor.
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Per-field text size in DIPs, scaled by the theme like the body font.
    /// Applies to the closed paint, the caret/selection and the native editor.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size.max(1.0));
        self
    }

    pub fn cue(mut self, cue: impl Into<String>) -> Self {
        self.cue = cue.into();
        self
    }

    pub fn cue_text(&self) -> &str {
        &self.cue
    }

    /// Replace the cue (placeholder) silently (the skin's `cue` attribute).
    pub fn set_cue(&mut self, cue: impl Into<String>) {
        self.cue = cue.into();
    }

    pub fn password(mut self) -> Self {
        self.password = true;
        self.refresh_mask();
        self
    }

    /// Multiline fields commit on focus loss instead of Enter. Word wrap and
    /// a vertical scrollbar are the defaults.
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }

    /// Word-wrap long lines (default on for multiline fields). With wrap off,
    /// hard newlines are the only line breaks and the field scrolls
    /// horizontally.
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Native/closed scrollbars for a multiline field (default vertical).
    pub fn scrollbars(mut self, bars: ScrollBars) -> Self {
        self.scrollbars = bars;
        self
    }

    /// Scrollbar thickness/insets/minimum thumb and hover/active colors
    /// (default: a 7 DIP track with a 12 DIP minimum pill thumb).
    pub fn scrollbar_style(mut self, style: ScrollBarStyle) -> Self {
        self.bar_style = style;
        self
    }

    pub fn scrollbar(&self) -> &ScrollBarStyle {
        &self.bar_style
    }

    /// Edit in place with framework painting instead of hosting a native
    /// `EDIT` child. The field owns typing, selection, IME composition,
    /// clipboard, undo and its context menu while it is focused.
    pub fn painted(mut self) -> Self {
        self.painted = true;
        self
    }

    /// Whether [`TextEdit::painted`] is on.
    pub fn is_painted(&self) -> bool {
        self.painted
    }

    // --- inline field seam -------------------------------------------------
    //
    // The composite widgets (`Table`, `TreeView`, `PropertyGrid`) own a
    // painted field and drive it through these crate-internal calls, which
    // avoid the `Widget<M>` trait so a composite method needs no `M: 'static`
    // bound. The logic underneath is the same code the standalone painted
    // editor runs: one buffer, one layout cache, one caret/selection model,
    // undo/redo, IME composition and the built-in edit menu.

    /// Configure a composite session: newline-containing values edit with the
    /// painted multiline model (soft wrap, per-line layout, caret reveal), a
    /// value without one keeps a single visual line.
    pub(crate) fn configure_inline(&mut self, multiline: bool, padding: Insets, align: TextAlign) {
        self.multiline = multiline;
        self.padding = Some(padding);
        self.wrap = true;
        self.line_spacing = Some(1.0);
        self.vertical_center = Some(!multiline);
        self.align = align;
    }

    /// Whether this session paints the field's own frame/background.
    pub(crate) fn inline_set_chrome(&mut self, chrome: bool) {
        self.chrome = chrome;
    }

    /// Per-session `max_length` (0 = unlimited).
    pub(crate) fn inline_set_max_length(&mut self, max: u32) {
        self.max_length = max;
    }

    pub(crate) fn inline_is_multiline(&self) -> bool {
        self.multiline
    }

    pub(crate) fn inline_text(&self) -> &str {
        &self.text
    }

    pub(crate) fn inline_rect(&self) -> Rect {
        self.rect
    }

    pub(crate) fn inline_set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    /// Open (or re-position) the session; `position` places the caret from a
    /// pointer press, `None` keeps the caret where it is.
    pub(crate) fn inline_begin(&mut self, position: Option<Point>) -> bool {
        self.begin_edit_impl(position)
    }

    /// Close the session. `false` restores the value as it was when opened.
    pub(crate) fn inline_finish(&mut self, commit: bool) {
        self.finish_edit_impl(commit);
    }

    /// Replace the value and put the caret at its end (the numeric stepper
    /// path). Closes any selection.
    pub(crate) fn inline_set_text(&mut self, text: String) {
        self.text = crate::text::normalize_newlines(&text).into_owned();
        self.caret = self.char_len();
        self.scroll_y.set(0.0);
        self.scroll_x.set(0.0);
        self.anchor = None;
        self.goal_x = None;
        self.composition.clear();
        self.composition_cursor = 0;
        self.selecting = false;
        self.touch_text();
    }

    pub(crate) fn inline_select_all(&mut self) {
        self.select_all();
    }

    pub(crate) fn inline_insert(&mut self, text: &str) -> bool {
        if self.read_only {
            return false;
        }
        self.insert_impl(text, true)
    }

    pub(crate) fn inline_set_composition(&mut self, text: &str, cursor: usize) -> bool {
        if self.read_only {
            return false;
        }
        let text = crate::text::normalize_newlines(text).into_owned();
        let cursor = cursor.min(text.chars().count());
        if self.composition == text && self.composition_cursor == cursor {
            return false;
        }
        self.composition = text;
        self.composition_cursor = cursor;
        self.reveal.set(true);
        true
    }

    pub(crate) fn inline_caret_rect(&self) -> Option<Rect> {
        self.caret_rect.get()
    }

    pub(crate) fn inline_take_clipboard(&mut self) -> Option<String> {
        self.clipboard.take()
    }

    pub(crate) fn inline_menu(&self) -> Option<Menu> {
        self.edit_menu_impl()
    }

    pub(crate) fn inline_command(&mut self, command: u32) -> bool {
        self.edit_command_impl(command)
    }

    pub(crate) fn inline_paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        self.paint_painted(ctx, state);
    }

    /// Session input from the composite: pointer drag/double-click/selection
    /// motion, the wheel and keys. Enter is left to the composite for a
    /// single-line value (the container commits it); a multiline value inserts
    /// the newline here. Returns whether the input was consumed.
    pub(crate) fn inline_input(&mut self, input: WidgetInput) -> bool {
        if !self.enabled {
            return false;
        }
        match input {
            WidgetInput::PressAt(pos) => {
                self.handle_bar_press(pos);
                true
            }
            WidgetInput::DragTo(pos) => {
                if self.selecting {
                    self.auto_scroll_drag(pos);
                    let caret = self.caret_at(pos);
                    if caret != self.caret {
                        let anchor = self.anchor.unwrap_or(self.caret);
                        self.anchor = Some(anchor);
                        self.caret = caret;
                        self.goal_x = None;
                        self.reveal.set(false);
                    }
                }
                true
            }
            WidgetInput::ReleaseAt(_) | WidgetInput::DragEnd(_) => {
                self.vbar.active = false;
                self.hbar.active = false;
                self.selecting = false;
                self.last_run = EditRun::None;
                true
            }
            WidgetInput::RightClickAt(_) => {
                self.selecting = false;
                true
            }
            WidgetInput::DoubleClickAt(pos) => {
                self.select_word_at(pos);
                true
            }
            WidgetInput::Wheel(delta) => {
                self.scroll_wheel(delta);
                true
            }
            WidgetInput::Key(key) => {
                if key.code == KeyCode::Enter && !self.multiline {
                    return false;
                }
                self.handle_key(key);
                true
            }
            _ => false,
        }
    }

    /// Colours of a painted field (ordinary text/background, selection
    /// text/background, caret, cue, borders, composition underline).
    pub fn colors(mut self, colors: EditColors) -> Self {
        self.colors = colors;
        self
    }

    pub fn text_color(mut self, color: crate::Color) -> Self {
        self.colors.text = Some(color);
        self
    }

    /// Replace the field text colour silently (the skin's `color`); `None`
    /// restores the palette's text colour.
    pub fn set_text_color(&mut self, color: Option<crate::Color>) {
        self.colors.text = color;
    }

    /// The explicit text colour, when one is set.
    pub fn text_color_value(&self) -> Option<crate::Color> {
        self.colors.text
    }

    pub fn edit_background(mut self, color: crate::Color) -> Self {
        self.colors.background = Some(color);
        self
    }

    pub fn selection_colors(mut self, text: crate::Color, background: crate::Color) -> Self {
        self.colors = self.colors.selection(text, background);
        self
    }

    pub fn caret_color(mut self, color: crate::Color) -> Self {
        self.colors.caret = Some(color);
        self
    }

    /// Caret bar width in DIPs. The default follows the system accessibility
    /// caret width; this overrides it for one field.
    pub fn caret_width(mut self, width: f32) -> Self {
        self.caret_width = Some(width.clamp(0.5, 6.0));
        self
    }

    /// Content padding (default `(9, 2)` single-line, `(9, 6)` multiline).
    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    /// Extra space between lines in DIPs (default 2.0).
    pub fn line_spacing(mut self, spacing: f32) -> Self {
        self.line_spacing = Some(spacing.max(0.0));
        self
    }

    /// Center a single line vertically (default on for single-line fields).
    pub fn vertical_center(mut self, center: bool) -> Self {
        self.vertical_center = Some(center);
        self
    }

    /// Whether an inline editing session is open.
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    /// Inline IME composition text (empty when no composition is active).
    pub fn composition(&self) -> &str {
        &self.composition
    }

    pub fn max_length(mut self, max: u32) -> Self {
        self.max_length = max;
        self
    }

    /// Lock the field for `WidgetProps::read_only` and `Widget::read_only`.
    /// A locked painted field still takes pointer/keyboard focus, moves its
    /// caret, selects with the pointer or Shift+arrows, blinks and copies; it
    /// only refuses inputs that would change the value (typed characters, IME,
    /// paste, cut, delete, undo). Programmatic setters (`set_text`,
    /// `insert_text`, `clear`) stay available to the application.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Called with the new value after every user edit that changed the text
    /// (typing, paste, cut, delete, undo/redo, committed IME). Programmatic
    /// setters do not fire it. Returns the message to deliver, or `None`.
    pub fn on_change(mut self, handler: impl Fn(&str) -> Option<M> + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Called for each typed character (keyboard or IME result) before it is
    /// inserted into an open session. The insertion still happens, so this is
    /// an observation hook, not a filter. Returns the message to deliver, or
    /// `None`.
    pub fn on_char(mut self, handler: impl Fn(char) -> Option<M> + 'static) -> Self {
        self.on_char = Some(Box::new(handler));
        self
    }

    /// Called on a left-button click (press and release on the field).
    pub fn on_click(mut self, handler: impl Fn(Point) -> Option<M> + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// Called on a right-button click (the built-in edit menu still opens when
    /// the application installed no context menu).
    pub fn on_right_click(mut self, handler: impl Fn(Point) -> Option<M> + 'static) -> Self {
        self.on_right_click = Some(Box::new(handler));
        self
    }

    /// Called on a double click (the built-in word selection still runs).
    pub fn on_double_click(mut self, handler: impl Fn(Point) -> Option<M> + 'static) -> Self {
        self.on_double_click = Some(Box::new(handler));
        self
    }

    /// Called on every key press the field receives (auto-repeat included),
    /// before the built-in editing/commit behavior. `KeyUp` carries the key
    /// release for the same key.
    pub fn on_key_down(mut self, handler: impl Fn(KeyEvent) -> Option<M> + 'static) -> Self {
        self.on_key_down = Some(Box::new(handler));
        self
    }

    /// Called on every key release the field receives. A painted field gets
    /// these while it owns the keyboard; a native session is owned by the
    /// `EDIT` child, so its key releases never reach the widget.
    pub fn on_key_up(mut self, handler: impl Fn(KeyEvent) -> Option<M> + 'static) -> Self {
        self.on_key_up = Some(Box::new(handler));
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Current text, for the typed access seam (`Ui::with_widget`).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Caret position in characters (0..=char count).
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// Selected character range `(start, end)`, normalized; `None` when the
    /// selection is collapsed.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        let start = anchor.min(self.caret);
        let end = anchor.max(self.caret);
        (start != end).then_some((start, end))
    }

    /// Move the caret, clearing any selection. Returns whether anything moved.
    pub fn set_caret(&mut self, caret: usize) -> bool {
        let caret = self.clamp_chars(caret);
        if self.caret == caret && self.anchor.is_none() {
            return false;
        }
        self.caret = caret;
        self.anchor = None;
        self.goal_x = None;
        self.reveal.set(true);
        true
    }

    /// Select between `anchor` and `caret` (character offsets, either order).
    /// An empty span collapses to a caret at `caret`.
    pub fn set_selection(&mut self, anchor: usize, caret: usize) -> bool {
        let anchor = self.clamp_chars(anchor);
        let caret = self.clamp_chars(caret);
        if anchor == caret {
            return self.set_caret(caret);
        }
        let changed = self.caret != caret || self.anchor != Some(anchor);
        self.caret = caret;
        self.anchor = Some(anchor);
        self.goal_x = None;
        self.reveal.set(true);
        changed
    }

    /// Select the whole value, as Ctrl+A does in the native editor.
    pub fn select_all(&mut self) -> bool {
        self.set_selection(0, self.clamp_chars(usize::MAX))
    }

    /// Clear the field and reset the caret. Programmatic edits do not enforce
    /// `max_length`; the painted/native editors do while the user types.
    pub fn clear(&mut self) -> bool {
        if self.text.is_empty() && self.caret == 0 && self.anchor.is_none() {
            return false;
        }
        self.text.clear();
        self.caret = 0;
        self.anchor = None;
        self.goal_x = None;
        self.touch_text();
        true
    }

    /// Insert text at the caret, replacing the selection when there is one.
    /// Password masks stay in sync. Returns whether anything changed.
    pub fn insert_text(&mut self, text: &str) -> bool {
        self.insert_impl(text, false)
    }

    /// Insert typed/pasted/IME text, enforcing `max_length` and returning
    /// whether the value changed.
    fn insert_impl(&mut self, text: &str, enforce_limit: bool) -> bool {
        let text = crate::text::normalize_newlines(text);
        let (mut start, mut end) = self.selection().unwrap_or((self.caret, self.caret));
        start = self.clamp_chars(start);
        end = self.clamp_chars(end);
        let mut insert = text.as_ref();
        if !self.multiline {
            // A single-line field must not smuggle a line break in.
            insert = insert.trim_matches(['\r', '\n']);
        }
        let mut insert: std::borrow::Cow<'_, str> = std::borrow::Cow::Borrowed(insert);
        if enforce_limit && self.max_length > 0 {
            let remaining = (self.max_length as usize)
                .saturating_sub(self.char_len() - (end - start));
            if insert.chars().count() > remaining {
                let cut = crate::text::fitting_prefix_by_chars(&insert, remaining);
                insert = std::borrow::Cow::Owned(insert[..cut].to_string());
            }
        }
        if insert.is_empty() && start == end {
            return false;
        }
        if insert.is_empty() {
            self.push_undo(EditRun::DeleteBack);
        } else {
            self.push_undo(EditRun::Insert);
        }
        let start_byte = self.byte_at(start);
        let end_byte = self.byte_at(end);
        self.text.replace_range(start_byte..end_byte, &insert);
        self.caret = start + insert.chars().count();
        self.anchor = None;
        self.goal_x = None;
        self.touch_text();
        true
    }

    /// User-driven insertion (typing, paste, IME, a newline key). A read-only
    /// field refuses it; the application's `insert_text`/`set_text` bypass
    /// this and always work.
    fn user_insert(&mut self, text: &str, enforce_limit: bool) -> bool {
        if self.read_only {
            return false;
        }
        let changed = self.insert_impl(text, enforce_limit);
        if changed {
            self.mark_changed();
        }
        changed
    }

    /// Record that a user edit changed the text, for `on_change`.
    fn mark_changed(&self) {
        if self.on_change.is_some() {
            self.change_pending.set(true);
        }
    }

    /// Consume the pending `on_change` message (if any).
    fn on_change_message(&self) -> Option<M> {
        if !self.change_pending.replace(false) {
            return None;
        }
        self.on_change.as_ref().and_then(|handler| handler(&self.text))
    }

    /// The observer message for a user input: an earlier observer that already
    /// produced a message wins over `on_change`, and either consumes the
    /// pending change so it cannot leak into a later input.
    fn change_message(&self, message: Option<M>) -> Option<M> {
        if message.is_some() {
            self.change_pending.set(false);
            return message;
        }
        self.on_change_message()
    }

    /// Any text/mask change: invalidate the layout and mirror a password.
    fn touch_text(&mut self) {
        self.text_rev = self.text_rev.wrapping_add(1);
        self.layout.borrow_mut().stale = true;
        self.refresh_mask();
        self.reveal.set(true);
    }

    fn char_len(&self) -> usize {
        self.text.chars().count()
    }

    fn clamp_chars(&self, position: usize) -> usize {
        position.min(self.char_len())
    }

    /// Byte offset of character index `position` (clamped to the end).
    fn byte_at(&self, position: usize) -> usize {
        self.text
            .char_indices()
            .nth(position)
            .map(|(byte, _)| byte)
            .unwrap_or(self.text.len())
    }

    /// Current caret/selection as byte offsets into the *published* value
    /// (the mask for password fields, not the secret text).
    fn selection_bytes(&self) -> (usize, usize) {
        let (start, end) = self.selection().unwrap_or((self.caret, self.caret));
        let shown = self.shown();
        let total = shown.chars().count();
        let start = start.min(total);
        let end = end.min(total);
        (
            char_prefix(shown, start).len(),
            char_prefix(shown, end).len(),
        )
    }

    /// The closed field paints its own caret/selection only when keyboard or
    /// assistive focus is visible, the field is enabled and no native session
    /// is open. Pointer focus and the native child own the caret otherwise;
    /// painting ours as well left a stripe outside the child's text strip.
    fn shows_closed_caret(&self, state: &WidgetState) -> bool {
        state.focused && state.focus_visible && self.enabled && !state.editing
    }

    /// Selection background for the closed field (character offsets over the
    /// painted text, so password masks highlight the right glyphs).
    fn paint_selection(
        &self,
        ctx: &mut WidgetPaint<'_>,
        inner: Rect,
        shift: f32,
        style: &TextStyle,
    ) {
        let Some((start, end)) = self.selection() else {
            return;
        };
        let shown = self.shown();
        let total = shown.chars().count();
        let start = start.min(total);
        let end = end.min(total);
        if start >= end {
            return;
        }
        let x0 = inner.left + shift + ctx.measure_text_styled(char_prefix(shown, start), style).width;
        let x1 = inner.left + shift + ctx.measure_text_styled(char_prefix(shown, end), style).width;
        let x1 = x1.min(inner.right);
        if x0 < x1 {
            ctx.canvas.fill_rect(
                Rect::new(x0, inner.top + 2.0, x1, inner.bottom - 2.0),
                ctx.palette.row_selected,
            );
        }
    }

    fn paint_caret(
        &self,
        ctx: &mut WidgetPaint<'_>,
        inner: Rect,
        shift: f32,
        caret: usize,
        style: &TextStyle,
    ) {
        let shown = self.shown();
        let caret = caret.min(shown.chars().count());
        let width = ctx
            .measure_text_styled(char_prefix(shown, caret), style)
            .width;
        let x = (inner.left + shift + width).clamp(inner.left, (inner.right - 1.0).max(inner.left));
        ctx.canvas.fill_rect(
            Rect::new(x, inner.top + 3.0, x + 1.0, inner.bottom - 3.0),
            ctx.palette.text,
        );
    }

    /// Offset that places the text run according to [`TextEdit::align`], so
    /// the closed-field caret and selection follow the painted glyphs.
    fn align_shift(&self, ctx: &mut WidgetPaint<'_>, inner: Rect, style: &TextStyle) -> f32 {
        let total = ctx.measure_text_styled(self.shown(), style).width;
        match self.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => ((inner.width() - total) * 0.5).max(0.0),
            TextAlign::Right => (inner.width() - total).max(0.0),
        }
    }

    /// Text style for the closed paint: the theme body font, with the
    /// per-field size (absolute DIPs) applied when set.
    fn body_style(&self, painter: &crate::painter::Painter) -> TextStyle {
        let mut style = painter.body().clone();
        if let Some(size) = self.font_size {
            style.size = size;
        }
        style
    }

    /// Content padding: explicit override, else `(9, 2)` single-line / `(9, 6)`
    /// multiline.
    fn content_padding(&self) -> Insets {
        self.padding
            .unwrap_or_else(|| if self.multiline { Insets::xy(9.0, 6.0) } else { Insets::xy(9.0, 2.0) })
    }

    /// Which scrollbar chrome the geometry reserves.
    fn scrollbar_chrome(&self) -> (bool, bool) {
        (
            self.multiline && self.scrollbars.vertical(),
            self.multiline && self.scrollbars.horizontal() && !self.wrapping(),
        )
    }

    /// Word wrap applies to multiline fields only; a single-line value always
    /// scrolls horizontally.
    fn wrapping(&self) -> bool {
        self.multiline && self.wrap
    }

    /// Text area inside the frame, minus any reserved scrollbar strip.
    fn content_rect(&self) -> Rect {
        let inner = self.rect.inset_by(self.content_padding());
        let (vbar, hbar) = self.scrollbar_chrome();
        let reserve = self.bar_style.thickness + 3.0;
        Rect::new(
            inner.left,
            inner.top,
            inner.right - if vbar { reserve } else { 0.0 },
            inner.bottom - if hbar { reserve } else { 0.0 },
        )
    }

    fn v_track(&self) -> Rect {
        let inner = self.rect.inset_by(self.content_padding());
        let thickness = self.bar_style.thickness;
        Rect::new(
            self.rect.right - 4.0 - thickness,
            inner.top,
            self.rect.right - 4.0,
            self.content_rect().bottom,
        )
    }

    fn h_track(&self) -> Rect {
        let content = self.content_rect();
        let thickness = self.bar_style.thickness;
        Rect::new(
            content.left,
            content.bottom + 2.0,
            content.right,
            content.bottom + 2.0 + thickness,
        )
    }

    /// The notch where the two bars meet, when both are shown. Neither track
    /// covers it (the horizontal one stops at the content edge, the vertical
    /// one above it), so it is filled with the track color — the same junction
    /// mask the scroll view paints. Content never paints in the reserved
    /// strips, so only the background would show through.
    fn bar_corner(&self) -> Option<Rect> {
        let (vbar, hbar) = self.active_bars();
        if !(vbar && hbar) {
            return None;
        }
        let inner = self.rect.inset_by(self.content_padding());
        let content = self.content_rect();
        Some(Rect::new(
            content.right,
            content.bottom,
            inner.right,
            inner.bottom,
        ))
    }

    /// Fill the bar junction with the track color (both paint paths call it
    /// after their bars).
    fn paint_bar_corner(&self, ctx: &mut WidgetPaint<'_>) {
        let Some(corner) = self.bar_corner() else {
            return;
        };
        let colors = self.bar_style.resolved_colors(&ctx.palette);
        ctx.canvas.fill_rect(corner, colors.track);
    }

    /// `(track, content extent, visible extent, offset)` for one bar; `None`
    /// when the bar is not shown. Vertical units are lines, horizontal DIPs.
    fn bar_metrics(&self, vertical: bool) -> Option<(Rect, f32, f32, f32)> {
        let (vbar, hbar) = self.active_bars();
        if vertical && !vbar {
            return None;
        }
        if !vertical && !hbar {
            return None;
        }
        let content = self.content_rect();
        let cache = self.layout.borrow();
        if vertical {
            let line_height = cache.line_height.max(1.0);
            Some((
                self.v_track(),
                cache.lines.len().max(1) as f32 * line_height,
                content.height(),
                self.scroll_y.get(),
            ))
        } else {
            Some((
                self.h_track(),
                (cache.max_width + self.caret_gutter()).max(content.width()),
                content.width(),
                self.scroll_x.get(),
            ))
        }
    }

    fn bar_thumb(&self, track: Rect, total: f32, visible: f32, offset: f32, vertical: bool) -> Option<Rect> {
        scrollbar::thumb_rect(track, total, visible, offset, vertical, &self.bar_style)
    }

    fn update_bar_hover(&mut self, pos: Point) {
        let v = self.bar_metrics(true);
        let h = self.bar_metrics(false);
        if let Some((track, total, visible, offset)) = v {
            let thumb = self.bar_thumb(track, total, visible, offset, true);
            self.vbar.hovered = scrollbar::hit(track, thumb, pos).is_some();
        } else {
            self.vbar.hovered = false;
        }
        if let Some((track, total, visible, offset)) = h {
            let thumb = self.bar_thumb(track, total, visible, offset, false);
            self.hbar.hovered = scrollbar::hit(track, thumb, pos).is_some();
        } else {
            self.hbar.hovered = false;
        }
    }

    fn clear_bar_hover(&mut self) {
        if !self.vbar.active {
            self.vbar.hovered = false;
        }
        if !self.hbar.active {
            self.hbar.hovered = false;
        }
    }

    /// Start a thumb drag or a track jump. Returns whether the press belonged
    /// to a bar (the text/editor must then ignore it).
    fn handle_bar_press(&mut self, pos: Point) -> bool {
        let v = self.bar_metrics(true);
        if let Some((track, total, visible, offset)) = v {
            let thumb = self.bar_thumb(track, total, visible, offset, true);
            match scrollbar::hit(track, thumb, pos) {
                Some(BarHit::Thumb) => {
                    if let Some(thumb) = thumb {
                        self.vbar.active = true;
                        self.vbar.hovered = true;
                        self.vbar.grab = pos.y - thumb.top;
                        return true;
                    }
                }
                Some(BarHit::Track) => {
                    if let Some(thumb) = thumb {
                        let new = scrollbar::track_click_offset(
                            pos,
                            track,
                            thumb,
                            total,
                            visible,
                            offset,
                            true,
                            &self.bar_style,
                        );
                        let max = (total - visible).max(0.0);
                        self.scroll_y.set(new.clamp(0.0, max));
                        self.vbar.active = true;
                        self.vbar.hovered = true;
                        self.vbar.grab = thumb.height() * 0.5;
                        return true;
                    }
                }
                None => {}
            }
        }
        let h = self.bar_metrics(false);
        if let Some((track, total, visible, offset)) = h {
            let thumb = self.bar_thumb(track, total, visible, offset, false);
            match scrollbar::hit(track, thumb, pos) {
                Some(BarHit::Thumb) => {
                    if let Some(thumb) = thumb {
                        self.hbar.active = true;
                        self.hbar.hovered = true;
                        self.hbar.grab = pos.x - thumb.left;
                        return true;
                    }
                }
                Some(BarHit::Track) => {
                    if let Some(thumb) = thumb {
                        let new = scrollbar::track_click_offset(
                            pos,
                            track,
                            thumb,
                            total,
                            visible,
                            offset,
                            false,
                            &self.bar_style,
                        );
                        let max = (total - visible).max(0.0);
                        self.scroll_x.set(new.clamp(0.0, max));
                        self.hbar.active = true;
                        self.hbar.hovered = true;
                        self.hbar.grab = thumb.width() * 0.5;
                        return true;
                    }
                }
                None => {}
            }
        }
        false
    }

    /// Continue a bar drag. Returns whether a bar consumed the move.
    fn handle_bar_drag(&mut self, pos: Point) -> bool {
        if self.vbar.active {
            if let Some((track, total, visible, offset)) = self.bar_metrics(true)
                && let Some(thumb) = self.bar_thumb(track, total, visible, offset, true)
            {
                let new = scrollbar::drag_offset(
                    pos,
                    track,
                    thumb,
                    self.vbar.grab,
                    total,
                    visible,
                    true,
                );
                let max = (total - visible).max(0.0);
                self.scroll_y.set(new.clamp(0.0, max));
            }
            return true;
        }
        if self.hbar.active {
            if let Some((track, total, visible, offset)) = self.bar_metrics(false)
                && let Some(thumb) = self.bar_thumb(track, total, visible, offset, false)
            {
                let new = scrollbar::drag_offset(
                    pos,
                    track,
                    thumb,
                    self.hbar.grab,
                    total,
                    visible,
                    false,
                );
                self.scroll_x.set(new.clamp(0.0, (total - visible).max(0.0)));
            }
            return true;
        }
        false
    }

    /// Whether the bars are actually needed (overflow), for hit-testing.
    fn active_bars(&self) -> (bool, bool) {
        let (vbar, hbar) = self.scrollbar_chrome();
        let cache = self.layout.borrow();
        let total = cache.lines.len();
        let visible = cache.visible_lines.max(1);
        let v = vbar && total > visible;
        let h = hbar && cache.max_width > self.content_rect().width() + 0.5;
        (v, h)
    }

    /// Rebuild the cached line layout when the text, width, font, wrapping or
    /// spacing changed. Called by both paint paths.
    fn refresh_layout(
        &self,
        ctx: &mut WidgetPaint<'_>,
        content: Rect,
        style: &TextStyle,
        line_height: f32,
    ) {
        let spacing = line_height;
        let mut cache = self.layout.borrow_mut();
        if !cache.stale
            && (cache.width - content.width()).abs() < 0.01
            && (cache.font_size - style.size).abs() < 0.01
            && cache.wrap == self.wrapping()
            && cache.text_rev == self.text_rev
            && (cache.line_height - spacing).abs() < 0.01
        {
            cache.visible_lines = ((content.height() / line_height).floor() as usize).max(1);
            return;
        }
        cache.lines = editor::layout_lines(self.shown(), content.width(), self.wrapping(), &mut |slice| {
            ctx.canvas.measure_text(slice, style).width
        });
        cache.max_width = cache
            .lines
            .iter()
            .map(LineMetrics::width)
            .fold(0.0, f32::max);
        cache.width = content.width();
        cache.font_size = style.size;
        cache.wrap = self.wrapping();
        cache.line_height = spacing;
        cache.text_rev = self.text_rev;
        cache.visible_lines = ((content.height() / line_height).floor() as usize).max(1);
        cache.stale = false;
    }

    /// Clamped scroll state for a paint or a hit test.
    fn scroll_state(&self, cache: &TextLayout, content: Rect) -> (f32, f32) {
        let total = cache.lines.len() as f32 * cache.line_height.max(1.0);
        let max_y = (total - content.height()).max(0.0);
        let scroll = self.scroll_y.get().clamp(0.0, max_y);
        let max_width = if self.wrapping() {
            content.width()
        } else {
            cache.max_width
        };
        let scroll_x = if self.wrapping() {
            0.0
        } else {
            self.scroll_x
                .get()
                .clamp(0.0, (max_width + self.caret_gutter() - content.width()).max(0.0))
        };
        (scroll, scroll_x)
    }

    /// Room reserved at the right edge for the caret (its width plus a small
    /// margin), so a caret at the end of the text stays visible.
    fn caret_gutter(&self) -> f32 {
        self.caret_width.unwrap_or(2.0) + 2.0
    }

    /// Horizontal offset of a single line according to `align` (multiline
    /// fields stay left-aligned so wrapping is predictable).
    fn line_align_shift(&self, line_width: f32, content: Rect) -> f32 {
        if self.multiline {
            return 0.0;
        }
        match self.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => ((content.width() - line_width) * 0.5).max(0.0),
            TextAlign::Right => (content.width() - line_width).max(0.0),
        }
    }

    fn line_top(&self, index: usize, content: Rect, line_height: f32, scroll: f32) -> f32 {
        if self.multiline {
            content.top + index as f32 * line_height - scroll
        } else if self.vertical_center.unwrap_or(true) {
            content.top + ((content.height() - line_height) * 0.5).max(0.0)
        } else {
            content.top
        }
    }

    /// One-line rectangle at the first line, where an empty field paints its
    /// cue. A multiline field top-aligns its lines, so a cue drawn across the
    /// whole content area would sit in the vertical middle instead of on the
    /// caret's line. The rectangle keeps the full content width so the
    /// single-line alignment still applies.
    fn cue_rect(&self, content: Rect, line_height: f32, scroll: f32) -> Rect {
        let top = self.line_top(0, content, line_height, scroll);
        Rect::new(content.left, top, content.right, top + line_height)
    }

    /// Closed paint for a multiline field: lines from the top, clipped to the
    /// content area, with optional scrollbars. The closed view scrolls with the
    /// wheel; the inline/native editor takes over when the user edits.
    fn paint_multiline(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        let (border, background) = field_colors(&palette, self.enabled, state.focused);
        self.surface.frame(ctx.canvas, self.rect, border, background);
        let content = self.content_rect();
        if content.width() <= 1.0 || content.height() <= 1.0 {
            return;
        }
        let (vbar, hbar) = self.scrollbar_chrome();
        let base = self.body_style(ctx.painter);
        let style = ctx.style(&base);
        let line_height = ctx.canvas.measure_text("Ag", &style).height.max(style.size)
            + self.line_spacing.unwrap_or(2.0);
        self.refresh_layout(ctx, content, &style, line_height);
        let cache = self.layout.borrow();
        let (scroll, scroll_x) = self.scroll_state(&cache, content);
        let color = if self.enabled {
            palette.text
        } else {
            palette.text_disabled
        };
        ctx.canvas.push_clip(content);
        let shown = self.shown();
        if shown.is_empty() {
            if !self.cue.is_empty() {
                let cue_rect = self.cue_rect(content, line_height, scroll);
                ctx.canvas
                    .text(cue_rect, &self.cue, &style, TextAlign::Left, palette.muted);
            }
        } else {
            let first = (scroll / line_height).floor().max(0.0) as usize;
            let last = (((scroll + content.height()) / line_height).ceil() as usize)
                .clamp(first, cache.lines.len());
            for (index, line) in cache.lines.iter().enumerate().take(last).skip(first) {
                let top = self.line_top(index, content, line_height, scroll);
                ctx.canvas
                    .text_overflow(
                        Rect::new(
                            content.left,
                            top,
                            content.left + cache.max_width.max(content.width()) + 1.0,
                            top + line_height,
                        ),
                        &shown[line.start()..line.end()],
                        &style,
                        TextAlign::Left,
                        color,
                        TextOverflow::Clip,
                    );
            }
            let _ = scroll_x;
        }
        ctx.canvas.pop_clip();
        if vbar {
            let track = self.v_track();
            let total = cache.lines.len().max(1) as f32 * line_height;
            let thumb = self.bar_thumb(track, total, content.height(), scroll, true);
            scrollbar::paint(ctx, track, thumb, &self.vbar, &self.bar_style);
        }
        if hbar {
            let track = self.h_track();
            let total = cache.max_width.max(content.width());
            let visible = content.width();
            let thumb = scrollbar::thumb_rect(track, total, visible, scroll_x, false, &self.bar_style);
            scrollbar::paint(ctx, track, thumb, &self.hbar, &self.bar_style);
        }
        self.paint_bar_corner(ctx);
    }

    /// Framework-painted editor: frame, selection, text segments, inline IME
    /// composition, caret and scrollbars.
    fn paint_painted(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        let colors = self.colors.resolve(&palette, self.enabled);
        let editing = self.editing && self.enabled;
        let caret_width = self
            .caret_width
            .unwrap_or(ctx.painter.caret_width())
            .clamp(0.5, 6.0);
        // An open session gets the active frame and a faint accent tint, so
        // "focused" and "editing" never look the same — including when the
        // session was opened from the keyboard or by assistive technology
        // (where there is no pointer cue).
        if self.chrome {
            let border = if editing { colors.border } else { colors.border_idle };
            let background = if editing {
                colors.background.mix(colors.border, 0.07)
            } else {
                colors.background
            };
            self.surface.frame(ctx.canvas, self.rect, border, background);
            if editing {
                // A second 1 DIP ring inside the frame: visibly thicker while
                // editing without moving the text by changing the border
                // width. It follows the corner radius so the edit look matches.
                match self.surface.focus_radius() {
                    Some(radius) if radius > 1.0 => ctx.canvas.stroke_round_rect(
                        self.rect.inset(1.0, 1.0),
                        radius - 1.0,
                        1.0,
                        colors.border,
                    ),
                    _ => {
                        outline_inside(&mut *ctx.canvas, self.rect.inset(1.0, 1.0), colors.border);
                    }
                }
            }
        }
        let content = self.content_rect();
        if content.width() <= 1.0 || content.height() <= 1.0 {
            return;
        }
        let (vbar, hbar) = self.scrollbar_chrome();
        let base = self.body_style(ctx.painter);
        let style = ctx.style(&base);
        let line_height = ctx.canvas.measure_text("Ag", &style).height.max(style.size)
            + self.line_spacing.unwrap_or(2.0);
        self.refresh_layout(ctx, content, &style, line_height);
        let cache = self.layout.borrow();
        let (mut scroll, mut scroll_x) = self.scroll_state(&cache, content);
        let shown = self.shown();
        let caret_char = self.caret.min(shown.chars().count());
        let caret_byte = editor::byte_at_char(shown, caret_char);
        let caret_line = editor::line_of(&cache.lines, caret_byte);
        let align = self.line_align_shift(cache.max_width, content);
        let composition_width = if self.composition.is_empty() {
            0.0
        } else {
            ctx.canvas.measure_text(&self.composition, &style).width
        };
        // Keep the caret (and the composition tail) visible after a move or an
        // edit. Wheel scrolling does not set `reveal`, so it never snaps back.
        if self.reveal.replace(false) {
            if self.multiline {
                let caret_top = caret_line as f32 * line_height;
                let caret_bottom = caret_top + line_height;
                if caret_top < scroll {
                    scroll = caret_top;
                } else if caret_bottom > scroll + content.height() {
                    scroll = caret_bottom - content.height();
                }
                let limit = (cache.lines.len() as f32 * line_height - content.height()).max(0.0);
                scroll = scroll.clamp(0.0, limit);
            }
            if !self.wrapping() {
                let caret_x = cache.lines[caret_line].x_at(caret_byte);
                let end = caret_x + composition_width;
                // Keep the caret fully inside the content area, not just its
                // left edge: a caret at the very end used to land on the
                // right border and disappear under the clip.
                let right = scroll_x + content.width() - self.caret_gutter();
                if end > right {
                    scroll_x = end - content.width() + self.caret_gutter();
                }
                if caret_x < scroll_x {
                    scroll_x = caret_x;
                }
                let limit = (cache.max_width + self.caret_gutter() - content.width()).max(0.0);
                scroll_x = scroll_x.clamp(0.0, limit);
            }
        }
        // Persist the revealed offsets: `paint` takes `&self`, and the next
        // paint (caret blink, a rejected keystroke at `max_length`, a hover)
        // must not jump back to the start.
        self.scroll_y.set(scroll);
        self.scroll_x.set(scroll_x);
        let selection = self
            .selection()
            .map(|(start, end)| (editor::byte_at_char(shown, start), editor::byte_at_char(shown, end)));

        ctx.canvas.push_clip(content);
        if shown.is_empty() && self.composition.is_empty() && !self.cue.is_empty() {
            let cue_rect = self.cue_rect(content, line_height, scroll);
            let cue_align = if self.multiline {
                TextAlign::Left
            } else {
                self.align
            };
            ctx.canvas
                .text(cue_rect, &self.cue, &style, cue_align, colors.cue);
        }
        let first = (scroll / line_height).floor().max(0.0) as usize;
        let last = (((scroll + content.height()) / line_height).ceil() as usize)
            .clamp(first, cache.lines.len());
        for (index, line) in cache.lines.iter().enumerate().take(last).skip(first) {
            let top = self.line_top(index, content, line_height, scroll);
            if top >= content.bottom || top + line_height <= content.top {
                continue;
            }
            let left = content.left + align - scroll_x;
            // Selection rectangle (and the sliver that shows a selected
            // newline).
            let mut selected = None;
            if let Some((sb, eb)) = selection {
                let start = sb.max(line.start());
                let end = eb.min(line.end());
                if start < end {
                    let x0 = left + line.x_at(start);
                    let x1 = left + line.x_at(end);
                    ctx.canvas.fill_rect(
                        Rect::new(x0.max(content.left), top + 1.0, x1.min(content.right), top + line_height - 1.0),
                        colors.selection_background,
                    );
                    selected = Some((start, end));
                } else if sb <= line.end()
                    && eb > line.end()
                    && shown.as_bytes().get(line.end()) == Some(&b'\n')
                {
                    let x = left + line.width();
                    if x < content.right {
                        ctx.canvas.fill_rect(
                            Rect::new(x, top + 1.0, (x + 4.0).min(content.right), top + line_height - 1.0),
                            colors.selection_background,
                        );
                    }
                }
            }
            match selected {
                Some((start, end)) => {
                    self.paint_text_segment(
                        ctx, line, left, top, line_height, line.start(), start, shown, &style, colors.text,
                    );
                    self.paint_text_segment(
                        ctx, line, left, top, line_height, start, end, shown, &style, colors.selection_text,
                    );
                    self.paint_text_segment(
                        ctx, line, left, top, line_height, end, line.end(), shown, &style, colors.text,
                    );
                }
                None => self.paint_text_segment(
                    ctx, line, left, top, line_height, line.start(), line.end(), shown, &style, colors.text,
                ),
            }
        }

        let line_left = content.left + align - scroll_x;
        let caret_top = self.line_top(caret_line, content, line_height, scroll);
        let caret_x = line_left + cache.lines[caret_line].x_at(caret_byte);
        // Inline IME composition: an opaque strip starting at the caret. It
        // covers the text underneath (which a real inline composition would
        // push aside) and is underlined like every Win32 IME.
        let mut caret_screen = caret_x;
        if !self.composition.is_empty() {
            let width = composition_width;
            ctx.canvas.fill_rect(
                Rect::new(caret_x, caret_top, caret_x + width, caret_top + line_height),
                colors.background,
            );
            ctx.canvas.text_overflow(
                Rect::new(caret_x, caret_top, caret_x + width + 1.0, caret_top + line_height),
                &self.composition,
                &style,
                TextAlign::Left,
                colors.text,
                TextOverflow::Clip,
            );
            ctx.canvas.fill_rect(
                Rect::new(caret_x, caret_top + line_height - 1.0, caret_x + width, caret_top + line_height),
                colors.composition_underline,
            );
            let prefix = char_prefix(&self.composition, self.composition_cursor);
            caret_screen = caret_x + ctx.measure_text_styled(prefix, &style).width;
        }
        let caret_visible = self.enabled
            && state.caret_on
            && editing
            && caret_screen + caret_width <= content.right + 0.5
            && caret_top + line_height > content.top;
        let caret_rect = Rect::new(
            caret_screen,
            caret_top + 2.0,
            caret_screen + caret_width,
            (caret_top + line_height - 2.0).max(caret_top + 3.0),
        );
        if caret_visible {
            ctx.canvas.fill_rect(caret_rect, colors.caret);
        }
        ctx.canvas.pop_clip();
        // Touch selection handles sit above the text clip so the circles are
        // never sliced by the content edge (D121).
        if editing && self.touch_handles && self.selection().is_some() {
            self.paint_selection_handles(ctx, &colors);
        }
        if editing {
            self.caret_rect.set(Some(caret_rect));
        } else {
            self.caret_rect.set(None);
        }
        if vbar {
            let track = self.v_track();
            let total = cache.lines.len().max(1) as f32 * line_height;
            let thumb = self.bar_thumb(track, total, content.height(), scroll, true);
            scrollbar::paint(ctx, track, thumb, &self.vbar, &self.bar_style);
        }
        if hbar {
            let track = self.h_track();
            let total = cache.max_width.max(content.width());
            let visible = content.width();
            let thumb = scrollbar::thumb_rect(track, total, visible, scroll_x, false, &self.bar_style);
            scrollbar::paint(ctx, track, thumb, &self.hbar, &self.bar_style);
        }
        self.paint_bar_corner(ctx);
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_text_segment(
        &self,
        ctx: &mut WidgetPaint<'_>,
        line: &LineMetrics,
        left: f32,
        top: f32,
        line_height: f32,
        start: usize,
        end: usize,
        text: &str,
        style: &TextStyle,
        color: crate::Color,
    ) {
        if start >= end {
            return;
        }
        let content = self.content_rect();
        let x0 = left + line.x_at(start);
        let x1 = left + line.x_at(end);
        if x1 <= content.left || x0 >= content.right {
            return;
        }
        ctx.canvas.text_overflow(
            Rect::new(x0, top, (x1 + 1.0).max(x0 + 1.0), top + line_height),
            &text[start..end],
            style,
            TextAlign::Left,
            color,
            TextOverflow::Clip,
        );
    }

    /// Caret character index at a client point, using the last painted layout.
    fn caret_at(&self, pos: Point) -> usize {
        let content = self.content_rect();
        let cache = self.layout.borrow();
        if cache.lines.is_empty() {
            return self.char_len();
        }
        let (scroll, scroll_x) = self.scroll_state(&cache, content);
        let index = if self.multiline {
            // The offset is fractional, so add the pixel scroll before
            // dividing. A drag past an edge selects the nearest visible line;
            // `auto_scroll_drag` reveals the next one.
            let offset = ((pos.y - content.top + scroll) / cache.line_height.max(1.0)).floor();
            let offset = if offset < 0.0 { 0.0 } else { offset };
            (offset as usize).min(cache.lines.len() - 1)
        } else {
            0
        };
        let line = &cache.lines[index];
        let align = self.line_align_shift(cache.max_width, content);
        let x = pos.x - content.left - align + scroll_x;
        let byte = line.byte_at(x);
        editor::char_at_byte(self.shown(), byte)
    }

    fn place_caret_at(&mut self, pos: Point) {
        let caret = self.caret_at(pos);
        self.caret = caret;
        self.anchor = None;
        self.goal_x = None;
        self.reveal.set(false);
    }

    fn snapshot(&self) -> EditSnapshot {
        EditSnapshot {
            text: self.text.clone(),
            caret: self.caret,
            anchor: self.anchor,
        }
    }

    fn push_undo(&mut self, run: EditRun) {
        if self.last_run != run {
            self.undo.push(self.snapshot());
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_run = run;
    }

    fn restore(&mut self, snapshot: EditSnapshot) {
        self.text = snapshot.text;
        self.caret = self.clamp_chars(snapshot.caret);
        self.anchor = snapshot.anchor.map(|anchor| self.clamp_chars(anchor));
        self.touch_text();
        self.mark_changed();
    }

    fn undo(&mut self) -> bool {
        if self.read_only {
            return false;
        }
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.redo.push(current);
        self.restore(snapshot);
        self.last_run = EditRun::None;
        true
    }

    fn redo(&mut self) -> bool {
        if self.read_only {
            return false;
        }
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.undo.push(current);
        self.restore(snapshot);
        self.last_run = EditRun::None;
        true
    }

    /// Raw range deletion without an undo step.
    fn remove_range(&mut self, start: usize, end: usize) -> bool {
        if start >= end {
            return false;
        }
        let start = start.min(self.char_len());
        let end = end.min(self.char_len());
        if start >= end {
            return false;
        }
        let start_byte = self.byte_at(start);
        let end_byte = self.byte_at(end);
        self.text.replace_range(start_byte..end_byte, "");
        self.caret = start;
        self.anchor = None;
        self.goal_x = None;
        self.touch_text();
        self.mark_changed();
        true
    }

    fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection() else {
            return false;
        };
        self.push_undo(EditRun::DeleteBack);
        self.remove_range(start, end)
    }

    fn delete_back(&mut self, word: bool) -> bool {
        if self.read_only {
            return false;
        }
        if self.delete_selection() {
            return true;
        }
        if self.caret == 0 {
            return false;
        }
        let start = if word {
            editor::word_left(self.shown(), self.caret)
        } else {
            editor::prev_char(self.caret)
        };
        self.push_undo(EditRun::DeleteBack);
        self.remove_range(start, self.caret)
    }

    fn delete_forward(&mut self, word: bool) -> bool {
        if self.read_only {
            return false;
        }
        if self.delete_selection() {
            return true;
        }
        let len = self.char_len();
        if self.caret >= len {
            return false;
        }
        let end = if word {
            editor::word_right(self.shown(), self.caret)
        } else {
            editor::next_char(self.caret, len)
        };
        self.push_undo(EditRun::DeleteForward);
        self.remove_range(self.caret, end)
    }

    fn move_caret(&mut self, caret: usize, extend: bool) {
        let caret = self.clamp_chars(caret);
        if extend {
            let anchor = self.anchor.unwrap_or(self.caret);
            self.anchor = Some(anchor);
        } else {
            self.anchor = None;
        }
        self.caret = caret;
        self.goal_x = None;
        self.last_run = EditRun::None;
        self.reveal.set(true);
    }

    /// Bounds of the visual line containing `caret` (or of the hard line when
    /// no layout is available yet).
    fn visual_line_bounds(&self, caret: usize) -> (usize, usize) {
        let shown = self.shown();
        let cache = self.layout.borrow();
        if cache.lines.is_empty() {
            return (
                editor::hard_line_start(shown, caret),
                editor::hard_line_end(shown, caret),
            );
        }
        let byte = editor::byte_at_char(shown, caret);
        let index = editor::line_of(&cache.lines, byte);
        let line = &cache.lines[index];
        (
            editor::char_at_byte(shown, line.start()),
            editor::char_at_byte(shown, line.end()),
        )
    }

    fn move_vertical(&mut self, delta: i32, extend: bool) {
        if !self.multiline || delta == 0 {
            return;
        }
        let shown = self.shown();
        let cache = self.layout.borrow();
        if cache.lines.is_empty() {
            return;
        }
        let byte = editor::byte_at_char(shown, self.caret);
        let index = editor::line_of(&cache.lines, byte);
        let x = self.goal_x.unwrap_or_else(|| cache.lines[index].x_at(byte));
        let target = (index as i32 + delta).clamp(0, cache.lines.len() as i32 - 1) as usize;
        let byte = cache.lines[target].byte_at(x);
        let caret = editor::char_at_byte(shown, byte);
        drop(cache);
        if extend {
            let anchor = self.anchor.unwrap_or(self.caret);
            self.anchor = Some(anchor);
        } else {
            self.anchor = None;
        }
        self.caret = caret;
        self.goal_x = Some(x);
        self.last_run = EditRun::None;
        self.reveal.set(true);
    }

    fn page_lines(&self) -> usize {
        let cache = self.layout.borrow();
        cache.visible_lines.max(1).saturating_sub(1).max(1)
    }

    fn copy_selection(&mut self, cut: bool) -> bool {
        let Some((start, end)) = self.selection() else {
            return false;
        };
        if self.password {
            // Never put a secret on the clipboard through the built-in menu.
            return false;
        }
        let shown = self.shown();
        let text = shown[char_prefix(shown, start).len()..char_prefix(shown, end).len()].to_string();
        self.clipboard = Some(text);
        // A read-only field still copies the selection, but never cuts it.
        if cut && !self.read_only {
            self.push_undo(EditRun::DeleteBack);
            self.remove_range(start, end);
        }
        true
    }

    /// The start/end selection handle in client DIPs: the point on the text
    /// line (stem anchor) and the circle centre. `None` before the first
    /// layout or without a selection.
    fn handle_geometry(&self, edge: SelectionEdge) -> Option<(Point, Point)> {
        let (start, end) = self.selection()?;
        let shown = self.shown();
        let char_index = match edge {
            SelectionEdge::Start => start,
            SelectionEdge::End => end,
        };
        let byte = editor::byte_at_char(shown, char_index);
        let content = self.content_rect();
        if content.width() <= 1.0 || content.height() <= 1.0 {
            return None;
        }
        let cache = self.layout.borrow();
        if cache.lines.is_empty() || cache.line_height <= 0.0 {
            return None;
        }
        let (scroll, scroll_x) = self.scroll_state(&cache, content);
        let line = editor::line_of(&cache.lines, byte);
        let align = self.line_align_shift(cache.max_width, content);
        let x = content.left + align - scroll_x + cache.lines[line].x_at(byte);
        let top = self.line_top(line, content, cache.line_height, scroll);
        let (anchor, center_y) = match edge {
            SelectionEdge::Start => (Point::new(x, top), top - HANDLE_GAP),
            SelectionEdge::End => (
                Point::new(x, top + cache.line_height),
                top + cache.line_height + HANDLE_GAP,
            ),
        };
        // Keep the circle inside the field: the container repaints exactly the
        // widget rectangle, so a handle outside it would smear on the page.
        let min_x = self.rect.left + HANDLE_RADIUS;
        let max_x = (self.rect.right - HANDLE_RADIUS).max(min_x);
        let min_y = self.rect.top + HANDLE_RADIUS;
        let max_y = (self.rect.bottom - HANDLE_RADIUS).max(min_y);
        let center = Point::new(anchor.x.clamp(min_x, max_x), center_y.clamp(min_y, max_y));
        Some((anchor, center))
    }

    /// Move the dragged handle to the caret under `pos`. Crossing the other
    /// edge flips the selection; `selection()` normalises the pair.
    fn drag_handle_to(&mut self, pos: Point) {
        let Some(edge) = self.handle_drag else {
            return;
        };
        let (start, end) = self.selection().unwrap_or((self.caret, self.caret));
        let target = self.caret_at(pos);
        match edge {
            SelectionEdge::Start => {
                self.anchor = Some(target);
                self.caret = end;
            }
            SelectionEdge::End => {
                self.anchor = Some(start);
                self.caret = target;
            }
        }
        self.goal_x = None;
        self.reveal.set(false);
    }

    /// The touch selection handles, painted above the text (the clip is
    /// already popped) so the circles are not cut by the content edge.
    fn paint_selection_handles(
        &self,
        ctx: &mut WidgetPaint<'_>,
        colors: &ResolvedEditColors,
    ) {
        for edge in [SelectionEdge::Start, SelectionEdge::End] {
            let Some((anchor, center)) = self.handle_geometry(edge) else {
                continue;
            };
            // Stem from the text edge to the circle.
            ctx.canvas.fill_rect(
                Rect::new(
                    center.x - 0.75,
                    anchor.y.min(center.y),
                    center.x + 0.75,
                    anchor.y.max(center.y),
                ),
                colors.caret,
            );
            let outer = Rect::from_xywh(
                center.x - HANDLE_RADIUS,
                center.y - HANDLE_RADIUS,
                HANDLE_RADIUS * 2.0,
                HANDLE_RADIUS * 2.0,
            );
            ctx.canvas.fill_circle(outer, colors.background);
            let inner_radius = HANDLE_RADIUS - 1.5;
            ctx.canvas.fill_circle(
                Rect::from_xywh(
                    center.x - inner_radius,
                    center.y - inner_radius,
                    inner_radius * 2.0,
                    inner_radius * 2.0,
                ),
                colors.caret,
            );
        }
    }

    fn select_word_at(&mut self, pos: Point) {
        let caret = self.caret_at(pos);
        let (start, end) = editor::word_range(self.shown(), caret);
        self.anchor = Some(start);
        self.caret = end;
        self.goal_x = None;
        self.reveal.set(true);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<M> {
        if !self.enabled {
            return None;
        }
        let len = self.char_len();
        let shift = key.shift;
        if key.ctrl && !key.alt {
            match key.code {
                KeyCode::Letter(0) => {
                    self.select_all();
                }
                KeyCode::Letter(2) => {
                    self.copy_selection(false);
                }
                KeyCode::Letter(23) => {
                    self.copy_selection(true);
                }
                KeyCode::Letter(24) => {
                    self.redo();
                }
                KeyCode::Letter(25) => {
                    if shift {
                        self.redo();
                    } else {
                        self.undo();
                    }
                }
                KeyCode::Left => {
                    let to = editor::word_left(self.shown(), self.caret);
                    self.move_caret(to, shift);
                }
                KeyCode::Right => {
                    let to = editor::word_right(self.shown(), self.caret);
                    self.move_caret(to, shift);
                }
                KeyCode::Home => self.move_caret(0, shift),
                KeyCode::End => self.move_caret(len, shift),
                KeyCode::Backspace => {
                    self.delete_back(true);
                }
                KeyCode::Delete => {
                    self.delete_forward(true);
                }
                _ => {}
            }
            return None;
        }
        match key.code {
            KeyCode::Left => {
                let to = editor::prev_char(self.caret);
                self.move_caret(to, shift);
            }
            KeyCode::Right => {
                let to = editor::next_char(self.caret, len);
                self.move_caret(to, shift);
            }
            KeyCode::Home => {
                let (start, _) = self.visual_line_bounds(self.caret);
                self.move_caret(start, shift);
            }
            KeyCode::End => {
                let (_, end) = self.visual_line_bounds(self.caret);
                self.move_caret(end, shift);
            }
            KeyCode::Up => self.move_vertical(-1, shift),
            KeyCode::Down => self.move_vertical(1, shift),
            KeyCode::PageUp => {
                let step = self.page_lines() as i32;
                self.move_vertical(-step, shift);
            }
            KeyCode::PageDown => {
                let step = self.page_lines() as i32;
                self.move_vertical(step, shift);
            }
            KeyCode::Backspace => {
                self.delete_back(false);
            }
            KeyCode::Delete => {
                self.delete_forward(false);
            }
            KeyCode::Enter => {
                if self.multiline {
                    self.user_insert("\n", true);
                } else {
                    return self.finish_edit_impl(true);
                }
            }
            _ => {}
        }
        None
    }

    fn scroll_wheel(&mut self, delta: f32) {
        if !self.multiline || delta == 0.0 {
            return;
        }
        let content = self.content_rect();
        let (line_height, total) = {
            let cache = self.layout.borrow();
            let line_height = cache.line_height.max(1.0);
            (line_height, cache.lines.len() as f32 * line_height)
        };
        let max = (total - content.height()).max(0.0);
        if max <= 0.0 {
            return;
        }
        // The system's lines per notch (three by default), fractional: the
        // closed/painted view scrolls smoothly and matches the other widgets.
        let step = line_height * scrollbar::wheel_lines();
        self.scroll_y
            .set((self.scroll_y.get() - delta * step).clamp(0.0, max));
    }

    /// While a selection drag is held beyond an edge, scroll toward the hidden
    /// text. Called on every drag event; the step grows with the overshoot so
    /// dragging far outside scrolls faster. Holding still sends no events and
    /// therefore does not scroll (a timer would be needed for that).
    fn auto_scroll_drag(&mut self, pos: Point) {
        let multiline = self.multiline;
        let wrapping = self.wrapping();
        let content = self.content_rect();
        let (mut scroll, mut scroll_x, line_height, total, visible, max_width) = {
            let cache = self.layout.borrow();
            let (scroll, scroll_x) = self.scroll_state(&cache, content);
            let line_height = cache.line_height.max(1.0);
            (
                scroll,
                scroll_x,
                line_height,
                cache.lines.len() as f32 * line_height,
                content.height(),
                cache.max_width,
            )
        };
        let mut changed = false;
        if multiline {
            if pos.y < content.top {
                let step = ((content.top - pos.y) / line_height).ceil().clamp(1.0, 6.0) * line_height;
                let next = (scroll - step).max(0.0);
                if next != scroll {
                    scroll = next;
                    changed = true;
                }
            } else if pos.y >= content.bottom {
                let step = ((pos.y - content.bottom) / line_height).ceil().clamp(1.0, 6.0) * line_height;
                let max = (total - visible).max(0.0);
                let next = (scroll + step).min(max);
                if next != scroll {
                    scroll = next;
                    changed = true;
                }
            }
        }
        if !wrapping {
            if pos.x < content.left {
                let next = (scroll_x - 24.0).max(0.0);
                if next != scroll_x {
                    scroll_x = next;
                    changed = true;
                }
            } else if pos.x >= content.right {
                let max = (max_width - content.width()).max(0.0);
                let next = (scroll_x + 24.0).min(max);
                if next != scroll_x {
                    scroll_x = next;
                    changed = true;
                }
            }
        }
        if changed {
            self.scroll_y.set(scroll);
            self.scroll_x.set(scroll_x);
        }
    }

    /// Start (or re-position) the inline session without the container's
    /// read-only/paint-state checks.
    fn begin_edit_impl(&mut self, position: Option<Point>) -> bool {
        if !self.painted || !self.enabled {
            return false;
        }
        if !self.editing {
            self.previous = Some(self.snapshot());
            self.editing = true;
            self.undo.clear();
            self.redo.clear();
            self.last_run = EditRun::None;
            self.composition.clear();
            self.composition_cursor = 0;
            self.goal_x = None;
        }
        match position {
            Some(pos) => {
                // A press inside the current selection keeps it: the press may
                // turn into a drag-copy of the text (D122), and the click that
                // follows without a drag collapses it at the pointer. Any
                // other press places the caret as usual.
                let keep = self.caret_inside_selection(pos);
                if !keep {
                    self.place_caret_at(pos);
                }
                self.selecting = !keep;
                // A fresh press chooses the mouse model: the touch handles
                // belong to the gesture that started them (D121).
                self.touch_handles = false;
                self.handle_drag = None;
                self.press_kept = keep;
            }
            None => {
                self.selecting = false;
                self.reveal.set(false);
                self.press_kept = false;
            }
        }
        true
    }

    /// Whether the character under `pos` is inside the current selection.
    fn caret_inside_selection(&self, pos: Point) -> bool {
        let Some((start, end)) = self.selection() else {
            return false;
        };
        let caret = self.caret_at(pos);
        caret >= start && caret <= end
    }

    fn finish_edit_impl(&mut self, commit: bool) -> Option<M> {
        if !self.editing {
            return None;
        }
        self.editing = false;
        self.selecting = false;
        self.touch_handles = false;
        self.handle_drag = None;
        self.press_kept = false;
        self.vbar.active = false;
        self.hbar.active = false;
        self.vbar.hovered = false;
        self.hbar.hovered = false;
        self.composition.clear();
        self.composition_cursor = 0;
        self.goal_x = None;
        self.last_run = EditRun::None;
        self.caret_rect.set(None);
        let previous = self.previous.take();
        if !commit {
            if let Some(previous) = previous {
                self.text = previous.text;
                self.caret = self.clamp_chars(previous.caret);
                self.anchor = previous.anchor.map(|anchor| self.clamp_chars(anchor));
                self.touch_text();
            }
            return None;
        }
        let changed = previous.is_some_and(|previous| previous.text != self.text);
        self.undo.clear();
        self.redo.clear();
        // Committing closes the session; the highlight goes with it, so a
        // closed field never looks like an open editor.
        self.anchor = None;
        if changed {
            self.on_commit
                .as_ref()
                .map(|handler| handler(self.text.clone()))
        } else {
            None
        }
    }

    fn edit_menu_impl(&self) -> Option<Menu> {
        if !self.painted {
            return None;
        }
        let has_selection = self.selection().is_some() && !self.password;
        let has_text = !self.text.is_empty();
        let editable = !self.read_only;
        let item = |command: u32, label: &str, enabled: bool| {
            MenuEntry::Item(MenuItem::new(command, label).disabled(!enabled))
        };
        // `&` access keys: the shell shows the letter as "(U)" like the
        // native EDIT menu and activates the item when the key is pressed
        // while the popup is open.
        let control = |command: u32, label: &str| item(command, label, self.editing && editable);
        Some(
            Menu::new()
                .push(item(edit_command::UNDO, "撤销(&U)", editable && !self.undo.is_empty()))
                .push(item(edit_command::REDO, "重做(&R)", editable && !self.redo.is_empty()))
                .separator()
                .push(item(edit_command::CUT, "剪切(&T)", editable && has_selection))
                .push(item(edit_command::COPY, "复制(&C)", has_selection))
                .push(item(edit_command::PASTE, "粘贴(&P)", self.editing && editable))
                .push(item(edit_command::DELETE, "删除(&D)", editable && has_selection))
                .separator()
                .push(item(edit_command::SELECT_ALL, "全选(&A)", has_text))
                .separator()
                .push(MenuEntry::Submenu(crate::menu::Submenu::new(
                    "插入 Unicode 控制字符(&I)",
                    Menu::new()
                        .push(control(edit_command::INSERT_LRM, "从左到右标记(&L) (LRM)"))
                        .push(control(edit_command::INSERT_RLM, "从右到左标记(&R) (RLM)"))
                        .push(control(edit_command::INSERT_ZWSP, "零宽空格(&W) (ZWSP)"))
                        .push(control(edit_command::INSERT_ZWJ, "零宽连接符(&J) (ZWJ)"))
                        .push(control(edit_command::INSERT_ZWNJ, "零宽非连接符(&N) (ZWNJ)"))
                        .push(control(edit_command::INSERT_BOM, "字节顺序标记(&B) (BOM)")),
                ))),
        )
    }

    fn edit_command_impl(&mut self, command: u32) -> bool {
        if self.read_only {
            // Copy and select-all are the only menu commands a locked field
            // honors; everything else would mutate the value.
            return match command {
                edit_command::COPY => self.copy_selection(false),
                edit_command::SELECT_ALL => self.select_all(),
                _ => false,
            };
        }
        match command {
            edit_command::UNDO => self.undo(),
            edit_command::REDO => self.redo(),
            edit_command::CUT => self.copy_selection(true),
            edit_command::COPY => self.copy_selection(false),
            edit_command::DELETE => self.delete_selection(),
            edit_command::SELECT_ALL => self.select_all(),
            edit_command::INSERT_LRM => self.user_insert("\u{200E}", true),
            edit_command::INSERT_RLM => self.user_insert("\u{200F}", true),
            edit_command::INSERT_ZWSP => self.user_insert("\u{200B}", true),
            edit_command::INSERT_ZWJ => self.user_insert("\u{200D}", true),
            edit_command::INSERT_ZWNJ => self.user_insert("\u{200C}", true),
            edit_command::INSERT_BOM => self.user_insert("\u{FEFF}", true),
            _ => false,
        }
    }

    fn scrollbar_hit(&self, pos: Point) -> bool {
        let (vbar, hbar) = self.active_bars();
        (vbar && self.v_track().contains_point(pos)) || (hbar && self.h_track().contains_point(pos))
    }

    fn refresh_mask(&mut self) {
        if !self.password {
            return;
        }
        // One bullet per real character: caret/selection geometry indexes the
        // shown text, so a capped mask would drift once the value is longer
        // than the bullet template.
        let mut remaining = self.text.chars().count().min(4096);
        self.masked.clear();
        while remaining > 0 {
            let take = remaining.min(MASK.chars().count());
            self.masked.extend(MASK.chars().take(take));
            remaining -= take;
        }
    }

    fn shown(&self) -> &str {
        if self.password {
            &self.masked
        } else {
            &self.text
        }
    }
}

impl<M: 'static> Widget<M> for TextEdit<M> {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
    }

    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::TEXT
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        EDIT_SKIN_ATTRS
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("cue", SkinValue::Text(cue)) => self.cue = (*cue).to_string(),
            ("color", SkinValue::Color(_)) => {
                if let Some(color) = value.as_color() {
                    self.set_text_color(Some(color));
                }
            }
            _ => {}
        }
    }

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

    fn preferred_height(&self, _width: f32) -> f32 {
        if self.multiline { 84.0 } else { 34.0 }
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.editing = false;
            self.selecting = false;
            self.vbar.reset();
            self.hbar.reset();
            self.composition.clear();
            self.composition_cursor = 0;
            self.caret_rect.set(None);
        }
    }

    fn wants_edit(&self) -> bool {
        self.enabled && !self.painted
    }

    /// A closed multiline field reserves its scrollbar strips: a press there
    /// drives the bar instead of opening the native editor.
    fn wants_edit_at(&self, pos: Point) -> bool {
        self.wants_edit() && !self.scrollbar_hit(pos)
    }

    fn inline_edit(&self) -> bool {
        self.painted && self.enabled
    }

    fn inline_edit_at(&self, pos: Point) -> bool {
        self.inline_edit() && !self.scrollbar_hit(pos)
    }

    fn editing(&self) -> bool {
        self.editing
    }

    fn multiline_edit(&self) -> bool {
        self.multiline
    }

    /// A multiline field scrolls its own lines with the wheel; a single-line
    /// field leaves the notch to a scroll view around it (D65).
    fn scrolls_with_wheel(&self) -> bool {
        self.multiline
    }

    /// A selection drag, a bar drag or a bar hover changes what is painted on
    /// pointer motion; a plain hover over the text does not. The bar's hover
    /// flag is set by the hover event itself, so this returns true from the
    /// next move on (the entry event repaints on the following move).
    fn repaint_on_move(&self) -> bool {
        self.selecting
            || self.vbar.active
            || self.hbar.active
            || self.vbar.hovered
            || self.hbar.hovered
    }

    fn begin_inline_edit(&mut self, position: Option<Point>) -> bool {
        self.begin_edit_impl(position)
    }

    fn finish_edit(&mut self, commit: bool) -> Option<M> {
        self.finish_edit_impl(commit)
    }

    fn take_session(&mut self) -> Option<SessionSnapshot> {
        if !self.painted {
            return None;
        }
        let snapshot = SessionSnapshot {
            text: self.text.clone(),
            caret: self.caret,
            anchor: self.anchor,
            original: self.previous.as_ref().map(|previous| previous.text.clone()),
            open: self.editing,
        };
        // Close the draft without committing or reverting; the container's
        // normal removal then has nothing left to finish.
        self.editing = false;
        self.selecting = false;
        self.composition.clear();
        self.composition_cursor = 0;
        self.goal_x = None;
        self.last_run = EditRun::None;
        self.vbar.active = false;
        self.hbar.active = false;
        self.caret_rect.set(None);
        self.previous = None;
        self.undo.clear();
        self.redo.clear();
        Some(snapshot)
    }

    fn restore_session(&mut self, snapshot: SessionSnapshot) -> bool {
        if !self.painted || !self.enabled {
            return false;
        }
        self.text = crate::text::normalize_newlines(&snapshot.text).into_owned();
        self.caret = self.clamp_chars(snapshot.caret);
        self.anchor = snapshot.anchor.map(|anchor| self.clamp_chars(anchor));
        self.touch_text();
        if snapshot.open {
            self.previous = Some(EditSnapshot {
                text: snapshot
                    .original
                    .unwrap_or_else(|| self.text.clone()),
                caret: self.caret,
                anchor: self.anchor,
            });
            self.editing = true;
            self.selecting = false;
            self.composition.clear();
            self.composition_cursor = 0;
            self.undo.clear();
            self.redo.clear();
            self.last_run = EditRun::None;
            self.reveal.set(true);
        }
        true
    }

    fn insert_text(&mut self, text: &str) -> bool {
        self.user_insert(text, true)
    }

    /// A touch press held in place: select the word under it and turn on the
    /// draggable handles, the mobile text-selection model (D121).
    fn long_press(&mut self, pos: Point) -> bool {
        if !self.painted || !self.enabled || !self.editing {
            return false;
        }
        self.select_word_at(pos);
        self.touch_handles = true;
        self.selecting = false;
        self.handle_drag = None;
        self.press_kept = false;
        true
    }

    fn grab_selection_handle(&mut self, pos: Point) -> bool {
        if !self.painted || !self.editing || !self.touch_handles {
            return false;
        }
        for edge in [SelectionEdge::Start, SelectionEdge::End] {
            let Some((_, center)) = self.handle_geometry(edge) else {
                continue;
            };
            let dx = pos.x - center.x;
            let dy = pos.y - center.y;
            if dx * dx + dy * dy <= HANDLE_HIT_RADIUS * HANDLE_HIT_RADIUS {
                self.handle_drag = Some(edge);
                self.selecting = false;
                self.press_kept = false;
                return true;
            }
        }
        false
    }

    fn take_drag_text(&mut self, _pos: Point) -> Option<String> {
        if !self.painted || !self.editing || self.handle_drag.is_some() || self.password {
            return None;
        }
        let (start, end) = self.selection()?;
        if start == end {
            return None;
        }
        let shown = self.shown();
        let text = shown[char_prefix(shown, start).len()..char_prefix(shown, end).len()].to_string();
        (!text.is_empty()).then_some(text)
    }

    /// A text drag released over the field: open a session if needed, put the
    /// caret under the pointer and insert (a user edit, so `on_change` fires).
    fn insert_dropped_text(&mut self, text: &str, pos: Point) -> bool {
        if !self.painted || !self.enabled || self.read_only || text.is_empty() {
            return false;
        }
        if !self.editing && !self.begin_edit_impl(None) {
            return false;
        }
        self.place_caret_at(pos);
        self.touch_handles = false;
        self.handle_drag = None;
        self.press_kept = false;
        self.user_insert(text, true)
    }

    fn observe_char(&mut self, ch: char) -> Option<M> {
        if !self.enabled || self.read_only {
            return None;
        }
        self.on_char.as_ref().and_then(|handler| handler(ch))
    }

    fn take_change_message(&mut self) -> Option<M> {
        self.on_change_message()
    }

    fn set_composition(&mut self, text: &str, cursor: usize) -> bool {
        if self.read_only {
            return false;
        }
        let text = crate::text::normalize_newlines(text).into_owned();
        let cursor = cursor.min(text.chars().count());
        if self.composition == text && self.composition_cursor == cursor {
            return false;
        }
        self.composition = text;
        self.composition_cursor = cursor;
        self.reveal.set(true);
        true
    }

    fn take_clipboard(&mut self) -> Option<String> {
        self.clipboard.take()
    }

    fn caret_rect(&self) -> Option<Rect> {
        self.caret_rect.get()
    }

    fn edit_menu(&self) -> Option<Menu> {
        self.edit_menu_impl()
    }

    fn edit_command(&mut self, command: u32) -> bool {
        self.edit_command_impl(command)
    }

    fn edit_request(&self) -> Option<EditRequest<'_>> {
        if self.painted {
            return None;
        }
        self.enabled.then(|| EditRequest {
            id: self.id,
            rect: self.rect,
            text: &self.text,
            cue: &self.cue,
            password: self.password,
            multiline: self.multiline,
            wrap: self.wrap,
            scrollbars: self.scrollbars,
            numbers_only: false,
            align: self.align,
            font_size: self.font_size,
            font_family: None,
            max_length: self.max_length,
        })
    }

    fn commit_text(&mut self, text: String) -> Option<M> {
        let text = crate::text::normalize_newlines(&text).into_owned();
        if text == self.text {
            return None;
        }
        self.text = text;
        self.caret = self.char_len();
        self.anchor = None;
        self.scroll_y.set(0.0);
        self.scroll_x.set(0.0);
        self.goal_x = None;
        self.touch_text();
        self.on_commit
            .as_ref()
            .map(|handler| handler(self.text.clone()))
    }

    fn set_text(&mut self, text: String) {
        self.text = crate::text::normalize_newlines(&text).into_owned();
        self.caret = self.char_len();
        self.scroll_y.set(0.0);
        self.scroll_x.set(0.0);
        self.anchor = None;
        self.goal_x = None;
        self.composition.clear();
        self.composition_cursor = 0;
        self.touch_text();
    }

    /// The focus ring would be covered by the native editor; the widget paints
    /// an accent frame instead.
    fn focus_inset(&self) -> f32 {
        0.0
    }

    fn focus_ring_radius(&self) -> Option<f32> {
        self.surface.focus_radius()
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        if self.painted {
            self.paint_painted(ctx, state);
            return;
        }
        if self.multiline {
            self.paint_multiline(ctx, state);
            return;
        }
        let pal = ctx.palette;
        let (border, background) = field_colors(&pal, self.enabled, state.focused);
        self.surface.frame(ctx.canvas, self.rect, border, background);
        let inner = self.rect.inset(9.0, 2.0);
        let base = self.body_style(ctx.painter);
        // WidgetProps::font overrides the per-field size/family.
        let style = ctx.style(&base);
        // The native editor covers the widget while a session is open; a
        // keyboard-focused closed field still shows its caret/selection so
        // Tab-then-Enter has a visible target.
        let closed_focus = self.shows_closed_caret(state);
        let shift = if self.text.is_empty() {
            0.0
        } else {
            self.align_shift(ctx, inner, &style)
        };
        if self.text.is_empty() {
            if closed_focus {
                self.paint_caret(ctx, inner, shift, 0, &style);
            }
            if !self.cue.is_empty() {
                ctx.text_styled(inner, &self.cue, &style, self.align, pal.muted);
            }
            return;
        }
        if closed_focus {
            self.paint_selection(ctx, inner, shift, &style);
        }
        let color = if self.enabled { pal.text } else { pal.text_disabled };
        let shown = self.shown();
        ctx.text_styled(inner, shown, &style, self.align, color);
        if closed_focus {
            let caret = self.caret.min(shown.chars().count());
            self.paint_caret(ctx, inner, shift, caret, &style);
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        let mut message = None;
        // Scrollbar interaction is shared by the painted editor and the closed
        // multiline field: hover feedback, thumb drags and track jumps.
        match input {
            WidgetInput::HoverAt(pos) => self.update_bar_hover(pos),
            WidgetInput::HoverLeave => self.clear_bar_hover(),
            WidgetInput::PressAt(pos) => {
                self.handle_bar_press(pos);
            }
            WidgetInput::DragTo(pos) => {
                if self.handle_drag.is_some() {
                    self.drag_handle_to(pos);
                    return None;
                }
                if self.handle_bar_drag(pos) {
                    return None;
                }
                if self.painted && self.selecting {
                    self.auto_scroll_drag(pos);
                    let caret = self.caret_at(pos);
                    if caret != self.caret {
                        let anchor = self.anchor.unwrap_or(self.caret);
                        self.anchor = Some(anchor);
                        self.caret = caret;
                        self.goal_x = None;
                        self.reveal.set(false);
                    }
                }
            }
            WidgetInput::ReleaseAt(_) | WidgetInput::DragEnd(_) => {
                self.vbar.active = false;
                self.hbar.active = false;
                self.selecting = false;
                self.handle_drag = None;
                self.last_run = EditRun::None;
            }
            WidgetInput::RightClickAt(pos) => {
                // The container opens the edit menu; a right press never
                // starts a selection or a scrollbar drag.
                self.selecting = false;
                self.vbar.active = false;
                self.hbar.active = false;
                message = self.on_right_click.as_ref().and_then(|handler| handler(pos));
            }
            WidgetInput::DoubleClickAt(pos) => {
                if self.painted {
                    self.select_word_at(pos);
                }
                message = self.on_double_click.as_ref().and_then(|handler| handler(pos));
            }
            WidgetInput::ClickAt(pos) => {
                // A bar press/release is not a text click.
                if !self.scrollbar_hit(pos) {
                    // A press inside the selection kept it for a possible
                    // drag-out; a plain click collapses it here (D122).
                    if self.press_kept {
                        self.place_caret_at(pos);
                        self.press_kept = false;
                        self.touch_handles = false;
                    }
                    message = self.on_click.as_ref().and_then(|handler| handler(pos));
                }
            }
            WidgetInput::Wheel(delta) => self.scroll_wheel(delta),
            WidgetInput::Key(key) => {
                let observed = self.on_key_down.as_ref().and_then(|handler| handler(key));
                let handled = if self.painted { self.handle_key(key) } else { None };
                message = observed.or(handled);
            }
            WidgetInput::KeyUp(key) => {
                message = self.on_key_up.as_ref().and_then(|handler| handler(key));
            }
            WidgetInput::Char(ch) => {
                if self.painted && self.editing && !self.read_only {
                    let observed = self.on_char.as_ref().and_then(|handler| handler(ch));
                    let mut buffer = [0u8; 4];
                    self.user_insert(ch.encode_utf8(&mut buffer), true);
                    message = observed;
                }
            }
            WidgetInput::Activate if self.painted => {
                self.begin_edit_impl(None);
            }
            _ => {}
        }
        self.change_message(message)
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::IBeam)
    }

    fn cursor_at(&self, pos: Point) -> Option<Cursor> {
        // Both modes expose their bars (a closed multiline field's press
        // drags it), and a thumb drag is not a text click.
        if self.scrollbar_hit(pos) {
            Some(Cursor::Arrow)
        } else {
            Some(Cursor::IBeam)
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let name = if self.cue.is_empty() {
            self.id.0.to_string()
        } else {
            self.cue.clone()
        };
        let value = if self.password {
            self.shown().to_string()
        } else {
            self.text.clone()
        };
        let (selection_start, selection_end) = self.selection_bytes();
        Some(
            A11yInfo::new(A11yRole::TextField, name)
                .value(value)
                .read_only(!self.enabled || self.read_only)
                .text_selection(selection_start, selection_end),
        )
    }

    fn a11y_set_value(&mut self, text: &str) -> Option<M> {
        if !self.enabled || self.read_only {
            return None;
        }
        self.commit_text(text.to_string())
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }
}

/// Prefix of `text` up to `chars` characters (clamped to the end).
fn char_prefix(text: &str, chars: usize) -> &str {
    text.char_indices()
        .nth(chars)
        .map_or(text, |(byte, _)| &text[..byte])
}

/// A 1 DIP rectangle outline (the active editor's inner frame).
fn outline_inside(canvas: &mut dyn crate::backend::Canvas, rect: Rect, color: crate::Color) {
    if rect.is_empty() {
        return;
    }
    canvas.fill_rect(Rect::new(rect.left, rect.top, rect.right, rect.top + 1.0), color);
    canvas.fill_rect(
        Rect::new(rect.left, rect.bottom - 1.0, rect.right, rect.bottom),
        color,
    );
    canvas.fill_rect(Rect::new(rect.left, rect.top, rect.left + 1.0, rect.bottom), color);
    canvas.fill_rect(
        Rect::new(rect.right - 1.0, rect.top, rect.right, rect.bottom),
        color,
    );
}

/// Which stepper zone the pointer is over / pressing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StepperZone {
    Up,
    Down,
}

pub struct NumberInput<M> {
    id: WidgetId,
    rect: Rect,
    value: i64,
    min: i64,
    max: i64,
    /// Increment used by the stepper buttons and `step()`.
    step: i64,
    /// Prefix and suffix are painted around the number (unit, currency).
    prefix: String,
    suffix: String,
    /// Show the up/down stepper zones on the right edge.
    steppers: bool,
    /// Zone under the pointer / held down, for per-zone hover and press paint.
    hover_zone: Option<StepperZone>,
    press_zone: Option<StepperZone>,
    /// Cached decimal form of `value`, refreshed off the paint path.
    display: String,
    /// Fixed decimal places for the displayed/parsed value (0 = integer).
    decimals: u8,
    /// Custom display text; defaults to the fixed-point form.
    format: Option<Box<dyn Fn(i64) -> String>>,
    /// Custom committed-text parser; defaults to the fixed-point form.
    parse: Option<Box<dyn Fn(&str) -> Option<i64>>>,
    /// Number alignment (right by default, like a spreadsheet cell).
    align: TextAlign,
    /// Per-widget text size (theme-scaled DIPs).
    font_size: Option<f32>,
    enabled: bool,
    /// Locks the value without disabling the field: the session still selects
    /// and copies, only mutations (typing, steppers) are refused.
    read_only: bool,
    on_change: Box<dyn Fn(i64) -> M>,
    /// The embedded painted field, open while the number is being typed.
    field: RefCell<PaintedField<M>>,
    editing: bool,
    /// The text area inside the decorations, measured by the last paint: the
    /// editing field and the closed display share it, so the edit width never
    /// moves and the stepper/prefix/suffix chrome never covers the text.
    text_area: std::cell::Cell<Rect>,
    surface: SurfaceStyle,
}

impl<M> NumberInput<M> {
    pub fn new(
        id: &'static str,
        value: i64,
        range: std::ops::RangeInclusive<i64>,
        on_change: impl Fn(i64) -> M + 'static,
    ) -> Self {
        let (min, max) = (*range.start(), *range.end());
        let value = value.clamp(min, max);
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 120.0, 34.0),
            value,
            min,
            max,
            step: 1,
            prefix: String::new(),
            suffix: String::new(),
            steppers: false,
            hover_zone: None,
            press_zone: None,
            display: value.to_string(),
            decimals: 0,
            format: None,
            parse: None,
            align: TextAlign::Right,
            font_size: None,
            enabled: true,
            read_only: false,
            on_change: Box::new(on_change),
            field: RefCell::new(PaintedField::new()),
            editing: false,
            text_area: std::cell::Cell::new(Rect::default()),
            surface: SurfaceStyle::new(),
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Lock the value like `TextEdit::read_only`: focus, selection and copy
    /// stay available, mutations (typing, the steppers) are refused, and the
    /// UIA value reports read-only. Distinct from `disabled`, which also
    /// removes focus/mouse policy and greys the field.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Number alignment and the native editor's alignment (right by default).
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Per-field text size in DIPs, scaled by the theme like the body font.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size.max(1.0));
        self
    }

    /// Text painted before the number, e.g. `"¥"`.
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Text painted after the number, e.g. `"px"` or `"秒"`.
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }

    pub fn prefix_text(&self) -> &str {
        &self.prefix
    }

    pub fn suffix_text(&self) -> &str {
        &self.suffix
    }

    /// Replace the painted prefix silently (the skin's `prefix` attribute).
    pub fn set_prefix(&mut self, prefix: impl Into<String>) {
        self.prefix = prefix.into();
    }

    /// Replace the painted suffix silently (the skin's `suffix` attribute).
    pub fn set_suffix(&mut self, suffix: impl Into<String>) {
        self.suffix = suffix.into();
    }

    /// Value increment for the stepper zones (minimum 1).
    pub fn step_size(mut self, step: i64) -> Self {
        self.step = step.max(1);
        self
    }

    /// Show the up/down stepper zones on the right edge. Clicking a zone
    /// increments/decrements and emits the change; the editable area excludes
    /// them, so a stepper click never opens the native editor.
    pub fn steppers(mut self) -> Self {
        self.steppers = true;
        self
    }

    /// Show and parse the value with `decimals` fixed decimal places
    /// (`value = 250, decimals = 2` paints `2.50`; typing `2.75` commits
    /// `275`). The value itself stays an integer, so steppers and the
    /// RangeValue pattern keep working in raw units.
    pub fn decimals(mut self, decimals: u8) -> Self {
        self.decimals = decimals.min(9);
        self.refresh_display();
        self
    }

    /// Custom display text (a unit conversion, thousands separators). Pair it
    /// with [`NumberInput::parse`] so the editor can read the text back.
    pub fn format(mut self, format: impl Fn(i64) -> String + 'static) -> Self {
        self.format = Some(Box::new(format));
        self.refresh_display();
        self
    }

    /// Custom parser for committed text; returns the raw integer or `None` to
    /// keep the current value. Replaces the fixed-point parser.
    pub fn parse(mut self, parse: impl Fn(&str) -> Option<i64> + 'static) -> Self {
        self.parse = Some(Box::new(parse));
        self
    }

    /// The text the field paints and starts its editor with.
    pub fn display(&self) -> &str {
        &self.display
    }

    /// Whether a painted typing session is open.
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    fn refresh_display(&mut self) {
        let text = match &self.format {
            Some(format) => format(self.value),
            None => format_fixed(self.value, self.decimals),
        };
        self.display.clear();
        self.display.push_str(&text);
    }

    fn parse_committed(&self, text: &str) -> Option<i64> {
        let text = text.trim();
        match &self.parse {
            Some(parse) => parse(text),
            None => parse_fixed(text, self.decimals),
        }
    }

    fn stepper_rects(&self) -> Option<(Rect, Rect)> {
        if !self.steppers {
            return None;
        }
        let zone = Rect::new(
            self.rect.right - 20.0,
            self.rect.top + 1.0,
            self.rect.right - 1.0,
            self.rect.bottom - 1.0,
        );
        let center = zone.center().y;
        Some((
            Rect::new(zone.left, zone.top, zone.right, center),
            Rect::new(zone.left, center, zone.right, zone.bottom),
        ))
    }

    fn zone_at(&self, pos: Point) -> Option<StepperZone> {
        let (up, down) = self.stepper_rects()?;
        if up.contains_point(pos) {
            Some(StepperZone::Up)
        } else if down.contains_point(pos) {
            Some(StepperZone::Down)
        } else {
            None
        }
    }

    /// Current value, for the typed access seam (`Ui::with_widget`).
    pub fn value(&self) -> i64 {
        self.value
    }

    /// Replace the value (clamped into `min..=max`). Emits nothing; returns
    /// whether it changed. While the user is typing, the open session owns the
    /// text, so the programmatic value is ignored until it commits.
    pub fn set_value(&mut self, value: i64) -> bool {
        if self.editing {
            return false;
        }
        let next = value.clamp(self.min, self.max);
        if next == self.value {
            return false;
        }
        self.set_value_clamped(next);
        true
    }

    /// Step the value within `min..=max`. Emits nothing: the application
    /// decides whether the new value becomes a message.
    pub fn step(&mut self, delta: i64) -> bool {
        let next = self.value.saturating_add(delta).clamp(self.min, self.max);
        if next == self.value {
            return false;
        }
        self.set_value_clamped(next);
        true
    }

    /// Stepper click: adjust and report the new value when it changed.
    fn adjust(&mut self, delta: i64) -> Option<M> {
        let next = self
            .value
            .saturating_add(delta.saturating_mul(self.step))
            .clamp(self.min, self.max);
        if next == self.value {
            return None;
        }
        self.set_value_clamped(next);
        Some((self.on_change)(self.value))
    }

    fn set_value_clamped(&mut self, value: i64) {
        self.value = value.clamp(self.min, self.max);
        self.refresh_display();
    }

    /// Open (or re-position) the painted session over the whole field. The
    /// widget draws its own frame, prefix/suffix and steppers, so the field's
    /// chrome is off; while editing those decorations are hidden, exactly
    /// like the native editor used to cover them.
    fn begin_edit(&mut self, position: Option<Point>) -> bool
    where
        M: 'static,
    {
        if !self.enabled {
            return false;
        }
        // Only the text area is the editor; the steppers stay buttons and the
        // prefix/suffix stay labels while typing.
        if let Some(pos) = position
            && self.zone_at(pos).is_some()
        {
            return false;
        }
        if self.editing {
            return match position {
                Some(pos) => self.field.get_mut().place_at(pos),
                None => true,
            };
        }
        let rect = {
            let cached = self.text_area.get();
            if cached.is_empty() {
                let inner = self.rect.inset(9.0, 2.0);
                let right = match self.stepper_rects() {
                    Some((up, _)) => (up.left - 4.0).max(inner.left),
                    None => inner.right,
                };
                Rect::new(inner.left, inner.top, right, inner.bottom)
            } else {
                cached
            }
        };
        self.field.get_mut().set_read_only(self.read_only);
        let opened = self.field.get_mut().begin_styled(
            rect,
            &self.display,
            position,
            Insets::xy(0.0, 0.0),
            self.align,
            false,
            12,
        );
        if opened {
            self.editing = true;
            self.hover_zone = None;
            self.press_zone = None;
        }
        opened
    }

    /// Close the session: parse the text into the raw value and report a
    /// change, or keep the value on a cancel.
    fn finish_edit(&mut self, commit: bool) -> Option<M> {
        if !self.editing {
            return None;
        }
        self.editing = false;
        if self.read_only {
            self.field.get_mut().finish(false);
            return None;
        }
        let text = self.field.get_mut().finish(commit)?;
        let parsed = self.parse_committed(&text);
        let next = parsed.unwrap_or(self.value);
        let changed = next.clamp(self.min, self.max) != self.value;
        self.set_value_clamped(next);
        changed.then(|| (self.on_change)(self.value))
    }

    fn close_edit(&mut self) {
        if self.editing {
            self.editing = false;
            self.field.get_mut().finish(false);
        }
    }
}

impl<M: 'static> Widget<M> for NumberInput<M> {
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        Some(&mut self.surface)
    }

    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::NUMBER
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        NUMBER_SKIN_ATTRS
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("prefix", SkinValue::Text(prefix)) => self.prefix = (*prefix).to_string(),
            ("suffix", SkinValue::Text(suffix)) => self.suffix = (*suffix).to_string(),
            _ => {}
        }
    }

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

    fn preferred_height(&self, _width: f32) -> f32 {
        34.0
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover_zone = None;
            self.press_zone = None;
            self.close_edit();
        }
    }

    // --- painted session ---------------------------------------------------

    fn inline_edit(&self) -> bool {
        self.enabled
    }

    /// The stepper zones are click targets, not the editor — while typing too
    /// (they stay visible and a click commits the session before stepping).
    fn inline_edit_at(&self, pos: Point) -> bool {
        if !self.enabled {
            return false;
        }
        match self.stepper_rects() {
            Some((up, down)) => !up.contains_point(pos) && !down.contains_point(pos),
            None => true,
        }
    }

    fn editing(&self) -> bool {
        self.editing
    }

    fn begin_inline_edit(&mut self, position: Option<Point>) -> bool {
        self.begin_edit(position)
    }

    fn finish_edit(&mut self, commit: bool) -> Option<M> {
        NumberInput::finish_edit(self, commit)
    }

    fn insert_text(&mut self, text: &str) -> bool {
        if !self.editing {
            return false;
        }
        let negative = self.min < 0;
        self.field.get_mut().insert_filtered(text, |ch| {
            ch.is_ascii_digit() || ch == '.' || (negative && ch == '-')
        })
    }

    fn set_composition(&mut self, _text: &str, _cursor: usize) -> bool {
        // A number field accepts digits, a dot and (for a signed range) a
        // minus; IME composition would bypass that filter. Committed IME text
        // still arrives through `insert_text`.
        false
    }

    fn caret_rect(&self) -> Option<Rect> {
        if !self.editing {
            return None;
        }
        self.field.borrow().caret_rect()
    }

    fn edit_menu(&self) -> Option<Menu> {
        self.field.borrow().edit_menu()
    }

    fn edit_command(&mut self, command: u32) -> bool {
        self.field.get_mut().edit_command(command)
    }

    fn take_clipboard(&mut self) -> Option<String> {
        self.field.get_mut().take_clipboard()
    }

    fn commit_text(&mut self, text: String) -> Option<M> {
        if self.read_only {
            self.close_edit();
            return None;
        }
        self.close_edit();
        let parsed = self.parse_committed(&text);
        let next = parsed.unwrap_or(self.value);
        let changed = next.clamp(self.min, self.max) != self.value;
        self.set_value_clamped(next);
        changed.then(|| (self.on_change)(self.value))
    }

    fn set_number(&mut self, value: i64) {
        if self.editing {
            self.close_edit();
        }
        self.set_value_clamped(value);
    }

    fn focus_inset(&self) -> f32 {
        0.0
    }

    fn focus_ring_radius(&self) -> Option<f32> {
        self.surface.focus_radius()
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let pal = ctx.palette;
        let (border, background) = field_colors(&pal, self.enabled, state.focused);
        self.surface.frame(ctx.canvas, self.rect, border, background);
        let color = if self.enabled { pal.text } else { pal.text_disabled };
        let inner = self.rect.inset(9.0, 2.0);
        let mut base = ctx.painter.body().clone();
        if let Some(size) = self.font_size {
            base.size = size;
        }
        let style = ctx.style(&base);
        let mut text_right = inner.right;
        // The steppers stay visible while typing: the editor is only the text
        // area, so the arrows never cover the selection and the field width
        // never moves.
        let steppers = self.stepper_rects();
        if let Some((up, down)) = steppers {
            ctx.canvas.fill_rect(
                Rect::new(up.left, up.top, up.left + 1.0, down.bottom),
                pal.divider,
            );
            for (zone, kind, glyph) in [
                (up, StepperZone::Up, '\u{E70E}'),
                (down, StepperZone::Down, '\u{E70D}'),
            ] {
                let pressed = self.enabled && self.press_zone == Some(kind);
                let hovered = self.enabled && self.hover_zone == Some(kind);
                if pressed || hovered {
                    ctx.canvas.fill_rect(
                        zone,
                        if pressed {
                            pal.button_pressed
                        } else {
                            pal.button_hover
                        },
                    );
                }
                let glyph_color = if !self.enabled {
                    pal.text_disabled
                } else if pressed || hovered {
                    pal.text
                } else {
                    pal.muted
                };
                let mut buf = [0u16; 2];
                let text = String::from_utf16_lossy(&*glyph.encode_utf16(&mut buf));
                ctx.text(
                    zone,
                    &text,
                    TextStyleRole::Glyph,
                    TextAlign::Center,
                    glyph_color,
                );
            }
            text_right = up.left - 4.0;
        }
        if !self.suffix.is_empty() {
            let width = ctx
                .measure_text(&self.suffix, TextStyleRole::Small)
                .width;
            ctx.text(
                Rect::new(text_right - width, inner.top, text_right, inner.bottom),
                &self.suffix,
                TextStyleRole::Small,
                TextAlign::Right,
                pal.muted,
            );
            text_right = (text_right - width - 4.0).max(inner.left);
        }
        let mut text_left = inner.left;
        if !self.prefix.is_empty() {
            let width = ctx
                .measure_text(&self.prefix, TextStyleRole::Small)
                .width;
            ctx.text(
                Rect::new(inner.left, inner.top, inner.left + width, inner.bottom),
                &self.prefix,
                TextStyleRole::Small,
                TextAlign::Left,
                pal.muted,
            );
            text_left = (inner.left + width + 4.0).min(text_right);
        }
        let area = Rect::new(text_left, inner.top, text_right, inner.bottom);
        self.text_area.set(area);
        if self.editing {
            let mut field = self.field.borrow_mut();
            if field.rect() != area {
                field.set_rect(area);
            }
            field.paint(ctx, state);
        } else {
            ctx.text_styled(area, &self.display, &style, self.align, color);
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if !self.enabled {
            return None;
        }
        if self.read_only {
            // Focus, selection, copy and the caret stay; the steppers and
            // typed text are mutations and are refused.
            if self.editing {
                self.field.get_mut().input(input);
            }
            return None;
        }
        if self.editing {
            // The steppers stay clickable while typing: a click commits the
            // typed text first, then steps (the final value is what the
            // application sees). Every other pointer/key input belongs to the
            // open session.
            if let WidgetInput::ClickAt(pos) = input
                && let Some(zone) = self.zone_at(pos)
            {
                let _ = self.finish_edit(true);
                return self.adjust(match zone {
                    StepperZone::Up => 1,
                    StepperZone::Down => -1,
                });
            }
            self.field.get_mut().input(input);
            return None;
        }
        match input {
            WidgetInput::HoverAt(pos) => {
                self.hover_zone = self.zone_at(pos);
                None
            }
            WidgetInput::HoverLeave => {
                self.hover_zone = None;
                None
            }
            WidgetInput::PressAt(pos) => {
                self.press_zone = self.zone_at(pos);
                None
            }
            WidgetInput::ReleaseAt(_) | WidgetInput::DragEnd(_) => {
                self.press_zone = None;
                None
            }
            WidgetInput::ClickAt(pos) => match self.zone_at(pos) {
                Some(StepperZone::Up) => self.adjust(1),
                Some(StepperZone::Down) => self.adjust(-1),
                None => None,
            },
            _ => None,
        }
    }

    fn repaint_on_move(&self) -> bool {
        self.steppers
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::IBeam)
    }

    /// Stepper zones are buttons: arrow; the editable area keeps the I-beam.
    fn cursor_at(&self, pos: crate::geometry::Point) -> Option<Cursor> {
        if let Some((up, down)) = self.stepper_rects()
            && (up.contains_point(pos) || down.contains_point(pos))
        {
            return Some(Cursor::Arrow);
        }
        Some(Cursor::IBeam)
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let value = if self.editing {
            self.field.borrow().text().to_string()
        } else {
            self.display.clone()
        };
        Some(
            A11yInfo::new(A11yRole::NumberField, self.id.0)
                .value(value)
                .range(self.min as f64, self.max as f64, self.value as f64)
                .read_only(self.read_only || !self.enabled),
        )
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        self.field.get_mut().set_read_only(read_only);
        if read_only {
            self.hover_zone = None;
            self.press_zone = None;
        }
    }

    fn a11y_set_value(&mut self, text: &str) -> Option<M> {
        if !self.enabled || self.read_only {
            return None;
        }
        self.commit_text(text.to_string())
    }
}

/// Fixed-point display of a raw integer (`250, 2` → `2.50`).
fn format_fixed(value: i64, decimals: u8) -> String {
    if decimals == 0 {
        return value.to_string();
    }
    let decimals = decimals.min(9) as u32;
    let scale = 10i64.pow(decimals);
    let negative = value < 0;
    // i128 so `i64::MIN`'s magnitude is representable.
    let magnitude = (value as i128).unsigned_abs();
    let scale_i = scale as u128;
    let integer = magnitude / scale_i;
    let fraction = magnitude % scale_i;
    format!(
        "{}{integer}.{fraction:0width$}",
        if negative { "-" } else { "" },
        width = decimals as usize
    )
}

/// Parse a fixed-point text back to the raw integer. `2.75` with two decimals
/// is `275`; extra fraction digits are truncated, missing ones padded.
fn parse_fixed(text: &str, decimals: u8) -> Option<i64> {
    if decimals == 0 {
        return text.parse::<i64>().ok();
    }
    let decimals = decimals.min(9) as usize;
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (integer, fraction) = match digits.split_once('.') {
        Some((integer, fraction)) => (integer, fraction),
        None => (digits, ""),
    };
    if (integer.is_empty() && fraction.is_empty())
        || !integer.chars().all(|ch| ch.is_ascii_digit())
        || !fraction.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    let integer = if integer.is_empty() {
        0
    } else {
        integer.parse::<i64>().ok()?
    };
    let mut fraction: String = fraction.chars().take(decimals).collect();
    while fraction.len() < decimals {
        fraction.push('0');
    }
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i64>().ok()?
    };
    let scale = 10i64.checked_pow(decimals as u32)?;
    let value = integer
        .checked_mul(scale)?
        .checked_add(fraction)?;
    Some(if negative { -value } else { value })
}

surface_builders!(TextEdit);
surface_builders!(NumberInput);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;

    #[test]
    fn a_read_only_number_input_refuses_steppers_and_typing() {
        let mut field: NumberInput<&'static str> =
            NumberInput::new("n", 5, 0..=10, |_| "changed")
                .steppers()
                .read_only(true);
        assert!(field.is_read_only());
        // The session opens for selection/copy, but mutations are refused.
        assert!(field.begin_inline_edit(None));
        assert!(!field.insert_text("7"));
        assert_eq!(field.finish_edit(true), None);
        assert_eq!(field.value(), 5);
        // A stepper click is a user mutation too.
        let (up, _) = field.stepper_rects().expect("steppers");
        assert_eq!(field.input(WidgetInput::ClickAt(up.center())), None);
        assert_eq!(field.value(), 5);
        // The container's read-only property reaches the widget, and UIA
        // reports it.
        assert!(<NumberInput<&'static str> as Widget<&'static str>>::read_only(&field));
        assert!(field.a11y().expect("a11y").read_only);
        // Read-only is not disabled: the programmatic setter still works.
        field.set_number(7);
        assert_eq!(field.value(), 7);
    }

    #[derive(Default)]
    struct Recorder {
        texts: Vec<String>,
        text_rects: Vec<Rect>,
        fills: Vec<(Rect, Color)>,
    }

    impl crate::backend::Canvas for Recorder {
        fn scale(&self) -> f32 { 1.0 }
        fn dpi(&self) -> f32 { 96.0 }
        fn fill_rect(&mut self, rect: Rect, color: Color) {
            self.fills.push((rect, color));
        }
        fn text(
            &mut self,
            rect: Rect,
            text: &str,
            _style: &TextStyle,
            _align: TextAlign,
            _color: Color,
        ) {
            self.texts.push(text.to_string());
            self.text_rects.push(rect);
        }
        fn measure_text(&mut self, text: &str, _style: &TextStyle) -> crate::geometry::Size {
            crate::geometry::Size::new(text.chars().count() as f32 * 8.0, 16.0)
        }
        fn ellipse(&mut self, _rect: Rect, _color: Color) {}
        fn vertical_gradient(&mut self, _rect: Rect, _top: Color, _bottom: Color) {}
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

    fn paint_field(
        field: &TextEdit<String>,
        painter: &crate::painter::Painter,
        images: &crate::image::ImageStore,
        canvas: &mut Recorder,
    ) {
        paint_state(field, painter, images, canvas, false, true);
    }

    fn paint_state(
        field: &TextEdit<String>,
        painter: &crate::painter::Painter,
        images: &crate::image::ImageStore,
        canvas: &mut Recorder,
        focused: bool,
        caret_on: bool,
    ) {
        let mut paint = WidgetPaint {
            painter,
            canvas,
            palette: *painter.palette(),
            font: None,
            images,
            viewport: Rect::from_xywh(0.0, 0.0, 400.0, 300.0),
        dirty: &[],
        };
        field.paint(
            &mut paint,
            &WidgetState {
                enabled: true,
                focused,
                caret_on,
                ..Default::default()
            },
        );
    }

    fn key(code: KeyCode) -> WidgetInput {
        WidgetInput::Key(KeyEvent {
            code,
            vk: 0,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        })
    }

    fn key_mod(code: KeyCode, ctrl: bool, shift: bool) -> WidgetInput {
        WidgetInput::Key(KeyEvent {
            code,
            vk: 0,
            ctrl,
            shift,
            alt: false,
            repeat: false,
        })
    }

    fn painted(text: &str) -> TextEdit<String> {
        let mut field: TextEdit<String> = TextEdit::new("field", text, |text| text).painted();
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        field
    }

    fn layout(field: &mut TextEdit<String>) {
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = Recorder::default();
        paint_field(field, &painter, &images, &mut canvas);
    }

    #[test]
    fn a_long_press_picks_a_word_and_paints_selection_handles() {
        let mut field = painted("alpha beta");
        layout(&mut field);
        field.begin_edit_impl(None);
        assert!(field.long_press(Point::new(20.0, 17.0)));
        assert_eq!(field.selection(), Some((0, 5)), "the word under the finger");
        assert!(field.touch_handles);

        // The start handle hangs above the line, the end handle below it, and
        // both circles stay inside the widget (the repaint rectangle).
        let (start_anchor, start_center) = field.handle_geometry(SelectionEdge::Start).unwrap();
        let (end_anchor, end_center) = field.handle_geometry(SelectionEdge::End).unwrap();
        assert!(start_center.y < start_anchor.y);
        assert!(end_center.y > end_anchor.y);
        for point in [start_center, end_center] {
            assert!(field.rect.contains_point(point), "{point:?} outside the field");
        }

        // Painting the handles adds strokes the plain field does not have.
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut without = Recorder::default();
        field.touch_handles = false;
        paint_field(&field, &painter, &images, &mut without);
        let mut with = Recorder::default();
        field.touch_handles = true;
        paint_field(&field, &painter, &images, &mut with);
        assert!(
            with.fills.len() >= without.fills.len() + 2,
            "the two handles paint ({} vs {} fills)",
            with.fills.len(),
            without.fills.len()
        );
    }

    #[test]
    fn a_handle_drag_moves_that_edge_only() {
        let mut field = painted("alpha beta");
        layout(&mut field);
        field.begin_edit_impl(None);
        field.long_press(Point::new(20.0, 17.0));
        assert_eq!(field.selection(), Some((0, 5)));

        // Grabbing the end handle and dragging left shrinks the selection
        // from the end; the anchor (start) stays put.
        let (_, end_center) = field.handle_geometry(SelectionEdge::End).unwrap();
        assert!(field.grab_selection_handle(end_center));
        field.input(WidgetInput::DragTo(Point::new(20.0, 17.0)));
        assert_eq!(field.selection(), Some((0, 1)));
        field.input(WidgetInput::ReleaseAt(end_center));
        assert!(field.handle_drag.is_none());

        // The start handle moves the other edge.
        let (_, start_center) = field.handle_geometry(SelectionEdge::Start).unwrap();
        assert!(field.grab_selection_handle(start_center));
        field.input(WidgetInput::DragTo(Point::new(28.0, 17.0)));
        let (start, end) = field.selection().unwrap();
        assert!(start > 0 && end > start);
    }

    #[test]
    fn dropped_text_opens_a_session_and_inserts_at_the_point() {
        let mut field = painted("abcdef");
        layout(&mut field);
        assert!(!field.editing);
        assert!(field.insert_dropped_text("XY", Point::new(20.0, 17.0)));
        assert!(field.editing, "a drop is a text session");
        assert_eq!(field.text, "aXYbcdef", "inserted where the drop landed");
        assert_eq!(field.caret, 3);
        // A read-only field refuses the drop.
        let mut locked = painted("abcdef");
        layout(&mut locked);
        locked.read_only = true;
        assert!(!locked.insert_dropped_text("XY", Point::new(20.0, 17.0)));
        assert_eq!(locked.text, "abcdef");
    }

    #[test]
    fn a_selection_can_be_dragged_out_but_never_a_touch_handle_drag() {
        let mut field = painted("alpha beta");
        layout(&mut field);
        field.begin_edit_impl(None);
        field.select_all();
        assert_eq!(
            field.take_drag_text(Point::new(20.0, 17.0)),
            Some("alpha beta".to_string()),
            "a drag copies the selection"
        );
        assert_eq!(field.text, "alpha beta", "the field keeps its text");

        // A touch handle is a selection adjustment, not a drag-out.
        field.long_press(Point::new(20.0, 17.0));
        let (_, center) = field.handle_geometry(SelectionEdge::End).unwrap();
        assert!(field.grab_selection_handle(center));
        assert_eq!(field.take_drag_text(Point::new(20.0, 17.0)), None);

        // A password field never hands its selection out, like the clipboard.
        let mut secret = TextEdit::new("secret", "hunter2", |text| text)
            .painted()
            .password();
        secret.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        layout(&mut secret);
        secret.begin_edit_impl(None);
        secret.select_all();
        assert_eq!(secret.take_drag_text(Point::new(20.0, 17.0)), None);
    }

    #[test]
    fn a_long_single_line_keeps_the_caret_visible_and_persists_the_scroll() {
        let mut field: TextEdit<String> = TextEdit::new("field", "", |text| text)
            .painted()
            .font_size(14.0);
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        field.begin_edit_impl(None);
        for _ in 0..40 {
            field.input(WidgetInput::Char('m'));
        }
        layout(&mut field);
        let content = field.content_rect();
        let caret = field.caret_rect().expect("the end caret must be painted");
        assert!(field.scroll_x.get() > 0.0, "the line must scroll to the caret");
        assert!(
            caret.right <= content.right + 0.5 && caret.left >= content.left,
            "the caret at the end must stay inside the content: {caret:?} in {content:?}"
        );

        // A repaint without a reveal (caret blink, hover) keeps the same
        // offset and the caret in place instead of jumping to the start.
        let before = (caret, field.scroll_x.get());
        layout(&mut field);
        assert_eq!(field.caret_rect(), Some(before.0));
        assert_eq!(field.scroll_x.get(), before.1);
    }

    #[test]
    fn reaching_max_length_does_not_jump_back_to_the_start() {
        let mut field: TextEdit<String> = TextEdit::new("field", "", |text| text)
            .painted()
            .max_length(20);
        field.set_rect(Rect::from_xywh(0.0, 0.0, 120.0, 34.0));
        field.begin_edit_impl(None);
        for _ in 0..20 {
            field.input(WidgetInput::Char('m'));
        }
        layout(&mut field);
        let caret = field.caret_rect().expect("caret at the limit");
        let scroll = field.scroll_x.get();
        assert!(scroll > 0.0);

        // Extra keystrokes are rejected at the limit; the next paint must not
        // reset the offset (this used to render from x = 0, so the text looked
        // like it jumped to the front).
        for _ in 0..3 {
            field.input(WidgetInput::Char('x'));
            layout(&mut field);
        }
        assert_eq!(field.text.chars().count(), 20);
        assert_eq!(field.scroll_x.get(), scroll);
        assert_eq!(field.caret_rect(), Some(caret));
    }

    #[test]
    fn multiline_caret_reveal_persists_between_paints() {
        let mut field: TextEdit<String> = TextEdit::new("note", "", |text| text)
            .painted()
            .multiline();
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 60.0));
        field.begin_edit_impl(None);
        for _ in 0..8 {
            field.insert_impl("\n", true);
        }
        layout(&mut field);
        let scrolled = field.scroll_y.get();
        assert!(scrolled > 0.0, "the view follows the caret down");
        layout(&mut field);
        assert_eq!(field.scroll_y.get(), scrolled, "a blink keeps the scroll offset");
    }

    #[test]
    fn multiline_request_carries_wrap_and_scrollbars() {
        let field = TextEdit::new("note", "x", |text| text)
            .multiline()
            .wrap(false)
            .scrollbars(ScrollBars::Both);
        let request = field.edit_request().unwrap();
        assert!(request.multiline);
        assert!(!request.wrap);
        assert_eq!(request.scrollbars, ScrollBars::Both);
        assert!(request.scrollbars.vertical() && request.scrollbars.horizontal());
        let plain = TextEdit::new("plain", "x", |text| text).multiline();
        let request = plain.edit_request().unwrap();
        assert!(request.wrap && request.scrollbars == ScrollBars::Vertical);
    }

    #[test]
    fn closed_multiline_paint_draws_hard_lines_and_scrolls() {
        let mut field: TextEdit<String> =
            TextEdit::new("note", "a\nb\nc\nd\ne", |text| text).multiline();
        field.set_rect(Rect::from_xywh(0.0, 0.0, 220.0, 64.0));
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = Recorder::default();
        paint_field(&field, &painter, &images, &mut canvas);
        // Hard lines are painted separately, not joined into one run.
        assert!(canvas.texts.contains(&"a".to_string()));
        assert!(canvas.texts.contains(&"b".to_string()));
        assert!(!canvas.texts.iter().any(|line| line.contains("a\nb")));
        // More lines than fit: the vertical scrollbar thumb is painted.
        assert!(
            canvas
                .fills
                .iter()
                .any(|(_, color)| *color == painter.palette().scroll_thumb),
            "an overflowing multiline field must show a scrollbar"
        );
        // The wheel scrolls the closed view. A notch is capped at a quarter of
        // the range (so a short field needs a few notches and the bar does not
        // jump from the top to the bottom in one event).
        for _ in 0..6 {
            field.input(WidgetInput::Wheel(-1.0));
        }
        assert!(field.scroll_y.get() > 0.0);
        let mut canvas = Recorder::default();
        paint_field(&field, &painter, &images, &mut canvas);
        assert!(canvas.texts.contains(&"c".to_string()));
        assert!(!canvas.texts.contains(&"a".to_string()));
    }

    #[test]
    fn closed_multiline_wraps_when_enabled() {
        let long = "abcdefghij klmnopqrst uvwxyz";
        let mut field: TextEdit<String> =
            TextEdit::new("note", long, |text| text).multiline().wrap(true);
        field.set_rect(Rect::from_xywh(0.0, 0.0, 100.0, 90.0));
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = Recorder::default();
        paint_field(&field, &painter, &images, &mut canvas);
        assert!(canvas.texts.len() >= 2, "wrapped text must use several lines");
        assert!(!canvas.texts.iter().any(|line| line == long));
    }

    #[test]
    fn text_edit_request_carries_text_and_cue() {
        let field = TextEdit::new("name", "Bamboo", |text| text).cue("名称");
        let request = field.edit_request().expect("field is editable");
        assert_eq!(request.id, WidgetId::new("name"));
        assert_eq!(request.text, "Bamboo");
        assert_eq!(request.cue, "名称");
        assert!(!request.password);
    }

    #[test]
    fn commit_normalizes_crlf_from_the_native_control() {
        let mut field: TextEdit<String> = TextEdit::new("note", "a\nb", |text| text).multiline();
        // The classic EDIT control returns CRLF; it must compare equal to the
        // stored LF text so no spurious message is emitted.
        assert_eq!(field.commit_text("a\r\nb".to_string()), None);
        assert_eq!(field.edit_request().unwrap().text, "a\nb");
        assert_eq!(
            field.commit_text("a\r\nc".to_string()),
            Some("a\nc".to_string())
        );
        assert_eq!(field.edit_request().unwrap().text, "a\nc");
        // Programmatic insertion is normalized too.
        field.insert_text("x\r\ny");
        assert!(!field.edit_request().unwrap().text.contains('\r'));
    }

    #[test]
    fn text_edit_commit_stores_and_emits() {
        let mut field = TextEdit::new("name", "old", |text| text);
        assert_eq!(field.commit_text("new".to_string()), Some("new".to_string()));
        assert_eq!(field.edit_request().unwrap().text, "new");
        // Clicking in and out without typing must not emit a message.
        assert_eq!(field.commit_text("new".to_string()), None);
    }

    #[test]
    fn password_fields_mask_the_painted_text() {
        let mut field = TextEdit::new("secret", "", |text| text).password();
        field.commit_text("abcd".to_string());
        assert_eq!(field.shown().chars().count(), 4);
        assert!(!field.shown().contains('a'));
        // The native request still carries the real value.
        assert_eq!(field.edit_request().unwrap().text, "abcd");
    }

    #[test]
    fn number_input_clamps_and_normalizes() {
        let mut field = NumberInput::new("count", 5, 1..=10, |value| value);
        assert_eq!(field.commit_text("99".to_string()), Some(10));
        assert_eq!(field.display, "10");
        // Text that does not parse keeps the previous value and emits nothing.
        assert_eq!(field.commit_text("abc".to_string()), None);
        assert_eq!(field.display, "10");
        // Re-entering the same value normalizes but does not emit.
        assert_eq!(field.commit_text("010".to_string()), None);
        assert_eq!(field.display, "10");
    }

    #[test]
    fn number_input_formats_and_parses_fixed_decimals() {
        let mut price = NumberInput::new("price", 275, 0..=100_000, |value| value)
            .decimals(2)
            .prefix("¥");
        assert_eq!(price.display(), "2.75");
        // Editing uses the same text; extra digits truncate, short digits pad.
        assert_eq!(price.commit_text("2.7".to_string()), Some(270));
        assert_eq!(price.display(), "2.70");
        assert_eq!(price.commit_text(".5".to_string()), Some(50));
        assert_eq!(price.display(), "0.50");
        assert_eq!(price.commit_text("-1".to_string()), Some(0), "clamped");
        assert_eq!(price.commit_text("x".to_string()), None);
        // A custom format/parse pair round-trips.
        let mut custom = NumberInput::new("k", 1500, 0..=999_999, |value| value)
            .format(|value| format!("{} k", value / 1000))
            .parse(|text| text.trim_end_matches(" k").parse::<i64>().ok().map(|k| k * 1000));
        assert_eq!(custom.display(), "1 k");
        assert_eq!(custom.commit_text("3 k".to_string()), Some(3000));
        assert_eq!(custom.display(), "3 k");
    }

    #[test]
    fn fixed_point_helpers_handle_signs_and_overflow_boundaries() {
        assert_eq!(format_fixed(0, 2), "0.00");
        assert_eq!(format_fixed(-5, 3), "-0.005");
        assert_eq!(format_fixed(i64::MIN, 1), "-922337203685477580.8");
        assert_eq!(parse_fixed("2.75", 2), Some(275));
        assert_eq!(parse_fixed("+2.5", 2), Some(250));
        assert_eq!(parse_fixed("-0.05", 2), Some(-5));
        assert_eq!(parse_fixed("", 2), None);
        assert_eq!(parse_fixed("1.2.3", 2), None);
        assert_eq!(parse_fixed(" 3", 2), None, "the commit path trims first");
        assert_eq!(parse_fixed("9223372036854775808", 2), None, "overflow");
        assert_eq!(parse_fixed("7", 0), Some(7));
    }

    #[test]
    fn the_painted_number_session_parses_commits_and_cancels() {
        let mut field = NumberInput::new("delay", 250, 0..=999, |value| value).decimals(2);
        field.set_rect(Rect::from_xywh(0.0, 0.0, 120.0, 34.0));
        assert_eq!(field.display(), "2.50");
        assert!(field.begin_inline_edit(None));
        assert!(field.editing());
        assert!(field.insert_text("2.75x"));
        assert_eq!(field.field.borrow().text(), "2.75");
        assert_eq!(field.finish_edit(true), Some(275));
        assert_eq!(field.value(), 275);
        assert_eq!(field.display(), "2.75");
        assert!(!field.editing());
        // A cancel keeps the committed value; an unparseable draft keeps it
        // too (the parse failure falls back to the current value).
        assert!(field.begin_inline_edit(None));
        assert!(field.insert_text("9"));
        assert_eq!(field.finish_edit(false), None);
        assert_eq!(field.display(), "2.75");
        assert!(field.begin_inline_edit(None));
        assert!(field.insert_text("."));
        assert_eq!(field.finish_edit(true), None);
        assert_eq!(field.value(), 275);
        // The steppers keep working once the session closes.
        let mut field = field.steppers();
        let (up, _) = field.stepper_rects().expect("steppers on");
        assert_eq!(field.input(WidgetInput::ClickAt(up.center())), Some(276));
    }

    #[test]
    fn the_steppers_stay_live_while_the_number_is_edited() {
        let mut field = NumberInput::new("count", 5, 1..=10, |value| value).steppers();
        field.set_rect(Rect::from_xywh(0.0, 0.0, 120.0, 34.0));
        let (up, _) = field.stepper_rects().expect("steppers on");
        assert!(!field.inline_edit_at(up.center()));
        assert!(field.begin_inline_edit(None));
        assert!(field.editing());
        // The arrows stay visible and clickable while typing; the editor only
        // covers the text area, so they never sit over the selection.
        assert!(!field.inline_edit_at(up.center()));
        let area = field.text_area.get();
        assert!(area.right <= up.left + 0.01, "the editor stops before the steppers");
        // A click on the strip commits the typed text, then steps.
        assert!(field.insert_text("8"));
        assert_eq!(field.input(WidgetInput::ClickAt(up.center())), Some(9));
        assert!(!field.editing());
        assert_eq!(field.value(), 9);
    }

    #[test]
    fn the_numeric_filter_allows_a_minus_only_for_a_signed_range() {
        let mut signed = NumberInput::new("offset", -3, -10..=10, |value| value);
        assert!(signed.begin_inline_edit(None));
        assert!(signed.insert_text("-1x.5"));
        assert_eq!(signed.field.borrow().text(), "-1.5");
        signed.finish_edit(false);
        let mut unsigned = NumberInput::new("count", 3, 0..=10, |value| value);
        assert!(unsigned.begin_inline_edit(None));
        assert!(unsigned.insert_text("-4"));
        assert_eq!(unsigned.field.borrow().text(), "4");
        unsigned.finish_edit(false);
    }

    #[test]
    fn stepper_zones_adjust_by_the_step_and_keep_the_editor_out() {
        let mut field = NumberInput::new("count", 5, 1..=10, |value| value)
            .step_size(2)
            .steppers();
        field.set_rect(Rect::from_xywh(0.0, 0.0, 120.0, 34.0));
        let (up, down) = field.stepper_rects().expect("steppers are on");
        // A press in a stepper zone must not open the editor.
        assert!(!field.inline_edit_at(up.center()));
        assert!(field.inline_edit_at(Rect::new(0.0, 0.0, 40.0, 34.0).center()));
        // Up steps by `step_size`, down clamps at the minimum.
        assert_eq!(field.input(WidgetInput::ClickAt(up.center())), Some(7));
        assert_eq!(field.input(WidgetInput::ClickAt(up.center())), Some(9));
        assert_eq!(field.input(WidgetInput::ClickAt(up.center())), Some(10));
        assert_eq!(field.input(WidgetInput::ClickAt(up.center())), None);
        assert_eq!(field.input(WidgetInput::ClickAt(down.center())), Some(8));
        // A click in the editable area emits nothing.
        assert_eq!(
            field.input(WidgetInput::ClickAt(Rect::new(0.0, 0.0, 40.0, 34.0).center())),
            None
        );
        // Without the option there are no zones and every press edits.
        let field = NumberInput::new("plain", 5, 1..=10, |value| value);
        assert!(field.stepper_rects().is_none());
        assert!(field.inline_edit_at(Rect::new(0.0, 0.0, 120.0, 34.0).center()));
    }

    #[test]
    fn prefix_and_suffix_are_stored_for_the_paint_and_a11y_paths() {
        let field = NumberInput::new("delay", 3, 0..=30, |value| value)
            .prefix("约 ")
            .suffix(" 秒");
        assert_eq!(field.prefix, "约 ");
        assert_eq!(field.suffix, " 秒");
    }

    #[test]
    fn steppers_use_an_arrow_cursor_and_the_text_an_ibeam() {
        let mut field = NumberInput::new("n", 5, 1..=9, |value| value).steppers();
        field.set_rect(Rect::from_xywh(0.0, 0.0, 120.0, 34.0));
        let (up, down) = field.stepper_rects().expect("steppers are on");
        // The arrows keep painting while a session is open; they are always
        // arrow-cursor buttons, never text.
        assert_eq!(field.cursor_at(up.center()), Some(Cursor::Arrow));
        assert_eq!(field.cursor_at(down.center()), Some(Cursor::Arrow));
        // Per-zone hover and press feedback drives the paint.
        assert!(field.repaint_on_move());
        field.input(WidgetInput::HoverAt(up.center()));
        assert_eq!(field.hover_zone, Some(StepperZone::Up));
        field.input(WidgetInput::HoverAt(crate::geometry::Point::new(30.0, 17.0)));
        assert_eq!(field.hover_zone, None);
        field.input(WidgetInput::HoverLeave);
        field.input(WidgetInput::PressAt(down.center()));
        assert_eq!(field.press_zone, Some(StepperZone::Down));
        field.input(WidgetInput::ReleaseAt(down.center()));
        assert_eq!(field.press_zone, None);
        field.set_enabled(false);
        field.input(WidgetInput::HoverAt(up.center()));
        assert_eq!(field.hover_zone, None);
        field.set_enabled(true);
        assert_eq!(
            field.cursor_at(crate::geometry::Point::new(30.0, 17.0)),
            Some(Cursor::IBeam)
        );
        // Without the option the whole field is editable.
        let mut plain = NumberInput::new("n", 5, 1..=9, |value| value);
        plain.set_rect(Rect::from_xywh(0.0, 0.0, 120.0, 34.0));
        assert_eq!(
            plain.cursor_at(crate::geometry::Point::new(110.0, 17.0)),
            Some(Cursor::IBeam)
        );
    }

    #[test]
    fn text_field_alignment_reaches_the_native_request() {
        let field = TextEdit::new("name", "abc", |text| text).align(TextAlign::Center);
        assert_eq!(field.edit_request().unwrap().align, TextAlign::Center);
        let field = TextEdit::new("name", "abc", |text| text);
        assert_eq!(field.edit_request().unwrap().align, TextAlign::Left);
    }

    #[test]
    fn caret_starts_at_the_end_and_select_all_insert_replaces() {
        let mut field = TextEdit::new("name", "Bamboo", |text| text);
        assert_eq!(field.caret(), 6);
        assert_eq!(field.selection(), None);
        assert!(field.select_all());
        assert_eq!(field.selection(), Some((0, 6)));
        assert!(field.insert_text("竹"));
        assert_eq!(field.text(), "竹");
        assert_eq!(field.caret(), 1);
        assert_eq!(field.selection(), None);
    }

    #[test]
    fn insert_replaces_a_partial_selection_and_clamps_offsets() {
        let mut field = TextEdit::new("name", "Bamboo", |text| text);
        // Select "mboo" backwards (anchor after the caret) and replace it.
        assert!(field.set_selection(6, 2));
        assert_eq!(field.selection(), Some((2, 6)));
        assert!(field.insert_text("X"));
        assert_eq!(field.text(), "BaX");
        assert_eq!(field.caret(), 3);

        // Out-of-range offsets clamp instead of panicking.
        assert!(field.set_caret(0));
        assert!(field.set_caret(99));
        assert_eq!(field.caret(), 3);
        assert!(!field.set_caret(3));
        // A collapsed selection is a caret, not an empty selection.
        assert!(field.set_selection(1, 1));
        assert_eq!(field.selection(), None);
        assert_eq!(field.caret(), 1);
    }

    #[test]
    fn commit_resets_the_caret_and_selection() {
        let mut field = TextEdit::new("name", "old", |text| text);
        field.select_all();
        assert_eq!(field.commit_text("新内容".to_string()), Some("新内容".to_string()));
        assert_eq!(field.caret(), 3);
        assert_eq!(field.selection(), None);
        assert_eq!(field.a11y().unwrap().text_selection, Some((9, 9)));
    }

    #[test]
    fn closed_field_selection_uses_byte_offsets_into_the_value() {
        let mut field = TextEdit::new("name", "竹韵", |text| text);
        assert_eq!(field.a11y().unwrap().text_selection, Some((6, 6)));
        field.set_selection(0, 1);
        assert_eq!(field.a11y().unwrap().text_selection, Some((0, 3)));
    }

    #[test]
    fn password_selection_indexes_the_mask() {
        let mut field = TextEdit::new("secret", "abcd", |text| text).password();
        field.set_selection(0, 2);
        let info = field.a11y().unwrap();
        // Two bullets, three bytes each, and no secret character leaks.
        assert_eq!(info.text_selection, Some((0, 6)));
        assert_eq!(info.value.as_deref(), Some("••••"));
    }

    #[test]
    fn closed_caret_hides_while_the_native_editor_is_open() {
        let mut field = TextEdit::new("name", "abc", |text| text);
        let keyboard_focus = WidgetState {
            focused: true,
            focus_visible: true,
            enabled: true,
            ..Default::default()
        };
        assert!(field.shows_closed_caret(&keyboard_focus));
        // Pointer focus never paints it (right-click after clicking in).
        let pointer_focus = WidgetState {
            focus_visible: false,
            ..keyboard_focus
        };
        assert!(!field.shows_closed_caret(&pointer_focus));
        let editing = WidgetState {
            editing: true,
            ..keyboard_focus
        };
        assert!(!field.shows_closed_caret(&editing));
        field.set_enabled(false);
        assert!(!field.shows_closed_caret(&keyboard_focus));
    }

    #[test]
    fn painted_fields_edit_in_place_and_commit_or_revert() {
        let mut field = painted("ab");
        assert!(field.inline_edit());
        assert!(!field.wants_edit(), "no native child for a painted field");
        assert!(field.edit_request().is_none());
        layout(&mut field);
        assert!(field.begin_inline_edit(None));
        assert!(field.editing());
        field.insert_impl("c", true);
        assert_eq!(field.text(), "abc");
        assert_eq!(field.finish_edit_impl(true), Some("abc".to_string()));
        assert!(!field.editing());

        // Escape/cancel: the value returns to what it was when editing began.
        assert!(field.begin_inline_edit(None));
        field.insert_impl("!", true);
        assert_eq!(field.text(), "abc!");
        assert_eq!(field.finish_edit_impl(false), None);
        assert_eq!(field.text(), "abc");
        assert!(!field.editing());
    }

    #[test]
    fn painted_keyboard_moves_selects_and_deletes() {
        let mut field = painted("hello");
        layout(&mut field);
        assert!(field.begin_inline_edit(None));
        field.input(key(KeyCode::Home));
        assert_eq!(field.caret(), 0);
        field.input(key_mod(KeyCode::Right, false, true));
        assert_eq!(field.selection(), Some((0, 1)));
        // Typing replaces the selection.
        field.input(WidgetInput::Char('H'));
        assert_eq!(field.text(), "Hello");
        assert_eq!(field.caret(), 1);
        // Backspace deletes the previous character, not the field.
        field.input(key(KeyCode::Backspace));
        assert_eq!(field.text(), "ello");
        field.input(key(KeyCode::End));
        assert_eq!(field.caret(), 4);
        field.input(key_mod(KeyCode::Left, true, false));
        assert_eq!(field.caret(), 0);
        // Ctrl+A then Delete clears the selection.
        field.input(key_mod(KeyCode::Letter(0), true, false));
        assert_eq!(field.selection(), Some((0, 4)));
        field.input(key(KeyCode::Delete));
        assert_eq!(field.text(), "");
    }

    #[test]
    fn painted_undo_coalesces_typing_and_redo_restores_it() {
        let mut field = painted("");
        layout(&mut field);
        assert!(field.begin_inline_edit(None));
        field.input(WidgetInput::Char('a'));
        field.input(WidgetInput::Char('b'));
        field.input(WidgetInput::Char('c'));
        assert_eq!(field.text(), "abc");
        field.input(key_mod(KeyCode::Letter(25), true, false)); // Ctrl+Z
        assert_eq!(field.text(), "");
        field.input(key_mod(KeyCode::Letter(24), true, false)); // Ctrl+Y
        assert_eq!(field.text(), "abc");
        // A separate deletion run undoes on its own.
        field.input(key(KeyCode::Backspace));
        assert_eq!(field.text(), "ab");
        field.input(key_mod(KeyCode::Letter(25), true, false));
        assert_eq!(field.text(), "abc");
        field.input(key_mod(KeyCode::Letter(25), true, false));
        assert_eq!(field.text(), "");
    }

    #[test]
    fn painted_copy_cut_and_delete_use_the_selection() {
        let mut field = painted("hello");
        layout(&mut field);
        assert!(field.begin_inline_edit(None));
        field.select_all();
        assert!(field.edit_command(edit_command::COPY));
        assert_eq!(field.take_clipboard().as_deref(), Some("hello"));
        assert_eq!(field.text(), "hello");
        assert!(field.edit_command(edit_command::CUT));
        assert_eq!(field.take_clipboard().as_deref(), Some("hello"));
        assert_eq!(field.text(), "");
        field.insert_impl("world", true);
        field.select_all();
        assert!(field.edit_command(edit_command::DELETE));
        assert_eq!(field.text(), "");
    }

    #[test]
    fn painted_edit_menu_matches_the_native_shape() {
        let mut field = painted("hello");
        layout(&mut field);
        let menu = field.edit_menu().expect("a painted field has a menu");
        let labels: Vec<String> = menu
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item(item) => Some(item.label().to_string()),
                MenuEntry::Submenu(sub) => Some(sub.label().to_string()),
                MenuEntry::Separator => None,
            })
            .collect();
        assert_eq!(
            labels,
            vec![
                "撤销(&U)".to_string(),
                "重做(&R)".to_string(),
                "剪切(&T)".to_string(),
                "复制(&C)".to_string(),
                "粘贴(&P)".to_string(),
                "删除(&D)".to_string(),
                "全选(&A)".to_string(),
                "插入 Unicode 控制字符(&I)".to_string(),
            ]
        );
        // Without a selection cut/copy/delete are disabled; select-all is not.
        assert!(menu.command(edit_command::CUT).is_some_and(|item| item.is_disabled()));
        assert!(menu.command(edit_command::SELECT_ALL).is_some_and(|item| !item.is_disabled()));
    }

    #[test]
    fn unicode_control_commands_insert_their_character() {
        let mut field = painted("ab");
        layout(&mut field);
        assert!(field.begin_inline_edit(None));
        field.set_caret(1);
        assert!(field.edit_command(edit_command::INSERT_LRM));
        assert!(field.text().contains('\u{200E}'));
        assert_eq!(field.caret, 2);
        assert!(field.edit_command(edit_command::INSERT_ZWNJ));
        assert!(field.text().contains('\u{200C}'));
        assert_eq!(field.text().chars().count(), 4);
    }

    #[test]
    fn painted_max_length_and_single_line_filtering() {
        let mut field: TextEdit<String> =
            TextEdit::new("field", "", |text| text).painted().max_length(3);
        layout(&mut field);
        assert!(field.begin_inline_edit(None));
        field.insert_impl("abcdef", true);
        assert_eq!(field.text(), "abc");
        // Replacing a selection frees exactly the selected character count.
        field.set_selection(0, 2);
        field.insert_impl("xy", true);
        assert_eq!(field.text(), "xyc");
        // A pasted line break never enters a single-line field.
        field.insert_impl("\nz", true);
        assert!(!field.text().contains('\n'));
        assert_eq!(field.text(), "xyc");
    }

    #[test]
    fn painted_multiline_vertical_movement_keeps_the_goal_column() {
        let mut field: TextEdit<String> =
            TextEdit::new("field", "abc\ndef\nghi", |text| text)
                .painted()
                .multiline();
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 120.0));
        layout(&mut field);
        assert!(field.begin_inline_edit(None));
        field.set_caret(3);
        field.input(key(KeyCode::Down));
        assert_eq!(field.caret(), 7, "same x on the second line");
        field.input(key(KeyCode::Down));
        assert_eq!(field.caret(), 11);
        field.input(key(KeyCode::Up));
        assert_eq!(field.caret(), 7);
        // Home/End stay on the visual line.
        field.input(key(KeyCode::Home));
        assert_eq!(field.caret(), 4);
        field.input(key(KeyCode::End));
        assert_eq!(field.caret(), 7);
    }

    #[test]
    fn painted_ime_composition_is_visible_and_commits() {
        let mut field = painted("ab");
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = Recorder::default();
        paint_field(&field, &painter, &images, &mut canvas);
        assert!(field.begin_inline_edit(None));
        assert!(field.set_composition("にほ", 1));
        assert_eq!(field.composition(), "にほ");
        let mut canvas = Recorder::default();
        paint_state(&field, &painter, &images, &mut canvas, true, true);
        assert!(canvas.texts.iter().any(|text| text == "にほ"));
        // The final IME result is inserted and the composition clears.
        field.insert_impl("日", true);
        assert!(field.set_composition("", 0));
        assert_eq!(field.text(), "ab日");
        assert!(field.composition().is_empty());
    }

    #[test]
    fn painted_colors_drive_selection_caret_and_background() {
        let text_color = Color::new(20, 22, 26);
        let background = Color::new(255, 252, 240);
        let selection_text = Color::new(250, 250, 250);
        let selection_background = Color::new(64, 120, 220);
        let caret_color = Color::new(220, 40, 40);
        let mut field: TextEdit<String> = TextEdit::new("field", "abc", |text| text)
            .painted()
            .colors(
                EditColors::new()
                    .text(text_color)
                    .background(background)
                    .selection(selection_text, selection_background)
                    .caret(caret_color),
            );
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = Recorder::default();
        paint_field(&field, &painter, &images, &mut canvas);
        assert!(field.begin_inline_edit(None));
        field.set_selection(0, 2);
        let mut canvas = Recorder::default();
        paint_state(&field, &painter, &images, &mut canvas, true, true);
        let colors: Vec<Color> = canvas.fills.iter().map(|(_, color)| *color).collect();
        // An open session tints the configured background toward the active
        // border so editing is visible even without the blink phase.
        let tinted = background.mix(painter.palette().accent, 0.07);
        assert!(colors.contains(&tinted));
        assert!(colors.contains(&selection_background));
        assert!(colors.contains(&caret_color));
    }

    #[test]
    fn painted_caret_hides_on_the_off_blink_phase() {
        let mut field = painted("abc");
        layout(&mut field);
        assert!(field.begin_inline_edit(None));
        field.set_caret(1);
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut on = Recorder::default();
        paint_state(&field, &painter, &images, &mut on, true, true);
        let mut off = Recorder::default();
        paint_state(&field, &painter, &images, &mut off, true, false);
        let caret_color = *painter.palette();
        let caret = caret_color.text;
        assert!(on.fills.iter().any(|(_, color)| *color == caret));
        assert!(!off.fills.iter().any(|(_, color)| *color == caret));
        // The caret rectangle is reported even on the off phase (IME/blink
        // bookkeeping needs it).
        assert!(field.caret_rect().is_some());
    }

    #[test]
    fn painted_frame_shows_whether_the_session_is_open() {
        let active_border = Color::new(220, 120, 40);
        let idle_border = Color::new(190, 180, 160);
        let caret = Color::new(200, 20, 20);
        let mut field: TextEdit<String> = TextEdit::new("field", "abc", |text| text)
            .painted()
            .colors(
                EditColors::new()
                    .border(active_border)
                    .border_idle(idle_border)
                    .caret(caret),
            );
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();

        // Focused but closed: the idle frame, no caret.
        let mut closed = Recorder::default();
        paint_state(&field, &painter, &images, &mut closed, true, true);
        let closed_colors: Vec<Color> = closed.fills.iter().map(|(_, color)| *color).collect();
        assert!(closed_colors.contains(&idle_border));
        assert!(!closed_colors.contains(&active_border));
        assert!(!closed_colors.contains(&caret));

        // Editing: the active frame (outer fill plus an inner ring), a tinted
        // background and the caret.
        assert!(field.begin_inline_edit(None));
        let mut editing = Recorder::default();
        paint_state(&field, &painter, &images, &mut editing, true, true);
        let editing_colors: Vec<Color> = editing.fills.iter().map(|(_, color)| *color).collect();
        assert!(editing_colors.contains(&active_border));
        assert!(editing_colors.contains(&caret));
        assert!(!editing_colors.contains(&idle_border));
    }

    #[test]
    fn painted_scrollbar_drag_scrolls_and_repaints() {
        let mut field: TextEdit<String> = TextEdit::new(
            "note",
            "0\n1\n2\n3\n4\n5\n6\n7\n8\n9",
            |text| text,
        )
        .painted()
        .multiline();
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 100.0));
        layout(&mut field);
        assert!(field.active_bars().0, "ten lines overflow a 100 DIP field");
        let track = field.v_track();
        let x = track.center().x;
        // A press below the thumb jumps the list and arms the drag.
        field.input(WidgetInput::PressAt(Point::new(x, track.bottom - 2.0)));
        let clicked = field.scroll_y.get();
        assert!(clicked > 0.0);
        assert!(field.repaint_on_move(), "a held scrollbar repaints on drag");
        // Dragging back to the top scrolls up without waiting for the release.
        field.input(WidgetInput::DragTo(Point::new(x, track.top + 4.0)));
        assert!(field.scroll_y.get() < 0.01);
        field.input(WidgetInput::ReleaseAt(Point::new(x, track.top + 4.0)));
        assert!(
            field.repaint_on_move(),
            "the pointer is still over the bar, so it keeps its hover tint"
        );
        field.input(WidgetInput::HoverLeave);
        assert!(!field.repaint_on_move());
    }

    #[test]
    fn painted_caret_width_follows_the_override() {
        let mut field = painted("abc").caret_width(3.0);
        layout(&mut field);
        assert!(field.begin_inline_edit(None));
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = Recorder::default();
        paint_state(&field, &painter, &images, &mut canvas, true, true);
        let caret = painter.palette().text;
        let (rect, _) = canvas
            .fills
            .iter()
            .find(|(_, color)| *color == caret)
            .expect("the caret is painted");
        assert!((rect.width() - 3.0).abs() < 0.01);
    }

    #[test]
    fn painted_drag_past_the_bottom_scrolls_and_selects() {
        let mut field: TextEdit<String> = TextEdit::new(
            "note",
            "0\n1\n2\n3\n4\n5\n6\n7\n8\n9",
            |text| text,
        )
        .painted()
        .multiline();
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 100.0));
        layout(&mut field);
        assert!(field.begin_inline_edit(Some(Point::new(20.0, 12.0))));
        assert!(field.selecting);
        let content = field.content_rect();
        field.input(WidgetInput::DragTo(Point::new(20.0, content.bottom + 40.0)));
        assert!(field.scroll_y.get() > 0.0, "dragging below the field scrolls");
        assert!(field.selection().is_some(), "the drag extends the selection");
        let scrolled = field.scroll_y.get();
        field.input(WidgetInput::DragTo(Point::new(20.0, content.top - 40.0)));
        assert!(field.scroll_y.get() < scrolled, "dragging above scrolls back");
    }

    fn overflow_field(painted: bool) -> TextEdit<String> {
        let text = (0..30)
            .map(|index| format!("第 {index} 行"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut field: TextEdit<String> = TextEdit::new("note", text, |text| text)
            .multiline()
            .wrap(true)
            .scrollbars(ScrollBars::Vertical);
        if painted {
            field = field.painted();
        }
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 100.0));
        layout(&mut field);
        field
    }

    #[test]
    fn painted_editor_scrollbar_hovers_and_drags() {
        let mut field = overflow_field(true);
        assert!(field.painted);
        let (track, total, visible, offset) = field.bar_metrics(true).expect("vertical bar");
        assert!(total > visible, "the text overflows the field");
        let thumb = field
            .bar_thumb(track, total, visible, offset, true)
            .expect("thumb");
        let over = Point::new(track.center().x, thumb.center().y);
        field.input(WidgetInput::HoverAt(over));
        assert!(field.vbar.hovered);
        assert!(field.repaint_on_move(), "bar hover repaints on motion");
        assert_eq!(field.cursor_at(over), Some(Cursor::Arrow));

        field.input(WidgetInput::PressAt(over));
        assert!(field.vbar.active);
        let before = field.scroll_y.get();
        field.input(WidgetInput::DragTo(Point::new(over.x, over.y + 30.0)));
        assert!(field.scroll_y.get() > before, "dragging the thumb scrolls");
        field.input(WidgetInput::ReleaseAt(Point::new(over.x, over.y + 30.0)));
        assert!(!field.vbar.active);
    }

    #[test]
    fn painted_vertical_scroll_is_fractional() {
        let mut field = overflow_field(true);
        // A wheel notch moves three lines in DIPs instead of a line index.
        field.input(WidgetInput::Wheel(-1.0));
        let after = field.scroll_y.get();
        assert!(after > 0.0);
        let (track, total, visible, offset) = field.bar_metrics(true).unwrap();
        let thumb = field
            .bar_thumb(track, total, visible, offset, true)
            .unwrap();
        let start = thumb.center();
        field.input(WidgetInput::PressAt(start));
        field.input(WidgetInput::DragTo(Point::new(start.x, start.y + 7.0)));
        let dragged = field.scroll_y.get();
        assert!(dragged > after, "the drag keeps scrolling");
        let line_height = field.layout.borrow().line_height.max(1.0);
        let remainder = dragged % line_height;
        assert!(
            remainder > 0.01 && remainder < line_height - 0.01,
            "offset {dragged} snapped to a line ({line_height})"
        );
        field.input(WidgetInput::ReleaseAt(Point::new(start.x, start.y + 7.0)));
    }

    #[test]
    fn the_painted_vertical_thumb_tracks_the_pixel_offset() {
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let field = overflow_field(true);
        let track = field.v_track();
        let thumb_top = |canvas: &Recorder| {
            canvas
                .fills
                .iter()
                .filter(|(_, color)| *color == painter.palette().scroll_thumb)
                .map(|(rect, _)| rect.top)
                .fold(f32::MAX, f32::min)
        };
        let mut canvas = Recorder::default();
        paint_field(&field, &painter, &images, &mut canvas);
        let top = thumb_top(&canvas);
        assert!((top - track.top).abs() < 1.0, "thumb starts at the top: {top}");
        // Set one line of scroll directly (independent of the system's wheel
        // setting): the units must match the offset — a line-based total with a
        // pixel offset jumped straight to the end, so the bar looked like it
        // had two steps.
        let line_height = field.layout.borrow().line_height.max(1.0);
        field.scroll_y.set(line_height);
        let mut canvas = Recorder::default();
        paint_field(&field, &painter, &images, &mut canvas);
        let moved = thumb_top(&canvas);
        assert!(
            moved > top + 0.5 && moved < track.bottom - 20.0,
            "thumb moved {top} -> {moved} (track {track:?})"
        );
    }

    #[test]
    fn painted_editor_track_click_scrolls_further() {
        let mut field = overflow_field(true);
        let (track, _, _, _) = field.bar_metrics(true).unwrap();
        let before = field.scroll_y.get();
        field.input(WidgetInput::PressAt(Point::new(
            track.center().x,
            track.bottom - 2.0,
        )));
        assert!(field.scroll_y.get() > before, "a track click jumps forward");
        field.input(WidgetInput::ReleaseAt(Point::new(track.center().x, track.bottom - 2.0)));
    }

    #[test]
    fn closed_multiline_reserves_the_bar_from_the_native_editor() {
        let mut field = overflow_field(false);
        let (track, _, _, _) = field.bar_metrics(true).unwrap();
        assert!(
            !field.wants_edit_at(Point::new(track.center().x, track.center().y)),
            "a press on the closed bar must not open the editor"
        );
        let content = field.content_rect();
        assert!(field.wants_edit_at(Point::new(content.left + 4.0, content.top + 4.0)));
        // The closed field still drags its bar.
        let thumb = field
            .bar_thumb(track, 30.0, field.layout.borrow().visible_lines.max(1) as f32, 0.0, true)
            .unwrap();
        let over = Point::new(track.center().x, thumb.center().y);
        field.input(WidgetInput::PressAt(over));
        field.input(WidgetInput::DragTo(Point::new(over.x, over.y + 30.0)));
        assert!(field.scroll_y.get() > 0.0);
        field.input(WidgetInput::ReleaseAt(Point::new(over.x, over.y + 30.0)));
    }

    #[test]
    fn unwrapped_multiline_drags_its_horizontal_bar() {
        let mut field: TextEdit<String> = TextEdit::new(
            "note",
            "这是一行非常长的文本，用于让打开换行关闭后的横向滚动条出现。这一行必须足够长。",
            |text| text,
        )
        .multiline()
        .wrap(false)
        .scrollbars(ScrollBars::Both)
        .painted();
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 100.0));
        layout(&mut field);
        let (track, total, visible, offset) = field.bar_metrics(false).expect("horizontal bar");
        assert!(total > visible);
        let thumb = field.bar_thumb(track, total, visible, offset, false).expect("thumb");
        let over = Point::new(thumb.center().x, track.center().y);
        field.input(WidgetInput::PressAt(over));
        assert!(field.hbar.active);
        field.input(WidgetInput::DragTo(Point::new(over.x + 40.0, over.y)));
        assert!(field.scroll_x.get() > 0.0, "dragging the horizontal thumb scrolls");
        field.input(WidgetInput::ReleaseAt(Point::new(over.x + 40.0, over.y)));
        assert!(!field.hbar.active);
    }

    #[test]
    fn a_multiline_cue_is_painted_on_the_first_line() {
        let mut field: TextEdit<String> = TextEdit::new("note", "", |text| text)
            .painted()
            .multiline()
            .cue("在左侧选择条目，然后在此输入译文…");
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 120.0));
        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = Recorder::default();
        paint_field(&field, &painter, &images, &mut canvas);
        let content = field.content_rect();
        let index = canvas
            .texts
            .iter()
            .position(|text| text.starts_with("在左侧"))
            .expect("cue painted");
        let rect = canvas.text_rects[index];
        assert!(
            (rect.top - content.top).abs() < 0.5,
            "the cue must sit on the caret's (first) line, not in the middle: {rect:?} in {content:?}"
        );
        assert!(
            rect.height() < content.height() * 0.5,
            "the cue rectangle must be one line tall, not the whole field"
        );
    }

    #[test]
    fn a_read_only_painted_field_keeps_selection_and_copy_but_refuses_edits() {
        let mut field: TextEdit<String> = TextEdit::new("field", "Bamboo", |text| text)
            .painted()
            .read_only(true);
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        assert!(field.begin_edit_impl(None), "a locked field still opens a session");
        assert!(field.editing);

        // Typing, deleting and pasting are refused.
        assert_eq!(field.input(WidgetInput::Char('x')), None);
        field.input(key(KeyCode::Backspace));
        field.input(key(KeyCode::Delete));
        assert!(!Widget::<String>::insert_text(&mut field, "pasted"));
        assert_eq!(field.text, "Bamboo");

        // Selection and copy work; cut only copies.
        field.input(key_mod(KeyCode::Letter(0), true, false));
        assert_eq!(field.selection(), Some((0, 6)));
        field.input(key_mod(KeyCode::Letter(2), true, false));
        assert_eq!(field.take_clipboard().as_deref(), Some("Bamboo"));
        field.input(key_mod(KeyCode::Letter(23), true, false));
        assert_eq!(field.text, "Bamboo", "cut must not remove text from a locked field");
        assert_eq!(field.take_clipboard().as_deref(), Some("Bamboo"));

        // The menu mirrors the lock and assistive technology reports it.
        let menu = field.edit_menu_impl().expect("built-in menu");
        let paste = menu.command(edit_command::PASTE).expect("paste item");
        let cut = menu.command(edit_command::CUT).expect("cut item");
        let copy = menu.command(edit_command::COPY).expect("copy item");
        assert!(paste.disabled && cut.disabled, "mutations stay locked");
        assert!(!copy.disabled, "copy stays available for the selection");
        assert!(field.a11y().unwrap().read_only);

        // The application can still replace the value.
        Widget::<String>::set_text(&mut field, "New".to_string());
        assert_eq!(field.text, "New");
    }

    #[test]
    fn a_pointer_drag_selects_in_a_read_only_field() {
        let mut field: TextEdit<String> = TextEdit::new("field", "Bamboo", |text| text)
            .painted()
            .read_only(true);
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        layout(&mut field);
        let content = field.content_rect();
        let y = content.center().y;
        assert!(field.begin_edit_impl(Some(Point::new(content.left + 1.0, y))));
        field.input(WidgetInput::DragTo(Point::new(content.right + 40.0, y)));
        assert_eq!(field.selection(), Some((0, 6)), "the whole word is selected");
        assert_eq!(field.text, "Bamboo", "selection must not edit the locked value");
    }

    #[test]
    fn edit_observers_see_chars_clicks_keys_and_changes() {
        let mut field: TextEdit<String> = TextEdit::new("field", "", |text| format!("commit:{text}"))
            .painted()
            .on_char(|ch| (ch == '!').then(|| "bang".to_string()))
            .on_change(|text| Some(format!("change:{text}")))
            .on_click(|_| Some("click".to_string()))
            .on_right_click(|_| Some("right".to_string()))
            .on_double_click(|_| Some("double".to_string()))
            .on_key_down(|key| (key.code == KeyCode::Left).then(|| "down".to_string()))
            .on_key_up(|key| (key.code == KeyCode::Left).then(|| "up".to_string()));
        field.set_rect(Rect::from_xywh(0.0, 0.0, 200.0, 34.0));
        field.input(WidgetInput::Activate);

        // A character with no `on_char` message reports the change instead.
        assert_eq!(
            field.input(WidgetInput::Char('a')),
            Some("change:a".to_string())
        );
        assert_eq!(
            field.input(WidgetInput::Char('!')),
            Some("bang".to_string())
        );
        assert_eq!(field.text, "a!");
        assert_eq!(
            field.input(WidgetInput::ClickAt(Point::new(4.0, 4.0))),
            Some("click".to_string())
        );
        assert_eq!(
            field.input(WidgetInput::RightClickAt(Point::new(4.0, 4.0))),
            Some("right".to_string())
        );
        assert_eq!(
            field.input(WidgetInput::DoubleClickAt(Point::new(4.0, 4.0))),
            Some("double".to_string())
        );
        assert_eq!(
            field.input(key(KeyCode::Left)),
            Some("down".to_string())
        );
        assert_eq!(
            field.input(WidgetInput::KeyUp(KeyEvent {
                code: KeyCode::Left,
                vk: 0,
                ctrl: false,
                shift: false,
                alt: false,
                repeat: false,
            })),
            Some("up".to_string())
        );

        // Programmatic changes do not fire the update hook.
        Widget::<String>::set_text(&mut field, "quiet".to_string());
        assert_eq!(field.text, "quiet");
    }
}
