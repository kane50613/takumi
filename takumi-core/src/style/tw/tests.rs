use std::{assert_matches, sync::Arc};

use super::*;
use crate::style::{ComputedStyle, LonghandId, Style, properties::BackgroundImage};

/// The declarations a utility produces without any variable defined, which is
/// what these assertions are about.
fn parse_property(token: &str) -> Option<Vec<StyleDeclaration>> {
  Some(match TailwindProperty::parse(token)? {
    TailwindProperty::VarUtility(var_utility) => var_utility.builtin_declarations(),
    property => expand(property),
  })
}

fn expect(property: TailwindProperty) -> Option<Vec<StyleDeclaration>> {
  Some(expand(property))
}

fn expand(property: TailwindProperty) -> Vec<StyleDeclaration> {
  property.expand_targets().into_vec()
}

/// The declarations a value's property produces, paired with its variant.
fn parse_value(token: &str) -> Option<(Vec<StyleDeclaration>, Option<Breakpoint>, bool)> {
  let value = TailwindValue::parse(token)?;

  Some((
    match value.property {
      TailwindProperty::VarUtility(var_utility) => var_utility.builtin_declarations(),
      property => expand(property),
    },
    value.breakpoint,
    value.important,
  ))
}

#[test]
fn test_box_sizing() {
  assert_eq!(
    parse_property("box-border"),
    expect(TailwindProperty::BoxSizing(BoxSizing::BorderBox))
  );
}

#[test]
fn test_parse_width() {
  assert_eq!(
    parse_property("w-64"),
    expect(TailwindProperty::Width(Length::from_spacing(64.0)))
  );
  assert_eq!(
    parse_property("h-32"),
    expect(TailwindProperty::Height(Length::from_spacing(32.0)))
  );
  assert_eq!(
    parse_property("justify-self-center"),
    expect(TailwindProperty::JustifySelf(AlignItems::Center))
  );
}

#[test]
fn test_parse_color() {
  assert_eq!(
    parse_property("text-black/30"),
    expect(TailwindProperty::Color(ColorInput::Value(Color([
      0,
      0,
      0,
      (0.3_f32 * 255.0).round() as u8
    ]))))
  );
}

#[test]
fn test_parse_red_500_color_utilities() {
  let cases: &[(&str, TailwindProperty)] = &[
    (
      "shadow-red-500",
      TailwindProperty::ShadowColor(TwVarColor::parse_tw("red-500").expect("palette colour")),
    ),
    (
      "text-shadow-red-500",
      TailwindProperty::TextShadowColor(TwVarColor::parse_tw("red-500").expect("palette colour")),
    ),
    (
      "decoration-red-500",
      TailwindProperty::TextDecorationColor(ColorInput::Value(Color([251, 44, 54, 255]))),
    ),
    (
      "bg-red-500",
      TailwindProperty::BackgroundColor(ColorInput::Value(Color([251, 44, 54, 255]))),
    ),
  ];
  for (input, expected) in cases {
    assert_eq!(parse_property(input), expect(expected.clone()), "{input}");
  }
}

#[test]
fn test_parse_text_decoration_lines() {
  assert_eq!(
    parse_property("underline"),
    expect(TailwindProperty::TextDecorationLine(
      TextDecorationLines::UNDERLINE
    ))
  );
  assert_eq!(
    parse_property("no-underline"),
    expect(TailwindProperty::TextDecorationLine(
      TextDecorationLines::empty()
    ))
  );
}

#[test]
fn test_parse_arbitrary_color() {
  assert_eq!(
    parse_property("text-[rgb(0, 191, 255)]"),
    expect(TailwindProperty::Color(ColorInput::Value(Color([
      0, 191, 255, 255
    ]))))
  );
}

#[test]
fn test_parse_arbitrary_mask_image_url() {
  assert_eq!(
    parse_property("mask-[url('https://example.com/logo.svg')]"),
    expect(TailwindProperty::MaskImage(BackgroundImage::Url(
      "https://example.com/logo.svg".into()
    )))
  );
}

#[test]
fn test_split_variant_ignores_colons_in_arbitrary_values() {
  assert_eq!(split_variant("sm:mt-0"), Some(("sm", "mt-0")));
  assert_eq!(split_variant("sm:hover:mt-0"), Some(("sm", "hover:mt-0")));
  assert_eq!(split_variant("mt-0"), None);
  assert_eq!(
    split_variant("mask-[url('https://example.com/a.svg')]"),
    None
  );
  assert_eq!(split_variant("grid-cols-[repeat(2,minmax(0,1fr))]"), None);
  assert_eq!(split_variant("[mask:foo]"), None);
  assert_eq!(
    split_variant("md:mask-[url('https://example.com/a.svg')]"),
    Some(("md", "mask-[url('https://example.com/a.svg')]"))
  );
  assert_eq!(split_variant(r#"content-['a:b]']"#), None);
  assert_eq!(split_variant(r#"content-["a\":b"]"#), None);
}

#[test]
fn test_decode_arbitrary_value() {
  assert_eq!(decode_arbitrary_value("3_1_auto"), "3 1 auto");
  assert_eq!(decode_arbitrary_value("10px"), "10px");
  assert_eq!(decode_arbitrary_value(r"foo\_bar"), "foo_bar");
  assert_eq!(
    decode_arbitrary_value("url('https://example.com/my_logo.svg')"),
    "url('https://example.com/my_logo.svg')"
  );
  // var()/theme(): first argument keeps underscores, later args don't.
  assert_eq!(decode_arbitrary_value("var(--my_color)"), "var(--my_color)");
  assert_eq!(
    decode_arbitrary_value("theme(--spacing_4)"),
    "theme(--spacing_4)"
  );
  assert_eq!(decode_arbitrary_value("var(--x,_a_b)"), "var(--x, a b)");
  assert_eq!(decode_arbitrary_value("calc(1_+_2)"), "calc(1 + 2)");
}

#[test]
fn test_extract_arbitrary_value_preserves_url_underscores() {
  assert_eq!(
    parse_property("mask-[url('https://example.com/my_logo.svg')]"),
    expect(TailwindProperty::MaskImage(BackgroundImage::Url(
      "https://example.com/my_logo.svg".into()
    )))
  );
}

#[test]
fn test_parse_value_arbitrary_url_with_scheme_colon() {
  let url_image = expand(TailwindProperty::MaskImage(BackgroundImage::Url(
    "https://example.com/a.svg".into(),
  )));

  assert_eq!(
    parse_value("mask-[url('https://example.com/a.svg')]"),
    Some((url_image.clone(), None, false))
  );

  assert_eq!(
    parse_value("md:mask-[url('https://example.com/a.svg')]"),
    Some((url_image, Breakpoint::parse("md"), false))
  );
}

#[test]
fn test_parse_arbitrary_flex_with_spaces() {
  assert_eq!(
    parse_property("flex-[3_1_auto]"),
    expect(TailwindProperty::Flex(Flex {
      grow: 3.0,
      shrink: 1.0,
      basis: Length::Auto,
    }))
  );
}

#[test]
fn test_parse_tailwind_animation_preset() {
  assert_matches!(
    TailwindProperty::parse("animate-spin"),
    Some(TailwindProperty::Animation(tw_animation))
      if tw_animation.token.as_deref() == Some("spin")
        && tw_animation.animations.as_ref() == [Animation {
          duration: AnimationTime::from_milliseconds(1000.0),
          timing_function: AnimationTimingFunction::Linear,
          iteration_count: AnimationIterationCount::Infinite,
          name: Some("spin".to_string()),
          ..Animation::default()
        }]
  );
}

#[test]
fn test_parse_tailwind_animation_arbitrary_value() {
  assert_matches!(
    TailwindProperty::parse("animate-[wiggle_1s_ease-in-out_infinite]"),
    Some(TailwindProperty::Animation(tw_animation))
      if tw_animation.token.is_none()
        && tw_animation.animations.as_ref() == [Animation {
          duration: AnimationTime::from_milliseconds(1000.0),
          timing_function: AnimationTimingFunction::EaseInOut,
          iteration_count: AnimationIterationCount::Infinite,
          name: Some("wiggle".to_string()),
          ..Animation::default()
        }]
  );
}

#[test]
fn test_parse_negative_margin() {
  assert_eq!(
    parse_property("-ml-4"),
    expect(TailwindProperty::MarginLeft(Length::from_spacing(-4.0)))
  );
}

#[test]
fn test_parse_border_radius() {
  assert_eq!(
    parse_property("rounded-xs"),
    expect(TailwindProperty::Rounded(TwRounded(Length::Rem(0.125))))
  );
  assert_eq!(
    parse_property("rounded-full"),
    expect(TailwindProperty::Rounded(TwRounded(Length::Px(9999.0))))
  );
}

#[test]
fn test_parse_font_size_with_arbitrary_line_height() {
  assert_eq!(
    parse_property("text-base/[12.34]"),
    expect(TailwindProperty::FontSize(TwFontSize {
      font_size: (Length::Rem(1.0).into()),
      line_height: Some(LineHeight::Unitless(12.34)),
    }))
  );
}

#[test]
fn test_parse_border_width() {
  assert_eq!(
    parse_property("border"),
    expect(TailwindProperty::BorderDefault)
  );
  assert_eq!(
    parse_property("border-b"),
    expect(TailwindProperty::BorderBottomWidth(LineWidth::Length(
      Length::Px(1.0)
    )))
  );
  assert_eq!(
    parse_property("border-y"),
    expect(TailwindProperty::BorderYWidth(LineWidth::Length(
      Length::Px(1.0)
    )))
  );
  assert_eq!(
    parse_property("border-t-2"),
    expect(TailwindProperty::BorderTopWidth(LineWidth::Length(
      Length::Px(2.0)
    )))
  );
  assert_eq!(
    parse_property("border-x-4"),
    expect(TailwindProperty::BorderXWidth(LineWidth::Length(
      Length::Px(4.0)
    )))
  );
  assert_eq!(
    parse_property("border-solid"),
    expect(TailwindProperty::BorderStyle(BorderStyle::Solid))
  );
  assert_eq!(
    parse_property("border-dashed"),
    expect(TailwindProperty::BorderStyle(BorderStyle::Dashed))
  );
  assert_eq!(
    parse_property("border-dotted"),
    expect(TailwindProperty::BorderStyle(BorderStyle::Dotted))
  );
  assert_eq!(
    parse_property("border-none"),
    expect(TailwindProperty::BorderStyle(BorderStyle::None))
  );
}

#[test]
fn test_parse_outline() {
  assert_eq!(
    parse_property("outline"),
    expect(TailwindProperty::OutlineDefault)
  );
  assert_eq!(
    parse_property("outline-2"),
    expect(TailwindProperty::OutlineWidth(LineWidth::Length(
      Length::Px(2.0)
    )))
  );
  assert_eq!(
    parse_property("outline-red-500"),
    expect(TailwindProperty::OutlineColor(ColorInput::Value(Color([
      251, 44, 54, 255
    ]))))
  );
  assert_eq!(
    parse_property("outline-solid"),
    expect(TailwindProperty::OutlineStyle(BorderStyle::Solid))
  );
  assert_eq!(
    parse_property("outline-dashed"),
    expect(TailwindProperty::OutlineStyle(BorderStyle::Dashed))
  );
  assert_eq!(
    parse_property("outline-offset-4"),
    expect(TailwindProperty::OutlineOffset(LineWidth::Length(
      Length::Px(4.0)
    )))
  );
  assert_eq!(
    parse_property("outline-none"),
    expect(TailwindProperty::OutlineStyle(BorderStyle::None))
  );
}

#[test]
fn test_parse_col_end() {
  assert_eq!(
    parse_property("col-end-1"),
    expect(TailwindProperty::GridColumnEnd(GridPlacement::Line(1)))
  );
}

#[test]
fn test_grid_column_start_emits_only_start_longhand() {
  let values = TailwindValues::from_str("col-start-2").expect("tailwind values should parse");
  let declarations = values.into_declaration_block(Viewport::new((100, 100)), &Default::default());

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::grid_column_start(GridPlacement::Line(2))]
  );
}

#[test]
fn test_grid_row_end_emits_only_end_longhand() {
  let values = TailwindValues::from_str("row-end-3").expect("tailwind values should parse");
  let declarations = values.into_declaration_block(Viewport::new((100, 100)), &Default::default());

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::grid_row_end(GridPlacement::Line(3))]
  );
}

#[test]
fn test_grid_longhand_importance_is_tracked_per_side() {
  let values =
    TailwindValues::from_str("col-end-3 !col-start-2").expect("tailwind values should parse");
  let declarations = values.into_declaration_block(Viewport::new((100, 100)), &Default::default());

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::grid_column_end(GridPlacement::Line(3)),
      &StyleDeclaration::grid_column_start(GridPlacement::Line(2)),
    ]
  );
  assert!(
    declarations
      .importance
      .longhands
      .contains(&LonghandId::GridColumnStart)
  );
  assert!(
    !declarations
      .importance
      .longhands
      .contains(&LonghandId::GridColumnEnd)
  );
}

#[test]
fn test_parse_overflow_clip() {
  let cases: &[(&str, TailwindProperty)] = &[
    ("overflow-clip", TailwindProperty::Overflow(Overflow::Clip)),
    (
      "overflow-x-clip",
      TailwindProperty::OverflowX(Overflow::Clip),
    ),
    (
      "overflow-y-clip",
      TailwindProperty::OverflowY(Overflow::Clip),
    ),
  ];
  for (input, expected) in cases {
    assert_eq!(parse_property(input), expect(expected.clone()), "{input}");
  }
}

#[test]
fn test_comprehensive_mappings() {
  // Test various prefix mappings to ensure they're working
  let should_parse = vec![
    // Layout
    "flex",
    "grid",
    "hidden",
    "block",
    "inline",
    // SizingContext
    "w-4",
    "h-8",
    "size-12",
    "min-w-0",
    "max-h-96",
    // Spacing
    "m-2",
    "mx-4",
    "my-auto",
    "mt-8",
    "mr-6",
    "mb-4",
    "ml-2",
    "p-3",
    "px-5",
    "py-2",
    "pt-1",
    "pr-4",
    "pb-3",
    "pl-2",
    // Colors
    "text-red-500",
    "bg-blue-200",
    "border-gray-300",
    // Typography
    "text-sm",
    "font-bold",
    "font-stretch-condensed",
    "font-stretch-ultra-expanded",
    "font-stretch-75%",
    "uppercase",
    "tracking-wide",
    "animate-spin",
    "animate-[wiggle_1s_ease-in-out_infinite]",
    // Flexbox
    "justify-center",
    "items-end",
    "self-start",
    "flex-grow",
    "shrink",
    // Borders
    "border",
    "border-t-2",
    "border-solid",
    "border-none",
    "outline",
    "outline-2",
    "outline-red-500",
    "outline-solid",
    "outline-offset-2",
    "rounded-lg",
    // Transforms
    "rotate-45",
    "scale-75",
    "translate-x-4",
    // Grid
    "grid-cols-3",
    "col-span-2",
    // Backdrop Filters
    "backdrop-blur-md",
    "backdrop-brightness-50",
    "backdrop-contrast-125",
    "backdrop-grayscale",
    "backdrop-hue-rotate-90",
    "backdrop-invert",
    "backdrop-opacity-50",
    "backdrop-saturate-200",
    "backdrop-sepia",
    "backdrop-filter-[blur(4px)_brightness(0.5)]",
  ];

  let should_not_parse = vec!["nonexistent-class", "invalid-prefix-1", "random-string"];

  for class in should_parse {
    assert!(
      parse_property(class).is_some(),
      "Expected '{}' to parse successfully",
      class
    );
  }

  for class in should_not_parse {
    assert!(
      parse_property(class).is_none(),
      "Expected '{}' to fail parsing",
      class
    );
  }
}

#[test]
fn test_breakpoint_matches() {
  let viewport = Viewport::new((1000, 1000));

  assert!(Breakpoint::parse("sm").is_some_and(|bp| bp.matches(viewport, &Default::default())));
}

#[test]
fn test_breakpoint_does_not_match() {
  let viewport = Viewport::new((1000, 1000));

  // 80 * 16 = 1280 > 1000
  assert!(Breakpoint::parse("xl").is_some_and(|bp| !bp.matches(viewport, &Default::default())));
}

#[test]
fn test_value_parsing() {
  assert_eq!(
    parse_value("md:!mt-4"),
    Some((
      expand(TailwindProperty::MarginTop(Length::Rem(1.0))),
      Breakpoint::parse("md"),
      true
    ))
  );
}

#[test]
fn test_values_sorting() {
  let values = TailwindValues::from_str("md:!mt-4 sm:mt-8 !mt-12 mt-16")
    .expect("tailwind values should parse");

  let order = values
    .inner
    .iter()
    .map(|value| {
      let declarations = match &value.property {
        TailwindProperty::VarUtility(var_utility) => var_utility.builtin_declarations(),
        property => expand(property.clone()),
      };

      (declarations, value.breakpoint.clone(), value.important)
    })
    .collect::<Vec<_>>();

  assert_eq!(
    order,
    vec![
      // mt-16
      (
        expand(TailwindProperty::MarginTop(Length::Rem(4.0))),
        None,
        false
      ),
      // sm:mt-8
      (
        expand(TailwindProperty::MarginTop(Length::Rem(2.0))),
        Breakpoint::parse("sm"),
        false
      ),
      // !mt-12
      (
        expand(TailwindProperty::MarginTop(Length::Rem(3.0))),
        None,
        true
      ),
      // md:!mt-4
      (
        expand(TailwindProperty::MarginTop(Length::Rem(1.0))),
        Breakpoint::parse("md"),
        true
      ),
    ]
  )
}

#[test]
fn test_filters_append() {
  use crate::style::properties::Filter;

  let values = TailwindValues::from_str("blur-sm brightness-150 contrast-125")
    .expect("tailwind values should parse");
  let viewport = Viewport::new((100, 100));

  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());

  assert_eq!(
    style.filter,
    vec![
      Filter::Blur(Length::Px(8.0)),
      Filter::Brightness(PercentageNumber(1.5)),
      Filter::Contrast(PercentageNumber(1.25))
    ]
  )
}

#[test]
fn test_transform_utilities_resolve_to_standard_longhands() {
  let values = TailwindValues::from_str("translate-x-4 translate-y-8 scale-75 scale-x-50")
    .expect("tailwind values should parse");
  let viewport = Viewport::new((100, 100));

  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());

  assert_eq!(
    style.translate,
    SpacePair::from_pair(Length::Rem(1.0), Length::Rem(2.0))
  );
  assert_eq!(
    style.scale,
    SpacePair::from_pair(PercentageNumber(0.5), PercentageNumber(0.75))
  );
}

#[test]
fn test_parse_blend_mode() {
  assert_eq!(
    parse_property("mix-blend-multiply"),
    expect(TailwindProperty::MixBlendMode(BlendMode::Multiply))
  );
  assert_eq!(
    parse_property("bg-blend-screen"),
    expect(TailwindProperty::BackgroundBlendMode(BlendMode::Screen))
  );
}
#[test]
fn test_parse_vertical_align() {
  let keywords: &[(&str, VerticalAlignKeyword)] = &[
    ("align-baseline", VerticalAlignKeyword::Baseline),
    ("align-top", VerticalAlignKeyword::Top),
    ("align-middle", VerticalAlignKeyword::Middle),
    ("align-bottom", VerticalAlignKeyword::Bottom),
    ("align-text-top", VerticalAlignKeyword::TextTop),
    ("align-text-bottom", VerticalAlignKeyword::TextBottom),
    ("align-sub", VerticalAlignKeyword::Sub),
    ("align-super", VerticalAlignKeyword::Super),
  ];
  for (input, kw) in keywords {
    assert_eq!(
      parse_property(input),
      expect(TailwindProperty::VerticalAlign(VerticalAlign::Keyword(*kw))),
      "{input}"
    );
  }
  // Arbitrary-value lengths
  assert_eq!(
    parse_property("align-[10px]"),
    expect(TailwindProperty::VerticalAlign(VerticalAlign::Length(
      Length::Px(10.0)
    )))
  );
  assert_eq!(
    parse_property("align-[25%]"),
    expect(TailwindProperty::VerticalAlign(VerticalAlign::Length(
      Length::Percentage(25.0)
    )))
  );
  assert_eq!(
    parse_property("align-[-0.5em]"),
    expect(TailwindProperty::VerticalAlign(VerticalAlign::Length(
      Length::Em(-0.5)
    )))
  );
}

#[test]
fn test_parse_decoration_thickness() {
  assert_eq!(
    parse_property("decoration-4"),
    expect(TailwindProperty::TextDecorationThickness(
      TextDecorationThickness::Length(Length::Px(4.0))
    ))
  );
  assert_eq!(
    parse_property("decoration-auto"),
    expect(TailwindProperty::TextDecorationThickness(
      TextDecorationThickness::Length(Length::Auto)
    ))
  );
  assert_eq!(
    parse_property("decoration-from-font"),
    expect(TailwindProperty::TextDecorationThickness(
      TextDecorationThickness::FromFont
    ))
  );
  assert_eq!(
    parse_property("decoration-[3px]"),
    expect(TailwindProperty::TextDecorationThickness(
      TextDecorationThickness::Length(Length::Px(3.0))
    ))
  );
}

#[test]
fn test_linear_gradient_apply() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("bg-linear-to-r from-red-500 via-green-500 to-blue-500")
    .expect("tailwind values should parse");

  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());

  assert_eq!(
    style.background_image,
    Some(
      [BackgroundImage::Linear(LinearGradient {
        repeating: false,
        direction: crate::style::LinearGradientDirection::Angle(Angle::new(90.0)),
        interpolation: ColorInterpolationMethod::default(),
        stops: [
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([251, 44, 54, 255])),
            hint: Some(StopPosition(Length::Percentage(0.0))),
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([0, 201, 80, 255])),
            hint: Some(StopPosition(Length::Percentage(50.0))),
          },
          GradientStop::ColorHint {
            color: ColorInput::Value(Color([43, 127, 255, 255])),
            hint: Some(StopPosition(Length::Percentage(100.0))),
          },
        ]
        .into(),
      })]
      .into()
    )
  );
}

#[test]
fn test_shadow_color_overrides_shadow_preset_in_any_order() {
  let viewport = Viewport::new((100, 100));

  for classes in ["shadow-md shadow-red-500", "shadow-red-500 shadow-md"] {
    let values = TailwindValues::from_str(classes)
      .unwrap_or_else(|_| panic!("tailwind values should parse: {classes}"));
    let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
      .inherit(&ComputedStyle::default());

    assert_eq!(
      style.box_shadow,
      Some(
        [
          BoxShadow {
            inset: false,
            offset_x: Length::Px(0.0),
            offset_y: Length::Px(4.0),
            blur_radius: Length::Px(6.0),
            spread_radius: Length::Px(-1.0),
            color: ColorInput::Value(Color([251, 44, 54, 255])),
          },
          BoxShadow {
            inset: false,
            offset_x: Length::Px(0.0),
            offset_y: Length::Px(2.0),
            blur_radius: Length::Px(4.0),
            spread_radius: Length::Px(-2.0),
            color: ColorInput::Value(Color([251, 44, 54, 255])),
          },
        ]
        .into()
      )
    );
  }
}

#[test]
fn test_text_shadow_color_overrides_preset_in_any_order() {
  let viewport = Viewport::new((100, 100));

  for classes in [
    "text-shadow-sm text-shadow-red-500",
    "text-shadow-red-500 text-shadow-sm",
  ] {
    let values = TailwindValues::from_str(classes)
      .unwrap_or_else(|_| panic!("tailwind values should parse: {classes}"));
    let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
      .inherit(&ComputedStyle::default());

    assert_eq!(
      style.text_shadow,
      Some(
        [
          TextShadow {
            offset_x: Length::Px(0.0),
            offset_y: Length::Px(1.0),
            blur_radius: Length::Px(0.0),
            color: ColorInput::Value(Color([251, 44, 54, 255])),
          },
          TextShadow {
            offset_x: Length::Px(0.0),
            offset_y: Length::Px(1.0),
            blur_radius: Length::Px(1.0),
            color: ColorInput::Value(Color([251, 44, 54, 255])),
          },
          TextShadow {
            offset_x: Length::Px(0.0),
            offset_y: Length::Px(2.0),
            blur_radius: Length::Px(2.0),
            color: ColorInput::Value(Color([251, 44, 54, 255])),
          },
        ]
        .into()
      )
    );
  }
}

#[test]
fn test_bare_rounded_is_radius_sm() {
  assert_eq!(
    parse_property("rounded"),
    expect(TailwindProperty::Rounded(TwRounded(Length::Rem(0.25))))
  );
}

#[test]
fn test_negative_color_is_rejected() {
  assert_eq!(parse_property("-bg-red-500"), None);
  assert_eq!(parse_property("-text-blue-500"), None);
}

#[test]
fn test_negative_grid_line() {
  assert_eq!(
    parse_property("-col-start-1"),
    expect(TailwindProperty::GridColumnStart(GridPlacement::Line(-1)))
  );
}

#[test]
fn test_logical_resolves_to_physical_ltr() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("ms-4 me-2 ps-3 pe-1").unwrap();
  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());
  assert_eq!(style.margin_left, Length::from_spacing(4.0));
  assert_eq!(style.margin_right, Length::from_spacing(2.0));
  assert_eq!(style.padding_left, Length::from_spacing(3.0));
  assert_eq!(style.padding_right, Length::from_spacing(1.0));
}

#[test]
fn test_logical_physical_cascade_order_ltr() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("ms-2 ml-4").unwrap();
  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());
  assert_eq!(style.margin_left, Length::from_spacing(4.0));

  let values = TailwindValues::from_str("ml-4 ms-2").unwrap();
  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());
  assert_eq!(style.margin_left, Length::from_spacing(2.0));
}

#[test]
fn test_logical_resolves_to_physical_rtl() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("ms-4 me-2 ps-3 pe-1").unwrap();
  let mut block = values.into_declaration_block(viewport, &Default::default());
  block.push(StyleDeclaration::direction(Direction::Rtl), false);
  let style = Style::from(block).inherit(&ComputedStyle::default());
  assert_eq!(style.margin_right, Length::from_spacing(4.0));
  assert_eq!(style.margin_left, Length::from_spacing(2.0));
  assert_eq!(style.padding_right, Length::from_spacing(3.0));
  assert_eq!(style.padding_left, Length::from_spacing(1.0));
}

#[test]
fn test_logical_resolves_when_direction_declared_after() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("ms-4").unwrap();
  let mut block = values.into_declaration_block(viewport, &Default::default());
  block.push(StyleDeclaration::direction(Direction::Rtl), false);
  let style = Style::from(block).inherit(&ComputedStyle::default());
  assert_eq!(style.margin_right, Length::from_spacing(4.0));
  assert_eq!(style.margin_left, Length::Px(0.0));
}

#[test]
fn test_filter_none_clears_previous_filters() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("blur-sm brightness-150 filter-none").unwrap();
  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());
  assert_eq!(style.filter, Filters::default());

  let values = TailwindValues::from_str("backdrop-blur-sm backdrop-filter-none").unwrap();
  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());
  assert_eq!(style.backdrop_filter, Filters::default());
}

#[test]
fn test_font_numeric_weight() {
  assert_eq!(
    parse_property("font-700"),
    expect(TailwindProperty::FontWeight(FontWeight::from(700.0)))
  );
}

#[test]
fn test_line_clamp_none() {
  assert_eq!(
    parse_property("line-clamp-none"),
    expect(TailwindProperty::LineClamp(LineClamp::default()))
  );
}

#[test]
fn test_bg_auto() {
  assert_eq!(
    parse_property("bg-auto"),
    expect(TailwindProperty::BackgroundSize(BackgroundSize::Explicit {
      width: Length::Auto,
      height: Length::Auto,
    }))
  );
}

#[test]
fn test_bg_repeat_v4_names() {
  assert_eq!(
    parse_property("bg-repeat-round"),
    expect(TailwindProperty::BackgroundRepeat(BackgroundRepeat::round()))
  );
  assert_eq!(
    parse_property("bg-repeat-space"),
    expect(TailwindProperty::BackgroundRepeat(BackgroundRepeat::space()))
  );
}

#[test]
fn test_grid_cols_none() {
  assert_eq!(
    parse_property("grid-cols-none"),
    expect(TailwindProperty::GridTemplateColumns(TwGridTemplate(
      GridTemplateComponents::default()
    )))
  );
}

#[test]
fn test_col_auto_row_auto() {
  assert_eq!(
    parse_property("col-auto"),
    expect(TailwindProperty::GridColumn(GridLine {
      start: GridPlacement::auto(),
      end: GridPlacement::auto(),
    }))
  );
  assert_eq!(
    parse_property("row-auto"),
    expect(TailwindProperty::GridRow(GridLine {
      start: GridPlacement::auto(),
      end: GridPlacement::auto(),
    }))
  );
}

#[test]
fn test_shadow_md_is_composite() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("shadow-md").unwrap();
  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());
  assert_eq!(style.box_shadow.as_ref().map(|s| s.len()), Some(2));
}

#[test]
fn test_text_shadow_sm_is_composite() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("text-shadow-sm").unwrap();
  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());
  assert_eq!(style.text_shadow.as_ref().map(|s| s.len()), Some(3));
}

#[test]
fn test_shadow_none_overrides_color_in_either_order() {
  let viewport = Viewport::new((100, 100));
  for classes in ["shadow-none shadow-red-500", "shadow-red-500 shadow-none"] {
    let values = TailwindValues::from_str(classes).unwrap();
    let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
      .inherit(&ComputedStyle::default());
    assert_eq!(style.box_shadow, None, "case: {classes}");
  }
}

#[test]
fn test_bg_conic_standalone() {
  assert_eq!(
    parse_property("bg-conic"),
    expect(TailwindProperty::BgConicAngle(Angle::zero()))
  );
}

#[test]
fn test_gradient_stop_position_is_used_in_apply() {
  let viewport = Viewport::new((100, 100));
  let values =
    TailwindValues::from_str("bg-linear-to-r from-red-500 from-10% to-blue-500 to-80%").unwrap();
  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());
  let images = style.background_image.as_deref().unwrap();
  let [BackgroundImage::Linear(gradient)] = images else {
    panic!("expected a single linear gradient");
  };
  let positions: Vec<Length> = gradient
    .stops
    .iter()
    .filter_map(|s| match s {
      GradientStop::ColorHint {
        hint: Some(StopPosition(pos)),
        ..
      } => Some(*pos),
      _ => None,
    })
    .collect();
  assert_eq!(
    positions,
    vec![Length::Percentage(10.0), Length::Percentage(80.0)]
  );
}

#[test]
fn test_border_width_implies_solid_and_per_side_color() {
  let viewport = Viewport::new((100, 100));
  let computed = |tw: &str| {
    Style::from(
      TailwindValues::from_str(tw)
        .expect("tailwind values should parse")
        .into_declaration_block(viewport, &Default::default()),
    )
    .inherit(&ComputedStyle::default())
  };

  let top = computed("border-t-8");
  assert_eq!(top.border_top_width, LineWidth::Length(Length::Px(8.0)));
  assert_eq!(top.border_top_style, BorderStyle::Solid);
  assert_eq!(top.border_bottom_style, BorderStyle::None);

  let all = computed("border-4");
  assert_eq!(all.border_top_style, BorderStyle::Solid);
  assert_eq!(all.border_right_style, BorderStyle::Solid);
  assert_eq!(all.border_bottom_style, BorderStyle::Solid);
  assert_eq!(all.border_left_style, BorderStyle::Solid);

  let dashed = computed("border-2 border-dashed");
  assert_eq!(dashed.border_top_style, BorderStyle::Dashed);

  assert_eq!(
    parse_property("border-t-blue-500"),
    expect(TailwindProperty::BorderTopColor(ColorInput::Value(Color(
      [43, 127, 255, 255]
    ))))
  );

  let bar = computed("border-t-8 border-t-blue-500");
  assert_eq!(bar.border_top_width, LineWidth::Length(Length::Px(8.0)));
  assert_eq!(bar.border_top_style, BorderStyle::Solid);
  assert_eq!(
    bar.border_top_color,
    ColorInput::Value(Color([43, 127, 255, 255]))
  );
}

/// Mirrors the utilities documented at https://tailwindcss.com/docs/list-style-type
#[test]
fn test_parse_list_utilities() {
  for (token, expected) in [
    ("list-item", TailwindProperty::Display(Display::ListItem)),
    (
      "list-disc",
      TailwindProperty::ListStyleType(ListStyleType::Disc),
    ),
    (
      "list-decimal",
      TailwindProperty::ListStyleType(ListStyleType::Decimal),
    ),
    (
      "list-none",
      TailwindProperty::ListStyleType(ListStyleType::None),
    ),
    (
      "list-[upper-roman]",
      TailwindProperty::ListStyleType(ListStyleType::UpperRoman),
    ),
    (
      "list-inside",
      TailwindProperty::ListStylePosition(ListStylePosition::Inside),
    ),
    (
      "list-outside",
      TailwindProperty::ListStylePosition(ListStylePosition::Outside),
    ),
    (
      "list-image-none",
      TailwindProperty::ListStyleImage(ListStyleImage::default()),
    ),
  ] {
    assert_eq!(
      parse_property(token),
      expect(expected),
      "failed for {token}"
    );
  }

  assert_matches!(
    TailwindProperty::parse("list-image-[url(marker.png)]"),
    Some(TailwindProperty::ListStyleImage(image))
      if matches!(image.image(), Some(BackgroundImage::Url(url)) if &**url == "marker.png")
  );
}

/// Stands in for the `:root` rule a stylesheet would supply.
fn root_with(variables: &[(&str, &str)]) -> ComputedStyle {
  let mut root = ComputedStyle::default();
  let properties = Arc::make_mut(&mut root.custom_properties);

  for (name, value) in variables {
    properties.insert((*name).to_owned(), (*value).to_owned());
  }

  root
}

/// Logical sides resolve to a physical one at apply time, after substitution.
#[test]
fn test_logical_side_reads_a_css_variable() {
  let values =
    TailwindValues::from_str("ms-gutter ps-gutter").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--spacing-gutter", "3rem")]));

  assert_eq!(computed.margin_left, Length::Rem(3.0));
  assert_eq!(computed.padding_left, Length::Rem(3.0));
}

#[test]
fn test_corner_radius_reads_a_css_variable() {
  let values = TailwindValues::from_str("rounded-t-card").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--radius-card", "12px")]));

  assert_eq!(computed.border_top_left_radius.x, Length::Px(12.0));
}

#[test]
fn test_shadow_preset_shape_reads_a_css_variable() {
  use crate::style::properties::BoxShadow;

  let values = TailwindValues::from_str("shadow-md").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--shadow-md", "0 5px 5px #ff0000")]));

  assert_eq!(
    computed.box_shadow.as_deref(),
    Some(
      &[BoxShadow {
        inset: false,
        offset_x: Length::Px(0.0),
        offset_y: Length::Px(5.0),
        blur_radius: Length::Px(5.0),
        spread_radius: Length::Px(0.0),
        color: ColorInput::Value(Color::from_rgb(0xff0000)),
      }][..]
    )
  );
}

#[test]
fn test_text_shadow_preset_shape_reads_a_css_variable() {
  use crate::style::properties::TextShadow;

  let values = TailwindValues::from_str("text-shadow-sm").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--text-shadow-sm", "1px 1px 0 #00ff00")]));

  assert_eq!(
    computed.text_shadow.as_deref(),
    Some(
      &[TextShadow {
        offset_x: Length::Px(1.0),
        offset_y: Length::Px(1.0),
        blur_radius: Length::Px(0.0),
        color: ColorInput::Value(Color::from_rgb(0x00ff00)),
      }][..]
    )
  );
}

#[test]
fn test_breakpoint_reads_a_css_variable() {
  let overrides = BreakpointOverrides::from([("md".to_owned(), Length::Px(400.0))]);
  let viewport = Viewport::new((500, 500));
  let values = TailwindValues::from_str("md:mt-4").expect("tailwind values should parse");

  // 500px sits below the built-in 48rem, so only the override applies it.
  let unthemed = Style::from(
    values
      .clone()
      .into_declaration_block(viewport, &Default::default()),
  )
  .inherit(&ComputedStyle::default());
  assert_eq!(unthemed.margin_top, Length::Px(0.0));

  let themed = Style::from(values.into_declaration_block(viewport, &overrides))
    .inherit(&ComputedStyle::default());
  assert_eq!(themed.margin_top, Length::Rem(1.0));
}

#[test]
fn test_unknown_breakpoint_token_reads_a_css_variable() {
  let overrides = BreakpointOverrides::from([("3xl".to_owned(), Length::Px(400.0))]);
  let viewport = Viewport::new((500, 500));
  let values = TailwindValues::from_str("3xl:mt-4").expect("tailwind values should parse");

  let unthemed = Style::from(
    values
      .clone()
      .into_declaration_block(viewport, &Default::default()),
  )
  .inherit(&ComputedStyle::default());
  assert_eq!(unthemed.margin_top, Length::Px(0.0));

  let themed = Style::from(values.into_declaration_block(viewport, &overrides))
    .inherit(&ComputedStyle::default());
  assert_eq!(themed.margin_top, Length::Rem(1.0));
}

#[test]
fn test_animate_preset_reads_a_css_variable() {
  let values = TailwindValues::from_str("animate-spin").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--animate-spin", "wobble 2s ease-in 3")]));

  assert_eq!(
    computed.animation_name.as_ref(),
    [Some("wobble".to_string())]
  );
  assert_eq!(
    computed.animation_duration.as_ref(),
    [AnimationTime::from_milliseconds(2000.0)]
  );
}

#[test]
fn test_animate_preset_falls_back_to_the_builtin_animation() {
  let values = TailwindValues::from_str("animate-spin").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&ComputedStyle::default());

  assert_eq!(computed.animation_name.as_ref(), [Some("spin".to_string())]);
  assert_eq!(
    computed.animation_duration.as_ref(),
    [AnimationTime::from_milliseconds(1000.0)]
  );
  assert_eq!(
    computed.animation_iteration_count.as_ref(),
    [AnimationIterationCount::Infinite]
  );
}

#[test]
fn test_unknown_animate_token_reads_a_css_variable() {
  let values = TailwindValues::from_str("animate-wiggle").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let themed = style
    .clone()
    .inherit(&root_with(&[("--animate-wiggle", "wiggle 1s linear")]));
  assert_eq!(themed.animation_name.as_ref(), [Some("wiggle".to_string())]);

  let unthemed = style.inherit(&ComputedStyle::default());
  assert_eq!(
    unthemed.animation_name,
    ComputedStyle::default().animation_name
  );
}

#[test]
fn test_blur_preset_reads_a_css_variable() {
  use crate::style::properties::Filter;

  let values =
    TailwindValues::from_str("blur-md backdrop-blur-md").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--blur-md", "20px")]));

  assert_eq!(computed.filter, vec![Filter::Blur(Length::Px(20.0))]);
  assert_eq!(
    computed.backdrop_filter,
    vec![Filter::Blur(Length::Px(20.0))]
  );
}

#[test]
fn test_blur_preset_falls_back_to_the_builtin_radius() {
  use crate::style::properties::Filter;

  let values = TailwindValues::from_str("blur-md").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&ComputedStyle::default());

  assert_eq!(computed.filter, vec![Filter::Blur(Length::Px(12.0))]);
}

#[test]
fn test_drop_shadow_preset_reads_a_css_variable() {
  use crate::style::properties::{Filter, TextShadow};

  let values = TailwindValues::from_str("drop-shadow-md").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--drop-shadow-md", "0 5px 5px #ff0000")]));

  assert_eq!(
    computed.filter,
    vec![Filter::DropShadow(TextShadow {
      offset_x: Length::Px(0.0),
      offset_y: Length::Px(5.0),
      blur_radius: Length::Px(5.0),
      color: ColorInput::Value(Color::from_rgb(0xff0000)),
    })]
  );
}

/// The built-in scale has to stay reachable as a variable, not just as a value
/// baked into the utility.
#[test]
fn test_builtin_token_is_overridable() {
  let values = TailwindValues::from_str("text-red-500").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--color-red-500", "#00a63e")]));

  assert_eq!(computed.color, ColorInput::Value(Color::from_rgb(0x00a63e)));
}

#[test]
fn test_spacing_step_scales_numeric_utilities() {
  let values = TailwindValues::from_str("ms-4").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--spacing", "0.5rem")]));
  let sizing = SizingContext::builder()
    .viewport(Viewport::new((100, 100)))
    .build();

  // `calc(var(--spacing) * 4)` stays a calc, so read it as the length it is.
  assert_eq!(computed.margin_left.to_px(&sizing, 0.0), 32.0);
}

/// A prefix that reads two namespaces emits one variable per group, so the same
/// token can mean a size or a colour depending on which one is defined.
#[test]
fn test_overloaded_prefix_reads_either_namespace() {
  let values = TailwindValues::from_str("text-brand").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let sized = style
    .clone()
    .inherit(&root_with(&[("--text-brand", "2rem")]));
  let coloured = style.inherit(&root_with(&[("--color-brand", "#5b21b6")]));

  assert_eq!(sized.font_size, FontSize::Length(Length::Rem(2.0)));
  assert_eq!(coloured.color, ColorInput::Value(Color::from_rgb(0x5b21b6)));
}

/// An opacity modifier mixes the variable rather than falling back to the
/// built-in scale, which is how Tailwind compiles it.
#[test]
fn test_opacity_modifier_mixes_the_variable() {
  let values = TailwindValues::from_str("bg-brand-500/50").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--color-brand-500", "#5b21b6")]));

  assert_eq!(
    computed.background_color,
    ColorInput::Value(Color([91, 33, 182, 128]))
  );
}

#[test]
fn test_aspect_keywords_survive_the_move_out_of_the_fixed_table() {
  let square = TailwindValues::from_str("aspect-square").expect("tailwind values should parse");
  let video = TailwindValues::from_str("aspect-video").expect("tailwind values should parse");

  let square =
    Style::from(square.into_declaration_block(Viewport::new((100, 100)), &Default::default()))
      .inherit(&ComputedStyle::default());
  let styled =
    Style::from(video.into_declaration_block(Viewport::new((100, 100)), &Default::default()))
      .inherit(&root_with(&[("--aspect-video", "4/3")]));

  assert_eq!(square.aspect_ratio, AspectRatio::Ratio(1.0));
  assert_eq!(styled.aspect_ratio, AspectRatio::Ratio(4.0 / 3.0));
}

/// A custom `--text-*` token spells its line height in the companion variable,
/// not in the token itself.
#[test]
fn test_custom_text_token_line_height_reads_the_companion() {
  let values = TailwindValues::from_str("text-brand").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let sized = style
    .clone()
    .inherit(&root_with(&[("--text-brand", "2rem")]));
  let leaded = style.inherit(&root_with(&[
    ("--text-brand", "2rem"),
    ("--text-brand--line-height", "3rem"),
  ]));

  assert_eq!(sized.line_height, ComputedStyle::default().line_height);
  assert_eq!(leaded.line_height, LineHeight::Length(Length::Rem(3.0)));
}

/// With both namespaces defined, `max-w` takes `--container-*`, the namespace
/// Tailwind prefers.
#[test]
fn test_max_w_prefers_the_container_namespace() {
  let values = TailwindValues::from_str("max-w-page").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let both = style.clone().inherit(&root_with(&[
    ("--container-page", "60rem"),
    ("--spacing-page", "1rem"),
  ]));
  let spacing_only = style.inherit(&root_with(&[("--spacing-page", "1rem")]));

  assert_eq!(both.max_width, MaxSize::Length(Length::Rem(60.0)));
  assert_eq!(spacing_only.max_width, MaxSize::Length(Length::Rem(1.0)));
}

/// Numeric `leading-*` multiplies the spacing step, as Tailwind compiles it.
#[test]
fn test_numeric_leading_scales_with_spacing() {
  let values = TailwindValues::from_str("leading-7").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--spacing", "0.5rem")]));
  let sizing = SizingContext::builder()
    .viewport(Viewport::new((100, 100)))
    .build();

  let LineHeight::Length(line_height) = computed.line_height else {
    panic!("expected a length line height");
  };

  assert_eq!(line_height.to_px(&sizing, 0.0), 56.0);
}

#[test]
fn test_gradient_reads_css_variables() {
  let values = TailwindValues::from_str("bg-linear-to-r from-brand-500 to-red-500")
    .expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[
    ("--color-brand-500", "#5b21b6"),
    ("--color-red-500", "#00a63e"),
  ]));

  let images = computed.background_image.as_deref().expect("gradient");
  let [BackgroundImage::Linear(gradient)] = images else {
    panic!("expected a single linear gradient");
  };

  let colors: Vec<ColorInput> = gradient
    .stops
    .iter()
    .map(|stop| match stop {
      GradientStop::ColorHint { color, .. } => *color,
      GradientStop::Hint(..) => panic!("expected color stops"),
    })
    .collect();

  assert_eq!(
    colors,
    vec![
      ColorInput::Value(Color::from_rgb(0x5b21b6)),
      ColorInput::Value(Color::from_rgb(0x00a63e)),
    ]
  );
}

/// Stops and the gradient shape compose through custom properties, so utility
/// order cannot matter.
#[test]
fn test_gradient_utility_order_does_not_matter() {
  let compute = |classes: &str| {
    let values = TailwindValues::from_str(classes).expect("tailwind values should parse");

    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()))
      .inherit(&ComputedStyle::default())
      .background_image
  };

  let forward = compute("bg-linear-to-r from-red-500 to-blue-500");

  assert!(forward.is_some());
  assert_eq!(forward, compute("from-red-500 to-blue-500 bg-linear-to-r"));
}

/// Stops alone declare no `background-image`, as Tailwind compiles them.
#[test]
fn test_stops_without_a_gradient_paint_nothing() {
  let values =
    TailwindValues::from_str("from-red-500 to-blue-500").expect("tailwind values should parse");
  let computed =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()))
      .inherit(&ComputedStyle::default());

  assert_eq!(computed.background_image, None);
}

/// `--tw-*` state is `inherits: false`, so a child gradient starts from its
/// own stops, not the parent's.
#[test]
fn test_gradient_state_does_not_inherit() {
  let parent_values = TailwindValues::from_str("bg-linear-to-r from-red-500 to-blue-500")
    .expect("tailwind values should parse");
  let parent = Style::from(
    parent_values.into_declaration_block(Viewport::new((100, 100)), &Default::default()),
  )
  .inherit(&ComputedStyle::default());

  let child_values = TailwindValues::from_str("bg-radial").expect("tailwind values should parse");
  let child = Style::from(
    child_values.into_declaration_block(Viewport::new((100, 100)), &Default::default()),
  )
  .inherit(&parent);

  // With the parent's stops out of reach, `var(--tw-gradient-stops)` fails to
  // substitute and the child paints no gradient, as a browser would.
  assert_eq!(child.background_image, None);
}

/// The shadow colour utility overrides every layer through the variable it
/// sets, and reads its variable like any colour utility.
#[test]
fn test_shadow_color_reads_css_variables() {
  let values =
    TailwindValues::from_str("shadow-md shadow-brand-500").expect("tailwind values should parse");
  let computed =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()))
      .inherit(&root_with(&[("--color-brand-500", "#5b21b6")]));

  let shadows = computed.box_shadow.as_deref().expect("shadows");

  assert_eq!(shadows.len(), 2);

  for shadow in shadows {
    assert_eq!(shadow.color, ColorInput::Value(Color::from_rgb(0x5b21b6)));
  }
}

/// Filters compose through `--tw-*` variables in Tailwind's fixed chain
/// order, whatever order the utilities appear in.
#[test]
fn test_filters_compose_through_variables() {
  let compute = |classes: &str| {
    let values = TailwindValues::from_str(classes).expect("tailwind values should parse");

    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()))
      .inherit(&ComputedStyle::default())
      .filter
  };

  let forward = compute("blur-sm brightness-125");

  assert!(matches!(
    &forward[..],
    [Filter::Blur(..), Filter::Brightness(..)]
  ));
  assert_eq!(forward, compute("brightness-125 blur-sm"));
}

#[test]
fn test_translate_composes_through_variables() {
  let values =
    TailwindValues::from_str("translate-x-4 -translate-y-2").expect("tailwind values should parse");
  let computed =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()))
      .inherit(&ComputedStyle::default());

  assert_eq!(computed.translate.x, Length::Rem(1.0));
  assert_eq!(computed.translate.y, Length::Rem(-0.5));
}

#[test]
fn test_class_list_parsing_shares_storage() {
  let first: TailwindValues = "flex items-center gap-2".parse().unwrap();
  let second: TailwindValues = "flex items-center gap-2".parse().unwrap();
  let deserialized: TailwindValues = serde_json::from_str("\"flex items-center gap-2\"").unwrap();

  assert!(Arc::ptr_eq(&first.inner, &second.inner));
  assert!(Arc::ptr_eq(&first.inner, &deserialized.inner));
  assert_eq!(first, second);
  assert_eq!(first, " flex  items-center gap-2 ".parse().unwrap());
}

#[test]
fn test_expansion_cache_keeps_class_lists_distinct() {
  let first = TailwindValues::parse("w-1");
  let second = TailwindValues::parse("w-2");
  let viewport = Viewport::new((100, 100));
  let breakpoints = BreakpointOverrides::default();
  let cache = TwCache::default();
  let first_blocks = first.declaration_blocks(viewport, &breakpoints, &cache);
  let second_blocks = second.declaration_blocks(viewport, &breakpoints, &cache);

  assert!(!Rc::ptr_eq(&first_blocks, &second_blocks));
  assert_ne!(first_blocks.normal, second_blocks.normal);
  assert!(Rc::ptr_eq(
    &second_blocks,
    &second.declaration_blocks(viewport, &breakpoints, &cache),
  ));
  assert_eq!(
    first_blocks.normal,
    first
      .declaration_blocks(viewport, &breakpoints, &cache)
      .normal,
  );
}

#[test]
fn test_expansion_cache_retains_its_key() {
  let values = TailwindValues::parse("w-3");
  let weak = Arc::downgrade(&values.inner);
  let cache = TwCache::default();
  values.declaration_blocks(Viewport::new((100, 100)), &Default::default(), &cache);
  drop(values);
  assert!(weak.upgrade().is_some());
  drop(cache);
  assert!(weak.upgrade().is_none());
}

#[test]
fn test_expansion_cache_stops_retaining_unique_lists_when_full() {
  let cache = TwCache::default();
  let viewport = Viewport::new((100, 100));
  let breakpoints = BreakpointOverrides::default();
  let first = TailwindValues::parse("w-1");
  let blocks = first.declaration_blocks(viewport, &breakpoints, &cache);
  for _ in 1..EXPANSION_CACHE_MAX_ENTRIES {
    TailwindValues::parse("w-1").declaration_blocks(viewport, &breakpoints, &cache);
  }

  let overflow = TailwindValues::parse("w-2");
  let weak = Arc::downgrade(&overflow.inner);
  let expanded = overflow.declaration_blocks(viewport, &breakpoints, &cache);
  assert_eq!(cache.blocks.borrow().len(), EXPANSION_CACHE_MAX_ENTRIES);
  assert_eq!(
    expanded.normal,
    overflow
      .clone()
      .into_declaration_block(viewport, &breakpoints)
      .split_importance()
      .0,
  );
  drop(overflow);
  assert!(weak.upgrade().is_none());
  assert!(Rc::ptr_eq(
    &blocks,
    &first.declaration_blocks(viewport, &breakpoints, &cache),
  ));
}

#[test]
fn test_expansion_cache_tracks_breakpoint_inputs() {
  let values: TailwindValues = "w-1 sm:w-2".parse().unwrap();
  let viewport = Viewport::new((800, 100));
  let cache = TwCache::default();
  let breakpoints = BreakpointOverrides::default();
  let active = values.declaration_blocks(viewport, &breakpoints, &cache);

  for inactive_viewport in [
    Viewport::new((400, 100)),
    viewport.with_font_size(32.0),
    viewport.with_device_pixel_ratio(2.0),
  ] {
    let inactive = values.declaration_blocks(inactive_viewport, &breakpoints, &cache);
    assert_ne!(active.normal, inactive.normal);
  }

  let themed = values.declaration_blocks(
    viewport,
    &HashMap::from([("sm".to_owned(), Length::Px(900.0))]),
    &TwCache::default(),
  );
  assert_ne!(active.normal, themed.normal);
}
