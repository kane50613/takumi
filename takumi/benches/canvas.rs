use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{
    AlignItems, BackgroundClip, BackgroundImages, BackgroundRepeats, BackgroundSizes, BorderRadius,
    Color, ColorInput, Display, FlexWrap, Fonts, FromCssStr, FromHtml, FromHtmlOptions, ImageData,
    ImageSourceInput, JustifyContent,
    Length::{Percentage, Px},
    Node, ObjectFit, Overflow, PositionValues, RenderOptions, RgbaImage, SpacePair, Style,
    StyleDeclaration, StylePresets, Viewport,
  },
  render,
};

const BENCH_WIDTH: u32 = 800;
const BENCH_HEIGHT: u32 = 800;
const IMAGE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/images/yeecord.png");

fn render_node(fonts: &Fonts, node: Node) {
  let options = RenderOptions::builder()
    .viewport(Viewport::new((BENCH_WIDTH, BENCH_HEIGHT)))
    .node(node)
    .fonts(fonts)
    .build();
  let image = render(options).unwrap();
  black_box(image);
}

fn nested_clip_masks_fixture() -> Node {
  let leaf = |radius: &str| {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with_border_radius(BorderRadius::from_css_str(radius).unwrap())
        .with_overflow(SpacePair::from_single(Overflow::Clip))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([200, 30, 30, 255]),
        ))),
    )
  };

  let mut current = leaf("12px");
  for radius in ["24px", "32px", "40px", "48px", "56px"] {
    current = Node::container([current]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(95.0)))
        .with(StyleDeclaration::height(Percentage(95.0)))
        .with_border_radius(BorderRadius::from_css_str(radius).unwrap())
        .with_overflow(SpacePair::from_single(Overflow::Clip))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([30, 30, 200, 255]),
        ))),
    );
  }

  Node::container([current]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )
}

fn sibling_clip_masks_fixture() -> Node {
  let cards = (0..64).map(|index| {
    let inner = Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([200, 30, index as u8, 255]),
        ))),
    );

    Node::container([inner]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(92.0)))
        .with(StyleDeclaration::height(Px(92.0)))
        .with(StyleDeclaration::margin_top(Px(4.0)))
        .with(StyleDeclaration::margin_left(Px(4.0)))
        .with_border_radius(BorderRadius::from_css_str("24px").unwrap())
        .with_overflow(SpacePair::from_single(Overflow::Clip)),
    )
  });

  Node::container(cards.collect::<Vec<_>>()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_wrap(FlexWrap::Wrap))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )
}

fn scaled_image_fixture() -> Node {
  Node::container([Node::image(IMAGE_PATH).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::object_fit(ObjectFit::Cover)),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(720.0)))
      .with(StyleDeclaration::height(Px(720.0)))
      .with_border_radius(BorderRadius::from_css_str("96px").unwrap())
      .with_overflow(SpacePair::from_single(Overflow::Clip))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([245, 247, 250, 255]),
      ))),
  )
}

fn gradient_clip_mask_fixture() -> Node {
  let gradient = BackgroundImages::from_css_str(
    "linear-gradient(135deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();
  Node::container([Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(640.0)))
      .with(StyleDeclaration::height(Px(640.0)))
      .with_border_radius(BorderRadius::from_css_str("64px").unwrap())
      .with_overflow(SpacePair::from_single(Overflow::Clip))
      .with(StyleDeclaration::background_image(Some(gradient)))
      .with(StyleDeclaration::background_size(
        BackgroundSizes::from_css_str("100% 100%").unwrap(),
      ))
      .with(StyleDeclaration::background_position(
        PositionValues::from_css_str("0 0").unwrap(),
      ))
      .with(StyleDeclaration::background_repeat(
        BackgroundRepeats::from_css_str("no-repeat").unwrap(),
      ))
      .with(StyleDeclaration::background_clip(BackgroundClip::BorderBox)),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )
}

/// The `<body>` of an HTML fixture, as the HTML fixture test parses it.
fn html_fixture(html: &str) -> Node {
  let body = html
    .split_once("<body")
    .and_then(|(_, rest)| rest.split_once('>'))
    .and_then(|(_, rest)| rest.split_once("</body>"))
    .map(|(inner, _)| inner.trim())
    .unwrap();
  let options = FromHtmlOptions::builder()
    .presets(StylePresets::empty())
    .build();

  Node::from_html(body, options).unwrap()
}

/// Full-canvas layers at partial opacity, each composited over the ones below.
fn translucent_layers_fixture() -> Node {
  let layers = ["#ff5f6d", "#3a1c71", "#30a46c", "#ffb454"]
    .iter()
    .map(|color| {
      format!(
        r#"<div style="position: absolute; inset: 0; opacity: 0.6; background-image: linear-gradient(135deg, {color}, transparent)"></div>"#
      )
    })
    .collect::<String>();

  html_fixture(&format!(
    r#"<body><div style="position: relative; width: 100%; height: 100%; background-color: white">{layers}</div></body>"#
  ))
}

/// A card with rounded corners over a gradient, the most common OG image layout.
fn rounded_gradient_card_fixture() -> Node {
  html_fixture(
    r#"<body><div style="width: 100%; height: 100%; padding: 40px; background-color: white"><div style="width: 100%; height: 100%; border-radius: 48px; background-image: linear-gradient(135deg, #ff5f6d, #3a1c71)"></div></div></body>"#,
  )
}

/// A 640px photo-like image with rounded corners, drawn at its own size or scaled to `size`.
fn rounded_image_fixture(size: &str) -> Node {
  let pixels = (0..640 * 640)
    .flat_map(|index: u32| {
      let noise = index.wrapping_mul(2_654_435_761) >> 24;

      [(index % 640) as u8, (index / 640) as u8, noise as u8, 255]
    })
    .collect();
  let image = ImageData {
    src: ImageSourceInput::Rgba(RgbaImage::new(pixels, 640, 640, true).unwrap()),
    width: None,
    height: None,
  };

  Node::container([Node::image(image).with_tw(
    format!("w-[{size}] h-[{size}] rounded-[64px]")
      .parse()
      .unwrap(),
  )])
  .with_tw(
    "flex w-full h-full items-center justify-center"
      .parse()
      .unwrap(),
  )
}

/// A full-canvas radial gradient under a `clip-path`, which paints through the clip mask.
fn clipped_radial_gradient_fixture() -> Node {
  html_fixture(
    r#"<body><div style="width: 100%; height: 100%; clip-path: circle(45%); background-image: radial-gradient(circle at 30% 40%, #ff5f6d, #3a1c71 60%, #30a46c)"></div></body>"#,
  )
}

fn bench_canvas(c: &mut Criterion) {
  let fonts = Fonts::default();
  let mut group = c.benchmark_group("canvas");

  group.bench_function("nested_clip_masks", |b| {
    b.iter(|| render_node(&fonts, black_box(nested_clip_masks_fixture())))
  });
  group.bench_function("sibling_clip_masks", |b| {
    b.iter(|| render_node(&fonts, black_box(sibling_clip_masks_fixture())))
  });
  group.bench_function("scaled_image_clip", |b| {
    b.iter(|| render_node(&fonts, black_box(scaled_image_fixture())))
  });
  group.bench_function("gradient_clip_mask", |b| {
    b.iter(|| render_node(&fonts, black_box(gradient_clip_mask_fixture())))
  });
  group.bench_function("translucent_layers", |b| {
    b.iter(|| render_node(&fonts, black_box(translucent_layers_fixture())))
  });
  group.bench_function("rounded_gradient_card", |b| {
    b.iter(|| render_node(&fonts, black_box(rounded_gradient_card_fixture())))
  });
  group.bench_function("rounded_image", |b| {
    b.iter(|| render_node(&fonts, black_box(rounded_image_fixture("640px"))))
  });
  group.bench_function("rounded_scaled_image", |b| {
    b.iter(|| render_node(&fonts, black_box(rounded_image_fixture("700px"))))
  });
  group.bench_function("clipped_radial_gradient", |b| {
    b.iter(|| render_node(&fonts, black_box(clipped_radial_gradient_fixture())))
  });
  group.bench_function("square_border_sides", |b| {
    b.iter(|| {
      render_node(
        &fonts,
        black_box(html_fixture(include_str!(
          "../tests/fixtures-html/border_side_miters.html"
        ))),
      )
    })
  });
  group.bench_function("rounded_border_sides", |b| {
    b.iter(|| {
      render_node(
        &fonts,
        black_box(html_fixture(include_str!(
          "../tests/fixtures-html/border_rounded_sides.html"
        ))),
      )
    })
  });

  group.finish();
}

mod common;

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_canvas
}
criterion_main!(benches);
