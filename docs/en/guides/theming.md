# Theming

> Change the accent, light/dark mode, fonts, list density and corner radii at
> startup and at runtime, then move the whole palette into a `.buitheme`
> document.

## What a theme is

A theme has three layers:

1. **`ThemeSettings`** — the inputs an application edits: accent, font family,
   base font size, list density.
2. **`Palette`** — the resolved colors a widget paints with. `Palette::new`
   derives every field from the settings and a light/dark decision.
3. **`Painter`** — owns the installed palette and the text styles. The window
   installs it; widgets read it through `WidgetPaint::palette`.

The light/dark decision is the application's. `ThemeSettings` has no mode
field: resolve "follow system" yourself and pass the resulting `bool` to
`Palette::new` or `WindowBuilder::theme`.

## Steps

### 1. Build the settings

```rust
use bamboo_native_ui::{ListDensity, ThemeSettings, ACCENT_PRESETS};

fn settings() -> ThemeSettings {
    ThemeSettings {
        accent: ACCENT_PRESETS[1].1,          // 海蓝
        font_family: Some("Microsoft YaHei UI".to_string()),
        font_size: 15.0,
        list_density: ListDensity::Compact,
    }
}
```

`ThemeSettings::default()` uses the first accent preset (Bamboo green,
`[127, 211, 168]`), `Microsoft YaHei UI` at `13.5` DIPs and
`ListDensity::Comfortable`.

| Field | Default | Used by |
|---|---|---|
| `accent` | `ACCENT_PRESETS[0].1` | `Palette::new` derives hover, press, selection and play colors from it. |
| `font_family` | `Some("Microsoft YaHei UI")` | `Painter` builds the body/small/title/brand styles. |
| `font_size` | `13.5` | `font_scale() = (font_size / 13.5).clamp(0.85, 1.4)`. |
| `list_density` | `ListDensity::Comfortable` | Only through `row_height()`; widgets do not read it implicitly. |

`ListDensity::row_height()` returns `34.0` (`Compact`), `40.0`
(`Comfortable`) or `48.0` (`Spacious`). Pass it to the lists that should follow
the density:

```rust
use bamboo_native_ui::widgets::ListView;
use bamboo_native_ui::{ThemeSettings, Ui};

fn add_list(ui: &mut Ui<()>, settings: &ThemeSettings) {
    ui.add(ListView::new("library").row_height(settings.row_height()));
}
```

Without that call a list keeps its own default (`ListView` and
`PlaylistView` use `44.0`).

### 2. Install the theme at startup

```rust
use bamboo_native_ui::{ThemeSettings, WindowBuilder};

fn theme() -> (ThemeSettings, bool) {
    (ThemeSettings::default(), true)
}

fn main() -> windows::core::Result<()> {
    let (settings, dark) = theme();
    WindowBuilder::new(App::new(settings.clone()))
        .title("Bamboo")
        .theme(settings, dark)
        .run()?;
    Ok(())
}
```

`WindowBuilder::theme(settings, dark)` is applied before the first paint, so
the window, the native menu bar and the owner-drawn chrome agree from frame
one.

### 3. Switch at runtime

`WindowHandle::set_theme(settings, dark)` rebuilds the palette and the text
styles, drops cached fonts when the styles changed, re-themes native menus and
repaints the whole window once.

```rust
use bamboo_native_ui::{ListDensity, ThemeSettings, WindowHandle};

fn toggle_density(&mut self, window: &mut WindowHandle<'_>) {
    self.settings.list_density = match self.settings.list_density {
        ListDensity::Compact => ListDensity::Comfortable,
        ListDensity::Comfortable => ListDensity::Spacious,
        ListDensity::Spacious => ListDensity::Compact,
    };
    window.set_theme(self.settings.clone(), self.dark);
    // Recomputed rectangles and content belong to the application: adjust them
    // and invalidate once (set_theme already repaints the window).
    self.relayout(window.size());
}
```

Switching light/dark is the same call with a different `bool`:

```rust
use bamboo_native_ui::{ThemeSettings, WindowHandle};

fn set_dark(dark: bool, settings: &ThemeSettings, window: &mut WindowHandle<'_>) {
    window.set_theme(settings.clone(), dark);
}
```

If you need the current palette (for a page fill, a custom widget or a layout
apply), read it back with `WindowHandle::palette()`, which returns
`&Palette`.

### 4. Presets and radii

```rust
use bamboo_native_ui::theme::radius;
use bamboo_native_ui::{Color, ACCENT_NAMES, ACCENT_PRESETS};

let labels = ACCENT_NAMES;                  // same order as the presets
let accent = Color::rgb(ACCENT_PRESETS[3].1); // 珊瑚
let _soft = radius::SMALL;                  // 4.0
let _card = radius::MEDIUM;                 // 6.0
let _panel = radius::LARGE;                 // 10.0
```

`radius::pill(height)` returns `height.max(0.0) / 2.0`. The layout schema's
`radius="small|medium|large|pill"` resolves to the same values, so a
`.buitheme` and a `.buixml` file share one corner vocabulary. The names in
`ACCENT_NAMES` ship in Chinese; build your own label array for another
language.

### 5. Move the palette into a `.buitheme` document

Enable the feature:

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["theme-xml"] }
```

`theme-xml` enables the `layout-core` theme reader; it does not require
`layout-xml`, and a build without either feature links no XML parser.

Create `themes/night.buitheme`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<theme version="1" name="Night" base="dark" accent="#7aa2f7">
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
  <color name="card" value="#1d2027"/>
</theme>
```

`base` is required (`light` or `dark`); it decides derivation and ignores the
application's current mode. `accent` is optional and feeds `Palette::new`
before derivation, so the accent-derived fields (`accent-text`,
`row-selected`, button press states, `play_*`) follow it. Each `<color>`
overrides one palette field after derivation, in declaration order. A
`<color name="accent">` sets that one field explicitly without re-deriving the
rest.

### 6. Apply the document

```rust
use bamboo_native_ui::theme_xml::{ThemeDocument, ThemeDocumentExt};
use bamboo_native_ui::{Palette, ThemeSettings, WindowHandle};

fn apply_theme(
    text: &str,
    settings: &ThemeSettings,
    window: &mut WindowHandle<'_>,
) -> Option<(ThemeDocument, Palette)> {
    let document = ThemeDocument::parse(text).ok()?;
    let palette = document.palette(settings);
    window.set_theme(settings.clone(), palette.dark);
    window.set_palette(palette);
    Some((document, palette))
}
```

`palette(settings)` copies the settings, applies the document's accent if any,
calls `Palette::new` with `base`, then applies the overrides. `set_theme` sets
the mode and rebuilds text styles; `set_palette` installs the derived table and
repaints once — do both when a document is active.

If a layout document is installed, re-apply it with the new palette so its
`background` tokens resolve against it:

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;
use bamboo_native_ui::{Palette, Size, Ui, UiHost};

fn reapply(
    ui: &mut Ui<()>,
    document: &LayoutDocument,
    palette: &Palette,
    client: Size,
    sink: &mut impl UiHost,
) {
    if let Err(failure) = ui.apply_layout(document, palette, client, sink) {
        eprintln!("layout rejected: {failure}");
    }
}
```

The palette is `Copy`, so passing it to `apply_layout` does not move it.

### 7. Reload a theme document

Theme documents are plain data; the library does not watch files.

```rust
use bamboo_native_ui::theme_xml::ThemeDocument;
use bamboo_native_ui::{ThemeSettings, WindowHandle};

fn reload_theme(
    text: &str,
    current: Option<&ThemeDocument>,
    settings: &ThemeSettings,
    window: &mut WindowHandle<'_>,
) -> Option<ThemeDocument> {
    let document = ThemeDocument::parse(text).ok()?;
    if current.is_some_and(|old| old.generation() == document.generation()) {
        return None;
    }
    let palette = document.palette(settings);
    window.set_theme(settings.clone(), palette.dark);
    window.set_palette(palette);
    Some(document)
}
```

Poll it from `Host::timer` after installing a timer with
`WindowHandle::set_animation_timer(Some(Duration::from_millis(250)))`. A poll
does not paint unless the generation changes.

### 8. Write one token without a document

```rust
use bamboo_native_ui::theme_xml::set_palette_token;
use bamboo_native_ui::{Color, Palette};

fn recolor(mut palette: Palette) -> Palette {
    let _ = set_palette_token(&mut palette, "accent", Color::from_hex(0x68_8c_ff));
    assert!(set_palette_token(&mut palette, "divider", Color::new(60, 64, 72)));
    palette
}
```

`set_palette_token` returns whether the name is a palette token. The accepted
names are the 31 tokens in
[palette tokens](../reference/palette-tokens.md).

## Trade-offs

| Approach | Gives you | Costs |
|---|---|---|
| `ThemeSettings` only | Accent, fonts, scale, density; no file | Recompilation or application code for new colors |
| `.buitheme` document | Whole palette as data; hot reload | `theme-xml`, a file to ship, one apply step |
| Per-widget `WidgetProps::palette` | One card or panel off-theme | Manual per-widget state; not global |
| Per-widget `background` token | One surface follows the theme | Only the surface fill, not text |

A theme document defines colors only. Fonts, density and radii stay in
`ThemeSettings` and the widget/surface builders.

## Common mistakes

- **Calling `set_palette` and expecting fonts to change** — it never touches
  text styles. Use `set_theme` for mode and fonts.
- **A light `.buitheme` that looks dark** — `base` decides the mode; the
  application's current `dark` flag is not consulted.
- **Overrides not taking effect** — `<color>` names are the palette token
  names (`nav-active`, `accent-text`), not the Rust field names
  (`nav_active`, `accent_text`).
- **An unknown token is silently skipped** — the parser rejects it with
  `theme.color.unknown`; check the parse result instead of ignoring it.
- **The layout keeps the old colors** — the palette is resolved when
  `apply_layout` runs; re-apply after installing a new palette.

## See also

- [Theme and palette API](../api/theme-and-palette.md)
- [Painting and Canvas](../api/painting-and-canvas.md)
- [Palette tokens](../reference/palette-tokens.md)
- [XML elements](../reference/xml-elements.md)
- [XML layout authoring](xml-layout-authoring.md)
- [Host and window](../api/host-and-window.md)
