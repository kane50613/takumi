use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use takumi::{
  GlobalContext,
  layout::{
    Viewport,
    node::{ContainerNode, NodeKind},
    style::{BackgroundImages, FromCss, Length, Style, StyleDeclaration},
  },
  rendering::{RenderOptionsBuilder, render},
};

fn run_gradient_render(global: &GlobalContext, background_image_str: &str) {
  let style = Style::default()
    .with(StyleDeclaration::width(Length::Px(256.0)))
    .with(StyleDeclaration::height(Length::Px(256.0)))
    .with(StyleDeclaration::background_image(
      BackgroundImages::from_str(background_image_str).ok(),
    ));

  let node = NodeKind::Container(ContainerNode {
    children: None,
    preset: None,
    style: Some(style),
    tw: None,
    class_name: None,
    id: None,
    tag_name: None,
  });

  let viewport = Viewport::new(Some(512), Some(512));

  let options = RenderOptionsBuilder::default()
    .viewport(viewport)
    .node(node)
    .global(global)
    .build()
    .unwrap();

  let _image = render(options).unwrap();
}

fn bench_gradients(c: &mut Criterion) {
  let global = GlobalContext::default();

  let mut group = c.benchmark_group("gradient");

  // Basic two-stop linear gradient
  group.bench_function("linear_2_stops", |b| {
    b.iter(|| run_gradient_render(&global, black_box("linear-gradient(to right, red, blue)")))
  });

  // More complex multi-stop linear gradient
  group.bench_function("linear_5_stops", |b| {
    b.iter(|| {
      run_gradient_render(
        &global,
        black_box("linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)"),
      )
    })
  });

  // Semi-transparent gradient
  group.bench_function("linear_transparent", |b| {
    b.iter(|| {
      run_gradient_render(
        &global,
        black_box("linear-gradient(180deg, rgba(0,128,255,0.9), rgba(0,128,255,0))"),
      )
    })
  });

  group.finish();
}

criterion_group!(benches, bench_gradients);
criterion_main!(benches);
