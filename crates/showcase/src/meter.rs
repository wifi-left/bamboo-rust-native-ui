// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! A third-party widget for the `布局` page: the M7 widget-declared skin
//! protocol from outside the library crate.
//!
//! It declares two local scalar attributes and one data child element:
//!
//! ```xml
//! <widget id="m7_meter" x="18" y="444" w="cw-36" h="76"
//!         x-level="72" x-units="dB">
//!   <x-item label="低频" weight="3"/>
//!   <x-item label="中频" weight="2"/>
//! </widget>
//! ```
//!
//! The library knows none of these names: `validate_*` checks them before the
//! applier mutates anything, and `apply_*` writes the data. Everything here is
//! the same six trait methods any application widget would implement.

use bamboo_native_ui::{
    A11yInfo, A11yRole, Rect, SkinAttrDecl, SkinElement, SkinValue, SkinValueKind, TextAlign,
    TextStyleRole, Widget, WidgetId, WidgetInput, WidgetPaint, WidgetState,
};

const ATTRS: &[SkinAttrDecl] = &[
    SkinAttrDecl::new("x-level", SkinValueKind::Number),
    SkinAttrDecl::new("x-units", SkinValueKind::Text),
];

const CHILDREN: &[&str] = &["x-item"];

/// One data row of the meter.
#[derive(Clone, Debug, PartialEq)]
pub struct MeterItem {
    pub label: String,
    pub weight: f64,
}

/// A horizontal level bar over a row of labelled segments.
pub struct MeterWidget {
    id: WidgetId,
    rect: Rect,
    level: f64,
    units: String,
    items: Vec<MeterItem>,
    /// How many times the local skin data was written, for the self-test.
    writes: u32,
}

impl MeterWidget {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: WidgetId::new(id),
            rect: Rect::from_xywh(0.0, 0.0, 240.0, 76.0),
            level: 0.0,
            units: String::new(),
            items: Vec::new(),
            writes: 0,
        }
    }

    pub fn level(&self) -> f64 {
        self.level
    }

    pub fn units(&self) -> &str {
        &self.units
    }

    pub fn items(&self) -> &[MeterItem] {
        &self.items
    }

    /// How many times a layout document wrote this widget's local data.
    pub fn writes(&self) -> u32 {
        self.writes
    }
}

impl<M> Widget<M> for MeterWidget {
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
        76.0
    }

    fn focusable(&self) -> bool {
        false
    }

    fn paint(&self, ctx: &mut WidgetPaint<'_>, _state: &WidgetState) {
        let palette = ctx.palette;
        let rect = self.rect;
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        ctx.painter
            .round_rect(&mut *ctx.canvas, rect, 10.0, palette.card);
        let pad = 12.0;
        let inner = rect.inset(pad, pad);
        if inner.width() <= 0.0 || inner.height() <= 0.0 {
            return;
        }

        // Three bands sized from the available height: a caption, an 8 DIP
        // track and a label row. The row keeps at least ~20 DIPs at the
        // demo's 76 DIP height, so the labels are never clipped at the
        // bottom (the bug the user saw).
        let caption_h = (inner.height() * 0.30).clamp(14.0, 20.0);
        let gap = (inner.height() * 0.08).clamp(2.0, 5.0);
        let track_h = 8.0;
        let track_top = inner.top + caption_h + gap;
        let mut label_top = track_top + track_h + gap;
        // A very short widget still shows a readable label row.
        label_top = label_top.min(inner.bottom - 14.0).max(track_top + track_h);

        let caption = if self.units.is_empty() {
            "信号表".to_string()
        } else {
            format!("信号表 · {:.0} {}", self.level, self.units)
        };
        ctx.text(
            Rect::new(inner.left, inner.top, inner.right, inner.top + caption_h),
            &caption,
            TextStyleRole::Body,
            TextAlign::Left,
            palette.text,
        );

        let track = Rect::new(
            inner.left,
            track_top,
            inner.right,
            track_top + track_h,
        );
        ctx.painter
            .round_rect(&mut *ctx.canvas, track, 4.0, palette.scroll_track);
        let fraction = (self.level / 100.0).clamp(0.0, 1.0) as f32;
        if fraction > 0.0 {
            let filled = Rect::new(
                track.left,
                track.top,
                track.left + track.width() * fraction,
                track.bottom,
            );
            ctx.painter
                .round_rect(&mut *ctx.canvas, filled, 4.0, palette.accent);
        }

        let total: f64 = self.items.iter().map(|item| item.weight.max(0.1)).sum();
        if total <= 0.0 || inner.bottom - label_top <= 2.0 {
            return;
        }
        let mut left = inner.left;
        let row = Rect::new(inner.left, label_top, inner.right, inner.bottom);
        for item in &self.items {
            let share = (item.weight.max(0.1) / total) as f32;
            let right = (left + inner.width() * share).min(inner.right);
            let chip = Rect::new(left, row.top, right, row.bottom);
            ctx.painter.round_rect(
                &mut *ctx.canvas,
                chip.inset(1.0, 1.0),
                6.0,
                palette.button_bg,
            );
            // Vertical padding keeps the label off the chip edges; it scales
            // with the row so a short widget stays legible too.
            let vpad = (chip.height() * 0.18).clamp(2.0, 6.0);
            ctx.text(
                chip.inset(2.0, vpad),
                &item.label,
                TextStyleRole::Small,
                TextAlign::Center,
                palette.muted,
            );
            left = right + 4.0;
        }
    }

    fn input(&mut self, _input: WidgetInput) -> Option<M> {
        None
    }

    fn skin_attrs(&self) -> &'static [SkinAttrDecl] {
        ATTRS
    }

    fn validate_skin_attr(&self, name: &str, value: &SkinValue<'_>) -> bool {
        match (name, value) {
            ("x-level", SkinValue::Number(level)) => (0.0..=100.0).contains(level),
            ("x-units", SkinValue::Text(units)) => !units.trim().is_empty(),
            _ => true,
        }
    }

    fn apply_skin_attr(&mut self, name: &str, value: &SkinValue<'_>) {
        match (name, value) {
            ("x-level", SkinValue::Number(level)) => {
                self.level = *level;
                self.writes += 1;
            }
            ("x-units", SkinValue::Text(units)) => {
                self.units = units.to_string();
                self.writes += 1;
            }
            _ => {}
        }
    }

    fn skin_children(&self) -> &'static [&'static str] {
        CHILDREN
    }

    fn validate_skin_children(&self, children: &[SkinElement]) -> bool {
        children.iter().all(|child| {
            child.name == "x-item"
                && child
                    .attr("label")
                    .map(|label| !label.trim().is_empty())
                    .unwrap_or(false)
                && child.attr("weight").map_or(true, |weight| {
                    weight.trim().parse::<f64>().is_ok_and(|value| value > 0.0)
                })
        })
    }

    fn apply_skin_children(&mut self, children: &[SkinElement]) {
        self.items = children
            .iter()
            .filter_map(|child| {
                Some(MeterItem {
                    label: child.attr("label")?.to_string(),
                    weight: child
                        .attr("weight")
                        .and_then(|weight| weight.trim().parse::<f64>().ok())
                        .unwrap_or(1.0),
                })
            })
            .collect();
        self.writes += 1;
    }

    fn a11y(&self) -> Option<A11yInfo> {
        let mut info = A11yInfo::new(A11yRole::ProgressBar, "信号表")
            .range(0.0, 100.0, self.level)
            .display_only();
        if !self.units.is_empty() {
            info = info.value(format!("{:.0} {}", self.level, self.units));
        }
        Some(info)
    }
}
