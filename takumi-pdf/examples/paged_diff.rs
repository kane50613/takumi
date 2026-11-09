//! Renders HTML files to PDFs on a fixed page for differential tests:
//! paged_diff <width> <height> <in.html> <out.pdf> [<in.html> <out.pdf> ...]

use std::{fs, path::Path, sync::Arc};

use takumi_core::{
  Fonts,
  html::{FromHtmlOptions, parse_html},
  resources::font::FontResource,
  style::StyleSheet,
};
use takumi_pdf::{PageMargins, PageOptions, PdfOptions, render};

fn main() {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let width: f32 = args[0].parse().expect("width");
  let height: f32 = args[1].parse().expect("height");
  let mut fonts = Fonts::default();
  let font = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../assets/fonts/inter/inter-latin-opsz-wght-normal.woff2");

  fonts
    .register(FontResource::new(fs::read(font).expect("read font")))
    .expect("load font");

  for pair in args[2..].chunks(2) {
    let html = fs::read_to_string(&pair[0]).expect("read html");
    let result = parse_html(&html, FromHtmlOptions::default())
      .map_err(|error| error.to_string())
      .and_then(|parsed| {
        render(
          PdfOptions::builder()
            .node(parsed.node)
            .stylesheet(Arc::new(StyleSheet::parse_loosy(&parsed.css.join("\n"))))
            .page(PageOptions {
              width,
              height,
              margin: PageMargins::uniform(0.0),
            })
            .fonts(&fonts)
            .build(),
        )
        .map_err(|error| error.to_string())
      });

    match result {
      Ok(pdf) => fs::write(&pair[1], pdf).expect("write pdf"),
      Err(error) => eprintln!("{}: {error}", pair[0]),
    }
  }
}
