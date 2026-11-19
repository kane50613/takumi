//! Renders that run in parallel and share one `Cache`, as a server's workers do. Each iteration
//! starts the same render on every thread and waits for all of them, after a warm-up render filled
//! the cache, so it measures the lookups renders contend on.

use std::{collections::HashMap, hint::black_box, num::NonZero, sync::Arc, thread};

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{
    Cache, Display, FlexWrap, Fonts, ImageCacheMode, ImageSource, LengthUnit, Node, RenderOptions,
    Style, Viewport,
  },
  render,
};

mod common;

const ICON_SRC: &str = "icon.svg";

/// A star that reads its host's color, so each draw goes through the reads its layout recorded.
const ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="currentColor" d="M12 2l3 7h7l-5.5 4.5 2 7.5-6.5-4.5-6.5 4.5 2-7.5-5.5-4.5h7z"/></svg>"#;

const PARAGRAPH: &str = "The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs. \
  How vexingly quick daft zebras jump! Sphinx of black quartz, judge my vow.";

fn grid(children: impl IntoIterator<Item = Node>) -> Node {
  Node::container(children.into_iter().collect::<Vec<_>>()).with_style(
    Style::default()
      .display(Display::Flex)
      .flex_wrap(FlexWrap::Wrap)
      .width(100.0.percent())
      .height(100.0.percent()),
  )
}

fn icons() -> Node {
  grid(
    (0..400).map(|_| {
      Node::image(ICON_SRC).with_style(Style::default().width(16.0.px()).height(16.0.px()))
    }),
  )
}

fn text() -> Node {
  grid((0..4).map(|_| Node::text(PARAGRAPH.to_string())))
}

/// Renders `node` once on each of `threads` threads at the same time, all sharing `cache`.
fn render_on(
  threads: usize,
  fonts: &Fonts,
  cache: &Cache,
  images: &HashMap<Arc<str>, ImageSource>,
  node: &Node,
) {
  thread::scope(|scope| {
    for _ in 0..threads {
      scope.spawn(|| {
        let options = RenderOptions::builder()
          .viewport(Viewport::new((320, 320)))
          .fonts(fonts)
          .node(node.clone())
          .images(images.clone())
          .cache(cache.clone())
          .build();

        black_box(render(options).unwrap());
      });
    }
  });
}

fn bench_shared_cache(c: &mut Criterion) {
  let fonts = common::fonts();
  let cache = Cache::default();
  let images = HashMap::from([(
    Arc::from(ICON_SRC),
    cache
      .decode_image(ICON.as_bytes(), ImageCacheMode::Auto)
      .unwrap(),
  )]);
  let threads = thread::available_parallelism().map_or(4, NonZero::get);

  for (name, node) in [("svg_icons", icons()), ("text", text())] {
    render_on(1, &fonts, &cache, &images, &node);

    for count in [1, threads] {
      c.bench_function(&format!("shared_cache/{name}/{count}_threads"), |b| {
        b.iter(|| render_on(count, &fonts, &cache, &images, &node));
      });
    }
  }
}

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_shared_cache
}
criterion_main!(benches);
