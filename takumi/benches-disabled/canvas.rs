use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use takumi::{
  Fonts,
  layout::{
    Viewport,
    node::Node,
    style::{
      AlignItems, BackgroundClip, BackgroundImages, BackgroundPositions, BackgroundRepeats,
      BackgroundSizes, BorderRadius, Color, ColorInput, Display, FromCss, JustifyContent,
      Length::{Percentage, Px},
      ObjectFit, Overflow, SpacePair, Style, StyleDeclaration,
    },
  },
  rendering::{RenderOptions, render},
};

const BENCH_WIDTH: u32 = 800;
const BENCH_HEIGHT: u32 = 800;
const IMAGE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/images/yeecord.png");

fn render_node(fonts: &Fonts, node: Node) {
  let options = RenderOptions::builder()
    .viewport(Viewport::new((BENCH_WIDTH, BENCH_HEIGHT)))
    .node(node)
    .fonts(fonts)
    .build();
  let image = render(options).unwrap();
  black_box(image);
}

fn nested_clip_masks_fixture() -> Node {
  let leaf = |radius: &str| {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with_border_radius(BorderRadius::from_str(radius).unwrap())
        .with_overflow(SpacePair::from_single(Overflow::Clip))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([200, 30, 30, 255]),
        ))),
    )
  };

  let mut current = leaf("12px");
  for radius in ["24px", "32px", "40px", "48px", "56px"] {
    current = Node::container([current]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(95.0)))
        .with(StyleDeclaration::height(Percentage(95.0)))
        .with_border_radius(BorderRadius::from_str(radius).unwrap())
        .with_overflow(SpacePair::from_single(Overflow::Clip))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([30, 30, 200, 255]),
        ))),
    );
  }

  Node::container([current]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )
}

fn scaled_image_fixture() -> Node {
  Node::container([Node::image(IMAGE_PATH).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::Cover)),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(720.0)))
      .with(StyleDeclaration::height(Px(720.0)))
      .with_border_radius(BorderRadius::from_str("96px").unwrap())
      .with_overflow(SpacePair::from_single(Overflow::Clip))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([245, 247, 250, 255]),
      ))),
  )
}

fn gradient_clip_mask_fixture() -> Node {
  let gradient = BackgroundImages::from_str(
    "linear-gradient(135deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();
  Node::container([Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(640.0)))
      .with(StyleDeclaration::height(Px(640.0)))
      .with_border_radius(BorderRadius::from_str("64px").unwrap())
      .with_overflow(SpacePair::from_single(Overflow::Clip))
      .with(StyleDeclaration::background_image(Some(gradient)))
      .with(StyleDeclaration::background_size(
        BackgroundSizes::from_str("100% 100%").unwrap(),
      ))
      .with(StyleDeclaration::background_position(
        BackgroundPositions::from_str("0 0").unwrap(),
      ))
      .with(StyleDeclaration::background_repeat(
        BackgroundRepeats::from_str("no-repeat").unwrap(),
      ))
      .with(StyleDeclaration::background_clip(BackgroundClip::BorderBox)),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )
}

fn bench_canvas(c: &mut Criterion) {
  let fonts = Fonts::default();
  let mut group = c.benchmark_group("canvas");

  group.bench_function("nested_clip_masks", |b| {
    b.iter(|| render_node(&fonts, black_box(nested_clip_masks_fixture())))
  });
  group.bench_function("scaled_image_clip", |b| {
    b.iter(|| render_node(&fonts, black_box(scaled_image_fixture())))
  });
  group.bench_function("gradient_clip_mask", |b| {
    b.iter(|| render_node(&fonts, black_box(gradient_clip_mask_fixture())))
  });

  group.finish();
}

mod common;

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_canvas
}
criterion_main!(benches);
