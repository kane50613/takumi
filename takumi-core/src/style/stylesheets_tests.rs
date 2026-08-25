use std::{collections::HashMap, rc::Rc, str::FromStr};

use cssparser::{Parser, ParserInput};
use serde_json::{from_value, json};

use super::{
  CssWideKeyword, LonghandId, PropertyId, PropertyMask, ShorthandId, StyleDeclarationBlock,
  stylesheets_vars::resolve_var_references,
};
use crate::{
  geometry::Size,
  style::{CalcArena, ComputedStyle, SizingContext, Style, StyleDeclaration, properties::*},
  viewport::Viewport,
};

fn style_with(declarations: impl IntoIterator<Item = StyleDeclaration>) -> Style {
  let mut style = Style::default();
  for declaration in declarations {
    style.push(declaration, false);
  }
  style
}

fn parse_declarations(name: &str, css: &str) -> StyleDeclarationBlock {
  let mut input = ParserInput::new(css);
  let mut parser = Parser::new(&mut input);
  let declarations_result = StyleDeclarationBlock::parse(name, &mut parser);
  assert!(declarations_result.is_ok());
  declarations_result.unwrap_or_default()
}

fn parse_declarations_is_err(name: &str, css: &str) -> bool {
  let mut input = ParserInput::new(css);
  let mut parser = Parser::new(&mut input);
  StyleDeclarationBlock::parse(name, &mut parser).is_err()
}

fn inherited_style_from_pairs(
  declarations: impl IntoIterator<Item = (&'static str, &'static str)>,
  parent: &ComputedStyle,
) -> ComputedStyle {
  let mut style = Style::default();
  for (name, value) in declarations {
    style.append_block(parse_declarations(name, value));
  }
  style.inherit(parent)
}

fn resolve_var(
  specified_value: &str,
  custom_properties: impl IntoIterator<Item = (&'static str, &'static str)>,
) -> Option<String> {
  let custom_properties = custom_properties
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .collect::<HashMap<_, _>>();

  resolve_var_references(specified_value, &custom_properties, &mut Vec::new())
}

#[test]
fn test_merge_from_inline_over_tailwind() {
  let mut tw_style = style_with([
    StyleDeclaration::width(Length::Rem(10.0)),
    StyleDeclaration::height(Length::Rem(20.0)),
    StyleDeclaration::color(ColorInput::Value(Color([255, 0, 0, 255]))),
  ]);
  let inline_style = style_with([StyleDeclaration::width(Length::Px(100.0))]);

  tw_style.merge_from(inline_style);

  let resolved = tw_style.inherit(&ComputedStyle::default());
  assert_eq!(resolved.width, Length::Px(100.0));
  assert_eq!(resolved.height, Length::Rem(20.0));
  assert_eq!(resolved.color, ColorInput::Value(Color([255, 0, 0, 255])));
}

#[test]
fn test_deserialize_numeric_opacity_preserves_fraction() -> Result<(), serde_json::Error> {
  let style = from_value::<Style>(json!({ "opacity": 0.3 }))?;
  let computed = style.inherit(&ComputedStyle::default());

  assert_eq!(computed.opacity, PercentageNumber(0.3));
  Ok(())
}

/// The JSX presets ship these as a camel-case style object.
#[test]
fn test_deserialize_list_style_properties() -> Result<(), serde_json::Error> {
  let style = from_value::<Style>(json!({
    "display": "list-item",
    "listStyleType": "decimal",
    "listStylePosition": "inside",
  }))?;
  let computed = style.inherit(&ComputedStyle::default());

  assert_eq!(computed.display, Display::ListItem);
  assert_eq!(computed.list_style_type, ListStyleType::Decimal);
  assert_eq!(computed.list_style_position, ListStylePosition::Inside);
  Ok(())
}

#[test]
fn test_deserialize_skips_null_declarations() -> Result<(), serde_json::Error> {
  let style = from_value::<Style>(json!({ "color": null, "opacity": 0.3 }))?;
  let computed = style.inherit(&ComputedStyle::default());

  assert_eq!(computed.opacity, PercentageNumber(0.3));
  assert_eq!(computed.color, ComputedStyle::default().color);
  Ok(())
}

fn feature_tags(features: &[FontFeature]) -> Vec<(String, u16)> {
  features
    .iter()
    .map(|feature| {
      (
        String::from_utf8_lossy(&feature.tag.to_bytes()).into_owned(),
        feature.value,
      )
    })
    .collect()
}

#[test]
fn font_variant_longhands_expand_to_features() {
  let style = inherited_style_from_pairs(
    [
      ("font-variant-numeric", "tabular-nums ordinal"),
      ("font-variant-caps", "all-small-caps"),
      ("font-variant-east-asian", "traditional ruby"),
      ("font-variant-position", "super"),
    ],
    &ComputedStyle::default(),
  );

  assert_eq!(
    feature_tags(&style.resolved_font_features()),
    vec![
      ("tnum".into(), 1),
      ("ordn".into(), 1),
      ("trad".into(), 1),
      ("ruby".into(), 1),
      ("c2sc".into(), 1),
      ("smcp".into(), 1),
      ("sups".into(), 1),
    ],
  );
}

#[test]
fn font_variant_ligatures_none_disables_defaults() {
  let style = inherited_style_from_pairs(
    [("font-variant-ligatures", "none")],
    &ComputedStyle::default(),
  );

  assert_eq!(
    feature_tags(&style.resolved_font_features()),
    vec![
      ("liga".into(), 0),
      ("clig".into(), 0),
      ("dlig".into(), 0),
      ("hlig".into(), 0),
      ("calt".into(), 0),
    ],
  );
}

#[test]
fn font_kerning_expands_to_kern_feature() {
  let auto = inherited_style_from_pairs([("font-kerning", "auto")], &ComputedStyle::default());
  assert!(auto.resolved_font_features().is_empty());

  let normal = inherited_style_from_pairs([("font-kerning", "normal")], &ComputedStyle::default());
  assert_eq!(
    feature_tags(&normal.resolved_font_features()),
    vec![("kern".into(), 1)],
  );

  let none = inherited_style_from_pairs([("font-kerning", "none")], &ComputedStyle::default());
  assert_eq!(
    feature_tags(&none.resolved_font_features()),
    vec![("kern".into(), 0)],
  );
}

#[test]
fn font_feature_settings_override_font_kerning() {
  let style = inherited_style_from_pairs(
    [
      ("font-kerning", "none"),
      ("font-feature-settings", "\"kern\" 1"),
    ],
    &ComputedStyle::default(),
  );

  assert_eq!(
    feature_tags(&style.resolved_font_features()),
    vec![("kern".into(), 0), ("kern".into(), 1)],
  );
}

#[test]
fn font_variant_shorthand_distributes_and_settings_win() {
  let style = inherited_style_from_pairs(
    [
      ("font-variant", "common-ligatures tabular-nums small-caps"),
      // explicit feature-settings is appended last so it overrides the variant tag
      ("font-feature-settings", "\"tnum\" 0"),
    ],
    &ComputedStyle::default(),
  );

  let tags = feature_tags(&style.resolved_font_features());
  assert_eq!(tags.first(), Some(&("liga".to_string(), 1)));
  assert!(tags.contains(&("smcp".to_string(), 1)));
  // both tnum entries present; the last (settings) wins under swash's last-tag semantics
  assert_eq!(tags.last(), Some(&("tnum".to_string(), 0)));
}

#[test]
fn property_id_accepts_kebab_and_camel_case() {
  let padding_left_kebab = PropertyId::from_kebab_case("padding-left");
  let padding_left_camel = PropertyId::from_camel_case("paddingLeft");
  assert_ne!(padding_left_kebab, PropertyId::Ignored);
  assert_ne!(padding_left_camel, PropertyId::Ignored);
  assert_eq!(padding_left_kebab, padding_left_camel);

  let webkit_text_fill_color_kebab = PropertyId::from_kebab_case("-webkit-text-fill-color");
  let webkit_text_fill_color_camel = PropertyId::from_camel_case("WebkitTextFillColor");
  assert_ne!(webkit_text_fill_color_kebab, PropertyId::Ignored);
  assert_ne!(webkit_text_fill_color_camel, PropertyId::Ignored);
  assert_eq!(
    webkit_text_fill_color_kebab,
    PropertyId::Longhand(LonghandId::WebkitTextFillColor)
  );
  assert_eq!(
    webkit_text_fill_color_camel,
    PropertyId::Longhand(LonghandId::WebkitTextFillColor)
  );
}

#[test]
fn custom_properties_map_to_custom_property_id() {
  assert_eq!(
    PropertyId::from_kebab_case("--padding-left"),
    PropertyId::Custom
  );
  assert_eq!(
    PropertyId::from_kebab_case("--webkit-mask-image"),
    PropertyId::Custom
  );
}

#[test]
fn property_id_accepts_webkit_aliases() {
  let line_clamp = PropertyId::Shorthand(ShorthandId::LineClamp);

  assert_eq!(PropertyId::from_kebab_case("line-clamp"), line_clamp);
  assert_eq!(PropertyId::from_camel_case("lineClamp"), line_clamp);
  assert_eq!(
    PropertyId::from_kebab_case("-webkit-line-clamp"),
    line_clamp
  );
  assert_eq!(PropertyId::from_camel_case("WebkitLineClamp"), line_clamp);
  assert_eq!(
    PropertyId::from_kebab_case("-webkit-text-fill-color"),
    PropertyId::Longhand(LonghandId::WebkitTextFillColor)
  );
  assert_eq!(
    PropertyId::from_kebab_case("-webkit-text-stroke-color"),
    PropertyId::Longhand(LonghandId::WebkitTextStrokeColor)
  );
  assert_eq!(
    PropertyId::from_camel_case("WebkitTextStroke"),
    PropertyId::Shorthand(ShorthandId::WebkitTextStroke)
  );
  assert_eq!(
    PropertyId::from_kebab_case("-webkit-mask-image"),
    PropertyId::Longhand(LonghandId::MaskImage)
  );
}

#[test]
fn parse_webkit_line_clamp_matches_line_clamp() {
  let mut line_clamp = Style::default();
  line_clamp.append_block(parse_declarations("line-clamp", "2"));

  let mut webkit_line_clamp = Style::default();
  webkit_line_clamp.append_block(parse_declarations("-webkit-line-clamp", "2"));

  let webkit = webkit_line_clamp.inherit(&ComputedStyle::default());
  let plain = line_clamp.inherit(&ComputedStyle::default());
  assert_eq!(webkit.max_lines, Some(2));
  assert_eq!(webkit.max_lines, plain.max_lines);
  assert_eq!(webkit.block_ellipsis, plain.block_ellipsis);
}

#[test]
fn line_clamp_shorthand_expands_to_longhands() {
  let mut parent = Style::default();
  parent.append_block(parse_declarations("line-clamp", "3 \"...\""));
  let parent = parent.inherit(&ComputedStyle::default());

  assert_eq!(parent.max_lines, Some(3));
  assert_eq!(
    parent.block_ellipsis,
    BlockEllipsis::String("...".to_owned())
  );
  assert_eq!(parent.r#continue, Continue::Collapse);

  // Only `block-ellipsis` inherits; `max-lines` and `continue` do not.
  let child = Style::default().inherit(&parent);
  assert_eq!(child.max_lines, None);
  assert_eq!(child.r#continue, Continue::Normal);
  assert_eq!(
    child.block_ellipsis,
    BlockEllipsis::String("...".to_owned())
  );
}

#[test]
fn continue_property_resolves_by_name() {
  let continue_id = PropertyId::Longhand(LonghandId::Continue);
  assert_eq!(PropertyId::from_kebab_case("continue"), continue_id);
  assert_eq!(PropertyId::from_camel_case("continue"), continue_id);
}

#[test]
fn max_lines_clamps_only_inside_fragmentation_context() {
  // Bare `max-lines` (continue: normal) does not clamp.
  let bare = inherited_style_from_pairs([("max-lines", "3")], &ComputedStyle::default());
  assert!(bare.clamp_lines().is_none());

  // `continue: collapse` turns it into a fragmentation context that clamps.
  let clamped = inherited_style_from_pairs(
    [("max-lines", "3"), ("continue", "collapse")],
    &ComputedStyle::default(),
  );
  assert_eq!(clamped.clamp_lines(), Some(3));
}

#[test]
fn property_id_accepts_legacy_page_break_aliases() {
  assert_eq!(
    PropertyId::from_kebab_case("page-break-before"),
    PropertyId::Longhand(LonghandId::BreakBefore)
  );
  assert_eq!(
    PropertyId::from_camel_case("pageBreakAfter"),
    PropertyId::Longhand(LonghandId::BreakAfter)
  );
  assert_eq!(
    PropertyId::from_kebab_case("page-break-inside"),
    PropertyId::Longhand(LonghandId::BreakInside)
  );
}

#[test]
fn property_id_accepts_legacy_gap_aliases() {
  assert_eq!(
    PropertyId::from_kebab_case("grid-gap"),
    PropertyId::Shorthand(ShorthandId::Gap)
  );
  assert_eq!(
    PropertyId::from_camel_case("gridGap"),
    PropertyId::Shorthand(ShorthandId::Gap)
  );
  assert_eq!(
    PropertyId::from_kebab_case("grid-row-gap"),
    PropertyId::Longhand(LonghandId::RowGap)
  );
  assert_eq!(
    PropertyId::from_camel_case("gridColumnGap"),
    PropertyId::Longhand(LonghandId::ColumnGap)
  );
}

#[test]
fn parse_style_declaration_supports_css_wide_keywords_for_longhands() {
  let declarations = parse_declarations("color", "inherit");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::CssWideKeyword(
      LonghandId::Color,
      CssWideKeyword::Inherit,
    )]
  );
}

#[test]
fn parse_style_declaration_still_parses_normal_longhand_values() {
  let declarations = parse_declarations("color", "#ff0000");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::color(ColorInput::Value(Color([
      255, 0, 0, 255
    ])))]
  );
}

#[test]
fn parse_style_declaration_parses_order_integer() {
  let declarations = parse_declarations("order", "-2");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::order(Order(-2))]
  );
}

#[test]
fn parse_text_fit_values() {
  let cases = [
    (
      "none",
      TextFit {
        mode: TextFitMode::None,
        target: TextFitTarget::Consistent,
        limit: None,
      },
    ),
    (
      "grow",
      TextFit {
        mode: TextFitMode::Grow,
        target: TextFitTarget::Consistent,
        limit: None,
      },
    ),
    (
      "shrink",
      TextFit {
        mode: TextFitMode::Shrink,
        target: TextFitTarget::Consistent,
        limit: None,
      },
    ),
    (
      "grow per-line 200%",
      TextFit {
        mode: TextFitMode::Grow,
        target: TextFitTarget::PerLine,
        limit: Some(2.0),
      },
    ),
    (
      "shrink consistent 50%",
      TextFit {
        mode: TextFitMode::Shrink,
        target: TextFitTarget::Consistent,
        limit: Some(0.5),
      },
    ),
  ];

  for (input, expected) in cases {
    let declarations = parse_declarations("text-fit", input);
    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![&StyleDeclaration::text_fit(expected)]
    );
  }
}

#[test]
fn parse_text_fit_rejects_duplicate_components() {
  for input in ["grow shrink", "grow per-line consistent", "grow 120% 140%"] {
    assert!(parse_declarations_is_err("text-fit", input));
  }
}

#[test]
fn parse_style_declaration_parses_z_index_values() {
  let numeric = parse_declarations("z-index", "8");
  assert_eq!(
    numeric.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::z_index(ZIndex::Integer(8))]
  );

  let auto = parse_declarations("z-index", "auto");
  assert_eq!(
    auto.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::z_index(ZIndex::Auto)]
  );
}

#[test]
fn parse_style_declaration_expands_shorthands_in_order() {
  let declarations = parse_declarations("padding", "1px 2px");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::padding_top(Length::Px(1.0)),
      &StyleDeclaration::padding_right(Length::Px(2.0)),
      &StyleDeclaration::padding_bottom(Length::Px(1.0)),
      &StyleDeclaration::padding_left(Length::Px(2.0)),
    ]
  );
}

#[test]
fn parse_style_declaration_expands_flex_flow() {
  let declarations = parse_declarations("flex-flow", "wrap row-reverse");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::flex_direction(FlexDirection::RowReverse),
      &StyleDeclaration::flex_wrap(FlexWrap::Wrap),
    ]
  );
}

#[test]
fn parse_style_declaration_expands_place_items() {
  let declarations = parse_declarations("place-items", "center stretch");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::align_items(AlignItems::Center),
      &StyleDeclaration::justify_items(AlignItems::Stretch),
    ]
  );
}

#[test]
fn parse_style_declaration_expands_place_content() {
  let declarations = parse_declarations("place-content", "space-between center");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::align_content(JustifyContent::SpaceBetween),
      &StyleDeclaration::justify_content(JustifyContent::Center),
    ]
  );
}

#[test]
fn parse_style_declaration_expands_place_self() {
  let declarations = parse_declarations("place-self", "end stretch");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::align_self(AlignItems::End),
      &StyleDeclaration::justify_self(AlignItems::Stretch),
    ]
  );
}

#[test]
fn parse_align_items_safe_keywords() {
  assert_eq!(
    parse_declarations("align-items", "safe center")
      .iter()
      .collect::<Vec<_>>(),
    vec![&StyleDeclaration::align_items(AlignItems::SafeCenter)]
  );
  assert_eq!(
    parse_declarations("align-items", "unsafe flex-end")
      .iter()
      .collect::<Vec<_>>(),
    vec![&StyleDeclaration::align_items(AlignItems::FlexEnd)]
  );
  assert!(parse_declarations_is_err("align-items", "safe stretch"));
  assert!(parse_declarations_is_err("align-items", "safe"));
}

#[test]
fn parse_justify_content_safe_keywords() {
  assert_eq!(
    parse_declarations("justify-content", "safe flex-start")
      .iter()
      .collect::<Vec<_>>(),
    vec![&StyleDeclaration::justify_content(
      JustifyContent::SafeFlexStart
    )]
  );
  assert!(parse_declarations_is_err(
    "justify-content",
    "safe space-between"
  ));
}

#[test]
fn parse_place_items_safe_keywords() {
  assert_eq!(
    parse_declarations("place-items", "safe start safe end")
      .iter()
      .collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::align_items(AlignItems::SafeStart),
      &StyleDeclaration::justify_items(AlignItems::SafeEnd),
    ]
  );
}

#[test]
fn parse_style_declaration_expands_grid_row_shorthand() {
  let declarations = parse_declarations("grid-row", "span 2 / 5");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::grid_row_start(GridPlacement::span(2)),
      &StyleDeclaration::grid_row_end(GridPlacement::Line(5)),
    ]
  );
}

#[test]
fn parse_style_declaration_expands_grid_area_shorthand() {
  let declarations = parse_declarations("grid-area", "header / sidebar");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::grid_row_start(GridPlacement::Named("header".to_string())),
      &StyleDeclaration::grid_column_start(GridPlacement::Named("sidebar".to_string())),
      &StyleDeclaration::grid_row_end(GridPlacement::Named("header".to_string())),
      &StyleDeclaration::grid_column_end(GridPlacement::Named("sidebar".to_string())),
    ]
  );
}

#[test]
fn parse_style_declaration_supports_legacy_grid_gap_aliases() {
  let row_gap = parse_declarations("grid-row-gap", "4px");
  assert_eq!(
    row_gap.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::row_gap(Gap::Length(Length::Px(4.0)))]
  );

  let column_gap = parse_declarations("grid-column-gap", "3px");
  assert_eq!(
    column_gap.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::column_gap(Gap::Length(Length::Px(3.0)))]
  );

  let gap = parse_declarations("grid-gap", "1px 2px");
  assert_eq!(
    gap.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::row_gap(Gap::Length(Length::Px(1.0))),
      &StyleDeclaration::column_gap(Gap::Length(Length::Px(2.0))),
    ]
  );
}

#[test]
fn parse_legacy_box_properties_lower_to_flex() {
  let orient = parse_declarations("-webkit-box-orient", "vertical");
  assert_eq!(
    orient.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::flex_direction(FlexDirection::Column)]
  );

  let inline_axis = parse_declarations("-webkit-box-orient", "inline-axis");
  assert_eq!(
    inline_axis.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::flex_direction(FlexDirection::Row)]
  );

  let pack = parse_declarations("-webkit-box-pack", "justify");
  assert_eq!(
    pack.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::justify_content(
      JustifyContent::SpaceBetween
    )]
  );

  let align = parse_declarations("-webkit-box-align", "center");
  assert_eq!(
    align.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::align_items(AlignItems::Center)]
  );
}

#[test]
fn parse_style_declaration_expands_border_side_shorthands() {
  let border_top = parse_declarations("border-top", "2px solid red");
  assert_eq!(
    border_top.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::border_top_width(LineWidth::Length(Length::Px(2.0))),
      &StyleDeclaration::border_top_style(BorderStyle::Solid),
      &StyleDeclaration::border_top_color(ColorInput::Value(Color([255, 0, 0, 255]))),
    ]
  );

  let border_left = parse_declarations("border-left", "solid #00ff00");
  assert_eq!(
    border_left.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::border_left_width(LineWidth::default()),
      &StyleDeclaration::border_left_style(BorderStyle::Solid),
      &StyleDeclaration::border_left_color(ColorInput::Value(Color([0, 255, 0, 255]))),
    ]
  );
}

#[test]
fn parse_style_declaration_expands_border_style_and_color_shorthands() {
  let border_style = parse_declarations("border-style", "dashed hidden");
  assert_eq!(
    border_style.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::border_top_style(BorderStyle::Dashed),
      &StyleDeclaration::border_right_style(BorderStyle::Hidden),
      &StyleDeclaration::border_bottom_style(BorderStyle::Dashed),
      &StyleDeclaration::border_left_style(BorderStyle::Hidden),
    ]
  );

  let border_color = parse_declarations("border-color", "red green blue yellow");
  assert_eq!(
    border_color.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::border_top_color(ColorInput::Value(Color([255, 0, 0, 255]))),
      &StyleDeclaration::border_right_color(ColorInput::Value(Color([0, 128, 0, 255]))),
      &StyleDeclaration::border_bottom_color(ColorInput::Value(Color([0, 0, 255, 255]))),
      &StyleDeclaration::border_left_color(ColorInput::Value(Color([255, 255, 0, 255]))),
    ]
  );
}

#[test]
fn parse_style_declaration_ignores_unknown_properties() {
  let declarations = parse_declarations("not-a-real-property", "123");

  assert!(declarations.iter().next().is_none());
}

#[test]
fn style_declaration_block_from_str_parses_multiple_declarations() {
  let declarations = StyleDeclarationBlock::from_str("color: #ff0000; padding: 1px 2px;")
    .expect("declarations should parse");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::color(ColorInput::Value(Color([255, 0, 0, 255]))),
      &StyleDeclaration::padding_top(Length::Px(1.0)),
      &StyleDeclaration::padding_right(Length::Px(2.0)),
      &StyleDeclaration::padding_bottom(Length::Px(1.0)),
      &StyleDeclaration::padding_left(Length::Px(2.0)),
    ]
  );
}

#[test]
fn style_declaration_block_from_str_tracks_important_declarations() {
  let declarations = StyleDeclarationBlock::from_str("color: inherit !important;")
    .expect("declarations should parse");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::CssWideKeyword(
      LonghandId::Color,
      CssWideKeyword::Inherit,
    )]
  );
  assert!(
    declarations
      .importance
      .longhands
      .contains(&LonghandId::Color)
  );
}

#[test]
fn style_declaration_block_supports_reference_iteration() {
  let declarations = parse_declarations("padding", "1px 2px");

  assert_eq!(declarations.iter().count(), 4);
}

#[test]
fn style_declaration_block_collect_style_fetch_tasks_collects_url_images() {
  let background_url = "https://placehold.co/80x80/22c55e/white";
  let mask_url = "https://placehold.co/40x40/000000/white";
  let declarations = StyleDeclarationBlock::from(style_with([
    StyleDeclaration::background_image(Some(
      [
        BackgroundImage::Url(background_url.into()),
        BackgroundImage::None,
      ]
      .into(),
    )),
    StyleDeclaration::mask_image(Some([BackgroundImage::Url(mask_url.into())].into())),
  ]));

  assert_eq!(
    declarations.image_urls().collect::<Vec<_>>(),
    vec![background_url, mask_url]
  );
}

#[test]
fn style_declaration_block_into_iter_yields_owned_values() {
  let declarations = parse_declarations("padding", "1px 2px");

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::padding_top(Length::Px(1.0)),
      &StyleDeclaration::padding_right(Length::Px(2.0)),
      &StyleDeclaration::padding_bottom(Length::Px(1.0)),
      &StyleDeclaration::padding_left(Length::Px(2.0)),
    ]
  );
}

#[test]
fn test_merge_from_text_decoration_longhands_clear_lower_priority_color() {
  let mut preset_style = style_with([StyleDeclaration::text_decoration_color(ColorInput::Value(
    Color([255, 0, 0, 255]),
  ))]);
  let inline_style = style_with([
    StyleDeclaration::text_decoration_line(Some(TextDecorationLines::UNDERLINE)),
    StyleDeclaration::text_decoration_style(TextDecorationStyle::default()),
    StyleDeclaration::text_decoration_color(ColorInput::default()),
    StyleDeclaration::text_decoration_thickness(TextDecorationThickness::default()),
  ]);

  preset_style.merge_from(inline_style);

  let inherited = preset_style.inherit(&ComputedStyle::default());
  assert_eq!(inherited.text_decoration_color, ColorInput::default());
  assert_eq!(
    inherited.text_decoration_line,
    Some(TextDecorationLines::UNDERLINE)
  );
}

#[test]
fn test_merge_from_background_longhands_clear_lower_priority_background_color() {
  let mut preset_style = style_with([StyleDeclaration::background_color(ColorInput::Value(
    Color([255, 0, 0, 255]),
  ))]);
  let inline_style = style_with([
    StyleDeclaration::background_image(Some([BackgroundImage::None].into())),
    StyleDeclaration::background_position([PositionValue::default()].into()),
    StyleDeclaration::background_size([BackgroundSize::default()].into()),
    StyleDeclaration::background_repeat([BackgroundRepeat::default()].into()),
    StyleDeclaration::background_blend_mode([BlendMode::default()].into()),
    StyleDeclaration::background_color(ColorInput::default()),
    StyleDeclaration::background_clip(BackgroundClip::default()),
  ]);

  preset_style.merge_from(inline_style);

  let inherited = preset_style.inherit(&ComputedStyle::default());
  assert_eq!(inherited.background_color, ColorInput::default());
}

#[test]
fn test_needs_offscreen_compositing_for_clip_path_and_mask_image() {
  let mut style = ComputedStyle::default();
  assert!(!style.needs_offscreen_compositing());

  style.clip_path = BasicShape::from_css_str("inset(10px)").ok();
  assert!(style.needs_offscreen_compositing());

  style.clip_path = None;
  style.mask_image = Some([BackgroundImage::Url("https://example.com/mask.png".into())].into());
  assert!(style.needs_offscreen_compositing());
}

#[test]
fn test_is_z_index_applicable_matches_supported_scope() {
  let mut style = ComputedStyle {
    z_index: ZIndex::Integer(2),
    position: Position::Relative,
    ..Default::default()
  };
  assert!(style.is_z_index_applicable(false));

  style.position = Position::Absolute;
  assert!(style.is_z_index_applicable(false));

  // `z-index` does not apply to a static element.
  style.position = Position::Static;
  assert!(!style.is_z_index_applicable(false));

  style.position = Position::Relative;
  style.z_index = ZIndex::Auto;
  assert!(!style.is_z_index_applicable(false));
}

#[test]
fn test_creates_stacking_context_from_z_index_scope() {
  let mut style = ComputedStyle::default();
  let sizing = SizingContext {
    viewport: Viewport::new((1200, 630)),
    container_size: Size::NONE,
    container_read: Default::default(),
    font_size: 16.0,
    root_font_size: None,
    line_height: 0.0,
    root_line_height: None,
    calc_arena: Rc::new(CalcArena::default()),
  };
  let border_box = Size {
    width: 200.0,
    height: 100.0,
  };

  style.position = Position::Relative;
  style.z_index = ZIndex::Integer(1);
  assert!(style.creates_stacking_context(border_box.width, border_box.height, &sizing, false));

  style.position = Position::Absolute;
  assert!(style.creates_stacking_context(border_box.width, border_box.height, &sizing, false));
}

#[test]
fn test_offset_properties_parse_from_css() {
  let style = inherited_style_from_pairs(
    [
      ("offset-path", "path('M 0 0 L 100 0')"),
      ("offset-distance", "25%"),
      ("offset-rotate", "reverse 30deg"),
    ],
    &ComputedStyle::default(),
  );

  assert!(matches!(
    style.offset_path,
    Some(OffsetPath::Shape(BasicShape::Path(_)))
  ));
  assert_eq!(style.offset_distance, Length::Percentage(25.0));
  assert_eq!(style.offset_rotate, OffsetRotate::Reverse(Angle::new(30.0)));
}

#[test]
fn test_offset_shorthand_expands() {
  let style = inherited_style_from_pairs(
    [("offset", "ray(45deg closest-side) 10px auto / 100% 0%")],
    &ComputedStyle::default(),
  );

  assert!(matches!(style.offset_path, Some(OffsetPath::Ray(_))));
  assert_eq!(style.offset_distance, Length::Px(10.0));
  assert_eq!(style.offset_rotate, OffsetRotate::Auto(Angle::zero()));
  assert!(matches!(style.offset_anchor, OffsetAnchor::Position(_)));
}

#[test]
fn test_offset_path_moves_element_onto_path_and_creates_stacking_context() {
  let mut style = ComputedStyle::default();
  let sizing = SizingContext {
    viewport: Viewport::new((1200, 630)),
    container_size: Size::NONE,
    container_read: Default::default(),
    font_size: 16.0,
    root_font_size: None,
    line_height: 0.0,
    root_line_height: None,
    calc_arena: Rc::new(CalcArena::default()),
  };
  let border_box = Size {
    width: 100.0,
    height: 40.0,
  };

  style.offset_path = OffsetPath::from_css_str("path('M 0 0 L 100 0')").ok();
  style.offset_distance = Length::Percentage(50.0);

  assert!(style.creates_stacking_context(border_box.width, border_box.height, &sizing, false));

  // The default offset-anchor is transform-origin (the box center). At 50% the
  // anchor lands on the path midpoint (50, 0).
  let local = style.local_transform(border_box.width, border_box.height, &sizing);
  let (placed_x, placed_y) = local.transform_point(50.0, 20.0);
  assert!((placed_x - 50.0).abs() < 0.5, "x = {}", placed_x);
  assert!(placed_y.abs() < 0.5, "y = {}", placed_y);
}

#[test]
fn test_relative_position_participates_in_positioned_paint_bucket() {
  let style = ComputedStyle {
    position: Position::Relative,
    ..Default::default()
  };
  assert!(style.participates_in_positioned_paint_bucket(false));
}

#[test]
fn test_non_identity_transform_detection() {
  let mut style = ComputedStyle::default();
  let sizing = SizingContext {
    viewport: Viewport::new((1200, 630)),
    container_size: Size::NONE,
    container_read: Default::default(),
    font_size: 16.0,
    root_font_size: None,
    line_height: 0.0,
    root_line_height: None,
    calc_arena: Rc::new(CalcArena::default()),
  };
  let border_box = Size {
    width: 200.0,
    height: 100.0,
  };

  assert!(!style.has_non_identity_transform(border_box.width, border_box.height, &sizing));

  style.transform = Some([Transform::Rotate(Angle::new(0.0))].into());
  assert!(!style.has_non_identity_transform(border_box.width, border_box.height, &sizing));

  style.transform = Some([Transform::Rotate(Angle::new(10.0))].into());
  assert!(style.has_non_identity_transform(border_box.width, border_box.height, &sizing));
}

#[test]
fn test_transform_creates_stacking_context_without_offscreen_compositing() {
  let mut style = ComputedStyle::default();
  let sizing = SizingContext {
    viewport: Viewport::new((1200, 630)),
    container_size: Size::NONE,
    container_read: Default::default(),
    font_size: 16.0,
    root_font_size: None,
    line_height: 0.0,
    root_line_height: None,
    calc_arena: Rc::new(CalcArena::default()),
  };
  let border_box = Size {
    width: 200.0,
    height: 100.0,
  };

  style.transform = Some([Transform::Rotate(Angle::new(10.0))].into());

  assert!(style.creates_stacking_context(border_box.width, border_box.height, &sizing, false));
  assert!(!style.needs_offscreen_compositing());
}

#[test]
fn test_text_overflow_ellipsis_forces_single_line_clamp_on_nowrap() {
  let style = ComputedStyle {
    text_wrap_mode: TextWrapMode::NoWrap,
    text_overflow: TextOverflow::Ellipsis,
    ..Default::default()
  };

  assert_eq!(style.resolved_text_wrap_mode(), TextWrapMode::Wrap);
  assert_eq!(style.clamp_lines(), Some(1));
}

#[test]
fn test_position_absolute_blockifies_inline_display() {
  let mut style = style_with([
    StyleDeclaration::display(Display::Inline),
    StyleDeclaration::position(Position::Absolute),
  ])
  .inherit(&ComputedStyle::default());

  let sizing = SizingContext {
    viewport: Viewport::new((1200, 630)),
    container_size: Size::NONE,
    container_read: Default::default(),
    font_size: 16.0,
    root_font_size: None,
    line_height: 0.0,
    root_line_height: None,
    calc_arena: Rc::new(CalcArena::default()),
  };

  style.make_computed(&sizing);

  assert_eq!(style.display, Display::Block);
}

#[test]
fn test_inherited_em_text_lengths_are_computed_once() {
  let mut parent = style_with([
    StyleDeclaration::font_size(Length::Em(2.0).into()),
    StyleDeclaration::letter_spacing(Length::Em(1.0)),
    StyleDeclaration::line_height(LineHeight::Length(Length::Em(1.5))),
  ])
  .inherit(&ComputedStyle::default());
  parent.make_computed(&SizingContext {
    viewport: Viewport::new((1200, 630)),
    container_size: Size::NONE,
    container_read: Default::default(),
    font_size: 32.0,
    root_font_size: None,
    line_height: 0.0,
    root_line_height: None,
    calc_arena: Rc::new(CalcArena::default()),
  });

  let inherited_child = Style::default().inherit(&parent);
  let inherited_child_sizing = SizingContext {
    viewport: Viewport::new((1200, 630)),
    container_size: Size::NONE,
    container_read: Default::default(),
    font_size: 32.0,
    root_font_size: None,
    line_height: 0.0,
    root_line_height: None,
    calc_arena: Rc::new(CalcArena::default()),
  };
  let inherited_font_size = inherited_child
    .font_size
    .to_px(&inherited_child_sizing, inherited_child_sizing.font_size);
  assert_eq!(inherited_font_size, 32.0);

  let child_with_own_font_size =
    style_with([StyleDeclaration::font_size(Length::Px(10.0).into())]).inherit(&parent);
  let child_sizing = SizingContext {
    viewport: Viewport::new((1200, 630)),
    container_size: Size::NONE,
    container_read: Default::default(),
    font_size: 10.0,
    root_font_size: None,
    line_height: 0.0,
    root_line_height: None,
    calc_arena: Rc::new(CalcArena::default()),
  };

  let inherited_letter_spacing = child_with_own_font_size
    .letter_spacing
    .to_px(&child_sizing, child_sizing.font_size);
  assert_eq!(inherited_letter_spacing, 32.0);

  let inherited_line_height = match child_with_own_font_size.line_height {
    LineHeight::Length(length) => length.to_px(&child_sizing, child_sizing.font_size),
    _ => 0.0,
  };
  assert_eq!(inherited_line_height, 48.0);
}

#[test]
fn test_var_resolves_local_custom_property() {
  let style = inherited_style_from_pairs(
    [("--size", "24px"), ("width", "var(--size)")],
    &ComputedStyle::default(),
  );

  assert_eq!(style.width, Length::Px(24.0));
}

#[test]
fn test_var_uses_fallback_when_missing() {
  let style = inherited_style_from_pairs(
    [("width", "var(--missing, 18px)")],
    &ComputedStyle::default(),
  );

  assert_eq!(style.width, Length::Px(18.0));
}

#[test]
fn test_var_supports_nested_custom_properties() {
  let style = inherited_style_from_pairs(
    [
      ("--space-base", "12px"),
      ("--space", "var(--space-base)"),
      ("padding-left", "var(--space)"),
    ],
    &ComputedStyle::default(),
  );

  assert_eq!(style.padding_left, Length::Px(12.0));
}

#[test]
fn test_var_resolves_custom_property_declared_later_on_same_element() {
  let style = inherited_style_from_pairs(
    [("width", "var(--card-width)"), ("--card-width", "24px")],
    &ComputedStyle::default(),
  );

  assert_eq!(style.width, Length::Px(24.0));
}

#[test]
fn test_var_inherits_custom_properties_from_parent() {
  let parent = inherited_style_from_pairs([("--card-width", "320px")], &ComputedStyle::default());
  let child = inherited_style_from_pairs([("width", "var(--card-width)")], &parent);

  assert_eq!(child.width, Length::Px(320.0));
}

#[test]
fn test_var_drops_invalid_declaration_without_fallback() {
  let style = inherited_style_from_pairs([("width", "var(--missing)")], &ComputedStyle::default());

  assert_eq!(style.width, Length::default());
}

#[test]
fn test_var_uses_fallback_for_cycles() {
  let style = inherited_style_from_pairs(
    [
      ("--a", "var(--b)"),
      ("--b", "var(--a)"),
      ("width", "var(--a, 14px)"),
    ],
    &ComputedStyle::default(),
  );

  assert_eq!(style.width, Length::Px(14.0));
}

#[test]
fn test_var_resolves_inside_shorthand() {
  let style = inherited_style_from_pairs(
    [
      ("--block", "6px"),
      ("--inline", "10px"),
      ("padding", "var(--block) var(--inline)"),
    ],
    &ComputedStyle::default(),
  );

  assert_eq!(style.padding_top, Length::Px(6.0));
  assert_eq!(style.padding_right, Length::Px(10.0));
  assert_eq!(style.padding_bottom, Length::Px(6.0));
  assert_eq!(style.padding_left, Length::Px(10.0));
}

#[test]
fn test_var_rejects_non_custom_property_name() {
  let style = inherited_style_from_pairs([("width", "var(size, 18px)")], &ComputedStyle::default());

  assert_eq!(style.width, Length::default());
}

#[test]
fn test_var_allows_trailing_tokens_when_property_parser_is_loose() {
  let style = inherited_style_from_pairs(
    [("--size", "24px"), ("width", "var(--size) 10px")],
    &ComputedStyle::default(),
  );

  assert_eq!(style.width, Length::Px(24.0));
}

#[test]
fn test_var_rejects_missing_separator_in_function() {
  let style = inherited_style_from_pairs(
    [("--size", "24px"), ("width", "var(--size 18px)")],
    &ComputedStyle::default(),
  );

  assert_eq!(style.width, Length::default());
}

#[test]
fn test_var_supports_nested_fallback_chains() {
  let style = inherited_style_from_pairs(
    [
      ("--backup", "22px"),
      ("width", "var(--missing, var(--backup, 14px))"),
    ],
    &ComputedStyle::default(),
  );

  assert_eq!(style.width, Length::Px(22.0));
}

#[test]
fn test_var_resolves_inside_nested_functions() {
  let resolved = resolve_var("calc(var(--space) + 2px)", [("--space", "8px")]);

  assert_eq!(resolved.as_deref(), Some("calc(8px + 2px)"));
}

#[test]
fn test_var_resolves_inside_nested_blocks() {
  let resolved = resolve_var(
    "(var(--x)) [var(--y)] {var(--z)}",
    [("--x", "1px"), ("--y", "2px"), ("--z", "3px")],
  );

  assert_eq!(resolved.as_deref(), Some("(1px) [2px] {3px}"));
}

#[test]
fn test_var_gives_up_on_exponential_fan_out() {
  let mut custom_properties = HashMap::from([("--l0".to_owned(), "x".to_owned())]);
  for level in 1..=24 {
    custom_properties.insert(
      format!("--l{level}"),
      format!("var(--l{prev})var(--l{prev})", prev = level - 1),
    );
  }

  let resolved = resolve_var_references("var(--l24)", &custom_properties, &mut Vec::new());

  assert_eq!(resolved, None);
}

#[test]
fn test_var_gives_up_on_deeply_nested_blocks() {
  let specified_value = format!("{}1px{}", "(".repeat(200), ")".repeat(200));
  let resolved = resolve_var_references(&specified_value, &HashMap::new(), &mut Vec::new());

  assert_eq!(resolved, None);
}

#[test]
fn test_var_drops_declaration_when_substitution_stays_invalid() {
  let style = inherited_style_from_pairs(
    [("--size", "red"), ("width", "var(--size)")],
    &ComputedStyle::default(),
  );

  assert_eq!(style.width, Length::default());
}

#[test]
fn test_property_mask_iter_yields_only_set_bits_in_order() {
  let mut mask = PropertyMask::new();
  mask.insert(LonghandId::Width);
  mask.insert(LonghandId::Color);
  mask.insert(LonghandId::ALL[LonghandId::COUNT - 1]);

  let collected: Vec<_> = mask.iter().collect();
  let mut expected = [
    LonghandId::Width,
    LonghandId::Color,
    LonghandId::ALL[LonghandId::COUNT - 1],
  ];
  expected.sort_by_key(|id| id.index());

  assert_eq!(collected, expected);
  assert_eq!(PropertyMask::new().iter().count(), 0);
}

#[test]
fn test_var_defers_when_property_parser_accepts_a_prefix() {
  // https://github.com/kane50613/takumi/issues/712
  let style = inherited_style_from_pairs(
    [
      ("--onefifty", "150px"),
      ("background-position", "0 var(--onefifty)"),
    ],
    &ComputedStyle::default(),
  );

  assert_eq!(
    style.background_position.as_ref(),
    [PositionValue(SpacePair::from_pair(
      PositionComponent::Length(Length::Px(0.0)),
      PositionComponent::Length(Length::Px(150.0)),
    ))]
    .as_slice(),
  );
}

#[test]
fn test_border_radius_calc_infinity_parses_from_stylesheet_declaration() {
  let style = inherited_style_from_pairs(
    [("border-radius", "calc(infinity * 1px)")],
    &ComputedStyle::default(),
  );
  let sizing = SizingContext {
    viewport: Viewport::new((1200, 630)),
    container_size: Size::NONE,
    container_read: Default::default(),
    font_size: 16.0,
    root_font_size: None,
    line_height: 0.0,
    root_line_height: None,
    calc_arena: Rc::new(CalcArena::default()),
  };
  let radius = style.border_top_left_radius.x.to_px(
    &sizing,
    sizing.viewport.size.width.unwrap_or_default() as f32,
  );

  assert_eq!(
    radius,
    i32::MAX as f32,
    "parsed={:?}",
    style.border_top_left_radius.x
  );
}

#[test]
fn overflow_visible_paired_with_hidden_computes_to_hidden() {
  let mixed = inherited_style_from_pairs(
    [("overflow-x", "hidden"), ("overflow-y", "visible")],
    &ComputedStyle::default(),
  );
  assert_eq!(
    mixed.resolve_overflows(),
    SpacePair::from_pair(Overflow::Hidden, Overflow::Hidden)
  );

  let mirrored = inherited_style_from_pairs(
    [("overflow-x", "visible"), ("overflow-y", "hidden")],
    &ComputedStyle::default(),
  );
  assert_eq!(
    mirrored.resolve_overflows(),
    SpacePair::from_pair(Overflow::Hidden, Overflow::Hidden)
  );
}

#[test]
fn overflow_visible_paired_with_clip_stays_mixed() {
  let style = inherited_style_from_pairs(
    [("overflow-x", "clip"), ("overflow-y", "visible")],
    &ComputedStyle::default(),
  );

  assert_eq!(
    style.resolve_overflows(),
    SpacePair::from_pair(Overflow::Clip, Overflow::Visible)
  );
}
