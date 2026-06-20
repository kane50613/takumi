//! End-to-end checks for the `takumi-svg` vector backend: render a node tree to
//! SVG, rasterize the emitted SVG with resvg, and assert the pixels match what
//! the box model should produce. This catches malformed SVG and gross
//! positioning/color regressions without requiring pixel-exact parity with the
//! raster backend (a different rasterizer).

use std::{fs, path::Path};

use resvg::{
  tiny_skia::{Pixmap, Transform},
  usvg::{Options, Tree},
};
use takumi::prelude::{Length::*, *};
use takumi_svg::{SvgOptions, render};

const W: u32 = 200;
const H: u32 = 100;

fn context() -> Fonts {
  let mut fonts = Fonts::default();
  let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf");
  let data = fs::read(&path).expect("read test font");
  fonts
    .register(FontResource::new(data))
    .expect("load test font");
  fonts
}

fn rasterize(node: Node, fonts: &Fonts) -> Pixmap {
  let svg = render(
    SvgOptions::builder()
      .node(node)
      .viewport(Viewport::new((W, H)))
      .fonts(fonts)
      .build(),
  )
  .expect("render svg");
  let tree = Tree::from_str(&svg, &Options::default()).expect("resvg parses emitted svg");
  let mut pixmap = Pixmap::new(W, H).expect("alloc pixmap");
  resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
  pixmap
}

/// Straight-alpha RGBA at a pixel.
fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
  let p = pixmap.pixel(x, y).expect("pixel in bounds").demultiply();
  [p.red(), p.green(), p.blue(), p.alpha()]
}

#[track_caller]
fn assert_close(actual: [u8; 4], expected: [u8; 4], tol: i32) {
  let ok = actual
    .iter()
    .zip(expected.iter())
    .all(|(a, b)| (*a as i32 - *b as i32).abs() <= tol);
  assert!(ok, "pixel {actual:?} not within {tol} of {expected:?}");
}

/// A viewport-filling container carrying the given style declarations.
fn full_container(decls: impl IntoIterator<Item = StyleDeclaration>) -> Node {
  let mut style = Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::width(Percentage(100.0)))
    .with(StyleDeclaration::height(Percentage(100.0)));
  for declaration in decls {
    style = style.with(declaration);
  }
  Node::container([]).with_style(style)
}

#[test]
fn solid_background_renders_as_fill() {
  let node = full_container([StyleDeclaration::background_color(ColorInput::Value(
    Color([255, 0, 0, 255]),
  ))]);
  let pixmap = rasterize(node, &context());
  assert_close(pixel(&pixmap, W / 2, H / 2), [255, 0, 0, 255], 2);
}

#[test]
fn linear_gradient_endpoints_match_stops() {
  let images = BackgroundImages::from_str("linear-gradient(to right, #ff0000, #0000ff)")
    .expect("parse gradient");
  let node = full_container([StyleDeclaration::background_image(Some(images))]);
  let pixmap = rasterize(node, &context());
  assert_close(pixel(&pixmap, 1, H / 2), [255, 0, 0, 255], 12);
  assert_close(pixel(&pixmap, W - 2, H / 2), [0, 0, 255, 255], 12);
}

#[test]
fn opacity_group_halves_alpha() {
  let node = full_container([
    StyleDeclaration::background_color(ColorInput::Value(Color([255, 0, 0, 255]))),
    StyleDeclaration::opacity(PercentageNumber(0.5)),
  ]);
  let pixmap = rasterize(node, &context());
  let [r, g, b, a] = pixel(&pixmap, W / 2, H / 2);
  assert!((120..=135).contains(&a), "alpha {a} not ~128");
  assert_close([r, g, b, 255], [255, 0, 0, 255], 4);
}

#[test]
fn text_renders_visible_glyphs() {
  let text = Node::text("Hello".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        0, 0, 0, 255,
      ]))))
      .with(StyleDeclaration::font_size(FontSize::Length(Px(48.0)))),
  );
  let node = Node::container([text]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      ))),
  );
  let pixmap = rasterize(node, &context());
  let dark = pixmap
    .pixels()
    .iter()
    .filter(|p| p.alpha() > 0 && p.demultiply().red() < 100)
    .count();
  assert!(dark > 0, "expected dark glyph pixels, found none");
}

#[test]
fn underline_decoration_renders() {
  let text = Node::text("Hello".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        0, 0, 0, 255,
      ]))))
      .with(StyleDeclaration::font_size(FontSize::Length(Px(48.0))))
      .with_text_decoration(
        TextDecoration::builder()
          .line(TextDecorationLines::UNDERLINE)
          .color(ColorInput::Value(Color([255, 0, 0, 255])))
          .build(),
      ),
  );
  let node = Node::container([text]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      ))),
  );
  let pixmap = rasterize(node, &context());
  let red = pixmap
    .pixels()
    .iter()
    .filter(|p| {
      let c = p.demultiply();
      c.red() > 150 && c.green() < 90 && c.blue() < 90
    })
    .count();
  assert!(red > 0, "expected red underline pixels, found none");
}
