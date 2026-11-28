# Gamepad

> Translate an XInput controller into the keyboard events focus navigation already consumes.

## Overview

A pad is polled by the host (XInput; 33 ms is plenty for menus) and the changes
become ordinary `KeyEvent`s: the D-pad or left stick presses the arrow keys, **A**
is Enter, **B** is Escape, **RB** is Tab, **LB** is Shift+Tab, **Y** opens the
focused widget's context menu (`Shift+F10`) and **X** is Space. The application
forwards the keys to `Ui::pad_key` (or `Ui::key`), so a page written for the
keyboard works without a second input model.

`pad_key` also raises the Windows on-screen keyboard when a pad opens a text
field, and routes the pad to that keyboard while it is up. Held directions repeat
(400 ms delay, then every 90 ms) so a long list scrolls without re-tapping.

The `Gamepad`, `PadButtons`, `PadState` models and the pure `update` half are
cross-platform and testable without hardware; `read_state` is the only platform
call and returns `None` off Windows. The soft keyboard is Windows-only.

## Key types

### `PadButtons`

```rust
pub struct PadButtons(pub u16);
```

Bit constants: `DPAD_UP`, `DPAD_DOWN`, `DPAD_LEFT`, `DPAD_RIGHT`, `START`,
`BACK`, `LEFT_THUMB`, `RIGHT_THUMB`, `LEFT_SHOULDER`, `RIGHT_SHOULDER`, `A`,
`B`, `X`, `Y`. Methods: `contains(bit) -> bool`,
`pressed_since(previous) -> PadButtons` (bits newly down).

### `PadState`

```rust
pub struct PadState {
    pub buttons: PadButtons,
    pub left_stick: (f32, f32),
    pub right_stick: (f32, f32),
    pub left_trigger: u8,
    pub right_trigger: u8,
}
```

Sticks are `-1.0..=1.0` with y up; triggers are `0..=255`.

### `Gamepad`

```rust
pub struct Gamepad { /* private */ }

impl Gamepad {
    pub fn new(slot: u32) -> Self; // clamped to 0..=3
}
```

| Method | Signature | Description |
|---|---|---|
| `poll` | `poll(&mut self, now: Instant, active: bool) -> Vec<KeyEvent>` | Read the pad and translate the change; inactive/disabled resets. |
| `update` | `update(&mut self, state: PadState, now: Instant) -> Vec<KeyEvent>` | The pure half of `poll`. |
| `slot` | `slot(&self) -> u32` | The XInput slot. |
| `set_enabled` | `set_enabled(&mut self, enabled: bool)` | Switch the layer off; drops held state. |
| `is_enabled` | `is_enabled(&self) -> bool` | Current switch. |
| `reset` | `reset(&mut self)` | Forget held state (deactivation, unplug). |

`read_state(slot: u32) -> Option<PadState>` reads one slot (`None` when no pad is
connected). It is re-exported at the crate root as `read_gamepad`.

### Soft keyboard

`bamboo_native_ui::soft_keyboard` re-exports `platform::keyboard` on Windows:
`show(layout: Layout) -> bool`, `hide() -> bool`, `is_open() -> bool`, with
`Layout { Gamepad, Touch }`. `Ui::pad_key` calls the sink's soft-keyboard hooks;
applications rarely call the module directly.

### Methods on `Ui`

`Ui::pad_key(&mut self, key: KeyEvent, sink: &mut impl UiHost) -> bool` routes a
synthetic key. It returns whether the key was consumed.

## Example

Poll on a timer and forward the keys:

```rust
use std::time::{Duration, Instant};
use bamboo_native_ui::{read_gamepad, Gamepad, WindowHandle};

fn attach_pad(&mut self, window: &mut bamboo_native_ui::WindowHandle<'_>) {
    if read_gamepad(0).is_some() {
        self.pad = Some(Gamepad::new(0));
        window.set_animation_timer(Some(Duration::from_millis(33)));
    }
}

fn timer(&mut self, window: &mut WindowHandle<'_>) {
    let Some(pad) = self.pad.as_mut() else {
        return;
    };
    for key in pad.poll(Instant::now(), window.is_foreground()) {
        if self.ui.pad_key(key, window) {
            self.drain(window);
        }
    }
}
```

`window.is_foreground()` is the `active` flag: a background window never
receives pad navigation, the same contract as a console dashboard. When the pad
goes away, drop it and the slow watch tick can re-attach it later.

## Notes and limits

- `poll` returns `Vec<KeyEvent>`; an empty vector means no change. Face and
  shoulder buttons are edge-triggered (holding A does not repeat); directions are
  held and repeat after `400 ms`, then every `90 ms`.
- The left stick counts as a direction past a `0.55` dead zone on its dominant
  axis; the D-pad wins over a diagonal stick. XInput y grows upward, so negative
  y maps to Down.
- Synthetic keys carry `vk: 0`; compare with `KeyEvent::code`, not `vk`.
- `pad_key` raises the on-screen keyboard when the pad opens a text field. While
  the keyboard is up it owns the pad and keys are not forwarded; **A** re-raises
  a dismissed keyboard, **B** closes it and leaves the field (committing).
- A platform without XInput returns `None` from `read_state`; the pure `update`
  path still works for tests and synthetic input.
- Polling and the soft keyboard are Windows-only; the models compile everywhere.

## See also

- `hotkeys.md` (bind a chord to an action)
- `menus.md` (Y opens the focused context menu)
- `accessibility.md` (announce focus movement)
- `../widgets/controls/toggle.md`
