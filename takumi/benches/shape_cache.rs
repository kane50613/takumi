use std::{cell::Cell, hint::black_box};

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{Fonts, Node, RenderOptions, Viewport},
  render,
  unstable::base::resources::shape_cache::set_shape_cache_max_bytes,
};

const DASHBOARD_ROWS: usize = 24;
const DASHBOARD_CHANGING_VALUES: usize = 3;

const BENCH_WIDTH: u32 = 1200;
const BENCH_HEIGHT: u32 = 630;

/// A kind of dashboard text.
#[derive(Clone, Copy)]
enum Kind {
  /// Short single-line labels and values.
  Short,
  /// Long runs that wrap.
  Wrapping,
  /// Runs with emoji.
  Emoji,
  /// Text around an inline-block, which is never cached.
  InlineBox,
}

impl Kind {
  const ALL: [(&'static str, Kind); 4] = [
    ("short", Kind::Short),
    ("wrapping", Kind::Wrapping),
    ("emoji", Kind::Emoji),
    ("inline_box", Kind::InlineBox),
  ];

  fn row(self, row: usize, value: u64) -> Node {
    let text =
      |text: String| Node::text(text).with_tw("text-[20px] text-gray-900".parse().unwrap());

    match self {
      Kind::Short => Node::container([text(format!("Metric number {row}")), text(format!("{value} ms"))]),
      Kind::Wrapping => Node::container([text(format!(
        "#{row} Fix flaky layout measurement when a wrapped paragraph is re-rendered with a longer title ({value})"
      ))])
      .with_tw("block w-[360px]".parse().unwrap()),
      Kind::Emoji => Node::container([text(format!("🚀 Deploy {row} ✅ build {value} passed 🔥"))]),
      Kind::InlineBox => Node::container([
        text(format!("Review requested on repo {row} ")),
        Node::container([text(format!("{value}"))])
          .with_tw("inline-block px-2 bg-gray-200".parse().unwrap()),
        text(" open".to_string()),
      ])
      .with_tw("block".parse().unwrap()),
    }
  }
}

/// A column of `kind` rows where `tick` changes only the first few values.
fn dashboard_node(kind: Kind, tick: u64) -> Node {
  let rows = (0..DASHBOARD_ROWS).map(|row| {
    let value = if row < DASHBOARD_CHANGING_VALUES {
      1000 + tick * 7 + row as u64
    } else {
      row as u64
    };

    kind.row(row, value)
  });

  Node::container(rows.collect::<Vec<_>>())
    .with_tw("flex flex-col w-full h-full p-4".parse().unwrap())
}

fn render_node(fonts: &Fonts, node: Node) {
  let opts = RenderOptions::builder()
    .viewport(Viewport::new((BENCH_WIDTH, BENCH_HEIGHT)))
    .node(node)
    .fonts(fonts)
    .build();

  black_box(render(opts).unwrap());
}

fn bench_dashboard(c: &mut Criterion) {
  let fonts = common::fonts();

  for (kind_name, kind) in Kind::ALL {
    let mut group = c.benchmark_group(format!("shape_cache_dashboard_{kind_name}"));

    for (name, budget) in [("cache_off", 0), ("cache_on", 16 * 1024 * 1024)] {
      let tick = Cell::new(0u64);

      group.bench_function(name, |b| {
        set_shape_cache_max_bytes(budget);
        b.iter(|| {
          tick.set(tick.get() + 1);
          render_node(&fonts, black_box(dashboard_node(kind, tick.get())));
        });
      });
    }

    group.finish();
  }

  set_shape_cache_max_bytes(0);
}

mod common;

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_dashboard
}
criterion_main!(benches);
