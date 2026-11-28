// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Gamepad input mapped to the keyboard events focus navigation consumes.
//!
//! A pad is polled by the host (XInput, 33 ms is plenty for menus) and the
//! changes become ordinary [`KeyEvent`]s: the D-pad or left stick presses the
//! arrow keys, **A** is Enter, **B** is Escape, **RB** is Tab, **LB** is
//! Shift+Tab, **Y** opens the focus's context menu (Shift+F10) and **X** is
//! Space. The application forwards the keys to [`crate::Ui::pad_key`] (or
//! [`crate::Ui::key`]), so a page written for the keyboard works without a
//! second input model; `pad_key` also raises the Windows on-screen keyboard
//! when a pad opens a text field.
//!
//! Held directions repeat (400 ms delay, then every 90 ms) so a long list
//! scrolls without re-tapping. The mapping is pure and tested without hardware;
//! [`read_state`] is the only platform call.

use crate::event::{KeyCode, KeyEvent};
use std::time::{Duration, Instant};

/// Button bits (the XInput `wButtons` layout).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PadButtons(pub u16);

impl PadButtons {
    pub const DPAD_UP: u16 = 0x0001;
    pub const DPAD_DOWN: u16 = 0x0002;
    pub const DPAD_LEFT: u16 = 0x0004;
    pub const DPAD_RIGHT: u16 = 0x0008;
    pub const START: u16 = 0x0010;
    pub const BACK: u16 = 0x0020;
    pub const LEFT_THUMB: u16 = 0x0040;
    pub const RIGHT_THUMB: u16 = 0x0080;
    pub const LEFT_SHOULDER: u16 = 0x0100;
    pub const RIGHT_SHOULDER: u16 = 0x0200;
    pub const A: u16 = 0x1000;
    pub const B: u16 = 0x2000;
    pub const X: u16 = 0x4000;
    pub const Y: u16 = 0x8000;

    pub fn contains(self, bit: u16) -> bool {
        self.0 & bit != 0
    }

    /// Bits pressed in `self` but not in `previous`.
    pub fn pressed_since(self, previous: PadButtons) -> PadButtons {
        PadButtons(self.0 & !previous.0)
    }
}

/// One pad sample: buttons, both sticks (-1.0..=1.0, y up) and triggers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadState {
    pub buttons: PadButtons,
    pub left_stick: (f32, f32),
    pub right_stick: (f32, f32),
    pub left_trigger: u8,
    pub right_trigger: u8,
}

/// Analog dead zone for a stick to count as a direction.
const STICK_THRESHOLD: f32 = 0.55;
/// A held direction fires on press, then after this delay…
const REPEAT_DELAY: Duration = Duration::from_millis(400);
/// …and then this often.
const REPEAT_INTERVAL: Duration = Duration::from_millis(90);

/// One slot's polling state: the previous buttons and the direction repeat.
pub struct Gamepad {
    slot: u32,
    /// The application switch: a disabled pad is polled but emits nothing.
    enabled: bool,
    previous: PadButtons,
    direction: Option<KeyCode>,
    next_repeat: Option<Instant>,
}

impl Gamepad {
    pub fn new(slot: u32) -> Self {
        Self {
            slot: slot.min(3),
            enabled: true,
            previous: PadButtons::default(),
            direction: None,
            next_repeat: None,
        }
    }

    pub fn slot(&self) -> u32 {
        self.slot
    }

    /// Application switch for the whole gamepad layer. Disabling it drops the
    /// held-direction state, so re-enabling never resumes a stale repeat.
    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.enabled = enabled;
            self.reset();
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Forget the held state (window deactivated, pad unplugged, disabled).
    /// The next press always fires immediately instead of resuming a repeat.
    pub fn reset(&mut self) {
        self.previous = PadButtons::default();
        self.direction = None;
        self.next_repeat = None;
    }

    /// Read the pad and translate the change since the last update. `active`
    /// is the window's focus state: a background window never receives pad
    /// navigation (the same contract as a console dashboard).
    pub fn poll(&mut self, now: Instant, active: bool) -> Vec<KeyEvent> {
        if !self.enabled || !active {
            self.reset();
            return Vec::new();
        }
        match read_state(self.slot) {
            Some(state) => self.update(state, now),
            None => {
                self.reset();
                Vec::new()
            }
        }
    }

    /// Translate one sample (the pure half of [`Gamepad::poll`]).
    pub fn update(&mut self, state: PadState, now: Instant) -> Vec<KeyEvent> {
        let mut keys = Vec::new();
        let pressed = state.buttons.pressed_since(self.previous);
        self.previous = state.buttons;
        // Face/shoulder buttons are edge-triggered.
        for (bit, code, shift) in [
            (PadButtons::A, KeyCode::Enter, false),
            (PadButtons::START, KeyCode::Enter, false),
            (PadButtons::B, KeyCode::Escape, false),
            (PadButtons::BACK, KeyCode::Escape, false),
            (PadButtons::X, KeyCode::Space, false),
            (PadButtons::Y, KeyCode::Function(10), true),
            (PadButtons::RIGHT_SHOULDER, KeyCode::Tab, false),
            (PadButtons::LEFT_SHOULDER, KeyCode::Tab, true),
        ] {
            if pressed.contains(bit) {
                keys.push(key(code, shift));
            }
        }
        // A direction is held, not tapped: repeat while it stays the same.
        let direction = direction_of(state);
        if direction != self.direction {
            self.direction = direction;
            match direction {
                Some(code) => {
                    keys.push(key(code, false));
                    self.next_repeat = Some(now + REPEAT_DELAY);
                }
                None => self.next_repeat = None,
            }
        } else if let (Some(code), Some(next)) = (direction, self.next_repeat)
            && now >= next
        {
            keys.push(key(code, false));
            self.next_repeat = Some(next + REPEAT_INTERVAL);
        }
        keys
    }
}

/// The dominant direction of the D-pad or left stick, if any.
fn direction_of(state: PadState) -> Option<KeyCode> {
    let buttons = state.buttons;
    if buttons.contains(PadButtons::DPAD_UP) {
        return Some(KeyCode::Up);
    }
    if buttons.contains(PadButtons::DPAD_DOWN) {
        return Some(KeyCode::Down);
    }
    if buttons.contains(PadButtons::DPAD_LEFT) {
        return Some(KeyCode::Left);
    }
    if buttons.contains(PadButtons::DPAD_RIGHT) {
        return Some(KeyCode::Right);
    }
    let (x, y) = state.left_stick;
    if x.abs() < STICK_THRESHOLD && y.abs() < STICK_THRESHOLD {
        return None;
    }
    Some(if x.abs() >= y.abs() {
        if x < 0.0 {
            KeyCode::Left
        } else {
            KeyCode::Right
        }
    } else if y < 0.0 {
        // XInput y grows upward, the keyboard's Down is negative y.
        KeyCode::Down
    } else {
        KeyCode::Up
    })
}

fn key(code: KeyCode, shift: bool) -> KeyEvent {
    KeyEvent {
        code,
        vk: 0,
        ctrl: false,
        shift,
        alt: false,
        repeat: false,
    }
}

/// Read one pad slot. `None` means no controller is connected or readable.
#[cfg(windows)]
pub fn read_state(slot: u32) -> Option<PadState> {
    use windows::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_STATE};
    let mut state = XINPUT_STATE::default();
    // ERROR_SUCCESS is zero; anything else is "not connected".
    if unsafe { XInputGetState(slot.min(3), &mut state) } != 0 {
        return None;
    }
    let pad = state.Gamepad;
    Some(PadState {
        buttons: PadButtons(pad.wButtons.0),
        left_stick: (
            pad.sThumbLX as f32 / 32768.0,
            pad.sThumbLY as f32 / 32768.0,
        ),
        right_stick: (
            pad.sThumbRX as f32 / 32768.0,
            pad.sThumbRY as f32 / 32768.0,
        ),
        left_trigger: pad.bLeftTrigger,
        right_trigger: pad.bRightTrigger,
    })
}

#[cfg(not(windows))]
pub fn read_state(_slot: u32) -> Option<PadState> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(buttons: u16, stick: (f32, f32)) -> PadState {
        PadState {
            buttons: PadButtons(buttons),
            left_stick: stick,
            ..Default::default()
        }
    }

    #[test]
    fn face_and_shoulder_buttons_become_edge_triggered_keys() {
        let mut pad = Gamepad::new(0);
        let now = Instant::now();
        let keys = pad.update(state(PadButtons::A, (0.0, 0.0)), now);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].code, KeyCode::Enter);
        // Holding A does not repeat.
        assert!(pad.update(state(PadButtons::A, (0.0, 0.0)), now).is_empty());
        // B is Escape, RB is Tab, LB is Shift+Tab, Y is the context chord.
        let keys = pad.update(
            state(
                PadButtons::B | PadButtons::RIGHT_SHOULDER | PadButtons::Y,
                (0.0, 0.0),
            ),
            now,
        );
        let codes: Vec<KeyCode> = keys.iter().map(|key| key.code).collect();
        assert!(codes.contains(&KeyCode::Escape));
        assert!(codes.contains(&KeyCode::Tab));
        let context = keys
            .iter()
            .find(|key| key.code == KeyCode::Function(10))
            .expect("Y is Shift+F10");
        assert!(context.shift);
        // Releasing everything and pressing LB alone is Shift+Tab.
        pad.update(state(0, (0.0, 0.0)), now);
        let keys = pad.update(state(PadButtons::LEFT_SHOULDER, (0.0, 0.0)), now);
        assert_eq!(keys[0].code, KeyCode::Tab);
        assert!(keys[0].shift);
    }

    #[test]
    fn the_enable_switch_drops_held_state() {
        let mut pad = Gamepad::new(0);
        let now = Instant::now();
        pad.update(state(PadButtons::A | PadButtons::DPAD_DOWN, (0.0, 0.0)), now);
        assert!(pad.is_enabled());
        pad.set_enabled(false);
        assert!(!pad.is_enabled());
        pad.set_enabled(true);
        // The held state was forgotten, so pressing A again is a fresh edge.
        let keys = pad.update(state(PadButtons::A, (0.0, 0.0)), now);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].code, KeyCode::Enter);
        // `reset` does the same for a window that lost focus.
        pad.update(state(PadButtons::DPAD_UP, (0.0, 0.0)), now);
        pad.reset();
        assert_eq!(
            pad.update(state(PadButtons::DPAD_UP, (0.0, 0.0)), now)[0].code,
            KeyCode::Up
        );
    }

    #[test]
    fn a_held_direction_repeats_after_the_delay() {
        let mut pad = Gamepad::new(0);
        let start = Instant::now();
        let down = state(PadButtons::DPAD_DOWN, (0.0, 0.0));
        assert_eq!(pad.update(down, start)[0].code, KeyCode::Down);
        assert!(pad.update(down, start + Duration::from_millis(100)).is_empty());
        assert!(pad.update(down, start + Duration::from_millis(399)).is_empty());
        assert_eq!(
            pad.update(down, start + REPEAT_DELAY)[0].code,
            KeyCode::Down
        );
        assert_eq!(
            pad.update(down, start + REPEAT_DELAY + REPEAT_INTERVAL)[0].code,
            KeyCode::Down
        );
        // Releasing and re-pressing fires immediately again.
        pad.update(state(0, (0.0, 0.0)), start);
        assert_eq!(
            pad.update(state(PadButtons::DPAD_UP, (0.0, 0.0)), start)[0].code,
            KeyCode::Up
        );
    }

    #[test]
    fn the_left_stick_maps_its_dominant_axis() {
        assert_eq!(direction_of(state(0, (0.9, 0.2))), Some(KeyCode::Right));
        assert_eq!(direction_of(state(0, (-0.9, -0.7))), Some(KeyCode::Left));
        assert_eq!(direction_of(state(0, (0.1, 0.8))), Some(KeyCode::Up));
        assert_eq!(direction_of(state(0, (0.2, -0.8))), Some(KeyCode::Down));
        assert_eq!(direction_of(state(0, (0.3, 0.3))), None);
        // The D-pad wins over a diagonal stick.
        assert_eq!(
            direction_of(state(PadButtons::DPAD_LEFT, (0.9, 0.9))),
            Some(KeyCode::Left)
        );
    }
}
