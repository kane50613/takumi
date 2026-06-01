use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use takumi::{
  FontContext,
  layout::{
    Viewport,
    node::Node,
    style::{
      AlignItems, BackgroundClip, BackgroundImages, BackgroundPositions, BackgroundRepeats,
      BackgroundSizes, BorderRadius, Color, ColorInput, Display, FlexDirection, FontWeight,
      FromCss, JustifyContent,
      Length::{Percentage, Px},
      ObjectFit, Overflow, Sides, SpacePair, Style, StyleDeclaration,
    },
  },
  rendering::{RenderOptions, render},
};

const BENCH_WIDTH: u32 = 1200;
const BENCH_HEIGHT: u32 = 630;
const IMAGE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/images/yeecord.png");

fn render_fixture(font_context: &FontContext, node: Node) {
  let options = RenderOptions::builder()
    .viewport(Viewport::new((BENCH_WIDTH, BENCH_HEIGHT)))
    .node(node)
    .font_context(font_context)
    .build();

  let image = render(options).unwrap();
  black_box(image);
}

fn simple_image_blit_fixture() -> Node {
  Node::image(IMAGE_PATH).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::Fill)),
  )
}

fn gradient_clip_text_fixture() -> Node {
  let gradient = BackgroundImages::from_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  Node::container([
    Node::text("Gradient Text Benchmark".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
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
        .with(StyleDeclaration::background_clip(BackgroundClip::Text))
        .with(StyleDeclaration::color(ColorInput::Value(
          Color::transparent(),
        ))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([242, 242, 242, 255]),
      )))
      .with(StyleDeclaration::font_size(Px(72.0).into()))
      .with(StyleDeclaration::font_weight(FontWeight::from(800.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center)),
  )
}

fn emoji_social_fixture() -> Node {
  Node::container([
    Node::container([Node::image(IMAGE_PATH).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(220.0)))
        .with(StyleDeclaration::height(Px(220.0)))
        .with(StyleDeclaration::object_fit(ObjectFit::Cover)),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with_overflow(SpacePair::from_single(Overflow::Clip))
        .with_border_radius(BorderRadius::from_str("40px").unwrap()),
    ),
    Node::container([
      Node::text("Ship faster with tiny-skia".to_string()).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::font_size(Px(76.0).into()))
          .with(StyleDeclaration::font_weight(FontWeight::from(800.0))),
      ),
      Node::text("Emoji load test 🚀✨🔥🙂‍↔️🎉📈".to_string()).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::font_size(Px(40.0).into()))
          .with(StyleDeclaration::color(ColorInput::Value(Color([
            70, 78, 92, 255,
          ])))),
      ),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column)),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with_padding(Sides([Px(48.0), Px(56.0), Px(48.0), Px(56.0)]))
      .with(StyleDeclaration::background_image(Some(
        BackgroundImages::from_str(
          "linear-gradient(135deg, #f8fafc 0%, #e2e8f0 45%, #cbd5e1 100%)",
        )
        .unwrap(),
      )))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(
        JustifyContent::SpaceBetween,
      )),
  )
}

fn bench_fixtures(c: &mut Criterion) {
  let font_context = FontContext::default();
  let mut group = c.benchmark_group("fixtures");

  group.bench_function("simple_image_blit", |b| {
    b.iter(|| render_fixture(&font_context, black_box(simple_image_blit_fixture())))
  });
  group.bench_function("gradient_clip_text", |b| {
    b.iter(|| render_fixture(&font_context, black_box(gradient_clip_text_fixture())))
  });
  group.bench_function("emoji_social", |b| {
    b.iter(|| render_fixture(&font_context, black_box(emoji_social_fixture())))
  });

  group.finish();
}

mod common;

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_fixtures
}
criterion_main!(benches);
