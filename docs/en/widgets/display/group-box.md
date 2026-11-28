# GroupBox

> A titled frame that groups the ordinary widgets placed inside it.

## Introduction

`GroupBox` paints a fieldset-like frame: a card-coloured fill, a 1 DIP border in
the explicit-border colour, a small title in the top band and a 1 DIP divider
rule under that band. The band is 28 DIPs high, and the title plus rule are
skipped unless the rectangle is wider than 20 DIPs and taller than 28.

The frame is decoration only. `Ui` has no container tree, so the application
places ordinary widgets inside the rectangle as siblings: add the group box
first so it paints behind them, and leave the 28 DIP header band free for the
title. The widget takes no input, emits no message and is not focusable; its
accessibility node is a display-only `Group` that joins the tree only when the
instance opts in.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{GroupBox, Toggle};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Shuffle(bool) }

let mut ui: Ui<Msg> = Ui::new();
ui.add(GroupBox::new("playback_group", "Playback options"));
ui.add(Toggle::new("shuffle", "Shuffle", false, Msg::Shuffle));

ui.set_rect(WidgetId::new("playback_group"), Rect::from_xywh(16.0, 16.0, 240.0, 120.0));
ui.set_rect(WidgetId::new("shuffle"), Rect::from_xywh(28.0, 56.0, 180.0, 24.0));
```

`header_height()` returns the 28 DIP band, so a layout can compute where the
first child starts instead of hard-coding the offset. Renaming the group at
runtime goes through the shared label setter:

```rust
ui.set_label(WidgetId::new("playback_group"), "Playback");
```

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document:

```xml
<widget id="playback_group" x="16" y="16" w="240" h="120" label="Playback options"/>
```

`label` is shared content and writes the title. The frame has no widget-local
attributes, so its look is fixed.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, title)` | `&'static str, impl Into<String>` | — | Creates a `240 × 120` frame. |
| `.title()` | `&str` | — | Reads the title. |
| `.header_height()` | `f32` | `28.0` | Height of the title band; children start below it. |

### Behaviour

- The preferred height is 120 DIPs and the widget is not focusable.
- The frame paints `explicit_border` over `card`; it does not adopt
  `SurfaceStyle`, so there is no radius, background or transparency override.
- The title is small text at `left + 10`, top-aligned in the band; the divider
  runs from `left + 1` to `right - 1` at `top + 28`.
- `Ui::set_label` renames it silently (`content_caps = LABEL`); there is no
  public `set_title`.
- Accessibility: `Group`, display-only, named by the title.

## Events

The frame is decoration: its `input` always returns `None`, it is not focusable,
and it emits no message. Events belong to the widgets placed inside it, not to
the frame itself.

### Messages

None. The constructor takes no callback.

### Common observers

This widget installs none of the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`).

### Widget-specific callbacks

None.

### Ui triggers

Any widget id can be bound with `Ui::bind(WidgetId::new("playback_group"), Trigger::Click, |event, ctx| ...)`;
returning `Some(Msg::...)` queues a message, and the built-in behaviour still
runs after the handler returns (`ctx.run_default()` runs it inline, calling
neither suppresses it). The group box has no built-in behaviour, so a binding
only reports presses that land on the frame itself, not on the children.

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

No trigger is meaningful for normal use; the pointer triggers would only fire on
the frame's own area, and `Activate`, the key/focus triggers and `ValueChange`
never fire because the frame is not focusable and carries no value.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- **Geometry:** normal `widget` rules apply; give it at least 20 × 28 DIPs or
  the title band is omitted.
- **Content:** `label` (`content_caps = LABEL`) writes the title; `text` is
  unsupported.
- **Local attributes:** none. `skin_attrs()` is empty.
- **Style tokens:** `font-size` and `font-family` apply; `radius`, `corners`
  and `background` are `xml.style.unsupported` warnings and are skipped.
- **Children:** none. The widgets shown inside are ordinary siblings in the
  layout document, not children of the group.
- **Properties:** the generic runtime set applies, including `expose`, which is
  how the group joins the accessibility tree.

## See also

- `card.md`
- `section-title.md`
- `../navigation/accordion.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
