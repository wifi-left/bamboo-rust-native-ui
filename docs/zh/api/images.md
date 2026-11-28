# 图像

> 由应用持有的 RGBA 像素，控件通过 `ImageId` 引用它们。

## 概述

后端绘制 RGBA 图像时会按每个 `ImageId` 维护缓存，但控件从不持有像素：像素由应用持有，
控件只知道一个 id。页面通过 `Ui::set_image` 注册其持有的像素；控件则通过
`WidgetPaint::image` 请求它们。存储对每张图像只保留一个 `Arc`，因此一张被网格、头像和
正在播放卡片共享的封面只需一份副本。

像素按行顺序为 RGBA，恰好为 `width * height * 4` 字节。渐变和纯色填充让示例和测试
无需资源管线即可临时代替解码后的美术资源。

该模块跨平台。`ImageId`、`UiImage`、`ImageStore` 以及 `Ui` 的方法在所有目标上都可用；
实际的栅格化由平台后端负责。

## 关键类型

### `UiImage`

```rust
pub struct UiImage {
    pub id: ImageId,
    pub width: u32,
    pub height: u32,
    pub rgba: std::sync::Arc<[u8]>,
}
```

与页面关联、由应用持有的 RGBA 像素。

| 方法 | 签名 | 描述 |
|---|---|---|
| `new` | `new(id, width, height, rgba: impl Into<Arc<[u8]>>) -> Option<Self>` | 校验缓冲区长度；维度为零或不匹配时返回 `None`。 |
| `solid` | `solid(id, width, height, color: Color) -> Self` | 用于临时替代和测试的单色图像。 |
| `gradient` | `gradient(id, width, height, top: Color, bottom: Color) -> Self` | 垂直双色渐变。 |

`new` 会复制任何可字节切片的数据一次；若应用已将其作为 `Arc<[u8]>` 持有，则共享而不复制。

### `ImageId`

```rust
pub struct ImageId(pub u64);
```

后端缓存中的稳定标识。同一个 id 可配合新像素复用（如封面修订版），所有引用它的控件都会
重绘新数据。

### `ImageStore`

```rust
pub struct ImageStore { /* private */ }
```

一个页面上已注册的图像，按 `ImageId` 索引。

| 方法 | 签名 | 描述 |
|---|---|---|
| `new` | `new() -> Self` | 一个空存储。 |
| `insert` | `insert(&mut self, image: UiImage)` | 按 id 插入或替换。 |
| `remove` | `remove(&mut self, id: ImageId) -> Option<UiImage>` | 移除一张图像并返回它。 |
| `get` | `get(&self, id: ImageId) -> Option<&UiImage>` | 借用已注册的图像。 |
| `contains` | `contains(&self, id: ImageId) -> bool` | 该 id 是否已注册。 |
| `len` / `is_empty` | `len() -> usize`, `is_empty() -> bool` | 条目数。 |
| `clear` | `clear(&mut self)` | 丢弃所有图像。 |

### `ImageData`

```rust
pub struct ImageData<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
}
```

后端在未命中时转换并缓存的 RGBA 借用视图。应用通常注册 `UiImage`；`ImageData` 是接触
`Canvas`/`Renderer` 的代码所看到的后端级形式。

### `Ui` 方法

| 方法 | 签名 | 描述 |
|---|---|---|
| `set_image` | `set_image(&mut self, image: UiImage)` | 为页面注册或替换一张图像。 |
| `remove_image` | `remove_image(&mut self, id: ImageId) -> Option<UiImage>` | 移除一张图像。 |
| `image` | `image(&self, id: ImageId) -> Option<&UiImage>` | 借用已注册的图像。 |
| `images` | `images(&self) -> &ImageStore` | 整个存储。 |

## 示例

```rust
use bamboo_native_ui::{Color, ImageId, Ui, UiImage};

fn register_covers(ui: &mut Ui<Msg>) {
    let accent = Color::rgb([0x2E, 0x7D, 0x32]);
    // 解码后的封面会以 Arc<[u8]> 的形式到达；这里用它们代替。
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

展示美术资源的控件会引用这些 id；自定义绘制闭包调用
`WidgetPaint::image(rect, crop_square, id)`，该 id 未注册时返回 `false`，控件可以
改为绘制回退内容。

## 说明与限制

- `set_image` 不会使任何内容失效：请自行重绘展示该图像的控件（`WindowHandle::invalidate` /
  `Ui` 的损伤刷新）。复用 id 可保持现有引用有效。
- 移除图像后，引用它的控件在再次注册前不会绘制任何内容（或只绘制其回退内容）。移除未
  知的 id 会返回 `None`。
- `UiImage::new` 在宽度/高度为零，或缓冲区长度不恰好为 `width * height * 4` 字节时返回
  `None`；`solid` 和 `gradient` 总是成功，并生成正确的长度。
- 图像是页面作用域的：`set_image` 作用于 `Ui`，而非窗口。若页面重建并创建了新的 `Ui`，
  则必须重新注册其图像。
- `WidgetPaint::image` 接受 `crop_square`；对不能拉伸的头像/封面传 `true`，要精确填满
  矩形则传 `false`。

## 另见

- `../widgets/content/avatar.md`
- `../widgets/content/icon.md`
- `../quickstart/03-first-window.md`
- `scrolling.md`
