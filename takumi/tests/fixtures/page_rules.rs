use std::{fs::File, sync::Arc};

use takumi::{prelude::*, unstable::raster::render_pages, write_image};
use takumi_core::{
  geometry::{Rect, Size},
  layout::page_sizes::PageSizer,
};

use crate::test_utils::{CONTEXT, generated_path};

/// A first page, an unnamed page, and a named page, each rasterized at the area its `@page` rule
/// gives it.
#[test]
fn page_rules() {
  let node = Node::from_html(
    r#"<div class="a"></div><div class="b"></div><div class="c"></div>"#,
    FromHtmlOptions::builder().build(),
  )
  .unwrap();
  let stylesheet = Arc::new(
    StyleSheet::parse(
      "@page { size: 160px 120px; margin: 20px }
       @page :first { size: 240px 100px; margin: 10px 30px }
       @page wide { size: 300px 80px; margin: 10% }
       div { height: 40px }
       .a { background: tomato }
       .b { break-before: page; background: gold }
       .c { page: wide; background: teal }",
    )
    .unwrap(),
  );
  let options = RenderOptions::builder()
    .viewport(Viewport::new((200, 200)))
    .node(node)
    .fonts(&CONTEXT)
    .stylesheet(Arc::clone(&stylesheet))
    .build();
  let sizer = PageSizer::from_page_rules(
    stylesheet,
    Size {
      width: 200.0,
      height: 200.0,
    },
    Rect::all(0.0),
  );
  let mut pages = Vec::new();

  render_pages(options, sizer, 8, |page| {
    pages.push(page);
    Ok(())
  })
  .unwrap();

  let sizes: Vec<_> = pages
    .iter()
    .map(|page| (page.width(), page.height()))
    .collect();

  assert_eq!(sizes, [(180, 80), (120, 80), (240, 64)]);

  for (index, page) in pages.iter().enumerate() {
    let mut file = File::create(generated_path(&format!(
      "page_rules_page{}.webp",
      index + 1
    )))
    .unwrap();

    write_image(page, &mut file, OutputFormat::WebPLossless).unwrap();
  }
}

/// `vw` and `vh` in an `@page` rule take the fractional page size.
#[test]
fn page_rules_viewport_units() {
  let stylesheet = Arc::new(StyleSheet::parse("@page { size: 100vw 100vh; margin: 0 }").unwrap());
  let options = RenderOptions::builder()
    .viewport(Viewport::new((200, 200)))
    .node(Node::container([]))
    .fonts(&CONTEXT)
    .stylesheet(Arc::clone(&stylesheet))
    .build();
  let sizer = PageSizer::from_page_rules(
    stylesheet,
    Size {
      width: 200.9,
      height: 100.9,
    },
    Rect::all(0.0),
  );
  let mut sizes = Vec::new();

  render_pages(options, sizer, 1, |page| {
    sizes.push((page.width(), page.height()));
    Ok(())
  })
  .unwrap();

  assert_eq!(sizes, [(201, 101)]);
}
