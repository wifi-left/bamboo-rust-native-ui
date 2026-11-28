// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! M7.6 inline Rust with a layout context (`context = Type`) and `<rust>`
//! child-list bindings (§16.2).
//!
//! The generated module is `fixtures/context_generated.rs` (module
//! `context_layout`, context `AppLayout`); the codegen crate's golden test
//! keeps it current. The same file is what the dynamic mode refuses.

#![cfg(feature = "layout-compiled")]

use bamboo_native_ui::geometry::{Rect, Size};
use bamboo_native_ui::layout_xml::RadiusAttr;
use bamboo_native_ui::skin::SkinElement;
use bamboo_native_ui::widgets::{Button, Label, ListView};
use bamboo_native_ui::{
    Corners, Invalidator, Palette, ThemeSettings, Ui, UiHost, Widget, WidgetId, WidgetProps,
};

include!("fixtures/context_generated.rs");
include!("fixtures/binding_generated.rs");

/// The `<rust target="list" code="static_rows()"/>` binding of the context-free
/// fixture: evaluated once into a static, not per apply.
fn static_rows() -> Vec<SkinElement> {
    let mut row = SkinElement::new("item", 1, 1);
    row.push_attr("title", "静态绑定");
    vec![row]
}

/// The layout context the generated `install` reads. It lives in the including
/// module, so the module's `use super::*` resolves it by name.
pub struct AppLayout {
    pad: f64,
    title: &'static str,
    show_action: bool,
    radius: RadiusAttr,
}

impl AppLayout {
    /// The `<rust target="list" code="context.rows()"/>` binding: a real
    /// widget's child list built from application data.
    fn rows(&self) -> Vec<SkinElement> {
        let mut first = SkinElement::new("item", 1, 1);
        first.push_attr("title", "第一行");
        first.push_attr("subtitle", self.title);
        let mut second = SkinElement::new("item", 2, 1);
        second.push_attr("title", "第二行");
        vec![first, second]
    }
}

struct TestHost;

impl Invalidator for TestHost {
    fn invalidate(&mut self, _rect: Rect) {}
}

impl UiHost for TestHost {}

fn page() -> Ui<&'static str> {
    let mut ui: Ui<&'static str> = Ui::new();
    ui.add(Button::new("action", "原操作", "clicked"));
    // Exposed so the XML `name` (the context title) is published in the tree.
    ui.add_with(
        Label::new("title", "原标题"),
        WidgetProps::default().expose(true).name("原标题"),
    );
    ui.add(ListView::new("list"));
    for id in ["action", "title", "list"] {
        ui.set_rect(WidgetId::new(id), Rect::from_xywh(0.0, 0.0, 40.0, 20.0));
    }
    ui
}

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default(), false)
}

#[test]
fn the_context_reaches_geometry_properties_content_and_child_lists() {
    let context = AppLayout {
        pad: 12.0,
        title: "上下文标题",
        show_action: true,
        radius: RadiusAttr::Pill,
    };
    let mut ui = page();
    let mut host = TestHost;
    let palette = palette();
    let report = context_layout::install(&mut ui, &mut host, &palette, Size::new(300.0, 200.0), &context)
        .expect("the context module applies at the base breakpoint");
    assert_eq!(report.variant_id, "base");

    let installed = ui.installed_layout().expect("installed");
    assert_eq!(
        installed.rect(WidgetId::new("title")),
        Some(Rect::from_xywh(12.0, 12.0, 276.0, 24.0)),
        "x/y read the context's pad and w the client width in the captured closure"
    );
    assert_eq!(
        installed.rect(WidgetId::new("list")),
        Some(Rect::from_xywh(12.0, 120.0, 200.0, 160.0)),
        "a literal rectangle stays a static fn"
    );
    assert_eq!(
        ui.with_widget::<Label, _>(WidgetId::new("title"), |label| label.text().to_string()),
        Some("上下文标题".to_string()),
        "the content value is the context's title"
    );
    assert_eq!(
        ui.with_widget::<Button<&'static str>, _>(WidgetId::new("action"), |button| {
            button.surface_style().map(|style| style.corners)
        }),
        Some(Some(Corners::uniform(16.0))),
        "the radius is the context's RadiusAttr (pill clamps to the 32 DIP height)"
    );
    let rows = ui
        .with_widget::<ListView<&'static str>, _>(WidgetId::new("list"), |list| {
            list.rows()
                .iter()
                .map(|row| (row.title.clone(), row.subtitle.clone()))
                .collect::<Vec<_>>()
        })
        .expect("the list is in the tree");
    assert_eq!(
        rows,
        vec![
            ("第一行".to_string(), "上下文标题".to_string()),
            ("第二行".to_string(), String::new()),
        ],
        "the <rust> binding built the child list from the context"
    );

    let action = ui
        .a11y_nodes()
        .into_iter()
        .find(|node| node.id == WidgetId::new("action"))
        .expect("visible=true from the context keeps the button in the tree");
    assert!(action.enabled);
    let title = ui
        .a11y_nodes()
        .into_iter()
        .find(|node| node.id == WidgetId::new("title"))
        .expect("the exposed label is published");
    assert_eq!(
        title.name, "上下文标题",
        "the XML `name` read from the context replaced the application's"
    );

    // The wide breakpoint reads the same context but its own rectangles.
    let wide = context_layout::install(&mut ui, &mut host, &palette, Size::new(600.0, 400.0), &context)
        .expect("the same context applies at the wide breakpoint");
    assert_eq!(wide.variant_id, "wide");
    assert_eq!(
        ui.installed_layout().and_then(|layout| layout.rect(WidgetId::new("list"))),
        Some(Rect::from_xywh(12.0, 120.0, 360.0, 160.0))
    );
    assert_eq!(
        ui.with_widget::<Label, _>(WidgetId::new("title"), |label| label.text().to_string()),
        Some("上下文标题".to_string()),
        "content is generation-scoped: a breakpoint switch does not rewrite it"
    );
}

#[test]
fn a_context_change_is_read_on_the_next_apply_but_content_stays_per_generation() {
    let first = AppLayout {
        pad: 10.0,
        title: "第一次",
        show_action: true,
        radius: RadiusAttr::Medium,
    };
    let mut ui = page();
    let mut host = TestHost;
    let palette = palette();
    context_layout::install(&mut ui, &mut host, &palette, Size::new(300.0, 200.0), &first)
        .expect("applies");

    // A different context on the same generation: the rectangle follows (it is
    // evaluated per apply), the content does not (written once per generation).
    let second = AppLayout {
        pad: 20.0,
        title: "第二次",
        show_action: false,
        radius: RadiusAttr::Small,
    };
    let report = context_layout::install(&mut ui, &mut host, &palette, Size::new(300.0, 200.0), &second)
        .expect("re-applies");
    assert_eq!(report.content_writes, 0);
    assert_eq!(
        ui.installed_layout().and_then(|layout| layout.rect(WidgetId::new("title"))),
        Some(Rect::from_xywh(20.0, 20.0, 260.0, 24.0)),
        "geometry is re-evaluated from the new context"
    );
    assert_eq!(
        ui.with_widget::<Label, _>(WidgetId::new("title"), |label| label.text().to_string()),
        Some("第一次".to_string())
    );
    assert!(
        ui.a11y_nodes()
            .into_iter()
            .all(|node| node.id != WidgetId::new("action")),
        "visible=false from the new context hides the button"
    );
}

#[test]
fn the_static_descriptor_reports_a_context_rectangle() {
    let mut ui = page();
    let mut host = TestHost;
    let palette = palette();
    let failure = ui
        .apply_compiled(&context_layout::LAYOUT, &palette, Size::new(300.0, 200.0), &mut host)
        .expect_err("a context rectangle has no static form");
    assert!(
        failure
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "xml.rust.context"),
        "every rejection names the context rule: {:?}",
        failure.diagnostics
    );
    assert!(!failure.diagnostics.is_empty());
    assert!(ui.installed_layout().is_none());
    assert_eq!(
        ui.with_widget::<Label, _>(WidgetId::new("title"), |label| label.text().to_string()),
        Some("原标题".to_string()),
        "the failed apply mutated nothing"
    );
}

#[cfg(feature = "layout-xml")]
#[test]
fn the_dynamic_mode_refuses_context_and_bindings_atomically() {
    use bamboo_native_ui::layout_xml::LayoutDocument;

    let document = LayoutDocument::parse(include_str!("fixtures/context.buixml")).expect("parses");
    let mut ui = page();
    let mut host = TestHost;
    let palette = palette();
    let failure = ui
        .apply_layout(&document, &palette, Size::new(300.0, 200.0), &mut host)
        .expect_err("a runtime file is data, never code");
    assert!(
        failure
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "xml.rust.unsupported"),
        "inline Rust is refused before anything mutates: {:?}",
        failure.diagnostics
    );
    assert!(failure.diagnostics.len() >= 2, "scalars and the binding both report");
    assert!(ui.installed_layout().is_none());
}

#[test]
fn a_binding_without_a_context_applies_through_the_static_descriptor() {
    let mut ui = page();
    let mut host = TestHost;
    let palette = palette();
    binding_layout::install(&mut ui, &mut host, &palette, Size::new(400.0, 300.0))
        .expect("a context-free module applies through its own install");
    let rows = ui
        .with_widget::<ListView<&'static str>, _>(WidgetId::new("list"), |list| {
            list.rows()
                .iter()
                .map(|row| row.title.clone())
                .collect::<Vec<_>>()
        })
        .expect("the list is in the tree");
    assert_eq!(rows, vec!["静态绑定".to_string()]);
    // The static `LAYOUT` descriptor is enough here: no context rectangles.
    ui.clear_layout(&mut host);
    ui.apply_compiled(
        &binding_layout::LAYOUT,
        &palette,
        Size::new(400.0, 300.0),
        &mut host,
    )
    .expect("the static descriptor applies too");
}
