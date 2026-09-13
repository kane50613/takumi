use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{
    BackgroundImages, BorderRadius, Color, ColorInput, Display, Filters, FlexWrap, Fonts,
    FromCssStr, Length::Px, Node, RenderOptions, Style, StyleDeclaration, Viewport,
  },
  render,
};

fn run_effect_render(fonts: &Fonts, effect_tw: &str) {
  // We set a reasonable size and background so the effect is actually computed
  let node = Node::container([]).with_tw(
    format!("w-[256px] h-[256px] bg-white {effect_tw}")
      .parse()
      .unwrap(),
  );

  let viewport = Viewport::new((512, 512));

  let options = RenderOptions::builder()
    .viewport(viewport)
    .node(node)
    .fonts(fonts)
    .build();

  let image = render(options).unwrap();
  black_box(image);
}

fn backdrop_blur_cards() -> Node {
  let cards = (0..16).map(|_| {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::width(Px(120.0)))
        .with(StyleDeclaration::height(Px(120.0)))
        .with(StyleDeclaration::margin_top(Px(4.0)))
        .with(StyleDeclaration::margin_left(Px(4.0)))
        .with_border_radius(BorderRadius::from_css_str("24px").unwrap())
        .with(StyleDeclaration::backdrop_filter(
          Filters::from_css_str("blur(10px)").unwrap(),
        ))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 255, 255, 80]),
        ))),
    )
  });

  Node::container(cards.collect::<Vec<_>>()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_wrap(FlexWrap::Wrap))
      .with(StyleDeclaration::width(Px(512.0)))
      .with(StyleDeclaration::height(Px(512.0)))
      .with(StyleDeclaration::background_image(Some(
        BackgroundImages::from_css_str("linear-gradient(135deg, #ff0080, #4000ff)").unwrap(),
      ))),
  )
}

fn bench_effects(c: &mut Criterion) {
  let fonts = Fonts::default();

  let mut group = c.benchmark_group("effects");

  group.bench_function("blur_md", |b| {
    b.iter(|| run_effect_render(&fonts, black_box("blur-md")))
  });
  group.bench_function("shadow_md", |b| {
    b.iter(|| run_effect_render(&fonts, black_box("shadow-md")))
  });
  group.bench_function("drop_shadow_md", |b| {
    b.iter(|| run_effect_render(&fonts, black_box("drop-shadow-md")))
  });
  group.bench_function("backdrop_blur_cards", |b| {
    b.iter(|| {
      let options = RenderOptions::builder()
        .viewport(Viewport::new((512, 512)))
        .node(black_box(backdrop_blur_cards()))
        .fonts(&fonts)
        .build();
      black_box(render(options).unwrap());
    })
  });

  group.finish();
}

mod common;

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_effects
}
criterion_main!(benches);
