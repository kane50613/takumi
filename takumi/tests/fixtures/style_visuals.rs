use takumi::layout::{
  node::Node,
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_background_color() {
  let container = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 255]),
      ))),
  );

  run_fixture_test(container, "style_background_color");
}

#[test]
fn test_style_border_radius() {
  let container = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 255]),
      )))
      .with_border_radius(Box::new(BorderRadius(Sides(
        [SpacePair::from_single(Px(20.0)); 4],
      )))),
  );

  run_fixture_test(container, "style_border_radius");
}

#[test]
fn test_style_border_radius_per_corner() {
  let container = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 255]),
      )))
      .with(StyleDeclaration::border_top_left_radius(
        SpacePair::from_single(Px(40.0)),
      ))
      .with(StyleDeclaration::border_top_right_radius(
        SpacePair::from_single(Px(10.0)),
      ))
      .with(StyleDeclaration::border_bottom_right_radius(
        SpacePair::from_single(Px(80.0)),
      ))
      .with(StyleDeclaration::border_bottom_left_radius(
        SpacePair::from_single(Px(0.0)),
      )),
  );

  run_fixture_test(container, "style_border_radius_per_corner");
}

#[test]
fn test_style_border_width() {
  let container = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with_border_width(Sides([Px(10.0); 4]))
      .with(StyleDeclaration::border_style(BorderStyle::Solid))
      .with(StyleDeclaration::border_color(ColorInput::Value(Color([
        255, 0, 0, 255,
      ])))),
  );

  run_fixture_test(container, "style_border_width");
}

#[test]
fn test_style_border_width_with_radius() {
  let container = Node::container([Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Rem(16.0)))
      .with(StyleDeclaration::height(Rem(8.0)))
      .with_border_radius(Box::new(BorderRadius(Sides(
        [SpacePair::from_single(Px(10.0)); 4],
      ))))
      .with(StyleDeclaration::border_color(ColorInput::Value(Color([
        255, 0, 0, 255,
      ]))))
      .with_border_width(Sides([Px(4.0); 4]))
      .with(StyleDeclaration::border_style(BorderStyle::Solid)),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with_padding(Sides([Rem(4.0); 4]))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(container, "style_border_width_with_radius");
}

#[test]
fn test_style_box_shadow() {
  let container = Node::container([Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(100.0)))
      .with(StyleDeclaration::height(Px(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 255]),
      )))
      .with(StyleDeclaration::box_shadow(Some(
        vec![
          BoxShadow::builder()
            .color(ColorInput::Value(Color([0, 0, 0, 128])))
            .offset_x(Px(5.0))
            .offset_y(Px(5.0))
            .blur_radius(Px(10.0))
            .spread_radius(Px(0.0))
            .inset(false)
            .build(),
        ]
        .into_boxed_slice(),
      ))),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_box_shadow");
}

#[test]
fn test_style_box_shadow_inset() {
  let container = Node::container([Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(120.0)))
      .with(StyleDeclaration::height(Px(80.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with_border_radius(Box::new(BorderRadius(Sides(
        [SpacePair::from_single(Px(16.0)); 4],
      ))))
      .with(StyleDeclaration::box_shadow(Some(
        vec![
          BoxShadow::builder()
            .color(ColorInput::Value(Color([0, 0, 0, 153])))
            .offset_x(Px(4.0))
            .offset_y(Px(6.0))
            .blur_radius(Px(18.0))
            .spread_radius(Px(8.0))
            .inset(true)
            .build(),
        ]
        .into_boxed_slice(),
      ))),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_box_shadow_inset");
}

#[test]
fn test_style_position() {
  let container = Node::container([Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(100.0)))
      .with(StyleDeclaration::height(Px(100.0)))
      .with(StyleDeclaration::position(Position::Absolute))
      .with_inset(Sides([Px(20.0); 4]))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 255]),
      ))),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_position");
}

#[test]
fn test_style_border_radius_circle() {
  let container = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(300.0)))
      .with(StyleDeclaration::height(Px(300.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 255]),
      )))
      .with_border_radius(Box::new(BorderRadius(Sides(
        [SpacePair::from_single(Percentage(50.0)); 4],
      )))),
  );

  run_fixture_test(container, "style_border_radius_circle");
}

// https://github.com/kane50613/takumi/issues/151
#[test]
fn test_style_border_radius_width_offset() {
  let container =
    Node::container([
      Node::container([Node::text("The newest blog post".to_string()).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::width(Percentage(100.0)))
          .with_padding(Sides([Rem(4.0); 4]))
          .with(StyleDeclaration::font_size(Rem(4.0).into()))
          .with(StyleDeclaration::font_weight(FontWeight::from(500.0)))
          .with(StyleDeclaration::line_height(LineHeight::Length(Rem(
            4.0 * 1.5,
          )))),
      )])
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::width(Percentage(100.0)))
          .with(StyleDeclaration::height(Percentage(100.0)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color::white(),
          )))
          .with_border_width(Sides([Px(1.0); 4]))
          .with(StyleDeclaration::border_style(BorderStyle::Solid))
          .with_border_radius(Box::new(BorderRadius(Sides(
            [SpacePair::from_single(Px(24.0)); 4],
          ))))
          .with(StyleDeclaration::border_color(ColorInput::Value(Color([
            0, 0, 0, 255,
          ])))),
      ),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([128, 128, 128, 255]),
        )))
        .with_padding(Sides([Rem(2.0); 4])),
    );

  run_fixture_test(container, "style_border_radius_width_offset");
}

#[test]
fn test_style_border_radius_circle_avatar() {
  let container = Node::container([Node::container([Node::image("assets/images/yeecord.png")
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with_border_radius(Box::new(BorderRadius(Sides(
          [SpacePair::from_single(Percentage(50.0)); 4],
        )))),
    )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Rem(12.0)))
      .with(StyleDeclaration::height(Rem(12.0)))
      .with_border_radius(Box::new(BorderRadius(Sides(
        [SpacePair::from_single(Percentage(50.0)); 4],
      ))))
      .with(StyleDeclaration::border_color(ColorInput::Value(Color([
        128, 128, 128, 128,
      ]))))
      .with_border_width(Sides([Px(4.0); 4]))
      .with(StyleDeclaration::border_style(BorderStyle::Solid)),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center)),
  );

  run_fixture_test(container, "style_border_radius_circle_avatar");
}

#[test]
fn test_style_border_width_on_image_node() {
  let avatar = Node::image("assets/images/yeecord.png").with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with_border_radius(Box::new(BorderRadius(Sides(
        [SpacePair::from_single(Percentage(100.0)); 4],
      ))))
      .with_border_width(Sides([Px(2.0); 4]))
      .with(StyleDeclaration::border_style(BorderStyle::Solid))
      .with(StyleDeclaration::border_color(ColorInput::Value(Color([
        202, 202, 202, 255,
      ]))))
      .with(StyleDeclaration::width(Px(128.0)))
      .with(StyleDeclaration::height(Px(128.0))),
  );

  let container = Node::container([avatar]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center)),
  );

  run_fixture_test(container, "style_border_width_on_image_node");
}

#[test]
fn test_style_outline() {
  let outlined_box = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(240.0)))
      .with(StyleDeclaration::height(Px(140.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([14, 165, 233, 255]),
      )))
      .with_border_radius(Box::new(BorderRadius(Sides(
        [SpacePair::from_single(Px(16.0)); 4],
      ))))
      .with(StyleDeclaration::outline_width(Px(10.0)))
      .with(StyleDeclaration::outline_color(ColorInput::Value(Color([
        17, 24, 39, 255,
      ]))))
      .with(StyleDeclaration::outline_offset(Px(8.0)))
      .with(StyleDeclaration::outline_style(BorderStyle::Solid)),
  );

  let container = Node::container([outlined_box]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center)),
  );

  run_fixture_test(container, "style_outline");
}
