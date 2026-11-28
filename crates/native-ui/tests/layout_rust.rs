// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! M7.6 inline Rust (`rust:` values): the compiled mode splices the text so
//! `rustc` type-checks it; the dynamic mode refuses the same file.
//!
//! The generated module is `fixtures/rust_generated.rs` (module `rust_layout`);
//! the codegen crate's golden test keeps it current.

#![cfg(feature = "layout-compiled")]

use bamboo_native_ui::geometry::{Rect, Size};
use bamboo_native_ui::layout_xml::RadiusAttr;
use bamboo_native_ui::widgets::{Button, Label};
use bamboo_native_ui::{
    Corners, Invalidator, Palette, ThemeSettings, Ui, UiHost, Widget, WidgetId,
};

include!("fixtures/rust_generated.rs");

// The values the document splices in. They live in the including module; the
// generated module glob-imports it (`use super::*`).
const PAGE_PAD: f64 = 12.0;
const TITLE: &str = "动态标题";
const SHOW_ACTION: bool = true;
const LOCKED: bool = false;
const RADIUS: RadiusAttr = RadiusAttr::Medium;

struct TestHost;

impl Invalidator for TestHost {
    fn invalidate(&mut self, _rect: Rect) {}
}

impl UiHost for TestHost {}

fn page() -> Ui<&'static str> {
    let mut ui: Ui<&'static str> = Ui::new();
    ui.add(Button::new("action", "原操作", "clicked"));
    ui.add(Label::new("title", "原标题"));
    for id in ["action", "title"] {
        ui.set_rect(WidgetId::new(id), Rect::from_xywh(0.0, 0.0, 40.0, 20.0));
    }
    ui
}

#[test]
fn inline_rust_values_are_spliced_into_the_compiled_module() {
    let mut ui = page();
    let mut host = TestHost;
    let palette = Palette::new(&ThemeSettings::default(), false);
    let report = ui
        .apply_compiled(&rust_layout::LAYOUT, &palette, Size::new(400.0, 300.0), &mut host)
        .expect("the generated module applies");

    assert_eq!(report.variant_id, "base");
    let installed = ui.installed_layout().expect("installed");
    assert_eq!(
        installed.rect(WidgetId::new("title")),
        Some(Rect::from_xywh(12.0, 12.0, 376.0, 24.0)),
        "x/y read PAGE_PAD and w reads cw in the generated f64 expression"
    );
    assert_eq!(
        ui.with_widget::<Label, _>(WidgetId::new("title"), |label| label.text().to_string()),
        Some(TITLE.to_string()),
        "the content value is the spliced constant"
    );
    assert_eq!(
        ui.with_widget::<Button<&'static str>, _>(WidgetId::new("action"), |button| {
            button.surface_style().map(|style| style.corners)
        }),
        Some(Some(Corners::uniform(6.0))),
        "the radius reads the spliced RadiusAttr"
    );
    // `visible="rust:SHOW_ACTION"` keeps the widget in the tree; `enabled` is
    // `!LOCKED`.
    let action = ui
        .a11y_nodes()
        .into_iter()
        .find(|node| node.id == WidgetId::new("action"))
        .expect("the visible button is published");
    assert!(action.enabled);
}

#[cfg(feature = "layout-xml")]
#[test]
fn the_dynamic_mode_refuses_inline_rust_atomically() {
    use bamboo_native_ui::layout_xml::LayoutDocument;

    let document = LayoutDocument::parse(include_str!("fixtures/rust.buixml")).expect("parses");
    let mut ui = page();
    let mut host = TestHost;
    let palette = Palette::new(&ThemeSettings::default(), false);
    let failure = ui
        .apply_layout(&document, &palette, Size::new(400.0, 300.0), &mut host)
        .expect_err("a runtime file is data, never code");
    assert!(
        failure
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "xml.rust.unsupported"),
        "every rejection names the inline-Rust rule: {:?}",
        failure.diagnostics
    );
    assert!(!failure.diagnostics.is_empty());
    assert!(ui.installed_layout().is_none());
    assert_eq!(
        ui.with_widget::<Label, _>(WidgetId::new("title"), |label| label.text().to_string()),
        Some("原标题".to_string())
    );
}
