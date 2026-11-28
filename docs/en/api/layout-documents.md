# Layout documents (`.buixml`)

> The parsed layout model — variants, entries, geometry, constants, expressions and diagnostics — plus the canonical writer.

## Overview

`bamboo_native_ui::layout_xml` (feature `layout-xml`) exposes the immutable
model that a `.buixml` file parses into, and the canonical writer that turns a
model back into byte-stable text. The types live in
`bamboo-native-ui-layout-core`, which compiles them unconditionally; reading a
file needs `feature = "layout-xml"` (or `layout-core/parse`) on top.

By default a document *addresses* ids the Rust side already built, assigning
rectangles, runtime properties, style tokens, content initial values and
widget-declared skin data; it does not create, remove, reorder or reparent those
addressed widgets. An entry may opt into creation with `type="…"`; see
[widget-registry-and-creation.md](widget-registry-and-creation.md).

Parsing is pure and total: `LayoutDocument::parse` returns the whole model or a
diagnostic list. Application is a separate step (`Ui::apply_layout`, feature
`layout-xml`), which is what makes a hot reload atomic — a file that does not
parse is never half-applied. The same model serves the dynamic path
(`layout-xml`) and the two compiled paths (`layout-compiled`); this page covers
the model, [layout-codegen.md](layout-codegen.md) the compiled consumers.

## Key types

### `LayoutDocument`

```rust
pub struct LayoutDocument {
    pub generation: u64, pub canonical: String,
    pub constants: LayoutConstants, pub variants: Vec<LayoutVariant>,
}

impl LayoutDocument {
    pub fn parse(text: &str) -> Result<Self, Vec<LayoutDiagnostic>>; // feature "layout-xml"
    pub fn generation(&self) -> u64;
    pub fn variants(&self) -> &[LayoutVariant];
    pub fn constants(&self) -> &LayoutConstants;
    pub fn select(&self, client: Size) -> &LayoutVariant;
    pub fn to_xml(&self) -> String;
}
```

- `generation` is the FNV-1a hash of `canonical`; content and skin data are
  written only when it changes. `to_xml` returns `canonical` itself.
- `select` picks the variant that applies at `client` DIPs: among all variants
  whose `(min_width, min_height)` both fit, the greatest pair; the `(0,0)` base
  variant otherwise. The parser requires exactly one base variant.

### `LayoutVariant`, `LayoutEntry`, `ScrollEntry`

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

`ScrollEntry` is a registered scroll panel: `id`, geometry, and a
`ContentHeight` (`Keep`, `Exact(Expr)` or `Auto`) from the `content-height`
attribute. `RustBinding` is a `<rust target="id" code="…"/>` child-list
binding; compiled mode evaluates it, dynamic mode rejects the document.

### `LayoutConstants`, `LayoutSpace`, `EntrySpace`

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

A `LayoutSpace` is a width, a height and the document's constants.
`EntrySpace::Content(id)` means the id names a scroll panel whose content area
owns the rectangle; constants resolve in declaration order.

### Geometry and attribute views

```rust
pub struct Geometry { pub horizontal: Horizontal, pub vertical: Vertical }

pub enum Horizontal { LeftWidth { x: Expr, w: Expr }, RightWidth { right: Expr, w: Expr },
                     LeftRight { x: Expr, right: Expr } }

pub enum Vertical { TopHeight { y: Expr, h: Expr }, BottomHeight { bottom: Expr, h: Expr },
                   TopBottom { y: Expr, bottom: Expr } }

pub struct PropAttrs { /* visible, enabled, interactive, focusable, focus_order,
                          read_only, tooltip, name, description, automation_id,
                          role, expose, cursor — all Option<…> */ }

pub struct StyleAttrs { /* radius, corners, background, font_size, font_family */ }

pub enum RadiusAttr { Dips(f32), Small, Medium, Large, Pill }
pub enum BackgroundAttr { Token(&'static str), Literal(Color), None }

pub enum ContentAttr { Text(String), Label(String), Value(f64), Progress(f64),
                       Number(i64), Checked(bool), Selected(u32) }
```

`RadiusAttr::resolve(height)` maps `Small`/`Medium`/`Large` to the shared radii
and `Pill` to `height / 2`. `BackgroundAttr::Token` is one of the palette token
names in `PALETTE_TOKENS` (`accent`, `card`, `panel`, `text`, …).

The rectangle rule is exactly one horizontal form and one vertical form:

| Horizontal | Result | Vertical | Result |
|---|---|---|---|
| `x` + `w` | left edge + width | `y` + `h` | top edge + height |
| `right` + `w` | `x = cw - right - w` | `bottom` + `h` | `y = ch - bottom - h` |
| `x` + `right` | stretched `w = cw - x - right` | `y` + `bottom` | stretched `h = ch - y - bottom` |

All three (`x`+`right`+`w`) is `xml.geom.overdetermined`; neither a size nor both
edges is `xml.geom.underdetermined`. `w`/`h` must be `>= 0` and `right`/`bottom`
must be `>= 0` (`xml.geom.negative`). `x`/`y` may be negative.

## Expressions

Every geometry attribute, and `content-height`, is an expression parsed once and
evaluated against a `LayoutSpace` on each apply.

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

- Variables: `cw`/`right` = space width, `ch`/`bottom` = space height,
  `left`/`top` = 0, `cx` = width / 2, `cy` = height / 2. Any other identifier is
  a constant (`xml.const.undefined` when unknown). `N%` is `N/100` of the
  expression's axis: `x`, `w`, `right` use `cw`; `y`, `h`, `bottom` use `ch`.
- Maximum nesting depth is 32 (`xml.expr.depth`); division by zero
  (`xml.expr.div-zero`) and a non-finite result (`xml.expr.non-finite`) are
  apply-time errors.
- Constants may reference earlier constants, but not space variables or `%`
  (`xml.const.vars`); names match `[a-z][a-z0-9_]*`. A `rust:` prefix is
  preserved in the model: dynamic mode refuses the document with
  `xml.rust.unsupported`, the compiled generator splices it (see
  [layout-codegen.md](layout-codegen.md)).

## Example

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
Applying the document needs the live container; see
[05-xml-dynamic.md](../quickstart/05-xml-dynamic.md).

## Notes and limits

- Feature gate: `LayoutDocument::parse` and `to_xml` require
  `feature = "layout-xml"` (it enables `layout-core/parse`). The model types
  themselves compile without it; `to_xml` is canonical, so reformatting an
  expression never changes the model or the generation hash.
- Hard limits: 4 MiB of source, 50 000 entries and 100 diagnostics per parse.
  Text nodes must be whitespace only; a `<!DOCTYPE>` is rejected. Errors reject
  the whole document; warnings are reported and the rest applies.
- Content (`text`, `label`, `value`, `progress`, `number`, `checked`,
  `selected`) and widget-local skin data are generation-scoped: a resize never
  rewrites them, a reload does; variants must agree on one id's content
  (`xml.content.conflict`). Nothing here paints or measures — every entry
  determines its own rectangle.

## See also

- [layout-codegen.md](layout-codegen.md) — compiled mode, `layout_xml!`, parity
- [widget-registry-and-creation.md](widget-registry-and-creation.md) — `type`, `command`, `parent`
- [diagnostics-and-debug.md](diagnostics-and-debug.md) — reports and codes
- [XML layout authoring](../guides/xml-layout-authoring.md), [05-xml-dynamic.md](../quickstart/05-xml-dynamic.md)
