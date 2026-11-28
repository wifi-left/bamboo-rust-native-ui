# Widget registry and creation

> Let a layout document create widgets by type name, and map its `command` ids to the application's messages.

## Overview

Before M7.7 a `.buixml` document could only address widgets the application had
already built. The registry changes that: `<widget type="button" …>` creates a
`Button` when its id does not exist, using the factory registered for that type
name. The module `bamboo_native_ui::creation` is always compiled; *creation*
needs `feature = "layout-xml"` or `feature = "layout-compiled"` because it runs
inside an apply.

A created widget is an ordinary widget. Geometry, shared content, surface/font
tokens and widget-local skin data go through the same applier as for an
addressed id; the factory only supplies the constructor. That is why a factory
may build with empty text — the file's `label`/`text`/`selected` values arrive
right after insertion.

The document owns what it creates: a reload that no longer declares an id
removes it, and `Ui::clear_layout` removes every created widget. Created
widgets are inserted in the **page layer** — before the first backdrop widget
(a `Scrim`), or at the end when there is none — so a modal overlay keeps
painting above them.

## Key types

### `WidgetRegistry`

```rust
pub struct WidgetRegistry<M> { /* private */ }

impl<M: 'static> WidgetRegistry<M> {
    pub fn new() -> Self;
    pub fn standard() -> Self where M: Clone;
    pub fn register(
        &mut self,
        type_name: &'static str,
        factory: impl Fn(&WidgetSpec<'_>, &FactoryCtx<'_, M>)
                -> Result<Box<dyn Widget<M>>, FactoryError>
            + 'static,
    );
    pub fn set_commands(&mut self, commands: impl CommandMap<M> + 'static);
    pub fn has_type(&self, type_name: &str) -> bool;
}

impl<M> WidgetRegistry<M> {
    pub fn type_names(&self) -> impl Iterator<Item = &'static str> + '_;
    pub fn has_commands(&self) -> bool;
}
```

`new()` is empty: a document's `type` entries are then `xml.type.unknown`.
`register` replaces a same-named factory. `standard()` installs the twelve
built-ins in `STANDARD_TYPES`; `toggle` and `slider` report their command on
every change, so `standard()` requires `M: Clone`.

```rust
pub const STANDARD_TYPES: &[&str] = &[
    "label", "button", "toggle", "slider", "progress", "icon", "badge",
    "card", "divider", "kbd", "avatar", "spinner",
];
```

### `Factory`, `WidgetSpec`, `FactoryCtx`

```rust
pub type Factory<M> =
    dyn Fn(&WidgetSpec<'_>, &FactoryCtx<'_, M>) -> Result<Box<dyn Widget<M>>, FactoryError>;

pub struct WidgetSpec<'a> {
    pub id: WidgetId,
    pub rect: Rect,
    pub content: &'a [ContentAttr],
    pub attrs: &'a [(String, String)],
    pub children: &'a [SkinElement],
    pub command: Option<u32>,
    pub line: usize,
    pub path: &'a str,
}

impl WidgetSpec<'_> {
    pub fn id(&self) -> WidgetId;
    pub fn label(&self) -> Option<&str>;       // first label/text value
    pub fn attr(&self, name: &str) -> Option<&str>;
}

pub struct FactoryCtx<'a, M> {
    pub palette: &'a Palette,
    pub client: Size,
    pub commands: Option<&'a dyn CommandMap<M>>,
}

impl<M> FactoryCtx<'_, M> {
    pub fn message(&self, command: Option<u32>) -> Option<M>;
}
```

`rect` is already resolved for the active breakpoint. `attrs` and `children`
are the file's raw local data (values are never expressions); the applier writes
them after insertion, so a factory usually only needs the command.

### `CommandMap`, `FactoryError`

```rust
pub trait CommandMap<M> {
    fn message(&self, command: u32) -> Option<M>;
}

impl<M, F> CommandMap<M> for F where F: Fn(u32) -> Option<M> { /* … */ }

pub enum FactoryError {
    Missing(&'static str),   // reported as xml.command.missing
    Rejected(String),        // reported as xml.type.create
}

impl FactoryError {
    pub fn rejected(message: impl Into<String>) -> Self;
}
```

A plain closure is a `CommandMap`, so `set_commands(|id| …)` is the usual form.
One command id maps to one message value.

### Installing on the container

```rust
impl<M> Ui<M> {
    pub fn set_widget_registry(&mut self, registry: WidgetRegistry<M>);
    pub fn widget_registry(&self) -> Option<&WidgetRegistry<M>>;
    pub fn widget_registry_mut(&mut self) -> Option<&mut WidgetRegistry<M>>;
    pub fn clear_widget_registry(&mut self);
}
```

Replacing the registry affects the next apply; widgets already on screen stay
until a reload removes them.

## Document attributes

| Attribute | Meaning |
|---|---|
| `type="name"` | Create this id from the registry when it does not exist. Names match `[a-z][a-z0-9_.-]*`; `x-*`/`vendor.type` names belong to applications. |
| `command="101"` | The `u32` command id a created interactive widget resolves through the `CommandMap`. Legal only with `type`. |
| `parent="panel"` | The scroll panel the entry belongs to. On a created entry it is installed membership; on an addressed entry it asserts the application's membership (`xml.parent.missing` on mismatch). |

Absent `type` keeps the address-only behaviour. On a generation change a `type`
entry updates an id this document already created (same `type`/`command`), or
removes and re-creates it (different `type`/`command`). A `type` id that exists
and is not document-owned is `xml.id.duplicate`; an unregistered name is
`xml.type.unknown`.

```xml
<layouts version="1">
  <layout id="base">
    <widget id="hello" type="label" x="10" y="10" w="100" h="20" label="你好"/>
    <widget id="go" type="button" command="101" x="10" y="40" w="80" h="30" label="出发"/>
    <widget id="row" type="label" parent="panel" x="0" y="250" w="80" h="20" label="行"/>
  </layout>
</layouts>
```

## Example

```rust
use bamboo_native_ui::creation::WidgetRegistry;
use bamboo_native_ui::Ui;

#[derive(Clone)]
enum Msg {
    Save,
    Volume(f32),
}

fn registry() -> WidgetRegistry<Msg> {
    let mut registry = WidgetRegistry::<Msg>::standard();
    registry.set_commands(|command| match command {
        101 => Some(Msg::Save),
        102 => Some(Msg::Volume(0.0)),
        _ => None,
    });
    registry
}

fn install(ui: &mut Ui<Msg>) {
    ui.set_widget_registry(registry());
}
```

`<widget id="go" type="button" command="101" …/>` then creates a `Button` whose
click reports `Msg::Save`.

To add a private type, register it from your own code (`Meter` here is any
`Widget<Msg>` implementation):

```rust
let mut registry = WidgetRegistry::<Msg>::new();
registry.register("x-meter", |spec, _ctx| {
    Ok(Box::new(Meter::new(spec.id.name(), spec.rect)))
});
```

The factory must return a widget whose `id()` is exactly `spec.id`; the applier
rejects anything else as `xml.type.create`. See
[XML layout authoring](../guides/xml-layout-authoring.md#custom-widgets).

## Notes and limits

- Created widgets are content: their skin data and content are written on a
  generation change, not on a resize.
- Interactive standard types need a command table and a `command` value;
  without either the factory returns `FactoryError::Missing("command")`
  (`xml.command.missing`).
- `M` need not be `Clone` for click-only controls. Only `toggle` and `slider`
  require it (their callback produces the same message on every change; read
  the state back through the typed accessors).
- A document that declares `type` for an id in one variant must declare it in
  every variant, with the same type/command (`xml.type.conflict`).
- Factory errors are atomic: one failing entry rejects the whole apply and
  leaves the previous layout in place.
- The registry is visible to the debug form as `widget_types` and
  `has_commands`; see [diagnostics-and-debug.md](diagnostics-and-debug.md).
- Compiled documents carry the same `type`/`command`/`parent` data
  (`CompiledEntry::type_name`/`command`/`parent`); nothing else changes.

## See also

- [layout-documents.md](layout-documents.md) — `LayoutEntry::type_name`, `command`, `parent`
- [layout-codegen.md](layout-codegen.md) — creation in compiled documents
- [props-and-narration.md](props-and-narration.md) — `WidgetProps` a factory may honor
- [XML layout authoring](../guides/xml-layout-authoring.md#custom-widgets)
