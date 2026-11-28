# Tabs

> A tab strip that selects one page at a time, with optional badges and close targets.

## Introduction

`Tabs<M>` paints a row of equal-width tab cells over a 1 DIP baseline. The
selected tab gets an accent underline inset 8 DIP from each side; the hovered
tab gets `row_hover`. Each tab may carry a right-aligned badge (a count or a
short label) and, when `closable` is set, a small `×` close target.

Selection reports the tab index through `on_select`. Closing reports through a
separate `on_close` handler and does **not** select the tab. The widget owns the
selected index and the label/badge lists; badges can be updated at runtime for
live notification counts.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::Tabs;
use bamboo_native_ui::{Rect, TextAlign, Ui, WidgetId};

enum Msg {
    Tab(usize),
    CloseTab(usize),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Tabs::new("pages", &["Now playing", "Queue", "History"], 0, Msg::Tab)
        .badge(1, "12")
        .closable(Msg::CloseTab)
        .label_align(TextAlign::Left),
);
ui.set_rect(WidgetId::new("pages"), Rect::from_xywh(16.0, 16.0, 360.0, 32.0));
```

`on_select` and `on_close` both take `Fn(usize) -> M`. `label_align` takes
`TextAlign` (`Left`, `Center`, `Right`); centered labels stay centered even when
a badge or close target occupies the right edge, while left/right labels stop
before that chrome. The widget does not remove a closed tab — the application
updates its state and calls `set_labels` (or the document does).

## Usage (XML)

`Tabs` is not created by the standard registry. Create it in Rust and address it
by `id`; the document supplies labels, the initial selection and the label
alignment:

```xml
<widget id="pages" x="16" y="16" w="360" h="32"
        selected="1" label-align="left">
  <option label="Now playing"/>
  <option label="Queue"/>
  <option label="History"/>
</widget>
```

`<option>` requires a non-empty `label`, accepts no other attributes and no
children. Badges and `closable` are application-owned: the file speaks labels,
not editor state. `selected` applies after the labels; badges keep their
positions for the tabs that remain.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, labels, selected, on_select)` | `&'static str, &[&'static str], usize, impl Fn(usize) -> M` | — | Creates the strip; `360 × 32` DIP, not closable, no badges, centered labels. |
| `.closable(on_close)` | `impl Fn(usize) -> M` | not closable | Paints a close target on every tab. |
| `.badge(index, text)` | `usize, impl Into<String>` | none | Attaches a badge to one tab. |
| `.set_badge(index, Option<String>)` | `usize, Option<String>` | — | Sets or clears a badge at runtime; returns whether it changed. |
| `.badge_text(index)` | — | — | The badge text, ignoring empty strings. |
| `.is_closable()` | — | — | Whether close targets are painted. |
| `.label_align(TextAlign)` | `TextAlign` | `Center` | Label alignment inside each cell. |
| `.label_alignment()` | — | — | The current alignment. |
| `.set_label_align(TextAlign)` | `TextAlign` | — | Runtime alignment change; returns whether it changed. |
| `.selected()` | — | — | The selected index. |
| `.labels()` | — | — | The tab labels. |
| `.set_labels(Vec<String>)` | `Vec<String>` | — | Replaces labels, resizes badges, clamps the selection; returns whether it changed. |
| `.select(usize)` | `usize` | — | Selects through the normal path; `None` when unchanged or out of range. |
| `.hovered()` | — | — | The hovered tab index, if any. |
| `.on_click(handler)` | `Fn(Point) -> Option<M>` | none | Additive click observer; right/double-click, key and focus observers exist beside it. |

### Behaviour

- **Pointer.** A click selects the tab it lands on and reports `on_select`. A
  click inside the 12 × 12 close target reports `on_close(index)` instead and
  leaves the selection alone.
- **Keyboard.** Left/Up and Right/Down step with wrap-around, Home/End jump to
  the first/last tab, Enter/Space re-reports the selected index, and `Delete`
  closes the selected tab (only when closable).
- **Badges.** Painted as an accent chip at the right edge; the label rectangle
  shrinks to make room. Badge text is application data and can change on every
  frame without rebuilding the widget.
- **Accessibility.** Role `tab-list` with one child per tab, carrying the label,
  bounds and selected state.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(usize) -> M` passed to `Tabs::new` as `on_select` | A tab is clicked, the keyboard moves the selection (arrows, Home/End), or `Enter`/`Space` re-reports the selected tab. |

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
| `.closable(on_close)` | `Fn(usize) -> M` | A click lands on the tab's `×` target, or `Delete` is pressed on the selected tab. The tab is not selected. |

`badge`/`set_badge` change painting only and report nothing.

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("pages"), Trigger::Click, |event, ctx| {
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

The strip reacts to `Click` (including the close target), `Activate`, the
arrow/Home/End keys and `Delete`, plus `HoverEnter`/`HoverLeave` for the per-tab
hover; it ignores `Wheel`, `DragStart`/`DragEnd` and `ValueChange`.

## XML definition

- **Creation:** no `type`; this widget is addressed only. The application must
  have added the `Tabs` instance before the document is applied.
- **Geometry:** one horizontal form (`x`+`w`, `right`+`w`, `x`+`right`) and one
  vertical form (`y`+`h`, `bottom`+`h`, `y`+`bottom`).
- **Content:** `selected`; plus the local `label-align` attribute with the
  values `left`, `center` or `right` (anything else is `xml.skin.value`).
- **Style:** `font-size` and `font-family`. Tabs has no surface, so `radius`,
  `corners` and `background` are `xml.style.unsupported` and skipped.
- **Properties:** the generic runtime set (`enabled`, `visible`, `tooltip`,
  `name`, …). `tooltip-at`-style per-tab tips are set through `WidgetProps` in
  Rust, not in the document.
- **Children:** `<option label="…"/>` elements only.

## See also

- `./segmented.md`
- `./pager.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
