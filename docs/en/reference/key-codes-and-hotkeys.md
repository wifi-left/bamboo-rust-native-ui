# Key codes and hotkeys

> The `KeyCode`/`KeyEvent` vocabulary, virtual-key mapping, `Hotkey` syntax and
> labels, and the gamepad-to-key translation.

## `KeyCode`

```rust
pub enum KeyCode {
    Backspace, Tab, Enter, Escape, Space,
    Left, Right, Up, Down, Home, End, PageUp, PageDown, Insert, Delete,
    Letter(u8),    // A..Z as 0..=25
    Digit(u8),     // 0..9 as 0..=9
    Function(u8),  // F1..F24 as 1..=24
    Other(u16),    // any other virtual-key code
}
```

Methods:

| Method | Returns |
|---|---|
| `is_letter(u8)` | Whether this is `Letter` equal to the given 0-based index. |
| `from_vk(u16)` | A named variant when the virtual-key code is known, else `Other(vk)`. |
| `to_vk()` | `Option<u16>`; `None` for `Other(0)` and a `Function` outside `1..=24`. |

`from_vk`/`to_vk` use numeric Win32 virtual-key values but expose no Win32 type.

| Key | VK | Key | VK |
|---|---|---|---|
| Backspace | `0x08` | Left | `0x25` |
| Tab | `0x09` | Up | `0x26` |
| Enter | `0x0D` | Right | `0x27` |
| Escape | `0x1B` | Down | `0x28` |
| Space | `0x20` | Home | `0x24` |
| PageUp | `0x21` | PageDown | `0x22` |
| Insert | `0x2D` | Delete | `0x2E` |
| End | `0x23` | `A`..`Z` | `0x41`..`0x5A` |
| `0`..`9` | `0x30`..`0x39` | `F1`..`F24` | `0x70`..`0x87` |

## `KeyEvent`

```rust
pub struct KeyEvent {
    pub code: KeyCode,
    pub vk: u16,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub repeat: bool,   // auto-repeat: the key was already down
}
```

`Event::Key(KeyEvent)` reaches `Ui::key`; `Event::KeyUp` reaches `Ui::key_up`
(observation). `Event::Char(char)` is separate and never carries control
characters: Enter, Tab and Backspace arrive as `Key`. A handler that should act
once per press checks `repeat`.

Pointer modifiers are a separate two-field struct:

```rust
pub struct PointerModifiers { pub ctrl: bool, pub shift: bool }
```

Constants `NONE`, `ctrl(bool)`, `shift(bool)` and `any()` are provided;
`PointerDown` and `PointerUp` each report the state at their own moment.

## `Hotkey`

```rust
pub struct HotkeyModifiers {
    pub ctrl: bool, pub shift: bool, pub alt: bool, pub win: bool,
}

pub struct Hotkey {
    pub modifiers: HotkeyModifiers,
    pub vk: u16,   // same numeric space as KeyEvent::vk
}
```

| API | Signature | Notes |
|---|---|---|
| `Hotkey::new` | `new(modifiers, key: KeyCode) -> Option<Self>` | `None` when the key has no stable VK. |
| `Hotkey::from_vk` | `from_vk(modifiers, vk: u16) -> Self` | From raw input. |
| `Hotkey::parse` | `parse(spec: &str) -> Option<Self>` | See the grammar below. |
| `Hotkey::label` | `label(&self) -> String` | Persistable text; round-trips through `parse`. |
| `Hotkey::key` | `key(&self) -> KeyCode` | Named key, or `Other(vk)`. |
| `Hotkey::is_global_safe` | `is_global_safe(&self) -> bool` | Has a modifier or is a function key. |
| `Hotkey::from_key_event` | `from_key_event(&KeyEvent) -> Option<Self>` | Uses the event's ctrl/shift/alt; never sets Win. |

`HotkeyModifiers` constants: `NONE`, `CTRL`, `SHIFT`, `ALT`, `WIN`; builders
`with_ctrl`, `with_shift`, `with_alt`, `with_win`; `is_empty`.
`hotkey::is_modifier_vk(vk) -> bool` reports a code that only contributes to a
chord (Shift/Ctrl/Alt, Win, or the left/right modifier variants).

### Parse grammar

`spec` is split on `+`; every token is trimmed and compared case-insensitively.

| Token | Meaning |
|---|---|
| `ctrl`, `control` | Ctrl |
| `shift` | Shift |
| `alt` | Alt |
| `win`, `windows`, `meta`, `super` | Win |
| a single ASCII letter | `Letter(0..=25)` |
| a single ASCII digit | `Digit(0..=9)` |
| `enter`, `return` | Enter |
| `space`, `tab` | Space, Tab |
| `escape`, `esc` | Escape |
| `backspace`, `bksp` | Backspace |
| `delete`, `del`, `insert`, `ins` | Delete, Insert |
| `home`, `end` | Home, End |
| `pageup`, `pgup`, `pagedown`, `pgdn` | PageUp, PageDown |
| `up`, `down`, `left`, `right` (+ `arrow*`) | Arrows |
| `f1`..`f24` | Function key |
| `vk` + hex | Any virtual-key code (`VK2C` → `Other(0x2C)`) |

Rules:

- Modifiers may appear in any order and more than once (`ctrl+control` is
  still Ctrl). Exactly one non-modifier key is allowed.
- A shortcut must have at least one modifier **unless** the key is a function
  key. `M` and `Enter` are rejected; `F9` and `Shift+A` are accepted.
- Empty tokens, duplicate keys, unknown names and out-of-range function keys
  return `None`.

`label()` prints Ctrl, Alt, Shift, Win in that order, then the key. Key labels
include `Esc`, `PageUp`, `PageDown`, `Backspace`, `Delete`, `Insert`, `Home`,
`End`, `Up`, `Down`, `Left`, `Right`, `Space`, `Tab`, `Enter`, `F1`..`F24`, and
`VKxx` (uppercase hex) for `Other`.

```rust
use bamboo_native_ui::Hotkey;

let chord = Hotkey::parse("ctrl+shift+m").expect("valid");
assert_eq!(chord.label(), "Ctrl+Shift+M");   // parse("Ctrl+Shift+M") == Some(chord)
assert!(Hotkey::parse("M").is_none());       // bare printable key
assert!(Hotkey::parse("F9").is_some());      // function key needs no modifier
```

## Registration

| API | Where it fires | Notes |
|---|---|---|
| `WindowHandle::register_hotkey(hotkey)` | `Host::hotkey` | System-wide (`RegisterHotKey`); `false` when another process owns it. |
| `WindowHandle::unregister_hotkey(hotkey)` | — | `false` when it was not registered. |
| `WindowHandle::register_window_hotkey(hotkey, command)` | `Host::menu_bar_command` | Window-scoped; no system registration, so no cross-process conflict. |
| `WindowHandle::clear_window_hotkeys()` | — | Removes every window shortcut. |
| `WindowHandle::begin_hotkey_capture()` | `Host::hotkey_captured` | Captures the next chord; `None` means Esc/focus loss/cancel. |

`register_window_hotkey` rejects command `0`, a chord without a modifier that
is not a function key, and duplicates. The menu bar's accelerator labels are
registered automatically; an explicit window hotkey for the same chord
overrides them. A captured chord is **not** registered automatically; the
application decides.

## Common chords

These are `KeyEvent` patterns an application sees (not registered global
hotkeys). Text fields implement their own editing commands; other widgets
receive the raw key.

| Chord | `code` + modifiers | Typical use |
|---|---|---|
| Ctrl+C / Ctrl+V / Ctrl+X | `Letter(2/21/23)` + `ctrl` | Clipboard copy/paste/cut. |
| Ctrl+A | `Letter(0)` + `ctrl` | Select all. |
| Ctrl+Z / Ctrl+Y | `Letter(25/24)` + `ctrl` | Undo / redo. |
| Ctrl+S / Ctrl+O / Ctrl+N | `Letter(18/14/13)` + `ctrl` | Save / open / new. |
| Ctrl+F | `Letter(5)` + `ctrl` | Find. |
| Shift+Tab | `Tab` + `shift` | Focus backwards. |
| Shift+F10 | `Function(10)` + `shift` | Context menu (also what gamepad Y sends). |
| F1 | `Function(1)` | Help. |
| Alt+F4 | `Function(4)` + `alt` | Shell close request (`Host::close_requested`). |
| Delete / Backspace | `Delete` / `Backspace` | Remove. |
| Enter / Space | `Enter` / `Space` | `WidgetInput::Activate` on a focused control. |

Use `Hotkey::parse` for a **global** shortcut instead: `"Ctrl+Alt+P"`,
`"Alt+F10"`, `"F9"`, `"Shift+Esc"` are typical; `"Ctrl+S"` as a global hotkey
would steal Save from every application.

## Access keys (mnemonics)

Chrome labels (menu bar, menu button, toolbar, split button) parse `&`:

- `"&File"` paints `File` and answers `Alt+F`.
- `"&&"` is a literal ampersand.
- A lone trailing `&` and `&` followed by whitespace stay literal.
- Only the first valid marker is the access key; later markers lose the `&`
  but keep the character.

The container routes `Alt+key`; widgets expose `Widget::mnemonic` and
`Widget::mnemonic_key`. Ordinary buttons do not use access keys: reach them
with Tab and Space/Enter.

## Gamepad mapping

`Gamepad` polls one XInput slot and returns `Vec<KeyEvent>`; forward them to
`Ui::pad_key` (or `Ui::key`). The mapping is pure and testable without hardware;
`read_state(slot)` is the only platform call.

| Pad input | Key event |
|---|---|
| D-pad up/down/left/right | `Up` / `Down` / `Left` / `Right` |
| Left stick (dominant axis, `abs(axis) >= 0.55`) | Arrow key |
| A, Start | `Enter` |
| B, Back | `Escape` |
| X | `Space` |
| Y | `Function(10)` + `shift` (context menu) |
| RB | `Tab` |
| LB | `Tab` + `shift` |

Face/shoulder buttons are edge-triggered. A held direction fires immediately,
then repeats after 400 ms and every 90 ms. The D-pad wins over the stick. A
disabled pad or a background window emits nothing and drops the held state, so
re-enabling never resumes a stale repeat. **A** on a focused text field also
raises the Windows on-screen keyboard through `Ui::pad_key`.

## See also

- [Events and input](../api/events-and-input.md)
- [Hotkeys API](../api/hotkeys.md)
- [Gamepad](../api/gamepad.md)
- [The Ui container](../api/ui-container.md)
- [Accessibility guide](../guides/accessibility.md)
- [Host and window](../api/host-and-window.md)
