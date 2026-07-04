use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use takumi::{
  Fonts,
  layout::{
    Viewport,
    node::Node,
    style::{BackgroundImages, FromCss, Length, Style, StyleDeclaration},
  },
  rendering::{RenderOptions, render},
};

const BENCH_WIDTH: u32 = 1200;
const BENCH_HEIGHT: u32 = 630;

fn build_gradient_node(background_images: Option<BackgroundImages>) -> Node {
  let style = Style::default()
    .with(StyleDeclaration::width(Length::Px(BENCH_WIDTH as f32)))
    .with(StyleDeclaration::height(Length::Px(BENCH_HEIGHT as f32)))
    .with(StyleDeclaration::background_image(background_images));

  Node::container([]).with_style(style)
}

fn render_gradient_node(fonts: &Fonts, node: Node) {
  let viewport = Viewport::new((BENCH_WIDTH, BENCH_HEIGHT));

  let options = RenderOptions::builder()
    .viewport(viewport)
    .node(node)
    .fonts(fonts)
    .build();

  let image = render(options).unwrap();
  black_box(image);
}

fn run_gradient_render(fonts: &Fonts, background_image_str: &str) {
  let background_images = BackgroundImages::from_css_str(background_image_str).ok();
  let node = build_gradient_node(background_images);
  render_gradient_node(fonts, node);
}

fn bench_gradients(c: &mut Criterion) {
  let fonts = Fonts::default();
  let mut group = c.benchmark_group("gradient");

  group.bench_function("linear_2_stops_1200x630", |b| {
    b.iter(|| run_gradient_render(&fonts, black_box("linear-gradient(to right, red, blue)")))
  });
  group.bench_function("radial_2_stops_1200x630", |b| {
    b.iter(|| run_gradient_render(&fonts, black_box("radial-gradient(circle, red, blue)")))
  });
  group.bench_function("conic_2_stops_1200x630", |b| {
    b.iter(|| run_gradient_render(&fonts, black_box("conic-gradient(red, blue)")))
  });

  group.finish();
}

mod common;

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_gradients
}
criterion_main!(benches);
