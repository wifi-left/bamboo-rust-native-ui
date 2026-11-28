# 05 — Dynamic XML

> Move rectangles, properties and initial content into a `.buixml` file and
> apply it at runtime with `feature = "layout-xml"`.

## What you will build

The window from [03](03-first-window.md) with its layout described by
`layouts/main.buixml`. In this tutorial the Rust side creates the label and the
button; the document positions them, styles the button and sets their initial
labels. A 250 ms timer polls the file and reloads it on change, so editing the
XML updates a running window. An optional step also shows the document
creating a widget itself through a `WidgetRegistry`.

**New terms on this page**

- **XML** — a plain-text format for nested data. An *element* is a tag such as
  `<widget>`; an *attribute* is a name/value pair inside the tag, such as
  `id="status"`. A *comment* is `<!-- ... -->` and the parser ignores it.
- **Root element** — the single outermost element of an XML file. Here it is
  `<layouts>`; every other element sits inside it.
- **`.buixml` layout document** — an XML file that follows the framework's
  layout schema: constants, variants, widgets and their geometry.
- **Cargo feature flag** — an opt-in switch in `Cargo.toml`. Code behind a
  feature is compiled only when the feature is listed, which is how one crate
  offers an optional XML parser.
- **`LayoutDocument`** — the in-memory, already-validated form of the file.
  Parsing turns text into this value; applying turns it into rectangles.
- **Apply** — copy the document's rectangles, properties and initial content
  onto widgets that Rust already created.
- **Hot reload** — re-read the file while the program runs, without rebuilding.
- **Variant** — one `<layout>` block in the document; the applier picks one by
  client size. Page 07 uses two variants; this page needs only the `0,0`
  fallback.
- **Expression** — a geometry value such as `cw-2*pad` that is computed from
  the client size and document constants.
- **`WidgetRegistry`** — a table that maps a `type="..."` name to code that
  creates that widget, used only in the optional step.

## Prerequisites

- The project from [03](03-first-window.md) builds and runs.
- Enable the layout feature in `bamboo-hello/Cargo.toml`:

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

`layout-xml` adds the parser (`LayoutDocument::parse`) and the applier
(`Ui::apply_layout`). It does not change any existing API.

The snippet replaces the dependency line from page 02 in
`bamboo-hello/bamboo-hello/Cargo.toml` — the crate folder that also holds
`src/`, not the workspace folder above it. A *feature* is opt-in at compile
time: without `layout-xml`, the parser and applier are not part of the
compiled crate, so the compiler reports them as missing rather than failing at
run time.

Every path on this page is relative to that crate folder. `cargo build` and
`cargo run` work from the crate folder or from the workspace folder one level
up, because Cargo finds the workspace manifest on its own.

## Steps

### 1. Create the document

Create `bamboo-hello/bamboo-hello/layouts/main.buixml`. That path is counted
from the workspace folder from page 01; if your terminal is already in the
crate folder, create `layouts/main.buixml` next to `src`. Type the full name: if
your editor appends `.txt`, remove it, because both the build-time
`include_str!` and the reload timer look for exactly `main.buixml`.

```xml
<?xml version="1.0" encoding="utf-8"?>
<!-- The single root element holds the whole document. -->
<layouts version="1">
  <!-- A document-scope number usable by the geometry below. -->
  <const name="pad" value="24"/>

  <!-- The required fallback variant: minimums of 0,0. -->
  <layout id="base" min-width="0" min-height="0">
    <!-- x/y/w/h are DIPs; "pad" and "cw" are expressions. -->
    <widget id="status"
            x="pad" y="24" w="cw-2*pad" h="28"
            label="No clicks yet"/>

    <!-- Styled button on the right edge; the id matches a Rust widget. -->
    <widget id="greet"
            right="pad" y="64" w="140" h="36"
            label="Say hello"
            radius="medium" background="accent"/>
  </layout>
</layouts>
```

Every rectangle is in DIPs. `cw` and `ch` are the client width and height; a
`<const>` is a document-scope number. The horizontal form is one of `x`+`w`,
`right`+`w` or `x`+`right`, and the vertical form is one of `y`+`h`,
`bottom`+`h` or `y`+`bottom`. A document must contain a variant whose minimums
are `0,0`; the widest matching variant wins.

Read the file as a tree: `<layouts>` is the root element, each `<const>`,
`<layout>` and `<widget>` is an element, and `id="status"` is an attribute.
Empty elements end in `/>`; a comment starts with `<!--` and ends with `-->`.
XML is case-sensitive and every attribute value must be quoted.

Saving the file changes nothing yet: it is still inert text until Rust reads
it in the next step.

### 1b. Geometry expressions in detail

Every `x`, `y`, `w`, `h`, `right` and `bottom` value is an expression. It is
evaluated against the **layout space** — usually the window's client area, or
the content area of a registered scroll panel — and produces a number of DIPs.

#### The six forms

A rectangle needs exactly one horizontal form and one vertical form:

| Axis | Write | Meaning |
|---|---|---|
| Horizontal | `x` + `w` | left edge and width |
| Horizontal | `right` + `w` | distance from the right edge and width |
| Horizontal | `x` + `right` | both edges; the width stretches |
| Vertical | `y` + `h` | top edge and height |
| Vertical | `bottom` + `h` | distance from the bottom edge and height |
| Vertical | `y` + `bottom` | both edges; the height stretches |

"Exactly one" is literal: two values per axis. Three (`x`, `w`, `right`) is
overdetermined; one is underdetermined. Both are document errors, not warnings.

#### The pieces of an expression

| Piece | Example | Meaning |
|---|---|---|
| Number | `24`, `1.5` | DIPs |
| Percentage | `50%` | percentage of this attribute's axis |
| Constant | `pad` | a value declared by `<const name="pad" value="24"/>` |
| Space variable | `cw`, `ch`, `cx`, `cy` | client (or content) width, height, centre |
| Edge variable | `left`, `top`, `right`, `bottom` | `0` and the space's far edges |
| Operator | `+ - * /` | arithmetic; `*` and `/` bind tighter |
| Parentheses | `(cw-48)/2` | group first |

The percentage axis rule is the part that surprises people: a percentage in a
horizontal attribute (`x`, `w`, `right`) is a percentage of `cw`; the same
number in a vertical attribute (`y`, `h`, `bottom`) is a percentage of `ch`.
So `w="50%"` is half the width, while `h="50%"` is half the height.

`cw`, `ch`, `cx` and `cy` are computed from the space, not from the parent
widget: this is a flat coordinate system, not a nested flexbox. `right` and
`bottom` are also the space's right and bottom edges when used inside an
expression.

#### Worked examples on the tutorial window

The window from page 03 is 520×320 client DIPs. With `pad = 24`, these
fragments produce the stated rectangles:

| Goal | Write | Resolves to |
|---|---|---|
| Full width, 24 DIP margins | `x="24" w="cw-48"` | `x = 24`, `w = 472` |
| Right-aligned, 140 wide | `right="24" w="140"` | `x = 356`, `w = 140` |
| Centred, 140 wide | `x="cx-70" w="140"` | `x = 190`, `w = 140` |
| Half width | `x="24" w="50%"` | `x = 24`, `w = 260` |
| Offset percentage | `x="25%+8" w="50%-16"` | `x = 138`, `w = 244` |
| Stretch below a 64 DIP header | `y="64" bottom="24"` | `y = 64`, `h = 232` |
| Bottom bar 36 high | `bottom="24" h="36"` | `y = 260`, `h = 36` |
| Two equal columns, 16 gap | `x="24" w="(cw-64)/2"` and `x="40+(cw-64)/2" w="(cw-64)/2"` | `w = 228`; `x = 24` and `x = 268` |
| Centred card 200×100 | `x="cx-100" y="cy-50" w="200" h="100"` | `x = 160`, `y = 110` |

Why `(cw-64)/2`? The 64 is two 24 DIP margins plus the 16 DIP gap; each column
gets half of what remains. The second column starts one gap after the first:
`24 + 228 + 16 = 268`.

#### Operator precedence

`*` and `/` bind tighter than `+` and `-`; parentheses override. These are
different:

- `w="cw-2*pad"` → `cw - (2*pad)` = 472.
- `w="(cw-2)*pad"` → `(520 - 2) * 24` = 12432, far outside the window.

If you are unsure, add parentheses; the parser does not guess.

#### Common mistakes

| Symptom | Cause | Fix |
|---|---|---|
| `w="50"` made a 50 DIP button | `50` is a number, not a percentage | `w="50%"` |
| A "half-width" widget starts in the middle | `x="50%"` moves the left edge; it does not set the width | use `w="50%"`, or `x="25%" w="50%"` to centre it |
| `xml.const.undefined` | a constant name is misspelled or declared after it is used | declare `<const>` before the `<layout>` that uses it |
| `xml.geom.overdetermined` | three of `x`/`right`/`w` (or the vertical trio) | remove one |
| `xml.geom.underdetermined` | only one of the horizontal or vertical values | add the missing pair |
| The widget is off-screen | `cx` is the centre, not the right edge; `cx+100` starts past the centre | subtract half the width: `x="cx-100" w="200"` |

#### Scrolled content

Inside a registered scroll panel, the same expressions run in the panel's
**content** space: `cw`/`ch` are the content width/height, not the visible
viewport. This is how a wide table or a tall list gets a rectangle taller than
the window. With `content-height="auto"`, `ch` is one apply behind, so a
bottom-anchored child may jump on the first frame; use an explicit
`content-height` when that matters.

The [XML elements reference](../reference/xml-elements.md#geometry) has the
same recipe table for a 1000×700 space, and page 07 uses these expressions for
breakpoints. Try changing `w="140"` on the `greet` button to `w="200"` and
reload: because it is anchored with `right`, the right edge stays put and the
left edge moves.

### 2. Create the widgets in Rust, without rectangles

By default a skin only *addresses* widgets the application already created.
Create the widgets as before and let the document assign the rectangles:

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;
use bamboo_native_ui::widgets::{Button, Label};
use bamboo_native_ui::Ui;

struct App {
    ui: Ui<Msg>,
    clicks: u32,
    // The document currently installed; a reload replaces it.
    layout: LayoutDocument,
    // Timestamp seen at the last poll; `None` means the file was never read.
    modified: Option<std::time::SystemTime>,
}

fn build() -> App {
    let mut ui = Ui::new();
    // Widgets must exist before the document can address their ids.
    ui.add(Label::new("status", "No clicks yet"));
    ui.add(Button::new("greet", "Say hello", Msg::Greet));
    // include_str! bakes the file's text into the binary at build time, so the
    // first parse never touches the disk. The reload below reads it again.
    let layout = LayoutDocument::parse(include_str!("../layouts/main.buixml"))
        .expect("the embedded layout is valid");
    App {
        ui,
        clicks: 0,
        layout,
        modified: None,
    }
}
```

The ids in the file (`status`, `greet`) must match the ids passed to `new`. If
one does not exist, the apply reports `xml.id.unknown-widget` and changes
nothing.

XML is declarative: it describes *where* a widget sits and *how* it looks, but
it cannot invent a message handler or application state. The `Button` still
owns its `Msg::Greet` behaviour because Rust created it. That is why this page
creates the widgets in Rust and moves only the rectangles, style tokens and
initial labels into the document.

`include_str!` is a compile-time text include; the path is relative to the
source file that contains it (`src/main.rs`), not to the folder you run Cargo
in. If the file is missing when you build, the compiler stops with
`couldn't read ../layouts/main.buixml`. Keep that path unchanged and the
crate-folder location from step 1 is enough.

**What just happened.** `App` now carries a parsed `LayoutDocument`, and both
widgets exist without useful rectangles. Nothing visible changes until
`resize` applies the document in step 3.

### 2b. Optional — let the document create a widget

Install a `WidgetRegistry` to let `type="…"` entries create widgets whose ids
do not exist yet. The standard registry knows twelve types (`label`, `button`,
`toggle`, `slider`, `progress`, `icon`, `badge`, `card`, `divider`, `kbd`,
`avatar`, `spinner`); a third-party widget registers its own type name from
Rust. Interactive types need a `command`, which the application maps to its
message:

```rust
use bamboo_native_ui::creation::WidgetRegistry;

// A factory table: `type="label"` etc. becomes code that builds the widget.
let mut registry = WidgetRegistry::<Msg>::standard();
// Interactive created widgets report command ids; map them to your messages.
registry.set_commands(|id| match id {
    101 => Some(Msg::Greet),
    _ => None,
});
// Installed on the Ui; every apply consults it for missing ids.
ui.set_widget_registry(registry);
```

Add the created entries to the document (no matching `ui.add` in Rust):

```xml
<!-- No ui.add for this id: type="label" lets the registry create it. -->
<widget id="xml_hint" type="label"
        x="pad" y="112" w="cw-2*pad" h="24"
        label="Created by the document"/>

<!-- command="101" is turned into Msg::Greet by set_commands above. -->
<widget id="xml_button" type="button" command="101"
        right="pad" y="112" w="120" h="32"
        label="XML button"/>
```

On a reload, a created id that the new file still declares keeps running; one
that disappears is removed; changing its `type`, `command` or `parent`
replaces it. Created widgets are owned by the document and `clear_layout`
removes them. Other built-in widgets and your own `Widget<M>` types still need
a Rust factory — a `type` name only creates what the registry knows.

The registry is consulted only for a `type` entry whose id does not exist yet;
the widgets from step 2 are never replaced. `command` is a plain number, and
`set_commands` is the bridge from those numbers to your typed messages. Without
an installed registry, the same entries fail with `xml.type.unknown`, and the
apply changes nothing.

**What just happened.** This optional step gives the document a limited ability
to construct: two widgets whose ids exist only in XML. Their geometry, label
and command still flow through the same applier, so reload can add, replace or
remove them without recompiling Rust.

### 3. Apply it in `resize`

```rust
impl Host for App {
    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        // The document's colour tokens resolve against this palette.
        let palette = *window.palette();
        // Validate first, then move every id the document names.
        if let Err(failure) = self.ui.apply_layout(&self.layout, &palette, size, window) {
            eprintln!("layout rejected: {failure}");
        }
        // One repaint request covering the rectangles that changed.
        window.invalidate_all();
    }
}
```

`apply_layout(&document, &palette, client_size, sink)` resolves and validates
every id, rectangle and expression first. On any error it mutates nothing and
the previously installed layout stays active. On success it records one damage
union and keeps the result, which `ui.installed_layout()` exposes.

The `sink` is any `UiHost`; `WindowHandle` implements it. `window.palette()` is
the palette the window is painting with, and the document's `background` tokens
resolve against it.

`resize` is called by the framework at startup and again whenever the client
size changes, so this one method covers both the first layout and later
resizes. The `size` argument is the client area in DIPs, the same unit as the
document's numbers.

**What just happened.** At the first resize, the document replaced the widget
rectangles from page 03: the label spans the width minus two `pad` margins and
the button sits `pad` DIPs from the right edge. `radius="medium"` and
`background="accent"` restyled the button. If a line starting with
`layout rejected:` appears instead, the widgets kept the default rectangles
their constructors gave them, which usually leaves the window looking empty or
misplaced.

### 4. Reload on change

The library performs no file IO and starts no watcher. Poll the file and call
`parse` then `apply_layout`, keeping the old document when either step fails:

```rust
fn reload(&mut self, text: &str, window: &mut WindowHandle<'_>) {
    let document = match LayoutDocument::parse(text) {
        Ok(document) => document,
        Err(_) => return, // keep the working layout
    };
    if document.generation() == self.layout.generation() {
        return; // byte-for-byte the same canonical model
    }
    let palette = *window.palette();
    let size = window.size();
    match self.ui.apply_layout(&document, &palette, size, window) {
        Ok(_) => self.layout = document, // only remember a document that applied
        Err(failure) => eprintln!("layout rejected: {failure}"),
    }
}
```

`generation()` is a hash of the canonical model, so a comment-only edit does
nothing. Content initial values are written only when the generation changes;
a resize or a breakpoint switch never overwrites text the user typed.

Drive it from a timer, and read the file only when its modification time moves.
Resolve the path from the crate directory so the working directory does not
matter:

```rust
fn layout_path() -> std::path::PathBuf {
    // Fixed at compile time to the crate folder; the shell's cwd is irrelevant.
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("layouts/main.buixml")
}

impl Host for App {
    fn started(&mut self, window: &mut WindowHandle<'_>) {
        // A 250 ms heartbeat: four file checks a second, no painting by itself.
        window.set_animation_timer(Some(std::time::Duration::from_millis(250)));
    }

    fn timer(&mut self, window: &mut WindowHandle<'_>) {
        // metadata() is cheap; only a moved timestamp leads to a read.
        let Ok(modified) = std::fs::metadata(layout_path())
            .and_then(|meta| meta.modified())
        else {
            return; // file moved or unreadable: keep the running layout
        };
        if self.modified == Some(modified) {
            return; // same timestamp as the previous tick
        }
        self.modified = Some(modified);
        if let Ok(text) = std::fs::read_to_string(layout_path()) {
            self.reload(&text, window);
        }
    }
}
```

The timer wakes the message loop four times a second; it does not paint unless
the reload changes something. That keeps the "idle means zero paints" rule.

To drop the document and go back to application-owned rectangles, call
`ui.clear_layout(window)`. It hands every claimed field back and removes the
widgets the document created; rectangles and content are not restored.

**What just happened.** A save now reaches the running window: the timestamp
changes, the text is parsed, a new generation is applied and the changed
rectangles repaint. `generation()` filters noise, so saving without changing
the layout (or only editing a comment) causes no repaint.

**Run it.** Open a terminal in `bamboo-hello/` (the workspace folder that holds
the outer `Cargo.toml`) or in the crate folder `bamboo-hello/bamboo-hello/`;
both work. Then run:

```powershell
cargo run
```

A 520×320 window appears with the label at the top and the button at the
right. Leave it open, change `w="140"` to `w="200"` in
`layouts/main.buixml`, save, and the button widens within about a quarter of a
second. No rebuild and no restart.

### 5. What XML can and cannot do

Think of the document as configuration for widgets that already exist, plus the
narrow creation exception from step 2b. This list is the contract; everything
not listed stays in application code.

The document **can**:

- assign rectangles to existing widget ids: anchors, percentages, arithmetic,
  constants, and breakpoint variants selected by client size;
- set runtime properties: `visible`, `enabled`, `interactive`, `focusable`,
  `focus-order`, `read-only`, `tooltip`, `name`, `description`,
  `automation-id`, `role`, `expose`, `cursor`;
- set style tokens: `radius`, `corners`, `background` (a palette token,
  `#rrggbb`, or `none`), `font-size`, `font-family`;
- set content initial values: `text`, `label`, `value`, `progress`, `number`,
  `checked`, `selected`;
- describe registered scroll panels (`<scroll>` entries);
- set widget-declared local attributes through the skin protocol;
- supply widget data as child elements (`<item>`, `<row>`, `<option>`, `<track>`)
  for the widgets that declare those lists;
- create widgets, but only when the application installs a
  `WidgetRegistry` (`ui.set_widget_registry`) and the entry declares `type`
  (plus `command` for interactive types). A reload can add, replace or remove
  those created ids without recompiling Rust.

It **cannot**:

- create, remove, reorder or reparent widgets outside that registry;
- bind a widget's data live: child lists replace the data model on a generation
  change, they are not a two-way binding;
- install a whole palette (that is a `.buitheme` theme document); per-entry text
  colour exists only through the widget-declared `color` attribute on the text
  widgets that publish it (`Label` and its relatives, text fields), not as a
  generic token;
- use expressions in style or content values (content is a literal);
- bind content live: values are initial values, not bindings;
- include other files, use a DTD, or run a watcher on its own.

## Expected result

Running the program shows the same window with the label at the top and the
button on the right, both positioned by the file. Editing
`layouts/main.buixml`, for example changing the button's `w` to `200`, repaints
the window within a quarter of a second with no rebuild. Making a mistake (a
bad expression, an unknown id) logs the failure and leaves the last working
layout on screen.

The file on disk and the text embedded at build time are separate copies: the
first apply uses the embedded copy, and the timer reads the disk. Restarting
the program therefore starts from the version that was compiled in, not from
the last runtime edit. Runtime reloads never write the file back.

## Troubleshooting

- **`LayoutDocument` or `layout_xml` does not resolve** — the `layout-xml`
  feature is not enabled. Check `Cargo.toml` and the exact spelling; Cargo
  rejects an unknown feature rather than ignoring it.
- **`xml.id.unknown-widget`** — the document names an id that was not added with
  `ui.add`. The apply is atomic, so nothing moves. Compare the `id` attributes
  with the strings passed to `new`; ids are case-sensitive.
- **`xml.layout.base`** — no `<layout>` has `min-width="0" min-height="0"`, or
  two variants share minimums. Keep a `0,0` fallback variant.
- **`xml.geom.underdetermined`** — an entry has neither `w`/`h` nor both edges.
  Every entry must determine a rectangle; there is no measure pass.
- **`xml.syntax` with a line and column** — the XML itself is malformed: a tag
  is not closed, an attribute value is missing its quotes, or an empty element
  lost its `/`. The parser stops at the first one. During a reload the running
  window keeps its last good layout; on the initial parse the `.expect` in
  `build` panics, so fix the file before `cargo run`.
- **`xml.unknown-attribute` or `xml.unknown-element`** — a typo in a name. Both
  names are case-sensitive; `background` is not `Background`.
- **Nothing changes when you edit the file** — first confirm the editor saved.
  Many editors keep the text in memory until you press Ctrl+S, and some rename
  the file to `main.buixml.txt` when the extension is unfamiliar. Check the
  terminal: a parse error there explains why the old layout stayed.
- **Edits never reach the window** — the timer reads
  `<crate>/layouts/main.buixml`. A file saved into `src/` or into the workspace
  folder is a different file. The build-time `include_str!` path and the
  runtime path must point at the same file.
- **The window is empty or misplaced instead of showing the labels** — every
  apply failed, so the widgets kept their constructor rectangles. Read the
  first `layout rejected:` line; it names the error code and the document
  position.
- **The text reverts after an edit** — a save changed the generation, and content
  initial values were rewritten. That is the intended timing; remove the
  content attribute if the application owns the text.
- **Clicking the button does nothing** — the document can set
  `enabled="false"` or `interactive="false"`, and your page-03 `apply` still
  has to turn `Msg::Greet` into the label change. Check both.

## Next

[06 — Compiled XML](06-xml-compiled.md): generate the same layout as Rust and
drop the runtime parser.
