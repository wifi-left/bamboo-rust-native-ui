# Toggle

> An on/off switch with a text label and a pill-shaped track.

## Introduction

`Toggle<M>` is a simple control that pairs a text label with a 44 × 22 DIP
switch track and an 18 DIP knob. The knob sits on the left when off and on the
right when on; the track uses `accent` while on and `button_bg` while off, with
`button_hover` under the pointer.

Clicking it, or pressing Enter/Space while focused, flips its state and reports
`on_change(new_state)`. The widget aliases the MP3-player vocabulary: `Switch<M>`
is a type alias for `Toggle<M>` exported by `bamboo_native_ui::widgets` (and
`widgets::inputs`). `Ui::set_toggle` changes the state silently, without
reporting a message.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{LabelSide, Toggle};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    MuteChanged(bool),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Toggle::new("mute", "Mute", false, Msg::MuteChanged)
        .label_side(LabelSide::End)
        .on_label("开")
        .off_label("关"),
);
ui.set_rect(WidgetId::new("mute"), Rect::from_xywh(16.0, 16.0, 180.0, 36.0));
```

`on_change` is `Fn(bool) -> M` and receives the **new** state, so the message
carries the value. `on_label`/`off_label` are painted inside the track; leave
them empty for a bare switch. To update the state from application code use
`ui.set_toggle(WidgetId::new("mute"), true)`; that emits nothing.

## Usage (XML)

`Toggle` is created by the standard registry (`type="toggle"`) and, like every
value-reporting standard type, requires a `command`. The factory builds a toggle
whose callback clones the resolved message on every change, so the standard set
needs `M: Clone`:

```xml
<widget id="mute" type="toggle" command="102"
        x="16" y="16" w="180" h="36" label="Mute" checked="false"/>
```

The file can set the initial state with `checked` and the label with `label`. The
message does not distinguish "on" from "off"; if the application needs the new
value it must read the state back (or register its own factory with
`WidgetRegistry::register`). Label side and in-track texts are Rust-only.
`Toggle` declares no widget-local attributes.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, label, on, on_change)` | `&'static str, impl Into<String>, bool, impl Fn(bool) -> M` | — | Creates the toggle; `160 × 36` DIP, label at the start, enabled. |
| `.label_side(LabelSide)` | `LabelSide` | `Start` | `Start` = label left of the track; `End` = label right of it. |
| `.on_label(impl Into<String>)` | `String` | `""` | Text painted inside the track while on. |
| `.off_label(impl Into<String>)` | `String` | `""` | Text painted inside the track while off. |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | none | Additive click observer. |
| `.on_focus_gained(handler)` / `.on_focus_lost(handler)` | `Fn(FocusEvent) -> Option<M>` | none | Additive focus observers. |

### Behaviour

- **Track geometry.** The track is 44 × 22 DIP and hugs the side opposite the
  label, 2 DIP from the rectangle edge; the label occupies the rest minus a
  10 DIP gap. The focus ring is a pill that hugs the track and stays inside the
  layout cell.
- **State.** `Toggle` has no `disabled()` builder and no public state getter.
  Disable it with `Ui::set_enabled`; a Rust-built toggle delivers the new state
  in its `on_change` message. A document-created toggle re-reports its fixed
  command, so read the state from `Ui::a11y_nodes` or register a factory that
  carries the value. `Toggle` declares `content_caps = TOGGLED`, so `checked` is
  the document-side initial value.
- **Accessibility.** Role `toggle` with the checked state; the accessible name
  is the label. Enter/Space (`Activate`) behaves exactly like a click.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(bool) -> M` passed to `Toggle::new` | The state flips, through a click or `Enter`/`Space`; the argument is the **new** state. |

`Ui::set_toggle` changes the state without reporting a message.

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

None. The `on_change` closure given to the constructor is the only callback.

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("mute"), Trigger::Click, |event, ctx| {
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

The toggle reacts to `Click` and `Activate` (both flip the state) and to
`HoverEnter`/`HoverLeave` for the track fill; it ignores `Wheel`,
`DragStart`/`DragEnd` and `ValueChange`.

## XML definition

- **Creation:** `type="toggle"` plus `command`. Missing command is
  `xml.command.missing`; the standard factory needs `M: Clone`.
- **Geometry:** one horizontal form (`x`+`w`, `right`+`w`, `x`+`right`) and one
  vertical form (`y`+`h`, `bottom`+`h`, `y`+`bottom`).
- **Content:** `label` and `checked` only.
- **Style:** `radius`, `corners`, `background`, `font-size`, `font-family`. A
  `background` override changes the field fill, not the track state colors.
- **Properties:** the generic runtime set (`enabled`, `visible`, `tooltip`,
  `name`, …).
- **Children:** none.
- **Parent:** optional `parent="panel-id"` for a registered scroll panel.

## See also

- `./button.md`
- `./segmented.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
