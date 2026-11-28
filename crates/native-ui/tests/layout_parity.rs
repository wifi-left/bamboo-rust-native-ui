// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! M7.6 parity: one `.buixml` document applied through the dynamic parser and
//! through generated Rust must produce the same layout, properties, content and
//! accessibility tree (D128).
//!
//! The generated module is checked in (`fixtures/parity_generated.rs`, produced
//! by `bamboo-native-ui-layout-codegen`); the codegen crate's `golden` test
//! fails when the file is stale.

#![cfg(all(feature = "layout-xml", feature = "layout-compiled"))]

include!("fixtures/parity_generated.rs");

use bamboo_native_ui::geometry::{Rect, Size};
use bamboo_native_ui::layout_xml::{AppliedLayout, EntrySpace, LayoutDocument};
use bamboo_native_ui::widget::ScrollBars;
use bamboo_native_ui::widgets::{Button, Divider, Label, ListView, Slider};
use bamboo_native_ui::{
    A11yRole, Invalidator, Palette, ScrollChild, ScrollView, ThemeSettings, Ui, UiHost, WidgetId,
};

struct TestHost {
    invalidated: Vec<Rect>,
}

impl Default for TestHost {
    fn default() -> Self {
        Self {
            invalidated: Vec::new(),
        }
    }
}

impl Invalidator for TestHost {
    fn invalidate(&mut self, rect: Rect) {
        self.invalidated.push(rect);
    }
}

impl UiHost for TestHost {}

fn page() -> Ui<&'static str> {
    let mut ui: Ui<&'static str> = Ui::new();
    ui.add(Button::new("action", "原操作", "clicked"));
    ui.add(Label::new("title", "原标题"));
    ui.add(Slider::new("slider", 0.0, 0.0..=1.0, |_| "changed"));
    ui.add(Divider::new("rule"));
    ui.add(ListView::new("list"));
    ui.add(Button::new("only_wide", "只宽", "wide"));
    for id in ["action", "title", "slider", "rule", "list", "only_wide"] {
        ui.set_rect(WidgetId::new(id), Rect::from_xywh(0.0, 0.0, 40.0, 20.0));
    }
    let mut host = TestHost::default();
    ui.set_scroll_view(
        ScrollView::new(
            WidgetId::new("page"),
            Rect::from_xywh(0.0, 0.0, 300.0, 200.0),
            ScrollBars::Vertical,
        )
        .children([ScrollChild::new(
            WidgetId::new("title"),
            Rect::from_xywh(0.0, 0.0, 50.0, 20.0),
        )]),
        &mut host,
    );
    ui
}

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default(), false)
}

#[derive(Debug, PartialEq)]
struct Node {
    id: String,
    role: A11yRole,
    name: String,
    value: Option<String>,
    description: Option<String>,
    rect: Rect,
    enabled: bool,
    focusable: bool,
    read_only: bool,
}

#[derive(Debug, PartialEq)]
struct Snapshot {
    generation: u64,
    variant: String,
    entries: Vec<(String, Rect, EntrySpace)>,
    content_writes: usize,
    skin_writes: usize,
    warnings: Vec<(&'static str, usize, String)>,
    nodes: Vec<Node>,
    title_text: String,
    slider_value: f32,
    rows: Vec<String>,
}

fn snapshot(ui: &Ui<&'static str>) -> Snapshot {
    let installed: &AppliedLayout = ui.installed_layout().expect("a layout is installed");
    let nodes = ui
        .a11y_nodes()
        .into_iter()
        .map(|node| Node {
            id: node.id.name().to_string(),
            role: node.role,
            name: node.name,
            value: node.value,
            description: node.description,
            rect: node.rect,
            enabled: node.enabled,
            focusable: node.focusable,
            read_only: node.read_only,
        })
        .collect();
    Snapshot {
        generation: installed.generation(),
        variant: installed.variant_id().to_string(),
        entries: installed
            .entries()
            .iter()
            .map(|entry| (entry.id.name().to_string(), entry.rect, entry.space))
            .collect(),
        content_writes: installed.content_writes(),
        skin_writes: installed.skin_writes(),
        warnings: installed
            .warnings()
            .iter()
            .map(|warning| (warning.code, warning.line, warning.path.clone()))
            .collect(),
        nodes,
        title_text: ui
            .with_widget::<Label, _>(WidgetId::new("title"), |label| label.text().to_string())
            .unwrap(),
        slider_value: ui
            .with_widget::<Slider<&'static str>, _>(WidgetId::new("slider"), |slider| {
                slider.value()
            })
            .unwrap(),
        rows: ui
            .with_widget::<ListView<&'static str>, _>(WidgetId::new("list"), |list| {
                list.rows()
                    .iter()
                    .map(|row| row.title.clone())
                    .collect()
            })
            .unwrap(),
    }
}

/// Everything except the write counters, which are per generation: a re-apply
/// at the same generation legitimately reports zero even though the applied
/// state is identical.
fn same_layout(left: &Snapshot, right: &Snapshot) -> bool {
    left.generation == right.generation
        && left.variant == right.variant
        && left.entries == right.entries
        && left.warnings == right.warnings
        && left.nodes == right.nodes
        && left.title_text == right.title_text
        && left.slider_value == right.slider_value
        && left.rows == right.rows
}

#[test]
fn compiled_and_dynamic_produce_the_same_result() {
    let document = LayoutDocument::parse(include_str!("fixtures/parity.buixml")).unwrap();
    let palette = palette();
    let client = Size::new(800.0, 600.0);

    let mut dynamic_ui = page();
    let mut dynamic_host = TestHost::default();
    let mut compiled_ui = page();
    let mut compiled_host = TestHost::default();

    let dynamic_report = dynamic_ui
        .apply_layout(&document, &palette, client, &mut dynamic_host)
        .expect("dynamic apply");
    let compiled_report = compiled_ui
        .apply_compiled(&parity::LAYOUT, &palette, client, &mut compiled_host)
        .expect("compiled apply");

    assert_eq!(dynamic_report.variant_id, "wide");
    assert_eq!(compiled_report.variant_id, dynamic_report.variant_id);
    assert_eq!(compiled_report.generation, dynamic_report.generation);
    assert_eq!(
        compiled_report.applied_entries,
        dynamic_report.applied_entries
    );
    assert_eq!(compiled_report.content_writes, dynamic_report.content_writes);
    assert_eq!(compiled_report.skin_writes, dynamic_report.skin_writes);
    assert!(same_layout(&snapshot(&compiled_ui), &snapshot(&dynamic_ui)));

    // The wide variant hides `only_wide`; the base variant restores the
    // application's visibility (a claim, not a declaration).
    let wide = snapshot(&dynamic_ui);
    assert!(
        !wide.nodes.iter().any(|node| node.id == "only_wide"),
        "a hidden widget leaves the a11y tree"
    );

    let client = Size::new(400.0, 300.0);
    dynamic_ui
        .apply_layout(&document, &palette, client, &mut dynamic_host)
        .expect("dynamic base apply");
    compiled_ui
        .apply_compiled(&parity::LAYOUT, &palette, client, &mut compiled_host)
        .expect("compiled base apply");
    let base = snapshot(&dynamic_ui);
    assert_eq!(base.variant, "base");
    assert!(
        base.nodes.iter().any(|node| node.id == "only_wide"),
        "the base variant restores the widget's own visibility"
    );
    assert!(same_layout(&snapshot(&compiled_ui), &base));

    // A generation that is already installed writes no content again, and a
    // breakpoint round trip restores every claimed field the same way.
    dynamic_ui
        .apply_layout(&document, &palette, Size::new(800.0, 600.0), &mut dynamic_host)
        .expect("dynamic re-apply");
    compiled_ui
        .apply_compiled(&parity::LAYOUT, &palette, Size::new(800.0, 600.0), &mut compiled_host)
        .expect("compiled re-apply");
    let again = snapshot(&dynamic_ui);
    assert_eq!(again.content_writes, 0);
    assert_eq!(again.skin_writes, 0);
    assert!(same_layout(&again, &wide));
    assert!(same_layout(&snapshot(&compiled_ui), &again));
}

#[test]
fn a_missing_target_rejects_atomically_in_both_modes() {
    let document = LayoutDocument::parse(include_str!("fixtures/parity.buixml")).unwrap();
    let palette = palette();
    let mut ui: Ui<&'static str> = Ui::new();
    ui.add(Button::new("action", "原操作", "clicked"));
    let mut host = TestHost::default();

    let dynamic = ui
        .apply_layout(&document, &palette, Size::new(800.0, 600.0), &mut host)
        .expect_err("missing ids must reject");
    let compiled = ui
        .apply_compiled(&parity::LAYOUT, &palette, Size::new(800.0, 600.0), &mut host)
        .expect_err("missing ids must reject");

    assert!(ui.installed_layout().is_none());
    let mut dynamic_codes: Vec<&'static str> = dynamic.diagnostics.iter().map(|item| item.code).collect();
    let mut compiled_codes: Vec<&'static str> = compiled.diagnostics.iter().map(|item| item.code).collect();
    dynamic_codes.sort();
    compiled_codes.sort();
    dynamic_codes.dedup();
    compiled_codes.dedup();
    assert_eq!(compiled_codes, dynamic_codes);
    assert!(!dynamic_codes.is_empty());
}
