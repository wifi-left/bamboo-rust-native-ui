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

## Prerequisites

- The window from [03](03-first-window.md) builds and runs.
- No extra feature flag is needed; `CustomWidget` and `Widget` are always
  compiled.

## Steps

### 1. Extend the message type

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Msg {
    Greet,
    BarClicked,
}
```

### 2. Path A — `CustomWidget`

`CustomWidget::new(id, paint)` takes one paint closure; `.input(handler)` adds an
input closure that returns `Option<M>`. Register it like any widget:

```rust
use bamboo_native_ui::widgets::CustomWidget;
use bamboo_native_ui::{Rect, WidgetId, WidgetInput};

fn add_level_bar(ui: &mut bamboo_native_ui::Ui<Msg>) {
    ui.add(
        CustomWidget::new("level", |ctx, rect, state| {
            let palette = ctx.palette;
            let radius = rect.height() / 2.0;
            ctx.painter
                .round_rect(ctx.canvas, rect, radius, palette.scroll_track);
            let filled = Rect::from_xywh(rect.left, rect.top, rect.width() * 0.6, rect.height());
            let color = if state.hovered {
                palette.accent
            } else {
                palette.scroll_thumb
            };
            ctx.painter.round_rect(ctx.canvas, filled, radius, color);
        })
        .rect(Rect::from_xywh(24.0, 120.0, 240.0, 12.0))
        .interactive(true)
        .input(|_rect, input| match input {
            WidgetInput::ClickAt(_) => Some(Msg::BarClicked),
            _ => None,
        }),
    );
}
```

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

Call `add_level_bar(&mut self.ui)` from `App::new` and handle the message:

```rust
Msg::BarClicked => {
    self.ui.set_label(WidgetId::new("status"), "Bar clicked");
}
```

### 3. Path B — implement `Widget<M>`

The trait's required methods are `id`, `rect`, `set_rect`, `as_any`,
`as_any_mut`, `paint` and `input`. Everything else has a default.

```rust
use bamboo_native_ui::{Rect, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

struct LevelBar {
    id: WidgetId,
    rect: Rect,
    level: f32,
}

impl LevelBar {
    fn new(id: &'static str, level: f32) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 12.0),
            level,
        }
    }
}

impl Widget<Msg> for LevelBar {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let palette = ctx.palette;
        let radius = self.rect.height() / 2.0;
        ctx.painter
            .round_rect(ctx.canvas, self.rect, radius, palette.scroll_track);
        let width = self.rect.width() * self.level.clamp(0.0, 1.0);
        let filled = Rect::from_xywh(self.rect.left, self.rect.top, width, self.rect.height());
        let color = if state.hovered {
            palette.accent
        } else {
            palette.scroll_thumb
        };
        ctx.painter.round_rect(ctx.canvas, filled, radius, color);
    }

    fn input(&mut self, input: WidgetInput) -> Option<Msg> {
        match input {
            WidgetInput::ClickAt(_) => Some(Msg::BarClicked),
            _ => None,
        }
    }
}
```

Register and position it like a built-in:

```rust
self.ui.add(LevelBar::new("level", 0.6));
// in resize:
self.ui.set_rect(
    WidgetId::new("level"),
    Rect::from_xywh(24.0, 120.0, size.width - 48.0, 12.0),
);
```

`as_any`/`as_any_mut` are what `Ui::with_widget` and `Ui::edit_widget` use to
reach the concrete type, so return `self`.

### 4. Optional — declare XML skin attributes

A widget can accept local attributes from a `.buixml` document. Publish the
names with `skin_attrs`, validate each value, and apply it. The defaults
declare nothing, so this is opt-in and works without
`feature = "layout-xml"`.

```rust
use bamboo_native_ui::{SkinAttrDecl, SkinValue, SkinValueKind};

static LEVEL_ATTRS: &[SkinAttrDecl] = &[SkinAttrDecl::new("level", SkinValueKind::Number)];

// These methods go inside the `impl Widget<Msg> for LevelBar` block:
fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
    LEVEL_ATTRS
}

fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
    if name != "level" {
        return;
    }
    if let SkinValue::Number(level) = value {
        self.level = (*level as f32).clamp(0.0, 1.0);
    }
}

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

```powershell
cargo run
```

Hover the bar: the filled portion switches to the accent colour. Click it: the
status label changes to `Bar clicked`.

## Expected result

A rounded bar under the button. Hover changes only the bar's rectangle; the
click emits a message that the application turns into a label change. The label
repaint and the bar repaint are separate dirty rectangles.

## Troubleshooting

- **The bar is invisible** — the paint closure drew outside `rect`, or the
  rectangle is empty. A widget must paint inside its own rectangle; the host
  only blits that region.
- **Clicks do nothing** — `CustomWidget` defaults to non-interactive. Add
  `.interactive(true)`. For a manual implementation, do not override
  `focusable`/`interactive` to `false` unless you handle input yourself.
- **`with_widget` returns `None`** — the widget was added as the wrong type, or
  `as_any` returns something other than `self`.
- **The hover highlight stays after the pointer leaves** — the container does
  not know the widget moved. Return the correct rectangle from `rect()` and call
  `set_rect` when it changes.

## Next

[05 — Dynamic XML](05-xml-dynamic.md): move the rectangles and some properties
out of Rust into a `.buixml` file.
