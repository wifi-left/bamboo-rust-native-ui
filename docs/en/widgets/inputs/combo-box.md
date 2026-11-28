# ComboBox

> A drop-down list whose open options are painted above its siblings.

## Introduction

`ComboBox<M>` paints a closed box with the selected item and a chevron. Clicking
the box (or pressing Down/Up while it is closed) opens a list below it;
clicking a row selects it, closes the list and emits `on_select(index)`.
Clicking the box while open closes without selecting, and Escape dismisses the
list.

The open list is an overlay painted by the widget itself above its siblings and
hit-tested first. It is capped at `max_visible` rows (8 by default) and scrolls
with the wheel, by following the keyboard highlight, or by dragging its themed
bar. Navigation keys: Down/Up step the highlight (opening first if closed),
Home/End jump, PageUp/PageDown move by a page, Enter/Space select, Escape
dismisses. If the list would leave the viewport and there is room, it flips
above the box automatically; `.drop_up(true)` forces that direction.

Accessibility is `A11yRole::ComboBox` with the selected text as its value and
the `label` as its name (falling back to the value). `M` is the message type
returned by `on_select`.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::ComboBox;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg { Mode(usize) }

fn build(ui: &mut Ui<Msg>) {
    ui.add(
        ComboBox::new("mode", &["In order", "Shuffle", "Repeat one"], 0, Msg::Mode)
            .label("Playback mode")
            .max_visible(6),
    );
    ui.set_rect(WidgetId::new("mode"), Rect::from_xywh(16.0, 16.0, 200.0, 32.0));
}
```

The constructor clamps `selected` into the item list. `selected_text()` returns
the visible value; `set_selected(index)` and `set_items(Vec<String>)` update the
widget without emitting, for model synchronisation.

## Usage (XML)

`ComboBox` is not in the standard registry. Create it in Rust and address it by
`id` from the document. The `<option>` child list replaces the items:

```xml
<widget id="mode" x="16" y="16" w="200" h="32" selected="0">
  <option label="In order"/>
  <option label="Shuffle"/>
  <option label="Repeat one"/>
</widget>
```

`selected` is the only shared content (`content_caps` exposes `SELECTION`) and
is applied silently. Each `<option>` requires a non-empty `label` and accepts
no other attribute or child.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, items, selected, on_select)` | `&'static str, &[&str], usize, impl Fn(usize) -> M + 'static` | — | Creates the box. |
| `.label(impl Into<String>)` | `impl Into<String>` | `""` | Accessible name for the list. |
| `.max_visible(usize)` | `usize` | `8` | Rows shown before the list scrolls (minimum 1). |
| `.row_height(f32)` | `f32` | `26.0` | Option row height in DIPs (minimum 16). |
| `.drop_up(bool)` | `bool` | `false` | Always open the list above the box. |
| `.arrow(closed, open)` | `char, char` | `'▾', '▴'` | Chevron characters for both states. |
| `.arrow_closed(char)` | `char` | `'▾'` | Closed-state character only. |
| `.scrollbar_style(ScrollBarStyle)` | `ScrollBarStyle` | 6 DIP track, 12 DIP minimum thumb | Open-list bar thickness and colours. |
| `.disabled(bool)` | `bool` | `false` | Grey the box and ignore input. |

Getters and setters: `selected()`, `selected_text()`, `items()`, `is_open()`,
`arrow_chars()`, `scrollbar()`, `set_selected(index)` and
`set_items(Vec<String>)` (both silent). Observer hooks (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`) run before the built-in behavior.

Behaviour notes:

- `overlay()` is true while open, and `paint_bounds()` covers the open list so
  invalidation reaches it.
- Only an open list consumes wheel notches; a closed box leaves them to a
  surrounding scroll view.
- A private-use arrow character is drawn with the glyph face; a text character
  with the UI face.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(usize) -> M` passed to `new` (`on_select`) | An option is chosen by click or by Enter/Space on the highlighted row; the argument is the new index. |

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

The box reacts to `Click` (open/close, choose a row) and `Activate` (open, or
select the highlighted row). `Wheel` scrolls only while the list is open;
`DragStart`/`DragEnd` drive the open list's scrollbar. `ValueChange` follows a
selection change.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

- Geometry: normal `widget` rules apply; the rectangle is the closed box only
  (the list is painted outside it).
- Shared content: `selected`.
- Local attributes: none.
- Style tokens: no `SurfaceStyle`; `radius`, `corners` and `background` are
  unsupported (`xml.style.unsupported`, a warning).
- Child elements: `<option>` with a required `label`; the list replaces the
  widget's items. Other names are `xml.skin.unknown`.

## See also

- `../index.md`
- `multi-select-list.md`
- `../controls/button.md`
- `../../guides/xml-layout-authoring.md`
