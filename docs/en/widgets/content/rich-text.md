# RichText

> Wrapped text with inline emphasis and clickable link runs.

## Introduction

`RichText<M>` lays out a sequence of `RichRun` values as one paragraph. Runs
are measured together, so word wrap crosses run boundaries and a space at a
break point is not painted. Each run has a `RunStyle`:

- `Normal` uses the palette text colour.
- `Strong` uses the semibold weight.
- `Muted` uses the palette's muted colour.
- `Link` uses the accent text colour, paints an underline, and is clickable.

With `.on_link(handler)` installed, clicking a link run emits
`handler(run.link)`; the cursor is the hand only over a link rectangle, and the
whole string remains the accessible name. Without a handler the widget is not
interactive. `RichRun` is the value type and `RunStyle` the emphasis enum; both
are re-exported by `bamboo_native_ui::widgets`.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{RichRun, RichText, RunStyle};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Link(u32) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        RichText::new("note")
            .run("Inline styles: ")
            .strong("bold")
            .run(", ")
            .muted("muted")
            .run(", ")
            .link(7, "details")
            .on_link(Msg::Link),
    );
    ui.set_rect(WidgetId::new("note"), Rect::from_xywh(16.0, 16.0, 320.0, 40.0));

    ui.add(RichText::new("explicit").runs(vec![
        RichRun::new("Hello "),
        RichRun::new("world").style(RunStyle::Strong),
    ]));
    ui.set_rect(WidgetId::new("explicit"), Rect::from_xywh(16.0, 64.0, 320.0, 24.0));
}
```

`.link(id, text)` stores an opaque `u32` id that the handler receives; use it
to tell links apart. `plain_text()` returns the concatenated runs, which is
also the accessible name and the copy text.

## Usage (XML)

`RichText` is not in the standard registry. Create it in Rust and address it by
`id` from the document. The `<run>` child list replaces the inline runs:

```xml
<widget id="note" x="16" y="16" w="320" h="40" wrap="true" align="left">
  <run text="Inline styles: "/>
  <run text="bold" style="strong"/>
  <run text=", " />
  <run text="details" style="link" link="7"/>
</widget>
```

A shared `label` on the same entry is written after the children and therefore
replaces the runs with one plain run.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates empty rich text. |
| `.run(text)` | `impl Into<String>` | — | Append a normal run. |
| `.strong(text)` | `impl Into<String>` | — | Append a semibold run. |
| `.muted(text)` | `impl Into<String>` | — | Append a muted run. |
| `.link(id, text)` | `u32, impl Into<String>` | — | Append a link run carrying `id`. |
| `.runs(Vec<RichRun>)` | `Vec<RichRun>` | empty | Replace all runs. |
| `.wrap(bool)` | `bool` | `true` | Wrap long lines at word boundaries. |
| `.align(TextAlign)` | `TextAlign` | `TextAlign::Left` | Line alignment. |
| `.style(LabelStyle)` | `LabelStyle` | `LabelStyle::Body` | Base theme text role. |
| `.on_link(handler)` | `impl Fn(u32) -> M + 'static` | `None` | Emit the link id on click. |

Runtime seams: `set_wrap(bool)`, `set_align(TextAlign)`,
`set_text_style(LabelStyle)` (each returns whether it changed),
`set_color(Option<Color>)`, `set_runs(Vec<RichRun>)`, `wrap_enabled()`,
`alignment()`, `text_style()`, `color_value()`, `runs_ref()` and
`plain_text()`.

### `RichRun` and `RunStyle`

| Item | Type | Default | Description |
|---|---|---|---|
| `RichRun::new(text)` | `impl Into<String>` | — | Creates a run with `Normal` style and link id 0. |
| `RichRun::style(RunStyle)` | `RunStyle` | `RunStyle::Normal` | Emphasis. |
| `RichRun::text` | `String` | — | Public field; the run's text. |
| `RichRun::link` | `u32` | `0` | Public field; id passed to the link handler. |

`RunStyle` has `Normal` (default), `Strong`, `Muted` and `Link`.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(u32) -> M` passed to `.on_link(handler)` | A click lands on a `RunStyle::Link` run; the argument is that run's link id. |

### Common observers

`RichText` does not install the shared observer builders. Its only callback
is the widget-specific `.on_link(handler)`; without it the widget is not
interactive and no run is clickable.

### Widget-specific callbacks

| Callback | Handler | Fires when |
|---|---|---|
| `.on_link(handler)` | `impl Fn(u32) -> M + 'static` | A click inside a link run's painted rectangle; receives the run's `link` id. |

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

Only `Click` matters, and only over a link run when `.on_link` is set;
`HoverEnter`/`HoverLeave` only switch the hand cursor over such a run. The
remaining triggers are not handled.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- Geometry: normal `widget` rules apply.
- Shared content: `label` only. It replaces the runs with one normal run.
- Local attributes: `wrap` (bool), `align` (`left`, `center`, `right`),
  `text-style` (`body`, `small`, `title`) and `color` (normal/strong runs;
  muted and link runs keep their palette roles).
- Child elements: `<run>` with `text` (required, non-empty), `style`
  (`normal`, `strong`, `muted`, `link`) and `link` (a `u32`, legal only when
  `style="link"`). A `<run>` takes no children.
- Style tokens: no `SurfaceStyle`; `radius`, `corners` and `background` are
  unsupported (`xml.style.unsupported`, a warning).

## See also

- `../index.md`
- `label.md`
- `copyable-text.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
