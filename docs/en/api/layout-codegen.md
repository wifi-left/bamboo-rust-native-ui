# Layout codegen and compiled layouts

> Turn a `.buixml` document into plain Rust at build time, and install it through the shared applier.

## Overview

The compiled mode is the second of the three ways to consume a layout document.
A generator reads the `.buixml`, folds constants into native arithmetic and
emits an ordinary Rust module: one `static LAYOUT: CompiledLayout`, a
`fn(Size) -> Result<Rect, RectError>` per rectangle, `LazyLock` statics for
properties and style, and `&'static` tables for content and data children.
There is no XML parser and no expression interpreter in the binary.

The runtime half lives in `bamboo_native_ui::layout_xml::compiled` behind
`feature = "layout-compiled"`. `Ui::apply_compiled` resolves the descriptor into
the same internal plan `Ui::apply_layout` builds, then runs the shared install
step — claims, content timing, child/content/attribute order and the damage
flush are identical by construction. `layout_parity` asserts the two paths
produce the same rectangles, properties and content.

| Path | When the file is read | Needs at runtime |
|---|---|---|
| Dynamic | On install and each reload | `layout-xml` (parser + applier) |
| Codegen CLI / library | Once at build time, output checked in | `layout-compiled` |
| `layout_xml!` proc macro | At compile time, expanded in place | `layout-compiled` |

Use the dynamic path while authoring, theming or hot-reloading. Use the compiled
path for release builds and large static pages: no parse, no validation walk and
no model allocation at startup, and `rustc` type-checks every spliced value.

## Key types

### Generator library and CLI

Crate `bamboo-native-ui-layout-codegen` (library + binary):

```rust
pub fn module_name(source_name: &str) -> String;
// `layout-demo.buixml` -> `layout_demo`

pub fn generate(xml: &str, source_name: &str, module: &str)
    -> Result<String, Vec<LayoutDiagnostic>>;

#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    pub module: &'a str,
    pub context: Option<&'a str>,
}

pub fn generate_with(xml: &str, source_name: &str, options: Options<'_>)
    -> Result<String, Vec<LayoutDiagnostic>>;
```

```text
bamboo-native-ui-layout-codegen <input.buixml> [-o out.rs] [--module name] [--context Type]
```

`--module` defaults to `module_name`; `--context Type` selects the context mode
below. On a document error the CLI prints each diagnostic with its `.buixml`
line and column and exits with status 1, writing nothing.

### Generated module shape

The output is ordinary source, wrapped in one module:

```rust
pub mod parity {
    pub static LAYOUT: CompiledLayout; // name, generation, variants, targets

    pub fn install<M: 'static, H: UiHost>(
        ui: &mut Ui<M>,
        sink: &mut H,
        palette: &Palette,
        client: Size,
    ) -> Result<LayoutReport, LayoutFailure>;
}
```

With `context = Type`, `install` takes one more argument
(`context: &Type`). `LAYOUT` names the file, carries the FNV-1a generation and
the full preflight target list, and holds the variants. `install` calls
`Ui::apply_compiled` (or `apply_compiled_with` for a context module).

### Runtime vocabulary

```rust
pub type RectFn = fn(Size) -> Result<Rect, RectError>;

pub enum RectError {
    Negative(&'static str),  // a width/height side went below zero
    NotFinite,               // e.g. a division by zero at this size
    Context,                 // exists only through the generated `install`
}

pub fn rect(x: f32, y: f32, width: f32, height: f32) -> Result<Rect, RectError>;
pub fn edges(left: f64, top: f64, right: f64, bottom: f64) -> Result<Rect, RectError>;

pub struct CompiledLayout {
    pub name: &'static str,
    pub generation: u64,
    pub variants: &'static [CompiledVariant],
    pub targets: &'static [CompiledTarget],
}

pub struct CompiledVariant {
    pub id: &'static str,
    pub min_width: f32,
    pub min_height: f32,
    pub entries: &'static [CompiledEntry],
    pub scrolls: &'static [CompiledScroll],
}

pub enum TargetKind { Widget, Scroll }
pub enum CompiledContent {
    Text(&'static str), Label(&'static str), Value(f64), Progress(f64),
    Number(i64), Checked(bool), Selected(u32),
}
pub enum CompiledHeight { Keep, Exact(fn(Size) -> Option<f32>), Auto }

impl CompiledLayout {
    pub fn select_index(&self, client: Size) -> usize;
    pub fn select(&self, client: Size) -> &CompiledVariant;
}
```

`CompiledEntry` carries the id, source line/path, `rect: RectFn`,
`type_name`/`command`/`parent`, `props`/`style` statics, content, skin
attributes and children (and an optional `rust_children` static). `edges` casts
`f64` to `f32` exactly like the dynamic resolver, so both paths round the same.

### Applying

```rust
impl<M> Ui<M> {
    pub fn apply_compiled(
        &mut self,
        layout: &CompiledLayout,
        palette: &Palette,
        client: Size,
        sink: &mut impl UiHost,
    ) -> Result<LayoutReport, LayoutFailure> where M: 'static;

    pub fn apply_compiled_with(
        &mut self,
        layout: &CompiledLayout,
        values: &CompiledValues<'_>,
        palette: &Palette,
        client: Size,
        sink: &mut impl UiHost,
    ) -> Result<LayoutReport, LayoutFailure> where M: 'static;
}
```

Both require `feature = "layout-compiled"` (the `compiled` module is only
compiled then). `apply_compiled` passes empty `CompiledValues`. Every target id
is validated before the first mutation; a failure returns `LayoutFailure` and
changes nothing.

### `layout_xml!`

```rust
use bamboo_native_ui_layout_macros::layout_xml;

layout_xml!("layouts/main.buixml");                        // mod main
layout_xml!("layouts/main.buixml", module = app_layout);   // mod app_layout
layout_xml!("layouts/main.buixml", context = AppLayout);   // install(.., &AppLayout)
```

The macro reads the path relative to `CARGO_MANIFEST_DIR` (forward slashes; no
escapes), runs the same generator and expands the module in place. A document
error becomes a `compile_error!` naming the `.buixml` line and column. The
macro crate itself has no feature flag; the module it emits needs
`feature = "layout-compiled"` in the depending crate.

## Example

```rust
// Build step: cargo run -p bamboo-native-ui-layout-codegen -- \
//   layouts/main.buixml -o src/main_layout.rs --module main_layout
include!("main_layout.rs");

use bamboo_native_ui::{Host, Size, WindowHandle};

impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let palette = *window.palette();
        if let Err(failure) = self
            .ui
            .apply_compiled(&main_layout::LAYOUT, &palette, size, window)
        {
            eprintln!("layout rejected: {failure}");
            return;
        }
        window.invalidate_all();
    }
}
```

`install` does the same call, but takes `(ui, sink, palette, client)` — note the
argument order differs from `apply_compiled`.

## Parity and inline Rust

- Both paths share `Ui::install_prepared`, so a page can switch from dynamic to
  compiled without editing the XML. `cargo test -p bamboo-native-ui --features
  layout-xml,layout-compiled layout_parity` asserts it.
- `rust:` values are compiled-mode only: geometry code is emitted inside the
  generated rectangle function, where `cw`, `ch`, `cx`, `cy` exist as `f64`
  locals; property/style/content code goes into the entry's `LazyLock` (or a
  per-apply value with a context). The module starts with `use super::*;`, so
  the including module's constants and types resolve by name.
- `context = Type` binds `context` in every inline-Rust value and makes the
  generated `install` take `&Type`. Context rectangles report
  `RectError::Context` from the static `RectFn`, so apply such a module through
  `install` / `apply_compiled_with`:
  `CompiledValues { variants: Vec<Vec<CompiledEntryValues<'_>>> }`.
- `<rust target="id" code="expr"/>` replaces one widget's child list; the
  expression must produce `Vec<SkinElement>`. See the `binding.buixml` and
  `context.buixml` fixtures.
- The dynamic applier rejects any of these with `xml.rust.unsupported`; a
  runtime `.buixml` stays portable data.

## Notes and limits

- Feature gates: generated output needs `feature = "layout-compiled"` to
  compile; the parser is not linked. `layout-xml` is independent — a build may
  have either, both, or neither.
- Generated code is checked in (the fixtures' `*_generated.rs` are golden
  tests), so a stale module fails the codegen test suite. Regenerate rather than
  edit by hand.
- The proc macro cannot register a file dependency on stable Rust: editing the
  `.buixml` does not by itself trigger a rebuild. Use the CLI or a build script
  when the document changes independently.
- Inline Rust makes the file source code: `rustc` checks types at the include
  site. Do not load such a file from an untrusted location.
- Binary size grows with generated code; that is the trade-off for the startup
  win. No allocation happens in the paint path.

## See also

- [layout-documents.md](layout-documents.md) — the model both paths share
- [widget-registry-and-creation.md](widget-registry-and-creation.md) — `type` entries in compiled documents
- [diagnostics-and-debug.md](diagnostics-and-debug.md) — `LayoutReport` and `LayoutFailure`
- [06-xml-compiled.md](../quickstart/06-xml-compiled.md) — the end-to-end tutorial
- [XML layout authoring](../guides/xml-layout-authoring.md) — document grammar
