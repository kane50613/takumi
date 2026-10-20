use std::hint::black_box;

use base64::{Engine, engine::general_purpose::STANDARD};
use criterion::{Criterion, criterion_group, criterion_main};
use takumi_core::Fonts;
use takumi_html::{FromHtmlOptions, from_html};
use takumi_pdf::{PageOptions, PdfOptions, render};

/// The same photo drawn 120 times across pages, as a logo or avatar repeats through a report.
fn repeated_photo(style: &str) -> String {
  let photo = STANDARD.encode(include_bytes!("../../assets/images/fuma.jpg"));
  let image = format!(
    r#"<img src="data:image/jpeg;base64,{photo}" style="width: 120px; height: 120px; {style}">"#
  );

  format!(
    r#"<div style="display: flex; flex-wrap: wrap; gap: 8px">{}</div>"#,
    image.repeat(120)
  )
}

fn render_html(fonts: &Fonts, html: &str) {
  let node = from_html(html, FromHtmlOptions::default()).expect("parse bench document");

  black_box(
    render(
      PdfOptions::builder()
        .node(node)
        .page(PageOptions::A4)
        .fonts(fonts)
        .build(),
    )
    .expect("render pdf"),
  );
}

fn bench_images(c: &mut Criterion) {
  let fonts = Fonts::default();
  let plain = repeated_photo("");
  let filtered = repeated_photo("filter: grayscale(1)");
  let mut group = c.benchmark_group("pdf_images");

  group.sample_size(20);
  group.bench_function("repeated_photo", |b| b.iter(|| render_html(&fonts, &plain)));
  group.bench_function("repeated_filtered_photo", |b| {
    b.iter(|| render_html(&fonts, &filtered))
  });
  group.finish();
}

criterion_group!(benches, bench_images);
criterion_main!(benches);
