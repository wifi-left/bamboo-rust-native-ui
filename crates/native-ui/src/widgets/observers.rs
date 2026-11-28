// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Additive input observers shared by the standard widgets.
//!
//! Each observer runs *before* the widget's built-in behavior, which still
//! runs; it only adds a notification and never replaces the default. That is
//! why it takes no default handle (see D106/D109). Suppressing or replacing a
//! built-in action stays the job of the `Ui::bind` trigger seam, which offers
//! `TriggerContext::default()` / `run_default()`.
//!
//! The container routes the events: `Ui::interact` asks the widget's
//! `Widget::observe_click`/`observe_right_click`/`observe_double_click` before
//! dispatching the input, `Ui::key` asks `observe_key_down` before navigation,
//! `Ui::key_up` forwards `observe_key_up`, and `Ui::focus_move` asks
//! `observe_focus_gained`/`observe_focus_lost`. A widget therefore only stores
//! this struct and forwards the trait methods; the `observer_methods!` and
//! `observer_builders!` macros keep that boilerplate identical everywhere.

use crate::event::KeyEvent;
use crate::geometry::Point;
use crate::ui::FocusEvent;

type PointFn<M> = Box<dyn Fn(Point) -> Option<M>>;
type KeyFn<M> = Box<dyn Fn(KeyEvent) -> Option<M>>;
type FocusFn<M> = Box<dyn Fn(FocusEvent) -> Option<M>>;

/// The additive observers a widget may carry. All optional; `None` keeps the
/// widget silent for that event.
pub struct Observers<M> {
    click: Option<PointFn<M>>,
    right_click: Option<PointFn<M>>,
    double_click: Option<PointFn<M>>,
    key_down: Option<KeyFn<M>>,
    key_up: Option<KeyFn<M>>,
    focus_gained: Option<FocusFn<M>>,
    focus_lost: Option<FocusFn<M>>,
}

impl<M> Default for Observers<M> {
    fn default() -> Self {
        Self {
            click: None,
            right_click: None,
            double_click: None,
            key_down: None,
            key_up: None,
            focus_gained: None,
            focus_lost: None,
        }
    }
}

impl<M> Observers<M> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_click(&mut self, handler: impl Fn(Point) -> Option<M> + 'static) {
        self.click = Some(Box::new(handler));
    }

    pub fn set_right_click(&mut self, handler: impl Fn(Point) -> Option<M> + 'static) {
        self.right_click = Some(Box::new(handler));
    }

    pub fn set_double_click(&mut self, handler: impl Fn(Point) -> Option<M> + 'static) {
        self.double_click = Some(Box::new(handler));
    }

    pub fn set_key_down(&mut self, handler: impl Fn(KeyEvent) -> Option<M> + 'static) {
        self.key_down = Some(Box::new(handler));
    }

    pub fn set_key_up(&mut self, handler: impl Fn(KeyEvent) -> Option<M> + 'static) {
        self.key_up = Some(Box::new(handler));
    }

    pub fn set_focus_gained(&mut self, handler: impl Fn(FocusEvent) -> Option<M> + 'static) {
        self.focus_gained = Some(Box::new(handler));
    }

    pub fn set_focus_lost(&mut self, handler: impl Fn(FocusEvent) -> Option<M> + 'static) {
        self.focus_lost = Some(Box::new(handler));
    }

    pub fn click(&self, pos: Point) -> Option<M> {
        self.click.as_ref().and_then(|handler| handler(pos))
    }

    pub fn right_click(&self, pos: Point) -> Option<M> {
        self.right_click.as_ref().and_then(|handler| handler(pos))
    }

    pub fn double_click(&self, pos: Point) -> Option<M> {
        self.double_click.as_ref().and_then(|handler| handler(pos))
    }

    pub fn key_down(&self, key: KeyEvent) -> Option<M> {
        self.key_down.as_ref().and_then(|handler| handler(key))
    }

    pub fn key_up(&self, key: KeyEvent) -> Option<M> {
        self.key_up.as_ref().and_then(|handler| handler(key))
    }

    pub fn focus_gained(&self, event: FocusEvent) -> Option<M> {
        self.focus_gained.as_ref().and_then(|handler| handler(event))
    }

    pub fn focus_lost(&self, event: FocusEvent) -> Option<M> {
        self.focus_lost.as_ref().and_then(|handler| handler(event))
    }
}

/// Forward the `Widget::observe_*` methods to a widget's `observers` field.
/// Invoke inside the `impl Widget<M> for T<M>` block; fully qualified paths
/// keep the call site free of extra imports.
#[macro_export]
macro_rules! observer_methods {
    () => {
        fn observe_click(&mut self, pos: $crate::geometry::Point) -> Option<M> {
            self.observers.click(pos)
        }

        fn observe_right_click(&mut self, pos: $crate::geometry::Point) -> Option<M> {
            self.observers.right_click(pos)
        }

        fn observe_double_click(&mut self, pos: $crate::geometry::Point) -> Option<M> {
            self.observers.double_click(pos)
        }

        fn observe_key_down(&mut self, key: $crate::event::KeyEvent) -> Option<M> {
            self.observers.key_down(key)
        }

        fn observe_key_up(&mut self, key: $crate::event::KeyEvent) -> Option<M> {
            self.observers.key_up(key)
        }

        fn observe_focus_gained(&mut self, event: $crate::ui::FocusEvent) -> Option<M> {
            self.observers.focus_gained(event)
        }

        fn observe_focus_lost(&mut self, event: $crate::ui::FocusEvent) -> Option<M> {
            self.observers.focus_lost(event)
        }
    };
}

/// Generate the fluent builder methods for a widget's `observers` field.
/// Invoke at module scope with the widget's type name; the widget must have an
/// `observers: Observers<M>` field and must not already define these methods
/// (`TextEdit` defines its own, D106).
#[macro_export]
macro_rules! observer_builders {
    ($ty:ident) => {
        impl<M> $ty<M> {
            /// Called on a left-button click (press and release on the widget)
            /// before the built-in action, which still runs.
            pub fn on_click(
                mut self,
                handler: impl Fn($crate::geometry::Point) -> Option<M> + 'static,
            ) -> Self {
                self.observers.set_click(handler);
                self
            }

            /// Called on a right-button click before the built-in behavior
            /// (a context menu still opens).
            pub fn on_right_click(
                mut self,
                handler: impl Fn($crate::geometry::Point) -> Option<M> + 'static,
            ) -> Self {
                self.observers.set_right_click(handler);
                self
            }

            /// Called on a double click before the built-in behavior.
            pub fn on_double_click(
                mut self,
                handler: impl Fn($crate::geometry::Point) -> Option<M> + 'static,
            ) -> Self {
                self.observers.set_double_click(handler);
                self
            }

            /// Called on every key press that reaches the focused widget,
            /// before the built-in keyboard behavior.
            pub fn on_key_down(
                mut self,
                handler: impl Fn($crate::event::KeyEvent) -> Option<M> + 'static,
            ) -> Self {
                self.observers.set_key_down(handler);
                self
            }

            /// Called on every key release the focused widget sees.
            pub fn on_key_up(
                mut self,
                handler: impl Fn($crate::event::KeyEvent) -> Option<M> + 'static,
            ) -> Self {
                self.observers.set_key_up(handler);
                self
            }

            /// Called when focus moves onto the widget (after the container's
            /// decision hook, if any). Additive: the focus move still happens.
            pub fn on_focus_gained(
                mut self,
                handler: impl Fn($crate::ui::FocusEvent) -> Option<M> + 'static,
            ) -> Self {
                self.observers.set_focus_gained(handler);
                self
            }

            /// Called when focus leaves the widget (after its editor commits).
            /// Additive: the move still happens.
            pub fn on_focus_lost(
                mut self,
                handler: impl Fn($crate::ui::FocusEvent) -> Option<M> + 'static,
            ) -> Self {
                self.observers.set_focus_lost(handler);
                self
            }
        }
    };
}

pub use observer_builders;
pub use observer_methods;
