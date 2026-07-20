//! Platform-agnostic glue shared by the napi and wasm bindings.
//!
//! Both bindings lower raw JS input into a takumi render request the same way —
//! the embedded fallback fonts, a font resource from optional fields, the
//! stylesheet. That lowering lives here so neither binding re-derives it. Each
//! binding keeps only its platform-specific glue (JS type coercion, error
//! mapping, threading).

use takumi_core::{
  Fonts,
  resources::font::{FontError, FontOverride, FontResource},
  style::{FontStyle, KeyframesRule, StyleSheet},
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
  generic: Option<String>,
) -> Result<FontResource<'a>, FontError> {
  let resource = FontResource::new(bytes).override_info(FontOverride {
    family_name: name.map(Into::into),
    weight,
    style,
    ..Default::default()
  });

  let resource = match subset_of {
    Some(logical) => resource.subset_of(logical),
    None => resource,
  };

  match generic {
    Some(generic) => Ok(resource.generic_family(generic.parse()?)),
    None => Ok(resource),
  }
}

/// The stylesheet for a render: the loose-parsed sheet list with its keyframes.
pub fn stylesheet(stylesheets: Option<Vec<String>>, keyframes: Vec<KeyframesRule>) -> StyleSheet {
  let mut stylesheet = StyleSheet::parse_owned_list_loosy(stylesheets.unwrap_or_default());
  stylesheet.extend_keyframes(keyframes);
  stylesheet
}
