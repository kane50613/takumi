//! PDF byte-golden fixtures.
//!
//! Every case renders twice (guarding against nondeterministic output) and
//! writes the result to `tests/fixtures-generated/<name>.pdf`. The goldens are
//! committed; CI's dirty-tree check catches drift, so a changed .pdf in `git
//! diff` is a real rendering change to review.

use std::{collections::HashMap, fs, path::Path, sync::Arc};

use takumi_core::{
  Fonts,
  layout::node::{ImageData, ImageSourceInput, Node, RgbaImage},
  resources::{font::FontResource, image::ImageSource, image_buffer::ImageBuffer},
  style::{
    BreakBetween, Color, ColorInput, Display, FlexDirection, FontSize, Length::*, ObjectFit, Style,
    StyleDeclaration,
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
  let fonts = fonts();
  let first = render(build(&fonts)).expect("render pdf fixture");
  let second = render(build(&fonts)).expect("render pdf fixture again");

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
  let haystack = String::from_utf8_lossy(&list);

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
  let haystack = String::from_utf8_lossy(&pdf);

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
/// link annotations on the page owning their box.
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

  // Annotation dictionaries and the outline root serialize uncompressed, so
  // substring checks hold; revisit with a PDF parser if that changes.
  let haystack = String::from_utf8_lossy(&pdf);

  for needle in [
    "https://example.com/numbers",
    "https://example.com/data",
    "/Outlines",
  ] {
    assert!(haystack.contains(needle), "missing {needle} in pdf");
  }
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
