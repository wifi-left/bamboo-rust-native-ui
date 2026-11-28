# ColorPicker

> A color field that opens an in-window palette popup with an optional alpha strip.

## Introduction

`ColorPicker<M>` is a composite picker: the field shows a small swatch over a
checkerboard (so translucency is visible), the color as `#RRGGBB` or
`#RRGGBBAA`, and a chevron. Clicking the field, or pressing Enter, opens a popup
with the built-in 8 × 6 palette. Selecting a swatch reports the color and closes
the popup. Escape closes without changing the value.

`allow_alpha` adds an eight-step transparency strip under the palette; picking a
step reports the new `Rgba` and keeps the popup open so the user can tune
translucency against the same palette. `on_more` adds a "more colors…" row that
closes the popup and reports a handoff message, where the application opens its
own full color dialog. The popup paints as an overlay, flips above the field
when there is no room below, and owns the mouse wheel while open.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::ColorPicker;
use bamboo_native_ui::{Color, Rect, Rgba, Ui, WidgetId};

enum Msg {
    Accent(Rgba),
    OpenColorDialog,
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    ColorPicker::new_rgba("accent", Rgba::opaque(Color::rgb([25, 113, 194])), Msg::Accent)
        .allow_alpha(true)
        .on_more(|| Msg::OpenColorDialog),
);
ui.set_rect(WidgetId::new("accent"), Rect::from_xywh(16.0, 16.0, 190.0, 34.0));
```

`new` reports `Fn(Color) -> M`; `new_rgba` reports `Fn(Rgba) -> M` from the start.
`on_change_rgba` switches a `new`-built picker to the `Rgba` callback. The
callback fires when a palette swatch or an alpha step changes the value; picking
the color that is already current closes the popup and reports nothing.

## Usage (XML)

`ColorPicker` is not created by the standard registry. Create it in Rust and
address it by `id`; the document can seed the color and the open state:

```xml
<widget id="accent" x="16" y="16" w="190" h="34"
        color="#1971C280" open="false"/>
```

`color` uses the shared color grammar and keeps its alpha, so `#rrggbbaa` seeds a
translucent default (an opaque value becomes alpha 255). `open="true"` opens the
popup when the document generation is installed. Whether the *user* may choose
an alpha is the Rust-only `allow_alpha` flag; a value the application set keeps
its alpha either way. There are no child elements.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, color, on_change)` | `&'static str, Color, impl Fn(Color) -> M` | — | Creates an opaque picker; `190 × 34` DIP. |
| `new_rgba(id, rgba, on_change)` | `&'static str, Rgba, impl Fn(Rgba) -> M` | — | Creates a picker reporting color + alpha. |
| `.enabled(bool)` / `.disabled()` | `bool` | `true` | Enable state; disabling closes the popup. |
| `.on_change_rgba(impl Fn(Rgba) -> M)` | closure | none | Switch the report to `Rgba`. |
| `.on_more(impl Fn() -> M)` | closure | none | Adds the "more colors…" row and its handoff message. |
| `.allow_alpha(bool)` | `bool` | `false` | Shows the transparency strip. |
| `.set_allow_alpha(bool)` | `bool` | — | Runtime strip toggle; returns whether it changed. |
| `.allows_alpha()` | — | `false` | Whether the strip is shown. |
| `.color()` / `.value()` | — | — | The current color. |
| `.rgba()` / `.alpha()` | — | — | Current color with alpha, and the alpha byte. |
| `.set_color(Color)` / `.set_value(Color)` | `Color` | — | Replaces the color, keeps the alpha; returns whether it changed. |
| `.set_rgba(Rgba)` | `Rgba` | — | Replaces color and alpha; returns whether anything changed. |
| `.set_alpha(u8)` | `u8` | — | Replaces the alpha; returns whether it changed. |
| `ColorPicker::<M>::palette()` | — | — | The built-in 8 × 6 palette (`&'static [Color]`). |
| `.is_open()` / `.open()` / `.close()` | — | `false` | Popup state; `open`/`close` return whether they changed it. |
| `.corner_radius(f32)` / `.corners(Corners)` | — | zero | Rounds the field and focus ring. |
| `.background(Color)` / `.transparent_background(bool)` | — | theme | Field surface override. |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | none | Additive click observer; right/double-click, key and focus observers exist beside it. |

### Behaviour

- **Field.** A 20 DIP swatch, the hex string (`#RRGGBB` when opaque,
  `#RRGGBBAA` otherwise) and a chevron; the border turns `accent` when focused or
  open. `preferred_height` is 34 DIP.
- **Keyboard.** Enter/Space opens the popup, arrows move the cursor, Home/End
  jump to the row edges, Enter/Space selects, Escape closes and keeps the value.
  Selecting the current color closes without a message.
- **Pointer.** Clicking the field toggles the popup; clicking outside it (through
  the container's dismiss path) closes it. A click on the field while open
  closes without changing anything.
- **Alpha.** The strip has eight fixed steps: `0, 36, 73, 109, 146, 182, 219,
  255`. Turning `allow_alpha` off only removes the user-facing strip. The strip
  and "more" row use the built-in captions `透明度` and `更多颜色…`; they are not
  configurable.
- **Accessibility.** Role `button` with the hex value as its value, expanded
  state, and `read_only` when disabled.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(Color) -> M` passed to `ColorPicker::new` as `on_change` | A palette swatch or alpha step changes the value, and the popup closes with that change. |
| `Fn(Rgba) -> M` passed to `ColorPicker::new_rgba` or `.on_change_rgba` | Same, reporting color plus alpha. |
| `Fn() -> M` passed to `.on_more` | The "more colors…" row is activated; the popup closes and the app opens its own dialog. |

Selecting the color that is already current closes the popup and reports
nothing.

### Common observers

This widget installs the shared observer builders. All are additive: the
built-in behaviour still runs after the handler returns.

| Builder | Handler | Fires when |
|---|---|---|
| `.on_click(f)` | `Fn(Point) -> Option<M>` | Left-button click on the widget. |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | Right-button click, before the context menu. |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | Platform double click. |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | Key press while the widget has focus. |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | Key release while the widget has focus. |
| `.on_focus_gained(f)` | `Fn(FocusEvent) -> Option<M>` | Focus moves onto the widget. |
| `.on_focus_lost(f)` | `Fn(FocusEvent) -> Option<M>` | Focus leaves the widget (after an editor commits). |

### Widget-specific callbacks

| Builder | Handler | Fires when |
|---|---|---|
| `.on_change_rgba(f)` | `Fn(Rgba) -> M` | Replaces the reporter on a `new`-built picker, so changes report `Rgba`. |
| `.on_more(f)` | `Fn() -> M` | Installs the "more colors…" row and its handoff message. |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("accent"), Trigger::Click, |event, ctx| {
    // Return Some(Msg::...) to queue a message; the built-in action still runs
    // unless ctx.run_default() is called explicitly.
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

The picker reacts to `Click` (field and popup slots), `Activate` (opens the
popup), the arrows/Home/End/Enter/Space keys while open, and
`HoverEnter`/`HoverLeave` for slot hover; it ignores `Wheel`,
`DragStart`/`DragEnd` and `ValueChange`.

## XML definition

- **Creation:** no `type`; this widget is addressed only.
- **Geometry:** one horizontal form and one vertical form. The field itself is
  `preferred_height = 34`; the popup is painted outside the rectangle.
- **Content:** none of the shared content names apply; the color is the local
  `color` attribute.
- **Local attributes:** `color` (`Color`, alpha accepted) and `open` (`Bool`).
- **Style:** `radius`, `corners`, `background`, `font-size`, `font-family`.
- **Properties:** the generic runtime set (`enabled`, `visible`, `tooltip`,
  `name`, `focus-order`, …).
- **Children:** none.

## See also

- `../controls/color-swatch.md`
- `./date-picker.md`
- `./time-picker.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
