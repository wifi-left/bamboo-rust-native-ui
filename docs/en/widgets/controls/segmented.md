# Segmented

> A horizontal strip of mutually exclusive options, like a radio group drawn as buttons.

## Introduction

`Segmented<M>` splits its rectangle into equal cells, frames each cell with the
divider color, and fills the selected cell with `button_active`. Hovering an
enabled cell paints it `row_hover`. Options can be disabled one by one; a
disabled cell paints `button_disabled`, ignores pointer input and is skipped by
the keyboard.

The control behaves like a radio group: clicking a cell selects it and reports
its index through `on_select`. `M` is the application message; the widget owns
the selected index so keyboard navigation and the painting stay coherent.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::Segmented;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Filter(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Segmented::new("filter", &["All", "Recent", "Favorites"], 0, Msg::Filter)
        .disable(2),
);
ui.set_rect(WidgetId::new("filter"), Rect::from_xywh(16.0, 16.0, 300.0, 32.0));
```

`on_select` is `Fn(usize) -> M`; `disable(index)` is the builder form of
`set_option_disabled`. The third argument is the initial selected index. Option
labels are `&'static str` at construction; change them later with
`set_options(Vec<String>)`.

## Usage (XML)

`Segmented` is not created by the standard registry. Create it in Rust and
address it by `id` from the document:

```xml
<widget id="filter" x="16" y="16" w="300" h="32" selected="1">
  <option label="All"/>
  <option label="Recent"/>
  <option label="Favorites"/>
</widget>
```

A `<option>` must have a non-empty `label`, no other attributes and no children;
the list replaces the option labels when the document generation changes.
Per-option disabled flags stay application-owned and are not expressible here.
`selected` applies after the labels.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, options, selected, on_select)` | `&'static str, &[&'static str], usize, impl Fn(usize) -> M` | — | Creates the strip; `300 × 32` DIP, all options enabled. |
| `.disable(index)` | `usize` | — | Disables one option (builder form). |
| `.set_option_disabled(index, disabled)` | `usize, bool` | — | Runtime per-option state; returns whether it changed. |
| `.option_enabled(index)` | `usize` | — | Whether that option can be chosen. |
| `.selected()` | — | — | The selected index. |
| `.options()` | — | — | The option labels. |
| `.set_options(Vec<String>)` | `Vec<String>` | — | Replaces labels; resizes the disabled flags, clamps the selection, clears hover. Returns whether it changed. |
| `.select(usize)` | `usize` | — | Selects through the normal path; `None` for a disabled or out-of-range index. |
| `.hovered()` | — | — | The hovered cell index, if any. |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | none | Additive click observer. |

### Behaviour

- **Keyboard.** Left/Up step back, Right/Down step forward, both with wrap-around
  and skipping disabled options. Home/End jump to the first/last enabled option.
  Enter/Space (`Activate`) re-reports the current index when it is enabled.
- **Pointer.** Clicking a disabled cell reports nothing and keeps the selection;
  hover never lands on a disabled cell.
- **Painting.** Each cell is `width / count` wide, ends 1 DIP short of the next,
  and is framed with the divider color; the selected label uses `accent_text`,
  other labels `muted`, disabled labels `text_disabled`.
- **Accessibility.** Role `radio-group` whose children carry per-option name,
  bounds, selected and enabled state.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(usize) -> M` passed to `Segmented::new` as `on_select` | A cell is clicked, the keyboard moves the selection (arrows, Home/End), or `Enter`/`Space` re-reports the enabled selection. The argument is the new index. |

Disabled options never report, and re-selecting the current index reports
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

None. `on_select` is the only callback; `select(index)` and
`set_option_disabled` are state seams that report through it where applicable.

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("filter"), Trigger::Click, |event, ctx| {
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

The strip reacts to `Click`, `Activate` and the arrow/Home/End keys, and to
`HoverEnter`/`HoverLeave` for the per-cell hover; it ignores `Wheel`,
`DragStart`/`DragEnd` and `ValueChange`.

## XML definition

- **Creation:** no `type`; this widget is addressed only. The document's
  geometry/content passes run against an instance the application already added.
- **Geometry:** one of `x`+`w`, `right`+`w`, `x`+`right`, plus one vertical form;
  a zero size is a warning.
- **Content:** `selected` (non-negative integer). `label`/`text` are rejected.
- **Style:** `font-size` and `font-family` apply. Segmented does not adopt a
  surface, so `radius`, `corners` and `background` are reported as
  `xml.style.unsupported` and skipped.
- **Properties:** the generic runtime set (`enabled`, `visible`, `tooltip`,
  `name`, `focus-order`, …).
- **Children:** `<option label="…"/>` elements only, as shown above.

## See also

- `./tabs.md`
- `./button.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
