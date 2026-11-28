# StatCard

> A compact metric block with a large value and a caption.

## Introduction

`StatCard` paints a small statistic: the value in the title style and accent
text colour, with a caption underneath in the small muted style, inset
horizontally by the card's padding. `emphasis(true)` swaps the background from
the `card` token to the `play_button` tint, marking a highlighted figure. The
frame is a 1 DIP divider border on a square rectangle.

The widget is display-only: it is not focusable, takes no input and emits no
message. It does not adopt `SurfaceStyle`, so the shape is always square and
there is no radius or background builder. Its accessibility node is a
display-only `Text` named `"{caption} {value}"` with the value as the node's
value.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::StatCard;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(StatCard::new("tracks", "600", "Tracks").emphasis(true));
ui.set_rect(WidgetId::new("tracks"), Rect::from_xywh(16.0, 16.0, 140.0, 72.0));
```

The value is the first constructor argument and the caption the second. Update
the value silently through the typed seam (there is no getter for either
field):

```rust
ui.edit_widget::<StatCard>(WidgetId::new("tracks"), |stat| {
    stat.set_value("612");
    true
});
```

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document:

```xml
<widget id="tracks" x="16" y="16" w="140" h="72" label="612"/>
```

`label` is shared content but `StatCard::set_label` writes the **value**, not
the caption. The caption is constructor-only, so a Rust-created card is the
normal path and the document just positions it.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, value, label)` | `&'static str, impl Into<String>, impl Into<String>` | — | Creates a `140 × 72` stat with padding 12. |
| `.padding(f32)` | `f32` | `12.0` | Inner padding in DIPs, clamped at zero. |
| `.emphasis(bool)` | `bool` | `false` | Uses the emphasised background tint. |
| `.set_value(text)` | `impl Into<String>` | — | Silent value replacement. |

### Behaviour

- The value occupies the band `top + 8` to `top + 44` and the caption the strip
  below it; long strings are clipped.
- `Ui::set_label` routes to `set_value` (`content_caps = LABEL`), not to the
  caption.
- The preferred height is 72 DIPs and the widget is not focusable.
- Accessibility: `Text`, display-only, name `"{label} {value}"`,
  `value(value)`; expose it with `WidgetProps::expose(true)` when it matters.

## Events

`StatCard` is display-only: its `input` always returns `None`, it is not
focusable, and it emits no message. Values arrive through `set_value` or
`Ui::set_label`, never through an event.

### Messages

None. The constructor takes no callback.

### Common observers

This widget installs none of the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`).

### Widget-specific callbacks

None.

### Ui triggers

Any widget id can be bound with `Ui::bind(WidgetId::new("tracks"), Trigger::Click, |event, ctx| ...)`;
returning `Some(Msg::...)` queues a message, and the built-in behaviour still
runs after the handler returns (`ctx.run_default()` runs it inline, calling
neither suppresses it). The stat has no built-in behaviour, so a binding is the
only way to make the block clickable.

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

Only the pointer triggers are meaningful, and only when the stat doubles as a
clickable tile; `Activate`, the key and focus triggers never fire because the
widget is not focusable, and `ValueChange` never fires because the widget takes
no input that could change its accessible value.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- **Geometry:** normal `widget` rules apply; `140 × 72` is the designed size.
- **Content:** `label` (`content_caps = LABEL`) writes the value. `text` and
  `value` are unsupported (`xml.content.unsupported`, warnings) and skipped.
- **Local attributes:** none. `skin_attrs()` is empty.
- **Style tokens:** `font-size` and `font-family` apply; the widget has no
  `SurfaceStyle`, so `radius`, `corners` and `background` are
  `xml.style.unsupported` warnings and are skipped.
- **Children:** none.
- **Properties:** the generic runtime set applies (`visible`, `tooltip`,
  `name`, `expose`, `automation-id`, …).

## See also

- `card.md`
- `empty-state.md`
- `../feedback/banner.md`
- `../index.md`
- `../../api/theme-and-palette.md`
