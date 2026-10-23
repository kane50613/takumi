//! A page painted from the window of a layout that has laid out only the pages before it has the
//! chunks the scene of every page gives it.

use std::{fs, path::Path};

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
use takumi_html::{FromHtmlOptions, from_html};

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

/// Paints every page of `name` from its window as the layout and the windows allow.
fn assert_windows_match(name: &str, page_size: Size<f32>) -> usize {
  let root = document(name, page_size);
  let whole = PagedScene::lay_out(root.clone(), page_size, 1000, true)
    .expect("pages lay out")
    .into_scene();
  let mut layout = PagedLayout::new(&root, page_size, 1000);
  let mut windows = PageWindows::new(page_size);
  let mut emitted = 0;

  loop {
    let stepped = layout.next_page().expect("page lays out");

    windows
      .refresh(
        &root,
        layout.view(),
        layout.page_count(),
        layout.finalized_pages(),
      )
      .expect("pages refresh");

    let ready = layout.settled_pages();

    for page in emitted..ready {
      let pages = windows.pages_reaching(page);
      let window =
        SceneRef::window(&root, layout.view(), page_size, &pages, true).expect("window builds");

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
    }
    emitted = emitted.max(ready);
    if !stepped {
      return emitted;
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
