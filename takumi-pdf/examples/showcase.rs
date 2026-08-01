//! Renders the showcase PDFs (an invoice and a certificate) from the crate's
//! HTML fixtures in `fixtures/`.
//!
//! ```sh
//! cargo run -p takumi-pdf --example showcase
//! ```

use std::{fs, path::Path};

use takumi_core::{Fonts, layout::node::Node, resources::font::FontResource, viewport::Viewport};
use takumi_html::{FromHtmlOptions, from_html};
use takumi_pdf::{PageOptions, PdfOptions, render};

fn main() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR"));
  let mut fonts = Fonts::default();
  let data = fs::read(root.join("../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf"))
    .expect("read font");

  fonts
    .register(FontResource::new(data))
    .expect("register font");

  let invoice = render(
    PdfOptions::builder()
      .node(fixture(root, "invoice.html"))
      .viewport(Viewport::new((794, 1123)))
      .page(PageOptions::a4().with_margin(36.0))
      .footer(fixture(root, "invoice-footer.html"))
      .fonts(&fonts)
      .build(),
  )
  .expect("render invoice");

  fs::write(root.join("../target/showcase-invoice.pdf"), invoice).expect("write invoice");

  let certificate = render(
    PdfOptions::builder()
      .node(fixture(root, "certificate.html"))
      .viewport(Viewport::new((1123, 794)))
      .fonts(&fonts)
      .build(),
  )
  .expect("render certificate");

  fs::write(root.join("../target/showcase-certificate.pdf"), certificate)
    .expect("write certificate");
  println!("wrote target/showcase-invoice.pdf and target/showcase-certificate.pdf");
}

fn fixture(root: &Path, name: &str) -> Node {
  let source = fs::read_to_string(root.join("fixtures").join(name)).expect("read fixture");

  from_html(&source, FromHtmlOptions::default()).expect("parse fixture")
}
