# Performance

> Keep idle paints at zero, repaint only what changed, and understand the
> back buffer and the backend caches.

## The performance contract

Bamboo Native UI is a retained-mode framework. A state change does not
relayout and does not repaint the window; it records damage. The host merges
the damage into a small set of rectangles, paints only widgets that intersect
them into a persistent back buffer, and blits only those pieces to the screen.

| Rule | Consequence |
|---|---|
| The application computes rectangles | No measure pass; layout cost is your arithmetic on resize. |
| Only dirty rectangles paint | A hover on two far-apart controls costs two small repaints. |
| One back buffer per window | A widget may paint its whole rectangle; untouched pixels stay correct. |
| No allocation on the paint path | Paint cost is stable and independent of the allocator. |
| Idle means zero paints | No periodic repaint unless you install a timer. |

The full rationale is in [Design principles](../overview/design-principles.md).

## Steps

### 1. Let setters record damage, then flush once

Every `Ui` setter (`set_label`, `set_progress`, `set_value`, `set_visible`,
...) records the affected widget as pending damage. Nothing repaints until the
host invalidates the window. Call `flush_damage` once at the end of the
callback that made the changes:

```rust
use bamboo_native_ui::{Host, WindowHandle};

impl Host for App {
    fn event(
        &mut self,
        event: &bamboo_native_ui::Event,
        window: &mut WindowHandle<'_>,
    ) -> bool {
        let consumed = self.route(event, window);
        self.ui.flush_damage(window);
        consumed
    }
}
```

A burst of setters in one callback becomes one repaint request. Distant widgets
stay separate pieces, so a timer updating a label and a spinner does not repaint
the page between them.

Damage merges rectangles that overlap or touch (with a hair of tolerance) and
keeps up to 32 separate pieces. Past that cap the list collapses to one
bounding box, so a pathological page degrades to a full repaint instead of
unbounded bookkeeping.

### 2. Keep rectangles accurate

`Ui` uses the widget's paint bounds to decide whether a widget intersects a
dirty rectangle. A widget with a stale rectangle leaves pixels behind or
repaints the wrong area. Recompute every rectangle in `Host::resize` (the only
place layout may run) and call `Ui::set_rect`; a rectangle change is the
application's signal that the page moved.

For a vertical flow, the helpers in `bamboo_native_ui::layout` (`VStack`,
`HStack`, `padded`, `split_h`, `split_v`, `grid`) are pure rectangle
arithmetic. Use them on resize, never in `paint`.

### 3. Use an animation timer only while animating

There is no frame loop. An animation installs a timer and invalidates only the
animated region:

```rust
use std::time::Duration;
use bamboo_native_ui::{Host, Rect, WindowHandle};

impl Host for App {
    fn started(&mut self, window: &mut WindowHandle<'_>) {
        window.set_animation_timer(Some(Duration::from_millis(16)));
    }

    fn timer(&mut self, window: &mut WindowHandle<'_>) {
        self.phase = (self.phase + 0.08) % 1.0;
        self.ui.set_progress(bamboo_native_ui::WidgetId::new("meter"), self.phase);
        self.ui.flush_damage(window);
    }
}
```

Stop the timer with `set_animation_timer(None)` when the animation ends. A
window with no timer and no input does not paint at all. A spinner is the
typical exception: it keeps a timer while it is visible and stops when the
application hides it.

### 4. Mark application-owned repaints explicitly

Most changes go through a setter. When the application paints something the
container cannot observe (an application-owned overlay beside a widget, a
composite effect), record the widget or the rectangle yourself:

```rust
use bamboo_native_ui::{Rect, Ui, WidgetId};

fn damage_hud(ui: &mut Ui<()>, window: &mut bamboo_native_ui::WindowHandle<'_>) {
    ui.mark_dirty(WidgetId::new("hud"));
    window.invalidate(Rect::from_xywh(16.0, 16.0, 180.0, 48.0));
    ui.flush_damage(window);
}
```

`Ui::mark_dirty` records the widget's current bounds and returns whether it
recorded anything (hidden and zero-sized widgets record nothing).
`WindowHandle::invalidate` takes one rectangle; `invalidate_all` is for a page
switch, resize or theme change. `invalidate_force` exists for the case where
something covering the window just disappeared and the window is still
considered covered.

### 5. Paint only inside the widget rectangle

The host blits each dirty piece from the back buffer; it does not clip every
widget to its rectangle for you. A widget that draws outside `rect` either gets
clipped at the dirty boundary or leaves stale pixels. A widget must also not
assume pixels outside its rectangle are current: another widget may have
repainted its own area independently.

A widget that paints an expensive full-surface effect (a translucent scrim)
must restrict it to `WidgetPaint::dirty`; repainting the whole surface for a
small invalidation is slow and, for alpha, visibly wrong when pieces overlap.

### 6. Do not allocate in `paint`

Widget paint code must not allocate. Text is borrowed, dirty rectangles are
borrowed, and scratch values belong on the widget or in the container. The
common violations are building a `String` per frame (`format!`,
`to_string()`) to draw text, and collecting rows into a `Vec` during paint.

The painter itself has no per-frame state. The backend caches for the lifetime
of the window:

| Cache | What it holds |
|---|---|
| Brushes | One GDI brush per solid color. |
| Fonts | One font per face/size/weight; dropped on a theme or DPI change. |
| Images | Converted BGRA pixels keyed by `ImageId`, 16 MiB budget. |
| Rounded masks | Anti-aliased corner masks keyed by pixel size, shape and color, 8 MiB budget. |
| Coverage maps | One byte per pixel shared by every color of a geometry, 4 MiB budget. |

`Painter::set_theme` returns whether the text styles changed; the window uses
that to drop cached fonts. `Host::dpi_changed` likewise arrives after the font
cache was dropped. A very large rounded fill (over ~1 million device pixels)
falls back to composed rectangles and ellipses instead of building a mask, so
avoid giant rounded surfaces when an anti-aliased edge matters.

Images are page-scoped and keyed by `ImageId`. Registering the same id again
replaces the pixels; the backend cache reuses the converted image while the id
lives. Do not create a new `ImageId` per frame.

### 7. Cull scroll content

`Ui::paint` clips a scroll view's children to the viewport and skips children
whose bounds do not intersect it, so a 600-row list paints only the visible
rows and one look-ahead. The list widgets also repaint only on a real hover
change: moving inside one row or along an already-hovered scrollbar does not
invalidate the list.

For a page longer than the window, put the content in a `ScrollView` rather
than offsetting every widget by hand; the panel owns clipping, culling, wheel
routing and focus reveal.

### 8. Choose the renderer deliberately

`Renderer::Gdi` is the default and the baseline. `d2d-software` and
`d2d-hardware` draw into the same back buffer through `ID2D1DCRenderTarget`;
hardware pays for a D3D device (tens of megabytes of private memory) and is
only worth it for heavy text/shape pages. A Direct2D mode that cannot start
logs under `BAMBOO_UI_DEBUG` and falls back to GDI permanently.

Select with `WindowBuilder::renderer`, `BAMBOO_UI_RENDERER`, or the showcase's
`--renderer` flag.

## Measuring

```powershell
# Fixed-length run: frames, ticks, invalidations, average paint time,
# private bytes and the active renderer.
cargo run -p bamboo-native-ui-showcase -- --bench 3

# One paint, then exit 0 (also exercises a native edit session).
cargo run -p bamboo-native-ui-showcase -- --self-test

# Per-paint diagnostic: with debug-form, any widget slower than 1.5 ms is
# printed with its id and rectangle; the showcase adds a frame line with
# dirty/widget counts and microseconds.
$env:BAMBOO_UI_PAINT_LOG=1; cargo run -p bamboo-native-ui-showcase

# Frame/invalidation bursts from a covered window are expected: occlusion
# suppresses paints and exposure repaints add invalidations. Compare
# like-for-like runs on an unobstructed desktop.
```

`bamboo_native_ui::diagnostics::private_bytes()` returns the process's private
commit in bytes (`None` off Windows); the showcase prints it after a run.

## Common mistakes

- **Calling `invalidate_all` every timer tick** — that defeats dirty
  rectangles. Invalidate the animated rectangle.
- **Calling `flush_damage` after every setter** — one flush per callback is
  enough; intermediate flushes create extra repaint requests.
- **Forgetting `flush_damage`** — the state changes and the widget looks
  stale until something else invalidates it.
- **Recomputing layout inside `paint`** — layout belongs to `resize` (or a
  page rebuild); paint must be a pure function of the current state.
- **Building text with `format!` in `paint`** — store the formatted string on
  the widget when the value changes.
- **Drawing outside `rect`** — the container blits dirty pieces, not your
  intended shape.
- **Assuming a widget paints under a popup** — overlays paint in a second pass,
  and a covered scroll child is skipped entirely.
- **Using a 60 Hz timer for a slow progress bar** — update once per second and
  stop the timer when nothing animates.

## See also

- [Design principles](../overview/design-principles.md)
- [Architecture](../overview/architecture.md)
- [The Ui container](../api/ui-container.md)
- [Painting and Canvas](../api/painting-and-canvas.md)
- [Scrolling](../api/scrolling.md)
- [Debugging](debugging.md)
- [Feature flags](feature-flags.md)
