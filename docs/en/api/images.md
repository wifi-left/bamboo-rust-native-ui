# Images

> Application-owned RGBA pixels that widgets reference by `ImageId`.

## Overview

The backend draws RGBA images with a per-`ImageId` cache, but a widget never
holds pixels: the application owns them and the widget only knows an id. A page
registers owned pixels with `Ui::set_image`; widgets ask for them through
`WidgetPaint::image`. The store keeps one `Arc` per image, so a cover shared by
a grid, an avatar and a now-playing card costs a single copy.

Pixels are RGBA in row order, exactly `width * height * 4` bytes. Gradients and
solid fills let demos and tests stand in for decoded artwork without an asset
pipeline.

This module is cross-platform. `ImageId`, `UiImage`, `ImageStore` and the `Ui`
methods are available on every target; the actual rasterization is the
platform backend's job.

## Key types

### `UiImage`

```rust
pub struct UiImage {
    pub id: ImageId,
    pub width: u32,
    pub height: u32,
    pub rgba: std::sync::Arc<[u8]>,
}
```

Owned RGBA pixels registered with a page.

| Method | Signature | Description |
|---|---|---|
| `new` | `new(id, width, height, rgba: impl Into<Arc<[u8]>>) -> Option<Self>` | Validates the buffer length; `None` on a zero dimension or a mismatch. |
| `solid` | `solid(id, width, height, color: Color) -> Self` | A one-color image for stand-ins and tests. |
| `gradient` | `gradient(id, width, height, top: Color, bottom: Color) -> Self` | A vertical two-color gradient. |

`new` copies anything that is byte-sliceable once; a buffer the application
already owns as `Arc<[u8]>` is shared without copying.

### `ImageId`

```rust
pub struct ImageId(pub u64);
```

A stable identity in the backend cache. The same id can be reused with new
pixels (a cover revision) and every widget that references it repaints the new
data.

### `ImageStore`

```rust
pub struct ImageStore { /* private */ }
```

The images registered on one page, keyed by `ImageId`.

| Method | Signature | Description |
|---|---|---|
| `new` | `new() -> Self` | An empty store. |
| `insert` | `insert(&mut self, image: UiImage)` | Insert or replace by id. |
| `remove` | `remove(&mut self, id: ImageId) -> Option<UiImage>` | Drop an image and return it. |
| `get` | `get(&self, id: ImageId) -> Option<&UiImage>` | Borrow a registered image. |
| `contains` | `contains(&self, id: ImageId) -> bool` | Whether the id is registered. |
| `len` / `is_empty` | `len() -> usize`, `is_empty() -> bool` | Entry count. |
| `clear` | `clear(&mut self)` | Drop every image. |

### `ImageData`

```rust
pub struct ImageData<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
}
```

A borrowed RGBA view the backend converts and caches on a miss. Applications
normally register a `UiImage`; `ImageData` is the backend-level form that code
touching `Canvas`/`Renderer` sees.

### `Ui` methods

| Method | Signature | Description |
|---|---|---|
| `set_image` | `set_image(&mut self, image: UiImage)` | Register or replace an image for the page. |
| `remove_image` | `remove_image(&mut self, id: ImageId) -> Option<UiImage>` | Drop an image. |
| `image` | `image(&self, id: ImageId) -> Option<&UiImage>` | Borrow a registered image. |
| `images` | `images(&self) -> &ImageStore` | The whole store. |

## Example

```rust
use bamboo_native_ui::{Color, ImageId, Ui, UiImage};

fn register_covers(ui: &mut Ui<Msg>) {
    let accent = Color::rgb([0x2E, 0x7D, 0x32]);
    // A decoded cover would arrive as an Arc<[u8]>; these stand in for it.
    ui.set_image(UiImage::gradient(
        ImageId(1),
        160,
        160,
        accent.darken(0.45),
        accent.lighten(0.10),
    ));
    ui.set_image(UiImage::solid(ImageId(2), 64, 64, Color::from_hex(0x20_20_20)));
}
```

Widgets that show artwork reference those ids; a custom paint closure calls
`WidgetPaint::image(rect, crop_square, id)`, which returns `false` when the id
is not registered so the widget can draw its fallback instead.

## Notes and limits

- Nothing is invalidated by `set_image`: repaint the widgets that show the
  image yourself (`WindowHandle::invalidate` / `Ui` damage flush). Reusing an id
  keeps existing references valid.
- Removing an image makes referencing widgets paint nothing (or their
  fallback) until it is registered again. Removing an unknown id returns
  `None`.
- `UiImage::new` returns `None` for a zero width/height or a buffer that is not
  exactly `width * height * 4` bytes; `solid` and `gradient` always succeed and
  produce the right length.
- Images are page-scoped: `set_image` touches `Ui`, not the window. A page
  rebuild that creates a new `Ui` must register its images again.
- `WidgetPaint::image` takes `crop_square`; pass `true` for avatars/covers that
  must not stretch, `false` to fill the rectangle exactly.

## See also

- `../widgets/content/avatar.md`
- `../widgets/content/icon.md`
- `../quickstart/03-first-window.md`
- `scrolling.md`
