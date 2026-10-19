//! Fonts for shaping tests.

use std::{fs::read, path::Path, rc::Rc};

use crate::{
  font_style::ExpandedFontFamily,
  fonts::{
    font::Font,
    font_description::{FontDescription, FontSelectionDescription},
    font_selection_types::{
      FontSelectionRequest, NORMAL_SLOPE_VALUE, NORMAL_WEIGHT_VALUE, NORMAL_WIDTH_VALUE,
    },
  },
  resources::font::{FontOverride, FontResource, Fonts},
  style::{FontFamily, FontLanguageOverride, FontSynthesic, FontVariantCaps, FromCssStr},
};

pub(crate) const GEIST: &str = "../assets/fonts/geist/Geist[wght].woff2";
pub(crate) const GEIST_MONO: &str = "../assets/fonts/geist/GeistMono[wght].woff2";
pub(crate) const NOTO_SANS_TC: &str =
  "../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2";

/// Registers the font at `path`, from the crate root, as `family_name`.
pub(crate) fn register(fonts: &mut Fonts, path: &str, family_name: &str) {
  let bytes = read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();

  fonts
    .register(FontResource::new(bytes).override_info(FontOverride {
      family_name: Some(family_name.into()),
      ..Default::default()
    }))
    .unwrap();
}

/// A 16px font for the `font-family` list `family`.
pub(crate) fn font(fonts: &mut Fonts, family: &str) -> Font {
  let family = FontFamily::from_css_str(family).unwrap();

  Font::new(
    fonts,
    FontDescription {
      family: Rc::new(ExpandedFontFamily::expand(&family)),
      selection: FontSelectionDescription::for_request(FontSelectionRequest::new(
        NORMAL_WEIGHT_VALUE,
        NORMAL_WIDTH_VALUE,
        NORMAL_SLOPE_VALUE,
      )),
      computed_size: 16.0,
      specified_size: 16.0,
      features: Vec::new(),
      locale: None,
      language_override: FontLanguageOverride::Normal,
      variant_caps: FontVariantCaps::Normal,
      font_synthesis_small_caps: FontSynthesic::Auto,
      letter_spacing: 0.0,
      word_spacing: 0.0,
      is_letter_spacing_fixed: true,
      is_word_spacing_fixed: true,
    },
  )
}

/// Geist, alone.
pub(crate) fn geist() -> (Fonts, Font) {
  let mut fonts = Fonts::default();

  register(&mut fonts, GEIST, "Geist");

  let font = font(&mut fonts, "Geist");

  (fonts, font)
}

/// Geist Mono, alone: no kerning.
pub(crate) fn geist_mono() -> (Fonts, Font) {
  let mut fonts = Fonts::default();

  register(&mut fonts, GEIST_MONO, "Geist Mono");

  let font = font(&mut fonts, "'Geist Mono'");

  (fonts, font)
}
