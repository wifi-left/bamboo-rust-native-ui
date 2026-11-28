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
    Name(String), Notes(String), Pin(String), Search(String),
    Count(i64), Price(i64), Duration(i64), Char(char), Clicked, Shortcut,
}

struct App { ui: Ui<Msg> }

impl App {
    fn apply(&mut self, message: Msg, window: &mut WindowHandle<'_>) {
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
            TextEdit::new("name", "Bamboo Native UI", Msg::Name)
                .cue("Track name")
                .max_length(64),
        );
        ui.add(
            TextArea::new("notes", "Line one\nLine two", Msg::Notes)
                .multiline().wrap(true).scrollbars(ScrollBars::Vertical)
                .painted()
                .colors(EditColors::new()
                    .background(Color::new(255, 250, 235))
                    .text(Color::new(58, 46, 24))
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

### 3. Observe edits and keys

The constructor handler is only the commit. Add the widget's own callbacks to
see every edit, every typed character and input that never reaches a commit.
This replaces the shorter `notes` builder above.

```rust
ui.add(
    TextArea::new("notes", "", Msg::Notes)
        .painted().multiline()
        .on_change(|text| Some(Msg::Notes(text.to_string())))
        .on_char(|ch| (!ch.is_control()).then_some(Msg::Char(ch)))
        .on_click(|_| Some(Msg::Clicked))
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
let _state = self.ui.with_widget::<TextEdit<Msg>, _>(WidgetId::new("notes"),
    |edit| (edit.caret(), edit.selection()));
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
        .debounce(Duration::from_millis(250)),
);
```

`.debounce` is a **hint**: the widget stores it (`debounce_value()`) and never
starts a timer; the message arrives once per commit, not per keystroke.

### 6. Use a number input

`NumberInput::new(id, value, range, on_change)` clamps the initial value to the
inclusive range. The value stays an `i64`.

```rust
ui.add(NumberInput::new("count", 8, 1..=500, Msg::Count)
    .steppers().step_size(5).suffix(" px"));
ui.add(NumberInput::new("price", 250, 0..=100_000, Msg::Price).decimals(2));
ui.add(
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
        if self.ui.ime_compose(text, cursor, window) {
            for message in self.ui.drain_messages() {
                self.apply(message, window);
            }
        }
    }

    fn ime_commit(&mut self, text: &str, window: &mut WindowHandle<'_>) {
        self.ui.ime_commit(text, window);
        for message in self.ui.drain_messages() {
            self.apply(message, window);
        }
    }

    fn edit_committed(&mut self, id: WidgetId, text: &str, window: &mut WindowHandle<'_>) {
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
window.set_caret_rect(Some(Rect::from_xywh(120.0, 40.0, 2.0, 18.0))); // caret
window.set_caret_rect(None); // stop the blink timer
```

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

- **The native field looks frozen while typing** — it updates on commit. Use
  `edit_changed` for a live status, or `.painted()` to edit in place.
- **IME shows no composition** — `Host::ime_compose` is not forwarding to
  `Ui::ime_compose`; the host hides the system composition window.
- **The candidate window opens in the corner** — the caret rectangle is missing.
  Built-in painted fields report it; a custom editor must call
  `window.set_caret_rect`.
- **A number reverts after typing** — the text did not parse. Pair a custom
  `.format` with a matching `.parse`, or keep the fixed-point form.

## Next

[13 — Player UI](13-player-ui.md): compose a cover wall, a playlist, a transport
strip and a waveform seek bar into a media-player page.
