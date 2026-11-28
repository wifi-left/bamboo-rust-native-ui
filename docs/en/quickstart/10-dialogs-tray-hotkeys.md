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

## Steps

### 1. Import the integration types

```rust
use bamboo_native_ui::{
    DialogId, DialogResult, FileDialog, FileFilter, Host, Hotkey, HotkeyModifiers,
    KeyCode, Menu, MessageAnswer, MessageButtons, MessageDefault, MessageDialog, MessageIcon,
    Notification, NotificationResult, TrayAction, TrayEvent, TrayIcon, TrayImage, Ui, WidgetId,
    WindowHandle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    ChooseFile,
    Remove,
    Capture,
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
    confirm: Option<DialogId>,
    hotkey: Option<Hotkey>,
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
    let dialog = MessageDialog::new("Clear the click history?")
        .title("Bamboo Hello").icon(MessageIcon::Warning)
        .buttons(MessageButtons::YesNo).default_button(MessageDefault::Second);
    self.confirm = Some(window.show_dialog(dialog));
}
```

The answer is `Option<MessageAnswer>`; `None` means dismissal. A
`.button(label, command)` makes it a task dialog reporting `Custom(command)`.

### 4. Pick a file or folder

```rust
fn open_file(&mut self, window: &mut WindowHandle<'_>) {
    window.show_dialog(
        FileDialog::open()
            .title("Open audio")
            .filter("Audio", "*.mp3;*.flac;*.wav")
            .filter("All files", "*.*")
            .initial_dir("C:\\Music")
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
    let menu = Menu::new().item(1, "Show window").separator().item(2, "Quit");
    window.set_tray_icon(Some(TrayIcon::new("Bamboo Hello").menu(menu)));

    let chord = Hotkey::parse("Ctrl+Shift+M").expect("a valid chord");
    self.hotkey = Some(chord);
    let _ = window.register_hotkey(chord);
    // A window shortcut needs no system registration; it reports command 301.
    let _ = window.register_window_hotkey(Hotkey::parse("Ctrl+K").unwrap(), 301);
}

fn tray_event(&mut self, event: TrayEvent, window: &mut WindowHandle<'_>) {
    match event {
        TrayEvent::Action { action: TrayAction::Click | TrayAction::KeySelect, .. } => {
            window.show()
        }
        TrayEvent::Action { .. } => {} // DoubleClick, RightClick, MiddleClick
        TrayEvent::Command { command: 1, .. } => window.show(),
        TrayEvent::Command { command: 2, .. } => window.quit(0),
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
            self.clicks = 0;
            self.confirm = None;
            self.status("History cleared".to_string(), window);
        }
        (true, _) => {
            self.confirm = None;
            self.status("History kept".to_string(), window);
        }
        (false, DialogResult::File(Some(path))) => {
            self.status(format!("Chosen: {path}"), window);
            self.notify(&path, window);
        }
        (false, _) => {}
    }
}

fn notify(&mut self, path: &str, window: &mut WindowHandle<'_>) {
    let outcome = window.notify(&Notification::new("Track queued", path));
    let text = match outcome {
        NotificationResult::Toast => "Toast shown",
        NotificationResult::Balloon => "Tray balloon shown",
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
        self.clicks += 1;
        self.status(format!("{} · {} click(s)", hotkey.label(), self.clicks), window);
    }
}

fn menu_bar_command(&mut self, command: u32, window: &mut WindowHandle<'_>) {
    if command == 301 {
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
        self.status("Capture canceled".to_string(), window);
        return;
    };
    let shown = chord.label();
    let ok = window.register_hotkey(chord);
    if ok { self.hotkey = Some(chord); }
    let text = if ok { format!("Registered {shown}") } else { format!("{shown} is taken") };
    self.status(text, window);
}
```

Call `window.begin_hotkey_capture()` from the `Msg::Capture` handler. Keep the
old chord until the new one is accepted, or call `unregister_hotkey` first to
free it; registering the same chord twice is a no-op that returns `true`.

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

## Troubleshooting

- **`dialog_finished` never runs** — show dialogs from a callback, not before the loop starts.
- **`register_hotkey` returns `false`** — another process owns the chord; `registered_hotkeys()` lists yours.
- **`Hotkey::parse` returns `None`** — no modifier and not a function key, two
  keys (`Ctrl+A+B`), or an unknown key name.
- **No toast or balloon** — set `WindowBuilder::app_id`, register the shortcut, and install a tray icon.
- **No tray icon** — `set_tray_icon` ran before the window existed. Call it from
  `started`.

## Next

[11 — Theming and images](11-theming-and-images.md): switch light and dark,
load a `.buitheme` palette and feed gradients to avatars and cover walls.
