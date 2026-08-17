use takumi::prelude::{Length::*, *};

use crate::test_utils::{CONTEXT, create_test_viewport, run_fixture_test_with_css};

#[test]
fn test_stylesheets() {
  let root = Node::container([Node::container([
    Node::text("Stylesheets".to_string())
      .with_tag_name("h1")
      .with_class_name("title"),
    Node::text("Selectors apply before inline styles".to_string())
      .with_tag_name("p")
      .with_class_name("subtitle"),
  ])
  .with_tag_name("section")
  .with_class_name("card")
  .with_id("hero-card")
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center)),
  )])
  .with_tag_name("div")
  .with_class_name("root")
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([245, 245, 245, 255]),
      ))),
  );

  let css = r#"
    .card {
      width: 560px;
      height: 260px;
      background-color: rgb(17, 24, 39);
      border-radius: 24px;
      padding: 32px;
      row-gap: 16px;
    }

    #hero-card {
      box-shadow: 0 16px 40px rgba(0, 0, 0, 0.25);
    }

    section .title {
      color: rgb(255, 255, 255);
      font-size: 56px;
      font-weight: 700;
      text-align: center;
    }

    section .subtitle {
      color: rgb(148, 163, 184);
      font-size: 24px;
      text-align: center;
    }
  "#;
  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(root)
    .fonts(&CONTEXT)
    .stylesheet(StyleSheet::parse(css).unwrap().into())
    .build();

  run_fixture_test_with_css(options, css, "stylesheets");
}

#[test]
fn test_stylesheets_background_multiple_gradients() {
  let root = Node::container([Node::container([])
    .with_tag_name("section")
    .with_class_name("multi-gradient-card")
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(700.0)))
        .with(StyleDeclaration::height(Px(360.0)))
        .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(24.0)); 4]))),
    )])
  .with_tag_name("div")
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([22, 22, 22, 255]),
      ))),
  );

  let css = r#"
    .multi-gradient-card {
      background: radial-gradient(circle at 80% 20%, #FF3D00 0%, transparent 40%), radial-gradient(circle at 20% 80%, #00E5FF 0%, transparent 40%);
      box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
    }
  "#;
  let build_options = || {
    RenderOptions::builder()
      .viewport(create_test_viewport())
      .node(root.clone())
      .fonts(&CONTEXT)
      .stylesheet(StyleSheet::parse(css).unwrap().into())
      .build()
  };

  run_fixture_test_with_css(
    build_options(),
    css,
    "stylesheets_background_multiple_gradients",
  );
}
