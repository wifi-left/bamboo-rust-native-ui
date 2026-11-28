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
- For steps 4 and 5, enable the theme parser. Edit the crate manifest
  (`bamboo-hello/bamboo-hello/Cargo.toml`):

```toml
[dependencies]
# `theme-xml` adds `.buitheme` parsing; `layout-xml` is not required.
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["theme-xml"] }
```

`theme-xml` adds `ThemeDocument::parse` and `ThemeDocumentExt::palette`; it is
independent of `layout-xml`.

## New terms on this page

- **Theme settings** — the few inputs the app chooses: accent colour, font
  family, base font size and list density. `ThemeSettings` is a plain value.
- **Palette** — the full table of colours widgets read while painting
  (`background`, `text`, `button-hover`, ...). It is *derived* from the settings
  and the light/dark mode, not written by hand.
- **Palette token** — one named entry in that table. Overriding a token changes
  one colour and leaves the rest of the derivation intact.
- **Light/dark** — the mode used during derivation; dark mode darkens surfaces
  and light mode brightens them.
- **Accent** — the one brand colour the other accent-derived tokens (selection,
  focus, active tabs) are shaded from.
- **`.buitheme`** — Bamboo's small XML theme document: a base mode, an optional
  accent and a list of token overrides.
- **Image store** — the `Ui`'s table of pixels. `ImageId` is the number used to
  refer to one entry; it is not a file name.
- **RGBA** — four bytes per pixel: red, green, blue and alpha (opacity).
- **Gradient** — a smooth blend between two colours; `UiImage::gradient`
  interpolates from the top colour to the bottom one.

All Rust on this page goes into the crate's `src/main.rs`
(`bamboo-hello/bamboo-hello/src/main.rs`).

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
    ToggleDark, // light <-> dark
    NextAccent, // step through the six presets
    NightTheme, // apply the .buitheme document
}

struct App {
    ui: Ui<Msg>,
    settings: ThemeSettings, // inputs: accent, font, size, density
    dark: bool,              // mode passed to palette derivation
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
        // Rebuild every colour from the settings and the mode.
        let palette = Palette::new(&self.settings, self.dark);
        // Settings drive fonts and menu colours; the palette drives widgets.
        window.set_theme(self.settings.clone(), self.dark);
        window.set_palette(palette);
    }
}
```

`paint` is page 03's: fill each dirty rect with
`ctx.painter.palette().background`, then call `ui.paint(ctx)`. Each setter
invalidates the window.

**What just happened** — `apply_theme` read the current settings and mode,
derived a complete colour table, stored the settings on the window and installed
the table. Widgets did not need to be told about colours individually: they look
them up from the palette when they repaint, which the two setters requested.

### 3. Toggle light/dark and cycle accents

`ACCENT_PRESETS` is six `(label, [u8; 3])` pairs; `ACCENT_NAMES` repeats the
labels. The first entry is the default green.

```rust
impl App {
    fn apply(&mut self, message: Msg, window: &mut WindowHandle<'_>) {
        match message {
            Msg::ToggleDark => {
                self.dark = !self.dark;
                self.apply_theme(window); // re-derive and repaint
            }
            Msg::NextAccent => {
                // Find the current preset, then wrap to the next one.
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

Create `bamboo-hello/themes/night.buitheme` — in the crate folder, beside
`src/`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<!-- `base` is required; `accent` replaces the settings accent before derivation. -->
<theme version="1" name="Night Blue" base="dark" accent="#7aa2f7">
  <!-- Each color overrides one palette token after derivation. -->
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
        // Embedded at compile time; the path is relative to `src/main.rs`.
        let text = include_str!("../themes/night.buitheme");
        let Ok(document) = ThemeDocument::parse(text) else {
            return; // a bad document leaves the current palette alone
        };
        // Derive from the settings, then apply the document's overrides.
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

**What just happened** — clicking **Night theme** parsed the embedded XML,
derived a palette with the document's dark base and blue accent, and overlaid
the three `<color>` entries. The app then copied the resulting mode and accent
back into its own state so the next toggle starts from what is on screen; the
untouched tokens kept their derived values.

### 5. Override a single token

Token names are kebab-case, as in the layout schema: `background`, `nav-active`, `button-hover`, …

```rust
use bamboo_native_ui::theme_xml::set_palette_token;

impl App {
    fn tint_card(&self, window: &mut WindowHandle<'_>) {
        // Start from the derived palette, then replace one token.
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
        // A solid 96x96 image: 8 bits each for red, green, blue, alpha.
        ui.set_image(UiImage::solid(ImageId(1), 96, 96, Color::from_hex(0x4C6EF5)));
        // A 160x160 image blending from the top colour to the bottom one.
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
ui.add(Avatar::new("me", "Bamboo Fan").image(ImageId(1))); // centre-crops to a circle
ui.add(
    CoverWall::new("wall")
        .covers(vec![
            // First cover uses the gradient registered as ImageId(10).
            Cover::new("Nocturne").subtitle("Jay Chou").image(ImageId(10)),
            // No image: the cover paints its fallback tile instead.
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
  <!-- `image` is a number, not a file name: it must match an ImageId. -->
  <cover title="Nocturne" subtitle="Jay Chou" image="10"/>
  <cover title="Morning Mist" subtitle="Lin Xiangwan"/>
</widget>
```

`image` is parsed as a number and becomes `ImageId(10)`. XML cannot load a file
or name an image; an unregistered id paints the fallback tile.

### 7. Run it

Run from the crate folder (`bamboo-hello/bamboo-hello`, the one beside
`src/main.rs`).

```powershell
# Working directory: bamboo-hello/bamboo-hello
cargo run
```

**Next accent** shifts every accent-derived colour; light/dark switches the page,
menus and dialogs; **Night theme** applies the document.

If the build stops at `include_str!`, the `.buitheme` file is not at
`themes/night.buitheme` next to `src/`; the compiler prints the path it tried.

## Expected result

The window opens with the page's initial dark mode and green accent. From there:

- Light/dark switches the page, the owner-drawn menu bar and the tooltips.
- Accent cycling recolours controls without a restart.
- The Night document loads and leaves tokens it does not override intact.
- The avatar shows the solid image; the first cover shows the gradient and the
  second the fallback tile.

## Troubleshooting

- **Nothing changes after editing `self.settings`** — `ThemeSettings` is a
  value. Call `set_theme` and `set_palette`.
- **The page still looks dark after clicking Light/dark** — only the palette was
  replaced. `set_theme` also refreshes fonts and menu colours; call both, as
  `apply_theme` does.
- **An accent button changes the label but not the colours** — the palette was
  not re-derived. Change `settings.accent`, then call `Palette::new` and
  `set_palette` again.
- **A `.buixml` page keeps the old colours** — tokens resolve when the layout is
  applied. Re-apply it with the new palette.
- **`include_str!` fails to find `themes/night.buitheme`** — the path is
  resolved relative to the source file. Put `themes/` beside `src/`, not beside
  the workspace manifest.
- **Covers are blank or all fallback tiles** — the `ImageId` was never registered;
  ids are plain numbers, not filenames.
- **The avatar is a plain circle** — `Avatar::image` received an id that was not
  registered with `Ui::set_image`; an unknown id paints no image.
- **A replaced image still shows old pixels** — `set_image` does not invalidate.
  Repaint the covering rectangles.
- **Text uses an unexpected font** — the family in `ThemeSettings` is not
  installed, so Windows substitutes another. Pick an installed family or keep
  the default.
- **`ThemeDocument::parse` fails** — `version="1"` is required, `base` must be
  `light` or `dark`, and an unknown `<color>` name is rejected.

## Next

[12 — Text editing and IME](12-text-editing-and-ime.md): native and painted text
fields, number and search inputs, and Chinese/Japanese input.
