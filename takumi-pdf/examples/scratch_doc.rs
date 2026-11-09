use std::{error::Error, fs, time::Instant};

use takumi_core::html::{FromHtmlOptions, from_html};
use takumi_core::{Fonts, resources::font::FontResource};
use takumi_pdf::{PageMargins, PageOptions, PdfOptions, render};

fn main() -> Result<(), Box<dyn Error>> {
  let path = std::env::args().nth(1).expect("html path");
  let mut fonts = Fonts::default();
  let archivo: &[u8] =
    include_bytes!("../../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf");
  fonts.register(FontResource::new(archivo.to_vec()))?;
  let html = fs::read_to_string(&path)?;
  let start = Instant::now();
  let node = from_html(&html, FromHtmlOptions::default())?;
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
  )?;
  let elapsed = start.elapsed();
  let text = String::from_utf8_lossy(&pdf);
  let s = text
    .find("/Type/Pages/Count ")
    .ok_or("the PDF has no page count")?
    + "/Type/Pages/Count ".len();
  let pages: usize = text[s..]
    .chars()
    .take_while(char::is_ascii_digit)
    .collect::<String>()
    .parse()?;
  println!(
    "render {pages} pages in {elapsed:?} ({:?}/page), {} bytes",
    elapsed / pages as u32,
    pdf.len()
  );
  fs::write(format!("{path}.pdf"), &pdf)?;
  println!(
    "{}",
    fs::read_to_string("/proc/self/status")
      .unwrap_or_default()
      .lines()
      .find(|l| l.starts_with("VmHWM"))
      .unwrap_or("")
  );

  Ok(())
}
