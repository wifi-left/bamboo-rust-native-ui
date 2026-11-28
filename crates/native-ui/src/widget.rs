// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Retained widget contract.
//!
//! Widgets are created once per page rebuild and persist until the next
//! rebuild. The [`crate::Ui`] container owns hit-testing, hover/press/focus
//! state and invalidation, so widgets only paint and translate high-level
//! input into messages.

use crate::a11y::A11yInfo;
use crate::backend::Canvas;
use crate::event::{Cursor, KeyCode, KeyEvent};
use crate::geometry::{Point, Rect};
use crate::menu::Menu;
use crate::painter::Painter;
use crate::theme::Palette;
use crate::ui::FocusEvent;

pub use bamboo_native_ui_layout_core::widget::WidgetId;

/// A framework-painted editing session captured for a re-inserted widget
/// (`Ui::remove_preserving_session`, D111). A plain value so the container can
/// hold it while the old widget is gone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSnapshot {
    /// The live draft.
    pub text: String,
    /// Caret position in characters.
    pub caret: usize,
    /// Selection anchor, when the selection is not collapsed.
    pub anchor: Option<usize>,
    /// The value the session opened with, so Escape still reverts after a
    /// restore.
    pub original: Option<String>,
    /// The session was open (as opposed to a closed field's caret state).
    pub open: bool,
}

/// Visual/interaction state supplied by the container at paint time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    /// Keyboard adjustment mode (sliders): solid focus frame.
    pub adjusting: bool,
    pub enabled: bool,
    /// Focus came from the keyboard or assistive technology (the focus ring is
    /// drawn). Pointer focus clears it, so a closed text field does not paint
    /// a caret after a click/right-click.
    pub focus_visible: bool,
    /// A native edit session is open for this widget. The closed-field caret
    /// and selection stay hidden; the native child owns them.
    pub editing: bool,
    /// The caret blink phase is on. Framework-painted editors hide the caret
    /// on the off phase; a repaint of the same rectangle toggles it.
    pub caret_on: bool,
}

/// Paint context for one widget.
pub struct WidgetPaint<'a> {
    pub painter: &'a Painter,
    pub canvas: &'a mut dyn Canvas,
    pub palette: Palette,
    /// Per-instance font override from [`crate::WidgetProps::font`]. Widgets
    /// that draw text through [`WidgetPaint::text`] / `measure_text` (the
    /// built-ins do) honor it automatically; glyph runs keep their face.
    pub font: Option<crate::props::FontSpec>,
    /// Page images registered with [`crate::Ui::set_image`]. Widgets look them
    /// up with [`WidgetPaint::image`]; an unknown id paints nothing, so the
    /// caller can fall back to a placeholder.
    pub images: &'a crate::image::ImageStore,
    /// Visible client area in DIPs. Popups use it to flip above the anchor
    /// when the list does not fit below.
    pub viewport: Rect,
    /// Disjoint dirty rectangles for this paint pass, in DIPs. Ordinary
    /// widgets ignore it (the back buffer keeps their previous pixels), but a
    /// widget that paints an expensive full-surface effect (a translucent
    /// scrim) must restrict the effect to these pieces: repainting the whole
    /// surface for a small invalidation is both slow and, for alpha, visibly
    /// wrong when pieces overlap.
    pub dirty: &'a [Rect],
}

/// Which theme text role a widget paints with. `Glyph` runs are never
/// restyled by a per-widget font override.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyleRole {
    Body,
    Small,
    Title,
    Brand,
    Glyph,
}

impl WidgetPaint<'_> {
    pub fn enabled_color(&self, enabled: bool, normal: crate::Color, disabled: crate::Color) -> crate::Color {
        if enabled { normal } else { disabled }
    }

    /// Draw UI text through a theme role, with the per-widget font override
    /// applied. Glyph runs are never restyled.
    pub fn text(
        &mut self,
        rect: Rect,
        text: &str,
        role: TextStyleRole,
        align: crate::backend::TextAlign,
        color: crate::Color,
    ) {
        let painter = self.painter;
        let base = match role {
            TextStyleRole::Body => painter.body(),
            TextStyleRole::Small => painter.small(),
            TextStyleRole::Title => painter.title(),
            TextStyleRole::Brand => painter.brand(),
            TextStyleRole::Glyph => painter.glyph(),
        };
        let style = self.font.as_ref().map(|font| font.apply(base));
        let style = style.as_ref().unwrap_or(base);
        self.painter.text(&mut *self.canvas, rect, text, style, align, color);
    }

    /// Draw text with an explicitly resolved style (per-field font size).
    pub fn text_styled(
        &mut self,
        rect: Rect,
        text: &str,
        base: &crate::backend::TextStyle,
        align: crate::backend::TextAlign,
        color: crate::Color,
    ) {
        let style = self.font.as_ref().map(|font| font.apply(base));
        let style = style.as_ref().unwrap_or(base);
        self.painter.text(&mut *self.canvas, rect, text, style, align, color);
    }

    /// Draw UI text through a theme role and choose the edge behavior (clip or
    /// ellipsis). The per-widget font override applies like [`Self::text`].
    pub fn text_overflow(
        &mut self,
        rect: Rect,
        text: &str,
        role: TextStyleRole,
        align: crate::backend::TextAlign,
        color: crate::Color,
        overflow: crate::backend::TextOverflow,
    ) {
        let painter = self.painter;
        let base = match role {
            TextStyleRole::Body => painter.body(),
            TextStyleRole::Small => painter.small(),
            TextStyleRole::Title => painter.title(),
            TextStyleRole::Brand => painter.brand(),
            TextStyleRole::Glyph => painter.glyph(),
        };
        let style = self.font.as_ref().map(|font| font.apply(base));
        let style = style.as_ref().unwrap_or(base);
        self.canvas
            .text_overflow(rect, text, style, align, color, overflow);
    }

    /// Measure UI text with the same override `text` would use.
    pub fn measure_text(&mut self, text: &str, role: TextStyleRole) -> crate::Size {
        let painter = self.painter;
        let base = match role {
            TextStyleRole::Body => painter.body(),
            TextStyleRole::Small => painter.small(),
            TextStyleRole::Title => painter.title(),
            TextStyleRole::Brand => painter.brand(),
            TextStyleRole::Glyph => painter.glyph(),
        };
        let style = self.font.as_ref().map(|font| font.apply(base));
        let style = style.as_ref().unwrap_or(base);
        self.canvas.measure_text(text, style)
    }

    pub fn measure_text_styled(
        &mut self,
        text: &str,
        base: &crate::backend::TextStyle,
    ) -> crate::Size {
        let style = self.font.as_ref().map(|font| font.apply(base));
        let style = style.as_ref().unwrap_or(base);
        self.canvas.measure_text(text, style)
    }

    /// Resolve the effective style for measurements the widget does itself
    /// (closed-field caret/selection, per-field font size).
    pub fn style(&self, base: &crate::backend::TextStyle) -> crate::backend::TextStyle {
        self.font
            .as_ref()
            .map(|font| font.apply(base))
            .unwrap_or_else(|| base.clone())
    }

    /// Draw a page image. Returns false when the id is not registered (the
    /// widget can then paint a placeholder or stay blank).
    pub fn image(
        &mut self,
        rect: Rect,
        crop_square: bool,
        id: crate::backend::ImageId,
    ) -> bool {
        let Some(image) = self.images.get(id) else {
            return false;
        };
        self.canvas.image(
            rect,
            crop_square,
            id,
            crate::backend::ImageData {
                rgba: &image.rgba,
                width: image.width,
                height: image.height,
            },
        );
        true
    }

    /// Draw a page image clipped to a rounded rectangle (`radius` DIPs). Same
    /// registration rule as [`WidgetPaint::image`].
    pub fn image_round(
        &mut self,
        rect: Rect,
        radius: f32,
        crop_square: bool,
        id: crate::backend::ImageId,
    ) -> bool {
        let Some(image) = self.images.get(id) else {
            return false;
        };
        self.canvas.image_round(
            rect,
            radius,
            crop_square,
            id,
            crate::backend::ImageData {
                rgba: &image.rgba,
                width: image.width,
                height: image.height,
            },
        );
        true
    }

    /// Draw a page image cropped to the circle inscribed in `rect` (an
    /// avatar). Same registration rule as [`WidgetPaint::image`].
    pub fn image_circle(
        &mut self,
        rect: Rect,
        crop_square: bool,
        id: crate::backend::ImageId,
    ) -> bool {
        let Some(image) = self.images.get(id) else {
            return false;
        };
        self.canvas.image_circle(
            rect,
            crop_square,
            id,
            crate::backend::ImageData {
                rgba: &image.rgba,
                width: image.width,
                height: image.height,
            },
        );
        true
    }
}

/// High-level input after the container has resolved hit-testing.
#[derive(Clone, Copy, Debug)]
pub enum WidgetInput {
    HoverEnter,
    HoverLeave,
    /// Pointer moved while hovering this widget.
    HoverAt(Point),
    PressAt(Point),
    /// Pointer moved while the button is held on this widget.
    DragTo(Point),
    ReleaseAt(Point),
    /// Press and release landed on this widget.
    ClickAt(Point),
    /// Press and release with Ctrl/Shift held, for a widget that opted into
    /// modified selection (`multi_selectable`). A list reads it as
    /// Ctrl-toggle / Shift-range; other widgets never receive it.
    SelectWith(Point, crate::event::PointerModifiers),
    /// The platform reported a double click.
    DoubleClickAt(Point),
    /// Secondary-button press and release landed on this widget.
    RightClickAt(Point),
    /// Pointer moved past the drag threshold while the button is held.
    DragStart(Point),
    /// The button was released after a drag.
    DragEnd(Point),
    Wheel(f32),
    /// Keyboard step in a slider (negative decreases).
    Step(i32),
    /// Whether Enter/Space while focused (non-adjustable widgets).
    Activate,
    /// A translated character (keyboard or IME result). Control characters
    /// are filtered by the host; Enter, Tab and Backspace arrive as
    /// [`WidgetInput::Key`] instead.
    Char(char),
    Key(KeyEvent),
    /// A key release for [`WidgetInput::Key`]. Observation only: the container
    /// forwards it to the focused widget (the edit-box `on_key_up` hook) and
    /// does not interpret it.
    KeyUp(KeyEvent),
}

/// Scrollbar configuration for a multiline field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollBars {
    None,
    /// Vertical bar; with wrapping this is the usual editor shape.
    #[default]
    Vertical,
    /// Horizontal bar (only meaningful when wrapping is off).
    Horizontal,
    Both,
}

impl ScrollBars {
    pub fn vertical(self) -> bool {
        matches!(self, ScrollBars::Vertical | ScrollBars::Both)
    }

    pub fn horizontal(self) -> bool {
        matches!(self, ScrollBars::Horizontal | ScrollBars::Both)
    }
}

/// Reserved command ids for the built-in editor context menu.
///
/// A framework-painted field supplies its own menu through
/// [`Widget::edit_menu`]; the application forwards the selected id to
/// [`crate::Ui::menu_command`], which recognizes this range and dispatches it
/// back to the widget. Application menus must not use these ids.
pub mod edit_command {
    pub const UNDO: u32 = 0x00E0_1001;
    pub const REDO: u32 = 0x00E0_1002;
    pub const CUT: u32 = 0x00E0_1003;
    pub const COPY: u32 = 0x00E0_1004;
    pub const PASTE: u32 = 0x00E0_1005;
    pub const DELETE: u32 = 0x00E0_1006;
    pub const SELECT_ALL: u32 = 0x00E0_1007;

    /// "Insert Unicode control character" entries (LRM, RLM, ZWSP, ZWJ,
    /// ZWNJ, BOM) in the order the built-in submenu lists them.
    pub const INSERT_LRM: u32 = 0x00E0_1010;
    pub const INSERT_RLM: u32 = 0x00E0_1011;
    pub const INSERT_ZWSP: u32 = 0x00E0_1012;
    pub const INSERT_ZWJ: u32 = 0x00E0_1013;
    pub const INSERT_ZWNJ: u32 = 0x00E0_1014;
    pub const INSERT_BOM: u32 = 0x00E0_1015;

    /// Whether `command` belongs to the built-in editor menu.
    pub fn is_edit(command: u32) -> bool {
        (UNDO..=SELECT_ALL).contains(&command) || (INSERT_LRM..=INSERT_BOM).contains(&command)
    }
}

/// A native popup a widget asks the host to show.
#[derive(Clone, Debug)]
pub struct NativeMenuRequest {
    pub menu: Menu,
    /// Where the popup's top-left goes, in client DIPs (a menu that has to
    /// flip is placed above `avoid` instead).
    pub anchor: Point,
    /// The control rectangle the popup must not cover, in client DIPs. When
    /// the popup opens upward the host aligns its bottom to this rectangle's
    /// top, so the button that opened it stays visible.
    pub avoid: Option<Rect>,
}

/// A request to open a native text-editing session for a widget.
///
/// The platform host places a borderless child edit control over `rect` and
/// returns the committed text through [`crate::Host::edit_committed`]. The
/// widget keeps rendering its own text while the session is closed, so a page
/// repaint never depends on the native control being present.
pub struct EditRequest<'a> {
    pub id: WidgetId,
    pub rect: Rect,
    /// Current text; the native editor starts with it selected.
    pub text: &'a str,
    /// Placeholder shown while the text is empty.
    pub cue: &'a str,
    pub password: bool,
    pub multiline: bool,
    /// Word-wrap long lines (multiline only). With wrap off the control keeps
    /// one visual line per hard newline and scrolls horizontally.
    pub wrap: bool,
    /// Which scrollbars the native control gets (multiline only).
    pub scrollbars: ScrollBars,
    pub numbers_only: bool,
    /// Horizontal text alignment; the native control maps it to
    /// `ES_LEFT`/`ES_CENTER`/`ES_RIGHT`.
    pub align: crate::backend::TextAlign,
    /// Per-widget text size in DIPs (already theme-scaled by the widget); the
    /// host creates the native control's font at this size. `None` uses the
    /// theme body font.
    pub font_size: Option<f32>,
    /// Explicit family for the native control; `None` uses the theme face.
    /// Owned because the container may fill it from `WidgetProps::font`.
    pub font_family: Option<String>,
    /// Character limit; 0 keeps the platform default.
    pub max_length: u32,
}

/// One virtual list row.
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub title: String,
    pub subtitle: String,
    pub trailing: String,
    pub current: bool,
    pub disabled: bool,
}

impl Row {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    pub fn trailing(mut self, trailing: impl Into<String>) -> Self {
        self.trailing = trailing.into();
        self
    }

    pub fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Build a row from a skin child element (`<item>`/`<track>`), or `None`
    /// when the required `title` is missing. The caller validates first.
    pub(crate) fn from_skin(element: &crate::skin::SkinElement) -> Option<Self> {
        Some(Self {
            title: element.attr("title")?.to_string(),
            subtitle: element.attr("subtitle").unwrap_or("").to_string(),
            trailing: element.attr("trailing").unwrap_or("").to_string(),
            current: element.attr_bool("current").unwrap_or(false),
            disabled: element.attr_bool("disabled").unwrap_or(false),
        })
    }

    /// The fixed child vocabulary of a row element.
    pub(crate) const SKIN_ATTRS: &'static [&'static str] =
        &["title", "subtitle", "trailing", "current", "disabled"];

    /// Validate one row child element.
    pub(crate) fn valid_skin_row(element: &crate::skin::SkinElement) -> bool {
        element.attr("title").is_some_and(|title| !title.trim().is_empty())
            && element.attrs_are(Self::SKIN_ATTRS)
            && element.attr_bool_ok("current")
            && element.attr_bool_ok("disabled")
    }
}

/// A retained control. `M` is the application message type.
pub trait Widget<M> {
    fn id(&self) -> WidgetId;
    fn rect(&self) -> Rect;
    fn set_rect(&mut self, rect: Rect);

    /// Downcast seam for [`crate::Ui::with_widget`] / [`crate::Ui::edit_widget`].
    /// Widgets are added as trait objects, so widget-specific methods need a
    /// typed view; the container keeps the invalidation and a11y rules.
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// Height the widget wants in a vertical stack.
    fn preferred_height(&self, _width: f32) -> f32 {
        36.0
    }

    fn enabled(&self) -> bool {
        true
    }

    fn focusable(&self) -> bool {
        true
    }

    /// Whether the widget is currently shown. Hidden widgets receive no input,
    /// are skipped by focus navigation, are not painted and stay out of the
    /// accessibility tree.
    fn visible(&self) -> bool {
        true
    }

    /// Whether pointer hit-testing may land on this widget. Display-only
    /// widgets default to their focusability, so titles and cards never block
    /// a click meant for the control underneath them.
    fn interactive(&self) -> bool {
        self.focusable()
    }

    /// Whether `pos` hits this widget's painted shape. The default is the whole
    /// layout rectangle; widgets with non-rectangular visuals (a round swatch,
    /// a rounded card) override it so empty corners fall through to whatever
    /// is underneath.
    fn hit_test(&self, pos: Point) -> bool {
        self.rect().contains_point(pos)
    }

    /// Cursor shape while the pointer is over this widget. `None` falls back
    /// to `Hand` for interactive widgets and `Arrow` otherwise.
    fn cursor(&self) -> Option<Cursor> {
        None
    }

    /// Cursor at a specific client position. Composite fields override this so
    /// their click zones (number steppers) get an arrow while the editable
    /// area keeps the I-beam; the default is the whole-widget cursor.
    fn cursor_at(&self, _pos: Point) -> Option<Cursor> {
        self.cursor()
    }

    /// Enter/Space toggles "adjust" mode; arrows then operate the widget's
    /// contents (slider value, list rows, segment/tab selection) instead of
    /// moving focus. Esc/Backspace leave the mode, Tab always moves focus.
    fn adjustable(&self) -> bool {
        false
    }

    /// Whether clicking or pressing Enter opens a native edit session.
    fn wants_edit(&self) -> bool {
        false
    }

    /// Whether a press at `pos` opens a native edit session. Composite fields
    /// with their own click zones (number steppers) narrow the editable area;
    /// the default is the whole widget.
    fn wants_edit_at(&self, _pos: Point) -> bool {
        self.wants_edit()
    }

    /// Describe the native edit session for this widget.
    fn edit_request(&self) -> Option<EditRequest<'_>> {
        None
    }

    /// The native editor closed with committed text. Store it and return the
    /// message the application should handle, if any.
    fn commit_text(&mut self, _text: String) -> Option<M> {
        None
    }

    /// The widget edits its text in place with framework painting instead of
    /// asking the host for a native `EDIT` child. Container routing sends keys,
    /// characters, clipboard text and IME composition to it while it is
    /// focused.
    fn inline_edit(&self) -> bool {
        false
    }

    /// Whether a press at `pos` starts inline editing. Composite painted
    /// fields narrow this to their text area (scrollbar tracks keep their own
    /// drag behavior).
    fn inline_edit_at(&self, _pos: Point) -> bool {
        self.inline_edit()
    }

    /// An inline editing session is open (a draft is live).
    fn editing(&self) -> bool {
        false
    }

    /// Enter inserts a newline (multiline) instead of committing.
    fn multiline_edit(&self) -> bool {
        false
    }

    /// Start (or re-position) the inline session. `position` is the press
    /// point; `None` starts from the existing caret (Enter, assistive
    /// technology).
    fn begin_inline_edit(&mut self, _position: Option<Point>) -> bool {
        false
    }

    /// Close the inline session. With `commit` the draft becomes the value and
    /// the returned message (usually `on_commit`) goes to the application;
    /// without it the text reverts to what it was when editing began.
    fn finish_edit(&mut self, _commit: bool) -> Option<M> {
        None
    }

    /// Insert text at the caret, replacing the selection. Used for typed
    /// characters, pasted clipboard text and committed IME results. Returns
    /// whether the text changed.
    fn insert_text(&mut self, _text: &str) -> bool {
        false
    }

    /// Observe one typed character the container is about to insert (the
    /// edit-box `on_char` hook). Runs before [`Widget::insert_text`]; the
    /// insertion still happens. `None` stays silent.
    fn observe_char(&mut self, _ch: char) -> Option<M> {
        None
    }

    /// Take the pending `on_change` message of a direct text insertion (a
    /// typed character routed by the container, a paste, a committed IME
    /// result). The container queues it after the insertion; widgets without
    /// observers return `None`.
    fn take_change_message(&mut self) -> Option<M> {
        None
    }

    /// Replace the inline IME composition string. `cursor` is the composition
    /// cursor in characters. An empty string clears it.
    fn set_composition(&mut self, _text: &str, _cursor: usize) -> bool {
        false
    }

    /// Text the widget just copied (Ctrl+C/Ctrl+X); the container writes it to
    /// the clipboard.
    fn take_clipboard(&mut self) -> Option<String> {
        None
    }

    // Touch and drag-and-drop text model (D121/D122). Only a painted editor
    // implements these; every other widget keeps the defaults.

    /// A touch press was held in place at `pos`: select the word there and
    /// turn on the painted selection handles, like a mobile text field.
    /// Returns whether anything changed.
    fn long_press(&mut self, _pos: Point) -> bool {
        false
    }

    /// The press at `pos` grabbed one of the painted selection handles; the
    /// following drags move that edge instead of selecting from scratch.
    fn grab_selection_handle(&mut self, _pos: Point) -> bool {
        false
    }

    /// The selected text a drag started inside it copies out (the field keeps
    /// its own text). The container stores it as the session's text payload.
    fn take_drag_text(&mut self, _pos: Point) -> Option<String> {
        None
    }

    /// Insert `text` dropped at client point `pos`, replacing a selection and
    /// leaving the caret after the insert. Returns whether the text changed.
    fn insert_dropped_text(&mut self, _text: &str, _pos: Point) -> bool {
        false
    }

    /// Caret rectangle while inline editing, for IME candidate placement and
    /// the blink timer. `None` when the widget has no caret to show.
    fn caret_rect(&self) -> Option<Rect> {
        None
    }

    /// Built-in context menu for a framework-painted editor. The container
    /// shows it when the application installed no menu of its own.
    fn edit_menu(&self) -> Option<Menu> {
        None
    }

    /// Apply a reserved [`edit_command`] id. Returns whether it was handled.
    fn edit_command(&mut self, _command: u32) -> bool {
        false
    }

    /// Whether pointer motion over this widget changes its visual state
    /// (list row hover, slider drag preview). Avoids repainting on every move
    /// for ordinary buttons.
    fn repaint_on_move(&self) -> bool {
        false
    }

    /// Position-aware [`Widget::repaint_on_move`]: the container asks with the
    /// new pointer position before delivering the hover. Composite widgets
    /// compare the cell/row under `pos` with their stored hover, so moving
    /// inside one row repaints nothing and a long list does not repaint its
    /// whole rectangle on every mouse event.
    fn repaint_on_move_at(&self, _pos: Point) -> bool {
        self.repaint_on_move()
    }

    /// Whether pointer state (hover, press, wheel, click) can change what this
    /// widget paints. The container skips invalidation when it is false, so a
    /// full-window backdrop such as a scrim is not repainted every time the
    /// pointer crosses it. Keyboard and assistive input still repaint.
    fn paints_pointer_state(&self) -> bool {
        true
    }

    /// This widget is a full-window backdrop that starts an overlay layer: the
    /// widgets before it are the page and the ones after it are the overlay.
    /// [`crate::Ui::paint`] uses that split to composite a modal over a cached
    /// page (see [`crate::Host::overlay_paint`]). Only `Scrim` opts in.
    fn is_backdrop(&self) -> bool {
        false
    }

    /// Exact repaint rectangle for the input that just ran, when only a part
    /// of the widget changed. A scrollable widget narrows bar chrome changes
    /// (hover tint, a thumb press) to its scrollbar strip so moving along a
    /// 600-row list does not repaint every visible row. `None` means the whole
    /// paint bounds changed. The container takes the hint after every input
    /// and drops it when that input does not repaint, so it can never leak into
    /// a later event.
    fn take_repaint_hint(&mut self) -> Option<Rect> {
        None
    }

    /// Whether the wheel scrolls this widget's own content (a list, table, tree
    /// or multiline field). A [`crate::ScrollView`] leaves the wheel to such a
    /// child instead of scrolling its own content, so nested scrolling never
    /// double-applies a notch. See D65.
    fn scrolls_with_wheel(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState);

    /// Translate input into a message. Returns `Some` when the application
    /// should react.
    fn input(&mut self, input: WidgetInput) -> Option<M>;

    /// A second message produced by the input that just ran, queued after the
    /// primary one. Widgets use it for additive notifications that must not
    /// displace the built-in message: a slider's `on_drag_end` alongside its
    /// `on_change` report, or an input observer on a control that also emits
    /// its own action. The container drains it after every input, so it can
    /// never leak into a later event.
    fn take_extra_message(&mut self) -> Option<M> {
        None
    }

    /// Capture a live framework-painted editing session so a widget re-inserted
    /// with the same id can inherit it (`Ui::remove_preserving_session`, D111).
    /// Closing the draft here is part of the capture: the container's normal
    /// removal then has nothing left to commit. `None` means there is nothing
    /// to hand over (a native `EDIT` session owns its control, and a composite
    /// keeps its embedded field internal).
    fn take_session(&mut self) -> Option<SessionSnapshot> {
        None
    }

    /// Restore a session captured by [`Widget::take_session`]. Returns whether
    /// this widget accepted it (the right kind, enabled).
    fn restore_session(&mut self, _snapshot: SessionSnapshot) -> bool {
        false
    }

    // Additive input observers (D109). Each runs before the widget's built-in
    // behavior, which still runs; the message is delivered alongside it. The
    // container routes the events, so a widget only forwards them to its
    // `Observers` field. `TextEdit` keeps its own observer fields (D106) and
    // leaves these at their defaults.
    fn observe_click(&mut self, _pos: Point) -> Option<M> {
        None
    }
    fn observe_right_click(&mut self, _pos: Point) -> Option<M> {
        None
    }
    fn observe_double_click(&mut self, _pos: Point) -> Option<M> {
        None
    }
    fn observe_key_down(&mut self, _key: KeyEvent) -> Option<M> {
        None
    }
    fn observe_key_up(&mut self, _key: KeyEvent) -> Option<M> {
        None
    }
    fn observe_focus_gained(&mut self, _event: FocusEvent) -> Option<M> {
        None
    }
    fn observe_focus_lost(&mut self, _event: FocusEvent) -> Option<M> {
        None
    }

    /// What a screen reader should announce for this widget; `None` hides it
    /// from the accessibility tree (display-only decorations).
    fn a11y(&self) -> Option<A11yInfo> {
        None
    }

    /// Assistive technology selected option/row `index`.
    fn a11y_select_child(&mut self, _index: usize) -> Option<M> {
        None
    }

    /// Assistive technology set a new value (text, number or slider position).
    fn a11y_set_value(&mut self, _text: &str) -> Option<M> {
        None
    }

    /// Assistive technology set a published child's value (a grid cell). The
    /// default forwards to [`Widget::a11y_set_value`]; widgets whose children
    /// are editable (the table's cells) override it.
    fn a11y_set_value_at(&mut self, _child: Option<usize>, text: &str) -> Option<M> {
        self.a11y_set_value(text)
    }

    /// Assistive technology expanded or collapsed this element (a menu, a
    /// menu bar) or one published child (a tree row). Unlike a toggle this is
    /// idempotent: expanding an open menu leaves it open.
    fn a11y_expand(&mut self, _child: Option<usize>, _expanded: bool) -> Option<M> {
        None
    }

    /// Assistive technology scrolled the viewport (UIA Scroll pattern).
    /// `None` leaves an axis alone; the widget clamps each target to its own
    /// content. Returns whether the viewport moved.
    fn a11y_scroll(&mut self, _horizontal: Option<f32>, _vertical: Option<f32>) -> bool {
        false
    }

    /// The widget's own read-only declaration (a property inspector with
    /// per-instance locking). The container ORs it with the `WidgetProps`
    /// read-only flag before opening editors or reporting the a11y state, so a
    /// composite widget that edits its contents in place can honor both.
    fn read_only(&self) -> bool {
        false
    }

    /// The container pushed the instance's `WidgetProps::read_only` value.
    /// Widgets that never edit their contents ignore it.
    fn set_read_only(&mut self, _read_only: bool) {}

    // State updates pushed by the application when its data changes. The
    // container dispatches by id; widgets that do not use a setter ignore it.
    fn set_enabled(&mut self, _enabled: bool) {}
    fn set_value(&mut self, _value: f32) {}
    fn set_toggle(&mut self, _on: bool) {}
    fn set_selected(&mut self, _index: usize) {}
    fn set_rows(&mut self, _rows: Vec<Row>) {}
    fn set_label(&mut self, _label: String) {}
    fn set_progress(&mut self, _value: f32) {}
    fn set_visible(&mut self, _visible: bool) {}
    fn set_message(&mut self, _text: String, _error: bool) {}
    fn set_text(&mut self, _text: String) {}
    fn set_number(&mut self, _value: i64) {}

    /// The widget's [`crate::SurfaceStyle`], when it adopts one. This is the
    /// runtime seam the layout skin reads and writes, so the applier never has
    /// to downcast a widget type.
    fn surface_style(&self) -> Option<&crate::surface::SurfaceStyle> {
        None
    }

    fn surface_style_mut(&mut self) -> Option<&mut crate::surface::SurfaceStyle> {
        None
    }

    /// Which content initial values this widget's setters accept (M7 §6). The
    /// layout-skin applier warns instead of writing an unsupported value.
    fn content_caps(&self) -> crate::props::ContentCaps {
        crate::props::ContentCaps::NONE
    }

    // The widget-declared skin protocol (M7 §6.2–6.4, D127). A widget
    // publishes the attributes and data child elements it accepts; the applier
    // parses each value by its kind, validates everything, and only then
    // applies. The defaults declare nothing, which keeps every existing widget
    // byte-for-byte the same and lets a third-party widget opt in from outside
    // this crate (`crate::skin` needs no feature flag).

    /// Local attributes this widget accepts (names outside the shared
    /// vocabulary). `x-` names are reserved for user code.
    fn skin_attrs(&self) -> &'static [crate::skin::SkinAttrDecl] {
        &[]
    }

    /// Whether `value` is acceptable for the declared attribute `name`.
    /// Side-effect free: the applier calls this for every attribute before it
    /// applies any of them.
    fn validate_skin_attr(&self, _name: &str, _value: &crate::skin::SkinValue<'_>) -> bool {
        true
    }

    /// Apply one validated local attribute. Must not fire callbacks or create,
    /// remove, move or focus widgets.
    fn apply_skin_attr(&mut self, _name: &str, _value: &crate::skin::SkinValue<'_>) {}

    /// Data child element names this widget accepts (rows, options, tracks).
    fn skin_children(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether the child list is acceptable. Side-effect free, like
    /// [`Widget::validate_skin_attr`].
    fn validate_skin_children(&self, _children: &[crate::skin::SkinElement]) -> bool {
        true
    }

    /// Replace this widget's data model with the children. Never creates
    /// widgets; the application layer applies `selected`/state afterwards.
    fn apply_skin_children(&mut self, _children: &[crate::skin::SkinElement]) {}

    /// Rectangle a keyboard/a11y-focus tooltip hangs below. Composite controls
    /// override this with the selected option/row so a tip on a 600-row list
    /// points at the focused row instead of the bottom of the container.
    fn tooltip_rect(&self) -> Rect {
        self.rect()
    }

    /// Sub-item under `pos` for composite controls (a list row, a segmented or
    /// tab cell). The tooltip identity includes this index, so moving between
    /// rows/cells dismisses the tip and starts a fresh dwell; plain widgets
    /// return `None`.
    fn child_at(&self, _pos: Point) -> Option<usize> {
        None
    }

    /// Sub-item that owns keyboard/a11y focus (the selected list row or
    /// option/cell). Used to pick a per-item tooltip while the pointer is
    /// elsewhere; plain widgets return `None`.
    fn focus_item(&self) -> Option<usize> {
        None
    }

    /// Focus ring extra margin (buttons look better with a tighter ring).
    fn focus_inset(&self) -> f32 {
        2.0
    }

    /// Rectangle the focus ring follows. Widgets whose visuals do not fill
    /// their layout cell (a switch's track, a slider's row) override this so
    /// the ring does not cut through painted content.
    fn focus_ring_rect(&self) -> Rect {
        let inset = self.focus_inset();
        self.rect().inset(inset, inset)
    }

    /// Corner radius for the focus ring. `None` draws the usual rectangle; a
    /// circular control (a standalone radio dot) returns half the ring height,
    /// a pill (a switch track) its full half-height. The container draws the
    /// ring with the anti-aliased rounded stroke.
    fn focus_ring_radius(&self) -> Option<f32> {
        None
    }

    /// Everything the widget paints. A widget with a transient overlay (an
    /// open combo drop-down, a flyout) reports the union so invalidation
    /// repaints the part outside its layout rectangle too. Defaults to the
    /// layout rectangle.
    fn paint_bounds(&self) -> Rect {
        self.rect()
    }

    /// Whether this widget paints above its siblings (an open drop-down).
    /// Overlay widgets are painted last and hit-tested first.
    fn overlay(&self) -> bool {
        false
    }

    /// Whether the widget understands modifier clicks (`WidgetInput::SelectWith`
    /// with Ctrl/Shift) for multi-selection. Only list-like widgets opt in;
    /// other controls still receive ordinary clicks under a modifier.
    fn multi_selectable(&self) -> bool {
        false
    }

    /// Access key from the label (`&File` → `F`). The container runs it on
    /// Alt+key: focus the widget and activate it.
    fn mnemonic(&self) -> Option<char> {
        None
    }

    /// Claim an `Alt+key` chord for one of this widget's items (a menu bar's
    /// `Alt+F`). The container calls this on every visible widget before the
    /// single-mnemonic path; returning true means the chord is consumed. The
    /// widget performs its action (opening a menu) itself, and the container
    /// focuses it afterwards so keyboard navigation continues from there.
    fn mnemonic_key(&mut self, _ch: char) -> bool {
        false
    }

    /// A menu this widget wants the host to show natively. `Ui` drains it
    /// after the widget handled the input and requests the popup through
    /// `UiHost::show_context_menu`, so the widget never touches the host and
    /// the menu is the same deferred native popup a context menu uses.
    /// `Ui::menu_command` then replays the selection.
    fn take_native_menu(&mut self) -> Option<NativeMenuRequest> {
        None
    }

    /// The menu this widget would open for its current state, without opening
    /// it (a menu button's model, a menu bar's cursor cell, a toolbar's
    /// overflow). `Ui::open_menu_for` asks for it when application code opens
    /// a widget's menu imperatively; `None` means the widget has no menu of
    /// its own and the registered context menu applies.
    fn native_menu(&self) -> Option<NativeMenuRequest> {
        None
    }

    /// Replace this widget's own menu model at runtime (a menu button or split
    /// button). Returns whether the widget owns a menu slot; `false` lets the
    /// container keep the model in its per-widget registry instead.
    fn set_native_menu(&mut self, _menu: Menu) -> bool {
        false
    }

    /// Close a transient overlay state. `Ui` calls this on Escape and on a
    /// pointer press outside the focused widget, then repaints when it returns
    /// true. Default: no such state.
    fn dismiss(&mut self) -> bool {
        false
    }

    /// Show or hide access-key underlines (the application-level Alt mode).
    /// Only widgets that paint `&`-marked labels respond; the return value says
    /// whether their visuals changed, so the container invalidates just those.
    /// The default is always-on, matching the plain label behavior.
    fn set_mnemonic_underlines(&mut self, _show: bool) -> bool {
        false
    }

    /// Whether an open transient overlay makes this widget own the navigation
    /// keys. While `true`, `Ui` sends Up/Down/Home/End/PageUp/PageDown and
    /// Enter/Space to the widget instead of moving focus (Tab and Escape keep
    /// their container meanings). A combo drop-down uses this so its highlight
    /// moves with the arrow keys; ordinary widgets return `false`.
    fn owns_navigation_keys(&self) -> bool {
        false
    }

    /// Whether one navigation key belongs to this widget rather than to focus
    /// movement. Defaults to [`Widget::owns_navigation_keys`]; a menu bar or
    /// toolbar keeps Left/Right for its item cursor only while there is room
    /// to move, so pressing Left on the first item (or Right on the last,
    /// Up/Down anywhere) moves focus out instead of doing nothing.
    fn owns_navigation_key(&self, _key: KeyCode) -> bool {
        self.owns_navigation_keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interned_ids_are_stable_and_distinct() {
        let a = WidgetId::intern("row-1");
        let b = WidgetId::intern("row-1");
        let other = WidgetId::intern("row-2");
        assert_eq!(a, b, "the same text interns to one id");
        assert_eq!(a.name(), "row-1");
        assert_ne!(a, other);
        // A static id with the same text compares equal: equality is by text.
        assert_eq!(a, WidgetId::new("row-1"));
    }
}
