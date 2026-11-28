# 11 — Theming and images

> Switch light and dark, cycle accent presets, load a `.buitheme` palette and
> feed application-owned gradients to avatars and cover walls.

## What you will build

A window with three buttons — **Light / dark**, **Next accent** and **Night
theme** — an avatar and a cover wall. A button re-derives the whole palette,
installs it on the window and repaints; widgets follow because they read colours
from the palette at paint time. The Night theme is a `.buitheme` document.

The images are independent: the page registers two RGBA images with
`Ui::set_image` and hands their numeric `ImageId` to `Avatar` and `CoverWall`.
No image files are involved.

## Prerequisites

- The project from [03](03-first-window.md) builds and runs.
- For steps 4 and 5, enable the theme parser:

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["theme-xml"] }
```

`theme-xml` adds `ThemeDocument::parse` and `ThemeDocumentExt::palette`; it is
independent of `layout-xml`.

## Steps

### 1. Keep the theme inputs in application state

`ThemeSettings` is a plain value. The defaults are the Bamboo green accent,
`Microsoft YaHei UI`, a 13.5 DIP base size and comfortable rows.

```rust
use bamboo_native_ui::widgets::{Avatar, Cover, CoverWall};
use bamboo_native_ui::{
    ACCENT_NAMES, ACCENT_PRESETS, Color, ImageId, ListDensity, Palette, ThemeSettings, Ui,
    UiImage, WidgetId, WindowHandle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    ToggleDark,
    NextAccent,
    NightTheme,
}

struct App {
    ui: Ui<Msg>,
    settings: ThemeSettings,
    dark: bool,
}
```

`new` adds a label and three buttons, sets `dark: true` and
`settings.list_density = ListDensity::Compact`. `ThemeSettings` is
`{ accent, font_family, font_size, list_density }`; row heights are 34, 40 and
48 DIPs.

### 2. Derive and install a palette

`Palette::new(&settings, dark)` derives every token from the accent and the
mode. `set_theme` stores the settings, rebuilds fonts and menu colours and
invalidates; `set_palette` installs an already-derived table.

```rust
impl App {
    fn apply_theme(&mut self, window: &mut WindowHandle<'_>) {
        let palette = Palette::new(&self.settings, self.dark);
        window.set_theme(self.settings.clone(), self.dark);
        window.set_palette(palette);
    }
}
```

`paint` is page 03's: fill each dirty rect with
`ctx.painter.palette().background`, then call `ui.paint(ctx)`. Each setter
invalidates the window.

### 3. Toggle light/dark and cycle accents

`ACCENT_PRESETS` is six `(label, [u8; 3])` pairs; `ACCENT_NAMES` repeats the
labels. The first entry is the default green.

```rust
impl App {
    fn apply(&mut self, message: Msg, window: &mut WindowHandle<'_>) {
        match message {
            Msg::ToggleDark => {
                self.dark = !self.dark;
                self.apply_theme(window);
            }
            Msg::NextAccent => {
                let current = ACCENT_PRESETS.iter()
                    .position(|(_, rgb)| *rgb == self.settings.accent)
                    .unwrap_or(0);
                let next = (current + 1) % ACCENT_PRESETS.len();
                self.settings.accent = ACCENT_PRESETS[next].1;
                self.ui.set_label(WidgetId::new("accent"), ACCENT_NAMES[next]);
                self.apply_theme(window);
            }
            Msg::NightTheme => self.apply_night(window),
        }
    }
}
```

The preset labels are the Chinese names the settings page shows; the RGB value
is what matters to the palette. Any other `[u8; 3]` works as an accent.

### 4. Load a `.buitheme` document

Create `bamboo-hello/themes/night.buitheme`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<theme version="1" name="Night Blue" base="dark" accent="#7aa2f7">
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
  <color name="card" value="#22262e"/>
</theme>
```

`base` is required. `accent` replaces the settings accent before derivation;
each `<color>` overrides one token. Derivation is `Palette::new` plus overrides.

```rust
use bamboo_native_ui::theme_xml::{ThemeDocument, ThemeDocumentExt};

impl App {
    fn apply_night(&mut self, window: &mut WindowHandle<'_>) {
        let text = include_str!("../themes/night.buitheme");
        let Ok(document) = ThemeDocument::parse(text) else {
            return; // a bad document leaves the current palette alone
        };
        let palette = document.palette(&self.settings);
        self.dark = palette.dark;
        self.settings.accent = palette.accent.to_rgb();
        window.set_theme(self.settings.clone(), self.dark);
        window.set_palette(palette);
    }
}
```

`palette()` comes from `ThemeDocumentExt`, so the trait must be in scope. Keep
the old palette when `parse` returns diagnostics.

### 5. Override a single token

Token names are kebab-case, as in the layout schema: `background`, `nav-active`, `button-hover`, …

```rust
use bamboo_native_ui::theme_xml::set_palette_token;

impl App {
    fn tint_card(&self, window: &mut WindowHandle<'_>) {
        let mut palette = Palette::new(&self.settings, self.dark);
        if set_palette_token(&mut palette, "card", Color::from_hex(0x23262e)) {
            window.set_palette(palette);
        }
    }
}
```

`set_palette_token` returns `false` for an unknown name and writes nothing.

### 6. Register images and reference them by id

An image is RGBA pixels the application owns. `UiImage::solid` fills one colour;
`UiImage::gradient` interpolates top to bottom. `UiImage::new` takes real
decoded pixels.

```rust
impl App {
    fn register_images(ui: &mut Ui<Msg>) {
        ui.set_image(UiImage::solid(ImageId(1), 96, 96, Color::from_hex(0x4C6EF5)));
        ui.set_image(UiImage::gradient(
            ImageId(10), 160, 160,
            Color::from_hex(0x5B6BA8), Color::from_hex(0x222B4C),
        ));
    }
}
```

Call `register_images` from `new` before adding widgets. `Avatar::image`
centre-crops to the circle; `Cover::image` falls back to a tinted tile.

```rust
ui.add(Avatar::new("me", "Bamboo Fan").image(ImageId(1)));
ui.add(
    CoverWall::new("wall")
        .covers(vec![
            Cover::new("Nocturne").subtitle("Jay Chou").image(ImageId(10)),
            Cover::new("Morning Mist").subtitle("Lin Xiangwan"),
        ])
        .tile(148.0).gap(14.0).corner_radius(6.0),
);
```

The page owns the store: `UiImage` holds one `Arc<[u8]>`, so a cover shared by a
wall, an avatar and a playing card costs one copy. Registering another image
under the same `ImageId` replaces the pixels for every reference. `set_image`
does not invalidate, so repaint the widgets that show it.

A document uses the same numeric ids; there is no image-name registry. Register
the pixels in Rust before applying the layout.

```xml
<widget id="wall" x="16" y="120" w="360" h="320">
  <cover title="Nocturne" subtitle="Jay Chou" image="10"/>
  <cover title="Morning Mist" subtitle="Lin Xiangwan"/>
</widget>
```

`image` is parsed as a number and becomes `ImageId(10)`. XML cannot load a file
or name an image; an unregistered id paints the fallback tile.

### 7. Run it

`cargo run`. **Next accent** shifts every accent-derived colour; light/dark
switches the page, menus and dialogs; **Night theme** applies the document.

## Expected result

- Light/dark switches the page, the owner-drawn menu bar and the tooltips.
- Accent cycling recolours controls without a restart.
- The Night document loads and leaves tokens it does not override intact.
- The avatar shows the solid image; the first cover shows the gradient and the
  second the fallback tile.

## Troubleshooting

- **Nothing changes after editing `self.settings`** — `ThemeSettings` is a
  value. Call `set_theme` and `set_palette`.
- **A `.buixml` page keeps the old colours** — tokens resolve when the layout is
  applied. Re-apply it with the new palette.
- **Covers are blank or all fallback tiles** — the `ImageId` was never registered;
  ids are plain numbers, not filenames.
- **A replaced image still shows old pixels** — `set_image` does not invalidate.
  Repaint the covering rectangles.
- **`ThemeDocument::parse` fails** — `version="1"` is required, `base` must be
  `light` or `dark`, and an unknown `<color>` name is rejected.

## Next

[12 — Text editing and IME](12-text-editing-and-ime.md): native and painted text
fields, number and search inputs, and Chinese/Japanese input.
