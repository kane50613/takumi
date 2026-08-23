use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{Fonts, Node, RenderOptions, Viewport},
  render,
};

mod common;

const FANOUT: usize = 4;
const DEPTHS: [u32; 3] = [4, 5, 6];

/// A nested flex tree, the shape that makes taffy run several sizing passes
/// over every node.
fn nested_flex(depth: u32) -> Node {
  if depth == 0 {
    return Node::container([]).with_tw("w-2 h-2 bg-white".parse().expect("valid classes"));
  }

  Node::container(
    (0..FANOUT)
      .map(|_| nested_flex(depth - 1))
      .collect::<Vec<_>>(),
  )
  .with_tw("flex flex-row grow".parse().expect("valid classes"))
}

fn bench_nested_flex(c: &mut Criterion) {
  let fonts = Fonts::default();
  let mut group = c.benchmark_group("layout");

  for depth in DEPTHS {
    let node = nested_flex(depth);

    group.bench_function(BenchmarkId::new("nested_flex", depth), |b| {
      b.iter(|| {
        black_box(
          render(
            RenderOptions::builder()
              .viewport(Viewport::new((512, 512)))
              .node(node.clone())
              .fonts(&fonts)
              .build(),
          )
          .expect("renders"),
        )
      })
    });
  }

  group.finish();
}

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_nested_flex
}
criterion_main!(benches);
