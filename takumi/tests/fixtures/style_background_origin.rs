use takumi::prelude::{Length::*, *};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_background_origin() {
  let make_box = |label: &str, origin: BackgroundOrigin| -> Node {
    Node::container([Node::text(label)]).with_style(
      Style::default()
        .with_border_width(Sides([Px(40.0).into(); 4]))
        .with_border_style(Sides([BorderStyle::Solid; 4]))
        .with_border_color(Sides([ColorInput::Value(Color([120, 120, 200, 120])); 4]))
        .with_padding(Sides([Px(30.0); 4]))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(280.0)))
        .with(StyleDeclaration::height(Px(160.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([230, 230, 230, 255]),
        )))
        .with(StyleDeclaration::background_image(Some(
          BackgroundImages::from_css_str("linear-gradient(#d00, #d00)").unwrap(),
        )))
        .with(StyleDeclaration::background_size(
          BackgroundSizes::from_css_str("70px 70px").unwrap(),
        ))
        .with(StyleDeclaration::background_repeat(
          BackgroundRepeats::from_css_str("no-repeat").unwrap(),
        ))
        .with(StyleDeclaration::background_position(
          PositionValues::from_css_str("left top").unwrap(),
        ))
        .with(StyleDeclaration::background_origin(origin)),
    )
  };

  let container = Node::container([
    make_box("border-box", BackgroundOrigin::BorderBox),
    make_box("padding-box", BackgroundOrigin::PaddingBox),
    make_box("content-box", BackgroundOrigin::ContentBox),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::row_gap(Px(20.0).into()))
      .with_padding(Sides([Px(30.0); 4]))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_background_origin");
}
