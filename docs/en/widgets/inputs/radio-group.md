# RadioGroup

> A vertical radio group that paints and manages its own options.

## Introduction

`RadioGroup<M>` is one widget that paints a stack of radio rows and owns the
selection: clicking a row, pressing Space/Enter, or moving with the arrow keys
selects that option and emits `on_select(index)`. Rows are `row_height` DIPs
tall (28 by default) with a `gap` of 4 DIPs.

The group behaves like a native radio group. Up/Left step backwards and
Down/Right step forwards, wrapping around and skipping disabled options; Home
selects option 0 and End selects the last option (each only when that option is
enabled and different from the current one). A disabled option is skipped by
pointer, keyboard and the accessibility tree. Selection changes emit only when
the index actually changes.

Accessibility is `A11yRole::RadioGroup` with one child per option (carrying
`selected`, `enabled` and the row rectangle). `M` is the message type returned
by `on_select`.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::RadioGroup;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Mode(usize) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        RadioGroup::new("mode", &["In order", "Shuffle", "By popularity"], 0, Msg::Mode)
            .disable(2),
    );
    ui.set_rect(WidgetId::new("mode"), Rect::from_xywh(16.0, 16.0, 200.0, 96.0));
}
```

The default rectangle is 200×96. `preferred_height` is
`options.len() * (row_height + gap) - gap`, so a host that sizes rows can use
it instead of hard-coding 96. `Ui::set_selected` moves the selection silently.

## Usage (XML)

`RadioGroup` is not in the standard registry. Create it in Rust and address it
by `id` from the document:

```xml
<widget id="mode" x="16" y="16" w="200" h="96" selected="1"/>
```

`selected` is the only supported shared content (`content_caps` exposes
`SELECTION`) and moves the selection without emitting. The option labels are
data owned by the widget, not XML children; there is no `<option>` list for
this widget.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, options, selected, on_select)` | `&'static str, &[&str], usize, impl Fn(usize) -> M + 'static` | — | Creates the group. |
| `.disable(index)` | `usize` | none disabled | Mark one option disabled (out-of-range is ignored). |

Getters and setters: `selected()`, `option_enabled(index)` and
`set_option_disabled(index, bool)`. Observer hooks (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`) run before the built-in behavior.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(usize) -> M` passed to `new` (`on_select`) | The selection moves to a different enabled option; the argument is the new index. |

### Common observers

This widget installs the shared observer builders. They are additive: the
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

None. The constructor's `on_select` message callback is the only widget
callback.

### Ui triggers

Any widget id can be bound with `Ui::bind(id, Trigger, handler)` (or the
`Ui::on_click` / `Ui::on_activate` shorthands). A handler that returns
`Some(M)` queues a message; the built-in action still runs unless the handler
calls `ctx.run_default()`.

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

A `Click` on a row selects it, and `HoverEnter`/`HoverLeave` highlight a row;
arrow/Home/End keys move the selection but are not triggers. `Activate`
re-selects the current option, so it emits nothing. `ValueChange` follows the
selection change.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- Geometry: normal `widget` rules apply. Give it enough height for every row;
  the group does not clip its rows to the rectangle.
- Shared content: `selected` (a non-negative index; an out-of-range index is
  ignored by the widget).
- Local attributes: none.
- Style tokens: no `SurfaceStyle`; `radius`, `corners` and `background` are
  unsupported (`xml.style.unsupported`, a warning).
- Child elements: none. Row labels and disabled flags are constructor data.

## See also

- `../index.md`
- `radio-button.md`
- `checkbox.md`
- `../../guides/xml-layout-authoring.md`
