use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{Fonts, Node, RenderOptions, Viewport},
  render,
};

fn render_node(fonts: &Fonts, node: Node) {
  let options = RenderOptions::builder()
    .viewport(Viewport::new((1200, 630)))
    .node(node)
    .fonts(fonts)
    .build();

  black_box(render(options).unwrap());
}

fn scene(inner: &str) -> Node {
  Node::container([Node::container([]).with_tw(inner.parse().unwrap())]).with_tw(
    "w-full h-full flex items-center justify-center bg-gradient-to-br from-pink-500 to-indigo-600"
      .parse()
      .unwrap(),
  )
}

fn bench(c: &mut Criterion) {
  let fonts = Fonts::default();
  let mut group = c.benchmark_group("blur_large");

  for (name, tw) in [
    (
      "filter_blur_24",
      "w-[1000px] h-[500px] bg-white blur-[24px]",
    ),
    (
      "filter_blur_64",
      "w-[1000px] h-[500px] bg-white blur-[64px]",
    ),
    (
      "shadow_2xl",
      "w-[800px] h-[400px] bg-white rounded-3xl shadow-2xl",
    ),
    (
      "shadow_blur_96",
      "w-[800px] h-[400px] bg-white shadow-[0_40px_96px_rgba(0,0,0,0.5)]",
    ),
    (
      "backdrop_blur_40",
      "w-[800px] h-[400px] bg-white/20 backdrop-blur-[40px]",
    ),
  ] {
    group.bench_function(name, |b| {
      b.iter(|| render_node(&fonts, scene(black_box(tw))))
    });
  }
}

criterion_group!(benches, bench);
criterion_main!(benches);
