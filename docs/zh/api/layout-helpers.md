# 布局辅助函数

> 在 `Host::resize` 内做堆叠、内边距、分割和网格布局的纯矩形运算。

## 概述

`bamboo_native_ui::layout` 包含应用用来
计算控件矩形的若干小型辅助函数。它们是作用于 `Rect` 的纯函数，仅在
窗口尺寸改变或页面重建时运行——绝不在状态变化时运行。因为它们
产生的是普通矩形，绘制和命中测试共享完全相同的
几何。

这些辅助函数不了解控件或树：`VStack` 和 `HStack` 是你手动推进的
游标，`split_h`/`split_v` 按权重
分割一个矩形，`grid` 将其平铺，而 `padded` 应用 `Insets`。结果通过
`Ui::set_rect` 分配（当插入顺序和
`preferred_height` 足够时，也可用 `Ui::layout_vertical`）。

## 关键类型

### `VStack` 与 `HStack`

```rust
pub struct VStack { /* private */ }

impl VStack {
    pub fn new(area: Rect, gap: f32) -> Self;
    /// `height` 高度的下一行；推进游标。
    pub fn next(&mut self, height: f32) -> Rect;
    /// 游标下方的剩余空间。
    pub fn remaining(&self) -> Rect;
    pub fn cursor(&self) -> f32;
}

pub struct HStack { /* private */ }

impl HStack {
    pub fn new(area: Rect, gap: f32) -> Self;
    /// `width` 宽度的下一个单元格；推进游标。
    pub fn next(&mut self, width: f32) -> Rect;
    pub fn remaining(&self) -> Rect;
}
```

`VStack::next` 返回一行，横跨 `area` 的整个宽度，并经过钳制，使其
底部永远不会越过 `area.bottom`；随后游标推进到
`row.bottom + gap`。`remaining` 报告游标下方的空间（不包含间隙）。
`HStack` 是水平方向的镜像：单元格横跨整个高度，
宽度被钳制到 `area.right`。

### `padded`、`split_h`、`split_v`、`grid`

```rust
pub fn padded(rect: Rect, insets: Insets) -> Rect;

/// 按给定权重水平分割为单元格。
pub fn split_h(rect: Rect, weights: &[f32], gap: f32) -> Vec<Rect>;

/// 按给定权重垂直分割为单元格。
pub fn split_v(rect: Rect, weights: &[f32], gap: f32) -> Vec<Rect>;

/// `rect` 内的均匀网格，按行优先返回。
pub fn grid(rect: Rect, columns: usize, rows: usize, gap: f32) -> Vec<Rect>;
```

| 函数 | 行为 |
|---|---|
| `padded` | `rect.inset_by(insets)`；正内边距会收缩矩形。 |
| `split_h` | 减去 `gap * (n - 1)` 后分割宽度；每个单元格得到 `usable * weight / total`。 |
| `split_v` | 对高度做同样的处理。 |
| `grid` | `columns` × `rows` 个带间隙的等大单元格，按行优先；两个数量都钳制到至少为 1。 |

权重是相对的，且不必和为 1。总权重为零或负值时会被
视为 `1.0`，因此所有单元格均等分配，而不会产生 NaN。空的
权重列表返回空向量。

## 示例

```rust
use bamboo_native_ui::layout::{grid, padded, split_h, VStack};
use bamboo_native_ui::{Insets, Rect};

fn layout_page(client: Rect) -> Vec<(&'static str, Rect)> {
    let page = padded(client, Insets::uniform(16.0));

    // 首先是标题行，随后将主体分割为侧边栏和内容列。
    let mut rows = VStack::new(page, 12.0);
    let header = rows.next(40.0);
    let body = rows.remaining();

    let columns = split_h(body, &[1.0, 3.0], 12.0);
    let sidebar = columns[0];
    let content = columns[1];

    // 内容列内的三张等大卡片。
    let cards = grid(content, 3, 1, 12.0);

    vec![
        ("header", header),
        ("sidebar", sidebar),
        ("card-0", cards[0]),
        ("card-1", cards[1]),
        ("card-2", cards[2]),
    ]
}
```

## 说明与限制

- 这些是布局阶段的辅助函数。在绘制或输入路径中调用它们
  会重新计算容器已经拥有的矩形；请改为在 `resize` 或
  页面重建中重新计算。
- `split_h`/`split_v` 在扣除间隙*之后*分配空间，因此在
  100 DIP 的矩形上调用 `split_h(rect, &[1.0, 1.0], 10.0)` 会得到 45 DIP 的单元格，
  且它们之间相隔 10 DIP。
- `grid` 将 `columns` 和 `rows` 钳制到至少为 1，并将单元格尺寸钳制到 `0.0`；当间隙或
  舍入溢出时，它不会将单元格裁剪到 `rect` 内。
- `remaining` 排除末尾的间隙：它从 `cursor - gap` 开始，钳制在
  区域的顶部/左侧。
- `VStack`/`HStack` 不保留控件状态；每次布局阶段都创建一个新的游标。
- 对按插入顺序排列的垂直控件堆叠，`Ui::layout_vertical`
  用每个控件的 `preferred_height` 包装 `VStack`（见
  [Ui 容器](./ui-container.md)）。

## 另见

- [几何与颜色](./geometry-and-color.md)
- [Ui 容器](./ui-container.md)
- [控件 trait](./widget-trait.md)
- [Label](../widgets/content/label.md)
- [第一个窗口快速上手](../quickstart/03-first-window.md)
