# TooltipCard

> A styled in-page hint panel with a title and a body line.

## Introduction

`TooltipCard` paints a small rounded panel for hints that live inside the page:
a title in the body style and an optional body line in the small muted style,
inside a 1 DIP border on the panel colour with a medium corner radius. It is an
ordinary widget in the paint and hit-test order, so the application positions
it, shows it and hides it with the normal rectangle and visibility APIs.

This is not the native host tooltip. The M4 tooltip is a separate top-level
window the platform shows for `WidgetProps::tooltip`, with its own dwell and
anchor behaviour. `TooltipCard` only shares the palette family and the rounded
shape; it is a page widget used for inline explanations, callouts and
onboarding hints.

The card is display-only: it is not focusable, takes no input and emits no
message. Its accessibility node is a display-only `Text` named by the title,
with the body as the description.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::TooltipCard;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(TooltipCard::new("hint", "Now playing").body("Space toggles playback."));
ui.set_rect(WidgetId::new("hint"), Rect::from_xywh(296.0, 120.0, 240.0, 56.0));
```

Place it where the hint should appear, for example next to the control it
explains. Showing and hiding is the usual visibility toggle; the card has no
open/close state of its own.

For a hint that should appear on hover over a control, use the generic
`WidgetProps::tooltip` instead: that path opens the native tooltip window and
also feeds the accessible help text.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. The document can position it and set
generic properties such as `visible`, `expose` and `name`:

```xml
<widget id="hint" x="296" y="120" w="240" h="56" name="Playback hint"/>
```

The title and body are constructor-only; the document cannot set them.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, title)` | `&'static str, impl Into<String>` | — | Creates a `240 × 56` card with an empty body. |
| `.body(text)` | `impl Into<String>` | `""` | Second line in the small muted style. |

### Behaviour

- The panel uses the medium radius token (6 DIPs), an `explicit_border` outline
  and a `panel` fill; both are painted directly, not through `SurfaceStyle`.
- A title-only card keeps the title's own line height; adding a body shifts the
  layout to the two-line arrangement painted from the top.
- The preferred height is 56 DIPs and the widget is not focusable.
- There are no silent setters for the title or body; rebuild or pass new text
  at construction time.
- Accessibility: `Text`, display-only, name = title, description = body.

## Events

`TooltipCard` is display-only: its `input` always returns `None`, so it emits no
message and fires no callback of its own. Showing and hiding is the ordinary
visibility toggle the application performs.

### Messages

None. The constructor takes no callback.

### Common observers

This widget installs none of the shared observer builders. It is not focusable
and consumes no input, so `on_click`, `on_right_click`, `on_double_click`,
`on_key_down`, `on_key_up`, `on_focus_gained` and `on_focus_lost` do not exist
on it.

### Widget-specific callbacks

None.

### Ui triggers

Any widget id can be bound with `Ui::bind(WidgetId::new("hint"), Trigger::Click, |event, ctx| ...)`;
returning `Some(Msg::...)` queues a message, and the built-in behaviour still
runs after the handler returns (`ctx.run_default()` runs it inline, calling
neither suppresses it). The card has no built-in behaviour, so a binding is the
only way to react.

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

Only the pointer triggers are meaningful for a hint card; `Activate`, the key and
focus triggers never fire because it is not focusable, and it has no value, so
neither does `ValueChange`.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- **Geometry:** normal `widget` rules apply; 240 DIPs wide fits a one-line body.
- **Content:** none. `label` and `text` are unsupported
  (`xml.content.unsupported`, a warning) and skipped.
- **Local attributes:** none. `skin_attrs()` is empty.
- **Style tokens:** `font-size` and `font-family` apply; the card has no
  `SurfaceStyle`, so `radius`, `corners` and `background` are
  `xml.style.unsupported` warnings and are skipped.
- **Children:** none.
- **Properties:** the generic runtime set (`visible`, `tooltip`, `name`,
  `description`, `expose`, `automation-id`, …) applies.

## See also

- `banner.md`
- `../display/card.md`
- `../content/label.md`
- `../index.md`
- `../../api/widget-trait.md`
