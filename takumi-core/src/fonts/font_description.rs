//! What shaping reads of a font's style, named for Blink's `FontDescription`.

use std::rc::Rc;

use std::str::FromStr;

use harfrust::{Feature, Language, Variation};
use parley::fontique::Attributes;

use super::font_selection_types::FontSelectionRequest;
use crate::{
  font_style::{ExpandedFontFamily, SizedFontStyle},
  sort_key::sort_by_key,
  style::{FontOpticalSizing, FontSynthesic, FontVariantCaps},
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
  /// Whether `letter-spacing` is a length, which Blink's `ComputedLetterSpacing` keeps fixed.
  pub(crate) is_letter_spacing_fixed: bool,
  /// Whether `word-spacing` is a length, which Blink's `ComputedWordSpacing` keeps fixed.
  pub(crate) is_word_spacing_fixed: bool,
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
      is_letter_spacing_fixed: !computed.inherited_data.letter_spacing.has_percentage(),
      is_word_spacing_fixed: !computed.inherited_data.word_spacing.has_percentage(),
    }
  }
}

/// What font matching reads of Blink's `FontDescription`, apart from the families; joins
/// [`FontDescription`] in place of its fontique `attributes` once matching drops fontique.
#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: font matching moves off fontique once the registry is rewired"
  )
)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FontSelectionDescription {
  /// Blink's `GetFontSelectionRequest`.
  pub(crate) font_selection_request: FontSelectionRequest,
  /// Whether `font-style` is the `italic` keyword, Blink's `StyleSyntax::kItalicKeyword`.
  pub(crate) is_italic_keyword: bool,
  /// Blink's `SyntheticBoldAllowed`.
  pub(crate) synthetic_bold_allowed: bool,
  /// Blink's `SyntheticItalicAllowed`.
  pub(crate) synthetic_italic_allowed: bool,
  /// Blink's `IsSyntheticBold`.
  pub(crate) is_synthetic_bold: bool,
  /// Blink's `IsSyntheticItalic`.
  pub(crate) is_synthetic_italic: bool,
  /// Blink's `EffectiveFontSize`.
  pub(crate) effective_font_size: f32,
  /// Blink's `AdjustedSpecifiedSize`, which an automatic `opsz` takes.
  pub(crate) adjusted_specified_size: f32,
  /// Blink's `FontOpticalSizing`.
  pub(crate) font_optical_sizing: FontOpticalSizing,
  /// Blink's `VariationSettings`.
  pub(crate) variation_settings: Vec<Variation>,
}

#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: font matching moves off fontique once the registry is rewired"
  )
)]
impl FontSelectionDescription {
  /// Blink's `MergeFontVariationSettingsWithDescriptor`: the descriptor's axes the description
  /// leaves unset join its own, ordered by tag.
  pub(crate) fn merge_font_variation_settings_with_descriptor(&mut self, descriptor: &[Variation]) {
    let mut merged = self.variation_settings.clone();

    for axis in descriptor {
      if !self
        .variation_settings
        .iter()
        .any(|existing| existing.tag == axis.tag)
      {
        merged.push(*axis);
      }
    }

    sort_by_key(&mut merged, |axis| {
      u32::from_be_bytes(axis.tag.to_be_bytes())
    });
    self.variation_settings = merged;
  }

  /// A 16px description of `request` that allows synthesis and leaves `opsz` alone.
  #[cfg(test)]
  pub(crate) fn for_request(font_selection_request: FontSelectionRequest) -> Self {
    Self {
      font_selection_request,
      is_italic_keyword: false,
      synthetic_bold_allowed: true,
      synthetic_italic_allowed: true,
      is_synthetic_bold: false,
      is_synthetic_italic: false,
      effective_font_size: 16.0,
      adjusted_specified_size: 16.0,
      font_optical_sizing: FontOpticalSizing::None,
      variation_settings: Vec::new(),
    }
  }
}

/// Orders variation settings by tag, as Blink's `FontVariationAxis` compares.
#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: font matching moves off fontique once the registry is rewired"
  )
)]
pub(crate) fn sort_by_tag(settings: &mut [Variation]) {
  sort_by_key(settings, |axis| u32::from_be_bytes(axis.tag.to_be_bytes()));
}
