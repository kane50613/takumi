use std::{collections::HashMap, hint::black_box, sync::Arc};

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{
    AlignItems, BackgroundImages, BorderRadius, Color, ColorInput, Display, FlexDirection,
    FontWeight, Fonts, FromCssStr, ImageSource, JustifyContent,
    Length::{Percentage, Px},
    Node, ObjectFit, Overflow, RenderOptions, Sides, SpacePair, Style, StyleDeclaration, Viewport,
  },
  render,
};

const BENCH_WIDTH: u32 = 1200;
const BENCH_HEIGHT: u32 = 630;

fn render_node(fonts: &Fonts, node: Node, images: &HashMap<Arc<str>, ImageSource>) {
  let options = RenderOptions::builder()
    .viewport(Viewport::new((BENCH_WIDTH, BENCH_HEIGHT)))
    .node(node)
    .fonts(fonts)
    .images(images.clone())
    .build();

  let image = render(options).unwrap();
  black_box(image);
}

fn simple_image_blit_fixture() -> Node {
  Node::image(common::IMAGE_SRC).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::Fill)),
  )
}

fn emoji_social_fixture() -> Node {
  Node::container([
    Node::container([Node::image(common::IMAGE_SRC).with_style(
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
        .with_border_radius(BorderRadius::from_css_str("40px").unwrap()),
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
        BackgroundImages::from_css_str(
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
  let fonts = common::fonts();
  let images = common::images();
  let mut group = c.benchmark_group("fixtures");

  group.bench_function("simple_image_blit", |b| {
    b.iter(|| render_node(&fonts, black_box(simple_image_blit_fixture()), &images))
  });
  group.bench_function("gradient_clip_text", |b| {
    b.iter(|| {
      render_node(
        &fonts,
        black_box(common::gradient_clip_text_fixture()),
        &images,
      )
    })
  });
  group.bench_function("emoji_social", |b| {
    b.iter(|| render_node(&fonts, black_box(emoji_social_fixture()), &images))
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
