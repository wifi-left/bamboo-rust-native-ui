# Dialogs

> Native message boxes, shell file/font/color pickers and the library's themed color window, all delivered asynchronously.

## Overview

Dialogs come in two presentations. **Modal** (the default) disables the owner
window while the dialog is up; **parallel** (`.parallel()`) leaves the
application enabled and usable. Both return their result asynchronously: a worker
thread hosts the native dialog, and the answer is delivered to
`Host::dialog_finished` on the UI thread after the dialog closes. The UI thread
never enters the dialog's nested message loop, so paints and timers keep working.

`Dialog` is the request enum; `MessageDialog`, `FileDialog`, `FontDialog`,
`ColorDialog` and `ColorWindow` each convert into it. The native bodies are
Windows-only; the request/result types compile on every target.

## Key types

### `MessageDialog`

```rust
pub struct MessageDialog { /* private */ }

impl MessageDialog {
    pub fn new(text: impl Into<String>) -> Self; // informational OK, modal
}
```

Builders: `title`, `icon(MessageIcon)`, `buttons(MessageButtons)`,
`default_button(MessageDefault)`, `button(label, command: u32)` (a custom task-
dialog button), `instruction(text)`, `icon_file(path)`, `parallel()`.
Getters: `text()`, `dialog_title()`, `custom_buttons()`.

| Enum | Variants |
|---|---|
| `MessageIcon` | `Info` (default), `Warning`, `Error`, `Question` |
| `MessageButtons` | `Ok` (default), `OkCancel`, `YesNo`, `YesNoCancel`, `RetryCancel`, `AbortRetryIgnore` |
| `MessageAnswer` | `Ok`, `Cancel`, `Yes`, `No`, `Retry`, `Abort`, `Ignore`, `Custom(u32)` |
| `MessageDefault` | `First` (default), `Second`, `Third` |

A dialog with at least one custom `button` is shown as a task dialog: the
selected id arrives as `MessageAnswer::Custom(u32)`, and Escape / the close box
report `MessageAnswer::Cancel`. `MessageButton { label, command }` is built with
`MessageButton::new(label, command)`.

### `FileDialog`

```rust
impl FileDialog {
    pub fn open() -> Self;
    pub fn save() -> Self;
    pub fn folder() -> Self;
}
```

Builders: `title`, `filter(label, pattern)`, `initial_dir`, `initial_name`,
`multi_select(bool)`, `parallel()`. Getters: `mode() -> FileDialogMode`,
`is_multi_select()`, `returns_multiple()`.

`FileDialogMode` is `Open | Save | Folder`. `FileFilter::new(label, pattern)`
takes a visible name and a semicolon list such as `"*.mp3;*.flac"`. A
multi-select **open** dialog returns `DialogResult::Files`; save and folder modes
ignore the flag.

### `FontDialog` / `ColorDialog` / `ColorWindow`

| Type | Purpose |
|---|---|
| `FontDialog::new()` | System font chooser; `.initial(FontChoice)`, `.parallel()`. |
| `FontChoice::new(family, size)` | Result face/size; `.bold(bool)`, `.italic(bool)`. |
| `ColorDialog::new()` | System color chooser; `.initial(Color)`, `.custom_colors([Color; 16])`, `.parallel()`. |
| `ColorWindow::new(Rgba)` | Themed, owned color window; `.initial(Rgba)`, `.allow_alpha(bool)`, `.parallel()`. |

`custom_colors() -> [Color; 16]` reads the process-wide custom wells;
`set_custom_colors([Color; 16])` replaces them (the shell writes a mixed color
back, so the next picker offers it again).

### `DialogId` and `DialogResult`

```rust
pub struct DialogId(pub u32);

pub enum DialogResult {
    Message(Option<MessageAnswer>),
    File(Option<String>),
    Files(Option<Vec<String>>),
    Font(Option<FontChoice>),
    Color(Option<Color>),
    ColorWindow(Option<Rgba>),
}
```

`None` means dismissed (Cancel, the close box or a failure to show).
`DialogResult::is_some()` reports whether a value was produced.

### `WindowHandle` and `Host`

| API | Signature |
|---|---|
| `show_dialog` | `show_dialog(&mut self, dialog: impl Into<Dialog>) -> DialogId` |
| `show_color_window` | `show_color_window(&mut self, initial: Rgba, allow_alpha: bool) -> DialogId` |
| `Host::dialog_finished` | `fn dialog_finished(&mut self, id: DialogId, result: DialogResult, window: &mut WindowHandle<'_>)` |

Any number of dialogs may be open at once; `DialogId` correlates each result.

## Example

```rust
use bamboo_native_ui::{MessageButtons, MessageDialog, MessageIcon};

// Inside a Host method that has `&mut WindowHandle` (for example `Ui` message handling):
fn confirm_quit(window: &mut bamboo_native_ui::WindowHandle<'_>) -> bamboo_native_ui::DialogId {
    window.show_dialog(
        MessageDialog::new("Discard the current queue?")
            .title("Quit")
            .icon(MessageIcon::Warning)
            .buttons(MessageButtons::YesNo)
            .default_button(bamboo_native_ui::MessageDefault::Second),
    )
}
```

The result arrives later:

```rust
use bamboo_native_ui::{
    DialogId, DialogResult, MessageAnswer, WindowHandle,
};

fn dialog_finished(
    &mut self,
    id: DialogId,
    result: DialogResult,
    window: &mut WindowHandle<'_>,
) {
    if id != self.quit_dialog {
        return;
    }
    if matches!(result, DialogResult::Message(Some(MessageAnswer::Yes))) {
        window.quit(0);
    }
}
```

## Notes and limits

- The native dialog bodies (`MessageBox`, the shell file picker, the
  font/color choosers, the task dialog) are Windows-only. The builders and
  results compile everywhere.
- A modal dialog disables its owner across thread boundaries; the framework's UI
  thread always pumps, so both loops stay responsive. A parallel dialog passes no
  owner and brings itself to the foreground.
- `dialog_finished` runs on the UI thread after the dialog thread is gone, so the
  UI is safe to update and a modal owner is already re-enabled.
- Task dialogs (custom buttons, `instruction`, `icon_file`) need comctl32 v6 (an
  application manifest). Without it the request falls back to a standard message
  box and custom labels cannot be represented.
- File pickers start COM as a single-threaded apartment on the worker thread;
  cancellation from `Show` is reported as "no selection" (`None`).
- A font picker reports point size; the dialog uses 96 DPI as its own reference.

## See also

- `../widgets/pickers/color-picker.md`
- `../widgets/controls/color-swatch.md`
- `tray-and-notifications.md`
- `../quickstart/03-first-window.md`
