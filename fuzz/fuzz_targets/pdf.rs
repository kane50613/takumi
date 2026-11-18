//! Parses arbitrary text as HTML and prints it to a PDF, with the CSS it holds.
#![no_main]

use std::sync::LazyLock;

use libfuzzer_sys::fuzz_target;
use takumi::prelude::*;
use takumi_pdf::PdfOptions;

static FONTS: LazyLock<Fonts> = LazyLock::new(|| {
  let mut fonts = Fonts::default();
  let _ = fonts.register(FontResource::new(
    &include_bytes!("../../assets/fonts/geist/geist-latin-wght-300-800.ttf")[..],
  ));
  fonts
});

fuzz_target!(|html: &str| {
  let Ok(parsed) = takumi::parse_html(html, FromHtmlOptions::default()) else {
    return;
  };
  let stylesheet = StyleSheet::parse(&parsed.css.join("\n")).unwrap_or_default();
  let options = PdfOptions::builder()
    .viewport(Viewport::new((96, 64)))
    .node(parsed.node)
    .fonts(&FONTS)
    .stylesheet(stylesheet.into())
    .build();

  let _ = takumi_pdf::render(options);
});
