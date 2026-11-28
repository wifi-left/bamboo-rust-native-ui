# Pager

> A previous/next navigation bar with a centered page label.

## Introduction

`Pager<M>` paints two equal click zones at the leading and trailing ends and a
centered label between them: the current page and the page count. The zones are
at most 80 DIP wide (30% of the widget), fill `button_bg` when usable and
`button_disabled` when not, and carry the built-in captions `上一页` ("previous")
and `下一页` ("next").

The widget reports navigation deltas through `on_nav(i32)` and never changes the
page itself. The application updates its own state and writes the page back with
`Ui::set_selected(id, page)`; the widget clamps it and refreshes the label.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::Pager;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Page(i32),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    Pager::new("pager", 0, 5, Msg::Page)
        .label_with(|page, pages| format!("Page {page} of {pages}")),
);
ui.set_rect(WidgetId::new("pager"), Rect::from_xywh(16.0, 16.0, 360.0, 32.0));
```

`on_nav` receives `-1` for previous and `+1` for next. `label_with` takes
`Fn(usize, usize) -> String` with the page **1-based** and the page count; the
closure replaces the built-in `第 N / M 页` label. Apply the reported delta in
your update loop, for example with
`ui.set_selected(WidgetId::new("pager"), page + 1)`.

## Usage (XML)

`Pager` is not created by the standard registry. Create it in Rust and address
it by `id`; per-page labels come from `<page>` children:

```xml
<widget id="pager" x="16" y="16" w="360" h="32" selected="0">
  <page label="Overview"/>
  <page label="Tracks"/>
  <page label="Credits"/>
</widget>
```

A `<page>` requires a non-empty `label`, accepts no other attributes and no
children. The list installs per-page labels, drops any custom format and makes
the page count the list length; an existing `selected` then selects one of them.
Without `<page>` children, the center label is the built-in Chinese
`第 N / M 页`, so applications with English copy should call `label_with`.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, page, pages, on_nav)` | `&'static str, usize, usize, impl Fn(i32) -> M` | — | Creates the pager; `360 × 32` DIP, `pages` clamped to at least 1. |
| `.label_with(format)` | `impl Fn(usize, usize) -> String` | built-in `第 N / M 页` | Replaces the center label; clears per-page labels. |
| `.set_label_format(format)` | closure | — | Runtime format swap; always returns `true`. |
| `.clear_label_format()` | — | — | Returns to the built-in label; returns whether a format was removed. |
| `.set_page_labels(Vec<String>)` | `Vec<String>` | — | Installs per-page labels, drops the format, sets `pages = len`; returns whether it changed. |
| `.page_labels()` | — | — | The installed per-page labels. |
| `.label_text()` | — | — | The label currently painted. |
| `.page()` / `.pages()` | — | — | Current page (0-based) and page count. |
| `.set_pages(usize)` | `usize` | — | Changes the count, clamps the page, truncates labels; emits nothing. |

### Behaviour

- **Click bounds.** Previous reports only when `page > 0`; next only when
  `page + 1 < pages`. `Activate` (Enter/Space) always reports `+1` without a
  bounds check, so the application must ignore it on the last page.
- **State.** `page` changes only through `Widget::set_selected` (and therefore
  `Ui::set_selected` or document `selected`). `set_pages` and
  `set_page_labels` clamp it silently.
- **Captions.** The zone captions are fixed at `上一页`/`下一页`; only the center
  label is replaceable. Painting uses `Small` text, `muted` for the label.
- **Accessibility.** Role `button`, named from the label (or the id when the
  label is empty), with the value `"current/total"`.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(i32) -> M` passed to `Pager::new` as `on_nav` | A zone is clicked: `-1` for previous (only when `page > 0`) or `+1` for next (only when `page + 1 < pages`). `Enter`/`Space` always reports `+1`. |

### Common observers

This widget does **not** install the shared observer builders. It has no
`Observers` field and no focus/key handling of its own; navigation is the
constructor callback above. Bind `Ui::bind` when an application-side hook is
needed.

### Widget-specific callbacks

None beyond `on_nav`. The widget never changes the page itself; apply the delta
and write the page back with `Ui::set_selected`.

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("pager"), Trigger::Click, |event, ctx| {
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

The pager reacts to `Click` inside its two zones and to `Activate`; it ignores
`Wheel`, `DragStart`/`DragEnd` and `ValueChange`.

## XML definition

- **Creation:** no `type`; this widget is addressed only.
- **Geometry:** one horizontal form (`x`+`w`, `right`+`w`, `x`+`right`) and one
  vertical form (`y`+`h`, `bottom`+`h`, `y`+`bottom`). The zones need roughly
  80 DIP each plus room for the label.
- **Content:** `selected` (the 0-based page). `label`/`text`/`value` do not
  apply; the painted text is the current page label.
- **Style:** `font-size`, `font-family`. Pager has no surface, so `radius`,
  `corners` and `background` are `xml.style.unsupported`.
- **Properties:** the generic runtime set (`visible`, `tooltip`, `name`, …).
  `enabled` is accepted but `Pager` has no disabled seam, so it has no effect;
  hide the pager or ignore its messages instead.
- **Children:** `<page label="…"/>` elements only, as shown above.

## See also

- `./tabs.md`
- `./segmented.md`
- `./progress-bar.md`
- `../index.md`
- `../../guides/xml-layout-authoring.md`
