use takumi::prelude::{Length::*, *};
use takumi_core::layout::style::FromCss;

use crate::test_utils::run_fixture_test;

const CURVE: &str = "path('M 80 480 C 360 60, 840 60, 1120 480')";
const STOPS: &[f32] = &[0.0, 12.5, 25.0, 37.5, 50.0, 62.5, 75.0, 87.5, 100.0];

fn marker(color: Color, declarations: impl IntoIterator<Item = StyleDeclaration>) -> Node {
  let mut style = Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::position(Position::Absolute))
    .with(StyleDeclaration::left(Px(0.0)))
    .with(StyleDeclaration::top(Px(0.0)))
    .with(StyleDeclaration::width(Px(72.0)))
    .with(StyleDeclaration::height(Px(18.0)))
    .with(StyleDeclaration::background_color(ColorInput::Value(color)));

  for declaration in declarations {
    style = style.with(declaration);
  }

  Node::container([]).with_style(style)
}

fn root(children: Vec<Node>) -> Node {
  Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Px(1200.0)))
      .with(StyleDeclaration::height(Px(630.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )
}

// Markers ride a cubic bezier, turned to the tangent by `offset-rotate: auto`.
#[test]
fn test_offset_path_along_curve() {
  let markers = STOPS
    .iter()
    .enumerate()
    .map(|(index, distance)| {
      let shade = (index * 20) as u8;
      marker(
        Color([40, 90, 220u8.saturating_sub(shade), 255]),
        [
          StyleDeclaration::offset_path(Some(OffsetPath::from_str(CURVE).unwrap())),
          StyleDeclaration::offset_distance(Percentage(*distance)),
          StyleDeclaration::offset_rotate(OffsetRotate::default()),
        ],
      )
    })
    .collect();

  run_fixture_test(root(markers), "offset_path_along_curve");
}

// `ray()` markers fan out from the center at evenly spaced angles.
#[test]
fn test_offset_path_ray_burst() {
  let markers = (0..12)
    .map(|index| {
      let angle = (index * 30) as f32;
      marker(
        Color([220, 60, 120, 255]),
        [
          StyleDeclaration::offset_path(Some(
            OffsetPath::from_str(&format!("ray({angle}deg closest-side at 50% 50%)")).unwrap(),
          )),
          StyleDeclaration::offset_distance(Percentage(80.0)),
          StyleDeclaration::offset_rotate(OffsetRotate::default()),
        ],
      )
    })
    .collect();

  run_fixture_test(root(markers), "offset_path_ray_burst");
}

// A six-colour palette cycled across glyphs for the text demos.
const PALETTE: &[Color] = &[
  Color([233, 69, 96, 255]),
  Color([243, 146, 55, 255]),
  Color([255, 202, 58, 255]),
  Color([138, 201, 38, 255]),
  Color([25, 130, 196, 255]),
  Color([106, 76, 147, 255]),
];

fn glyph(character: char, index: usize, distance: f32, path: &str, font_size: f32) -> Node {
  let text = Node::text(character.to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::font_size(Px(font_size).into()))
      .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
      .with(StyleDeclaration::color(ColorInput::Value(
        PALETTE[index % PALETTE.len()],
      ))),
  );

  // The wrapper carries the motion; the glyph rides inside it.
  Node::container([text]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::left(Px(0.0)))
      .with(StyleDeclaration::top(Px(0.0)))
      .with(StyleDeclaration::offset_path(Some(
        OffsetPath::from_str(path).unwrap(),
      )))
      .with(StyleDeclaration::offset_distance(Percentage(distance)))
      .with(StyleDeclaration::offset_rotate(OffsetRotate::default())),
  )
}

// Glyphs ride a circle, each turned to the tangent: a circular seal.
#[test]
fn test_offset_path_circular_text() {
  let ring = "★ TAKUMI ★ MOTION PATH ★ OFFSET ★ ";
  let circle = "ellipse(240px 240px at 600px 315px)";
  let characters: Vec<char> = ring.chars().collect();
  let count = characters.len();

  let glyphs = characters
    .iter()
    .enumerate()
    .map(|(index, character)| {
      let distance = index as f32 / count as f32 * 100.0;
      glyph(*character, index, distance, circle, 40.0)
    })
    .collect();

  run_fixture_test(root(glyphs), "offset_path_circular_text");
}

// "offset-path" spelled along a wave, each letter banking with the slope.
#[test]
fn test_offset_path_wave_text() {
  let wave = "path('M 120 315 C 360 60, 480 570, 720 315 S 1080 60, 1180 315')";
  let word = "offset-path";
  let characters: Vec<char> = word.chars().collect();
  let count = characters.len();

  let glyphs = characters
    .iter()
    .enumerate()
    .map(|(index, character)| {
      let distance = index as f32 / (count - 1) as f32 * 100.0;
      glyph(*character, index, distance, wave, 72.0)
    })
    .collect();

  run_fixture_test(root(glyphs), "offset_path_wave_text");
}
