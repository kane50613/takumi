use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{
    BackgroundImages, BorderRadius, Cache, Color, ColorInput, Display, Filters, FlexWrap, Fonts,
    FromCssStr, LengthUnit, Node, RenderOptions, Style, Viewport,
  },
  render,
};

fn render_node(fonts: &Fonts, node: Node) {
  let options = RenderOptions::builder()
    .viewport(Viewport::new((512, 512)))
    .node(node)
    .fonts(fonts)
    .build();
  let image = render(options).unwrap();
  black_box(image);
}

/// [`render_node`] with a cache of its own, so an SVG image rasterizes again on every render.
fn render_uncached(fonts: &Fonts, node: Node) {
  let options = RenderOptions::builder()
    .viewport(Viewport::new((512, 512)))
    .node(node)
    .fonts(fonts)
    .cache(Cache::default())
    .build();
  let image = render(options).unwrap();
  black_box(image);
}

fn effect_node(effect_tw: &str) -> Node {
  Node::container([]).with_tw(
    format!("w-[256px] h-[256px] bg-white {effect_tw}")
      .parse()
      .unwrap(),
  )
}

fn backdrop_blur_cards() -> Node {
  let cards = (0..16).map(|_| {
    Node::container([]).with_style(
      Style::default()
        .width(120.0.px())
        .height(120.0.px())
        .margin_top(4.0.px())
        .margin_left(4.0.px())
        .border_radius(BorderRadius::from_css_str("24px").unwrap())
        .backdrop_filter(Filters::from_css_str("blur(10px)").unwrap())
        .background_color(ColorInput::Value(Color([255, 255, 255, 80]))),
    )
  });

  Node::container(cards.collect::<Vec<_>>()).with_style(
    Style::default()
      .display(Display::Flex)
      .flex_wrap(FlexWrap::Wrap)
      .width(512.0.px())
      .height(512.0.px())
      .background_image(Some(
        BackgroundImages::from_css_str("linear-gradient(135deg, #ff0080, #4000ff)").unwrap(),
      )),
  )
}

/// A full-canvas SVG noise overlay, the grain texture many cards draw with `feTurbulence`.
fn svg_turbulence_node() -> Node {
  svg_node(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"><filter id="n"><feTurbulence type="fractalNoise" baseFrequency="0.8" numOctaves="4" stitchTiles="stitch"/></filter><rect width="100%" height="100%" filter="url(#n)"/></svg>"#,
  )
}

/// A blurred SVG shape, filtered in the default `linearRGB` space.
fn svg_blur_node() -> Node {
  svg_node(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"><filter id="b"><feGaussianBlur stdDeviation="12"/></filter><circle cx="256" cy="256" r="160" fill="#3a6ff7" filter="url(#b)"/></svg>"##,
  )
}

/// Full-canvas linear, radial, and focal conical gradients, which shade every pixel.
fn svg_gradients_node() -> Node {
  svg_node(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"><defs><linearGradient id="l" x2="1" y2="1"><stop offset="0" stop-color="#ff0080"/><stop offset=".5" stop-color="#ffd000"/><stop offset="1" stop-color="#4000ff"/></linearGradient><radialGradient id="r"><stop offset="0" stop-color="#fff" stop-opacity=".8"/><stop offset="1" stop-color="#00c2ff" stop-opacity="0"/></radialGradient><radialGradient id="f" fx=".2" fy=".3" r=".6" spreadMethod="reflect"><stop offset="0" stop-color="#0f0"/><stop offset="1" stop-color="#00f"/></radialGradient></defs><rect width="512" height="512" fill="url(#l)"/><rect width="512" height="512" fill="url(#r)"/><circle cx="256" cy="256" r="200" fill="url(#f)" fill-opacity=".5"/></svg>"##,
  )
}

/// Curved, joined, and dashed strokes, which go through the stroker, the dasher, and the scan.
fn svg_strokes_node() -> Node {
  svg_node(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"><path d="M32 400C96 32 224 480 288 112S448 32 480 400" fill="none" stroke="#222" stroke-width="12" stroke-linejoin="round" stroke-linecap="round"/><circle cx="256" cy="256" r="180" fill="none" stroke="#e33" stroke-width="6" stroke-dasharray="18 10"/><path d="M48 64Q256 -32 464 64T464 256" fill="none" stroke="#36f" stroke-width="20" stroke-dasharray="40 16 4 16" stroke-linecap="square"/></svg>"##,
  )
}

/// A pattern tile repeated over the canvas, which samples through the image shader.
fn svg_pattern_node() -> Node {
  svg_node(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"><defs><pattern id="p" width="24" height="24" patternUnits="userSpaceOnUse" patternTransform="rotate(30)"><rect width="12" height="12" fill="#f60"/><circle cx="18" cy="18" r="5" fill="#06f"/></pattern></defs><rect width="512" height="512" fill="url(#p)"/></svg>"##,
  )
}

/// `svg` drawn as a 512px image.
fn svg_node(svg: &'static str) -> Node {
  Node::image(svg).with_style(Style::default().width(512.0.px()).height(512.0.px()))
}

fn bench_effects(c: &mut Criterion) {
  let fonts = Fonts::default();
  let mut group = c.benchmark_group("effects");

  group.bench_function("svg_turbulence", |b| {
    b.iter(|| render_node(&fonts, black_box(svg_turbulence_node())))
  });
  group.bench_function("svg_blur", |b| {
    b.iter(|| render_node(&fonts, black_box(svg_blur_node())))
  });
  group.bench_function("svg_gradients", |b| {
    b.iter(|| render_uncached(&fonts, black_box(svg_gradients_node())))
  });
  group.bench_function("svg_strokes", |b| {
    b.iter(|| render_uncached(&fonts, black_box(svg_strokes_node())))
  });
  group.bench_function("svg_pattern", |b| {
    b.iter(|| render_uncached(&fonts, black_box(svg_pattern_node())))
  });

  group.bench_function("blur_md", |b| {
    b.iter(|| render_node(&fonts, effect_node(black_box("blur-md"))))
  });
  group.bench_function("shadow_md", |b| {
    b.iter(|| render_node(&fonts, effect_node(black_box("shadow-md"))))
  });
  group.bench_function("drop_shadow_md", |b| {
    b.iter(|| render_node(&fonts, effect_node(black_box("drop-shadow-md"))))
  });
  group.bench_function("color_filters", |b| {
    b.iter(|| {
      render_node(
        &fonts,
        effect_node(black_box(
          "bg-gradient-to-r from-red-500 to-blue-500 grayscale-[50%] sepia saturate-150 hue-rotate-90",
        )),
      )
    })
  });
  group.bench_function("backdrop_blur_cards", |b| {
    b.iter(|| render_node(&fonts, black_box(backdrop_blur_cards())))
  });

  group.finish();
}

mod common;

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_effects
}
criterion_main!(benches);
