# XML layout authoring

> Write a `.buixml` document that positions widgets, sets runtime properties,
> applies style tokens, supplies initial content and data lists, and can even
> create widgets — then apply it at runtime or compile it.

## What a `.buixml` document is

A `.buixml` document is primarily a **skin**: it assigns rectangles, properties,
style tokens and initial content to widget ids the Rust side already created. It
cannot remove, reorder or reparent those addressed widgets, or bind events. Two
exceptions are explicit: `type="..."` creates a widget through an installed
`WidgetRegistry` (12 standard types; third-party types register from Rust;
created ids are document-owned and a reload removes or replaces them), and child
elements (`<item>`, `<row>`, ...) replace a widget's data model through its skin
protocol.

The runtime reader is `feature = "layout-xml"`; the same file compiles with
`feature = "layout-compiled"` (see [06](../quickstart/06-xml-compiled.md)).

## Steps

### 1. Enable the feature and create the file

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

Create `layouts/main.buixml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <const name="pad" value="16"/>
  <layout id="base" min-width="0" min-height="0">
    <widget id="title" x="pad" y="pad" w="cw-2*pad" h="28" label="Library"/>
    <widget id="search" right="pad" y="pad" w="220" h="28"/>
  </layout>
</layouts>
```

`version` is required and must be `1`. The file is UTF-8 (a BOM is accepted),
names are case-sensitive and kebab-case, comments are ignored, and non-whitespace
text nodes are rejected.

### 2. Create the widgets in Rust

The ids in the file must exist before the first apply, unless the entry creates
them.

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;
use bamboo_native_ui::widgets::{Label, SearchBox};
use bamboo_native_ui::Ui;

#[derive(Clone)] enum Msg { Save }

struct App { ui: Ui<Msg>, layout: LayoutDocument }

impl App {
    fn new() -> Self {
        let mut ui = Ui::new();
        ui.add(Label::new("title", "Library"));
        ui.add(SearchBox::new("search", "Search"));
        let layout = LayoutDocument::parse(include_str!("../layouts/main.buixml"))
            .expect("the embedded layout is valid");
        Self { ui, layout }
    }
}
```

An unknown id is `xml.id.unknown-widget`, and the apply is atomic: nothing moves.

### 3. Apply it in `resize`

```rust
use bamboo_native_ui::{Host, Size, WindowHandle};

impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        let palette = *window.palette();
        if let Err(failure) = self.ui.apply_layout(&self.layout, &palette, size, window) {
            eprintln!("layout rejected: {failure}");
        }
        window.invalidate_all();
    }
}
```

`apply_layout` resolves every id, rectangle and expression first. Any error
returns `LayoutFailure` and mutates nothing; warnings go to
`LayoutReport::warnings`. `WindowHandle` is the `UiHost` sink, and the palette
resolves `background` tokens.

### 4. Structure: root, variants, constants

The root `<layouts version="1">` holds document-scope `<const>` definitions and
breakpoint `<layout>` variants; each variant holds `<scroll>` panels,
`<widget>` entries and compiled-mode `<rust>` bindings. A variant needs `id`
plus optional `min-width` and `min-height`. Exactly one variant must be
`(0, 0)` and minimum pairs must be unique. Selection is pure: among variants
whose minimums both fit the client size, the greatest `min-width`, then the
greatest `min-height` wins. Variants only differ in geometry, properties and
style; content and local skin data must be identical in every variant that
declares them (`xml.content.conflict`, `xml.skin.conflict`).

`<const>` declares a document-scope number; `<scroll>` addresses a panel
registered with `Ui::set_scroll_view`; `<rust>` is a compiled-mode binding.

### 5. Geometry expressions

Every rectangle is one horizontal and one vertical form: `x`+`w`, `right`+`w`
or `x`+`right` (stretched); `y`+`h`, `bottom`+`h` or `y`+`bottom` (stretched).

```text
expr   := term (('+' | '-') term)*
term   := factor (('*' | '/') factor)*
factor := number | number '%' | variable | '(' expr ')' | '-' factor
```

`left`/`top` are `0`; `right`/`bottom` are the space edges; `cw`/`ch` are the
space width/height; `cx`/`cy` are its centre. `N%` is `N/100` of the
attribute's axis (`x`/`w`/`right` use `cw`; `y`/`h`/`bottom` use `ch`).
Constants resolve before evaluation. `w`/`h` must be `>= 0`; `right`/`bottom`
must be `>= 0`; a zero size is a warning.

A 1000×700 window with `pad = 16` makes these concrete:

| Goal | Geometry |
|---|---|
| Full-width header | `x="pad" y="pad" w="cw-2*pad" h="48"` |
| Sidebar 240 wide, full height below the header | `x="pad" y="80" w="240" bottom="pad"` |
| Content to its right | `x="272" y="80" right="pad" bottom="pad"` |
| Two equal columns with a 16 DIP gap | `x="pad" w="(cw-3*pad)/2"` and `x="2*pad+(cw-3*pad)/2" w="(cw-3*pad)/2"` |
| Centred 400×280 card | `x="cx-200" y="cy-140" w="400" h="280"` |
| Bottom bar 40 high | `x="pad" bottom="pad" w="cw-2*pad" h="40"` |

The same shell as a document:

```xml
<const name="pad" value="16"/>
<layout id="base" min-width="0" min-height="0">
  <widget id="header"  x="pad" y="pad" w="cw-2*pad" h="48"/>
  <widget id="sidebar" x="pad" y="80" w="240" bottom="pad"/>
  <widget id="content" x="272" y="80" right="pad" bottom="pad"/>
</layout>
```

A widget scrolled by a registered panel is positioned in that panel's **content**
space, and `cw`/`ch` mean that content's extent. `cw` inside an `auto`
content-height panel is one apply behind; use an explicit `content-height` for
bottom-anchored children.

### 6. Properties

Optional and runtime-safe: `visible`, `enabled`, `interactive`, `focusable`,
`focus-order`, `read-only`, `tooltip`, `name`, `description`, `automation-id`,
`role`, `expose`, `cursor`. Booleans accept `true`/`false`/`1`/`0`; `role` takes
a kebab-case `A11yRole` name; `cursor` takes
`arrow`/`hand`/`i-beam`/`size-we`/`size-ns`. They land through existing
container setters, so a document never rebuilds a widget. Omitted means "leave
whatever the application set"; `name`/`description` change narration only, not
painted text.

### 7. Style tokens

| Attribute | Accepted values |
|---|---|
| `radius` | non-negative number, `small` (4), `medium` (6), `large` (10), `pill` (half the height) |
| `corners` | four numbers `"tl tr br bl"` (spaces or commas) |
| `background` | a palette token, a color literal, or `none` |
| `font-size` | number greater than zero (DIPs) |
| `font-family` | non-empty string |

`radius` and `corners` are mutually exclusive. `background="none"` keeps the
border and drops the fill. A widget without a `SurfaceStyle` reports
`xml.style.unsupported` and skips the surface token; `font-*` works on every
widget. The token list is in [palette tokens](../reference/palette-tokens.md).

### 8. Content initial values

`text` → `TEXT`, `label` → `LABEL`, `value` → `VALUE`, `progress` → `PROGRESS`
(clamped to `0..=1`), `number` → `NUMBER`, `checked` → `TOGGLED`, `selected` →
`SELECTION`. `text`/`label` conflict, as do `value`/`progress`/`number`.

Content is an **initial value**, written only when the document generation
changes. A resize, a breakpoint switch, a theme change or re-applying the same
document never overwrites text the user typed. A widget that does not publish
the capability reports `xml.content.unsupported` and the attribute is skipped.

### 9. Child lists

Child elements replace a widget's data model; they are not live bindings. A
`PlaylistView` takes `<track title subtitle current disabled>` children and a
`Table` takes `<column>`/`<row>` (plus `<cell>` inside a row). Common child
names are `item`, `option`, `row`, `cell`, `column`, `node`, `section`,
`cover`, `track`, `segment`, `step`, `page`, `run` and `point`. The per-widget
vocabulary and accepted attributes are in
[XML elements](../reference/xml-elements.md).

### 10. Creating widgets (`type`, `command`, `parent`)

An entry with `type` creates an id that does not exist yet. Interactive types
need a `command`, resolved through the registry's `CommandMap`.

```xml
<widget id="save" type="button" command="101" x="16" y="8" w="96" h="32" label="Save"/>
<widget id="row-1" type="label" parent="list" x="0" y="250" w="120" h="20" label="Row"/>
```

```rust
use bamboo_native_ui::creation::WidgetRegistry;

let mut registry = WidgetRegistry::<Msg>::standard();
registry.set_commands(|command| (command == 101).then_some(Msg::Save));
ui.set_widget_registry(registry);
```

`WidgetRegistry::standard()` registers `label`, `button`, `toggle`, `slider`,
`progress`, `icon`, `badge`, `card`, `divider`, `kbd`, `avatar` and `spinner`.
`parent` joins a created widget to a registered panel and resolves its rectangle
in the panel's content space. A created widget belongs to the document: a reload
that drops it removes it, and `Ui::clear_layout` removes every created widget.
Every variant must declare every created id with the same `type`/`command`/`parent`.

<a id="custom-widgets"></a>
### 11. Custom widgets

A widget outside the crate publishes the attributes and child names it accepts.

```rust
use bamboo_native_ui::{SkinAttrDecl, SkinValue, SkinValueKind, Widget};

static LEVEL_ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("level", SkinValueKind::Number)];

impl Widget<Msg> for LevelBar {
    fn skin_attrs(&self) -> &'static [SkinAttrDecl] { LEVEL_ATTRS }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        matches!(
            (name, value),
            ("level", SkinValue::Number(level)) if (0.0..=1.0).contains(level)
        )
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        if let ("level", SkinValue::Number(level)) = (name, value) {
            self.level = *level as f32;
        }
    }
}
```

The block shows only the three skin methods; the required `id`, `rect`,
`set_rect`, `as_any`, `as_any_mut`, `paint` and `input` methods are those of
any manual widget (see [04 — Custom widgets](../quickstart/04-custom-widgets.md)).
A document can then write
`<widget id="level" x="16" y="120" w="240" h="12" level="0.8"/>`.

The applier validates every declared attribute and child list before mutating,
then applies child lists, content and attributes in order and marks the widget
dirty once. `x-` names are reserved for application code; the shared content
names are reserved for the schema. Local values are content: written only on a
generation change and not restored by `clear_layout`. See
[CustomWidget](../widgets/custom/custom-widget.md) for the full protocol.

### 12. Hot reload

The library performs no file IO and installs no watcher. Poll the file and keep
the last working document.

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;
use bamboo_native_ui::{Palette, Size, Ui, UiHost};

fn reload(text: &str, current: &LayoutDocument, ui: &mut Ui<Msg>,
          palette: &Palette, size: Size, sink: &mut impl UiHost) -> Option<LayoutDocument> {
    let document = LayoutDocument::parse(text).ok()?;
    if document.generation() == current.generation() {
        return None;
    }
    ui.apply_layout(&document, palette, size, sink).ok()?;
    Some(document)
}
```

Drive it from `Host::timer` after
`window.set_animation_timer(Some(Duration::from_millis(250)))`; a poll paints only on a
generation change.

### 13. Limits and failure behaviour

- The file is limited to 4 MiB, 50 000 entries and 10 000 child elements per
  widget. At most 100 diagnostics are collected per parse.
- Expressions are limited to 32 nesting levels; division by zero and
  non-finite results are errors.
- No DTD, CDATA, namespaces, includes or watcher. No expressions in style or
  content values. No event binding: clicks, keys and drags stay in Rust
  (`Widget::input`, `Ui::bind`).
- Errors reject the whole document and leave the previous layout installed;
  warnings (`xml.style.unsupported`, `xml.content.unsupported`,
  `xml.value.clamped`, ...) apply the rest.
- `Ui::clear_layout(window)` hands every claimed field back and removes created
  widgets; rectangles and content are not restored. The complete diagnostic
  code list is in [XML elements](../reference/xml-elements.md).

## Trade-offs

| Choice | Benefit | Cost |
|---|---|---|
| Dynamic `layout-xml` | Edit and reload without a rebuild | XML parser linked; parse at runtime |
| Compiled `layout-compiled` | No parser; values type-checked by `rustc` | A generation step or macro rebuild |
| `type` + registry | Pages described entirely by data | A registry/command table to install |

## See also

- [XML elements](../reference/xml-elements.md)
- [05 — Dynamic XML](../quickstart/05-xml-dynamic.md)
- [06 — Compiled XML](../quickstart/06-xml-compiled.md)
- [The Ui container](../api/ui-container.md)
- [CustomWidget](../widgets/custom/custom-widget.md)
- [Scrolling](../api/scrolling.md)
- [Debugging](debugging.md)
