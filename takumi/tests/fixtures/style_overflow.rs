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
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .build()
        .unwrap(),
    ),
    children: Some(
      [ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .display(Display::Block)
            .width(Px(200.0))
            .height(Px(200.0))
            .border_width(Some(Sides([Px(4.0); 4])))
            .border_style(Some(BorderStyle::Solid))
            .border_color(Some(Color([255, 0, 0, 255]).into()))
            .overflow(overflows)
            .build()
            .unwrap(),
        ),
        children: Some(
          [ImageNode {
            class_name: None,
            id: None,
            tag_name: None,
            preset: None,
            tw: None,
            style: Some(
              StyleBuilder::default()
                .width(Px(300.0))
                .height(Px(300.0))
                .border_width(Some(Sides([Px(4.0); 4])))
                .border_style(Some(BorderStyle::Solid))
                .border_color(Some(Color([0, 255, 0, 255]).into()))
                .build()
                .unwrap(),
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
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .build()
        .unwrap(),
    ),
    children: Some([ContainerNode {
        class_name: None,
        id: None,
        tag_name: None,
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .display(Display::Block)
            .width(Px(400.0))
            .height(Px(200.0))
            .border_width(Some(Sides([Px(4.0); 4])))
            .border_style(Some(BorderStyle::Solid))
            .border_color(Some(Color([0, 0, 0, 255]).into()))
            .overflow(overflows)
            .build()
            .unwrap(),
        ),
        children: Some([
          TextNode {
        class_name: None,
        id: None,
        tag_name: None,
            preset: None,
            tw: None,
            style: Some(
              StyleBuilder::default()
              .font_size(Some(Rem(4.0)))
              .color(ColorInput::Value(Color([0, 0, 0, 255])))
              .border_width(Some(Sides([Px(2.0); 4])))
              .border_style(Some(BorderStyle::Solid))
              .border_color(Some(Color([255, 0, 0, 255]).into()))
              .build()
              .unwrap(),
          ),
          text: "This is a very long text that should overflow the container and demonstrate text overflow behavior with a large font size of 4rem.".to_string(),
        }.into()].into()),
      }
      .into()].into()),
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
