//! PDF byte-golden fixtures.
//!
//! Every case renders twice (guarding against nondeterministic output) and
//! writes the result to `tests/fixtures-generated/<name>.pdf`. The goldens are
//! committed; CI's dirty-tree check catches drift, so a changed .pdf in `git
//! diff` is a real rendering change to review.

use std::{fs, path::Path};

use takumi_core::{
  Fonts,
  layout::node::{ImageData, ImageSourceInput, Node, RgbaImage},
  resources::font::FontResource,
  style::{
    BreakBetween, Color, ColorInput, Display, FlexDirection, FontSize, Length::*, ObjectFit, Style,
    StyleDeclaration,
  },
  viewport::Viewport,
};
use takumi_html::{FromHtmlOptions, from_html};
use takumi_pdf::{PageMargins, PageOptions, PdfOptions, render};

fn fonts() -> Fonts {
  let mut fonts = Fonts::default();
  let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf");
  let data = fs::read(&path).expect("read test font");

  fonts
    .register(FontResource::new(data))
    .expect("load test font");
  fonts
}

fn html_fixture(name: &str) -> Node {
  let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("fixtures")
    .join(name);
  let source = fs::read_to_string(path).expect("read html fixture");

  from_html(&source, FromHtmlOptions::default()).expect("parse html fixture")
}

/// Renders the case twice, asserts determinism, and writes the golden.
fn run_pdf_fixture(name: &str, build: impl Fn(&Fonts) -> PdfOptions<'_>) {
  let fonts = fonts();
  let first = render(build(&fonts)).expect("render pdf fixture");
  let second = render(build(&fonts)).expect("render pdf fixture again");

  assert_eq!(first, second, "nondeterministic pdf output for {name}");
  assert!(first.starts_with(b"%PDF-"), "not a pdf: {name}");

  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures-generated");

  fs::create_dir_all(&dir).expect("create golden directory");
  fs::write(dir.join(format!("{name}.pdf")), &first).expect("write pdf golden");
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
      .footer(text("Page {page} of {pages}", 12.0))
      .fonts(fonts)
      .build()
  });
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
            <div>Page {{page}} of {{pages}}</div>
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
