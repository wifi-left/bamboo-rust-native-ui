# 04 — Custom widgets

> Paint an application-specific component two ways: closure-only
> `CustomWidget`, or a manual `Widget<M>` implementation with owned state.

## What you will build

A `LevelBar`: a rounded track with a filled portion, which turns to the accent
colour on hover and emits `Msg::BarClicked` when clicked. You will add it to the
window from [03 — First window](03-first-window.md); the rest of that program is
unchanged.

Two paths are shown because they fit different needs:

- `CustomWidget` for a component that is a rectangle, some drawing and an input
  handler.
- A manual `Widget<M>` implementation when the component owns state that paint
  and input must share, or needs to publish XML skin attributes.

Both paths register the widget with `ui.add` and give it an id that `resize` and
`set_rect` use later. The difference is where state lives: a `CustomWidget`
closure has no field of its own, so the Path A fill is fixed at 60%; the manual
widget keeps `level` in a struct field, which is what a skin attribute can
update. Start with Path A unless you need owned state or XML attributes.

The two paths are alternatives: each registers a widget with the id `"level"`,
and each has its own `use` line that brings in the same names. Pick one; pasting
both into one file produces duplicate imports and two bars.

## New terms on this page

- **widget** — here, the four things the framework needs: a stable `WidgetId`,
  a rectangle in DIPs, a `paint` method and an `input` method.
- **`CustomWidget`** — a builder that turns one paint closure (and an optional
  input closure) into a widget; good for simple components that own no mutable
  state.
- **`Widget<M>`** — the trait for a manually implemented widget; the type owns
  its fields, so paint and input can share state.
- **closure** — an inline function written at the point of use, such as
  `|ctx, rect, state| { ... }`; it can read nearby variables.
- **`Option<M>`** — zero or one message: `Some(message)` reports something,
  `None` reports nothing.
- **`WidgetInput`** — normalized input delivered to a widget, for example
  `ClickAt(position)` or `Activate`.
- **`WidgetState`** — the interaction state the container passes to paint:
  `hovered`, `pressed`, `focused`, `enabled`, `editing` and `caret_on`.
- **hit test** — deciding whether a point belongs to a widget; rectangular by
  default, replaceable with `.hit_test`.
- **a11y** — accessibility: the role and name published to screen readers.
- **`WidgetId`** — the stable id used by `set_rect`, `with_widget` and focus.
- **skin attribute** — an optional per-instance value a widget declares so a
  `.buixml` document can set it; the widget validates and applies it.
- **`.buixml`** — a layout document (page 05) that can address widgets by id and
  set their declared attributes.

## Prerequisites

- The window from [03](03-first-window.md) builds and runs.
- No extra feature flag is needed; `CustomWidget` and `Widget` are always
  compiled.

## Steps

### 1. Extend the message type

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    Greet,      // Existing button message from page 03.
    BarClicked, // Sent when the level bar is clicked.
}
```

Keep the `apply` method's `match` exhaustive: add the new arm from step 2, or the
compiler reports a missing case.

### 2. Path A — `CustomWidget`

`CustomWidget::new(id, paint)` takes one paint closure; `.input(handler)` adds an
input closure that returns `Option<M>`. A closure is an inline function written
where it is used; here the first receives the paint context, the widget
rectangle and the container's `WidgetState`, and the second receives the
rectangle and one `WidgetInput`. Register the result like any widget. Put this
helper at the bottom of `main.rs`, next to `main`:

```rust
// The closure-painted widget type.
use bamboo_native_ui::widgets::CustomWidget;
// Geometry and input types used by the closures below.
use bamboo_native_ui::{Rect, WidgetId, WidgetInput};

// Module-level helper: add the bar once, from App::new.
fn add_level_bar(ui: &mut bamboo_native_ui::Ui<Msg>) {
    ui.add(
        // Paint closure: ctx has canvas/painter, rect is the widget bounds,
        // state carries hover/press/focus from the container.
        CustomWidget::new("level", |ctx, rect, state| {
            let palette = ctx.palette;
            let radius = rect.height() / 2.0; // Pill shape.
            ctx.painter
                // Track behind the fill.
                .round_rect(ctx.canvas, rect, radius, palette.scroll_track);
            // Fixed 60% fill for this demo; drawn inside rect.
            let filled = Rect::from_xywh(rect.left, rect.top, rect.width() * 0.6, rect.height());
            let color = if state.hovered {
                palette.accent // The container reports hover; nothing local.
            } else {
                palette.scroll_thumb
            };
            ctx.painter.round_rect(ctx.canvas, filled, radius, color);
        })
        // Initial rectangle; a later set_rect can replace it.
        .rect(Rect::from_xywh(24.0, 120.0, 240.0, 12.0))
        // Required for pointer input; CustomWidget defaults to false.
        .interactive(true)
        // Input closure: return zero or one message.
        .input(|_rect, input| match input {
            WidgetInput::ClickAt(_) => Some(Msg::BarClicked),
            _ => None, // Ignore other input.
        }),
    );
}
```

`Rect` and `WidgetInput` are used in the two closures. `WidgetId` is in the same
import because Path B uses it; if you stop after Path A, the compiler may warn
that the import is unused.

Notes:

- `CustomWidget` defaults to `focusable(false)` and `interactive(false)`, so a
  display-only component never steals clicks. Call `.interactive(true)` to
  receive pointer input, and `.focusable(true)` to join the Tab order (the
  container then paints the focus ring).
- The paint closure receives the widget rectangle and a `WidgetState` with
  `hovered`, `pressed`, `focused`, `enabled`, `editing` and `caret_on`.
- The other builders are `preferred_height`, `enabled`, `cursor`, `hit_test`
  (a non-rectangular hit area), `a11y` and the surface builders
  (`.corner_radius`, `.background`).

Call `add_level_bar(&mut ui)` in `App::new`, after the label and button are
added; `App::new` has no `self`, so pass its local `ui`. Then handle the message
in `App::apply`:

```rust
// Inside the match in App::apply, next to the Msg::Greet arm.
Msg::BarClicked => {
    // Reuse the status label from page 03.
    self.ui.set_label(WidgetId::new("status"), "Bar clicked");
}
```

### 3. Path B — implement `Widget<M>`

The trait's required methods are `id`, `rect`, `set_rect`, `as_any`,
`as_any_mut`, `paint` and `input`. Everything else has a default.

`id` identifies the widget; `rect`/`set_rect` expose its bounds to the container;
`paint` draws inside those bounds; `input` returns `Option<Msg>`. The two
`as_any` methods support downcasting, which is how `Ui::with_widget` and
`Ui::edit_widget` recover your concrete `LevelBar` type from a `dyn Widget<Msg>`.
Here the type owns `level`, so paint and input share it through `&self` and
`&mut self`.

```rust
// The widget trait and the types its methods use.
use bamboo_native_ui::{Rect, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

struct LevelBar {
    id: WidgetId, // Stable identity for set_rect, focus and accessibility.
    rect: Rect,   // Current bounds in DIPs.
    level: f32,   // Owned state shared by paint and input (0.0..=1.0).
}

impl LevelBar {
    fn new(id: &'static str, level: f32) -> Self {
        Self {
            id: WidgetId::new(id),
            // Nominal size; resize() replaces the rectangle.
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 12.0),
            level,
        }
    }
}

// Implement the framework's widget interface; the trait defaults the rest.
impl Widget<Msg> for LevelBar {
    fn id(&self) -> WidgetId {
        self.id
    }

    // The container hit-tests with this rectangle.
    fn rect(&self) -> Rect {
        self.rect
    }

    // Called by the application, usually from resize.
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    // Downcast support for Ui::with_widget.
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    // Downcast support for Ui::edit_widget.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    // Draw only inside self.rect; the host blits only that area.
    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        let radius = self.rect.height() / 2.0; // Pill shape.
        ctx.painter
            // Track behind the fill.
            .round_rect(ctx.canvas, self.rect, radius, palette.scroll_track);
        // Clamp so an out-of-range level stays inside the track.
        let width = self.rect.width() * self.level.clamp(0.0, 1.0);
        let filled = Rect::from_xywh(self.rect.left, self.rect.top, width, self.rect.height());
        let color = if state.hovered {
            palette.accent // Hover state comes from the container.
        } else {
            palette.scroll_thumb
        };
        ctx.painter.round_rect(ctx.canvas, filled, radius, color);
    }

    // Return one message for a click, None for everything else.
    fn input(&mut self, input: WidgetInput) -> Option<Msg> {
        match input {
            WidgetInput::ClickAt(_) => Some(Msg::BarClicked),
            _ => None,
        }
    }
}
```

Register it in `App::new` and position it in `App::resize`, like a built-in:

```rust
// In App::new, next to the label and button.
self.ui.add(LevelBar::new("level", 0.6)); // 60% fill.
// In App::resize, next to the other set_rect calls:
self.ui.set_rect(
    WidgetId::new("level"), // Same id as LevelBar::new.
    Rect::from_xywh(24.0, 120.0, size.width - 48.0, 12.0),
);
```

`as_any`/`as_any_mut` are what `Ui::with_widget` and `Ui::edit_widget` use to
reach the concrete type, so return `self`.

### 4. Optional — declare XML skin attributes

A widget can accept local attributes from a `.buixml` document. The skin
protocol is the agreement that makes this safe: the widget publishes the
attribute names and value types it understands, the parser validates a document
against that list, and only then does the widget apply the value. This lets a
document author tune one instance (`level="0.8"`) without a Rust change or a
recompile. The defaults declare nothing, so this is opt-in and the widget still
works when created from Rust; the declaration compiles without
`feature = "layout-xml"`.

```rust
// Types for declaring and receiving .buixml attributes.
use bamboo_native_ui::{SkinAttrDecl, SkinValue, SkinValueKind};

// Declares "level" as numeric. The parser rejects other types for this name.
static LEVEL_ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("level", SkinValueKind::Number)];

// These methods go inside the `impl Widget<Msg> for LevelBar` block:
// Publish the attribute names this widget accepts.
fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
    LEVEL_ATTRS
}

fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
    // Ignore names this widget does not declare.
    if name != "level" {
        return;
    }
    // Check the value kind before using it.
    if let SkinValue::Number(level) = value {
        self.level = (*level as f32).clamp(0.0, 1.0);
    }
}

// No data child elements.
fn skin_children(&self) -> &'static [&'static str] {
    &[]
}
```

The document can then write
`<widget id="level" x="16" y="120" w="240" h="12" level="0.8"/>`. Related
methods: `validate_skin_attr` (side-effect-free check before anything is
applied), `skin_children`, `validate_skin_children` and `apply_skin_children`
(for data child elements). Attribute names starting with `x-` are reserved for
application code. The full protocol is in
[Custom widget](../widgets/custom/custom-widget.md).

### 5. Run it

Run this in the outer `bamboo-hello` folder, the one with the workspace
`Cargo.toml`:

```powershell
# Build if needed and open the window; close it to return to the prompt.
cargo run
```

Hover the bar: the filled portion switches to the accent colour. Click it: the
status label changes to `Bar clicked`. Click again: it stays at `Bar clicked`,
because this page maps the message to a fixed string rather than a counter.

If the window does not appear, the compile error or the `bamboo: ...` line is in
this terminal; a window cannot show errors.

**What just happened.** The widget was added to the same `Ui` as the label and
button, so it takes part in hit-testing, dirty rectangles and paint exactly like
a built-in. Its input handler returned a message, and `apply` changed the label.

## Expected result

A rounded bar under the button, 240 DIPs wide and 12 DIPs tall in Path A. Hover
changes only the bar's rectangle; the click emits a message that the application
turns into a label change. The label repaint and the bar repaint are separate
dirty rectangles, which is why hovering does not redraw the button.

## Troubleshooting

- **The bar is invisible** — the paint closure drew outside `rect`, or the
  rectangle is empty. A widget must paint inside its own rectangle; the host
  only blits that region. Check that `set_rect` ran for the id `"level"` and
  that the rectangle is at y = 120, inside the 320-DIP-tall client.
- **Clicks do nothing** — `CustomWidget` defaults to non-interactive. Add
  `.interactive(true)`. For a manual implementation, do not override
  `focusable`/`interactive` to `false` unless you handle input yourself. The bar
  is only 12 DIPs tall, so aim inside it; a click on the surrounding background
  is outside the widget.
- **`with_widget` returns `None`** — the widget was added as the wrong type, or
  `as_any` returns something other than `self`.
- **The hover highlight stays after the pointer leaves** — the container does
  not know the widget moved. Return the correct rectangle from `rect()` and call
  `set_rect` when it changes.
- **A compile error such as "expected one of ..." after pasting** — the methods
  belong inside the `impl` blocks. The skin helpers (`skin_attrs`,
  `apply_skin_attr`, `skin_children`) go inside `impl Widget<Msg> for LevelBar`,
  and `add_level_bar` is a free function at module level, not a method of `App`.
- **`cannot find function add_level_bar`** — the helper is outside the current
  scope, or it was placed inside `impl App`. Put it next to `main` and call it
  from `App::new` with the local `ui`: `add_level_bar(&mut ui);`.
- **"non-exhaustive patterns: `Msg::BarClicked` not covered"** — the `match` in
  `App::apply` still has only the old arm. Add the `Msg::BarClicked` arm.
- **The window opens but the bar never appeared** — `add_level_bar` was defined
  but not called, or no rectangle reached the widget. The widget is registered
  by `add`; its rectangle comes from `CustomWidget`'s `.rect(...)` or from a
  later `set_rect`. Ensure one of them supplies a non-empty rectangle.
- **Hover works but clicking does not change the label** — the input closure is
  missing or returns `None`, the message arm is missing, or `flush_damage` was
  removed. Each step is needed: closure → queued message → `apply` → dirty
  label → flush.
- **`unused import: WidgetId` warning** — expected if you implemented only
  Path A. Either delete `WidgetId` from that import or keep Path B, which uses
  it.
- **The XML `level` attribute has no effect** — the document requires
  `feature = "layout-xml"` (page 05), and the name must be declared in
  `skin_attrs`. Names starting with `x-` belong to application code, not the
  widget.
- **The old window still appears after an edit** — the editor has not saved
  `main.rs`, or the previous program is still open. Save with Ctrl+S, close the
  window from the previous run, and start `cargo run` again.
- **The link step fails with "link.exe not found"** — the C++ build tools or
  Windows SDK are missing. Install the **Desktop development with C++**
  workload and open a new terminal; page 01 has the details.
- **`cargo` is not recognized, or `could not find Cargo.toml`** — use the outer
  `bamboo-hello` folder and a terminal opened after installing Rust; the same
  checks as page 01 apply here.

## Next

[05 — Dynamic XML](05-xml-dynamic.md): move the rectangles and some properties
out of Rust into a `.buixml` file.
