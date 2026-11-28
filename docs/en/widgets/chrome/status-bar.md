# StatusBar

> A read-only bottom strip of status segments, an optional progress segment and a size grip.

## Introduction

`StatusBar` paints a horizontal strip of `StatusSegment` cells over
`palette.panel`, with a 1-DIP top divider and a thin divider between segments.
Each segment carries text, an optional leading icon and an optional fixed width;
unset widths share the remaining space. A right-aligned progress segment and
the classic diagonal size grip can occupy the right edge, reserved out of the
segments' width.

The bar is decoration for pointer and keyboard input: it is not focusable, not
interactive and not a Tab stop. It still publishes a `StatusBar` node with one
text child per segment, so a screen reader can read it. Segment 0 is the message
segment, which the generic `Ui::set_text` path updates when the application has
no typed access.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{StatusBar, StatusSegment};
use bamboo_native_ui::{Rect, TextAlign, Ui, WidgetId};

enum Msg {
    Tick,
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    StatusBar::new("status")
        .segment(StatusSegment::new("Ready").icon('♪'))
        .segment(
            StatusSegment::new("600 tracks · lossless")
                .align(TextAlign::Right)
                .width(160.0),
        )
        .segment(StatusSegment::new("44.1 kHz").align(TextAlign::Right).width(96.0))
        .size_grip(true)
        .progress(0.62)
        .label("Status"),
);
ui.set_rect(WidgetId::new("status"), Rect::from_xywh(0.0, 576.0, 800.0, 24.0));
```

`progress` clamps to `0..=1`; update it without a message with `set_progress`,
which returns whether the value changed. Update a segment's text with
`set_segment(index, text)`.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. Its child list is `<segment>`, and the
shared `text` and `progress` content attributes also apply:

```xml
<widget id="status" x="0" y="576" w="800" h="24"
        text="Ready" progress="0.62">
  <segment text="600 tracks · lossless" align="right" width="160"/>
  <segment text="44.1 kHz" icon="♫" width="96"/>
</widget>
```

The applier writes the child list first and the shared content afterwards, so
`text` overwrites the first segment's text; `progress` adds (or updates) the
right-aligned progress segment. Use one style or the other for segment 0.

## Properties

`StatusBar`:

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates an empty bar; `420 × 24` DIP. |
| `.segment(StatusSegment)` | segment | — | Appends one segment. |
| `.text(text)` | text | — | Appends a plain left-aligned segment. |
| `.label(text)` | text | widget id | Accessible name of the strip. |
| `.size_grip(bool)` | `bool` | `false` | Draws the diagonal grip and reserves 14 DIP. |
| `.progress(f32)` | `f32` | `None` | Adds a right-aligned progress bar, clamped to `0..=1`. |
| `.segments()` / `.set_segments(Vec<StatusSegment>)` | segments | — | Read or replace every segment (silent). |
| `.has_size_grip()` / `.progress_value()` | `bool` / `Option<f32>` | — | Current options. |
| `.set_progress(f32)` | `bool` | — | Runtime progress (`0..=1`); returns whether it changed. |
| `.set_segment(index, text)` | `bool` | — | Runtime segment text; returns whether it changed. |

`StatusSegment`:

| Method | Type | Default | Description |
|---|---|---|---|
| `new(text)` | `impl Into<String>` | — | A left-aligned segment. |
| `.icon(char)` | `char` | none | Leading MDL2 codepoint or text character (20 DIP strip). |
| `.width(f32)` | `f32` | flexible | Fixed width, clamped to `>= 20`; unset widths share the rest. |
| `.align(TextAlign)` | `TextAlign` | `Left` | Text alignment inside the segment. |

### Behaviour

- **Layout.** Each segment is inset 10 DIP from its edges; a segment divider is
  painted 5 DIP in from the top and bottom. The progress segment reserves 96
  DIP and the grip 14 DIP at the right edge.
- **Painting.** A progress value paints a 6-DIP track centered vertically with
  an accent fill; the grip is three diagonal rows of dots in `scroll_thumb`.
- **Accessibility.** Role `StatusBar`, display-only, named by `.label` (or the
  widget id). Children are the segments plus a `ProgressBar` child named
  `进度 N%` when a progress value is set.

## Events

### Messages

`StatusBar` emits no messages. It has no constructor message callback and is
read-only: the application updates it through `set_progress`,
`set_segment(index, text)`, `set_segments` or the generic `Ui::set_text`, none
of which emit.

### Common observers

This widget installs none of the shared observer builders. It is not focusable,
not interactive and not a Tab stop, so there is no input to observe.

### Widget-specific callbacks

None. The bar exposes silent setters (`set_progress`, `set_segment`,
`set_segments`) and a `Ui::set_text` path instead of callbacks.

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("status"), Trigger::Click, |event, ctx| {
    // Return Some(Msg::...) to queue a message; call ctx.default() to keep the
    // built-in action, which then runs after the handler.
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

The bar reacts to none of them: it is display-only and cannot take focus, so no
pointer or keyboard trigger reaches it.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties (`visible`,
`tooltip`, `name`, `description`, `automation-id`, `role`) apply. Content
support is `text` (segment 0) and `progress` (the progress segment). The bar
paints from the palette and has no `SurfaceStyle`, so `radius`, `corners` and
`background` are reported as `xml.style.unsupported`.

Widget-local attributes: none (`size_grip` and `label` are Rust builders).

Child elements:

| Element | Attributes | Description |
|---|---|---|
| `<segment>` | `text` (required, non-empty), `icon` (one character), `width` (number, clamped to `>= 20`), `align` (`left`/`center`/`right`) | One cell; the list replaces every segment. |

## See also

- `./toolbar.md`
- `./menu-bar.md`
- `../controls/progress-bar.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
