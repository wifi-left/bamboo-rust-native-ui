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

## Prerequisites

- The project from [03](03-first-window.md) builds and runs.
- Enable the layout feature in `bamboo-hello/Cargo.toml`:

```toml
[dependencies]
bamboo-native-ui = { path = "../native-ui/crates/native-ui", features = ["layout-xml"] }
```

`layout-xml` adds the parser (`LayoutDocument::parse`) and the applier
(`Ui::apply_layout`). It does not change any existing API.

## Steps

### 1. Create the document

Create `bamboo-hello/bamboo-hello/layouts/main.buixml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <const name="pad" value="24"/>

  <layout id="base" min-width="0" min-height="0">
    <widget id="status"
            x="pad" y="24" w="cw-2*pad" h="28"
            label="No clicks yet"/>

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
    layout: LayoutDocument,
    modified: Option<std::time::SystemTime>,
}

fn build() -> App {
    let mut ui = Ui::new();
    ui.add(Label::new("status", "No clicks yet"));
    ui.add(Button::new("greet", "Say hello", Msg::Greet));
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

### 2b. Optional — let the document create a widget

Install a `WidgetRegistry` to let `type="…"` entries create widgets whose ids
do not exist yet. The standard registry knows twelve types (`label`, `button`,
`toggle`, `slider`, `progress`, `icon`, `badge`, `card`, `divider`, `kbd`,
`avatar`, `spinner`); a third-party widget registers its own type name from
Rust. Interactive types need a `command`, which the application maps to its
message:

```rust
use bamboo_native_ui::creation::WidgetRegistry;

let mut registry = WidgetRegistry::<Msg>::standard();
registry.set_commands(|id| match id {
    101 => Some(Msg::Greet),
    _ => None,
});
ui.set_widget_registry(registry);
```

Add the created entries to the document (no matching `ui.add` in Rust):

```xml
<widget id="xml_hint" type="label"
        x="pad" y="112" w="cw-2*pad" h="24"
        label="Created by the document"/>

<widget id="xml_button" type="button" command="101"
        right="pad" y="112" w="120" h="32"
        label="XML button"/>
```

On a reload, a created id that the new file still declares keeps running; one
that disappears is removed; changing its `type`, `command` or `parent`
replaces it. Created widgets are owned by the document and `clear_layout`
removes them. Other built-in widgets and your own `Widget<M>` types still need
a Rust factory — a `type` name only creates what the registry knows.

### 3. Apply it in `resize`

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

`apply_layout(&document, &palette, client_size, sink)` resolves and validates
every id, rectangle and expression first. On any error it mutates nothing and
the previously installed layout stays active. On success it records one damage
union and keeps the result, which `ui.installed_layout()` exposes.

The `sink` is any `UiHost`; `WindowHandle` implements it. `window.palette()` is
the palette the window is painting with, and the document's `background` tokens
resolve against it.

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
        Ok(_) => self.layout = document,
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
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("layouts/main.buixml")
}

impl Host for App {
    fn started(&mut self, window: &mut WindowHandle<'_>) {
        window.set_animation_timer(Some(std::time::Duration::from_millis(250)));
    }

    fn timer(&mut self, window: &mut WindowHandle<'_>) {
        let Ok(modified) = std::fs::metadata(layout_path())
            .and_then(|meta| meta.modified())
        else {
            return;
        };
        if self.modified == Some(modified) {
            return;
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

### 5. What XML can and cannot do

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

## Troubleshooting

- **`LayoutDocument` or `layout_xml` does not resolve** — the `layout-xml`
  feature is not enabled. Check `Cargo.toml`.
- **`xml.id.unknown-widget`** — the document names an id that was not added with
  `ui.add`. The apply is atomic, so nothing moves.
- **`xml.layout.base`** — no `<layout>` has `min-width="0" min-height="0"`, or
  two variants share minimums. Keep a `0,0` fallback variant.
- **`xml.geom.underdetermined`** — an entry has neither `w`/`h` nor both edges.
  Every entry must determine a rectangle; there is no measure pass.
- **The text reverts after an edit** — a save changed the generation, and content
  initial values were rewritten. That is the intended timing; remove the
  content attribute if the application owns the text.

## Next

[06 — Compiled XML](06-xml-compiled.md): generate the same layout as Rust and
drop the runtime parser.
