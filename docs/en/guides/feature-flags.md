# Feature flags

> Choose what the framework links: the Windows backend, the runtime XML
> readers, the compiled layout mode, or the debug API.

## The five features

All features are declared in `crates/native-ui/Cargo.toml`. `gdi` is the only
default.

| Feature | Default | What it enables | Extra dependencies |
|---|---|---|---|
| `gdi` | yes | The Windows GDI backend and the Direct2D renderer options (`Renderer::D2dSoftware`, `D2dHardware`). The public API is the same when it is off, but a running window needs it. | `windows`, `windows-core` (Windows only) |
| `layout-xml` | no | The runtime `.buixml` reader (`LayoutDocument::parse`) and applier (`Ui::apply_layout`), plus the widget skin protocol in the applier. | `layout-core` with `parse` (`quick-xml`) |
| `theme-xml` | no | The `.buitheme` reader (`ThemeDocument::parse`), `ThemeDocumentExt::palette` and `set_palette_token`. | `layout-core` with `parse` |
| `layout-compiled` | no | Runtime support for generated Rust: `layout_xml::compiled::CompiledLayout`, `Ui::apply_compiled`. No XML parser. | none |
| `debug-form` | no | `Ui::debug_snapshot`, `DebugSnapshot`, `PaintStats` and the diagnostics ring. | none directly; **implies `layout-xml`** |

The widget-declared skin types (`SkinAttrDecl`, `SkinValue`, `SkinElement`)
and the `Widget` skin methods are compiled **unconditionally**. A third-party
widget can publish its vocabulary without any feature flag; only reading a
document needs one of the layout features.

## Capability matrix

| Capability | default | `layout-xml` | `theme-xml` | `layout-compiled` | `debug-form` |
|---|---|---|---|---|---|
| Widgets, container, painter, GDI window | yes | yes | yes | yes | yes |
| Parse `.buixml` at runtime | — | yes | — | — | yes |
| `Ui::apply_layout` | — | yes | — | — | yes |
| Hot reload of a `.buixml` file | — | yes | — | — | yes |
| Parse `.buitheme` | — | — | yes | — | — |
| `Ui::apply_compiled` | — | — | — | yes | — |
| `Ui::debug_snapshot` | — | — | — | — | yes |
| XML parser linked (`quick-xml`) | no | yes | yes | no | yes |

`layout-xml` and `layout-compiled` are independent and can be enabled
together; the showcase does that so one build type-checks both modes. The same
document produces the same rectangles, properties and content either way.

`theme-xml` enables only `layout-core/parse`. It does not turn on the layout
reader, and a theme-only build can still apply a layout compiled with
`layout-compiled`.

## Enabling features

Runtime XML, development:

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

Theme documents only:

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["theme-xml"] }
```

Compiled layouts, release:

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-compiled", "theme-xml"] }
```

The generated module is produced by
`bamboo-native-ui-layout-codegen` (CLI or build script) or by the
`layout_xml!` proc macro from `bamboo-native-ui-layout-macros`. See
[06 — Compiled XML](../quickstart/06-xml-compiled.md).

Debug API (development builds; it pulls the parser in):

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["debug-form"] }
```

Portable library (non-Windows targets, widget tests, tooling):

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", default-features = false }
```

The `platform` module is `#[cfg(windows)]` and imports the GDI backend, so a
**Windows** build needs `gdi`; disabling default features is for a non-Windows
target (a Linux CI runner compiling the portable layer and running widget
tests). On Windows the portable layer is still available with `gdi` enabled.

## Build and verify

```powershell
cargo build                                       # default: gdi only, 0 warnings
cargo build -p bamboo-native-ui --features layout-xml
cargo build -p bamboo-native-ui --features theme-xml
cargo build -p bamboo-native-ui --features layout-compiled
cargo build -p bamboo-native-ui --features debug-form
cargo build -p bamboo-native-ui --features layout-xml,layout-compiled
cargo test  -p bamboo-native-ui --features layout-xml
cargo test  -p bamboo-native-ui --features theme-xml
cargo test  -p bamboo-native-ui --features layout-compiled
cargo test  -p bamboo-native-ui --features debug-form
cargo test  -p bamboo-native-ui-layout-core --features parse
cargo test  -p bamboo-native-ui-layout-codegen
cargo test  -p bamboo-native-ui-layout-macros
```

The showcase's `m7` feature enables all four opt-in features together, so one
`cargo test`/`cargo build` at the workspace level type-checks the combinations
an application is likely to use:

```powershell
cargo run -p bamboo-native-ui-showcase -- --self-test
cargo run -p bamboo-native-ui-showcase --no-default-features -- --self-test
```

## Trade-offs

| If you… | Choose | Because |
|---|---|---|
| Iterate on layout in a running app | `layout-xml` | Edit and reload without a rebuild. |
| Ship a release with no XML parser | `layout-compiled` | The binary has no parser and no expression interpreter. |
| Use only a `.buitheme` file | `theme-xml` | It does not pull the layout reader in. |
| Diagnose effective state in a dev build | `debug-form` | Snapshots and paint stats; keep it out of release. |
| Build a widget library or tool | default | No XML code; the portable types stay platform-neutral. |
| Support both XML modes in one binary | `layout-xml,layout-compiled` | They share the applier. |

## Common mistakes

- **`LayoutDocument` does not resolve** — the `layout-xml` feature is off.
- **`ThemeDocument` does not resolve** — the `theme-xml` feature is off; it is
  separate from `layout-xml`.
- **`apply_compiled` does not resolve** — the `layout-compiled` feature is off,
  or the generated module is from an older framework version.
- **A release still links `quick-xml`** — `debug-form` implies `layout-xml`.
  Disable `debug-form` for release.
- **`cargo build` fails with `cannot find gdi in backend`** — `default-features
  = false` removed `gdi` on a Windows build; keep the default feature there.
- **Editing a `.buixml` does not rebuild a `layout_xml!` module** — stable proc
  macros cannot register a file dependency; regenerate with the CLI or touch
  the including source file.

## See also

- [Crates and features](../overview/architecture.md)
- [06 — Compiled XML](../quickstart/06-xml-compiled.md)
- [XML layout authoring](xml-layout-authoring.md)
- [Theming](theming.md)
- [Testing](testing.md)
- [Debugging](debugging.md)
