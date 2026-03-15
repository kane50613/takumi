use takumi::layout::{
  node::Node,
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

fn create_luma_logo_container() -> Node {
  Node::container([Node::image("assets/images/luma.svg").with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(204.0)))
      .with(StyleDeclaration::height(Px(76.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::Contain)),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_image(Some(
        BackgroundImages::from_str("linear-gradient(135deg, #2d3748 0%, #1a202c 100%)").unwrap(),
      )))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center)),
  )
}

#[test]
fn test_svg_luma_logo_gradient_background() {
  run_fixture_test(
    create_luma_logo_container(),
    "svg_luma_logo_gradient_background",
  );
}

#[test]
fn test_svg_attr_size_in_absolute_flex_container() {
  let svg = r##"<svg width="100" height="100" viewBox="0 0 40 40" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M20 0L24.4903 15.5097L40 20L24.4903 24.4903L20 40L15.5097 24.4903L0 20L15.5097 15.5097L20 0Z" fill="#E0FF25"/></svg>"##;

  let node: Node = Node::container([Node::container([Node::image(svg).with_tag_name("svg")])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::position(Position::Absolute))
        .with_inset(Sides([Auto, Px(40.0), Px(40.0), Auto]))
        .with(StyleDeclaration::display(Display::Flex)),
    )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([35, 35, 35, 255]),
      ))),
  );

  run_fixture_test(node, "svg_attr_size_in_absolute_flex_container");
}

#[test]
fn test_svg_current_color_fixture() {
  let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="120"><rect x="0" y="0" width="120" height="120" fill="currentColor"/></svg>"#;

  let swatch = |color: Color| {
    let children: Vec<Node> = vec![
      Node::image(svg).with_tag_name("svg").with_style(
        Style::default()
          .with(StyleDeclaration::width(Px(120.0)))
          .with(StyleDeclaration::height(Px(120.0))),
      ),
      Node::text("Hello"),
    ];

    let container: Node = Node::container(children).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(160.0)))
        .with(StyleDeclaration::height(Px(160.0)))
        .with_padding(Sides([Px(20.0); 4]))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([240, 240, 240, 255]),
        )))
        .with(StyleDeclaration::color(ColorInput::Value(color)))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::align_items(AlignItems::Center)),
    );

    container
  };

  let node: Node = Node::container([
    swatch(Color([230, 40, 70, 255])),
    swatch(Color([60, 140, 255, 255])),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with_gap(SpacePair::from_single(Px(24.0)))
      .with_padding(Sides([Px(40.0); 4]))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([30, 30, 30, 255]),
      ))),
  );

  run_fixture_test(node, "svg_current_color_fixture");
}

#[test]
fn test_twemoji_svg() {
  // https://github.com/nuxt-modules/og-image/blob/0209474b99e1ffa8a9010df359f170563024056f/src/runtime/server/og-image/core/transforms/emojis/fetch.ts#L54
  fn create_svg_node(svg: &str) -> Node {
    Node::image(svg).with_tag_name("svg").with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::width(Px(48.0)))
        .with(StyleDeclaration::vertical_align(VerticalAlign::Length(Em(
          -0.1,
        ))))
        .with_padding_inline(SpacePair::from_single(Px(4.0))),
    )
  }

  let children: Vec<Node> = vec![
    Node::text("Laboris ex do ipsum. Quis mollit magna anim elit reprehenderit consequat irure ex duis adipisicing.".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      ,
    create_svg_node(include_str!(
      "../../../assets/images/twemoji/grinning-squinting-face.svg"
    )),
    create_svg_node(include_str!("../../../assets/images/twemoji/hamburger.svg")),
    create_svg_node(include_str!(
      "../../../assets/images/twemoji/waving-hand.svg"
    )),
    Node::text("Ullamco occaecat anim mollit magna laborum elit ea tempor fugiat sit qui.".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
      ,
  ];

  let node: Node = Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Block))
      .with_padding(Sides([Px(40.0); 4]))
      .with(StyleDeclaration::font_size(Px(48.0).into()))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(node, "svg_twemoji");
}
