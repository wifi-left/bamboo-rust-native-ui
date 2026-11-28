# 几何与颜色

> 用于布局、命中测试、损伤跟踪和绘制的平台无关值类型。

## 概述

几何类型位于 `bamboo_native_ui::geometry`（从
`bamboo-native-ui-layout-core` 重导出）；`Point`、`Size`、`Insets`、`Corners` 和 `Rect`
也在 crate 根部重导出。颜色位于
`bamboo_native_ui::color`，并以 `Color` 和 `Rgba` 重导出。

所有几何都以 **DIP**（设备无关像素）为单位：一个 DIP 在 96 DPI 下等于一个
像素，后端在绘制时乘以 `dpi / 96`（`Canvas::scale`）。因此布局、命中测试、`Ui::set_rect`
和控件契约共享同一套坐标系；只有后端才转换为物理像素。

`Rect` 以四条边存储，而不是原点加尺寸，因此平铺时绝不会累积舍入漂移。`contains`
是半开区间：右边缘和下边缘不包含在内，因此相邻单元格共享一条边，不会有某个点同时属于
两者。`merge_damage` 将相距较远的脏矩形保持分开，并合并那些重叠或相接触的矩形。

## 关键类型

### 值类型

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point { pub x: f32, pub y: f32 }
impl Point { pub const fn new(x: f32, y: f32) -> Self; }

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size { pub width: f32, pub height: f32 }
impl Size {
    pub const fn new(width: f32, height: f32) -> Self;
    pub const ZERO: Self = Self::new(0.0, 0.0);
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets { pub left: f32, pub top: f32, pub right: f32, pub bottom: f32 }
impl Insets {
    pub const fn uniform(value: f32) -> Self;
    pub const fn xy(x: f32, y: f32) -> Self;
    pub const ZERO: Self = Self::uniform(0.0);
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners {
    pub top_left: f32, pub top_right: f32, pub bottom_right: f32, pub bottom_left: f32,
}
impl Corners {
    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self;
    pub const fn uniform(radius: f32) -> Self;
    pub const ZERO: Self = Self::uniform(0.0);
    pub fn max(self) -> f32; pub fn clamped(self, width: f32, height: f32) -> Self;
    pub fn to_array(self) -> [f32; 4]; pub fn map(self, f: impl Fn(f32) -> f32) -> Self;
}
```

圆角从左上角开始按顺时针排列。每个半径在绘制时都会被限制到较短边的一半，因此
`Corners::uniform(rect.height() / 2.0)` 就是一个胶囊。`Insets` 向内为正：`Rect::inset_by`
按它们收缩。

### `Rect` 与 `PxRect`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect { pub left: f32, pub top: f32, pub right: f32, pub bottom: f32 }
impl Rect {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self;
    pub const fn from_size(size: Size) -> Self;
    pub const fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self;
    pub fn width(self) -> f32; pub fn height(self) -> f32; pub fn size(self) -> Size;
    pub fn center(self) -> Point; pub fn is_empty(self) -> bool;
    pub fn contains(self, x: f32, y: f32) -> bool;
    pub fn contains_point(self, point: Point) -> bool;
    pub fn intersects(self, other: Rect) -> bool;
    pub fn intersect(self, other: Rect) -> Option<Rect>;
    pub fn union(self, other: Rect) -> Rect;
    pub fn inset(self, dx: f32, dy: f32) -> Rect;
    pub fn inset_by(self, insets: Insets) -> Rect;
    pub fn inflate(self, dx: f32, dy: f32) -> Rect;
    pub fn translate(self, dx: f32, dy: f32) -> Rect;
    pub fn offset_into(self, outer: Rect, offset: Point) -> Rect;
    pub fn to_px(self, scale: f32) -> PxRect;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PxRect { pub left: i32, pub top: i32, pub right: i32, pub bottom: i32 }
impl PxRect {
    pub fn width(self) -> i32; pub fn height(self) -> i32; pub fn is_empty(self) -> bool;
    pub fn intersect(self, other: PxRect) -> Option<PxRect>;
    pub fn union(self, other: PxRect) -> PxRect;
    pub fn area(self) -> i64;
}
```

`from_xywh` 由原点和尺寸构建。`width`/`height` 以零为下限，
`contains` 是半开区间。空矩形永远不相交，`union` 会忽略它们。`inset`
传入负值时向外扩张。`to_px` 向外对齐取整（左/上取 `floor`，右/下取
`ceil`）；它是公共 API 中唯一的 DIP 到像素转换。

### `merge_damage`

```rust
pub fn merge_damage(list: &mut Vec<Rect>, rect: Rect, cap: usize);
```

添加 `rect`，把它和每个与它重叠或相接触的矩形合并（合并后会重新开始扫描，
因为并集现在可能与更早的一块相接触）。空矩形会被忽略。超过 `cap`
后，列表坍缩为一个包围矩形：此时跟踪各块的代价已超过那些额外像素。

### `Color` 与 `Rgba`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color { pub r: u8, pub g: u8, pub b: u8 }
impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self;
    pub const fn rgb(rgb: [u8; 3]) -> Self;
    pub const fn from_hex(hex: u32) -> Self;   // 0xRRGGBB
    pub const fn to_hex(self) -> u32; pub const fn to_rgb(self) -> [u8; 3];
    pub fn luminance(self) -> f32;
    pub fn mix(self, other: Color, t: f32) -> Color;
    pub fn lighten(self, t: f32) -> Color; pub fn darken(self, t: f32) -> Color;
    #[cfg(windows)] pub fn to_colorref(self) -> u32;
    #[cfg(windows)] pub const fn from_colorref(value: u32) -> Self;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba { pub color: Color, pub alpha: u8 }
impl Rgba {
    pub const fn new(color: Color, alpha: u8) -> Self;
    pub const fn opaque(color: Color) -> Self;
    pub fn alpha_unit(self) -> f32;
    pub fn display(self) -> String;   // 不透明时为 #RRGGBB，否则为 #RRGGBBAA
}
```

`Color` 是不透明的 RGB；透明度是单独的 `Rgba` 值，供皮肤协议和会绘制 alpha 的
表面使用。`luminance` 使用 Rec. 601 权重；
`mix`、`lighten` 和 `darken` 将系数限制在 `0..=1`。

## 示例

```rust
use bamboo_native_ui::geometry::merge_damage;
use bamboo_native_ui::{Color, Rect};

fn repaint_two_widgets() {
    let mut damage: Vec<Rect> = Vec::new();
    merge_damage(&mut damage, Rect::from_xywh(16.0, 16.0, 240.0, 24.0), 32);
    merge_damage(&mut damage, Rect::from_xywh(16.0, 60.0, 240.0, 32.0), 32);
    // 与两者都重叠，因此三块坍缩为一块。
    merge_damage(&mut damage, Rect::from_xywh(16.0, 36.0, 240.0, 28.0), 32);
    assert_eq!(damage.len(), 1);

    let accent = Color::from_hex(0x7FD3A8);
    assert!(accent.lighten(0.12).luminance() > accent.luminance());

    // 后端在 150% DPI 下位块传输的内容。
    let physical = damage[0].to_px(1.5);
    assert!(physical.right >= physical.left);
}
```

## 说明与限制

- 所有几何都是 DIP；后端通过 `Canvas::dpi` 和
  `Canvas::scale` 报告比例。值类型是 `Copy` 且没有身份——控件身份是
  `WidgetId`（见[控件 trait](./widget-trait.md)）。
- `Color` 没有 alpha；在需要透明度的地方使用 `Rgba`。
  `to_colorref`/`from_colorref` 仅限 Windows（`#[cfg(windows)]`，BGR 顺序）。
- 超过 `cap` 后被合并的损伤列表会变成一个包围矩形；`Ui` 通过
  `mark_dirty` / `flush_damage` 将其输入（见
  [Ui 容器](./ui-container.md)）。

## 另见

- [Ui 容器](./ui-container.md)
- [绘制与 Canvas](./painting-and-canvas.md)
- [主题与调色板](./theme-and-palette.md)
- [布局辅助函数](./layout-helpers.md)
- [Label](../widgets/content/label.md)
- [架构概览](../overview/architecture.md)
