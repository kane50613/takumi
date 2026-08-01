//! End-to-end POC check: render a styled node tree to PDF and assert the file
//! is structurally a PDF with an embedded font program.

use std::{fs, path::Path};

use takumi_core::{
  Fonts,
  layout::node::Node,
  resources::font::FontResource,
  style::{
    Color, ColorInput, Display, FlexDirection, FontSize, Length::*, Style, StyleDeclaration,
  },
  viewport::Viewport,
};
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

#[test]
fn renders_text_pdf() {
  let text = Node::text("Hello PDF from Takumi".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        20, 20, 60, 255,
      ]))))
      .with(StyleDeclaration::font_size(FontSize::Length(Px(32.0)))),
  );
  let node = Node::container([text]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([235, 244, 255, 255]),
      ))),
  );

  let fonts = fonts();
  let pdf = render(
    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((600, 300)))
      .fonts(&fonts)
      .build(),
  )
  .expect("render pdf");

  assert!(pdf.starts_with(b"%PDF-"), "not a PDF header");
  assert!(pdf.len() > 1_000, "suspiciously small: {} bytes", pdf.len());

  let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/takumi-pdf-poc.pdf");
  fs::write(&out, &pdf).expect("write poc pdf");
}

#[test]
fn renders_multi_page_pdf() {
  let lines: Vec<Node> = (1..=40)
    .map(|i| {
      Node::text(format!("Line {i} of the paginated report body")).with_style(
        Style::default()
          .with(StyleDeclaration::color(ColorInput::Value(Color([
            30, 30, 30, 255,
          ]))))
          .with(StyleDeclaration::font_size(FontSize::Length(Px(16.0)))),
      )
    })
    .collect();
  let node = Node::container(lines).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      ))),
  );

  let fonts = fonts();
  let pdf = render(
    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((400, 200)))
      .page(PageOptions {
        width: 400.0,
        height: 300.0,
        margin: 24.0,
      })
      .fonts(&fonts)
      .build(),
  )
  .expect("render paged pdf");

  assert!(pdf.starts_with(b"%PDF-"));

  let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/takumi-pdf-poc-pages.pdf");
  fs::write(&out, &pdf).expect("write paged poc pdf");
}
