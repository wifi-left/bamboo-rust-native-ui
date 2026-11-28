# 02 — Add the crate

> Link `bamboo-native-ui` by path, pick the features you need, and build once.

## What you will build

The `bamboo-hello` crate from page 01 with the framework as a dependency. The
build compiles the framework and its Windows dependencies; `main.rs` still
prints `Hello, world!` until page 03.

## New terms on this page

- **dependency** — another crate your crate calls; it is listed under
  `[dependencies]` in your package manifest.
- **feature flag** — a named, optional switch inside a crate. Code behind a
  feature is compiled only when the feature is listed in `features = [...]`.
- **default features** — the features a crate turns on unless you disable them;
  `gdi` is the framework's only default.
- **path dependency** — a dependency located by a folder path; Cargo uses that
  checkout directly, so local edits are picked up.
- **git dependency** — a dependency Cargo clones from a Git URL and caches; no
  local checkout is needed.
- **submodule** — a Git repository nested inside another at a pinned commit;
  combined with a path dependency it gives an exact framework revision.
- **virtual workspace** — a workspace manifest with `[workspace]` but no
  `[package]`; it coordinates crates but is not a library itself.
- **crates.io** — the public Rust package registry. This framework is not
  published there, which is why the dependency is by path or Git.
- **`Cargo.lock`** — the file that records the exact versions Cargo resolved;
  commit it for an application.
- **revision (`rev`)** — one pinned Git commit hash.
- **code generation** — turning a `.buixml` file into Rust source ahead of time;
  the codegen CLI does this.
- **proc macro** — a macro that expands into Rust code during compilation;
  `layout_xml!` is one.

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
workspace. Run this in the folder that contains your `bamboo-hello` workspace
(`F:\codes` in the examples), so `native-ui/` becomes a sibling:

```powershell
# Public repository; clones into native-ui/ beside the workspace.
git clone https://github.com/wifi-left/bamboo-rust-native-ui.git native-ui
```

The result is:

```
F:\codes\
├── bamboo-hello\          your workspace from page 01
└── native-ui\             the framework repository
    └── crates\native-ui\  the library crate
```

If the framework belongs to your project's history, add it as a submodule so
the commit pins the exact revision. Run these at the root of your own Git
repository (the folder that contains its `.git`):

```powershell
# Record the framework as a submodule at the revision you add.
git submodule add https://github.com/wifi-left/bamboo-rust-native-ui.git native-ui
# Materialize the submodule in this clone, including nested submodules.
git submodule update --init --recursive
```

Both forms produce the `native-ui/` folder used below. The crate itself is in
`native-ui/crates/native-ui`; the repository root is a virtual workspace and is
not a library.

**What just happened.** Git downloaded the repository. `crates/native-ui` is the
crate you depend on; the other `crates/` folders are parts of the same workspace
(the layout model, codegen, macros and showcase). Nothing has been compiled
yet.

### 2. Point at the framework

The framework is not on crates.io, so the dependency names a source instead:
a **path dependency** pointing at a local checkout, or a **git dependency**
that Cargo fetches. A submodule is a path dependency whose checkout your own
repository pins.

A dependency line has three parts: the name your code imports
(`bamboo-native-ui`), the source (`path` or `git`), and optional `features`.
Add it to the **crate manifest** — the `Cargo.toml` with a `[package]` section,
which page 01 created at `bamboo-hello/bamboo-hello/Cargo.toml`. The workspace
manifest (`bamboo-hello/Cargo.toml`) only has `[workspace]`; a `[dependencies]`
section there is rejected, because a virtual manifest has no package to apply it
to.

If your project sits next to the repository:

```
F:\codes\
├── bamboo-hello\          your workspace from page 01
└── native-ui\             the framework repository
    └── crates\native-ui\
```

then edit the crate manifest:

```toml
[dependencies]
# Path to the crate folder, not the repository root.
bamboo-native-ui = { path = "../../native-ui/crates/native-ui" }
```

The relative path is counted from the manifest file, not from the folder where
you ran `git clone`. In the page-01 layout the crate manifest is
`bamboo-hello/bamboo-hello/Cargo.toml`, two folders below `F:\codes`, so
`../../native-ui/crates/native-ui` reaches the sibling repository. Add one `..`
for each extra folder level; Cargo prints the path it tried when the manifest is
missing.

When the framework is checked into your repository as a git submodule, the path
depends on where the submodule sits. If it is inside the outer workspace, next
to the crate folder:

```toml
# Submodule at bamboo-hello/native-ui, seen from the crate manifest.
bamboo-native-ui = { path = "../native-ui/crates/native-ui" }
```

If the crate manifest is at the repository root and the submodule is
`native-ui/` beside it (the player's layout), use
`native-ui/crates/native-ui` instead.

Use the path to the **crate** (`crates/native-ui`), not to the workspace root.
A path to the root resolves to the virtual workspace manifest, which has no
`[package]` and no library target, so Cargo cannot link it.

#### Or depend on the GitHub repository directly

If you do not want a local checkout, Cargo can fetch the workspace:

```toml
[dependencies]
# package = "..." selects the crate; the URL points at the workspace root.
bamboo-native-ui = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui" }
```

The `package` key is required: the URL points at the workspace root, and Cargo
selects the `bamboo-native-ui` member by name. Pin `rev = "<commit>"` for
reproducible builds, or use `branch = "main"` to follow the default branch. Add
features the same way:

```toml
# Same Git source, with optional .buixml parsing compiled in.
bamboo-native-ui = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui", features = ["layout-xml"] }
```

The git form fetches the whole workspace on the first build and caches it under
`~/.cargo/git`; the path form is faster for local iteration. The codegen CLI is
a binary, so clone the repository to run it. The `layout_xml!` proc macro can be
a git dependency too:

```toml
# Proc-macro crate that provides layout_xml!; same Git repository.
bamboo-native-ui-layout-macros = { git = "https://github.com/wifi-left/bamboo-rust-native-ui", package = "bamboo-native-ui-layout-macros" }
```

### 3. Choose features

A feature flag is a compile-time choice, not a runtime setting: it decides
which code exists in the binary. `bamboo-native-ui` has five features. Only
`gdi` is on by default, which is enough for page 03.

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
# Add optional .buixml parsing; default gdi remains enabled.
bamboo-native-ui = { path = "../../native-ui/crates/native-ui", features = ["layout-xml"] }
```

`gdi` is additive, so `features = ["layout-xml"]` keeps it enabled. To build
the examples in pages 05 and 06 you need `layout-xml` and `layout-compiled`
respectively.

### 4. Build the workspace

Run this in the outer `bamboo-hello` folder (the workspace root, the one with
the workspace `Cargo.toml`):

```powershell
# Resolve, download and compile every dependency of bamboo-hello.
cargo build
```

The first build downloads the `windows` crate source and compiles it together
with its generated bindings, then the framework crates, then your crate, so it
takes a minute or more. Later builds reuse the results in `target/`. Expected
output (paths and timing vary):

```
   Compiling windows v0.62.2
   Compiling bamboo-native-ui v0.1.0 (F:\codes\native-ui\crates\native-ui)
   Compiling bamboo-hello v0.1.0 (F:\codes\bamboo-hello\bamboo-hello)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 12s
```

**How to tell the framework is linked.** The
`Compiling bamboo-native-ui v0.1.0 (...)` line is direct evidence that Cargo
reached the dependency. On later builds it may not appear because the result is
cached and Cargo prints only `Finished`. To inspect the graph, run this in the
same outer folder:

```powershell
# List the crates bamboo-hello depends on.
cargo tree
```

`bamboo-native-ui` appears under `bamboo-hello`, with the `windows` crates
below it. If it is missing, the `[dependencies]` line is in the wrong manifest
or the file was not saved.

**What just happened.** Cargo resolved the new dependency, downloaded and
compiled it and its Windows support crates, and then compiled your unchanged
`main.rs`. The program still prints `Hello, world!`; page 03 replaces it with a
window.

A successful build with no warnings is the baseline the framework's own
`docs/HANDOFF.md` requires before a commit; if you see warnings from *your*
crate, fix them before moving on.

### 5. About the submodule

If the framework lives in your repository as a git submodule, fresh clones must
initialise it. Run the first command wherever you keep repositories; run the
second inside an existing clone:

```powershell
# Clone your repository and its submodules in one step.
git clone --recursive <your-repo>
# or, in an existing clone, fetch the pinned submodule revisions:
git submodule update --init --recursive
```

Do not add the submodule folder to `.gitignore`; the parent repository must
track the gitlink, otherwise a fresh clone cannot resolve the path dependency.

**What just happened.** A fresh clone creates an empty `native-ui/` folder until
the submodule is initialised; only then does the path dependency point at a
checkout. This is why a build can work on your machine and fail on a colleague's
fresh clone.

## Expected result

`cargo build` finishes with `Finished` and no errors. The first build prints a
`Compiling bamboo-native-ui ...` line, and `cargo tree` lists the framework
under `bamboo-hello`. `cargo run` still prints `Hello, world!` and exits; the
framework is linked and ready to use, but nothing draws yet.

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
- **`cargo` is not recognized as a command** — Rust is not installed, or this
  terminal predates the installation. Install from <https://rustup.rs> and open
  a new PowerShell window.
- **`error: could not find \`Cargo.toml\``** — `cargo build` is running in the
  wrong folder. Use the outer `bamboo-hello` folder that contains the workspace
  manifest.
- **"this virtual manifest specifies a `dependencies` section"** — the
  dependency was added to the workspace manifest (`[workspace]`, no
  `[package]`). Move the `[dependencies]` line to the crate manifest
  (`bamboo-hello/bamboo-hello/Cargo.toml`).
- **The build succeeds but never compiles `bamboo-native-ui`** — the edit went
  to another `Cargo.toml`, or the file was not saved. Confirm the crate
  manifest contains the line and run `cargo tree` from the outer folder.
- **The old manifest is still used after an edit** — the editor has not saved,
  or a second terminal is running in a different checkout. Save with Ctrl+S and
  check the path in the error or in `cargo tree` output.
- **`git` is not recognized as a command** — install Git for Windows, open a new
  terminal, and check `git --version`.
- **`destination path 'native-ui' already exists`** — a checkout is already
  there. Reuse it, or delete or rename it before cloning again.
- **The link step fails with "link.exe not found"** — the C++ build tools or
  Windows SDK are missing. Install the **Desktop development with C++**
  workload and open a new terminal; see page 01.
- **The build succeeds but no window appears** — expected on this page.
  `main.rs` still prints `Hello, world!`; page 03 replaces it with a window.

## Next

[03 — First window](03-first-window.md): write the complete `Host`
implementation and open a window.
