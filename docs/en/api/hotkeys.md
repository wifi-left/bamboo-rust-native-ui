# Hotkeys

> Parse and label keyboard chords, register them system-wide or per window, and capture the next chord for a settings UI.

## Overview

A `Hotkey` is a modifier set plus a virtual-key code in the same numeric space as
`KeyEvent::vk`, so no Win32 type appears in the public API. The host registers it
with `RegisterHotKey` (system-wide) or as a window shortcut (no system
registration). Plain printable keys require a modifier: `parse` rejects a bare
`A` because a global `A` shortcut would swallow ordinary typing everywhere.

The `Hotkey` and `HotkeyModifiers` models plus `parse`/`label` are
cross-platform. Registration, capture and the `Host`/`WindowHandle` callbacks are
Windows-only.

## Key types

### `HotkeyModifiers`

```rust
pub struct HotkeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
}
```

Constants: `NONE`, `CTRL`, `SHIFT`, `ALT`, `WIN`. Methods: `is_empty()`,
`with_ctrl()`, `with_shift()`, `with_alt()`, `with_win()`.

### `Hotkey`

```rust
pub struct Hotkey {
    pub modifiers: HotkeyModifiers,
    pub vk: u16,
}
```

| Method | Signature | Description |
|---|---|---|
| `new` | `new(modifiers, key: KeyCode) -> Option<Self>` | From a named key; `None` if it has no stable virtual key. |
| `from_vk` | `from_vk(modifiers, vk: u16) -> Self` | From a raw virtual-key code. |
| `parse` | `parse(spec: &str) -> Option<Self>` | Parse `"Ctrl+Shift+M"`, `"Alt+F10"`, ... |
| `label` | `label(&self) -> String` | `"Ctrl+Shift+M"` style text; round-trips through `parse`. |
| `key` | `key(&self) -> KeyCode` | The key as a named `KeyCode` (or `Other(vk)`). |
| `is_global_safe` | `is_global_safe(&self) -> bool` | Has a modifier or is a function key. |
| `from_key_event` | `from_key_event(&KeyEvent) -> Option<Self>` | Build from a keyboard event's state. |

`hotkey::is_modifier_vk(vk) -> bool` reports whether a code only contributes to
the chord.

`parse` is case-insensitive, accepts modifiers in any order (`ctrl`/`control`,
`shift`, `alt`, `win`/`windows`/`meta`/`super`), named keys (`enter`, `tab`,
`escape`/`esc`, `delete`/`del`, `pageup`/`pgup`, arrows, function keys `F1`–`F24`)
and a persisted `VKxx` code. `label` prints modifiers in the order Ctrl, Alt,
Shift, Win, then the key.

### Registration and capture (`WindowHandle`)

| API | Signature |
|---|---|
| `register_hotkey` | `register_hotkey(&mut self, hotkey: Hotkey) -> bool` |
| `unregister_hotkey` | `unregister_hotkey(&mut self, hotkey: Hotkey) -> bool` |
| `registered_hotkeys` | `registered_hotkeys(&self) -> Vec<Hotkey>` |
| `register_window_hotkey` | `register_window_hotkey(&mut self, hotkey, command: u32) -> bool` |
| `unregister_window_hotkey` | `unregister_window_hotkey(&mut self, hotkey) -> bool` |
| `clear_window_hotkeys` | `clear_window_hotkeys(&mut self)` |
| `window_hotkeys` | `window_hotkeys(&self) -> Vec<(Hotkey, u32)>` |
| `begin_hotkey_capture` | `begin_hotkey_capture(&mut self)` |
| `cancel_hotkey_capture` | `cancel_hotkey_capture(&mut self)` |
| `hotkey_capture_active` | `hotkey_capture_active(&self) -> bool` |

`Host::hotkey(hotkey, window)` receives a fired global hotkey;
`Host::hotkey_captured(hotkey: Option<Hotkey>, window)` receives a capture result.

## Example

Parsing and labelling:

```rust
use bamboo_native_ui::Hotkey;

let play = Hotkey::parse("ctrl+alt+p").expect("valid chord");
assert_eq!(play.label(), "Ctrl+Alt+P");

let help = Hotkey::parse("F1").expect("function keys need no modifier");
assert_eq!(help.label(), "F1");

// A bare printable key is rejected: it would capture ordinary typing.
assert!(Hotkey::parse("A").is_none());
```

Registering globally and capturing a new chord:

```rust
use bamboo_native_ui::{Hotkey, WindowHandle};

fn install(window: &mut WindowHandle<'_>) {
    if let Some(hotkey) = Hotkey::parse("Ctrl+Alt+P") {
        let _ = window.register_hotkey(hotkey);
    }
}

fn hotkey(&mut self, hotkey: Hotkey, window: &mut WindowHandle<'_>) {
    window.show();
    self.status = format!("{} pressed", hotkey.label());
}

fn hotkey_captured(&mut self, hotkey: Option<Hotkey>, window: &mut WindowHandle<'_>) {
    if let Some(hotkey) = hotkey {
        // The app decides whether the captured chord becomes global.
        let _ = window.register_hotkey(hotkey);
    }
}
```

## Notes and limits

- `register_hotkey` is system-wide on Windows and returns `false` when another
  process already owns the chord; registering the same hotkey again is a no-op
  that returns `true`. Registrations are released when the window is destroyed.
- `register_window_hotkey` needs no system registration, so it cannot conflict
  with another process. While the window has focus it fires
  `Host::menu_bar_command` with `command` (after the `MenuEvent` hook) and the
  page never sees the chord. It returns `false` for command `0`, a chord without
  a modifier that is not a function key, or a duplicate.
- The native menu bar's accelerator labels are registered as window shortcuts
  automatically; an explicit `register_window_hotkey` for the same chord
  overrides them.
- Capture consumes keys before the application sees them (including their
  translated character) until it finishes. Modifier-only presses keep waiting;
  `Some` means a chord with a modifier or function key, `None` means Esc, focus
  loss or `cancel_hotkey_capture`. The result is **not** registered
  automatically.
- Persist `hotkey.label()` strings; restore with `register_hotkey` after checking
  the `bool` result (the system may already own the chord).
- All registration and capture are Windows-only; the parse/label models are
  cross-platform.

## See also

- `menus.md` (accelerator labels on menu items)
- `accessibility.md` (announcing status)
- `gamepad.md` (pad buttons mapped to keys)
- `../quickstart/03-first-window.md`
