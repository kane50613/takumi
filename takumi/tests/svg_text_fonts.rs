//! A text SVG shared through the resource cache re-resolves fonts per
//! snapshot instead of keeping the first snapshot's glyphs.

use std::fs;

use takumi::prelude::*;
use takumi_core::{
  context::RenderContext,
  resources::image::{RenderedImage, ResourceCache},
};

use crate::test_utils::{repo_base_path, write_goldens};

const GEIST: &str = "assets/fonts/geist/Geist[wght].woff2";

const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"><text x="10" y="55" font-family="Swap" font-size="48" fill="black">Hgo</text></svg>"#;

fn fonts_with(path: &str) -> Fonts {
  let bytes = fs::read(repo_base_path(path)).unwrap();
  let mut fonts = Fonts::default();

  fonts
    .register(FontResource::new(bytes).override_info(FontOverride {
      family_name: Some("Swap".into()),
      ..Default::default()
    }))
    .unwrap();
  fonts
}

fn decode(cache: &ResourceCache, svg: &str) -> ImageSource {
  cache
    .get_or_decode(svg.as_bytes(), ImageCacheMode::Auto)
    .unwrap()
}

fn raster(source: &ImageSource, fonts: &Fonts) -> Vec<u8> {
  let host = RenderContext::builder()
    .fonts(fonts.snapshot())
    .sizing(
      SizingContext::builder()
        .viewport(Viewport::new((200, 80)))
        .build(),
    )
    .build();
  let RenderedImage::Rasterized(buffer) = source
    .render_for_layout(200, 80, ImageScalingAlgorithm::Auto, 0, &host)
    .unwrap()
  else {
    panic!("expected rasterized svg");
  };

  buffer.data().to_vec()
}

#[test]
fn shared_text_svg_follows_the_font_snapshot() {
  let cache = ResourceCache::default();
  let source = decode(&cache, SVG);

  let geist = fonts_with(GEIST);
  let mono = fonts_with("assets/fonts/geist/GeistMono[wght].woff2");

  let with_geist = raster(&source, &geist);
  let with_mono = raster(&source, &mono);

  assert_ne!(with_geist, with_mono);
  assert_eq!(with_geist, raster(&source, &geist));
}

fn has_ink(rgba: &[u8]) -> bool {
  rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] > 0)
}

#[test]
fn text_without_font_family_falls_back_to_a_registered_face() {
  let cache = ResourceCache::default();
  let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"><text x="10" y="55" font-size="48" fill="black">Hgo</text></svg>"#;
  let source = decode(&cache, svg);
  let fonts = fonts_with(GEIST);

  assert!(has_ink(&raster(&source, &fonts)));
}

#[test]
fn namespace_prefixed_text_is_rendered() {
  let cache = ResourceCache::default();
  let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg" width="200" height="80"><s:text x="10" y="55" font-family="Swap" font-size="48" fill="black">Hgo</s:text></svg>"#;
  let source = decode(&cache, svg);
  let fonts = fonts_with(GEIST);

  assert!(has_ink(&raster(&source, &fonts)));
}

#[test]
fn empty_and_paragraph_separator_text_do_not_panic() {
  let cache = ResourceCache::default();
  let fonts = fonts_with(GEIST);

  for svg in [
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><text></text></svg>"#,
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40"><text x="0" y="20" font-size="16">A&#x2029;B</text></svg>"#,
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><text> </text></svg>"#,
  ] {
    let source = decode(&cache, svg);

    raster(&source, &fonts);
  }
}

/// SVG `<text>` draws a bitmap glyph in its own colors and its fill's opacity, and leaves it out
/// without a fill, as Skia draws color glyphs.
#[test]
fn bitmap_glyphs_draw_in_svg_text() {
  let mut fonts = Fonts::default();

  for path in [GEIST, "assets/fonts/noto-sans/NotoColorEmoji.ttf"] {
    fonts
      .register(FontResource::new(fs::read(repo_base_path(path)).unwrap()))
      .unwrap();
  }

  let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='360' height='120'>\
    <rect width='360' height='120' fill='white'/>\
    <text x='12' y='50' font-family='Geist' font-size='36' fill='black'>Aa \u{1F600}\u{1F389}</text>\
    <text x='12' y='100' font-family='Geist' font-size='36' fill='black' fill-opacity='0.4'>Aa \u{1F600}</text>\
    <text x='220' y='100' font-family='Geist' font-size='36' fill='none' stroke='black'>B\u{1F600}</text>\
    </svg>";
  let node = Node::image((
    format!("data:image/svg+xml;utf8,{svg}").as_str(),
    360.0,
    120.0,
  ));

  write_goldens(
    RenderOptions::builder()
      .viewport(Viewport::new((360, 120)))
      .node(node)
      .fonts(&fonts)
      .build(),
    "svg_text_bitmap_glyphs",
  )
  .unwrap();
}
