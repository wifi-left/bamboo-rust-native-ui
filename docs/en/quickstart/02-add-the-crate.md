# 02 — Add the crate

> Link `bamboo-native-ui` by path, pick the features you need, and build once.

## What you will build

The `bamboo-hello` crate from page 01 with the framework as a dependency. The
build compiles the framework and its Windows dependencies; `main.rs` still
prints `Hello, world!` until page 03.

## Prerequisites

Complete [01 — Create the project](01-create-project.md). You need the
`bamboo-hello` workspace, a working MSVC toolchain and the Windows SDK, and a
local checkout of the native-ui repository.

## Steps

### 1. Point at the framework

The framework is not on crates.io; reference the crate folder. If your project
sits next to the repository:

```
F:\codes\
├── bamboo-hello\          your workspace from page 01
└── native-ui\             the framework repository
    └── crates\native-ui\
```

then edit `bamboo-hello/Cargo.toml`:

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui" }
```

When the framework is checked into your repository as a git submodule, the
player's layout is the reference:

```toml
bamboo-native-ui = { path = "native-ui/crates/native-ui" }
```

Use the path to the **crate** (`crates/native-ui`), not to the workspace root.
A path to the root resolves to the virtual workspace manifest, which is not a
library.

### 2. Choose features

`bamboo-native-ui` has five features. Only `gdi` is on by default, which is
enough for page 03.

| Feature | Default | Enables |
|---|---|---|
| `gdi` | yes | The GDI backend (and the Direct2D renderer options). Keep it on. |
| `layout-xml` | no | `.buixml` parsing (`LayoutDocument::parse`) and `Ui::apply_layout` for hot reload |
| `layout-compiled` | no | Generated layouts (`CompiledLayout`, `Ui::apply_compiled`); no parser |
| `theme-xml` | no | `.buitheme` theme documents mapped onto `Palette` |
| `debug-form` | no | Read-only debugging snapshots (`Ui::debug_snapshot`) and paint statistics; implies `layout-xml` |

Enable them now if you know you need them, or add them on the page that uses
them:

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

`gdi` is additive, so `features = ["layout-xml"]` keeps it enabled. To build
the examples in pages 05 and 06 you need `layout-xml` and `layout-compiled`
respectively.

### 3. Build the workspace

```powershell
cargo build
```

The first build compiles the `windows` crate and its generated bindings, so it
takes a minute or more. Expected output (paths and timing vary):

```
   Compiling windows v0.62.2
   Compiling bamboo-native-ui v0.1.0 (F:\codes\native-ui\crates\native-ui)
   Compiling bamboo-hello v0.1.0 (F:\codes\bamboo-hello\bamboo-hello)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 12s
```

A successful build with no warnings is the baseline the framework's own
`docs/HANDOFF.md` requires before a commit; if you see warnings from *your*
crate, fix them before moving on.

### 4. About the submodule

If the framework lives in your repository as a git submodule, fresh clones must
initialise it:

```powershell
git clone --recursive <your-repo>
# or, in an existing clone:
git submodule update --init --recursive
```

Do not add the submodule folder to `.gitignore`; the parent repository must
track the gitlink, otherwise a fresh clone cannot resolve the path dependency.

## Expected result

`cargo build` finishes with `Finished` and no errors. `cargo run` still prints
`Hello, world!`. The framework is linked and ready to use.

## Troubleshooting

- **Cargo reports "failed to load manifest for dependency bamboo-native-ui"** —
  the path is wrong. It must end at `crates/native-ui`. Check the relative depth:
  from `bamboo-hello/bamboo-hello/Cargo.toml`, the repository next to the
  workspace is `../../native-ui/crates/native-ui`.
- **Cargo reports "no matching package named bamboo-native-ui-layout-core
  found"** — the checkout is incomplete or you pointed at a copied `native-ui`
  folder that has no `crates/` siblings. Use a full checkout (or
  `git submodule update --init`).
- **An unknown feature warning** — feature names are exact:
  `layout-xml`, `layout-compiled`, `theme-xml`, `debug-form`, `gdi`. A typo is
  reported by Cargo rather than silently ignored.
- **The first build is very slow** — expected; the `windows` bindings are large.
  Later builds reuse them.

## Next

[03 — First window](03-first-window.md): write the complete `Host`
implementation and open a window.
