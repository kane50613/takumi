use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use takumi::{
  FontContext,
  layout::{Viewport, node::Node},
  rendering::{RenderOptions, render},
};

fn run_effect_render(font_context: &FontContext, effect_tw: &str) {
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
    .font_context(font_context)
    .build();

  let image = render(options).unwrap();
  black_box(image);
}

fn bench_effects(c: &mut Criterion) {
  let font_context = FontContext::default();

  let mut group = c.benchmark_group("effects");

  group.bench_function("blur_md", |b| {
    b.iter(|| run_effect_render(&font_context, black_box("blur-md")))
  });
  group.bench_function("shadow_md", |b| {
    b.iter(|| run_effect_render(&font_context, black_box("shadow-md")))
  });
  group.bench_function("drop_shadow_md", |b| {
    b.iter(|| run_effect_render(&font_context, black_box("drop-shadow-md")))
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
