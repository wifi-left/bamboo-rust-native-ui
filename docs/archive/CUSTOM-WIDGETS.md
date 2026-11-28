# Custom widgets in a layout skin

> Status: implemented (D127/D137, built-ins in M7.3/D139, child lists in
> M7.4/D140). `crate::skin` is compiled unconditionally; only reading a
> `.buixml` file needs the `layout-xml` feature. This page is the user guide;
> `docs/M7-XML.md` is the schema contract.

A layout document (`.buixml`) never creates widgets. It moves, styles and seeds
widget ids the application already created. That is deliberate: the app owns
the widget tree, and the file is data.

For a widget outside the library — your own `Widget<M>` implementation — the
library knows none of its vocabulary. You do **not** patch the parser or add a
case to a central `match`. Instead, your widget declares the extra attributes
and child elements it accepts, and the applier validates and writes exactly
that set.

## 1. The six methods

Every method has an empty default on `Widget<M>`, so existing widgets are
unaffected:

```rust
fn skin_attrs(&self) -> &'static [SkinAttrDecl] { &[] }
fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool { true }
fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {}

fn skin_children(&self) -> &'static [&'static str] { &[] }
fn validate_skin_children(&self, children: &[SkinElement]) -> bool { true }
fn apply_skin_children(&mut self, children: &[SkinElement]) {}
```

Contract:

- `skin_attrs` / `skin_children` are the source of truth. The writer accepts
  exactly these names; the debug form will print them.
- `validate_*` must be **side-effect free**. The applier calls every validator
  before it mutates anything, so one bad value rejects the whole document and
  the previous UI stays intact.
- `apply_*` must be **silent**: no callbacks, no creating/removing/moving
  widgets, no focus or Tab-order changes. Write your fields and return.
- Local data is **content**: written only when the document generation changes,
  so a resize never clobbers it and a reload does. `Ui::clear_layout` does not
  restore it.

## 2. Names and value kinds

| Kind | Parsed value | Accepts |
|---|---|---|
| `SkinValueKind::Text` | `SkinValue::Text(&str)` | any text; your validator interprets it |
| `SkinValueKind::Number` | `SkinValue::Number(f64)` | a finite decimal |
| `SkinValueKind::Bool` | `SkinValue::Bool(bool)` | `true`/`false`/`1`/`0` |
| `SkinValueKind::Color` | `SkinValue::Color(Rgba)` | `#rgb`/`#rgba`/`#rrggbb`/`#rrggbbaa`, `r,g,b[,a]` (commas or spaces), `rgb(r,g,b)`, `rgba(r,g,b,a)`, or a palette token (`accent`, `card`, `panel`, `text`, `muted`, …); whitespace is ignored |

There is one color value type: `SkinValue::Color(Rgba)`. The applier resolves a
palette token against the active palette *before* calling your
`validate_skin_attr`/`apply_skin_attr`, so `x-tint="accent"` arrives as an
opaque `Rgba` and follows the theme. The literal grammar also accepts decimal
channel lists (commas or spaces) and the functional `rgb(...)`/`rgba(...)`
forms (spaces anywhere are ignored); a functional alpha follows CSS (`0..1`, or
`0..255` above 1), while a bare list uses 0..255 (a fractional value like `0.5`
is 0..1). Read
`value.as_color()` for the RGB part (use this when your widget has no
transparency — the alpha is silently dropped) or `value.as_rgba()` for the full
value (the color picker does). `SkinValueKind::Color::parse` is the
literal-only form; `parse_with(raw, resolve)` adds palette tokens (the applier
uses it). `background="none"` is the transparent-fill spelling; a local color
has no `none` state — that is your widget's own `Option`.

- Use names beginning with `x-` for user vocabulary (`x-level`, `x-units`).
  The library never claims that namespace.
- The shared content names are reserved and cannot be redeclared:
  `text`, `label`, `value`, `progress`, `number`, `checked`, `selected`
  (`xml.skin.reserved`). Use the shared name instead if it fits: implement the
  matching setter and advertise it through `content_caps`.
- An undeclared local name is `xml.skin.unknown`; a value that fails its kind or
  your validator is `xml.skin.value`. Variants that disagree on one id's local
  data are `xml.skin.conflict`.
- Names are parsed by kind before `validate_skin_attr` runs, so you can trust
  the `SkinValue` variant matches the declared kind.

## 3. A complete widget

```rust
use bamboo_native_ui::{
    Rect, SkinAttrDecl, SkinElement, SkinValue, SkinValueKind, Widget, WidgetId,
};

static ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("x-level", SkinValueKind::Number),
    SkinAttrDecl::new("x-units", SkinValueKind::Text),
];
static CHILDREN: &[&str] = &["x-item"];

pub struct Meter {
    id: WidgetId,
    rect: Rect,
    level: f64,
    units: String,
    items: Vec<(String, f64)>,
}

impl<M> Widget<M> for Meter {
    // ... id/rect/set_rect/as_any/as_any_mut/paint/input ...

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] { ATTRS }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("x-level", SkinValue::Number(level)) => (0.0..=100.0).contains(level),
            ("x-units", SkinValue::Text(units)) => !units.trim().is_empty(),
            _ => true,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("x-level", SkinValue::Number(level)) => self.level = *level,
            ("x-units", SkinValue::Text(units)) => self.units = units.to_string(),
            _ => {}
        }
    }

    fn skin_children(&self) -> &'static [&'static str] { CHILDREN }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(|child| {
            child.name == "x-item"
                && child.attr("label").is_some_and(|label| !label.trim().is_empty())
                && child.attr_number_ok("weight")
                && child.attrs_are(&["label", "weight"])
        })
    }

    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        self.items = children
            .iter()
            .filter_map(|child| {
                Some((
                    child.attr("label")?.to_string(),
                    child.attr_number("weight").unwrap_or(1.0),
                ))
            })
            .collect();
    }
}
```

Then the file can address it like any other widget:

```xml
<widget id="meter" x="18" y="484" w="cw-36" h="92"
        x-level="72" x-units="dB">
  <x-item label="低频" weight="3"/>
  <x-item label="中频" weight="2"/>
</widget>
```

`SkinElement` helpers: `attr`, `attr_bool`, `attr_number`, `attr_bool_ok`,
`attr_number_ok`, `attrs_are` (typo guard for a fixed child vocabulary),
`same_data` (structural equality), `line`/`column` for diagnostics.

## 4. What the app does (unchanged)

```rust
let text = std::fs::read_to_string("layouts/layout-demo.buixml")?;
let document = bamboo_native_ui::layout_xml::LayoutDocument::parse(&text)?;
ui.apply_layout(&document, &palette, client_size, window)?;
```

Your widget only has to exist in `ui` under the id the file names. No registry,
no `TypeId` table, no feature flag on your crate. The applier's order is:
child lists → shared content (including `selected`) → local attributes, once
per generation, with one damage mark per widget.

## 5. Gotchas

- **The widget must already exist.** A skin cannot instantiate `Meter` or add a
  second instance; create it in Rust and let the file position/seed it.
- **Child elements replace, they do not append.** Build the new model and
  assign it; never mutate an element in place during `apply_skin_children`.
- **Images are ids, not names.** A page registers pixels with `Ui::set_image`
  and widgets reference the `ImageId`; there is no name registry yet, so a
  custom child image attribute should take the raw id (a number) and document
  that (D42).
- **Structural changes stay Rust-owned.** A child list that would change the
  widget's *structure* (tab count, table columns, toolbar cells) must be
  refused by `validate_skin_children` until the widget owns that data at
  runtime.
- **No text colour.** Colour at the section/attribute level is not parsed yet;
  a custom widget can accept its own `x-tint` (a `Color` kind).

## 6. Rust-side custom components (no layout file)

Not every custom component needs a skin; the same public seams cover ordinary
Rust widgets.

**Closure widget.** `CustomWidget<M>` is registered like any built-in and needs
no trait impl:

```rust
use bamboo_native_ui::{A11yRole, Color, Cursor, CustomWidget, Rect, WidgetInput};

ui.add(
    CustomWidget::new("meter", |ctx, rect, state| {
        ctx.painter
            .round_rect(&mut *ctx.canvas, rect.inset(4.0, 4.0), 8.0, ctx.palette.accent);
        ctx.text(rect, "72%", TextStyleRole::Body, TextAlign::Center,
            if state.hovered { ctx.palette.text } else { ctx.palette.muted });
    })
    .rect(Rect::from_xywh(20.0, 20.0, 160.0, 40.0))
    .focusable(true)
    .interactive(true)
    .cursor(Cursor::Hand)
    .corner_radius(10.0)
    .background(Color::rgb(0x2B2F3A))
    .a11y_name(A11yRole::Button, "电平表")
    .input(|_rect, input| match input {
        WidgetInput::ClickAt(_) | WidgetInput::Activate => Some(Msg::MeterClicked),
        _ => None,
    }),
);
```

- The paint closure gets `(&mut WidgetPaint, Rect, &WidgetState)`; draw only
  inside `rect`. A non-default surface (`.corner_radius`/`.background`/…)
  paints before the closure, so the closure usually draws content only.
- The input closure gets `(Rect, WidgetInput)` and returns `Option<M>`. Use
  `.hit_test(|rect, pos| …)` for a non-rectangular shape.
- Everything else is the built-in contract: `Ui::add` / `add_with` +
  `WidgetProps`, `Ui::with_widget`/`edit_widget` typing, the focus ring,
  a11y (`.a11y` / `.a11y_name` / `.a11y_fn`), and the layout skin's
  `SurfaceStyle` seam.
- `ui.add_custom("meter", rect, |ctx, rect, state| { … })` is the paint-only
  shorthand.

**Owned state.** Implement `Widget<M>` directly (see `meter.rs`). The library
exports the helpers the built-ins use, and both macros use `$crate` paths so
they expand correctly in your crate:

- `Observers<M>` + `observer_builders!(MyWidget);` / `observer_methods!();`
  give `.on_click` / `.on_right_click` / `.on_double_click` / `.on_key_down` /
  `.on_key_up` / `.on_focus_gained` / `.on_focus_lost`. The struct needs an
  `observers: Observers<M>` field; invoke `observer_methods!()` inside the
  `impl Widget<M>` block.
- `surface_builders!(MyWidget);` / `surface_builders_plain!(MyWidget);` give
  `.corner_radius` / `.corners` / `.background` / `.background_option` /
  `.transparent_background` / `.surface()`. The struct needs a
  `surface: SurfaceStyle` field; also forward `Widget::surface_style` /
  `surface_style_mut` so the layout skin can style it.

```rust
use bamboo_native_ui::{Observers, SurfaceStyle};

struct Chip<M> {
    surface: SurfaceStyle,
    observers: Observers<M>,
}
bamboo_native_ui::surface_builders!(Chip);
bamboo_native_ui::observer_builders!(Chip);

impl<M: 'static> Widget<M> for Chip<M> {
    // ... id/rect/set_rect/as_any/as_any_mut/paint/input ...
    bamboo_native_ui::observer_methods!();
    fn surface_style(&self) -> Option<&SurfaceStyle> { Some(&self.surface) }
    fn surface_style_mut(&mut self) -> Option<&mut SurfaceStyle> { Some(&mut self.surface) }
}
```

## 7. Where to look

- `crate::skin` — `SkinAttrDecl`, `SkinValueKind`, `SkinValue`, `SkinElement`,
  `is_shared_name`.
- `bamboo_native_ui::CustomWidget`, `bamboo_native_ui::Observers` and the
  exported `surface_builders!`/`observer_builders!` macros.
- `crates/showcase/src/meter.rs` — a working third-party widget used by the
  showcase's `布局` page (implemented trait + skin), checked by `--self-test`
  and `--a11y-dump`.
- `crates/showcase/src/main.rs` — the API page's `api_custom_draw` is a live
  `CustomWidget`; the self-test clicks it and checks the application label.
- `docs/API.md` — the public surface; `docs/M7-XML.md` §6 — the protocol rules;
  `docs/DECISIONS.md` D127/D137/D139/D140 — why it works this way.
