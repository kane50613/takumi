use std::borrow::Cow;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  FontContext,
  layout::{Viewport, node::Node},
  rendering::{ImageOutputFormat, RenderOptions, render, write_image},
};

mod common;

const W: u32 = 1200;
const H: u32 = 630;
const IMAGE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/images/yeecord.png");

fn bench_encode(c: &mut Criterion) {
  let font_context = FontContext::default();
  let node =
    Node::container([Node::image(IMAGE_PATH).with_tw("flex w-full h-full".parse().unwrap())])
      .with_tw("flex w-full h-full bg-white".parse().unwrap());
  let image = render(
    RenderOptions::builder()
      .viewport(Viewport::new((W, H)))
      .node(node)
      .font_context(&font_context)
      .build(),
  )
  .unwrap();

  let mut group = c.benchmark_group("encode");
  for (name, format) in [
    ("png", ImageOutputFormat::Png),
    ("webp", ImageOutputFormat::WebP),
    ("jpeg", ImageOutputFormat::Jpeg),
  ] {
    group.bench_function(name, |b| {
      b.iter(|| {
        let mut buf = Vec::with_capacity(1 << 20);
        write_image(Cow::Borrowed(&image), &mut buf, format, None).unwrap();
        black_box(buf.len())
      })
    });
  }
  group.finish();
}

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_encode
}
criterion_main!(benches);
