# 10 — Dialogs, tray and hotkeys

> Native message and file dialogs, a tray icon, toasts and global shortcuts.

## What you will build

The greeting window from [03](03-first-window.md) gains an integration strip.
**Remove history** asks a yes/no question, **Choose file** opens the shell
picker, a tray icon restores the window and offers a menu, choosing a file posts
a notification, and `Ctrl+Shift+M` counts clicks from anywhere in Windows.
**Change shortcut** captures the next chord and registers it.

Results are asynchronous: `show_dialog` returns a `DialogId` and the answer
arrives in `Host::dialog_finished`. Tray and hotkey callbacks run on the UI
thread between other callbacks.

## Prerequisites

- The project from [03](03-first-window.md) builds and runs.
- No extra feature; these are part of the default Windows host.

## New terms on this page

- **Dialog** — a small window the app opens to ask a question or let the user
  choose something. A **native dialog** is drawn by Windows itself (the standard
  yes/no box or file picker), so it matches the rest of the system.
- **Modal** — while a modal dialog is open, its owner window does not accept
  clicks until the dialog is answered or closed.
- **Asynchronous result** — `show_dialog` returns immediately with a `DialogId`
  (a handle to the open dialog, not the answer). The answer arrives later in the
  `dialog_finished` callback, so the app keeps running while the dialog is open.
- **File filter** — the pattern list in a file picker (`*.mp3;*.flac`) that
  decides which files are offered.
- **Tray icon** — the small icon in the Windows notification area, next to the
  clock. It keeps the app reachable while its window is hidden.
- **Balloon** — the older notification bubble that points at a tray icon;
  Windows falls back to it when a modern toast cannot be shown.
- **Notification / toast** — the short pop-up Windows shows in the corner of the
  screen.
- **Hotkey** — a key combination the app reacts to. A **global hotkey** works
  while any application has focus; a **window hotkey** works only while this
  window is active. A **chord** is the combination itself (`Ctrl+Shift+M`), and
  **capture** means asking the user to press the next chord.

All Rust on this page goes into the crate's `src/main.rs`
(`bamboo-hello/bamboo-hello/src/main.rs`), the same file as page 03.

## Steps

### 1. Import the integration types

```rust
// Dialogs, tray and hotkeys: the app passes descriptions; the host builds
// the Win32 objects and calls back with results.
use bamboo_native_ui::{
    DialogId, DialogResult, FileDialog, FileFilter, Host, Hotkey, HotkeyModifiers,
    KeyCode, Menu, MessageAnswer, MessageButtons, MessageDefault, MessageDialog, MessageIcon,
    Notification, NotificationResult, TrayAction, TrayEvent, TrayIcon, TrayImage, Ui, WidgetId,
    WindowHandle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    ChooseFile, // "Choose file" button: open the shell picker
    Remove,     // "Remove history" button: ask a yes/no question
    Capture,    // "Change shortcut" button: capture the next chord
}
```

The application passes descriptions; the host turns them into Win32 objects.

### 2. Extend the application state

Keep `paint`, `resize` and `event` from page 03. Route `Msg::ChooseFile`,
`Remove` and `Capture` from `apply` to `open_file`, `open_remove` and
`begin_hotkey_capture`. Put `Host` methods in `impl Host for App` and helpers
such as `notify` in `impl App`.

```rust
struct App {
    ui: Ui<Msg>,
    clicks: u32,
    confirm: Option<DialogId>, // the open yes/no question, if any
    hotkey: Option<Hotkey>,    // the chord currently registered
}

impl App {
    fn status(&mut self, text: String, window: &mut WindowHandle<'_>) {
        self.ui.set_label(WidgetId::new("status"), text);
        // Dialog/tray callbacks run outside `event`, so flush here.
        self.ui.flush_damage(window);
    }
}
```

### 3. Ask a question with `MessageDialog`

`MessageDialog::new(text)` is a modal `OK` box; `.icon`, `.buttons` and
`.default_button` pick the glyph, buttons and focus. `.parallel()` frees the owner.

```rust
fn open_remove(&mut self, window: &mut WindowHandle<'_>) {
    // A description, not a Win32 handle: title, glyph, buttons, initial focus.
    let dialog = MessageDialog::new("Clear the click history?")
        .title("Bamboo Hello").icon(MessageIcon::Warning)
        .buttons(MessageButtons::YesNo).default_button(MessageDefault::Second);
    // Returns at once; the answer arrives later in `dialog_finished`.
    self.confirm = Some(window.show_dialog(dialog));
}
```

The answer is `Option<MessageAnswer>`; `None` means dismissal. A
`.button(label, command)` makes it a task dialog reporting `Custom(command)`.

**What just happened** — clicking **Remove history** ran `open_remove`, which
described the question and handed it to the host. The call returned a
`DialogId` at once, so the click handler finished while the box is still on
screen. When the user answers, the host calls `Host::dialog_finished` with that
same id; the app compares it with `self.confirm` to know which dialog the answer
belongs to.

### 4. Pick a file or folder

```rust
fn open_file(&mut self, window: &mut WindowHandle<'_>) {
    window.show_dialog(
        FileDialog::open() // the "Open" picker; `::save`/`::folder` are the others
            .title("Open audio")
            // Only these extensions are offered under the "Audio" filter.
            .filter("Audio", "*.mp3;*.flac;*.wav")
            .filter("All files", "*.*")
            .initial_dir("C:\\Music") // folder the picker starts in
            .initial_name("track.mp3"),
    );
}
```

`FileDialog::open`/`::save`/`::folder` build the three pickers. `.filter` takes
a `FileFilter::new(label, pattern)` pair; `.multi_select(true)` makes an open
picker report `DialogResult::Files`.

### 5. Install a tray icon and register hotkeys

Install the icon once the window exists, in `started`. The menu reuses the
widget `Menu` model and its nonzero application commands.

```rust
fn started(&mut self, window: &mut WindowHandle<'_>) {
    // Tray menu: commands 1 and 2 come back in `tray_event`.
    let menu = Menu::new().item(1, "Show window").separator().item(2, "Quit");
    window.set_tray_icon(Some(TrayIcon::new("Bamboo Hello").menu(menu)));

    let chord = Hotkey::parse("Ctrl+Shift+M").expect("a valid chord");
    self.hotkey = Some(chord);
    let _ = window.register_hotkey(chord); // global: works with another app focused
    // A window shortcut needs no system registration; it reports command 301.
    let _ = window.register_window_hotkey(Hotkey::parse("Ctrl+K").unwrap(), 301);
}

fn tray_event(&mut self, event: TrayEvent, window: &mut WindowHandle<'_>) {
    match event {
        // Left click or keyboard select: bring the window back.
        TrayEvent::Action { action: TrayAction::Click | TrayAction::KeySelect, .. } => {
            window.show()
        }
        TrayEvent::Action { .. } => {} // DoubleClick, RightClick, MiddleClick
        TrayEvent::Command { command: 1, .. } => window.show(), // "Show window"
        TrayEvent::Command { command: 2, .. } => window.quit(0), // "Quit"
        TrayEvent::Command { .. } => {}
    }
}
```

`TrayIcon::new` leaves `TrayImage::Default`; `.icon_file(path)` sets
`TrayImage::File`. `set_tray_icon_at` adds a second icon; return `false` from
`Host::close_requested` to stay in the tray.

### 6. Resolve dialogs and post notifications

```rust
fn dialog_finished(&mut self, id: DialogId, result: DialogResult, window: &mut WindowHandle<'_>) {
    match (Some(id) == self.confirm, result) {
        (true, DialogResult::Message(Some(MessageAnswer::Yes))) => {
            // Yes on our confirm box: clear the counter.
            self.clicks = 0;
            self.confirm = None;
            self.status("History cleared".to_string(), window);
        }
        (true, _) => {
            // No, or the box was dismissed: leave the counter alone.
            self.confirm = None;
            self.status("History kept".to_string(), window);
        }
        (false, DialogResult::File(Some(path))) => {
            // A file picker (not the confirm box) returned one path.
            self.status(format!("Chosen: {path}"), window);
            self.notify(&path, window);
        }
        (false, _) => {}
    }
}

fn notify(&mut self, path: &str, window: &mut WindowHandle<'_>) {
    // Try a toast first; Windows may refuse and use a tray balloon instead.
    let outcome = window.notify(&Notification::new("Track queued", path));
    let text = match outcome {
        NotificationResult::Toast => "Toast shown",      // modern corner pop-up
        NotificationResult::Balloon => "Tray balloon shown", // older bubble
        NotificationResult::Failed => "No notifier",
    };
    self.status(text.to_string(), window);
}
```

An unpackaged process needs an AppUserModelID (`WindowBuilder::app_id` plus
`window.register_notification_shortcut`). When the toast is refused, `notify`
falls back to a balloon on the tray icon (step 5 installs one);
`NotificationResult::shown()` tests "either path worked".

### 7. Register, react to and capture shortcuts

`Host::hotkey` fires while the window exists, even when another application has
focus; `register_hotkey` returns `false` when the system owns the chord.

```rust
fn hotkey(&mut self, hotkey: Hotkey, window: &mut WindowHandle<'_>) {
    if Some(hotkey) == self.hotkey {
        // Fired globally, even while another app has focus.
        self.clicks += 1;
        self.status(format!("{} · {} click(s)", hotkey.label(), self.clicks), window);
    }
}

fn menu_bar_command(&mut self, command: u32, window: &mut WindowHandle<'_>) {
    if command == 301 {
        // The window shortcut: move focus to the first button.
        self.ui.set_focus(WidgetId::new("greet"), window);
    }
}
```

`Hotkey::new(HotkeyModifiers::CTRL.with_shift(), KeyCode::Letter(12))` returns
`Option<Hotkey>`; `Letter(12)` is `M`. `parse` rejects a bare printable key
because a global `A` would swallow ordinary typing everywhere.

To replace the chord, `begin_hotkey_capture` consumes the next non-modifier key
and reports it through `Host::hotkey_captured` (Esc, focus loss or
`cancel_hotkey_capture` reports `None`); nothing is registered automatically.

```rust
fn hotkey_captured(&mut self, hotkey: Option<Hotkey>, window: &mut WindowHandle<'_>) {
    let Some(chord) = hotkey else {
        // Esc, focus loss or `cancel_hotkey_capture` reports `None`.
        self.status("Capture canceled".to_string(), window);
        return;
    };
    let shown = chord.label();
    let ok = window.register_hotkey(chord); // false when another app owns it
    if ok { self.hotkey = Some(chord); }
    let text = if ok { format!("Registered {shown}") } else { format!("{shown} is taken") };
    self.status(text, window);
}
```

Call `window.begin_hotkey_capture()` from the `Msg::Capture` handler. Keep the
old chord until the new one is accepted, or call `unregister_hotkey` first to
free it; registering the same chord twice is a no-op that returns `true`.

### 8. Run it

Run from the crate folder — the inner `bamboo-hello/bamboo-hello`, the one that
holds `src/main.rs`. The outer `bamboo-hello` folder also works because it lists
the crate as its only workspace member.

```powershell
# Working directory: bamboo-hello/bamboo-hello (the crate with src/main.rs)
cargo run
```

A window titled "Bamboo Hello" appears with the integration strip. The tray icon
appears in the notification area; if Windows hid it, click the `^` chevron next
to the clock. `Ctrl+Shift+M` increments the click counter in the status label
even while another window is focused. A successful start prints Cargo's
`Finished` and `Running` lines; a compile error prints the failing line and a
non-zero exit code.

**What just happened** — Cargo compiled the crate, `WindowBuilder::run` entered
the Windows message loop, and `started` installed the tray icon and the two
shortcuts. From then on the loop delivered clicks, hotkeys and dialog results to
your `Host` methods.

## Expected result

Running `cargo run` and pressing `Ctrl+Shift+M` with another window focused
counts a click. **Remove history** → Yes clears it. **Choose file** shows the
path and posts a toast (or a tray balloon). A tray left click restores a window
hidden by `close_requested`.

- A modal warning with the second button focused; Yes clears, anything else
  keeps the count.
- The shell picker opens with the audio filter; the chosen path is displayed.
- A tray icon with Show/Quit; `Ctrl+Shift+M` works globally and `Ctrl+K`
  focuses the first button while the window is active.
- A toast or balloon after a file is chosen; **Change shortcut** installs the
  new global chord.

If one of these never appears, the matching item under Troubleshooting names the
usual cause.

## Troubleshooting

- **The dialog never appears** — `show_dialog` was called before the message
  loop started, or the window is hidden. Open dialogs from a button handler or
  another callback that runs after `started`.
- **Choose file opens more than one picker** — each click calls `show_dialog`
  again because results are asynchronous. Store the returned `DialogId` or
  ignore clicks while a picker is already open.
- **`dialog_finished` never runs** — show dialogs from a callback, not before the loop starts.
- **A multi-file pick is ignored** — with `.multi_select(true)` the result is
  `DialogResult::Files`, not `DialogResult::File(Some(..))`; handle both arms.
- **`register_hotkey` returns `false`** — another process owns the chord; `registered_hotkeys()` lists yours.
- **The hotkey does not fire** — `Host::hotkey` is not implemented, the chord
  was never registered, or another app owns it. A window hotkey also needs this
  window to be active, unlike a global one.
- **The hotkey fires but nothing changes** — the comparison
  `Some(hotkey) == self.hotkey` failed; `hotkey` may still hold the old chord.
  Register the new chord before storing it.
- **`Hotkey::parse` returns `None`** — no modifier and not a function key, two
  keys (`Ctrl+A+B`), or an unknown key name.
- **Nothing happens after clicking Change shortcut** — `begin_hotkey_capture`
  was not called from the `Msg::Capture` handler; Esc cancels the capture.
- **No toast or balloon** — set `WindowBuilder::app_id`, register the shortcut, and install a tray icon.
- **The toast is refused but no balloon shows** — the balloon needs a tray icon;
  `notify` only falls back when `set_tray_icon` ran in `started`.
- **No tray icon** — `set_tray_icon` ran before the window existed. Call it from
  `started`. Windows may also hide a new icon in the overflow area: open the `^`
  chevron to find it.
- **The tray icon is there but its menu does nothing** — `Host::tray_event` is
  not implemented, or the command numbers differ from the `Menu` items.
- **Closing the window quits instead of hiding** — `Host::close_requested` must
  return `false`, and a tray icon must exist so the user can restore it.

## Next

[11 — Theming and images](11-theming-and-images.md): switch light and dark,
load a `.buitheme` palette and feed gradients to avatars and cover walls.
