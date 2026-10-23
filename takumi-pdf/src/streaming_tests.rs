//! A page drawn as soon as no later page paints on it comes out as it does when the layout runs
//! to its end first.

use std::cell::Cell;

use takumi_core::{Fonts, layout::paged_layout::PagedLayout};
use takumi_html::{FromHtmlOptions, from_html};

use crate::{
  PageMargin, PageMargins, PageOptions, PageRange, PdfError, PdfOptions, Tagging, render,
  render_settling, test_support::fonts,
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

/// How the document renders, with the pages the last layout had laid out when the first was
/// drawn.
struct Rendered {
  pdf: Vec<u8>,
  pages: usize,
  /// How many times the content was laid out, the last being the one that draws.
  layouts: usize,
  laid_at_first_page: usize,
}

fn render_drawing_as_settled(options: PdfOptions<'_>) -> Rendered {
  let laid_at_first_page = Cell::new(usize::MAX);
  let layouts = Cell::new(0);
  let pdf = render_settling(options, &|layout| {
    let settled = layout.settled_pages();

    if layout.page_count() == 1 {
      layouts.set(layouts.get() + 1);
      laid_at_first_page.set(usize::MAX);
    }
    if settled > 0 {
      laid_at_first_page.set(laid_at_first_page.get().min(layout.page_count()));
    }
    settled
  })
  .expect("the document renders");

  Rendered {
    pages: page_count(&pdf),
    pdf,
    layouts: layouts.get(),
    laid_at_first_page: laid_at_first_page.get(),
  }
}

/// The document with the layout run to its end before the first page is drawn.
fn render_after_layout(options: PdfOptions<'_>) -> Vec<u8> {
  render_settling(options, &|layout: &PagedLayout<'_>| {
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
fn a_total_pages_band_draws_its_first_page_before_the_second_pass_ends() {
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

  assert_eq!(rendered.pages, 6);
  assert_eq!(rendered.layouts, 2, "one pass counts, one draws");
  assert!(
    rendered.laid_at_first_page < rendered.pages,
    "{} of {} pages were laid out",
    rendered.laid_at_first_page,
    rendered.pages
  );
}

#[test]
fn a_page_number_band_draws_its_first_page_before_layout_ends() {
  let source = plain_document();
  let rendered = assert_draws_like_after_layout("a page number band", |fonts| {
    let mut options = options(&source, fonts);

    options.footer = Some(
      from_html(
        r#"<div style="text-align: right;">Page <span class="pageNumber"></span></div>"#,
        FromHtmlOptions::default(),
      )
      .expect("parse the footer"),
    );
    options
  });

  assert_eq!(rendered.layouts, 2);
  assert!(rendered.laid_at_first_page < rendered.pages);
}

#[test]
fn a_band_that_grows_with_the_page_count_settles_before_the_pages_draw() {
  let lines: String = (0..110)
    .map(|line| format!("<div>line {line}</div>"))
    .collect();
  let source = format!(r#"<div style="font-size: 14px">{lines}</div>"#);
  let rendered = assert_draws_like_after_layout("a band that wraps at ten pages", |fonts| {
    let mut options = options(&source, fonts);

    options.page = Some(PageOptions {
      margin: PageMargins {
        bottom: PageMargin::Auto,
        ..PageMargins::uniform(24.0)
      },
      ..PAGE
    });
    options.footer = Some(
      from_html(
        r#"<div style="width: 80px; font-size: 14px;">Page <span class="pageNumber"></span> of <span class="totalPages"></span></div>"#,
        FromHtmlOptions::default(),
      )
      .expect("parse the footer"),
    );
    options
  });

  assert!(rendered.pages >= 10, "{} pages", rendered.pages);
  assert_eq!(rendered.layouts, 3, "two passes count, one draws");
  assert!(rendered.laid_at_first_page < rendered.pages);
}

const ALL_SECTIONS: [(&str, usize); 6] =
  [("a", 4), ("b", 4), ("c", 4), ("d", 4), ("e", 4), ("f", 4)];

#[test]
fn a_link_to_a_later_page_draws_its_first_page_before_layout_ends() {
  let source = sections(
    r##"<a href="#f" style="display: flex;">To the end</a><a href="#missing" style="display: flex;">Nowhere</a>"##,
    &ALL_SECTIONS,
    "",
  );
  let rendered = assert_draws_like_after_layout("a forward link", |fonts| options(&source, fonts));

  assert_eq!(rendered.layouts, 2, "one pass finds the anchors, one draws");
  assert!(rendered.laid_at_first_page < rendered.pages);
}

#[test]
fn a_link_to_an_earlier_page_draws_its_first_page_before_layout_ends() {
  let source = sections(
    "",
    &ALL_SECTIONS,
    r##"<a href="#a" style="display: flex;">Back to the start</a>"##,
  );
  let rendered = assert_draws_like_after_layout("a backward link", |fonts| options(&source, fonts));

  assert_eq!(rendered.layouts, 2);
  assert!(rendered.laid_at_first_page < rendered.pages);
}

#[test]
fn a_link_to_a_dropped_page_loses_its_annotation() {
  let source = sections(
    r##"<a href="#f" style="display: flex;">To the end</a><a href="#a" style="display: flex;">To the start</a>"##,
    &ALL_SECTIONS,
    "",
  );
  let rendered = assert_draws_like_after_layout("a link to a dropped page", |fonts| {
    let mut options = options(&source, fonts);

    options.page_ranges = Some(vec![
      PageRange {
        from: None,
        to: Some(3),
      },
      PageRange::single(5),
    ]);
    options
  });

  assert_eq!(rendered.pages, 4);
  assert_eq!(
    String::from_utf8_lossy(&rendered.pdf)
      .matches("/Subtype/Link")
      .count(),
    1,
    "only the link to the start keeps its annotation"
  );
  assert!(rendered.laid_at_first_page < 6);
}

#[test]
fn a_tagged_document_with_an_outline_and_links_draws_before_layout_ends() {
  let source = sections(
    r##"<h1 id="top" style="font-size: 22px; margin: 0;">Report</h1><a href="#e" style="display: flex;">To the fifth section</a><a href="https://example.com/report" style="display: flex;">Online</a>"##,
    &ALL_SECTIONS,
    r##"<a href="#top" style="display: flex;">Back to the top</a>"##,
  );
  let rendered = assert_draws_like_after_layout("a tagged document with links", |fonts| {
    let mut options = options(&source, fonts);

    options.outline = true;
    options.tagged = Tagging::On;
    options
  });

  assert_eq!(rendered.layouts, 2);
  assert!(rendered.laid_at_first_page < rendered.pages);
  assert!(String::from_utf8_lossy(&rendered.pdf).contains("/StructTreeRoot"));
}

#[test]
fn a_link_beside_a_total_pages_band_and_target_counters_draws_before_layout_ends() {
  let source = sections(
    r##"<a href="#d" style="display: flex; column-gap: 4px;">Section d on page <span class="targetPageNumber"></span></a>"##,
    &ALL_SECTIONS,
    r#"<div>Page <span class="pageNumber"></span> of <span class="totalPages"></span></div>"#,
  );
  let rendered = assert_draws_like_after_layout("links and counters", |fonts| {
    let mut options = options(&source, fonts);

    options.footer = Some(
      from_html(
        r#"<div>Page <span class="pageNumber"></span> of <span class="totalPages"></span></div>"#,
        FromHtmlOptions::default(),
      )
      .expect("parse the footer"),
    );
    options
  });

  assert_eq!(rendered.layouts, 3, "two passes number, one draws");
  assert!(rendered.laid_at_first_page < rendered.pages);
}

#[test]
fn content_counters_draw_their_first_page_before_the_last_layout_ends() {
  let source = sections(
    "",
    &[("a", 4), ("b", 4), ("c", 4)],
    r#"<div>Page <span class="pageNumber"></span> of <span class="totalPages"></span></div>"#,
  );
  let rendered =
    assert_draws_like_after_layout("content counters", |fonts| options(&source, fonts));

  assert_eq!(rendered.pages, 3);
  assert_eq!(rendered.layouts, 3, "two passes number, one draws");
  assert!(rendered.laid_at_first_page < rendered.pages);
}

/// `lines` lines, then a tail whose width fits the page count in roman numerals only up to a
/// point, so that numbering the page count can move it.
fn numbered_tail(lines: usize, tail_width: usize) -> String {
  let lines: String = (1..=lines)
    .map(|line| format!("<div>line {line}</div>"))
    .collect();

  format!(
    r#"<div style="width: 100%; font-size: 14px;">{lines}<div style="width: {tail_width}px;">Total <span class="totalPages lower-roman"></span></div></div>"#
  )
}

#[test]
fn counters_that_move_the_page_count_are_numbered_again_before_the_pages_draw() {
  let source = numbered_tail(83, 40);
  let rendered =
    assert_draws_like_after_layout("counters that move", |fonts| options(&source, fonts));

  assert_eq!(rendered.layouts, 4, "three passes number, one draws");
  assert!(rendered.laid_at_first_page < rendered.pages);
}

#[test]
fn counters_that_never_settle_lay_out_to_the_end() {
  let source = numbered_tail(95, 46);
  let rendered = assert_draws_like_after_layout("counters that never settle", |fonts| {
    options(&source, fonts)
  });

  assert_eq!(rendered.layouts, 3, "the last layout never asks the hook");
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

#[test]
fn a_long_document_of_every_kind_of_box_draws_like_after_layout() {
  let rows: String = (0..110)
    .map(|row| format!("<tr><td>row {row}</td><td>cell</td></tr>"))
    .collect();
  let blocks: String = (0..120)
    .map(|index| {
      format!(
        r#"<section><h2 style="margin: 0">Section {index}</h2><p style="margin: 0">Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor</p><div style="position: relative; height: 20px"><span style="position: absolute; right: 0; top: 0">badge {index}</span></div><div style="display: flex"><div>left {index}</div><div>right</div></div><div style="float: left; width: 40px">float</div><span style="display: inline-block; width: 60px">inline {index}</span></section>"#
      )
    })
    .collect();
  let source = format!(
    r#"<div style="font-size: 14px"><table><thead><tr><th>Name</th><th>Value</th></tr></thead><tfoot><tr><td>Sum</td><td>0</td></tr></tfoot><tbody>{rows}</tbody></table>{blocks}<p>The end</p></div>"#
  );
  let rendered = assert_draws_like_after_layout("a long document", |fonts| options(&source, fonts));

  assert!(
    rendered.pages > 60,
    "the document has {} pages",
    rendered.pages
  );
  assert!(
    rendered.laid_at_first_page < rendered.pages,
    "{} of {} pages were laid out",
    rendered.laid_at_first_page,
    rendered.pages
  );
}

#[test]
fn a_long_document_with_a_fixed_box_draws_like_after_layout() {
  let lines: String = (0..200)
    .map(|line| format!("<div>line {line}</div>"))
    .collect();
  let source = format!(
    r#"<div style="font-size: 14px"><div style="position: fixed; top: 0; left: 0"><div>Running head</div></div>{lines}</div>"#
  );
  let rendered = assert_draws_like_after_layout("a fixed box", |fonts| options(&source, fonts));

  assert!(
    rendered.pages > 10,
    "the document has {} pages",
    rendered.pages
  );
}

#[test]
fn an_absolute_box_found_on_a_later_page_holds_the_pages_of_its_containing_block_until_then() {
  let lines = |range: std::ops::RangeInclusive<usize>| -> String {
    range
      .map(|line| format!("<div>Line {line}</div>"))
      .collect()
  };
  let source = format!(
    r#"<div style="position: relative; width: 100%; font-size: 14px;">{}<div style="position: absolute; top: 10px; left: 0; width: 60px; height: 20px; background-color: rgb(200, 40, 40);"></div>{}</div>"#,
    lines(1..=40),
    lines(41..=50)
  );
  let rendered = assert_draws_like_after_layout("an absolute box of a long block", |fonts| {
    options(&source, fonts)
  });
  let fonts = fonts();
  let next_page_laid = render_settling(options(&source, &fonts), &|layout| {
    if layout.is_finished() {
      layout.page_count()
    } else {
      layout.page_count() - 1
    }
  })
  .expect("the document renders");

  assert!(rendered.pages > 3);
  assert_ne!(
    next_page_laid, rendered.pdf,
    "the box paints on a page before the one that lays it out"
  );
  assert!(rendered.laid_at_first_page > 3);
}

#[test]
fn an_invalid_page_range_fails_before_any_page_is_laid_out() {
  let source = plain_document();
  let fonts = fonts();

  for range in [
    PageRange::single(0),
    PageRange {
      from: Some(3),
      to: Some(2),
    },
  ] {
    let asked = Cell::new(0);
    let mut options = options(&source, &fonts);

    options.page_ranges = Some(vec![range]);

    let result = render_settling(options, &|layout| {
      asked.set(asked.get() + 1);
      layout.settled_pages()
    });

    assert!(
      matches!(result, Err(PdfError::InvalidPageRange(_))),
      "{result:?}"
    );
    assert_eq!(asked.get(), 0);
  }
}
