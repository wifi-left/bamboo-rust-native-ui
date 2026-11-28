// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Text-run layout helpers.
//!
//! The backend [`Canvas`](crate::backend::Canvas) paints one line at a time.
//! Widgets that show paragraphs (a wrapping label) need line ranges first;
//! [`wrap_ranges`] computes them against a measurement callback, so it stays
//! independent of the backend and testable without a window.
//!
//! Ranges are **byte offsets into the original string**. Lines are stored as
//! slices of the caller's text, so painting a wrapped label needs no copy; only
//! the range vector itself is allocated.

/// Split `text` into line ranges that fit `max_width`.
///
/// Greedy word wrapping: a line breaks at the last whitespace that still fits,
/// otherwise between characters (CJK text has no spaces). A hard `\n` always
/// starts a new line, and consecutive newlines keep their blank lines. Leading
/// spaces of a continuation line and trailing spaces before a break are not
/// part of any range, so a painted line never starts or ends with padding.
///
/// `measure` returns the width of one slice (a character at a time, so kerning
/// across a break is not considered). An empty string produces one empty range.
pub fn wrap_ranges(
    text: &str,
    max_width: f32,
    measure: &mut dyn FnMut(&str) -> f32,
) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    if text.is_empty() {
        lines.push((0, 0));
        return lines;
    }
    let mut line_start = 0;
    'lines: loop {
        if line_start >= text.len() {
            break;
        }
        let mut width = 0.0f32;
        let mut last_break: Option<usize> = None;
        let mut line_end = line_start;
        for (offset, ch) in text[line_start..].char_indices() {
            let start = line_start + offset;
            let end = start + ch.len_utf8();
            if ch == '\n' {
                lines.push((line_start, start));
                line_start = end;
                continue 'lines;
            }
            let char_width = measure(&text[start..end]);
            if width + char_width > max_width && start > line_start {
                // Break before this character; prefer the last whitespace so
                // words stay whole.
                let cut = last_break.unwrap_or(start);
                lines.push(trim_end(text, line_start, cut));
                line_start = skip_spaces(text, cut);
                continue 'lines;
            }
            width += char_width;
            line_end = end;
            if ch.is_whitespace() {
                last_break = Some(end);
            }
        }
        lines.push(trim_end(text, line_start, line_end));
        break;
    }
    if lines.is_empty() {
        lines.push((0, 0));
    }
    lines
}

/// Longest prefix of `text` that fits `max_width`, as a byte length. Used for
/// pre-truncation before an ellipsis is appended.
pub fn fitting_prefix(text: &str, max_width: f32, measure: &mut dyn FnMut(&str) -> f32) -> usize {
    let mut width = 0.0f32;
    let mut end = 0;
    for (offset, ch) in text.char_indices() {
        let next = offset + ch.len_utf8();
        let char_width = measure(&text[offset..next]);
        if width + char_width > max_width {
            break;
        }
        width += char_width;
        end = next;
    }
    end
}

/// Byte length of the first `max_chars` characters of `text` (the whole
/// string when it is shorter).
pub fn fitting_prefix_by_chars(text: &str, max_chars: usize) -> usize {
    text.char_indices()
        .nth(max_chars)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

fn trim_end(text: &str, start: usize, end: usize) -> (usize, usize) {
    let mut end = end;
    while end > start {
        match text[start..end].chars().next_back() {
            Some(ch) if ch.is_whitespace() => end -= ch.len_utf8(),
            _ => break,
        }
    }
    (start, end)
}

fn skip_spaces(text: &str, start: usize) -> usize {
    let mut cursor = start;
    for (offset, ch) in text[start..].char_indices() {
        if ch == ' ' || ch == '\t' {
            cursor = start + offset + 1;
        } else {
            break;
        }
    }
    cursor
}


/// Normalize Windows line endings (`\r\n`, or a lone `\r`) to `\n`.
///
/// The classic multiline `EDIT` control reads back CRLF, while the framework
/// stores and paints `\n`; normalizing at the platform boundary keeps the
/// widget state, its accessibility value and the painted lines identical.
pub fn normalize_newlines(text: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    if !text.contains('\r') {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(ch);
        }
    }
    Cow::Owned(out)
}

/// The form the native multiline `EDIT` control expects: every `\n` as
/// `\r\n` (the input is normalized first, so this is idempotent).
pub fn to_crlf(text: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    let normalized = normalize_newlines(text);
    if !normalized.contains('\n') {
        return normalized;
    }
    Cow::Owned(normalized.replace('\n', "\r\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ten units per character, so a 100-unit line holds ten characters.
    fn measure(text: &str) -> f32 {
        text.chars().count() as f32 * 10.0
    }

    fn slices<'a>(text: &'a str, ranges: &[(usize, usize)]) -> Vec<&'a str> {
        ranges.iter().map(|&(s, e)| &text[s..e]).collect()
    }

    #[test]
    fn short_text_stays_one_line() {
        let ranges = wrap_ranges("hello", 100.0, &mut measure);
        assert_eq!(ranges, vec![(0, 5)]);
    }

    #[test]
    fn ascii_wraps_at_the_last_space() {
        let text = "hello world again";
        let ranges = wrap_ranges(text, 100.0, &mut measure);
        // "hello " is 6 chars, adding "world" would be 11 (> 10).
        assert_eq!(slices(text, &ranges), vec!["hello", "world", "again"]);
    }

    #[test]
    fn cjk_breaks_between_characters() {
        let text = "一二三四五六七八九十甲乙";
        let ranges = wrap_ranges(text, 50.0, &mut measure);
        assert_eq!(slices(text, &ranges), vec!["一二三四五", "六七八九十", "甲乙"]);
    }

    #[test]
    fn hard_newlines_start_new_lines_and_keep_blanks() {
        let text = "ab\n\ncd";
        let ranges = wrap_ranges(text, 100.0, &mut measure);
        assert_eq!(slices(text, &ranges), vec!["ab", "", "cd"]);
    }

    #[test]
    fn a_long_word_breaks_between_characters() {
        let text = "abcdefghijklm";
        let ranges = wrap_ranges(text, 50.0, &mut measure);
        assert_eq!(slices(text, &ranges), vec!["abcde", "fghij", "klm"]);
    }

    #[test]
    fn breaking_skips_the_spaces_it_breaks_at() {
        let text = "aa bb";
        // Fits "aa bb" exactly at 50; one unit smaller forces the break.
        let ranges = wrap_ranges(text, 49.0, &mut measure);
        assert_eq!(slices(text, &ranges), vec!["aa", "bb"]);
    }

    #[test]
    fn empty_text_yields_one_empty_range() {
        assert_eq!(wrap_ranges("", 100.0, &mut measure), vec![(0, 0)]);
    }

    #[test]
    fn fitting_prefix_counts_characters_not_bytes() {
        assert_eq!(fitting_prefix("一二三", 25.0, &mut measure), 6);
        assert_eq!(fitting_prefix("abc", 25.0, &mut measure), 2);
        assert_eq!(fitting_prefix("abc", 100.0, &mut measure), 3);
    }

    #[test]
    fn newlines_normalize_and_convert_for_the_native_control() {
        use super::{normalize_newlines, to_crlf};
        assert_eq!(normalize_newlines("a\r\nb\rc\nd"), "a\nb\nc\nd");
        assert_eq!(normalize_newlines("plain"), "plain");
        assert_eq!(to_crlf("a\nb"), "a\r\nb");
        assert_eq!(to_crlf("a\r\nb"), "a\r\nb");
        assert_eq!(to_crlf("no break"), "no break");
    }
}
