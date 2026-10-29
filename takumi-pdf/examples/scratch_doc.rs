use std::{fs, time::Instant};

use takumi_core::html::{FromHtmlOptions, from_html};
use takumi_core::{Fonts, resources::font::FontResource};
use takumi_pdf::{PageMargins, PageOptions, PdfOptions, render};

fn main() {
  let path = std::env::args().nth(1).expect("html path");
  let mut fonts = Fonts::default();
  let archivo: &[u8] =
    include_bytes!("../../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf");
  fonts.register(FontResource::new(archivo.to_vec())).unwrap();
  let html = fs::read_to_string(&path).unwrap();
  let start = Instant::now();
  let node = from_html(&html, FromHtmlOptions::default()).unwrap();
  println!("parse {:?}", start.elapsed());
  let start = Instant::now();
  let pdf = render(
    PdfOptions::builder()
      .node(node)
      .page(PageOptions {
        width: 816.0,
        height: 1056.0,
        margin: PageMargins::uniform(24.0),
      })
      .fonts(&fonts)
      .build(),
  )
  .unwrap();
  let elapsed = start.elapsed();
  let text = String::from_utf8_lossy(&pdf);
  let s = text.find("/Type/Pages/Count ").unwrap() + "/Type/Pages/Count ".len();
  let pages: usize = text[s..]
    .chars()
    .take_while(char::is_ascii_digit)
    .collect::<String>()
    .parse()
    .unwrap();
  println!(
    "render {pages} pages in {elapsed:?} ({:?}/page), {} bytes",
    elapsed / pages as u32,
    pdf.len()
  );
  fs::write(format!("{path}.pdf"), &pdf).unwrap();
  println!(
    "{}",
    fs::read_to_string("/proc/self/status")
      .unwrap()
      .lines()
      .find(|l| l.starts_with("VmHWM"))
      .unwrap_or("")
  );
}
