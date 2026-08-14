//! PDF byte-golden fixtures.
//!
//! Every case renders twice (guarding against nondeterministic output) and
//! writes the result to `tests/fixtures-generated/<name>.pdf`. The goldens are
//! committed; CI's dirty-tree check catches drift, so a changed .pdf in `git
//! diff` is a real rendering change to review.

use std::{
  collections::{HashMap, HashSet},
  fs,
  io::Read,
  path::Path,
  sync::Arc,
};

use flate2::read::ZlibDecoder;
use takumi_core::{
  Fonts,
  layout::node::{ImageData, ImageSourceInput, Node, RgbaImage},
  resources::{
    font::{FontOverride, FontResource},
    image::{ImageCacheMode, ImageSource, ResourceCache},
    image_buffer::ImageBuffer,
  },
  style::{
    BreakBetween, Color, ColorInput, Display, FlexDirection, FontSize, Length::*, LineHeight,
    ObjectFit, Style, StyleDeclaration,
  },
  viewport::Viewport,
};
use takumi_html::{FromHtmlOptions, from_html};
use takumi_pdf::{
  Attachment, AttachmentRelationship, MeasureOptions, PageMargins, PageOptions, PdfDate, PdfError,
  PdfMetadata, PdfOptions, PdfStandard, Tagging, XmpProperty, XmpSchema, measure, render,
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

fn html_fixture(name: &str) -> Node {
  let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("fixtures")
    .join(name);
  let source = fs::read_to_string(path).expect("read html fixture");

  from_html(&source, FromHtmlOptions::default()).expect("parse html fixture")
}

/// Renders the case twice, asserts determinism, writes the golden, and
/// returns the bytes.
fn run_pdf_fixture(name: &str, build: impl Fn(&Fonts) -> PdfOptions<'_>) -> Vec<u8> {
  run_pdf_fixture_with(name, &fonts(), build)
}

fn run_pdf_fixture_with(
  name: &str,
  fonts: &Fonts,
  build: impl Fn(&Fonts) -> PdfOptions<'_>,
) -> Vec<u8> {
  let first = render(build(fonts)).expect("render pdf fixture");
  let second = render(build(fonts)).expect("render pdf fixture again");

  assert_eq!(first, second, "nondeterministic pdf output for {name}");
  assert!(first.starts_with(b"%PDF-"), "not a pdf: {name}");

  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures-generated");

  fs::create_dir_all(&dir).expect("create golden directory");
  fs::write(dir.join(format!("{name}.pdf")), &first).expect("write pdf golden");
  first
}

fn text(content: &str, size: f32) -> Node {
  Node::text(content.to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        20, 20, 20, 255,
      ]))))
      .with(StyleDeclaration::font_size(FontSize::Length(Px(size)))),
  )
}

fn column(children: Vec<Node>) -> Node {
  Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::width(Percentage(100.0))),
  )
}

#[test]
fn text_basic() {
  run_pdf_fixture("text-basic", |fonts| {
    PdfOptions::builder()
      .node(
        Node::container([text("Hello PDF from Takumi", 32.0)]).with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::width(Percentage(100.0)))
            .with(StyleDeclaration::height(Percentage(100.0)))
            .with(StyleDeclaration::background_color(ColorInput::Value(
              Color([235, 244, 255, 255]),
            ))),
        ),
      )
      .viewport(Viewport::new((600, 300)))
      .fonts(fonts)
      .build()
  });
}

#[test]
fn text_ligatures() {
  run_pdf_fixture("text-ligatures", |fonts| {
    PdfOptions::builder()
      .node(
        Node::container([text("Difficult office traffic affix", 24.0)]).with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::width(Percentage(100.0)))
            .with(StyleDeclaration::height(Percentage(100.0))),
        ),
      )
      .viewport(Viewport::new((600, 100)))
      .fonts(fonts)
      .build()
  });
}

#[test]
fn paged_lines() {
  run_pdf_fixture("paged-lines", |fonts| {
    let lines = (1..=40)
      .map(|i| text(&format!("Line {i} of the paginated report body"), 16.0))
      .collect();

    PdfOptions::builder()
      .node(column(lines))
      .page(PageOptions {
        width: 400.0,
        height: 300.0,
        margin: PageMargins::uniform(24.0),
      })
      .fonts(fonts)
      .build()
  });
}

#[test]
fn paged_footer_counters() {
  run_pdf_fixture("paged-footer", |fonts| {
    let rows = (1..=40).map(|i| text(&format!("Row {i}"), 16.0)).collect();

    PdfOptions::builder()
      .node(column(rows))
      .page(PageOptions {
        width: 400.0,
        height: 300.0,
        margin: PageMargins::uniform(24.0),
      })
      .footer(
        from_html(
          r#"<div style="display: flex; column-gap: 3px; font-size: 12px; color: #141414;">
            Page <span class="pageNumber"></span> of <span class="totalPages"></span>,
            page <span class="pageNumber muted trad-chinese-informal"></span> in Chinese,
            <span class="pageNumber lower-roman"></span> in roman
          </div>"#,
          FromHtmlOptions::default(),
        )
        .expect("parse footer fixture"),
      )
      .fonts(fonts)
      .build()
  });
}

/// Guards `widows` / `orphans`: the default 2/2 minimums must move a line
/// across the page cut that minimums of 1/1 leave in place.
#[test]
fn paged_widow_orphan_control() {
  use takumi_core::style::MinLines;

  let fonts = fonts();
  let document = |relaxed: bool| {
    let rows: Vec<Node> = (1..=12).map(|i| text(&format!("Row {i}"), 16.0)).collect();
    let paragraph = Node::text(
      "The closing paragraph runs long enough to wrap into several lines \
       and straddle the page boundary, which is exactly where the widow \
       and orphan minimums earn their keep in a paginated report."
        .to_string(),
    )
    .with_style(
      Style::default()
        .with(StyleDeclaration::color(ColorInput::Value(Color([
          20, 20, 20, 255,
        ]))))
        .with(StyleDeclaration::font_size(FontSize::Length(Px(16.0))))
        // Air between the line bands, so the widow move is distinguishable
        // from the atom pass cascading through touching lines.
        .with(StyleDeclaration::line_height(LineHeight::Unitless(1.8))),
    );
    let mut children = rows;

    children.push(paragraph);
    let root = column(children);

    if relaxed {
      // Inherited minimums of one restore the unconstrained cut.
      root.with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::flex_direction(FlexDirection::Column))
          .with(StyleDeclaration::width(Percentage(100.0)))
          .with(StyleDeclaration::widows(MinLines::from(1)))
          .with(StyleDeclaration::orphans(MinLines::from(1))),
      )
    } else {
      root
    }
  };
  fn page() -> PageOptions {
    PageOptions {
      width: 400.0,
      height: 300.0,
      margin: PageMargins::uniform(20.0),
    }
  }
  let strict = run_pdf_fixture_with("paged-widows-orphans", &fonts, |fonts| {
    PdfOptions::builder()
      .node(document(false))
      .page(page())
      .fonts(fonts)
      .build()
  });
  let relaxed = render(
    PdfOptions::builder()
      .node(document(true))
      .page(page())
      .fonts(&fonts)
      .build(),
  )
  .expect("render relaxed variant");

  assert_ne!(
    strict, relaxed,
    "default widow/orphan minimums did not move any line across the cut"
  );
}

#[test]
fn paged_breaks() {
  run_pdf_fixture("paged-breaks", |fonts| {
    let section = |title: &str| {
      column(
        (1..=3)
          .map(|i| text(&format!("{title} row {i}"), 14.0))
          .collect(),
      )
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::flex_direction(FlexDirection::Column))
          .with(StyleDeclaration::break_before(BreakBetween::Page)),
      )
    };

    PdfOptions::builder()
      .node(column(vec![section("Alpha"), section("Beta")]))
      .page(PageOptions {
        width: 400.0,
        height: 400.0,
        margin: PageMargins::uniform(24.0),
      })
      .fonts(fonts)
      .build()
  });
}

/// A table of contents whose entries carry `targetPageNumber` hooks. Each
/// section is forced onto its own page, so the entries have to read 2, 3 and 4.
fn toc_document(cells: [&str; 3]) -> String {
  let entry = |id: &str, title: &str, cell: &str| {
    format!(
      r##"<a href="#{id}" style="display: flex; column-gap: 4px;"><span>{title}</span>{cell}</a>"##
    )
  };
  let section = |id: &str, title: &str| {
    format!(
      r##"<div id="{id}" style="display: flex; flex-direction: column; break-before: page; font-size: 18px;">{title}</div>"##
    )
  };

  format!(
    r##"<div style="display: flex; flex-direction: column; width: 100%; font-size: 14px; color: #141414;">
      <div style="display: flex; flex-direction: column;">{}{}{}</div>
      {}{}{}
    </div>"##,
    entry("alpha", "Alpha", cells[0]),
    entry("beta", "Beta", cells[1]),
    entry("gamma", "Gamma", cells[2]),
    section("alpha", "Alpha"),
    section("beta", "Beta"),
    section("gamma", "Gamma"),
  )
}

fn toc_options<'f>(source: &str, fonts: &'f Fonts) -> PdfOptions<'f> {
  PdfOptions::builder()
    .node(from_html(source, FromHtmlOptions::default()).expect("parse toc fixture"))
    .page(PageOptions {
      width: 320.0,
      height: 240.0,
      margin: PageMargins::uniform(24.0),
    })
    .fonts(fonts)
    .build()
}

#[test]
fn paged_target_counters() {
  let hooked = toc_document([
    r#"<span class="targetPageNumber"></span>"#,
    r#"<span class="targetPageNumber"></span>"#,
    r#"<span class="targetPageNumber upper-roman"></span>"#,
  ]);
  let pdf = run_pdf_fixture("paged-target-counters", |fonts| toc_options(&hooked, fonts));
  // Numbering the entries by hand has to render the same document, which pins
  // the resolved pages to 2, 3 and 4 without decoding a subset font.
  let numbered = toc_document(["<span>2</span>", "<span>3</span>", "<span>IV</span>"]);
  let expected = render(toc_options(&numbered, &fonts())).expect("render numbered toc");

  assert_eq!(
    pdf, expected,
    "target counters did not resolve to 2, 3 and 4"
  );
}

/// Entries whose title all but fills the row, so a number wraps each of them
/// onto a second line. Filling the counters therefore doubles the contents
/// page, which pushes every section one page further along and renumbers the
/// counters that caused it.
fn wrapping_toc_document(cells: [&str; 8]) -> String {
  let entry = |index: usize, cell: &str| {
    format!(
      r##"<a href="#s{index}" style="display: flex; flex-wrap: wrap; width: 100%;"><span style="width: 268px;">Section {index}</span>{cell}</a>"##
    )
  };
  let section = |index: usize| {
    format!(
      r##"<div id="s{index}" style="display: flex; break-before: page; font-size: 18px;">Section {index}</div>"##
    )
  };
  let entries: String = cells
    .iter()
    .enumerate()
    .map(|(index, cell)| entry(index + 1, cell))
    .collect();
  let sections: String = (1..=cells.len()).map(section).collect();

  format!(
    r##"<div style="display: flex; flex-direction: column; width: 100%; font-size: 14px; color: #141414;">{entries}{sections}</div>"##
  )
}

#[test]
fn target_counters_settle_after_they_move_their_own_page() {
  let hooked = wrapping_toc_document([r#"<span class="targetPageNumber"></span>"#; 8]);
  let pdf = render(toc_options(&hooked, &fonts())).expect("render wrapping toc");
  // The first pass numbers a contents page one line per entry, which puts the
  // sections on pages 2 to 9. Those numbers wrap the entries, and the second
  // pass has to renumber them from the taller contents page.
  let numbered = wrapping_toc_document([
    "<span>3</span>",
    "<span>4</span>",
    "<span>5</span>",
    "<span>6</span>",
    "<span>7</span>",
    "<span>8</span>",
    "<span>9</span>",
    "<span>10</span>",
  ]);
  let expected = render(toc_options(&numbered, &fonts())).expect("render numbered wrapping toc");

  assert_eq!(pdf, expected, "target counters did not settle after rewrap");
}

#[test]
fn target_counter_in_a_band_drops_its_placeholder() {
  let band = |cell: &str| {
    format!(
      r##"<div style="display: flex; column-gap: 3px; font-size: 12px; color: #141414;">Page <span class="pageNumber"></span>, section {cell}</div>"##
    )
  };
  let body = toc_document(["", "", ""]);
  let banded = |footer: String, fonts: &Fonts| {
    render(
      PdfOptions::builder()
        .node(from_html(&body, FromHtmlOptions::default()).expect("parse toc"))
        .page(PageOptions {
          width: 320.0,
          height: 240.0,
          margin: PageMargins::uniform(24.0),
        })
        .footer(from_html(&footer, FromHtmlOptions::default()).expect("parse band"))
        .fonts(fonts)
        .build(),
    )
  };
  let fonts = fonts();
  let hooked = banded(
    band(r##"<a href="#alpha"><span class="targetPageNumber">99</span></a>"##),
    &fonts,
  )
  .expect("render band with a target hook");
  let empty = banded(band(r##"<a href="#alpha"><span></span></a>"##), &fonts)
    .expect("render band without one");

  assert_eq!(hooked, empty, "a band hook kept its placeholder");
}

#[test]
fn target_counter_without_a_target_renders_empty() {
  let dangling = toc_document([r#"<span class="targetPageNumber"></span>"#; 3])
    .replace("#alpha", "#missing")
    .replace("#beta", "#missing-too");
  let pdf = render(toc_options(&dangling, &fonts())).expect("render dangling toc");
  let blank = toc_document(["<span></span>", "<span></span>", "<span>4</span>"])
    .replace("#alpha", "#missing")
    .replace("#beta", "#missing-too");
  let expected = render(toc_options(&blank, &fonts())).expect("render blank toc");

  assert_eq!(
    pdf, expected,
    "a fragment naming no element must render empty"
  );
}

fn checker_pixels() -> Vec<u8> {
  let mut pixels = Vec::with_capacity(8 * 8 * 4);

  for row in 0..8u32 {
    for col in 0..8u32 {
      let on = (row / 2 + col / 2) % 2 == 0;

      pixels.extend_from_slice(if on {
        &[220, 60, 60, 255]
      } else {
        &[60, 60, 220, 255]
      });
    }
  }
  pixels
}

/// One image per `object-fit` value in a non-square box, so every sizing
/// branch (stretch, letterbox, crop, shrink cap, intrinsic) is on the page.
#[test]
fn image_object_fit() {
  run_pdf_fixture("image-object-fit", |fonts| {
    let fits = [
      ObjectFit::Fill,
      ObjectFit::Contain,
      ObjectFit::Cover,
      ObjectFit::ScaleDown,
      ObjectFit::None,
    ];
    let images: Vec<Node> = fits
      .iter()
      .map(|fit| {
        Node::image(ImageData {
          src: ImageSourceInput::Rgba(
            RgbaImage::new(checker_pixels(), 8, 8, false).expect("rgba image"),
          ),
          width: Some(72.0),
          height: Some(48.0),
        })
        .with_style(
          Style::default()
            .with(StyleDeclaration::object_fit(*fit))
            .with(StyleDeclaration::margin_left(Px(12.0))),
        )
      })
      .collect();

    PdfOptions::builder()
      .node(
        Node::container(images).with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::width(Percentage(100.0)))
            .with(StyleDeclaration::height(Percentage(100.0)))
            .with(StyleDeclaration::padding_top(Px(16.0))),
        ),
      )
      .viewport(Viewport::new((460, 90)))
      .fonts(fonts)
      .build()
  });
}

/// An SVG logo (gradient circle + stroked check) embeds as vector paths and
/// shading patterns, never as a rasterized image XObject.
#[test]
fn svg_vector_image() {
  let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24">
  <defs><linearGradient id="g" x1="0" y1="0" x2="24" y2="24" gradientUnits="userSpaceOnUse">
    <stop offset="0" stop-color="#ff0044"/><stop offset="1" stop-color="#0044ff"/>
  </linearGradient></defs>
  <circle cx="12" cy="12" r="10" fill="url(#g)"/>
  <path d="M6 12 L11 17 L18 7" stroke="#fff" stroke-width="2.5" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
</svg>"##;
  let pdf = run_pdf_fixture("svg-vector-image", |fonts| {
    let logo = Node::image(ImageData {
      src: ImageSourceInput::Buffer(svg.as_bytes().to_vec()),
      width: Some(22.0),
      height: Some(22.0),
    });

    PdfOptions::builder()
      .node(
        Node::container(vec![logo]).with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::padding_top(Px(8.0))),
        ),
      )
      .viewport(Viewport::new((120, 60)))
      .fonts(fonts)
      .build()
  });
  let text = String::from_utf8_lossy(&pdf);

  assert!(
    !text.contains("/Subtype/Image"),
    "svg image fell back to raster"
  );
  assert!(text.contains("/Shading"), "gradient lost its shading");
}

/// Repeat-spread gradients with singular `gradientTransform`s (zero and
/// rank-1) render instead of panicking on the non-invertible transform.
#[test]
fn svg_singular_gradient_transform() {
  let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24">
  <defs><linearGradient id="g" x1="0" y1="0" x2="8" y2="0" gradientUnits="userSpaceOnUse" gradientTransform="scale(0)" spreadMethod="repeat">
    <stop offset="0" stop-color="#ff0044"/><stop offset="1" stop-color="#0044ff"/>
  </linearGradient>
  <linearGradient id="h" x1="0" y1="0" x2="8" y2="0" gradientUnits="userSpaceOnUse" gradientTransform="matrix(1 1 0 0 0 0)" spreadMethod="repeat">
    <stop offset="0" stop-color="#ff0044"/><stop offset="1" stop-color="#0044ff"/>
  </linearGradient></defs>
  <rect width="24" height="12" fill="url(#g)"/>
  <rect y="12" width="24" height="12" fill="url(#h)"/>
</svg>"##;
  run_pdf_fixture("svg-singular-gradient-transform", |fonts| {
    let image = Node::image(ImageData {
      src: ImageSourceInput::Buffer(svg.as_bytes().to_vec()),
      width: Some(22.0),
      height: Some(22.0),
    });

    PdfOptions::builder()
      .node(Node::container(vec![image]))
      .viewport(Viewport::new((60, 60)))
      .fonts(fonts)
      .build()
  });
}

/// Filters rasterize, luminance masks become soft masks, and pattern fills
/// become tiling patterns.
#[test]
fn svg_fallback_and_masks() {
  let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 24" width="48" height="24">
  <defs>
    <filter id="b"><feGaussianBlur stdDeviation="1"/></filter>
    <mask id="m" maskUnits="userSpaceOnUse" x="16" y="0" width="16" height="24">
      <rect x="16" y="0" width="16" height="24" fill="#888"/>
    </mask>
    <pattern id="p" width="4" height="4" patternUnits="userSpaceOnUse">
      <rect width="2" height="2" fill="#e33"/>
    </pattern>
  </defs>
  <circle cx="8" cy="12" r="6" fill="#3a3" filter="url(#b)"/>
  <rect x="18" y="4" width="12" height="16" fill="#33a" mask="url(#m)"/>
  <rect x="34" y="4" width="12" height="16" fill="url(#p)"/>
</svg>"##;
  let pdf = run_pdf_fixture("svg-fallback-and-masks", |fonts| {
    let image = Node::image(ImageData {
      src: ImageSourceInput::Buffer(svg.as_bytes().to_vec()),
      width: Some(48.0),
      height: Some(24.0),
    });

    PdfOptions::builder()
      .node(
        Node::container(vec![image]).with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::padding_top(Px(8.0))),
        ),
      )
      .viewport(Viewport::new((120, 60)))
      .fonts(fonts)
      .build()
  });
  let text = String::from_utf8_lossy(&pdf);

  assert!(
    text.contains("/Subtype/Image"),
    "filtered subtree should rasterize"
  );
  assert!(text.contains("/SMask"), "mask lost its soft mask");
  assert!(
    text.contains("/PatternType 1"),
    "pattern fill lost its tiling pattern"
  );
}

#[test]
fn box_chrome() {
  run_pdf_fixture("box-chrome", |fonts| {
    let source = r##"<div style="display: flex; width: 100%; height: 100%; padding: 24px; background-color: #ebf0fa;">
      <div style="display: flex; width: 300px; height: 120px; padding: 16px; background-color: #ffffff; border: 3px solid #b42828; border-radius: 16px; opacity: 0.8; font-size: 20px; color: #14143c;">Chrome card</div>
    </div>"##;
    let node = from_html(source, FromHtmlOptions::default()).expect("parse chrome fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((400, 200)))
      .fonts(fonts)
      .build()
  });
}

#[test]
fn gradients() {
  run_pdf_fixture("gradients", |fonts| {
    let source = r##"<div style="display: flex; width: 100%; height: 100%; padding: 20px; column-gap: 20px; background-color: #ffffff;">
      <div style="width: 110px; height: 110px; background-image: linear-gradient(135deg, #ff5f6d, #3a1c71);"></div>
      <div style="width: 110px; height: 110px; background-image: radial-gradient(circle, #fddb92, #4481eb);"></div>
      <div style="width: 110px; height: 110px; background-image: conic-gradient(from 0deg, red, yellow, lime, cyan, blue, magenta, red);"></div>
    </div>"##;
    let node = from_html(source, FromHtmlOptions::default()).expect("parse gradients fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((440, 160)))
      .fonts(fonts)
      .build()
  });
}

/// `mask-image` fades an element out through a soft mask rather than a
/// rasterized copy of it.
#[test]
fn mask_image() {
  let pdf = run_pdf_fixture("mask-image", |fonts| {
    let source = r##"<div style="display: flex; width: 100%; height: 100%; padding: 12px; column-gap: 12px; background-color: #ffffff;">
      <div style="width: 120px; height: 80px; background-color: #1d4ed8; mask-image: linear-gradient(to right, rgba(0,0,0,1), rgba(0,0,0,0));"></div>
      <div style="width: 120px; height: 80px; background-image: linear-gradient(135deg, #ff5f6d, #3a1c71); mask-image: radial-gradient(circle, rgba(0,0,0,1), rgba(0,0,0,0));"></div>
      <div style="width: 120px; height: 80px; background-color: #047857; mask-image: radial-gradient(circle, rgba(0,0,0,1), rgba(0,0,0,0)); mask-size: 30px 20px; mask-repeat: repeat;"></div>
      <div style="width: 120px; height: 80px; background-color: #b91c1c; filter: opacity(0.5); mask-image: linear-gradient(to bottom, rgba(0,0,0,1), rgba(0,0,0,0));"></div>
    </div>"##;
    let node = from_html(source, FromHtmlOptions::default()).expect("parse mask fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((550, 110)))
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  assert!(
    haystack.contains("/SMask"),
    "expected a soft mask in the graphics state"
  );
  assert_eq!(
    haystack.matches("/S/Alpha").count(),
    4,
    "expected one alpha mask per element"
  );
  // The filtered cell's opacity lands once: on its content, not also on the
  // mask that covers it, which would compound to a quarter.
  assert_eq!(
    haystack.matches("/ca 0.5").count(),
    1,
    "expected the element filter to set half opacity exactly once"
  );
  // The tiled mask resolves through the same placement as a background layer.
  assert!(
    haystack.contains("/XStep 30/YStep 20"),
    "expected the mask layer to tile at its mask-size"
  );
}

/// The color half of `filter`: each cell paints the same red, transformed by a
/// different filter, so the fills carry different colors.
#[test]
fn color_filters() {
  let pdf = run_pdf_fixture("color-filters", |fonts| {
    let cell = |filter: &str| {
      format!(
        r##"<div style="width: 70px; height: 70px; background-color: #e11d48; filter: {filter};"></div>"##
      )
    };
    let source = format!(
      r##"<div style="display: flex; width: 100%; height: 100%; padding: 10px; column-gap: 10px; background-color: #ffffff;">
        {}{}{}{}{}{}{}
      </div>"##,
      cell("none"),
      cell("grayscale(1)"),
      cell("sepia(1)"),
      cell("invert(1)"),
      cell("hue-rotate(180deg)"),
      cell("hue-rotate(90deg)"),
      // A filter covers the whole rendered element, shadows included.
      r##"<div style="width: 70px; height: 70px; background-color: #ffffff; box-shadow: 4px 4px 0 0 #e11d48; filter: grayscale(1);"></div>"##,
    );
    let node = from_html(&source, FromHtmlOptions::default()).expect("parse filter fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((520, 100)))
      .fonts(fonts)
      .build()
  });
  let fills = fill_colors(&pdf);
  // A fill line ends with the three color components before the `rg` operator.
  let rounded = |color: &str| {
    let components: Vec<&str> = color.split_whitespace().rev().take(3).collect();

    components
      .into_iter()
      .rev()
      .map(|part| (part.parse::<f32>().unwrap_or_default() * 255.0).round() as u8)
      .collect::<Vec<_>>()
  };
  let colors: Vec<Vec<u8>> = fills.iter().map(|color| rounded(color)).collect();

  // The page background, then one fill per cell.
  assert_eq!(
    colors.len(),
    9,
    "expected one fill per cell, got {colors:?}"
  );
  assert_eq!(colors[1], vec![225, 29, 72], "unfiltered #e11d48");
  // Rec. 709 luma of the source color.
  assert_eq!(colors[2], vec![74, 74, 74], "grayscale(1)");
  assert_eq!(
    colors[4],
    vec![30, 226, 183],
    "invert(1) is 255 minus source"
  );
  assert_ne!(
    colors[6], colors[5],
    "hue-rotate(90deg) differs from 180deg"
  );
  assert_ne!(colors[6], colors[1], "hue-rotate(90deg) changes the color");
  // The shadow of the last cell is grayscaled like the rest of the element.
  assert_eq!(colors[7], vec![74, 74, 74], "shadow follows the filter");
}

/// Collects the `rg` fill colors from the deflated page content streams.
fn fill_colors(pdf: &[u8]) -> Vec<String> {
  let mut colors = Vec::new();
  let mut rest = pdf;

  while let Some(start) = find(rest, b"stream\n") {
    let body = &rest[start + 7..];
    let Some(end) = find(body, b"endstream") else {
      break;
    };
    let mut decoded = Vec::new();

    if ZlibDecoder::new(&body[..end])
      .read_to_end(&mut decoded)
      .is_ok()
    {
      let text = String::from_utf8_lossy(&decoded).into_owned();

      colors.extend(
        text
          .lines()
          .filter_map(|line| line.split_once(" rg").map(|(color, _)| color.to_string())),
      );
    }
    rest = &body[end..];
  }
  colors
}

/// `box-shadow`: a sharp shadow is one exact ring, a blurred one is a stack of
/// bands, and an inset shadow fills the box minus the hole it casts.
#[test]
fn box_shadows() {
  let pdf = run_pdf_fixture("box-shadows", |fonts| {
    let cell = |shadow: &str| {
      format!(
        r##"<div style="width: 90px; height: 90px; margin: 20px; border-radius: 12px; background-color: #ffffff; box-shadow: {shadow};"></div>"##
      )
    };
    let source = format!(
      r##"<div style="display: flex; width: 100%; height: 100%; padding: 8px; background-color: #f4f4f5;">
        {}{}{}{}
      </div>"##,
      cell("6px 6px 0 0 #111827"),
      cell("0 8px 16px rgba(17, 24, 39, 0.45)"),
      cell("inset 0 0 0 8px #111827"),
      // A transparent border must not carry inset shadow paint: CSS draws inset
      // shadows inside the padding box.
      cell("inset 0 0 0 6px rgba(17, 24, 39, 0.4); border: 10px solid transparent"),
    );
    let node = from_html(&source, FromHtmlOptions::default()).expect("parse box shadow fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((450, 150)))
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  // The blurred cell needs partial opacity, and so does the translucent inset
  // one: a band's opacity multiplies the color's alpha rather than replacing it.
  assert!(
    haystack.contains("/ca "),
    "expected a shadow to set fill opacity"
  );
}

/// `clip-path` basic shapes clip the element and its decorations: an inset with
/// a radius, an ellipse, a polygon, and a `path()`.
#[test]
fn clip_path_shapes() {
  let pdf = run_pdf_fixture("clip-path-shapes", |fonts| {
    let cell = |clip: &str| {
      format!(
        r##"<div style="width: 110px; height: 110px; background-image: linear-gradient(135deg, #ff5f6d, #3a1c71); border: 4px solid #111827; clip-path: {clip};"></div>"##
      )
    };
    let source = format!(
      r##"<div style="display: flex; width: 100%; height: 100%; padding: 16px; column-gap: 16px; background-color: #ffffff;">
        {}{}{}{}{}{}
      </div>"##,
      cell("inset(10px 12px round 16px)"),
      cell("ellipse(45px 30px at 55px 55px)"),
      cell("polygon(50% 0%, 100% 100%, 0% 100%)"),
      cell("path('M 10 10 H 100 V 100 H 10 Z')"),
      // A shape with no area hides the element instead of leaving it visible.
      cell("inset(50% 0)"),
      // An even-odd rule leaves the inner ring of a self-overlapping polygon
      // unpainted, and the shape's own rule wins over `clip-rule`.
      cell(
        "polygon(evenodd, 55px 5px, 105px 105px, 5px 105px, 105px 40px, 5px 40px); clip-rule: nonzero",
      ),
    );
    let node = from_html(&source, FromHtmlOptions::default()).expect("parse clip path fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((800, 150)))
      .fonts(fonts)
      .build()
  });

  // Five non-zero shape clips (the sixth is even-odd), plus the rounded-box
  // clip each gradient layer pushes.
  assert_eq!(
    clip_operators(&pdf),
    11,
    "expected one clip per shape, before its decorations"
  );
  assert_eq!(
    even_odd_clip_operators(&pdf),
    1,
    "expected the even-odd shape to clip with W*"
  );
}

/// Counts even-odd clip operators, which end their line with `W*`.
fn even_odd_clip_operators(pdf: &[u8]) -> usize {
  content_lines(pdf)
    .filter(|line| line.ends_with(b"W*"))
    .count()
}

/// Counts non-zero clip operators across the page content streams.
fn clip_operators(pdf: &[u8]) -> usize {
  content_lines(pdf)
    .filter(|line| line.ends_with(b"W"))
    .count()
}

/// The lines of every deflated content stream in the document.
fn content_lines(pdf: &[u8]) -> impl Iterator<Item = Vec<u8>> {
  let mut lines = Vec::new();
  let mut rest = pdf;

  while let Some(start) = find(rest, b"stream\n") {
    let body = &rest[start + 7..];
    let Some(end) = find(body, b"endstream") else {
      break;
    };
    let mut decoded = Vec::new();

    if ZlibDecoder::new(&body[..end])
      .read_to_end(&mut decoded)
      .is_ok()
    {
      lines.extend(decoded.split(|byte| *byte == b'\n').map(<[u8]>::to_vec));
    }
    rest = &body[end + "endstream".len()..];
  }
  lines.into_iter()
}

/// The document's text with every deflated stream inflated, so a structure
/// element reads the same whether or not it sits in an object stream.
fn inflated_text(pdf: &[u8]) -> String {
  let mut text = String::from_utf8_lossy(pdf).into_owned();

  for line in content_lines(pdf) {
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&line));
  }

  text
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
  haystack
    .windows(needle.len())
    .position(|window| window == needle)
}

/// CSS `outline`: a ring outside the border box, offset outward, following the
/// border radius, with no effect on layout.
#[test]
fn outlines() {
  let pdf = run_pdf_fixture("outlines", |fonts| {
    let cell = |style: &str| {
      format!(
        r##"<div style="width: 90px; height: 90px; margin: 24px; background-color: #e0e7ff; border-radius: 10px; {style}"></div>"##
      )
    };
    let source = format!(
      r##"<div style="display: flex; width: 100%; height: 100%; padding: 8px; background-color: #ffffff;">
        {}{}{}
      </div>"##,
      cell("outline: 4px solid #4338ca;"),
      cell("outline: 4px solid #4338ca; outline-offset: 6px;"),
      // A negative offset pulls the ring inside the border box.
      cell("outline: 3px dashed #b91c1c; outline-offset: -12px;"),
    );
    let node = from_html(&source, FromHtmlOptions::default()).expect("parse outline fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((420, 150)))
      .fonts(fonts)
      .build()
  });
  // A solid ring fills; a dashed one strokes its centerline so the dashes
  // survive. Both carry their outline color into the deflated content streams.
  let content: Vec<Vec<u8>> = content_lines(&pdf).collect();

  for needle in [
    &b"0.2627 0.2196 0.7922 rg"[..],
    &b"0.7255 0.1098 0.1098 RG"[..],
  ] {
    assert!(
      content.iter().any(|line| find(line, needle).is_some()),
      "expected an outline color"
    );
  }
}

/// `background-origin` moves the positioning area, `background-clip` shrinks
/// the painted region, `border-area` paints over the borders, and
/// `background-blend-mode` blends a layer into the one below.
#[test]
fn background_boxes() {
  let pdf = run_pdf_fixture("background-boxes", |fonts| {
    let cell = |style: &str| {
      format!(
        r##"<div style="width: 100px; height: 100px; padding: 14px; border: 8px solid rgba(17, 24, 39, 0.35); background-color: #fef3c7; background-image: linear-gradient(135deg, #ff5f6d, #3a1c71); background-size: 40px 40px; background-repeat: no-repeat; {style}"></div>"##
      )
    };
    let source = format!(
      r##"<div style="display: flex; width: 100%; height: 100%; padding: 10px; column-gap: 10px; background-color: #ffffff;">
        {}{}{}{}{}
      </div>"##,
      cell("background-origin: border-box;"),
      cell("background-origin: content-box;"),
      cell("background-clip: content-box;"),
      cell("background-clip: border-area;"),
      cell("background-blend-mode: multiply;"),
    );
    let node = from_html(&source, FromHtmlOptions::default()).expect("parse background boxes");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((600, 130)))
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  assert!(
    haystack.contains("/Multiply"),
    "expected the blended layer to set its blend mode"
  );
}

/// `text-shadow` draws shifted glyph passes under the text, and
/// `-webkit-text-stroke` strokes the glyph outlines around the fill.
#[test]
fn text_shadow_and_stroke() {
  let pdf = run_pdf_fixture("text-shadow-stroke", |fonts| {
    let source = r##"<div style="display: flex; flex-direction: column; row-gap: 8px; width: 100%; height: 100%; padding: 16px; background-color: #ffffff; font-size: 28px; color: #111827;">
      <div style="text-shadow: 3px 3px 0 #f59e0b;">Sharp shadow</div>
      <div style="text-shadow: 2px 2px 4px rgba(17, 24, 39, 0.5);">Blurred shadow</div>
      <div style="-webkit-text-stroke: 1px #b91c1c; color: #fef3c7;">Stroked text</div>
      <div style="-webkit-text-stroke: 2px rgba(185, 28, 28, 0.25); color: #fef3c7;">Faded stroke</div>
      <div>Plain <span style="-webkit-text-stroke: 1px #2563eb;">span stroke</span></div>
      <div style="-webkit-text-stroke: 3px #b91c1c;">Same words</div>
      <div style="-webkit-text-stroke: 9px #b91c1c;">Same words</div>
      <div style="background-image: linear-gradient(90deg, #ff5f6d, #3a1c71); background-clip: text; color: transparent;">Gradient text</div>
      <div style="background-image: linear-gradient(90deg, #ff5f6d, #3a1c71); background-clip: text; color: transparent; -webkit-text-stroke: 6px transparent;">Ringed text</div>
      <div style="background-image: url('data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 width=%228%22 height=%228%22%3E%3Crect width=%228%22 height=%228%22 fill=%22%2316a34a%22/%3E%3C/svg%3E'); background-clip: text; color: transparent;">Image text</div>
    </div>"##;
    let node = from_html(source, FromHtmlOptions::default()).expect("parse text shadow fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((360, 200)))
      .fonts(fonts)
      .build()
  });
  let content: Vec<Vec<u8>> = content_lines(&pdf).collect();
  let contains = |needle: &[u8]| content.iter().any(|line| find(line, needle).is_some());

  // The amber shadow pass fills before the text color does.
  assert!(
    contains(b"0.9608 0.6196 0.0431 rg"),
    "expected the shadow color fill"
  );
  // The stroke sets a red stroke color (RG) next to the cream fill.
  assert!(
    contains(b"0.7255 0.1098 0.1098 RG"),
    "expected the text stroke color"
  );
  // A span sets the stroke for itself, so the blue outline reaches the file
  // even though the line it sits on carries none.
  assert!(
    contains(b"0.1451 0.3882 0.9216 RG"),
    "expected the inline span stroke color"
  );
  // Two lines of the same words differ only in stroke width, so the shaping
  // cache has to key on it.
  assert!(
    contains(b"3 w") && contains(b"9 w"),
    "one stroke width served both lines"
  );
  // A transparent `-webkit-text-stroke` reveals a background-coloured ring, so
  // the background pass widens the glyph coverage by the stroke width.
  assert!(contains(b"6 w"), "expected the widened clip-text coverage");
  // The clip-text lines fill their glyphs with the shading and the image, both
  // of which reach the glyphs as a pattern rather than a colour.
  assert!(
    contains(b"/Pattern cs"),
    "expected a gradient fill on the clip-text glyphs"
  );
  // An image layer has no paint of its own. Without a pattern carrying it, the
  // layer is dropped and the transparent text comes out invisible, taking the
  // embedded image with it.
  assert!(
    find(&pdf, b"/Subtype /Image").is_some() || find(&pdf, b"/Subtype/Image").is_some(),
    "the image layer left the clip-text glyphs with nothing to fill them"
  );
  // A stroke colour keeps its alpha: the quarter-opaque outline reaches the
  // file as a stroking alpha, not as a solid line. The value is the alpha byte
  // over 255, so it lands a hair off the quarter it was written as.
  assert!(
    stroke_alphas(&inflated_text(&pdf))
      .iter()
      .any(|alpha| (alpha - 0.25).abs() < 0.01),
    "translucent text stroke reached the file opaque"
  );
}

/// `url()` layers: a bitmap background sized by its intrinsic dimensions,
/// tiled, covered, and used as a `mask-image` alpha source.
#[test]
fn url_layers() {
  let pdf = run_pdf_fixture("url-layers", |fonts| {
    let cell = |style: &str| {
      format!(
        r##"<div style="width: 96px; height: 96px; background-color: #f4f4f5; {style}"></div>"##
      )
    };
    let source = format!(
      r##"<div style="display: flex; width: 100%; height: 100%; padding: 12px; column-gap: 12px; background-color: #ffffff;">
        {}{}{}{}
      </div>"##,
      // background-size defaults to auto: the 8x8 checker's intrinsic size.
      cell("background-image: url(checker); background-repeat: no-repeat;"),
      cell("background-image: url(checker); background-size: 24px 24px;"),
      cell("background-image: url(checker); background-size: cover;"),
      cell(
        "background-color: #1d4ed8; mask-image: url(checker); mask-size: 48px 48px; mask-repeat: repeat;"
      ),
    );
    let node = from_html(&source, FromHtmlOptions::default()).expect("parse url layer fixture");
    let buffer =
      ImageBuffer::from_rgba_bytes(checker_pixels(), 8, 8).expect("checker image buffer");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((460, 120)))
      .images(HashMap::from([(
        "checker".into(),
        ImageSource::Bitmap(Arc::new(buffer)),
      )]))
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  assert!(
    haystack.contains("/Subtype/Image"),
    "expected image XObjects for the url() layers"
  );
  assert!(
    haystack.contains("/XStep 48/YStep 48"),
    "expected the tiled mask layer to repeat at its mask-size"
  );
}

/// `background-size`, `-position` and the four `-repeat` styles: a sized tile
/// placed once, tiled, spaced out, and rounded to fit whole tiles.
#[test]
fn background_placement() {
  let pdf = run_pdf_fixture("background-placement", |fonts| {
    let cell = |style: &str| {
      format!(
        r##"<div style="width: 120px; height: 120px; background-color: #f4f4f5; background-image: linear-gradient(135deg, #ff5f6d, #3a1c71); background-size: 50px 35px; {style}"></div>"##
      )
    };
    let source = format!(
      r##"<div style="display: flex; width: 100%; height: 100%; padding: 16px; column-gap: 16px; background-color: #ffffff;">
        {}{}{}{}{}
      </div>"##,
      cell("background-repeat: no-repeat; background-position: right bottom;"),
      // A tile as wide as the box still tiles: the phase pulls a second one in.
      cell("background-repeat: repeat; background-size: 120px 120px; background-position: 20px 0;"),
      cell("background-repeat: repeat;"),
      cell("background-repeat: space;"),
      // The position still applies to the rescaled tile, shifting its phase.
      cell("background-repeat: round; background-position: center;"),
    );
    let node =
      from_html(&source, FromHtmlOptions::default()).expect("parse background placement fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((720, 160)))
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  // Three of the four cells tile, and a tiling pattern is one shading reused
  // by a pattern object rather than one shading per tile. In a 120px box a
  // 50x35 tile repeats at its own size, spaces out to 70x42.5, and rounds to
  // two by three whole tiles.
  for needle in [
    "/XStep 50/YStep 35",
    "/XStep 70/YStep 42.5",
    "/XStep 60/YStep 40",
  ] {
    assert!(haystack.contains(needle), "missing pattern step {needle}");
  }
  assert_eq!(
    haystack.matches("/PatternType 1").count(),
    4,
    "expected one tiling pattern per repeating cell"
  );
}

/// `box-decoration-break: clone` on a fragmented container: every page
/// fragment paints full borders and radius; the avoided child moves whole.
#[test]
fn paged_clone_decorations() {
  run_pdf_fixture("paged-clone-decorations", |fonts| {
    let rows: String = (1..=24)
      .map(|i| {
        format!(
          r#"<div style="font-size: 13px; color: #1c1917;">Clause {i} of the agreement</div>"#
        )
      })
      .collect();
    let source = format!(
      r#"<div style="display: flex; flex-direction: column; width: 100%; padding: 14px; row-gap: 4px; border: 3px solid #1c1917; border-radius: 14px; box-decoration-break: clone; background-color: #fafaf9;">
        {rows}
        <div style="display: flex; flex-direction: column; row-gap: 4px; padding: 10px; background-color: #e7e5e4; break-inside: avoid;">
          <div style="font-size: 13px;">Kept-together block line one</div>
          <div style="font-size: 13px;">Kept-together block line two</div>
          <div style="font-size: 13px;">Kept-together block line three</div>
        </div>
      </div>"#
    );

    PdfOptions::builder()
      .node(from_html(&source, FromHtmlOptions::default()).expect("parse clone fixture"))
      .page(PageOptions {
        width: 360.0,
        height: 260.0,
        margin: PageMargins::uniform(20.0),
      })
      .fonts(fonts)
      .build()
  });
}

/// Transformed subtrees become unsplittable atoms: the rotated card near a cut
/// moves whole to the next page; the skewed divider stays intact.
#[test]
fn paged_transform_atoms() {
  run_pdf_fixture("paged-transforms", |fonts| {
    let source = r#"<div style="display: flex; flex-direction: column; width: 100%; row-gap: 10px;">
      <div style="height: 150px; background-color: #dbeafe;"></div>
      <div style="width: 100%; height: 10px; transform: skewY(4deg); background-color: #111111;"></div>
      <div style="width: 220px; height: 90px; transform: rotate(8deg); background-color: #fecaca; border: 2px solid #b91c1c;"></div>
      <div style="height: 140px; background-color: #dcfce7;"></div>
      <div style="width: 200px; height: 70px; transform: scale(1.2) translate(20px, 0px); background-color: #fde68a;"></div>
      <div style="height: 150px; background-color: #f3e8ff;"></div>
    </div>"#;

    PdfOptions::builder()
      .node(from_html(source, FromHtmlOptions::default()).expect("parse transforms fixture"))
      .page(PageOptions {
        width: 400.0,
        height: 300.0,
        margin: PageMargins::uniform(24.0),
      })
      .fonts(fonts)
      .build()
  });
}

/// Header and footer bands together, counters in both, a forced break, and a
/// keep-together block taller than the window (hard cut).
#[test]
fn paged_header_footer() {
  run_pdf_fixture("paged-header-footer", |fonts| {
    let tall_rows: String = (1..=30)
      .map(|i| format!(r#"<div style="font-size: 12px;">Overflowing row {i}</div>"#))
      .collect();
    let source = format!(
      r#"<div style="display: flex; flex-direction: column; width: 100%; row-gap: 4px;">
        <div style="font-size: 14px; break-after: page;">Section one ends here</div>
        <div style="display: flex; flex-direction: column; row-gap: 4px; break-inside: avoid; background-color: #f5f5f4;">
          {tall_rows}
        </div>
        <div style="font-size: 14px;">Trailing content</div>
      </div>"#
    );
    let band = |label: &str| {
      from_html(
        &format!(
          r#"<div style="display: flex; width: 100%; justify-content: space-between; font-size: 11px; color: #57534e; padding: 6px 0;">
            <div>{label}</div>
            <div style="display: flex; column-gap: 3px">Page <span class="pageNumber"></span> of <span class="totalPages"></span></div>
          </div>"#
        ),
        FromHtmlOptions::default(),
      )
      .expect("parse band fixture")
    };

    PdfOptions::builder()
      .node(from_html(&source, FromHtmlOptions::default()).expect("parse header-footer fixture"))
      .page(PageOptions {
        width: 400.0,
        height: 320.0,
        margin: PageMargins::uniform(24.0),
      })
      .header(band("Quarterly report"))
      .footer(band("Confidential"))
      .fonts(fonts)
      .build()
  });
}

/// Blend modes, nested opacity, and isolation on overlapping circles.
#[test]
fn blend_opacity_isolation() {
  run_pdf_fixture("blend-opacity", |fonts| {
    let source = r#"<div style="display: flex; width: 100%; height: 100%; padding: 20px; background-color: #ffffff;">
      <div style="display: flex; isolation: isolate; opacity: 0.9;">
        <div style="width: 110px; height: 110px; border-radius: 50%; background-color: #ef4444; mix-blend-mode: multiply;"></div>
        <div style="width: 110px; height: 110px; border-radius: 50%; margin-left: -40px; background-color: #3b82f6; mix-blend-mode: multiply;"></div>
        <div style="width: 110px; height: 110px; border-radius: 50%; margin-left: -40px; background-color: #22c55e; mix-blend-mode: screen; opacity: 0.6;"></div>
      </div>
    </div>"#;
    let node = from_html(source, FromHtmlOptions::default()).expect("parse blend fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((320, 160)))
      .fonts(fonts)
      .build()
  });
}

/// PDF/UA-2 asks every link inside a document to name a structure element. A
/// link can point at anything with an id, including markup that carries no
/// meaning of its own and would otherwise leave no element to name.
#[test]
fn a_link_target_without_meaning_still_gets_an_element() {
  for target in [
    r#"<div id="target">plain target div</div>"#,
    r#"<div id="target"></div>"#,
  ] {
    let doc = format!(
      r##"<div style="width:700px;font-size:20px">
        <p><a href="#target">jump to target</a></p>
        <h1>heading for the outline</h1>
        {target}
      </div>"##
    );
    let pdf = render(
      PdfOptions::builder()
        .node(from_html(&doc, FromHtmlOptions::default()).expect("parse anchor doc"))
        .page(PageOptions::A4)
        .tagged(Tagging::Ua2)
        .standard(PdfStandard::A4)
        .lang(Some(takumi_core::style::Lang::parse("en").expect("lang")))
        .metadata(PdfMetadata {
          title: Some("Anchors".into()),
          creation_date: Some(PdfDate {
            year: 2026,
            month: 8,
            day: 7,
            hour: 0,
            minute: 0,
            second: 0,
          }),
          ..Default::default()
        })
        .fonts(&fonts())
        .build(),
    )
    .expect("render anchor doc");

    // The destination names this element, so the element has to be in the file.
    assert!(
      inflated_text(&pdf).contains("n.0.2"),
      "the link target left no structure element to name: {target}"
    );
  }
}

/// A clip keeps content off the page, but a PDF clip does not keep it out of
/// the text layer. Content an ancestor cuts away must never be emitted, or it
/// stays extractable on whichever page its own geometry happens to land on.
///
/// The cut-away line is the only Chinese on the page, so the face it would need
/// says whether it reached the file.
#[test]
fn a_clipped_away_line_reaches_no_page() {
  let doc = r#"<div style="width:700px">
    <div style="overflow:hidden;height:40px;background:#eee">
      <div style="height:2600px">
        <div style="margin-top:1500px;font-size:24px">裁掉的秘密</div>
      </div>
    </div>
    <div style="font-size:24px;height:2200px">visible after box</div>
  </div>"#;
  let pdf = render(
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse clipped doc"))
      .page(PageOptions::A4)
      .fonts(&fonts())
      .build(),
  )
  .expect("render clipped doc");
  let haystack = inflated_text(&pdf);

  assert!(
    embedded_subsets(&haystack, "Archivo") > 0,
    "the fixture stopped covering what it was meant to keep"
  );
  assert_eq!(
    embedded_subsets(&haystack, "NotoSansTC"),
    0,
    "content an overflow clip cuts away still reached the file"
  );
}

/// Overflow clipping: rounded clip on both axes, and a single-axis clip that
/// leaves the other axis unbounded.
#[test]
fn overflow_clipping() {
  run_pdf_fixture("overflow-clip", |fonts| {
    let source = r#"<div style="display: flex; width: 100%; height: 100%; padding: 16px; column-gap: 24px; background-color: #ffffff;">
      <div style="overflow: hidden; border-radius: 24px; width: 140px; height: 110px; border: 2px solid #333333;">
        <div style="width: 300px; height: 300px; background-image: linear-gradient(45deg, #f97316, #0ea5e9);"></div>
      </div>
      <div style="overflow-x: hidden; width: 120px; height: 110px; border: 2px solid #999999;">
        <div style="width: 300px; height: 80px; background-color: #a3e635;"></div>
      </div>
    </div>"#;
    let node = from_html(source, FromHtmlOptions::default()).expect("parse overflow fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((360, 150)))
      .fonts(fonts)
      .build()
  });
}

/// Repeating gradient variants and a stacked multi-layer background.
#[test]
fn repeating_gradients() {
  run_pdf_fixture("repeating-gradients", |fonts| {
    let source = r#"<div style="display: flex; width: 100%; height: 100%; padding: 20px; column-gap: 20px; background-color: #ffffff;">
      <div style="width: 110px; height: 110px; background-image: repeating-linear-gradient(45deg, #0f172a 0px, #0f172a 8px, #f8fafc 8px, #f8fafc 16px);"></div>
      <div style="width: 110px; height: 110px; background-image: repeating-radial-gradient(circle, #7c3aed 0px, #7c3aed 10px, #ede9fe 10px, #ede9fe 20px);"></div>
      <div style="width: 110px; height: 110px; background-image: linear-gradient(180deg, rgba(255, 0, 0, 0.5), rgba(255, 0, 0, 0)), conic-gradient(from 45deg, #fbbf24, #10b981, #fbbf24);"></div>
    </div>"#;
    let node = from_html(source, FromHtmlOptions::default()).expect("parse repeating fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((440, 160)))
      .fonts(fonts)
      .build()
  });
}

/// Decoration lines with custom colors, letter spacing, and variable weight.
#[test]
fn text_decorations() {
  run_pdf_fixture("text-decorations", |fonts| {
    let source = r#"<div style="display: flex; flex-direction: column; width: 100%; height: 100%; padding: 16px; row-gap: 8px; background-color: #ffffff; font-size: 18px; color: #111111;">
      <div style="text-decoration-line: underline; text-decoration-color: #dc2626;">Underlined in red</div>
      <div style="text-decoration-line: line-through;">Struck through</div>
      <div style="text-decoration-line: overline underline;">Over and under</div>
      <div style="letter-spacing: 4px;">Wide tracking</div>
      <div style="font-weight: 700;">Bold weight text</div>
    </div>"#;
    let node = from_html(source, FromHtmlOptions::default()).expect("parse decorations fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((360, 220)))
      .fonts(fonts)
      .build()
  });
}

/// Images flowing across a page cut are atoms: the straddling image moves
/// whole to the next page.
#[test]
fn paged_images() {
  run_pdf_fixture("paged-images", |fonts| {
    let checker = |dark: [u8; 4], light: [u8; 4]| {
      let mut pixels = Vec::with_capacity(8 * 8 * 4);

      for row in 0..8u32 {
        for col in 0..8u32 {
          let on = (row / 2 + col / 2) % 2 == 0;

          pixels.extend_from_slice(if on { &dark } else { &light });
        }
      }
      Node::image(ImageData {
        src: ImageSourceInput::Rgba(RgbaImage::new(pixels, 8, 8, false).expect("rgba image")),
        width: Some(120.0),
        height: Some(120.0),
      })
    };
    let children = vec![
      text("Before the images", 14.0),
      checker([220, 60, 60, 255], [255, 235, 235, 255]),
      checker([60, 60, 220, 255], [235, 235, 255, 255]),
      checker([60, 180, 90, 255], [230, 250, 235, 255]),
      text("After the images", 14.0),
    ];

    PdfOptions::builder()
      .node(column(children))
      .page(PageOptions {
        width: 300.0,
        height: 260.0,
        margin: PageMargins::uniform(20.0),
      })
      .fonts(fonts)
      .build()
  });
}

/// Degenerate inputs stay silent instead of panicking or emitting garbage:
/// zero-sized boxes, empty and zero-font-size text, transparent paint.
#[test]
fn edge_degenerate() {
  run_pdf_fixture("edge-degenerate", |fonts| {
    let source = r#"<div style="display: flex; flex-direction: column; width: 100%; height: 100%; padding: 12px; row-gap: 6px; background-color: #ffffff;">
      <div style="width: 0px; height: 40px; background-color: #ef4444;"></div>
      <div style="width: 120px; height: 0px; background-color: #22c55e;"></div>
      <div style="font-size: 14px;"> </div>
      <div style="font-size: 0px;">Zero font size</div>
      <div style="opacity: 0; font-size: 14px;">Fully transparent text</div>
      <div style="width: 80px; height: 20px; background-color: rgba(0, 0, 0, 0); border: 2px solid rgba(255, 0, 0, 0);"></div>
      <div style="width: 1px; height: 1px; background-color: #3b82f6;"></div>
      <div style="width: 40px; height: 40px; border-radius: 50%; border: 1px solid #111111; background-color: #d4d4d8;"></div>
      <div style="font-size: 13px; color: #111111;">Visible sentinel after the degenerates</div>
    </div>"#;
    let node = from_html(source, FromHtmlOptions::default()).expect("parse degenerate fixture");

    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((300, 260)))
      .fonts(fonts)
      .build()
  });
}

fn wrapping_rows(count: usize) -> Node {
  column(
    (1..=count)
      .map(|i| {
        text(
          &format!("Paragraph {i}: text long enough to wrap onto several lines when the page gets narrow, exercising line breaking against the page width"),
          13.0,
        )
      })
      .collect(),
  )
}

/// The same wrapping content on a narrow tall page: many short lines, cuts
/// landing mid-paragraph.
#[test]
fn paged_narrow() {
  run_pdf_fixture("paged-narrow", |fonts| {
    PdfOptions::builder()
      .node(wrapping_rows(10))
      .page(PageOptions {
        width: 200.0,
        height: 420.0,
        margin: PageMargins::uniform(16.0),
      })
      .fonts(fonts)
      .build()
  });
}

/// The same wrapping content on landscape US Letter with a wide margin: long
/// lines, few per page, preset + landscape + with_margin all in play.
#[test]
fn paged_landscape() {
  run_pdf_fixture("paged-landscape", |fonts| {
    PdfOptions::builder()
      .node(wrapping_rows(60))
      .page(PageOptions::LETTER.landscape().with_margin(60.0))
      .fonts(fonts)
      .build()
  });
}

#[test]
fn invoice() {
  run_pdf_fixture("invoice", |fonts| {
    PdfOptions::builder()
      .node(html_fixture("invoice.html"))
      .page(PageOptions::A4.with_margin(36.0))
      .footer(html_fixture("invoice-footer.html"))
      .fonts(fonts)
      .build()
  });
}

/// The invoice (paged, footer, gradients, links) renders under PDF/A-2b with
/// an sRGB output intent, and under PDF/A-4 with a PDF 2.0 header.
#[test]
fn archival_standards() {
  let a2b = run_pdf_fixture("invoice-pdfa-2b", |fonts| {
    PdfOptions::builder()
      .node(html_fixture("invoice.html"))
      .page(PageOptions::A4.with_margin(36.0))
      .footer(html_fixture("invoice-footer.html"))
      .standard(PdfStandard::A2b)
      .fonts(fonts)
      .build()
  });
  let haystack = String::from_utf8_lossy(&a2b);

  assert!(haystack.starts_with("%PDF-1.7"));
  assert!(haystack.contains("GTS_PDFA1"), "missing output intent");

  let a4 = run_pdf_fixture("invoice-pdfa-4", |fonts| {
    PdfOptions::builder()
      .node(html_fixture("invoice.html"))
      .page(PageOptions::A4.with_margin(36.0))
      .footer(html_fixture("invoice-footer.html"))
      .standard(PdfStandard::A4)
      .fonts(fonts)
      .build()
  });

  assert!(String::from_utf8_lossy(&a4).starts_with("%PDF-2.0"));
}

const FACTUR_X_NAMESPACE: &str = "urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0#";

fn factur_x_schema() -> XmpSchema {
  XmpSchema {
    name: "Factur-X PDF/A Extension".to_string(),
    prefix: "fx".to_string(),
    namespace: FACTUR_X_NAMESPACE.to_string(),
    properties: vec![XmpProperty {
      name: "DocumentFileName".to_string(),
      value: "factur-x.xml".to_string(),
      description: "name of the embedded XML invoice file".to_string(),
    }],
  }
}

/// A schema whose prefix cannot be an XML name rejects the render: the XMP
/// writer would serialize it verbatim into a packet nothing can parse.
#[test]
fn invalid_xmp_schema_rejects() {
  let metadata = PdfMetadata {
    xmp: vec![XmpSchema {
      prefix: "1fx bad".to_string(),
      ..factur_x_schema()
    }],
    ..PdfMetadata::default()
  };
  let result = render(
    PdfOptions::builder()
      .node(text("invalid xmp", 16.0))
      .viewport(Viewport::new((200, 100)))
      .fonts(&fonts())
      .metadata(metadata)
      .build(),
  );

  assert!(matches!(result, Err(PdfError::InvalidXmpSchema(prefix)) if prefix == "1fx bad"));
}

/// A character no registered font covers shapes to `.notdef`. It paints nothing
/// and leaves nothing in the text layer, so the render stops and names it
/// instead of handing back a page with the character quietly gone.
#[test]
fn uncovered_character_stops_the_render() {
  let latin_only = {
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
  };
  let render_with = |content: &str| {
    render(
      PdfOptions::builder()
        .node(text(content, 16.0))
        .viewport(Viewport::new((200, 100)))
        .fonts(&latin_only)
        .build(),
    )
  };

  assert!(render_with("covered").is_ok());
  assert!(
    matches!(render_with("uncovered \u{76F4}"), Err(PdfError::MissingGlyphs(named)) if named == "直 (U+76F4)")
  );
}

/// The invoice carries a machine-readable XML attachment under PDF/A-3b:
/// the file spec, association kind, and name tree all serialize, the
/// modification date falls back to the metadata creation date, and a custom
/// XMP fragment lands inside the packet krilla writes.
#[test]
fn attachments() {
  let attachment = || Attachment {
    name: "factur-x.xml".to_string(),
    data: b"<invoice total=\"1290\"/>".to_vec(),
    mime_type: Some("application/xml".to_string()),
    description: Some("Factur-X invoice data".to_string()),
    relationship: AttachmentRelationship::Alternative,
    modification_date: None,
  };
  let metadata = || PdfMetadata {
    title: Some("Invoice".to_string()),
    creation_date: Some(PdfDate {
      year: 2026,
      month: 8,
      day: 6,
      hour: 0,
      minute: 0,
      second: 0,
    }),
    xmp: vec![factur_x_schema()],
    ..PdfMetadata::default()
  };
  let a3b = run_pdf_fixture("invoice-pdfa-3b-attachment", |fonts| {
    PdfOptions::builder()
      .node(html_fixture("invoice.html"))
      .page(PageOptions::A4.with_margin(36.0))
      .footer(html_fixture("invoice-footer.html"))
      .standard(PdfStandard::A3b)
      .metadata(metadata())
      .attachments(vec![attachment()])
      .fonts(fonts)
      .build()
  });
  let haystack = String::from_utf8_lossy(&a3b);

  assert!(haystack.contains("factur-x.xml"), "missing file spec name");
  assert!(haystack.contains("/EmbeddedFiles"), "missing name tree");
  assert!(haystack.contains("/AFRelationship"), "missing association");
  // Scoped to the embedded file's Params dict: the Info dict also carries a
  // /ModDate, so a document-wide match would not prove the fallback.
  assert!(
    haystack.contains("/Params<</Size 23/ModDate(D:20260806000000Z)>>"),
    "missing attachment modification date fallback"
  );

  let (packet, _) = haystack
    .split_once("</rdf:RDF>")
    .expect("missing XMP packet");

  assert!(
    packet.contains("<fx:DocumentFileName>factur-x.xml</fx:DocumentFileName>"),
    "custom property missing from the packet"
  );
  // A packet carries at most one schema bag, so the custom entry has to land in
  // the one krilla writes: a second bag makes the whole packet unparseable.
  assert_eq!(
    packet.matches("<pdfaExtension:schemas>").count(),
    1,
    "custom schema entry did not merge into the packet's schema bag"
  );
  assert!(
    packet.contains(FACTUR_X_NAMESPACE),
    "custom schema description missing from the packet"
  );

  // PDF/A-4f is the PDF 2.0 spelling of the same container.
  let a4f = run_pdf_fixture("invoice-pdfa-4f-attachment", |fonts| {
    PdfOptions::builder()
      .node(html_fixture("invoice.html"))
      .page(PageOptions::A4.with_margin(36.0))
      .footer(html_fixture("invoice-footer.html"))
      .standard(PdfStandard::A4f)
      .metadata(metadata())
      .attachments(vec![attachment()])
      .fonts(fonts)
      .build()
  });
  let haystack = String::from_utf8_lossy(&a4f);

  assert!(haystack.starts_with("%PDF-2.0"));
  assert!(haystack.contains("/EmbeddedFiles"), "missing name tree");

  let fonts = fonts();
  let duplicate = render(
    PdfOptions::builder()
      .node(text("dup", 16.0))
      .page(PageOptions::A4)
      .attachments(vec![attachment(), attachment()])
      .fonts(&fonts)
      .build(),
  );

  assert!(matches!(duplicate, Err(PdfError::DuplicateAttachment(name)) if name == "factur-x.xml"));

  let invalid_mime = render(
    PdfOptions::builder()
      .node(text("mime", 16.0))
      .page(PageOptions::A4)
      .attachments(vec![Attachment {
        mime_type: Some("not-a-mime".to_string()),
        ..attachment()
      }])
      .fonts(&fonts)
      .build(),
  );

  assert!(matches!(invalid_mime, Err(PdfError::InvalidMimeType(mime)) if mime == "not-a-mime"));

  // PDF/A-2 forbids arbitrary attachments; PDF/A-3 requires the descriptive
  // fields and a date. These reach the render through Rust and wasm callers,
  // which the TypeScript union cannot guard.
  let a2b = render(
    PdfOptions::builder()
      .node(text("a2b", 16.0))
      .page(PageOptions::A4)
      .standard(PdfStandard::A2b)
      .metadata(metadata())
      .attachments(vec![attachment()])
      .fonts(&fonts)
      .build(),
  );

  assert!(
    matches!(a2b, Err(PdfError::Krilla(_))),
    "A-2b must reject attachments"
  );

  for stripped in [
    Attachment {
      mime_type: None,
      ..attachment()
    },
    Attachment {
      description: None,
      ..attachment()
    },
  ] {
    let incomplete = render(
      PdfOptions::builder()
        .node(text("incomplete", 16.0))
        .page(PageOptions::A4)
        .standard(PdfStandard::A3b)
        .metadata(metadata())
        .attachments(vec![stripped])
        .fonts(&fonts)
        .build(),
    );

    assert!(
      matches!(incomplete, Err(PdfError::Krilla(_))),
      "A-3b must require the field"
    );
  }

  let dateless = render(
    PdfOptions::builder()
      .node(text("dateless", 16.0))
      .page(PageOptions::A4)
      .standard(PdfStandard::A3b)
      .attachments(vec![attachment()])
      .fonts(&fonts)
      .build(),
  );

  assert!(
    matches!(dateless, Err(PdfError::Krilla(_))),
    "A-3b must require a date when the metadata fallback is absent"
  );
}

/// The report renders tagged under PDF/UA-1 and PDF/A-2a: heading structure,
/// link alt text, document language, title and date all satisfy the
/// validators, and the structure tree serializes.
#[test]
fn tagged_standards() {
  let metadata = || PdfMetadata {
    title: Some("Annual report".into()),
    creation_date: Some(PdfDate {
      year: 2026,
      month: 8,
      day: 6,
      hour: 0,
      minute: 0,
      second: 0,
    }),
    ..Default::default()
  };
  let lang = || takumi_core::style::Lang::parse("en").expect("lang");
  // PDF/UA-1 combined with PDF/A-2a: both validators run on one render.
  let ua1 = run_pdf_fixture("report-tagged-ua1", |fonts| {
    PdfOptions::builder()
      .node(html_fixture("report.html"))
      .page(PageOptions::A4)
      .tagged(Tagging::Ua1)
      .standard(PdfStandard::A2a)
      .lang(Some(lang()))
      .metadata(metadata())
      .fonts(fonts)
      .build()
  });
  let haystack = String::from_utf8_lossy(&ua1);

  assert!(
    haystack.contains("StructTreeRoot"),
    "missing structure tree"
  );

  let list_doc = r#"<main style="display:flex;flex-direction:column;font-size:14px;color:#141414;">
    <h1>Checklist</h1>
    <p>Steps with <strong>bold</strong> and <code>code</code>:</p>
    <ul><li>First item</li><li>Second item</li></ul>
    <ol><li>Ordered one</li><li>Ordered two</li></ol>
  </main>"#;

  let list = run_pdf_fixture("list-tagged-ua1", |fonts| {
    PdfOptions::builder()
      .node(from_html(list_doc, FromHtmlOptions::default()).expect("parse list doc"))
      .page(PageOptions::A4)
      .tagged(Tagging::Ua1)
      .lang(Some(lang()))
      .metadata(metadata())
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&list);

  for name in [
    "/S/LI",
    "/S/LBody",
    "/ListNumbering/Disc",
    "/ListNumbering/Decimal",
  ] {
    assert!(haystack.contains(name), "missing {name} structure element");
  }

  run_pdf_fixture("report-tagged-a2a", |fonts| {
    PdfOptions::builder()
      .node(html_fixture("report.html"))
      .page(PageOptions::A4)
      .standard(PdfStandard::A2a)
      .lang(Some(lang()))
      .metadata(metadata())
      .fonts(fonts)
      .build()
  });
}

/// An inline-block lays out in a subtree of its own. Its content still belongs
/// to the structure tree, down to the elements nested inside it.
#[test]
fn tagged_inline_block_subtree() {
  let source = r#"<main style="display:flex;flex-direction:column;font-size:14px;color:#141414;">
    <h1>Report</h1>
    <div>Before <span style="display:inline-block;width:160px;"><span style="display:inline-block;"><h2>Inner heading</h2></span></span> after</div>
  </main>"#;

  let pdf = run_pdf_fixture("inline-block-tagged", |fonts| {
    PdfOptions::builder()
      .node(from_html(source, FromHtmlOptions::default()).expect("parse inline-block doc"))
      .page(PageOptions::A4)
      .tagged(Tagging::Ua1)
      .lang(Some(takumi_core::style::Lang::parse("en").expect("lang")))
      .metadata(PdfMetadata {
        title: Some("Report".into()),
        ..Default::default()
      })
      .fonts(fonts)
      .build()
  });

  assert!(
    inflated_text(&pdf).contains("/S/H2"),
    "the element inside the inline-block never reached the structure tree"
  );
}

/// Every structure element takumi emits, under PDF/A-4. A PDF 2.0 tag carries a
/// namespace and a role map that the PDF 1.7 fixtures never exercise.
#[test]
fn structure_types_pdf20() {
  let doc = r##"<main style="display:flex;flex-direction:column;font-size:14px;color:#141414;">
    <h1 id="top">Structure types</h1>
    <section>
      <h2>Prose</h2>
      <p>A paragraph with <strong>bold</strong>, <em>italic</em> and <code>code</code>.</p>
      <blockquote>A quotation on its own.</blockquote>
    </section>
    <article>
      <h3>Lists</h3>
      <ul><li>Unordered one</li><li>Unordered two</li></ul>
      <ol><li>Ordered one</li><li>Ordered two</li></ol>
    </article>
    <figure>
      <img src="pixel" alt="a grey square" style="width:40px;height:40px;" />
      <figcaption>A described pixel.</figcaption>
    </figure>
    <p><a href="#top">Back to the top</a> and <a href="https://example.com">out to the web</a>.</p>
  </main>"##;
  let pdf = run_pdf_fixture("structure-types-a4", |fonts| {
    let buffer = ImageBuffer::from_rgba_bytes(vec![128; 4 * 4 * 4], 4, 4).expect("image buffer");

    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse structure doc"))
      .images(HashMap::from([(
        "pixel".into(),
        ImageSource::Bitmap(Arc::new(buffer)),
      )]))
      .page(PageOptions::A4)
      .standard(PdfStandard::A4)
      .lang(Some(takumi_core::style::Lang::parse("en").expect("lang")))
      .metadata(PdfMetadata {
        title: Some("Structure types".into()),
        creation_date: Some(PdfDate {
          year: 2026,
          month: 8,
          day: 8,
          hour: 0,
          minute: 0,
          second: 0,
        }),
        ..Default::default()
      })
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  for name in [
    "/Sect",
    "/Art",
    "/H1",
    "/H2",
    "/H3",
    "/P",
    "/BlockQuote",
    "/L",
    "/LI",
    "/LBody",
    "/Figure",
    "/Caption",
    "/Link",
  ] {
    assert!(haystack.contains(name), "missing {name} structure element");
  }
}

/// PDF/UA-2 rides on PDF 2.0, so it pairs with PDF/A-4 and validates the same
/// structure tree PDF/UA-1 does.
#[test]
fn tagged_ua2() {
  let doc = r#"<main style="display:flex;flex-direction:column;font-size:14px;color:#141414;">
    <h1>Accessible report</h1>
    <p>A paragraph of prose.</p>
    <h2>Findings</h2>
    <ul><li>First finding</li><li>Second finding</li></ul>
    <figure>
      <img src="pixel" alt="a grey square" style="width:40px;height:40px;" />
      <figcaption>A described pixel.</figcaption>
    </figure>
  </main>"#;
  let pdf = run_pdf_fixture("report-tagged-ua2", |fonts| {
    let buffer = ImageBuffer::from_rgba_bytes(vec![128; 4 * 4 * 4], 4, 4).expect("image buffer");

    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse ua2 doc"))
      .images(HashMap::from([(
        "pixel".into(),
        ImageSource::Bitmap(Arc::new(buffer)),
      )]))
      .page(PageOptions::A4)
      .standard(PdfStandard::A4)
      .tagged(Tagging::Ua2)
      .lang(Some(takumi_core::style::Lang::parse("en").expect("lang")))
      .metadata(PdfMetadata {
        title: Some("Accessible report".into()),
        creation_date: Some(PdfDate {
          year: 2026,
          month: 8,
          day: 8,
          hour: 0,
          minute: 0,
          second: 0,
        }),
        ..Default::default()
      })
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  assert!(
    haystack.contains("<pdfuaid:part>2</pdfuaid:part>"),
    "missing PDF/UA-2 identification"
  );
  assert!(
    haystack.contains("/IDTree"),
    "outline destinations did not name a structure element"
  );
}

/// A border decoration is content no reader should announce, so it belongs in
/// an artifact sequence. A border that paints nothing belongs in no sequence at
/// all: an empty `BMC`/`EMC` pair is a region with nothing in it.
#[test]
fn tagged_borders_are_artifacts() {
  let doc = r#"<main style="display:flex;flex-direction:column;gap:8px;font-size:14px;color:#141414;">
    <h1>Borders</h1>
    <p style="border:3px dashed #b91c1c;padding:4px;">Dashed all round</p>
    <p style="border-top:4px solid rgba(255,0,0,0);border-left:2px solid rgba(0,0,255,0);padding:4px;">Invisible sides</p>
  </main>"#;
  let pdf = run_pdf_fixture("borders-tagged-ua1", |fonts| {
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse border doc"))
      .page(PageOptions::A4)
      .standard(PdfStandard::A3a)
      .tagged(Tagging::Ua1)
      .lang(Some(takumi_core::style::Lang::parse("en").expect("lang")))
      .metadata(PdfMetadata {
        title: Some("Borders".into()),
        creation_date: Some(PdfDate {
          year: 2026,
          month: 8,
          day: 9,
          hour: 0,
          minute: 0,
          second: 0,
        }),
        ..Default::default()
      })
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);
  let stroke = haystack
    .find(" d ")
    .expect("no dashed stroke in the content stream");
  let opened = haystack[..stroke]
    .rfind("/Artifact BMC")
    .expect("the dashed border opened no artifact");

  assert!(
    !haystack[opened..stroke].contains("EMC"),
    "the dashed border strokes outside its artifact"
  );
  for region in haystack.split("/Artifact BMC").skip(1) {
    let region = &region[..region.find("EMC").expect("an artifact was never closed")];

    assert!(
      region
        .split_whitespace()
        .any(|token| matches!(token, "f" | "f*" | "S" | "Do")),
      "an artifact holds no painted content: {region:?}"
    );
  }
}

/// PDF/UA-2 requires the catalog to declare the document language, so a render
/// without one fails instead of writing a file that claims conformance.
#[test]
fn tagged_ua2_needs_lang() {
  let doc = "<main><h1>No language</h1></main>";
  let error = render(
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse ua2 doc"))
      .page(PageOptions::A4)
      .standard(PdfStandard::A4)
      .tagged(Tagging::Ua2)
      .metadata(PdfMetadata {
        title: Some("No language".into()),
        creation_date: Some(PdfDate {
          year: 2026,
          month: 8,
          day: 8,
          hour: 0,
          minute: 0,
          second: 0,
        }),
        ..Default::default()
      })
      .fonts(&fonts())
      .build(),
  )
  .expect_err("a document without a language cannot claim PDF/UA-2");

  assert!(
    format!("{error:?}").contains("NoDocumentLanguage"),
    "unexpected error: {error:?}"
  );
}

/// An image inside a wrapper is an inline box rather than a node of its own,
/// so it draws from the inline layout. Only a direct child of the root used to
/// reach the page.
#[test]
fn inline_images() {
  let doc = r#"<main style="display:flex;flex-direction:column;font-size:14px;color:#141414;">
    <div><img src="wrapped" alt="wrapped in a div" style="width:40px;height:40px;" /></div>
    <div style="display:block">Text before <img src="inline" alt="between words" style="width:20px;height:20px;opacity:0.5;" /> and after.</div>
  </main>"#;
  let pdf = run_pdf_fixture("inline-images", |fonts| {
    // Distinct pixels: krilla dedups images by content, so one bitmap for both
    // would let a single painted box satisfy the assertion below.
    let wrapped = ImageBuffer::from_rgba_bytes(vec![64; 4 * 4 * 4], 4, 4).expect("image buffer");
    let inline = ImageBuffer::from_rgba_bytes(vec![192; 4 * 4 * 4], 4, 4).expect("image buffer");

    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse image doc"))
      .images(HashMap::from([
        ("wrapped".into(), ImageSource::Bitmap(Arc::new(wrapped))),
        ("inline".into(), ImageSource::Bitmap(Arc::new(inline))),
      ]))
      .page(PageOptions::A4)
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  for name in ["/x0 Do", "/x1 Do"] {
    assert!(
      haystack.contains(name),
      "an inline image never reached the page: {name} missing"
    );
  }
  // The second image is half transparent, and an inline box gets its paint
  // state here rather than from the paint list.
  assert!(
    haystack.contains("/ca 0.5"),
    "an inline image ignored its opacity"
  );
}

/// CSS 2.1 Appendix E paints the outline last, so a negative `outline-offset`
/// draws over the box's own text instead of under it.
#[test]
fn outline_over_content() {
  let doc = r#"<main style="display:flex;font-size:40px;color:#141414;"><div style="display:block;outline:8px solid #ff0000;outline-offset:-8px;background-color:#ffffff;">TEXT</div></main>"#;
  let pdf = run_pdf_fixture("outline-over-content", |fonts| {
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse outline doc"))
      .page(PageOptions::A4)
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);
  let outline = haystack.find("1 0 0 rg").expect("no outline fill");
  let glyphs = haystack.find("Tj").or_else(|| haystack.find("TJ"));

  assert!(
    glyphs.is_some_and(|glyphs| outline > glyphs),
    "the outline painted under the text"
  );
}

/// The outline paints after the box's content but still under the box's own
/// transform, so a rotated box's outline rotates with it.
#[test]
fn outline_under_transform() {
  let doc = r#"<main style="display:flex;font-size:30px;color:#141414;"><div style="display:block;transform:rotate(20deg);outline:6px solid #ff0000;">TEXT</div></main>"#;
  let pdf = run_pdf_fixture("outline-under-transform", |fonts| {
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse outline doc"))
      .page(PageOptions::A4)
      .fonts(fonts)
      .build()
  });
  // The outline's fill has to sit inside the box's own rotation, so the two
  // land in the same `q` block.
  let haystack = inflated_text(&pdf);
  let block = haystack
    .split("q ")
    .find(|block| block.contains("1 0 0 rg"))
    .expect("no outline fill");

  assert!(
    block.starts_with("0.7047695 -0.2565151"),
    "the outline painted outside the box transform"
  );
}

/// An inline-level container is laid out by the inline layout, not the paint
/// list, so it needs a layout pass and a scene of its own to reach the page.
#[test]
fn inline_containers() {
  let doc = r#"<main style="display:flex;flex-direction:column;font-size:20px;color:#141414;">
    <div style="display:block">before <span style="display:inline-block;background-color:#ff0000;">block</span> after</div>
    <div style="display:block">before <span style="display:inline-flex;background-color:#00ff00;"><span>fl</span><span>ex</span></span> after</div>
    <div style="display:block">before <span style="display:inline-block;background-color:#0000ff;"><span style="display:inline-block;background-color:#ffff00;">nested</span></span> after</div>
    <div style="display:block"><span style="float:left;width:30px;height:30px;background-color:#ff00ff;"></span>floated</div>
  </main>"#;
  let pdf = run_pdf_fixture("inline-containers", |fonts| {
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse inline doc"))
      .page(PageOptions::A4)
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  for (name, fill) in [
    ("inline-block", "1 0 0 rg"),
    ("inline-flex", "0 1 0 rg"),
    ("nested inline-block", "1 1 0 rg"),
    ("float", "1 0 1 rg"),
  ] {
    assert!(haystack.contains(fill), "{name} never reached the page");
  }
  assert!(
    haystack.matches("Tj").count() + haystack.matches("TJ").count() > 8,
    "text inside the inline containers is missing"
  );
}

/// `alt=""` marks an image decorative: its content is an artifact and no
/// `Figure` element enters the structure tree. A non-empty `alt` still
/// produces a `Figure` that satisfies PDF/UA-1.
#[test]
fn decorative_image_artifact() {
  let doc = r#"<main style="display:flex;flex-direction:column;font-size:14px;color:#141414;">
    <h1>Images</h1>
    <img src="pixel" alt="" style="width:40px;height:40px;" />
    <img src="pixel" alt="a described pixel" style="width:40px;height:40px;" />
  </main>"#;
  let pdf = run_pdf_fixture("decorative-image-ua1", |fonts| {
    let buffer = ImageBuffer::from_rgba_bytes(vec![128; 4 * 4 * 4], 4, 4).expect("image buffer");

    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse image doc"))
      .images(HashMap::from([(
        "pixel".into(),
        ImageSource::Bitmap(Arc::new(buffer)),
      )]))
      .page(PageOptions::A4)
      .tagged(Tagging::Ua1)
      .lang(Some(takumi_core::style::Lang::parse("en").expect("lang")))
      .metadata(PdfMetadata {
        title: Some("Images".into()),
        ..Default::default()
      })
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  assert_eq!(
    haystack.matches("/S/Figure").count(),
    1,
    "decorative image must not produce a Figure element"
  );
}

#[test]
fn certificate() {
  run_pdf_fixture("certificate", |fonts| {
    PdfOptions::builder()
      .node(html_fixture("certificate.html"))
      .viewport(Viewport::new((1123, 794)))
      .fonts(fonts)
      .build()
  });
}

/// Headings across a forced page break become outline entries; anchors become
/// link annotations on the page owning their box. An `href="#id"` resolves to
/// a destination in the document, and one pointing at no element is dropped.
#[test]
fn report_links_outline() {
  let pdf = run_pdf_fixture("report-links-outline", |fonts| {
    PdfOptions::builder()
      .node(html_fixture("report.html"))
      .page(PageOptions::A4)
      .outline(true)
      .metadata(PdfMetadata {
        title: Some("Annual report".into()),
        description: Some("Fixture exercising metadata, links, and outline".into()),
        authors: vec!["Takumi".into()],
        keywords: vec!["report".into(), "fixture".into()],
        creator: Some("takumi-pdf fixtures".into()),
        creation_date: None,
        xmp: Vec::new(),
      })
      .fonts(fonts)
      .build()
  });

  // `inflated_text` inflates every deflated stream, so a substring check finds
  // what it is after wherever the object ended up.
  let haystack = inflated_text(&pdf);

  for needle in [
    "https://example.com/numbers",
    "https://example.com/data",
    "/Dest",
    // A percent-encoded fragment resolves to the id it decodes to.
    "(#raw%20data)",
    "/Outlines",
  ] {
    assert!(haystack.contains(needle), "missing {needle} in pdf");
  }

  assert!(
    !haystack.contains("#nowhere"),
    "a fragment matching no element still produced an annotation"
  );
}

/// Measuring at a page lays out at the full page width; counter hooks are
/// filled with three-digit numbers so a counter-only band measures its real
/// height.
#[test]
fn measure_band_at_page_width() {
  let fonts = fonts();
  let band = from_html(
    r#"<div style="display: flex; justify-content: center; font-size: 12px;">
      Page <span class="pageNumber"></span> of <span class="totalPages"></span>
    </div>"#,
    FromHtmlOptions::default(),
  )
  .expect("parse band");
  let size = measure(
    MeasureOptions::builder()
      .node(band)
      .page(PageOptions::A4)
      .fonts(&fonts)
      .build(),
  )
  .expect("measure band");

  assert_eq!(size.width, PageOptions::A4.width.floor());
  assert!(size.height >= 12.0, "band height {}", size.height);
}

/// Measuring reports the size the tree laid out at, not the size it was laid
/// out against. A box narrower than the page measures its own width.
#[test]
fn measure_reports_content_width() {
  let fonts = fonts();
  let node = || {
    from_html(
      r#"<div style="border: 1px solid #000; width: 100px; height: 100px;">A</div>"#,
      FromHtmlOptions::default(),
    )
    .expect("parse node")
  };
  let at_page = measure(
    MeasureOptions::builder()
      .node(node())
      .page(PageOptions::A4)
      .fonts(&fonts)
      .build(),
  )
  .expect("measure at page");

  assert_eq!((at_page.width, at_page.height), (100.0, 100.0));

  let at_viewport = measure(
    MeasureOptions::builder()
      .node(node())
      .viewport(Viewport::new((600, Some(400))))
      .fonts(&fonts)
      .build(),
  )
  .expect("measure at viewport");

  assert_eq!((at_viewport.width, at_viewport.height), (100.0, 100.0));
}

/// Without a page, measurement uses the viewport; omitting both is an error.
#[test]
fn measure_viewport_and_missing_viewport() {
  let fonts = fonts();
  let node = || {
    from_html(
      r#"<div style="font-size: 16px;">A line of wrapped text that needs several rows at a narrow width</div>"#,
      FromHtmlOptions::default(),
    )
    .expect("parse node")
  };
  let narrow = measure(
    MeasureOptions::builder()
      .node(node())
      .viewport(Viewport::new((120, None)))
      .fonts(&fonts)
      .build(),
  )
  .expect("measure at viewport");
  let wide = measure(
    MeasureOptions::builder()
      .node(node())
      .viewport(Viewport::new((600, None)))
      .fonts(&fonts)
      .build(),
  )
  .expect("measure at wide viewport");

  assert!(narrow.height > wide.height);
  assert!(
    measure(MeasureOptions::builder().node(node()).fonts(&fonts).build()).is_err(),
    "expected MissingViewport"
  );
}

/// Font programs that leave the plain TrueType path: CFF outlines become a
/// `CIDFontType0`, colour tables become `Type3` glyph procedures, and a
/// collection file carries several faces. Scripts that reorder or join while
/// shaping ride along, because their `ToUnicode` maps are what the `u` and `a`
/// levels require. Every level renders the same document, so CI's veraPDF step
/// validates each conformance claim the renderer can make.
#[test]
fn font_format_standards() {
  let mut fonts = Fonts::default();
  let mut families = Vec::new();

  for path in [
    "../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf",
    "../assets/fonts/cjk-locl-test/CJKLoclTest.woff2",
    "../assets/fonts/ubuntu/Ubuntu.ttc",
    "../assets/fonts/twemoji/TwemojiMozilla-colr.woff2",
    "../assets/fonts/sil/scheherazade-new-v17-arabic-regular.woff2",
    "../assets/fonts/noto-sans/noto-sans-devanagari-v30-devanagari-regular.woff2",
  ] {
    let data = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).expect("read test font");
    let registered = fonts
      .register(FontResource::new(data))
      .expect("load test font");

    families.push(registered.first().expect("registered family").name.clone());
  }

  let [base, cff, collection, colr, arabic, devanagari]: [String; 6] =
    families.try_into().expect("six families");
  let doc = format!(
    r#"<main style="display:flex;flex-direction:column;font-family:{base};font-size:16px;color:#141414;">
      <h1>Font formats</h1>
      <p lang="zh" style="font-family:{cff};">直 骨 今 海 真 令 説 器</p>
      <p style="font-family:{collection};">A face out of a TrueType collection</p>
      <p style="font-family:{colr};">Colour glyphs 🎉 🚀</p>
      <p lang="ar" style="font-family:{arabic};">نص عربي للتشكيل</p>
      <p lang="hi" style="font-family:{devanagari};">संयुक्ताक्षर क्षत्र</p>
    </main>"#
  );
  let metadata = || PdfMetadata {
    title: Some("Font formats".into()),
    creation_date: Some(PdfDate {
      year: 2026,
      month: 8,
      day: 7,
      hour: 0,
      minute: 0,
      second: 0,
    }),
    ..Default::default()
  };

  for (name, standard) in [
    ("2b", PdfStandard::A2b),
    ("2u", PdfStandard::A2u),
    ("2a", PdfStandard::A2a),
    ("3b", PdfStandard::A3b),
    ("3u", PdfStandard::A3u),
    ("3a", PdfStandard::A3a),
    ("4", PdfStandard::A4),
  ] {
    // PDF/UA-1 is PDF 1.7 only, so it cannot ride along with PDF/A-4.
    let tagging = if standard == PdfStandard::A4 {
      Tagging::On
    } else {
      Tagging::Ua1
    };
    let pdf = run_pdf_fixture_with(&format!("font-formats-pdfa-{name}"), &fonts, |fonts| {
      PdfOptions::builder()
        .node(from_html(&doc, FromHtmlOptions::default()).expect("parse font doc"))
        .page(PageOptions::A4)
        .standard(standard)
        .tagged(tagging)
        .lang(Some(takumi_core::style::Lang::parse("en").expect("lang")))
        .metadata(metadata())
        .fonts(fonts)
        .build()
    });
    let haystack = inflated_text(&pdf);

    for subtype in ["/CIDFontType0", "/CIDFontType2", "/Type3"] {
      assert!(
        haystack.contains(subtype),
        "missing {subtype} font in font-formats-pdfa-{name}"
      );
    }
    // A paragraph in another language says so, so a reader switches voice
    // rather than reading Arabic aloud in English.
    for lang in ["/Lang(zh)", "/Lang(ar)", "/Lang(hi)"] {
      assert!(
        haystack.contains(lang),
        "missing {lang} in font-formats-pdfa-{name}"
      );
    }
  }
}

/// An inline box carries its own language, and the text around it goes back to
/// the paragraph's when the box ends. Both sit on a path of their own: the box
/// is tagged where the inline run places it, not where a block would be, and
/// the owner reopens after it.
///
/// The box's own subtree is a separate matter. It renders through a nested
/// emitter that tags nothing at all, so the Hindi word inside it reaches the
/// page unmarked.
#[test]
fn inline_box_language() {
  let doc = r#"<main style="display:flex;flex-direction:column;font-size:14px;color:#141414;">
    <h1>Inline language</h1>
    <p lang="ar">before <span style="display:inline-block;" lang="hi">inside</span> after</p>
  </main>"#;
  let pdf = run_pdf_fixture("inline-box-lang-ua1", |fonts| {
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse inline lang doc"))
      .page(PageOptions::A4)
      .standard(PdfStandard::A3a)
      .tagged(Tagging::Ua1)
      .lang(Some(takumi_core::style::Lang::parse("en").expect("lang")))
      .metadata(PdfMetadata {
        title: Some("Inline language".into()),
        creation_date: Some(PdfDate {
          year: 2026,
          month: 8,
          day: 7,
          hour: 0,
          minute: 0,
          second: 0,
        }),
        ..Default::default()
      })
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  assert!(
    haystack.matches("/Lang(ar)").count() >= 2,
    "the paragraph's language stops at the inline box instead of resuming after it"
  );
  assert!(
    haystack.contains("/Lang(hi)"),
    "the inline box does not carry its own language"
  );
}

/// HTML numbers headings for looks, so a document can open at `h2` or jump
/// from `h1` to `h4`. PDF/UA rejects both, and rejects a list item without a
/// list around it. The structure tree renumbers by nesting depth and gives an
/// orphan item a list of its own. An empty heading is dropped without shifting
/// the ones that follow, and a heading whose text sits in child elements still
/// reaches the outline, which PDF/UA requires.
#[test]
fn heading_levels_and_orphan_list_item() {
  let doc = r#"<main style="display:flex;flex-direction:column;font-size:14px;color:#141414;">
    <h1></h1>
    <h2>Opens below h1</h2>
    <p>Body</p>
    <h4>Skips two levels</h4>
    <h3>Wrapped <strong>bold</strong> text</h3>
    <li>An item with no list</li>
  </main>"#;
  let pdf = run_pdf_fixture("heading-levels-ua1", |fonts| {
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse heading doc"))
      .page(PageOptions::A4)
      .standard(PdfStandard::A3a)
      .tagged(Tagging::Ua1)
      .lang(Some(takumi_core::style::Lang::parse("en").expect("lang")))
      .metadata(PdfMetadata {
        title: Some("Headings".into()),
        creation_date: Some(PdfDate {
          year: 2026,
          month: 8,
          day: 7,
          hour: 0,
          minute: 0,
          second: 0,
        }),
        ..Default::default()
      })
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  for name in ["/S/H1", "/S/H2", "/S/L", "/S/LI"] {
    assert!(haystack.contains(name), "missing {name} structure element");
  }
  assert!(
    !haystack.contains("/S/H4"),
    "heading levels reached the file unnormalized"
  );
  // The structure element carries the same text under `/T`, so the outline's
  // own key is what proves the entry reached the bookmarks.
  assert!(
    haystack.contains("/Title(Wrapped bold text)"),
    "heading with inline children missing from the outline"
  );
}

/// Distinct subsets embedded for a family. Each one is written as
/// `/BaseFont/ABCDEF+Family`, with one tag per instanced font.
fn embedded_subsets(haystack: &str, family: &str) -> usize {
  haystack
    .match_indices("/BaseFont/")
    .filter_map(|(index, marker)| {
      haystack[index + marker.len()..]
        .split(|c: char| !(c.is_ascii_alphanumeric() || "+-,#".contains(c)))
        .next()
    })
    .filter(|name| {
      name
        .split_once('+')
        .is_some_and(|(_, rest)| rest.starts_with(family))
    })
    .collect::<HashSet<_>>()
    .len()
}

/// A variable font must be embedded at the weight the run was shaped at, once
/// per weight. A face with no bold or italic of its own gets the same faux bold
/// and faux oblique the raster renderer applies, so weight survives across
/// scripts either way.
#[test]
fn font_weights() {
  let mut fonts = Fonts::default();
  let mut families = Vec::new();

  for path in [
    "../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf",
    "../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
    "../assets/fonts/sil/scheherazade-new-v17-arabic-regular.woff2",
    "../assets/fonts/noto-sans/noto-sans-devanagari-v30-devanagari-regular.woff2",
  ] {
    let data = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).expect("read test font");
    let registered = fonts
      .register(FontResource::new(data))
      .expect("load test font");

    families.push(registered.first().expect("registered family").name.clone());
  }

  let [latin, chinese, arabic, devanagari]: [String; 4] =
    families.try_into().expect("four families");
  let weights = [100, 300, 400, 600, 700, 900];
  let latin_rows = weights
    .iter()
    .map(|weight| {
      format!(r#"<p style="font-weight:{weight};">Weight {weight} · Variable axis</p>"#)
    })
    .collect::<String>();
  let chinese_rows = weights
    .iter()
    .map(|weight| {
      format!(r#"<p lang="zh-Hant" style="font-family:{chinese};font-weight:{weight};">字重 {weight} 的中文字樣</p>"#)
    })
    .collect::<String>();
  let doc = format!(
    r#"<main style="display:flex;flex-direction:column;font-family:{latin};font-size:16px;color:#141414;">
      <h1 style="font-weight:700;">Font weights</h1>
      {latin_rows}
      <p style="font-style:italic;">Oblique from the same variable face</p>
      {chinese_rows}
      <p lang="ar" style="font-family:{arabic};font-weight:700;">نص عربي عريض</p>
      <p lang="ar" style="font-family:{arabic};">نص عربي عادي</p>
      <p lang="hi" style="font-family:{devanagari};font-weight:700;">मोटा देवनागरी</p>
      <p lang="hi" style="font-family:{devanagari};font-style:italic;">तिरछा देवनागरी</p>
      <p lang="hi" style="font-family:{devanagari};font-weight:700;background-image:linear-gradient(90deg,#ff5f6d,#3a1c71);background-clip:text;color:transparent;">मोटा देवनागरी</p>
    </main>"#
  );

  let pdf = run_pdf_fixture_with("font-weights", &fonts, |fonts| {
    PdfOptions::builder()
      .node(from_html(&doc, FromHtmlOptions::default()).expect("parse weights doc"))
      .page(PageOptions::A4)
      .lang(Some(takumi_core::style::Lang::parse("en").expect("lang")))
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  // One embedded subset per weight: a single subset would mean they all fell
  // back to the variable font's default instance.
  assert_eq!(
    embedded_subsets(&haystack, "Archivo"),
    weights.len(),
    "variable latin face not embedded once per weight"
  );
  assert_eq!(
    embedded_subsets(&haystack, "NotoSansTC"),
    weights.len(),
    "variable chinese face not embedded once per weight"
  );
  // Faux bold strokes what it fills, which is text rendering mode 2.
  assert!(
    haystack.contains(" 2 Tr"),
    "no synthesized bold for the static faces"
  );
  // `background-clip: text` paints through the widened outline as well, so the
  // gradient reaches the stroke colour and not only the fill.
  assert!(
    haystack.contains("/Pattern CS"),
    "clip-text background missing from the synthesized bold outline"
  );
  // The clipped text is transparent, and a colour's alpha lives beside its
  // paint. A stroke built from the paint alone outlines it in solid black.
  assert!(
    stroke_alphas(&haystack).contains(&0.0),
    "faux bold outlines transparent text opaquely"
  );
}

/// A page counter renders in whatever counter style it is given, and a
/// non-decimal style reaches for characters no latin face carries. The counter
/// is generated rather than authored, so nothing in the document tells the
/// caller which font it will need.
#[test]
fn counter_style_needs_a_covering_font() {
  let rows: String = (1..=60)
    .map(|row| format!(r#"<div style="font-size:16px">Row {row}</div>"#))
    .collect();
  let doc = format!(r#"<div style="display:flex;flex-direction:column;width:100%">{rows}</div>"#);
  let footer = r#"<div style="display:flex;font-size:12px"><span class="totalPages trad-chinese-informal"></span></div>"#;
  let paged = |fonts: &Fonts| {
    render(
      PdfOptions::builder()
        .node(from_html(&doc, FromHtmlOptions::default()).expect("parse counter doc"))
        .footer(from_html(footer, FromHtmlOptions::default()).expect("parse counter footer"))
        .page(PageOptions {
          width: 400.0,
          height: 300.0,
          margin: PageMargins::uniform(24.0),
        })
        .fonts(fonts)
        .build(),
    )
  };
  let latin = {
    let mut latin = Fonts::default();
    let data = fs::read(
      Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf"),
    )
    .expect("read latin font");

    latin
      .register(FontResource::new(data))
      .expect("load latin font");
    latin
  };

  assert!(
    matches!(paged(&latin), Err(PdfError::MissingGlyphs(_))),
    "a chinese counter over a latin face should say what it cannot draw"
  );
  // The shared set carries a CJK face alongside the latin one.
  assert!(paged(&fonts()).is_ok(), "a covered chinese counter renders");
}

/// The stroking alphas a file sets. Read as numbers because `/CA 0` is a prefix
/// of `/CA 0.25`.
fn stroke_alphas(haystack: &str) -> Vec<f32> {
  haystack
    .match_indices("/CA ")
    .filter_map(|(at, marker)| {
      haystack[at + marker.len()..]
        .split(|character: char| !matches!(character, '0'..='9' | '.'))
        .next()?
        .parse()
        .ok()
    })
    .collect()
}

/// Registers both test fonts as coverage subsets of one logical family, ranked the way
/// their declared `unicode-range` would rank them.
fn ranked_subset_fonts() -> Fonts {
  let mut fonts = Fonts::default();

  for (path, name, rank) in [
    (
      "tests/fonts/noto-sans-tc-caps.subset.ttf",
      "Grouped cjk",
      0x4e00,
    ),
    (
      "../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf",
      "Grouped latin",
      0,
    ),
  ] {
    let data = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).expect("read test font");

    fonts
      .register(
        FontResource::new(data)
          .override_info(FontOverride {
            family_name: Some(name.into()),
            ..Default::default()
          })
          .subset_of("Grouped")
          .subset_rank(rank),
      )
      .expect("load test font");
  }
  fonts
}

/// A subset whose `cmap` reaches past the range it was cut for must not steal codepoints
/// from the subset that declares them. `Grouped cjk` also encodes the ASCII space and the
/// capitals, and sorts first by name, so without the rank it takes those and leaves the
/// lowercase to `Grouped latin` — one text object per fragment, each repositioned from
/// scratch. Extractors rebuild words from glyph geometry, and that is the shape they read
/// wrong.
#[test]
fn a_ranked_subset_group_keeps_a_latin_run_whole() {
  let fonts = ranked_subset_fonts();
  let doc = r#"<main style="font-family:Grouped;font-size:16px;">
    <p>Average App Rating</p>
  </main>"#;
  let pdf = render(
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse the doc"))
      .page(PageOptions::A4)
      .fonts(&fonts)
      .build(),
  )
  .expect("render the ranked group");

  let shows = content_lines(&pdf)
    .filter(|line| line.ends_with(b"TJ") || line.ends_with(b"Tj"))
    .count();

  assert_eq!(shows, 1, "the run was split across {shows} text objects");
}

/// Encoded bitmaps reach the PDF as the bytes they came in as: a JPEG embeds
/// as a `DCTDecode` stream rather than a re-encode of its pixels.
#[test]
fn encoded_bitmaps_embed_their_own_bytes() {
  const JPEG: &[u8] = include_bytes!("images/checker.jpg");
  const WEBP: &[u8] = include_bytes!("images/checker.webp");

  let cache = ResourceCache::new(1 << 20);
  let images: HashMap<Arc<str>, ImageSource> = [("jpeg", JPEG), ("webp", WEBP)]
    .into_iter()
    .map(|(name, bytes)| {
      let source = cache
        .get_or_decode(bytes, ImageCacheMode::Auto)
        .expect("decode test image");

      (name.into(), source)
    })
    .collect();
  let source = r##"<div style="display: flex; column-gap: 8px; padding: 8px;">
      <img src="jpeg" style="width: 32px; height: 32px;" />
      <img src="webp" style="width: 32px; height: 32px;" />
    </div>"##;
  let pdf = run_pdf_fixture("encoded-bitmaps", |fonts| {
    PdfOptions::builder()
      .node(from_html(source, FromHtmlOptions::default()).expect("parse the fixture"))
      .viewport(Viewport::new((120, 48)))
      .images(images.clone())
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  assert_eq!(
    haystack.matches("/Subtype/Image").count(),
    2,
    "expected an image XObject per source"
  );
  assert!(
    haystack.contains("/DCTDecode"),
    "expected the JPEG to embed as a JPEG stream"
  );
  assert!(
    find(&pdf, JPEG).is_some(),
    "expected the original JPEG bytes in the PDF"
  );
}

/// `blur()` has no PDF equivalent. Dropping it would print a page that quietly
/// disagrees with the stylesheet, so the render stops and names the function.
#[test]
fn an_unsupported_filter_stops_the_render() {
  let doc =
    r#"<div style="filter: blur(4px); width: 40px; height: 40px; background: #000;"></div>"#;
  let error = render(
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse the doc"))
      .viewport(Viewport::new((80, 80)))
      .fonts(&fonts())
      .build(),
  )
  .expect_err("blur() should stop the render");

  assert!(
    matches!(&error, PdfError::UnsupportedFilter(filter) if filter == "blur(4px)"),
    "unexpected error: {error:?}"
  );
}

/// An image whose bytes will not decode leaves a hole where the page expects a
/// picture. The render stops and names the source.
#[test]
fn an_undecodable_image_stops_the_render() {
  let mut bytes = ImageBuffer::from_rgba_bytes(vec![255; 8 * 8 * 4], 8, 8)
    .expect("an 8x8 buffer")
    .encode_png()
    .expect("encode the png");
  // The IHDR still reports 8x8; the compressed pixels no longer inflate.
  let idat = find(&bytes, b"IDAT").expect("an IDAT chunk") + 8;

  bytes[idat..].fill(0);

  let cache = ResourceCache::new(1 << 20);
  let source = cache
    .get_or_decode(&bytes, ImageCacheMode::Auto)
    .expect("the png header still parses");
  let doc = r#"<img src="broken" style="filter: grayscale(1); width: 32px; height: 32px;" />"#;
  let error = render(
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse the doc"))
      .viewport(Viewport::new((48, 48)))
      .images(HashMap::from([("broken".into(), source)]))
      .fonts(&fonts())
      .build(),
  )
  .expect_err("a broken image should stop the render");

  assert!(
    matches!(&error, PdfError::UndrawableImage(reason) if reason.starts_with("broken:")),
    "unexpected error: {error:?}"
  );
}

/// A `fixed` box the initial containing block holds repeats on every page, laid
/// out against the page area rather than the content column: a watermark, which
/// is what the property is for in print. Tagging is on, so the run also covers
/// a repeated link against the tag tree, which only knows the content.
#[test]
fn a_fixed_box_repeats_on_every_page() {
  let doc = r#"<main>
      <div style="position: fixed; inset: 0; display: flex; align-items: center; justify-content: center;">
        <span style="font-size: 48px;">DRAFT</span>
      </div>
      <div style="position: fixed; top: 20px; left: 30px; width: 40px; height: 40px; background: #000;"></div>
      <a href="https://takumi.kane.tw" style="position: fixed; bottom: 10px; left: 10px;">source</a>
      <div style="position: fixed; top: 200px; left: 200px; width: 50px; height: 50px; z-index: -1; background: #eee;"></div>
      <p style="height: 900px;">first</p>
      <p style="height: 900px;">second</p>
    </main>"#;
  let pdf = run_pdf_fixture("fixed-repeats-per-page", |fonts| {
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse the doc"))
      .page(PageOptions::A4)
      .tagged(Tagging::On)
      .fonts(fonts)
      .build()
  });
  let haystack = inflated_text(&pdf);

  assert_eq!(
    haystack.matches("/Count 2").count(),
    1,
    "expected a two-page document"
  );
  assert_eq!(
    content_lines(&pdf)
      .filter(|line| line.ends_with(b"TJ") || line.ends_with(b"Tj"))
      .count(),
    6,
    "expected both fixed boxes on both pages, next to each page's own text"
  );
  assert_eq!(
    haystack.matches("/Subtype/Link").count(),
    2,
    "expected the fixed link to be clickable on both pages"
  );

  // Every fixed box paints under one page-space transform, so the operands are
  // page-area pixels: the corner box at its own insets, the watermark centered.
  let corners: Vec<[f32; 4]> = content_lines(&pdf)
    .filter_map(|line| operands(&line, "re"))
    .filter(|rect| rect.len() == 4)
    .map(|rect| [rect[0], rect[1], rect[2], rect[3]])
    .filter(|rect| rect[2] == 40.0 && rect[3] == 40.0)
    .collect();

  assert_eq!(
    corners,
    [[30.0, 20.0, 40.0, 40.0]; 2],
    "expected the offset box at its own insets on both pages"
  );

  let lines: Vec<Vec<u8>> = content_lines(&pdf).collect();
  let under = lines
    .iter()
    .position(|line| find(line, b"200 200 50 50 re").is_some())
    .expect("the z-index: -1 box");
  let first_text = lines
    .iter()
    .position(|line| line.ends_with(b"TJ") || line.ends_with(b"Tj"))
    .expect("some text");

  assert!(
    under < first_text,
    "expected a negative z-index box to paint under the content"
  );

  let watermarks: Vec<(f32, f32)> = content_lines(&pdf)
    .filter(|line| find(line, b"/f0 48 Tf").is_some())
    .filter_map(|line| operands(&line, "Tm"))
    .filter(|matrix| matrix.len() == 6)
    .map(|matrix| (matrix[4], matrix[5]))
    .collect();

  assert_eq!(watermarks.len(), 2, "expected a watermark on both pages");
  assert_eq!(
    watermarks[0], watermarks[1],
    "expected the watermark at the same place on both pages"
  );

  let (_, baseline) = watermarks[0];
  let footer = content_lines(&pdf)
    .filter_map(|line| operands(&line, "Tm"))
    .filter(|matrix| matrix.len() == 6)
    .map(|matrix| matrix[5])
    .fold(0.0_f32, f32::max);

  assert!(
    baseline > 100.0 && baseline < footer - 100.0,
    "expected the watermark centered in the page area, not pinned to an edge: {baseline} of {footer}"
  );
}

/// The numbers preceding a content-stream operator, as the operator's operands.
fn operands(line: &[u8], operator: &str) -> Option<Vec<f32>> {
  let text = std::str::from_utf8(line).ok()?;
  let (before, _) = text.rsplit_once(&format!(" {operator}"))?;

  Some(
    before
      .split_whitespace()
      .rev()
      .map_while(|token| token.parse::<f32>().ok())
      .collect::<Vec<_>>()
      .into_iter()
      .rev()
      .collect(),
  )
}

/// The paper sits under everything, including a box the content would
/// otherwise cover: paper, then the negative `z-index` watermark, then the
/// text.
#[test]
fn the_paper_paints_under_a_repeated_box() {
  let doc = r#"<main>
      <div style="position: fixed; top: 100px; left: 100px; width: 60px; height: 60px; z-index: -1; background: #123456;"></div>
      <p style="height: 900px;">first</p>
      <p style="height: 900px;">second</p>
    </main>"#;
  let pdf = run_pdf_fixture("page-background", |fonts| {
    PdfOptions::builder()
      .node(from_html(doc, FromHtmlOptions::default()).expect("parse the doc"))
      .page(PageOptions::A4)
      .background_color(Color([239, 231, 213, 255]))
      .fonts(fonts)
      .build()
  });
  let lines: Vec<Vec<u8>> = content_lines(&pdf).collect();
  let paper = lines
    .iter()
    .position(|line| find(line, b"0.9373 0.9059 0.8353 rg").is_some())
    .expect("the paper fill");
  let watermark = lines
    .iter()
    .position(|line| find(line, b"100 100 60 60 re").is_some())
    .expect("the repeated box");
  let text = lines
    .iter()
    .position(|line| line.ends_with(b"TJ") || line.ends_with(b"Tj"))
    .expect("some text");

  assert!(
    paper < watermark && watermark < text,
    "expected paper, then the box, then the text: {paper} {watermark} {text}"
  );
}
