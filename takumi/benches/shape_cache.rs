//! Baseline for a cross-render text-shaping cache: text shaping is currently
//! re-done from scratch on every separate `render()` call, even for
//! byte-identical static text, because `ShapeCache` is rebuilt fresh per
//! render. These benches measure the cost of that today, before any fix:
//!
//! - `shape_cache_repeated_static_text` / `shape_cache_repeated_panel`: the
//!   same text (a single line, then several independent labels) rendered
//!   across many separate `render()` calls sharing one `Fonts` -- the
//!   pattern a long-running caller (e.g. a status bar re-rendering the same
//!   labels every tick) hits, and the case a cross-render cache would speed
//!   up.
//! - `shape_cache_unique_text_every_call`: different text on every call, so
//!   no cache could ever hit here -- the baseline a future fix must not
//!   regress.

use std::{cell::Cell, hint::black_box};

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{Fonts, Node, RenderOptions, Viewport},
  render,
};

const BENCH_WIDTH: u32 = 1200;
const BENCH_HEIGHT: u32 = 630;

// A status-bar-shaped line: mixed static labels and short dynamic values,
// representative of a long-running caller's repeated-render workload.
const STATIC_LABEL: &str = "CPU 42% MEM 8.1G Mon 5 09:42 main fix/issue-113";

// Several independent static labels stacked in one scene, closer to a real
// status-bar panel than a single line of text: most modules are unchanged
// between ticks, one or two carry the dynamic value that actually changes.
const PANEL_LABELS: &[&str] = &[
  "Mon 5 09:42",
  "CPU 42%",
  "MEM 8.1G",
  "main fix/issue-113",
  "  workspace-1",
  "  workspace-2",
  "  workspace-3",
  "Battery 87%",
  "Volume 64%",
  "eth0 up",
];

fn text_node(text: &str) -> Node {
  Node::container([
    Node::text(text.to_string()).with_tw("text-[28px] text-gray-900".parse().unwrap())
  ])
  .with_tw("flex w-full h-full p-6 bg-white".parse().unwrap())
}

fn panel_node() -> Node {
  Node::container(
    PANEL_LABELS
      .iter()
      .map(|text| {
        Node::text(text.to_string()).with_tw("text-[20px] text-gray-900 block".parse().unwrap())
      })
      .collect::<Vec<_>>(),
  )
  .with_tw("flex flex-col w-full h-full p-4 bg-white".parse().unwrap())
}

fn render_node(fonts: &Fonts, node: Node) {
  let opts = RenderOptions::builder()
    .viewport(Viewport::new((BENCH_WIDTH, BENCH_HEIGHT)))
    .node(node)
    .fonts(fonts)
    .build();

  black_box(render(opts).unwrap());
}

fn bench_repeated_static_text(c: &mut Criterion) {
  let fonts = common::fonts();
  let mut group = c.benchmark_group("shape_cache_repeated_static_text");

  group.bench_function("baseline", |b| {
    b.iter(|| render_node(&fonts, black_box(text_node(STATIC_LABEL))));
  });

  group.finish();
}

fn bench_repeated_panel(c: &mut Criterion) {
  let fonts = common::fonts();
  let mut group = c.benchmark_group("shape_cache_repeated_panel");

  group.bench_function("baseline", |b| {
    b.iter(|| render_node(&fonts, black_box(panel_node())));
  });

  group.finish();
}

fn bench_unique_text_every_call(c: &mut Criterion) {
  let fonts = common::fonts();
  let mut group = c.benchmark_group("shape_cache_unique_text_every_call");
  let counter = Cell::new(0u64);

  group.bench_function("baseline", |b| {
    b.iter(|| {
      let sample = counter.get();
      counter.set(sample + 1);
      render_node(
        &fonts,
        black_box(text_node(&format!("unique sample {sample}"))),
      );
    });
  });

  group.finish();
}

mod common;

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_repeated_static_text, bench_repeated_panel, bench_unique_text_every_call
}
criterion_main!(benches);
