use takumi::prelude::{Length::*, *};

use crate::test_utils::run_fixture_test;

/// The inline box and the outline are both placed in the container's own
/// coordinates, so the container's rotation has to apply to them too. A
/// device-space offset would drop the box off its line and skew the ring.
#[test]
fn test_inline_box_in_rotated_container() {
  let inline_block = Node::container([Node::text("box".to_string())]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::InlineBlock))
      .with(StyleDeclaration::width(Px(120.0)))
      .with(StyleDeclaration::height(Px(48.0)))
      .with_margin(Sides([Px(8.0); 4]))
      .with(StyleDeclaration::color(ColorInput::Value(Color::white())))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([220, 38, 38, 255]),
      ))),
  );

  let rotated = Node::container([
    Node::text("before ".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    inline_block,
    Node::text(" after".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::rotate(Some(Angle::new(30.0))))
      .with(StyleDeclaration::width(Px(520.0)))
      .with(StyleDeclaration::font_size(Px(28.0).into()))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([226, 232, 240, 255]),
      )))
      .with(StyleDeclaration::outline_style(BorderStyle::Solid))
      .with(StyleDeclaration::outline_width(Px(6.0).into()))
      .with(StyleDeclaration::outline_offset(Px(10.0)))
      .with(StyleDeclaration::outline_color(ColorInput::Value(Color([
        22, 163, 74, 255,
      ])))),
  );

  let container = Node::container([rotated]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(container, "inline_box_in_rotated_container");
}
