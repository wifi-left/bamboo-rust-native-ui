// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Building blocks for the framework-painted text editor.
//!
//! [`crate::widgets::TextEdit`] owns the editing state; this module keeps the
//! parts that can be tested without a window: the colour configuration, a line
//! layout that covers every character (spaces included, unlike the label
//! wrapper) and character/word navigation helpers.
//!
//! Layout ranges are byte offsets into the shown text. Every character belongs
//! to exactly one visual line, so a caret at any character boundary has a
//! stable line and x position; soft wraps keep the space they broke after on
//! the previous line and never trim it, because an editor must show what the
//! user typed.

use crate::Color;
use crate::theme::Palette;

/// Per-field colours for the framework-painted editor. Every field is
/// optional; `None` keeps the theme colour, so a field without configuration
/// looks exactly like the built-in one.
///
/// ```ignore
/// let colors = EditColors::new()
///     .text(Color::new(20, 22, 26))
///     .background(Color::new(255, 252, 240))
///     .selection(Color::new(255, 255, 255), Color::new(64, 120, 220))
///     .caret(Color::new(200, 60, 60));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EditColors {
    /// Ordinary text (and the caret when no caret colour is set).
    pub text: Option<Color>,
    /// Field fill behind the text.
    pub background: Option<Color>,
    /// Selected text.
    pub selection_text: Option<Color>,
    /// Selection highlight.
    pub selection_background: Option<Color>,
    /// Caret (I-beam).
    pub caret: Option<Color>,
    /// Placeholder shown while the value is empty.
    pub cue: Option<Color>,
    /// Focused border (defaults to the accent).
    pub border: Option<Color>,
    /// Idle border (defaults to the explicit border colour).
    pub border_idle: Option<Color>,
    /// Underline under the IME composition string.
    pub composition_underline: Option<Color>,
}

impl EditColors {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn text(mut self, color: Color) -> Self {
        self.text = Some(color);
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// Set both selection colours together; the usual pair.
    pub fn selection(mut self, text: Color, background: Color) -> Self {
        self.selection_text = Some(text);
        self.selection_background = Some(background);
        self
    }

    pub fn selection_text(mut self, color: Color) -> Self {
        self.selection_text = Some(color);
        self
    }

    pub fn selection_background(mut self, color: Color) -> Self {
        self.selection_background = Some(color);
        self
    }

    pub fn caret(mut self, color: Color) -> Self {
        self.caret = Some(color);
        self
    }

    pub fn cue(mut self, color: Color) -> Self {
        self.cue = Some(color);
        self
    }

    pub fn border(mut self, color: Color) -> Self {
        self.border = Some(color);
        self
    }

    pub fn border_idle(mut self, color: Color) -> Self {
        self.border_idle = Some(color);
        self
    }

    pub fn composition_underline(mut self, color: Color) -> Self {
        self.composition_underline = Some(color);
        self
    }
}

/// Resolved colours for one paint pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResolvedEditColors {
    pub text: Color,
    pub background: Color,
    pub selection_text: Color,
    pub selection_background: Color,
    pub caret: Color,
    pub cue: Color,
    /// Active-editor frame (an open inline session).
    pub border: Color,
    /// Frame while the field is closed.
    pub border_idle: Color,
    pub composition_underline: Color,
}

impl EditColors {
    /// Fill unset entries from the palette. Disabled fields use the palette's
    /// disabled text/background unless the application overrides them; the
    /// border is never the accent while disabled. The caller picks `border`
    /// while an inline session is open and `border_idle` otherwise, so a
    /// focused-but-closed field is visually distinct from one being edited.
    pub(crate) fn resolve(
        &self,
        palette: &Palette,
        enabled: bool,
    ) -> ResolvedEditColors {
        let text = self.text.unwrap_or(if enabled {
            palette.text
        } else {
            palette.text_disabled
        });
        let background = self.background.unwrap_or(if enabled {
            palette.search_bg
        } else {
            palette.button_disabled
        });
        let selection_background = self.selection_background.unwrap_or(palette.accent);
        let selection_text = self
            .selection_text
            .unwrap_or_else(|| contrast_on(selection_background));
        let border = if enabled {
            self.border.unwrap_or(palette.accent)
        } else {
            palette.divider
        };
        let border_idle = if enabled {
            self.border_idle.unwrap_or(palette.explicit_border)
        } else {
            palette.divider
        };
        ResolvedEditColors {
            text,
            background,
            selection_text,
            selection_background,
            caret: self.caret.unwrap_or(text),
            cue: self.cue.unwrap_or(if enabled {
                palette.muted
            } else {
                palette.text_disabled
            }),
            border,
            border_idle,
            composition_underline: self.composition_underline.unwrap_or(text),
        }
    }
}

/// Black or white, whichever reads better on `background`.
pub(crate) fn contrast_on(background: Color) -> Color {
    if background.luminance() < 0.6 {
        Color::new(250, 250, 250)
    } else {
        Color::new(24, 26, 30)
    }
}

/// One visual line. `bytes` holds `chars + 1` byte offsets (the first is the
/// line start, the last one past its end); `xs` holds the matching x offsets
/// relative to the line start.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LineMetrics {
    pub bytes: Vec<usize>,
    pub xs: Vec<f32>,
}

impl LineMetrics {
    pub fn start(&self) -> usize {
        self.bytes.first().copied().unwrap_or(0)
    }

    pub fn end(&self) -> usize {
        self.bytes.last().copied().unwrap_or(0)
    }

    pub fn chars(&self) -> usize {
        self.bytes.len().saturating_sub(1)
    }

    pub fn width(&self) -> f32 {
        self.xs.last().copied().unwrap_or(0.0)
    }

    /// x offset of the character boundary at `byte` (clamped into the line).
    pub fn x_at(&self, byte: usize) -> f32 {
        let index = match self.bytes.binary_search(&byte) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        }
        .min(self.chars());
        self.xs.get(index).copied().unwrap_or_else(|| self.width())
    }

    /// Nearest character boundary to `x` (relative to the line start), as a
    /// byte offset.
    pub fn byte_at(&self, x: f32) -> usize {
        let mut best = 0;
        let mut best_distance = f32::MAX;
        for (index, candidate) in self.xs.iter().enumerate() {
            let distance = (candidate - x).abs();
            if distance < best_distance {
                best_distance = distance;
                best = index;
            }
        }
        self.bytes
            .get(best)
            .copied()
            .unwrap_or_else(|| self.end())
    }
}

/// Build all visual lines of `text`. Every character is included; with `wrap`,
/// lines break greedily and prefer the last whitespace that still fits, but
/// trailing spaces stay on the line they were typed on. Hard `\n` always
/// starts a new line and consecutive newlines keep their blank lines.
pub(crate) fn layout_lines(
    text: &str,
    max_width: f32,
    wrap: bool,
    measure: &mut dyn FnMut(&str) -> f32,
) -> Vec<LineMetrics> {
    let mut lines = Vec::new();
    let mut hard_start = 0;
    loop {
        let newline = text[hard_start..].find('\n').map(|offset| hard_start + offset);
        let end = newline.unwrap_or(text.len());
        let run = measure_run(text, hard_start, end, measure);
        if wrap && max_width > 1.0 {
            split_wrapped(text, &run, max_width, &mut lines);
        } else {
            lines.push(run);
        }
        match newline {
            Some(offset) => hard_start = offset + 1,
            None => break,
        }
    }
    if lines.is_empty() {
        lines.push(LineMetrics {
            bytes: vec![0],
            xs: vec![0.0],
        });
    }
    lines
}

/// Measure one hard line, keeping every character boundary.
fn measure_run(
    text: &str,
    start: usize,
    end: usize,
    measure: &mut dyn FnMut(&str) -> f32,
) -> LineMetrics {
    let mut bytes = Vec::with_capacity(text[start..end].chars().count() + 1);
    let mut xs = Vec::with_capacity(bytes.capacity());
    bytes.push(start);
    xs.push(0.0);
    let mut width = 0.0;
    for (offset, ch) in text[start..end].char_indices() {
        let byte = start + offset;
        width += measure(&text[byte..byte + ch.len_utf8()]);
        bytes.push(byte + ch.len_utf8());
        xs.push(width);
    }
    LineMetrics { bytes, xs }
}

fn split_wrapped(
    text: &str,
    run: &LineMetrics,
    max_width: f32,
    lines: &mut Vec<LineMetrics>,
) {
    let total = run.chars();
    if total == 0 {
        lines.push(run.clone());
        return;
    }
    let mut begin = 0;
    while begin < total {
        let base = run.xs[begin];
        // Furthest boundary that fits.
        let mut end = begin + 1;
        while end < total && run.xs[end + 1] - base <= max_width {
            end += 1;
        }
        if run.xs[end] - base > max_width && end > begin + 1 {
            // The last character did not fit; drop it from this line.
            end -= 1;
        }
        if end >= total {
            lines.push(slice(run, begin, total));
            return;
        }
        // Prefer breaking after the last space in this line so words stay
        // whole. The space itself stays on the line.
        let mut cut = end;
        let mut probe = end;
        while probe > begin + 1 {
            let from = run.bytes[probe - 1];
            let to = run.bytes[probe];
            if text[from..to].chars().next().is_some_and(char::is_whitespace) {
                cut = probe;
                break;
            }
            probe -= 1;
        }
        lines.push(slice(run, begin, cut));
        begin = cut;
    }
}

fn slice(run: &LineMetrics, begin: usize, end: usize) -> LineMetrics {
    let base = run.xs[begin];
    LineMetrics {
        bytes: run.bytes[begin..=end].to_vec(),
        xs: run.xs[begin..=end]
            .iter()
            .map(|x| (x - base).max(0.0))
            .collect(),
    }
}

/// Visual line containing `byte`. A caret at a soft break (the boundary is
/// both the previous line's end and the next line's start) belongs to the
/// following line, like every editor that draws it at the line start. A caret
/// before a hard newline stays at the end of the current line.
pub(crate) fn line_of(lines: &[LineMetrics], byte: usize) -> usize {
    let mut chosen = 0;
    for (index, line) in lines.iter().enumerate() {
        if byte >= line.start() {
            chosen = index;
            if byte < line.end() {
                return index;
            }
        } else {
            break;
        }
    }
    chosen
}

/// Character index of a byte offset (clamped).
pub(crate) fn char_at_byte(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].chars().count()
}

/// Byte offset of a character index (clamped to the end).
pub(crate) fn byte_at_char(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

fn char_at(text: &str, index: usize) -> Option<char> {
    text.chars().nth(index)
}

/// Move one character left.
pub(crate) fn prev_char(index: usize) -> usize {
    index.saturating_sub(1)
}

/// Move one character right, bounded by `len`.
pub(crate) fn next_char(index: usize, len: usize) -> usize {
    (index + 1).min(len)
}

/// Start of the word to the left: skip whitespace, then the word.
pub(crate) fn word_left(text: &str, from: usize) -> usize {
    let mut index = from;
    while index > 0 && char_at(text, index - 1).is_some_and(char::is_whitespace) {
        index -= 1;
    }
    while index > 0 && char_at(text, index - 1).is_some_and(|ch| !ch.is_whitespace()) {
        index -= 1;
    }
    index
}

/// End of the word to the right: finish the current word, then its trailing
/// whitespace.
pub(crate) fn word_right(text: &str, from: usize) -> usize {
    let len = text.chars().count();
    let mut index = from;
    while index < len && char_at(text, index).is_some_and(|ch| !ch.is_whitespace()) {
        index += 1;
    }
    while index < len && char_at(text, index).is_some_and(char::is_whitespace) {
        index += 1;
    }
    index
}

/// Character range of the word at `index`, for double-click selection. A
/// whitespace character selects its run of whitespace; a character selects the
/// run of non-whitespace around it (CJK text has no spaces, so a phrase
/// selects as one run until the next space or line break).
pub(crate) fn word_range(text: &str, index: usize) -> (usize, usize) {
    let len = text.chars().count();
    if len == 0 {
        return (0, 0);
    }
    let index = index.min(len - 1);
    let whitespace = char_at(text, index).is_some_and(char::is_whitespace);
    let mut start = index;
    let mut end = index + 1;
    while start > 0
        && char_at(text, start - 1).is_some_and(|ch| ch.is_whitespace() == whitespace)
    {
        start -= 1;
    }
    while end < len && char_at(text, end).is_some_and(|ch| ch.is_whitespace() == whitespace) {
        end += 1;
    }
    (start, end)
}

/// Start of the hard line containing `index`.
pub(crate) fn hard_line_start(text: &str, index: usize) -> usize {
    let mut start = index;
    while start > 0 && char_at(text, start - 1) != Some('\n') {
        start -= 1;
    }
    start
}

/// End of the hard line containing `index` (before the newline).
pub(crate) fn hard_line_end(text: &str, index: usize) -> usize {
    let len = text.chars().count();
    let mut end = index.min(len);
    while end < len && char_at(text, end) != Some('\n') {
        end += 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ten units per character, so 100 units hold ten characters.
    fn measure(text: &str) -> f32 {
        text.chars().count() as f32 * 10.0
    }

    fn slices<'a>(text: &'a str, lines: &[LineMetrics]) -> Vec<&'a str> {
        lines
            .iter()
            .map(|line| &text[line.start()..line.end()])
            .collect()
    }

    #[test]
    fn layout_covers_every_character_including_spaces() {
        let text = "aa bb cc";
        let lines = layout_lines(text, 45.0, true, &mut measure);
        // "aa " is 30; "bb " would need 30 more, so the space stays with
        // "bb" on the middle line.
        assert_eq!(slices(text, &lines), vec!["aa ", "bb ", "cc"]);
        assert_eq!(lines.iter().map(LineMetrics::chars).sum::<usize>(), text.chars().count());
    }

    #[test]
    fn layout_keeps_blank_lines_and_trailing_newlines() {
        let text = "a\n\nb\n";
        let lines = layout_lines(text, 1000.0, true, &mut measure);
        assert_eq!(slices(text, &lines), vec!["a", "", "b", ""]);
        let text = "";
        let lines = layout_lines(text, 100.0, true, &mut measure);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].chars(), 0);
    }

    #[test]
    fn wrapping_prefers_spaces_and_never_drops_them() {
        let text = "hello world again";
        let lines = layout_lines(text, 65.0, true, &mut measure);
        // "hello " is 60 and "world" would overflow: break after the space.
        assert_eq!(slices(text, &lines), vec!["hello ", "world ", "again"]);
        let rebuilt: String = slices(text, &lines).concat();
        assert_eq!(rebuilt, text);
    }

    #[test]
    fn cjk_wraps_between_characters() {
        let text = "一二三四五六";
        let lines = layout_lines(text, 35.0, true, &mut measure);
        assert_eq!(slices(text, &lines), vec!["一二三", "四五六"]);
    }

    #[test]
    fn no_wrap_keeps_one_line_per_hard_newline() {
        let text = "a very long line\nshort";
        let lines = layout_lines(text, 30.0, false, &mut measure);
        assert_eq!(slices(text, &lines), vec!["a very long line", "short"]);
    }

    #[test]
    fn caret_positions_follow_soft_breaks() {
        let text = "aa bb";
        let lines = layout_lines(text, 30.0, true, &mut measure);
        // "aa " then "bb".
        assert_eq!(line_of(&lines, 0), 0);
        assert_eq!(line_of(&lines, 3), 1, "the soft break belongs to the next line");
        let first = &lines[0];
        assert_eq!(first.x_at(3), 30.0);
        assert_eq!(first.byte_at(29.0), 3);
        assert_eq!(lines[1].byte_at(0.0), 3);
    }

    #[test]
    fn caret_before_a_hard_newline_stays_on_its_line() {
        let text = "ab\ncd";
        let lines = layout_lines(text, 100.0, true, &mut measure);
        assert_eq!(line_of(&lines, 2), 0);
        assert_eq!(line_of(&lines, 3), 1);
    }

    #[test]
    fn word_navigation_stops_at_word_boundaries() {
        let text = "  hello world ";
        // Ctrl+Right from whitespace lands on the next word, then on the
        // boundary after the word and its trailing space.
        assert_eq!(word_right(text, 0), 2);
        assert_eq!(word_right(text, 2), 8);
        assert_eq!(word_right(text, 8), 14);
        assert_eq!(word_left(text, 14), 8);
        assert_eq!(word_left(text, 8), 2);
        assert_eq!(word_left(text, 2), 0);
        assert_eq!(word_left(text, 0), 0);
        assert_eq!(word_right("", 0), 0);
    }

    #[test]
    fn double_click_word_range_handles_words_and_spaces() {
        assert_eq!(word_range("hello world", 1), (0, 5));
        assert_eq!(word_range("hello world", 5), (5, 6));
        assert_eq!(word_range("hello world", 8), (6, 11));
        assert_eq!(word_range("竹韵播放器", 2), (0, 5));
        assert_eq!(word_range("", 0), (0, 0));
    }

    #[test]
    fn hard_line_limits_ignore_newlines() {
        let text = "ab\ncd\nef";
        assert_eq!(hard_line_start(text, 4), 3);
        assert_eq!(hard_line_end(text, 4), 5);
        assert_eq!(hard_line_start(text, 6), 6);
        assert_eq!(hard_line_end(text, 6), 8);
    }

    #[test]
    fn byte_and_char_offsets_round_trip_for_multibyte_text() {
        let text = "竹a韵";
        assert_eq!(byte_at_char(text, 0), 0);
        assert_eq!(byte_at_char(text, 1), 3);
        assert_eq!(byte_at_char(text, 9), text.len());
        assert_eq!(char_at_byte(text, 0), 0);
        assert_eq!(char_at_byte(text, 3), 1);
        assert_eq!(char_at_byte(text, 4), 2);
        assert_eq!(char_at_byte(text, 100), 3);
    }

    #[test]
    fn colors_fall_back_to_the_palette_and_allow_overrides() {
        let palette = Palette::new(&crate::theme::ThemeSettings::default(), true);
        let defaults = EditColors::new().resolve(&palette, true);
        assert_eq!(defaults.text, palette.text);
        assert_eq!(defaults.background, palette.search_bg);
        assert_eq!(defaults.selection_background, palette.accent);
        assert_eq!(defaults.caret, palette.text);
        assert_eq!(defaults.border, palette.accent);
        assert_eq!(defaults.border_idle, palette.explicit_border);

        let custom = EditColors::new()
            .text(Color::new(10, 20, 30))
            .background(Color::new(250, 240, 200))
            .selection(Color::new(250, 250, 250), Color::new(40, 90, 200))
            .caret(Color::new(220, 40, 40))
            .border(Color::new(1, 2, 3))
            .border_idle(Color::new(4, 5, 6));
        let resolved = custom.resolve(&palette, true);
        assert_eq!(resolved.text, Color::new(10, 20, 30));
        assert_eq!(resolved.background, Color::new(250, 240, 200));
        assert_eq!(resolved.selection_text, Color::new(250, 250, 250));
        assert_eq!(resolved.selection_background, Color::new(40, 90, 200));
        assert_eq!(resolved.caret, Color::new(220, 40, 40));
        assert_eq!(resolved.border, Color::new(1, 2, 3));
        assert_eq!(resolved.border_idle, Color::new(4, 5, 6));
        // Disabled fields never use the accent border.
        let disabled = custom.resolve(&palette, false);
        assert_eq!(disabled.border, palette.divider);
        assert_eq!(disabled.border_idle, palette.divider);
    }

    #[test]
    fn automatic_selection_text_contrasts_with_its_background() {
        let dark = contrast_on(Color::new(20, 30, 40));
        let light = contrast_on(Color::new(230, 235, 240));
        assert!(dark.luminance() > 0.6);
        assert!(light.luminance() < 0.4);
    }
}
