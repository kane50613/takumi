use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

fn create_overflow_fixture(overflows: SpacePair<Overflow>) -> NodeKind {
  ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        )))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center)),
    ),
    children: Some(
      [ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::display(Display::Block))
            .with(StyleDeclaration::width(Px(200.0)))
            .with(StyleDeclaration::height(Px(200.0)))
            .with_border_width(Sides([Px(4.0); 4]))
            .with(StyleDeclaration::border_style(BorderStyle::Solid))
            .with(StyleDeclaration::border_color(
              Color([255, 0, 0, 255]).into(),
            ))
            .with_overflow(overflows),
        ),
        children: Some(
          [ImageNode {
            class_name: None,
            id: None,
            tag_name: None,
            preset: None,
            tw: None,
            style: Some(
              Style::default()
                .with(StyleDeclaration::width(Px(300.0)))
                .with(StyleDeclaration::height(Px(300.0)))
                .with_border_width(Sides([Px(4.0); 4]))
                .with(StyleDeclaration::border_style(BorderStyle::Solid))
                .with(StyleDeclaration::border_color(
                  Color([0, 255, 0, 255]).into(),
                )),
            ),
            width: None,
            height: None,
            src: "assets/images/yeecord.png".into(),
          }
          .into()]
          .into(),
        ),
      }
      .into()]
      .into(),
    ),
  }
  .into()
}

fn create_text_overflow_fixture(overflows: SpacePair<Overflow>) -> NodeKind {
  ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(Color::white())))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with(StyleDeclaration::justify_content(JustifyContent::Center)),
    ),
    children: Some(
      [ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          Style::default()
            .with(StyleDeclaration::display(Display::Block))
            .with(StyleDeclaration::width(Px(400.0)))
            .with(StyleDeclaration::height(Px(200.0)))
            .with_border_width(Sides([Px(4.0); 4]))
            .with(StyleDeclaration::border_style(BorderStyle::Solid))
            .with(StyleDeclaration::border_color(Color([0, 0, 0, 255]).into()))
            .with_overflow(overflows),
        ),
        children: Some(
          [TextNode {
            class_name: None,
            id: None,
            tag_name: None,
            preset: None,
            tw: None,
            style: Some(
              Style::default()
                .with(StyleDeclaration::font_size(Rem(4.0).into()))
                .with(StyleDeclaration::color(ColorInput::Value(Color([0, 0, 0, 255]))))
                .with_border_width(Sides([Px(2.0); 4]))
                .with(StyleDeclaration::border_style(BorderStyle::Solid))
                .with(StyleDeclaration::border_color(Color([255, 0, 0, 255]).into())),
            ),
            text: "This is a very long text that should overflow the container and demonstrate text overflow behavior with a large font size of 4rem.".to_string(),
          }
          .into()]
          .into(),
        ),
      }
      .into()]
      .into(),
    ),
  }
  .into()
}

#[test]
fn test_style_overflow_visible() {
  let container = create_overflow_fixture(SpacePair::from_single(Overflow::Visible));

  run_fixture_test(container, "style_overflow_visible_image");
}

#[test]
fn test_overflow_hidden() {
  let container = create_overflow_fixture(SpacePair::from_single(Overflow::Hidden));

  run_fixture_test(container, "style_overflow_hidden_image");
}

#[test]
fn test_overflow_clip() {
  let container = create_overflow_fixture(SpacePair::from_single(Overflow::Clip));

  run_fixture_test(container, "style_overflow_clip_image");
}

#[test]
fn test_overflow_mixed_axes() {
  let container =
    create_overflow_fixture(SpacePair::from_pair(Overflow::Hidden, Overflow::Visible));

  run_fixture_test(container, "style_overflow_hidden_visible_image");
}

#[test]
fn test_text_overflow_visible() {
  let container = create_text_overflow_fixture(SpacePair::from_single(Overflow::Visible));

  run_fixture_test(container, "style_overflow_visible_text");
}

#[test]
fn test_text_overflow_hidden() {
  let container = create_text_overflow_fixture(SpacePair::from_single(Overflow::Hidden));

  run_fixture_test(container, "style_overflow_hidden_text");
}

#[test]
fn test_text_overflow_clip() {
  let container = create_text_overflow_fixture(SpacePair::from_single(Overflow::Clip));

  run_fixture_test(container, "style_overflow_clip_text");
}

#[test]
fn test_text_overflow_mixed_axes() {
  let container =
    create_text_overflow_fixture(SpacePair::from_pair(Overflow::Hidden, Overflow::Visible));

  run_fixture_test(container, "style_overflow_hidden_visible_text");
}
