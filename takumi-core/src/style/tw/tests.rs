use std::assert_matches;

use super::*;
use crate::style::{ComputedStyle, LonghandId, Style, properties::BackgroundImage};

#[test]
fn test_box_sizing() {
  assert_eq!(
    TailwindProperty::parse("box-border"),
    Some(TailwindProperty::BoxSizing(BoxSizing::BorderBox))
  );
}

#[test]
fn test_parse_width() {
  assert_eq!(
    TailwindProperty::parse("w-64"),
    Some(TailwindProperty::Width(Length::from_spacing(64.0)))
  );
  assert_eq!(
    TailwindProperty::parse("h-32"),
    Some(TailwindProperty::Height(Length::from_spacing(32.0)))
  );
  assert_eq!(
    TailwindProperty::parse("justify-self-center"),
    Some(TailwindProperty::JustifySelf(AlignItems::Center))
  );
}

#[test]
fn test_parse_color() {
  assert_eq!(
    TailwindProperty::parse("text-black/30"),
    Some(TailwindProperty::Color(ColorInput::Value(Color([
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
      TailwindProperty::ShadowColor(ColorInput::Value(Color([251, 44, 54, 255]))),
    ),
    (
      "text-shadow-red-500",
      TailwindProperty::TextShadowColor(ColorInput::Value(Color([251, 44, 54, 255]))),
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
    assert_eq!(
      TailwindProperty::parse(input),
      Some(expected.clone()),
      "{input}"
    );
  }
}

#[test]
fn test_parse_text_decoration_lines() {
  assert_eq!(
    TailwindProperty::parse("underline"),
    Some(TailwindProperty::TextDecorationLine(
      TextDecorationLines::UNDERLINE
    ))
  );
  assert_eq!(
    TailwindProperty::parse("no-underline"),
    Some(TailwindProperty::TextDecorationLine(
      TextDecorationLines::empty()
    ))
  );
}

#[test]
fn test_parse_arbitrary_color() {
  assert_eq!(
    TailwindProperty::parse("text-[rgb(0, 191, 255)]"),
    Some(TailwindProperty::Color(ColorInput::Value(Color([
      0, 191, 255, 255
    ]))))
  );
}

#[test]
fn test_parse_arbitrary_mask_image_url() {
  assert_eq!(
    TailwindProperty::parse("mask-[url('https://example.com/logo.svg')]"),
    Some(TailwindProperty::MaskImage(BackgroundImage::Url(
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
    TailwindProperty::parse("mask-[url('https://example.com/my_logo.svg')]"),
    Some(TailwindProperty::MaskImage(BackgroundImage::Url(
      "https://example.com/my_logo.svg".into()
    )))
  );
}

#[test]
fn test_parse_value_arbitrary_url_with_scheme_colon() {
  let url_image =
    TailwindProperty::MaskImage(BackgroundImage::Url("https://example.com/a.svg".into()));

  assert_eq!(
    TailwindValue::parse("mask-[url('https://example.com/a.svg')]"),
    Some(TailwindValue {
      property: TailwindProperty::MaskImage(BackgroundImage::Url(
        "https://example.com/a.svg".into(),
      )),
      breakpoint: None,
      important: false,
    })
  );

  assert_eq!(
    TailwindValue::parse("md:mask-[url('https://example.com/a.svg')]"),
    Some(TailwindValue {
      property: url_image,
      breakpoint: Some(Breakpoint(Length::Rem(48.0))),
      important: false,
    })
  );
}

#[test]
fn test_parse_arbitrary_flex_with_spaces() {
  assert_eq!(
    TailwindProperty::parse("flex-[3_1_auto]"),
    Some(TailwindProperty::Flex(Flex {
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
    Some(TailwindProperty::Animation(animations))
      if animations.as_ref() == [Animation {
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
    Some(TailwindProperty::Animation(animations))
      if animations.as_ref() == [Animation {
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
    TailwindProperty::parse("-ml-4"),
    Some(TailwindProperty::MarginLeft(Length::from_spacing(-4.0)))
  );
}

#[test]
fn test_parse_border_radius() {
  assert_eq!(
    TailwindProperty::parse("rounded-xs"),
    Some(TailwindProperty::Rounded(TwRounded(Length::Rem(0.125))))
  );
  assert_eq!(
    TailwindProperty::parse("rounded-full"),
    Some(TailwindProperty::Rounded(TwRounded(Length::Px(9999.0))))
  );
}

#[test]
fn test_parse_font_size_with_arbitrary_line_height() {
  assert_eq!(
    TailwindProperty::parse("text-base/[12.34]"),
    Some(TailwindProperty::FontSize(TwFontSize {
      font_size: (Length::Rem(1.0).into()),
      line_height: Some(LineHeight::Unitless(12.34)),
    }))
  );
}

#[test]
fn test_parse_border_width() {
  assert_eq!(
    TailwindProperty::parse("border"),
    Some(TailwindProperty::BorderDefault)
  );
  assert_eq!(
    TailwindProperty::parse("border-b"),
    Some(TailwindProperty::BorderBottomWidth(LineWidth::Length(
      Length::Px(1.0)
    )))
  );
  assert_eq!(
    TailwindProperty::parse("border-y"),
    Some(TailwindProperty::BorderYWidth(LineWidth::Length(
      Length::Px(1.0)
    )))
  );
  assert_eq!(
    TailwindProperty::parse("border-t-2"),
    Some(TailwindProperty::BorderTopWidth(LineWidth::Length(
      Length::Px(2.0)
    )))
  );
  assert_eq!(
    TailwindProperty::parse("border-x-4"),
    Some(TailwindProperty::BorderXWidth(LineWidth::Length(
      Length::Px(4.0)
    )))
  );
  assert_eq!(
    TailwindProperty::parse("border-solid"),
    Some(TailwindProperty::BorderStyle(BorderStyle::Solid))
  );
  assert_eq!(
    TailwindProperty::parse("border-dashed"),
    Some(TailwindProperty::BorderStyle(BorderStyle::Dashed))
  );
  assert_eq!(
    TailwindProperty::parse("border-dotted"),
    Some(TailwindProperty::BorderStyle(BorderStyle::Dotted))
  );
  assert_eq!(
    TailwindProperty::parse("border-none"),
    Some(TailwindProperty::BorderStyle(BorderStyle::None))
  );
}

#[test]
fn test_parse_outline() {
  assert_eq!(
    TailwindProperty::parse("outline"),
    Some(TailwindProperty::OutlineDefault)
  );
  assert_eq!(
    TailwindProperty::parse("outline-2"),
    Some(TailwindProperty::OutlineWidth(LineWidth::Length(
      Length::Px(2.0)
    )))
  );
  assert_eq!(
    TailwindProperty::parse("outline-red-500"),
    Some(TailwindProperty::OutlineColor(ColorInput::Value(Color([
      251, 44, 54, 255
    ]))))
  );
  assert_eq!(
    TailwindProperty::parse("outline-solid"),
    Some(TailwindProperty::OutlineStyle(BorderStyle::Solid))
  );
  assert_eq!(
    TailwindProperty::parse("outline-dashed"),
    Some(TailwindProperty::OutlineStyle(BorderStyle::Dashed))
  );
  assert_eq!(
    TailwindProperty::parse("outline-offset-4"),
    Some(TailwindProperty::OutlineOffset(LineWidth::Length(
      Length::Px(4.0)
    )))
  );
  assert_eq!(
    TailwindProperty::parse("outline-none"),
    Some(TailwindProperty::OutlineStyle(BorderStyle::None))
  );
}

#[test]
fn test_parse_col_end() {
  assert_eq!(
    TailwindProperty::parse("col-end-1"),
    Some(TailwindProperty::GridColumnEnd(GridPlacement::Line(1)))
  );
}

#[test]
fn test_grid_column_start_emits_only_start_longhand() {
  let values = TailwindValues::from_str("col-start-2").expect("tailwind values should parse");
  let declarations = values.into_declaration_block(Viewport::new((100, 100)));

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::grid_column_start(GridPlacement::Line(2))]
  );
}

#[test]
fn test_grid_row_end_emits_only_end_longhand() {
  let values = TailwindValues::from_str("row-end-3").expect("tailwind values should parse");
  let declarations = values.into_declaration_block(Viewport::new((100, 100)));

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![&StyleDeclaration::grid_row_end(GridPlacement::Line(3))]
  );
}

#[test]
fn test_grid_longhand_importance_is_tracked_per_side() {
  let values =
    TailwindValues::from_str("col-end-3 !col-start-2").expect("tailwind values should parse");
  let declarations = values.into_declaration_block(Viewport::new((100, 100)));

  assert_eq!(
    declarations.iter().collect::<Vec<_>>(),
    vec![
      &StyleDeclaration::grid_column_start(GridPlacement::Line(2)),
      &StyleDeclaration::grid_column_end(GridPlacement::Line(3)),
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
    assert_eq!(
      TailwindProperty::parse(input),
      Some(expected.clone()),
      "{input}"
    );
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
      TailwindProperty::parse(class).is_some(),
      "Expected '{}' to parse successfully",
      class
    );
  }

  for class in should_not_parse {
    assert!(
      TailwindProperty::parse(class).is_none(),
      "Expected '{}' to fail parsing",
      class
    );
  }
}

#[test]
fn test_breakpoint_matches() {
  let viewport = Viewport::new((1000, 1000));

  assert!(Breakpoint::parse("sm").is_some_and(|bp| bp.matches(viewport)));
}

#[test]
fn test_breakpoint_does_not_match() {
  let viewport = Viewport::new((1000, 1000));

  // 80 * 16 = 1280 > 1000
  assert!(Breakpoint::parse("xl").is_some_and(|bp| !bp.matches(viewport)));
}

#[test]
fn test_value_parsing() {
  assert_eq!(
    TailwindValue::parse("md:!mt-4"),
    Some(TailwindValue {
      property: TailwindProperty::MarginTop(Length::Rem(1.0)),
      breakpoint: Some(Breakpoint(Length::Rem(48.0))),
      important: true,
    })
  );
}

#[test]
fn test_values_sorting() {
  assert_eq!(
    TailwindValues::from_str("md:!mt-4 sm:mt-8 !mt-12 mt-16"),
    Ok(TailwindValues {
      inner: vec![
        // mt-16
        TailwindValue {
          property: TailwindProperty::MarginTop(Length::Rem(4.0)),
          breakpoint: None,
          important: false,
        },
        // sm:mt-8
        TailwindValue {
          property: TailwindProperty::MarginTop(Length::Rem(2.0)),
          breakpoint: Some(Breakpoint(Length::Rem(40.0))),
          important: false,
        },
        // !mt-12
        TailwindValue {
          property: TailwindProperty::MarginTop(Length::Rem(3.0)),
          breakpoint: None,
          important: true,
        },
        // md:!mt-4
        TailwindValue {
          property: TailwindProperty::MarginTop(Length::Rem(1.0)),
          breakpoint: Some(Breakpoint(Length::Rem(48.0))),
          important: true,
        },
      ]
    })
  )
}

#[test]
fn test_filters_append() {
  use crate::style::properties::Filter;

  let values = TailwindValues::from_str("blur-sm brightness-150 contrast-125")
    .expect("tailwind values should parse");
  let viewport = Viewport::new((100, 100));

  let style =
    Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());

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

  let style =
    Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());

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
    TailwindProperty::parse("mix-blend-multiply"),
    Some(TailwindProperty::MixBlendMode(BlendMode::Multiply))
  );
  assert_eq!(
    TailwindProperty::parse("bg-blend-screen"),
    Some(TailwindProperty::BackgroundBlendMode(BlendMode::Screen))
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
      TailwindProperty::parse(input),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Keyword(*kw))),
      "{input}"
    );
  }
  // Arbitrary-value lengths
  assert_eq!(
    TailwindProperty::parse("align-[10px]"),
    Some(TailwindProperty::VerticalAlign(VerticalAlign::Length(
      Length::Px(10.0)
    )))
  );
  assert_eq!(
    TailwindProperty::parse("align-[25%]"),
    Some(TailwindProperty::VerticalAlign(VerticalAlign::Length(
      Length::Percentage(25.0)
    )))
  );
  assert_eq!(
    TailwindProperty::parse("align-[-0.5em]"),
    Some(TailwindProperty::VerticalAlign(VerticalAlign::Length(
      Length::Em(-0.5)
    )))
  );
}

#[test]
fn test_parse_decoration_thickness() {
  assert_eq!(
    TailwindProperty::parse("decoration-4"),
    Some(TailwindProperty::TextDecorationThickness(
      TextDecorationThickness::Length(Length::Px(4.0))
    ))
  );
  assert_eq!(
    TailwindProperty::parse("decoration-auto"),
    Some(TailwindProperty::TextDecorationThickness(
      TextDecorationThickness::Length(Length::Auto)
    ))
  );
  assert_eq!(
    TailwindProperty::parse("decoration-from-font"),
    Some(TailwindProperty::TextDecorationThickness(
      TextDecorationThickness::FromFont
    ))
  );
  assert_eq!(
    TailwindProperty::parse("decoration-[3px]"),
    Some(TailwindProperty::TextDecorationThickness(
      TextDecorationThickness::Length(Length::Px(3.0))
    ))
  );
}

#[test]
fn test_linear_gradient_apply() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("bg-linear-to-r from-red-500 via-green-500 to-blue-500")
    .expect("tailwind values should parse");

  let style =
    Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());

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
    let style =
      Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());

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
    let style =
      Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());

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
    TailwindProperty::parse("rounded"),
    Some(TailwindProperty::Rounded(TwRounded(Length::Rem(0.25))))
  );
}

#[test]
fn test_negative_color_is_rejected() {
  assert_eq!(TailwindProperty::parse("-bg-red-500"), None);
  assert_eq!(TailwindProperty::parse("-text-blue-500"), None);
}

#[test]
fn test_negative_grid_line() {
  assert_eq!(
    TailwindProperty::parse("-col-start-1"),
    Some(TailwindProperty::GridColumnStart(GridPlacement::Line(-1)))
  );
}

#[test]
fn test_logical_resolves_to_physical_ltr() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("ms-4 me-2 ps-3 pe-1").unwrap();
  let style =
    Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());
  assert_eq!(style.margin_left, Length::from_spacing(4.0));
  assert_eq!(style.margin_right, Length::from_spacing(2.0));
  assert_eq!(style.padding_left, Length::from_spacing(3.0));
  assert_eq!(style.padding_right, Length::from_spacing(1.0));
}

#[test]
fn test_logical_physical_cascade_order_ltr() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("ms-2 ml-4").unwrap();
  let style =
    Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());
  assert_eq!(style.margin_left, Length::from_spacing(4.0));

  let values = TailwindValues::from_str("ml-4 ms-2").unwrap();
  let style =
    Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());
  assert_eq!(style.margin_left, Length::from_spacing(2.0));
}

#[test]
fn test_logical_resolves_to_physical_rtl() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("ms-4 me-2 ps-3 pe-1").unwrap();
  let mut block = values.into_declaration_block(viewport);
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
  let mut block = values.into_declaration_block(viewport);
  block.push(StyleDeclaration::direction(Direction::Rtl), false);
  let style = Style::from(block).inherit(&ComputedStyle::default());
  assert_eq!(style.margin_right, Length::from_spacing(4.0));
  assert_eq!(style.margin_left, Length::Px(0.0));
}

#[test]
fn test_filter_none_clears_previous_filters() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("blur-sm brightness-150 filter-none").unwrap();
  let style =
    Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());
  assert_eq!(style.filter, Filters::default());

  let values = TailwindValues::from_str("backdrop-blur-sm backdrop-filter-none").unwrap();
  let style =
    Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());
  assert_eq!(style.backdrop_filter, Filters::default());
}

#[test]
fn test_font_numeric_weight() {
  assert_eq!(
    TailwindProperty::parse("font-700"),
    Some(TailwindProperty::FontWeight(FontWeight::from(700.0)))
  );
}

#[test]
fn test_line_clamp_none() {
  assert_eq!(
    TailwindProperty::parse("line-clamp-none"),
    Some(TailwindProperty::LineClamp(LineClamp::default()))
  );
}

#[test]
fn test_bg_auto() {
  assert_eq!(
    TailwindProperty::parse("bg-auto"),
    Some(TailwindProperty::BackgroundSize(BackgroundSize::Explicit {
      width: Length::Auto,
      height: Length::Auto,
    }))
  );
}

#[test]
fn test_bg_repeat_v4_names() {
  assert_eq!(
    TailwindProperty::parse("bg-repeat-round"),
    Some(TailwindProperty::BackgroundRepeat(BackgroundRepeat::round()))
  );
  assert_eq!(
    TailwindProperty::parse("bg-repeat-space"),
    Some(TailwindProperty::BackgroundRepeat(BackgroundRepeat::space()))
  );
}

#[test]
fn test_grid_cols_none() {
  assert_eq!(
    TailwindProperty::parse("grid-cols-none"),
    Some(TailwindProperty::GridTemplateColumns(TwGridTemplate(
      GridTemplateComponents::default()
    )))
  );
}

#[test]
fn test_col_auto_row_auto() {
  assert_eq!(
    TailwindProperty::parse("col-auto"),
    Some(TailwindProperty::GridColumn(GridLine {
      start: GridPlacement::auto(),
      end: GridPlacement::auto(),
    }))
  );
  assert_eq!(
    TailwindProperty::parse("row-auto"),
    Some(TailwindProperty::GridRow(GridLine {
      start: GridPlacement::auto(),
      end: GridPlacement::auto(),
    }))
  );
}

#[test]
fn test_shadow_md_is_composite() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("shadow-md").unwrap();
  let style =
    Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());
  assert_eq!(style.box_shadow.as_ref().map(|s| s.len()), Some(2));
}

#[test]
fn test_text_shadow_sm_is_composite() {
  let viewport = Viewport::new((100, 100));
  let values = TailwindValues::from_str("text-shadow-sm").unwrap();
  let style =
    Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());
  assert_eq!(style.text_shadow.as_ref().map(|s| s.len()), Some(3));
}

#[test]
fn test_shadow_none_overrides_color_in_either_order() {
  let viewport = Viewport::new((100, 100));
  for classes in ["shadow-none shadow-red-500", "shadow-red-500 shadow-none"] {
    let values = TailwindValues::from_str(classes).unwrap();
    let style =
      Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());
    assert_eq!(style.box_shadow, None, "case: {classes}");
  }
}

#[test]
fn test_bg_conic_standalone() {
  assert_eq!(
    TailwindProperty::parse("bg-conic"),
    Some(TailwindProperty::BgConicAngle(Angle::zero()))
  );
}

#[test]
fn test_gradient_stop_position_is_used_in_apply() {
  let viewport = Viewport::new((100, 100));
  let values =
    TailwindValues::from_str("bg-linear-to-r from-red-500 from-10% to-blue-500 to-80%").unwrap();
  let style =
    Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());
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
        .into_declaration_block(viewport),
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
    TailwindProperty::parse("border-t-blue-500"),
    Some(TailwindProperty::BorderTopColor(ColorInput::Value(Color(
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
      TailwindProperty::parse(token),
      Some(expected),
      "failed for {token}"
    );
  }

  assert_matches!(
    TailwindProperty::parse("list-image-[url(marker.png)]"),
    Some(TailwindProperty::ListStyleImage(image))
      if matches!(image.image(), Some(BackgroundImage::Url(url)) if &**url == "marker.png")
  );
}
