# 布局文档（`.buixml`）

> 解析后的布局模型——变体、条目、几何、常量、表达式和诊断——以及规范化写入器。

## 概述

`bamboo_native_ui::layout_xml`（特性 `layout-xml`）公开 `.buixml` 文件解析得到的不可变模型，
以及将模型转换回字节稳定文本的规范化写入器。这些类型位于
`bamboo-native-ui-layout-core` 中，该 crate 无条件编译它们；读取文件还需在其上启用
`feature = "layout-xml"`（或 `layout-core/parse`）。

默认情况下，文档*寻址* Rust 侧已构建的 id，为其分配矩形、运行时属性、样式令牌、内容初始值
以及控件声明的皮肤数据；它不会创建、移除、重排或重新设置这些被寻址控件的父级。条目可通过
`type="…"` 选择加入创建；参见
[widget-registry-and-creation.md](widget-registry-and-creation.md)。

解析是纯函数，也是全函数：`LayoutDocument::parse` 要么返回整个模型，要么返回诊断列表。应用是
一个单独的步骤（`Ui::apply_layout`，特性 `layout-xml`），这正是热重载具备原子性的原因——
无法解析的文件永远不会被应用一半。同一模型服务于动态路径（`layout-xml`）和两个编译路径
（`layout-compiled`）；本页介绍该模型，[layout-codegen.md](layout-codegen.md) 介绍编译后的
消费者。

## 关键类型

### `LayoutDocument`

```rust
pub struct LayoutDocument {
    pub generation: u64, pub canonical: String,
    pub constants: LayoutConstants, pub variants: Vec<LayoutVariant>,
}

impl LayoutDocument {
    pub fn parse(text: &str) -> Result<Self, Vec<LayoutDiagnostic>>; // 特性 "layout-xml"
    pub fn generation(&self) -> u64;
    pub fn variants(&self) -> &[LayoutVariant];
    pub fn constants(&self) -> &LayoutConstants;
    pub fn select(&self, client: Size) -> &LayoutVariant;
    pub fn to_xml(&self) -> String;
}
```

- `generation` 是 `canonical` 的 FNV-1a 哈希；内容与皮肤数据仅在其变化时才写入。`to_xml`
  返回 `canonical` 本身。
- `select` 选择在 `client` DIP 下适用的变体：在所有 `(min_width, min_height)` 都适配的变体
  中，选择最大的那一对；否则回退到 `(0,0)` 基础变体。解析器要求恰好有一个基础变体。

### `LayoutVariant`、`LayoutEntry`、`ScrollEntry`

```rust
pub struct LayoutVariant {
    pub id: String, pub min_width: f32, pub min_height: f32,
    pub entries: Vec<LayoutEntry>, pub scrolls: Vec<ScrollEntry>,
    pub rust_bindings: Vec<RustBinding>,
}

impl LayoutVariant {
    pub fn id(&self) -> &str;
    pub fn minimums(&self) -> (f32, f32);
    pub fn entries(&self) -> &[LayoutEntry];
    pub fn scrolls(&self) -> &[ScrollEntry];
    pub fn entry(&self, id: WidgetId) -> Option<&LayoutEntry>;
    pub fn rust_bindings(&self) -> &[RustBinding];
}

pub struct LayoutEntry {
    pub id: WidgetId, pub line: usize, pub column: usize, pub path: String,
    pub type_name: Option<String>, pub command: Option<u32>, pub parent: Option<WidgetId>,
    pub geometry: Geometry, pub props: PropAttrs, pub style: StyleAttrs,
    pub content: Vec<ContentAttr>,
    pub skin_attrs: Vec<(String, String)>, pub skin_children: Vec<SkinElement>,
    pub rust_attrs: Vec<(&'static str, String)>,
}

impl LayoutEntry {
    pub fn resolve(&self, space: LayoutSpace<'_>) -> Result<Rect, LayoutDiagnostic>;
    pub fn type_name(&self) -> Option<&str>;
    pub fn command(&self) -> Option<u32>;
    pub fn parent(&self) -> Option<WidgetId>;
    pub fn has_skin(&self) -> bool;
    pub fn has_rust(&self) -> bool;
}
```

`ScrollEntry` 是一个已注册的滚动画布：`id`、几何，以及来自 `content-height` 属性的
`ContentHeight`（`Keep`、`Exact(Expr)` 或 `Auto`）。`RustBinding` 是一个
`<rust target="id" code="…"/>` 子项列表绑定；编译模式会求值它，动态模式会拒绝该文档。

### `LayoutConstants`、`LayoutSpace`、`EntrySpace`

```rust
pub struct LayoutConstants { /* private */ }

impl LayoutConstants {
    pub fn get(&self, name: &str) -> Option<f64>;
    pub fn iter(&self) -> impl Iterator<Item = (&str, f64)> + '_;
    pub fn insert(&mut self, name: String, value: f64, source: String) -> bool;
}

pub struct LayoutSpace<'a> { /* private */ }

impl<'a> LayoutSpace<'a> {
    pub fn client(size: Size, constants: &'a LayoutConstants) -> Self;
    pub fn content(width: f32, height: f32, constants: &'a LayoutConstants) -> Self;
}

pub enum EntrySpace {
    Client,
    Content(WidgetId),
}
```

`LayoutSpace` 是一个宽度、一个高度和文档的常量。`EntrySpace::Content(id)` 表示该 id 命名
一个滚动画布，其内容区域拥有该矩形；常量按声明顺序解析。

### 几何与属性视图

```rust
pub struct Geometry { pub horizontal: Horizontal, pub vertical: Vertical }

pub enum Horizontal { LeftWidth { x: Expr, w: Expr }, RightWidth { right: Expr, w: Expr },
                     LeftRight { x: Expr, right: Expr } }

pub enum Vertical { TopHeight { y: Expr, h: Expr }, BottomHeight { bottom: Expr, h: Expr },
                   TopBottom { y: Expr, bottom: Expr } }

pub struct PropAttrs { /* visible, enabled, interactive, focusable, focus_order,
                          read_only, tooltip, name, description, automation_id,
                          role, expose, cursor — 均为 Option<…> */ }

pub struct StyleAttrs { /* radius, corners, background, font_size, font_family */ }

pub enum RadiusAttr { Dips(f32), Small, Medium, Large, Pill }
pub enum BackgroundAttr { Token(&'static str), Literal(Color), None }

pub enum ContentAttr { Text(String), Label(String), Value(f64), Progress(f64),
                       Number(i64), Checked(bool), Selected(u32) }
```

`RadiusAttr::resolve(height)` 将 `Small`/`Medium`/`Large` 映射到共享圆角，将 `Pill` 映射到
`height / 2`。`BackgroundAttr::Token` 是 `PALETTE_TOKENS` 中的一个调色板令牌名（`accent`、
`card`、`panel`、`text` 等）。

矩形规则恰好是一个水平形式加一个垂直形式：

| 水平 | 结果 | 垂直 | 结果 |
|---|---|---|---|
| `x` + `w` | 左边缘 + 宽度 | `y` + `h` | 上边缘 + 高度 |
| `right` + `w` | `x = cw - right - w` | `bottom` + `h` | `y = ch - bottom - h` |
| `x` + `right` | 拉伸 `w = cw - x - right` | `y` + `bottom` | 拉伸 `h = ch - y - bottom` |

三者俱全（`x`+`right`+`w`）为 `xml.geom.overdetermined`；既无尺寸又无两条边为
`xml.geom.underdetermined`。`w`/`h` 必须 `>= 0`，`right`/`bottom` 必须 `>= 0`
（`xml.geom.negative`）。`x`/`y` 可以为负。

## 表达式

每个几何属性以及 `content-height` 都是一个表达式，解析一次，并在每次应用时针对
`LayoutSpace` 求值。

```rust
pub enum Expr {
    Num(f64), Var(Var), Const(String), Percent(f64), Rust(String),
    Neg(Box<Expr>), Bin(BinOp, Box<Expr>, Box<Expr>),
}

pub enum Var { Left, Top, Right, Bottom, Cw, Ch, Cx, Cy }

pub fn parse_expr(text: &str, constants: &LayoutConstants) -> Result<Expr, ExprError>;
pub fn eval(expr: &Expr, width: f64, height: f64, axis: Axis,
            constants: &LayoutConstants) -> Result<f64, ExprError>;
```

```text
expr   := term (('+' | '-') term)*
term   := factor (('*' | '/') factor)*
factor := number | number '%' | variable | '(' expr ')' | '-' factor
```

- 变量：`cw`/`right` = 空间宽度，`ch`/`bottom` = 空间高度，`left`/`top` = 0，
  `cx` = 宽度 / 2，`cy` = 高度 / 2。任何其他标识符都是常量（未知时为
  `xml.const.undefined`）。`N%` 是表达式所在轴空间的 `N/100`：`x`、`w`、`right` 使用 `cw`；
  `y`、`h`、`bottom` 使用 `ch`。
- 最大嵌套深度为 32（`xml.expr.depth`）；除以零（`xml.expr.div-zero`）和非有限结果
  （`xml.expr.non-finite`）是应用时错误。
- 常量可以引用较早的常量，但不能引用空间变量或 `%`（`xml.const.vars`）；名称匹配
  `[a-z][a-z0-9_]*`。`rust:` 前缀在模型中被保留：动态模式以 `xml.rust.unsupported` 拒绝
  该文档，编译生成器则将其拼接进去（参见
  [layout-codegen.md](layout-codegen.md)）。

在 1000×700 的空间里，`x="16" w="cw-32"` 得到 `x = 16, w = 968`；
`x="cx-60" w="120"` 让 120 宽的控件水平居中；`x="25%+8" w="50%-16"`
得到 `x = 258, w = 484`；`y="48" bottom="16"` 把高度拉伸到 636。完整的
写法表见 [XML 元素参考](../reference/xml-elements.md)。

## 示例

```rust
use bamboo_native_ui::layout_xml::{LayoutDocument, LayoutSpace};
use bamboo_native_ui::{Rect, Size, WidgetId};

const DOC: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <const name="pad" value="16"/>
  <layout id="wide" min-width="1000" min-height="700">
    <widget id="title" x="pad" y="12" w="cw-2*pad" h="30" font-size="18" label="Now playing"/>
  </layout>
  <layout id="base">
    <widget id="title" x="pad" y="12" w="cw-2*pad" h="24" font-size="16" label="Now playing"/>
  </layout>
</layouts>"##;

fn main() {
    let document = LayoutDocument::parse(DOC).expect("valid document");
    let client = Size::new(1280.0, 800.0);
    let variant = document.select(client);
    assert_eq!(variant.id(), "wide");

    let space = LayoutSpace::client(client, document.constants());
    let entry = variant.entry(WidgetId::new("title")).expect("title entry");
    let _rect: Rect = entry.resolve(space).expect("resolves");
}
```
应用文档需要活动的容器；参见
[05-xml-dynamic.md](../quickstart/05-xml-dynamic.md)。

## 说明与限制

- 特性门控：`LayoutDocument::parse` 和 `to_xml` 需要 `feature = "layout-xml"`（它会启用
  `layout-core/parse`）。模型类型本身无需该特性即可编译；`to_xml` 是规范化的，因此重新
  格式化表达式永远不会改变模型或 generation 哈希。
- 硬性限制：每次解析的源代码为 4 MiB、50 000 个条目和 100 条诊断。文本节点必须仅为空白；
  会拒绝 `<!DOCTYPE>`。错误会拒绝整个文档；警告会被报告，其余部分照常应用。
- 内容（`text`、`label`、`value`、`progress`、`number`、`checked`、`selected`）以及控件
  本地的皮肤数据是 generation 作用域的：调整尺寸永远不会重写它们，重载则会；变体必须就
  同一 id 的内容保持一致（`xml.content.conflict`）。这里不做任何绘制或测量——每个条目
  自行确定其矩形。

## 另见

- [layout-codegen.md](layout-codegen.md) — 编译模式、`layout_xml!`、一致性
- [widget-registry-and-creation.md](widget-registry-and-creation.md) — `type`、`command`、`parent`
- [diagnostics-and-debug.md](diagnostics-and-debug.md) — 报告与代码
- [XML layout authoring](../guides/xml-layout-authoring.md)、[05-xml-dynamic.md](../quickstart/05-xml-dynamic.md)
