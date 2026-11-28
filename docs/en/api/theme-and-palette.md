# Theme and palette

> Runtime theme settings, the resolved color palette, accent presets, list density and named radii.

## Overview

`bamboo_native_ui::theme` owns what an application changes to restyle the UI at
runtime: `ThemeSettings` (inputs), `Palette` (resolved colors) and
`ListDensity`. It also re-exports the layout-core `radius` module so widgets
share one set of named corner radii.

A palette is not a widget-owned singleton: the window builds a `Palette` from
`ThemeSettings` plus the resolved light/dark decision, installs it into the
`Painter`, and each widget paints with the palette it receives
(`WidgetPaint::palette`). `WidgetProps::palette` overrides it for one widget.

The `.buitheme` document format is feature-gated: with
`feature = "theme-xml"` the `theme_xml` module re-exports the `ThemeDocument`
reader and provides `ThemeDocumentExt::palette(&settings)` — `Palette::new`
followed by the document's overrides — plus `set_palette_token`. Layout
features stay independent of it.

## Key types

### `ThemeSettings` and `ListDensity`

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeSettings {
    pub accent: [u8; 3],
    pub font_family: Option<String>,
    /// UI base size in DIPs; 13.5 is the reference.
    pub font_size: f32,
    pub list_density: ListDensity,
}

impl Default for ThemeSettings { /* see the table */ }

impl ThemeSettings {
    pub fn font_scale(&self) -> f32;   // (font_size / 13.5).clamp(0.85, 1.4)
    pub fn row_height(&self) -> f32;   // 34 / 40 / 48 by density
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListDensity {
    Compact,
    #[default]
    Comfortable,
    Spacious,
}
```

| Field | Default | Meaning |
|---|---|---|
| `accent` | `ACCENT_PRESETS[0].1` = `[127, 211, 168]` | RGB accent; accent-derived palette fields follow it. |
| `font_family` | `Some("Microsoft YaHei UI")` | UI family; an empty string falls back to the theme face. |
| `font_size` | `13.5` | Base UI size in DIPs. |
| `list_density` | `ListDensity::Comfortable` | `Compact` 34.0, `Comfortable` 40.0, `Spacious` 48.0 DIP rows. |

### `Palette`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub dark: bool,
    pub background: Color, pub sidebar: Color, pub player: Color, pub cover: Color,
    pub nav_active: Color, pub accent: Color, pub text: Color, pub muted: Color,
    pub divider: Color, pub row_selected: Color, pub scroll_track: Color,
    pub scroll_thumb: Color, pub accent_text: Color, pub search_bg: Color,
    pub button_bg: Color, pub button_hover: Color, pub button_pressed: Color,
    pub button_active: Color, pub button_active_hover: Color,
    pub explicit_border: Color, pub nav_hover: Color, pub row_hover: Color,
    pub nav_pressed: Color, pub row_pressed: Color, pub scroll_thumb_hover: Color,
    pub card: Color, pub panel: Color, pub button_disabled: Color,
    pub text_disabled: Color, pub play_button: Color, pub play_hover: Color,
}

impl Palette {
    /// `dark` is the resolved light/dark decision ("follow system" is resolved
    /// by the caller).
    pub fn new(settings: &ThemeSettings, dark: bool) -> Self;
}
```

| Group | Fields |
|---|---|
| Surfaces | `background`, `sidebar`, `player`, `cover`, `card`, `panel`, `search_bg` |
| Navigation and rows | `nav_active`, `nav_hover`, `nav_pressed`, `row_hover`, `row_pressed`, `row_selected` |
| Text | `text`, `muted`, `accent_text`, `text_disabled` |
| Accent | `accent`, `play_button`, `play_hover` |
| Buttons | `button_bg`, `button_hover`, `button_pressed`, `button_active`, `button_active_hover`, `button_disabled` |
| Borders and chrome | `divider`, `explicit_border`, `scroll_track`, `scroll_thumb`, `scroll_thumb_hover` |
| Mode | `dark` |

The accent-derived fields (`nav_active`, `row_selected`, `button_pressed`,
`button_active*`, `nav_pressed`, `row_pressed`, `play_*`, `accent_text`) are
mixed from the accent at construction, so rebuilding the palette from new
settings also restyles hover and press states consistently.

### Accent presets and radii

```rust
pub const ACCENT_PRESETS: [(&str, [u8; 3]); 6] = [
    ("竹绿", [127, 211, 168]), ("海蓝", [104, 140, 255]),
    ("紫罗兰", [167, 139, 250]), ("珊瑚", [251, 146, 120]),
    ("琥珀", [245, 190, 90]), ("青碧", [120, 200, 210]),
];
pub const ACCENT_NAMES: [&str; 6] = ["竹绿", "海蓝", "紫罗兰", "珊瑚", "琥珀", "青碧"];

// bamboo_native_ui::theme::radius
pub const SMALL: f32 = 4.0;
pub const MEDIUM: f32 = 6.0;
pub const LARGE: f32 = 10.0;
pub fn pill(height: f32) -> f32;   // height.max(0.0) / 2.0
```

The first preset is the historical Bamboo green and the default accent.
`ACCENT_NAMES` is a label array in the same order, for a segmented picker. The
radius constants are the values the layout schema's `radius` token resolves
to; `pill` is a helper, not a token.

## Example

```rust
use bamboo_native_ui::theme::{radius, ACCENT_PRESETS, Palette, ThemeSettings};

fn install_theme() -> Palette {
    let settings = ThemeSettings {
        accent: ACCENT_PRESETS[1].1,
        font_size: 15.0,
        ..ThemeSettings::default()
    };
    assert_eq!(settings.row_height(), 40.0);
    assert!(settings.font_scale() > 1.0);
    let _corner = radius::MEDIUM;

    let palette = Palette::new(&settings, true);
    assert!(palette.dark);
    assert_eq!(palette.accent, bamboo_native_ui::Color::rgb(settings.accent));
    palette
}
```

`Palette::new` only derives colors; fonts and text styles come from
`Painter::set_theme`, which returns whether the text styles changed so the
backend can drop cached fonts (see
[Painting and Canvas](./painting-and-canvas.md)).

## Notes and limits

- `Palette` is `Copy`; pass it by value. Widgets read `WidgetPaint::palette`,
  and `WidgetProps::palette` overrides it per widget.
- The light/dark decision is the caller's: `ThemeSettings` does not read the
  system theme, so "follow system" is resolved before `Palette::new`.
- `ACCENT_NAMES` are display strings in the language the presets ship with
  (Chinese); use your own labels for a differently localized app.
- `font_scale` clamps to `0.85..=1.4`, so very small or large `font_size`
  values do not scale linearly.
- `.buitheme` parsing and its palette mapping need `feature = "theme-xml"`;
  without it `Palette::new` still works and no XML reader is linked.
- Radius constants are DIPs and are additionally clamped to half the shorter
  side at paint time, so an oversized constant becomes a pill, not an error.

## See also

- [Geometry and color](./geometry-and-color.md)
- [Painting and Canvas](./painting-and-canvas.md)
- [The Ui container](./ui-container.md)
- [The widget trait](./widget-trait.md)
- [Button](../widgets/controls/button.md)
- [Theming guide](../guides/theming.md)
