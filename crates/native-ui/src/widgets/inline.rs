// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Shared inline text field behind the framework-painted composite editors.
//!
//! A composite widget that edits text in place (a table cell, a tree row
//! label, a property value) owns one [`PaintedField`] for the lifetime of the
//! widget. It embeds the same self-painted editor the standalone [`crate::widgets::TextEdit`]
//! uses — buffer and layout cache, caret/selection model, undo/redo, clipboard,
//! the built-in edit menu, inline IME, caret reveal/scroll and the UIA value —
//! so a cell and a field behave identically. The container session stays the
//! routing layer; the composite owns the session lifetime and applies the
//! committed value through its own callback.

use crate::backend::TextAlign;
use crate::event::KeyEvent;
use crate::geometry::{Insets, Point, Rect};
use crate::menu::Menu;
use crate::widget::{Widget, WidgetInput, WidgetPaint, WidgetState};
use crate::widgets::edit::TextEdit;

/// A painted edit field embedded in a composite widget.
pub(crate) struct PaintedField<M> {
    edit: TextEdit<M>,
    open: bool,
}

impl<M> PaintedField<M> {
    pub(crate) fn new() -> Self {
        Self {
            edit: TextEdit::inline_field("inline-field"),
            open: false,
        }
    }

    /// Whether a session is open.
    #[cfg(test)]
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// Set the embedded field's text colour; `None` restores the palette.
    pub(crate) fn set_text_color(&mut self, color: Option<crate::Color>) {
        self.edit.set_text_color(color);
    }

    /// Lock the embedded field (selection and copy stay, mutations are
    /// refused).
    pub(crate) fn set_read_only(&mut self, read_only: bool)
    where
        M: 'static,
    {
        Widget::set_read_only(&mut self.edit, read_only);
    }

    /// Open a session over `rect` with `text`. `position` places the caret
    /// from a pointer press; without one the whole value starts selected, so
    /// typing from the keyboard replaces it. A value containing a newline
    /// uses the painted multiline model.
    pub(crate) fn begin(
        &mut self,
        rect: Rect,
        text: &str,
        position: Option<Point>,
        padding: Insets,
    ) -> bool {
        self.begin_styled(rect, text, position, padding, TextAlign::Left, true, 0)
    }

    /// [`PaintedField::begin`] with the session's alignment, own-chrome flag
    /// and `max_length`. A widget that draws its own frame and decorations (a
    /// search box's magnifier and clear button, a number input's prefix/suffix
    /// and steppers) passes `chrome = false` and paints its own chrome around
    /// the field.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn begin_styled(
        &mut self,
        rect: Rect,
        text: &str,
        position: Option<Point>,
        padding: Insets,
        align: TextAlign,
        chrome: bool,
        max_length: u32,
    ) -> bool {
        // A session left open on another cell/row is closed first, so the new
        // session's Escape snapshot and undo stack start from its own value.
        if self.open {
            self.edit.inline_finish(false);
            self.open = false;
        }
        let multiline = text.contains('\n');
        self.edit.inline_set_rect(rect);
        self.edit.configure_inline(multiline, padding, align);
        self.edit.inline_set_chrome(chrome);
        self.edit.inline_set_max_length(max_length);
        self.edit.inline_set_text(text.to_string());
        if !self.edit.inline_begin(position) {
            return false;
        }
        if position.is_none() {
            self.edit.inline_select_all();
        }
        self.open = true;
        true
    }

    /// Close the session. `commit` returns the edited value; a cancel restores
    /// the value the session opened with and returns `None`.
    pub(crate) fn finish(&mut self, commit: bool) -> Option<String> {
        if !self.open {
            return None;
        }
        self.open = false;
        let text = self.edit.inline_text().to_string();
        self.edit.inline_finish(commit);
        commit.then_some(text)
    }

    pub(crate) fn text(&self) -> &str {
        self.edit.inline_text()
    }

    /// Caret position in characters (0..=char count).
    #[cfg(test)]
    pub(crate) fn caret(&self) -> usize {
        self.edit.caret()
    }

    /// Whether the field has a non-collapsed selection.
    #[cfg(test)]
    pub(crate) fn has_selection(&self) -> bool {
        self.edit.selection().is_some()
    }

    pub(crate) fn multiline(&self) -> bool {
        self.edit.inline_is_multiline()
    }

    pub(crate) fn rect(&self) -> Rect {
        self.edit.inline_rect()
    }

    pub(crate) fn set_rect(&mut self, rect: Rect) {
        self.edit.inline_set_rect(rect);
    }

    /// Replace the value and put the caret at its end (the numeric stepper).
    pub(crate) fn set_text(&mut self, text: String) {
        self.edit.inline_set_text(text);
    }

    #[cfg(test)]
    pub(crate) fn select_all(&mut self) {
        self.edit.inline_select_all();
    }

    /// Move the caret to the glyph boundary under a pointer press and start a
    /// pointer selection (the click-inside-the-open-field path).
    pub(crate) fn place_at(&mut self, position: Point) -> bool {
        self.open && self.edit.inline_begin(Some(position))
    }

    pub(crate) fn input(&mut self, input: WidgetInput) -> bool {
        self.open && self.edit.inline_input(input)
    }

    pub(crate) fn key(&mut self, key: KeyEvent) -> bool {
        self.input(WidgetInput::Key(key))
    }

    pub(crate) fn insert_text(&mut self, text: &str) -> bool {
        self.edit.inline_insert(text)
    }

    /// Insert text after dropping characters the field does not accept (the
    /// numeric property editor); an empty result changes nothing.
    pub(crate) fn insert_filtered(&mut self, text: &str, keep: impl Fn(char) -> bool) -> bool {
        let filtered: String = text.chars().filter(|ch| keep(*ch)).collect();
        if filtered.is_empty() {
            return false;
        }
        self.edit.inline_insert(&filtered)
    }

    pub(crate) fn set_composition(&mut self, text: &str, cursor: usize) -> bool {
        self.edit.inline_set_composition(text, cursor)
    }

    /// Caret rectangle from the last paint (IME placement and the host caret).
    pub(crate) fn caret_rect(&self) -> Option<Rect> {
        self.edit.inline_caret_rect()
    }

    pub(crate) fn take_clipboard(&mut self) -> Option<String> {
        self.edit.inline_take_clipboard()
    }

    /// The built-in edit menu while a session is open.
    pub(crate) fn edit_menu(&self) -> Option<Menu> {
        if !self.open {
            return None;
        }
        self.edit.inline_menu()
    }

    pub(crate) fn edit_command(&mut self, command: u32) -> bool {
        self.open && self.edit.inline_command(command)
    }

    pub(crate) fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        self.edit.inline_paint(ctx, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{KeyCode, KeyEvent};

    fn key(code: KeyCode, shift: bool, ctrl: bool) -> KeyEvent {
        KeyEvent {
            code,
            vk: 0,
            ctrl,
            shift,
            alt: false,
            repeat: false,
        }
    }

    fn field() -> PaintedField<()> {
        let mut field: PaintedField<()> = PaintedField::new();
        let rect = Rect::from_xywh(0.0, 0.0, 200.0, 24.0);
        assert!(field.begin(rect, "名字", None, Insets::xy(4.0, 2.0)));
        field
    }

    #[test]
    fn a_keyboard_session_selects_all_and_commits_the_typed_value() {
        let mut field = field();
        assert!(field.has_selection());
        assert!(field.insert_text("新"));
        assert_eq!(field.text(), "新");
        assert_eq!(field.caret(), 1);
        assert_eq!(field.finish(true).as_deref(), Some("新"));
        assert!(!field.is_open());
        assert_eq!(field.finish(true), None, "a closed session returns nothing");
    }

    #[test]
    fn opening_another_value_starts_a_fresh_session() {
        let mut field = field();
        field.insert_text("草稿");
        // Re-beginning on another value closes the first session without
        // applying it; the new session's cancel restores its own value.
        assert!(field.begin(
            Rect::from_xywh(0.0, 0.0, 200.0, 24.0),
            "第二",
            None,
            Insets::xy(4.0, 2.0),
        ));
        assert_eq!(field.text(), "第二");
        assert_eq!(field.finish(false), None);
        assert_eq!(field.text(), "第二");
    }

    #[test]
    fn cancel_restores_the_value_the_session_opened_with() {
        let mut field = field();
        field.insert_text("草稿");
        assert_eq!(field.finish(false), None);
        assert_eq!(field.text(), "名字");
    }

    #[test]
    fn a_numeric_filter_drops_rejected_characters() {
        let mut field: PaintedField<()> = PaintedField::new();
        assert!(field.begin(
            Rect::from_xywh(0.0, 0.0, 200.0, 24.0),
            "12",
            None,
            Insets::xy(4.0, 2.0),
        ));
        assert!(field.insert_filtered("3x.4", |ch| ch.is_ascii_digit() || ch == '.'));
        assert_eq!(field.text(), "3.4");
        assert!(!field.insert_filtered("x", |ch| ch.is_ascii_digit()));
    }

    #[test]
    fn caret_motion_selection_and_deletion_come_from_the_painted_core() {
        let mut field: PaintedField<()> = PaintedField::new();
        assert!(field.begin(
            Rect::from_xywh(0.0, 0.0, 200.0, 24.0),
            "abc",
            Some(Point::new(0.0, 0.0)),
            Insets::xy(4.0, 2.0),
        ));
        // A pointer op leaves the caret where the click placed it (the layout
        // has not painted yet, so the end).
        assert_eq!(field.caret(), 3);
        field.key(key(KeyCode::Left, false, false));
        field.key(key(KeyCode::Left, true, false));
        assert!(field.has_selection());
        field.key(key(KeyCode::Backspace, false, false));
        assert_eq!(field.text(), "ac");
        // Ctrl+A then typing replaces everything, like the standalone editor.
        field.key(key(KeyCode::Letter(0), false, true));
        assert!(field.insert_text("z"));
        assert_eq!(field.text(), "z");
    }

    #[test]
    fn a_value_with_a_newline_edits_multiline_and_enter_inserts_a_line() {
        let mut field: PaintedField<()> = PaintedField::new();
        assert!(field.begin(
            Rect::from_xywh(0.0, 0.0, 200.0, 60.0),
            "一
二",
            None,
            Insets::xy(4.0, 2.0),
        ));
        assert!(field.multiline());
        field.key(key(KeyCode::End, false, false));
        field.key(key(KeyCode::Enter, false, false));
        assert_eq!(field.text(), "一
二
");
        assert_eq!(field.finish(true).as_deref(), Some("一
二
"));
    }

    #[test]
    fn a_single_line_value_never_takes_a_newline_from_the_field() {
        let mut field = field();
        field.key(key(KeyCode::Enter, false, false));
        assert!(!field.multiline());
        assert_eq!(field.text(), "名字", "Enter is the container's commit");
    }
}
