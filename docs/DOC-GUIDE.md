# Documentation authoring guide

This file is the contract for everyone writing the English documentation under
`docs/en/`. It is read-only for writing agents; only the integrator edits it.
Read it fully before creating a page.

## Ground rules

1. **Write English only** in `docs/en/`. The Chinese mirror in `docs/zh/` is
   produced later by translation agents.
2. **Never invent APIs.** Open the real source under
   `crates/native-ui/src/` (and `crates/layout-core/src/` for the XML model)
   and copy names, parameter types and defaults from it. If a builder method
   is not in the source, it does not exist.
3. **Stay inside your owned paths.** Do not create or edit `index.md`,
   `llms.txt`, `README.md`, `mkdocs*.yml`, `docs/README.md`, or another
   agent's folder. Report index entries in your final message instead.
4. **No preview images.** The repository ships no screenshots and the
   showcase has no capture mode. Describe the widget's appearance in prose;
   omit a "Preview" heading entirely.
5. **One concept per page.** Keep pages focused; link out instead of repeating
   whole API references.
6. **Use relative links.** From `docs/en/widgets/content/label.md`:
   - another widget page: `../controls/button.md`
   - the widgets index: `../index.md` (that is `docs/en/widgets/index.md`)
   - an API page: `../../api/theme-and-palette.md`
   - a guide: `../../guides/theming.md`
   - the docs home: `../../index.md`
   Never use absolute `/docs/...` paths.

## Terminology

| Concept | Use |
|---|---|
| The library | "Bamboo Native UI" |
| The framework crate | `bamboo-native-ui` |
| The container | `Ui<M>` |
| The application message type | `M` (examples declare `enum Msg`) |
| Widget identifier | `WidgetId`, built from `&'static str` |
| Device-independent pixels | DIPs |
| Layout document | `.buixml` (runtime: "dynamic"; codegen/macro: "compiled") |
| Theme document | `.buitheme` |

Feature gates are written `feature = "layout-xml"` and explained on first use.
Mention Windows-only items explicitly ("Windows-only", `#[cfg(windows)]`).

## Code conventions for examples

- Start every Rust snippet with the imports it needs.
- Use a local `enum Msg` for messages; do not assume the reader's type.
- Widget ids are `&'static str` literals.
- Position widgets with `ui.set_rect(WidgetId::new("id"), rect)` inside the
  host's `resize`, or by constructing a `Rect`.
- Keep snippets small (5–20 lines) and compilable in spirit; no `...`
  placeholders in the middle of an expression. Elide unrelated host code with
  a comment and a short `fn` boundary instead.
- Do not show `unsafe`, Win32 calls, or `windows` crate types unless the page
  is specifically about the host/platform layer.

## Widget page template (use exactly this shape)

````markdown
# Label

> One-sentence description of what the widget is for.

## Introduction

Two or three short paragraphs: what it renders, what interaction it supports,
whether it is a simple control, a composite control or a container. Mention
the generic message parameter `M` if the widget sends messages. Mention
Windows-only or feature-gated behaviour only if relevant.

## Usage (Rust)

```rust
use bamboo_native_ui::{Label, Rect, Ui, WidgetId};

let mut ui: Ui<Msg> = Ui::new();
ui.add(Label::new("title", "Now playing").wrap(true));
ui.set_rect(WidgetId::new("title"), Rect::from_xywh(16.0, 16.0, 320.0, 24.0));
```

Explain the non-obvious parts of the snippet in one short paragraph.

## Usage (XML)

If the widget has a standard `type="..."` name, show creation:

```xml
<widget id="title" type="label" x="16" y="16" w="320" h="24" text="Now playing"/>
```

If it has no `type`, say so and show how a Rust-created widget is addressed by
id in a `.buixml` document:

```xml
<widget id="title" x="16" y="16" w="320" h="24"/>
```

If it accepts skin attributes or child elements, show them here and list the
accepted names. If it accepts none, state that explicitly.

## Properties

A table of the public builder methods and important getters. Read the source
for defaults.

| Method | Type | Default | Description |
|---|---|---|---|
| `new(id, text)` | `&'static str, impl Into<String>` | — | Creates the widget. |
| `.wrap(bool)` | `bool` | `false` | Wraps onto multiple lines. |

Then a short "Behaviour" note for anything that does not fit the table
(keyboard handling, focus, message timing, accessibility role).

## Events (required section for every widget)

Insert an `## Events` section between `## Properties` and `## XML definition`.
It duplicates the event information readers would otherwise have to hunt for in
the API pages. Use this shape:

````markdown
## Events

### Messages

| Message | Emitted when |
|---|---|
| `Fn(bool) -> M` passed to `new` | The checked state changes. |

### Common observers

This widget installs the shared observer builders. All are additive: the
built-in behaviour still runs after the handler returns.

| Builder | Handler | Fires when |
|---|---|---|
| `.on_click(f)` | `Fn(Point) -> Option<M>` | Left-button click on the widget. |
| `.on_right_click(f)` | `Fn(Point) -> Option<M>` | Right-button click, before the context menu. |
| `.on_double_click(f)` | `Fn(Point) -> Option<M>` | Platform double click. |
| `.on_key_down(f)` | `Fn(KeyEvent) -> Option<M>` | Key press while the widget has focus. |
| `.on_key_up(f)` | `Fn(KeyEvent) -> Option<M>` | Key release while the widget has focus. |
| `.on_focus_gained(f)` | `Fn(FocusEvent) -> Option<M>` | Focus moves onto the widget. |
| `.on_focus_lost(f)` | `Fn(FocusEvent) -> Option<M>` | Focus leaves the widget (after an editor commits). |

If the widget does **not** install the shared observers, say so and name the
mechanism it does use (its constructor message callback, a widget-specific
callback, or `Ui::bind`). Do not invent observer methods.

### Widget-specific callbacks

A table of the callbacks declared by this widget itself (for example
`Slider::on_drag_start`, `TextEdit::on_change`, `Table::on_cell_edit`,
`TreeView::on_rename`, `Tabs::closable`, `Banner::action`). Give the real
signature and when it fires. If there are none, state that.

### Ui triggers

In addition to the builders above, any widget id can be bound with `Ui::bind`:

```rust
ui.bind(WidgetId::new("play"), Trigger::Click, |event, ctx| {
    // Return Some(Msg::...) to queue a message; the built-in action still runs
    // unless ctx.run_default() is called explicitly.
    None
});
```

| Trigger | Fires when |
|---|---|
| `Click` | Press and release on the same widget. |
| `DoubleClick` | The platform reports a double click. |
| `RightClick` | Secondary-button click, before the context menu. |
| `HoverEnter` / `HoverLeave` | The pointer enters / leaves the widget. |
| `Wheel` | A wheel event over the widget (`event.wheel_delta`). |
| `Activate` | Enter/Space on a focused widget, or a UIA Invoke/Toggle. |
| `DragStart` / `DragEnd` | The pointer crosses the drag threshold / the button is released. |
| `ValueChange` | The widget's accessibility value changed (after the built-in update). |
````

If a row is genuinely not meaningful for the widget (for example `Wheel` on a
button), keep the table complete anyway — it is the shared vocabulary — and
add one sentence saying which triggers the widget actually reacts to.

## XML definition

Describe exactly how the widget appears in a `.buixml` document:
- creation (`type`, `command`, `parent`) if supported;
- geometry attributes (normal `widget` rules apply: one of `x+w` / `right+w`
  / `x+right`, and the same vertically);
- supported style tokens (`radius`, `corners`, `background`, `font-size`,
  `font-family`) and widget-specific attributes;
- supported child elements and their attributes.

If the widget is addressed-only, say: "This widget is not created by the
standard registry. Create it in Rust and address it by `id` from the
document."

## See also

- `../controls/button.md`
- `../../api/widget-trait.md`
- `../../guides/xml-layout-authoring.md`
````

## API page template

````markdown
# Events and input

> One-sentence purpose.

## Overview

What the module owns, which feature gates apply, and where it sits in the
runtime (host → `Ui` → widget).

## Key types

### `Event`

```rust
pub enum Event { ... }
```

A table or bullet list of variants and when each is produced.

### `KeyCode`

...signatures and notes...

## Example

```rust
// A small, realistic use of the API.
```

## Notes and limits

Platform-specific behaviour, timing, ownership, and common mistakes.

## See also

- `../widget-trait.md`
- `../../guides/debugging.md`
````

## Quickstart page template

Each quickstart is a standalone, linear tutorial:

1. **What you will build** (one paragraph).
2. **Prerequisites** (Rust toolchain, Windows SDK, MSVC).
3. **Steps** with complete files/commands.
4. **Expected result**.
5. **Troubleshooting** (2–4 common errors).
6. **Next** link.

The core pages are:

| File | Topic |
|---|---|
| `01-create-project.md` | `cargo new`, workspace layout, Windows target |
| `02-add-the-crate.md` | path dependency, feature flags, first build |
| `03-first-window.md` | `Host`, `WindowBuilder`, `Ui`, paint/resize/event |
| `04-custom-widgets.md` | implement `Widget<M>`, `CustomWidget`, skin protocol |
| `05-xml-dynamic.md` | `.buixml` at runtime (`layout-xml`) |
| `06-xml-compiled.md` | codegen CLI and `layout_xml!` (`layout-compiled`) |

The practical pages are independent recipes that reuse the same project:

| File | Topic |
|---|---|
| `07-layout-and-breakpoints.md` | layout helpers, variants, expressions |
| `08-lists-tables-trees.md` | virtualized data views, XML child lists |
| `09-menus-toolbars-status.md` | application chrome, command routing |
| `10-dialogs-tray-hotkeys.md` | native dialogs, tray, notifications, hotkeys |
| `11-theming-and-images.md` | theme/palette, `.buitheme`, image store |
| `12-text-editing-and-ime.md` | native/painted editors, selection, IME |
| `13-player-ui.md` | cover wall, playlist, transport, waveform seek |

## Guide / reference template

- Guides are task-oriented ("how do I theme…"), with steps and trade-offs.
- Reference pages are lookup tables (palette tokens, XML elements, key codes);
  keep prose minimal and make every row accurate.
- Start with a one-line purpose, then the content, then "See also".

## Accuracy checklist (run before you finish)

- [ ] Every Rust identifier exists in the source.
- [ ] Every default in a property table matches the constructor.
- [ ] The XML snippet uses only attributes the parser accepts.
- [ ] Relative links point at files that exist.
- [ ] No `TODO`, `TBD`, `placeholder`, or "coming soon".
- [ ] No preview-image heading.
- [ ] The page has the required sections for its type.
