# ProgressBar

> A determinate or indeterminate progress track with an optional label or percentage.

## Introduction

`ProgressBar` paints a 6 DIP track filling its height center and an accent fill
proportional to `value` (0.0 to 1.0). With `indeterminate()` it instead paints a
moving segment — 30% of the track, at least 24 DIP wide — whose position is the
`phase`. Advance the phase from an application timer with `set_phase`.

The bar is a display widget: it is not focusable, ignores input and reports no
messages. A label or a percentage is painted at the right edge over the track.
It adopts `SurfaceStyle`, so radius, per-corner radii and a background override
apply to the groove and fill.

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::ProgressBar;
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Tick,
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    ProgressBar::new("import", 0.35)
        .show_value(true)
        .corner_radius(3.0),
);
ui.set_rect(WidgetId::new("import"), Rect::from_xywh(16.0, 16.0, 200.0, 28.0));
```

Start an unknown-duration bar with `.indeterminate()` and advance it from a
frame timer, for example with the phase the timer computes:

```rust
ui.edit_widget::<ProgressBar>(WidgetId::new("import"), |bar| bar.set_phase(0.37));
```

`set_phase` wraps with `rem_euclid(1.0)` and returns whether the phase changed,
so the timer can skip a repaint when the value is unchanged.

## Usage (XML)

`ProgressBar` is created by the standard registry (`type="progress"`). It has no
message and needs no `command`:

```xml
<widget id="import" type="progress"
        x="16" y="16" w="200" h="24" progress="0.35"
        background="none"/>
```

The file sets the initial progress and style. The label, the value readout, the
indeterminate mode and the phase are Rust-only; call `Ui::edit_widget` on the
created widget or declare it without `type` and build it in Rust.
`ProgressBar` declares no widget-local attributes.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, value)` | `&'static str, f32` | — | Creates the bar; `200 × 28` DIP, value clamped to `0..=1`, no label. |
| `.label(impl Into<String>)` | `String` | `""` | Text painted at the right edge. |
| `.show_value(bool)` | `bool` | `false` | Paints `"{:.0}%"` at the right edge instead of the label. |
| `.indeterminate()` | — | `false` | Moving segment rather than a proportional fill. |
| `.phase(f32)` | `f32` | `0.0` | Starting phase, wrapped into `0..=1`. |
| `.set_phase(f32)` | `f32` | — | Runtime phase advance; returns whether it changed. |
| `.is_indeterminate()` | — | — | Whether the moving segment is painted. |
| `.value()` | — | — | Current progress in `0..=1`. |
| `.corner_radius(f32)` | `f32` | `0.0` | Uniform radius of groove and fill. |
| `.corners(Corners)` | `Corners` | zero | Per-corner radii. |
| `.background(Color)` | `Color` | theme | Overrides the groove color. |
| `.transparent_background(bool)` | `bool` | `false` | Shows only the fill. |
| `.surface()` | `&SurfaceStyle` | — | The configured surface. |

### Behaviour

- **Value source.** `Ui::set_progress` writes the value and clamps it to
  `0..=1`; it also accepts document `progress` content.
- **Text precedence.** `show_value` wins over `label`. With `indeterminate` the
  percentage is suppressed and the label (if any) is painted instead.
- **Size.** The track is always 6 DIP high, vertically centered, and spans the
  full width; a taller cell only adds empty space around it.
- **Accessibility.** Role `progress-bar`, `display_only`, named from the label or
  the widget id. A determinate bar reports a `0..=1` range plus its percentage;
  an indeterminate bar reports a status text instead of a range. The instance
  joins the accessibility tree only when it opts in (`WidgetProps::expose(true)`).

## Events

### Messages

None. `ProgressBar` reports no messages; write its state with `Ui::set_progress`
or `set_phase`.

### Common observers

This widget does **not** install the shared observer builders. It has no
`Observers` field, is not focusable and its `Widget::input` returns `None`, so
there is no constructor callback or widget-specific handler either. Use
`Ui::bind` on the widget id when a trigger is needed for an application side
effect.

### Widget-specific callbacks

None.

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("import"), Trigger::ValueChange, |event, ctx| {
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

The bar reacts to none of them itself: it is a display widget, so a binding only
observes the event before the (empty) built-in handling.

## XML definition

- **Creation:** `type="progress"`, no `command` needed (the factory ignores one).
  `command` without a `type` is the only schema error.
- **Geometry:** one horizontal and one vertical form; give a height large enough
  for the track and text.
- **Content:** `progress` only (`content_caps = PROGRESS`). `value` is rejected
  even though the Rust type has a `value()` getter.
- **Style:** `radius`, `corners`, `background`, `font-size`, `font-family`.
  `background="none"` keeps only the fill.
- **Properties:** the generic runtime set (`visible`, `tooltip`, `name`, …).
  `enabled` is accepted by the schema but `ProgressBar` has no disabled
  appearance and ignores it.
- **Children:** none.
- **Parent:** optional `parent="panel-id"`.

## See also

- `./slider.md`
- `./pager.md`
- `../index.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
