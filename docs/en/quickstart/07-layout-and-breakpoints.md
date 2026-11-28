# 07 — Layout and breakpoints

> Compute a responsive shell twice: with `bamboo_native_ui::layout` helpers in
> `Host::resize`, then with `.buixml` breakpoint variants selected by client
> size.

## What you will build

A window with a sidebar, a header with two buttons, a content pane and a status
line. When the client is narrower than 900 DIPs the sidebar collapses from 220
DIPs to 56 DIPs; above the threshold it expands again. The first half of the
page does this with `VStack`, `HStack`, `split_h`, `split_v`, `grid` and
`padded`. The second half moves the same six widgets into a `.buixml` document
with a `wide` and a `base` variant, and lets `Ui::apply_layout` pick one.

## Prerequisites

- The window from [03](03-first-window.md) builds and runs.
- For the second half, `layout-xml` is enabled as in [05](05-xml-dynamic.md):

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

The helpers in `bamboo_native_ui::layout` need no feature flag.

## Steps

### 1. Create the shell widgets

Create six widgets once. They are the same ids the document will address later,
so the two halves of the page swap only the placement code.

```rust
use bamboo_native_ui::widgets::{Button, Label, LabelStyle};
use bamboo_native_ui::{Host, Insets, Rect, Size, Ui, WidgetId, WindowHandle};

#[derive(Clone, Copy, Debug)]
enum Msg {
    Open,
    Save,
}

struct App {
    ui: Ui<Msg>,
}

impl App {
    fn new() -> Self {
        let mut ui = Ui::new();
        ui.add(Label::new("title", "Bamboo Player").style(LabelStyle::Title));
        ui.add(Button::new("open", "Open", Msg::Open));
        ui.add(Button::new("save", "Save", Msg::Save));
        ui.add(Label::new("sidebar", "Playlists · Favorites").wrap(true));
        ui.add(Label::new("body", "Select a track from the sidebar.").wrap(true));
        ui.add(Label::new("hint", "Ready"));
        Self { ui }
    }
}
```

The widgets are created without knowing where they will sit; the same `ui` can
be laid out by hand or by a document.

### 2. Compute the layout in `resize`

The [layout helpers](../api/layout-helpers.md) are pure functions over `Rect`,
so run them only when the client size changes. Replace the `resize` method of
the `Host` implementation from 03 with this version and add the `layout`
function.

```rust
use bamboo_native_ui::layout::{grid, padded, split_h, split_v, HStack, VStack};

fn layout(ui: &mut Ui<Msg>, size: Size) {
    // One page inset; every other rectangle is derived from it.
    let page = padded(
        Rect::from_xywh(0.0, 0.0, size.width, size.height),
        Insets::uniform(12.0),
    );

    // Header row, then everything below it.
    let mut rows = VStack::new(page, 10.0);
    let header = rows.next(44.0);
    let body = rows.remaining();

    // Header: title on the left, two equal button cells on the right.
    let header_cells = split_h(header, &[3.0, 1.0], 10.0);
    ui.set_rect(WidgetId::new("title"), header_cells[0]);
    let actions = grid(header_cells[1], 2, 1, 8.0);
    ui.set_rect(WidgetId::new("open"), actions[0]);
    ui.set_rect(WidgetId::new("save"), actions[1]);

    // The sidebar width follows the breakpoint; content takes the rest.
    let mut columns = HStack::new(body, 10.0);
    let sidebar_width = if size.width < 900.0 { 56.0 } else { 220.0 };
    let sidebar = columns.next(sidebar_width);
    let content = columns.remaining();
    ui.set_rect(WidgetId::new("sidebar"), sidebar);

    // Content: a main pane and a footer strip, split by weight.
    let content_cells = split_v(content, &[5.0, 1.0], 10.0);
    ui.set_rect(WidgetId::new("body"), content_cells[0]);
    ui.set_rect(WidgetId::new("hint"), content_cells[1]);
}

impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        layout(&mut self.ui, size);
        window.invalidate_all();
    }
}
```

`VStack::next` returns a full-width row and advances the cursor; `remaining`
reports the space left below it (the trailing gap is excluded, so it starts at
the previous cell's edge). `split_h`/`split_v` divide the space after
subtracting `gap * (n - 1)` and give each cell `usable * weight / total`;
weights need not sum to one. `grid` tiles the area row-major and clamps both
counts to at least one. `padded` is `rect.inset_by(insets)`. Use the split
helpers when you want gutters, as the header and content column do here.

### 3. Write the breakpoint document

A [`.buixml` layout document](../api/layout-documents.md) holds one `<layout>`
per variant. Every variant needs a rectangle for every id it moves; it may also
hide a widget or change a font. The parser requires exactly one `0,0` variant;
`LayoutDocument::select(size)` chooses, among the variants whose two minimums
fit, the greatest `(min-width, min-height)` pair. Minimums are client DIPs.

Create `layouts/shell.buixml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <const name="pad" value="16"/>
  <const name="collapsed" value="56"/>
  <const name="expanded" value="220"/>
  <const name="header" value="56"/>
  <const name="footer" value="64"/>

  <layout id="wide" min-width="900">
    <widget id="sidebar" x="0" y="0" w="expanded" h="ch" background="sidebar"/>
    <widget id="title" x="expanded+pad" y="pad" w="cw-expanded-2*pad-272" h="30" font-size="20"/>
    <widget id="open" right="pad+132" y="pad" w="120" h="36" radius="medium" background="accent"/>
    <widget id="save" right="pad" y="pad" w="120" h="36" radius="medium"/>
    <widget id="body" x="expanded+pad" y="header" right="pad" bottom="footer"/>
    <widget id="hint" x="expanded+pad" right="pad" bottom="pad" h="24"/>
  </layout>

  <layout id="base" min-width="0" min-height="0">
    <widget id="sidebar" x="0" y="0" w="collapsed" h="ch" background="sidebar"/>
    <widget id="title" x="collapsed+pad" y="pad" w="cw-collapsed-2*pad" h="30" font-size="16"/>
    <widget id="open" x="collapsed+pad" y="header" right="cx-collapsed/2" h="36"
            radius="medium" background="accent"/>
    <widget id="save" x="cx+collapsed/2" y="header" right="pad" h="36" radius="medium"/>
    <widget id="body" x="collapsed+pad" y="header+52" right="pad" bottom="footer"/>
    <widget id="hint" x="collapsed+pad" right="pad" bottom="pad" h="24"/>
  </layout>
</layouts>
```

The wide variant puts the buttons on the right of the header. The base variant
stacks them under the title and splits the content width evenly: `x` plus
`right` stretches each rectangle, and `cx` locates the divider. The sidebar
changes only `w`.

### 4. Apply the document in `resize`

Add a `layout: LayoutDocument` field to the app, assigned from this helper in
`new`:

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;

fn load_layout() -> LayoutDocument {
    LayoutDocument::parse(include_str!("../layouts/shell.buixml"))
        .expect("the embedded layout is valid")
}
```

Then replace the programmatic `layout` call with `apply_layout`. The palette
comes from the window, so `background="sidebar"` and `background="accent"`
resolve against the active theme.

```rust
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

`apply_layout` resolves every id, rectangle and expression before it mutates
anything, so a failure leaves the previous layout on screen. A resize on the same
generation rewrites rectangles only: content and skin data are written once per
document generation, so text the user edited is not reset by a breakpoint.

### 5. Read the expression language

Every geometry attribute is an expression. It is parsed once and evaluated
against the current space (the client area, or a scroll panel's content area)
on every apply.

| Name | Value |
|---|---|
| `cw`, `right` | Space width in DIPs. |
| `ch`, `bottom` | Space height in DIPs. |
| `cx`, `cy` | Width / 2, height / 2. |
| `left`, `top` | `0`. |
| `N%` | `N/100` of the axis: `x`, `w`, `right` use width; `y`, `h`, `bottom` use height. |

Operators are `+`, `-`, `*` and `/`, with precedence and parentheses. A
`<const>` is a document-scope number resolved in declaration order; constants may
use earlier constants but not space variables or `%`. The horizontal form is
exactly one of `x`+`w`, `right`+`w` or `x`+`right`; the vertical form is one of
`y`+`h`, `bottom`+`h` or `y`+`bottom`.

## Expected result

- At 1100 DIPs the sidebar is 220 DIPs, the title is beside two buttons and the
  hint line sits under the content.
- Below 900 DIPs the base variant shows a 56 DIP sidebar and buttons under the
  title; the six widgets are otherwise the same.
- The document version needs no rebuild to change rectangles.

## Troubleshooting

- **`xml.id.unknown-widget`** — the document names an id that `App::new` did not
  add. The apply is atomic, so nothing moves. Add the widget or fix the id.
- **`xml.layout.base`** — no variant has `min-width="0" min-height="0"`, or two
  variants share the same minimums. Keep exactly one base variant.
- **`xml.geom.underdetermined`** — an entry has neither a size nor both edges on
  one axis. Every entry determines its own rectangle; there is no measure pass.
- **A widget keeps a stale rectangle after shrinking** — it is declared in
  `wide` but missing from `base`. Declare every id in every variant (hide it
  with `visible="false"` if needed), or its last rectangle stays.
- **The layout never changes with the window** — `select` compares client DIPs;
  a variant whose `min-width` is above the client can never be chosen. Check the
  threshold and that `resize` actually runs.

## Next

[08 — Lists, tables and trees](08-lists-tables-trees.md): fill the content pane
with virtualized data views.
