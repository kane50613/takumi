//! Profiling target: renders the invoice fixture repeatedly.
//! `samply record target/bench/examples/profile` (build with `--profile bench`
//! for symbols).

use std::{fs, hint::black_box, path::Path, time::Instant};

use takumi_core::{Fonts, resources::font::FontResource};
use takumi_html::{FromHtmlOptions, from_html};
use takumi_pdf::{PageOptions, PdfOptions, render};

fn main() {
  let mut fonts = Fonts::default();

  for path in [
    "../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf",
    "../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
  ] {
    let data = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).expect("read font");

    fonts.register(FontResource::new(data)).expect("load font");
  }
  let source =
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/invoice.html"))
      .expect("read fixture");
  let iterations: usize = std::env::args()
    .nth(1)
    .and_then(|n| n.parse().ok())
    .unwrap_or(200);
  let start = Instant::now();

  for _ in 0..iterations {
    let node = from_html(&source, FromHtmlOptions::default()).expect("parse fixture");
    let pdf = render(
      PdfOptions::builder()
        .node(node)
        .page(PageOptions::A4.with_margin(36.0))
        .fonts(&fonts)
        .build(),
    )
    .expect("render");

    black_box(pdf);
  }
  let elapsed = start.elapsed();

  println!(
    "{iterations} renders in {elapsed:?} ({:?}/render)",
    elapsed / iterations as u32
  );
}
