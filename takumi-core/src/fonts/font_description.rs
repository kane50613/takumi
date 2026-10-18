//! What shaping reads of a font's style, named for Blink's `FontDescription`.

use std::rc::Rc;

use std::str::FromStr;

use harfrust::{Feature, Language, Variation};
use parley::fontique::Attributes;

use crate::{
  font_style::{ExpandedFontFamily, SizedFontStyle},
  style::{FontSynthesic, FontVariantCaps},
};

/// The style a font is matched and shaped with.
#[derive(Clone, PartialEq)]
pub(crate) struct FontDescription {
  /// The `font-family` list.
  pub(crate) family: Rc<ExpandedFontFamily>,
  /// `font-weight`, `font-style`, and `font-stretch` as font matching takes them.
  pub(crate) attributes: Attributes,
  /// Blink's `ComputedSize`.
  pub(crate) computed_size: f32,
  /// Blink's `SpecifiedSize`.
  pub(crate) specified_size: f32,
  /// Blink's `FontFeatures`.
  pub(crate) features: Vec<Feature>,
  /// `font-variation-settings`.
  pub(crate) variation_settings: Vec<Variation>,
  /// Blink's `LocaleOrDefault` as a HarfBuzz language.
  pub(crate) locale: Option<Language>,
  /// Blink's `VariantCaps`, whose features the shaper adds.
  pub(crate) variant_caps: FontVariantCaps,
  /// Blink's `GetFontSynthesisSmallCaps`.
  pub(crate) font_synthesis_small_caps: FontSynthesic,
  /// Blink's `LetterSpacing`, in pixels.
  pub(crate) letter_spacing: f32,
  /// Blink's `WordSpacing`, in pixels.
  pub(crate) word_spacing: f32,
}

impl FontDescription {
  /// The description of `style`'s font, as Blink's `FontBuilder` resolves it.
  pub(crate) fn new(style: &SizedFontStyle<'_>) -> Self {
    let computed = style.parent;

    Self {
      family: Rc::clone(&style.font_family),
      attributes: computed.font_attributes(),
      computed_size: style.sizing.font_size,
      specified_size: style.sizing.font_size,
      features: computed
        .resolved_font_features(style.letter_spacing != 0.0, FontVariantCaps::Normal)
        .iter()
        .map(|feature| feature.into_harfrust())
        .collect(),
      variation_settings: style
        .font_variations
        .iter()
        .map(|variation| Variation {
          tag: variation.tag.into_harfrust(),
          value: variation.value,
        })
        .collect(),
      locale: computed
        .lang
        .as_ref()
        .and_then(|lang| Language::from_str(lang.as_str()).ok()),
      variant_caps: style.caps,
      font_synthesis_small_caps: computed.inherited_data.font_synthesis_small_caps,
      letter_spacing: style.letter_spacing,
      word_spacing: style.word_spacing,
    }
  }
}
