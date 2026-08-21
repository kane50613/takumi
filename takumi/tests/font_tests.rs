use std::{
  assert_matches,
  fs::File,
  io::Read,
  path::{Path, PathBuf},
};

use takumi::{prelude::*, render};

fn font_path(path: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../assets/fonts/")
    .join(path)
    .to_path_buf()
}

fn read_font(path: &str) -> Vec<u8> {
  let mut data = Vec::new();
  File::open(font_path(path))
    .unwrap()
    .read_to_end(&mut data)
    .unwrap();
  data
}

/// Registers `path` as a uniquely-named coverage subset of the logical family `logical`.
fn register_subset(fonts: &mut Fonts, path: &str, unique_name: &str, logical: &str) {
  fonts
    .register(
      FontResource::new(read_font(path))
        .override_info(FontOverride {
          family_name: Some(unique_name.into()),
          ..Default::default()
        })
        .generic_family(GenericFamily::SANS_SERIF)
        .subset_of(logical),
    )
    .unwrap();
}

fn render_devanagari(fonts: &Fonts, family: &str) -> Bitmap {
  let node = Node::text("नमस्ते".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::font_size(FontSize::Length(Length::Px(
        72.0,
      ))))
      .with(StyleDeclaration::font_family(
        FontFamily::from_css_str(family).unwrap(),
      )),
  );

  render(
    RenderOptions::builder()
      .viewport(Viewport::new((400, 140)))
      .node(node)
      .fonts(fonts)
      .build(),
  )
  .unwrap()
}

fn inked_pixels(bitmap: &Bitmap) -> u32 {
  let (pixels, _) = bitmap.as_raw().as_chunks::<4>();

  pixels.iter().filter(|p| p[3] > 0).count() as u32
}

fn pixel_diff(a: &Bitmap, b: &Bitmap) -> u32 {
  let (a_pixels, _) = a.as_raw().as_chunks::<4>();
  let (b_pixels, _) = b.as_raw().as_chunks::<4>();

  a_pixels
    .iter()
    .zip(b_pixels)
    .filter(|(x, y)| x != y)
    .count() as u32
}

/// Two logical families, each with its OWN Devanagari subset, must route per family:
/// `font-family: Alpha` shapes with Alpha's Devanagari (Noto), `Beta` with Beta's
/// (Poppins). The pre-fix global fallback bucket was family-blind and gave both the same.
#[test]
fn subset_groups_route_per_family() {
  let mut fonts = Fonts::default();
  register_subset(
    &mut fonts,
    "geist/Geist[wght].woff2",
    "Alpha-latin",
    "Alpha",
  );
  register_subset(
    &mut fonts,
    "noto-sans/noto-sans-devanagari-v30-devanagari-regular.woff2",
    "Alpha-deva",
    "Alpha",
  );
  register_subset(
    &mut fonts,
    "archivo/Archivo-VariableFont_wdth,wght.ttf",
    "Beta-latin",
    "Beta",
  );
  register_subset(
    &mut fonts,
    "poppins/poppins-v24-devanagari_latin-regular.woff2",
    "Beta-deva",
    "Beta",
  );

  let alpha = render_devanagari(&fonts, "Alpha");
  let beta = render_devanagari(&fonts, "Beta");
  let beta_explicit = render_devanagari(&fonts, "\"Beta-latin\", \"Beta-deva\"");

  assert!(inked_pixels(&alpha) > 500, "Alpha must render real glyphs");
  assert!(inked_pixels(&beta) > 500, "Beta must render real glyphs");
  assert!(
    pixel_diff(&alpha, &beta) > 1000,
    "per-family routing: Alpha (Noto) and Beta (Poppins) Devanagari must differ"
  );
  assert_eq!(
    pixel_diff(&beta, &beta_explicit),
    0,
    "expanding `font-family: Beta` must match the explicit subset stack"
  );
}

#[test]
fn test_ttf_font_loading() {
  let mut context = Fonts::default();

  let mut font_data = Vec::new();
  File::open(font_path("noto-sans/NotoColorEmoji.ttf"))
    .unwrap()
    .read_to_end(&mut font_data)
    .unwrap();

  assert!(context.register(FontResource::new(font_data)).is_ok());
}

#[test]
fn test_ttc_font_loading() {
  let mut context = Fonts::default();

  let mut font_data = Vec::new();
  File::open(font_path("ubuntu/Ubuntu.ttc"))
    .unwrap()
    .read_to_end(&mut font_data)
    .unwrap();

  assert!(context.register(FontResource::new(font_data)).is_ok());
}

#[test]
fn test_woff2_font_loading() {
  let mut context = Fonts::default();

  let mut font_data = Vec::new();
  File::open(font_path("geist/Geist[wght].woff2"))
    .unwrap()
    .read_to_end(&mut font_data)
    .unwrap();

  assert!(context.register(FontResource::new(font_data)).is_ok());
}

#[test]
fn test_invalid_format_detection() {
  // Test with invalid data
  let invalid_data = vec![0x00, 0x01, 0x02, 0x03];
  let mut context = Fonts::default();

  let result = context.register(FontResource::new(invalid_data));
  assert_matches!(result, Err(FontError::UnsupportedFormat));
}

#[test]
fn test_empty_data() {
  // Test with empty data
  let empty_data = Vec::<u8>::new();
  let mut context = Fonts::default();

  let result = context.register(FontResource::new(empty_data));
  assert_matches!(result, Err(FontError::UnsupportedFormat));
}

#[test]
fn test_too_short_data() {
  // Test with data too short for format detection
  let short_data = vec![0x00, 0x01, 0x00];
  let mut context = Fonts::default();

  let result = context.register(FontResource::new(short_data));
  assert_matches!(result, Err(FontError::UnsupportedFormat));
}
