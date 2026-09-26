use std::{fs, io::ErrorKind};

use takumi::prelude::{Length::*, *};

use crate::test_utils::{generated_path, run_fixture_test};

fn create_container_with(
  background_images: BackgroundImages,
  background_size: BackgroundSizes,
  background_position: PositionValues,
  background_repeat: BackgroundRepeats,
) -> Node {
  Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_image(Some(background_images)))
      .with(StyleDeclaration::background_size(background_size))
      .with(StyleDeclaration::background_position(background_position))
      .with(StyleDeclaration::background_repeat(background_repeat)),
  )
}

/// `background-size` with one auto axis re-derives that axis from the image's
/// intrinsic ratio after `round` rescales the other one. The SVG backend used
/// to skip that step and disagree with the raster one.
#[test]
fn test_background_size_auto_axis_round() {
  let images = BackgroundImages::from_css_str("url(assets/images/yeecord.png)").unwrap();
  let container = create_container_with(
    images,
    BackgroundSizes::from_css_str("auto 80px").unwrap(),
    PositionValues::from_css_str("left top").unwrap(),
    BackgroundRepeats::from_css_str("round").unwrap(),
  );

  // The fixture harness rewrites its goldens without comparing, so the geometry
  // is asserted here. Removing the file first keeps a stale one from a previous
  // run out of the assertion.
  let svg_path = generated_path("style_background_size_auto_axis_round.svg");

  match fs::remove_file(&svg_path) {
    Ok(()) => {}
    Err(error) if error.kind() == ErrorKind::NotFound => {}
    Err(error) => panic!("could not clear the stale svg: {error}"),
  }

  run_fixture_test(container, "style_background_size_auto_axis_round");

  let svg = fs::read_to_string(&svg_path).expect("the fixture wrote no svg");
  let pattern = svg
    .split_once("<pattern ")
    .expect("no background pattern in the svg")
    .1
    .split_once('>')
    .expect("unterminated pattern element")
    .0;
  let attr = |name: &str| {
    pattern
      .split_once(&format!("{name}=\""))
      .and_then(|(_, rest)| rest.split_once('"'))
      .map(|(value, _)| value.to_string())
      .unwrap_or_default()
  };

  // The fixed axis rounds to 90, the auto axis follows the intrinsic ratio and
  // then rounds to 92. Without the re-derivation the auto axis stays at 80.
  assert_eq!((attr("width"), attr("height")), ("92".into(), "90".into()));
}
