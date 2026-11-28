// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Document-created widgets (M7.7, D148).
//!
//! Until M7.7 a layout document could only *address* widgets the application
//! already built. A [`WidgetRegistry<M>`] changes that: an entry with
//! `type="button"` creates the widget when its id does not exist yet, using the
//! factory registered for that type name. The library ships a small standard
//! set ([`WidgetRegistry::standard`]); a third-party crate registers its own
//! names from its own code — no library change, no `TypeId` table.
//!
//! A created widget is an ordinary widget: the entry's geometry, shared
//! content, surface/font and local skin data apply through the same applier as
//! for an addressed one, and the factory only supplies the constructor. That is
//! why a factory may build with empty text: the file's `label`/`text`/
//! `selected` values arrive after creation.
//!
//! What a factory cannot invent is the application's message value:
//!
//! ```ignore
//! let mut registry = WidgetRegistry::<Msg>::standard();
//! registry.set_commands(|id| match id {
//!     101 => Some(Msg::Save),
//!     _ => None,
//! });
//! ui.set_widget_registry(registry);
//! ```
//!
//! `<widget id="save" type="button" command="101" …/>` then creates a `Button`
//! whose click reports `Msg::Save`. `M` need not be `Clone` for the standard
//! set except for the value-reporting controls (`toggle`, `slider`), whose
//! callback produces the same message on every change; the state itself is read
//! back through the typed accessors.
//!
//! Created widgets are owned by the document: a reload that no longer declares
//! an id removes it, and [`crate::Ui::clear_layout`] removes every created
//! widget. They are inserted in the **page layer** — before the first backdrop
//! widget (a `Scrim`), or at the end when there is none — so a modal overlay
//! keeps painting above them.

use crate::geometry::{Rect, Size};
/// The shared content vocabulary a spec exposes (`text`/`label`/`value`/…).
pub use bamboo_native_ui_layout_core::model::ContentAttr;
use crate::skin::SkinElement;
use crate::theme::Palette;
use crate::widget::{Widget, WidgetId};

/// Everything a factory may read about the entry that is creating a widget.
///
/// Geometry is the rectangle resolved for the current breakpoint; content and
/// local data are the file's values (the applier writes them right after the
/// widget is inserted, so a factory usually only needs the command).
pub struct WidgetSpec<'a> {
    pub id: WidgetId,
    pub rect: Rect,
    /// Shared content initial values, in source order.
    pub content: &'a [ContentAttr],
    /// Widget-local attributes (raw strings, source order).
    pub attrs: &'a [(String, String)],
    /// Widget-local child elements.
    pub children: &'a [SkinElement],
    /// The entry's `command` value, when the file declares one.
    pub command: Option<u32>,
    /// 1-based source line and path, for factory diagnostics.
    pub line: usize,
    pub path: &'a str,
}

impl WidgetSpec<'_> {
    pub fn id(&self) -> WidgetId {
        self.id
    }

    /// The first `label`/`text` content value, the usual constructor text.
    pub fn label(&self) -> Option<&str> {
        self.content.iter().find_map(|item| match item {
            ContentAttr::Label(text) | ContentAttr::Text(text) => Some(text.as_str()),
            _ => None,
        })
    }

    /// One local attribute value.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Why a factory could not build the widget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FactoryError {
    /// The factory needs something the document did not supply (`"command"`),
    /// reported as `xml.command.missing`.
    Missing(&'static str),
    /// The factory refused the entry; the text is developer-facing and becomes
    /// `xml.type.create`.
    Rejected(String),
}

impl FactoryError {
    /// A refusal with an explanation.
    pub fn rejected(message: impl Into<String>) -> Self {
        Self::Rejected(message.into())
    }
}

/// Resolves a document `command` attribute to the application's message.
///
/// One id maps to one value: a value-reporting control (`toggle`, `slider`)
/// reports its command on every change and the application reads the new state
/// back through the typed accessors, which keeps `M` free of a `Clone` bound
/// for click-only controls. A blanket implementation covers plain closures.
pub trait CommandMap<M> {
    fn message(&self, command: u32) -> Option<M>;
}

impl<M, F> CommandMap<M> for F
where
    F: Fn(u32) -> Option<M>,
{
    fn message(&self, command: u32) -> Option<M> {
        self(command)
    }
}

/// The per-apply context a factory sees besides the entry.
pub struct FactoryCtx<'a, M> {
    pub palette: &'a Palette,
    /// Client size the entry's rectangle was resolved against.
    pub client: Size,
    /// The application's command table, when the registry carries one.
    pub commands: Option<&'a dyn CommandMap<M>>,
}

impl<M> FactoryCtx<'_, M> {
    /// Resolve an entry command to its message, if the registry can.
    pub fn message(&self, command: Option<u32>) -> Option<M> {
        self.commands?.message(command?)
    }
}

/// One type factory: build a widget from the entry it creates.
pub type Factory<M> =
    dyn Fn(&WidgetSpec<'_>, &FactoryCtx<'_, M>) -> Result<Box<dyn Widget<M>>, FactoryError>;

/// Type name → factory, plus the optional command table, installed on a
/// [`crate::Ui`] with `Ui::set_widget_registry`.
pub struct WidgetRegistry<M> {
    types: Vec<(&'static str, Box<Factory<M>>)>,
    commands: Option<Box<dyn CommandMap<M>>>,
}

impl<M> WidgetRegistry<M> {
    /// Registered type names, in registration order (the debug form lists
    /// them).
    pub fn type_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.types.iter().map(|(name, _)| *name)
    }

    /// Whether a command table is installed (the debug form reports it).
    pub fn has_commands(&self) -> bool {
        self.commands.is_some()
    }
}

impl<M: 'static> Default for WidgetRegistry<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: 'static> WidgetRegistry<M> {
    /// An empty registry: a document's `type` entries are `xml.type.unknown`.
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            commands: None,
        }
    }

    /// The library's standard types (see [`STANDARD_TYPES`]).
    ///
    /// `toggle` and `slider` report their command on every value change, so
    /// they require `M: Clone`; register your own factories for other message
    /// shapes.
    pub fn standard() -> Self
    where
        M: Clone,
    {
        let mut registry = Self::new();
        register_standard(&mut registry);
        registry
    }

    /// Register (or replace) one type name. Names are the document's
    /// `type="…"` vocabulary; `x-*`/`vendor.type` names are the application's.
    pub fn register(
        &mut self,
        type_name: &'static str,
        factory: impl Fn(&WidgetSpec<'_>, &FactoryCtx<'_, M>) -> Result<Box<dyn Widget<M>>, FactoryError>
            + 'static,
    ) {
        if let Some(existing) = self
            .types
            .iter_mut()
            .find(|(name, _)| *name == type_name)
        {
            existing.1 = Box::new(factory);
        } else {
            self.types.push((type_name, Box::new(factory)));
        }
    }

    /// Set the command table a created interactive widget resolves against.
    pub fn set_commands(&mut self, commands: impl CommandMap<M> + 'static) {
        self.commands = Some(Box::new(commands));
    }

    /// Whether a type name is registered.
    pub fn has_type(&self, type_name: &str) -> bool {
        self.types.iter().any(|(name, _)| *name == type_name)
    }

    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn factory(&self, type_name: &str) -> Option<&Factory<M>> {
        self.types
            .iter()
            .find(|(name, _)| *name == type_name)
            .map(|(_, factory)| factory.as_ref())
    }

    #[cfg(any(feature = "layout-xml", feature = "layout-compiled"))]
    pub(crate) fn commands(&self) -> Option<&dyn CommandMap<M>> {
        self.commands.as_deref()
    }
}

/// The type names [`WidgetRegistry::standard`] registers, with their
/// constructor rules. The file supplies the actual label/text/value through
/// the normal content pass.
pub const STANDARD_TYPES: &[&str] = &[
    "label",
    "button",
    "toggle",
    "slider",
    "progress",
    "icon",
    "badge",
    "card",
    "divider",
    "kbd",
    "avatar",
    "spinner",
];

/// The command a standard interactive factory needs, reported when the entry
/// has none or the registry has no command table.
const COMMAND: &str = "command";

fn register_standard<M: Clone + 'static>(registry: &mut WidgetRegistry<M>) {
    use crate::widgets::{
        Avatar, Badge, Button, Card, Divider, Icon, Kbd, Label, ProgressBar, Slider, Spinner,
        Toggle,
    };

    registry.register("label", |spec, _ctx| {
        Ok(Box::new(Label::new(spec.id.name(), "")))
    });
    registry.register("button", |spec, ctx| {
        let message = ctx
            .message(spec.command)
            .ok_or(FactoryError::Missing(COMMAND))?;
        Ok(Box::new(Button::new(spec.id.name(), "", message)))
    });
    registry.register("toggle", |spec, ctx| {
        let message = ctx
            .message(spec.command)
            .ok_or(FactoryError::Missing(COMMAND))?;
        Ok(Box::new(Toggle::new(spec.id.name(), "", false, move |_| {
            message.clone()
        })))
    });
    registry.register("slider", |spec, ctx| {
        let message = ctx
            .message(spec.command)
            .ok_or(FactoryError::Missing(COMMAND))?;
        Ok(Box::new(Slider::new(
            spec.id.name(),
            0.0,
            0.0..=1.0,
            move |_| message.clone(),
        )))
    });
    registry.register("progress", |spec, _ctx| {
        Ok(Box::new(ProgressBar::new(spec.id.name(), 0.0)))
    });
    registry.register("icon", |spec, _ctx| {
        Ok(Box::new(Icon::new(spec.id.name(), ' ')))
    });
    registry.register("badge", |spec, _ctx| {
        Ok(Box::new(Badge::<M>::new(spec.id.name(), "")))
    });
    registry.register("card", |spec, _ctx| {
        Ok(Box::new(Card::new(spec.id.name(), "", "")))
    });
    registry.register("divider", |spec, _ctx| {
        Ok(Box::new(Divider::new(spec.id.name())))
    });
    registry.register("kbd", |spec, _ctx| {
        Ok(Box::new(Kbd::new(spec.id.name(), "")))
    });
    registry.register("avatar", |spec, _ctx| {
        Ok(Box::new(Avatar::new(spec.id.name(), "")))
    });
    registry.register("spinner", |spec, _ctx| {
        Ok(Box::new(Spinner::new(spec.id.name())))
    });
}
