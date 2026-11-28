# Testing

> Run the workspace test matrix, keep the generated layout modules current, and
> unit-test a widget without opening a window.

## What can be tested without a window

Everything above `Painter` and `Canvas` is platform-neutral, so widget logic,
layout parsing, layout application, accessibility snapshots and the compiled
layout descriptor run in normal `cargo test`. Only the `#[cfg(windows)]` host
(`Host`, `WindowBuilder`, `WindowHandle`, the GDI/Direct2D backends and the UIA
provider) needs the showcase-style probes.

| Layer | Test style |
|---|---|
| A widget's paint/input/a11y | A fake `Canvas` plus `WidgetPaint` |
| Parser and model | `bamboo-native-ui-layout-core` unit tests |
| Dynamic apply | `Ui::apply_layout` with a `TestHost` sink |
| Compiled apply | Checked-in generated module plus the same `TestHost` |
| Host, UIA, IME | Showcase `--self-test` and `--a11y-dump` |

## Steps

### 1. Run the matrix

```powershell
cargo build
cargo test -p bamboo-native-ui
cargo test -p bamboo-native-ui-layout-core --features parse
cargo test -p bamboo-native-ui --features layout-xml
cargo test -p bamboo-native-ui --features theme-xml
cargo test -p bamboo-native-ui --features debug-form
cargo test -p bamboo-native-ui --features layout-compiled
cargo test -p bamboo-native-ui --features layout-xml,layout-compiled
cargo test -p bamboo-native-ui-layout-codegen
cargo test -p bamboo-native-ui-layout-macros
```

The default suite covers the widget library and the container. Each feature
suite adds its own tests:

- `layout-xml` adds parser/applier tests plus the integration tests
  `crates/native-ui/tests/layout_parity.rs` (dynamic and compiled modes produce
  the same result), `layout_rust.rs` (inline `rust:` values in compiled mode,
  dynamic rejection), `layout_context.rs` (`context = Type` modules) and
  `layout_create.rs` (document-created widgets).
- `theme-xml` adds the `.buitheme` round trip, base/accent derivation and
  token-override tests.
- `debug-form` adds snapshot and diagnostics-ring tests.
- `layout-compiled` adds the descriptor and compiled-apply tests; it links no
  XML parser.
- The codegen crate pins the checked-in generated modules; the macro crate
  checks `layout_xml!` expansion errors.

On a non-Windows CI target, also build with `--no-default-features`: it removes
the GDI backend (`feature = "gdi"`) and must still compile the portable layer.
On Windows the platform layer needs `gdi`, so keep the default feature there.

### 2. Keep generated modules current

The codegen golden test fails when a checked-in generated file is stale.
Regenerate after editing a fixture:

```powershell
cargo run -p bamboo-native-ui-layout-codegen -- crates/native-ui/tests/fixtures/parity.buixml -o crates/native-ui/tests/fixtures/parity_generated.rs
cargo run -p bamboo-native-ui-layout-codegen -- crates/native-ui/tests/fixtures/rust.buixml --module rust_layout -o crates/native-ui/tests/fixtures/rust_generated.rs
cargo run -p bamboo-native-ui-layout-codegen -- crates/native-ui/tests/fixtures/context.buixml --module context_layout --context AppLayout -o crates/native-ui/tests/fixtures/context_generated.rs
cargo run -p bamboo-native-ui-layout-codegen -- crates/native-ui/tests/fixtures/binding.buixml --module binding_layout -o crates/native-ui/tests/fixtures/binding_generated.rs
cargo run -p bamboo-native-ui-layout-codegen -- crates/native-ui/tests/fixtures/create.buixml --module create_layout -o crates/native-ui/tests/fixtures/create_generated.rs
cargo test -p bamboo-native-ui-layout-codegen
```

A document error makes the CLI exit 1, print the diagnostic with its `.buixml`
line and column, and write nothing.

### 3. Test a widget's paint and input

The backend trait is small enough to stub. The stub below measures 8 DIPs per
character and records nothing.

```rust
use bamboo_native_ui::backend::{Canvas, ImageData, ImageId, TextAlign, TextStyle};
use bamboo_native_ui::geometry::{Rect, Size};
use bamboo_native_ui::{Color, ImageStore, Painter, Widget, WidgetInput, WidgetPaint};

#[derive(Default)]
struct BlankCanvas;

impl Canvas for BlankCanvas {
    fn scale(&self) -> f32 { 1.0 }
    fn dpi(&self) -> f32 { 96.0 }
    fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
    fn text(
        &mut self,
        _rect: Rect,
        _text: &str,
        _style: &TextStyle,
        _align: TextAlign,
        _color: Color,
    ) {
    }
    fn measure_text(&mut self, text: &str, _style: &TextStyle) -> Size {
        Size::new(text.chars().count() as f32 * 8.0, 16.0)
    }
    fn ellipse(&mut self, _rect: Rect, _color: Color) {}
    fn vertical_gradient(&mut self, _rect: Rect, _top: Color, _bottom: Color) {}
    fn image(&mut self, _rect: Rect, _crop: bool, _id: ImageId, _data: ImageData<'_>) {}
    fn push_clip(&mut self, _rect: Rect) {}
    fn pop_clip(&mut self) {}
}

#[test]
fn a_click_flips_the_checkbox() {
    use bamboo_native_ui::widgets::Checkbox;
    use bamboo_native_ui::{Point, WidgetState};

    #[derive(Debug, PartialEq)]
    enum Msg {
        Remember(bool),
    }

    let mut check = Checkbox::new("remember", "Remember me", true, Msg::Remember);
    check.set_rect(Rect::from_xywh(0.0, 0.0, 180.0, 28.0));

    let painter = Painter::new();
    let palette = *painter.palette();
    let images = ImageStore::new();
    let mut canvas = BlankCanvas;
    let mut paint = WidgetPaint {
        painter: &painter,
        canvas: &mut canvas,
        palette,
        font: None,
        images: &images,
        viewport: Rect::from_xywh(0.0, 0.0, 400.0, 300.0),
        dirty: &[Rect::from_xywh(0.0, 0.0, 180.0, 28.0)],
    };
    check.paint(&mut paint, &WidgetState { enabled: true, ..Default::default() });

    let message = check.input(WidgetInput::ClickAt(Point::new(8.0, 14.0)));
    assert_eq!(message, Some(Msg::Remember(false)));
    assert!(!check.checked());
}
```

The same pattern tests `Activate`, `Key`, `Wheel`, `Step` and the hover inputs.
`WidgetState` carries `hovered`, `pressed`, `focused`, `enabled`, `editing` and
`caret_on`; set the ones your paint path reads.

For paint assertions, replace `BlankCanvas` with a recorder that pushes
`(Rect, Color)` into a `Vec` in `fill_rect` and `(Rect, String)` in `text`, as
the built-in widget tests do, then assert the recorded geometry and colors.

### 4. Test the container and a layout

A `Ui` test needs a sink implementing `Invalidator` (required `invalidate`) and
`UiHost` (all methods defaulted).

```rust
use bamboo_native_ui::geometry::Rect;
use bamboo_native_ui::{Invalidator, Ui, UiHost, WidgetId};

#[derive(Default)]
struct TestHost {
    invalidated: Vec<Rect>,
}

impl Invalidator for TestHost {
    fn invalidate(&mut self, rect: Rect) {
        self.invalidated.push(rect);
    }
}

impl UiHost for TestHost {}

#[test]
fn a_label_change_records_one_damage_rect() {
    let mut ui: Ui<()> = Ui::new();
    ui.add(bamboo_native_ui::widgets::Label::new("title", "Old"));
    ui.set_rect(WidgetId::new("title"), Rect::from_xywh(0.0, 0.0, 100.0, 20.0));
    ui.set_label(WidgetId::new("title"), "New");

    let mut host = TestHost::default();
    assert!(ui.flush_damage(&mut host));
    assert_eq!(host.invalidated.len(), 1);
}
```

For layout tests, parse a document, call `ui.apply_layout(&document, &palette,
Size::new(...), &mut host)` and assert the resolved rectangles, warnings,
`ui.installed_layout()`, the a11y tree (`ui.a11y_nodes()`) or the debug snapshot.
The existing `layout_parity.rs` shows the full pattern: one fixture applied
through both modes, then a structural comparison of rectangles, properties,
content and nodes.

### 5. Test the skin protocol

If a widget declares local attributes or child lists, test both phases:

```rust
use bamboo_native_ui::widgets::Card;
use bamboo_native_ui::{SkinValue, Widget};

#[test]
fn a_skin_attribute_reaches_the_widget() {
    let mut card = Card::new("card", "Old title", "Body");
    assert!(<Card as Widget<()>>::validate_skin_attr(
        &card,
        "title",
        &SkinValue::Text("New title"),
    ));
    <Card as Widget<()>>::apply_skin_attr(&mut card, "title", &SkinValue::Text("New title"));
    assert_eq!(card.title(), "New title");
}
```

The same shape tests a custom widget: validate a value, apply it, and read the
typed getter. `validate_skin_attr` must be side-effect free and must agree with
`apply_skin_attr`: a value that validates must apply. The applier relies on
that to keep a document atomic.

### 6. Run the host probes

The portable tests cannot cover the window procedure, native edit controls,
IME, menus, tray or UIA. That is what the showcase probes are for:

```powershell
cargo run -p bamboo-native-ui-showcase -- --self-test
cargo run -p bamboo-native-ui-showcase -- --a11y-dump
cargo run -p bamboo-native-ui-showcase --no-default-features -- --self-test
cargo run -p bamboo-native-ui-showcase --no-default-features -- --a11y-dump
```

`--self-test` paints the showcase, drives a native edit session and exits 0 on
success. `--a11y-dump` attaches a real UIA client and prints the tree; a
non-zero exit means the probe failed. Run both before a release and after a
platform-layer change. For a startup crash, add `BAMBOO_UI_DEBUG=1` (see
[Debugging](debugging.md)).

### 7. Add a test where the bug is

| Bug | Test location |
|---|---|
| Widget paints wrong / message wrong | The widget's `#[cfg(test)] mod tests` with a fake canvas |
| Container damage/focus/scroll | `ui.rs` unit tests with `TestHost` |
| Parser accepts/rejects something | `bamboo-native-ui-layout-core` parser tests |
| Apply changes nothing on error | `layout_*` integration tests |
| Generated code is stale | `crates/layout-codegen/tests/golden.rs` |
| Windows-only callback or UIA | The showcase probe |

Keep parser tests on the diagnostic code, not on the message text: codes are
stable, messages are developer-facing and may change.

## See also

- [Debugging](debugging.md)
- [Feature flags](feature-flags.md)
- [XML layout authoring](xml-layout-authoring.md)
- [The widget trait](../api/widget-trait.md)
- [Painting and Canvas](../api/painting-and-canvas.md)
- [Host and window](../api/host-and-window.md)
