use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{
    BackgroundImages, BorderRadius, Color, ColorInput, Display, Filters, FlexWrap, Fonts,
    FromCssStr, Length::Px, Node, RenderOptions, Style, StyleDeclaration, Viewport,
  },
  render,
};

fn render_node(fonts: &Fonts, node: Node) {
  let options = RenderOptions::builder()
    .viewport(Viewport::new((512, 512)))
    .node(node)
    .fonts(fonts)
    .build();
  let image = render(options).unwrap();
  black_box(image);
}

fn effect_node(effect_tw: &str) -> Node {
  Node::container([]).with_tw(
    format!("w-[256px] h-[256px] bg-white {effect_tw}")
      .parse()
      .unwrap(),
  )
}

fn backdrop_blur_cards() -> Node {
  let cards = (0..16).map(|_| {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::width(Px(120.0)))
        .with(StyleDeclaration::height(Px(120.0)))
        .with(StyleDeclaration::margin_top(Px(4.0)))
        .with(StyleDeclaration::margin_left(Px(4.0)))
        .border_radius(BorderRadius::from_css_str("24px").unwrap())
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

/// A full-canvas SVG noise overlay, the grain texture many cards draw with `feTurbulence`.
fn svg_turbulence_node() -> Node {
  svg_node(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"><filter id="n"><feTurbulence type="fractalNoise" baseFrequency="0.8" numOctaves="4" stitchTiles="stitch"/></filter><rect width="100%" height="100%" filter="url(#n)"/></svg>"#,
  )
}

/// A blurred SVG shape, filtered in the default `linearRGB` space.
fn svg_blur_node() -> Node {
  svg_node(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"><filter id="b"><feGaussianBlur stdDeviation="12"/></filter><circle cx="256" cy="256" r="160" fill="#3a6ff7" filter="url(#b)"/></svg>"##,
  )
}

/// `svg` drawn as a 512px image.
fn svg_node(svg: &'static str) -> Node {
  Node::image(svg).with_style(
    Style::default()
      .with(StyleDeclaration::width(Px(512.0)))
      .with(StyleDeclaration::height(Px(512.0))),
  )
}

fn bench_effects(c: &mut Criterion) {
  let fonts = Fonts::default();
  let mut group = c.benchmark_group("effects");

  group.bench_function("svg_turbulence", |b| {
    b.iter(|| render_node(&fonts, black_box(svg_turbulence_node())))
  });
  group.bench_function("svg_blur", |b| {
    b.iter(|| render_node(&fonts, black_box(svg_blur_node())))
  });

  group.bench_function("blur_md", |b| {
    b.iter(|| render_node(&fonts, effect_node(black_box("blur-md"))))
  });
  group.bench_function("shadow_md", |b| {
    b.iter(|| render_node(&fonts, effect_node(black_box("shadow-md"))))
  });
  group.bench_function("drop_shadow_md", |b| {
    b.iter(|| render_node(&fonts, effect_node(black_box("drop-shadow-md"))))
  });
  group.bench_function("color_filters", |b| {
    b.iter(|| {
      render_node(
        &fonts,
        effect_node(black_box(
          "bg-gradient-to-r from-red-500 to-blue-500 grayscale-[50%] sepia saturate-150 hue-rotate-90",
        )),
      )
    })
  });
  group.bench_function("backdrop_blur_cards", |b| {
    b.iter(|| render_node(&fonts, black_box(backdrop_blur_cards())))
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
