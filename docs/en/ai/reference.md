# Bamboo Native UI — AI reference

A single self-contained context file for AI assistants. It mirrors the public
surface of the workspace at `F:\codes\rust\mp3player\native-ui`. When this file
and the source disagree, the source wins.

## Mental model

- The **application** computes every widget rectangle and owns its state.
- The framework (`Ui<M>`) owns a retained widget tree, hit-testing, focus,
  hover/press state and damage tracking. It paints only dirty rectangles.
- One persistent back buffer per window; only the dirty bounding box is
  blitted. **Idle means zero paints.** No periodic repaint without a timer.
- Paint-path allocations are forbidden: borrow text/dirty rects, cache fonts,
  brushes and converted images for the window lifetime.
- Public types are platform-neutral. The Win32/GDI/D2D host lives behind
  `#[cfg(windows)]`. The framework is **Windows-only** at the host boundary.
- Messages: widgets return `Option<M>` from `input`, where `M` is the
  application's message enum. `ui.drain_messages()` yields `Vec<M>`.

## Crates and dependency

| Crate | Path | Role |
|---|---|---|
| `bamboo-native-ui` | `crates/native-ui` | Framework: `Ui`, widgets, `Host`, backends |
| `bamboo-native-ui-layout-core` | `crates/layout-core` | `.buixml`/`.buitheme` model, parser, writer, value types |
| `bamboo-native-ui-layout-codegen` | `crates/layout-codegen` | `.buixml` → Rust (library + CLI binary) |
| `bamboo-native-ui-layout-macros` | `crates/layout-macros` | `layout_xml!` proc macro |
| `bamboo-native-ui-showcase` | `crates/showcase` | All-controls demo binary (`showcase`) |

```toml
[dependencies]
bamboo-native-ui = { path = "native-ui/crates/native-ui" }
# optional:
bamboo-native-ui-layout-macros = { path = "native-ui/crates/layout-macros" }
```

The `windows` crate is required by the binary for the `run()` result type and
for user32/Win32 calls; the framework re-exports the platform surface.

## Feature flags (`bamboo-native-ui`)

| Feature | Default | Effect |
|---|---|---|
| `gdi` | yes | GDI/Windows backend; the public API stays platform-neutral |
| `layout-xml` | no | `.buixml` parser/model/applier; enables `layout-core/parse` (quick-xml) |
| `theme-xml` | no | `.buitheme` → `Palette`; enables `layout-core/parse` |
| `layout-compiled` | no | Runtime for codegen output; no XML parser linked |
| `debug-form` | no | Debug snapshots + hot reload; implies `layout-xml` |

`layout-core` alone has a `parse` feature (quick-xml), off by default.

## Minimal application

```rust
use bamboo_native_ui::{
    Host, PaintCtx, Rect, Size, ThemeSettings, Ui, WidgetId, WindowBuilder, WindowHandle,
};
use bamboo_native_ui::widgets::{Button, Label};

enum Msg { Play }

struct App { ui: Ui<Msg> }

impl App {
    fn new() -> Self {
        let mut ui = Ui::new();
        ui.add(Label::new("title", "Now playing").wrap(true));
        ui.add(Button::new("play", "Play", Msg::Play).primary());
        Self { ui }
    }
}

impl Host for App {
    fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        for rect in ctx.dirty {
            ctx.canvas.fill_rect(*rect, ctx.painter.palette().background);
        }
        self.ui.paint(ctx);
    }

    fn resize(&mut self, size: Size, _dpi: f32, window: &mut WindowHandle<'_>) {
        self.ui.set_rect(WidgetId::new("title"), Rect::from_xywh(16.0, 16.0, size.width - 32.0, 24.0));
        self.ui.set_rect(WidgetId::new("play"), Rect::from_xywh(16.0, 56.0, 120.0, 36.0));
        window.invalidate_all();
    }

    fn event(&mut self, _event: &bamboo_native_ui::Event, window: &mut WindowHandle<'_>) -> bool {
        for _msg in self.ui.drain_messages() { /* handle */ }
        self.ui.flush_damage(window);
        false
    }
}

fn main() -> windows::core::Result<()> {
    WindowBuilder::new(App::new())
        .title("Demo")
        .size(800.0, 600.0)
        .theme(ThemeSettings::default(), true)
        .run()
}
```

`run()` returns `windows::core::Result<i32>`. `WindowBuilder` methods:
`app_id`, `owner`, `menu_bar`, `renderer`, `window_hotkey(s)`, `title`,
`size`, `min_size`, `theme(settings, dark)`, `run`.

## Core types (signatures)

```rust
// geometry (DIPs; Rect::to_px(scale) snaps outward)
pub struct Point { pub x: f32, pub y: f32 }
pub struct Size  { pub width: f32, pub height: f32 }
pub struct Insets { pub left: f32, pub top: f32, pub right: f32, pub bottom: f32 }
pub struct Corners {
    pub top_left: f32, pub top_right: f32, pub bottom_right: f32, pub bottom_left: f32,
}
// Rect stores edges, not width/height.
pub struct Rect { pub left: f32, pub top: f32, pub right: f32, pub bottom: f32 }
impl Rect {
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self;
    pub fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self;
    pub fn from_size(size: Size) -> Self;
    pub fn width(self) -> f32; pub fn height(self) -> f32; pub fn size(self) -> Size;
    pub fn contains(self, x: f32, y: f32) -> bool;
    pub fn intersects(self, other: Rect) -> bool;
    pub fn union(self, other: Rect) -> Rect;
    pub fn inset(self, dx: f32, dy: f32) -> Rect;
    pub fn inflate(self, dx: f32, dy: f32) -> Rect;
    pub fn translate(self, dx: f32, dy: f32) -> Rect;
}

pub struct Color { pub r: u8, pub g: u8, pub b: u8 }   // opaque RGB
impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self;
    pub fn rgb(rgb: [u8; 3]) -> Self;
    pub fn from_hex(hex: u32) -> Self;                 // 0xRRGGBB
    pub fn to_hex(self) -> u32;
}
pub struct Rgba { pub color: Color, pub alpha: u8 }    // alpha 0..=255

pub struct ThemeSettings {
    pub accent: [u8; 3],
    pub font_family: Option<String>,
    pub font_size: f32,
    pub list_density: ListDensity, // Compact | Comfortable | Spacious
}
impl Palette { pub fn new(settings: &ThemeSettings, dark: bool) -> Palette; }
```

`Palette` has ~32 public fields: `dark, background, sidebar, player, cover,
nav_active, accent, text, muted, divider, row_selected, scroll_track,
scroll_thumb, accent_text, search_bg, button_bg, button_hover, button_pressed,
button_active, button_active_hover, explicit_border, nav_hover, row_hover,
nav_pressed, row_pressed, scroll_thumb_hover, card, panel, button_disabled,
text_disabled, play_button, play_hover`.

`WidgetId` is `pub struct WidgetId(pub &'static str)`; `WidgetId::new("id")`.

## `Ui<M>` — most-used methods

```rust
// tree
Ui::new(); ui.add(widget); ui.add_with(widget, WidgetProps::new());
ui.insert(at, widget); ui.remove(id, &mut sink); ui.clear();
ui.set_rect(id, rect); ui.rect(id); ui.ids(); ui.with_widget::<W, R>(id, f);
// bindings / focus
ui.bind(id, Trigger::Click, |event, ctx| Some(Msg::X));
ui.on_click(id, f); ui.on_activate(id, f); ui.set_focus(id, &mut sink);
// input (called by the host)
ui.pointer_move(p); ui.pointer_down(p, &mut sink); ui.pointer_up(p, &mut sink);
ui.pointer_down_button(p, MouseButton::Left, &mut sink);
ui.wheel(p, delta, &mut sink); ui.key(KeyEvent, &mut sink); ui.key_up(..);
// messages / damage
ui.drain_messages() -> Vec<M>; ui.flush_damage(&mut sink);
// setters (silent; record damage)
ui.set_enabled(id, bool); ui.set_visible(id, bool); ui.set_text(id, &str);
ui.set_label(id, &str); ui.set_value(id, f32); ui.set_toggle(id, bool);
ui.set_selected(id, usize); ui.set_rows(id, rows); ui.set_progress(id, f32);
// scrolling / images / menus / a11y
ui.set_scroll_view(view); ui.set_scroll_offset(id, offset);
ui.set_image(UiImage::gradient(id, w, h, top, bottom)); ui.image(id);
ui.set_context_menu(id, menu); ui.open_menu(id, &mut sink);
ui.a11y_nodes(); ui.apply_a11y(...);
// layout skins
ui.apply_layout(&document, &palette, size, &mut sink); // feature layout-xml
ui.apply_compiled(&LAYOUT, &palette, size, &mut sink); // feature layout-compiled
ui.clear_layout(&mut sink); ui.installed_layout();
```

`UiHost` is the sink trait (`Invalidator` + native edit, tooltip, context menu,
clipboard, soft keyboard, drag). `WindowHandle` implements it.

## `Widget<M>` contract

Required:

```rust
fn id(&self) -> WidgetId;
fn rect(&self) -> Rect;
fn set_rect(&mut self, rect: Rect);
fn as_any(&self) -> &dyn Any;
fn as_any_mut(&mut self) -> &mut dyn Any;
fn paint(&self, paint: &WidgetPaint<'_>, state: &WidgetState);
fn input(&mut self, input: WidgetInput) -> Option<M>;
```

Provided (override as needed): `preferred_height`, `enabled`, `focusable`,
`visible`, `interactive`, `hit_test`, `cursor`, `adjustable`, `paint_bounds`,
`overlay`, `focus_inset`, `focus_ring_rect/radius`, native edit methods
(`wants_edit`, `edit_request`, `commit_text`), inline edit methods, observers,
`a11y`, `read_only`, state setters (`set_enabled`, `set_value`, `set_toggle`,
`set_selected`, `set_rows`, `set_label`, `set_progress`, `set_text`,
`set_number`, `set_visible`), and the skin protocol:

```rust
fn skin_attrs(&self) -> &'static [SkinAttrDecl];
fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool;
fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>);
fn skin_children(&self) -> &'static [&'static str];
fn validate_skin_children(&self, children: &[SkinElement]) -> bool;
fn apply_skin_children(&mut self, children: &[SkinElement]);
```

`WidgetState`: `hovered, pressed, focused, adjusting, enabled, focus_visible,
editing, caret_on`.
`WidgetPaint`: `painter, canvas, palette, font, images, viewport, dirty` plus
`text`, `measure_text`, `style`, `image`, `enabled_color` helpers.
`WidgetInput`: `HoverEnter/Leave/At, PressAt, DragTo, ReleaseAt, ClickAt,
SelectWith, DoubleClickAt, RightClickAt, DragStart/End, Wheel, Step, Activate,
Char, Key, KeyUp`.

## `Host` and `PaintCtx`

Required: `paint(&mut self, ctx: &mut PaintCtx<'_>)`.
Provided hooks: `overlay_paint`, `event`, `timer`, `resize`, `dpi_changed`,
`started`, `close_requested`, `loaded`, `visibility_changed`, `activated`,
`visual_state_changed`, `moved`, `files_dropped`, drag callbacks, `destroying`,
`edit_changed/committed/canceled`, `ime_compose/commit`, `a11y_nodes`,
`a11y_action`, `menu_command`, `menu_event`, `menu_bar_command`,
`dialog_finished`, `tray_event`, `hotkey`, `hotkey_captured`, `destroy`.

`PaintCtx` fields: `painter`, `canvas`, `dirty: &[Rect]`, `size`, `dpi`,
`editing: Option<WidgetId>`, `caret_on`, `overlay`, `overlay_first`,
`caret: Option<Rect>` (output). Helpers `scale()`, `hits(rect)`, `full()`.

## Events and triggers

`Event`: `PointerMove, PointerDown, PointerUp, PointerDoubleClick,
PointerLeave, PointerKind, LongPress, Wheel, Key(KeyEvent), KeyUp(KeyEvent),
Alt(bool), Char(char), Focus(bool), Resize(Size), DpiChanged(f32)`.
`KeyCode`: `Backspace, Tab, Enter, Escape, Space, Left, Right, Up, Down, Home,
End, PageUp, PageDown, Insert, Delete, Letter(u8), Digit(u8), Function(u8),
Other(u16)`.

Shared observer builders installed by some widgets (additive; built-in action
still runs): `.on_click(Fn(Point) -> Option<M>)`,
`.on_right_click(Fn(Point) -> Option<M>)`,
`.on_double_click(Fn(Point) -> Option<M>)`,
`.on_key_down(Fn(KeyEvent) -> Option<M>)`,
`.on_key_up(Fn(KeyEvent) -> Option<M>)`,
`.on_focus_gained(Fn(FocusEvent) -> Option<M>)`,
`.on_focus_lost(Fn(FocusEvent) -> Option<M>)`.

Widgets with shared observers: Button, Toggle, Segmented, Tabs, Slider,
Checkbox, RadioButton, RadioGroup, ComboBox, MultiSelectList, ListView,
ColorPicker, DatePicker, TimePicker, MenuBar, MenuButton, SplitButton, Toolbar.
`TextEdit` defines its own additive `on_change`, `on_char`, `on_click`,
`on_key_down`, `on_key_up`.

`Trigger` (for `Ui::bind`): `Click, DoubleClick, RightClick, HoverEnter,
HoverLeave, Wheel, Activate, DragStart, DragEnd, ValueChange`.
`TriggerEvent { trigger, pos: Option<Point>, wheel_delta: f32 }`.

## Widget catalog

All widgets are re-exported from `bamboo_native_ui::widgets` (not the crate
root). Constructor messages use the application type `M`.

| Category | Types |
|---|---|
| Content | `Label`, `Icon`, `Badge`, `Divider`, `Avatar`, `Kbd`, `CopyableText`, `MarqueeText`, `RichText` |
| Input | `Checkbox`, `RadioButton`, `RadioGroup`, `SearchBox`, `TextEdit` (`PasswordBox`, `TextArea`), `NumberInput` (`NumberStepper`), `ComboBox`, `MultiSelectList` |
| Controls | `Button`, `Toggle` (`Switch`), `Segmented`, `Tabs`, `Slider`, `ProgressBar`, `Pager`, `ColorSwatch` |
| Pickers | `ColorPicker`, `DatePicker`, `TimePicker` |
| Data | `Sparkline`, `Gauge`, `RatingStars`, `ListView`, `Table`, `TreeView`, `PropertyGrid` |
| Navigation | `Accordion` (`Collapsible`), `Breadcrumb`, `NavRail`, `Splitter` |
| Feedback | `Spinner`, `ProgressRing`, `Banner`, `TooltipCard`, `Scrim`, `Toast` (`Snackbar`) |
| Display | `Card`, `GroupBox`, `StatCard`, `EmptyState`, `SectionTitle` |
| Player | `WaveformSeekBar`, `PlaybackTransport`, `CoverWall`, `PlaylistView` |
| Chrome | `MenuBar`, `MenuButton`, `SplitButton`, `Toolbar`, `StatusBar`, `Ribbon` |
| Overlay/custom | `Popover`, `CustomWidget` |

Key constructors:

```rust
Label::new("id", "text")
Icon::new("id", '\u{E768}')
Badge::new("id", "text").tone(BadgeTone::Accent)
Avatar::new("id", "Name")
Kbd::new("id", "Ctrl+K")
Button::new("id", "Label", Msg::Action).primary()
Toggle::new("id", "Label", true, Msg::Toggled)
Checkbox::new("id", "Label", true, Msg::Checked)
Slider::new("id", 0.5, 0.0..=1.0, Msg::Volume)
ProgressBar::new("id", 0.42)
Segmented::new("id", &["A", "B"], 0, Msg::Segment)
Tabs::new("id", &["One", "Two"], 0, Msg::Tab)
ComboBox::new("id", &["A", "B"], 0, Msg::Choice)
TextEdit::new("id", "text", Msg::Commit)
NumberInput::new("id", 10, 0..=100, Msg::Number)
ListView::new("id").rows(rows)             // Row::new("title")
Table::new("id").columns(cols).rows(rows)  // TableColumn::new("Title", 120.0)
TreeView::new("id").items(items)           // TreeItem::new("Label")
PropertyGrid::new("id").sections(sections) // PropertySection::new("Section")
Card::new("id", "Title", "Body")
Popover::new("id").anchor(rect)
CustomWidget::new("id", |paint, rect, state| { /* draw */ })
```

The showcase (`crates/showcase/src/main.rs`) exercises all of them.

## `.buixml` layout documents

A skin addresses widgets the Rust side created: it moves, styles and seeds
their ids. It can also **create** widgets: a `type="..."` entry with a
`WidgetRegistry` installed (the standard set covers 12 types; third-party types
register from Rust). Created widgets are document-owned — a reload removes or
replaces the ones the new document no longer declares — and interactive
creators need a `command` mapped to `M`. Arbitrary built-ins and custom
widgets still need a Rust factory.

```xml
<?xml version="1.0" encoding="utf-8"?>
<layouts version="1">
  <const name="pad" value="16"/>
  <layout id="base" min-width="0" min-height="0">
    <widget id="title" x="pad" y="16" w="cw-32" h="24"/>
    <widget id="play" type="button" command="101"
            x="cx-22" y="96" w="44" h="44" background="accent" radius="pill"/>
    <widget id="list" x="pad" y="160" w="cw-2*pad" h="180" row-height="36">
      <item title="First" subtitle="Sub" trailing="A"/>
    </widget>
  </layout>
</layouts>
```

Geometry: horizontal — exactly one of `x+w`, `right+w`, `x+right` (stretched);
vertical — exactly one of `y+h`, `bottom+h`, `y+bottom`.
Expression grammar: `expr := term (('+'|'-') term)*`,
`term := factor (('*'|'/') factor)*`,
`factor := number | number '%' | variable | '(' expr ')' | '-' factor`.
Space variables: `left/top` = 0, `right/bottom` = edge, `cw/ch` = size,
`cx/cy` = half. `N%` is N/100 of the axis extent. Constants declared with
`<const>` in order; only earlier constants may be referenced. Depth limit 32.

Widget attributes: `type`, `command` (u32, only with `type`), `parent`,
`visible`, `enabled`, `interactive`, `focusable`, `focus-order`, `read-only`,
`tooltip`, `name`, `description`, `automation-id`, `role`, `expose`, `cursor`,
`radius` (`0..N`, `small=4`, `medium=6`, `large=10`, `pill`), `corners`
(`"tl tr br bl"`), `background` (palette token, `#rgb`/`#rrggbb`, `none`),
`font-size`, `font-family`, and content attributes (`text`, `label`, `value`,
`progress`, `number`, `checked`, `selected`).

Variants: one `<layout>` per breakpoint with `min-width`/`min-height`; selection
is pure (greatest fitting minimums, else the `(0,0)` base).

Child lists (per widget): `<item>`, `<option>`, `<step>`, `<segment>`,
`<node>`, `<column>/<row>/<cell>`, `<section>`, `<track>`, `<cover>`,
`<point>`, `<run>`, `<page>`. Third-party widgets use `x-*` names declared via
the skin protocol.

Runtime API (`feature = "layout-xml"`):

```rust
use bamboo_native_ui::layout_xml::LayoutDocument;
let document = LayoutDocument::parse(text)?;
ui.apply_layout(&document, &palette, size, &mut sink)?; // atomic on error
ui.clear_layout(&mut sink);
```

No event handlers are bound by XML. `command` maps a created interactive widget
to an application message through `CommandMap<M>`. Content is written once per
document generation; it is not live-bound and is not restored by `clear_layout`.

## Compiled layouts

Codegen CLI:

```text
cargo run -p bamboo-native-ui-layout-codegen -- layouts/main.buixml \
    -o src/main_layout.rs --module main_layout [--context AppLayout]
```

Generated module: `pub static LAYOUT: CompiledLayout` and
`pub fn install<M, H: UiHost>(ui, sink, palette, client) -> Result<LayoutReport, LayoutFailure>`.
Use it with `include!` or a build script, then:

```rust
ui.apply_compiled(&main_layout::LAYOUT, &palette, size, &mut sink)?;
// or main_layout::install(&mut ui, &mut sink, &palette, size)?;
```

Proc macro:

```rust
bamboo_native_ui_layout_macros::layout_xml!("layouts/main.buixml");
bamboo_native_ui_layout_macros::layout_xml!("layouts/main.buixml", module = app_layout);
bamboo_native_ui_layout_macros::layout_xml!("layouts/main.buixml", context = AppLayout);
```

The macro expands the same module in place and needs `layout-compiled` in the
consumer. A stable proc macro cannot register a file dependency, so editing the
`.buixml` alone does not trigger a rebuild; use the CLI/build script for that.

Compiled mode additionally accepts inline Rust: `x="rust:EXPR"` (geometry,
visible/enabled/radius/content) and `<rust target="id" code="..."/>` for rows.

## `.buitheme` theme documents (`feature = "theme-xml"`)

```xml
<?xml version="1.0" encoding="utf-8"?>
<theme version="1" name="Night" base="dark" accent="#7aa2f7">
  <color name="background" value="#16181d"/>
  <color name="text" value="#e8ebee"/>
</theme>
```

`base` is `light` or `dark`; `<color name value>` overrides one palette token
after derivation. API:

```rust
use bamboo_native_ui::theme_xml::{ThemeDocument, ThemeDocumentExt};
let document = ThemeDocument::parse(text)?;
let palette = document.palette(&settings);
window.set_theme(settings.clone(), palette.dark);
window.set_palette(palette);
```

Palette tokens are the 31 kebab-case names used by `background=`:
`background, sidebar, player, cover, nav-active, accent, text, muted, divider,
row-selected, scroll-track, scroll-thumb, accent-text, search-bg, button-bg,
button-hover, button-pressed, button-active, button-active-hover,
explicit-border, nav-hover, row-hover, nav-pressed, row-pressed,
scroll-thumb-hover, card, panel, button-disabled, text-disabled, play-button,
play-hover`.

## Registry and standard `type=` names

`WidgetRegistry::standard()` creates:
`label, button, toggle, slider, progress, icon, badge, card, divider, kbd,
avatar, spinner`.

```rust
let mut registry = WidgetRegistry::<Msg>::standard();
registry.set_commands(|id| match id { 101 => Some(Msg::Save), _ => None });
registry.register("x-meter", |spec, ctx| { /* return Box<dyn Widget<Msg>> */ });
ui.set_widget_registry(registry);
```

Interactive creators (`button`, `toggle`, `slider`) require `command`.

## Common pitfalls

1. Widgets live in `bamboo_native_ui::widgets::*`, not the crate root (only
   `CustomWidget` is re-exported at the root).
2. Widget ids are `&'static str`; `WidgetId::new("id")`.
3. Set rectangles in `Host::resize`, then `window.invalidate_all()`.
4. Paint only what `ctx.dirty` intersects; call `ui.paint(ctx)` last.
5. `Ui` setters are silent and record damage; call `ui.flush_damage(window)`.
6. XML never binds event handlers; wire callbacks in Rust.
7. The dynamic XML path rejects `rust:` values; compiled mode accepts them.
8. `clear_layout` does not restore rectangles, scroll viewports or content.
9. Scrolling is `ScrollView` managed by `Ui`, not a widget.
10. The host/platform layer is Windows-only; neutral types compile elsewhere.

## Where to look next

- `docs/en/index.md` — documentation home.
- `docs/en/quickstart/` — thirteen tutorials (six core, seven practical).
- `docs/en/widgets/` — one page per widget (usage, properties, events, XML).
- `docs/en/api/` — one page per API area.
- `docs/en/guides/xml-layout-authoring.md` — the practical `.buixml` guide.
- `docs/en/reference/xml-elements.md` — every XML attribute/child.
- `docs/archive/` — design records and milestone contracts.
