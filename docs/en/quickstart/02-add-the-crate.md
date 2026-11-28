# 02 — Add the crate

> Link `bamboo-native-ui` by path, pick the features you need, and build once.

## What you will build

The `bamboo-hello` crate from page 01 with the framework as a dependency. The
build compiles the framework and its Windows dependencies; `main.rs` still
prints `Hello, world!` until page 03.

## Prerequisites

Complete [01 — Create the project](01-create-project.md). You need the
`bamboo-hello` workspace, a working MSVC toolchain and the Windows SDK, and
either a local checkout of
<https://github.com/wifi-left/bamboo-rust-native-ui> or network access to clone
it. A configured Git proxy is enough; the repository is public.

## Steps

### 1. Get the framework from GitHub

The repository is public at
<https://github.com/wifi-left/bamboo-rust-native-ui>. Clone it next to your
workspace:

```powershell
git clone https://github.com/wifi-left/bamboo-rust-native-ui.git native-ui
```

If the framework belongs to your project's history, add it as a submodule so
the commit pins the exact revision:

```powershell
git submodule add https://github.com/wifi-left/bamboo-rust-native-ui.git native-ui
git submodule update --init --recursive
```

Both forms produce the `native-ui/` folder used below. The crate itself is in
`native-ui/crates/native-ui`; the repository root is a virtual workspace and is
not a library.

### 2. Point at the framework

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

#### Or depend on the GitHub repository directly

If you do not want a local checkout, Cargo can fetch the workspace:

```toml
[dependencies]
bamboo-native-ui = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui" }
```

The `package` key is required: the URL points at the workspace root, and Cargo
selects the `bamboo-native-ui` member by name. Pin `rev = "<commit>"` for
reproducible builds, or use `branch = "main"` to follow the default branch. Add
features the same way:

```toml
bamboo-native-ui = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui", features = ["layout-xml"] }
```

The git form fetches the whole workspace on the first build and caches it under
`~/.cargo/git`; the path form is faster for local iteration. The codegen CLI is
a binary, so clone the repository to run it. The `layout_xml!` proc macro can be
a git dependency too:

```toml
bamboo-native-ui-layout-macros = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui-layout-macros" }
```

### 3. Choose features

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

### 4. Build the workspace

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

### 5. About the submodule

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
- **Cargo cannot fetch the git dependency** — the repository is public, so this
  is almost always a network or proxy problem. `git ls-remote
  https://github.com/wifi-left/bamboo-rust-native-ui` verifies access; without
  network, use a local clone and the path form.
- **Two copies of the framework are compiled** — a path dependency and a git
  dependency are different Cargo sources even when the revision is identical.
  Pick one form per workspace.
- **The first build is very slow** — expected; the `windows` bindings are large.
  Later builds reuse them.

## Next

[03 — First window](03-first-window.md): write the complete `Host`
implementation and open a window.
