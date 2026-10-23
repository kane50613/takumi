//! `render_pages` hands over the pages the whole-document path paints, whatever a later page paints
//! onto an earlier one.

use std::{
  cell::Cell,
  fs,
  path::{Path, PathBuf},
  sync::Arc,
};

use takumi_core::{
  Fonts,
  geometry::Size,
  layout::node::Node,
  resources::font::FontResource,
  scene::{PagedScene, SceneRef},
  style::StyleSheet,
  viewport::Viewport,
};
use takumi_html::{FromHtmlOptions, from_html};

use crate::{
  Bitmap, Canvas, Error, RenderOptions, Result,
  layout::tree::RenderNode,
  render::{render_pages, render_pages_settling},
  stacking_context::ScenePaint,
};

/// The page of most documents here.
const PAGE_SIZE: Size<f32> = Size {
  width: 200.0,
  height: 100.0,
};

fn fonts() -> Fonts {
  let mut fonts = Fonts::default();
  let data = fs::read(
    Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf"),
  )
  .expect("read test font");

  fonts
    .register(FontResource::new(data))
    .expect("load test font");
  fonts
}

fn options<'f>(
  fonts: &'f Fonts,
  node: &Node,
  stylesheet: &Arc<StyleSheet>,
  page_size: Size<f32>,
) -> RenderOptions<'f> {
  RenderOptions::builder()
    .viewport(Viewport::new((
      page_size.width as u32,
      page_size.height as u32,
    )))
    .node(node.clone())
    .fonts(fonts)
    .stylesheet(stylesheet.clone())
    .build()
}

/// The pages of the document laid out and painted whole, as `render_pages` once did.
fn whole_document(mut options: RenderOptions<'_>, page_size: Size<f32>) -> Result<Vec<Bitmap>> {
  let render_context = options.take_render_context();
  let root = RenderNode::from_node(&render_context, options.node);
  let paged = PagedScene::lay_out(root, page_size, usize::MAX, true)?;
  let page_count = paged.page_count();
  let size = page_size.map(|length| length.round() as u32);
  let scene = paged.into_scene();
  let paint = ScenePaint::new(scene.view());
  let mut pages = Vec::new();

  for index in 0..page_count {
    let origin = SceneRef::page_origin(page_size, index).map(|length| length.round() as i32);
    let mut canvas = Canvas::try_new(size)
      .ok_or(Error::InvalidViewport)?
      .with_origin(origin);

    paint.paint(&mut canvas)?;
    pages.push(canvas.into_inner()?);
  }
  Ok(pages)
}

fn streamed(options: RenderOptions<'_>, page_size: Size<f32>) -> Result<Vec<Bitmap>> {
  let mut pages = Vec::new();

  render_pages(options, page_size, usize::MAX, |page| {
    pages.push(page);
    Ok(())
  })?;
  Ok(pages)
}

/// The pages `render_pages` would hand over if a page were safe once the next is laid out.
fn next_page_laid(options: RenderOptions<'_>, page_size: Size<f32>) -> Result<Vec<Bitmap>> {
  let mut pages = Vec::new();

  render_pages_settling(
    options,
    page_size,
    usize::MAX,
    &|layout| {
      if layout.is_finished() {
        layout.page_count()
      } else {
        layout.page_count() - 1
      }
    },
    |page| {
      pages.push(page);
      Ok(())
    },
  )?;
  Ok(pages)
}

fn first_difference(expected: &[Bitmap], actual: &[Bitmap]) -> Option<String> {
  if expected.len() != actual.len() {
    return Some(format!(
      "{} pages, expected {}",
      actual.len(),
      expected.len()
    ));
  }

  expected
    .iter()
    .zip(actual)
    .position(|(expected, actual)| expected.as_raw() != actual.as_raw())
    .map(|page| format!("page {page} differs"))
}

fn parse(source: &str) -> Node {
  from_html(source, FromHtmlOptions::default()).expect("parse html")
}

/// Checks `render_pages` hands over the pages of `source` the whole-document path paints, and
/// returns how many there are.
fn assert_streams_like_whole(name: &str, source: &str, page_size: Size<f32>) -> usize {
  let fonts = fonts();
  let node = parse(source);
  let stylesheet = Arc::new(StyleSheet::default());
  let expected = whole_document(options(&fonts, &node, &stylesheet, page_size), page_size)
    .expect("whole document renders");
  let actual =
    streamed(options(&fonts, &node, &stylesheet, page_size), page_size).expect("streaming renders");

  assert_eq!(
    first_difference(&expected, &actual),
    None,
    "{name} on pages {page_size:?}"
  );
  expected.len()
}

/// Checks `source` streams like the whole document, and that it would not if a page were settled
/// once the next is laid out, so the document tests the rule that settles a page.
fn assert_needs_more_than_the_next_page(name: &str, source: &str) {
  let pages = assert_streams_like_whole(name, source, PAGE_SIZE);
  let fonts = fonts();
  let node = parse(source);
  let stylesheet = Arc::new(StyleSheet::default());
  let expected = whole_document(options(&fonts, &node, &stylesheet, PAGE_SIZE), PAGE_SIZE)
    .expect("whole document renders");
  let lax =
    next_page_laid(options(&fonts, &node, &stylesheet, PAGE_SIZE), PAGE_SIZE).expect("lax renders");

  assert!(pages > 1, "{name} spans pages");
  assert!(
    first_difference(&expected, &lax).is_some(),
    "{name} paints like the whole document even when a page is settled once the next is laid out"
  );
}

/// `count` empty blocks, one page tall each.
fn pages_of_blocks(count: usize) -> String {
  "<div style=\"height: 100px\"></div>".repeat(count)
}

#[test]
fn a_transform_on_a_late_page_paints_on_an_early_one() {
  assert_needs_more_than_the_next_page(
    "transform",
    &format!(
      "<div>{}<div style=\"height: 100px\"><div style=\"width: 40px; height: 20px; background: #e00; transform: translateY(-300px)\"></div></div>{}</div>",
      pages_of_blocks(5),
      pages_of_blocks(2)
    ),
  );
}

#[test]
fn a_relative_offset_on_a_late_page_paints_on_an_early_one() {
  assert_needs_more_than_the_next_page(
    "relative",
    &format!(
      "<div>{}<div style=\"height: 100px\"><div style=\"position: relative; top: -350px; width: 40px; height: 20px; background: #e00\"></div></div>{}</div>",
      pages_of_blocks(6),
      pages_of_blocks(2)
    ),
  );
}

#[test]
fn a_shadow_crossing_pages_paints_on_an_early_one() {
  assert_needs_more_than_the_next_page(
    "shadow",
    &format!(
      "<div>{}<div style=\"height: 100px\"><div style=\"width: 20px; height: 20px; box-shadow: 0 0 0 250px #06c\"></div></div>{}</div>",
      pages_of_blocks(5),
      pages_of_blocks(2)
    ),
  );
}

#[test]
fn an_initial_containing_block_box_found_on_a_late_page_paints_on_the_first() {
  assert_needs_more_than_the_next_page(
    "initial containing block",
    &format!(
      "<div>{}<div style=\"height: 100px\"><div style=\"position: absolute; top: 0; left: 0; width: 60px; height: 30px; background: #0a0\"></div></div>{}</div>",
      pages_of_blocks(4),
      pages_of_blocks(2)
    ),
  );
}

#[test]
fn a_held_box_taller_than_its_containing_block_paints_before_it() {
  assert_needs_more_than_the_next_page(
    "held box",
    &format!(
      "<div>{}<div style=\"position: relative; height: 200px\"><div style=\"position: absolute; bottom: 0; width: 40px; height: 450px; background: #a0a\"></div></div>{}</div>",
      pages_of_blocks(3),
      pages_of_blocks(2)
    ),
  );
}

#[test]
fn a_negative_margin_on_a_late_page_paints_on_an_early_one() {
  assert_needs_more_than_the_next_page(
    "negative margin",
    &format!(
      "<div>{}<div style=\"height: 20px; margin-bottom: -300px; background: #aaa\"></div><div style=\"height: 40px; width: 40px; background: #e00\"></div>{}</div>",
      pages_of_blocks(5),
      pages_of_blocks(2)
    ),
  );
}

#[test]
fn page_areas_that_go_on_after_the_root_paint_on_its_last_fragment() {
  assert_streams_like_whole(
    "page areas after the root",
    "<div style=\"height: 50px\"></div><div style=\"position: absolute; top: 500px; width: 9px; height: 1000px; background: #e00\"></div>",
    PAGE_SIZE,
  );
}

/// Documents of plain flow and out-of-flow boxes in each kind of containing block.
fn flow_documents() -> Vec<(&'static str, String)> {
  let paragraphs = (0..60)
    .map(|index| {
      format!(
        "<p style=\"margin: 4px 0\">Paragraph {index} of a document that runs over several pages.</p>"
      )
    })
    .collect::<String>();

  vec![
    (
      "text",
      format!("<div style=\"font-size: 14px\">{paragraphs}</div>"),
    ),
    (
      "flex containing block",
      "<div style=\"display: flex; position: relative; height: 350px\"><div style=\"position: absolute; top: 150px; left: 10px; width: 30px; height: 30px; background: #e00\"></div><div style=\"height: 350px; width: 20px; background: #ccc\"></div></div>".to_owned(),
    ),
    (
      "grid containing block",
      "<div style=\"display: grid; position: relative; height: 350px\"><div style=\"position: absolute; top: 250px; left: 10px; width: 30px; height: 30px; background: #e00\"></div><div style=\"height: 350px; width: 20px; background: #ccc\"></div></div>".to_owned(),
    ),
    (
      "table containing block",
      "<table style=\"position: relative; border-collapse: collapse\"><tr><td style=\"height: 320px; border: 1px solid #333\"><div style=\"position: absolute; top: 210px; width: 30px; height: 30px; background: #e00\"></div>cell</td></tr></table>".to_owned(),
    ),
    (
      "inline containing block",
      "<p style=\"margin: 0\">before <span style=\"position: relative\">inside<span style=\"position: absolute; top: 140px; left: 0; width: 30px; height: 30px; background: #e00\"></span></span> after</p><div style=\"height: 300px\"></div>".to_owned(),
    ),
    (
      "fixed footer",
      format!(
        "<div>{}<div style=\"position: fixed; bottom: 0; width: 100%; height: 20px; background: #036\"></div></div>",
        pages_of_blocks(4)
      ),
    ),
    (
      "break before",
      "<div style=\"height: 30px\">one</div><div style=\"break-before: page; height: 30px; position: relative\">two<div style=\"position: absolute; top: 5px; right: 0; width: 20px; height: 20px; background: #e00\"></div></div><div style=\"break-before: page; height: 30px\">three</div>".to_owned(),
    ),
    (
      "outline and text shadow",
      format!(
        "<div>{}<p style=\"margin: 0; outline: 6px solid #0a0; text-shadow: 4px 4px 2px #333; font-size: 20px\">outlined</p>{}</div>",
        pages_of_blocks(2),
        pages_of_blocks(2)
      ),
    ),
    (
      "opacity and filter",
      format!(
        "<div>{}<div style=\"height: 60px; opacity: 0.5; filter: blur(3px); background: #e00\"></div>{}</div>",
        pages_of_blocks(1),
        pages_of_blocks(2)
      ),
    ),
  ]
}

#[test]
fn flow_and_out_of_flow_documents_stream_like_the_whole_document() {
  for (name, source) in flow_documents() {
    for page_size in [
      PAGE_SIZE,
      Size {
        width: 300.0,
        height: 200.0,
      },
      Size {
        width: 160.0,
        height: 60.0,
      },
    ] {
      assert_streams_like_whole(name, &source, page_size);
    }
  }
}

#[test]
fn html_fixtures_stream_like_the_whole_document() {
  for name in [
    "certificate.html",
    "invoice-footer.html",
    "invoice.html",
    "report.html",
  ] {
    let source = fs::read_to_string(
      Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../takumi-pdf/fixtures")
        .join(name),
    )
    .expect("read html fixture");

    for (width, height) in [
      (523.0, 770.0),
      (400.0, 300.0),
      (300.0, 120.0),
      (250.0, 90.0),
    ] {
      assert_streams_like_whole(name, &source, Size { width, height });
    }
  }
}

/// Container-relative lengths paint against the box's container, which the tree no longer holds.
#[test]
fn container_relative_lengths_paint_in_a_paged_document() {
  let fonts = fonts();
  let stylesheet = Arc::new(StyleSheet::default());
  let render = |source: &str| {
    streamed(
      options(&fonts, &parse(source), &stylesheet, PAGE_SIZE),
      PAGE_SIZE,
    )
    .expect("streaming renders")
  };
  let rounded = |radius: &str| {
    format!(
      "<div>{}<div style=\"width: 100px\"><div style=\"width: 60px; height: 60px; background: #e00; border-radius: {radius}\"></div></div>{}</div>",
      pages_of_blocks(2),
      pages_of_blocks(2)
    )
  };
  let text = |font_size: &str| {
    format!(
      "<div>{}<div style=\"width: 200px\"><p style=\"margin: 0; font-size: {font_size}; line-height: 1\">cq text</p></div>{}</div>",
      pages_of_blocks(2),
      pages_of_blocks(2)
    )
  };

  assert_eq!(
    first_difference(&render(&rounded("30px")), &render(&rounded("30cqw"))),
    None
  );
  assert_eq!(
    first_difference(&render(&text("20px")), &render(&text("10cqw"))),
    None
  );
  assert_streams_like_whole("container relative radius", &rounded("30cqw"), PAGE_SIZE);
  assert_streams_like_whole("container relative text", &text("10cqw"), PAGE_SIZE);
}

/// The Web Platform Tests' checkout at `WPT_ROOT` or in `~/.cache/takumi-wpt`, when there is one.
fn wpt_root() -> Option<PathBuf> {
  std::env::var_os("WPT_ROOT")
    .map(PathBuf::from)
    .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".cache/takumi-wpt")))
    .filter(|root| root.is_dir())
}

fn print_documents(directory: &Path, found: &mut Vec<PathBuf>) {
  let Ok(entries) = fs::read_dir(directory) else {
    return;
  };

  for entry in entries.flatten() {
    let path = entry.path();

    if path.is_dir() {
      print_documents(&path, found);
      continue;
    }

    let is_document = path
      .extension()
      .is_some_and(|extension| matches!(extension.to_str(), Some("html" | "htm" | "xht")));
    let is_print = path
      .file_stem()
      .is_some_and(|stem| stem.to_string_lossy().contains("-print"))
      || path
        .components()
        .any(|component| component.as_os_str() == "print");

    if is_document && is_print {
      found.push(path);
    }
  }
}

/// The text of each `<style>` block of `source`.
fn style_blocks(source: &str) -> Vec<&str> {
  let mut blocks = Vec::new();
  let mut rest = source;

  while let Some(start) = rest.find("<style") {
    let Some(open) = rest[start..].find('>') else {
      break;
    };
    let body = &rest[start + open + 1..];
    let Some(end) = body.find("</style>") else {
      break;
    };

    blocks.push(&body[..end]);
    rest = &body[end..];
  }
  blocks
}

#[test]
fn wpt_print_documents_stream_like_the_whole_document() {
  let Some(root) = wpt_root() else {
    return;
  };
  let mut documents = Vec::new();

  print_documents(&root, &mut documents);
  documents.sort();

  // WPT's print page area.
  let page_size = Size {
    width: 384.0,
    height: 192.0,
  };
  let fonts = fonts();
  let mut compared = 0;

  for path in documents {
    let Ok(source) = fs::read_to_string(&path) else {
      continue;
    };
    let Ok(node) = from_html(&source, FromHtmlOptions::default()) else {
      continue;
    };
    let stylesheet = Arc::new(StyleSheet::parse_list_loosy(
      std::iter::once("* { box-sizing: content-box } table { box-sizing: border-box }".to_owned())
        .chain(style_blocks(&source).into_iter().map(str::to_owned)),
    ));
    let expected = whole_document(options(&fonts, &node, &stylesheet, page_size), page_size);
    let actual = streamed(options(&fonts, &node, &stylesheet, page_size), page_size);

    match (expected, actual) {
      (Ok(expected), Ok(actual)) => {
        assert_eq!(
          first_difference(&expected, &actual),
          None,
          "{}",
          path.display()
        );
        compared += 1;
      }
      (Err(_), Err(_)) => {}
      (expected, actual) => panic!(
        "{}: the whole document is {:?} and streaming is {:?}",
        path.display(),
        expected.map(|pages| pages.len()),
        actual.map(|pages| pages.len())
      ),
    }
  }
  eprintln!("{compared} WPT print documents stream like the whole document");
}

/// How many pages were laid out when the first page was handed over, and how many there are.
fn laid_when_first_page_goes(source: &str) -> (usize, usize) {
  let fonts = fonts();
  let node = parse(source);
  let stylesheet = Arc::new(StyleSheet::default());
  let laid = Cell::new(0);
  let mut first = None;
  let mut pages = 0;

  render_pages_settling(
    options(&fonts, &node, &stylesheet, PAGE_SIZE),
    PAGE_SIZE,
    usize::MAX,
    &|layout| {
      laid.set(layout.page_count());
      layout.settled_pages()
    },
    |_| {
      first.get_or_insert(laid.get());
      pages += 1;
      Ok(())
    },
  )
  .expect("pages render");
  (first.expect("a page goes"), pages)
}

#[test]
fn pages_go_while_later_pages_lay_out() {
  let (laid, pages) = laid_when_first_page_goes(&format!("<div>{}</div>", pages_of_blocks(8)));

  assert_eq!(pages, 8);
  assert!(laid < pages, "{laid} of {pages} pages were laid out");
}

#[test]
fn a_box_that_may_paint_any_pages_back_holds_every_page_until_the_layout_ends() {
  let (laid, pages) = laid_when_first_page_goes(&format!(
    "<div>{}<div style=\"transform: translateY(-300px)\"></div></div>",
    pages_of_blocks(8)
  ));

  assert_eq!(laid, pages);
}

#[test]
fn an_absolute_box_found_on_a_late_page_paints_on_the_first_page_of_its_containing_block() {
  assert_needs_more_than_the_next_page(
    "absolute box of a long block",
    &format!(
      "<div><div style=\"position: relative\">{}<div style=\"position: absolute; top: 10px; left: 0; width: 60px; height: 30px; background: #0a0\"></div>{}</div></div>",
      pages_of_blocks(5),
      pages_of_blocks(2)
    ),
  );
}

#[test]
fn a_box_with_a_transform_holds_pages_only_until_it_is_laid_out() {
  let source = format!(
    "<div><div style=\"width: 20px; height: 20px; background: #e00; transform: rotate(2deg)\"></div>{}</div>",
    pages_of_blocks(8)
  );
  let (laid, pages) = laid_when_first_page_goes(&source);

  assert!(laid < pages, "{laid} of {pages} pages were laid out");
  assert_streams_like_whole("rotated box on the first page", &source, PAGE_SIZE);
}

/// A `background-clip: text` box that holds an absolute box whose containing block is an ancestor,
/// followed by `pages_of_blocks`.
fn text_clip_holding_a_hoisted_box() -> String {
  format!(
    "<div style=\"position: relative; font-size: 30px\"><div style=\"background-image: linear-gradient(90deg, #e00, #e00); background-clip: text; color: transparent\"><div>Clip</div><div style=\"position: absolute; top: 10px; left: 50px; color: #06c\">Held</div></div>{}</div>",
    pages_of_blocks(7)
  )
}

#[test]
fn a_text_clip_places_the_boxes_hoisted_out_of_it_while_later_pages_lay_out() {
  let source = text_clip_holding_a_hoisted_box();
  let (laid, pages) = laid_when_first_page_goes(&source);

  assert!(laid < pages, "{laid} of {pages} pages were laid out");
  assert_streams_like_whole("text clip with a hoisted box", &source, PAGE_SIZE);
}
