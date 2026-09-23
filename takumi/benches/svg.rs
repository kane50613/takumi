use std::{fs, hint::black_box, path::Path};

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::prelude::{
  BorderRadius, BorderStyle, Color, ColorInput, Display, FlexWrap, FontResource, Fonts, FromCssStr,
  Length::{Percentage, Px},
  Node, Sides, Style, StyleDeclaration, Viewport,
};
use takumi_svg::{SvgOptions, render};

const BENCH_WIDTH: u32 = 1200;
const BENCH_HEIGHT: u32 = 630;

const PARAGRAPH: &str = "The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs. \
  How vexingly quick daft zebras jump! Sphinx of black quartz, judge my vow. The five boxing wizards jump quickly. \
  Jackdaws love my big sphinx of quartz. We promptly judged antique ivory buckles for the next prize.";

const CJK: &str = "日本利用壓電磁磚將腳步轉化為電能。這些瓷磚捕捉來自你腳步的動能。當你行走時，你的重量和動作會對瓷磚產生壓力。磁磚會輕微彎曲，從而產生機械應力。磁磚內部的壓電材料將這種應力轉化為電能。每一步都會產生少量電荷，而數百萬步結合在一起就能產生足夠的電力來驅動 LED燈、數位顯示器和感測器。";

fn fonts() -> Fonts {
  let mut fonts = Fonts::default();
  let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf");
  let data = fs::read(&path).expect("read test font");
  fonts
    .register(FontResource::new(data))
    .expect("load test font");
  fonts
}

fn render_node(fonts: &Fonts, node: Node) {
  let svg = render(
    SvgOptions::builder()
      .viewport(Viewport::new((BENCH_WIDTH, BENCH_HEIGHT)))
      .node(node)
      .fonts(fonts)
      .build(),
  )
  .unwrap();
  black_box(svg);
}

fn full(decls: impl IntoIterator<Item = StyleDeclaration>) -> Style {
  let mut style = Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::width(Percentage(100.0)))
    .with(StyleDeclaration::height(Percentage(100.0)));
  for declaration in decls {
    style = style.with(declaration);
  }
  style
}

fn paragraph_fixture() -> Node {
  Node::text(PARAGRAPH.to_string()).with_style(full([
    StyleDeclaration::background_color(ColorInput::Value(Color([240, 240, 240, 255]))),
    StyleDeclaration::color(ColorInput::Value(Color([20, 20, 20, 255]))),
    StyleDeclaration::font_size(Px(40.0).into()),
  ]))
}

fn cjk_fixture() -> Node {
  Node::text(CJK.to_string()).with_style(
    full([
      StyleDeclaration::background_color(ColorInput::Value(Color([240, 240, 240, 255]))),
      StyleDeclaration::font_size(Px(64.0).into()),
    ])
    .with_padding(Sides::from(Px(24.0))),
  )
}

fn shape_border_fixture() -> Node {
  let cells = (0..24).map(|i| {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(160.0)))
        .with(StyleDeclaration::height(Px(120.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([(i * 9) as u8, (255 - i * 9) as u8, 128, 255]),
        )))
        .with_border_radius(BorderRadius::from_css_str("24px").unwrap())
        .with(StyleDeclaration::border_top_width(Px(6.0).into()))
        .with(StyleDeclaration::border_right_width(Px(6.0).into()))
        .with(StyleDeclaration::border_bottom_width(Px(6.0).into()))
        .with(StyleDeclaration::border_left_width(Px(6.0).into()))
        .with(StyleDeclaration::border_top_style(BorderStyle::Solid))
        .with(StyleDeclaration::border_right_style(BorderStyle::Solid))
        .with(StyleDeclaration::border_bottom_style(BorderStyle::Solid))
        .with(StyleDeclaration::border_left_style(BorderStyle::Solid))
        .with(StyleDeclaration::border_top_color(ColorInput::Value(
          Color([30, 30, 30, 255]),
        )))
        .with(StyleDeclaration::border_right_color(ColorInput::Value(
          Color([30, 30, 30, 255]),
        )))
        .with(StyleDeclaration::border_bottom_color(ColorInput::Value(
          Color([30, 30, 30, 255]),
        )))
        .with(StyleDeclaration::border_left_color(ColorInput::Value(
          Color([30, 30, 30, 255]),
        ))),
    )
  });

  Node::container(cells.collect::<Vec<_>>()).with_style(full([
    StyleDeclaration::background_color(ColorInput::Value(Color([250, 250, 250, 255]))),
    StyleDeclaration::flex_wrap(FlexWrap::Wrap),
    StyleDeclaration::column_gap(Px(16.0).into()),
    StyleDeclaration::row_gap(Px(16.0).into()),
  ]))
}

fn bench_svg(c: &mut Criterion) {
  let fonts = fonts();
  let mut group = c.benchmark_group("svg");

  group.bench_function("paragraph", |b| {
    b.iter(|| render_node(&fonts, black_box(paragraph_fixture())))
  });
  group.bench_function("cjk", |b| {
    b.iter(|| render_node(&fonts, black_box(cjk_fixture())))
  });
  group.bench_function("gradient_clip_text", |b| {
    b.iter(|| render_node(&fonts, black_box(common::gradient_clip_text_fixture())))
  });
  group.bench_function("shape_border", |b| {
    b.iter(|| render_node(&fonts, black_box(shape_border_fixture())))
  });

  group.finish();
}

mod common;

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_svg
}
criterion_main!(benches);
