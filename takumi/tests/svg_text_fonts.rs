//! A text SVG shared through the resource cache re-resolves fonts per
//! snapshot instead of keeping the first snapshot's glyphs.

mod test_utils;

use takumi::prelude::*;
use takumi_core::resources::image::{RenderedImage, ResourceCache};
use test_utils::repo_base_path;

const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"><text x="10" y="55" font-family="Swap" font-size="48" fill="black">Hgo</text></svg>"#;

fn fonts_with(path: &str) -> Fonts {
  let bytes = std::fs::read(repo_base_path(path)).unwrap();
  let mut fonts = Fonts::default();

  fonts
    .register(FontResource::new(bytes).override_info(FontOverride {
      family_name: Some("Swap".into()),
      ..Default::default()
    }))
    .unwrap();
  fonts
}

fn raster(source: &ImageSource, fonts: &Fonts) -> Vec<u8> {
  let RenderedImage::Rasterized(buffer) = source
    .render_for_layout(
      200,
      80,
      ImageScalingAlgorithm::Auto,
      0,
      Color::black(),
      Some(&fonts.snapshot()),
    )
    .unwrap()
  else {
    panic!("expected rasterized svg");
  };

  buffer.encode_png().unwrap()
}

#[test]
fn shared_text_svg_follows_the_font_snapshot() {
  let cache = ResourceCache::default();
  let source = cache
    .get_or_decode(SVG.as_bytes(), ImageCacheMode::Auto)
    .unwrap();

  let geist = fonts_with("assets/fonts/geist/Geist[wght].woff2");
  let mono = fonts_with("assets/fonts/geist/GeistMono[wght].woff2");

  let with_geist = raster(&source, &geist);
  let with_mono = raster(&source, &mono);

  assert_ne!(with_geist, with_mono);
  assert_eq!(with_geist, raster(&source, &geist));
}

fn has_ink(png: &[u8]) -> bool {
  let decoded = image::load_from_memory(png).unwrap().to_rgba8();

  decoded.pixels().any(|pixel| pixel.0[3] > 0)
}

#[test]
fn text_without_font_family_falls_back_to_a_registered_face() {
  let cache = ResourceCache::default();
  let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"><text x="10" y="55" font-size="48" fill="black">Hgo</text></svg>"#;
  let source = cache
    .get_or_decode(svg.as_bytes(), ImageCacheMode::Auto)
    .unwrap();
  let fonts = fonts_with("assets/fonts/geist/Geist[wght].woff2");

  assert!(has_ink(&raster(&source, &fonts)));
}

#[test]
fn namespace_prefixed_text_is_rendered() {
  let cache = ResourceCache::default();
  let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg" width="200" height="80"><s:text x="10" y="55" font-family="Swap" font-size="48" fill="black">Hgo</s:text></svg>"#;
  let source = cache
    .get_or_decode(svg.as_bytes(), ImageCacheMode::Auto)
    .unwrap();
  let fonts = fonts_with("assets/fonts/geist/Geist[wght].woff2");

  assert!(has_ink(&raster(&source, &fonts)));
}
