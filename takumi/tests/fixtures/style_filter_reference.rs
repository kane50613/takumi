use takumi::prelude::{Length::*, *};

use crate::test_utils::run_fixture_test;

/// Percent-encodes everything outside the URL-unreserved set, so the result is
/// safe as an unquoted CSS `url()` token and survives nested data-URI decoding.
pub fn percent_encode(source: &str) -> String {
  let mut out = String::with_capacity(source.len() * 3);
  for byte in source.bytes() {
    match byte {
      b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
        out.push(byte as char);
      }
      _ => {
        out.push('%');
        out.push_str(&format!("{byte:02X}"));
      }
    }
  }
  out
}

pub fn filter_url(markup: &str) -> String {
  format!("url(data:image/svg+xml,{})", percent_encode(markup))
}

/// A 4x4 ordered Bayer threshold tile as an SVG data URI: 16 gray rects with
/// values `(m + 0.5) / 16`.
fn bayer_tile_uri() -> String {
  const MATRIX: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

  let mut rects = String::new();
  for (y, row) in MATRIX.iter().enumerate() {
    for (x, threshold) in row.iter().enumerate() {
      let value = ((*threshold as f32 + 0.5) / 16.0 * 255.0).round() as u32;
      rects.push_str(&format!(
        r#"<rect x="{x}" y="{y}" width="1" height="1" fill="rgb({value},{value},{value})"/>"#
      ));
    }
  }

  let svg =
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">{rects}</svg>"#);
  format!("data:image/svg+xml,{}", percent_encode(&svg))
}

const DISCRETE_4: &str = r#"<feFuncR type="discrete" tableValues="0 0.333 0.667 1"/><feFuncG type="discrete" tableValues="0 0.333 0.667 1"/><feFuncB type="discrete" tableValues="0 0.333 0.667 1"/>"#;

fn posterize_filter() -> String {
  filter_url(&format!(
    r#"<filter color-interpolation-filters="sRGB" x="0" y="0" width="100%" height="100%"><feComponentTransfer>{DISCRETE_4}</feComponentTransfer></filter>"#
  ))
}

/// Ordered Bayer dither at 4 levels per channel: tile the threshold matrix,
/// add it as `(bayer - 0.5) * binWidth`, quantize, restore the source alpha.
fn dither_filter(cell_px: u32) -> String {
  let tile = bayer_tile_uri();
  let tile_size = 4 * cell_px;
  filter_url(&format!(
    r#"<filter color-interpolation-filters="sRGB" x="0" y="0" width="100%" height="100%"><feImage href="{tile}" width="{tile_size}" height="{tile_size}" result="b"/><feTile in="b" result="tile"/><feComposite in="SourceGraphic" in2="tile" operator="arithmetic" k2="1" k3="0.25" k4="-0.125" result="noised"/><feComponentTransfer in="noised" result="quant">{DISCRETE_4}</feComponentTransfer><feComposite in="quant" in2="SourceAlpha" operator="in"/></filter>"#
  ))
}

/// 1-bit duotone in the dither-kit construction: luma as dot density,
/// full-range Bayer threshold, then a two-entry color table per channel.
fn duotone_filter(cell_px: u32, dark: [f32; 3], bright: [f32; 3]) -> String {
  let tile = bayer_tile_uri();
  let tile_size = 4 * cell_px;
  let table = |channel: usize| format!("{} {}", dark[channel], bright[channel]);
  filter_url(&format!(
    r#"<filter color-interpolation-filters="sRGB" x="0" y="0" width="100%" height="100%"><feImage href="{tile}" width="{tile_size}" height="{tile_size}" result="b"/><feTile in="b" result="tile"/><feColorMatrix in="SourceGraphic" type="matrix" values="0.2126 0.7152 0.0722 0 0 0.2126 0.7152 0.0722 0 0 0.2126 0.7152 0.0722 0 0 0 0 0 1 0" result="luma"/><feComposite in="luma" in2="tile" operator="arithmetic" k2="1" k3="1" k4="-0.5" result="noised"/><feComponentTransfer in="noised" result="bits"><feFuncR type="discrete" tableValues="0 1"/><feFuncG type="discrete" tableValues="0 1"/><feFuncB type="discrete" tableValues="0 1"/></feComponentTransfer><feComponentTransfer in="bits" result="tinted"><feFuncR type="table" tableValues="{r}"/><feFuncG type="table" tableValues="{g}"/><feFuncB type="table" tableValues="{b}"/></feComponentTransfer><feComposite in="tinted" in2="SourceAlpha" operator="in"/></filter>"#,
    r = table(0),
    g = table(1),
    b = table(2),
  ))
}

fn gradient_scene(filter: Option<&str>) -> Node {
  let mut style = Style::default()
    .with(StyleDeclaration::width(Px(260.0)))
    .with(StyleDeclaration::height(Px(180.0)))
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::justify_content(JustifyContent::FlexEnd))
    .with(StyleDeclaration::background_image(Some(
      BackgroundImages::from_css_str("linear-gradient(135deg, #8b5cf6, #1d4ed8 55%, #0b0d0c)")
        .unwrap(),
    )));

  if let Some(filter) = filter {
    style = style.with(StyleDeclaration::filter(
      Filters::from_css_str(filter).unwrap(),
    ));
  }

  let circle = Node::container(vec![]).with_style(
    Style::default()
      .with(StyleDeclaration::width(Px(88.0)))
      .with(StyleDeclaration::height(Px(88.0)))
      .with_margin(Sides([Px(18.0); 4]))
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(44.0)); 4])))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([241, 245, 249, 230]),
      ))),
  );

  Node::container(vec![circle]).with_style(style)
}

fn labeled_card(label: &str, content: Node) -> Node {
  Node::container(vec![
    content,
    Node::text(label.to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Block))),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with_gap(SpacePair::from_single(Px(12.0).into()))
      .with(StyleDeclaration::font_size(Px(18.0).into())),
  )
}

fn card_grid(children: Vec<Node>) -> Node {
  Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Grid))
      .with(StyleDeclaration::grid_template_columns(
        GridTemplateComponents::from_css_str("repeat(2, 1fr)").ok(),
      ))
      .with_gap(SpacePair::from_single(Px(16.0).into()))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::color(ColorInput::Value(Color([
        201, 212, 205, 255,
      ]))))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([11, 13, 12, 255]),
      ))),
  )
}

#[test]
fn test_style_filter_reference_dither() {
  let container = card_grid(vec![
    labeled_card("reference", gradient_scene(None)),
    labeled_card("posterize 4", gradient_scene(Some(&posterize_filter()))),
    labeled_card("dither 4 bayer", gradient_scene(Some(&dither_filter(1)))),
    labeled_card(
      "dither 4 bayer cell 3px",
      gradient_scene(Some(&dither_filter(3))),
    ),
  ]);

  run_fixture_test(container, "style_filter_reference_dither");
}

#[test]
fn test_style_filter_reference_duotone() {
  // dither-kit style button: a grayscale ramp is only the dot-density input;
  // the duotone table maps bits to the two brand colors.
  let button_ramp = Node::container(vec![]).with_style(
    Style::default()
      .with(StyleDeclaration::width(Px(220.0)))
      .with(StyleDeclaration::height(Px(72.0)))
      .with_border_radius(BorderRadius(Sides([SpacePair::from_single(Px(10.0)); 4])))
      .with(StyleDeclaration::background_image(Some(
        BackgroundImages::from_css_str("linear-gradient(to bottom, #4d4d4d, #ffffff)").unwrap(),
      )))
      .with(StyleDeclaration::filter(
        Filters::from_css_str(&duotone_filter(
          2,
          [0.075, 0.145, 0.310],
          [0.310, 0.553, 0.976],
        ))
        .unwrap(),
      )),
  );

  let phosphor = gradient_scene(Some(&duotone_filter(
    2,
    [0.043, 0.078, 0.055],
    [0.208, 0.878, 0.545],
  )));

  let chained = gradient_scene(Some(&format!(
    "contrast(1.4) {} brightness(1.1)",
    dither_filter(2)
  )));

  let container = card_grid(vec![
    labeled_card("duotone button", button_ramp),
    labeled_card("1-bit phosphor", phosphor),
    labeled_card("contrast + dither + brightness", chained),
    labeled_card("reference", gradient_scene(None)),
  ]);

  run_fixture_test(container, "style_filter_reference_duotone");
}
