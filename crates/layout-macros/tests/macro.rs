// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! `layout_xml!` expands the same module the CLI checks in, so the compiled
//! path works without a separate generation step.

use bamboo_native_ui::geometry::{Rect, Size};
use bamboo_native_ui::layout_xml::RadiusAttr;
use bamboo_native_ui::skin::SkinElement;
use bamboo_native_ui::widgets::{Button, Label, ListView};
use bamboo_native_ui::{Corners, Invalidator, Palette, ThemeSettings, Ui, UiHost, Widget, WidgetId};
use bamboo_native_ui_layout_macros::layout_xml;

const PAGE_PAD: f64 = 12.0;
const TITLE: &str = "动态标题";
const SHOW_ACTION: bool = true;
const LOCKED: bool = false;
const RADIUS: RadiusAttr = RadiusAttr::Medium;

layout_xml!("../native-ui/tests/fixtures/rust.buixml", module = rust_layout);
layout_xml!(
    "../native-ui/tests/fixtures/context.buixml",
    module = context_layout,
    context = AppLayout
);

/// The context type the second expansion takes; `context = AppLayout` resolves
/// here through the module's `use super::*`.
pub struct AppLayout {
    pad: f64,
    title: &'static str,
    show_action: bool,
    radius: RadiusAttr,
}

impl AppLayout {
    fn rows(&self) -> Vec<SkinElement> {
        let mut row = SkinElement::new("item", 1, 1);
        row.push_attr("title", self.title);
        vec![row]
    }
}

struct TestHost;

impl Invalidator for TestHost {
    fn invalidate(&mut self, _rect: Rect) {}
}

impl UiHost for TestHost {}

#[test]
fn the_macro_expands_a_working_module() {
    let mut ui: Ui<&'static str> = Ui::new();
    ui.add(Button::new("action", "原操作", "clicked"));
    ui.add(Label::new("title", "原标题"));
    for id in ["action", "title"] {
        ui.set_rect(WidgetId::new(id), Rect::from_xywh(0.0, 0.0, 40.0, 20.0));
    }
    let mut host = TestHost;
    let palette = Palette::new(&ThemeSettings::default(), false);
    ui.apply_compiled(&rust_layout::LAYOUT, &palette, Size::new(400.0, 300.0), &mut host)
        .expect("the generated module applies");
    assert_eq!(
        ui.installed_layout().and_then(|layout| layout.rect(WidgetId::new("title"))),
        Some(Rect::from_xywh(12.0, 12.0, 376.0, 24.0))
    );
    assert_eq!(
        ui.with_widget::<Label, _>(WidgetId::new("title"), |label| label.text().to_string()),
        Some(TITLE.to_string())
    );
    assert_eq!(
        ui.with_widget::<Button<&'static str>, _>(WidgetId::new("action"), |button| {
            button.surface_style().map(|style| style.corners)
        }),
        Some(Some(Corners::uniform(6.0)))
    );
}

#[test]
fn the_macro_expands_a_context_module() {
    let mut ui: Ui<&'static str> = Ui::new();
    ui.add(Button::new("action", "原操作", "clicked"));
    ui.add(Label::new("title", "原标题"));
    ui.add(ListView::new("list"));
    for id in ["action", "title", "list"] {
        ui.set_rect(WidgetId::new(id), Rect::from_xywh(0.0, 0.0, 40.0, 20.0));
    }
    let mut host = TestHost;
    let palette = Palette::new(&ThemeSettings::default(), false);
    let context = AppLayout {
        pad: 8.0,
        title: "上下文标题",
        show_action: true,
        radius: RadiusAttr::Small,
    };
    context_layout::install(&mut ui, &mut host, &palette, Size::new(320.0, 240.0), &context)
        .expect("the macro module applies with a context");
    assert_eq!(
        ui.installed_layout()
            .and_then(|layout| layout.rect(WidgetId::new("title"))),
        Some(Rect::from_xywh(8.0, 8.0, 304.0, 24.0))
    );
    assert_eq!(
        ui.with_widget::<Label, _>(WidgetId::new("title"), |label| label.text().to_string()),
        Some("上下文标题".to_string())
    );
    assert_eq!(
        ui.with_widget::<ListView<&'static str>, _>(WidgetId::new("list"), |list| {
            list.rows().len()
        }),
        Some(1),
        "the <rust> binding ran through the macro expansion"
    );
}
