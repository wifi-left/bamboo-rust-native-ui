# CustomWidget

> A closure-painted widget for application-specific components.

## Introduction

`CustomWidget<M>` is the light way to add a component the library does not
ship. It carries a rectangle plus application closures for painting and input,
and it is registered, focused, hit-tested and published to accessibility exactly
like a built-in. Use it for small components that need no owned state; implement
`Widget<M>` directly when the component owns state or needs its own layout
behaviour.

The paint closure receives the paint context, the widget rectangle and the
container state (hover, press, focus, enabled). The container blits the
widget's dirty rectangle, so the closure must draw only inside `rect`. An input
closure receives the rectangle and a `WidgetInput` and returns `Option<M>`;
`.hit_test` replaces the default rectangular hit area for circles or notched
cards. Accessibility is a fixed `A11yInfo` (`.a11y` / `.a11y_name`) or a
dynamic closure (`.a11y_fn`) for a value that changes every frame.

A configured surface (`.corner_radius`, `.corners`, `.background`,
`.transparent_background`) paints first, so the closure usually draws content
only.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::CustomWidget;
use bamboo_native_ui::{
    A11yRole, Color, Cursor, Rect, TextAlign, TextStyleRole, Ui, WidgetId, WidgetInput,
};

enum Msg {
    Clicked,
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    CustomWidget::new("meter", |ctx, rect, state| {
        let inner = rect.inset(4.0, 4.0);
        ctx.painter
            .round_rect(&mut *ctx.canvas, inner, 8.0, ctx.palette.accent);
        ctx.text(
            inner,
            "72%",
            TextStyleRole::Body,
            TextAlign::Center,
            if state.hovered { ctx.palette.text } else { ctx.palette.muted },
        );
    })
    .rect(Rect::from_xywh(16.0, 16.0, 160.0, 40.0))
    .focusable(true)
    .interactive(true)
    .cursor(Cursor::Hand)
    .corner_radius(10.0)
    .background(Color::new(43, 47, 58))
    .a11y_name(A11yRole::Button, "Level meter")
    .input(|_rect, input| match input {
        WidgetInput::ClickAt(_) | WidgetInput::Activate => Some(Msg::Clicked),
        _ => None,
    }),
);
```

`.interactive(true)` is required for pointer hit-testing to land on the widget;
pair it with `.input`. `.focusable(true)` makes it a Tab stop and the container
paints the focus ring. `ui.add_custom(id, rect, paint)` is the paint-only
shorthand.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. It has no built-in shared content or
child list, and it does not declare local skin attributes, so a document can
only move, show/hide and style it.

```xml
<widget id="meter" x="16" y="16" w="160" h="40" radius="10" background="#2B2F3A"/>
```

`.corner_radius`, `.corners` and `.background` reach the layout's
`SurfaceStyle` seam, so the `radius`, `corners` and `background` tokens apply.
A closure widget cannot gain `x-*` attributes or child elements; implement
`Widget<M>` directly for that.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, paint)` | `&'static str, impl Fn(&mut WidgetPaint, Rect, &WidgetState)` | — | Creates the widget; `120 × 32` DIP. |
| `.rect(Rect)` | `Rect` | `120 × 32` at origin | Initial rectangle (layout may move it). |
| `.preferred_height(f32)` | `f32` | `32.0` | Height wanted in a vertical stack. |
| `.focusable(bool)` | `bool` | `false` | Takes keyboard/a11y focus and gets the focus ring. |
| `.interactive(bool)` | `bool` | `false` | Pointer hit-testing may land on the widget. |
| `.enabled(bool)` | `bool` | `true` | Initial enabled state. |
| `.cursor(Cursor)` | `Cursor` | none | Pointer cursor over the widget. |
| `.input(handler)` | `FnMut(Rect, WidgetInput) -> Option<M>` | none | Input handler; `None` stays silent. |
| `.hit_test(handler)` | `Fn(Rect, Point) -> bool` | rectangle | Non-rectangular hit area. |
| `.a11y(A11yInfo)` | `A11yInfo` | none | Fixed accessibility node. |
| `.a11y_name(role, name)` | `A11yRole, impl Into<String>` | none | Shorthand for a fixed role + name. |
| `.a11y_fn(handler)` | `Fn() -> Option<A11yInfo>` | none | Dynamic accessibility node (checked first). |
| `.corner_radius(f32)` / `.corners(Corners)` | radius / corners | zero | Surface rounding. |
| `.background(Color)` / `.background_option(Option<Color>)` | `Color` / `Option<Color>` | theme | Surface fill override. |
| `.transparent_background(bool)` | `bool` | `false` | Keeps the frame, drops the fill. |
| `.surface()` | `&SurfaceStyle` | — | The configured surface. |

### Manual widgets

A component with owned state implements `Widget<M>` directly: supply `id`,
`rect`/`set_rect`, `as_any`/`as_any_mut`, `paint` and `input`, then override
`preferred_height`, `focusable`, `interactive`, `hit_test`, `cursor`, `a11y`
and the focus hooks as needed. Forward `surface_style` / `surface_style_mut`
when the type owns a `SurfaceStyle`, and store an `Observers<M>` field with the
`observer_methods!()` macro (plus `observer_builders!`) for the additive
`on_click` / `on_key_down` / `on_focus_*` builders. The `surface_builders!`
macro adds the same `.corner_radius` / `.background` family.

To accept document data, declare it with the skin protocol:
`skin_attrs` and `skin_children` publish the vocabulary, `validate_skin_attr` /
`validate_skin_children` are side-effect free, and `apply_skin_attr` /
`apply_skin_children` write silently. Names starting with `x-` are the
application's; the shared content names are reserved. The showcase's
`crates/showcase/src/meter.rs` is a complete third-party example.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `impl FnMut(Rect, WidgetInput) -> Option<M>` passed to `.input` | Every `WidgetInput` routed to the widget while `.interactive(true)` (pointer) or `.focusable(true)` (keyboard) lets it land; returning `None` stays silent. |

The closure receives the widget rectangle and the raw input, so it decides
which `WidgetInput` variants matter. `Ui::bind` triggers and the closure both
queue messages; they are independent.

### Common observers

This widget installs none of the shared observer builders. The `.input` closure
is its single input seam; a manual `Widget<M>` implementation stores an
`Observers<M>` field with `observer_methods!` / `observer_builders!` instead.

### Widget-specific callbacks

| Callback | Signature | Fires when |
|---|---|---|
| `.input(handler)` | `FnMut(Rect, WidgetInput) -> Option<M>` | Any input delivered to the widget (hover, press, drag, release, click, double click, wheel, key, `Step`, `Activate`). |
| `.hit_test(handler)` | `Fn(Rect, Point) -> bool` | Before a pointer event: decides whether it lands on the widget (rectangle by default). |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("meter"), Trigger::Click, |event, ctx| {
    // Return Some(Msg::...) to queue a message; call ctx.default() to let the
    // input closure run, which otherwise stays suppressed.
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

The widget reacts to whatever triggers its input closure handles: the binding
handler runs first, and calling `ctx.default()` then passes the same event on to
`.input`. Pointer triggers (including `Wheel`) require `.interactive(true)`;
`Activate` and key input need `.focusable(true)`. The widget publishes no
accessibility value, so `ValueChange` never fires.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties (`visible`,
`tooltip`, `name`, `description`, `automation-id`, `role`, `expose`, enabled,
focus policy) apply. Through `SurfaceStyle`, `radius`, `corners` and
`background` are supported; text drawn with `ctx.text` honors `font-size` and
`font-family`. `CustomWidget` publishes no content capability, so `text`,
`label`, `value`, `progress`, `selected` and friends are rejected as
`xml.content.unsupported` unless the application implements its own widget.

Widget-local attributes: none. Child elements: none. Declare `x-*` locals and
children only on a direct `Widget<M>` implementation.

## See also

- `../../guides/xml-layout-authoring.md#custom-widgets`
- `../../api/widget-trait.md`
- `../overlay/popover.md`
- `../index.md`
- `../../quickstart/04-custom-widgets.md`
