use takumi::prelude::{Length::*, *};

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
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(20.0)); 4]))),
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

fn corner_shape_box(shape: Superellipse, border_colors: [Color; 4]) -> Node {
  Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::width(Px(180.0)))
      .with(StyleDeclaration::height(Px(180.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 196, 0, 255]),
      )))
      .with_border_width(Sides([LineWidth::Length(Px(10.0)); 4]))
      .with_border_style(Sides([BorderStyle::Solid; 4]))
      .with_border_color(Sides(border_colors.map(ColorInput::Value)))
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(60.0)); 4])))
      .with_corner_shape(Sides([shape; 4])),
  )
}

#[test]
fn test_style_corner_shape() {
  let shapes = [
    Superellipse::ROUND,
    Superellipse::SQUIRCLE,
    Superellipse::BEVEL,
    Superellipse::SCOOP,
    Superellipse::NOTCH,
    Superellipse::SQUARE,
  ];

  let uniform_row = shapes.map(|shape| corner_shape_box(shape, [Color([15, 23, 42, 255]); 4]));
  let per_side_row = shapes.map(|shape| {
    corner_shape_box(
      shape,
      [
        Color([220, 38, 38, 255]),
        Color([22, 163, 74, 255]),
        Color([37, 99, 235, 255]),
        Color([217, 70, 239, 255]),
      ],
    )
  });

  let row_style = || {
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::justify_content(
        JustifyContent::SpaceEvenly,
      ))
  };

  let container = Node::container([
    Node::container(uniform_row).with_style(row_style()),
    Node::container(per_side_row).with_style(row_style()),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([30, 41, 59, 255]),
      )))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(
        JustifyContent::SpaceEvenly,
      )),
  );

  run_fixture_test(container, "style_corner_shape");
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
      .with_border_width(Sides([Px(10.0).into(); 4]))
      .with_border_style(Sides([BorderStyle::Solid; 4]))
      .with_border_color(Sides([ColorInput::Value(Color([255, 0, 0, 255])); 4])),
  );

  run_fixture_test(container, "style_border_width");
}

/// A fractional width paints evenly through coverage AA, whichever half of the
/// pixel grid each edge lands on — the chips step by 0.25px to cover the
/// landings.
#[test]
fn test_style_border_fractional_width() {
  let chips: Vec<Node> = (0..4)
    .map(|index| {
      Node::container([]).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::width(Px(180.0)))
          .with(StyleDeclaration::height(Px(41.0 + index as f32 * 0.25)))
          .with(StyleDeclaration::background_color(ColorInput::Value(
            Color::white(),
          )))
          .with_border_width(Sides([Px(2.5).into(); 4]))
          .with_border_style(Sides([BorderStyle::Solid; 4]))
          .with_border_color(Sides([ColorInput::Value(Color([51, 41, 27, 255])); 4]))
          .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(10.0)); 4]))),
      )
    })
    .collect();
  let container = Node::container(chips).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([239, 230, 205, 255]),
      )))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with_gap(SpacePair::from_single(Px(24.0).into())),
  );

  run_fixture_test(container, "style_border_fractional_width");
}

#[test]
fn test_style_border_per_side_color_and_width() {
  let container = Node::container([Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(220.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with_border_width(Sides([
        Px(12.0).into(),
        Px(24.0).into(),
        Px(36.0).into(),
        Px(48.0).into(),
      ]))
      .with_border_style(Sides([
        BorderStyle::Solid,
        BorderStyle::Solid,
        BorderStyle::Solid,
        BorderStyle::Solid,
      ]))
      .with_border_color(Sides([
        ColorInput::Value(Color([239, 68, 68, 255])),
        ColorInput::Value(Color([34, 197, 94, 255])),
        ColorInput::Value(Color([59, 130, 246, 255])),
        ColorInput::Value(Color([234, 179, 8, 255])),
      ])),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([17, 24, 39, 255]),
      )))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center)),
  );

  run_fixture_test(container, "style_border_per_side_color_and_width");
}

#[test]
fn test_style_border_width_with_radius() {
  let container = Node::container([Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Rem(16.0)))
      .with(StyleDeclaration::height(Rem(8.0)))
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(10.0)); 4])))
      .with_border_color(Sides([ColorInput::Value(Color([255, 0, 0, 255])); 4]))
      .with_border_width(Sides([Px(4.0).into(); 4]))
      .with_border_style(Sides([BorderStyle::Solid; 4])),
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
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(16.0)); 4])))
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

/// A spread wider than the border has to clear the border and still show, which
/// only holds while the shadow is placed against the padding box.
#[test]
fn test_style_box_shadow_inset_with_border() {
  let container = Node::container([Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(240.0)))
      .with(StyleDeclaration::height(Px(160.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(16.0)); 4])))
      .with_border_width(Sides([Px(18.0).into(); 4]))
      .with_border_style(Sides([BorderStyle::Solid; 4]))
      .with_border_color(Sides([ColorInput::Value(Color([120, 140, 200, 255])); 4]))
      .with(StyleDeclaration::box_shadow(Some(
        vec![
          BoxShadow::builder()
            .color(ColorInput::Value(Color([0, 0, 0, 153])))
            .offset_x(Px(0.0))
            .offset_y(Px(0.0))
            .blur_radius(Px(0.0))
            .spread_radius(Px(30.0))
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
        Color([230, 230, 230, 255]),
      ))),
  );

  run_fixture_test(container, "style_box_shadow_inset_with_border");
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
      .with_border_radius(BorderRadius(Sides(
        [SpacePair::from_single(Percentage(50.0)); 4],
      ))),
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
          .with_border_width(Sides([Px(1.0).into(); 4]))
          .with_border_style(Sides([BorderStyle::Solid; 4]))
          .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(24.0)); 4])))
          .with_border_color(Sides([ColorInput::Value(Color([0, 0, 0, 255])); 4])),
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
        .with_border_radius(BorderRadius(Sides(
          [SpacePair::from_single(Percentage(50.0)); 4],
        ))),
    )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Rem(12.0)))
      .with(StyleDeclaration::height(Rem(12.0)))
      .with_border_radius(BorderRadius(Sides(
        [SpacePair::from_single(Percentage(50.0)); 4],
      )))
      .with_border_color(Sides([ColorInput::Value(Color([128, 128, 128, 128])); 4]))
      .with_border_width(Sides([Px(4.0).into(); 4]))
      .with_border_style(Sides([BorderStyle::Solid; 4])),
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
      .with_border_radius(BorderRadius(Sides(
        [SpacePair::from_single(Percentage(100.0)); 4],
      )))
      .with_border_width(Sides([Px(2.0).into(); 4]))
      .with_border_style(Sides([BorderStyle::Solid; 4]))
      .with_border_color(Sides([ColorInput::Value(Color([202, 202, 202, 255])); 4]))
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
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(16.0)); 4])))
      .with(StyleDeclaration::outline_width(Px(10.0).into()))
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

#[test]
fn test_style_outline_with_text() {
  // Regression for #475: outline is not inherited, so a non-inline element
  // strokes only its own border-box, never its text content.
  let badge = Node::container([Node::text("Inner Outline".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::font_size(Px(28.0).into()))
      .with(StyleDeclaration::color(ColorInput::Value(Color::white()))),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::InlineBlock))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([139, 92, 246, 255]),
      )))
      .with_padding(Sides([Px(12.0), Px(32.0), Px(12.0), Px(32.0)]))
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(9999.0)); 4])))
      .with(StyleDeclaration::outline_width(Px(6.0).into()))
      .with(StyleDeclaration::outline_color(ColorInput::Value(Color([
        196, 181, 253, 255,
      ]))))
      .with(StyleDeclaration::outline_offset(Px(-12.0)))
      .with(StyleDeclaration::outline_style(BorderStyle::Solid)),
  );

  let container = Node::container([badge]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([15, 23, 42, 255]),
      )))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center)),
  );

  run_fixture_test(container, "style_outline_with_text");
}

#[test]
fn test_style_outline_over_child_box() {
  // CSS 2.1 Appendix E, and Blink's `kDescendantOutlinesOnly` phase, paint a
  // box's outline after everything inside it. A negative `outline-offset` drags
  // the ring across the child, so a backend that paints the outline with the
  // box's own decorations buries it under the child.
  let child = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::width(Px(320.0)))
      .with(StyleDeclaration::height(Px(200.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([250, 204, 21, 255]),
      ))),
  );

  let card = Node::container([child]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(240.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([226, 232, 240, 255]),
      )))
      .with(StyleDeclaration::outline_width(Px(12.0).into()))
      .with(StyleDeclaration::outline_color(ColorInput::Value(Color([
        220, 38, 38, 255,
      ]))))
      .with(StyleDeclaration::outline_offset(Px(-60.0)))
      .with(StyleDeclaration::outline_style(BorderStyle::Solid)),
  );

  let container = Node::container([card]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([15, 23, 42, 255]),
      )))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center)),
  );

  run_fixture_test(container, "style_outline_over_child_box");
}

#[test]
fn test_style_outline_over_children_and_text() {
  // CSS 2.1 Appendix E paints the outline after everything the box holds. A
  // negative `outline-offset` drags the ring inward until it crosses the line of
  // text, so a backend that paints the outline early buries it under the glyphs.
  let card = |offset: f32, text: &str| {
    Node::container([Node::text(text.to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::font_size(Px(48.0).into()))
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          15, 23, 42, 255,
        ])))),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::width(Px(420.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([226, 232, 240, 255]),
        )))
        .with_padding(Sides([Px(24.0); 4]))
        .with(StyleDeclaration::outline_width(Px(10.0).into()))
        .with(StyleDeclaration::outline_color(ColorInput::Value(Color([
          220, 38, 38, 255,
        ]))))
        .with(StyleDeclaration::outline_offset(Px(offset)))
        .with(StyleDeclaration::outline_style(BorderStyle::Solid)),
    )
  };

  let container = Node::container([card(-50.0, "Ring over text"), card(-64.0, "Deeper ring")])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([15, 23, 42, 255]),
        )))
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with_gap(SpacePair::from_single(Px(32.0).into())),
    );

  run_fixture_test(container, "style_outline_over_children_and_text");
}

fn label_text(label: &str) -> Node {
  Node::text(label.to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::font_size(Px(16.0).into()))
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        31, 41, 55, 255,
      ])))),
  )
}

fn preview_frame(node: Node) -> Node {
  Node::container([node]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(196.0)))
      .with(StyleDeclaration::height(Px(112.0)))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([243, 244, 246, 255]),
      )))
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(20.0)); 4]))),
  )
}

fn demo_card(label: &str, preview: Node) -> Node {
  Node::container([preview_frame(preview), label_text(label)]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::width(Px(208.0)))
      .with_gap(SpacePair::from_single(Px(12.0).into()))
      .with(StyleDeclaration::align_items(AlignItems::Center)),
  )
}

fn non_uniform_border_demo(label: &str, style: Sides<BorderStyle>, width: Sides<Length>) -> Node {
  let preview = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(164.0)))
      .with(StyleDeclaration::height(Px(92.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([56, 189, 248, 255]),
      )))
      .with_border_style(style)
      .with_border_width(width.map_axis(|w, _| w.into()))
      .with_border_color(Sides([ColorInput::Value(Color([17, 24, 39, 255])); 4])),
  );

  Node::container([
    preview,
    Node::text(label.to_string()).with_style(Style::default().with(StyleDeclaration::color(
      ColorInput::Value(Color([31, 41, 55, 255])),
    ))),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::width(Px(220.0)))
      .with_gap(SpacePair::from_single(Px(12.0).into()))
      .with(StyleDeclaration::align_items(AlignItems::Center)),
  )
}

fn border_style_demo(label: &str, style: BorderStyle) -> Node {
  let preview = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(152.0)))
      .with(StyleDeclaration::height(Px(72.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([14, 165, 233, 255]),
      )))
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(18.0)); 4])))
      .with_border_width(Sides([Px(8.0).into(); 4]))
      .with_border_style(Sides([style; 4]))
      .with_border_color(Sides([ColorInput::Value(Color([17, 24, 39, 255])); 4])),
  );

  demo_card(label, preview)
}

fn outline_style_demo(label: &str, style: BorderStyle) -> Node {
  let preview = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(152.0)))
      .with(StyleDeclaration::height(Px(72.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([251, 191, 36, 255]),
      )))
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(18.0)); 4])))
      .with_border_width(Sides([Px(2.0).into(); 4]))
      .with_border_style(Sides([BorderStyle::Solid; 4]))
      .with_border_color(Sides([ColorInput::Value(Color([255, 255, 255, 255])); 4]))
      .with(StyleDeclaration::outline_width(Px(8.0).into()))
      .with(StyleDeclaration::outline_offset(Px(8.0)))
      .with(StyleDeclaration::outline_style(style))
      .with(StyleDeclaration::outline_color(ColorInput::Value(Color([
        17, 24, 39, 255,
      ])))),
  );

  demo_card(label, preview)
}

#[test]
fn test_style_border_styles() {
  let demos = [
    ("none", BorderStyle::None),
    ("hidden", BorderStyle::Hidden),
    ("dotted", BorderStyle::Dotted),
    ("dashed", BorderStyle::Dashed),
    ("solid", BorderStyle::Solid),
    ("double", BorderStyle::Double),
    ("groove", BorderStyle::Groove),
    ("ridge", BorderStyle::Ridge),
    ("inset", BorderStyle::Inset),
    ("outset", BorderStyle::Outset),
  ]
  .into_iter()
  .map(|(label, style)| border_style_demo(label, style))
  .collect::<Vec<_>>();

  let container = Node::container(demos).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_wrap(FlexWrap::Wrap))
      .with_gap(SpacePair::from_single(Px(16.0).into()))
      .with_padding(Sides([Px(32.0); 4]))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_border_styles");
}

#[test]
fn test_style_border_non_uniform_patterns() {
  let demos = vec![
    non_uniform_border_demo(
      "dashed top only",
      Sides([
        BorderStyle::Dashed,
        BorderStyle::None,
        BorderStyle::None,
        BorderStyle::None,
      ]),
      Sides([Px(10.0), Px(0.0), Px(0.0), Px(0.0)]),
    ),
    non_uniform_border_demo(
      "dotted left only",
      Sides([
        BorderStyle::None,
        BorderStyle::None,
        BorderStyle::None,
        BorderStyle::Dotted,
      ]),
      Sides([Px(0.0), Px(0.0), Px(0.0), Px(10.0)]),
    ),
    non_uniform_border_demo(
      "dashed uneven widths",
      Sides([BorderStyle::Dashed; 4]),
      Sides([Px(10.0), Px(4.0), Px(7.0), Px(13.0)]),
    ),
    non_uniform_border_demo(
      "mixed dash + dot",
      Sides([
        BorderStyle::Dotted,
        BorderStyle::Dashed,
        BorderStyle::Dotted,
        BorderStyle::Dashed,
      ]),
      Sides([Px(10.0), Px(8.0), Px(10.0), Px(8.0)]),
    ),
  ];

  let container = Node::container(demos).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_wrap(FlexWrap::Wrap))
      .with_gap(SpacePair::from_single(Px(20.0).into()))
      .with_padding(Sides([Px(32.0); 4]))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_border_non_uniform_patterns");
}

#[test]
fn test_style_outline_styles() {
  let demos = [
    ("none", BorderStyle::None),
    ("hidden", BorderStyle::Hidden),
    ("dotted", BorderStyle::Dotted),
    ("dashed", BorderStyle::Dashed),
    ("solid", BorderStyle::Solid),
    ("double", BorderStyle::Double),
    ("groove", BorderStyle::Groove),
    ("ridge", BorderStyle::Ridge),
    ("inset", BorderStyle::Inset),
    ("outset", BorderStyle::Outset),
  ]
  .into_iter()
  .map(|(label, style)| outline_style_demo(label, style))
  .collect::<Vec<_>>();

  let container = Node::container(demos).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_wrap(FlexWrap::Wrap))
      .with_gap(SpacePair::from_single(Px(16.0).into()))
      .with_padding(Sides([Px(32.0); 4]))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_outline_styles");
}
