// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Normalized input events.
//!
//! The backend turns `WM_*` messages into these types; widgets never see
//! Win32 constants. Positions are in DIPs, so hit-testing and layout share the
//! same coordinate space.

use crate::geometry::{Point, Size};

/// A named key, or an unclassified virtual-key code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Backspace,
    Tab,
    Enter,
    Escape,
    Space,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    /// `A`..`Z` as 0..=25.
    Letter(u8),
    /// `0`..`9` as 0..=9.
    Digit(u8),
    /// `F1`..`F24` as 1..=24.
    Function(u8),
    /// Any other virtual-key code.
    Other(u16),
}

impl KeyCode {
    pub fn is_letter(self, letter: u8) -> bool {
        matches!(self, KeyCode::Letter(value) if value.eq_ignore_ascii_case(&letter))
    }

    /// Resolve a Windows virtual-key code. Platform-neutral on purpose: the
    /// numeric values happen to match Win32 VK constants.
    pub fn from_vk(vk: u16) -> Self {
        match vk {
            0x08 => KeyCode::Backspace,
            0x09 => KeyCode::Tab,
            0x0D => KeyCode::Enter,
            0x1B => KeyCode::Escape,
            0x20 => KeyCode::Space,
            0x25 => KeyCode::Left,
            0x26 => KeyCode::Up,
            0x27 => KeyCode::Right,
            0x28 => KeyCode::Down,
            0x24 => KeyCode::Home,
            0x23 => KeyCode::End,
            0x21 => KeyCode::PageUp,
            0x22 => KeyCode::PageDown,
            0x2D => KeyCode::Insert,
            0x2E => KeyCode::Delete,
            0x41..=0x5A => KeyCode::Letter((vk - 0x41) as u8),
            0x30..=0x39 => KeyCode::Digit((vk - 0x30) as u8),
            0x70..=0x87 => KeyCode::Function((vk - 0x70 + 1) as u8),
            other => KeyCode::Other(other),
        }
    }

    /// Virtual-key code for this key (the inverse of [`KeyCode::from_vk`]).
    /// `None` for an unclassified zero code or an out-of-range function key.
    pub const fn to_vk(self) -> Option<u16> {
        match self {
            KeyCode::Backspace => Some(0x08),
            KeyCode::Tab => Some(0x09),
            KeyCode::Enter => Some(0x0D),
            KeyCode::Escape => Some(0x1B),
            KeyCode::Space => Some(0x20),
            KeyCode::Left => Some(0x25),
            KeyCode::Up => Some(0x26),
            KeyCode::Right => Some(0x27),
            KeyCode::Down => Some(0x28),
            KeyCode::Home => Some(0x24),
            KeyCode::End => Some(0x23),
            KeyCode::PageUp => Some(0x21),
            KeyCode::PageDown => Some(0x22),
            KeyCode::Insert => Some(0x2D),
            KeyCode::Delete => Some(0x2E),
            KeyCode::Letter(letter) => Some(0x41 + letter as u16),
            KeyCode::Digit(digit) => Some(0x30 + digit as u16),
            KeyCode::Function(number) => {
                if number >= 1 && number <= 24 {
                    Some(0x70 + (number as u16 - 1))
                } else {
                    None
                }
            }
            KeyCode::Other(0) => None,
            KeyCode::Other(vk) => Some(vk),
        }
    }
}

/// A keyboard event with modifier state sampled at dispatch time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub vk: u16,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Auto-repeat (the key was already down).
    pub repeat: bool,
}

/// Mouse button identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

/// Whether a pointer event was left, right, middle, etc.
impl MouseButton {
    pub fn from_wparam(wparam: u32) -> Option<Self> {
        match wparam {
            0x0001 => Some(MouseButton::Left),
            0x0002 => Some(MouseButton::Right),
            0x0004 => Some(MouseButton::Middle),
            0x0005 => Some(MouseButton::Other(4)),
            0x0006 => Some(MouseButton::Other(5)),
            _ => None,
        }
    }
}

pub use bamboo_native_ui_layout_core::event::Cursor;

/// Keyboard modifiers held during a pointer event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointerModifiers {
    pub ctrl: bool,
    pub shift: bool,
}

impl PointerModifiers {
    pub const NONE: Self = Self {
        ctrl: false,
        shift: false,
    };

    pub const fn ctrl(ctrl: bool) -> Self {
        Self { ctrl, shift: false }
    }

    pub const fn shift(shift: bool) -> Self {
        Self { ctrl: false, shift }
    }

    /// Whether any modifier is held (the plain click is the default).
    pub fn any(self) -> bool {
        self.ctrl || self.shift
    }
}

/// What produced the pointer sequence that follows. A host sends
/// [`Event::PointerKind`] before the first pointer event of a sequence when
/// the device changes; a widget uses it to choose a touch-friendly input model
/// (the painted editor's long-press selection and selection handles) without
/// asking the user. A host that cannot tell devices just leaves it at `Mouse`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerKind {
    #[default]
    Mouse,
    Touch,
    Pen,
}

/// A normalized input event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    PointerMove {
        pos: Point,
        /// Bitmask of currently pressed buttons (MOUSE_BUTTON_* style bits).
        buttons: u32,
    },
    PointerDown {
        pos: Point,
        button: MouseButton,
        /// Ctrl/Shift held at press time (a multi-select list uses them on
        /// release, but hosts report both ends of the click).
        modifiers: PointerModifiers,
    },
    PointerUp {
        pos: Point,
        button: MouseButton,
        modifiers: PointerModifiers,
    },
    /// A double click reported by the platform (`CS_DBLCLKS`). The button-down
    /// and button-up that precede it are delivered as normal pointer events.
    PointerDoubleClick {
        pos: Point,
        button: MouseButton,
    },
    PointerLeave,
    /// The device driving the following pointer events. Sent by the host when
    /// it changes (a mouse and a touch screen can drive one window); the
    /// container stores it and the painted editor switches to touch
    /// selection (long press, handles, on-screen keyboard).
    PointerKind(PointerKind),
    /// A touch press held in place (~500 ms) without moving. The host owns the
    /// dwell (it has the timer); the application forwards it to
    /// [`crate::Ui::long_press`], which selects the word under the finger in a
    /// painted text field. A host without touch never sends it.
    LongPress {
        pos: Point,
    },
    Wheel {
        pos: Point,
        /// Positive means scroll up / left.
        delta_x: f32,
        delta_y: f32,
    },
    Key(KeyEvent),
    /// A key release. The host forwards it to [`crate::Ui::key_up`], which
    /// hands it to the focused widget (the edit-box `on_key_up` hook). It
    /// carries no navigation meaning.
    KeyUp(KeyEvent),
    /// The user tapped Alt (pressed and released it without a chord): access
    /// keys stay underlined until the next key, pointer press or focus change,
    /// which sends `false`. A held Alt chord does not toggle the mode.
    Alt(bool),
    Char(char),
    Focus(bool),
    Resize(Size),
    DpiChanged(f32),
}
