# SearchBox

> A search field with a leading glyph, a placeholder and a clear button.

## Introduction

`SearchBox<M>` paints a framed box with a search glyph, its text (or the cue
while empty) and a clear button (`×`) on the right. Editing happens in a
painted inline session inside the widget, not in a native child control: click
the text area to place the caret, then type, select and use the usual editing
keys. The session commits on Enter, Tab or focus loss, and `on_commit(text)` is
emitted only when the committed text differs from the current value.

The clear button is outside the editing session. Clicking it clears the value
and emits `on_commit(String::new())`; it is a mutation, so a read-only box
refuses it. `.read_only(true)` keeps focus, selection and copy available but
refuses typing, paste, cut, delete, undo and the clear button. `.debounce`
only stores a hint for the application's own timer; the widget emits one
message per commit and never starts a timer itself.

Accessibility is `A11yRole::TextField` with the live value and a `read_only`
flag. `M` is the message type returned by `on_commit`.

## Usage (Rust)

```rust
use std::time::Duration;
use bamboo_native_ui::widgets::SearchBox;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Search(String) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        SearchBox::new("search", "", Msg::Search)
            .cue("Search tracks")
            .debounce(Duration::from_millis(250))
            .max_length(64),
    );
    ui.set_rect(WidgetId::new("search"), Rect::from_xywh(16.0, 16.0, 240.0, 34.0));
}
```

The default cue is `"搜索"` (Chinese for "search"); set an English cue as
above. `debounce_value()` lets the application read the hint it configured.
`is_editing()` reports whether a painted session is open.

## Usage (XML)

`SearchBox` is not in the standard registry. Create it in Rust and address it
by `id` from the document:

```xml
<widget id="search" x="16" y="16" w="240" h="34"
        text="" cue="Search tracks" color="text" radius="medium"/>
```

`text` is the shared content (`content_caps` exposes `TEXT`). `cue` and `color`
are local attributes. The box adopts `SurfaceStyle`, so `radius`/`corners` and
`background` also apply.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, text, on_commit)` | `&'static str, impl Into<String>, impl Fn(String) -> M + 'static` | — | Creates the search field. |
| `.cue(impl Into<String>)` | `impl Into<String>` | `"搜索"` | Placeholder shown while the value is empty. |
| `.read_only(bool)` | `bool` | `false` | Refuse mutations but keep focus, selection and copy. |
| `.max_length(u32)` | `u32` | `0` (unlimited) | Maximum characters the editor accepts. |
| `.debounce(Duration)` | `Duration` | `None` | Hint for the application's own debounce timer. |
| `.corner_radius(f32)` | `f32` | `0.0` | Uniform frame radius. |
| `.corners(Corners)` | `Corners` | zero | Per-corner frame radii. |
| `.background(Color)` | `Color` | `None` (palette `search_bg`) | Frame fill override. |
| `.background_option(Option<Color>)` | `Option<Color>` | `None` | Set or clear the fill override. |
| `.transparent_background(bool)` | `bool` | `false` | Suppress the fill, keeping the border. |

Getters and setters: `text()`, `is_read_only()`, `is_editing()`,
`cue_text()`, `debounce_value()`, `color_value()`, `surface()`, `set_cue`
and `set_color(Option<Color>)`.

Behaviour notes:

- The widget owns the keyboard while the session is open (arrows, Home/End,
  Delete/Backspace, undo/redo, copy/cut); the container intercepts Tab, Escape,
  Enter and the paste chord.
- The clear button is a 22 DIP target near the right edge and is painted only
  while the value is non-empty; a press on it never opens the editing session.
- Cursor: I-beam over text, arrow over the clear target.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(String) -> M` passed to `new` (`on_commit`) | The painted session commits on Enter/Tab/focus loss, or the clear target is clicked; only when the committed text differs from the stored value. |

### Common observers

`SearchBox` does not install the shared observer builders. Its mechanism is
the constructor's `on_commit` callback plus the container-driven editing
session; use `Ui::bind` for additional triggers. `.debounce(Duration)` is only
a hint for the application's own timer and emits nothing itself.

### Widget-specific callbacks

None beyond `on_commit`. `.debounce(Duration)` stores a debounce hint and
`.max_length(u32)` bounds typing; neither is an event callback.

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

The box reacts to `Click` (only the clear target) and `Activate` (opens the
inline session); `HoverEnter`/`HoverLeave` highlight the clear target. While
editing, keys, wheel and drag go to the painted field. `ValueChange` follows a
committed value change.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- Geometry: normal `widget` rules apply; the default height is 34 DIPs.
- Shared content: `text`.
- Local attributes: `cue` (text) and `color` (text colour).
- Style tokens: `radius`, `corners` and `background` are supported through
  `SurfaceStyle`; `font-size` and `font-family` apply.
- Child elements: none. `read_only`, `max_length` and `debounce` are
  constructor-only.

## See also

- `../index.md`
- `text-edit.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
