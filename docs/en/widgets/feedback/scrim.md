# Scrim

> A dimmed, input-blocking backdrop for an in-window modal shell.

## Introduction

`Scrim` paints a translucent black rectangle over the page and swallows every
pointer press that lands on it. It is the backdrop of the library's in-window
modal pattern: add the scrim after the page's widgets and before the modal
panel, and the panel and its controls stay clickable while the page behind
cannot be reached.

The scrim is visually translucent but input-wise opaque: `interactive()` is
true, `hit_test` covers its rectangle, and `input` consumes presses and returns
no message. It is not focusable and has no accessibility node. Its pixels never
depend on pointer state (`paints_pointer_state` is false), so moving the pointer
across a full-window scrim does not invalidate the page.

`Scrim` is the overlay split. `Widget::is_backdrop` is true for it alone: the
widgets before it form the page layer and the widgets after it form the overlay
layer. With `Host::overlay_paint` the host composites the modal over a cached
page instead of repainting the page every frame (D87). A document-created
widget is always inserted before the first backdrop, so it stays in the page
layer.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{Card, Scrim};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Close }

let mut ui: Ui<Msg> = Ui::new();
ui.add(Card::new("page_card", "Library", "12 tracks"));
ui.add(Scrim::new("modal_scrim"));
ui.add(Card::new("modal_panel", "Delete playlist?", "This cannot be undone."));

ui.set_rect(WidgetId::new("page_card"), Rect::from_xywh(16.0, 16.0, 320.0, 96.0));
ui.set_rect(WidgetId::new("modal_scrim"), Rect::from_xywh(0.0, 0.0, 640.0, 480.0));
ui.set_rect(WidgetId::new("modal_panel"), Rect::from_xywh(160.0, 160.0, 320.0, 120.0));
```

The default rectangle is empty, so a scrim must always be sized, normally to
the whole client area on every resize. Showing and hiding it is a normal
visibility toggle through `Ui::set_visible`. Pair it with
`Ui::set_focus_scope(Some(vec![...]), sink)` so Tab stays inside the panel, and
`set_focus_scope(None, sink)` when the modal closes.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. The document can position it and toggle
the generic `visible` property:

```xml
<widget id="modal_scrim" x="0" y="0" w="cw" h="ch"/>
```

The alpha is constructor-only and cannot be declared in XML.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates a scrim with an empty rectangle and alpha 0.45. |
| `.alpha(f32)` | `f32` | `0.45` | Opacity of the black fill, clamped to `0.0..=1.0`. |

### Behaviour

- **Input blocking.** A press inside the rectangle is consumed; the page
  widgets behind the backdrop never see hover, press or click.
- **Paint.** The fill is black at `alpha`; when the container supplies a dirty
  list only the intersecting pieces are blended, so a hover inside the modal
  does not re-blend the whole window.
- **Cursor.** The scrim asks for the arrow cursor.
- **Preferred height.** `0.0`; the widget paints only the rectangle you assign.
- **Accessibility.** No node is exposed; the modal panel carries the semantics.
- **Ordering.** Add it before every overlay widget. Moving it into the middle of
  the page would put part of the page in the overlay layer.

## Events

The scrim takes every press that lands on it and returns `None` for all of them,
so it never emits a message and has no callback. Closing the modal is the
application's job: a `Click` binding on the scrim id, or a close control inside
the panel, dispatches its own message.

### Messages

None. The constructor takes no callback.

### Common observers

This widget installs none of the shared observer builders. It is not focusable,
so `on_click`, `on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained` and `on_focus_lost` do not exist on it.

### Widget-specific callbacks

None.

### Ui triggers

Any widget id can be bound with `Ui::bind(WidgetId::new("modal_scrim"), Trigger::Click, |event, ctx| ...)`;
returning `Some(Msg::Close)` queues a message, and the built-in behaviour still
runs after the handler returns (`ctx.run_default()` runs it inline, calling
neither suppresses it). The scrim has no built-in behaviour beyond swallowing the
press, so the binding is how a click-outside-to-close modal reports itself.

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

Only `Click` is meaningful — it is the only trigger that can observe the scrim's
press. The scrim is not focusable and exposes no accessibility node, so
`Activate`, the key/focus triggers and `ValueChange` never fire.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- **Geometry:** normal `widget` rules apply; size it to the client area.
- **Content:** none. `label` and `text` are unsupported
  (`xml.content.unsupported`, a warning) and skipped.
- **Local attributes:** none. `skin_attrs()` is empty.
- **Style tokens:** `font-size` and `font-family` are accepted but paint nothing
  (the scrim draws no text); the widget has no `SurfaceStyle`, so `radius`,
  `corners` and `background` are `xml.style.unsupported` warnings and are
  skipped. The fill is always the built-in black blend.
- **Children:** none.
- **Properties:** the generic runtime set applies, including `visible`, which
  is how a modal is opened and closed from a document.

## See also

- `../display/card.md`
- `toast.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
