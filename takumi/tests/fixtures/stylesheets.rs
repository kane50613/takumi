use takumi::layout::{
  node::{ContainerNode, TextNode},
  style::{Length::*, *},
};
use takumi::rendering::RenderOptionsBuilder;

use crate::test_utils::{CONTEXT, create_test_viewport, run_fixture_test_with_options};

#[test]
fn test_stylesheets() {
  let root = ContainerNode {
    class_name: Some("root".into()),
    id: None,
    tag_name: Some("div".into()),
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([245, 245, 245, 255]),
        ))),
    ),
    children: Some(
      [ContainerNode {
        class_name: Some("card".into()),
        id: Some("hero-card".into()),
        tag_name: Some("section".into()),
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::flex_direction(FlexDirection::Column))
            .with(StyleDeclaration::justify_content(JustifyContent::Center))
            .with(StyleDeclaration::align_items(AlignItems::Center)),
        ),
        children: Some(
          [
            TextNode {
              class_name: Some("title".into()),
              id: None,
              tag_name: Some("h1".into()),
              preset: None,
              tw: None,
              style: None,
              text: "Stylesheets".to_string(),
            }
            .into(),
            TextNode {
              class_name: Some("subtitle".into()),
              id: None,
              tag_name: Some("p".into()),
              preset: None,
              tw: None,
              style: None,
              text: "Selectors apply before inline styles".to_string(),
            }
            .into(),
          ]
          .into(),
        ),
      }
      .into()]
      .into(),
    ),
  };

  let options = RenderOptionsBuilder::default()
    .viewport(create_test_viewport())
    .node(root.into())
    .global(&CONTEXT)
    .stylesheets(vec![
      r#"
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
        "#
      .to_string(),
    ])
    .build()
    .unwrap();

  run_fixture_test_with_options(options, "stylesheets");
}

#[test]
fn test_stylesheets_background_multiple_gradients() {
  let root = ContainerNode {
    class_name: None,
    id: None,
    tag_name: Some("div".into()),
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([22, 22, 22, 255]),
        ))),
    ),
    children: Some(
      [ContainerNode {
        class_name: Some("multi-gradient-card".into()),
        id: None,
        tag_name: Some("section".into()),
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::width(Px(700.0)))
            .with(StyleDeclaration::height(Px(360.0)))
            .with_border_radius(Box::new(BorderRadius(Sides(
              [SpacePair::from_single(Px(24.0)); 4],
            )))),
        ),
        children: None,
      }
      .into()]
      .into(),
    ),
  };

  let build_options = || {
    RenderOptionsBuilder::default()
      .viewport(create_test_viewport())
      .node(root.clone().into())
      .global(&CONTEXT)
      .stylesheets(vec![
        r#"
          .multi-gradient-card {
            background: radial-gradient(circle at 80% 20%, #FF3D00 0%, transparent 40%), radial-gradient(circle at 20% 80%, #00E5FF 0%, transparent 40%);
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
          }
        "#
        .to_string(),
      ])
      .build()
      .unwrap()
  };

  run_fixture_test_with_options(build_options(), "stylesheets_background_multiple_gradients");
}
