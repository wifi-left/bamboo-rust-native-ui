# Accordion

> A stack of collapsible sections with one or more bodies open.

## Introduction

`Accordion<M>` paints a vertical stack of section headers with an optional
wrapped body under each open header. Clicking a header, or pressing Enter or
Space on the keyboard cursor, toggles it and reports `(section, open)` through
the `on_toggle` closure. Sections are independent by default; `single_open`
closes the previously open section.

The widget is a container only in the sense that it paints body text inside its
own rectangle; it does not own child widgets. The application sizes the
rectangle to the open state, using `preferred_height` if it lays out
vertically. Every section exposes an expand/collapse state, and open sections
are selected in the accessibility tree.

`Collapsible<M>` is a type alias for `Accordion<M>`; build it with one title.

## Usage (Rust)

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::Accordion;

enum Msg {
    Toggle(usize, bool),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Accordion::new("settings", &["Playback", "Audio"], Msg::Toggle)
        .body(0, "Resume where you left off and keep the queue.")
        .body(1, "Equalizer presets are applied to every output.")
        .open(0, true)
        .single_open(true)
        .header_height(34.0)
        .body_height(56.0)
        .label("Playback settings"),
);
ui.set_rect(WidgetId::new("settings"), Rect::from_xywh(16.0, 16.0, 300.0, 124.0));
```

`new` fixes the section count; bodies start empty and closed. `body` sets the
text, `open` sets an initial state without a message, and `single_open`
enforces at most one open section. A one-section `Collapsible` is
`Accordion::new(id, &["Title"], handler)`.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. The `open` local attribute is a section
index and opens only that section:

```xml
<widget id="settings" x="16" y="16" w="300" h="124" open="0" name="Playback settings"/>
```

The section titles, body texts and the toggle callback are Rust-side state.
There are no child elements: a document cannot add or retitle sections.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, titles, on_toggle)` | `&'static str, &[&str], impl Fn(usize, bool) -> M` | — | Creates one closed, empty section per title. |
| `.body(index, text)` | `usize, impl Into<String>` | `""` | Body text of one section (word-wrapped while open). |
| `.open(index, bool)` | `usize, bool` | `false` | Initial open state; silent. |
| `.single_open(bool)` | `bool` | `false` | Only one section open at a time. |
| `.header_height(f32)` | `f32` | `34.0` | Header row height, clamped to `>= 16`. |
| `.body_height(f32)` | `f32` | `56.0` | Body area of an open section, `>= 0`. |
| `.chevrons(collapsed, expanded)` | `char, char` | MDL2 `U+E70D`, `U+E70E` | Header marks. |
| `.label(impl Into<String>)` | text | widget id | Accessible name of the container. |
| `.disabled(bool)` | `bool` | `false` | Paints disabled and ignores input. |
| `.len()` / `.is_empty()` | `usize` / `bool` | — | Section count. |
| `.is_expanded(index)` | `bool` | — | One section's state. |
| `.open_index()` | `Option<usize>` | — | First open section. |
| `.set_expanded(index, bool)` | `bool` | — | Silent state change; in single-open mode closes others. Returns whether anything changed. |
| `.set_body(index, text)` / `.set_title(index, text)` | silent setters | — | Runtime body/title replacement. |
| `.toggle(index)` | `Option<M>` | — | Toggles through the normal path and returns the message. |

Behaviour: the preferred height is `headers * header_height + open bodies *
body_height`, and the paint is clipped to the widget rectangle, so the
application must grow the rectangle when a section opens. Keyboard input in
adjust mode uses Up/Left and Down/Right to move the cursor (wrapping), Home/End
to jump, and Enter/Space to toggle. Clicking inside an open body does nothing.
The accessibility role is `List`: each section is an item with
`expanded`/`selected` state, and a UIA expand/collapse request drives the same
path as a click.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(usize, bool) -> M` passed to `new` as `on_toggle` | A section toggled: `(section_index, open)`. Fires for header clicks, Enter/Space on the cursor, and UIA expand/collapse requests. |

`.open(index, bool)` and `set_expanded` are silent and never fire it.

### Common observers

This widget does not install the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`). The constructor's `on_toggle` is its event
seam; anything else goes through `Ui::bind`.

### Widget-specific callbacks

| Method | Handler | Fires when |
|---|---|---|
| `new(id, titles, on_toggle)` | `&'static str, &[&str], impl Fn(usize, bool) -> M + 'static` | Stores the toggle callback; it is the only callback the widget declares. |
| `.toggle(index)` | returns `Option<M>` | Runtime form: toggles normally and returns the message, or `None` when disabled or out of range. |

### Ui triggers

Any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("settings"), Trigger::Activate, |event, ctx| {
    // Return Some(Msg::...) to queue a message; the built-in action still runs
    // unless ctx.run_default() is called explicitly.
    None
});
```

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

It reacts to `Click` (header toggle), `Activate`, `HoverEnter`/`HoverLeave` and
`ValueChange` (a section opened or closed); a click inside an open body does
nothing. `Wheel`, `RightClick` and the drag triggers have no built-in effect.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties apply,
including `enabled` and `name`. `font-size` and `font-family` are paint-level.
The accordion paints from the palette and has no `SurfaceStyle`, so `radius`,
`corners` and `background` are reported as `xml.style.unsupported` and skipped.

Local attributes:

| Attribute | Kind | Description |
|---|---|---|
| `open` | number | Section index to open; every other section closes. Must be `>= 0`; an out-of-range index closes all sections. |

Child elements: none. The vector of titles is fixed at construction, and the
document cannot add, remove or retitle sections. The shared `selected`
attribute is accepted (it moves the keyboard cursor) but does not open a
section; use `open`.

## See also

- `nav-rail.md`
- `breadcrumb.md`
- `../index.md`
- `../../api/widget-trait.md`
