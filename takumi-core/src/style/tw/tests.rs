use std::{assert_matches, collections::HashMap, sync::Arc};

use super::{builder::TailwindDeclarationBuilder, parser::*, *};
use crate::style::{ComputedStyle, LonghandId, PropertyRule, Style, properties::BackgroundImage};

/// The declarations a utility emits with no variable defined, which is what these assertions are
/// about.
fn parse_property(token: &str) -> Option<Vec<StyleDeclaration>> {
  let mut builder = TailwindDeclarationBuilder::default();

  utility::emit(token, &mut builder)?;

  Some(builtin_declarations(&builder.finish()))
}

/// Each declaration, with a theme variable's read replaced by the built-in value it falls back to.
fn builtin_declarations(declarations: &StyleDeclarationBlock) -> Vec<StyleDeclaration> {
  declarations
    .iter()
    .filter_map(|declaration| match declaration {
      StyleDeclaration::VarRef(var_ref) => var_ref.fallback.as_deref().cloned(),
      declaration => Some(declaration.clone()),
    })
    .collect()
}

/// `declarations` finished as a utility's are, so a border width carries the style it draws in.
fn finished(declarations: impl IntoIterator<Item = StyleDeclaration>) -> Vec<StyleDeclaration> {
  let mut builder = TailwindDeclarationBuilder::default();

  builder.push_all(declarations);
  builder.finish().iter().cloned().collect()
}

fn expect(
  declarations: impl IntoIterator<Item = StyleDeclaration>,
) -> Option<Vec<StyleDeclaration>> {
  Some(finished(declarations))
}

/// The declarations a `border-*` style utility writes.
fn border_style(style: BorderStyle) -> Vec<StyleDeclaration> {
  vec![
    StyleDeclaration::CustomProperty("--tw-border-style".to_owned(), style.to_css_string()),
    StyleDeclaration::border_top_style(style),
    StyleDeclaration::border_right_style(style),
    StyleDeclaration::border_bottom_style(style),
    StyleDeclaration::border_left_style(style),
  ]
}

/// The declarations a `rounded` utility writes for `radius`.
fn rounded(radius: Length) -> Vec<StyleDeclaration> {
  let radius = SpacePair::from_single(radius);

  vec![
    StyleDeclaration::border_top_left_radius(radius),
    StyleDeclaration::border_top_right_radius(radius),
    StyleDeclaration::border_bottom_right_radius(radius),
    StyleDeclaration::border_bottom_left_radius(radius),
  ]
}

/// The rule's declarations with theme reads replaced by their fallbacks, paired with its variant.
fn parse_value(token: &str) -> Option<(Vec<StyleDeclaration>, Option<Breakpoint>, bool)> {
  let rule = TwRule::parse(token)?;

  Some((
    builtin_declarations(&rule.declarations),
    rule.breakpoint.clone(),
    rule.important,
  ))
}

#[test]
fn test_box_sizing() {
  assert_eq!(
    parse_property("box-border"),
    expect(vec![StyleDeclaration::box_sizing(BoxSizing::BorderBox)])
  );
}

#[test]
fn test_parse_width() {
  assert_eq!(
    parse_property("w-64"),
    expect(vec![StyleDeclaration::width(Size::Length(
      Length::from_spacing(64.0)
    ))])
  );
  assert_eq!(
    parse_property("h-32"),
    expect(vec![StyleDeclaration::height(Size::Length(
      Length::from_spacing(32.0)
    ))])
  );
  assert_eq!(
    parse_property("justify-self-center"),
    expect(vec![StyleDeclaration::justify_self(AlignItems::Center)])
  );
}

#[test]
fn test_parse_color() {
  assert_eq!(
    parse_property("text-black/30"),
    expect(vec![StyleDeclaration::color(ColorInput::Value(Color([
      0,
      0,
      0,
      (0.3_f32 * 255.0).round() as u8
    ])))])
  );
}

#[test]
fn test_parse_red_500_color_utilities() {
  let cases: &[(&str, Vec<StyleDeclaration>)] = &[
    (
      "shadow-red-500",
      vec![StyleDeclaration::CustomProperty(
        "--tw-shadow-color".to_owned(),
        TwVarColor::parse_tw("red-500")
          .expect("palette colour")
          .0
          .to_string(),
      )],
    ),
    (
      "text-shadow-red-500",
      vec![StyleDeclaration::CustomProperty(
        "--tw-text-shadow-color".to_owned(),
        TwVarColor::parse_tw("red-500")
          .expect("palette colour")
          .0
          .to_string(),
      )],
    ),
    (
      "decoration-red-500",
      vec![StyleDeclaration::text_decoration_color(ColorInput::Value(
        Color([251, 44, 54, 255]),
      ))],
    ),
    (
      "bg-red-500",
      vec![StyleDeclaration::background_color(ColorInput::Value(
        Color([251, 44, 54, 255]),
      ))],
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
    expect(vec![StyleDeclaration::text_decoration_line(Some(
      TextDecorationLines::UNDERLINE
    ))])
  );
  assert_eq!(
    parse_property("no-underline"),
    expect(vec![StyleDeclaration::text_decoration_line(Some(
      TextDecorationLines::empty()
    ))])
  );
}

#[test]
fn test_parse_arbitrary_color() {
  assert_eq!(
    parse_property("text-[rgb(0, 191, 255)]"),
    expect(vec![StyleDeclaration::color(ColorInput::Value(Color([
      0, 191, 255, 255
    ])))])
  );
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
    expect(vec![StyleDeclaration::mask_image(Some(
      [BackgroundImage::Url(
        "https://example.com/my_logo.svg".into()
      )]
      .into()
    ))])
  );
}

#[test]
fn test_parse_value_arbitrary_url_with_scheme_colon() {
  let url_image = finished(vec![StyleDeclaration::mask_image(Some(
    [BackgroundImage::Url("https://example.com/a.svg".into())].into(),
  ))]);

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
    expect(StyleDeclaration::expand_flex(Flex {
      grow: 3.0,
      shrink: 1.0,
      basis: FlexBasis::default(),
    }))
  );
}

#[test]
fn test_parse_tailwind_animation_preset() {
  assert_matches!(
    TwAnimation::parse_tw("spin"),
    Some(tw_animation)
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
    TwAnimation::parse_tw_with_arbitrary("[wiggle_1s_ease-in-out_infinite]"),
    Some(tw_animation)
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
    expect(vec![StyleDeclaration::margin_left(Length::from_spacing(
      -4.0
    ))])
  );
}

#[test]
fn test_parse_border_radius() {
  assert_eq!(
    parse_property("rounded-xs"),
    expect(rounded(Length::Rem(0.125)))
  );
  assert_eq!(
    parse_property("rounded-full"),
    expect(rounded(Length::Px(9999.0)))
  );
}

#[test]
fn test_parse_font_size_with_arbitrary_line_height() {
  assert_eq!(
    parse_property("text-base/[12.34]"),
    expect([
      StyleDeclaration::font_size(Length::Rem(1.0).into()),
      StyleDeclaration::line_height(LineHeight::Unitless(12.34)),
    ])
  );
}

#[test]
fn test_parse_border_width() {
  assert_eq!(
    parse_property("border"),
    expect(vec![
      StyleDeclaration::border_top_width(LineWidth::Length(Length::Px(1.0))),
      StyleDeclaration::border_right_width(LineWidth::Length(Length::Px(1.0))),
      StyleDeclaration::border_bottom_width(LineWidth::Length(Length::Px(1.0))),
      StyleDeclaration::border_left_width(LineWidth::Length(Length::Px(1.0)))
    ])
  );
  assert_eq!(
    parse_property("border-b"),
    expect(vec![StyleDeclaration::border_bottom_width(
      LineWidth::Length(Length::Px(1.0))
    )])
  );
  assert_eq!(
    parse_property("border-y"),
    expect(vec![
      StyleDeclaration::border_top_width(LineWidth::Length(Length::Px(1.0))),
      StyleDeclaration::border_bottom_width(LineWidth::Length(Length::Px(1.0)))
    ])
  );
  assert_eq!(
    parse_property("border-t-2"),
    expect(vec![StyleDeclaration::border_top_width(LineWidth::Length(
      Length::Px(2.0)
    ))])
  );
  assert_eq!(
    parse_property("border-x-4"),
    expect(vec![
      StyleDeclaration::border_left_width(LineWidth::Length(Length::Px(4.0))),
      StyleDeclaration::border_right_width(LineWidth::Length(Length::Px(4.0)))
    ])
  );
  assert_eq!(
    parse_property("border-solid"),
    expect(border_style(BorderStyle::Solid))
  );
  assert_eq!(
    parse_property("border-dashed"),
    expect(border_style(BorderStyle::Dashed))
  );
  assert_eq!(
    parse_property("border-dotted"),
    expect(border_style(BorderStyle::Dotted))
  );
  assert_eq!(
    parse_property("border-none"),
    expect(border_style(BorderStyle::None))
  );
}

#[test]
fn test_parse_outline() {
  assert_eq!(
    parse_property("outline"),
    expect(vec![
      StyleDeclaration::outline_width(LineWidth::Length(Length::Px(1.0))),
      StyleDeclaration::outline_style(BorderStyle::Solid)
    ])
  );
  assert_eq!(
    parse_property("outline-2"),
    expect(vec![StyleDeclaration::outline_width(LineWidth::Length(
      Length::Px(2.0)
    ))])
  );
  assert_eq!(
    parse_property("outline-red-500"),
    expect(vec![StyleDeclaration::outline_color(ColorInput::Value(
      Color([251, 44, 54, 255])
    ))])
  );
  assert_eq!(
    parse_property("outline-solid"),
    expect(vec![StyleDeclaration::outline_style(BorderStyle::Solid)])
  );
  assert_eq!(
    parse_property("outline-dashed"),
    expect(vec![StyleDeclaration::outline_style(BorderStyle::Dashed)])
  );
  assert_eq!(
    parse_property("outline-offset-4"),
    expect(vec![StyleDeclaration::outline_offset(Length::from(
      LineWidth::Length(Length::Px(4.0))
    ))])
  );
  assert_eq!(
    parse_property("outline-none"),
    expect(vec![StyleDeclaration::outline_style(BorderStyle::None)])
  );
}

#[test]
fn test_parse_col_end() {
  assert_eq!(
    parse_property("col-end-1"),
    expect(vec![StyleDeclaration::grid_column_end(
      GridPlacement::Line(1)
    )])
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
  let cases: &[(&str, Vec<StyleDeclaration>)] = &[
    (
      "overflow-clip",
      vec![
        StyleDeclaration::overflow_x(Overflow::Clip),
        StyleDeclaration::overflow_y(Overflow::Clip),
      ],
    ),
    (
      "overflow-x-clip",
      vec![StyleDeclaration::overflow_x(Overflow::Clip)],
    ),
    (
      "overflow-y-clip",
      vec![StyleDeclaration::overflow_y(Overflow::Clip)],
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
    let declarations = parse_property(class);
    assert!(
      declarations.is_some_and(|declarations| !declarations.is_empty()),
      "expected '{class}' to parse into at least one declaration"
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
      finished(vec![StyleDeclaration::margin_top(Length::Rem(1.0))]),
      Breakpoint::parse("md"),
      true
    ))
  );
}

/// The cascade applies the normal rules, then the `!important` ones, each with its breakpoint
/// variant after the plain utility, as Tailwind sorts them.
#[test]
fn test_values_sorting() {
  let values = TailwindValues::from_str("md:!mt-4 sm:mt-8 !mt-12 mt-16")
    .expect("tailwind values should parse");
  let order = cascade_order(values.inner.iter().map(Arc::as_ref))
    .into_iter()
    .map(|rule| {
      let declarations = rule
        .declarations
        .iter()
        .map(|declaration| match declaration {
          StyleDeclaration::VarRef(var_ref) => *var_ref.fallback.clone().unwrap(),
          declaration => declaration.clone(),
        })
        .collect::<Vec<_>>();

      (declarations, rule.breakpoint.clone(), rule.important)
    })
    .collect::<Vec<_>>();

  assert_eq!(
    order,
    vec![
      // mt-16
      (
        finished(vec![StyleDeclaration::margin_top(Length::Rem(4.0))]),
        None,
        false
      ),
      // sm:mt-8
      (
        finished(vec![StyleDeclaration::margin_top(Length::Rem(2.0))]),
        Breakpoint::parse("sm"),
        false
      ),
      // !mt-12
      (
        finished(vec![StyleDeclaration::margin_top(Length::Rem(3.0))]),
        None,
        true
      ),
      // md:!mt-4
      (
        finished(vec![StyleDeclaration::margin_top(Length::Rem(1.0))]),
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
    style.misc1_data.filter,
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
    style.misc4_data.translate,
    SpacePair::from_pair(Length::Rem(1.0), Length::Rem(2.0))
  );
  assert_eq!(
    style.misc3_data.scale,
    Some(SpacePair::from_pair(
      PercentageNumber(0.5),
      PercentageNumber(0.75)
    ))
  );
}

#[test]
fn test_parse_blend_mode() {
  assert_eq!(
    parse_property("mix-blend-multiply"),
    expect(vec![StyleDeclaration::mix_blend_mode(BlendMode::Multiply)])
  );
  assert_eq!(
    parse_property("bg-blend-screen"),
    expect(vec![StyleDeclaration::background_blend_mode(
      [BlendMode::Screen].into()
    )])
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
      expect(vec![StyleDeclaration::vertical_align(
        VerticalAlign::Keyword(*kw)
      )]),
      "{input}"
    );
  }
  // Arbitrary-value lengths
  assert_eq!(
    parse_property("align-[10px]"),
    expect(vec![StyleDeclaration::vertical_align(
      VerticalAlign::Length(Length::Px(10.0))
    )])
  );
  assert_eq!(
    parse_property("align-[25%]"),
    expect(vec![StyleDeclaration::vertical_align(
      VerticalAlign::Length(Length::Percentage(25.0))
    )])
  );
  assert_eq!(
    parse_property("align-[-0.5em]"),
    expect(vec![StyleDeclaration::vertical_align(
      VerticalAlign::Length(Length::Em(-0.5))
    )])
  );
}

#[test]
fn test_parse_decoration_style() {
  assert_eq!(
    parse_property("decoration-wavy"),
    expect(vec![StyleDeclaration::text_decoration_style(
      TextDecorationStyle::Wavy
    )])
  );
  assert_eq!(
    parse_property("decoration-dotted"),
    expect(vec![StyleDeclaration::text_decoration_style(
      TextDecorationStyle::Dotted
    )])
  );
}

#[test]
fn test_parse_decoration_thickness() {
  assert_eq!(
    parse_property("decoration-4"),
    expect(vec![StyleDeclaration::text_decoration_thickness(
      TextDecorationThickness::Length(Length::Px(4.0))
    )])
  );
  assert_eq!(
    parse_property("decoration-auto"),
    expect(vec![StyleDeclaration::text_decoration_thickness(
      TextDecorationThickness::Length(Length::Auto)
    )])
  );
  assert_eq!(
    parse_property("decoration-from-font"),
    expect(vec![StyleDeclaration::text_decoration_thickness(
      TextDecorationThickness::FromFont
    )])
  );
  assert_eq!(
    parse_property("decoration-[3px]"),
    expect(vec![StyleDeclaration::text_decoration_thickness(
      TextDecorationThickness::Length(Length::Px(3.0))
    )])
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
    style.background_data.background_image,
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
      style.misc1_data.box_shadow,
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
      style.misc_inherited1_data.text_shadow,
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
    expect(rounded(Length::Rem(0.25)))
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
    expect(vec![StyleDeclaration::grid_column_start(
      GridPlacement::Line(-1)
    )])
  );
}

#[test]
fn test_logical_resolves_to_physical_ltr() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("ms-4 me-2 ps-3 pe-1").unwrap();
  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());
  assert_eq!(style.box_data.margin_left, Length::from_spacing(4.0));
  assert_eq!(style.box_data.margin_right, Length::from_spacing(2.0));
  assert_eq!(style.box_data.padding_left, Length::from_spacing(3.0));
  assert_eq!(style.box_data.padding_right, Length::from_spacing(1.0));
}

/// Tailwind sorts `margin-inline-start` before `margin-left`, so `ml-*` wins whatever the class
/// order.
#[test]
fn test_logical_physical_cascade_order_ltr() {
  let viewport = Viewport::new((100, 100));

  for classes in ["ms-2 ml-4", "ml-4 ms-2"] {
    let values = TailwindValues::from_str(classes).unwrap();
    let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
      .inherit(&ComputedStyle::default());

    assert_eq!(
      style.box_data.margin_left,
      Length::from_spacing(4.0),
      "{classes}"
    );
  }
}

#[test]
fn test_logical_resolves_to_physical_rtl() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("ms-4 me-2 ps-3 pe-1").unwrap();
  let mut block = values.into_declaration_block(viewport, &Default::default());
  block.push(StyleDeclaration::direction(Direction::Rtl), false);
  let style = Style::from(block).inherit(&ComputedStyle::default());
  assert_eq!(style.box_data.margin_right, Length::from_spacing(4.0));
  assert_eq!(style.box_data.margin_left, Length::from_spacing(2.0));
  assert_eq!(style.box_data.padding_right, Length::from_spacing(3.0));
  assert_eq!(style.box_data.padding_left, Length::from_spacing(1.0));
}

#[test]
fn test_logical_resolves_when_direction_declared_after() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("ms-4").unwrap();
  let mut block = values.into_declaration_block(viewport, &Default::default());
  block.push(StyleDeclaration::direction(Direction::Rtl), false);
  let style = Style::from(block).inherit(&ComputedStyle::default());
  assert_eq!(style.box_data.margin_right, Length::from_spacing(4.0));
  assert_eq!(style.box_data.margin_left, Length::Px(0.0));
}

#[test]
fn test_filter_none_clears_previous_filters() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("blur-sm brightness-150 filter-none").unwrap();
  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());
  assert_eq!(style.misc1_data.filter, Filters::default());

  let values = TailwindValues::from_str("backdrop-blur-sm backdrop-filter-none").unwrap();
  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());
  assert_eq!(style.misc1_data.backdrop_filter, Filters::default());
}

#[test]
fn test_font_numeric_weight() {
  assert_eq!(
    parse_property("font-700"),
    expect(vec![StyleDeclaration::font_weight(FontWeight::from(700.0))])
  );
}

#[test]
fn test_line_clamp_none() {
  assert_eq!(
    parse_property("line-clamp-none"),
    expect(StyleDeclaration::expand_line_clamp(LineClamp::default()))
  );
}

#[test]
fn test_bg_auto() {
  assert_eq!(
    parse_property("bg-auto"),
    expect(vec![StyleDeclaration::background_size(
      [BackgroundSize::Explicit {
        width: Length::Auto,
        height: Length::Auto,
      }]
      .into()
    )])
  );
}

#[test]
fn test_bg_repeat_v4_names() {
  assert_eq!(
    parse_property("bg-repeat-round"),
    expect(vec![StyleDeclaration::background_repeat(
      [BackgroundRepeat::round()].into()
    )])
  );
  assert_eq!(
    parse_property("bg-repeat-space"),
    expect(vec![StyleDeclaration::background_repeat(
      [BackgroundRepeat::space()].into()
    )])
  );
}

#[test]
fn test_grid_cols_none() {
  assert_eq!(
    parse_property("grid-cols-none"),
    expect([StyleDeclaration::grid_template_columns(Some(
      GridTemplateComponents::default()
    ))])
  );
}

#[test]
fn test_col_auto_row_auto() {
  assert_eq!(
    parse_property("col-auto"),
    expect(StyleDeclaration::expand_grid_column(GridLine {
      start: GridPlacement::auto(),
      end: GridPlacement::auto(),
    }))
  );
  assert_eq!(
    parse_property("row-auto"),
    expect(StyleDeclaration::expand_grid_row(GridLine {
      start: GridPlacement::auto(),
      end: GridPlacement::auto(),
    }))
  );
}

#[test]
fn test_shadow_none_overrides_color_in_either_order() {
  let viewport = Viewport::new((100, 100));
  for classes in ["shadow-none shadow-red-500", "shadow-red-500 shadow-none"] {
    let values = TailwindValues::from_str(classes).unwrap();
    let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
      .inherit(&ComputedStyle::default());
    assert_eq!(style.misc1_data.box_shadow, None, "case: {classes}");
  }
}

#[test]
fn test_bg_conic_standalone() {
  assert_eq!(
    parse_property("bg-conic"),
    expect([StyleDeclaration::Deferred(DeferredDeclaration {
      property: PropertyId::Longhand(LonghandId::BackgroundImage),
      specified_value: "conic-gradient(in oklab, var(--tw-gradient-stops))".to_owned(),
    })])
  );
}

#[test]
fn test_gradient_stop_position_is_used_in_apply() {
  let viewport = Viewport::new((100, 100));
  let values =
    TailwindValues::from_str("bg-linear-to-r from-red-500 from-10% to-blue-500 to-80%").unwrap();
  let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
    .inherit(&ComputedStyle::default());
  let images = style.background_data.background_image.as_deref().unwrap();
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
  assert_eq!(
    top.box_data.border_top_width,
    LineWidth::Length(Length::Px(8.0))
  );
  assert_eq!(top.box_data.border_top_style, BorderStyle::Solid);
  assert_eq!(top.box_data.border_bottom_style, BorderStyle::None);

  let all = computed("border-4");
  assert_eq!(all.box_data.border_top_style, BorderStyle::Solid);
  assert_eq!(all.box_data.border_right_style, BorderStyle::Solid);
  assert_eq!(all.box_data.border_bottom_style, BorderStyle::Solid);
  assert_eq!(all.box_data.border_left_style, BorderStyle::Solid);

  let dashed = computed("border-2 border-dashed");
  assert_eq!(dashed.box_data.border_top_style, BorderStyle::Dashed);

  assert_eq!(
    parse_property("border-t-blue-500"),
    expect([StyleDeclaration::border_top_color(ColorInput::Value(
      Color([43, 127, 255, 255])
    ))])
  );

  let bar = computed("border-t-8 border-t-blue-500");
  assert_eq!(
    bar.box_data.border_top_width,
    LineWidth::Length(Length::Px(8.0))
  );
  assert_eq!(bar.box_data.border_top_style, BorderStyle::Solid);
  assert_eq!(
    bar.surround_data.border_top_color,
    ColorInput::Value(Color([43, 127, 255, 255]))
  );
}

/// Mirrors the utilities documented at https://tailwindcss.com/docs/width
#[test]
fn test_parse_sizing_keyword_utilities() {
  for (token, expected) in [
    ("w-min", vec![StyleDeclaration::width(Size::MinContent)]),
    ("w-max", vec![StyleDeclaration::width(Size::MaxContent)]),
    ("w-fit", vec![StyleDeclaration::width(Size::FitContent)]),
    ("h-min", vec![StyleDeclaration::height(Size::MinContent)]),
    ("h-max", vec![StyleDeclaration::height(Size::MaxContent)]),
    ("h-fit", vec![StyleDeclaration::height(Size::FitContent)]),
    (
      "size-fit",
      vec![
        StyleDeclaration::width(Size::FitContent),
        StyleDeclaration::height(Size::FitContent),
      ],
    ),
    (
      "basis-content",
      vec![StyleDeclaration::flex_basis(Some(FlexBasis::Content))],
    ),
    (
      "basis-full",
      vec![StyleDeclaration::flex_basis(Some(FlexBasis::Size(
        Size::Length(Length::Percentage(100.0)),
      )))],
    ),
  ] {
    assert_eq!(
      parse_property(token),
      expect(expected),
      "failed for {token}"
    );
  }
}

/// Mirrors the utilities documented at https://tailwindcss.com/docs/contain
#[test]
fn test_parse_contain_utilities() {
  for (token, expected) in [
    ("contain-none", Contain::NONE),
    ("contain-content", Contain::CONTENT),
    ("contain-layout", Contain::LAYOUT),
    ("contain-style", Contain::STYLE),
    ("contain-paint", Contain::PAINT),
  ] {
    assert_eq!(
      parse_property(token),
      expect(vec![StyleDeclaration::contain(expected)]),
      "failed for {token}"
    );
  }
}

/// Mirrors the utilities documented at https://tailwindcss.com/docs/display
#[test]
fn test_parse_flow_root_utility() {
  assert_eq!(
    parse_property("flow-root"),
    expect(vec![StyleDeclaration::display(Display::FlowRoot)])
  );
}

/// Mirrors the utilities documented at https://tailwindcss.com/docs/list-style-type
#[test]
fn test_parse_list_utilities() {
  for (token, expected) in [
    (
      "list-item",
      vec![StyleDeclaration::display(Display::ListItem)],
    ),
    (
      "list-disc",
      vec![StyleDeclaration::list_style_type(ListStyleType::Disc)],
    ),
    (
      "list-decimal",
      vec![StyleDeclaration::list_style_type(ListStyleType::Decimal)],
    ),
    (
      "list-none",
      vec![StyleDeclaration::list_style_type(ListStyleType::None)],
    ),
    (
      "list-[upper-roman]",
      vec![StyleDeclaration::list_style_type(ListStyleType::UpperRoman)],
    ),
    (
      "list-inside",
      vec![StyleDeclaration::list_style_position(
        ListStylePosition::Inside,
      )],
    ),
    (
      "list-outside",
      vec![StyleDeclaration::list_style_position(
        ListStylePosition::Outside,
      )],
    ),
    (
      "list-image-none",
      vec![StyleDeclaration::list_style_image(ListStyleImage::default())],
    ),
  ] {
    assert_eq!(
      parse_property(token),
      expect(expected),
      "failed for {token}"
    );
  }

  assert_matches!(
    parse_property("list-image-[url(marker.png)]").as_deref(),
    Some([StyleDeclaration::ListStyleImage(image)])
      if matches!(image.image(), Some(BackgroundImage::Url(url)) if &**url == "marker.png")
  );
}

/// Stands in for the `:root` rule a stylesheet would supply.
fn root_with(variables: &[(&str, &str)]) -> ComputedStyle {
  let mut root = ComputedStyle::default();
  for (name, value) in variables {
    root
      .custom_properties
      .set((*name).to_owned(), (*value).to_owned());
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

  assert_eq!(computed.box_data.margin_left, Length::Rem(3.0));
  assert_eq!(computed.box_data.padding_left, Length::Rem(3.0));
}

#[test]
fn test_corner_radius_reads_a_css_variable() {
  let values = TailwindValues::from_str("rounded-t-card").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--radius-card", "12px")]));

  assert_eq!(
    computed.surround_data.border_top_left_radius.x,
    Length::Px(12.0)
  );
}

#[test]
fn test_shadow_preset_shape_reads_a_css_variable() {
  use crate::style::properties::BoxShadow;

  let values = TailwindValues::from_str("shadow-md").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--shadow-md", "0 5px 5px #ff0000")]));

  assert_eq!(
    computed.misc1_data.box_shadow.as_deref(),
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
    computed.misc_inherited1_data.text_shadow.as_deref(),
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
  assert_eq!(unthemed.box_data.margin_top, Length::Px(0.0));

  let themed = Style::from(values.into_declaration_block(viewport, &overrides))
    .inherit(&ComputedStyle::default());
  assert_eq!(themed.box_data.margin_top, Length::Rem(1.0));
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
  assert_eq!(unthemed.box_data.margin_top, Length::Px(0.0));

  let themed = Style::from(values.into_declaration_block(viewport, &overrides))
    .inherit(&ComputedStyle::default());
  assert_eq!(themed.box_data.margin_top, Length::Rem(1.0));
}

#[test]
fn test_animate_preset_reads_a_css_variable() {
  let values = TailwindValues::from_str("animate-spin").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--animate-spin", "wobble 2s ease-in 3")]));

  assert_eq!(
    computed.misc1_data.animation_name.as_ref(),
    [Some("wobble".to_string())]
  );
  assert_eq!(
    computed.misc1_data.animation_duration.as_ref(),
    [AnimationTime::from_milliseconds(2000.0)]
  );
}

#[test]
fn test_animate_preset_falls_back_to_the_builtin_animation() {
  let values = TailwindValues::from_str("animate-spin").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&ComputedStyle::default());

  assert_eq!(
    computed.misc1_data.animation_name.as_ref(),
    [Some("spin".to_string())]
  );
  assert_eq!(
    computed.misc1_data.animation_duration.as_ref(),
    [AnimationTime::from_milliseconds(1000.0)]
  );
  assert_eq!(
    computed.misc1_data.animation_iteration_count.as_ref(),
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
  assert_eq!(
    themed.misc1_data.animation_name.as_ref(),
    [Some("wiggle".to_string())]
  );

  let unthemed = style.inherit(&ComputedStyle::default());
  assert_eq!(
    unthemed.misc1_data.animation_name,
    ComputedStyle::default().misc1_data.animation_name
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

  assert_eq!(
    computed.misc1_data.filter,
    vec![Filter::Blur(Length::Px(20.0))]
  );
  assert_eq!(
    computed.misc1_data.backdrop_filter,
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

  assert_eq!(
    computed.misc1_data.filter,
    vec![Filter::Blur(Length::Px(12.0))]
  );
}

#[test]
fn test_drop_shadow_preset_reads_a_css_variable() {
  use crate::style::properties::{Filter, TextShadow};

  let values = TailwindValues::from_str("drop-shadow-md").expect("tailwind values should parse");
  let style =
    Style::from(values.into_declaration_block(Viewport::new((100, 100)), &Default::default()));

  let computed = style.inherit(&root_with(&[("--drop-shadow-md", "0 5px 5px #ff0000")]));

  assert_eq!(
    computed.misc1_data.filter,
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

  assert_eq!(
    computed.inherited_data.color,
    ColorInput::Value(Color::from_rgb(0x00a63e))
  );
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
  assert_eq!(computed.box_data.margin_left.to_px(&sizing, 0.0), 32.0);
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

  assert_eq!(
    sized.inherited_data.font_size,
    FontSize::Length(Length::Rem(2.0))
  );
  assert_eq!(
    coloured.inherited_data.color,
    ColorInput::Value(Color::from_rgb(0x5b21b6))
  );
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
    computed.background_data.background_color,
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

  assert_eq!(square.surround_data.aspect_ratio, AspectRatio::Ratio(1.0));
  assert_eq!(
    styled.surround_data.aspect_ratio,
    AspectRatio::Ratio(4.0 / 3.0)
  );
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

  assert_eq!(
    sized.inherited_data.line_height,
    ComputedStyle::default().inherited_data.line_height
  );
  assert_eq!(
    leaded.inherited_data.line_height,
    LineHeight::Length(Length::Rem(3.0))
  );
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

  assert_eq!(both.box_data.max_width, MaxSize::Length(Length::Rem(60.0)));
  assert_eq!(
    spacing_only.box_data.max_width,
    MaxSize::Length(Length::Rem(1.0))
  );
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

  let LineHeight::Length(line_height) = computed.inherited_data.line_height else {
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

  let images = computed
    .background_data
    .background_image
    .as_deref()
    .expect("gradient");
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
      .background_data
      .background_image
      .clone()
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

  assert_eq!(computed.background_data.background_image, None);
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
  assert_eq!(child.background_data.background_image, None);
}

/// Registering a `--tw-*` name hands it to the `@property` rule, so the child
/// starts from the initial value that rule gives rather than losing the name.
#[test]
fn test_a_registered_gradient_name_starts_from_its_initial_value() {
  let mut parent = ComputedStyle::default();
  let rule = PropertyRule {
    name: "--tw-gradient-from-position".to_owned(),
    syntax: "<length-percentage>".to_owned(),
    inherits: false,
    initial_value: Some("0%".to_owned()),
    media_queries: Vec::new(),
  };

  parent.custom_properties.register_in_scope(
    &Arc::new(HashMap::from([(rule.name.clone(), rule)])),
    &CustomProperties::default(),
  );
  parent
    .custom_properties
    .set("--tw-gradient-from-position".to_owned(), "25%".to_owned());

  let child = ComputedStyle::from_parent(&parent);

  assert_eq!(
    child.custom_properties.get("--tw-gradient-from-position"),
    Some("0%")
  );
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

  let shadows = computed.misc1_data.box_shadow.as_deref().expect("shadows");

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
      .misc1_data
      .filter
      .clone()
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

  assert_eq!(computed.misc4_data.translate.x, Length::Rem(1.0));
  assert_eq!(computed.misc4_data.translate.y, Length::Rem(-0.5));
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
fn test_breakpoints_follow_the_viewport_and_theme() {
  let values: TailwindValues = "w-1 sm:w-2".parse().unwrap();
  let viewport = Viewport::new((800, 100));
  let breakpoints = BreakpointOverrides::default();
  let active = values
    .clone()
    .into_declaration_block(viewport, &breakpoints);

  for inactive_viewport in [
    Viewport::new((400, 100)),
    viewport.with_font_size(32.0),
    viewport.with_device_pixel_ratio(2.0),
  ] {
    let inactive = values
      .clone()
      .into_declaration_block(inactive_viewport, &breakpoints);
    assert_ne!(active, inactive);
  }

  let themed = values.into_declaration_block(
    viewport,
    &HashMap::from([("sm".to_owned(), Length::Px(900.0))]),
  );
  assert_ne!(active, themed);
}

/// A utility for one side sorts after one for every side, as Tailwind orders `padding-top` after
/// `padding`, whatever the class order.
#[test]
fn a_side_utility_beats_an_every_side_one() {
  let viewport = Viewport::new((100, 100));

  for classes in ["pt-2 p-4", "p-4 pt-2"] {
    let values = TailwindValues::from_str(classes).unwrap();
    let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
      .inherit(&ComputedStyle::default());

    assert_eq!(
      style.box_data.padding_top,
      Length::from_spacing(2.0),
      "{classes}"
    );
    assert_eq!(
      style.box_data.padding_left,
      Length::from_spacing(4.0),
      "{classes}"
    );
  }
}

/// Tailwind sorts `margin`, `margin-inline`, then `margin-inline-start`, so `m-4 mx-2 ms-1` gives
/// the left margin to `ms-1` and the right to `mx-2`, whatever the class order.
#[test]
fn logical_side_utilities_sort_as_tailwind_writes_them() {
  let viewport = Viewport::new((100, 100));

  for classes in ["m-4 mx-2 ms-1", "ms-1 mx-2 m-4", "mx-2 ms-1 m-4"] {
    let values = TailwindValues::from_str(classes).unwrap();
    let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
      .inherit(&ComputedStyle::default());

    assert_eq!(
      style.box_data.margin_top,
      Length::from_spacing(4.0),
      "{classes}"
    );
    assert_eq!(
      style.box_data.margin_right,
      Length::from_spacing(2.0),
      "{classes}"
    );
    assert_eq!(
      style.box_data.margin_left,
      Length::from_spacing(1.0),
      "{classes}"
    );
  }

  for classes in ["px-4 pe-1", "pe-1 px-4"] {
    let values = TailwindValues::from_str(classes).unwrap();
    let style = Style::from(values.into_declaration_block(viewport, &Default::default()))
      .inherit(&ComputedStyle::default());

    assert_eq!(
      style.box_data.padding_right,
      Length::from_spacing(1.0),
      "{classes}"
    );
  }
}

/// Tailwind registers `odd:` after `first:`, so `odd:` wins whatever the class order.
#[test]
fn structural_variants_cascade_as_tailwind_registers_them() {
  let order = |classes: &str| {
    let values = TailwindValues::from_str(classes).unwrap();

    cascade_order(values.inner.iter().map(Arc::as_ref))
      .into_iter()
      .map(|rule| rule.order.clone())
      .collect::<Vec<_>>()
  };

  assert_eq!(order("odd:mt-2 first:mt-4"), order("first:mt-4 odd:mt-2"));

  let values = TailwindValues::from_str("odd:mt-2 first:mt-4").unwrap();
  let last = cascade_order(values.inner.iter().map(Arc::as_ref))
    .pop()
    .unwrap()
    .id;

  assert_eq!(last, TwRule::parse("odd:mt-2").unwrap().id);
}
