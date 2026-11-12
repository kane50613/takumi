//! A page painted from the window of a layout that has laid out only the pages before it has the
//! chunks the scene of every page gives it.

use std::{fs, path::Path};

use takumi_core::html::{FromHtmlOptions, from_html};
use takumi_core::{
  Fonts,
  context::RenderContext,
  geometry::Size,
  layout::{node::Node, paged_layout::PagedLayout, tree::RenderNode},
  paint_chunk::PaintChunk,
  resources::font::FontResource,
  scene::{PageWindows, PagedScene, SceneBounds, SceneRef, StackingContextNode},
  style::{Display, Length, SizingContext, Style, StyleDeclaration},
  viewport::Viewport,
};

fn fonts() -> Fonts {
  let mut fonts = Fonts::default();

  for path in [
    "../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf",
    "../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
  ] {
    let data = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).expect("read test font");

    fonts
      .register(FontResource::new(data))
      .expect("load test font");
  }
  fonts
}

fn document(name: &str, page_size: Size<f32>) -> RenderNode {
  let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("fixtures")
    .join(name);
  let source = fs::read_to_string(path).expect("read html fixture");
  let node = from_html(&source, FromHtmlOptions::default()).expect("parse html fixture");
  let root = Node::container([node]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::FlowRoot))
      .with(StyleDeclaration::width(Length::Percentage(100.0))),
  );
  let viewport = Viewport::new((page_size.width as u32, page_size.height as u32));
  let context = RenderContext::builder()
    .fonts(fonts().snapshot())
    .sizing(SizingContext::builder().viewport(viewport).build())
    .build();

  RenderNode::from_initial_containing_block(&context, root)
}

fn page_chunks(contexts: &[StackingContextNode], page: usize, page_height: f32) -> Vec<String> {
  let (top, bottom) = (page as f32 * page_height, (page + 1) as f32 * page_height);
  let reaches = |bounds: Option<SceneBounds>| {
    bounds.is_none_or(|bounds| {
      !bounds.is_empty() && (bounds.top as f32) < bottom && (bounds.bottom as f32) > top
    })
  };

  PaintChunk::in_paint_order(contexts)
    .into_iter()
    .filter(|chunk| reaches(chunk.node.paint_bounds))
    .map(|chunk| {
      let paint = chunk.node;

      format!(
        "{:?} {} {:?} {:?} {:?}",
        paint.path, paint.fragment_index, chunk.part, paint.transform, paint.paint_bounds
      )
    })
    .collect()
}

/// Paints every page of `name` from its window as the layout and the windows allow, and returns
/// the pages each page's window reaches.
fn assert_windows_match(name: &str, page_size: Size<f32>) -> Vec<Vec<usize>> {
  let root = document(name, page_size);
  let whole = PagedScene::lay_out(root.clone(), page_size.into(), 1000, true)
    .expect("pages lay out")
    .into_scene();
  let mut layout = PagedLayout::new(&root, page_size.into(), 1000);
  let mut windows = PageWindows::default();
  let mut reached = Vec::new();

  loop {
    let stepped = layout.next_page().expect("page lays out");

    windows
      .refresh(
        &root,
        layout.view(),
        layout.sizes(),
        layout.finalized_pages(),
      )
      .expect("pages refresh");

    let ready = layout.settled_pages();

    for page in reached.len()..ready {
      let pages = windows.pages_reaching(layout.sizes(), page);
      let window = SceneRef::window(&root, layout.view(), layout.sizes(), &pages, true)
        .expect("window builds");

      let expected = page_chunks(&whole.contexts, page, page_size.height);
      let actual = page_chunks(&window.contexts, page, page_size.height);

      if actual != expected {
        let missing: Vec<_> = expected
          .iter()
          .filter(|line| !actual.contains(line))
          .collect();
        let extra: Vec<_> = actual
          .iter()
          .filter(|line| !expected.contains(line))
          .collect();

        panic!(
          "{name} on pages {page_size:?}: page {page} from the window of pages {pages:?}\nmissing {missing:#?}\nextra {extra:#?}\nsame set {}",
          actual.len() == expected.len()
        );
      }
      reached.push(pages);
    }
    if !stepped {
      return reached;
    }
  }
}

#[test]
fn html_fixtures_paint_from_their_windows() {
  for name in [
    "certificate.html",
    "invoice-footer.html",
    "invoice.html",
    "report.html",
  ] {
    for (width, height) in [
      (523.0, 770.0),
      (400.0, 300.0),
      (300.0, 120.0),
      (250.0, 90.0),
    ] {
      assert_windows_match(name, Size { width, height });
    }
  }
}

/// A box positioned in a parent broken across every page paints from the window of its own page,
/// not from one of every page the parent visits.
#[test]
fn positioned_box_of_a_parent_on_every_page_paints_from_its_page() {
  let page_size = Size {
    width: 400.0,
    height: 300.0,
  };
  let reached = assert_windows_match("positioned-in-fragmented-parent.html", page_size);

  assert!(reached.len() > 3);
  for (page, pages) in reached.iter().enumerate() {
    assert!(
      pages.iter().all(|&other| other.abs_diff(page) <= 1),
      "page {page} paints from the window of pages {pages:?}"
    );
  }
}
