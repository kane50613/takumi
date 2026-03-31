use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use takumi::{
  GlobalContext,
  layout::{
    Viewport,
    node::Node,
    style::{
      AlignItems, BackgroundClip, BackgroundImages, BackgroundPositions, BackgroundRepeats,
      BackgroundSizes, BorderRadius, BorderStyle, Color, ColorInput, Display, FlexDirection,
      FontWeight, FromCss, JustifyContent,
      Length::{Percentage, Px, Rem},
      ObjectFit, Overflow, Sides, SpacePair, Style, StyleDeclaration,
    },
  },
  rendering::{RenderOptions, render},
};

const BENCH_WIDTH: u32 = 1200;
const BENCH_HEIGHT: u32 = 630;
const IMAGE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/images/yeecord.png");

fn render_fixture(global: &GlobalContext, node: Node) {
  let options = RenderOptions::builder()
    .viewport(Viewport::new((BENCH_WIDTH, BENCH_HEIGHT)))
    .node(node)
    .global(global)
    .build();

  let _image = render(options).unwrap();
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

fn rounded_cover_image_fixture() -> Node {
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
      .with(StyleDeclaration::width(Px(980.0)))
      .with(StyleDeclaration::height(Px(540.0)))
      .with_border_radius(BorderRadius::from_str("64px").unwrap())
      .with_overflow(SpacePair::from_single(Overflow::Clip))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([245, 247, 250, 255]),
      ))),
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

fn overflow_text_fixture() -> Node {
  Node::container([Node::container([
    Node::text(
      "This is a very long text block that should overflow its box and exercise mask and clip handling in the text compositor."
        .to_string(),
    )
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Rem(3.5).into()))
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          16, 18, 24, 255,
        ])))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(540.0)))
      .with(StyleDeclaration::height(Px(220.0)))
      .with_border_width(Sides([Px(4.0); 4]))
      .with(StyleDeclaration::border_style(BorderStyle::Solid))
      .with(StyleDeclaration::border_color(Color([0, 0, 0, 255]).into()))
      .with_overflow(SpacePair::from_single(Overflow::Hidden)),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
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
  let global = GlobalContext::default();
  let mut group = c.benchmark_group("fixtures");

  group.bench_function("simple_image_blit", |b| {
    b.iter(|| render_fixture(&global, black_box(simple_image_blit_fixture())))
  });
  group.bench_function("rounded_cover_image", |b| {
    b.iter(|| render_fixture(&global, black_box(rounded_cover_image_fixture())))
  });
  group.bench_function("gradient_clip_text", |b| {
    b.iter(|| render_fixture(&global, black_box(gradient_clip_text_fixture())))
  });
  group.bench_function("overflow_text", |b| {
    b.iter(|| render_fixture(&global, black_box(overflow_text_fixture())))
  });
  group.bench_function("emoji_social", |b| {
    b.iter(|| render_fixture(&global, black_box(emoji_social_fixture())))
  });

  group.finish();
}

criterion_group!(benches, bench_fixtures);
criterion_main!(benches);
