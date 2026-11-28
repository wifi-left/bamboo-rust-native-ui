// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! M7.7: a document creates widgets from the `WidgetRegistry` (D148) — the
//! standard types, the command table, document ownership, reload removal and
//! every rejection path. The compiled mode shares the planner, so one module
//! fixture (`create.buixml`) proves the second consumption path too.

#![cfg(feature = "layout-xml")]

use bamboo_native_ui::creation::WidgetRegistry;
use bamboo_native_ui::geometry::{Rect, Size};
use bamboo_native_ui::layout_xml::LayoutDocument;
use bamboo_native_ui::scroll::ScrollView;
use bamboo_native_ui::widgets::{Button, Label, Scrim};
use bamboo_native_ui::{
    Invalidator, Palette, Point, ScrollBars, ThemeSettings, Ui, UiHost, Widget, WidgetId,
};

#[cfg(feature = "layout-compiled")]
include!("fixtures/create_generated.rs");

struct TestHost;

impl Invalidator for TestHost {
    fn invalidate(&mut self, _rect: Rect) {}
}

impl UiHost for TestHost {}

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default(), false)
}

fn registry() -> WidgetRegistry<&'static str> {
    let mut registry = WidgetRegistry::standard();
    registry.set_commands(|command| match command {
        101 => Some("go"),
        _ => None,
    });
    registry
}

const DOC: &str = r#"<layouts version="1"><layout id="base">
  <widget id="hello" type="label" x="10" y="10" w="100" h="20" label="你好"/>
  <widget id="go" type="button" command="101" x="10" y="40" w="80" h="30" label="出发"/>
</layout></layouts>"#;

fn ui_with_registry() -> Ui<&'static str> {
    let mut ui: Ui<&'static str> = Ui::new();
    ui.set_widget_registry(registry());
    ui
}

/// A UI with one application-registered scroll panel (the document does not lay
/// it out, so a created child reaches it through the attach path).
fn ui_with_panel() -> Ui<&'static str> {
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    ui.set_scroll_view(
        ScrollView::new(
            WidgetId::new("panel"),
            Rect::from_xywh(200.0, 0.0, 100.0, 100.0),
            ScrollBars::Vertical,
        )
        .content(Size::new(100.0, 300.0)),
        &mut host,
    );
    ui
}

const DOC_WITH_PARENT: &str = r#"<layouts version="1"><layout id="base">
  <widget id="hello" type="label" x="10" y="10" w="100" h="20" label="你好"/>
  <widget id="row" type="label" parent="panel" x="0" y="250" w="80" h="20" label="行"/>
</layout></layouts>"#;

#[test]
fn a_document_creates_widgets_and_the_command_reaches_the_application() {
    let document = LayoutDocument::parse(DOC).expect("parses");
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    let report = ui
        .apply_layout(&document, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("applies");
    assert_eq!(report.created, 2);
    assert_eq!(
        ui.installed_layout()
            .map(|layout| layout.created_ids().collect::<Vec<_>>()),
        Some(vec![WidgetId::new("hello"), WidgetId::new("go")])
    );

    // The file's geometry and content reach the created widgets.
    assert_eq!(
        ui.rect(WidgetId::new("hello")),
        Some(Rect::from_xywh(10.0, 10.0, 100.0, 20.0))
    );
    assert_eq!(
        ui.with_widget::<Label, _>(WidgetId::new("hello"), |label| label.text().to_string()),
        Some("你好".to_string())
    );
    let go = ui
        .a11y_nodes()
        .into_iter()
        .find(|node| node.id == WidgetId::new("go"))
        .expect("the created button is in the tree");
    assert_eq!(go.name, "出发", "the file's label reached the created button");

    // A click reports the command's message.
    ui.pointer_down(Point::new(20.0, 50.0), &mut host);
    ui.pointer_up(Point::new(20.0, 50.0), &mut host);
    assert_eq!(ui.drain_messages(), vec!["go"]);
}

#[test]
fn created_widgets_take_their_claimed_fields() {
    let document = LayoutDocument::parse(
        r#"<layouts version="1"><layout id="base">
  <widget id="hidden" type="label" x="0" y="0" w="80" h="20" visible="false" label="隐藏"/>
  <widget id="chip" type="label" x="0" y="30" w="80" h="20" radius="small" label="圆角"/>
</layout></layouts>"#,
    )
    .expect("parses");
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    ui.apply_layout(&document, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("applies");
    assert!(
        ui.a11y_nodes()
            .into_iter()
            .all(|node| node.id != WidgetId::new("hidden")),
        "visible=false from the file hides the created widget"
    );
    let corners = ui
        .with_widget::<Label, _>(WidgetId::new("chip"), |label| {
            <Label as Widget<&'static str>>::surface_style(label)
                .map(|style| style.corners)
        })
        .flatten();
    assert!(corners.is_some(), "the created label adopted a SurfaceStyle");
    assert!(
        corners.is_some_and(|corners| corners.max() > 0.0),
        "the file's radius reached the created widget"
    );
}

#[test]
fn a_created_widget_inherits_the_page_visibility_set_before_it_existed() {
    let document = LayoutDocument::parse(DOC).expect("parses");
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    // The application hides the page before the layout creates its widgets.
    ui.set_visible(WidgetId::new("hello"), false, &mut host);
    ui.apply_layout(&document, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("applies");
    assert!(
        ui.a11y_nodes()
            .into_iter()
            .all(|node| node.id != WidgetId::new("hello")),
        "the remembered override keeps the created widget hidden"
    );
}

#[test]
fn a_same_generation_reapply_updates_in_place() {
    let document = LayoutDocument::parse(DOC).expect("parses");
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    let palette = palette();
    ui.apply_layout(&document, &palette, Size::new(400.0, 300.0), &mut host)
        .expect("applies");
    let report = ui
        .apply_layout(&document, &palette, Size::new(300.0, 300.0), &mut host)
        .expect("re-applies");
    assert_eq!(report.created, 2, "the recorded set reflects the document");
    assert_eq!(report.content_writes, 0, "same generation: nothing is rewritten");
    assert!(ui.contains(WidgetId::new("hello")));
}

#[test]
fn clear_layout_removes_the_widgets_the_document_created() {
    let document = LayoutDocument::parse(DOC).expect("parses");
    let mut ui = ui_with_registry();
    ui.add(Label::new("app", "应用程序"));
    ui.set_rect(WidgetId::new("app"), Rect::from_xywh(0.0, 0.0, 40.0, 20.0));
    let mut host = TestHost;
    ui.apply_layout(&document, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("applies");
    ui.clear_layout(&mut host);
    assert!(!ui.contains(WidgetId::new("hello")));
    assert!(!ui.contains(WidgetId::new("go")));
    assert!(ui.contains(WidgetId::new("app")), "application widgets stay");
    assert!(ui.installed_layout().is_none());
}

#[test]
fn a_reload_removes_what_the_new_document_no_longer_declares() {
    let first = LayoutDocument::parse(DOC).expect("parses");
    let second = LayoutDocument::parse(
        r#"<layouts version="1"><layout id="base">
  <widget id="hello" type="label" x="10" y="10" w="120" h="20" label="你好"/>
</layout></layouts>"#,
    )
    .expect("parses");
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    ui.apply_layout(&first, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("applies");
    assert!(ui.contains(WidgetId::new("go")));
    let report = ui
        .apply_layout(&second, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("reloads");
    assert!(report.created == 1);
    assert!(ui.contains(WidgetId::new("hello")));
    assert!(!ui.contains(WidgetId::new("go")), "a vanished id is removed");
    assert_eq!(
        ui.installed_layout()
            .map(|layout| layout.created_ids().collect::<Vec<_>>()),
        Some(vec![WidgetId::new("hello")])
    );
}

#[test]
fn a_type_change_replaces_the_widget() {
    let label = LayoutDocument::parse(DOC).expect("parses");
    let button = LayoutDocument::parse(
        r#"<layouts version="1"><layout id="base">
  <widget id="hello" type="button" command="101" x="10" y="10" w="100" h="20"/>
  <widget id="go" type="button" command="101" x="10" y="40" w="80" h="30" label="出发"/>
</layout></layouts>"#,
    )
    .expect("parses");
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    ui.apply_layout(&label, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("applies");
    assert!(ui
        .with_widget::<Label, _>(WidgetId::new("hello"), |_| ())
        .is_some());
    ui.apply_layout(&button, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("replaces");
    assert!(
        ui.with_widget::<Label, _>(WidgetId::new("hello"), |_| ())
            .is_none(),
        "the label was removed"
    );
    assert!(
        ui.with_widget::<Button<&'static str>, _>(WidgetId::new("hello"), |_| ())
            .is_some(),
        "the button replaced it"
    );
}

#[test]
fn created_widgets_stay_in_the_page_layer() {
    let document = LayoutDocument::parse(DOC).expect("parses");
    let mut ui = ui_with_registry();
    ui.add(Label::new("app", "应用程序"));
    // The modal backdrop is added before the layout is installed, so a created
    // widget must go *below* it (the overlay split is insertion-ordered).
    ui.add(Scrim::new("scrim"));
    let mut host = TestHost;
    ui.apply_layout(&document, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("applies");
    let ids: Vec<WidgetId> = ui.ids().collect();
    let scrim = ids
        .iter()
        .position(|id| *id == WidgetId::new("scrim"))
        .expect("scrim");
    for id in ["hello", "go"] {
        let at = ids
            .iter()
            .position(|candidate| *candidate == WidgetId::new(id))
            .expect("created");
        assert!(at < scrim, "`{id}` is inserted before the backdrop");
    }
}

#[test]
fn unknown_types_and_missing_commands_reject_atomically() {
    let unknown = LayoutDocument::parse(
        r#"<layouts version="1"><layout id="base">
  <widget id="x" type="widget-gallery" x="0" y="0" w="10" h="10"/>
</layout></layouts>"#,
    )
    .expect("parses");
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    let failure = ui
        .apply_layout(&unknown, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect_err("unknown type");
    assert_eq!(failure.first().map(|item| item.code), Some("xml.type.unknown"));
    assert!(!ui.contains(WidgetId::new("x")));
    assert!(ui.installed_layout().is_none());

    let no_command = LayoutDocument::parse(
        r#"<layouts version="1"><layout id="base">
  <widget id="x" type="button" x="0" y="0" w="10" h="10"/>
</layout></layouts>"#,
    )
    .expect("parses");
    let mut ui = ui_with_registry();
    let failure = ui
        .apply_layout(&no_command, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect_err("no command");
    assert_eq!(
        failure.first().map(|item| item.code),
        Some("xml.command.missing")
    );
    assert!(!ui.contains(WidgetId::new("x")));
}

#[test]
fn an_application_widget_id_is_not_taken_over() {
    let document = LayoutDocument::parse(DOC).expect("parses");
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    ui.add(Label::new("hello", "应用自己的"));
    ui.set_rect(WidgetId::new("hello"), Rect::from_xywh(0.0, 0.0, 40.0, 20.0));
    let failure = ui
        .apply_layout(&document, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect_err("id collision");
    assert_eq!(failure.first().map(|item| item.code), Some("xml.id.duplicate"));
    assert_eq!(
        ui.with_widget::<Label, _>(WidgetId::new("hello"), |label| label.text().to_string()),
        Some("应用自己的".to_string()),
        "nothing was replaced"
    );
}

#[test]
fn a_factory_must_build_the_entrys_id() {
    let document = LayoutDocument::parse(
        r#"<layouts version="1"><layout id="base">
  <widget id="a" type="broken" x="0" y="0" w="10" h="10"/>
</layout></layouts>"#,
    )
    .expect("parses");
    let mut registry: WidgetRegistry<&'static str> = WidgetRegistry::new();
    registry.register("broken", |_spec, _ctx| {
        Ok(Box::new(Label::new("other", "wrong")))
    });
    let mut ui: Ui<&'static str> = Ui::new();
    ui.set_widget_registry(registry);
    let mut host = TestHost;
    let failure = ui
        .apply_layout(&document, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect_err("wrong id");
    assert_eq!(failure.first().map(|item| item.code), Some("xml.type.create"));
    assert!(!ui.contains(WidgetId::new("other")));
}

#[test]
fn a_created_widget_joins_the_panel_its_parent_names() {
    let document = LayoutDocument::parse(DOC_WITH_PARENT).expect("parses");
    let mut ui = ui_with_panel();
    let mut host = TestHost;
    let report = ui
        .apply_layout(&document, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("applies");
    assert_eq!(report.created, 2);
    let row = WidgetId::new("row");
    let panel = WidgetId::new("panel");
    assert_eq!(ui.scroll_owner_of(row), Some(panel));
    // The document's `x`/`y` are content coordinates; the container positions
    // the child at `content origin - offset`.
    assert_eq!(
        ui.scroll_view(panel).and_then(|view| view.child_rect(row)),
        Some(Rect::from_xywh(0.0, 250.0, 80.0, 20.0))
    );
    assert_eq!(ui.rect(row), Some(Rect::from_xywh(200.0, 250.0, 80.0, 20.0)));
    // Scrolling the panel moves it.
    assert!(ui.set_scroll_offset(panel, Point::new(0.0, 200.0), &mut host));
    assert_eq!(ui.rect(row), Some(Rect::from_xywh(200.0, 50.0, 80.0, 20.0)));
    // The content pass reached the created widget as usual.
    assert_eq!(
        ui.with_widget::<Label, _>(row, |label| label.text().to_string()),
        Some("行".to_string())
    );
}

#[test]
fn a_declared_panel_takes_the_created_child_too() {
    let document = LayoutDocument::parse(
        r#"<layouts version="1"><layout id="base">
  <scroll id="panel" x="10" y="10" w="200" h="100"/>
  <widget id="row" type="label" parent="panel" x="0" y="0" w="80" h="20" label="行"/>
</layout></layouts>"#,
    )
    .expect("parses");
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    ui.set_scroll_view(
        ScrollView::new(
            WidgetId::new("panel"),
            Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
            ScrollBars::Vertical,
        ),
        &mut host,
    );
    ui.apply_layout(&document, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("applies");
    let row = WidgetId::new("row");
    assert_eq!(
        ui.scroll_view(WidgetId::new("panel"))
            .map(|view| view.viewport()),
        Some(Rect::from_xywh(10.0, 10.0, 200.0, 100.0))
    );
    assert_eq!(ui.rect(row), Some(Rect::from_xywh(10.0, 10.0, 80.0, 20.0)));

    // A reload that drops the entry removes it and forgets the panel child.
    let reloaded = LayoutDocument::parse(
        r#"<layouts version="1"><layout id="base">
  <scroll id="panel" x="10" y="10" w="200" h="100"/>
</layout></layouts>"#,
    )
    .expect("parses");
    ui.apply_layout(&reloaded, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("reloads");
    assert!(!ui.contains(row));
    assert_eq!(ui.scroll_owner_of(row), None);
    assert!(
        ui.scroll_view(WidgetId::new("panel"))
            .is_some_and(|view| !view.contains(row))
    );
}

#[test]
fn parent_errors_reject_atomically() {
    // The parent must be a registered scroll panel.
    let unknown = LayoutDocument::parse(DOC_WITH_PARENT).expect("parses");
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    let failure = ui
        .apply_layout(&unknown, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect_err("no panel");
    assert_eq!(
        failure.first().map(|item| item.code),
        Some("xml.parent.missing")
    );
    assert!(!ui.contains(WidgetId::new("row")));
    assert!(ui.installed_layout().is_none());

    // An addressed widget asserts the application's own membership.
    let asserted = LayoutDocument::parse(
        r#"<layouts version="1"><layout id="base">
  <widget id="app" parent="panel" x="0" y="0" w="40" h="20"/>
</layout></layouts>"#,
    )
    .expect("parses");
    let mut ui = ui_with_panel();
    ui.add(Label::new("app", "应用"));
    ui.set_rect(WidgetId::new("app"), Rect::from_xywh(0.0, 0.0, 40.0, 20.0));
    let failure = ui
        .apply_layout(&asserted, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect_err("not a panel child");
    assert_eq!(
        failure.first().map(|item| item.code),
        Some("xml.parent.missing")
    );
    // The matching membership is accepted.
    let mut host = TestHost;
    ui.set_scroll_view(
        ScrollView::new(
            WidgetId::new("panel"),
            Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
            ScrollBars::Vertical,
        )
        .child(WidgetId::new("app"), Rect::from_xywh(0.0, 0.0, 40.0, 20.0)),
        &mut host,
    );
    ui.apply_layout(&asserted, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("membership matches");
}

#[test]
fn a_document_reorder_restacks_the_created_block() {
    let document = |body: &str| {
        LayoutDocument::parse(&format!(
            r#"<layouts version="1"><layout id="base">{body}</layout></layouts>"#
        ))
        .expect("parses")
    };
    let entry = |id: &str, y: &str| {
        format!(r#"<widget id="{id}" type="label" x="0" y="{y}" w="80" h="20" label="{id}"/>"#)
    };
    let position = |ui: &Ui<&'static str>, id: &'static str| {
        let ids: Vec<WidgetId> = ui.ids().collect();
        ids.iter()
            .position(|candidate| *candidate == WidgetId::new(id))
            .expect("created")
    };

    let abc = document(&format!(
        "{}{}{}",
        entry("a", "0"),
        entry("b", "20"),
        entry("c", "40")
    ));
    let acb = document(&format!(
        "{}{}{}",
        entry("a", "0"),
        entry("c", "20"),
        entry("b", "40")
    ));
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    ui.apply_layout(&abc, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("applies");
    assert!(position(&ui, "a") < position(&ui, "b"));
    assert!(position(&ui, "b") < position(&ui, "c"));

    // Reordering the document rebuilds the block in declaration order, even
    // though every id was already kept.
    let report = ui
        .apply_layout(&acb, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("reloads");
    assert_eq!(report.created, 3);
    assert!(position(&ui, "a") < position(&ui, "c"));
    assert!(position(&ui, "c") < position(&ui, "b"));
    // The rows also follow the new declaration order's geometry.
    assert_eq!(
        ui.rect(WidgetId::new("b")),
        Some(Rect::from_xywh(0.0, 40.0, 80.0, 20.0))
    );

    // A middle insertion restacks too: kept a/c keep their document slots and
    // the new b lands between them rather than at the end.
    let inserted = document(&format!(
        "{}{}{}",
        entry("a", "0"),
        entry("b", "20"),
        entry("c", "40")
    ));
    ui.apply_layout(&inserted, &palette(), Size::new(400.0, 300.0), &mut host)
        .expect("inserts");
    assert!(position(&ui, "a") < position(&ui, "b"));
    assert!(position(&ui, "b") < position(&ui, "c"));
}

#[test]
fn a_same_generation_apply_never_restacks() {
    let document = LayoutDocument::parse(DOC).expect("parses");
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    let palette = palette();
    ui.apply_layout(&document, &palette, Size::new(400.0, 300.0), &mut host)
        .expect("applies");
    let before: Vec<WidgetId> = ui.ids().collect();
    ui.apply_layout(&document, &palette, Size::new(300.0, 200.0), &mut host)
        .expect("re-applies");
    assert_eq!(ui.ids().collect::<Vec<_>>(), before);
}

#[cfg(feature = "layout-compiled")]
#[test]
fn the_compiled_mode_creates_the_same_widgets() {
    let mut ui = ui_with_registry();
    let mut host = TestHost;
    ui.set_scroll_view(
        ScrollView::new(
            WidgetId::new("panel"),
            Rect::from_xywh(200.0, 0.0, 100.0, 100.0),
            ScrollBars::Vertical,
        )
        .content(Size::new(100.0, 300.0)),
        &mut host,
    );
    let report = create_layout::install(&mut ui, &mut host, &palette(), Size::new(400.0, 300.0))
        .expect("the generated module applies");
    assert_eq!(report.created, 3);
    assert_eq!(
        ui.with_widget::<Label, _>(WidgetId::new("hello"), |label| label.text().to_string()),
        Some("你好".to_string())
    );
    // The compiled descriptor carries `parent` too: the row is a panel child in
    // content coordinates and the container positions it.
    assert_eq!(ui.scroll_owner_of(WidgetId::new("row")), Some(WidgetId::new("panel")));
    assert_eq!(
        ui.scroll_view(WidgetId::new("panel"))
            .and_then(|view| view.child_rect(WidgetId::new("row"))),
        Some(Rect::from_xywh(0.0, 250.0, 80.0, 20.0))
    );
    assert_eq!(
        ui.rect(WidgetId::new("row")),
        Some(Rect::from_xywh(200.0, 250.0, 80.0, 20.0))
    );
    ui.pointer_down(Point::new(20.0, 50.0), &mut host);
    ui.pointer_up(Point::new(20.0, 50.0), &mut host);
    assert_eq!(ui.drain_messages(), vec!["go"]);

    // The generated install takes the command map from the installed registry,
    // so swapping the registry (no commands) makes the button fail.
    let mut bare: Ui<&'static str> = Ui::new();
    bare.set_widget_registry(WidgetRegistry::standard());
    bare.set_scroll_view(
        ScrollView::new(
            WidgetId::new("panel"),
            Rect::from_xywh(200.0, 0.0, 100.0, 100.0),
            ScrollBars::Vertical,
        ),
        &mut host,
    );
    let failure = create_layout::install(&mut bare, &mut host, &palette(), Size::new(400.0, 300.0))
        .expect_err("no command table");
    assert_eq!(
        failure.first().map(|item| item.code),
        Some("xml.command.missing")
    );
    assert!(!bare.contains(WidgetId::new("hello")));
    assert!(!bare.contains(WidgetId::new("row")));
}
