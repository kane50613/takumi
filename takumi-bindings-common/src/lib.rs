//! Platform-agnostic glue shared by the napi and wasm bindings.
//!
//! Both bindings lower raw JS input into a takumi render request the same way —
//! the embedded fallback fonts, a font resource from optional fields, the
//! stylesheet. That lowering lives here so neither binding re-derives it. Each
//! binding keeps only its platform-specific glue (JS type coercion, error
//! mapping, threading).

use std::sync::Arc;

use takumi_core::{
  Fonts,
  resources::{
    font::{FontError, FontOverride, FontResource},
    image::ResourceCache,
  },
  style::{CssSource, CssSourceError, FontStyle, KeyframesRule, StyleSheet},
};

/// Last-resort only: no generic family claim, so `sans-serif` and friends
/// resolve to caller-registered fonts via the fallback bucket instead of this
/// face.
const EMBEDDED_FONTS: &[(&[u8], &str)] = &[(
  include_bytes!("../../assets/fonts/geist/geist-latin-wght-300-800.woff2"),
  "Geist",
)];

/// The default font set holding the embedded last-resort fonts.
pub fn default_fonts() -> Result<Fonts, FontError> {
  let mut fonts = Fonts::default();

  for (bytes, name) in EMBEDDED_FONTS {
    let resource = FontResource::new(*bytes)
      .override_info(FontOverride {
        family_name: Some((*name).into()),
        ..Default::default()
      })
      .last_resort();

    drop(fonts.register(resource)?);
  }

  Ok(fonts)
}

/// Builds a font resource from normalized optional fields. Each binding pulls
/// these out of its own input type before calling in.
pub fn build_font_resource<'a>(
  bytes: &'a [u8],
  name: Option<String>,
  weight: Option<f32>,
  style: Option<FontStyle>,
  subset_of: Option<String>,
  subset_rank: Option<u32>,
  generic: Option<String>,
) -> Result<FontResource<'a>, FontError> {
  let resource = FontResource::new(bytes).override_info(FontOverride {
    family_name: name.map(Into::into),
    weight,
    style,
    ..Default::default()
  });

  let resource = match subset_of {
    Some(logical) => resource
      .subset_of(logical)
      .subset_rank(subset_rank.unwrap_or_default()),
    None => resource,
  };

  match generic {
    Some(generic) => Ok(resource.generic_family(generic.parse()?)),
    None => Ok(resource),
  }
}

/// The stylesheet for a render: the loose-parsed sheet list with its keyframes.
/// Parsed sheets are cached by source text in `cache`; per-render keyframes are
/// grafted onto a copy so the cached parse stays pristine.
pub fn stylesheet(
  cache: &ResourceCache,
  css: Option<Vec<CssSource>>,
  keyframes: Vec<KeyframesRule>,
) -> Result<Arc<StyleSheet>, CssSourceError> {
  let sheets = css
    .unwrap_or_default()
    .into_iter()
    .map(CssSource::into_css)
    .collect::<Result<Vec<_>, _>>()?;

  let sheet = cache.get_or_parse_stylesheet(sheets);

  if keyframes.is_empty() {
    return Ok(sheet);
  }

  let mut extended = (*sheet).clone();
  extended.extend_keyframes(keyframes);
  Ok(Arc::new(extended))
}
