use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind},
  style::{LengthUnit::*, *},
};

mod test_utils;
use test_utils::run_style_width_test;

fn create_container_with_mask(
  mask_image: BackgroundImages,
  background_color: Color,
) -> ContainerNode<NodeKind> {
  ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(background_color))
        .mask_image(Some(mask_image))
        .build()
        .unwrap(),
    ),
    children: None,
  }
}

#[test]
fn test_style_mask_image_linear_gradient() {
  let mask_image =
    BackgroundImages::from_str("linear-gradient(to right, black, transparent)").unwrap();

  let container = create_container_with_mask(mask_image, Color([255, 0, 0, 255]));

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_mask_image_linear_gradient.webp",
  );
}

#[test]
fn test_style_mask_image_radial_gradient() {
  let mask_image =
    BackgroundImages::from_str("radial-gradient(circle, black, transparent)").unwrap();

  let container = create_container_with_mask(mask_image, Color([0, 128, 255, 255]));

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_mask_image_radial_gradient.webp",
  );
}

#[test]
fn test_style_mask_image_radial_gradient_ellipse() {
  let mask_image = BackgroundImages::from_str(
    "radial-gradient(ellipse at center, black 0%, black 50%, transparent 100%)",
  )
  .unwrap();

  let container = create_container_with_mask(mask_image, Color([34, 197, 94, 255]));

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_mask_image_radial_ellipse.webp",
  );
}

#[test]
fn test_style_mask_image_multiple_gradients() {
  let mask_image = BackgroundImages::from_str(
    "linear-gradient(to right, black, transparent), radial-gradient(circle at 25% 25%, black, transparent 50%)",
  )
  .unwrap();

  let container = create_container_with_mask(mask_image, Color([255, 165, 0, 255]));

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_mask_image_multiple_gradients.webp",
  );
}

#[test]
fn test_style_mask_image_diagonal_gradient() {
  let mask_image =
    BackgroundImages::from_str("linear-gradient(45deg, black 0%, black 50%, transparent 100%)")
      .unwrap();

  let container = create_container_with_mask(mask_image, Color([138, 43, 226, 255]));

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_mask_image_diagonal_gradient.webp",
  );
}

#[test]
fn test_style_mask_image_with_background_image() {
  let mask_image =
    BackgroundImages::from_str("radial-gradient(circle at center, black 40%, transparent 70%)")
      .unwrap();
  let background_image =
    BackgroundImages::from_str("linear-gradient(135deg, #667eea 0%, #764ba2 100%)").unwrap();

  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_image(Some(background_image))
        .mask_image(Some(mask_image))
        .build()
        .unwrap(),
    ),
    children: None,
  };

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_mask_image_with_background.webp",
  );
}

#[test]
fn test_style_mask_image_on_image_node() {
  let mask_image =
    BackgroundImages::from_str("radial-gradient(circle, black 60%, transparent 100%)").unwrap();

  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .background_color(ColorInput::Value(Color([240, 240, 240, 255])))
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
            .width(Rem(16.0))
            .height(Rem(16.0))
            .mask_image(Some(mask_image))
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
    "tests/fixtures/style_mask_image_on_image.webp",
  );
}

#[test]
fn test_style_mask_image_stripes_pattern() {
  let mask_image = BackgroundImages::from_str(
    "linear-gradient(90deg, black 0%, black 25%, transparent 25%, transparent 50%, black 50%, black 75%, transparent 75%, transparent 100%)",
  )
  .unwrap();

  let container = create_container_with_mask(mask_image, Color([255, 20, 147, 255]));

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_mask_image_stripes.webp",
  );
}

#[test]
fn test_style_mask_image_corner_fade() {
  let mask_image = BackgroundImages::from_str(
    "radial-gradient(ellipse at top left, transparent 0%, black 50%), radial-gradient(ellipse at bottom right, transparent 0%, black 50%)",
  )
  .unwrap();

  let container = create_container_with_mask(mask_image, Color([0, 200, 200, 255]));

  run_style_width_test(
    container.into(),
    "tests/fixtures/style_mask_image_corner_fade.webp",
  );
}
