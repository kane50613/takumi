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
    BreakBetween, Color, ColorInput, Display, FlexDirection, FontSize, Length::*, Style,
    StyleDeclaration,
  },
  viewport::Viewport,
};
use takumi_html::{FromHtmlOptions, from_html};
use takumi_pdf::{PageOptions, PdfOptions, render};

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

  fs::create_dir_all("tests/fixtures-generated").ok();
  fs::write(format!("tests/fixtures-generated/{name}.pdf"), &first).expect("write pdf golden");
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
        margin: 24.0,
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
        margin: 24.0,
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
        margin: 24.0,
      })
      .fonts(fonts)
      .build()
  });
}

#[test]
fn image_object_fit() {
  run_pdf_fixture("image-object-fit", |fonts| {
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
    let image = Node::image(ImageData {
      src: ImageSourceInput::Rgba(RgbaImage::new(pixels, 8, 8, false).expect("rgba image")),
      width: Some(96.0),
      height: Some(96.0),
    });

    PdfOptions::builder()
      .node(
        Node::container([image]).with_style(
          Style::default()
            .with(StyleDeclaration::display(Display::Flex))
            .with(StyleDeclaration::width(Percentage(100.0)))
            .with(StyleDeclaration::height(Percentage(100.0)))
            .with(StyleDeclaration::padding_top(Px(16.0)))
            .with(StyleDeclaration::padding_left(Px(16.0))),
        ),
      )
      .viewport(Viewport::new((160, 160)))
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
