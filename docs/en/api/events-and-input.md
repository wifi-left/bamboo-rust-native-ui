# Events and input

> Normalized, platform-neutral input events and the names the Windows host turns Win32 messages into.

## Overview

`bamboo_native_ui::event` defines the vocabulary the host hands to the
application. The backend converts `WM_*` messages into these values; a widget
never sees a Win32 constant, and every position is in DIPs. The flow is: the
Windows host produces an `Event`, `Host::event` routes it (a `Ui`-based app maps
it onto the matching `Ui` method), and `Ui` resolves hit-testing and focus
before calling the widget with a `WidgetInput` (see
[The widget trait](./widget-trait.md)).

The types are `Copy` value types with no event-object identity and no
allocation. Only `KeyCode::from_vk`/`to_vk`, `MouseButton::from_wparam` and the
`Color` `COLORREF` helpers carry Windows-shaped numbers, and they are
documented as such.

## Key types

### `Event`

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    PointerMove { pos: Point, buttons: u32 },          // -> pointer_move
    PointerDown { pos: Point, button: MouseButton, modifiers: PointerModifiers },
    PointerUp { pos: Point, button: MouseButton, modifiers: PointerModifiers },
    PointerDoubleClick { pos: Point, button: MouseButton }, // -> pointer_double_click
    PointerLeave,                                      // -> pointer_leave
    PointerKind(PointerKind),                          // -> set_pointer_kind
    LongPress { pos: Point },                          // -> long_press
    Wheel { pos: Point, delta_x: f32, delta_y: f32 },  // -> wheel_xy
    Key(KeyEvent),                                     // -> key
    KeyUp(KeyEvent),                                   // -> key_up (observation)
    Alt(bool),                                         // -> set_mnemonic_underlines
    Char(char),                                        // -> char
    Focus(bool),                                       // window focus gained/lost
    Resize(Size),                                      // -> Host::resize
    DpiChanged(f32),                                   // -> Host::dpi_changed
}
```

`PointerDown`/`PointerUp` route through `pointer_down_with` /
`pointer_up_with` (press-time and release-time modifiers). `Wheel` is positive
up/left. `Alt(true)` follows a bare Alt tap; a chord or focus change sends
`false`. The bundled Windows host reports size and DPI through `Host::resize` /
`Host::dpi_changed` directly, so `Resize`/`DpiChanged` are for hosts that
forward them. A double click does not replace the press/release pair.

### `KeyCode`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Backspace, Tab, Enter, Escape, Space,
    Left, Right, Up, Down, Home, End, PageUp, PageDown, Insert, Delete,
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
    pub fn is_letter(self, letter: u8) -> bool;
    pub fn from_vk(vk: u16) -> Self;
    pub const fn to_vk(self) -> Option<u16>;
}
```

`from_vk`/`to_vk` map Win32 virtual-key codes; the numbers happen to match VK
constants but no Win32 type is named. `to_vk` returns `None` for `Other(0)` and
an out-of-range `Function`.

### `KeyEvent`, `MouseButton`, `Cursor`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode, pub vk: u16,
    pub ctrl: bool, pub shift: bool, pub alt: bool,
    /// Auto-repeat (the key was already down).
    pub repeat: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton { Left, Right, Middle, Other(u16) }
impl MouseButton { pub fn from_wparam(wparam: u32) -> Option<Self>; }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cursor { #[default] Arrow, Hand, IBeam, SizeWE, SizeNS }
```

`Cursor` tells the host which system cursor to show; a widget suggests one
through `Widget::cursor`, reported to the application by `Ui::hover_cursor`.

### `PointerModifiers` and `PointerKind`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointerModifiers { pub ctrl: bool, pub shift: bool }
impl PointerModifiers {
    pub const NONE: Self;
    pub const fn ctrl(ctrl: bool) -> Self;
    pub const fn shift(shift: bool) -> Self;
    pub fn any(self) -> bool;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerKind { #[default] Mouse, Touch, Pen }
```

`PointerKind` is sent before the first pointer event of a sequence when the
device changes; a painted editor uses it to switch to a touch model (long
press, handles, on-screen keyboard) without asking the user. A host that cannot
tell devices leaves it at `Mouse`.

## Example

```rust
use bamboo_native_ui::{Event, Ui, UiHost};

enum Msg {
    Dismiss,
}

fn route(ui: &mut Ui<Msg>, event: &Event, sink: &mut impl UiHost) {
    match *event {
        Event::PointerMove { pos, .. } => {
            ui.pointer_move(pos, sink);
        }
        Event::PointerDown { pos, button, modifiers } => {
            ui.pointer_down_with(pos, button, modifiers, sink);
        }
        Event::PointerUp { pos, button, modifiers } => {
            ui.pointer_up_with(pos, button, modifiers, sink);
        }
        Event::Wheel { pos, delta_x, delta_y } => {
            ui.wheel_xy(pos, delta_x, delta_y, sink);
        }
        Event::Key(key) => {
            ui.key(key, sink);
        }
        Event::Char(ch) => {
            ui.char(ch, sink);
        }
        Event::Alt(show) => {
            ui.set_mnemonic_underlines(show);
        }
        _ => {}
    }
}
```

## Notes and limits

- Positions are client DIPs; the host divides the physical coordinate by
  `dpi / 96`.
- `PointerMove::buttons` is a bitmask of pressed buttons
  (`MOUSE_BUTTON_*` style bits), not a `MouseButton`.
- Modifiers are sampled twice: `PointerDown` reports press-time state and
  `PointerUp` its own. A multi-select list reads the release, but both ends are
  reported.
- `Key::repeat` flags auto-repeat; a handler that acts once per press checks it.
- `Char` excludes control characters: Enter, Tab and Backspace arrive as `Key`.
- `LongPress` needs a host with touch and a timer; a mouse-only host never sends
  it.

## See also

- [The widget trait](./widget-trait.md)
- [The Ui container](./ui-container.md)
- [Host and window](./host-and-window.md)
- [Gamepad](./gamepad.md)
- [Text edit](../widgets/inputs/text-edit.md)
- [First window quickstart](../quickstart/03-first-window.md)
