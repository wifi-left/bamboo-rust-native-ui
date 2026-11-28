# Popover

> An anchor-aware overlay card with a notch, viewport clamping and light dismissal.

## Introduction

`Popover<M>` is the themed surface of a flyout: a rounded panel with a border
and an optional notch that points at an anchor rectangle. It is an overlay
card, not a container. The application still owns layout, so placement is a
helper: call `place_popover` with the anchor, the card size, a `PopoverPlacement`
and the client viewport, then set the returned rectangle on the widget.

The card paints behind the application's content widgets. Add those widgets as
ordinary siblings **after** the popover, positioned inside `Popover::content_rect`,
so they paint above the card and receive pointer input first. The popover
paints no children itself.

With `dismiss_on_outside` (on by default) the widget's hit area covers the
whole client viewport while it is visible: a press outside the card returns the
`on_dismiss` message and is swallowed, so the page behind receives neither the
click nor a hover change. A press on the card is also swallowed, without a
message. Escape is the application's key to handle (or use a close control
inside the card). Turn the option off for a non-modal card that lets the page
receive clicks; then only the card rectangle is part of the hit area.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{place_popover, Label, Popover, PopoverPlacement};
use bamboo_native_ui::{Rect, Size, Ui, WidgetId};

enum Msg {
    Dismiss,
}

let anchor = Rect::from_xywh(16.0, 16.0, 120.0, 32.0);
let viewport = Rect::from_xywh(0.0, 0.0, 800.0, 600.0);
let card = place_popover(
    anchor,
    Size::new(240.0, 120.0),
    PopoverPlacement::Auto,
    viewport,
    8.0,
);

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Popover::new("flyout")
        .anchor(anchor)
        .pad(12.0)
        .arrow(true)
        .dismiss_on_outside(true)
        .name("Actions")
        .description("Press outside to close")
        .on_dismiss(|| Msg::Dismiss),
);
ui.set_rect(WidgetId::new("flyout"), card);

// Content widgets are siblings added after the card, inside its padding.
let content = card.inset(12.0, 12.0);
ui.add(Label::new("flyout_title", "Nocturne"));
ui.set_rect(WidgetId::new("flyout_title"), content);
```

`place_popover` does not move the card; it only computes the rectangle.
`content_rect()` is the card inset by `pad`, so a matching inset gives the same
area without typed access. Show and hide the flyout through `Ui::set_visible`
(which takes the host as a `UiHost` sink) on the popover **and** its content
widgets.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. It accepts no widget-local attributes and
no child elements.

```xml
<widget id="flyout" x="16" y="56" w="240" h="120" visible="false"/>
```

The card is positioned in Rust from `place_popover`; the document usually only
toggles `visible`. Its content widgets are separate entries with their own ids.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id)` | `&'static str` | — | Creates the card; `240 × 120` DIP. |
| `.anchor(Rect)` | `Rect` | none | Anchor the notch points at (does not move the card). |
| `.pad(f32)` | `f32` | `12.0` | Inset between the card edge and `content_rect`, clamped `>= 0`. |
| `.corner_radius(f32)` | `f32` | `6.0` (`radius::MEDIUM`) | Card corner radius, clamped `>= 0`. |
| `.dismiss_on_outside(bool)` | `bool` | `true` | A press outside the card dismisses. |
| `.arrow(bool)` | `bool` | `true` | Paint the notch pointing at the anchor. |
| `.name(text)` | text | `""` | Accessible name of the card group. |
| `.description(text)` | text | `""` | Accessible description. |
| `.on_dismiss(handler)` | `Fn() -> M` | none | Message emitted by an outside press. |
| `.content_rect()` | `Rect` | — | The card inset by `pad`, for content placement. |
| `.set_anchor(Rect)` | `Rect` | — | Runtime anchor update. |
| `.set_viewport(Rect)` / `.viewport()` | `Rect` | last paint | Client viewport used by the light-dismiss hit area. |
| `.dismiss_on_outside_enabled()` | `bool` | — | Current dismissal mode. |

`preferred_height` is 120 DIP and the card is not focusable.

### Behaviour

- **Placement.** `place_popover(anchor, size, placement, viewport, gap)` centers
  the card below the anchor for `Below`, above for `Above`, and beside it for
  `Right`/`Left`; `gap` is clamped to `>= 0`. `Auto` tries below, above, right,
  then left, and falls back to the roomier vertical side. A non-empty viewport
  clamps the result with an 8-DIP margin; an empty viewport means no clamping
  (the initial layout pass).
- **Notch.** The notch is painted on the edge facing the anchor when the arrow
  is on, the anchor does not overlap the card, and the card has an anchor. The
  widget's paint bounds grow by 10 DIP when a notch is present.
- **Input.** A card press is swallowed. With dismissal on, an outside press
  inside the client viewport emits the dismissal message; without a viewport
  only the card itself is hit. The card takes the arrow cursor and paints no
  pointer state, so moving across the page does not repaint it.
- **Accessibility.** Role `Group` named by `.name`, described by
  `.description`.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `impl Fn() -> M` passed to `.on_dismiss` | A press lands outside the card while `dismiss_on_outside` is on. |

A press on the card, and a press inside the viewport when dismissal is off,
emit nothing. Escape is not handled here; the application routes it.

### Common observers

This widget installs none of the shared observer builders. Use `on_dismiss` or
`Ui::bind`.

### Widget-specific callbacks

| Callback | Signature | Fires when |
|---|---|---|
| `.on_dismiss(handler)` | `impl Fn() -> M` | An outside press requests dismissal. |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("flyout"), Trigger::Click, |event, ctx| {
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

The card itself reacts to no trigger: it swallows presses inside its hit area,
paints no pointer state, and only an outside press produces `on_dismiss`.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties (`visible`,
`tooltip`, `name`, `description`, `automation-id`, `role`) apply. The card
paints `palette.panel` over an `explicit_border` frame and has no
`SurfaceStyle`, so `radius`, `corners` and `background` are reported as
`xml.style.unsupported`; use `.corner_radius` in Rust instead. It declares no
shared content and no child list.

Widget-local attributes: none. Child elements: none.

## See also

- `../chrome/menu-button.md`
- `../custom/custom-widget.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
