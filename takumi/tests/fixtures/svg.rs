use takumi::prelude::{Length::*, *};

use crate::test_utils::run_fixture_test;

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

// https://github.com/kane50613/takumi/issues/1058
#[test]
fn test_svg_current_color_inherits_host_color() {
  let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="96" height="96"><path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 17V3m-6 8l6 6l6-6m1 10H5"/></svg>"#;

  let node: Node = Node::container([Node::image(svg).with_tag_name("svg")]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        239, 68, 68, 255,
      ]))))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(node, "svg_current_color_inherits_host_color");
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
