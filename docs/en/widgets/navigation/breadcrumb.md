# Breadcrumb

> A clickable path where every crumb except the current page navigates.

## Introduction

`Breadcrumb<M>` paints a horizontal trail of labels separated by a chevron.
Earlier crumbs are links: clicking one, or pressing Enter on the keyboard
cursor, reports its index through `on_navigate`. The last crumb is the current
page: it is painted in the normal text color and never navigates.

Labels are measured while painting and cached, so hit-testing and painting
share one layout. The widget is display-sized: it paints inside its rectangle
and does not scroll or wrap a trail that is too long.

## Usage (Rust)

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};
use bamboo_native_ui::widgets::Breadcrumb;

enum Msg {
    Navigate(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Breadcrumb::new("location", &["Home", "Library", "Nocturne"], Msg::Navigate)
        .separator('/')
        .label("Location"),
);
ui.set_rect(WidgetId::new("location"), Rect::from_xywh(16.0, 16.0, 260.0, 24.0));
```

`new` treats the last label as the current page and parks the cursor there.
`separator` accepts one character; a private-use codepoint is drawn with the
glyph face and any other character with the UI face. `set_items` replaces the
trail silently and moves the cursor to the new last crumb.

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. `<step>` children replace the trail; the
last step is the current page:

```xml
<widget id="location" x="16" y="16" w="260" h="24" separator="/" name="Location">
  <step label="Home"/>
  <step label="Library"/>
  <step label="Nocturne"/>
</widget>
```

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, items, on_navigate)` | `&'static str, &[&str], impl Fn(usize) -> M` | — | Creates the trail; the last item is current. |
| `.separator(char)` | `char` | MDL2 `U+E76C` | Character painted between crumbs. |
| `.label(impl Into<String>)` | text | widget id | Accessible name of the trail. |
| `.disabled(bool)` | `bool` | `false` | Paints disabled and ignores input. |
| `.items()` | `&[String]` | — | The labels. |
| `.current_index()` | `Option<usize>` | last | Index of the current (non-navigating) crumb. |
| `.set_items(Vec<String>)` | silent | — | Replaces the trail; the cursor moves to the last crumb. |
| `.navigate(index)` | `Option<M>` | — | Navigates through the normal path; the current crumb and an out-of-range index return `None`. |
| `.set_separator(char)` | `bool` | — | Runtime separator swap; invalidates the measured trail. |
| `.separator_char()` | `char` | — | The current separator. |

Behaviour: only crumbs before the last are navigation targets; hovering one
draws an accent underline and a hand cursor, while the current crumb uses the
arrow cursor. In adjust mode Left/Right move the cursor (wrapping), Home/End
jump to the first/last crumb, and Enter/Space navigate. The preferred height is
24 DIPs. The accessibility role is `List` with the current crumb selected.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(usize) -> M` passed to `new` as `on_navigate` | An earlier crumb is clicked, or Enter/Space navigates the cursor; the argument is the crumb index. The current (last) crumb never navigates. |

`set_items` and `selected` move the cursor silently and emit nothing.

### Common observers

This widget does not install the shared observer builders (`on_click`,
`on_right_click`, `on_double_click`, `on_key_down`, `on_key_up`,
`on_focus_gained`, `on_focus_lost`). The constructor's `on_navigate` is its
event seam; anything else goes through `Ui::bind`.

### Widget-specific callbacks

| Method | Handler | Fires when |
|---|---|---|
| `new(id, items, on_navigate)` | `&'static str, &[&str], impl Fn(usize) -> M + 'static` | Stores the navigation callback; it is the only callback the widget declares. |
| `.navigate(index)` | returns `Option<M>` | Runtime form: navigates normally and returns the message, or `None` for the current crumb, an out-of-range index or a disabled trail. |

### Ui triggers

Any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("location"), Trigger::Click, |event, ctx| {
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

It reacts to `Click` and `Activate` (navigation) and `HoverEnter`/`HoverLeave`
(the link underline). `Wheel`, `RightClick`, the drag triggers and `ValueChange`
have no built-in effect — the published selection is the current page, which
only the trail itself changes.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties apply,
including `enabled` and `name`. `font-size` and `font-family` are paint-level.
The trail paints from the palette and has no `SurfaceStyle`, so `radius`,
`corners` and `background` are reported as `xml.style.unsupported` and skipped.

Local attributes:

| Attribute | Kind | Description |
|---|---|---|
| `separator` | text | Exactly one character. |

Child elements:

| Element | Attributes | Description |
|---|---|---|
| `<step>` | `label` (required, non-empty) | One crumb; the trail is replaced. |

The child list is content: written only when the document generation changes,
variant-invariant and silent. The shared `selected` attribute is accepted and
moves the keyboard cursor; the painted current page is still the last step.

## See also

- `nav-rail.md`
- `accordion.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
