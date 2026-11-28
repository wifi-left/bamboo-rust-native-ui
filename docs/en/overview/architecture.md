# Architecture

> How the crates, the runtime layers and the two layout consumption modes fit
> together.

## Crate map

The repository is a Cargo workspace with five members.

| Crate | Package | Role |
|---|---|---|
| `crates/native-ui` | `bamboo-native-ui` | The framework: widgets, the `Ui<M>` container, the painter and backend traits, and the Windows host (`WindowBuilder`, `WindowHandle`). |
| `crates/layout-core` | `bamboo-native-ui-layout-core` | The platform-neutral `.buixml` / `.buitheme` model, value types, canonical writer and generation hash. The XML reader is behind its `parse` feature. |
| `crates/layout-codegen` | `bamboo-native-ui-layout-codegen` | Library plus CLI that turns a `.buixml` document into an ordinary Rust module (a `CompiledLayout` static and an `install` function). |
| `crates/layout-macros` | `bamboo-native-ui-layout-macros` | The `layout_xml!` proc macro: expands the codegen output at compile time, with no separate generation step. |
| `crates/showcase` | `bamboo-native-ui-showcase` | The all-controls demo binary `showcase`; the reference wiring for the full API. |

The dependency direction is one-way:
`native-ui` → `layout-core`; `layout-codegen` → `layout-core`;
`layout-macros` → `layout-codegen`. `layout-core` links no UI code, so it can be
used from a build script or a tool.

## Runtime layers

```
  WM_* messages, WM_PAINT
            │
  ┌─────────▼──────────────────────────┐
  │ WindowBuilder / WindowHandle       │  platform/window.rs  (Windows only)
  │ message loop, DPI, timers,         │  normalizes input, owns one back
  │ dirty-rect merge, back-buffer blit │  buffer and the native surfaces
  └───────┬──────────────────▲─────────┘
          │ Event,           │ paint(&mut PaintCtx)
          │ WindowHandle     │
  ┌───────▼──────────────────┴─────────┐
  │ Host  (your application)           │  host.rs
  │ computes rectangles, fills the page│  implements paint/event/resize
  └───────┬──────────────────▲─────────┘
          │ ui.pointer_*(),  │ ui.paint(ctx)
          │ ui.key(), ...    │
  ┌───────▼──────────────────┴─────────┐
  │ Ui<M>                              │  ui.rs
  │ Box<dyn Widget<M>>, hit-testing,   │  hover/press/focus, scroll views,
  │ damage merge, message queue        │  drag and drop, a11y tree
  └───────┬──────────────────▲─────────┘
          │ Widget::input    │ Widget::paint(ctx, state)
  ┌───────▼──────────────────┴─────────┐
  │ Painter + dyn Canvas               │  painter.rs, backend/mod.rs
  │ platform-neutral draw calls        │  text, fills, rounded rects, images
  └───────┬──────────────────▲─────────┘
          │                  │
  ┌───────▼──────────────────┴─────────┐
  │ GDI / Direct2D backend             │  #[cfg(windows)]  backend/gdi.rs,
  │ cached brushes, fonts, masks       │  backend/d2d.rs
  └────────────────────────────────────┘
```

The host boundary is Windows-only: `crate::host`, `crate::platform` and the
`windows` crate dependency are compiled behind `#[cfg(windows)]`. Everything
above `Painter`/`Canvas` compiles on any platform, which is what lets the
widget tests run without a window.

## Message flow

1. The window procedure receives a `WM_*` message and the host converts it into
   a normalized `Event` (`PointerMove`, `PointerDown`, `Wheel`, `Key`,
   `Char`, `PointerKind`, ...). The application never sees raw messages.
2. `Host::event(&event, &mut WindowHandle)` is called. The application forwards
   pointer/keyboard events to `Ui` methods such as `pointer_move`,
   `pointer_down_with`, `pointer_up_with`, `wheel_xy`, `key` and `char`.
3. `Ui` performs hit-testing, updates hover/press/focus state, and calls the
   target widget's `Widget::input`. A widget returns `Option<M>`, the
   application message type.
4. Messages collect in a queue. The application drains them with
   `ui.drain_messages()` and updates its own state.
5. A programmatic change made through a `Ui` setter (`set_label`, `set_progress`,
   `edit_widget`, ...) records the affected rectangle as pending damage instead
   of repainting immediately. The application calls `ui.flush_damage(window)`
   once at the end of the callback, so a burst of setters costs one repaint
   request.
6. The host merges the invalidated rectangles (up to 32 separate pieces) and
   invalidates the client. `WM_PAINT` then runs one paint pass.

Lifecycle callbacks (`resize`, `dpi_changed`, `started`, `loaded`,
`close_requested`) run on the same thread, between paint passes. `resize` is the
only place layout may be recomputed.

## Paint flow

1. The host merges accumulated damage into a list of separate dirty rectangles
   (DIPs) and, past a threshold, collapses it to one bounding box.
2. It builds a `PaintCtx` with a borrowed dirty list, the client `Size`, the DPI,
   the editing state and any overlay flags.
3. `Host::paint` fills the page background under each dirty piece and calls
   `ui.paint(ctx)`.
4. `Ui::paint` iterates widgets in insertion order, skips any widget whose paint
   bounds intersect no dirty rectangle, and calls `Widget::paint`.
5. Widgets draw through `Painter` and `Canvas` into the back buffer.
6. The host blits each dirty piece from the back buffer to the window. Pixels
   outside the pieces are never touched, so idle widgets cost nothing.

There is no measure pass: a widget's rectangle comes from the application (or
from an applied layout document), never from its content.

## Feature gates

Features are declared in `crates/native-ui/Cargo.toml`.

| Feature | Default | What it enables | Extra dependencies |
|---|---|---|---|
| `gdi` | yes | The Windows GDI backend (and the Direct2D renderer options). The public API is the same when it is off, but a running window needs it. | `windows` |
| `layout-xml` | no | Runtime `.buixml` parsing (`LayoutDocument::parse`) and the applier (`Ui::apply_layout`). | `layout-core` with its `parse` feature (`quick-xml`) |
| `layout-compiled` | no | Runtime support for generated Rust: `layout_xml::compiled::CompiledLayout` and `Ui::apply_compiled`. No XML parser is linked. | none |
| `theme-xml` | no | The `.buitheme` reader, mapped onto `Palette` through `ThemeDocumentExt`. | `layout-core` with `parse` |
| `debug-form` | no | Read-only debugging snapshots (`Ui::debug_snapshot`, `DebugSnapshot`) and paint statistics (`PaintStats`). | enables `layout-xml` |

The two layout features are independent and can be enabled together; the
showcase does that so one build type-checks both modes.

## Where the XML layouts fit

`.buixml` is primarily a **skin**: a document assigns rectangles, property
overrides, style tokens and content initial values to widget ids that the Rust
side already created. It cannot remove, reorder or reparent those addressed
widgets. A `type="..."` entry can also *create* a widget through an installed
`WidgetRegistry` (12 standard types; third-party types register from Rust);
created widgets are owned by the document, so a reload removes the ones the new
document no longer declares. XML never binds event handlers.
Two consumption modes share one schema:

- **Dynamic** (`feature = "layout-xml"`). The application reads the file,
  calls `LayoutDocument::parse`, then `ui.apply_layout(&document, &palette,
  size, window)`. This is the hot-reload path used in development.
- **Compiled** (`feature = "layout-compiled"`). `bamboo-native-ui-layout-codegen`
  or the `layout_xml!` macro turns the same file into a Rust module. The
  application calls `ui.apply_compiled(&module::LAYOUT, &palette, size, window)`
  or the generated `install` helper. No parser is linked at runtime.

Both paths run the same internal applier (`Ui::install_prepared`), so the same
document produces the same rectangles, properties and content either way. The
shared model lives in `layout-core`; the applier lives in
`crates/native-ui/src/layout_xml/`.

Theme documents (`.buitheme`, `feature = "theme-xml"`) are a separate reader
that produces a `Palette`; a layout's colour tokens resolve against the palette
passed to the apply call.

## See also

- [Design principles](design-principles.md)
- [Host and window API](../api/host-and-window.md)
- [Theming guide](../guides/theming.md)
- [Quickstart](../quickstart/index.md)
- [Dynamic XML layouts](../quickstart/05-xml-dynamic.md)
