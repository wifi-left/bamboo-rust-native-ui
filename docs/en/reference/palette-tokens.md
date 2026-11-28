# Palette tokens

> The 31 color names accepted by `background=` in a `.buixml` document, by
> `<color name>` in a `.buitheme` document, and by `set_palette_token`.

## The token table

Token names are kebab-case. They are defined by `PALETTE_TOKENS` in
`bamboo-native-ui-layout-core` and map one-to-one onto the `Palette` struct
fields in `bamboo-native_ui::theme`.

| Token | `Palette` field | Group | Accent-derived |
|---|---|---|---|
| `background` | `background` | Surfaces | |
| `sidebar` | `sidebar` | Surfaces | |
| `player` | `player` | Surfaces | |
| `cover` | `cover` | Surfaces | |
| `card` | `card` | Surfaces | |
| `panel` | `panel` | Surfaces | |
| `search-bg` | `search_bg` | Surfaces | |
| `nav-active` | `nav_active` | Navigation and rows | yes |
| `nav-hover` | `nav_hover` | Navigation and rows | |
| `nav-pressed` | `nav_pressed` | Navigation and rows | yes |
| `row-hover` | `row_hover` | Navigation and rows | |
| `row-pressed` | `row_pressed` | Navigation and rows | yes |
| `row-selected` | `row_selected` | Navigation and rows | yes |
| `text` | `text` | Text | |
| `muted` | `muted` | Text | |
| `accent-text` | `accent_text` | Text | yes |
| `text-disabled` | `text_disabled` | Text | |
| `accent` | `accent` | Accent | input |
| `play-button` | `play_button` | Accent | yes |
| `play-hover` | `play_hover` | Accent | yes |
| `button-bg` | `button_bg` | Buttons | |
| `button-hover` | `button_hover` | Buttons | |
| `button-pressed` | `button_pressed` | Buttons | yes |
| `button-active` | `button_active` | Buttons | yes |
| `button-active-hover` | `button_active_hover` | Buttons | yes |
| `button-disabled` | `button_disabled` | Buttons | |
| `divider` | `divider` | Borders and chrome | |
| `explicit-border` | `explicit_border` | Borders and chrome | |
| `scroll-track` | `scroll_track` | Borders and chrome | |
| `scroll-thumb` | `scroll_thumb` | Borders and chrome | |
| `scroll-thumb-hover` | `scroll_thumb_hover` | Borders and chrome | |

"Accent-derived" means `Palette::new` mixes the accent into the base surface
color. The exact mix depends on light/dark, so a theme should override these
only when it wants to diverge from the accent. `accent` itself is the
`ThemeSettings::accent` input.

`Palette::dark: bool` is not a color and has no token; it follows the
`.buitheme` `base` or the `bool` passed to `Palette::new`.

## Accent presets

`ACCENT_PRESETS` and `ACCENT_NAMES` share this order. The first entry is the
default accent.

| Index | `ACCENT_NAMES` | RGB | Hex |
|---|---|---|---|
| 0 | 竹绿 | `[127, 211, 168]` | `#7fd3a8` |
| 1 | 海蓝 | `[104, 140, 255]` | `#688cff` |
| 2 | 紫罗兰 | `[167, 139, 250]` | `#a78bfa` |
| 3 | 珊瑚 | `[251, 146, 120]` | `#fb9278` |
| 4 | 琥珀 | `[245, 190, 90]` | `#f5be5a` |
| 5 | 青碧 | `[120, 200, 210]` | `#78c8d2` |

`ACCENT_PRESETS` is `[(&str, [u8; 3]); 6]`; the labels are display strings for
the language the presets ship with, not localized by the framework.

## Where the names are accepted

| Context | Form | Notes |
|---|---|---|
| Layout surface | `<widget background="accent"/>` | A token, a color literal, or `none`. |
| Layout local color | `<widget tint="accent"/>`, `color="text"` | Any widget whose local attribute has `SkinValueKind::Color`. |
| Theme document | `<color name="accent-text" value="#7aa2f7"/>` | The token is the `name`; the value must be a literal, not another token. |
| Rust | `set_palette_token(&mut palette, "nav-active", color)` | Returns `false` for an unknown name. |
| Rust | `palette.nav_active` | Direct field access on `Palette`. |

Color literals use one grammar everywhere: `#rgb`, `#rgba`, `#rrggbb`,
`#rrggbbaa`, `r,g,b`, `r,g,b,a`, `rgb(r,g,b)`, `rgba(r,g,b,a)`, or the
functional form with spaces. A surface fill drops alpha; a widget that supports
transparency reads `SkinValue::as_rgba()`.

## `.buitheme` root

| Attribute | Required | Accepted values |
|---|---|---|
| `version` | yes | `1` |
| `base` | yes | `light`, `dark` |
| `name` | no | Any text; kept trimmed in the canonical model. |
| `accent` | no | A color literal; feeds `Palette::new` before derivation. |

A `<color>` child requires both `name` (a token above) and `value` (a literal
color). Duplicates and unknown names are errors. A file with any error changes
nothing.

## `.buitheme` diagnostics

| Code | Severity | Cause |
|---|---|---|
| `theme.syntax` | error | Malformed XML or file over 1 MiB. |
| `theme.version` | error | Missing or unsupported `version`. |
| `theme.structure` | error | Wrong nesting, stray text, DTD, missing root. |
| `theme.unknown-element` | error | Element other than `<theme>`/`<color>`. |
| `theme.unknown-attribute` | error | Attribute outside the schema. |
| `theme.missing-attribute` | error | Required attribute absent. |
| `theme.base.value` | error | `base` is not `light`/`dark`. |
| `theme.color.value` | error | A color value is not a valid literal. |
| `theme.color.unknown` | error | `<color name>` is not a palette token. |
| `theme.color.duplicate` | error | The same token is declared twice. |

At most 512 `<color>` overrides and 100 diagnostics are accepted per document.

## See also

- [Theme and palette API](../api/theme-and-palette.md)
- [Theming guide](../guides/theming.md)
- [XML elements](xml-elements.md)
- [Geometry and color](../api/geometry-and-color.md)
