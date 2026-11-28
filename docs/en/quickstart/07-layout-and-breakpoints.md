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

**New terms on this page**

- **DIP** — device-independent pixel. The framework lays out in DIPs; the host
  scales them to physical pixels, so the numbers stay meaningful at 100%,
  150% or 200% display scaling.
- **Client area** — the drawable interior of the window, excluding its frame,
  title bar and any native menu bar. `size` in `resize` is the client area.
- **Layout helper** — a pure function that takes a `Rect` and returns one or
  more smaller `Rect`s. It paints nothing and owns nothing.
- **`VStack` / `HStack`** — a cursor over an area: `next(n)` takes the next row
  or cell, `remaining()` returns what is left.
- **`split_h` / `split_v`** — divide an area into weighted cells with gutters.
- **`grid`** — tile an area into equal cells, row-major.
- **`padded`** — shrink a rectangle by insets; the one-line page margin here.
- **Breakpoint** — a client-size threshold at which a different layout is
  appropriate (here: 900 DIPs wide).
- **Variant** — one `<layout>` block in the document. The document chooses the
  best-fitting variant for the current client size.
- **Expression** — a geometry attribute value such as `cw-expanded-2*pad`. It
  is parsed once and evaluated against the current space on every apply.

## Prerequisites

- The window from [03](03-first-window.md) builds and runs.
- For the second half, `layout-xml` is enabled as in [05](05-xml-dynamic.md):

```toml
[dependencies]
# Only the second half needs this; steps 1-2 use plain Rust.
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

The helpers in `bamboo_native_ui::layout` need no feature flag.

All files on this page live in the crate folder `bamboo-hello/bamboo-hello/`:
edit `src/main.rs` there, and create `layouts/shell.buixml` in a `layouts`
folder next to `src`. Run `cargo run` from that crate folder or from the
workspace folder one level up.

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
        // Six widgets, created once; neither the helper code nor the document
        // below creates or removes them.
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
        // `size` is the client area in DIPs; layout runs only on a resize.
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

Follow one pass to see how a rectangle is computed. At a 1100×700 client, the
12 DIP inset leaves a 1076×676 page. The header takes the first 44 DIPs and the
body gets the rest. `split_h` with weights `[3.0, 1.0]` and a 10 DIP gap makes
the title cell 799.5 DIPs wide and the action cell 266.5; `grid` then cuts the
action cell into two buttons of 129.25 DIPs. The body's `HStack` gives the
sidebar 220 DIPs and the content the remaining 856.

**What just happened.** Every widget now has a rectangle derived from one page
inset. Drag the window narrower than 900 client DIPs and `resize` runs again
with a smaller `size`: the same `if` chooses 56 for the sidebar, and the header,
buttons and content are recomputed from the new page. No widget is recreated,
and nothing is painted outside `invalidate_all`'s repaint.

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
  <!-- Document-scope numbers: names used by the expressions below. -->
  <const name="pad" value="16"/>
  <const name="collapsed" value="56"/>
  <const name="expanded" value="220"/>
  <const name="header" value="56"/>
  <const name="footer" value="64"/>

  <!-- Chosen at 900 client DIPs and wider. -->
  <layout id="wide" min-width="900">
    <widget id="sidebar" x="0" y="0" w="expanded" h="ch" background="sidebar"/>
    <widget id="title" x="expanded+pad" y="pad" w="cw-expanded-2*pad-272" h="30" font-size="20"/>
    <widget id="open" right="pad+132" y="pad" w="120" h="36" radius="medium" background="accent"/>
    <widget id="save" right="pad" y="pad" w="120" h="36" radius="medium"/>
    <widget id="body" x="expanded+pad" y="header" right="pad" bottom="footer"/>
    <widget id="hint" x="expanded+pad" right="pad" bottom="pad" h="24"/>
  </layout>

  <!-- The required 0,0 fallback: everything below 900 DIPs. -->
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

Selection is by minimums, not by order. At a 1100 DIP-wide client,
`LayoutDocument::select` returns `wide` (it fits and has the greatest
minimums); at 700 DIPs the only fitting variant is `base`. Nothing in the file
names a pixel breakpoint beyond `min-width="900"`. Every id must be declared in
the variant that should move it — a missing entry keeps its previous rectangle.

### 4. Apply the document in `resize`

Add a `layout: LayoutDocument` field to the app, assigned from this helper in
`new`:

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;

// Parse the text embedded at build time; a missing file blocks the build.
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
        // The document's colour tokens resolve against this palette.
        let palette = *window.palette();
        // Validate all six ids and expressions, then move them.
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

At startup the document's embedded copy is what gets applied. This page has no
file watcher, unlike [05](05-xml-dynamic.md), so editing `shell.buixml` while
the app runs changes nothing until you rebuild; resizing only re-applies the
compiled-in copy.

**What just happened.** The six widgets from step 1 are now placed by the
document instead of by hand. Crossing 900 client DIPs re-runs `resize`, so
`select` switches variants and `apply_layout` moves the same six ids to the
`base` rectangles. The hand-written `layout` function is no longer called;
delete it (or leave it and accept `dead_code`/`unused_imports` warnings from
your crate) so the build stays warning-free.

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

Two worked examples from `shell.buixml` at a 1100 DIP-wide client:

- The wide title's `w="cw-expanded-2*pad-272"` is
  `1100 - 220 - 32 - 272 = 576` DIPs.
- In the base variant, `cx` is `1100 / 2 = 550`. The first button ends at
  `1100 - (cx-collapsed/2) = 1100 - 522 = 578`, and the second starts at
  `cx+collapsed/2 = 578`, so they meet at the divider. `w="50%"` on an
  `x`-axis attribute would mean half the space width in the same way.

The `right` and `bottom` attributes are insets measured from the right and
bottom edges, not absolute coordinates; that is why a positive `right` moves a
widget left. `cw`/`ch`/`cx`/`cy` describe the space the entry is evaluated in,
so inside a scroll panel they refer to the panel's content area rather than the
window.

## Expected result

- At 1100 DIPs the sidebar is 220 DIPs, the title is beside two buttons and the
  hint line sits under the content.
- Below 900 DIPs the base variant shows a 56 DIP sidebar and buttons under the
  title; the six widgets are otherwise the same.
- Resizing re-applies the parsed document without a rebuild; changing the
  `.buixml` file itself does need one on this page, because there is no file
  watcher.

**Run it.** From the workspace folder `bamboo-hello/` or the crate folder
`bamboo-hello/bamboo-hello/`:

```powershell
cargo run
```

A window appears with a dark sidebar on the left, the title and two buttons in
the header, the content pane and the status line. If the window is still the
520×320 one from page 03, widen it first (drag an edge, or set
`.size(1100.0, 700.0)` on the `WindowBuilder`) — below 900 client DIPs you only
ever see `base`. Start wide, then drag the right edge inward past the threshold:
the sidebar narrows and the buttons move under the title. Drag outward and the
wide arrangement returns. The number that matters is the client width in DIPs,
not physical pixels — at 150% display scaling a 1350-pixel-wide client is
exactly 900 DIPs.

## Troubleshooting

- **`LayoutDocument` or `layout_xml` does not resolve** — the `layout-xml`
  feature is off. Steps 1-2 compile without it; step 4 cannot.
- **`xml.syntax` with a position** — the XML is malformed (an unclosed tag, an
  unquoted attribute, a missing `/` on an empty element). Because this page
  parses once with `.expect(...)`, the program panics at startup and no window
  appears; fix the file and rebuild.
- **`xml.id.unknown-widget`** — the document names an id that `App::new` did not
  add. The apply is atomic, so nothing moves. Add the widget or fix the id; the
  six ids in step 1 must match exactly.
- **`xml.layout.base`** — no variant has `min-width="0" min-height="0"`, or two
  variants share the same minimums. Keep exactly one base variant.
- **`xml.geom.underdetermined`** — an entry has neither a size nor both edges on
  one axis. Every entry determines its own rectangle; there is no measure pass.
- **`xml.geom.overdetermined`** — an entry gives too much on one axis, for
  example `x`, `right` and `w`. Use exactly one of `x`+`w`, `right`+`w` or
  `x`+`right` (and the matching vertical form).
- **`xml.expr.syntax` or `xml.const.undefined`** — a typo in an expression or a
  constant name. Constants are declared before use; `undefined` means the name
  was never declared in this document.
- **`xml.const.vars`** — a `<const>` used `cw`, `ch`, `cx`, `cy` or `%`. Constants
  are fixed numbers; only widget attributes may read the space.
- **`error: couldn't read ...\layouts\shell.buixml`** — `include_str!` resolves
  relative to `src/main.rs`, so the file must exist when you build. Create
  `layouts/shell.buixml` next to `src`, not inside `src`.
- **Editing the XML has no visible effect** — this page has no hot reload. The
  text was embedded at build time; run `cargo build`/`cargo run` again. Resizing
  only re-applies the old embedded document.
- **The sidebar never collapses at 900** — `resize` receives client DIPs, so a
  high-DPI window reaches the threshold at a larger pixel width. Check the
  value actually passed to `layout`/`apply_layout`, and that `resize` runs at
  all.
- **A widget keeps a stale rectangle after shrinking** — it is declared in
  `wide` but missing from `base`. Declare every id in every variant (hide it
  with `visible="false"` if needed), or its last rectangle stays.
- **The layout never changes with the window** — `select` compares client DIPs;
  a variant whose `min-width` is above the client can never be chosen. Check the
  threshold and that `resize` actually runs.
- **The window is blank or the widgets are stacked at the origin** — every
  apply was rejected, so the hand-written rectangles are gone and the widgets
  sit at their constructor defaults. Read the first `layout rejected:` line.

## Next

[08 — Lists, tables and trees](08-lists-tables-trees.md): fill the content pane
with virtualized data views.
