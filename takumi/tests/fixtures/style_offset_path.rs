use takumi::prelude::{Length::*, *};

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

// Markers ride a cubic bezier; `offset-rotate: auto` turns each to face the
// path tangent.
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
