use criterion::{Criterion, criterion_group, criterion_main};
use parley::{GenericFamily, fontique::FontInfoOverride};
use std::hint::black_box;
use takumi::{
  FontContext,
  layout::{Viewport, node::Node},
  rendering::{RenderOptions, render},
  resources::font::FontResource,
};

const BENCH_WIDTH: u32 = 1200;
const BENCH_HEIGHT: u32 = 630;

const LONG_TEXT: &str = "Typography is the art and technique of arranging type to make written language legible, \
   readable and appealing when displayed. The arrangement of type involves selecting typefaces, \
   point sizes, line lengths, line-spacing and letter-spacing, and adjusting the space between \
   pairs of letters. The term typography is also applied to the style, arrangement, and \
   appearance of the letters, numbers, and symbols created by the process. Type design is a \
   closely related craft, sometimes considered part of typography; most typographers do not \
   design typefaces, and some type designers do not consider themselves typographers.";

fn load_global() -> FontContext {
  let mut g = FontContext::default();
  let regular: &[u8] = include_bytes!("../../assets/fonts/geist/Geist[wght].woff2");
  g.load_and_store(
    FontResource::new(regular.to_vec())
      .override_info(FontInfoOverride {
        family_name: Some("Geist"),
        ..Default::default()
      })
      .generic_family(GenericFamily::SansSerif),
  )
  .unwrap();
  let emoji: &[u8] = include_bytes!("../../assets/fonts/twemoji/TwemojiMozilla-colr.woff2");
  g.load_and_store(
    FontResource::new(emoji.to_vec())
      .override_info(FontInfoOverride {
        family_name: Some("Twemoji Mozilla"),
        ..Default::default()
      })
      .generic_family(GenericFamily::Emoji),
  )
  .unwrap();
  g
}

fn render_node(font_context: &FontContext, node: Node) {
  let opts = RenderOptions::builder()
    .viewport(Viewport::new((BENCH_WIDTH, BENCH_HEIGHT)))
    .node(node)
    .font_context(font_context)
    .build();
  black_box(render(opts).unwrap());
}

fn long_paragraph() -> Node {
  Node::container([
    Node::text(LONG_TEXT.to_string()).with_tw("text-[28px] text-gray-900".parse().unwrap())
  ])
  .with_tw("flex w-full h-full p-12 bg-white".parse().unwrap())
}

fn clipped_text_paragraph() -> Node {
  Node::container([
    Node::text(LONG_TEXT.to_string()).with_tw(
      "text-[36px] font-extrabold bg-clip-text text-transparent bg-gradient-to-r from-red-500 via-yellow-400 to-blue-600".parse().unwrap(),
    ),
  ])
  .with_tw("flex w-full h-full p-12 items-center justify-center bg-white".parse().unwrap())
}

fn bench_text(c: &mut Criterion) {
  let g = load_global();
  let mut group = c.benchmark_group("text");
  group.bench_function("long_paragraph", |b| {
    b.iter(|| render_node(&g, black_box(long_paragraph())))
  });
  group.bench_function("clipped_text_paragraph", |b| {
    b.iter(|| render_node(&g, black_box(clipped_text_paragraph())))
  });
  group.finish();
}

mod common;

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_text
}
criterion_main!(benches);
