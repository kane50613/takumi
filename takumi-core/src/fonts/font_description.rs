//! What shaping reads of a font's style, named for Blink's `FontDescription`.

use std::rc::Rc;

use std::str::FromStr;

use harfrust::{Feature, Language, Tag, font::Variation};

use super::font_selection_types::FontSelectionRequest;
use crate::{
  font_style::{ExpandedFontFamily, SizedFontStyle},
  sort_key::sort_by_key,
  style::{FontLanguageOverride, FontOpticalSizing, FontSynthesic, FontVariantCaps},
};

/// The style a font is matched and shaped with.
#[derive(Clone, PartialEq)]
pub(crate) struct FontDescription {
  /// The families, matching, and sizes its fallback list reads.
  pub(crate) fallback: FontFallbackKey,
  /// Blink's `FontFeatures`.
  pub(crate) features: Vec<Feature>,
  /// The tags of Blink's `FeatureSettings`, which `font-feature-settings` sets.
  pub(crate) feature_setting_tags: Vec<Tag>,
  /// Blink's `LocaleOrDefault` as a HarfBuzz language.
  pub(crate) locale: Option<Language>,
  /// `font-language-override`, which Blink lacks.
  pub(crate) language_override: FontLanguageOverride,
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
  /// The language HarfBuzz shapes in: the locale, with `font-language-override`'s language
  /// system as a private-use `x-hbot` subtag, which HarfBuzz reads in place of the locale's.
  pub(crate) fn shaping_language(&self) -> Option<Language> {
    let FontLanguageOverride::Tag(tag) = self.language_override else {
      return self.locale.clone();
    };
    let ot_language = format!("x-hbot-{:08x}", u32::from_be_bytes(tag));
    let language = match &self.locale {
      Some(locale) => format!("{}-{ot_language}", locale.as_str()),
      None => ot_language,
    };

    Language::from_str(&language).ok()
  }

  /// The description of `style`'s font, as Blink's `FontBuilder` resolves it.
  pub(crate) fn new(style: &SizedFontStyle<'_>) -> Self {
    let computed = style.parent;

    Self {
      fallback: FontFallbackKey {
        family: Rc::clone(&style.font_family),
        selection: computed.font_selection_description(&style.sizing),
        computed_size: style.sizing.font_size,
        specified_size: style.sizing.font_size,
      },
      features: computed
        .resolved_font_features(style.letter_spacing != 0.0, FontVariantCaps::Normal)
        .iter()
        .map(|feature| feature.into_harfrust())
        .collect(),
      feature_setting_tags: computed
        .inherited_data
        .font_feature_settings
        .iter()
        .map(|feature| feature.tag.into_harfrust())
        .collect(),
      locale: computed
        .lang
        .as_ref()
        .and_then(|lang| Language::from_str(lang.as_str()).ok()),
      language_override: computed.inherited_data.font_language_override,
      variant_caps: style.caps,
      font_synthesis_small_caps: computed.inherited_data.font_synthesis_small_caps,
      letter_spacing: style.letter_spacing,
      word_spacing: style.word_spacing,
      is_letter_spacing_fixed: !computed.inherited_data.letter_spacing.has_percentage(),
      is_word_spacing_fixed: !computed.inherited_data.word_spacing.has_percentage(),
    }
  }
}

/// What a [`FontFallbackList`](super::font_fallback_list::FontFallbackList) reads of Blink's
/// `FontDescription`, so styles that differ only in shaping share one list.
#[derive(Clone, PartialEq)]
pub(crate) struct FontFallbackKey {
  /// The `font-family` list.
  pub(crate) family: Rc<ExpandedFontFamily>,
  /// What font matching reads of the style, apart from the families.
  pub(crate) selection: FontSelectionDescription,
  /// Blink's `ComputedSize`.
  pub(crate) computed_size: f32,
  /// Blink's `SpecifiedSize`.
  pub(crate) specified_size: f32,
}

/// What font matching reads of Blink's `FontDescription`, apart from the families.
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
pub(crate) fn sort_by_tag(settings: &mut [Variation]) {
  sort_by_key(settings, |axis| u32::from_be_bytes(axis.tag.to_be_bytes()));
}
