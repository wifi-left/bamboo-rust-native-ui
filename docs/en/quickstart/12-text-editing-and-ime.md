# 12 — Text editing and IME

> Native and framework-painted text fields, number and search inputs, and
> Chinese/Japanese input.

## What you will build

A form with five fields. **Name** is a native `EDIT` child; **Notes** is a
framework-painted multiline editor with its own colours; **PIN** is masked;
**Search** is a `SearchBox`; **Count** is a `NumberInput` with steppers. A
status label reports commit messages, observed characters and the caret read
back through `Ui::with_widget`. The last step forwards IME composition into the
focused painted editor.

## Prerequisites

- The project from [03](03-first-window.md) builds and runs.
- No extra feature. The native editor and IME callbacks are Windows-only; the
  painted editor works on every target that can paint.

## New terms on this page

- **Native editor** — the field hands typing to a real Windows `EDIT` control,
  so Windows draws the text, selection and caret and owns the keyboard handling.
- **Painted editor** — the framework draws the text into its own window
  (`.painted()`), which lets the app set every colour and keeps the value inside
  the framework's layout.
- **Commit** — handing the finished text back to the app as a message. Enter on
  a single line, Tab, or focus loss commits.
- **Caret** — the blinking insertion point, measured as a character offset.
- **Selection** — the highlighted range of characters; it can be empty.
- **Cue** — grey hint text shown while a field is empty.
- **IME** — Input Method Editor, the Windows service used to type Chinese,
  Japanese and Korean. **Composition** is the not-yet-converted text it shows
  inline before the user confirms a candidate.
- **Stepper** — the small up/down arrows that adjust a number field.
- **Focus** — which control receives typing and keyboard shortcuts.
- **Debounce** — a delay applied before reacting to rapid input; here it is only
  stored as a hint, not a running timer.

All Rust on this page goes into the crate's `src/main.rs`
(`bamboo-hello/bamboo-hello/src/main.rs`).

## Steps

### 1. Define the messages and state

```rust
use std::time::Duration;

use bamboo_native_ui::widgets::{EditColors, Label, NumberInput, SearchBox, TextArea, TextEdit};
use bamboo_native_ui::{
    Color, CloseReason, Host, KeyCode, Rect, ScrollBars, Ui, UiHost, WidgetId, WindowHandle,
};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    // One variant per field: each carries the text or number it committed.
    Name(String), Notes(String), Pin(String), Search(String),
    Count(i64), Price(i64), Duration(i64), Char(char), Clicked, Shortcut,
}

struct App { ui: Ui<Msg> }

impl App {
    fn apply(&mut self, message: Msg, window: &mut WindowHandle<'_>) {
        // Show the last message in the status label, then repaint it.
        self.ui.set_label(WidgetId::new("status"), format!("{message:?}"));
        self.ui.flush_damage(window);
    }
}
```

`TextEdit<M>` is the field type; `PasswordBox<M>` and `TextArea<M>` are aliases
for the same widget. `M` is the commit message and callback type.

### 2. Add a native and a painted field

For the default native field, a click or Enter asks the host for an `EDIT`
session. The field paints its own frame and value while closed. The constructor
handler is the **commit**: Enter on a single line, Tab or focus loss.

```rust
impl App {
    fn new() -> Self {
        let mut ui = Ui::new();
        ui.add(Label::new("status", "Edit a field"));
        ui.add(
            // Native by default: Windows opens an `EDIT` child on click/Enter.
            TextEdit::new("name", "Bamboo Native UI", Msg::Name)
                .cue("Track name") // grey hint while the field is empty
                .max_length(64),   // 64 characters; 0 would mean unlimited
        );
        ui.add(
            // `.painted()` keeps the text inside our window so colours apply.
            TextArea::new("notes", "Line one\nLine two", Msg::Notes)
                .multiline().wrap(true).scrollbars(ScrollBars::Vertical)
                .painted()
                .colors(EditColors::new()
                    .background(Color::new(255, 250, 235))
                    .text(Color::new(58, 46, 24))
                    // Selected text, then the highlight behind it.
                    .selection(Color::new(255, 255, 255), Color::new(64, 120, 220))
                    .caret(Color::new(210, 60, 60))
                    .composition_underline(Color::new(226, 168, 58))),
        );
        ui.add(TextEdit::new("pin", "", Msg::Pin).password().max_length(8));
        Self { ui }
    }
}
```

Builders: `.cue(text)` shows hint text; `.password()` masks the value;
`.multiline()` makes Enter insert a newline; `.wrap(bool)`;
`.scrollbars(ScrollBars)` is `None`, `Vertical` (default), `Horizontal` or
`Both`; `.max_length(u32)` (`0` = unlimited); `.read_only(bool)` keeps focus,
selection and copy but refuses mutations.

**What just happened** — `new` created three fields that look similar but behave
differently. **Name** and **PIN** will open a Windows `EDIT` child when clicked,
so only their value and frame are painted by the framework while closed.
**Notes** is `.painted()`, so it edits in place and uses the sand colours; its
`EditColors` also decide the selection highlight, caret and IME underline.

### 3. Observe edits and keys

The constructor handler is only the commit. Add the widget's own callbacks to
see every edit, every typed character and input that never reaches a commit.
This replaces the shorter `notes` builder above.

```rust
ui.add(
    TextArea::new("notes", "", Msg::Notes)
        .painted().multiline()
        // Fires after any user edit that changed the text (not setters).
        .on_change(|text| Some(Msg::Notes(text.to_string())))
        // Observation hook: sees each character before insertion.
        .on_char(|ch| (!ch.is_control()).then_some(Msg::Char(ch)))
        .on_click(|_| Some(Msg::Clicked))
        // Ctrl+S: `Letter(18)` is S.
        .on_key_down(|key| (key.ctrl && key.code == KeyCode::Letter(18))
            .then_some(Msg::Shortcut))
        .on_key_up(|_| None),
);
```

`on_change` runs after a user edit that changed the text (typing, paste, cut,
delete, undo, committed IME); programmatic setters do not fire it. `on_char`
observes each typed character or IME result **before** insertion — an
observation hook, not a filter. `on_click`, `on_right_click` and
`on_double_click` keep the built-in behaviour; `on_key_down`/`on_key_up` receive
`KeyEvent` (a native session's key releases never reach the widget). At most one
message is delivered per input, and an observer wins over `on_change`.

### 4. Read and set the caret and selection

```rust
// Read the caret offset and the highlighted range (offsets in characters).
let _state = self.ui.with_widget::<TextEdit<Msg>, _>(WidgetId::new("notes"),
    |edit| (edit.caret(), edit.selection()));
// Programmatic setter: it bypasses `max_length`, which only limits typing.
self.ui.edit_widget::<TextEdit<Msg>>(WidgetId::new("notes"), |edit| edit.select_all());
```

`caret()` is a character offset; `selection()` is a normalized `(start, end)`
pair or `None` when collapsed. `set_caret`, `set_selection`, `select_all`,
`insert_text` and `clear` are the programmatic setters; they do not enforce
`max_length`, which applies to user input.

### 5. Use a search box

`SearchBox` paints a leading magnifier and a clear button around an embedded
painted field. It commits on Enter, Tab or focus loss.

```rust
ui.add(
    SearchBox::new("search", "", Msg::Search)
        .cue("Search tracks")
        .max_length(64)
        // Stored as a hint only; no timer is started by the widget.
        .debounce(Duration::from_millis(250)),
);
```

`.debounce` is a **hint**: the widget stores it (`debounce_value()`) and never
starts a timer; the message arrives once per commit, not per keystroke.

### 6. Use a number input

`NumberInput::new(id, value, range, on_change)` clamps the initial value to the
inclusive range. The value stays an `i64`.

```rust
// Steppers clamp to the inclusive 1..=500 range; the value stays an i64.
ui.add(NumberInput::new("count", 8, 1..=500, Msg::Count)
    .steppers().step_size(5).suffix(" px"));
// `decimals(2)` paints 2.50 for the raw value 250.
ui.add(NumberInput::new("price", 250, 0..=100_000, Msg::Price).decimals(2));
ui.add(
    // Custom display/parse pair: show m:ss and accept m:ss back.
    NumberInput::new("duration", 95, 0..=3600, Msg::Duration)
        .format(|seconds| format!("{}:{:02}", seconds / 60, seconds % 60))
        .parse(|text| {
            let (m, s) = text.split_once(':')?;
            Some(m.trim().parse::<i64>().ok()? * 60 + s.trim().parse::<i64>().ok()?)
        }),
);
```

`decimals(2)` paints `2.50` for raw `250` and parses typing back into raw units,
so steppers and the accessibility RangeValue stay integer-based. `format` and
`parse` are a pair: a parser returning `None` keeps the current value.

### 7. Forward IME and manage the native editor

The host hides the system composition window for a painted editor, so the
application must forward the composition and the commit.

```rust
impl Host for App {
    fn ime_compose(&mut self, text: &str, cursor: usize, window: &mut WindowHandle<'_>) {
        // Show the uncommitted composition inside the focused painted field.
        if self.ui.ime_compose(text, cursor, window) {
            for message in self.ui.drain_messages() {
                self.apply(message, window);
            }
        }
    }

    fn ime_commit(&mut self, text: &str, window: &mut WindowHandle<'_>) {
        // The IME converted the text: insert it and clear the composition.
        self.ui.ime_commit(text, window);
        for message in self.ui.drain_messages() {
            self.apply(message, window);
        }
    }

    fn edit_committed(&mut self, id: WidgetId, text: &str, window: &mut WindowHandle<'_>) {
        // A native `EDIT` session finished: route its commit message.
        if let Some(message) = self.ui.commit_edit(id, text.to_string(), window) {
            self.apply(message, window);
        }
    }

    fn close_requested(&mut self, _reason: CloseReason, window: &mut WindowHandle<'_>) -> bool {
        if window.edit_active() {
            window.end_edit(); // commit the open editor before closing
        }
        true
    }
}
```

`Ui::ime_compose` writes the uncommitted string into the focused painted field;
`Ui::ime_commit` inserts the converted result, fires the change message and
clears the composition. `commit_edit` stores native text and returns the commit
message; `window.end_edit()` closes a native session (call it before a page
switch), and `window.edit_active()` answers whether one is open.

The `Ui` reports the focused painted editor's caret rectangle to the host
through `UiHost`. `WindowHandle` implements it, so the IME candidate window
follows the caret. A custom painted editor does the same explicitly:

```rust
// Tell the IME where the caret is, in client coordinates.
window.set_caret_rect(Some(Rect::from_xywh(120.0, 40.0, 2.0, 18.0))); // caret
window.set_caret_rect(None); // stop the blink timer
```

### 8. Run it

Run from the crate folder (`bamboo-hello/bamboo-hello`, the one holding
`src/main.rs`).

```powershell
# Working directory: bamboo-hello/bamboo-hello
cargo run
```

The window appears with the five fields and the status label. Click a field and
start typing; the status label shows one of the messages from step 1. A build
error names the failing file and line, and no window opens.

**What just happened** — the run compiled `main.rs` and opened the window.
Keystrokes now travel one of two ways: a native `EDIT` session that reports only
when it commits, or the painted editor's callbacks, which is why the status
label updates at different moments for different fields.

## Expected result

Run `cargo run`. Click **Notes**, switch to a Chinese IME and type: the
composition is drawn inline with the underline, then committed characters
replace it. `Ctrl+S` reports the shortcut; the steppers emit `Msg::Count`.

- **Name** and **PIN** open an `EDIT` child (PIN shows bullets).
- **Notes** edits in place, wraps, scrolls and uses the sand-coloured colours.
- The search box draws its glyph and clear button; number fields honour ranges,
  steppers, decimals and custom formats.
- IME composition is visible inline and the candidate window follows the caret.

## Troubleshooting

- **A field does not accept typing** — the click did not open the native
  `EDIT` session, or the painted field never got focus. Click inside the field
  rectangle, and check that `Ui` receives pointer and key events as on page 03.
- **The native field looks frozen while typing** — it updates on commit. Use
  `edit_changed` for a live status, or `.painted()` to edit in place.
- **Nothing arrives in `apply` while typing a native field** — native fields
  report on commit (Enter, Tab, focus loss), not per keystroke. Add the widget
  callbacks from step 3 or switch the field to `.painted()`.
- **The caret is missing or does not blink** — a painted editor needs focus and
  a caret colour; `.colors(EditColors::new().caret(...))` sets it. A custom
  editor must also report the caret rectangle.
- **Selection has no highlight** — set `EditColors::selection(text, background)`
  on a painted field; the two arguments are the selected text colour and the
  highlight behind it.
- **A password field shows plain text** — `.password()` is missing from the
  builder chain.
- **IME shows no composition** — `Host::ime_compose` is not forwarding to
  `Ui::ime_compose`; the host hides the system composition window.
- **IME text goes into the wrong field** — the composition is routed to the
  focused painted editor. Click the field you want, and make sure focus was not
  left on a native field that is still open.
- **The candidate window opens in the corner** — the caret rectangle is missing.
  Built-in painted fields report it; a custom editor must call
  `window.set_caret_rect`.
- **The search box emits only on commit** — `.debounce` is a stored hint and the
  message arrives once per commit (Enter, Tab or focus loss); there is no
  per-keystroke timer.
- **A number reverts after typing** — the text did not parse. Pair a custom
  `.format` with a matching `.parse`, or keep the fixed-point form.
- **The steppers stop changing the value** — the value reached the end of its
  inclusive range; `1..=500` clamps at 500.

## Next

[13 — Player UI](13-player-ui.md): compose a cover wall, a playlist, a transport
strip and a waveform seek bar into a media-player page.
