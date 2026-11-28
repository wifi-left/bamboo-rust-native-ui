# Card

> A surface panel with a title and a body line.

## Introduction

`Card` paints a framed surface: a 1 DIP border in the divider colour, a fill in
the card token, a title in the body style and a body line in the small muted
style, inset horizontally by the card's padding. `emphasis(true)` adds a 3 DIP
accent strip on the leading edge and shifts the text past it, marking a
selected or highlighted card.

The widget is display-only: it is not focusable, takes no input and emits no
message. It adopts `SurfaceStyle`, so the corner shape, the fill and the
transparency come from the standard surface builders; the default surface is
square. Its accessibility node is a display-only `Group` named by the title,
with the body as the description.

Despite its name the card is not a container: `Ui` is a flat widget list, so
place other widgets over the card's rectangle yourself, after the card in paint
order.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::Card;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Card::new("summary", "Now playing", "Track 3 of 12")
        .padding(14.0)
        .corner_radius(10.0),
);
ui.set_rect(WidgetId::new("summary"), Rect::from_xywh(16.0, 16.0, 240.0, 96.0));
```

The title sits in the first 24 DIPs below the top inset and the body starts
below it, so a body longer than the remaining height is clipped rather than
wrapped. Update the texts silently through the typed seam and report the change
so the card is damaged:

```rust
ui.edit_widget::<Card>(WidgetId::new("summary"), |card| {
    card.set_body("Track 4 of 12");
    true
});
```

## Usage (XML)

`card` is a standard registry type. Create it with `type="card"` when
`WidgetRegistry::standard()` is installed:

```xml
<widget id="summary" type="card" x="16" y="16" w="240" h="96"
        title="Now playing" body="Track 3 of 12" emphasis="true"
        radius="large"/>
```

The factory builds an empty card; the local attributes arrive in the same
generation-scoped pass. A card created in Rust is addressed by id:

```xml
<widget id="summary" x="16" y="16" w="240" h="96" title="Now playing"/>
```

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, title, body)` | `&'static str, impl Into<String>, impl Into<String>` | — | Creates a `240 × 96` card. |
| `.padding(f32)` | `f32` | `14.0` | Inner padding in DIPs, clamped at zero. |
| `.emphasis(bool)` | `bool` | `false` | Paints the 3 DIP accent strip and shifts the text. |
| `.title()` / `.body()` | `&str` | — | Read the current texts. |
| `.set_title(text)` | `impl Into<String>` | — | Silent title replacement. |
| `.set_body(text)` | `impl Into<String>` | — | Silent body replacement. |
| `.is_emphasis()` | `bool` | `false` | Current emphasis state. |
| `.set_emphasis(bool)` | `bool` | — | Silent emphasis toggle (the skin's `emphasis`). |

Surface builders: `corner_radius(f32)`, `corners(Corners)`, `background(Color)`,
`background_option(Option<Color>)`, `transparent_background(bool)` and
`surface()`.

### Behaviour

- The border always paints; `transparent_background(true)` drops only the fill.
- A `background` override replaces the `card` token fill.
- The preferred height is 96 DIPs and the widget is not focusable.
- `Ui::set_label` is not wired to the card (`content_caps` is `NONE`); use the
  silent setters above.
- Accessibility: `Group`, display-only, name = title, description = body.

## Events

`Card` is decoration: its `input` always returns `None`, it is not focusable, and
it emits no message. Making a card clickable means binding a trigger to its id
and using the card as the hit area.

### Messages

None. The constructor takes no callback.

### Common observers

This widget installs none of the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`).

### Widget-specific callbacks

None.

### Ui triggers

Any widget id can be bound with `Ui::bind(WidgetId::new("summary"), Trigger::Click, |event, ctx| ...)`;
returning `Some(Msg::...)` queues a message, and the built-in behaviour still
runs after the handler returns (`ctx.run_default()` runs it inline, calling
neither suppresses it). The card has no built-in behaviour, so the binding is the
whole interaction.

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

Only the pointer triggers are meaningful, and only when the card is used as a
clickable tile; `Activate`, the key and focus triggers never fire because it is
not focusable, and it has no value, so neither does `ValueChange`.

## XML definition

- **Creation:** `type="card"`, no `command` needed; with no standard registry
  the entry is `xml.type.unknown`. A created widget lands in the page layer,
  before the first backdrop (a `Scrim`).
- **Geometry:** normal `widget` rules apply.
- **Content:** none. `label` and `text` are unsupported
  (`xml.content.unsupported`, warnings) and skipped; the texts are local.
- **Local attributes:** `title` (text), `body` (text) and `emphasis` (bool).
  All three are silent writes.
- **Style tokens:** `radius`, `corners`, `background`, `font-size` and
  `font-family`, because the card adopts `SurfaceStyle`.
- **Children:** none. An optional `parent="panel-id"` binds a created entry to a
  registered scroll panel.

## See also

- `group-box.md`
- `stat-card.md`
- `../feedback/banner.md`
- `../index.md`
- `../../api/theme-and-palette.md`
