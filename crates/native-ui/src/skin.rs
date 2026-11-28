// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! The widget-declared skin protocol (M7 §6.2–6.4, D127).
//!
//! The types live in `bamboo-native-ui-layout-core` so the parsed model can
//! own child elements without depending on this crate; they are re-exported
//! here at their historical path and compiled unconditionally, so a
//! third-party widget can declare support in a build without `layout-xml`. The
//! applier validates every declaration and every value before it mutates
//! anything ([`crate::Ui::apply_layout`]).

pub use bamboo_native_ui_layout_core::skin::{
    is_shared_name, SkinAttrDecl, SkinElement, SkinValue, SkinValueKind,
};
#[cfg(test)]
pub(crate) use bamboo_native_ui_layout_core::skin::parse_color;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{Color, Rgba};

    #[test]
    fn value_kinds_parse_their_domain() {
        assert_eq!(
            SkinValueKind::Text.parse("  any text  "),
            Some(SkinValue::Text("  any text  "))
        );
        assert_eq!(
            SkinValueKind::Number.parse(" 12.5 "),
            Some(SkinValue::Number(12.5))
        );
        assert_eq!(SkinValueKind::Number.parse("NaN"), None);
        assert_eq!(SkinValueKind::Number.parse("inf"), None);
        assert_eq!(SkinValueKind::Number.parse("12px"), None);
        assert_eq!(SkinValueKind::Bool.parse("1"), Some(SkinValue::Bool(true)));
        assert_eq!(SkinValueKind::Bool.parse("false"), Some(SkinValue::Bool(false)));
        assert_eq!(SkinValueKind::Bool.parse("yes"), None);
        assert_eq!(
            SkinValueKind::Color.parse("#0af"),
            Some(SkinValue::Color(Rgba::opaque(Color::new(0, 0xaa, 0xff))))
        );
        assert_eq!(
            SkinValueKind::Color.parse("#112233"),
            Some(SkinValue::Color(Rgba::opaque(Color::new(0x11, 0x22, 0x33))))
        );
        // Shorthand and full alpha forms share the one color value.
        assert_eq!(
            SkinValueKind::Color.parse("#1234"),
            Some(SkinValue::Color(Rgba::new(
                Color::new(0x11, 0x22, 0x33),
                0x44
            )))
        );
        let translucent = SkinValueKind::Color.parse("#11223380").expect("rgba");
        assert_eq!(
            translucent.as_rgba(),
            Some(Rgba::new(Color::new(0x11, 0x22, 0x33), 0x80))
        );
        // A widget without alpha support silently keeps the RGB part.
        assert_eq!(translucent.as_color(), Some(Color::new(0x11, 0x22, 0x33)));
        // Seven digits are a typo, not a valid color.
        assert_eq!(SkinValueKind::Color.parse("112233"), None);
        assert_eq!(SkinValueKind::Color.parse("#1122334"), None);
    }

    #[test]
    fn color_literals_accept_hex_decimal_and_functional_forms() {
        let red = Rgba::new(Color::new(255, 0, 0), 255);
        for raw in [
            "#ff0000",
            "#f00",
            "255,0,0",
            "255, 0, 0",
            "255 0 0",
            "rgb(255,0,0)",
            "rgb(255 0 0)",
            "rgb( 255 , 0 , 0 )",
            "RGB(255,0,0)",
        ] {
            assert_eq!(
                SkinValueKind::Color.parse(raw),
                Some(SkinValue::Color(red)),
                "{raw}"
            );
        }
        let half = Rgba::new(Color::new(255, 0, 0), 128);
        for raw in [
            "rgba(255,0,0,0.5)",
            "rgba(255 0 0 0.5)",
            "rgba( 255 , 0 , 0 , 0.5 )",
            "255,0,0,128",
            "255, 0, 0, 0.5",
            "#ff000080",
        ] {
            assert_eq!(
                SkinValueKind::Color.parse(raw),
                Some(SkinValue::Color(half)),
                "{raw}"
            );
        }
        // Functional alpha follows CSS: 1 is opaque. A bare integer 1 is one
        // unit of 255.
        assert_eq!(
            SkinValueKind::Color.parse("rgba(255,0,0,1)"),
            Some(SkinValue::Color(red))
        );
        assert_eq!(
            SkinValueKind::Color.parse("255,0,0,1"),
            Some(SkinValue::Color(Rgba::new(Color::new(255, 0, 0), 1)))
        );
        // Channels clamp; malformed forms are rejected.
        assert_eq!(
            SkinValueKind::Color.parse("rgb(300,-5,0)"),
            Some(SkinValue::Color(red))
        );
        assert_eq!(SkinValueKind::Color.parse("rgb(1,2)"), None);
        assert_eq!(SkinValueKind::Color.parse("rgb(1,2,3,4,5)"), None);
        assert_eq!(SkinValueKind::Color.parse("rgb(1,2,3"), None);
        assert_eq!(SkinValueKind::Color.parse("rgb(1,2,3) x"), None);
        assert_eq!(SkinValueKind::Color.parse("orange"), None);
        // The layout's `background` literal shares this grammar.
        assert_eq!(
            crate::skin::parse_color("rgb(18, 52, 86)").map(|rgba| rgba.color),
            Some(Color::new(18, 52, 86))
        );
    }

    #[test]
    fn shared_names_are_the_content_vocabulary() {
        for name in ["text", "label", "value", "progress", "number", "checked", "selected"] {
            assert!(is_shared_name(name), "{name}");
        }
        for name in ["glyph", "x-level", "cue", "playing"] {
            assert!(!is_shared_name(name), "{name}");
        }
    }

    #[test]
    fn color_values_resolve_names_before_hex() {
        let accent = Color::new(1, 2, 3);
        let named = SkinValueKind::Color
            .parse_with(" accent ", |name| (name == "accent").then_some(accent))
            .expect("token");
        assert_eq!(named.as_color(), Some(accent));
        // An unresolved name still falls back to the literal grammar.
        let hex = SkinValueKind::Color
            .parse_with("#0af", |_| None)
            .expect("hex");
        assert_eq!(hex.as_color(), Some(Color::new(0, 0xaa, 0xff)));
        assert!(SkinValueKind::Color
            .parse_with("bogus", |_| None)
            .is_none());
        // Non-color kinds ignore the resolver.
        assert_eq!(
            SkinValueKind::Text.parse_with("accent", |_| Some(accent)),
            Some(SkinValue::Text("accent"))
        );
    }

    #[test]
    fn elements_expose_raw_attributes() {
        let mut item = SkinElement::new("x-item", 3, 5);
        item.push_attr("title", "夜曲");
        item.push_attr("level", "72");
        item.push_attr("current", "true");
        assert_eq!(item.attr("title"), Some("夜曲"));
        assert_eq!(item.attr_number("level"), Some(72.0));
        assert_eq!(item.attr_bool("current"), Some(true));
        assert_eq!(item.attr_bool("missing"), None);
        assert_eq!(item.attr_number("title"), None);
        let mut group = SkinElement::new("x-group", 1, 1);
        group.push_child(item);
        assert_eq!(group.children.len(), 1);
        assert_eq!(group.children[0].name, "x-item");
        assert_eq!(group.children[0].line, 3);
    }

    // A sample of the built-ins that declare local scalars (M7.3). The
    // applier-side coverage lives behind `layout-xml`; this keeps the
    // declaration-to-setter link honest in the default build too.
    #[test]
    fn built_in_widgets_publish_a_working_local_vocabulary() {
        use crate::widget::Widget;
        use crate::widgets::{Card, ColorSwatch, Icon, Kbd, Spinner};

        let names = |decls: &[SkinAttrDecl]| decls.iter().map(|decl| decl.name).collect::<Vec<_>>();

        let mut card = Card::new("c", "旧标题", "旧正文");
        assert_eq!(names(<Card as Widget<()>>::skin_attrs(&card)), ["title", "body", "emphasis"]);
        <Card as Widget<()>>::apply_skin_attr(&mut card, "title", &SkinValue::Text("新标题"));
        <Card as Widget<()>>::apply_skin_attr(&mut card, "emphasis", &SkinValue::Bool(true));
        assert_eq!(card.title(), "新标题");
        assert!(card.is_emphasis());

        let mut icon = Icon::new("i", 'A');
        assert_eq!(names(<Icon as Widget<()>>::skin_attrs(&icon)), ["glyph", "tint"]);
        <Icon as Widget<()>>::apply_skin_attr(&mut icon, "glyph", &SkinValue::Text("B"));
        assert_eq!(icon.glyph(), 'B');

        let mut kbd = Kbd::new("k", "Ctrl");
        <Kbd as Widget<()>>::apply_skin_attr(&mut kbd, "shortcut", &SkinValue::Text("Ctrl+K"));
        assert_eq!(kbd.text(), "Ctrl+K");

        let mut swatch = ColorSwatch::new("s", Color::new(0, 0, 0), "色");
        <ColorSwatch as Widget<()>>::apply_skin_attr(
            &mut swatch,
            "color",
            &SkinValue::Color(Rgba::opaque(Color::new(1, 2, 3))),
        );
        assert_eq!(swatch.color(), Color::new(1, 2, 3));

        let mut spinner = Spinner::new("sp");
        <Spinner as Widget<()>>::apply_skin_attr(&mut spinner, "phase", &SkinValue::Number(90.0));
        <Spinner as Widget<()>>::apply_skin_attr(&mut spinner, "animating", &SkinValue::Bool(false));
        assert!((spinner.phase() - 90.0).abs() < 0.01);
        assert!(!spinner.is_animating());
    }

    // The appearance/interaction locals of the data widgets (M7.4 follow-up).
    #[test]
    fn appearance_locals_work_without_layout_xml() {
        use crate::widget::Widget;
        use crate::widgets::{Sparkline, Table, Tabs};

        let names = |decls: &[SkinAttrDecl]| decls.iter().map(|decl| decl.name).collect::<Vec<_>>();

        let mut table = Table::<()>::new("table");
        assert_eq!(
            names(<Table<()> as Widget<()>>::skin_attrs(&table)),
            [
                "row-height",
                "header-height",
                "cell-padding",
                "striped",
                "grid-lines",
                "editable",
                "empty-text"
            ]
        );
        <Table<()> as Widget<()>>::apply_skin_attr(&mut table, "row-height", &SkinValue::Number(30.0));
        <Table<()> as Widget<()>>::apply_skin_attr(&mut table, "striped", &SkinValue::Bool(true));
        assert_eq!(table.row_height_value(), 30.0);
        assert!(table.is_striped());

        let mut tabs: Tabs<()> = Tabs::new("tabs", &["一"], 0, |_| ());
        <Tabs<()> as Widget<()>>::apply_skin_attr(
            &mut tabs,
            "label-align",
            &SkinValue::Text("right"),
        );
        assert_eq!(tabs.label_alignment(), crate::backend::TextAlign::Right);

        let mut spark = Sparkline::new("spark", &[1.0]);
        <Sparkline as Widget<()>>::apply_skin_attr(
            &mut spark,
            "color",
            &SkinValue::Color(Rgba::opaque(Color::new(1, 2, 3))),
        );
        assert_eq!(spark.color_value(), Some(Color::new(1, 2, 3)));
    }
}
