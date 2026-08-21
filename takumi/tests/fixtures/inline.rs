use takumi::prelude::{Length::*, *};

use crate::test_utils::run_fixture_test;

#[test]
fn inline_image() {
  // Inline image should behave as inline-level box content
  let children: Vec<Node> = vec![
    Node::text("Before ".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    Node::image(("assets/images/yeecord.png", 64.0, 64.0)).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Inline))
        .with_border_width(Sides([Px(12.0).into(); 4]))
        .with_border_style(Sides([BorderStyle::Solid; 4]))
        .with_border_color(Sides([ColorInput::Value(Color::transparent()); 4]))
        .with(StyleDeclaration::background_image(
          BackgroundImages::from_css_str("linear-gradient(to right, red, blue)").ok(),
        ))
        .with(StyleDeclaration::background_clip(
          BackgroundClip::BorderArea,
        )),
    ),
    Node::text(" After".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ];

  let container = Node::container([Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with_border_width(Sides([Px(2.0).into(); 4]))
      .with_border_style(Sides([BorderStyle::Solid; 4]))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::font_size(Px(48.0).into())),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with_white_space(WhiteSpace::pre()),
  );

  run_fixture_test(container, "inline_image");
}
