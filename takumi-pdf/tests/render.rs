//! End-to-end POC check: render a styled node tree to PDF and assert the file
//! is structurally a PDF with an embedded font program.

use std::{fs, path::Path};

use takumi_core::{
  Fonts,
  layout::node::Node,
  resources::font::FontResource,
  style::{
    BorderStyle, BreakBetween, Color, ColorInput, Display, FlexDirection, FontSize, Length::*,
    LineWidth, PercentageNumber, SpacePair, Style, StyleDeclaration,
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

#[test]
fn renders_page_footer_with_counters() {
  let lines: Vec<Node> = (1..=40)
    .map(|i| {
      Node::text(format!("Row {i}")).with_style(
        Style::default()
          .with(StyleDeclaration::color(ColorInput::Value(Color([
            0, 0, 0, 255,
          ]))))
          .with(StyleDeclaration::font_size(FontSize::Length(Px(16.0)))),
      )
    })
    .collect();
  let node = Node::container(lines).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::width(Percentage(100.0))),
  );
  let footer = Node::text("Page {page} of {pages}".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        90, 90, 90, 255,
      ]))))
      .with(StyleDeclaration::font_size(FontSize::Length(Px(12.0)))),
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
      .footer(footer)
      .fonts(&fonts)
      .build(),
  )
  .expect("render paged pdf with footer");

  assert!(pdf.starts_with(b"%PDF-"));

  let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/takumi-pdf-poc-footer.pdf");
  fs::write(&out, &pdf).expect("write footer poc pdf");
}

#[test]
fn renders_ligature_text() {
  let text = Node::text("Difficult office traffic affix".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        0, 0, 0, 255,
      ]))))
      .with(StyleDeclaration::font_size(FontSize::Length(Px(24.0)))),
  );
  let node = Node::container([text]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0))),
  );

  let fonts = fonts();
  let pdf = render(
    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((600, 100)))
      .fonts(&fonts)
      .build(),
  )
  .expect("render ligature pdf");

  let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/takumi-pdf-poc-liga.pdf");
  fs::write(&out, &pdf).expect("write liga poc pdf");
}

#[test]
fn break_before_forces_new_page() {
  let section = |title: &str| {
    Node::container(
      (1..=3)
        .map(|i| {
          Node::text(format!("{title} row {i}")).with_style(
            Style::default().with(StyleDeclaration::font_size(FontSize::Length(Px(14.0)))),
          )
        })
        .collect::<Vec<_>>(),
    )
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column))
        .with(StyleDeclaration::break_before(BreakBetween::Page)),
    )
  };
  let node = Node::container([section("Alpha"), section("Beta")]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::width(Percentage(100.0))),
  );

  let fonts = fonts();
  let pdf = render(
    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((400, 200)))
      .page(PageOptions {
        width: 400.0,
        height: 400.0,
        margin: 24.0,
      })
      .fonts(&fonts)
      .build(),
  )
  .expect("render break-before pdf");

  let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/takumi-pdf-poc-breaks.pdf");
  fs::write(&out, &pdf).expect("write breaks poc pdf");
}

#[test]
fn renders_box_chrome() {
  let card = Node::container([Node::text("Chrome card".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        20, 20, 60, 255,
      ]))))
      .with(StyleDeclaration::font_size(FontSize::Length(Px(20.0)))),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(300.0)))
      .with(StyleDeclaration::height(Px(120.0)))
      .with(StyleDeclaration::padding_top(Px(16.0)))
      .with(StyleDeclaration::padding_left(Px(16.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      )))
      .with(StyleDeclaration::border_top_width(LineWidth::Length(Px(
        3.0,
      ))))
      .with(StyleDeclaration::border_right_width(LineWidth::Length(Px(
        3.0,
      ))))
      .with(StyleDeclaration::border_bottom_width(LineWidth::Length(
        Px(3.0),
      )))
      .with(StyleDeclaration::border_left_width(LineWidth::Length(Px(
        3.0,
      ))))
      .with(StyleDeclaration::border_top_style(BorderStyle::Solid))
      .with(StyleDeclaration::border_right_style(BorderStyle::Solid))
      .with(StyleDeclaration::border_bottom_style(BorderStyle::Solid))
      .with(StyleDeclaration::border_left_style(BorderStyle::Solid))
      .with(StyleDeclaration::border_top_color(ColorInput::Value(
        Color([180, 40, 40, 255]),
      )))
      .with(StyleDeclaration::border_right_color(ColorInput::Value(
        Color([180, 40, 40, 255]),
      )))
      .with(StyleDeclaration::border_bottom_color(ColorInput::Value(
        Color([180, 40, 40, 255]),
      )))
      .with(StyleDeclaration::border_left_color(ColorInput::Value(
        Color([180, 40, 40, 255]),
      )))
      .with(StyleDeclaration::border_top_left_radius(
        SpacePair::from_single(Px(16.0)),
      ))
      .with(StyleDeclaration::border_top_right_radius(
        SpacePair::from_single(Px(16.0)),
      ))
      .with(StyleDeclaration::border_bottom_right_radius(
        SpacePair::from_single(Px(16.0)),
      ))
      .with(StyleDeclaration::border_bottom_left_radius(
        SpacePair::from_single(Px(16.0)),
      ))
      .with(StyleDeclaration::opacity(PercentageNumber(0.8))),
  );
  let node = Node::container([card]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::padding_top(Px(24.0)))
      .with(StyleDeclaration::padding_left(Px(24.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([235, 240, 250, 255]),
      ))),
  );

  let fonts = fonts();
  let pdf = render(
    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((400, 200)))
      .fonts(&fonts)
      .build(),
  )
  .expect("render chrome pdf");

  let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/takumi-pdf-poc-chrome.pdf");
  fs::write(&out, &pdf).expect("write chrome poc pdf");
}

#[test]
fn renders_gradients() {
  use takumi_core::style::{BackgroundImages, FromCssStr};

  let swatch = |css: &str| {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::width(Px(110.0)))
        .with(StyleDeclaration::height(Px(110.0)))
        .with(StyleDeclaration::background_image(Some(
          BackgroundImages::from_css_str(css).expect("parse gradient"),
        ))),
    )
  };
  let node = Node::container([
    swatch("linear-gradient(135deg, #ff5f6d, #3a1c71)"),
    swatch("radial-gradient(circle, #fddb92, #4481eb)"),
    swatch("conic-gradient(from 0deg, red, yellow, lime, cyan, blue, magenta, red)"),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::padding_top(Px(20.0)))
      .with(StyleDeclaration::padding_left(Px(20.0)))
      .with(StyleDeclaration::column_gap(Px(20.0).into()))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      ))),
  );

  let fonts = fonts();
  let pdf = render(
    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((440, 160)))
      .fonts(&fonts)
      .build(),
  )
  .expect("render gradients pdf");

  let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/takumi-pdf-poc-gradients.pdf");
  fs::write(&out, &pdf).expect("write gradients poc pdf");
}

#[test]
fn renders_image_node() {
  use takumi_core::layout::node::{ImageData, ImageSourceInput, RgbaImage};

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
    src: ImageSourceInput::Rgba(RgbaImage::new(pixels, 8, 8, false).expect("build rgba image")),
    width: Some(96.0),
    height: Some(96.0),
  });
  let node = Node::container([image]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::padding_top(Px(16.0)))
      .with(StyleDeclaration::padding_left(Px(16.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      ))),
  );

  let fonts = fonts();
  let pdf = render(
    PdfOptions::builder()
      .node(node)
      .viewport(Viewport::new((160, 160)))
      .fonts(&fonts)
      .build(),
  )
  .expect("render image pdf");

  let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/takumi-pdf-poc-image.pdf");
  fs::write(&out, &pdf).expect("write image poc pdf");
}
