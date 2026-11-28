# 06 — Compiled XML

> Compile the `.buixml` document into a Rust module and apply it with
> `feature = "layout-compiled"`. No XML parser is linked.

## What you will build

The same window and the same `layouts/main.buixml` as
[05](05-xml-dynamic.md), but the document becomes a Rust module at build time.
The application calls `ui.apply_compiled`, or the generated `install` helper,
instead of parsing text at runtime. You can keep `layout-xml` enabled beside it;
the two modes share the applier and produce the same result.

**New terms on this page**

- **Code generation** — a tool reads the `.buixml` file and writes ordinary
  Rust source. You then compile that source like any other module.
- **Proc macro** — a Rust macro that runs during compilation (`layout_xml!` is
  one) and expands into code. It is an alternative to running the generator
  yourself.
- **Build time vs runtime** — build time is while Cargo compiles; runtime is
  while the window is open. Dynamic mode converts the XML at runtime; compiled
  mode converts it at build time.
- **`layout-compiled`** — the Cargo feature that adds the static layout types
  (`CompiledLayout`, `Ui::apply_compiled`). It does not add an XML parser.
- **`CompiledLayout`** — the generated static description of the document:
  variants, rectangles as Rust functions, content values. No text is parsed.
- **Generated module** — `main_layout` here: a `pub mod` holding `LAYOUT` and
  `install`.
- **`include!`** — pastes a `.rs` file into the current module at compile time.
  It is text inclusion, unlike `mod`, which opens a nested module.
- **Rebuild** — running `cargo build`/`cargo run` again. Compiled mode needs a
  rebuild (or a regeneration plus a rebuild) before an XML edit is visible.

## Prerequisites

- The project from [05](05-xml-dynamic.md), including `layouts/main.buixml`.
- Enable the compiled mode in `bamboo-hello/Cargo.toml`:

```toml
[dependencies]
# Compiled mode: generated layouts, no XML parser in the binary.
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-compiled"] }
```

That line belongs in `bamboo-hello/bamboo-hello/Cargo.toml`, the same crate
manifest you edited on page 05. It replaces the page-05 line, so add
`"layout-xml"` beside `"layout-compiled"` if you still want the runtime parser
available in the same binary. Keep the `../native-ui` depth you used on page 02;
for the sibling folders shown there, the path from the crate folder is
`../../native-ui/crates/native-ui`.

`layout-compiled` adds the runtime types the generated code names
(`layout_xml::compiled::CompiledLayout` and friends). It pulls in **no** XML
parser: `quick-xml` is not compiled or linked in this configuration.

Paths in the commands below are relative to the folder you run them in, so each
step names that folder first.

## Steps

### 1. Generate the module with the CLI

The generator lives in the framework workspace. Run the next command from the
root of the `native-ui` checkout — the folder that contains the framework's
workspace `Cargo.toml` — because `-p` selects a package from that workspace.
Both file paths are resolved from the folder you run in.

```powershell
# Run from the native-ui workspace root; both paths are relative to it.
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

The input path must exist relative to the folder you run in. If it does not,
the CLI prints `cannot read layouts/main.buixml: ...` and exits 1 without writing
anything. The framework checkout does not contain this tutorial's
`layouts/main.buixml`, so this form applies when the document has been placed in
that workspace (or when you pass its actual path). This tutorial keeps the
document in the app, so the next form is the one you will normally use. Run it
from the app crate folder `bamboo-hello/bamboo-hello/` (the folder holding
`layouts/` and `src/`) and point Cargo at the framework manifest instead:

```powershell
# Run from the app crate folder; adjust ../native-ui to your framework location.
cargo run --manifest-path ../native-ui/Cargo.toml `
  -p bamboo-native-ui-layout-codegen -- `
  layouts/main.buixml -o src/main_layout.rs --module main_layout
```

`--manifest-path` is also resolved from your current folder. With the sibling
layout from page 02 (`F:\codes\bamboo-hello` next to `F:\codes\native-ui`), the
path from the crate folder is `../../native-ui/Cargo.toml`. Point it at the
framework's workspace `Cargo.toml`, not at the `crates/native-ui` library
manifest.

The output is ordinary Rust: a `pub mod main_layout` containing
`pub static LAYOUT: CompiledLayout` and
`pub fn install<M, H>(ui, sink, palette, client)`. Check it in, or generate it
from a build script (the `bamboo-native-ui-layout-codegen` library exposes
`generate` and `generate_with`). A document error makes the CLI exit with
status 1 and print the diagnostic with its `.buixml` line and column; no file is
written. The optional `--context Type` makes the generated `install` take an
extra `&Type` argument that inline Rust values in the document can read.

**What just happened.** The CLI parsed and validated the document once, then
wrote `src/main_layout.rs` next to `main.rs`. Nothing is parsed when the app
runs: the rectangles are already Rust values. If the command prints a line with
a `.buixml` position and exits 1, no file was written and the previous
generated file (if any) is untouched.

### 2. Include the generated module and apply it

Add the include at the top of `src/main.rs`. The file already wraps its contents
in a module, so include it rather than declaring `mod main_layout;` (a `mod`
would nest it and produce `main_layout::main_layout`):

```rust
// Textual include into this module: the file already declares `mod main_layout`.
include!("main_layout.rs");

use bamboo_native_ui::widgets::{Button, Label};
use bamboo_native_ui::{Host, Size, Ui, WindowHandle};
```

Replace the `apply_layout` call from page 05:

```rust
impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let palette = *window.palette();
        // LAYOUT is static data; this call parses no text.
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
// Same apply, different argument order: (ui, sink, palette, client).
main_layout::install(&mut self.ui, window, &palette, size)
```

Both return `Result<LayoutReport, LayoutFailure>` and share the dynamic path's
rules: validate everything before mutating, one damage flush at the end, content
written only when the generation changes, and `ui.clear_layout(window)` to hand
the fields back.

`include!` pastes the generated `pub mod main_layout { ... }` directly into
`main.rs`, so the type is `main_layout::LAYOUT`. A `mod main_layout;` would
instead look for `main_layout.rs` and wrap its contents in an outer module,
giving `main_layout::main_layout::LAYOUT` — a confusing error after a
successful generation. The include path is resolved relative to `main.rs`, which
is why the CLI writes into `src/`.

**What just happened.** The binary now carries the layout as generated Rust
data. There is no file IO and no parser: `apply_compiled` walks the same
in-memory descriptors that the dynamic path built from XML text, and produces
the same rectangles and the same error codes.

### 3. Alternative: the `layout_xml!` proc macro

The macro expands the same generated module at compile time, with no separate
step. It lives in its own crate; add it next to the framework:

```toml
[dependencies]
# Compiled mode, as above.
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-compiled"] }
# Proc-macro crate that turns layout_xml! into generated Rust at compile time.
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

There is no command to run for this path: the next `cargo build` (or
`cargo run`) expands the macro. The macro reads `layouts/main.buixml` relative
to the crate folder — the folder with `Cargo.toml` — so the same file from
page 05 works unchanged. The generated module exists only inside the compiled
binary; no `.rs` file appears on disk.

**What just happened.** During compilation, the macro read the document,
validated it and pasted the generated module where the invocation sits. The
`use` statement is the only clue in the source; the result is identical to the
CLI's output.

**Rebuild caveat.** Stable proc macros cannot register a file dependency, so
editing `layouts/main.buixml` does **not** by itself trigger a rebuild. Use the
CLI (or a build script) when the document changes independently, or touch the
including `.rs` file. `cargo clean -p <your-crate>` is the blunt fallback.

In practice: save the XML, run `cargo run`, and the window still shows the old
layout — no error, no warning. That is this caveat, not a broken macro. Touch
`src/main.rs` (add and remove a blank line), or regenerate with the CLI, then
build again.

### 4. Choose a consumption path

The three mode columns below answer the same question — *when is the XML turned
into rectangles?* — and the remaining table entries follow from that answer.

| | Runtime (`layout-xml`) | CLI / build script (`layout-compiled`) | `layout_xml!` (`layout-compiled`) |
|---|---|---|---|
| When the document is read | runtime, on install and reload | build time, explicit step | compile time, macro expansion |
| XML parser in the binary | yes | no | no |
| Errors | diagnostics; previous layout kept | CLI exits 1 with position | `compile_error!` with position |
| Hot reload | yes | no; rebuild | no; rebuild |
| Artifact to manage | the `.buixml` file | a checked-in or generated `.rs` file | none |
| Best for | development, theming, debug form | release builds, large static pages | small apps, one-step builds |

Read the last row as the default recommendation: use runtime mode while you are
still changing the layout, and compiled mode once the document is stable and
you want the parser out of the binary. The CLI form is easier to inspect and to
regenerate in CI; the macro form is fewer moving parts for a small app.

A page can move between the runtime and compiled modes without editing the XML.

## Expected result

`cargo build` succeeds and the window is identical to page 05: the same label
and button at the same rectangles. The difference is invisible at runtime — no
file is read, no parser is linked, and an invalid document was a build error
rather than a log line. Editing the `.buixml` has no effect until you regenerate
or rebuild.

**Run it.** Use the workspace folder `bamboo-hello/` or the crate folder
`bamboo-hello/bamboo-hello/`; Cargo resolves the same project from either:

```powershell
cargo run
```

The 520×320 window from page 05 appears, with the label and button in the same
places. The window is the proof that the generated rectangles are equivalent;
the build log is the proof that no parser was needed.

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
- **`error: package ID specification 'bamboo-native-ui-layout-codegen' did not match any packages`** — you are not inside the framework workspace. Run
  `cargo run -p ...` from the `native-ui` root, or use the
  `--manifest-path` form from the app folder.
- **`include!("main_layout.rs")` fails with "file not found"** — the path is
  relative to `main.rs`, so the file must be in `src/`. If the CLI wrote it
  somewhere else, the CLI's working directory differed from what you expected;
  regenerate with `-o src/main_layout.rs` from the crate folder.
- **The generated module is missing a function you expected** — `--module` sets
  the module name; `layout_xml!(..., module = ...)` must use the same name you
  call in Rust.
- **An edit to the `.buixml` is ignored** — expected for the macro. Regenerate
  with the CLI, add a build script, or touch the including source file. With the
  CLI form, remember that regenerating without rebuilding still runs the old
  compiled copy.
- **A typo in the XML produces no runtime message** — compiled mode has no
  runtime parse. The error appears when you run the generator or expand the
  macro, as a positioned diagnostic or `compile_error!`, not in the window.
- **`apply_compiled` is missing** — the `layout-compiled` feature is off, or you
  generated the module with a framework version that predates M7.6.
- **`no matching package named bamboo-native-ui-layout-macros`** — the macro
  crate path is wrong. It is `crates/layout-macros` in the framework checkout,
  a sibling of `crates/native-ui`.

## Next

That is the end of the quickstart. Continue with the
[widget reference](../widgets/index.md), the
[host and window API](../api/host-and-window.md), or the
[XML authoring guide](../guides/xml-layout-authoring.md).
