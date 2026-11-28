# RadioButton

> A standalone radio button that reports when it becomes selected.

## Introduction

`RadioButton<M>` paints a radio dot and a label. It emits `on_select()` only
when it transitions from unselected to selected; clicks and activation on an
already-selected button do nothing. Mutual exclusion is not built in: making
another button in the same group unselected is the application's job, or use
[`RadioGroup`](radio-group.md), which owns a whole option list.

A disabled button uses the disabled palette and ignores input. The focus ring
hugs the dot rather than the whole label. Accessibility is
`A11yRole::RadioItem` with `selected` set and `read_only` when disabled. `M` is
the message type returned by `on_select`.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::RadioButton;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { LocalOnly }

fn build(ui: &mut Ui<Msg>) {
    ui.add(RadioButton::new("local", "This device only", false, || Msg::LocalOnly));
    ui.set_rect(WidgetId::new("local"), Rect::from_xywh(16.0, 16.0, 180.0, 28.0));
}
```

For a real group, prefer `RadioGroup`: it paints the options and moves the
selection for you. If you keep standalone buttons, the application must call
`set_checked(false)` on the previously selected button when it handles the
message.

## Usage (XML)

`RadioButton` is not in the standard registry. Create it in Rust and address it
by `id` from the document:

```xml
<widget id="local" x="16" y="16" w="180" h="28" label="This device only"/>
```

Only `label` is supported as shared content; the radio button's `content_caps`
exposes `LABEL` but not `TOGGLED` or `SELECTION`, so `checked`/`selected` are
not written from the document. There are no local attributes or child
elements.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, label, selected, on_select)` | `&'static str, impl Into<String>, bool, impl Fn() -> M + 'static` | — | Creates the radio button. |
| `.disabled(bool)` | `bool` | `false` | Grey the button and ignore input. |

Getters and setters: `selected()` and `set_checked(bool)` (silent; the group
owner usually clears it). Observer hooks (`on_click`, `on_right_click`,
`on_double_click`, `on_key_down`, `on_key_up`, `on_focus_gained`,
`on_focus_lost`) run before the built-in behavior.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn() -> M` passed to `new` (`on_select`) | The button transitions from unselected to selected; clicks on an already-selected button emit nothing. |

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

The button reacts to `Click` and `Activate` (select when currently
unselected); `ValueChange` follows the selection change. The remaining
triggers are not handled.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- Geometry: normal `widget` rules apply.
- Shared content: `label`.
- Local attributes: none.
- Style tokens: no `SurfaceStyle`; `radius`, `corners` and `background` are
  unsupported (`xml.style.unsupported`, a warning).
- Child elements: none.

## See also

- `../index.md`
- `radio-group.md`
- `checkbox.md`
- `../../guides/xml-layout-authoring.md`
