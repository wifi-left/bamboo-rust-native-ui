# Bamboo Native UI

A small, declarative native Windows UI framework, built for Bamboo Music
Player. GDI + dirty-rectangle rendering, a backend-abstracted painter, and no
egui, wgpu or WebView.

> **AI-assisted project.** The code, tests and documentation are largely
> generated or co-written by AI coding assistants and may contain mistakes.
> Review and test before relying on it.

## Why

Immediate-mode UI relayouts and repaints the whole window every frame. This
framework keeps the opposite contract:

- The application computes widget rectangles; the framework paints only the
  dirty rectangles and routes normalized events.
- One persistent back buffer per window; only the dirty bounding box is blitted.
- Idle means zero paints. No periodic repaint without an explicit timer.
- Paint-path allocations are forbidden; brushes, fonts and converted images are
  cached for the lifetime of the window.
- Public types are platform-neutral; the Win32/GDI/Direct2D host is the only
  platform layer.

## Features

- 62 widgets: text/content, inputs, controls, pickers, data views (virtualized
  list/table/tree/property grid), navigation, feedback, player components,
  chrome (menu bar, ribbon, toolbar, status bar), overlay and custom widgets.
- Retained `Ui<M>` container with focus, hit-testing, hover/press, drag,
  scrolling, per-instance properties and trigger bindings.
- UI Automation, native menus, dialogs, tray icons, notifications, global and
  window hotkeys, system drag and drop, gamepad navigation.
- `.buixml` layout documents: dynamic at runtime (`layout-xml`) or compiled to
  Rust (`layout-compiled` / `layout_xml!`).
- `.buitheme` theme documents that declare a whole palette (`theme-xml`).
- GDI default, Direct2D software/hardware backends.

## Quick start

```powershell
cargo test
cargo run -p bamboo-native-ui-showcase
cargo run -p bamboo-native-ui-showcase -- --self-test   # paints once, exits 0
cargo run -p bamboo-native-ui-showcase -- --a11y-dump   # UIA client check + tree
cargo run -p bamboo-native-ui-showcase -- --light
```

Use it from another crate. Clone the repository first:

```powershell
git clone https://github.com/wifi-left/bamboo-rust-native-ui.git native-ui
```

```toml
[dependencies]
bamboo-native-ui = { path = "native-ui/crates/native-ui" }
# optional features:
# features = ["layout-xml", "layout-compiled", "theme-xml", "debug-form"]
```

Or let Cargo fetch the workspace directly (pin `rev = "<commit>"` for
reproducible builds):

```toml
[dependencies]
bamboo-native-ui = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui" }
```

The minimal program shape is a `Host` implementation plus a `Ui<M>`:

```rust
use bamboo_native_ui::{Host, PaintCtx, WindowBuilder, WindowHandle};

struct App { /* Ui<Msg> and application state */ }

impl Host for App {
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        for rect in ctx.dirty {
            ctx.canvas.fill_rect(*rect, ctx.painter.palette().background);
        }
        // self.ui.paint(ctx);
    }
    // resize(): position widgets with self.ui.set_rect(..); window.invalidate_all()
}

fn main() -> windows::core::Result<()> {
    WindowBuilder::new(App { /* .. */ })
        .title("Hello")
        .size(800.0, 600.0)
        .run()
}
```

See the [first-window tutorial](docs/en/quickstart/03-first-window.md) for the
complete program.

## Documentation

| Resource | Contents |
|---|---|
| [English docs](docs/en/index.md) | Overview, thirteen quick starts (six core, seven practical), API reference, 62 widget pages, guides, lookup tables. |
| [中文文档](docs/zh/index.md) | Simplified Chinese mirror of the English documentation. |
| [llms.txt](docs/llms.txt) | AI-oriented index following the llmstxt.org convention. |
| [AI reference](docs/en/ai/reference.md) | One self-contained context file: crate map, features, signatures, widget catalog, XML/theme grammar. |
| Local design records | `docs/archive/` (untracked): `API.md`, `DECISIONS.md`, `M7-XML.md`, `CUSTOM-WIDGETS.md`. |

Build the HTML site with [Zensical](https://zensical.org/) (it reads the
MkDocs-compatible YAML config):

```powershell
pip install -r requirements-docs.txt
zensical build -f mkdocs.yml     # English -> site/
zensical build -f mkdocs.zh.yml  # Chinese -> site/zh/
```

`.github/workflows/docs.yml` deploys both to GitHub Pages with the official
`configure-pages` / `upload-pages-artifact` / `deploy-pages` flow.

## Repository layout

```
crates/native-ui/        the framework (bamboo-native-ui)
crates/layout-core/      .buixml/.buitheme model, parser, writer
crates/layout-codegen/   .buixml -> Rust generator (CLI + library)
crates/layout-macros/    layout_xml! proc macro
crates/showcase/         all-controls demo (bin: showcase)
docs/                    English/Chinese documentation and archived design records
```

## Status

Widgets, per-instance properties, triggers, UI Automation, tooltips, context
menus, dialogs, tray, hotkeys, the M6.5 component groups, the M7 layout skin
and the M7.8 theme documents are implemented. The showcase exercises every
area. Open items are tracked in the local `TODO.md`; `docs/HANDOFF.md` is the
resume point for the next session. Both are intentionally untracked.

## Using it as a submodule

The player repository tracks this repository at `native-ui/`. Fresh clones must
use `git clone --recursive` (or `git submodule update --init --recursive`) so
the path dependency resolves. Do not add `native-ui/` to the parent
`.gitignore`.

## License

Apache License 2.0. See [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE). Commercial
and closed-source use is allowed; keep the copyright notice and `NOTICE` with
any distribution, and state significant changes if you modify the source.
