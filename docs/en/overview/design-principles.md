# Design principles

> The contract the framework is built around, and what it costs compared with
> immediate-mode UI.

## The contract

The rules live in the crate root documentation (`crates/native-ui/src/lib.rs`)
and in the README. They are not guidelines; every module is expected to keep
them.

### The application computes widget rectangles

There is no measure pass and no constraint solver. The application assigns a
rectangle to each widget, normally in `Host::resize`, and the framework never
changes it on its own (a layout document may, when the application applies one).
A widget's preferred height exists only for the optional vertical stack helper;
it is not used to lay out a page.

Consequence: layout is deterministic and cheap. A resize runs the application's
arithmetic once, sets the new rectangles, and invalidates the window.

### The framework paints only dirty rectangles

A state change never triggers a relayout or a full-window repaint. The container
records the changed widget's rectangle as damage, the host merges the damage
into a small set of separate pieces, and only widgets that intersect them are
repainted.

Consequence: a hover on two small controls costs two small repaints, even when
they are far apart on the page. The dirty pieces stay separate up to 32 pieces
before collapsing to a bounding box.

### One persistent back buffer per window

Every window owns a back buffer for its lifetime. Painting draws into that
buffer; after the widgets are drawn, only the dirty pieces are copied to the
window. A widget can therefore paint its whole rectangle even when only a
corner changed: the untouched pixels in the buffer are still correct.

Consequence: no partial widget state to reason about, and no flicker, at the
cost of one full-window-sized surface per window.

### No allocation on the paint path

Dirty rectangles are borrowed (`ctx.dirty`), text is borrowed, and brushes,
fonts, converted images and anti-aliased masks are cached in the backend for the
lifetime of the window. Widget paint code must not allocate; if it needs a
scratch value, it stores it on the widget or in the container.

Consequence: paint cost is stable and does not depend on allocator behaviour.
The rule also means a widget must not build a `String` per frame for text it
draws.

### Idle means zero paints

There is no periodic repaint. The window paints when something invalidates it
(input, a programmatic change, a resize, or an explicit animation timer). An
application that wants an animation installs a timer with
`WindowHandle::set_animation_timer` and invalidates the animated regions from
`Host::timer`.

Consequence: an idle window consumes no CPU for painting. A progress bar that
updates once per second causes one paint per second, not sixty.

### Public types are platform-neutral, the host boundary is Windows-only

`Rect`, `Size`, `Point`, `Color`, `Event`, `KeyCode`, `Palette`, `WidgetId`,
`Canvas` and `Painter` are plain Rust types. The GDI/Direct2D backend and the
window host live behind `#[cfg(windows)]`; `Host`, `PaintCtx`, `WindowBuilder`
and `WindowHandle` are exported only on Windows. Only the platform layer touches
the `windows` crate.

Consequence: widget code and tests compile without a window, and a future
backend can be added without changing a call site.

## The division of responsibility

| Question | Answered by |
|---|---|
| Where is the widget? | Application (`ui.set_rect`) or an applied layout document |
| What does it look like? | Widget (`Widget::paint`) |
| Is the pointer over it, is it pressed, does it have focus? | `Ui` (state passed as `WidgetState`) |
| Which widgets are dirty? | `Ui` records damage; the host merges it |
| When is the window repainted? | Host, from `WM_PAINT` after an invalidation |
| What does a click mean? | Widget (`Widget::input` returns `Option<M>`) |

A widget never invalidates the window directly: it returns a message from
`input`, and the application decides what changed and which rectangle to
damage. The application calls `ui.flush_damage(window)` once per callback so all
changes in one event become one repaint request.

## Trade-offs versus immediate mode

Immediate-mode libraries (egui and friends) rebuild the UI description every
frame. Bamboo Native UI keeps widgets alive and repaints changes. The costs and
benefits are the mirror image of each other.

| Concern | Immediate mode | Bamboo Native UI |
|---|---|---|
| Idle cost | One full relayout and repaint per frame | Zero paints until something invalidates |
| Cost of a small change | Whole window | Dirty pieces only |
| Layout | Flexible, with a measure pass | Application computes rectangles; no measure pass |
| State | Often captured in frame closures | Lives on retained widgets and application structs |
| Animation | Free, because every frame redraws | Explicit: install a timer and invalidate |
| Widget identity | Reconstructed each frame | Stable `WidgetId`, which focus, a11y and IME rely on |
| Writing a widget | Draw code plus layout hints | `paint` plus `input`, with the container owning interaction state |
| Resize | Automatic relayout | Application must recompute rectangles in `resize` |
| Partial redraw correctness | Not applicable | A widget must paint only inside its rectangle |

The main discipline the retained model asks for is invalidation: after changing
state outside a widget's `input` path, the application must record the damage
(usually by calling a `Ui` setter and then `flush_damage`). The main thing it
gives back is that nothing happens when nothing changes, which is the common
case for a desktop application.

## Consequences for widget authors

- Paint only inside `rect`. The container blits the widget's dirty rectangle and
  nothing else, so drawing outside it is either clipped or never copied.
- Do not allocate in `paint`. Reuse cached data on the widget or borrow from
  `WidgetPaint` (`text`, `images`, `palette`).
- Do not depend on pixels outside your rectangle being current. Another widget
  may have repainted its own area independently.
- Return messages from `input`; do not mutate other widgets. The application
  decides what a message means and which rectangles to damage.
- Read interaction state from `WidgetState` (`hovered`, `pressed`, `focused`,
  `enabled`, `editing`, `caret_on`) instead of tracking it yourself.

## See also

- [Architecture](architecture.md)
- [Introduction](introduction.md)
- [The widget trait](../api/widget-trait.md)
- [Custom widgets quickstart](../quickstart/04-custom-widgets.md)
