use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use takumi_core::html::{FromHtmlOptions, from_html};
use takumi_core::{Fonts, layout::node::Node, resources::font::FontResource};
use takumi_pdf::{PageOptions, PdfOptions, render};

/// A ledger of `rows` rows whose header repeats on every page it spans.
fn long_table(rows: usize) -> String {
  let body: String = (0..rows)
    .map(|row| {
      format!(
        "<tr><td>{row}</td><td>Invoice line {row} for consulting work</td><td>{}</td><td>{}.00</td></tr>",
        row % 12 + 1,
        row * 37 % 1000
      )
    })
    .collect();

  format!(
    r#"<table style="width: 100%; border-collapse: collapse; font-size: 12px">
      <thead><tr><th>#</th><th>Description</th><th>Qty</th><th>Amount</th></tr></thead>
      <tbody>{body}</tbody>
    </table>"#
  )
}

/// `paragraphs` paragraphs of wrapped text, as a long report reads.
fn long_text(paragraphs: usize) -> String {
  let sentence =
    "Layout splits this paragraph between pages wherever a line falls past the bottom. ";
  let paragraph = format!(
    r#"<p style="font-size: 12px; line-height: 1.5; margin: 0 0 8px">{}</p>"#,
    sentence.repeat(6)
  );

  paragraph.repeat(paragraphs)
}

fn parse(html: &str) -> Node {
  from_html(html, FromHtmlOptions::default()).expect("parse bench document")
}

fn render_node(fonts: &Fonts, node: Node) -> Vec<u8> {
  render(
    PdfOptions::builder()
      .node(node)
      .page(PageOptions::A4)
      .fonts(fonts)
      .build(),
  )
  .expect("render pdf")
}

fn page_count(pdf: &[u8]) -> usize {
  let text = String::from_utf8_lossy(pdf);
  let start = text.find("/Type/Pages/Count ").expect("a page tree") + "/Type/Pages/Count ".len();

  text[start..]
    .chars()
    .take_while(char::is_ascii_digit)
    .collect::<String>()
    .parse()
    .expect("a page count")
}

/// Archivo for text, since `Fonts::default()` carries no fonts.
fn fonts() -> Fonts {
  let mut fonts = Fonts::default();
  let archivo: &[u8] =
    include_bytes!("../../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf");

  fonts
    .register(FontResource::new(archivo.to_vec()))
    .expect("load latin font");
  fonts
}

fn bench_pagination(c: &mut Criterion) {
  let fonts = fonts();
  let documents = [
    ("long_table", parse(&long_table(3000))),
    ("long_text", parse(&long_text(600))),
    (
      "report",
      parse(&format!(
        "{}{}{}",
        long_text(100),
        long_table(1000),
        long_text(100)
      )),
    ),
  ];
  let mut group = c.benchmark_group("pdf_pagination");

  group.sample_size(10);
  for (name, node) in &documents {
    assert!(
      page_count(&render_node(&fonts, node.clone())) >= 30,
      "a bench document spans 30 pages"
    );
    group.bench_function(*name, |b| {
      b.iter_batched(
        || node.clone(),
        |node| black_box(render_node(&fonts, node)),
        BatchSize::SmallInput,
      )
    });
  }
  group.finish();
}

criterion_group!(benches, bench_pagination);
criterion_main!(benches);
