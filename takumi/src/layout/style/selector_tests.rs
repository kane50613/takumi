use super::*;
use crate::layout::style::{
  Color, ColorInput, ComputedStyle, Length, Style, StyleDeclaration, StyleDeclarationBlock,
};

fn computed_style_from_declarations(declarations: &StyleDeclarationBlock) -> ComputedStyle {
  let mut style = Style::default();
  for declaration in &declarations.declarations {
    declaration.merge_into_ref(&mut style);
  }
  style.inherit(&ComputedStyle::default())
}

#[test]
fn test_parse_stylesheet() {
  let css = r#"
            .box {
                width: 100px;
                color: red;
            }
        "#;
  let sheet = StyleSheet::parse(css);
  assert_eq!(sheet.rules.len(), 1);
  let rule = &sheet.rules[0];

  assert_eq!(rule.selectors.slice().len(), 1);
  assert_eq!(
    computed_style_from_declarations(&rule.normal_declarations).width,
    Length::Px(100.0)
  );
}

#[test]
fn test_parse_stylesheet_compound_selectors_specificity() {
  let sheet = StyleSheet::parse(
    r#"
        div.box { width: 10px; }
        #hero .label { height: 20px; }
      "#,
  );
  assert_eq!(sheet.rules.len(), 2);
  assert_eq!(sheet.rules[0].selectors.slice().len(), 1);
  assert_eq!(sheet.rules[1].selectors.slice().len(), 1);
  assert!(sheet.rules[0].selectors.slice()[0].specificity() > 0);
  assert!(
    sheet.rules[1].selectors.slice()[0].specificity()
      > sheet.rules[0].selectors.slice()[0].specificity()
  );
}

#[test]
fn test_parse_stylesheet_multiple_rules() {
  let sheet = StyleSheet::parse(
    r#"
        .a { width: 10px; }
        .b { height: 20px; }
      "#,
  );

  assert_eq!(sheet.rules.len(), 2);
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
    Length::Px(10.0)
  );
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[1].normal_declarations).height,
    Length::Px(20.0)
  );
}

#[test]
fn test_parse_stylesheet_multiple_selectors_in_rule() {
  let sheet = StyleSheet::parse(
    r#"
        .a, .b { width: 12px; }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(sheet.rules[0].selectors.slice().len(), 2);
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
    Length::Px(12.0)
  );
}

#[test]
fn test_parse_stylesheet_universal_selector() {
  let sheet = StyleSheet::parse(
    r#"
        * { width: 100px; }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(sheet.rules[0].selector_text, "*");
  assert_eq!(sheet.rules[0].selectors.slice().len(), 1);
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
    Length::Px(100.0)
  );
}

#[test]
fn test_parse_stylesheet_important_declaration() {
  let sheet = StyleSheet::parse(
    r#"
        .a { width: 10px !important; height: 20px; }
      "#,
  );

  let rule = &sheet.rules[0];
  assert_eq!(
    computed_style_from_declarations(&rule.important_declarations).width,
    Length::Px(10.0)
  );
  assert_eq!(
    computed_style_from_declarations(&rule.normal_declarations).height,
    Length::Px(20.0)
  );
}

#[test]
fn test_parse_stylesheet_shorthand_clears_prior_longhand() {
  let sheet = StyleSheet::parse(
    r#"
        .a { padding-left: 4px; padding: 10px; }
      "#,
  );

  let declarations = &sheet.rules[0].normal_declarations;
  assert_eq!(declarations.declarations.len(), 5);
  assert_eq!(
    declarations.declarations[0],
    StyleDeclaration::padding_left(Length::Px(4.0))
  );
  assert_eq!(
    declarations.declarations[1],
    StyleDeclaration::padding_top(Length::Px(10.0))
  );
  assert_eq!(
    declarations.declarations[2],
    StyleDeclaration::padding_right(Length::Px(10.0))
  );
  assert_eq!(
    declarations.declarations[3],
    StyleDeclaration::padding_bottom(Length::Px(10.0))
  );
  assert_eq!(
    declarations.declarations[4],
    StyleDeclaration::padding_left(Length::Px(10.0))
  );
}

#[test]
fn test_parse_stylesheet_webkit_alias_property() {
  let sheet = StyleSheet::parse(
    r#"
        .a { -webkit-text-fill-color: rgb(255, 0, 0); }
      "#,
  );

  let style = computed_style_from_declarations(&sheet.rules[0].normal_declarations);
  assert_eq!(
    style.webkit_text_fill_color,
    Some(ColorInput::Value(Color([255, 0, 0, 255])))
  );
}

#[test]
fn test_parse_stylesheet_unknown_property_does_not_drop_supported_declarations() {
  let sheet = StyleSheet::parse(
    r#"
        .a { --local-token: 1; width: 14px; unsupported-prop: 2; height: 6px; }
      "#,
  );

  let style = computed_style_from_declarations(&sheet.rules[0].normal_declarations);
  assert_eq!(style.width, Length::Px(14.0));
  assert_eq!(style.height, Length::Px(6.0));
}

#[test]
fn test_unsupported_attribute_selector_rule_is_rejected() {
  let sheet = StyleSheet::parse(
    r#"
        [data-kind="hero"] { width: 10px; }
      "#,
  );

  assert!(sheet.rules.is_empty());
}

#[test]
fn test_unsupported_pseudo_selector_rule_is_rejected() {
  let sheet = StyleSheet::parse(
    r#"
        .a:hover { width: 10px; }
      "#,
  );

  assert!(sheet.rules.is_empty());
}

#[test]
fn test_parse_keyframes_rule() {
  let sheet = StyleSheet::parse(
    r#"
        @keyframes fade {
          from { opacity: 0; }
          50% { opacity: 0.5; }
          to { opacity: 1; }
        }
      "#,
  );

  assert!(sheet.rules.is_empty());
  assert_eq!(sheet.keyframes.len(), 1);
  assert_eq!(sheet.keyframes[0].name, "fade");
  assert_eq!(sheet.keyframes[0].keyframes.len(), 3);
  assert_eq!(sheet.keyframes[0].keyframes[0].offsets, vec![0.0]);
  assert_eq!(sheet.keyframes[0].keyframes[1].offsets, vec![0.5]);
  assert_eq!(sheet.keyframes[0].keyframes[2].offsets, vec![1.0]);
}

#[test]
fn test_parse_media_rule_with_viewport_features() {
  let sheet = StyleSheet::parse(
    r#"
        @media screen and (min-width: 600px) and (orientation: landscape) {
          .card { width: 100px; }
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert!(sheet.keyframes.is_empty());
  assert!(
    sheet.rules[0]
      .media_queries
      .first()
      .is_some_and(|media| media.matches(Viewport::new(Some(800), Some(600))))
  );
  assert!(
    !sheet.rules[0]
      .media_queries
      .first()
      .is_some_and(|media| media.matches(Viewport::new(Some(500), Some(800))))
  );
}

#[test]
fn test_parse_media_rule_with_comma_list() {
  let sheet = StyleSheet::parse(
    r#"
        @media (max-width: 480px), (min-width: 1024px) {
          .card { width: 100px; }
        }
      "#,
  );

  let Some(media) = sheet.rules[0].media_queries.first() else {
    unreachable!("expected media queries on parsed rule");
  };
  assert!(media.matches(Viewport::new(Some(400), Some(800))));
  assert!(media.matches(Viewport::new(Some(1280), Some(800))));
  assert!(!media.matches(Viewport::new(Some(800), Some(800))));
}

#[test]
fn test_parse_nested_rule_is_flattened() {
  let sheet = StyleSheet::parse(
    r#"
        .card {
          width: 100px;
          .title { height: 20px; }
          & > .icon { width: 12px; }
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 3);
  assert_eq!(sheet.rules[0].selector_text, ".card");
  assert_eq!(sheet.rules[1].selector_text, ".card .title");
  assert_eq!(sheet.rules[2].selector_text, ".card > .icon");
}

#[test]
fn test_parse_nested_rule_cross_product_for_selector_lists() {
  let sheet = StyleSheet::parse(
    r#"
        .card, .panel {
          & .title, & .subtitle { width: 12px; }
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(
    sheet.rules[0].selector_text,
    ".card .title, .card .subtitle, .panel .title, .panel .subtitle"
  );
}

#[test]
fn test_parse_nested_media_and_supports_rules() {
  let sheet = StyleSheet::parse(
    r#"
        .card {
          @media (min-width: 600px) {
            @supports (display: grid) {
              width: 100px;
            }
          }
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(sheet.rules[0].selector_text, ".card");
  assert_eq!(sheet.rules[0].media_queries.len(), 1);
  assert!(
    sheet.rules[0]
      .media_queries
      .first()
      .is_some_and(|media| media.matches(Viewport::new(Some(800), Some(600))))
  );
}

#[test]
fn test_parse_multiple_nested_media_queries_accumulate() {
  let sheet = StyleSheet::parse(
    r#"
        .card {
          @media (min-width: 600px) {
            @media (orientation: landscape) {
              width: 100px;
            }
          }
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(sheet.rules[0].media_queries.len(), 2);
  assert!(sheet.rules[0].media_queries[0].matches(Viewport::new(Some(800), Some(600))));
  assert!(sheet.rules[0].media_queries[1].matches(Viewport::new(Some(800), Some(600))));
  assert!(!sheet.rules[0].media_queries[1].matches(Viewport::new(Some(500), Some(800))));
}

#[test]
fn test_parse_supports_rule_filters_unsupported_declarations() {
  let sheet = StyleSheet::parse(
    r#"
        @supports (display: grid) {
          .card { width: 100px; }
        }

        @supports (unknown-prop: nope) {
          .card { height: 20px; }
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(sheet.rules[0].selector_text, ".card");
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
    Length::Px(100.0)
  );
}

#[test]
fn test_parse_supports_not_and_or_conditions() {
  let sheet = StyleSheet::parse(
    r#"
        @supports (display: grid) and (not (unknown-prop: nope)) {
          .grid { width: 10px; }
        }

        @supports (unknown-prop: nope) or (display: flex) {
          .flex { height: 20px; }
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 2);
  assert_eq!(sheet.rules[0].selector_text, ".grid");
  assert_eq!(sheet.rules[1].selector_text, ".flex");
}

#[test]
fn test_parse_property_rule() {
  let sheet = StyleSheet::parse(
    r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: 10px;
        }
      "#,
  );

  assert_eq!(sheet.property_rules.len(), 1);
  assert_eq!(sheet.property_rules[0].name, "--box-size");
  assert_eq!(sheet.property_rules[0].syntax, "\"<length>\"");
  assert!(!sheet.property_rules[0].inherits);
  assert_eq!(sheet.property_rules[0].initial_value, "10px");
}

#[test]
fn test_parse_property_rule_requires_initial_value_for_typed_syntax() {
  let sheet = StyleSheet::parse(
    r#"
        @property --tw-rotate-x {
          syntax: "*";
          inherits: false;
        }
      "#,
  );

  assert_eq!(sheet.property_rules.len(), 1);
  assert_eq!(sheet.property_rules[0].name, "--tw-rotate-x");
  assert_eq!(sheet.property_rules[0].syntax, "\"*\"");
  assert!(!sheet.property_rules[0].inherits);
  assert_eq!(sheet.property_rules[0].initial_value, "");

  let sheet = StyleSheet::parse(
    r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
        }
      "#,
  );

  assert!(sheet.property_rules.is_empty());
}

#[test]
fn test_parse_property_rule_supports_extended_syntaxes() {
  let sheet = StyleSheet::parse(
    r#"
        @property --accent {
          syntax: "<length> | <color>";
          inherits: false;
          initial-value: red;
        }
        @property --display-state {
          syntax: "none | auto";
          inherits: false;
          initial-value: none;
        }
        @property --fade-duration {
          syntax: "<time>";
          inherits: false;
          initial-value: 150ms;
        }
        @property --move {
          syntax: "<transform-function>";
          inherits: false;
          initial-value: translate(10px, 20px);
        }
        @property --curve {
          syntax: "<easing-function>";
          inherits: false;
          initial-value: ease-in-out;
        }
        @property --fx {
          syntax: "<filter-function>";
          inherits: false;
          initial-value: blur(4px);
        }
        @property --bg {
          syntax: "<image>";
          inherits: false;
          initial-value: linear-gradient(red, blue);
        }
      "#,
  );

  assert_eq!(sheet.property_rules.len(), 7);
  assert_eq!(sheet.property_rules[0].syntax, "\"<length> | <color>\"");
  assert_eq!(sheet.property_rules[1].syntax, "\"none | auto\"");
  assert_eq!(sheet.property_rules[2].syntax, "\"<time>\"");
  assert_eq!(sheet.property_rules[3].syntax, "\"<transform-function>\"");
  assert_eq!(sheet.property_rules[4].syntax, "\"<easing-function>\"");
  assert_eq!(sheet.property_rules[5].syntax, "\"<filter-function>\"");
  assert_eq!(sheet.property_rules[6].syntax, "\"<image>\"");
}

#[test]
fn test_invalid_property_rule_name_is_rejected() {
  let sheet = StyleSheet::parse(
    r#"
        @property color {
          syntax: "<color>";
          inherits: false;
          initial-value: red;
        }
      "#,
  );

  assert!(sheet.property_rules.is_empty());
}

#[test]
fn test_property_rule_missing_syntax_is_rejected() {
  let sheet = StyleSheet::parse(
    r#"
        @property --box-size {
          inherits: false;
          initial-value: 10px;
        }
      "#,
  );

  assert!(sheet.property_rules.is_empty());
}

#[test]
fn test_property_rule_missing_inherits_is_rejected() {
  let sheet = StyleSheet::parse(
    r#"
        @property --accent {
          syntax: "<color>";
          initial-value: red;
        }
      "#,
  );

  assert!(sheet.property_rules.is_empty());
}

#[test]
fn test_property_rule_computationally_dependent_initial_value_is_rejected() {
  let sheet = StyleSheet::parse(
    r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: var(--fallback);
        }
      "#,
  );

  assert!(sheet.property_rules.is_empty());
}

#[test]
fn test_parse_layer_rule_without_block() {
  let sheet = StyleSheet::parse(
    r#"
        @layer theme, base, components, utilities;
        @layer utilities {
          .card { width: 100px; }
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(sheet.rules[0].selector_text, ".card");
  assert_eq!(
    sheet.rules[0].layer.as_deref(),
    Some(&["utilities".to_owned()][..])
  );
  assert_eq!(sheet.rules[0].layer_order, Some(3));
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
    Length::Px(100.0)
  );
}

#[test]
fn test_parse_nested_layers_are_transparent() {
  let sheet = StyleSheet::parse(
    r#"
        @layer theme {
          @layer components {
            .card { width: 100px; }
          }
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(sheet.rules[0].selector_text, ".card");
  assert_eq!(
    sheet.rules[0].layer.as_deref(),
    Some(&["theme".to_owned(), "components".to_owned()][..])
  );
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
    Length::Px(100.0)
  );
}

#[test]
fn test_parse_nested_rules_preserves_source_order() {
  let sheet = StyleSheet::parse(
    r#"
        .card {
          width: 100px;
          & .title { color: red; }
          height: 20px;
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 3);
  assert_eq!(sheet.rules[0].selector_text, ".card");
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
    Length::Px(100.0)
  );
  assert_eq!(sheet.rules[1].selector_text, ".card .title");
  assert_eq!(sheet.rules[2].selector_text, ".card");
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[2].normal_declarations).height,
    Length::Px(20.0)
  );
}

#[test]
fn test_nested_unsupported_supports_rule_is_discarded() {
  let sheet = StyleSheet::parse(
    r#"
        .card {
          width: 100px;
          @supports (unknown-prop: nope) {
            height: 20px;
            & .title { color: red; }
          }
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(sheet.rules[0].selector_text, ".card");

  let computed = computed_style_from_declarations(&sheet.rules[0].normal_declarations);
  assert_eq!(computed.width, Length::Px(100.0));
  assert_eq!(computed.height, Length::Auto);
}

#[test]
fn test_nested_keyframes_rule_is_rejected() {
  let sheet = StyleSheet::parse(
    r#"
        .card {
          width: 100px;
          @keyframes pulse {
            from { opacity: 0; }
            to { opacity: 1; }
          }
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(sheet.keyframes.len(), 0);
  assert_eq!(sheet.rules[0].selector_text, ".card");
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
    Length::Px(100.0)
  );
}

#[test]
fn test_nested_property_rule_is_rejected() {
  let sheet = StyleSheet::parse(
    r#"
        .card {
          width: 100px;
          @property --box-size {
            syntax: "<length>";
            inherits: false;
            initial-value: 10px;
          }
        }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert!(sheet.property_rules.is_empty());
  assert_eq!(sheet.rules[0].selector_text, ".card");
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[0].normal_declarations).width,
    Length::Px(100.0)
  );
}

#[test]
fn test_invalid_property_inherits_value_is_rejected() {
  let sheet = StyleSheet::parse(
    r#"
        @property --box-size {
          syntax: "<length>";
          inherits: maybe;
          initial-value: 10px;
        }

        .card { width: 100px; }
      "#,
  );

  assert!(sheet.property_rules.is_empty());
  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(sheet.rules[0].selector_text, ".card");
}

#[test]
fn test_unsupported_media_feature_rule_is_rejected() {
  let sheet = StyleSheet::parse(
    r#"
        @media (resolution: 2dppx) {
          .card { width: 100px; }
        }

        .panel { height: 20px; }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(sheet.rules[0].selector_text, ".panel");
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[0].normal_declarations).height,
    Length::Px(20.0)
  );
}

#[test]
fn test_unknown_at_rule_is_rejected() {
  let sheet = StyleSheet::parse(
    r#"
        @unknown something {
          .card { width: 100px; }
        }

        .panel { height: 20px; }
      "#,
  );

  assert_eq!(sheet.rules.len(), 1);
  assert_eq!(sheet.rules[0].selector_text, ".panel");
  assert_eq!(
    computed_style_from_declarations(&sheet.rules[0].normal_declarations).height,
    Length::Px(20.0)
  );
}
