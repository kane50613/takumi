use takumi::{
  prelude::{Length::*, *},
  render_animation, write_animated_webp,
};

use crate::test_utils::{CONTEXT, run_animation_fixture_test};

const FPS: u32 = 10;
const DURATION_MS: u32 = 900;
const SOURCE_FRAME_MS: u32 = 100;
const SOURCE_SIZE: u32 = 120;
const MARKER_SIZE: f32 = 40.0;
const BACKDROP: [u8; 4] = [235, 235, 235, 255];
const MARKER: [u8; 4] = [60, 60, 60, 255];

fn marker_frame(column: usize) -> AnimationFrame {
  let marker = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::width(Px(MARKER_SIZE)))
      .with(StyleDeclaration::height(Px(MARKER_SIZE)))
      .with(StyleDeclaration::margin_left(Px(
        column as f32 * MARKER_SIZE,
      )))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color(MARKER),
      ))),
  );

  let node = Node::container([marker]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color(BACKDROP),
      ))),
  );

  let scene = SequentialScene::builder()
    .options(
      RenderOptions::builder()
        .viewport(Viewport::new((SOURCE_SIZE, SOURCE_SIZE)))
        .node(node)
        .fonts(&CONTEXT)
        .build(),
    )
    .duration_ms(SOURCE_FRAME_MS)
    .build();

  let mut frames = render_animation(&[scene], FPS).expect("source frame");
  frames.pop().expect("one frame")
}

/// Three frames encoded as an animated WebP, so the fixture needs no binary
/// asset of its own.
fn animated_webp_bytes() -> Vec<u8> {
  let frames: Vec<AnimationFrame> = (0..3).map(marker_frame).collect();

  let mut bytes = Vec::new();
  write_animated_webp(
    frames.into(),
    &mut bytes,
    AnimatedWebpOptions::builder()
      .lossless(true)
      .blend(false)
      .build(),
  )
  .expect("animated webp");

  bytes
}

fn frames() -> Vec<AnimationFrame> {
  let image = ImageData {
    src: ImageSourceInput::Buffer(animated_webp_bytes()),
    width: None,
    height: None,
  };
  let node = Node::container([Node::image(image).with_style(
    Style::default()
      .with(StyleDeclaration::width(Px(SOURCE_SIZE as f32)))
      .with(StyleDeclaration::height(Px(SOURCE_SIZE as f32))),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color(BACKDROP),
      ))),
  );

  let scene = SequentialScene::builder()
    .options(
      RenderOptions::builder()
        .viewport(Viewport::new((200, 200)))
        .node(node)
        .fonts(&CONTEXT)
        .build(),
    )
    .duration_ms(DURATION_MS)
    .build();

  render_animation(&[scene], FPS).expect("animated frames")
}

fn marker_column(frame: &AnimationFrame) -> Option<usize> {
  let width = frame.image.width();
  let left = (width - SOURCE_SIZE) / 2;
  let row = frame.image.height() / 2;

  (0..3).find(|column| {
    let x = left + *column as u32 * MARKER_SIZE as u32 + MARKER_SIZE as u32 / 2;
    let offset = (row * width + x) as usize * 4;
    frame.image.as_raw()[offset..offset + 3] == MARKER[..3]
  })
}

#[test]
fn animated_webp_image_source() {
  let frames = frames();

  assert_eq!(marker_column(&frames[0]), Some(0));
  assert_eq!(marker_column(&frames[1]), Some(1));
  assert_eq!(marker_column(&frames[2]), Some(2));
  assert_eq!(marker_column(&frames[3]), Some(0));

  run_animation_fixture_test(frames, "animated_webp_image_source", DURATION_MS, FPS);
}
