use std::{assert_matches, fs, sync::Arc};

use takumi::{prelude::*, render, render_svg};

use crate::test_utils::{rasterize_svg, repo_base_path, write_goldens};

fn read_font(path: &str) -> Vec<u8> {
  fs::read(repo_base_path("assets/fonts").join(path)).unwrap()
}

const LATIN: UnicodeRange = UnicodeRange::new(0x0000, 0x00FF);
const DEVANAGARI: UnicodeRange = UnicodeRange::new(0x0900, 0x097F);

/// Registers `path` under `family`, serving only `range`, as one `@font-face` subset would.
fn register_subset(fonts: &mut Fonts, path: &str, family: &str, range: UnicodeRange) {
  fonts
    .register(
      FontResource::new(read_font(path))
        .override_info(FontOverride {
          family_name: Some(family.into()),
          ..Default::default()
        })
        .generic_family(GenericFamily::SANS_SERIF)
        .unicode_range(UnicodeRangeSet::from_iter([range])),
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
fn subsets_route_per_family() {
  let mut fonts = Fonts::default();
  register_subset(&mut fonts, "geist/Geist[wght].woff2", "Alpha", LATIN);
  register_subset(
    &mut fonts,
    "noto-sans/noto-sans-devanagari-v30-devanagari-regular.woff2",
    "Alpha",
    DEVANAGARI,
  );
  register_subset(
    &mut fonts,
    "archivo/Archivo-VariableFont_wdth,wght.ttf",
    "Beta",
    LATIN,
  );
  register_subset(
    &mut fonts,
    "poppins/poppins-v24-devanagari_latin-regular.woff2",
    "Beta",
    DEVANAGARI,
  );

  let alpha = render_devanagari(&fonts, "Alpha");
  let beta = render_devanagari(&fonts, "Beta");

  assert!(inked_pixels(&alpha) > 500, "Alpha must render real glyphs");
  assert!(inked_pixels(&beta) > 500, "Beta must render real glyphs");
  assert!(
    pixel_diff(&alpha, &beta) > 1000,
    "per-family routing: Alpha (Noto) and Beta (Poppins) Devanagari must differ"
  );
}

#[test]
fn test_ttc_font_loading() {
  let mut context = Fonts::default();

  assert!(
    context
      .register(FontResource::new(read_font("ubuntu/Ubuntu.ttc")))
      .is_ok()
  );
}

#[test]
fn test_invalid_format_detection() {
  let invalid_data = vec![0x00, 0x01, 0x02, 0x03];
  let mut context = Fonts::default();

  let result = context.register(FontResource::new(invalid_data));
  assert_matches!(result, Err(FontError::UnsupportedFormat));
}

#[test]
fn test_empty_data() {
  let empty_data = Vec::<u8>::new();
  let mut context = Fonts::default();

  let result = context.register(FontResource::new(empty_data));
  assert_matches!(result, Err(FontError::UnsupportedFormat));
}

#[test]
fn test_too_short_data() {
  let short_data = vec![0x00, 0x01, 0x00];
  let mut context = Fonts::default();

  let result = context.register(FontResource::new(short_data));
  assert_matches!(result, Err(FontError::UnsupportedFormat));
}

/// Pixels at or below row 100 that are opaque red.
fn red_rows_below_100(pixels: &[[u8; 4]], width: usize) -> usize {
  pixels
    .iter()
    .enumerate()
    .filter(|(index, pixel)| {
      index / width >= 100 && pixel[0] > 200 && pixel[1] < 50 && pixel[3] > 200
    })
    .count()
}

#[test]
fn a_bitmap_glyph_casts_a_text_shadow() {
  let mut fonts = Fonts::default();

  fonts
    .register(FontResource::new(read_font("noto-sans/NotoColorEmoji.ttf")))
    .unwrap();

  let node = Node::text("😀".to_string()).with_class_name("emoji");
  let stylesheet: Arc<StyleSheet> = StyleSheet::parse(
    ".emoji { font-size: 64px; line-height: 1; text-shadow: 0 100px 0 rgb(255, 0, 0); }",
  )
  .unwrap()
  .into();
  let viewport = Viewport::new((120, 200));
  let bitmap = render(
    RenderOptions::builder()
      .viewport(viewport)
      .node(node.clone())
      .fonts(&fonts)
      .stylesheet(stylesheet.clone())
      .build(),
  )
  .unwrap();
  let svg = render_svg(
    SvgOptions::builder()
      .viewport(viewport)
      .node(node)
      .fonts(&fonts)
      .stylesheet(stylesheet)
      .build(),
  )
  .unwrap();
  let rasterized = rasterize_svg(svg, 120, 200);
  let raster = red_rows_below_100(bitmap.as_raw().as_chunks::<4>().0, 120);
  let vector = red_rows_below_100(rasterized.as_raw().as_chunks::<4>().0, 120);

  assert!(
    raster > 500,
    "the raster emoji casts no shadow: {raster} red pixels"
  );
  assert!(
    vector > 500,
    "the SVG emoji casts no shadow: {vector} red pixels"
  );
}

#[test]
fn an_off_canvas_bitmap_glyph_blurs_its_shadow_onto_the_canvas() {
  let mut fonts = Fonts::default();

  fonts
    .register(FontResource::new(read_font("noto-sans/NotoColorEmoji.ttf")))
    .unwrap();

  let paint = |shadow: &str| {
    let stylesheet: Arc<StyleSheet> = StyleSheet::parse(&format!(
      ".emoji {{ font-size: 64px; line-height: 1; text-shadow: {shadow}; }}"
    ))
    .unwrap()
    .into();

    render(
      RenderOptions::builder()
        .viewport(Viewport::new((120, 80)))
        .node(Node::text("😀".to_string()).with_class_name("emoji"))
        .fonts(&fonts)
        .stylesheet(stylesheet)
        .build(),
    )
    .unwrap()
  };
  let shadowed = paint("-72px 0 16px rgb(255, 0, 0)");
  let plain = paint("none");
  let changed = shadowed
    .as_raw()
    .as_chunks::<4>()
    .0
    .iter()
    .zip(plain.as_raw().as_chunks::<4>().0)
    .filter(|(shadowed, plain)| shadowed != plain)
    .count();

  assert!(
    changed > 500,
    "the shadow loses its off-canvas part before the blur: {changed} pixels"
  );
}

/// Unregistering a family removes every subset of it, so it renders as an unknown family does,
/// and registering it again brings it back.
#[test]
fn unregister_removes_every_subset() {
  let mut fonts = Fonts::default();
  register_subset(&mut fonts, "geist/Geist[wght].woff2", "Alpha", LATIN);
  register_subset(
    &mut fonts,
    "noto-sans/noto-sans-devanagari-v30-devanagari-regular.woff2",
    "Alpha",
    DEVANAGARI,
  );
  register_subset(
    &mut fonts,
    "poppins/poppins-v24-devanagari_latin-regular.woff2",
    "Beta",
    DEVANAGARI,
  );

  let registered = render_devanagari(&fonts, "Alpha");

  assert!(
    fonts.unregister_family("alpha"),
    "family names match without case"
  );
  assert!(
    !fonts.unregister_family("Alpha"),
    "a removed family has nothing left"
  );

  let removed = render_devanagari(&fonts, "Alpha");

  assert_eq!(pixel_diff(&removed, &render_devanagari(&fonts, "Gamma")), 0);
  assert!(pixel_diff(&registered, &removed) > 1000);

  register_subset(&mut fonts, "geist/Geist[wght].woff2", "Alpha", LATIN);
  register_subset(
    &mut fonts,
    "noto-sans/noto-sans-devanagari-v30-devanagari-regular.woff2",
    "Alpha",
    DEVANAGARI,
  );

  assert_eq!(
    pixel_diff(&render_devanagari(&fonts, "Alpha"), &registered),
    0
  );
}

const REREGISTERED_SIZE: (u32, u32) = (360, 180);

fn reregistered_fonts() -> Fonts {
  let mut fonts = Fonts::default();
  let regular = read_font("poppins/poppins-v24-devanagari_latin-regular.woff2");

  for (family, weight, style) in [
    ("Alpha", None, None),
    ("Beta", Some(400.0), None),
    ("Beta", Some(700.0), None),
    ("Beta", None, Some(FontStyle::italic())),
  ] {
    fonts
      .register(
        FontResource::new(regular.clone()).override_info(FontOverride {
          family_name: Some(family.into()),
          weight,
          style,
          ..Default::default()
        }),
      )
      .unwrap();
  }

  fonts
}

fn render_reregistered(fonts: &Fonts, declaration: &str) -> Bitmap {
  let html = format!(
    r#"<div style="display: flex; width: 360px; height: 180px; background-color: white; color: black; font-size: 48px; {declaration}">Ticket 7</div>"#
  );

  render(
    RenderOptions::builder()
      .viewport(Viewport::new(REREGISTERED_SIZE))
      .node(Node::from_html(&html, FromHtmlOptions::default()).unwrap())
      .fonts(fonts)
      .build(),
  )
  .unwrap()
}

/// One file registered under other families, weights and styles draws unsynthesized in each.
#[test]
fn a_font_registered_again_under_other_descriptors_keeps_each_face() {
  let fonts = reregistered_fonts();
  let alpha = render_reregistered(&fonts, "font-family: Alpha");

  for declaration in [
    "font-family: Beta",
    "font-family: Beta; font-weight: 700",
    "font-family: Beta; font-style: italic",
  ] {
    assert_eq!(
      pixel_diff(&render_reregistered(&fonts, declaration), &alpha),
      0,
      "{declaration}"
    );
  }

  let rows = [
    "font-family: Alpha",
    "font-family: Beta; font-weight: 700",
    "font-family: Beta; font-style: italic",
  ]
  .map(|declaration| format!(r#"<span style="{declaration}">Ticket 7</span>"#))
  .join("");
  let html = format!(
    r#"<div style="display: flex; flex-direction: column; width: 360px; height: 180px; background-color: white; color: black; font-size: 40px">{rows}</div>"#
  );

  write_goldens(
    RenderOptions::builder()
      .viewport(Viewport::new(REREGISTERED_SIZE))
      .node(Node::from_html(&html, FromHtmlOptions::default()).unwrap())
      .fonts(&fonts)
      .build(),
    "font_registered_again_under_other_descriptors",
  )
  .unwrap();
}
