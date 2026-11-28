# Properties and narration

> Per-instance presentation and accessibility overrides: `WidgetProps`, `FontSpec`, `Narration` and `ContentCaps`.

## Overview

Widgets are built once and reused, so details that differ between two instances
of the same type do not live in the widget struct. `bamboo_native_ui::props`
owns them:

- `WidgetProps` — visibility, enabled, focusability, Tab weight, cursor,
  hit-testing, read-only, tooltips, context menu, automation id, palette, font,
  accessibility role, narration and accessibility exposure.
- `FontSpec` — the per-widget UI font override.
- `Narration` — application-owned accessible name, description and per-row
  formatter.
- `ContentCaps` — which content setters a widget accepts (published by the
  widget, read by the layout applier).

Apply props at creation with `Ui::add_with(widget, props)` or at runtime with
`Ui::set_props(id, props, sink)`. Every field is optional: an unset field keeps
the widget's own default, so existing pages narrate and paint exactly as before.

Props are not layout. Margins, grow and alignment stay application-computed
(the framework has no measure pass); props carry only per-instance state the
container can honor without rebuilding the tree.

## Key types

### `WidgetProps`

```rust
pub struct WidgetProps { /* all fields private; every setter is optional */ }

impl WidgetProps {
    pub fn new() -> Self;
    pub fn visible(self, visible: bool) -> Self;
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn focusable(self, focusable: bool) -> Self;
    pub fn focus_order(self, weight: i32) -> Self;
    pub fn cursor(self, cursor: Cursor) -> Self;
    pub fn mouse(self, interactive: bool) -> Self;
    pub fn read_only(self, read_only: bool) -> Self;
    pub fn tooltip(self, text: impl Into<String>) -> Self;
    pub fn tooltip_at(self, index: usize, text: impl Into<String>) -> Self;
    pub fn context_menu(self, menu: Menu) -> Self;
    pub fn automation_id(self, id: impl Into<String>) -> Self;
    pub fn palette(self, palette: Palette) -> Self;
    pub fn font(self, spec: FontSpec) -> Self;
    pub fn font_size(self, size: f32) -> Self;
    pub fn font_family(self, family: impl Into<String>) -> Self;
    pub fn role(self, role: A11yRole) -> Self;
    pub fn narrate(self, narration: Narration) -> Self;
    pub fn name(self, name: impl Into<String>) -> Self;
    pub fn describe(self, description: impl Into<String>) -> Self;
    pub fn expose(self, exposed: bool) -> Self;
}
```

| Builder | Reaches | Notes |
|---|---|---|
| `visible` | container visibility | Structural: input, Tab, a11y and scroll bars all follow it. |
| `enabled` | widget input | A disabled widget dismisses pointer state. |
| `focusable` | focus policy | Overrides the widget type's default. |
| `focus_order` | Tab order | Smaller weights first; equal weights keep insertion order. Default `0`. |
| `cursor` | pointer | Shape while hovering. |
| `mouse` | hit-testing | Display widgets default to pass-through; `mouse(true)` makes a card/toast clickable and enables trigger bindings. |
| `read_only` | editors + a11y | No native editing session; ORed with the widget's own `read_only`. |
| `tooltip` / `tooltip_at` | host tooltip | `tooltip_at` targets one row/option; an empty string suppresses that item. |
| `context_menu` | host menu | Right click, Menu key or Shift+F10. |
| `automation_id` | UIA | Defaults to the widget id. |
| `palette` | painting | This widget paints with a different palette. |
| `font`/`font_size`/`font_family` | UI text | See `FontSpec`; icon glyphs keep their own face. |
| `role` | a11y | Overrides the role derived from the widget type. |
| `narrate`/`name`/`describe` | a11y | Narration only; never changes painted text. |
| `expose` | a11y tree | Display-only widgets are out even when focusable. |

### `FontSpec`

```rust
pub struct FontSpec { /* private */ }

impl FontSpec {
    pub fn new() -> Self;
    pub fn size(self, size: f32) -> Self;          // absolute DIPs, floored at 1.0
    pub fn family(self, family: impl Into<String>) -> Self;
    pub fn is_empty(&self) -> bool;
    pub fn apply(&self, base: &TextStyle) -> TextStyle;
}
```

`apply` merges onto the painter's role style: the size replaces the scale and
the family replaces the face. A glyph face (`FontFace::Glyph`) is returned
untouched, so icons never turn into tofu. `WidgetProps::font(FontSpec::new())`
drops an override rather than storing an empty one.

### `Narration`

```rust
pub struct Narration { /* private */ }

impl Narration {
    pub fn new() -> Self;
    pub fn name(self, name: impl Into<String>) -> Self;
    pub fn name_with(self, format: impl Fn(&A11yInfo) -> String + 'static) -> Self;
    pub fn description(self, description: impl Into<String>) -> Self;
    pub fn description_with(self, format: impl Fn(&A11yInfo) -> String + 'static) -> Self;
    pub fn row(self, format: impl Fn(usize, &A11yChild) -> String + 'static) -> Self;
}
```

Fixed vs lazy: `name`/`description` store a string; `name_with`/
`description_with` build it from the widget's current `A11yInfo` on every
snapshot, so a name can follow playback or a count without rebuilding the tree.
`row` renames every published child of a composite control; the index is the
widget-level index (a scrolled list row keeps its absolute index).

Narration feeds the same `A11yInfo` a screen reader, `--a11y-dump` and
`--self-test` observe, so all three see one value.

### `ContentCaps`

```rust
pub struct ContentCaps(u16);

impl ContentCaps {
    pub const NONE: Self;
    pub const TEXT: Self;       // Widget::set_text
    pub const LABEL: Self;      // Widget::set_label
    pub const VALUE: Self;      // Widget::set_value
    pub const PROGRESS: Self;   // Widget::set_progress
    pub const NUMBER: Self;     // Widget::set_number
    pub const TOGGLED: Self;    // Widget::set_toggle
    pub const SELECTION: Self;  // Widget::set_selected

    pub const fn union(self, other: Self) -> Self;
    pub const fn contains(self, other: Self) -> bool;
    pub const fn is_empty(self) -> bool;
    pub const fn bits(self) -> u16;
}
```

A widget publishes its set through `Widget::content_caps`. The layout applier
consults it and warns `xml.content.unsupported` instead of silently writing a
value the widget cannot take.

## How props reach the runtime

- **Paint** — `palette` replaces the palette handed to the widget; the font
  override merges into `TextStyle`; visibility/enabled change what the
  container asks the widget to paint and hit-test.
- **Accessibility** — `role` overrides the derived role; `narrate`/`name`/
  `describe` replace the derived name and description; `expose` forces the
  widget in or out of the tree; all of it lands in `A11yInfo`.
- **Tooltips** — the host resolves `tooltip_for(item)` after its dwell: the
  per-item entry wins (an empty entry suppresses), otherwise the widget default.
- **Menus** — `context_menu` is a static `Menu`; a menu that reflects live state
  is rebuilt by the application.

## Example

```rust
use bamboo_native_ui::widgets::Card;
use bamboo_native_ui::{A11yRole, FontSpec, Narration, Ui, WidgetProps};

enum Msg {
    OpenAlbum,
}

fn add_card(ui: &mut Ui<Msg>) {
    ui.add_with(
        Card::new("details", "Album", "Night pieces"),
        WidgetProps::new()
            .mouse(true)
            .focus_order(2)
            .tooltip("Opens the album")
            .font(FontSpec::new().size(15.0))
            .role(A11yRole::Group)
            .narrate(Narration::new().name("Album details")),
    );
}
```

At runtime the same fields go through `set_props`; making the focused widget
invisible or non-focusable relocates focus and repaints both rectangles.

## Notes and limits

- Feature gate: `props` (and `creation`) compile under every feature
  combination; only applying a document needs `layout-xml`/`layout-compiled`.
- `Ui::add_with` records focus/visibility/enabled before the first layout, so it
  never needs focus relocation. `Ui::set_props` runs on a live page and does.
- Narration replaces the accessible strings only; the painted label/text is
  content, set through the widget's own setter or the layout document.
- `tooltip_at` indices are absolute list rows / segmented options / tabs, the
  same indices the accessibility tree publishes.
- `FontSpec` affects UI text, not icon glyphs. A size is absolute DIPs on top of
  the theme scale; the backend applies the DPI.
- Layout-document property attributes map onto the same fields: `visible`,
  `enabled`, `interactive`, `focusable`, `focus-order`, `read-only`, `tooltip`,
  `name`, `description`, `automation-id`, `role`, `expose`, `cursor`, plus
  `font-size`/`font-family`. See [layout-documents.md](layout-documents.md).
- In a document, a `name`/`description` is narration; `label`/`text` is content.

## See also

- [accessibility.md](accessibility.md) — `A11yInfo`, roles, the provider tree
- [widget-trait.md](widget-trait.md) — `content_caps` and the skin protocol
- [layout-documents.md](layout-documents.md) — the XML attribute vocabulary
- [Theming guide](../guides/theming.md)
