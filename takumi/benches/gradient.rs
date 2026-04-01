use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use takumi::{
  GlobalContext,
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

fn render_gradient_node(global: &GlobalContext, node: Node) {
  let viewport = Viewport::new((BENCH_WIDTH, BENCH_HEIGHT));

  let options = RenderOptions::builder()
    .viewport(viewport)
    .node(node)
    .global(global)
    .build();

  let image = render(options).unwrap();
  black_box(image);
}

fn run_gradient_render(global: &GlobalContext, background_image_str: &str) {
  let background_images = BackgroundImages::from_str(background_image_str).ok();
  let node = build_gradient_node(background_images);
  render_gradient_node(global, node);
}

fn run_gradient_render_preparsed(global: &GlobalContext, background_images: &BackgroundImages) {
  let node = build_gradient_node(Some(background_images.clone()));
  render_gradient_node(global, node);
}

fn run_gradient_render_prebuilt_node(global: &GlobalContext, node: &Node) {
  let viewport = Viewport::new((BENCH_WIDTH, BENCH_HEIGHT));

  let options = RenderOptions::builder()
    .viewport(viewport)
    .node(node.clone())
    .global(global)
    .build();

  let image = render(options).unwrap();
  black_box(image);
}

fn bench_gradients(c: &mut Criterion) {
  let global = GlobalContext::default();
  let simple_linear = "linear-gradient(to right, red, blue)";
  let multi_stop_linear = "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)";
  let transparent_linear = "linear-gradient(180deg, rgba(0,128,255,0.9), rgba(0,128,255,0))";
  let simple_radial = "radial-gradient(circle, red, blue)";
  let simple_conic = "conic-gradient(red, blue)";

  let mut group = c.benchmark_group("gradient");

  // Basic two-stop linear gradient
  group.bench_function("linear_2_stops_1200x630", |b| {
    b.iter(|| run_gradient_render(&global, black_box(simple_linear)))
  });

  // More complex multi-stop linear gradient
  group.bench_function("linear_5_stops_1200x630", |b| {
    b.iter(|| run_gradient_render(&global, black_box(multi_stop_linear)))
  });

  // Semi-transparent gradient
  group.bench_function("linear_transparent_1200x630", |b| {
    b.iter(|| run_gradient_render(&global, black_box(transparent_linear)))
  });

  group.bench_function("radial_2_stops_1200x630", |b| {
    b.iter(|| run_gradient_render(&global, black_box(simple_radial)))
  });

  group.bench_function("conic_2_stops_1200x630", |b| {
    b.iter(|| run_gradient_render(&global, black_box(simple_conic)))
  });

  group.finish();

  let parsed_simple_linear = BackgroundImages::from_str(simple_linear).unwrap();
  let preparsed_node = build_gradient_node(Some(parsed_simple_linear.clone()));

  let mut component_group = c.benchmark_group("gradient_components");

  component_group.bench_function("parse_linear_2_stops", |b| {
    b.iter(|| BackgroundImages::from_str(black_box(simple_linear)).unwrap())
  });

  component_group.bench_function("render_preparsed_linear_2_stops", |b| {
    b.iter(|| run_gradient_render_preparsed(&global, black_box(&parsed_simple_linear)))
  });

  component_group.bench_function("render_prebuilt_node_linear_2_stops", |b| {
    b.iter(|| run_gradient_render_prebuilt_node(&global, black_box(&preparsed_node)))
  });

  component_group.finish();
}

criterion_group!(benches, bench_gradients);
criterion_main!(benches);
