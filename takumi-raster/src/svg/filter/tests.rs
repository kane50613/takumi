use crate::pixmap::Pixmap;
use takumi_core::resources::image::parse_svg_filter;
use takumi_core::style::Affine;

use super::apply;
use crate::svg::apply_svg_filter;

fn filtered_layer(filter: &str, size: u32) -> Vec<u8> {
  let mut layer = vec![255u8; (size * size * 4) as usize];

  apply_svg_filter(&mut layer, size, size, filter, "f").expect("the filter runs");
  layer
}

#[test]
fn lighting_a_layer_larger_than_the_filter_region_stays_in_bounds() {
  for light in [
    r#"<feDiffuseLighting in="SourceGraphic"><feDistantLight azimuth="45" elevation="45"/></feDiffuseLighting>"#,
    r#"<feSpecularLighting in="SourceAlpha" specularExponent="2"><fePointLight x="3" y="3" z="4"/></feSpecularLighting>"#,
  ] {
    let filter = parse_svg_filter(
      &format!(
        r#"<filter id="f" filterUnits="userSpaceOnUse" x="2" y="2" width="5" height="5">{light}</filter>"#
      ),
      "f",
      12,
      12,
    )
    .expect("the markup parses")
    .expect("a filter");
    let mut layer = Pixmap::new(12, 12).expect("a pixmap");

    layer.pixels_mut().fill([0, 0, 0, 255]);
    apply(&filter, Affine::IDENTITY, &mut layer);
  }
}

#[test]
fn a_filter_region_of_a_million_units_allocates_only_the_layer() {
  filtered_layer(
    r#"<filter id="f" filterUnits="userSpaceOnUse" x="-1000000" y="-1000000" width="2000000" height="2000000"><feGaussianBlur stdDeviation="2"/></filter>"#,
    8,
  );
}

#[test]
fn a_filter_keeping_thousands_of_results_alive_ends() {
  let flood = (0..2000).fold(String::new(), |mut markup, index| {
    markup.push_str(&format!(r#"<feFlood result="r{index}"/>"#));
    markup
  });
  let nodes = (0..2000).fold(String::new(), |mut markup, index| {
    markup.push_str(&format!(r#"<feMergeNode in="r{index}"/>"#));
    markup
  });

  filtered_layer(
    &format!(r#"<filter id="f">{flood}<feMerge>{nodes}</feMerge></filter>"#),
    1024,
  );
}

#[test]
fn a_convolution_of_a_million_cells_is_invalid() {
  let kernel = "0 ".repeat(1_000_000);

  filtered_layer(
    &format!(r#"<filter id="f"><feConvolveMatrix order="1000" kernelMatrix="{kernel}"/></filter>"#),
    64,
  );
}

#[test]
fn a_morphology_radius_past_the_layer_costs_the_layer_once() {
  filtered_layer(
    r#"<filter id="f"><feMorphology radius="100000" operator="dilate"/></filter>"#,
    2000,
  );
}

#[test]
fn a_billion_octaves_of_noise_end() {
  filtered_layer(
    r#"<filter id="f"><feTurbulence baseFrequency="0.05" numOctaves="1000000000"/></filter>"#,
    16,
  );
}

fn pixel(layer: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
  let start = ((y * size + x) * 4) as usize;

  [
    layer[start],
    layer[start + 1],
    layer[start + 2],
    layer[start + 3],
  ]
}

#[test]
fn filter_markup_may_use_xlink_without_declaring_it() {
  let mut layer = vec![255u8; 16 * 16 * 4];

  assert!(
    apply_svg_filter(
      &mut layer,
      16,
      16,
      r##"<filter id="f"><feImage xlink:href="#missing"/></filter>"##,
      "f"
    )
    .is_ok()
  );
}

#[test]
fn a_tiled_linear_rgb_result_converts_back_to_srgb() {
  let layer = filtered_layer(
    r#"<filter id="f"><feFlood flood-color="rgb(200, 100, 50)"/><feGaussianBlur stdDeviation="1" result="a"/><feTile in="a"/></filter>"#,
    20,
  );
  let [red, green, blue, alpha] = pixel(&layer, 20, 10, 10);

  assert!(
    red.abs_diff(200) <= 2 && green.abs_diff(100) <= 2 && blue.abs_diff(50) <= 2 && alpha == 255,
    "{:?}",
    [red, green, blue, alpha]
  );
}

#[test]
fn a_lighting_color_lights_in_its_own_color() {
  let layer = filtered_layer(
    r#"<filter id="f"><feDiffuseLighting lighting-color="rgb(200, 100, 50)"><feDistantLight elevation="90"/></feDiffuseLighting></filter>"#,
    16,
  );
  let [red, green, blue, _] = pixel(&layer, 16, 8, 8);

  assert!(
    red.abs_diff(200) <= 2 && green.abs_diff(100) <= 2 && blue.abs_diff(50) <= 2,
    "{:?}",
    [red, green, blue]
  );
}

#[test]
fn a_specular_exponent_past_128_clamps_instead_of_clearing() {
  let layer = filtered_layer(
    r#"<filter id="f"><feSpecularLighting specularExponent="200"><feDistantLight elevation="90"/></feSpecularLighting></filter>"#,
    16,
  );

  assert_ne!(pixel(&layer, 16, 8, 8), [0, 0, 0, 0]);
}

#[test]
fn a_background_image_input_reads_the_result_before() {
  let layer = filtered_layer(
    r#"<filter id="f"><feFlood flood-color="rgb(0, 0, 255)"/><feComposite in="BackgroundImage" in2="BackgroundImage"/></filter>"#,
    16,
  );
  let [red, _, blue, _] = pixel(&layer, 16, 8, 8);

  assert!(blue > 200 && red < 50, "{:?}", pixel(&layer, 16, 8, 8));
}

#[test]
fn a_bounding_box_subregion_is_a_fraction_of_the_box() {
  let layer = filtered_layer(
    r#"<filter id="f" primitiveUnits="objectBoundingBox"><feColorMatrix type="saturate" values="1" x="0.25" width="0.5"/></filter>"#,
    40,
  );

  assert_eq!(pixel(&layer, 40, 20, 20), [255, 255, 255, 255]);
  assert_eq!(pixel(&layer, 40, 4, 20), [0, 0, 0, 0]);
}

#[test]
fn tiling_repeats_the_input_across_the_region() {
  let layer = filtered_layer(
    r#"<filter id="f" filterUnits="userSpaceOnUse" x="0" y="0" width="8" height="8"><feFlood x="2" y="2" width="2" height="2" flood-color="green" result="a"/><feTile in="a"/></filter>"#,
    8,
  );

  for (x, y) in [(0, 0), (3, 3), (7, 7)] {
    let index = (y * 8 + x) * 4;

    assert_eq!(layer[index..index + 4], [0, 128, 0, 255], "({x}, {y})");
  }
}
