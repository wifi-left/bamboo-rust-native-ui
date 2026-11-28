// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The checked-in generated module must stay current, constants must be folded,
//! and a bad document must report its source position.

use bamboo_native_ui_layout_codegen::{generate, generate_with, module_name, Options};

const SOURCE: &str = include_str!("../../native-ui/tests/fixtures/parity.buixml");
const GOLDEN: &str = include_str!("../../native-ui/tests/fixtures/parity_generated.rs");
const DEMO: &str = include_str!("../../showcase/layouts/layout-demo.buixml");
const RUST_SOURCE: &str = include_str!("../../native-ui/tests/fixtures/rust.buixml");
const RUST_GOLDEN: &str = include_str!("../../native-ui/tests/fixtures/rust_generated.rs");
const CONTEXT_SOURCE: &str = include_str!("../../native-ui/tests/fixtures/context.buixml");
const CREATE_SOURCE: &str = include_str!("../../native-ui/tests/fixtures/create.buixml");
const CREATE_GOLDEN: &str = include_str!("../../native-ui/tests/fixtures/create_generated.rs");
const BINDING_SOURCE: &str = include_str!("../../native-ui/tests/fixtures/binding.buixml");
const BINDING_GOLDEN: &str = include_str!("../../native-ui/tests/fixtures/binding_generated.rs");
const CONTEXT_GOLDEN: &str = include_str!("../../native-ui/tests/fixtures/context_generated.rs");
const CONTEXT_MODULE: Options<'static> = Options {
    module: "context_layout",
    context: Some("AppLayout"),
};

#[test]
fn the_checked_in_module_is_current() {
    let generated = generate(SOURCE, "parity.buixml", "parity").expect("the fixture parses");
    assert_eq!(
        generated, GOLDEN,
        "regenerate with: cargo run -p bamboo-native-ui-layout-codegen -- \
         crates/native-ui/tests/fixtures/parity.buixml -o \
         crates/native-ui/tests/fixtures/parity_generated.rs"
    );
}

#[test]
fn the_checked_in_inline_rust_module_is_current() {
    let generated =
        generate(RUST_SOURCE, "rust.buixml", "rust_layout").expect("the fixture parses");
    assert_eq!(generated, RUST_GOLDEN);
}

#[test]
fn inline_rust_text_is_spliced_verbatim() {
    let generated = generate(RUST_SOURCE, "rust.buixml", "rust_layout").unwrap();
    // Geometry sees the schema's space variables as f64 locals.
    assert!(generated.contains("let cw = s.width as f64;"));
    assert!(generated.contains("let w = (cw - 2.0 * PAGE_PAD);"));
    // Properties, style and content are spliced into the statics/tables.
    assert!(generated.contains("visible: Some(SHOW_ACTION)"));
    assert!(generated.contains("enabled: Some(!LOCKED)"));
    assert!(generated.contains("radius: Some(RADIUS)"));
    assert!(generated.contains("CompiledContent::Label(TITLE)"));
}

#[test]
fn the_checked_in_context_module_is_current() {
    let generated = generate_with(CONTEXT_SOURCE, "context.buixml", CONTEXT_MODULE)
        .expect("the fixture parses");
    assert_eq!(
        generated, CONTEXT_GOLDEN,
        "regenerate with: cargo run -p bamboo-native-ui-layout-codegen -- \
         crates/native-ui/tests/fixtures/context.buixml --module context_layout \
         --context AppLayout -o crates/native-ui/tests/fixtures/context_generated.rs"
    );
}

#[test]
fn context_values_are_evaluated_per_apply() {
    let generated = generate_with(CONTEXT_SOURCE, "context.buixml", CONTEXT_MODULE).unwrap();
    // The install takes the context and builds one value list per variant.
    assert!(generated.contains("context: &AppLayout,"));
    assert!(generated.contains(
        "let values = CompiledValues { variants: vec![__values_0(context), __values_1(context)] };"
    ));
    assert!(generated.contains("ui.apply_compiled_with(&LAYOUT, &values, palette, client, sink)"));
    // A context rectangle is a captured closure, not a static fn.
    assert!(generated.contains("Box::new(move |s: Size| -> Result<Rect, RectError>"));
    assert!(generated.contains("let w = (cw - 2.0 * context.pad);"));
    assert!(generated.contains("Err(RectError::Context)"));
    // Scalars and the binding move out of the statics into the value fns.
    assert!(generated.contains("props: Some(PropAttrs { visible: Some(context.show_action)"));
    assert!(generated.contains("style: Some(StyleAttrs { radius: Some(context.radius)"));
    assert!(generated.contains("content: Some(vec![ContentAttr::Label((context.title).to_string())])"));
    assert!(generated.contains("children: Some(context.rows())"));
    assert!(!generated.contains("LazyLock::new(|| { context."), "no context code in a static");
}

#[test]
fn a_context_needs_a_type() {
    let diagnostics = generate_with(
        SOURCE,
        "parity.buixml",
        Options {
            module: "parity",
            context: Some("  "),
        },
    )
    .expect_err("an empty context type rejects");
    assert!(diagnostics.iter().any(|item| item.code == "xml.rust.context"));
}

#[test]
fn the_checked_in_binding_module_is_current() {
    let generated = generate(BINDING_SOURCE, "binding.buixml", "binding_layout")
        .expect("the fixture parses");
    assert_eq!(
        generated, BINDING_GOLDEN,
        "regenerate with: cargo run -p bamboo-native-ui-layout-codegen --          crates/native-ui/tests/fixtures/binding.buixml --module binding_layout          -o crates/native-ui/tests/fixtures/binding_generated.rs"
    );
}

#[test]
fn the_checked_in_create_module_is_current() {
    let generated =
        generate(CREATE_SOURCE, "create.buixml", "create_layout").expect("the fixture parses");
    assert_eq!(
        generated, CREATE_GOLDEN,
        "regenerate with: cargo run -p bamboo-native-ui-layout-codegen --          crates/native-ui/tests/fixtures/create.buixml --module create_layout          -o crates/native-ui/tests/fixtures/create_generated.rs"
    );
}

#[test]
fn created_types_and_commands_are_descriptor_data() {
    let generated = generate(CREATE_SOURCE, "create.buixml", "create_layout").unwrap();
    assert!(generated.contains("type_name: Some(\"label\"), command: None"));
    assert!(generated.contains("type_name: Some(\"button\"), command: Some(101)"));
    // The generated install takes the registry from the Ui, so it is generic
    // over `M: 'static`.
    assert!(generated.contains("pub fn install<M: 'static, H: UiHost>("));
}

#[test]
fn a_binding_without_a_context_is_a_static() {
    let text = r#"<layouts version="1"><layout id="base" min-width="0" min-height="0">
  <widget id="a" x="0" y="0" w="10" h="10"/>
  <rust target="a" code="rows()"/>
</layout></layouts>"#;
    let generated = generate(text, "rows.buixml", "rows").unwrap();
    assert!(generated.contains("static __children_0_0: LazyLock<Vec<SkinElement>> = LazyLock::new(|| { rows() });"));
    assert!(generated.contains("rust_children: Some(&__children_0_0)"));
    assert!(!generated.contains("CompiledValues"), "no context, no values");
}

#[test]
fn constants_are_folded_into_the_output() {
    let generated = generate(SOURCE, "parity.buixml", "parity").unwrap();
    // `pad` is 10 in the fixture: `cw-2*pad` becomes native f64 arithmetic.
    assert!(
        generated.contains("s.width as f64) - ((2.0) * (10.0))"),
        "the constant is inlined, not named"
    );
    assert!(!generated.contains("pad"));
    // A percent expression stays the model's `value / 100 * extent` order.
    assert!(generated.contains("(25.0 / 100.0 * (s.width as f64))"));
}

#[test]
fn the_showcase_document_generates() {
    let generated = generate(DEMO, "layout-demo.buixml", "layout_demo").expect("the demo parses");
    assert!(generated.contains("pub mod layout_demo {"));
    assert!(generated.contains("pub static LAYOUT: CompiledLayout"));
    // Both variant breakpoints survive as literal minimums.
    assert!(generated.contains("min_width: 1000.0"));
    assert!(generated.contains("min_height: 700.0"));
}

#[test]
fn a_bad_document_reports_its_source_position() {
    let diagnostics = generate("<layouts version=\"1\"><layout id=\"a\"><widget id=\"x\" x=\"oops\"/></layout></layouts>", "bad.buixml", "bad")
        .expect_err("a malformed expression rejects");
    assert!(!diagnostics.is_empty());
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.line >= 1));
}

#[test]
fn module_names_are_identifiers() {
    assert_eq!(module_name("layout-demo.buixml"), "layout_demo");
    assert_eq!(module_name(r"C:\layouts\main.buixml"), "main");
    assert_eq!(module_name("9 lives.buixml"), "_9_lives");
    assert_eq!(module_name("...."), "layout");
}
