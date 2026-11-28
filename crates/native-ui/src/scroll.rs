// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! A scrollable viewport over a set of child widgets.
//!
//! `Ui` is a flat widget map, so a scroll view is a *container region*, not a
//! widget that owns children: the application supplies the viewport and lays
//! its children out once in content coordinates (the viewport's top-left corner
//! is the content origin). The container keeps every child's rectangle at
//! `content rect - offset`, so hit-testing, painting, typed access, the native
//! editor, tooltips and the accessibility tree keep reading ordinary client
//! rectangles and nothing else has to know about the second coordinate space.
//! See D65.
//!
//! The scrollbars are painted and dragged by the container with the shared
//! [`crate::widgets::scrollbar`] math; the panel only stores the style and the
//! per-bar pointer state.

use crate::backend::Canvas;
use crate::geometry::{Insets, Point, Rect, Size};
use crate::painter::Painter;
use crate::theme::Palette;
use crate::widget::{ScrollBars, WidgetId};
use crate::widgets::scrollbar::{self, ScrollBarState, ScrollBarStyle};

/// One child of a scroll view: the widget id and its rectangle in content
/// coordinates. The container positions the widget at `rect - offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollChild {
    pub id: WidgetId,
    /// Content coordinates; the viewport origin is `(0, 0)`.
    pub rect: Rect,
}

impl ScrollChild {
    pub fn new(id: WidgetId, rect: Rect) -> Self {
        Self { id, rect }
    }
}

/// A scrollable viewport: viewport rectangle, axes, style and the children laid
/// out in content coordinates.
///
/// Build it once and hand it to [`crate::Ui::set_scroll_view`]; call the setter
/// again after a resize or a child-layout change (the container re-positions
/// the children, so an application never moves a scrolled widget itself).
#[derive(Clone, Debug)]
pub struct ScrollView {
    id: WidgetId,
    /// Viewport as configured. A root panel's is in client coordinates; a
    /// nested panel's is in its parent's content coordinates.
    viewport: Rect,
    /// Viewport in client coordinates, resolved by the container (equal to
    /// `viewport` for a root panel).
    effective: Rect,
    /// Client-space clip: the effective viewport intersected with every
    /// ancestor panel's viewport.
    clip: Rect,
    /// Enclosing panel, if this panel is nested inside another scroll view.
    parent: Option<WidgetId>,
    axes: ScrollBars,
    style: ScrollBarStyle,
    children: Vec<ScrollChild>,
    offset: Point,
    content: Option<Size>,
    resolved: Size,
    wheel_step: f32,
    name: Option<String>,
    /// Inset between the viewport edge and the content area. The bars stay on
    /// the viewport edge, so a right/bottom padding also keeps the content out
    /// from under them.
    padding: Insets,
    vertical: ScrollBarState,
    horizontal: ScrollBarState,
}

/// One visible bar: the track and the thumb inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Bar {
    pub track: Rect,
    pub thumb: Rect,
}

/// Bar geometry for one paint or hit test. A bar exists only while its axis
/// actually overflows, so a short panel shows no empty track.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct BarsLayout {
    pub vertical: Option<Bar>,
    pub horizontal: Option<Bar>,
}

impl ScrollView {
    /// Wheel travel per notch in DIPs (three text lines).
    pub const DEFAULT_WHEEL_STEP: f32 = 48.0;

    pub fn new(id: WidgetId, viewport: Rect, axes: ScrollBars) -> Self {
        Self {
            id,
            viewport,
            effective: viewport,
            clip: viewport,
            parent: None,
            axes,
            style: ScrollBarStyle::default(),
            children: Vec::new(),
            offset: Point::new(0.0, 0.0),
            content: None,
            resolved: Size::new(0.0, 0.0),
            wheel_step: Self::DEFAULT_WHEEL_STEP,
            name: None,
            padding: Insets::ZERO,
            vertical: ScrollBarState::default(),
            horizontal: ScrollBarState::default(),
        }
    }

    pub fn children(mut self, children: impl IntoIterator<Item = ScrollChild>) -> Self {
        self.children = children.into_iter().collect();
        self
    }

    pub fn child(mut self, id: WidgetId, rect: Rect) -> Self {
        self.children.push(ScrollChild::new(id, rect));
        self
    }

    /// Initial scroll offset; clamped to the content on registration.
    pub fn offset(mut self, offset: Point) -> Self {
        self.offset = offset;
        self
    }

    /// Set the scroll offset (the container clamps it and re-positions the
    /// children afterwards).
    pub fn set_offset(&mut self, offset: Point) {
        self.offset = offset;
    }

    /// Explicit content extent. Without it the extent is derived from the
    /// children's rectangles, which is what a normal panel wants.
    pub fn content(mut self, content: Size) -> Self {
        self.content = Some(Size::new(content.width.max(0.0), content.height.max(0.0)));
        self
    }

    pub fn style(mut self, style: ScrollBarStyle) -> Self {
        self.style = style;
        self
    }

    pub fn wheel_step(mut self, dips: f32) -> Self {
        self.wheel_step = dips.max(1.0);
        self
    }

    /// Margin between the viewport edge and the content, in DIPs (all sides).
    /// The content is laid out, clipped and scrolled inside this inset area,
    /// so a row stops at the margin instead of running under a scrollbar.
    pub fn padding(mut self, dips: f32) -> Self {
        self.padding = Insets::uniform(dips.max(0.0));
        self
    }

    /// Per-side padding; see [`ScrollView::padding`].
    pub fn padding_by(mut self, insets: Insets) -> Self {
        self.padding = Insets {
            left: insets.left.max(0.0),
            top: insets.top.max(0.0),
            right: insets.right.max(0.0),
            bottom: insets.bottom.max(0.0),
        };
        self
    }

    /// Accessibility name of the viewport (a UIA `Group`). Without a name the
    /// viewport is not published; its children are published as usual.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Mark the viewport container and its children as one `Group`.
    pub fn named(self, name: impl Into<String>) -> Self {
        self.name(name)
    }

    pub fn id(&self) -> WidgetId {
        self.id
    }

    /// The viewport in client coordinates. For a nested panel this is the
    /// configured rectangle moved by the enclosing panel's offset.
    pub fn viewport(&self) -> Rect {
        self.effective
    }

    /// The viewport as the application configured it (client coordinates for
    /// a root panel, the parent's content coordinates for a nested one).
    pub fn configured_viewport(&self) -> Rect {
        self.viewport
    }

    /// Set the configured viewport. The container resolves the effective one
    /// (and a nested panel's, through its parent) on the next layout.
    pub fn set_viewport(&mut self, viewport: Rect) {
        self.viewport = viewport;
        if self.parent.is_none() {
            self.effective = viewport;
            self.clip = viewport;
        }
    }

    /// Nest this panel inside `parent`: the configured viewport becomes a
    /// rectangle in the parent's content coordinates, and both axes move and
    /// clip with the parent (D92).
    pub fn nested_in(mut self, parent: WidgetId) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn parent(&self) -> Option<WidgetId> {
        self.parent
    }

    pub(crate) fn set_parent(&mut self, parent: Option<WidgetId>) {
        self.parent = parent;
    }

    /// Client-space clip (the effective viewport intersected with every
    /// ancestor panel's).
    pub fn clip_rect(&self) -> Rect {
        self.clip
    }

    /// Resolve the client viewport/clip; the container calls this before
    /// laying the panel out.
    pub(crate) fn set_effective(&mut self, effective: Rect, clip: Rect) {
        self.effective = effective;
        self.clip = clip;
    }

    pub fn axes(&self) -> ScrollBars {
        self.axes
    }

    pub fn set_axes(&mut self, axes: ScrollBars) {
        self.axes = axes;
    }

    pub fn style_ref(&self) -> &ScrollBarStyle {
        &self.style
    }

    pub fn set_style(&mut self, style: ScrollBarStyle) {
        self.style = style;
    }

    pub fn children_ref(&self) -> &[ScrollChild] {
        &self.children
    }

    pub fn set_children(&mut self, children: impl IntoIterator<Item = ScrollChild>) {
        self.children = children.into_iter().collect();
    }

    /// Whether `id` is one of the children.
    pub fn contains(&self, id: WidgetId) -> bool {
        self.children.iter().any(|child| child.id == id)
    }

    /// The child's rectangle in content coordinates.
    pub fn child_rect(&self, id: WidgetId) -> Option<Rect> {
        self.children
            .iter()
            .find(|child| child.id == id)
            .map(|child| child.rect)
    }

    /// Drop a child (the widget was removed from the page).
    pub(crate) fn forget(&mut self, id: WidgetId) {
        self.children.retain(|child| child.id != id);
    }

    pub fn offset_of(&self) -> Point {
        self.offset
    }

    /// The content extent currently in use (the derived value when the
    /// application did not supply one).
    pub fn content_size(&self) -> Size {
        self.resolved
    }

    pub fn set_content_override(&mut self, content: Option<Size>) {
        self.content = content;
    }

    pub fn max_offset(&self) -> Point {
        let area = self.content_rect();
        Point::new(
            (self.resolved.width - area.width()).max(0.0),
            (self.resolved.height - area.height()).max(0.0),
        )
    }

    pub fn wheel_step_dips(&self) -> f32 {
        self.wheel_step
    }

    pub fn padding_insets(&self) -> Insets {
        self.padding
    }

    /// Which bars are actually shown (vertical, horizontal). An axis needs an
    /// enabled bar *and* content that does not fit the padded viewport. The
    /// decision deliberately ignores the *other* bar's thickness: content
    /// sized to the viewport must not grow a horizontal scrollbar the moment a
    /// vertical one appears (the bars overlay the content's edge while
    /// mid-scroll; only the range's end keeps the padding gap).
    pub(crate) fn bar_axes(&self) -> (bool, bool) {
        let padded = self.padded_rect();
        let vertical = self.axes.vertical() && self.resolved.height > padded.height() + 0.01;
        let horizontal = self.axes.horizontal() && self.resolved.width > padded.width() + 0.01;
        (vertical, horizontal)
    }

    /// The viewport inset by [`ScrollView::padding`] only. This is what a
    /// focus reveal compares against: the bars overlay the edge, so a widget
    /// touching this rectangle is fully reachable.
    pub fn padded_rect(&self) -> Rect {
        let viewport = self.viewport();
        let rect = viewport.inset_by(self.padding);
        if rect.is_empty() { viewport } else { rect }
    }

    /// The area the scroll range ends in: the viewport inset by
    /// [`ScrollView::padding`], then minus the thickness of every bar that is
    /// actually shown. Its origin is where content coordinates start (so the
    /// left/top padding is the layout margin); its right/bottom edge is where
    /// the content stops at the end of the range, one `padding` in from the
    /// bar. The *clip* rectangle is the whole viewport, not this one: content
    /// is shown at full size and may only slide under an overlay bar while
    /// mid-scroll, so the drawing area never shrinks.
    pub fn content_rect(&self) -> Rect {
        let viewport = self.viewport();
        let base = viewport.inset_by(self.padding);
        let base = if base.is_empty() { viewport } else { base };
        let thickness = self.style.thickness;
        let (vertical, horizontal) = self.bar_axes();
        let area = Rect::new(
            base.left,
            base.top,
            base.right - if vertical { thickness } else { 0.0 },
            base.bottom - if horizontal { thickness } else { 0.0 },
        );
        if area.is_empty() { base } else { area }
    }

    pub fn name_ref(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Pointer state of one bar (vertical or horizontal).
    pub(crate) fn bar_state(&self, vertical: bool) -> &ScrollBarState {
        if vertical {
            &self.vertical
        } else {
            &self.horizontal
        }
    }

    pub(crate) fn bar_state_mut(&mut self, vertical: bool) -> &mut ScrollBarState {
        if vertical {
            &mut self.vertical
        } else {
            &mut self.horizontal
        }
    }

    /// Recompute the content extent from the children (or the override) and
    /// the scrollbars' geometry.
    pub(crate) fn resolve(&mut self) {
        let derived = self.children.iter().fold(Size::new(0.0, 0.0), |size, child| {
            Size::new(
                size.width.max(child.rect.right.max(0.0)),
                size.height.max(child.rect.bottom.max(0.0)),
            )
        });
        let content = match self.content {
            Some(content) => Size::new(
                content.width.max(derived.width),
                content.height.max(derived.height),
            ),
            None => derived,
        };
        // `content_rect` decides which bars show from the content extent, so
        // resolve in two steps: the provisional extent, then the extent floored
        // by the resulting area (nothing beyond it still reports the visible
        // size, so the thumb ratio never divides by zero).
        self.resolved = content;
        let area = self.content_rect();
        self.resolved = Size::new(
            content.width.max(area.width()),
            content.height.max(area.height()),
        );
    }

    /// Clamp the offset into the scrollable range. Returns whether it moved.
    pub(crate) fn clamp(&mut self) -> bool {
        let max = self.max_offset();
        let clamped = Point::new(
            self.offset.x.clamp(0.0, max.x),
            self.offset.y.clamp(0.0, max.y),
        );
        if clamped == self.offset {
            return false;
        }
        self.offset = clamped;
        true
    }

    /// Scroll the least distance that brings `client_rect` fully into the
    /// viewport. Returns whether the offset moved.
    pub(crate) fn reveal(&mut self, client_rect: Rect) -> bool {
        let before = self.offset;
        let view = self.padded_rect();
        let mut offset = self.offset;
        if client_rect.top < view.top {
            offset.y -= view.top - client_rect.top;
        } else if client_rect.bottom > view.bottom {
            offset.y += client_rect.bottom - view.bottom;
        }
        if client_rect.left < view.left {
            offset.x -= view.left - client_rect.left;
        } else if client_rect.right > view.right {
            offset.x += client_rect.right - view.right;
        }
        if offset != before {
            self.offset = offset;
            self.clamp();
        }
        self.offset != before
    }
}

/// Bar tracks and thumbs for the panel's current offset and content.
pub(crate) fn bars_layout(view: &ScrollView) -> BarsLayout {
    let viewport = view.viewport();
    let area = view.content_rect();
    let style = &view.style;
    let thickness = style.thickness;
    let content = view.content_size();
    let offset = view.offset;
    let (vertical, horizontal) = view.bar_axes();
    let mut layout = BarsLayout::default();
    if vertical {
        // A shown bar stops where the other shown bar starts; an enabled axis
        // whose content fits keeps its full length.
        let track_area = if horizontal {
            Rect::new(
                viewport.left,
                viewport.top,
                viewport.right,
                viewport.bottom - thickness,
            )
        } else {
            viewport
        };
        let track = scrollbar::track_rect(track_area, true, style);
        if let Some(thumb) = scrollbar::thumb_rect(
            track,
            content.height,
            area.height(),
            offset.y,
            true,
            style,
        ) {
            layout.vertical = Some(Bar { track, thumb });
        }
    }
    if horizontal {
        let track_area = if vertical {
            Rect::new(
                viewport.left,
                viewport.top,
                viewport.right - thickness,
                viewport.bottom,
            )
        } else {
            viewport
        };
        let track = scrollbar::track_rect(track_area, false, style);
        if let Some(thumb) = scrollbar::thumb_rect(
            track,
            content.width,
            area.width(),
            offset.x,
            false,
            style,
        ) {
            layout.horizontal = Some(Bar { track, thumb });
        }
    }
    layout
}

/// The dead square where the two bars meet, when both are shown. It belongs to
/// neither track, so it is masked with the track color and treated as chrome
/// by hit-testing (content would otherwise show and click through it).
pub(crate) fn corner_rect(view: &ScrollView) -> Option<Rect> {
    let (vertical, horizontal) = view.bar_axes();
    if !(vertical && horizontal) {
        return None;
    }
    let thickness = view.style.thickness;
    Some(Rect::new(
        view.viewport().right - thickness,
        view.viewport().bottom - thickness,
        view.viewport().right,
        view.viewport().bottom,
    ))
}

/// One bar's geometry, if that axis currently overflows.
pub(crate) fn bar(view: &ScrollView, vertical: bool) -> Option<Bar> {
    let layout = bars_layout(view);
    if vertical {
        layout.vertical
    } else {
        layout.horizontal
    }
}

/// Paint one bar with the shared themed colors and the panel's hover/active
/// state.
pub(crate) fn paint_bar(
    painter: &Painter,
    canvas: &mut dyn Canvas,
    palette: &Palette,
    bar: Bar,
    state: &ScrollBarState,
    style: &ScrollBarStyle,
) {
    let colors = style.resolved_colors(palette);
    canvas.fill_rect(bar.track, colors.track);
    let color = if state.active {
        colors.thumb_active
    } else if state.hovered {
        colors.thumb_hover
    } else {
        colors.thumb
    };
    let radius = bar.thumb.width().min(bar.thumb.height()) / 2.0;
    painter.round_rect(canvas, bar.thumb, radius, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &'static str) -> WidgetId {
        WidgetId::new(name)
    }

    fn view() -> ScrollView {
        ScrollView::new(
            id("panel"),
            Rect::from_xywh(0.0, 0.0, 200.0, 100.0),
            ScrollBars::Vertical,
        )
        .child(id("a"), Rect::from_xywh(0.0, 0.0, 200.0, 40.0))
        .child(id("b"), Rect::from_xywh(0.0, 40.0, 200.0, 40.0))
        .child(id("c"), Rect::from_xywh(0.0, 80.0, 200.0, 40.0))
    }

    #[test]
    fn content_is_derived_and_offset_is_clamped() {
        let mut view = view();
        view.resolve();
        assert_eq!(view.content_size(), Size::new(200.0, 120.0));
        // The visible bar takes its thickness out of the content width.
        assert_eq!(view.max_offset(), Point::new(10.0, 20.0));
        view.offset = Point::new(40.0, 400.0);
        assert!(view.clamp());
        assert_eq!(view.offset, Point::new(10.0, 20.0));
        // Content that fits has a zero range and never moves.
        view.offset = Point::new(0.0, 0.0);
        assert!(!view.clamp());
    }

    #[test]
    fn explicit_content_extent_wins_when_larger() {
        let mut view = view().content(Size::new(400.0, 500.0));
        view.resolve();
        assert_eq!(view.content_size(), Size::new(400.0, 500.0));
        assert_eq!(view.max_offset(), Point::new(210.0, 400.0));
        // A smaller override never shrinks below the children.
        view.set_content_override(Some(Size::new(10.0, 10.0)));
        view.resolve();
        assert_eq!(view.content_size(), Size::new(200.0, 120.0));
    }

    #[test]
    fn reveal_scrolls_the_least_distance() {
        let mut view = view();
        view.resolve();
        // A child below the viewport scrolls just enough to show its bottom.
        assert!(view.reveal(Rect::from_xywh(0.0, 80.0, 100.0, 40.0)));
        assert_eq!(view.offset.y, 20.0);
        // Already visible: nothing moves.
        view.offset = Point::new(0.0, 10.0);
        assert!(!view.reveal(Rect::from_xywh(0.0, 20.0, 100.0, 40.0)));
        // A child above scrolls back up.
        assert!(view.reveal(Rect::from_xywh(0.0, -30.0, 100.0, 20.0)));
        assert_eq!(view.offset.y, 0.0);
    }

    #[test]
    fn both_axes_do_not_overlap_their_tracks() {
        let mut view = ScrollView::new(
            id("panel"),
            Rect::from_xywh(0.0, 0.0, 200.0, 100.0),
            ScrollBars::Both,
        )
        .children([ScrollChild::new(
            id("wide"),
            Rect::from_xywh(0.0, 0.0, 400.0, 300.0),
        )]);
        view.resolve();
        let bars = bars_layout(&view);
        let vertical = bars.vertical.expect("vertical bar");
        let horizontal = bars.horizontal.expect("horizontal bar");
        let thickness = view.style_ref().thickness;
        assert!((vertical.track.bottom - (100.0 - thickness)).abs() < 0.01);
        assert!((horizontal.track.right - (200.0 - thickness)).abs() < 0.01);
        assert!(!vertical.track.intersects(horizontal.track));
        // The vertical track starts at the top and the horizontal one at the
        // left edge, so both are reachable by the pointer.
        assert_eq!(vertical.track.top, 0.0);
        assert_eq!(horizontal.track.left, 0.0);
    }

    #[test]
    fn the_scroll_range_ends_at_the_padding_inside_the_bars() {
        // Padding 12 + a 10 DIP bar: at the end of the range the last row is
        // fully visible and keeps its 12 DIP margin from the bar's inner edge.
        let mut view = ScrollView::new(
            id("panel"),
            Rect::from_xywh(0.0, 0.0, 200.0, 100.0),
            ScrollBars::Both,
        )
        .padding(12.0)
        .child(id("body"), Rect::from_xywh(0.0, 0.0, 300.0, 200.0));
        view.resolve();
        view.offset = view.max_offset();
        let area = view.content_rect();
        let right = area.left + 300.0 - view.offset.x;
        let bottom = area.top + 200.0 - view.offset.y;
        assert_eq!(right, view.viewport.right - 10.0 - 12.0);
        assert_eq!(bottom, view.viewport.bottom - 10.0 - 12.0);
        // The bar's inner edge sits one padding past the content edge.
        let bars = bars_layout(&view);
        assert_eq!(bars.vertical.unwrap().track.left, right + 12.0);
    }

    #[test]
    fn fitted_axes_have_no_bar() {
        let mut view = ScrollView::new(
            id("panel"),
            Rect::from_xywh(0.0, 0.0, 200.0, 200.0),
            ScrollBars::Both,
        )
        .children([ScrollChild::new(
            id("body"),
            Rect::from_xywh(0.0, 0.0, 200.0, 120.0),
        )]);
        view.resolve();
        let bars = bars_layout(&view);
        assert!(bars.vertical.is_none());
        assert!(bars.horizontal.is_none());
        // A panel with no bar axis still scrolls programmatically.
        assert!(!ScrollBars::None.vertical());
    }

    #[test]
    fn padding_insets_the_content_area() {
        let mut view = ScrollView::new(
            id("panel"),
            Rect::from_xywh(0.0, 0.0, 200.0, 100.0),
            ScrollBars::Vertical,
        )
        .padding(8.0)
        .child(id("body"), Rect::from_xywh(0.0, 0.0, 200.0, 300.0));
        view.resolve();
        // The bar is outside the padded area: content keeps its 8 DIP margin
        // *and* stays clear of the 10 DIP track.
        assert_eq!(view.content_rect(), Rect::new(8.0, 8.0, 182.0, 92.0));
        // The scroll range uses the inset area, not the whole viewport.
        assert_eq!(view.max_offset(), Point::new(26.0, 300.0 - 84.0));
        // The bar keeps the full viewport height: it sits outside the margin.
        let bars = bars_layout(&view);
        let vertical = bars.vertical.expect("vertical bar");
        assert_eq!(
            vertical.track,
            scrollbar::track_rect(
                Rect::from_xywh(0.0, 0.0, 200.0, 100.0),
                true,
                view.style_ref()
            )
        );
        // Per-side padding is kept as given.
        let sided = view.padding_by(Insets {
            left: 4.0,
            top: 2.0,
            right: 12.0,
            bottom: 6.0,
        });
        // (The 10 DIP vertical bar is taken out of the right side as well.)
        assert_eq!(sided.content_rect(), Rect::new(4.0, 2.0, 178.0, 94.0));
    }

    #[test]
    fn the_showcase_panel_geometry_reaches_the_end() {
        // The demo panel: 250x214 viewport, padding 12, eight 300x30 rows.
        let mut view = ScrollView::new(
            id("panel"),
            Rect::from_xywh(792.0, 576.0, 250.0, 214.0),
            ScrollBars::Both,
        )
        .padding(12.0)
        .children((0..8).map(|index| {
            ScrollChild::new(
                id("row"),
                Rect::from_xywh(0.0, index as f32 * 34.0, 300.0, 30.0),
            )
        }));
        view.resolve();
        let area = view.content_rect();
        assert_eq!(area, Rect::new(804.0, 588.0, 792.0 + 250.0 - 10.0 - 12.0, 576.0 + 214.0 - 10.0 - 12.0));
        let max = view.max_offset();
        assert_eq!(max, Point::new(300.0 - area.width(), 268.0 - area.height()));
        view.set_offset(max);
        view.clamp();
        view.set_offset(max);
        // At the end of the range the last row's right edge is one padding in
        // from the vertical bar, and its bottom one padding above the
        // horizontal bar.
        let last = Rect::from_xywh(0.0, 238.0, 300.0, 30.0)
            .offset_into(area, Point::new(-view.offset_of().x, -view.offset_of().y));
        assert_eq!(last.right, area.right);
        assert_eq!(last.bottom, area.bottom);
        let bars = bars_layout(&view);
        let vertical = bars.vertical.unwrap();
        let horizontal = bars.horizontal.unwrap();
        assert_eq!(vertical.track.left, area.right + 12.0);
        assert_eq!(horizontal.track.top, area.bottom + 12.0);
        // The thumb at the end of the range reaches the end of its track.
        assert!((horizontal.thumb.right - horizontal.track.right).abs() < 0.01);
        assert!((vertical.thumb.bottom - vertical.track.bottom).abs() < 0.01);
        // At the start it sits flush with the other end (the travel is the
        // track minus the thumb length, which is what "a little more distance"
        // means for the content).
        view.set_offset(Point::new(0.0, 0.0));
        let bars = bars_layout(&view);
        assert!((bars.horizontal.unwrap().thumb.left - horizontal.track.left).abs() < 0.01);
    }

    #[test]
    fn only_a_shown_bar_shortens_its_neighbour_and_the_junction_is_masked() {
        // Both axes enabled, but only the content is tall: the vertical bar
        // keeps the full track length (no phantom horizontal bar) and there is
        // no junction to mask.
        let mut view = ScrollView::new(
            id("panel"),
            Rect::from_xywh(0.0, 0.0, 200.0, 100.0),
            ScrollBars::Both,
        )
        .child(id("body"), Rect::from_xywh(0.0, 0.0, 200.0, 300.0));
        view.resolve();
        let bars = bars_layout(&view);
        assert_eq!(
            bars.vertical.unwrap().track,
            Rect::from_xywh(190.0, 0.0, 10.0, 100.0)
        );
        assert!(bars.horizontal.is_none());
        assert!(corner_rect(&view).is_none());

        // Both overflow: each track stops at the other, and the 10x10 junction
        // square is masked (it belongs to neither track).
        let mut both = ScrollView::new(
            id("panel"),
            Rect::from_xywh(0.0, 0.0, 200.0, 100.0),
            ScrollBars::Both,
        )
        .child(id("body"), Rect::from_xywh(0.0, 0.0, 400.0, 300.0));
        both.resolve();
        let bars = bars_layout(&both);
        assert_eq!(bars.vertical.unwrap().track.bottom, 90.0);
        assert_eq!(bars.horizontal.unwrap().track.right, 190.0);
        assert_eq!(corner_rect(&both), Some(Rect::new(190.0, 90.0, 200.0, 100.0)));
    }

    #[test]
    fn forgetting_a_child_shrinks_the_content() {
        let mut view = view();
        view.forget(id("c"));
        view.resolve();
        assert_eq!(view.content_size(), Size::new(200.0, 100.0));
        assert!(!view.contains(id("c")));
    }
}
