//! Renders the same content repeatedly and asserts live heap bytes stay flat,
//! so cache-budget regressions and allocator-retention bugs fail a test
//! instead of surfacing as server memory growth.

mod test_utils;

use std::{
  alloc::{GlobalAlloc, Layout, System},
  collections::HashMap,
  sync::{
    Arc,
    atomic::{AtomicIsize, Ordering},
  },
};

use takumi::{
  prelude::{Length::*, *},
  render,
};
use takumi_core::resources::{image::ResourceCache, image_buffer::ImageBuffer};
use test_utils::{CONTEXT, create_test_viewport};

struct CountingAllocator;

static LIVE_BYTES: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    LIVE_BYTES.fetch_add(layout.size() as isize, Ordering::Relaxed);
    unsafe { System.alloc(layout) }
  }

  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    LIVE_BYTES.fetch_sub(layout.size() as isize, Ordering::Relaxed);
    unsafe { System.dealloc(ptr, layout) }
  }

  unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    LIVE_BYTES.fetch_add(
      new_size as isize - layout.size() as isize,
      Ordering::Relaxed,
    );
    unsafe { System.realloc(ptr, layout, new_size) }
  }

  unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    LIVE_BYTES.fetch_add(layout.size() as isize, Ordering::Relaxed);
    unsafe { System.alloc_zeroed(layout) }
  }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const CSS: &str = ".card { background-color: #1e293b; border-radius: 12px; padding: 24px; }";

const SVG_LOGO: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48"><circle cx="24" cy="24" r="20" fill="#38bdf8"/></svg>"##;

fn content() -> Node {
  Node::container([
    Node::image("logo.svg"),
    Node::image("photo.png"),
    Node::text("Retention check 記憶體迴歸 0123456789")
      .with_style(Style::default().with(StyleDeclaration::font_size(FontSize::Length(Px(32.0))))),
  ])
  .with_class_name("card")
  .with_style(Style::default().with(StyleDeclaration::display(Display::Flex)))
}

fn render_once(cache: &ResourceCache, photo_png: &[u8]) {
  let images: HashMap<Arc<str>, ImageSource> = [
    (
      Arc::from("logo.svg"),
      cache
        .get_or_decode(SVG_LOGO.as_bytes(), ImageCacheMode::Auto)
        .unwrap(),
    ),
    (
      Arc::from("photo.png"),
      cache
        .get_or_decode(photo_png, ImageCacheMode::Auto)
        .unwrap(),
    ),
  ]
  .into();

  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(content())
    .fonts(&CONTEXT)
    .images(images)
    .stylesheet(cache.get_or_parse_stylesheet(vec![CSS.to_string()]))
    .build();

  render(options).unwrap();
}

/// Warm caches with a few renders, then assert many more renders leave live
/// heap bytes where the warmup put them (small slack for allocator noise).
#[test]
fn repeated_renders_keep_live_bytes_flat() {
  const WARMUP_RENDERS: usize = 20;
  const MEASURED_RENDERS: usize = 180;
  const SLACK_BYTES: isize = 4 << 20;

  let photo_png = ImageBuffer::from_rgba_bytes(vec![128; 256 * 256 * 4], 256, 256)
    .unwrap()
    .encode_png()
    .unwrap();
  let cache = ResourceCache::default();

  for _ in 0..WARMUP_RENDERS {
    render_once(&cache, &photo_png);
  }
  let baseline = LIVE_BYTES.load(Ordering::Relaxed);

  for _ in 0..MEASURED_RENDERS {
    render_once(&cache, &photo_png);
  }
  let settled = LIVE_BYTES.load(Ordering::Relaxed);

  assert!(
    settled <= baseline + SLACK_BYTES,
    "live bytes grew from {baseline} to {settled} over {MEASURED_RENDERS} renders"
  );
}
