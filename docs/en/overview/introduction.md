# Introduction

> Bamboo Native UI is a small, declarative, retained-mode UI framework for
> native Windows desktop applications, painted with GDI on dirty rectangles.

## What it is

Bamboo Native UI (`bamboo-native-ui`) is a Rust library that opens one native
Win32 window, keeps a tree of retained widgets, and paints them into a
persistent back buffer. The application owns the layout: it assigns a rectangle
to every widget, usually when the window is resized. The framework owns the
pixels: it merges the damaged regions, paints only the widgets that intersect
them, and blits only those pieces to the screen.

It is not a general-purpose toolkit. There is no WebView and no immediate-mode
redraw loop. The default renderer is GDI; a Direct2D hardware or software
renderer can be selected, but neither is required. The windowing system is not
abstracted across platforms: everything that touches Win32 sits behind a single
host boundary, while the types an application and a widget use day to day
(`Rect`, `Size`, `Color`, `Event`, `Palette`, `Canvas`) are platform-neutral.

## Why it exists

Immediate-mode libraries relayout and repaint the whole window every frame. That
is a fine default for tools and prototypes, but a music player spends most of
its life idle: a static page, a playing track, a slowly changing progress bar.
Repainting the window sixty times a second to show the same picture is wasted
electricity and a source of jank on low-end machines.

Bamboo Native UI keeps the opposite contract:

- The application computes widget rectangles; the framework paints only the
  dirty rectangles and routes normalized events.
- One persistent back buffer per window; only the dirty bounding box is
  blitted.
- Idle means zero paints. No periodic repaint happens without an explicit
  timer.
- Paint-path allocations are forbidden; brushes, fonts and converted images are
  cached for the lifetime of the window.

The result is a framework whose cost is proportional to what changed. A hover
that changes two small widgets costs two small repaints, not a frame.

## Current status

- **Milestones M2 through M4 are complete.** The M2, M2.5 and M2.6 rounds built
  the widget library (including the per-widget gap pass), per-instance
  properties and triggers, and container-owned overlap and hit-testing. M3
  added UI Automation, including the closed-field text model. M4 added the
  native surfaces: tooltips, context menus, dialogs, the tray icon and global
  hotkeys. Only the layered window surface remains open.
- **M6.5 is in progress.** The text and content group is complete (wrapping
  label, icon, badge, divider, avatars, shortcut chip, copyable text, marquee,
  rich text with inline links). The input widgets, the feedback widgets and an
  in-window modal shell are in. Framework-painted text editors (`.painted()`)
  render inline IME composition and expose live UI Automation text without a
  native child control.
- **M7 is implemented.** Layout skins can be authored in `.buixml` and consumed
  at runtime (`feature = "layout-xml"`) or compiled into Rust
  (`feature = "layout-compiled"`). M7.8 added `.buitheme` theme documents
  (`feature = "theme-xml"`) that map onto a `Palette`.

The all-controls demo in `crates/showcase` is the reference wiring for every
feature listed above.

## Who it is for

- Rust developers building a **Windows desktop application** that wants native
  controls, native text rendering and predictable CPU use.
- Applications with a **mostly static page** and a small number of animated or
  interactive regions, such as media players, dashboards and device utilities.
- Projects that want to **own their layout arithmetic** rather than hand it to
  a constraint solver, and that are comfortable assigning rectangles on resize.
- Teams that need **UI Automation**, IME, clipboard and native menus without
  adopting a full GUI toolkit.

It is a poor fit if you need cross-platform windows, a browser rendering model,
a large third-party theme ecosystem, or free-form per-frame animation over the
whole window.

## License

Bamboo Native UI is licensed under the **Apache License, Version 2.0**. See
`LICENSE` and `NOTICE` in the repository root.

In short: commercial and closed-source use is allowed, but you must keep the
copyright notice and the `NOTICE` file with any distribution, and state
significant changes if you modify the source. This is attribution by notice,
not a requirement to display credit in the application UI. A README or
documentation credit is appreciated but not required.

The license is permissive, so it does not conflict with an application's own
(non-commercial) terms: a combined distribution keeps this project's `LICENSE`
and `NOTICE` alongside the application's license.

## See also

- [Architecture](architecture.md)
- [Design principles](design-principles.md)
- [Quickstart](../quickstart/index.md)
