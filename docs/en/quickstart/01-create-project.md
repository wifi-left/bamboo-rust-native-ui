# 01 — Create the project

> Set up an empty Cargo workspace that will host the Bamboo Native UI examples.

## What you will build

A binary crate named `bamboo-hello` inside a small Cargo workspace. By the end
of this page `cargo run` prints `Hello, world!`; the window arrives in
[03 — First window](03-first-window.md).

## New terms on this page

- **Rust** — the programming language. Its compiler turns the whole program
  into a native `.exe` and rejects code it cannot check.
- **Cargo** — the build tool that ships with Rust. It creates projects, reads
  `Cargo.toml`, downloads dependencies, compiles and starts programs.
- **crate** — one Rust package that compiles as a unit. A *library crate* is
  used by other code; a *binary crate* becomes an executable.
- **binary crate** — the crate that produces your `.exe`; it has a `main`
  function and is what `cargo run` starts.
- **workspace** — a Cargo project that groups one or more crates and shares a
  single `Cargo.lock` and `target/` folder.
- **manifest** — the `Cargo.toml` file that describes a crate or a workspace.
- **edition** — the version of the Rust language a crate is written against;
  `2024` is the newest and needs Rust 1.85 or newer.
- **resolver** — Cargo's algorithm for choosing dependency features; `"3"` is
  the resolver that matches edition 2024.
- **build** — compile the code into an executable without starting it.
- **run** — build if needed, then start the executable.
- **linker (`link.exe`)** — the tool that joins compiled Rust code with the
  Windows libraries to produce the `.exe`; the MSVC build tools provide it.
- **`target/`** — the folder where Cargo writes build output. It can be
  deleted; Cargo recreates it.

## Prerequisites

### Operating system

Windows 10 or Windows 11. The framework's window host is Windows-only
(`#[cfg(windows)]`); the framework itself compiles elsewhere for tests, but this
tutorial runs a real window.

### Rust toolchain

Install Rust from <https://rustup.rs>. The framework uses edition 2024 and a
`resolver = "3"` workspace, so you need Rust 1.85 or newer; any current stable
works.

Run these three commands in any PowerShell window. They change your user-wide
toolchain, not a project folder.

```powershell
# Download the Windows MSVC build of the stable toolchain.
rustup toolchain install stable-x86_64-pc-windows-msvc
# Make it the default for new terminals.
rustup default stable-x86_64-pc-windows-msvc
# Print the active host and toolchain.
rustup show
```

Expected output (the version numbers depend on your toolchain):

```
Default host: x86_64-pc-windows-msvc
active toolchain
----------------
stable-x86_64-pc-windows-msvc (default)
rustc 1.91.0
```

`rustup show` must report an **msvc** host and toolchain; that is the
configuration the framework is tested with. The `windows-gnu` toolchain is not
covered.

### C++ build tools and the Windows SDK

Install **Visual Studio 2022 Build Tools** and select the
**Desktop development with C++** workload. This provides `link.exe`, the C/C++
runtime and a Windows 10/11 SDK. The `windows` crate needs them at link time;
there is no separate SDK download.

To verify, run this in a normal PowerShell window; the project folder does not
matter:

```powershell
# Prints the full path of the C++ linker if it is on PATH.
where.exe link.exe
```

If it prints nothing, open a **Developer PowerShell for VS 2022** (the build
tools shortcut) and continue there, or add the workload. An already-open
terminal keeps its old `PATH`; after installing, open a new one.

## Steps

### 1. Create the workspace folder and the crate

Run these in the folder that should contain the project — `F:\codes` in the
examples. The first command creates `F:\codes\bamboo-hello`; the other two run
inside it.

```powershell
# Create the outer project folder.
mkdir bamboo-hello
# Move into it; every later command on this page runs from here.
cd bamboo-hello
# Create the crate in a subfolder, without a nested .git repository.
cargo new bamboo-hello --vcs none
```

Expected output:

```
    Creating binary (application) `bamboo-hello` package
```

`--vcs none` avoids creating a nested git repository. Drop the flag if you want
one.

`cargo new` has now created two levels:

```
bamboo-hello/               outer folder; you are here
└── bamboo-hello/           the crate
    ├── Cargo.toml          package manifest
    └── src/
        └── main.rs         prints "Hello, world!"
```

**What just happened.** `cargo new` wrote a complete minimal program: a manifest
that names the package, and `src/main.rs` with a `main` function that prints
`Hello, world!`. The outer folder is not a crate yet; step 2 makes it a
workspace. Nothing has been compiled so far.

### 2. Add the workspace manifest

A project with two `Cargo.toml` files is normal. The **outer** manifest describes
the workspace: which crates belong to it and how their dependencies are
resolved. The **inner** manifest describes your crate: its name, version,
edition and dependencies. Cargo searches upward from the current folder for the
nearest manifest, so a command run in the outer folder affects its members.

Create `bamboo-hello/Cargo.toml` in the outer folder — from the example
location, `F:\codes\bamboo-hello\Cargo.toml`, next to the crate folder:

```toml
[workspace]
# Edition-2024 feature resolver, the same one the framework uses.
resolver = "3"
# The crate folder created by cargo new.
members = ["bamboo-hello"]
```

`resolver = "3"` matches the framework's own workspace and the edition-2024
feature resolver. Listing the crate as a member means every `cargo` command in
the outer folder builds it. This manifest has no `[package]` section, so it
builds nothing by itself.

**What just happened.** The outer folder became a workspace root. It still has
no `src/` and produces no executable; it only coordinates the crates listed in
`members`.

### 3. Confirm the crate manifest

`bamboo-hello/bamboo-hello/Cargo.toml` is generated for you and should look like
this. This is the manifest whose `[package]` names your program; later pages add
dependencies here, not to the workspace manifest.

```toml
[package]
# The crate name; Cargo also uses it for the executable name.
name = "bamboo-hello"
version = "0.1.0"
# Language edition for this crate only; requires Rust 1.85 or newer.
edition = "2024"

[dependencies]
# Empty for now; page 02 adds bamboo-native-ui here.
```

Leave `edition = "2024"` in place. An edition is not a different compiler: it
enables newer syntax and rules while older crates keep compiling. The framework
itself also uses edition 2024, but a dependency's edition is independent, so
this setting is only for your code. The `resolver = "3"` in the workspace
matches it.

### 4. Build and run

Run these in the outer `bamboo-hello` folder (the one that contains the
workspace `Cargo.toml` and the `bamboo-hello` crate folder).

```powershell
# Compile in the debug profile; produces target\debug\bamboo-hello.exe.
cargo build
# Compile again if anything changed, then start the executable.
cargo run
```

`build` stops after producing the executable. `run` performs the same compile
step and then starts the program, which is why the output below shows both.

Expected output:

```
   Compiling bamboo-hello v0.1.0 (C:\path\to\bamboo-hello\bamboo-hello)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.6s
     Running `target\debug\bamboo-hello.exe`
Hello, world!
```

The timings and the path in the `Compiling` line depend on your machine. The
important parts are `Finished` (the compile succeeded) and `Hello, world!` (the
program started).

**What just happened.** Cargo compiled your crate, linked it into
`target\debug\bamboo-hello.exe`, and `cargo run` started that file. The text
appears in the terminal that ran the command; this page creates no window yet.
`Cargo.lock` and the `target/` folder were created next to the workspace
manifest.

## Project layout

```
bamboo-hello/
├── Cargo.toml              workspace manifest (resolver 3, members)
├── Cargo.lock              created by the first build
└── bamboo-hello/
    ├── Cargo.toml          the binary crate
    └── src/
        └── main.rs         "Hello, world!" for now
```

Everything you add in the next pages goes under `bamboo-hello/bamboo-hello/`.
Cargo writes the build output to `target/` in the outer folder; that folder is
generated and safe to delete. The native-ui repository is a sibling folder;
page 02 links it by path.

## Expected result

`cargo run` prints `Hello, world!` from the generated `main.rs`. The program
exits on its own, so the prompt returns immediately. No window is created yet;
if you expected one, that is page 03.

## Troubleshooting

- **The link step fails with "link.exe not found"** — the MSVC build tools are
  missing or not on `PATH`. Install the **Desktop development with C++**
  workload, or use a Developer PowerShell. Installing the workload does not
  update already-open terminals; open a new one and check `where.exe link.exe`
  again.
- **Cargo rejects the destination because it already exists** — you are already
  inside a folder named `bamboo-hello`. Run it from the parent folder, or pick
  another name.
- **`feature \`edition2024\` is required`** — the toolchain is older than Rust
  1.85. Run `rustup update stable`.
- **`rustup show` reports `windows-gnu`** — install the MSVC toolchain
  (`rustup toolchain install stable-x86_64-pc-windows-msvc`) and make it the
  default.
- **`cargo` is not recognized as a command** — Rust is not installed, or this
  terminal was opened before the installation finished. Install from
  <https://rustup.rs>, open a new PowerShell window, and check
  `cargo --version`. If `rustup --version` works but `cargo` does not, the
  toolchain step above did not complete.
- **`error: could not find \`Cargo.toml\``** — you are in the wrong folder. Run
  `cargo build` and `cargo run` from the outer `bamboo-hello` folder. A folder
  listing there shows `Cargo.toml` and the inner `bamboo-hello` folder; `src/`
  is inside the inner folder, not here.
- **The old program still runs after an edit** — the editor has not saved the
  file, or you edited a copy elsewhere. Check the editor tab for an unsaved
  marker, press Ctrl+S, and confirm the path is
  `...\bamboo-hello\bamboo-hello\src\main.rs`.
- **`cargo run` prints nothing and returns** — `main.rs` may no longer contain
  the generated `println!("Hello, world!");`. An empty `main` compiles and exits
  without output.
- **The console flashes and closes** — you started the `.exe` from Explorer.
  Run `cargo run` from a terminal so the output stays visible.
- **The build succeeds but no window appears** — expected on this page. The
  generated program only prints text; the first window is page 03.

## Next

[02 — Add the crate](02-add-the-crate.md): link `bamboo-native-ui` by path and
choose the features you need.
