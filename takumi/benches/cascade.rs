use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{Fonts, Node, RenderOptions, StyleSheet, Viewport},
  render,
};

mod common;

const SIZES: [(usize, usize); 3] = [(200, 200), (400, 400), (800, 800)];

/// A stylesheet where almost nothing matches, which is what a utility framework
/// looks like from any single node's point of view.
fn stylesheet(rules: usize) -> StyleSheet {
  let css: String = (0..rules)
    .map(|rule| format!(".unused-{rule} {{ color: rgb({}, 0, 0); }}\n", rule % 256))
    .collect();

  StyleSheet::parse(&css).expect("valid stylesheet")
}

fn document(nodes: usize) -> Node {
  Node::container(
    (0..nodes)
      .map(|node| {
        Node::container([])
          .with_class_name(format!("leaf-{node}"))
          .with_tw("w-1 h-1".parse().expect("valid classes"))
      })
      .collect::<Vec<_>>(),
  )
}

fn bench_cascade(c: &mut Criterion) {
  let fonts = Fonts::default();
  let mut group = c.benchmark_group("cascade");

  for (nodes, rules) in SIZES {
    let stylesheet = stylesheet(rules);
    let node = document(nodes);

    group.bench_function(
      BenchmarkId::from_parameter(format!("{nodes}x{rules}")),
      |b| {
        b.iter(|| {
          black_box(
            render(
              RenderOptions::builder()
                .viewport(Viewport::new((64, 64)))
                .node(node.clone())
                .fonts(&fonts)
                .stylesheet(stylesheet.clone().into())
                .build(),
            )
            .expect("renders"),
          )
        })
      },
    );
  }

  // The reset most stylesheets open with, which matches every element's `::before` and `::after`.
  let reset = StyleSheet::parse(
    "*, ::before, ::after { box-sizing: border-box; margin: 0; padding: 0; border: 0 solid; }",
  )
  .expect("valid stylesheet");
  let node = document(400);

  group.bench_function(BenchmarkId::from_parameter("400x_reset"), |b| {
    b.iter(|| {
      black_box(
        render(
          RenderOptions::builder()
            .viewport(Viewport::new((64, 64)))
            .node(node.clone())
            .fonts(&fonts)
            .stylesheet(reset.clone().into())
            .build(),
        )
        .expect("renders"),
      )
    })
  });

  // Tailwind v4's output registers its `--tw-*` names with `@property` and resets them on every
  // element.
  let registrations: String = (0..30)
    .map(|index| {
      format!("@property --tw-p{index} {{ syntax: \"*\"; inherits: false; initial-value: 0; }}\n")
    })
    .collect();
  let resets: String = (0..10)
    .map(|index| format!("--tw-p{index}: {index};"))
    .collect();
  let properties = format!("{registrations}* {{ {resets} }}");
  let property_rules = StyleSheet::parse(&properties).expect("valid stylesheet");

  group.bench_function(BenchmarkId::from_parameter("400x_property_rules"), |b| {
    b.iter(|| {
      black_box(
        render(
          RenderOptions::builder()
            .viewport(Viewport::new((64, 64)))
            .node(node.clone())
            .fonts(&fonts)
            .stylesheet(property_rules.clone().into())
            .build(),
        )
        .expect("renders"),
      )
    })
  });

  group.finish();
}

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_cascade
}
criterion_main!(benches);
