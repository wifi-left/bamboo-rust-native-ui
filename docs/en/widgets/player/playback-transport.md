# PlaybackTransport

> The five-cell player transport strip: shuffle, previous, play/pause, next and repeat.

## Introduction

`PlaybackTransport<M>` paints five icon buttons across one rectangle: shuffle,
previous, play/pause, next and repeat. Clicking a cell (or Enter/Space on the
keyboard cursor cell) reports the matching `TransportAction` through the
constructor handler. Play/pause is the primary action and paints as a filled
circle; it shows the pause glyph while `playing` is true.

The widget never decides what should happen. `playing`, `shuffle` and `repeat`
are application state pushed back through `set_playing`, `set_shuffle` and
`set_repeat`, each returning whether anything changed, so a failed playback
command leaves the strip showing the truth.

The keyboard model is a cursor, not a selection: Left/Right/Home/End move the
cursor, Enter/Space runs the cursor cell, and Up/Down leave for the neighbouring
widget. Assistive technology gets a `ToolBar` of five children: three Buttons
and two Toggles (shuffle and repeat).

## Usage (Rust)

```rust
use bamboo_native_ui::widgets::{PlaybackTransport, RepeatMode, TransportAction};
use bamboo_native_ui::{Rect, Ui, WidgetId};

enum Msg {
    Transport(TransportAction),
}

let mut ui: Ui<Msg> = Ui::new();
ui.add(
    PlaybackTransport::new("transport", Msg::Transport)
        .playing(false)
        .shuffle(false)
        .repeat(RepeatMode::Off),
);
ui.set_rect(WidgetId::new("transport"), Rect::from_xywh(120.0, 232.0, 300.0, 44.0));
```

`TransportAction::ALL` is the strip order. `RepeatMode::next` walks
Off → All → One → Off, and `RepeatMode::active` is true for either repeat
kind. A typical handler advances the mode and paints the new state:

```rust
ui.edit_widget::<PlaybackTransport<Msg>>(WidgetId::new("transport"), |strip| {
    if strip.repeat_mode() == RepeatMode::Off {
        strip.set_repeat(RepeatMode::All)
    } else {
        false
    }
});
```

## Usage (XML)

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document. It accepts three widget-local attributes
and no child elements:

```xml
<widget id="transport" x="120" y="232" w="300" h="44"
        playing="false" shuffle="true" repeat="all"/>
```

`repeat` is parsed case-insensitively: `off`/`none`, `all`/`list` and
`one`/`single` are accepted; any other value is rejected as `xml.skin.value`.
The attributes write state silently, exactly like the setters.

## Properties

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, on_action)` | `&'static str, impl Fn(TransportAction) -> M` | — | Creates the strip; `300 × 44` DIP, enabled. |
| `.playing(bool)` | `bool` | `false` | Initial playback state (pause glyph while true). |
| `.shuffle(bool)` | `bool` | `false` | Initial shuffle highlight. |
| `.repeat(RepeatMode)` | `RepeatMode` | `Off` | Initial repeat state. |
| `.set_playing(bool)` | `bool` | — | Application update; returns whether it changed. |
| `.set_shuffle(bool)` | `bool` | — | Application update; returns whether it changed. |
| `.set_repeat(RepeatMode)` | `bool` | — | Application update; returns whether it changed. |
| `.is_playing()` / `.shuffle_enabled()` / `.repeat_mode()` | `bool` / `bool` / `RepeatMode` | — | Read the current state. |
| `.cursor()` / `.set_cursor(usize)` | `usize` / `bool` | `2` | Keyboard cursor cell; the setter clamps and never emits. |

`TransportAction` variants: `Shuffle`, `Previous`, `PlayPause`, `Next`,
`Repeat`. `RepeatMode` variants: `Off` (default), `All`, `One`. Their `label()`
methods return the accessible names used by the widget (the source strings are
Chinese: `随机播放`, `上一曲`, `播放或暂停`, `下一曲`, `循环播放`, and
`关` / `列表` / `单曲` for repeat).

### Behaviour

- **Default rectangle.** `300 × 44` DIP; `preferred_height` is 44 and the
  cursor starts on the middle cell (play/pause).
- **Pointer.** Press moves the cursor to the pressed cell; a release over the
  same cell runs it. Pressing one cell and releasing over another is a drag and
  emits nothing. The strip swallows the click that follows the release so the
  action runs once.
- **Keyboard.** Left/Right/Home/End move the cursor with clamping (no wrap);
  Enter/Space, `Activate` and the accessibility child action run the cursor
  cell. `owns_navigation_key` keeps only Left/Right/Home/End/Enter/Space, so
  Up/Down and Tab move focus away.
- **Disabled.** A disabled strip ignores every input; hover and press state is
  cleared.

## Events

### Messages

| Message | Emitted when |
|---|---|
| `impl Fn(TransportAction) -> M` passed to `new` as `on_action` | A cell runs: a click release over the pressed cell, `Enter`/`Space` on the cursor cell, `Activate`, or a UIA child invoke. |

`TransportAction` is one of `Shuffle`, `Previous`, `PlayPause`, `Next` or
`Repeat`. Moving the cursor (arrows, `Step`, a press) emits nothing; pressing
one cell and releasing over another emits nothing.

### Common observers

This widget installs none of the shared observer builders. Use the constructor
`on_action` handler or `Ui::bind`.

### Widget-specific callbacks

| Callback | Signature | Fires when |
|---|---|---|
| `new(id, on_action)` | `&'static str, impl Fn(TransportAction) -> M` | A cell is run by pointer, keyboard, accessibility or UIA. |

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("transport"), Trigger::Click, |event, ctx| {
    // Return Some(Msg::...) to queue a message; call ctx.default() to keep the
    // built-in action, which then runs after the handler.
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

The strip reacts to `Click`, `HoverEnter`/`HoverLeave`, `Activate` and
`ValueChange` (the shuffle/repeat toggles); it has no wheel, right-click or
double-click behaviour.

## XML definition

This widget is not created by the standard registry. Create it in Rust and
address it by `id` from the document.

Geometry follows the normal `<widget>` rules. Generic properties (`visible`,
`tooltip`, `name`, `description`, `automation-id`, `role`, `expose`, enabled,
focus policy) apply. The strip paints from the palette and has no
`SurfaceStyle`, so `radius`, `corners` and `background` are reported as
`xml.style.unsupported`. It declares no shared content (`text`, `label`,
`value`, `progress`, `selected` are rejected), so `playing` / `shuffle` /
`repeat` are its only local vocabulary.

| Attribute | Kind | Description |
|---|---|---|
| `playing` | bool | Paints the pause glyph and the accent play circle. |
| `shuffle` | bool | Accent highlight and toggle state on the shuffle cell. |
| `repeat` | text | `off`, `all` or `one` (plus the aliases above). |

Child elements: none.

## See also

- `./waveform-seek-bar.md`
- `./playlist-view.md`
- `../chrome/toolbar.md`
- `../index.md`
- `../../api/widget-trait.md`
