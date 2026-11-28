# 06 — Compiled XML

> Compile the `.buixml` document into a Rust module and apply it with
> `feature = "layout-compiled"`. No XML parser is linked.

## What you will build

The same window and the same `layouts/main.buixml` as
[05](05-xml-dynamic.md), but the document becomes a Rust module at build time.
The application calls `ui.apply_compiled`, or the generated `install` helper,
instead of parsing text at runtime. You can keep `layout-xml` enabled beside it;
the two modes share the applier and produce the same result.

## Prerequisites

- The project from [05](05-xml-dynamic.md), including `layouts/main.buixml`.
- Enable the compiled mode in `bamboo-hello/Cargo.toml`:

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-compiled"] }
```

`layout-compiled` adds the runtime types the generated code names
(`layout_xml::compiled::CompiledLayout` and friends). It pulls in **no** XML
parser: `quick-xml` is not compiled or linked in this configuration.

## Steps

### 1. Generate the module with the CLI

The generator lives in the framework workspace. From a checkout of
`native-ui`:

```powershell
cargo run -p bamboo-native-ui-layout-codegen -- `
  layouts/main.buixml -o src/main_layout.rs --module main_layout
```

The command shape is:

```
bamboo-native-ui-layout-codegen <input.buixml> [-o out.rs] [--module name] [--context Type]
```

Expected output:

```
src/main_layout.rs: main.buixml -> mod main_layout
```

If your application is a separate repository, point Cargo at the framework
manifest and give absolute or app-relative paths:

```powershell
cargo run --manifest-path ../native-ui/Cargo.toml `
  -p bamboo-native-ui-layout-codegen -- `
  layouts/main.buixml -o src/main_layout.rs --module main_layout
```

The output is ordinary Rust: a `pub mod main_layout` containing
`pub static LAYOUT: CompiledLayout` and
`pub fn install<M, H>(ui, sink, palette, client)`. Check it in, or generate it
from a build script (the `bamboo-native-ui-layout-codegen` library exposes
`generate` and `generate_with`). A document error makes the CLI exit with
status 1 and print the diagnostic with its `.buixml` line and column; no file is
written. The optional `--context Type` makes the generated `install` take an
extra `&Type` argument that inline Rust values in the document can read.

### 2. Include the generated module and apply it

Add the include at the top of `src/main.rs`. The file already wraps its contents
in a module, so include it rather than declaring `mod main_layout;` (a `mod`
would nest it and produce `main_layout::main_layout`):

```rust
include!("main_layout.rs");

use bamboo_native_ui::widgets::{Button, Label};
use bamboo_native_ui::{Host, Size, Ui, WindowHandle};
```

Replace the `apply_layout` call from page 05:

```rust
impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let palette = *window.palette();
        if let Err(failure) = self
            .ui
            .apply_compiled(&main_layout::LAYOUT, &palette, size, window)
        {
            eprintln!("layout rejected: {failure}");
        }
        window.invalidate_all();
    }
}
```

The generated `install` helper does the same call; note its argument order is
`(ui, sink, palette, client)`, while `apply_compiled` takes
`(layout, palette, client, sink)`:

```rust
main_layout::install(&mut self.ui, window, &palette, size)
```

Both return `Result<LayoutReport, LayoutFailure>` and share the dynamic path's
rules: validate everything before mutating, one damage flush at the end, content
written only when the generation changes, and `ui.clear_layout(window)` to hand
the fields back.

### 3. Alternative: the `layout_xml!` proc macro

The macro expands the same generated module at compile time, with no separate
step. It lives in its own crate; add it next to the framework:

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-compiled"] }
bamboo-native-ui-layout-macros = { path = "../native-ui/crates/layout-macros" }
```

Then, at module scope:

```rust
use bamboo_native_ui_layout_macros::layout_xml;

// Paths are relative to CARGO_MANIFEST_DIR; use forward slashes.
layout_xml!("layouts/main.buixml", module = main_layout);
```

This defines `main_layout::LAYOUT` and `main_layout::install` exactly like the
CLI output. A document error becomes a `compile_error!` naming the `.buixml`
line and column.

**Rebuild caveat.** Stable proc macros cannot register a file dependency, so
editing `layouts/main.buixml` does **not** by itself trigger a rebuild. Use the
CLI (or a build script) when the document changes independently, or touch the
including `.rs` file. `cargo clean -p <your-crate>` is the blunt fallback.

### 4. Choose a consumption path

| | Runtime (`layout-xml`) | CLI / build script (`layout-compiled`) | `layout_xml!` (`layout-compiled`) |
|---|---|---|---|
| When the document is read | runtime, on install and reload | build time, explicit step | compile time, macro expansion |
| XML parser in the binary | yes | no | no |
| Errors | diagnostics; previous layout kept | CLI exits 1 with position | `compile_error!` with position |
| Hot reload | yes | no; rebuild | no; rebuild |
| Artifact to manage | the `.buixml` file | a checked-in or generated `.rs` file | none |
| Best for | development, theming, debug form | release builds, large static pages | small apps, one-step builds |

A page can move between the runtime and compiled modes without editing the XML.

## Expected result

`cargo build` succeeds and the window is identical to page 05: the same label
and button at the same rectangles. The difference is invisible at runtime — no
file is read, no parser is linked, and an invalid document was a build error
rather than a log line. Editing the `.buixml` has no effect until you regenerate
or rebuild.

## Troubleshooting

- **`layout_xml!` cannot read the file** — the path is relative to
  `CARGO_MANIFEST_DIR`, not to the source file, and must use forward slashes.
  `layout_xml!("layouts/main.buixml")` reads
  `<crate>/layouts/main.buixml`.
- **`main_layout::main_layout::LAYOUT` does not exist** — the generated file
  already contains `pub mod main_layout`. Use `include!("main_layout.rs")`, not
  `mod main_layout;`.
- **The generated file is not found by the CLI** — run the command from the
  crate directory, or pass `--manifest-path` for the framework and a correct
  relative input path.
- **An edit to the `.buixml` is ignored** — expected for the macro. Regenerate
  with the CLI, add a build script, or touch the including source file.
- **`apply_compiled` is missing** — the `layout-compiled` feature is off, or you
  generated the module with a framework version that predates M7.6.

## Next

That is the end of the quickstart. Continue with the
[widget reference](../widgets/index.md), the
[host and window API](../api/host-and-window.md), or the
[XML authoring guide](../guides/xml-layout-authoring.md).
