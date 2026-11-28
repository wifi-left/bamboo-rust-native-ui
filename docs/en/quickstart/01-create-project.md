# 01 — Create the project

> Set up an empty Cargo workspace that will host the Bamboo Native UI examples.

## What you will build

A binary crate named `bamboo-hello` inside a small Cargo workspace. By the end
of this page `cargo run` prints `Hello, world!`; the window arrives in
[03 — First window](03-first-window.md).

## Prerequisites

### Operating system

Windows 10 or Windows 11. The framework's window host is Windows-only
(`#[cfg(windows)]`); the framework itself compiles elsewhere for tests, but this
tutorial runs a real window.

### Rust toolchain

Install Rust from <https://rustup.rs>. The framework uses edition 2024 and a
`resolver = "3"` workspace, so you need Rust 1.85 or newer; any current stable
works.

```powershell
rustup toolchain install stable-x86_64-pc-windows-msvc
rustup default stable-x86_64-pc-windows-msvc
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

To verify from a normal terminal:

```powershell
where.exe link.exe
```

If it prints nothing, open a **Developer PowerShell for VS 2022** (the build
tools shortcut) and continue there, or add the workload.

## Steps

### 1. Create the workspace folder and the crate

```powershell
mkdir bamboo-hello
cd bamboo-hello
cargo new bamboo-hello --vcs none
```

Expected output:

```
    Creating binary (application) `bamboo-hello` package
```

`--vcs none` avoids creating a nested git repository. Drop the flag if you want
one.

### 2. Add the workspace manifest

Create `bamboo-hello/Cargo.toml` (the outer folder, next to the crate folder):

```toml
[workspace]
resolver = "3"
members = ["bamboo-hello"]
```

`resolver = "3"` matches the framework's own workspace and the edition-2024
feature resolver. Listing the crate as a member means every `cargo` command in
the outer folder builds it.

### 3. Confirm the crate manifest

`bamboo-hello/bamboo-hello/Cargo.toml` is generated for you and should look like
this:

```toml
[package]
name = "bamboo-hello"
version = "0.1.0"
edition = "2024"

[dependencies]
```

Leave `edition = "2024"` in place. The framework itself also uses edition 2024
but a dependency's edition is independent, so this is only for your code.

### 4. Build and run

```powershell
cargo build
cargo run
```

Expected output:

```
   Compiling bamboo-hello v0.1.0 (C:\path\to\bamboo-hello\bamboo-hello)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.6s
     Running `target\debug\bamboo-hello.exe`
Hello, world!
```

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
The native-ui repository is a sibling folder; page 02 links it by path.

## Expected result

`cargo run` prints `Hello, world!` from the generated `main.rs`. No window is
created yet.

## Troubleshooting

- **The link step fails with "link.exe not found"** — the MSVC build tools are
  missing or not on `PATH`. Install the **Desktop development with C++**
  workload, or use a Developer PowerShell.
- **Cargo rejects the destination because it already exists** — you are already
  inside a folder named `bamboo-hello`. Run it from the parent folder, or pick
  another name.
- **`feature \`edition2024\` is required`** — the toolchain is older than Rust
  1.85. Run `rustup update stable`.
- **`rustup show` reports `windows-gnu`** — install the MSVC toolchain
  (`rustup toolchain install stable-x86_64-pc-windows-msvc`) and make it the
  default.

## Next

[02 — Add the crate](02-add-the-crate.md): link `bamboo-native-ui` by path and
choose the features you need.
