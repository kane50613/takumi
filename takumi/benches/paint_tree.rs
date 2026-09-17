use std::{fs, hint::black_box, path::Path};

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use serde_json::{Value, json};
use takumi::{
  prelude::{FontOverride, FontResource, Fonts, GenericFamily, Node, RenderOptions, Viewport},
  render,
};
use takumi_core::paint_tree::{PaintTreeOptions, paint_tree};

mod common;

const WORDS: [&str; 12] = [
  "the",
  "quick",
  "brown",
  "fox",
  "jumps",
  "over",
  "lazy",
  "dog",
  "漢字混排",
  "العربية",
  "shapes",
  "type",
];

fn fonts() -> Fonts {
  let mut fonts = Fonts::default();
  let regular: &[u8] = include_bytes!("../../assets/fonts/geist/Geist[wght].woff2");
  fonts
    .register(
      FontResource::new(regular.to_vec())
        .override_info(FontOverride {
          family_name: Some("Geist".into()),
          ..Default::default()
        })
        .generic_family(GenericFamily::SANS_SERIF),
    )
    .unwrap();
  for fallback in [
    "../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
    "../assets/fonts/sil/scheherazade-new-v17-arabic-regular.woff2",
  ] {
    let bytes = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(fallback)).unwrap();
    fonts
      .register(FontResource::new(bytes).generic_family(GenericFamily::SANS_SERIF))
      .unwrap();
  }
  fonts
}

fn text(i: usize) -> Value {
  let words: Vec<&str> = (0..12).map(|k| WORDS[(i * 7 + k) % WORDS.len()]).collect();
  json!({
    "type": "text",
    "text": words.join(" "),
    "style": {
      "fontSize": 14 + (i % 5) * 2,
      "color": if !i.is_multiple_of(2) { "rgb(20,20,20)" } else { "#B3261E" },
      "fontWeight": if i.is_multiple_of(3) { 700 } else { 400 },
      "textDecoration": if i.is_multiple_of(4) { "underline" } else { "none" },
    }
  })
}

fn card(i: usize) -> Value {
  json!({
    "type": "container",
    "style": {
      "display": "flex", "flexDirection": "column", "padding": 12, "gap": 6,
      "backgroundColor": "#F7F3EC", "border": "2px solid #B3261E", "borderRadius": 8,
      "boxShadow": "0 2px 4px rgba(0,0,0,.2)", "width": 280,
    },
    "children": [
      text(i), text(i + 1),
      { "type": "container", "style": { "display": "flex", "gap": 8 }, "children": [text(i + 2), text(i + 3)] },
    ]
  })
}

fn cards() -> Node {
  let tree = json!({
    "type": "container",
    "tw": "flex flex-wrap gap-4 p-6 bg-white",
    "children": (0..24).map(card).collect::<Vec<_>>(),
  });
  serde_json::from_value(tree).expect("valid node")
}

fn bench_paint_tree(c: &mut Criterion) {
  let fonts = fonts();
  let node = cards();
  let mut group = c.benchmark_group("paint_tree");

  group.bench_function("paint", |b| {
    b.iter_batched(
      || node.clone(),
      |node| {
        black_box(
          paint_tree(
            PaintTreeOptions::builder()
              .viewport(Viewport::new((1200, 1600)))
              .node(node)
              .fonts(&fonts)
              .build(),
          )
          .expect("paints"),
        )
      },
      BatchSize::SmallInput,
    )
  });

  group.bench_function("raster", |b| {
    b.iter_batched(
      || node.clone(),
      |node| {
        black_box(
          render(
            RenderOptions::builder()
              .viewport(Viewport::new((1200, 1600)))
              .node(node)
              .fonts(&fonts)
              .build(),
          )
          .expect("renders"),
        )
      },
      BatchSize::SmallInput,
    )
  });

  group.finish();
}

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_paint_tree
}
criterion_main!(benches);
