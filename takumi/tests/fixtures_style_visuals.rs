use serde_json::{from_value, json};
use smallvec::smallvec;
use takumi::layout::{
  node::{ContainerNode, ImageNode, TextNode},
  style::{LengthUnit::*, *},
};

mod test_utils;
use test_utils::run_style_width_test;

#[test]
fn test_style_background_color() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_background_color.webp",
  );
}

#[test]
fn test_style_border_radius() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
        .border_radius(BorderRadius(Sides([SpacePair::from_single(Px(20.0)); 4])))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  run_style_width_test(container.into(), "tests/fixtures/style_border_radius.webp");
}

#[test]
fn test_style_border_radius_per_corner() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
        // Per-corner radii
        .border_top_left_radius(Some(SpacePair::from_single(Px(40.0))))
        .border_top_right_radius(Some(SpacePair::from_single(Px(10.0))))
        .border_bottom_right_radius(Some(SpacePair::from_single(Px(80.0))))
        .border_bottom_left_radius(Some(SpacePair::from_single(Px(0.0))))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_border_radius_per_corner.webp",
  );
}

#[test]
fn test_style_border_width() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .border_width(Some(Sides([Px(10.0); 4])))
        .border_color(Some(ColorInput::Value(Color([255, 0, 0, 255]))))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  run_style_width_test(container.into(), "tests/fixtures/style_border_width.webp");
}

#[test]
fn test_style_border_width_with_radius() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .padding(Sides([Rem(4.0); 4]))
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Rem(16.0))
            .height(Rem(8.0))
            .border_radius(BorderRadius(Sides([SpacePair::from_single(Px(10.0)); 4])))
            .border_color(Some(ColorInput::Value(Color([255, 0, 0, 255]))))
            .border_width(Some(Sides([Px(4.0); 4])))
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_border_width_with_radius.webp",
  );
}

#[test]
fn test_style_box_shadow() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([0, 0, 255, 255])))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Px(100.0))
            .height(Px(100.0))
            .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
            .box_shadow(Some(smallvec![BoxShadow {
              color: ColorInput::Value(Color([0, 0, 0, 128])),
              offset_x: Px(5.0),
              offset_y: Px(5.0),
              blur_radius: Px(10.0),
              spread_radius: Px(0.0),
              inset: false,
            }]))
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/style_box_shadow.webp");
}

#[test]
fn test_style_box_shadow_inset() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([0, 0, 255, 255]))) // Blue background container for contrast
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Px(120.0))
            .height(Px(80.0))
            .background_color(ColorInput::Value(Color::white())) // White child for inset visibility
            .border_radius(BorderRadius(Sides([SpacePair::from_single(Px(16.0)); 4])))
            .box_shadow(Some(smallvec![BoxShadow {
              color: ColorInput::Value(Color([0, 0, 0, 153])),
              offset_x: Px(4.0),
              offset_y: Px(6.0),
              blur_radius: Px(18.0),
              spread_radius: Px(8.0),
              inset: true,
            }]))
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_box_shadow_inset.webp",
  );
}

#[test]
fn test_style_position() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([0, 0, 255, 255])))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Px(100.0))
            .height(Px(100.0))
            .position(Position::Absolute) // Test the position property
            .inset(Sides([Px(20.0); 4])) // Position with inset properties
            .background_color(ColorInput::Value(Color([255, 0, 0, 255]))) // Red child to make it visible
            .build()
            .unwrap(),
        ),
        children: None,
      }
      .into(),
    ]),
  };

  run_style_width_test(container.into(), "tests/fixtures/style_position.webp");
}

#[test]
fn test_style_border_radius_circle() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(300.0))
        .height(Px(300.0))
        .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
        .border_radius(BorderRadius(Sides(
          [SpacePair::from_single(Percentage(50.0)); 4],
        )))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_border_radius_circle.webp",
  );
}

// https://github.com/kane50613/takumi/issues/151
#[test]
fn test_style_border_radius_width_offset() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([128, 128, 128, 255])))
        .padding(Sides([Rem(2.0); 4]))
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Percentage(100.0))
            .height(Percentage(100.0))
            .background_color(ColorInput::Value(Color::white()))
            .border_width(Some(Sides([Px(1.0); 4])))
            .border_radius(BorderRadius(Sides([SpacePair::from_single(Px(24.0)); 4])))
            .border_color(Some(ColorInput::Value(Color([0, 0, 0, 255]))))
            .build()
            .unwrap(),
        ),
        children: Some(vec![
          TextNode {
            tw: None,
            text: "The newest blog post".to_string(),
            style: Some(
              StyleBuilder::default()
                .width(Percentage(100.0))
                .padding(Sides([Rem(4.0); 4]))
                .font_size(Some(Rem(4.0)))
                .font_weight(FontWeight::from(500.0))
                .line_height(LineHeight(Rem(4.0 * 1.5)))
                .build()
                .unwrap(),
            ),
          }
          .into(),
        ]),
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_border_radius_width_offset.webp",
  );
}

#[test]
fn test_style_border_radius_circle_avatar() {
  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .build()
        .unwrap(),
    ),
    children: Some(vec![
      ContainerNode {
        tw: None,
        style: Some(
          StyleBuilder::default()
            .width(Rem(12.0))
            .height(Rem(12.0))
            .border_radius(BorderRadius(Sides(
              [SpacePair::from_single(Percentage(50.0)); 4],
            )))
            .border_color(Some(ColorInput::Value(Color([128, 128, 128, 128])))) // gray
            .border_width(Some(Sides([Px(4.0); 4])))
            .build()
            .unwrap(),
        ),
        children: Some(vec![
          ImageNode {
            tw: None,
            style: Some(
              StyleBuilder::default()
                .width(Percentage(100.0))
                .height(Percentage(100.0))
                .border_radius(BorderRadius(Sides(
                  [SpacePair::from_single(Percentage(50.0)); 4],
                )))
                .build()
                .unwrap(),
            ),
            src: "assets/images/yeecord.png".into(),
            width: None,
            height: None,
          }
          .into(),
        ]),
      }
      .into(),
    ]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_border_radius_circle_avatar.webp",
  );
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
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color::white()))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .build()
        .unwrap(),
    ),
    children: Some(vec![from_value(avatar).unwrap()]),
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_border_width_on_image_node.webp",
  );
}
