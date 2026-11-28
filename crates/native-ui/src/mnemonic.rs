// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Access keys (mnemonics) in labels.
//!
//! A label marks its access key with `&`: `"&File"` displays as **File** and
//! answers `Alt+F`. `"&&"` is a literal ampersand. The parse happens once at
//! construction, so painting a label does not allocate; the byte offset of the
//! key lets the paint path underline just that character.
//!
//! Mnemonic *activation* belongs to the container: closing a menu, opening a
//! menu bar and focusing a button are container decisions. Widgets expose
//! [`crate::widget::Widget::mnemonic`] (the first key) and
//! [`crate::widget::Widget::mnemonic_key`] (a per-item key, a menu bar's
//! `Alt+F`), and [`crate::Ui`] routes `Alt+key` to them.

/// A label split into display text plus its access key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MnemonicLabel {
    /// Text with the `&` markers removed.
    text: String,
    /// Access key, if the label declared one.
    key: Option<char>,
    /// Byte offset of the key character inside `text`.
    key_byte: usize,
    /// UTF-8 length of the key character in bytes (0 when there is no key).
    key_len: usize,
}

impl MnemonicLabel {
    /// Split `label` into display text and the first `&`-marked access key.
    ///
    /// The first valid marker wins. A lone trailing `&` and a `&` followed by
    /// whitespace stay literal, so ordinary text does not lose characters.
    /// Later markers are dropped (the character is kept), matching the way
    /// Win32 menus treat them.
    pub fn parse(label: &str) -> Self {
        let mut text = String::with_capacity(label.len());
        let mut key = None;
        let mut key_byte = 0;
        let mut key_len = 0;
        let mut chars = label.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch != '&' {
                text.push(ch);
                continue;
            }
            match chars.peek() {
                Some('&') => {
                    chars.next();
                    text.push('&');
                }
                Some(next) if key.is_none() && !next.is_whitespace() => {
                    let next = *next;
                    chars.next();
                    key = Some(next);
                    key_byte = text.len();
                    key_len = next.len_utf8();
                    text.push(next);
                }
                Some(next) if next.is_whitespace() => {
                    // `& ` is a literal ampersand; the space stays in the
                    // stream.
                    text.push('&');
                }
                Some(next) => {
                    // A later marker is dropped but its character is kept.
                    let next = *next;
                    chars.next();
                    text.push(next);
                }
                None => text.push('&'),
            }
        }
        Self {
            text,
            key,
            key_byte,
            key_len,
        }
    }

    /// A label without an access key (already unescaped).
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    /// Text as painted.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The access key character, if any.
    pub fn key(&self) -> Option<char> {
        self.key
    }

    /// Whether `ch` (case-insensitively) is this label's access key.
    pub fn matches(&self, ch: char) -> bool {
        self.key.is_some_and(|key| key.eq_ignore_ascii_case(&ch))
    }

    /// Byte offset of the key character, when it starts a valid UTF-8 prefix.
    pub fn key_prefix(&self) -> Option<(&str, &str)> {
        let key = self.key?;
        let prefix = self.text.get(..self.key_byte)?;
        let rest = self.text.get(self.key_byte..)?;
        let (_, suffix) = rest.split_at_checked(key.len_utf8())?;
        Some((prefix, suffix))
    }

    /// Repaint cue for the key character while it is underlined.
    pub fn key_byte(&self) -> Option<usize> {
        self.key.map(|_| self.key_byte)
    }

    /// UTF-8 length of the key character.
    pub fn key_len(&self) -> usize {
        self.key_len
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

impl From<&str> for MnemonicLabel {
    fn from(label: &str) -> Self {
        Self::parse(label)
    }
}

impl From<&String> for MnemonicLabel {
    fn from(label: &String) -> Self {
        Self::parse(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_is_stripped_and_key_kept() {
        let label = MnemonicLabel::parse("&File");
        assert_eq!(label.text(), "File");
        assert_eq!(label.key(), Some('F'));
        assert!(label.matches('f'));
        assert!(!label.matches('i'));
        let (prefix, suffix) = label.key_prefix().expect("key prefix");
        assert_eq!(prefix, "");
        assert_eq!(suffix, "ile");
    }

    #[test]
    fn double_marker_is_a_literal_ampersand() {
        let label = MnemonicLabel::parse("Search && Replace &R");
        assert_eq!(label.text(), "Search & Replace R");
        assert_eq!(label.key(), Some('R'));
        let (prefix, suffix) = label.key_prefix().expect("key prefix");
        assert_eq!(prefix, "Search & Replace ");
        assert_eq!(suffix, "");
    }

    #[test]
    fn trailing_and_spaced_markers_stay_literal() {
        assert_eq!(MnemonicLabel::parse("Tom &").text(), "Tom &");
        assert_eq!(MnemonicLabel::parse("Tom & Jerry").text(), "Tom & Jerry");
        assert_eq!(MnemonicLabel::parse("Tom & Jerry").key(), None);
        // A marker followed by whitespace stays literal.
        let label = MnemonicLabel::parse("&A & B");
        assert_eq!(label.text(), "A & B");
        assert_eq!(label.key(), Some('A'));
    }

    #[test]
    fn later_markers_are_dropped_but_keep_their_character() {
        let label = MnemonicLabel::parse("&Save &As");
        assert_eq!(label.text(), "Save As");
        assert_eq!(label.key(), Some('S'));
        assert_eq!(label.key_prefix().map(|(prefix, _)| prefix), Some(""));
    }

    #[test]
    fn unicode_key_offsets_are_byte_offsets() {
        let label = MnemonicLabel::parse("文件(&F)");
        assert_eq!(label.text(), "文件(F)");
        assert_eq!(label.key(), Some('F'));
        let (prefix, suffix) = label.key_prefix().expect("key prefix");
        assert_eq!(prefix, "文件(");
        assert_eq!(suffix, ")");

        // A CJK access key underlines a whole character.
        let label = MnemonicLabel::parse("&文 件");
        assert_eq!(label.key(), Some('文'));
        assert_eq!(label.key_len(), '文'.len_utf8());
        // The suffix starts after the key character.
        assert_eq!(label.key_prefix(), Some(("", " 件")));
    }

    #[test]
    fn plain_label_has_no_key() {
        let label = MnemonicLabel::plain("Status");
        assert_eq!(label.text(), "Status");
        assert_eq!(label.key(), None);
        assert_eq!(label.key_prefix(), None);
    }
}
