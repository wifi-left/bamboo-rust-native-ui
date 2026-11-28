# The widget trait

> The retained control contract: identity, paint, input translation, and the optional editing, accessibility and skin protocols.

## Overview

`Widget<M>` is what `Ui<M>` stores: created once per page rebuild and kept
until the next one, owning neither hover, press, focus nor invalidation state —
the container does. Its two required jobs are painting its rectangle for a
`WidgetState` and translating high-level input into an optional message of
type `M`.

Every other method is provided with a conservative default, so a widget stays
unchanged when the framework grows: a plain button ignores inline editing,
drag-and-drop, skin and overlay hooks, and a third-party widget can opt in from
outside the crate. A widget that never emits can use `M = ()`.

## Key types

### `WidgetId`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WidgetId(pub &'static str);
impl WidgetId {
    pub const fn new(name: &'static str) -> Self;
    /// Interns runtime text once per process.
    pub fn intern(name: &str) -> Self;
    pub const fn name(self) -> &'static str;
}
```

Equality is by text: `WidgetId::intern("row-1")` equals `WidgetId::new("row-1")`.
Use `new` for literals and `intern` for a bounded set of runtime names; each
distinct interned name leaks one small string.

### `WidgetState` and `WidgetPaint`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetState {
    pub hovered: bool, pub pressed: bool, pub focused: bool, pub adjusting: bool,
    pub enabled: bool, pub focus_visible: bool, pub editing: bool, pub caret_on: bool,
}

pub struct WidgetPaint<'a> {
    pub painter: &'a Painter,
    pub canvas: &'a mut dyn Canvas,
    pub palette: Palette,
    pub font: Option<FontSpec>,
    pub images: &'a ImageStore,
    pub viewport: Rect,
    pub dirty: &'a [Rect],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyleRole { Body, Small, Title, Brand, Glyph }
```

`WidgetPaint` text helpers apply the per-instance font override (never to
`Glyph` runs) and its image helpers return `false` for an unknown `ImageId`.

### `WidgetInput`

```rust
#[derive(Clone, Copy, Debug)]
pub enum WidgetInput {
    HoverEnter, HoverLeave, HoverAt(Point), PressAt(Point), DragTo(Point),
    ReleaseAt(Point), ClickAt(Point), SelectWith(Point, PointerModifiers),
    DoubleClickAt(Point), RightClickAt(Point), DragStart(Point), DragEnd(Point),
    Wheel(f32), Step(i32), Activate, Char(char), Key(KeyEvent), KeyUp(KeyEvent),
}
```

The container has already resolved hit-testing. `SelectWith` goes only to
`multi_selectable` widgets; `Step` is a slider adjustment.

### Other contract types

```rust
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub title: String, pub subtitle: String, pub trailing: String,
    pub current: bool, pub disabled: bool,
}
// Fluent builders: new, subtitle, trailing, current, disabled.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollBars { None, #[default] Vertical, Horizontal, Both }

pub struct EditRequest<'a> {
    pub id: WidgetId, pub rect: Rect, pub text: &'a str, pub cue: &'a str,
    pub password: bool, pub multiline: bool, pub wrap: bool,
    pub scrollbars: ScrollBars, pub numbers_only: bool, pub align: TextAlign,
    pub font_size: Option<f32>, pub font_family: Option<String>, pub max_length: u32,
}
```

`SessionSnapshot` (the capture value used by `take_session` /
`restore_session`) is a plain struct too, re-exported from `widget`.

## Required methods

```rust
pub trait Widget<M> {
    fn id(&self) -> WidgetId;
    fn rect(&self) -> Rect;
    fn set_rect(&mut self, rect: Rect);

    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState);
    fn input(&mut self, input: WidgetInput) -> Option<M>;
}
```

`as_any`/`as_any_mut` are the downcast seam for `Ui::with_widget` and
`Ui::edit_widget`; the container keeps invalidation and a11y rules either way.

## The paint and input contract

- **Paint** is `&self` and side-effect free. Draw only what intersects
  `ctx.dirty`; the back buffer keeps the rest. A full-surface effect (a scrim)
  must be restricted to the dirty pieces — compositing alpha over
  already-painted pixels is visibly wrong. No allocation on the paint path.
- **Input** is `&mut self` and returns the message or `None`. After every input
  the container drains `take_extra_message` (a second, additive message) and
  `take_repaint_hint` (a narrower repaint rect), so neither leaks into a later
  event.
- The container decides what a widget sees: `interactive()` gates pointer
  hit-testing, `hit_test` narrows the shape, `visible()` removes it from input,
  focus and the tree, and `paint_bounds()` is what gets invalidated (a popup
  reports its union). `paints_pointer_state()` lets a backdrop skip hover
  invalidation.

## Provided methods

### Layout, visibility, paint hints and input

| Methods | Defaults and notes |
|---|---|
| `preferred_height`, `enabled`, `focusable`, `visible` | `36.0`; `true`; `true`; `true`. Hidden widgets take no input, Tab, paint or a11y. |
| `interactive`, `hit_test`, `cursor`, `cursor_at` | `self.focusable()`; whole rect; `None` (falls back to `Hand`/`Arrow`); per-position cursor. |
| `paint_bounds`, `overlay`, `is_backdrop` | `rect()`; `false` (painted last, hit first); `false` (starts the modal layer; `Scrim` only). |
| `paints_pointer_state`, `repaint_on_move`, `repaint_on_move_at` | `true`; `false`; per-position variant. Controls hover-driven invalidation. |
| `take_repaint_hint`, `scrolls_with_wheel` | `None` (narrower repaint); `false` (the widget consumes wheel notches). |
| `focus_inset`, `focus_ring_rect`, `focus_ring_radius` | `2.0`; rect inset by it; `None` for a rectangular ring (pills return a radius). |
| `take_extra_message`, `adjustable`, `multi_selectable` | `None` (queued after the primary message); `false` (arrow-adjust mode); `false` (opts into `SelectWith`). |
| `mnemonic`, `mnemonic_key`, `set_mnemonic_underlines` | `None` (`&File` → `F`); `false` (claim one `Alt+key`); `false` (returns whether visuals changed). |
| `owns_navigation_keys`, `owns_navigation_key`, `dismiss` | `false` (an open overlay owns arrows/Enter/Space); per-key variant; `false` (Escape closes an overlay). |

### Editing — native and inline

| Methods | Defaults and notes |
|---|---|
| `wants_edit`, `wants_edit_at`, `edit_request`, `commit_text` | `false`; narrower click zone; `None`; `None`. A native `EDIT` child; the widget keeps painting its own text while closed. |
| `inline_edit`, `inline_edit_at`, `editing`, `multiline_edit` | `false`; narrower press zone; `false`; `false` (Enter inserts a newline). |
| `begin_inline_edit`, `finish_edit` | `false`; `None`. Start/reposition, then commit or revert. |
| `insert_text`, `observe_char`, `take_change_message` | `false`; `None`; `None`. Typed chars, paste and committed IME; the observer runs before insertion. |
| `set_composition`, `take_clipboard`, `caret_rect`, `edit_menu`, `edit_command` | `false`; `None`; `None`; `None`; `false`. IME composition, copied text, caret for IME/blink, built-in menu, reserved command ids. |

### Touch, drag-and-drop text and sessions

| Methods | Defaults and notes |
|---|---|
| `long_press`, `grab_selection_handle` | `false`; selects the word and shows handles. `false`; press grabbed a painted handle. |
| `take_drag_text`, `insert_dropped_text` | `None`; copies the selection out. `false`; inserts a dropped string. |
| `take_session`, `restore_session` | `None`; `false`. Capture/restore for `Ui::remove_preserving_session`. |
| `observe_click`, `observe_right_click`, `observe_double_click`, `observe_key_down`, `observe_key_up`, `observe_focus_gained`, `observe_focus_lost` | All `None`. Additive observers: each runs before the built-in behaviour, which still runs. |

### Accessibility, state and menus

| Methods | Defaults and notes |
|---|---|
| `a11y`, `a11y_select_child`, `a11y_set_value`, `a11y_set_value_at`, `a11y_expand`, `a11y_scroll` | `None`/`None`/`None`/forwards/`None` (idempotent expand)/`false` (UIA Scroll). `None` from `a11y` hides the widget. |
| `set_enabled`, `set_value`, `set_toggle`, `set_selected`, `set_rows`, `set_label`, `set_progress`, `set_visible`, `set_message`, `set_text`, `set_number` | All no-ops. Pushed by the container's setters; a widget ignores the ones it does not use. |
| `read_only`, `set_read_only` | `false`; no-op. ORed with `WidgetProps::read_only` by the container. |
| `take_native_menu`, `native_menu`, `set_native_menu`, `tooltip_rect`, `child_at`, `focus_item` | `None`; `None`; `false`; `rect()`; `None`; `None`. Popup requests, the menu a widget would open, and tooltip sub-item queries. |

## The surface and skin protocol

```rust
fn surface_style(&self) -> Option<&SurfaceStyle> { None }
fn surface_style_mut(&mut self) -> Option<&mut SurfaceStyle> { None }
fn content_caps(&self) -> ContentCaps { ContentCaps::NONE }

fn skin_attrs(&self) -> &'static [SkinAttrDecl] { &[] }
fn validate_skin_attr(&self, _name: &str, _value: &SkinValue<'_>) -> bool { true }
fn apply_skin_attr(&mut self, _name: &str, _value: &SkinValue<'_>) {}
fn skin_children(&self) -> &'static [&'static str] { &[] }
fn validate_skin_children(&self, _children: &[SkinElement]) -> bool { true }
fn apply_skin_children(&mut self, _children: &[SkinElement]) {}

pub struct SkinAttrDecl { pub name: &'static str, pub kind: SkinValueKind }
pub enum SkinValueKind { Text, Number, Bool, Color }
pub enum SkinValue<'a> { Text(&'a str), Number(f64), Bool(bool), Color(Rgba) }
```

A widget publishes the local attributes and data child elements it accepts.
`validate_*` runs for every value before anything is applied; `apply_*` must not
fire callbacks or create, move, focus or remove widgets. `x-` names are
reserved for user code, and the `skin` module is always compiled — a
third-party widget can opt in without `feature = "layout-xml"`, though only
that feature (or `layout-compiled`) installs a document that uses it.

## Example

```rust
use bamboo_native_ui::widget::{Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};
use bamboo_native_ui::{Color, Rect, TextAlign, TextStyleRole};

enum Msg {
    Ping,
}

struct Meter {
    id: WidgetId, rect: Rect, value: f32,
}

impl Widget<Msg> for Meter {
    fn id(&self) -> WidgetId { self.id }
    fn rect(&self) -> Rect { self.rect }
    fn set_rect(&mut self, rect: Rect) { self.rect = rect; }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        let fill = if state.hovered { ctx.palette.accent } else { Color::new(60, 60, 60) };
        let label = format!("{:.0}%", self.value * 100.0);
        ctx.painter.round_rect(ctx.canvas, self.rect, 6.0, fill);
        ctx.text(self.rect, &label, TextStyleRole::Small, TextAlign::Center, ctx.palette.text);
    }

    fn input(&mut self, input: WidgetInput) -> Option<Msg> {
        matches!(input, WidgetInput::ClickAt(_) | WidgetInput::Activate).then_some(Msg::Ping)
    }
}
```

## Notes and limits

- `paint` is `&self` and must not allocate: build strings in `input`/setters
  and borrow them in `paint`. Anything that changes while painting belongs in
  mutable state computed earlier, never in `paint`.
- `WidgetState::default()` has `enabled: false`; construct states through the
  container (or set every field) rather than relying on the default.
- `EditRequest` borrows text and cue from the widget; the host must not retain
  it past the call.
- Reserved editor command ids live in `widget::edit_command` (`UNDO`…`BOM`);
  application menus must not reuse that range.

## See also

- [The Ui container](./ui-container.md)
- [Events and input](./events-and-input.md)
- [Painting and Canvas](./painting-and-canvas.md)
- [Theme and palette](./theme-and-palette.md)
- [Text edit](../widgets/inputs/text-edit.md)
- [Custom widgets quickstart](../quickstart/04-custom-widgets.md)
- [XML layout authoring](../guides/xml-layout-authoring.md)
