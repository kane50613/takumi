//! Background-image rasterization at the size of a full-height sidebar panel.
//!
//! A bitmap drawn 1:1 as a background should cost about what the same bitmap
//! costs as an `<image>` node, so the latter is measured as the baseline.

use std::{collections::HashMap, hint::black_box, sync::Arc};

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{
    BackgroundImages, BackgroundRepeats, BackgroundSizes, Color, ColorInput, Display, Fonts,
    FromCssStr, ImageScalingAlgorithm, ImageSource,
    Length::{Percentage, Px},
    Node, ObjectFit, RenderOptions, Style, StyleDeclaration, Viewport,
  },
  render,
};
use takumi_core::resources::image_buffer::ImageBuffer;

const PANEL_WIDTH: u32 = 397;
const PANEL_HEIGHT: u32 = 2160;
const WALLPAPER_URL: &str = "bench://wallpaper";

/// A deterministic opaque bitmap. The content only has to vary from pixel to
/// pixel so no sampler can short-circuit on it.
fn wallpaper(width: u32, height: u32) -> ImageSource {
  let mut data = Vec::with_capacity((width as usize) * (height as usize) * 4);
  for y in 0..height {
    for x in 0..width {
      data.extend_from_slice(&[
        (x % 251) as u8,
        (y % 241) as u8,
        ((x + y) % 233) as u8,
        u8::MAX,
      ]);
    }
  }

  ImageSource::from(
    ImageBuffer::from_rgba_bytes(data, width, height).expect("wallpaper buffer dimensions"),
  )
}

fn render_node(fonts: &Fonts, node: Node, images: &HashMap<Arc<str>, ImageSource>) {
  let options = RenderOptions::builder()
    .viewport(Viewport::new((PANEL_WIDTH, PANEL_HEIGHT)))
    .node(node)
    .fonts(fonts)
    .images(images.clone())
    .build();
  let image = render(options).expect("render panel");
  black_box(image);
}

fn panel(style: Style) -> Node {
  Node::container([]).with_style(style)
}

fn base_style() -> Style {
  Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::width(Percentage(100.0)))
    .with(StyleDeclaration::height(Percentage(100.0)))
    .with(StyleDeclaration::background_color(ColorInput::Value(
      Color([18, 18, 22, 255]),
    )))
}

fn background_style(algorithm: ImageScalingAlgorithm) -> Style {
  base_style()
    .with(StyleDeclaration::background_image(Some(
      BackgroundImages::from_css_str(&format!("url({WALLPAPER_URL})")).expect("background url"),
    )))
    .with(StyleDeclaration::background_size(
      BackgroundSizes::from_css_str("100% 100%").expect("background size"),
    ))
    .with(StyleDeclaration::background_repeat(
      BackgroundRepeats::from_css_str("no-repeat").expect("background repeat"),
    ))
    .with(StyleDeclaration::image_rendering(algorithm))
}

fn image_node(source: ImageSource) -> Node {
  Node::container([Node::image(source).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(PANEL_WIDTH as f32)))
      .with(StyleDeclaration::height(Px(PANEL_HEIGHT as f32)))
      .with(StyleDeclaration::object_fit(ObjectFit::Fill)),
  )])
  .with_style(base_style())
}

fn bench_background(c: &mut Criterion) {
  let fonts = Fonts::default();
  let source = wallpaper(PANEL_WIDTH, PANEL_HEIGHT);
  let oversized = wallpaper(PANEL_WIDTH * 2, PANEL_HEIGHT);

  let images: HashMap<Arc<str>, ImageSource> =
    HashMap::from([(Arc::from(WALLPAPER_URL), source.clone())]);
  let scaled_images: HashMap<Arc<str>, ImageSource> =
    HashMap::from([(Arc::from(WALLPAPER_URL), oversized)]);
  let no_images = HashMap::new();

  let mut group = c.benchmark_group("background");

  group.bench_function("image_node", |b| {
    b.iter(|| render_node(&fonts, black_box(image_node(source.clone())), &no_images))
  });
  group.bench_function("one_to_one_auto", |b| {
    b.iter(|| {
      render_node(
        &fonts,
        black_box(panel(background_style(ImageScalingAlgorithm::Auto))),
        &images,
      )
    })
  });
  group.bench_function("downscaled_auto", |b| {
    b.iter(|| {
      render_node(
        &fonts,
        black_box(panel(background_style(ImageScalingAlgorithm::Auto))),
        &scaled_images,
      )
    })
  });

  group.finish();
}

mod common;

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_background
}
criterion_main!(benches);
