// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! A closure-painted widget for application-specific components.
//!
//! The library's own widgets implement [`Widget`] directly because they carry
//! a lot of state. A small custom component usually does not: it wants a
//! rectangle, a paint closure and maybe a click handler. [`CustomWidget`]
//! provides exactly that and is registered like any other component:
//!
//! ```ignore
//! ui.add(
//!     CustomWidget::new("meter", |ctx, rect, state| {
//!         ctx.painter
//!             .round_rect(ctx.canvas, rect.inset(4.0, 4.0), 8.0, ctx.palette.accent);
//!         ctx.text(rect, "72%", TextStyleRole::Body, TextAlign::Center,
//!             if state.hovered { ctx.palette.text } else { ctx.palette.muted });
//!     })
//!     .rect(Rect::from_xywh(20.0, 20.0, 160.0, 40.0))
//!     .input(|_rect, input| match input {
//!         WidgetInput::ClickAt(pos) => Some(Msg::Clicked(pos)),
//!         _ => None,
//!     }),
//! );
//! ```
//!
//! It implements the same seams as a built-in widget: `WidgetProps` through
//! [`crate::Ui::add_with`], typing through `Ui::with_widget`, the additive
//! [`crate::widgets::observers`] builders (store an `Observers<M>` field if you
//! need them), the focus ring, a11y, surface builders (`.corner_radius`,
//! `.background`, …) and the layout skin's `SurfaceStyle` seam.
//!
//! For behaviour that needs owned state, implement [`Widget`] directly (see
//! `crates/showcase/src/meter.rs` and `docs/CUSTOM-WIDGETS.md`).

use crate::a11y::{A11yInfo, A11yRole};
use crate::event::Cursor;
use crate::geometry::{Point, Rect};
use crate::surface::{SurfaceStyle, surface_builders};
use crate::widget::{Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

type PaintFn = Box<dyn for<'a, 'b> Fn(&'a mut WidgetPaint<'b>, Rect, &'a WidgetState)>;
type InputFn<M> = Box<dyn FnMut(Rect, WidgetInput) -> Option<M>>;
type HitFn = Box<dyn Fn(Rect, Point) -> bool>;
type A11yFn = Box<dyn Fn() -> Option<A11yInfo>>;

/// A widget whose paint and input behaviour are application closures.
pub struct CustomWidget<M> {
    id: WidgetId,
    rect: Rect,
    surface: SurfaceStyle,
    preferred_height: f32,
    focusable: bool,
    interactive: bool,
    enabled: bool,
    cursor: Option<Cursor>,
    a11y_fixed: Option<A11yInfo>,
    a11y_fn: Option<A11yFn>,
    hit_test: Option<HitFn>,
    paint: PaintFn,
    input: Option<InputFn<M>>,
}

impl<M> CustomWidget<M> {
    /// `paint` receives the paint context, the widget rectangle and the
    /// container state (hover/press/focus/enabled). It must draw only inside
    /// `rect`: the container blits the widget's dirty rectangle.
    pub fn new(
        id: &'static str,
        paint: impl for<'a, 'b> Fn(&'a mut WidgetPaint<'b>, Rect, &'a WidgetState) + 'static,
    ) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 120.0, 32.0),
            surface: SurfaceStyle::new(),
            preferred_height: 32.0,
            focusable: false,
            interactive: false,
            enabled: true,
            cursor: None,
            a11y_fixed: None,
            a11y_fn: None,
            hit_test: None,
            paint: Box::new(paint),
            input: None,
        }
    }

    /// Initial rectangle (layout may move it later).
    pub fn rect(mut self, rect: Rect) -> Self {
        self.rect = rect;
        self
    }

    /// Height the widget wants in a vertical stack (default 32).
    pub fn preferred_height(mut self, height: f32) -> Self {
        self.preferred_height = height;
        self
    }

    /// Whether the widget takes keyboard/a11y focus (default false). The
    /// container paints the focus ring, so a focusable custom widget gets one.
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    /// Whether pointer hit-testing may land on the widget (default false).
    /// Turn it on for a clickable component; pair it with `.input`.
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Initial enabled state (the container can change it later).
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn cursor(mut self, cursor: Cursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    /// The input handler. Positions are client DIPs; the rectangle is passed
    /// so the handler can test against sub-areas. Return `None` to stay silent.
    pub fn input(
        mut self,
        handler: impl FnMut(Rect, WidgetInput) -> Option<M> + 'static,
    ) -> Self {
        self.input = Some(Box::new(handler));
        self
    }

    /// A non-rectangular hit area (a circle, a rounded card with a notch).
    pub fn hit_test(mut self, hit_test: impl Fn(Rect, Point) -> bool + 'static) -> Self {
        self.hit_test = Some(Box::new(hit_test));
        self
    }

    /// A fixed accessibility node.
    pub fn a11y(mut self, info: A11yInfo) -> Self {
        self.a11y_fixed = Some(info);
        self
    }

    /// Convenience for the common fixed node: a role and a name.
    pub fn a11y_name(mut self, role: A11yRole, name: impl Into<String>) -> Self {
        self.a11y_fixed = Some(A11yInfo::new(role, name));
        self
    }

    /// A dynamic accessibility node (a value that changes every frame).
    pub fn a11y_fn(mut self, info: impl Fn() -> Option<A11yInfo> + 'static) -> Self {
        self.a11y_fn = Some(Box::new(info));
        self
    }
}

surface_builders!(CustomWidget);

impl<M: 'static> Widget<M> for CustomWidget<M> {
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

    fn preferred_height(&self, _width: f32) -> f32 {
        self.preferred_height
    }

    fn focusable(&self) -> bool {
        self.focusable
    }

    fn interactive(&self) -> bool {
        self.interactive
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn cursor(&self) -> Option<Cursor> {
        self.cursor
    }

    fn hit_test(&self, pos: Point) -> bool {
        match &self.hit_test {
            Some(hit) => hit(self.rect, pos),
            None => self.rect.contains_point(pos),
        }
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, state: &WidgetState) {
        // A configured surface (radius/background/transparent) paints first so
        // the closure only draws content. The default surface stays invisible.
        if self.surface != SurfaceStyle::new() {
            let palette = ctx.palette;
            self.surface
                .frame(ctx.canvas, self.rect, palette.explicit_border, palette.card);
        }
        (self.paint)(ctx, self.rect, state);
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        match &mut self.input {
            Some(handler) => handler(self.rect, input),
            None => None,
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        if let Some(dynamic) = &self.a11y_fn {
            return dynamic();
        }
        self.a11y_fixed.clone()
    }

    fn surface_style(&self) -> Option<&SurfaceStyle> {
        Some(&self.surface)
    }

    fn surface_style_mut(&mut self) -> Option<&mut SurfaceStyle> {
        Some(&mut self.surface)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Canvas, ImageData, ImageId, TextAlign, TextStyle};
    use crate::color::Color;
    use crate::geometry::Size;

    #[derive(Default)]
    struct Blank;

    impl Canvas for Blank {
        fn scale(&self) -> f32 {
            1.0
        }
        fn dpi(&self) -> f32 {
            96.0
        }
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
    fn a_custom_widget_paints_and_inputs_through_the_trait() {
        use std::cell::Cell;
        use std::rc::Rc;
        let painted = Rc::new(Cell::new(0));
        let seen = Rc::new(Cell::new((0.0f32, 0.0f32)));
        let painted_in = Rc::clone(&painted);
        let seen_in = Rc::clone(&seen);
        let mut widget: CustomWidget<&'static str> = CustomWidget::new(
            "custom",
            move |_ctx, rect, _state| {
                painted_in.set(painted_in.get() + 1);
                seen_in.set((rect.left, rect.top));
            },
        )
        .rect(Rect::from_xywh(10.0, 20.0, 80.0, 30.0))
        .focusable(true)
        .interactive(true)
        .input(|rect, input| match input {
            WidgetInput::ClickAt(pos) => Some(if rect.contains_point(pos) {
                "clicked"
            } else {
                "outside"
            }),
            _ => None,
        });

        let painter = crate::painter::Painter::new();
        let images = crate::image::ImageStore::new();
        let mut canvas = Blank;
        let state = WidgetState::default();
        let mut ctx = WidgetPaint {
            painter: &painter,
            canvas: &mut canvas,
            palette: *painter.palette(),
            font: None,
            images: &images,
            viewport: Rect::from_xywh(0.0, 0.0, 400.0, 300.0),
            dirty: &[],
        };
        <CustomWidget<&'static str> as Widget<&'static str>>::paint(&mut widget, &mut ctx, &state);
        assert_eq!(painted.get(), 1);
        assert_eq!(seen.get(), (10.0, 20.0));

        assert_eq!(
            <CustomWidget<&'static str> as Widget<&'static str>>::input(
                &mut widget,
                WidgetInput::ClickAt(Point::new(20.0, 25.0)),
            ),
            Some("clicked")
        );
        assert!(<CustomWidget<&'static str> as Widget<&'static str>>::focusable(&widget));
        assert!(<CustomWidget<&'static str> as Widget<&'static str>>::interactive(&widget));
        assert!(<CustomWidget<&'static str> as Widget<&'static str>>::hit_test(
            &widget,
            Point::new(15.0, 22.0)
        ));
    }

    #[test]
    fn surface_builders_and_a11y_are_available() {
        let widget: CustomWidget<()> = CustomWidget::new("c", |_, _, _| {})
            .corner_radius(8.0)
            .background(Color::new(1, 2, 3))
            .a11y_name(A11yRole::Button, "自定义");
        assert_eq!(
            widget.surface().corners,
            crate::geometry::Corners::uniform(8.0)
        );
        assert_eq!(widget.surface().background, Some(Color::new(1, 2, 3)));
        let info = <CustomWidget<()> as Widget<()>>::a11y(&widget).expect("a11y");
        assert_eq!(info.role, A11yRole::Button);
        assert_eq!(info.name, "自定义");
    }
}
