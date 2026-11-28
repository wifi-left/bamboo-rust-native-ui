# TextEdit

> A single-line or multiline text field with native or framework-painted editing.

## Introduction

`TextEdit<M>` is the framework's text field. It paints its own frame and text
while closed, so the page looks complete before editing starts. By default a
click (or Enter) asks the platform host to open a native `EDIT` session
(Windows); the committed text returns through `Ui::commit_edit` and becomes the
message returned by `on_commit`. `.painted()` instead edits in place with
framework painting: the widget owns typing, selection, IME composition,
clipboard, undo/redo and its context menu while it is focused.

Two aliases name the same widget with a different vocabulary, so there is no
second implementation to keep in sync:

- `PasswordBox<M>` = `TextEdit<M>`; call `.password()` when building it to mask
  the value with `•` bullets.
- `TextArea<M>` = `TextEdit<M>`; call `.multiline()` for a wrapping, vertically
  scrolling field. Enter inserts a newline; Tab/Escape/focus loss commit or
  cancel like a single-line field.

The closed field keeps its own caret and selection in characters, so programmatic
`set_caret`, `set_selection`, `select_all`, `insert_text` and `clear` work
without an open session. Accessibility is `A11yRole::TextField` with the live
value (masked for a password), a `read_only` flag and the selection range. `M`
is the message type returned by `on_commit` and the optional callbacks.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{EditColors, PasswordBox, TextEdit};
use bamboo_native_ui::{Color, Rect, Ui, WidgetId};

enum Msg { Name(String), Pin(String) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        TextEdit::new("name", "Bamboo Native UI", Msg::Name)
            .cue("Track name")
            .max_length(64)
            .painted()
            .colors(
                EditColors::new()
                    .background(Color::new(255, 250, 235))
                    .text(Color::new(58, 46, 24))
                    .caret(Color::new(210, 60, 60)),
            ),
    );
    ui.set_rect(WidgetId::new("name"), Rect::from_xywh(16.0, 16.0, 280.0, 34.0));

    ui.add(PasswordBox::new("pin", "", Msg::Pin).password());
    ui.set_rect(WidgetId::new("pin"), Rect::from_xywh(16.0, 60.0, 200.0, 34.0));
}
```

The default rectangle is 280×34 (preferred height 84 for multiline). Without
`.painted()` the platform hosts the editing session; `.painted()` is what makes
`EditColors`, inline IME composition and the painted caret applicable.

## Usage (XML)

`TextEdit` is not in the standard registry (neither are its aliases). Create it
in Rust and address it by `id` from the document:

```xml
<widget id="name" x="16" y="16" w="280" h="34"
        text="Bamboo Native UI" cue="Track name" color="text" radius="medium"/>
```

`text` is the shared content (`content_caps` exposes `TEXT`), written silently
once per document generation; if an editing session is open on the id the
write is skipped (`xml.content.session-open`, a warning). `cue` and `color` are
local attributes, and `radius`/`corners`/`background` apply through
`SurfaceStyle`.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, text, on_commit)` | `&'static str, impl Into<String>, impl Fn(String) -> M + 'static` | — | Creates the field. |
| `.painted()` | — | off (native `EDIT` session) | Edit in place with framework painting. |
| `.password()` | — | off | Mask the value with bullets. |
| `.multiline()` | — | off | Wrap, scroll vertically and commit on focus loss. |
| `.wrap(bool)` | `bool` | `true` | Word-wrap long lines (multiline). |
| `.scrollbars(ScrollBars)` | `ScrollBars` | `ScrollBars::Vertical` | Which bars a multiline field shows. |
| `.scrollbar_style(ScrollBarStyle)` | `ScrollBarStyle` | 7 DIP track, 12 DIP minimum thumb | Bar thickness and colours. |
| `.align(TextAlign)` | `TextAlign` | `TextAlign::Left` | Horizontal text alignment. |
| `.font_size(f32)` | `f32` | `None` (theme body) | Per-field text size in DIPs. |
| `.cue(impl Into<String>)` | `impl Into<String>` | `""` | Placeholder while the value is empty. |
| `.padding(Insets)` | `Insets` | `(9, 2)` single-line, `(9, 6)` multiline | Content padding. |
| `.line_spacing(f32)` | `f32` | `2.0` | Extra space between lines, in DIPs. |
| `.vertical_center(bool)` | `bool` | on for single-line | Center a single line vertically. |
| `.caret_width(f32)` | `f32` | `None` (system accessibility width) | Caret bar width, clamped to 0.5..6.0. |
| `.max_length(u32)` | `u32` | `0` (unlimited) | Maximum characters accepted while typing. |
| `.read_only(bool)` | `bool` | `false` | Refuse mutations; keep focus, selection and copy. |
| `.disabled()` | — | enabled | Disable the field. |
| `.corner_radius(f32)` | `f32` | `0.0` | Field frame corner radius. |
| `.corners(Corners)` | `Corners` | zero | Per-corner frame radii. |
| `.background(Color)` | `Color` | `None` (palette `search_bg`) | Frame fill override. |
| `.background_option(Option<Color>)` | `Option<Color>` | `None` | Set or clear the fill override. |
| `.transparent_background(bool)` | `bool` | `false` | Suppress the fill, keeping the border. |

Colours (all optional; `None` keeps the theme):

| Method | Type | Description |
|---|---|---|
| `.colors(EditColors)` | `EditColors` | Replace the whole colour set. |
| `.text_color(Color)` | `Color` | Ordinary text (and the caret default). |
| `.edit_background(Color)` | `Color` | Field fill. |
| `.selection_colors(text, background)` | `Color, Color` | Selected text and highlight. |
| `.caret_color(Color)` | `Color` | Caret colour. |

`EditColors` (`bamboo_native_ui::widgets::EditColors`) exposes public
`Option<Color>` fields `text`, `background`, `selection_text`,
`selection_background`, `caret`, `cue`, `border`, `border_idle` and
`composition_underline`, plus a builder per field and `selection(text, bg)`.

Callbacks: `.on_change(Fn(&str) -> Option<M>)` after every user edit that
changed the text; `.on_char(Fn(char) -> Option<M>)` before insertion (an
observation hook); `.on_click`/`.on_right_click`/`.on_double_click` with
`Fn(Point) -> Option<M>`; `.on_key_down`/`.on_key_up` with
`Fn(KeyEvent) -> Option<M>` (a native session's key releases never reach the
widget).

Accessors: `text()`, `caret()`, `selection() -> Option<(usize, usize)>`,
`is_editing()`, `composition()`, `is_painted()`, `cue_text()`,
`text_color_value()`, `scrollbar()`, `surface()`. Setters: `set_cue`,
`set_text_color(Option<Color>)`, `set_caret(usize)`, `set_selection(anchor,
caret)`, `select_all()`, `clear()` and `insert_text(&str)`.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(String) -> M` passed to `new` (`on_commit`) | The editing session commits (Enter for single-line, Tab or focus loss, or `Ui::commit_edit`); only when the text changed. |
| observer handlers (below) | An installed callback returns `Some(M)` for its input. |

### Common observers

`TextEdit` does **not** use the shared observer macro. It declares its own
callback methods with the same additive rule (the built-in behaviour still
runs) but without `on_focus_gained`/`on_focus_lost`:

| Method | Handler |
|---|---|
| `.on_change(f)` | `Fn(&str) -> Option<M>` |
| `.on_char(f)` | `Fn(char) -> Option<M>` |
| `.on_click(f)` | `Fn(Point) -> Option<M>` |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` |

### Widget-specific callbacks

| Callback | Handler | Fires when |
|---|---|---|
| `.on_change(f)` | `Fn(&str) -> Option<M>` | After every user edit that changed the text (typing, paste, cut, delete, undo/redo, committed IME); programmatic setters do not fire it. |
| `.on_char(f)` | `Fn(char) -> Option<M>` | For each typed/IME character before insertion; the insertion still happens. |
| `.on_click(f)` | `Fn(Point) -> Option<M>` | Left-button press and release on the field. |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | Right-button click; the edit menu still opens if the app installed none. |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | Platform double click; built-in word selection still runs. |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | Every key press (auto-repeat included) before built-in editing. |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | Every key release; a native session's releases never reach the widget. |

These are TextEdit-specific, not the shared observer builders.

### Ui triggers

Any widget id can be bound with `Ui::bind(id, Trigger, handler)` (or the
`Ui::on_click` / `Ui::on_activate` shorthands). A handler that returns
`Some(M)` queues a message; the built-in action still runs unless the handler
calls `ctx.run_default()`.

| Trigger | Fires when |
|---|---|
| `Click` | Press and release on the same widget. |
| `DoubleClick` | The platform reports a double click. |
| `RightClick` | Secondary-button click, before the context menu. |
| `HoverEnter` / `HoverLeave` | The pointer enters / leaves the widget. |
| `Wheel` | A wheel event over the widget (`event.wheel_delta`). |
| `Activate` | Enter/Space on a focused widget, or a UIA Invoke/Toggle. |
| `DragStart` / `DragEnd` | The pointer crosses the drag threshold / the button is released. |
| `ValueChange` | The widget's accessibility value changed (after the built-in update). |

The field reacts to pointer clicks/double/right clicks and drag selection,
`Activate` (opens the painted session), and `Wheel` (multiline scrolling);
`ValueChange` follows a text change.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- Geometry: normal `widget` rules apply.
- Shared content: `text`. `label` is not accepted for this widget.
- Local attributes: `cue` (text) and `color` (text colour).
- Style tokens: `radius`, `corners` and `background` are supported through
  `SurfaceStyle`; `font-size` and `font-family` apply to the closed paint and
  the editor.
- Child elements: none. `password`, `multiline`, `painted`, `scrollbars` and
  the callbacks are constructor-only.

## See also

- `../index.md`
- `search-box.md`
- `number-input.md`
- `../controls/button.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
