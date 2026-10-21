use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{Fonts, Node, RenderOptions, Viewport},
  render,
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

fn render_node(fonts: &Fonts, node: Node) {
  let options = RenderOptions::builder()
    .viewport(Viewport::new((BENCH_WIDTH, BENCH_HEIGHT)))
    .node(node)
    .fonts(fonts)
    .build();
  black_box(render(options).unwrap());
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

/// About 40 KB of text in a narrow column, so it breaks into hundreds of lines.
fn document_paragraph() -> Node {
  Node::container([
    Node::text(LONG_TEXT.repeat(64)).with_tw("text-[14px] text-gray-900".parse().unwrap())
  ])
  .with_tw("flex w-[320px] bg-white".parse().unwrap())
}

/// A wrapping row of inline-block badges, each a short word that every measure of its box shapes.
fn badge_row() -> Node {
  let badges = LONG_TEXT
    .split(' ')
    .take(60)
    .map(|word| {
      Node::container([Node::text(word.to_string())]).with_tw(
        "inline-block px-2 py-1 m-1 rounded bg-gray-100"
          .parse()
          .unwrap(),
      )
    })
    .collect::<Vec<_>>();

  Node::container(badges).with_tw(
    "block w-full h-full p-12 text-[18px] bg-white"
      .parse()
      .unwrap(),
  )
}

/// A wrapping row of inline-block tags, each holding a word and a nested inline-block count, so
/// every width the outer box is measured at lays the inner one out again.
fn nested_tags() -> Node {
  let tags = LONG_TEXT
    .split(' ')
    .take(40)
    .enumerate()
    .map(|(index, word)| {
      Node::container([
        Node::text(format!("{word} ")),
        Node::container([Node::text(index.to_string())])
          .with_tw("inline-block px-1 rounded bg-gray-300".parse().unwrap()),
      ])
      .with_tw(
        "inline-block px-2 py-1 m-1 rounded bg-gray-100"
          .parse()
          .unwrap(),
      )
    })
    .collect::<Vec<_>>();

  Node::container(tags).with_tw(
    "block w-full h-full p-12 text-[18px] bg-white"
      .parse()
      .unwrap(),
  )
}

/// Words and emoji interleaved, so every emoji run asks font fallback for a face.
fn emoji_paragraph() -> Node {
  let text = LONG_TEXT
    .split(' ')
    .zip(["🎨", "📐", "✨", "🖋️", "📚"].iter().cycle())
    .map(|(word, emoji)| format!("{word} {emoji}"))
    .collect::<Vec<_>>()
    .join(" ");
  let text = [text.as_str(); 4].join(" ");

  Node::container([Node::text(text).with_tw("text-[20px] text-gray-900".parse().unwrap())])
    .with_tw("flex w-full h-full p-12 bg-white".parse().unwrap())
}

/// A paragraph with a blurred `text-shadow`, which every glyph casts.
fn shadowed_paragraph() -> Node {
  Node::container([Node::text(LONG_TEXT.to_string()).with_tw(
    "text-[28px] text-gray-900 text-shadow-[2px_2px_6px_rgb(0_0_0/0.5)]"
      .parse()
      .unwrap(),
  )])
  .with_tw("flex w-full h-full p-12 bg-white".parse().unwrap())
}

fn bench_text(c: &mut Criterion) {
  let fonts = common::fonts();
  let mut group = c.benchmark_group("text");

  group.bench_function("long_paragraph", |b| {
    b.iter(|| render_node(&fonts, black_box(long_paragraph())))
  });
  group.bench_function("clipped_text_paragraph", |b| {
    b.iter(|| render_node(&fonts, black_box(clipped_text_paragraph())))
  });

  group.bench_function("document_paragraph", |b| {
    b.iter(|| render_node(&fonts, black_box(document_paragraph())))
  });
  group.bench_function("emoji_paragraph", |b| {
    b.iter(|| render_node(&fonts, black_box(emoji_paragraph())))
  });

  group.bench_function("badge_row", |b| {
    b.iter(|| render_node(&fonts, black_box(badge_row())))
  });
  group.bench_function("nested_tags", |b| {
    b.iter(|| render_node(&fonts, black_box(nested_tags())))
  });
  group.bench_function("shadowed_paragraph", |b| {
    b.iter(|| render_node(&fonts, black_box(shadowed_paragraph())))
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
