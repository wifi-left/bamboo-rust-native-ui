// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Anchor-aware popover card: the themed surface of a flyout.
//!
//! The application owns layout (D1), so placement is a helper: call
//! [`place_popover`] with the anchor rectangle, the card size and the client
//! viewport, then set the resulting rectangle on the widget. The card paints a
//! themed panel with a rounded border and an optional notch pointing at the
//! anchor; content widgets are ordinary siblings placed inside
//! [`Popover::content_rect`] and added **after** the popover, so they paint
//! above the card and receive pointer input first.
//!
//! With [`Popover::dismiss_on_outside`] (on by default) the widget's hit area
//! covers the client viewport while it is visible: a press outside the card
//! returns the dismissal message and is swallowed, so the page behind neither
//! receives the click nor a hover change. The application closes the flyout by
//! hiding the popover *and* its content widgets. Escape is the application's
//! key to handle (or a close control inside the card); the container's
//! transient-overlay `dismiss` path belongs to one-widget overlays such as the
//! combo drop-down.

use crate::a11y::{A11yInfo, A11yRole};
use crate::event::Cursor;
use crate::geometry::{Point, Rect, Size};
use crate::widget::{Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState};

/// Where the card goes relative to its anchor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PopoverPlacement {
    /// Below the anchor, horizontally centered.
    #[default]
    Below,
    Above,
    /// To the right of the anchor, vertically centered.
    Right,
    Left,
    /// Below when it fits, otherwise above; clamped into the viewport.
    Auto,
}

fn fits(rect: Rect, viewport: Rect) -> bool {
    viewport.is_empty()
        || (rect.top >= viewport.top
            && rect.left >= viewport.left
            && rect.bottom <= viewport.bottom
            && rect.right <= viewport.right)
}

/// Compute the card rectangle for `anchor`. The result is clamped into
/// `viewport` with an 8 DIP margin when the viewport is non-empty (an empty
/// viewport means "no clamping", which is what the initial layout pass uses).
pub fn place_popover(
    anchor: Rect,
    size: Size,
    placement: PopoverPlacement,
    viewport: Rect,
    gap: f32,
) -> Rect {
    let gap = gap.max(0.0);
    let width = size.width.max(1.0);
    let height = size.height.max(1.0);
    let center = anchor.center();
    let below = Rect::from_xywh(center.x - width * 0.5, anchor.bottom + gap, width, height);
    let above = Rect::from_xywh(center.x - width * 0.5, anchor.top - gap - height, width, height);
    let right = Rect::from_xywh(anchor.right + gap, center.y - height * 0.5, width, height);
    let left = Rect::from_xywh(anchor.left - gap - width, center.y - height * 0.5, width, height);
    let rect = match placement {
        PopoverPlacement::Below => below,
        PopoverPlacement::Above => above,
        PopoverPlacement::Right => right,
        PopoverPlacement::Left => left,
        PopoverPlacement::Auto => {
            if fits(below, viewport) {
                below
            } else if fits(above, viewport) {
                above
            } else if fits(right, viewport) {
                right
            } else if fits(left, viewport) {
                left
            } else {
                // Nothing fits cleanly: prefer the roomier vertical side and
                // let the clamp below keep it on screen.
                let below_room = if viewport.is_empty() {
                    f32::MAX
                } else {
                    viewport.bottom - anchor.bottom
                };
                let above_room = if viewport.is_empty() {
                    f32::MAX
                } else {
                    anchor.top - viewport.top
                };
                if below_room >= above_room {
                    below
                } else {
                    above
                }
            }
        }
    };
    if viewport.is_empty() {
        return rect;
    }
    let margin = 8.0;
    let max_x = (viewport.right - margin - width).max(viewport.left + margin);
    let max_y = (viewport.bottom - margin - height).max(viewport.top + margin);
    Rect::from_xywh(
        rect.left.clamp(viewport.left + margin, max_x),
        rect.top.clamp(viewport.top + margin, max_y),
        width,
        height,
    )
}

pub struct Popover<M> {
    id: WidgetId,
    rect: Rect,
    /// Anchor the notch points at. `None` paints a plain card.
    anchor: Option<Rect>,
    pad: f32,
    radius: f32,
    dismiss_on_outside: bool,
    arrow: bool,
    name: String,
    description: String,
    /// Last painted client viewport. The full-area light-dismiss hit test needs
    /// it; the container paints a visible widget before delivering a press.
    viewport: std::cell::Cell<Rect>,
    on_dismiss: Option<Box<dyn Fn() -> M>>,
}

impl<M> Popover<M> {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 120.0),
            anchor: None,
            pad: 12.0,
            radius: crate::theme::radius::MEDIUM,
            dismiss_on_outside: true,
            arrow: true,
            name: String::new(),
            description: String::new(),
            viewport: std::cell::Cell::new(Rect::default()),
            on_dismiss: None,
        }
    }

    /// Anchor the notch points at (it does not move the card; use
    /// [`place_popover`] for that).
    pub fn anchor(mut self, anchor: Rect) -> Self {
        self.anchor = Some(anchor);
        self
    }

    pub fn set_anchor(&mut self, anchor: Rect) {
        self.anchor = Some(anchor);
    }

    /// Padding between the card edge and [`Popover::content_rect`].
    pub fn pad(mut self, pad: f32) -> Self {
        self.pad = pad.max(0.0);
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.radius = radius.max(0.0);
        self
    }

    /// A press outside the card dismisses (the default). Turn it off for a
    /// non-modal card that lets the page receive clicks.
    pub fn dismiss_on_outside(mut self, dismiss: bool) -> Self {
        self.dismiss_on_outside = dismiss;
        self
    }

    /// Paint the notch that points at the anchor (default on).
    pub fn arrow(mut self, arrow: bool) -> Self {
        self.arrow = arrow;
        self
    }

    /// Accessible name of the card group.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Message emitted by an outside press.
    pub fn on_dismiss(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.on_dismiss = Some(Box::new(handler));
        self
    }

    /// Area for the application's content widgets (the card inset by padding).
    pub fn content_rect(&self) -> Rect {
        self.rect.inset(self.pad, self.pad)
    }

    pub fn dismiss_on_outside_enabled(&self) -> bool {
        self.dismiss_on_outside
    }

    /// Client viewport used by the light-dismiss hit area. `paint` refreshes
    /// it; the application can seed it before the first frame.
    pub fn set_viewport(&mut self, viewport: Rect) {
        self.viewport.set(viewport);
    }

    pub fn viewport(&self) -> Rect {
        self.viewport.get()
    }

    fn outside_card(&self, pos: Point) -> bool {
        if self.rect.contains_point(pos) {
            return false;
        }
        let viewport = self.viewport.get();
        !viewport.is_empty() && viewport.contains_point(pos)
    }

    /// The notch: the point on the card edge facing the anchor and which edge
    /// (0=top, 1=bottom, 2=left, 3=right). `None` when there is no anchor, the
    /// arrow is off, or the anchor overlaps the card.
    fn notch(&self) -> Option<(Point, u8)> {
        if !self.arrow {
            return None;
        }
        let anchor = self.anchor?;
        let min_gap = self.radius + 6.0;
        let (point, edge) = if anchor.bottom <= self.rect.top {
            (
                Point::new(
                    anchor
                        .center()
                        .x
                        .clamp(self.rect.left + min_gap, self.rect.right - min_gap),
                    self.rect.top,
                ),
                0,
            )
        } else if anchor.top >= self.rect.bottom {
            (
                Point::new(
                    anchor
                        .center()
                        .x
                        .clamp(self.rect.left + min_gap, self.rect.right - min_gap),
                    self.rect.bottom,
                ),
                1,
            )
        } else if anchor.right <= self.rect.left {
            (
                Point::new(
                    self.rect.left,
                    anchor
                        .center()
                        .y
                        .clamp(self.rect.top + min_gap, self.rect.bottom - min_gap),
                ),
                2,
            )
        } else if anchor.left >= self.rect.right {
            (
                Point::new(
                    self.rect.right,
                    anchor
                        .center()
                        .y
                        .clamp(self.rect.top + min_gap, self.rect.bottom - min_gap),
                ),
                3,
            )
        } else {
            return None;
        };
        Some((point, edge))
    }
}

impl<M: 'static> Widget<M> for Popover<M> {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn preferred_height(&self, _width: f32) -> f32 {
        120.0
    }

    fn focusable(&self) -> bool {
        false
    }

    /// Blocks presses meant for the page (the card in either mode, the whole
    /// viewport in light-dismiss mode).
    fn interactive(&self) -> bool {
        true
    }

    fn hit_test(&self, pos: Point) -> bool {
        self.rect.contains_point(pos)
            || (self.dismiss_on_outside && self.outside_card(pos))
    }

    /// No pointer feedback: moving across the page while the flyout is open
    /// must not repaint the whole viewport.
    fn paints_pointer_state(&self) -> bool {
        false
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::Arrow)
    }

    fn paint_bounds(&self) -> Rect {
        match self.notch() {
            Some((_, _)) => self.rect.inflate(10.0, 10.0),
            None => self.rect.inflate(1.0, 1.0),
        }
    }

    fn a11y(&self) -> Option<A11yInfo> {
        Some(
            A11yInfo::new(A11yRole::Group, self.name.clone())
                .description(self.description.clone()),
        )
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        self.viewport.set(ctx.viewport);
        let painter = ctx.painter;
        painter.round_rect(
            &mut *ctx.canvas,
            self.rect,
            self.radius,
            palette.explicit_border,
        );
        painter.round_rect(
            &mut *ctx.canvas,
            self.rect.inset(1.0, 1.0),
            (self.radius - 1.0).max(0.0),
            palette.panel,
        );
        // The notch is a small filled triangle straddling the card border, so
        // the card reads as pointing at the anchor. Rows are 1 DIP tall and
        // grow toward the card edge (no polygon primitive needed).
        if let Some((point, edge)) = self.notch() {
            const DEPTH: f32 = 8.0;
            const HALF: f32 = 6.0;
            let step = HALF / DEPTH;
            for i in 0..DEPTH as i32 {
                let grow = step * (i as f32 + 1.0);
                let span = match edge {
                    // Card below the anchor: apex at the top.
                    0 => {
                        let y = point.y - DEPTH + i as f32;
                        Rect::new(point.x - grow, y, point.x + grow, y + 1.0)
                    }
                    // Card above the anchor: apex at the bottom.
                    1 => {
                        let y = point.y + DEPTH - 1.0 - i as f32;
                        Rect::new(point.x - grow, y, point.x + grow, y + 1.0)
                    }
                    // Card right of the anchor: apex on the left.
                    2 => {
                        let x = point.x - DEPTH + i as f32;
                        Rect::new(x, point.y - grow, x + 1.0, point.y + grow)
                    }
                    // Card left of the anchor: apex on the right.
                    _ => {
                        let x = point.x + DEPTH - 1.0 - i as f32;
                        Rect::new(x, point.y - grow, x + 1.0, point.y + grow)
                    }
                };
                ctx.canvas.fill_rect(span, palette.panel);
            }
        }
    }

    fn input(&mut self, input: WidgetInput) -> Option<M> {
        if let WidgetInput::PressAt(pos) = input
            && self.dismiss_on_outside
            && self.outside_card(pos)
        {
            return self.on_dismiss.as_ref().map(|handler| handler());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::Widget;

    #[derive(Debug, PartialEq)]
    enum Msg {
        Dismiss,
    }

    fn popover() -> Popover<Msg> {
        Popover::new("flyout")
            .anchor(Rect::from_xywh(100.0, 100.0, 100.0, 32.0))
            .name("弹出层")
            .on_dismiss(|| Msg::Dismiss)
    }

    const VIEWPORT: Rect = Rect {
        left: 0.0,
        top: 0.0,
        right: 800.0,
        bottom: 600.0,
    };

    #[test]
    fn below_and_above_center_on_the_anchor() {
        let anchor = Rect::from_xywh(300.0, 200.0, 100.0, 30.0);
        let below = place_popover(
            anchor,
            Size::new(200.0, 80.0),
            PopoverPlacement::Below,
            VIEWPORT,
            8.0,
        );
        assert_eq!(below.left, 250.0);
        assert_eq!(below.top, 238.0);
        let above = place_popover(
            anchor,
            Size::new(200.0, 80.0),
            PopoverPlacement::Above,
            VIEWPORT,
            8.0,
        );
        assert_eq!(above.left, 250.0);
        assert_eq!(above.bottom, 192.0);
    }

    #[test]
    fn auto_flips_above_near_the_bottom_edge() {
        let anchor = Rect::from_xywh(300.0, 570.0, 100.0, 24.0);
        let rect = place_popover(
            anchor,
            Size::new(220.0, 90.0),
            PopoverPlacement::Auto,
            VIEWPORT,
            8.0,
        );
        assert!(rect.bottom < anchor.top, "the card must flip above the anchor");
        assert!(rect.top >= VIEWPORT.top);
    }

    #[test]
    fn auto_clamps_into_the_viewport() {
        let anchor = Rect::from_xywh(760.0, 100.0, 32.0, 32.0);
        let rect = place_popover(
            anchor,
            Size::new(220.0, 100.0),
            PopoverPlacement::Auto,
            VIEWPORT,
            8.0,
        );
        assert!(rect.right <= VIEWPORT.right - 8.0 + 0.01);
        assert!(rect.left >= VIEWPORT.left);
        assert_eq!(rect.width(), 220.0);
    }

    #[test]
    fn explicit_sides_are_available() {
        let anchor = Rect::from_xywh(400.0, 300.0, 20.0, 20.0);
        let right = place_popover(
            anchor,
            Size::new(120.0, 60.0),
            PopoverPlacement::Right,
            VIEWPORT,
            6.0,
        );
        assert_eq!(right.left, 426.0);
        let left = place_popover(
            anchor,
            Size::new(120.0, 60.0),
            PopoverPlacement::Left,
            VIEWPORT,
            6.0,
        );
        assert_eq!(left.right, 394.0);
    }

    #[test]
    fn content_rect_is_the_card_inset_by_padding() {
        let mut card = popover().pad(14.0);
        card.set_rect(Rect::from_xywh(100.0, 150.0, 240.0, 120.0));
        let content = card.content_rect();
        assert_eq!(content.left, 114.0);
        assert_eq!(content.top, 164.0);
        assert_eq!(content.right, 326.0);
        assert_eq!(content.bottom, 256.0);
    }

    #[test]
    fn outside_press_dismisses_but_the_card_swallows_it() {
        let mut card = popover();
        card.set_rect(Rect::from_xywh(100.0, 150.0, 240.0, 120.0));
        card.set_viewport(VIEWPORT);
        // The page area is part of the hit area so the press reaches the card.
        assert!(card.hit_test(Point::new(600.0, 500.0)));
        assert_eq!(
            card.input(WidgetInput::PressAt(Point::new(600.0, 500.0))),
            Some(Msg::Dismiss)
        );
        // A press on the card itself is swallowed without dismissing.
        assert_eq!(
            card.input(WidgetInput::PressAt(Point::new(200.0, 200.0))),
            None
        );
        // With light dismissal off the page is not part of the hit area.
        let mut plain = popover().dismiss_on_outside(false);
        plain.set_rect(Rect::from_xywh(100.0, 150.0, 240.0, 120.0));
        plain.set_viewport(VIEWPORT);
        assert!(!plain.hit_test(Point::new(600.0, 500.0)));
        assert_eq!(plain.input(WidgetInput::PressAt(Point::new(600.0, 500.0))), None);
    }

    #[test]
    fn a_missing_viewport_only_hits_the_card() {
        let mut card = popover();
        card.set_rect(Rect::from_xywh(100.0, 150.0, 240.0, 120.0));
        assert!(card.hit_test(Point::new(120.0, 160.0)));
        assert!(!card.hit_test(Point::new(600.0, 500.0)));
        assert_eq!(card.input(WidgetInput::PressAt(Point::new(600.0, 500.0))), None);
    }

    #[test]
    fn the_notch_points_at_the_anchor_on_the_nearest_side() {
        let mut card = popover();
        card.set_rect(Rect::from_xywh(100.0, 150.0, 240.0, 120.0));
        let (point, edge) = card.notch().expect("the anchor is above the card");
        assert_eq!(edge, 0, "the notch sits on the top edge");
        assert!((point.x - 150.0).abs() < 0.01);
        assert!((point.y - 150.0).abs() < 0.01);
        // No anchor, no notch.
        let mut plain: Popover<Msg> = Popover::new("f");
        plain.set_rect(Rect::from_xywh(100.0, 150.0, 240.0, 120.0));
        assert!(plain.notch().is_none());
    }

    #[test]
    fn a11y_publishes_a_named_group() {
        let mut card = popover().description("点击外部关闭");
        card.set_rect(Rect::from_xywh(100.0, 150.0, 240.0, 120.0));
        let info = card.a11y().expect("the card is exposed");
        assert_eq!(info.role, A11yRole::Group);
        assert_eq!(info.name, "弹出层");
        assert_eq!(info.description.as_deref(), Some("点击外部关闭"));
        assert!(!card.focusable());
    }
}
