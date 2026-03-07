use serde_json::{from_value, json};
use takumi::layout::{
  node::{ContainerNode, ImageNode, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_background_color() {
  let container = ContainerNode {
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
          Color([255, 0, 0, 255]),
        ))),
    ),
    children: None,
  };

  run_fixture_test(container.into(), "style_background_color");
}

#[test]
fn test_style_border_radius() {
  let container = ContainerNode {
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
          Color([255, 0, 0, 255]),
        )))
        .with_border_radius(Box::new(BorderRadius(Sides(
          [SpacePair::from_single(Px(20.0)); 4],
        )))),
    ),
    children: None,
  };

  run_fixture_test(container.into(), "style_border_radius");
}

#[test]
fn test_style_border_radius_per_corner() {
  let container = ContainerNode {
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
    ),
    children: None,
  };

  run_fixture_test(container.into(), "style_border_radius_per_corner");
}

#[test]
fn test_style_border_width() {
  let container = ContainerNode {
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
        .with_border_width(Sides([Px(10.0); 4]))
        .with(StyleDeclaration::border_style(BorderStyle::Solid))
        .with(StyleDeclaration::border_color(ColorInput::Value(Color([
          255, 0, 0, 255,
        ])))),
    ),
    children: None,
  };

  run_fixture_test(container.into(), "style_border_width");
}

#[test]
fn test_style_border_width_with_radius() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with_padding(Sides([Rem(4.0); 4]))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::white(),
        ))),
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
        ),
        children: None,
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_border_width_with_radius");
}

#[test]
fn test_style_box_shadow() {
  let container = ContainerNode {
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
          Color([0, 0, 255, 255]),
        ))),
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
            .with(StyleDeclaration::width(Px(100.0)))
            .with(StyleDeclaration::height(Px(100.0)))
            .with(StyleDeclaration::background_color(ColorInput::Value(
              Color([255, 0, 0, 255]),
            )))
            .with(StyleDeclaration::box_shadow(Some(
              [BoxShadow {
                color: ColorInput::Value(Color([0, 0, 0, 128])),
                offset_x: Px(5.0),
                offset_y: Px(5.0),
                blur_radius: Px(10.0),
                spread_radius: Px(0.0),
                inset: false,
              }]
              .into(),
            ))),
        ),
        children: None,
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_box_shadow");
}

#[test]
fn test_style_box_shadow_inset() {
  let container = ContainerNode {
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
          Color([0, 0, 255, 255]),
        ))),
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
            .with(StyleDeclaration::width(Px(120.0)))
            .with(StyleDeclaration::height(Px(80.0)))
            .with(StyleDeclaration::background_color(ColorInput::Value(
              Color::white(),
            )))
            .with_border_radius(Box::new(BorderRadius(Sides(
              [SpacePair::from_single(Px(16.0)); 4],
            ))))
            .with(StyleDeclaration::box_shadow(Some(
              [BoxShadow {
                color: ColorInput::Value(Color([0, 0, 0, 153])),
                offset_x: Px(4.0),
                offset_y: Px(6.0),
                blur_radius: Px(18.0),
                spread_radius: Px(8.0),
                inset: true,
              }]
              .into(),
            ))),
        ),
        children: None,
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_box_shadow_inset");
}

#[test]
fn test_style_position() {
  let container = ContainerNode {
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
          Color([0, 0, 255, 255]),
        ))),
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
            .with(StyleDeclaration::width(Px(100.0)))
            .with(StyleDeclaration::height(Px(100.0)))
            .with(StyleDeclaration::position(Position::Absolute))
            .with_inset(Sides([Px(20.0); 4]))
            .with(StyleDeclaration::background_color(ColorInput::Value(
              Color([255, 0, 0, 255]),
            ))),
        ),
        children: None,
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_position");
}

#[test]
fn test_style_border_radius_circle() {
  let container = ContainerNode {
    class_name: None,
    id: None,
    tag_name: None,
    preset: None,
    tw: None,
    style: Some(
      Style::default()
        .with(StyleDeclaration::width(Px(300.0)))
        .with(StyleDeclaration::height(Px(300.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 0, 0, 255]),
        )))
        .with_border_radius(Box::new(BorderRadius(Sides(
          [SpacePair::from_single(Percentage(50.0)); 4],
        )))),
    ),
    children: None,
  };

  run_fixture_test(container.into(), "style_border_radius_circle");
}

// https://github.com/kane50613/takumi/issues/151
#[test]
fn test_style_border_radius_width_offset() {
  let container = ContainerNode {
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
          Color([128, 128, 128, 255]),
        )))
        .with_padding(Sides([Rem(2.0); 4])),
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
        children: Some(
          [TextNode {
            class_name: None,
            id: None,
            tag_name: None,
            preset: None,
            tw: None,
            text: "The newest blog post".to_string(),
            style: Some(
              Style::default()
                .with(StyleDeclaration::width(Percentage(100.0)))
                .with_padding(Sides([Rem(4.0); 4]))
                .with(StyleDeclaration::font_size(Rem(4.0).into()))
                .with(StyleDeclaration::font_weight(FontWeight::from(500.0)))
                .with(StyleDeclaration::line_height(LineHeight::Length(Rem(
                  4.0 * 1.5,
                )))),
            ),
          }
          .into()]
          .into(),
        ),
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_border_radius_width_offset");
}

#[test]
fn test_style_border_radius_circle_avatar() {
  let container = ContainerNode {
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
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center)),
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
                .with(StyleDeclaration::width(Percentage(100.0)))
                .with(StyleDeclaration::height(Percentage(100.0)))
                .with_border_radius(Box::new(BorderRadius(Sides(
                  [SpacePair::from_single(Percentage(50.0)); 4],
                )))),
            ),
            src: "assets/images/yeecord.png".into(),
            width: None,
            height: None,
          }
          .into()]
          .into(),
        ),
      }
      .into()]
      .into(),
    ),
  };

  run_fixture_test(container.into(), "style_border_radius_circle_avatar");
}

#[test]
fn test_style_border_width_on_image_node() {
  let avatar = json!({
    "type": "image",
    "src": "assets/images/yeecord.png",
    "style": {
      "borderRadius": "100%",
      "borderWidth": 2,
      "borderStyle": "solid",
      "borderColor": "#cacaca",
      "width": 128,
      "height": 128
    }
  });

  let container = ContainerNode {
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
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center)),
    ),
    children: Some([from_value(avatar).unwrap()].into()),
  };

  run_fixture_test(container.into(), "style_border_width_on_image_node");
}

#[test]
fn test_style_outline() {
  let outlined_box = json!({
    "type": "container",
    "style": {
      "width": 240,
      "height": 140,
      "backgroundColor": "#0ea5e9",
      "borderRadius": 16,
      "outlineWidth": 10,
      "outlineColor": "#111827",
      "outlineOffset": 8,
      "outlineStyle": "solid"
    }
  });

  let container = ContainerNode {
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
        .with(StyleDeclaration::justify_content(JustifyContent::Center))
        .with(StyleDeclaration::align_items(AlignItems::Center)),
    ),
    children: Some([from_value(outlined_box).unwrap()].into()),
  };

  run_fixture_test(container.into(), "style_outline");
}
