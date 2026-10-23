//! A page drawn as soon as no later page paints on it comes out as it does when the layout runs
//! to its end first.

use std::cell::Cell;

use takumi_core::{Fonts, layout::paged_layout::PagedLayout};
use takumi_html::{FromHtmlOptions, from_html};

use crate::{
  PageMargins, PageOptions, PageRange, PdfOptions, Tagging, render, render_settling,
  test_support::fonts,
};

const PAGE: PageOptions = PageOptions {
  width: 320.0,
  height: 240.0,
  margin: PageMargins::uniform(24.0),
};

fn options<'f>(source: &str, fonts: &'f Fonts) -> PdfOptions<'f> {
  PdfOptions::builder()
    .node(from_html(source, FromHtmlOptions::default()).expect("parse the document"))
    .page(PAGE)
    .producer("takumi-pdf test".to_string())
    .fonts(fonts)
    .build()
}

/// A page of lines for each of `sections`, each starting on a page of its own.
fn sections(head: &str, sections: &[(&str, usize)], tail: &str) -> String {
  let sections: String = sections
    .iter()
    .map(|&(title, lines)| {
      let lines: String = (1..=lines)
        .map(|line| format!("<div>{title}, line {line}</div>"))
        .collect();

      format!(
        r#"<div id="{title}" style="display: flex; flex-direction: column; break-before: page;"><h2 style="font-size: 18px; margin: 0;">{title}</h2>{lines}</div>"#
      )
    })
    .collect();

  format!(
    r#"<div style="display: flex; flex-direction: column; width: 100%; font-size: 14px; color: #141414;">{head}{sections}{tail}</div>"#
  )
}

fn page_count(pdf: &[u8]) -> usize {
  let text = String::from_utf8_lossy(pdf);
  let start = text.find("/Type/Pages/Count ").expect("a page tree") + "/Type/Pages/Count ".len();

  text[start..]
    .chars()
    .take_while(char::is_ascii_digit)
    .collect::<String>()
    .parse()
    .expect("a page count")
}

/// How the document renders, with the pages the layout had laid out when the first was drawn.
struct Rendered {
  pdf: Vec<u8>,
  pages: usize,
  laid_at_first_page: usize,
}

fn render_drawing_as_settled(options: PdfOptions<'_>) -> Rendered {
  let laid_at_first_page = Cell::new(usize::MAX);
  let pdf = render_settling(options, &|layout, reach| {
    let settled = layout.settled_pages(reach);

    if settled > 0 {
      laid_at_first_page.set(laid_at_first_page.get().min(layout.page_count()));
    }
    settled
  })
  .expect("the document renders");

  Rendered {
    pages: page_count(&pdf),
    pdf,
    // A document that lays out first never asks the hook.
    laid_at_first_page: laid_at_first_page.get(),
  }
}

/// The document with the layout run to its end before the first page is drawn.
fn render_after_layout(options: PdfOptions<'_>) -> Vec<u8> {
  render_settling(options, &|layout: &PagedLayout<'_>, _| {
    if layout.is_finished() {
      layout.page_count()
    } else {
      0
    }
  })
  .expect("the document renders")
}

/// Renders the document both ways and checks the bytes, and returns how the first went.
fn assert_draws_like_after_layout(
  name: &str,
  configure: impl Fn(&Fonts) -> PdfOptions<'_>,
) -> Rendered {
  let fonts = fonts();
  let rendered = render_drawing_as_settled(configure(&fonts));
  let whole = render_after_layout(configure(&fonts));

  assert!(
    rendered.pdf == whole,
    "{name} draws differently as it is laid out"
  );
  assert_eq!(
    rendered.pdf,
    render(configure(&fonts)).expect("the document renders"),
    "{name} draws differently through render"
  );
  rendered
}

fn plain_document() -> String {
  sections(
    "",
    &[("a", 4), ("b", 4), ("c", 4), ("d", 4), ("e", 4), ("f", 4)],
    "",
  )
}

#[test]
fn a_plain_long_document_draws_its_first_page_before_layout_ends() {
  let source = plain_document();
  let rendered =
    assert_draws_like_after_layout("a plain document", |fonts| options(&source, fonts));

  assert_eq!(rendered.pages, 6);
  assert!(
    rendered.laid_at_first_page < rendered.pages,
    "{} of {} pages were laid out",
    rendered.laid_at_first_page,
    rendered.pages
  );
}

#[test]
fn headings_and_ids_are_summarized_as_pages_draw() {
  let source = sections(
    r#"<h1 id="top" style="font-size: 22px; margin: 0;">Report</h1>"#,
    &[("a", 4), ("b", 4), ("c", 4), ("d", 4), ("e", 4)],
    "",
  );
  let rendered = assert_draws_like_after_layout("headings and ids", |fonts| {
    let mut options = options(&source, fonts);

    options.outline = true;
    options.tagged = Tagging::On;
    options
  });

  assert!(rendered.laid_at_first_page < rendered.pages);
}

#[test]
fn dropped_pages_draw_nothing_and_still_count() {
  let source = plain_document();
  let rendered = assert_draws_like_after_layout("page ranges", |fonts| {
    let mut options = options(&source, fonts);

    options.page_ranges = Some(vec![
      PageRange::single(1),
      PageRange::single(3),
      PageRange {
        from: Some(5),
        to: None,
      },
    ]);
    options
  });

  assert_eq!(rendered.pages, 4);
  assert!(rendered.laid_at_first_page < 6);
}

#[test]
fn a_total_pages_band_lays_out_before_the_first_page_is_drawn() {
  let source = plain_document();
  let rendered = assert_draws_like_after_layout("a total pages band", |fonts| {
    let mut options = options(&source, fonts);

    options.header = Some(
      from_html(
        r#"<div>Page <span class="pageNumber"></span> of <span class="totalPages"></span></div>"#,
        FromHtmlOptions::default(),
      )
      .expect("parse the header"),
    );
    options
  });

  assert_eq!(rendered.laid_at_first_page, usize::MAX);
}

#[test]
fn a_link_to_a_later_page_lays_out_before_the_first_page_is_drawn() {
  let source = sections(
    r##"<a href="#f" style="display: flex;">To the end</a>"##,
    &[("a", 4), ("b", 4), ("c", 4), ("d", 4), ("e", 4), ("f", 4)],
    "",
  );
  let rendered = assert_draws_like_after_layout("a forward link", |fonts| options(&source, fonts));

  assert_eq!(rendered.laid_at_first_page, usize::MAX);
}

#[test]
fn content_counters_lay_out_before_the_first_page_is_drawn() {
  let source = sections(
    "",
    &[("a", 4), ("b", 4), ("c", 4)],
    r#"<div>Page <span class="pageNumber"></span> of <span class="totalPages"></span></div>"#,
  );
  let rendered =
    assert_draws_like_after_layout("content counters", |fonts| options(&source, fonts));

  assert_eq!(rendered.laid_at_first_page, usize::MAX);
}

#[test]
fn a_transform_that_paints_pages_back_holds_every_page_until_layout_ends() {
  let source = sections("", &[("a", 3), ("b", 3), ("c", 3), ("d", 3)], "").replace(
    "d, line 3</div>",
    r#"d, line 3</div><div style="width: 120px; height: 30px; background-color: rgb(200, 40, 40); transform: translateY(-576px);"></div>"#,
  );
  let rendered = assert_draws_like_after_layout("a transform", |fonts| options(&source, fonts));

  assert_eq!(rendered.laid_at_first_page, rendered.pages);
}

#[test]
fn a_child_held_to_the_bottom_of_a_relative_root_holds_every_page_until_layout_ends() {
  let lines: String = (1..=40)
    .map(|line| format!("<div>Line {line}</div>"))
    .collect();
  let source = format!(
    r#"<div style="position: relative; display: flex; flex-direction: column; width: 100%; font-size: 14px;">{lines}<div style="position: absolute; bottom: 0; height: 24px;">Held</div></div>"#
  );
  let rendered = assert_draws_like_after_layout("a held child", |fonts| options(&source, fonts));

  assert_eq!(rendered.laid_at_first_page, rendered.pages);
}
