//! Renders the same content repeatedly and asserts live heap bytes stay flat,
//! so cache-budget regressions and allocator-retention bugs fail a test
//! instead of surfacing as server memory growth.

mod test_utils;

use std::{
  alloc::{GlobalAlloc, Layout, System},
  collections::HashMap,
  sync::{
    Arc, Mutex,
    atomic::{AtomicIsize, Ordering},
  },
};

use takumi::{
  prelude::{Length::*, *},
  render,
  unstable::raster::render_pages,
};
use takumi_core::{
  geometry::Size,
  resources::{image::ResourceCache, image_buffer::ImageBuffer},
};
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

/// `LIVE_BYTES` is process-wide; serialize tests that measure it.
static MEASURE_LOCK: Mutex<()> = Mutex::new(());

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

  let _guard = MEASURE_LOCK.lock().unwrap();

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

const UNIQUE_TEXT_CSS: &str = "
.bg { width: 2400px; height: 1260px; background-image: radial-gradient(circle at 0% 0%, #e94560 0%, transparent 60%), radial-gradient(circle at 100% 100%, #533483 0%, #1a1a2e 70%); padding: 96px; }
.title { background-image: linear-gradient(90deg, #ffffff, #e94560); background-clip: text; color: transparent; }";

/// Eight CJK codepoints never rendered before, so every glyph is a fresh
/// mask-cache key and a guaranteed miss; repeated Latin strings would re-hit
/// the same glyph keys after the first few renders.
fn unique_text_card(index: usize) -> Node {
  let title: String = (0..8)
    .map(|offset| char::from_u32(0x4E00 + (index * 8 + offset) as u32).unwrap())
    .collect();

  Node::container([Node::container([Node::text(title)]).with_class_name("title")])
    .with_class_name("bg")
    .with_style(Style::default().with(StyleDeclaration::font_size(FontSize::Length(Px(128.0)))))
}

/// Every render inserts glyph masks under keys the cache has never seen, so
/// entries accumulate until eviction kicks in. Retained bytes must track the
/// glyph-cache byte budget: each entry is charged the capacity it actually
/// holds, so unique-text floods plateau near the budget instead of growing by
/// whatever allocation each mask arrived in (issue #1023).
#[test]
fn unique_text_renders_stay_near_glyph_cache_budget() {
  const WARMUP_RENDERS: usize = 8;
  const MEASURED_RENDERS: usize = 120;
  const SLACK_BYTES: isize = 20 << 20;

  let _guard = MEASURE_LOCK.lock().unwrap();

  let cache = ResourceCache::default();
  let stylesheet = cache.get_or_parse_stylesheet(vec![UNIQUE_TEXT_CSS.to_string()]);
  let viewport = Viewport::new((2400, 1260));
  let render_card = |index: usize| {
    let options = RenderOptions::builder()
      .viewport(viewport)
      .node(unique_text_card(index))
      .fonts(&CONTEXT)
      .stylesheet(stylesheet.clone())
      .build();

    render(options).unwrap();
  };

  for _ in 0..WARMUP_RENDERS {
    render_card(0);
  }
  let baseline = LIVE_BYTES.load(Ordering::Relaxed);

  for index in 1..=MEASURED_RENDERS {
    render_card(index);
  }
  let settled = LIVE_BYTES.load(Ordering::Relaxed);

  assert!(
    settled <= baseline + SLACK_BYTES,
    "live bytes grew from {baseline} to {settled} over {MEASURED_RENDERS} unique-text renders"
  );
}

/// Sections of text, an absolute badge, and a table with a header, which lay out across many pages.
fn sections(count: usize) -> String {
  (0..count)
    .map(|index| {
      format!(
        "<section><h2>Section {index}</h2><p>Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.</p><div style=\"position: relative; height: 24px\"><span style=\"position: absolute; right: 0; top: 0\">badge {index}</span></div><table><thead><tr><th>A</th><th>B</th></tr></thead><tbody><tr><td>1</td><td>2</td></tr></tbody></table></section>"
      )
    })
    .collect()
}

#[test]
fn a_streamed_document_keeps_live_bytes_flat_across_pages() {
  const EARLY_PAGE: usize = 50;
  const LATE_PAGE: usize = 200;
  const PAGE_SIZE: Size<f32> = Size::new(400.0, 300.0);

  let _guard = MEASURE_LOCK.lock().unwrap();

  let html = format!("<body style=\"font-size: 12px\">{}</body>", sections(2000));
  let options = RenderOptions::builder()
    .viewport(Viewport::new((400, 300)))
    .node(Node::from_html(&html, FromHtmlOptions::default()).unwrap())
    .fonts(&CONTEXT)
    .build();
  let (mut pages, mut early, mut late) = (0, 0, 0);

  render_pages(options, PAGE_SIZE, usize::MAX, |_| {
    pages += 1;
    match pages {
      EARLY_PAGE => early = LIVE_BYTES.load(Ordering::Relaxed),
      LATE_PAGE => late = LIVE_BYTES.load(Ordering::Relaxed),
      _ => {}
    }
    Ok(())
  })
  .unwrap();

  assert!(pages > LATE_PAGE, "the document has {pages} pages");
  assert!(
    late <= early + early / 20,
    "live bytes grew from {early} after page {EARLY_PAGE} to {late} after page {LATE_PAGE}"
  );
}
