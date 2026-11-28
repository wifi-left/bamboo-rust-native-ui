// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Read-only debugging snapshots (`debug-form`).
//!
//! The library exposes state; how it is shown is the application's decision.
//! The showcase drives these APIs from its `调试` menu — there is no painted
//! inspector inside the library. Everything here is compiled only with the
//! `debug-form` feature (which implies `layout-xml`), so the default build and
//! its binaries stay free of debug code.
//!
//! The snapshot is a value: [`crate::Ui::debug_snapshot`] walks the widget
//! tree once, resolves the effective state (visibility, focus, read-only,
//! narration) and returns owned data. Nothing is mutated.

use std::collections::{HashMap, VecDeque};

use crate::a11y::A11yInfo;
use crate::geometry::{Point, Rect, Size};
use crate::layout_xml::EntrySpace;
use crate::skin::SkinValueKind;
use crate::widget::WidgetId;

/// One resolved layout entry: the id, the document rectangle and the space the
/// rectangle lives in (client or a scroll panel's content).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DebugEntry {
    pub id: WidgetId,
    pub rect: Rect,
    pub space: EntrySpace,
}

/// One document-owned id and the fields the document claimed, with the
/// baseline rectangle recorded when the generation was installed.
#[derive(Clone, Debug, PartialEq)]
pub struct DebugClaim {
    pub id: WidgetId,
    /// Rectangle at install time (claims never restore rectangles).
    pub baseline: Rect,
    /// Claimed field names: `visible`, `enabled`, `focusable`, `focus-order`,
    /// `cursor`, `props`, `corners`, `background`.
    pub fields: Vec<&'static str>,
}

/// One scroll panel as registered with the container.
#[derive(Clone, Debug, PartialEq)]
pub struct DebugScroll {
    pub id: WidgetId,
    pub parent: Option<WidgetId>,
    pub viewport: Rect,
    pub offset: Point,
    pub content: Size,
}

/// The installed layout document (`Ui::installed_layout`), snapshot form.
#[derive(Clone, Debug, PartialEq)]
pub struct DebugLayout {
    pub generation: u64,
    pub variant: String,
    pub client: Size,
    pub content_writes: usize,
    pub skin_writes: usize,
    pub entries: Vec<DebugEntry>,
    pub claims: Vec<DebugClaim>,
    pub warnings: Vec<String>,
    pub scrolls: Vec<DebugScroll>,
}

/// One widget: effective state plus the vocabulary it publishes to the skin.
#[derive(Clone, Debug)]
pub struct DebugWidget {
    pub id: WidgetId,
    pub rect: Rect,
    /// Effective visibility (the container override applied).
    pub visible: bool,
    pub enabled: bool,
    /// Effective Tab eligibility (props override applied).
    pub focusable: bool,
    /// Effective read-only state (widget and props combined).
    pub read_only: bool,
    pub focused: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub overlay: bool,
    /// The scroll panel that owns the widget, when it is scrolled.
    pub scroll_parent: Option<WidgetId>,
    pub automation_id: Option<String>,
    /// What the widget publishes to accessibility, with the application's
    /// narration resolved into the name/description and child names.
    pub a11y: Option<A11yInfo>,
    /// Declared local attribute vocabulary (`skin_attrs`).
    pub skin_attrs: Vec<(&'static str, SkinValueKind)>,
    /// Declared data child element names (`skin_children`).
    pub skin_children: Vec<&'static str>,
}

/// One widget's cumulative paint cost since the process started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WidgetPaintStat {
    pub id: WidgetId,
    pub paints: u64,
    pub micros: u64,
}

/// Frame and per-widget paint measurements. Recording runs only with
/// `debug-form`; the totals are cumulative, the `last_*` fields describe the
/// most recent frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaintStats {
    pub frames: u64,
    pub widget_paints: u64,
    pub last_frame_widgets: u64,
    pub last_frame_micros: u64,
    /// Cumulative per-widget costs, longest first.
    pub per_widget: Vec<WidgetPaintStat>,
}

/// One entry of the bounded diagnostics ring (layout applies, warnings,
/// failures and application notes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugEvent {
    /// Monotonic sequence number (starts at 1).
    pub sequence: u64,
    /// A short category: `layout`, `warning`, `error`, `note`.
    pub kind: &'static str,
    pub text: String,
}

/// Everything the debug API returns in one value.
#[derive(Clone, Debug)]
pub struct DebugSnapshot {
    pub layout: Option<DebugLayout>,
    pub widgets: Vec<DebugWidget>,
    /// Registered widget type names, in registration order (M7.7). Empty when
    /// no registry is installed.
    pub widget_types: Vec<&'static str>,
    /// Whether the installed registry carries a command table.
    pub has_commands: bool,
    pub focus: Option<WidgetId>,
    pub hover: Option<WidgetId>,
    pub press: Option<WidgetId>,
    /// Damage waiting for the next `flush_damage`.
    pub pending_damage: Vec<Rect>,
    /// The pieces the last `flush_damage` handed to the host.
    pub last_damage: Vec<Rect>,
    pub paint: PaintStats,
    /// The diagnostics ring, oldest first.
    pub log: Vec<DebugEvent>,
}

/// Bounded ring of diagnostics. Capacity is small on purpose: the point is a
/// recent window an application can show in a menu or a dialog.
#[derive(Default)]
pub(crate) struct DebugLog {
    next: u64,
    events: VecDeque<DebugEvent>,
}

impl DebugLog {
    pub(crate) const CAPACITY: usize = 128;

    pub(crate) fn push(&mut self, kind: &'static str, text: impl Into<String>) {
        self.next += 1;
        self.events.push_back(DebugEvent {
            sequence: self.next,
            kind,
            text: text.into(),
        });
        while self.events.len() > Self::CAPACITY {
            self.events.pop_front();
        }
    }

    pub(crate) fn events(&self) -> impl Iterator<Item = &DebugEvent> {
        self.events.iter()
    }

    pub(crate) fn clear(&mut self) {
        self.events.clear();
    }
}

/// Internal accumulator behind [`PaintStats`].
#[derive(Default)]
pub(crate) struct PaintRecorder {
    frames: u64,
    widget_paints: u64,
    last_frame_widgets: u64,
    last_frame_micros: u64,
    per_widget: HashMap<WidgetId, (u64, u64)>,
}

impl PaintRecorder {
    pub(crate) fn end_frame(&mut self, frame_micros: u64, widgets: &[(WidgetId, u64)]) {
        self.frames += 1;
        self.last_frame_micros = frame_micros;
        self.last_frame_widgets = widgets.len() as u64;
        for &(id, micros) in widgets {
            self.widget_paints += 1;
            let stat = self.per_widget.entry(id).or_insert((0, 0));
            stat.0 += 1;
            stat.1 += micros;
        }
    }

    pub(crate) fn snapshot(&self) -> PaintStats {
        let mut per_widget: Vec<WidgetPaintStat> = self
            .per_widget
            .iter()
            .map(|(&id, &(paints, micros))| WidgetPaintStat {
                id,
                paints,
                micros,
            })
            .collect();
        per_widget.sort_by(|left, right| {
            right
                .micros
                .cmp(&left.micros)
                .then_with(|| left.id.0.cmp(right.id.0))
        });
        PaintStats {
            frames: self.frames,
            widget_paints: self.widget_paints,
            last_frame_widgets: self.last_frame_widgets,
            last_frame_micros: self.last_frame_micros,
            per_widget,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::layout_xml::LayoutDocument;
    use crate::theme::ThemeSettings;
    use crate::ui::{Invalidator, Ui, UiHost};
    use crate::widgets::{Button, ListView};

    #[derive(Default)]
    struct TestHost {
        invalidated: Vec<Rect>,
        editing: Option<WidgetId>,
    }

    impl Invalidator for TestHost {
        fn invalidate(&mut self, rect: Rect) {
            self.invalidated.push(rect);
        }
    }

    impl UiHost for TestHost {
        fn editing_id(&self) -> Option<WidgetId> {
            self.editing
        }
    }

    fn palette() -> crate::theme::Palette {
        crate::theme::Palette::new(&ThemeSettings::default(), true)
    }

    #[test]
    fn the_snapshot_reports_layout_widgets_damage_and_the_log() {
        let mut ui: Ui<&'static str> = Ui::new();
        ui.add(Button::new("action", "原操作", "clicked"));
        ui.add(ListView::new("list"));
        let mut host = TestHost::default();
        let document = LayoutDocument::parse(
            r##"<layouts version="1"><layout id="base">
  <widget id="action" x="10" y="20" w="100" h="30" label="新操作" visible="true"/>
  <widget id="list" x="10" y="60" w="200" h="120" selected="1">
    <item title="夜曲"/><item title="晴天"/>
  </widget>
</layout></layouts>"##,
        )
        .expect("valid");
        let report = ui
            .apply_layout(
                &document,
                &palette(),
                crate::geometry::Size::new(800.0, 600.0),
                &mut host,
            )
            .expect("applies");
        assert!(report.warnings.is_empty());

        let snapshot = ui.debug_snapshot();
        let layout = snapshot.layout.expect("installed");
        assert!(snapshot.widget_types.is_empty(), "no registry installed");
        assert!(!snapshot.has_commands);
        assert_eq!(layout.variant, "base");
        assert_eq!(layout.client, crate::geometry::Size::new(800.0, 600.0));
        assert_eq!(layout.entries.len(), 2);
        assert_eq!(layout.entries[0].id, WidgetId::new("action"));
        assert_eq!(
            layout.entries[0].rect,
            Rect::from_xywh(10.0, 20.0, 100.0, 30.0)
        );
        let claim = layout
            .claims
            .iter()
            .find(|claim| claim.id == WidgetId::new("action"))
            .expect("claim");
        assert!(claim.fields.contains(&"visible"));

        let action = snapshot
            .widgets
            .iter()
            .find(|widget| widget.id == WidgetId::new("action"))
            .expect("action");
        assert_eq!(
            action.a11y.as_ref().map(|info| info.name.as_str()),
            Some("新操作")
        );
        assert!(action.visible && action.focusable && !action.read_only);
        let list = snapshot
            .widgets
            .iter()
            .find(|widget| widget.id == WidgetId::new("list"))
            .expect("list");
        assert_eq!(list.skin_children, ["item"]);
        assert!(list.a11y.as_ref().expect("a11y").children.len() >= 2);

        // The ring recorded the apply; application notes append.
        assert!(snapshot
            .log
            .iter()
            .any(|event| event.kind == "layout" && event.text.contains("variant=base")));
        ui.debug_event("note", "hello");
        let snapshot = ui.debug_snapshot();
        assert_eq!(
            snapshot.log.last().map(|event| event.text.as_str()),
            Some("hello")
        );

        // Damage: a silent set is pending until the flush, then it is the last
        // flushed batch.
        ui.set_label(WidgetId::new("action"), "已改".to_string());
        assert!(!ui.debug_snapshot().pending_damage.is_empty());
        assert!(ui.flush_damage(&mut host));
        let snapshot = ui.debug_snapshot();
        assert!(snapshot.pending_damage.is_empty());
        assert!(!snapshot.last_damage.is_empty());

        // Focus is reported by id.
        assert!(ui.set_focus(WidgetId::new("action"), &mut host));
        assert_eq!(ui.debug_snapshot().focus, Some(WidgetId::new("action")));

        // A widget registry is listed by its type names, in registration order.
        let mut registry = crate::creation::WidgetRegistry::<&'static str>::standard();
        registry.set_commands(|command| (command == 1).then_some("one"));
        ui.set_widget_registry(registry);
        let snapshot = ui.debug_snapshot();
        assert_eq!(snapshot.widget_types, crate::creation::STANDARD_TYPES);
        assert!(snapshot.has_commands);
    }

    #[test]
    fn the_ring_is_bounded_and_paint_stats_sort_by_cost() {
        let mut log = DebugLog::default();
        for index in 0..(DebugLog::CAPACITY + 10) {
            log.push("note", format!("event {index}"));
        }
        assert_eq!(log.events().count(), DebugLog::CAPACITY);
        assert_eq!(log.events().next().map(|event| event.sequence), Some(11));

        let mut recorder = PaintRecorder::default();
        recorder.end_frame(
            900,
            &[(WidgetId::new("a"), 100), (WidgetId::new("b"), 500)],
        );
        recorder.end_frame(400, &[(WidgetId::new("a"), 300)]);
        let stats = recorder.snapshot();
        assert_eq!(stats.frames, 2);
        assert_eq!(stats.last_frame_micros, 400);
        assert_eq!(stats.last_frame_widgets, 1);
        assert_eq!(stats.widget_paints, 3);
        assert_eq!(
            stats.per_widget,
            [
                WidgetPaintStat {
                    id: WidgetId::new("b"),
                    paints: 1,
                    micros: 500
                },
                WidgetPaintStat {
                    id: WidgetId::new("a"),
                    paints: 2,
                    micros: 400
                }
            ]
        );
    }
}
