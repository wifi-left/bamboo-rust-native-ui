# EmptyState

> A centred title and hint for a view that has no content.

## Introduction

`EmptyState` paints the zero-content state for an empty view: a title in the body text
colour and a hint one line below it in the small muted style, both horizontally
centred on the widget's rectangle and stacked around its vertical centre. It
paints no frame or fill, so it sits directly on the page background.

The widget is display-only and inert: it is not focusable, takes no input and
emits no message. There are no builders beyond the constructor and no runtime
setters; the two texts are fixed for the instance's lifetime. Its accessibility
node is a display-only `Text` named by the title, with the hint as the
description.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::EmptyState;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(EmptyState::new(
    "queue_empty",
    "Nothing queued",
    "Add a track to start playing.",
));
ui.set_rect(WidgetId::new("queue_empty"), Rect::from_xywh(16.0, 96.0, 320.0, 120.0));
```

Centre the rectangle in the region the empty list would have occupied so the
two lines land where the content would be. To change the texts, remove and add
the widget again; there is no silent setter.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. The document can position it and set
generic properties such as `visible`, `expose` and `name`:

```xml
<widget id="queue_empty" x="16" y="96" w="320" h="120" name="Queue is empty"/>
```

The title and hint are constructor-only, and the entry accepts no content
attribute; the document supplies geometry and narration only.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, title, hint)` | `&'static str, impl Into<String>, impl Into<String>` | — | Creates a `320 × 120` empty state. |

### Behaviour

- The title is centred with its baseline band `centre.y - 28` to `centre.y - 4`;
  the hint occupies `centre.y` to `centre.y + 24`.
- The preferred height is 120 DIPs and the widget is not focusable.
- No fill, border or surface is painted; there is no `SurfaceStyle`.
- Accessibility: `Text`, display-only, name = title, description = hint.

## Events

`EmptyState` is inert: its `input` always returns `None`, it is not focusable,
and it emits no message. If the empty view needs a call to action, place a real
`Button` over the rectangle instead of binding the text block.

### Messages

None. The constructor takes no callback.

### Common observers

This widget installs none of the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`).

### Widget-specific callbacks

None.

### Ui triggers

Any widget id can be bound with `Ui::bind(WidgetId::new("queue_empty"), Trigger::Click, |event, ctx| ...)`;
returning `Some(Msg::...)` queues a message, and the built-in behaviour still
runs after the handler returns (`ctx.run_default()` runs it inline, calling
neither suppresses it). The empty state has no built-in behaviour; binding it
turns the whole centred block into one click target.

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

Only pointer triggers can be observed and only if the block is deliberately made
clickable; `Activate`, the key and focus triggers never fire because the widget
is not focusable, and it has no value, so neither does `ValueChange`.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- **Geometry:** normal `widget` rules apply; a wide short rectangle suits the
  centred pair of lines.
- **Content:** none. `label` and `text` are unsupported
  (`xml.content.unsupported`, warnings) and skipped.
- **Local attributes:** none. `skin_attrs()` is empty.
- **Style tokens:** `font-size` and `font-family` apply; `radius`, `corners` and
  `background` are `xml.style.unsupported` warnings and are skipped because
  nothing is filled.
- **Children:** none.
- **Properties:** the generic runtime set applies (`visible`, `tooltip`, `name`,
  `description`, `expose`, `automation-id`, …).

## See also

- `stat-card.md`
- `section-title.md`
- `../data/list-view.md`
- `../index.md`
- `../../api/widget-trait.md`
