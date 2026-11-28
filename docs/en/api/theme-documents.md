# Theme documents (`.buitheme`)

> A whole palette as data: the `ThemeDocument` reader, its palette derivation, and the token writer.

## Overview

A `.buitheme` file is a sibling grammar of `.buixml`: one root `<theme>` with a
required light/dark `base`, an optional `accent`, and any number of `<color>`
overrides. The model and reader live in
`bamboo_native_ui_layout_core::theme` and need no Win32 and no `Palette`.
`bamboo-native-ui`'s `theme-xml` feature re-exports them and adds the mapping
onto the window crate's `Palette`.

Applying a theme is the application swapping its palette and repainting. The
layout schema's colour tokens (`background="accent"`, `card`, `text`, …) then
resolve against the new palette on the next `apply_layout`. Derivation is
exactly `Palette::new(settings, base)` followed by the document's overrides, so
a theme may change one token or the whole table.

The `theme-xml` feature only enables `layout-core/parse`; it is independent of
`layout-xml` and `layout-compiled`. A build may load themes without loading
layouts.

## Key types

### `ThemeDocument`

```rust
pub struct ThemeDocument {
    pub version: u32,
    pub name: Option<String>,
    pub base: ThemeBase,
    pub accent: Option<Color>,
    pub colors: Vec<ThemeColor>,
    pub canonical: String,
    pub generation: u64,
}

impl ThemeDocument {
    pub fn parse(text: &str) -> Result<Self, Vec<LayoutDiagnostic>>; // feature "theme-xml"
    pub fn version(&self) -> u32;
    pub fn name(&self) -> Option<&str>;
    pub fn base(&self) -> ThemeBase;
    pub fn accent(&self) -> Option<Color>;
    pub fn colors(&self) -> &[ThemeColor];
    pub fn generation(&self) -> u64;
    pub fn to_xml(&self) -> String;
    pub fn color(&self, name: &str) -> Option<Color>;
}
```

- `version` must be `1`; `name` is a human-readable label kept trimmed in the
  canonical text.
- `canonical` is the normalized document; `generation` is its FNV-1a hash, so
  two saves of one model are byte-identical and the hash changes only with
  content.
- `colors()` enumerates overrides in declaration order; `color(name)` looks one
  up by token.

### `ThemeBase`, `ThemeColor`, `ThemeDiagnostics`

```rust
pub enum ThemeBase { Light, Dark }

impl ThemeBase {
    pub fn is_dark(self) -> bool;
    pub fn name(self) -> &'static str;          // "light" / "dark"
    pub fn from_name(name: &str) -> Option<Self>;
}

pub struct ThemeColor {
    pub name: String,
    pub color: Color,
    pub line: usize,
    pub column: usize,
}

pub type ThemeDiagnostics = Vec<LayoutDiagnostic>;
```

`ThemeColor` is one `<color name="…" value="…"/>` override. `ThemeDiagnostics`
is the error type `parse` returns; the codes are stable and prefixed
`theme.*`.

### Mapping onto `Palette`

```rust
pub trait ThemeDocumentExt {
    fn palette(&self, settings: &ThemeSettings) -> Palette;
}

impl ThemeDocumentExt for ThemeDocument { /* … */ }

pub fn set_palette_token(palette: &mut Palette, name: &str, color: Color) -> bool;
```

`palette(&settings)` clones the settings, replaces their accent with the
document's accent when present, derives `Palette::new(&settings, base.is_dark())`
and then applies every `<color>` override in declaration order.
`set_palette_token` writes one kebab-case palette field and returns whether the
name is a token; the function is total so an application can use it directly.
The token names are the same `PALETTE_TOKENS` table the layout `background`
attribute accepts.

## `.buitheme` syntax

```text
theme              root; version="1" and base required; name and accent optional
└── color*         name="<palette-token>" value="<color literal>"
```

- Attributes are case-sensitive and kebab-case; element order does not matter.
- `base` is required and must be `light` or `dark` (`theme.base.value`). The
  document does not follow the application's current mode.
- `accent` feeds `Palette::new` **before** derivation, so accent-derived tokens
  (`accent-text`, `row-selected`, …) follow it. An explicit
  `<color name="accent">` then sets that one field.
- `value` uses the one colour grammar: `#rgb`, `#rrggbb`, `#rgba`,
  `#rrggbbaa`, `rgb(...)`, `rgba(...)` or a decimal channel list. Alpha is
  accepted and dropped — a palette colour is opaque.
- Overrides are literals, not token references: the document *defines* the
  palette. Duplicate and unknown token names are errors.

The showcase ships `crates/showcase/themes/night.buitheme`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<theme version="1" name="夜蓝" base="dark" accent="#7aa2f7">
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
  <color name="card" value="#22262e"/>
</theme>
```

## Example

```rust
use bamboo_native_ui::theme::ThemeSettings;
use bamboo_native_ui::theme_xml::{ThemeDocument, ThemeDocumentExt};
use bamboo_native_ui::WindowHandle;

fn apply_theme(window: &mut WindowHandle<'_>, text: &str) -> Result<(), String> {
    let document = ThemeDocument::parse(text).map_err(|d| d[0].to_string())?;
    let settings = ThemeSettings::default();
    let palette = document.palette(&settings);
    window.set_theme(settings, palette.dark);
    window.set_palette(palette); // installs the table and repaints once
    Ok(())
}
```

`WindowHandle::set_palette` stores the palette in the window's painter, updates
the native chrome/menus and invalidates the window; `WindowHandle::palette()`
reads it back. Apply the layout afterwards with that palette:

```rust
ui.apply_layout(&layout, &palette, client, sink)?; // colour tokens resolve here
```

## Notes and limits

- Feature gate: `ThemeDocument::parse` and the `theme_xml` module require
  `feature = "theme-xml"`. The model is also reachable through
  `bamboo_native_ui_layout_core::theme`; its reader needs that crate's `parse`
  feature.
- A parse error changes nothing; installing the palette is the application's
  step. The parser caps the file at 1 MiB, 512 colours and 100 diagnostics.
- Base wins over the application's mode: there is no `auto`. Use the light or
  dark document explicitly.
- Accent derivation happens first; overrides happen second, so
  `<color name="accent">` does not re-derive `accent-text`. Set both if you want
  both.
- Codes: `theme.syntax`, `theme.version`, `theme.structure`,
  `theme.unknown-element`, `theme.unknown-attribute`, `theme.missing-attribute`,
  `theme.base.value`, `theme.color.value`, `theme.color.unknown`,
  `theme.color.duplicate`.
- Widgets with `WidgetProps::palette` keep their own palette regardless of the
  window setting.

## See also

- [theme-and-palette.md](theme-and-palette.md) — `ThemeSettings`, `Palette`, presets
- [layout-documents.md](layout-documents.md) — palette tokens used by `background`
- [Theming guide](../guides/theming.md)
- [05-xml-dynamic.md](../quickstart/05-xml-dynamic.md) — loading a layout that uses tokens
