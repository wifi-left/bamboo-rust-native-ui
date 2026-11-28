# SectionTitle

> A small muted heading that labels a section of a page.

## Introduction

`SectionTitle` paints one line of small text, left-aligned in its rectangle,
in the palette's muted colour unless an explicit colour is set. It is the
lightweight in-page heading used above lists, groups and secondary content; it
draws no frame, fill or divider.

The widget is display-only: it is not focusable, takes no input and emits no
message. Its accessibility node is a display-only `Heading` named by the label
and exposed at heading level 2, so a screen reader can jump between sections.

Use `Label` for ordinary body text with wrapping and alignment options, and
`GroupBox` when the section needs a frame as well as a title.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::SectionTitle;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {}

let mut ui: Ui<Msg> = Ui::new();
ui.add(SectionTitle::new("queue_title", "Up next"));
ui.set_rect(WidgetId::new("queue_title"), Rect::from_xywh(16.0, 16.0, 200.0, 24.0));

ui.add(SectionTitle::new("recent_title", "Recently played"));
ui.set_rect(WidgetId::new("recent_title"), Rect::from_xywh(16.0, 52.0, 200.0, 24.0));
```

There is no `color` builder; the colour is a silent setter and reads back
through `color_value()`:

```rust
use bamboo_native_ui::Color;

ui.edit_widget::<SectionTitle>(WidgetId::new("queue_title"), |title| {
    title.set_color(Some(Color::new(120, 160, 120)));
    true
});
```

Passing `None` returns to the muted palette colour. Renaming goes through the
shared label setter, `Ui::set_label`.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document:

```xml
<widget id="queue_title" x="16" y="16" w="200" h="24"
        label="Up next" color="muted"/>
```

`label` is shared content and writes the text. `color` is a widget-local
attribute; it accepts the common colour grammar (a palette token or a literal
such as `#7fd3a8`) and resolves against the active palette before it reaches
the widget. There is no `none` value: omit the attribute to keep the muted
default.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, label)` | `&'static str, impl Into<String>` | — | Creates a `200 × 24` heading. |
| `.set_color(Option<Color>)` | `Option<Color>` | `None` | Silent colour override; `None` uses the muted token. |
| `.color_value()` | `Option<Color>` | `None` | The explicit colour, if any. |

### Behaviour

- The text is painted with the small text role, left-aligned, vertically
  filling the rectangle; long labels are clipped, not wrapped.
- The preferred height is 24 DIPs and the widget is not focusable.
- `Ui::set_label` renames it silently (`content_caps = LABEL`).
- Accessibility: `Heading`, display-only, named by the label, heading level 2.

## Events

`SectionTitle` is a label: its `input` always returns `None`, it is not
focusable, and it emits no message. Text and colour arrive through the type's
setters, never through an event.

### Messages

None. The constructor takes no callback.

### Common observers

This widget installs none of the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`).

### Widget-specific callbacks

None.

### Ui triggers

Any widget id can be bound with `Ui::bind(WidgetId::new("queue_title"), Trigger::Click, |event, ctx| ...)`;
returning `Some(Msg::...)` queues a message, and the built-in behaviour still
runs after the handler returns (`ctx.run_default()` runs it inline, calling
neither suppresses it). The heading has no built-in behaviour, so a binding only
reports presses on the text line itself.

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

No trigger is meaningful for a heading; the pointer triggers would only fire on
the label's own 24 DIP band, and `Activate`, the key/focus triggers and
`ValueChange` never fire because the widget is not focusable and carries no
value.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- **Geometry:** normal `widget` rules apply; 24 DIPs is the designed height.
- **Content:** `label` (`content_caps = LABEL`) writes the text; `text` is
  unsupported.
- **Local attributes:** `color` (a colour value, token or literal). An unknown
  name or a malformed literal is `xml.skin.value`.
- **Style tokens:** `font-size` and `font-family` apply; the widget has no
  `SurfaceStyle`, so `radius`, `corners` and `background` are
  `xml.style.unsupported` warnings and are skipped.
- **Children:** none.
- **Properties:** the generic runtime set applies, including `role` (which can
  override the default `heading` token) and `name`.

## See also

- `../content/label.md`
- `group-box.md`
- `card.md`
- `../index.md`
- `../../api/theme-and-palette.md`
- `../../guides/xml-layout-authoring.md`
