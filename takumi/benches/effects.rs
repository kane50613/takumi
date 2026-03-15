use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use takumi::{
  GlobalContext,
  layout::{Viewport, node::Node},
  rendering::{RenderOptions, render},
};

fn run_effect_render(global: &GlobalContext, effect_tw: &str) {
  // We set a reasonable size and background so the effect is actually computed
  let node = Node::container([]).with_tw(
    format!("w-[256px] h-[256px] bg-white {effect_tw}")
      .parse()
      .unwrap(),
  );

  let viewport = Viewport::new(Some(512), Some(512));

  let options = RenderOptions::builder()
    .viewport(viewport)
    .node(node)
    .global(global)
    .build();

  let _image = render(options).unwrap();
}

fn bench_effects(c: &mut Criterion) {
  let global = GlobalContext::default();

  let mut group = c.benchmark_group("effects");

  // Basic blur
  group.bench_function("blur_md", |b| {
    b.iter(|| run_effect_render(&global, black_box("blur-md")))
  });
  group.bench_function("blur_3xl", |b| {
    b.iter(|| run_effect_render(&global, black_box("blur-3xl")))
  });

  // Box shadow
  group.bench_function("shadow_md", |b| {
    b.iter(|| run_effect_render(&global, black_box("shadow-md")))
  });
  group.bench_function("shadow_2xl", |b| {
    b.iter(|| run_effect_render(&global, black_box("shadow-2xl")))
  });

  // Drop shadow
  group.bench_function("drop_shadow_md", |b| {
    b.iter(|| run_effect_render(&global, black_box("drop-shadow-md")))
  });
  group.bench_function("drop_shadow_2xl", |b| {
    b.iter(|| run_effect_render(&global, black_box("drop-shadow-2xl")))
  });

  group.finish();
}

criterion_group!(benches, bench_effects);
criterion_main!(benches);
