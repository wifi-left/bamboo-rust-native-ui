// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Global hotkeys: parsing, registration and capture.
//!
//! A [`Hotkey`] is a modifier set plus a virtual-key code in the same numeric
//! space as [`crate::KeyEvent::vk`], so no Win32 type or constant appears in
//! this module's public API. The host registers it with `RegisterHotKey`
//! (system-wide, delivered as [`crate::Host::hotkey`] while the window exists)
//! and can capture the next chord for a settings UI
//! ([`crate::Host::hotkey_captured`]).
//!
//! Plain printable keys require a modifier: `parse` rejects them because a
//! global `A` shortcut would swallow ordinary typing everywhere.

use crate::event::{KeyCode, KeyEvent};

/// Modifier buttons held with the shortcut key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HotkeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
}

impl HotkeyModifiers {
    pub const NONE: Self = Self {
        ctrl: false,
        shift: false,
        alt: false,
        win: false,
    };
    pub const CTRL: Self = Self {
        ctrl: true,
        ..Self::NONE
    };
    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };
    pub const ALT: Self = Self {
        alt: true,
        ..Self::NONE
    };
    pub const WIN: Self = Self {
        win: true,
        ..Self::NONE
    };

    pub fn is_empty(&self) -> bool {
        !self.ctrl && !self.shift && !self.alt && !self.win
    }

    pub fn with_ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    pub fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }

    pub fn with_alt(mut self) -> Self {
        self.alt = true;
        self
    }

    pub fn with_win(mut self) -> Self {
        self.win = true;
        self
    }
}

/// A global shortcut: modifiers plus a virtual-key code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub modifiers: HotkeyModifiers,
    /// Virtual-key code (compare with `KeyEvent::vk`).
    pub vk: u16,
}

impl Hotkey {
    /// Build a hotkey from a named key. `None` when the key has no stable
    /// virtual-key code.
    pub fn new(modifiers: HotkeyModifiers, key: KeyCode) -> Option<Self> {
        key.to_vk().map(|vk| Self { modifiers, vk })
    }

    /// Build from a raw virtual-key code (`KeyEvent::vk`).
    pub fn from_vk(modifiers: HotkeyModifiers, vk: u16) -> Self {
        Self { modifiers, vk }
    }

    /// The key as a named [`KeyCode`] (or `Other(vk)`).
    pub fn key(&self) -> KeyCode {
        KeyCode::from_vk(self.vk)
    }

    /// Parse `"Ctrl+Shift+M"`, `"Alt+F4"`, `"Ctrl+Alt+Delete"`, ...
    ///
    /// Case-insensitive; modifiers may appear in any order. A shortcut must
    /// have at least one modifier unless the key is a function key, so a bare
    /// letter or digit is rejected.
    pub fn parse(spec: &str) -> Option<Self> {
        let mut modifiers = HotkeyModifiers::NONE;
        let mut key = None;
        for token in spec.split('+') {
            let name = token.trim();
            if name.is_empty() {
                return None;
            }
            match name.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "shift" => modifiers.shift = true,
                "alt" => modifiers.alt = true,
                "win" | "windows" | "meta" | "super" => modifiers.win = true,
                other => {
                    if key.is_some() {
                        return None;
                    }
                    key = parse_key(other);
                }
            }
        }
        let hotkey = Self {
            modifiers,
            vk: key?.to_vk()?,
        };
        hotkey.is_global_safe().then_some(hotkey)
    }

    /// `"Ctrl+Shift+M"` style label for menus and help text.
    pub fn label(&self) -> String {
        let mut label = String::new();
        let modifiers = self.modifiers;
        if modifiers.ctrl {
            label.push_str("Ctrl+");
        }
        if modifiers.alt {
            label.push_str("Alt+");
        }
        if modifiers.shift {
            label.push_str("Shift+");
        }
        if modifiers.win {
            label.push_str("Win+");
        }
        label.push_str(&key_label(self.key()));
        label
    }

    /// Whether this shortcut is safe to register globally: it has a modifier
    /// or the key is a function key. `parse` enforces this; an application
    /// that builds a hotkey from raw input should check it too.
    pub fn is_global_safe(&self) -> bool {
        !self.modifiers.is_empty() || matches!(self.key(), KeyCode::Function(_))
    }

    /// Capture a hotkey from a keyboard event, using the event's modifier
    /// state. Shift/Alt state may still be settling when a modifier itself was
    /// just pressed; the host samples `GetKeyState` for capture instead and
    /// uses this helper for tests and application code.
    pub fn from_key_event(event: &KeyEvent) -> Option<Self> {
        let vk = event.vk;
        if vk == 0 || is_modifier_vk(vk) {
            return None;
        }
        let modifiers = HotkeyModifiers {
            ctrl: event.ctrl,
            shift: event.shift,
            alt: event.alt,
            win: false,
        };
        let hotkey = Self { modifiers, vk };
        hotkey.is_global_safe().then_some(hotkey)
    }
}

/// Whether `vk` is a modifier key that only contributes to the chord.
pub fn is_modifier_vk(vk: u16) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
}

fn parse_key(name: &str) -> Option<KeyCode> {
    if name.len() == 1 {
        let ch = name.chars().next()?;
        if ch.is_ascii_alphabetic() {
            return Some(KeyCode::Letter(ch.to_ascii_uppercase() as u8 - b'A'));
        }
        if ch.is_ascii_digit() {
            return Some(KeyCode::Digit(ch as u8 - b'0'));
        }
    }
    match name {
        "enter" | "return" => Some(KeyCode::Enter),
        "space" => Some(KeyCode::Space),
        "tab" => Some(KeyCode::Tab),
        "escape" | "esc" => Some(KeyCode::Escape),
        "backspace" | "bksp" => Some(KeyCode::Backspace),
        "delete" | "del" => Some(KeyCode::Delete),
        "insert" | "ins" => Some(KeyCode::Insert),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" | "pgup" => Some(KeyCode::PageUp),
        "pagedown" | "pgdn" => Some(KeyCode::PageDown),
        "up" | "arrowup" => Some(KeyCode::Up),
        "down" | "arrowdown" => Some(KeyCode::Down),
        "left" | "arrowleft" => Some(KeyCode::Left),
        "right" | "arrowright" => Some(KeyCode::Right),
        _ => {
            // `label` prints an unmapped virtual key as `VK2C`; accept it back
            // so a persisted label round-trips for any code.
            if let Some(hex) = name.strip_prefix("vk")
                && let Ok(vk) = u16::from_str_radix(hex, 16)
            {
                return Some(KeyCode::from_vk(vk));
            }
            let number: u8 = name.strip_prefix('f')?.parse().ok()?;
            (1..=24)
                .contains(&number)
                .then_some(KeyCode::Function(number))
        }
    }
}

fn key_label(key: KeyCode) -> String {
    match key {
        KeyCode::Letter(letter) => char::from(b'A' + letter).to_string(),
        KeyCode::Digit(digit) => char::from(b'0' + digit).to_string(),
        KeyCode::Function(number) => format!("F{number}"),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Space => "Space".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Escape => "Esc".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Insert => "Insert".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Other(vk) => format!("VK{vk:02X}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modifiers_and_labels_round_trip() {
        let hotkey = Hotkey::parse("ctrl+Shift+m").unwrap();
        assert!(hotkey.modifiers.ctrl && hotkey.modifiers.shift);
        assert_eq!(hotkey.key(), KeyCode::Letter(12));
        assert_eq!(hotkey.label(), "Ctrl+Shift+M");

        let keys = Hotkey::parse("Alt+F10").unwrap();
        assert_eq!(keys.key(), KeyCode::Function(10));
        assert_eq!(keys.label(), "Alt+F10");

        let nav = Hotkey::parse("Control+Alt+PageDown").unwrap();
        assert_eq!(nav.key(), KeyCode::PageDown);
        assert_eq!(nav.label(), "Ctrl+Alt+PageDown");
    }

    #[test]
    fn labels_round_trip_through_parse_for_persistence() {
        for spec in [
            "Ctrl+Shift+M",
            "Alt+F10",
            "Ctrl+Alt+PageDown",
            "Shift+Esc",
            "Win+VK2C",
        ] {
            let hotkey = Hotkey::parse(spec).unwrap_or_else(|| panic!("parse {spec}"));
            let label = hotkey.label();
            assert_eq!(
                Hotkey::parse(&label),
                Some(hotkey),
                "label {label:?} did not round-trip"
            );
        }
    }

    #[test]
    fn bare_printable_keys_are_rejected_but_function_keys_are_not() {
        assert!(Hotkey::parse("M").is_none());
        assert!(Hotkey::parse("1").is_none());
        assert!(Hotkey::parse("Enter").is_none());
        assert_eq!(Hotkey::parse("F9").unwrap().key(), KeyCode::Function(9));
        assert!(Hotkey::parse("Shift+A").is_some());
    }

    #[test]
    fn malformed_specs_are_rejected() {
        for spec in [
            "",
            "+",
            "Ctrl+",
            "Ctrl+Shift",
            "Ctrl+A+B",
            "Hyper+A",
            "Ctrl+F25",
            "Ctrl+Unknown",
            "Ctrl++A",
        ] {
            assert!(Hotkey::parse(spec).is_none(), "accepted {spec:?}");
        }
    }

    #[test]
    fn from_key_event_uses_the_event_modifiers() {
        let event = KeyEvent {
            code: KeyCode::Letter(12),
            vk: 0x4D,
            ctrl: true,
            shift: true,
            alt: false,
            repeat: false,
        };
        let hotkey = Hotkey::from_key_event(&event).unwrap();
        assert_eq!(hotkey, Hotkey::parse("Ctrl+Shift+M").unwrap());
        // A bare letter event is not a valid global shortcut.
        let plain = KeyEvent {
            ctrl: false,
            shift: false,
            ..event
        };
        assert!(Hotkey::from_key_event(&plain).is_none());
        // Modifier keys never finish a chord on their own.
        let modifier = KeyEvent {
            code: KeyCode::Other(0x11),
            vk: 0x11,
            ..event
        };
        assert!(Hotkey::from_key_event(&modifier).is_none());
    }

    #[test]
    fn virtual_key_codes_round_trip() {
        for key in [
            KeyCode::Letter(0),
            KeyCode::Letter(25),
            KeyCode::Digit(0),
            KeyCode::Digit(9),
            KeyCode::Function(1),
            KeyCode::Function(24),
            KeyCode::Enter,
            KeyCode::PageDown,
            KeyCode::Other(0xBA),
        ] {
            let vk = key.to_vk().unwrap();
            assert_eq!(KeyCode::from_vk(vk), key);
        }
        assert_eq!(KeyCode::Other(0).to_vk(), None);
    }
}
