# Drag and drop

> System-wide drag and drop: text and file-list payloads, the effect a drop reports, and the container drag methods.

## Overview

A drag is a **system drag**: it leaves the window and the process (OLE
`DoDragDrop`), so the payload is limited to what the shell can transport —
Unicode text and file lists. An application that needs a richer payload encodes
it into the text (JSON, a URL, an id) instead of a process-local value.

The retained widget tree sees nothing of COM. The host translates `IDropTarget`
callbacks into `Host` methods (`drag_enter` / `drag_over` / `drag_leave` /
`drag_drop`) with client DIP coordinates, exactly like pointer events. The
container routes them to widget-registered sources, targets and hover callbacks.

`DragData` and `DropEffect` are cross-platform models; the OLE transport,
`WindowHandle::start_drag` and the host's system-drag callbacks are Windows-only.
Without OLE the container keeps an in-window fallback drag session.

## Key types

### `DragData`

```rust
pub enum DragData {
    Text(String),
    Files(Vec<std::path::PathBuf>),
}
```

`Text` is `CF_UNICODETEXT` (a field selection, a path, a URL or an encoded
object); `Files` is `CF_HDROP`, as dropped by Explorer. Constructors: `text`,
`files`. Readers: `as_text() -> Option<&str>`, `as_files() -> Option<&[PathBuf]>`,
`is_empty()`. A system drag prefers a file list when an object carries both
(Explorer drags usually do).

### `DropEffect`

```rust
pub enum DropEffect { None, Copy, Move, Link }
```

`None` is the default (no target accepted). A source reads the effect after the
drag loop ends: `Move` tells it to remove the original rows/files. `accepted()`
is `true` for every variant except `None`.

### `Ui` drag methods

| Method | Signature | Description |
|---|---|---|
| `set_drag_source` | `set_drag_source(&mut self, id, source: impl Fn() -> Option<DragData> + 'static) -> &mut Self` | Produce the payload when the gesture leaves this widget. |
| `clear_drag_source` | `clear_drag_source(&mut self, id)` | Remove a source. |
| `set_drop_target` | `set_drop_target(&mut self, id, drop: impl Fn(DragData, Point) -> Option<M> + 'static) -> &mut Self` | Accept any payload. |
| `set_text_drop_target` | `set_text_drop_target(&mut self, id, drop: impl Fn(String, Point) -> Option<M> + 'static) -> &mut Self` | Accept text only. |
| `set_files_drop_target` | `set_files_drop_target(&mut self, id, drop: impl Fn(Vec<PathBuf>, Point) -> Option<M> + 'static) -> &mut Self` | Accept file lists only. |
| `set_drop_hover` | `set_drop_hover(&mut self, id, hover: impl Fn(&DragData, Point, bool) -> bool + 'static) -> &mut Self` | Enter/leave feedback; return `true` when visuals changed. |
| `set_text_drag_source` | `set_text_drag_source(&mut self, id) -> &mut Self` | Drag the field's selected text (D122). |
| `drag_data` / `drag_text` | `drag_data() -> Option<&DragData>`, `drag_text() -> Option<&str>` | Peek at the payload in flight. |
| `drag_source` / `drag_target` | `drag_source() -> Option<WidgetId>`, `drag_target() -> Option<WidgetId>` | Endpoints of the current drag. |
| `cancel_drag` | `cancel_drag(&mut self, sink)` | Abort a drag started in this window. |

### Host callbacks

| Method | Signature |
|---|---|
| `drag_enter` | `fn drag_enter(&mut self, data: &DragData, pos: Point, window) -> DropEffect` |
| `drag_over` | `fn drag_over(&mut self, data: &DragData, pos: Point, window) -> DropEffect` |
| `drag_leave` | `fn drag_leave(&mut self, window)` |
| `drag_drop` | `fn drag_drop(&mut self, data: DragData, pos: Point, window) -> DropEffect` |
| `drag_finished` | `fn drag_finished(&mut self, effect: DropEffect, window)` |
| `files_dropped` | `fn files_dropped(&mut self, paths: Vec<String>, window)` (`WM_DROPFILES`) |

## Example

A text payload dragged between two widgets, with a text drop target:

```rust
use bamboo_native_ui::{DragData, Point, Ui, WidgetId};

fn wire_drag(ui: &mut Ui<Msg>, title: &str) -> &mut Ui<Msg> {
    let source = WidgetId::new("now-playing-title");
    let target = WidgetId::new("playlist");

    let payload = title.to_string();
    ui.set_drag_source(source, move || Some(DragData::text(payload.clone())));
    ui.set_text_drop_target(target, |text, _pos: Point| {
        Some(Msg::AppendToPlaylist(text))
    });
    ui.set_text_drop_hover(target, |_text, _pos, _over| true);
    ui
}
```

Forwarding the host's system-drag callbacks is what makes the same gesture work
from another window or application:

```rust
use bamboo_native_ui::{DragData, DropEffect, Point, WindowHandle};

fn drag_enter(&mut self, data: &DragData, pos: Point, window: &mut WindowHandle<'_>) -> DropEffect {
    self.ui.drag_enter(data, pos, window)
}

fn drag_drop(&mut self, data: DragData, pos: Point, window: &mut WindowHandle<'_>) -> DropEffect {
    self.ui.drag_drop(data, pos, window)
}
```

Beginning a drag from application code (for example a toolbar button):

```rust
use bamboo_native_ui::{DragData, WindowHandle};

fn export(window: &mut WindowHandle<'_>, path: std::path::PathBuf) -> bool {
    window.start_drag(DragData::files([path]))
}
```

## Notes and limits

- System drags are Windows-only (OLE). `DragData` and `DropEffect` compile
  everywhere, and the container keeps an in-window fallback drag session when OLE
  is unavailable (tests, automated runs).
- The payload is text or a file list only; it must survive crossing to another
  process. Encode richer data into the text.
- A painted text field registered with `set_text_drag_source` starts a drag only
  when the press begins inside a non-collapsed selection; password fields and
  touch selection handles never start it.
- A painted field used as a text drop target inserts the text at the release
  point first (a normal user edit, so its `on_change` also reports) unless it is
  read-only.
- `drag_over` is called continuously; only re-damage widgets whose hover state
  actually changed.
- `set_system_drag_enabled(false)` turns OLE drags off (used by automated runs
  whose synthetic pointer has no real button release); `system_drag_enabled()`
  reports the state. `WindowHandle::start_drag` returns `false` when OLE is off.
- `drag_finished` clears the source widget's drag state; OLE consumed the pointer
  release, so no `PointerUp` follows.

## See also

- `scrolling.md` (drop targets inside a scrolled list)
- `hotkeys.md` (keyboard equivalents of drag gestures)
- `../widgets/data/list-view.md`
- `../widgets/content/copyable-text.md`
