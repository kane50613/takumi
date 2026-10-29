use std::{collections::HashMap, hint::black_box, sync::Arc};

use criterion::{Criterion, criterion_group, criterion_main};
use takumi_core::html::{FromHtmlOptions, from_html};
use takumi_core::{Fonts, resources::image::ImageSource};
use takumi_pdf::{PageOptions, PdfOptions, render};

/// The same photo drawn 120 times across pages, as a logo or avatar repeats through a report.
fn repeated_photo(style: &str) -> String {
  let image = format!(r#"<img src="photo" style="width: 120px; height: 120px; {style}">"#);

  format!(
    r#"<div style="display: flex; flex-wrap: wrap; gap: 8px">{}</div>"#,
    image.repeat(120)
  )
}

fn render_html(fonts: &Fonts, images: &HashMap<Arc<str>, ImageSource>, html: &str) {
  let node = from_html(html, FromHtmlOptions::default()).expect("parse bench document");

  black_box(
    render(
      PdfOptions::builder()
        .node(node)
        .page(PageOptions::A4)
        .fonts(fonts)
        .images(images.clone())
        .build(),
    )
    .expect("render pdf"),
  );
}

fn bench_images(c: &mut Criterion) {
  let fonts = Fonts::default();
  let photo =
    ImageSource::from_bytes(include_bytes!("../../assets/images/fuma.jpg")).expect("decode photo");
  let images = HashMap::from([("photo".into(), photo)]);
  let plain = repeated_photo("");
  let filtered = repeated_photo("filter: grayscale(1)");
  let mut group = c.benchmark_group("pdf_images");

  group.sample_size(20);
  group.bench_function("repeated_photo", |b| {
    b.iter(|| render_html(&fonts, &images, &plain))
  });
  group.bench_function("repeated_filtered_photo", |b| {
    b.iter(|| render_html(&fonts, &images, &filtered))
  });
  group.finish();
}

criterion_group!(benches, bench_images);
criterion_main!(benches);
