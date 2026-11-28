# Quickstart

> Thirteen tutorials that take you from an empty folder to a running window —
> first the six core steps, then seven practical recipes for layout, data,
> chrome, desktop integration, theming, text input and a player page.

The six core tutorials are linear: each builds on the previous project, but
every page repeats the commands it needs, so you can also jump to the one you
care about and copy the project state it assumes. The practical tutorials reuse
the same project and add a complete feature area.

## Before you start

- **Operating system:** Windows 10 or Windows 11 on x64, which is the
  configuration the framework is tested on. The window host is Windows-only;
  the other tutorials still require it to run the result.
- **Rust:** a stable toolchain that supports edition 2024 (Rust 1.85 or newer).
  Install it from <https://rustup.rs> and make sure the MSVC target is active:
  `rustup show`.
- **C++ build tools:** Visual Studio 2022 Build Tools with the
  **Desktop development with C++** workload, which provides `link.exe` and the
  Windows SDK.
- A terminal. The examples use PowerShell notation; the commands are the same in
  cmd.

## Core tutorials

| Page | Topic | Adds |
|---|---|---|
| [01 — Create the project](01-create-project.md) | `cargo new`, a workspace, Windows prerequisites | A compilable binary |
| [02 — Add the crate](02-add-the-crate.md) | Path dependency, feature flags, first build | The framework linked |
| [03 — First window](03-first-window.md) | `Host`, `Ui<M>`, `WindowBuilder`, paint/resize/event | A window with a label and a button |
| [04 — Custom widgets](04-custom-widgets.md) | `CustomWidget`, implementing `Widget<M>`, skin protocol | A painted bar that emits a message |
| [05 — Dynamic XML](05-xml-dynamic.md) | `.buixml` at runtime (`feature = "layout-xml"`) | Rectangles, properties and created widgets from a file, with hot reload |
| [06 — Compiled XML](06-xml-compiled.md) | Codegen CLI and `layout_xml!` (`feature = "layout-compiled"`) | The same layout as generated Rust, no parser |

## Practical tutorials

| Page | Topic | Adds |
|---|---|---|
| [07 — Layout and breakpoints](07-layout-and-breakpoints.md) | `VStack`/`HStack`/`split_h`/`grid`, `.buixml` variants and expressions | A responsive sidebar shell |
| [08 — Lists, tables and trees](08-lists-tables-trees.md) | `ListView`, `Table`, `TreeView`, XML child lists | A virtualized track browser |
| [09 — Menus, toolbars and status](09-menus-toolbars-status.md) | `MenuBar`, menu buttons, `Toolbar`, `StatusBar`, context menus | Application chrome routed by command ids |
| [10 — Dialogs, tray and hotkeys](10-dialogs-tray-hotkeys.md) | Native dialogs, tray icon, notifications, global hotkeys and capture | Desktop integration |
| [11 — Theming and images](11-theming-and-images.md) | `ThemeSettings`/`Palette`, `.buitheme` (`theme-xml`), RGBA images | Runtime theming and media assets |
| [12 — Text editing and IME](12-text-editing-and-ime.md) | Native/painted `TextEdit`, `SearchBox`, `NumberInput`, IME composition | Text input |
| [13 — Player UI](13-player-ui.md) | `CoverWall`, `PlaylistView`, `PlaybackTransport`, `WaveformSeekBar` | A media-player page |

## What you need from each page

- Pages 01 and 02 are setup; the code appears in 03.
- Page 03 is the complete reference program. Most later pages modify it.
- Pages 05 and 06 are alternative ways to position widgets. They replace the
  `ui.set_rect` calls of page 03, not the `Host` implementation.
- Pages 07–13 are independent recipes; each names the earlier page whose
  project it assumes.

## Where to go next

- [Architecture](../overview/architecture.md) for how the layers fit.
- [Design principles](../overview/design-principles.md) for the dirty-rectangle
  contract.
- [Host and window](../api/host-and-window.md) for the full `Host` and
  `WindowHandle` surface.
