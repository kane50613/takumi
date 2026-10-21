//! Blink's [`FontFace`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/font_face.cc),
//! with the part of [`CSSFontFace`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/css_font_face.cc)
//! and `BinaryDataFontFaceSource` a registered face needs: one source, already decoded.
//!
//! An `auto` descriptor takes the face's own capabilities, as CSS Fonts 4 asks, where Blink
//! takes `normal`; see [`FontCustomPlatformData::auto_font_selection_capabilities`].

use std::sync::Arc;

use harfrust::font::Variation;

use super::{
  font_custom_platform_data::{FontCustomPlatformData, FontPlatformData},
  font_description::{FontSelectionDescription, sort_by_tag},
  font_selection_types::{
    FontSelectionCapabilities, FontSelectionRange, FontSelectionValue, RangeType,
  },
};
use crate::resources::unicode_range::UnicodeRangeSet;

/// The `@font-face` descriptors a face registers with; `None` is `auto`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FontFaceDescriptors {
  /// `font-family`, in place of the face's own family name.
  pub(crate) family: Option<Arc<str>>,
  /// `font-weight`, its start and end.
  pub(crate) weight: Option<(f32, f32)>,
  /// `font-stretch`, its start and end percentages.
  pub(crate) width: Option<(f32, f32)>,
  /// `font-style` as oblique angles: `normal` is `(0, 0)`, `italic` and bare `oblique` `(14, 14)`.
  pub(crate) slope: Option<(f32, f32)>,
  /// `unicode-range`.
  pub(crate) unicode_range: UnicodeRangeSet,
  /// `font-variation-settings`.
  pub(crate) variation_settings: Vec<Variation>,
}

/// Blink's `FontFace`.
#[derive(Debug)]
pub(crate) struct FontFace {
  /// Blink's `familyNameUnquoted`.
  pub(crate) family: Arc<str>,
  pub(crate) descriptors: FontFaceDescriptors,
  pub(crate) custom_platform_data: Arc<FontCustomPlatformData>,
}

impl FontFace {
  pub(crate) fn new(
    family: Arc<str>,
    descriptors: FontFaceDescriptors,
    custom_platform_data: Arc<FontCustomPlatformData>,
  ) -> Self {
    Self {
      family,
      descriptors,
      custom_platform_data,
    }
  }

  /// Blink's `GetFontSelectionCapabilities`.
  pub(crate) fn get_font_selection_capabilities(&self) -> FontSelectionCapabilities {
    let auto_capabilities = self.custom_platform_data.auto_font_selection_capabilities();
    let mut capabilities = auto_capabilities;

    if let Some(width) = self.descriptors.width {
      capabilities.width = explicit_range(width);
    }

    if let Some(slope) = self.descriptors.slope {
      capabilities.slope = explicit_range(slope);
    }

    if let Some((from, to)) = self.descriptors.weight {
      if from < 1.0 || to > 1000.0 {
        return auto_capabilities;
      }

      capabilities.weight = explicit_range((from, to));
    }

    capabilities
  }

  /// Blink's `GetFontVariationSettings`: the descriptor's axes by tag, the last of a tag winning.
  pub(crate) fn get_font_variation_settings(&self) -> Vec<Variation> {
    let mut settings: Vec<Variation> = Vec::new();

    for variation in &self.descriptors.variation_settings {
      match settings
        .iter_mut()
        .find(|existing| existing.tag == variation.tag)
      {
        Some(existing) => existing.value = variation.value,
        None => settings.push(*variation),
      }
    }

    sort_by_tag(&mut settings);
    settings
  }

  /// Blink's `CSSFontFace::Ranges`.
  pub(crate) fn ranges(&self) -> &UnicodeRangeSet {
    &self.descriptors.unicode_range
  }

  /// Blink's `CSSFontFace::GetFontData`, through `BinaryDataFontFaceSource::CreateFontData`.
  pub(crate) fn get_font_data(
    &self,
    font_description: &FontSelectionDescription,
  ) -> FontPlatformData {
    let mut description = font_description.clone();

    description.merge_font_variation_settings_with_descriptor(&self.get_font_variation_settings());
    self
      .custom_platform_data
      .get_font_platform_data(&description, &self.get_font_selection_capabilities())
  }
}

/// A descriptor's start and end as a range, swapped so it never decreases.
/// <https://drafts.csswg.org/css-fonts/#font-prop-desc>
fn explicit_range((from, to): (f32, f32)) -> FontSelectionRange {
  let (minimum, maximum) = if from < to { (from, to) } else { (to, from) };

  FontSelectionRange::with_type(
    FontSelectionValue::new(minimum),
    FontSelectionValue::new(maximum),
    RangeType::SetExplicitly,
  )
}

#[cfg(test)]
mod tests {
  use harfrust::Tag;

  use super::*;
  use crate::fonts::font_selection_types::{
    BOLD_WEIGHT_VALUE, ITALIC_SLOPE_VALUE, NORMAL_WEIGHT_VALUE,
  };

  const GEIST: &str = "../assets/fonts/geist/Geist[wght].woff2";

  fn geist(descriptors: FontFaceDescriptors) -> FontFace {
    FontFace::new(
      "Geist".into(),
      descriptors,
      FontCustomPlatformData::from_asset(GEIST).remove(0),
    )
  }

  #[test]
  fn auto_descriptors_take_the_face_s_own_capabilities() {
    let face = geist(FontFaceDescriptors::default());

    assert_eq!(
      face.get_font_selection_capabilities(),
      face.custom_platform_data.auto_font_selection_capabilities()
    );
  }

  #[test]
  fn descriptors_override_and_swap_decreasing_ranges() {
    let face = geist(FontFaceDescriptors {
      weight: Some((700.0, 400.0)),
      slope: Some((14.0, 14.0)),
      ..Default::default()
    });
    let capabilities = face.get_font_selection_capabilities();

    assert_eq!(
      capabilities.weight,
      FontSelectionRange::new(NORMAL_WEIGHT_VALUE, BOLD_WEIGHT_VALUE)
    );
    assert!(!capabilities.weight.is_range_set_from_auto());
    assert_eq!(
      capabilities.slope,
      FontSelectionRange::single(ITALIC_SLOPE_VALUE)
    );
  }

  #[test]
  fn an_out_of_range_weight_drops_every_descriptor() {
    let face = geist(FontFaceDescriptors {
      weight: Some((0.0, 400.0)),
      slope: Some((14.0, 14.0)),
      ..Default::default()
    });

    assert_eq!(
      face.get_font_selection_capabilities(),
      face.custom_platform_data.auto_font_selection_capabilities()
    );
  }

  #[test]
  fn descriptor_variations_yield_to_the_style_s() {
    let wght = Tag::new(b"wght");
    let grad = Tag::new(b"GRAD");
    let face = geist(FontFaceDescriptors {
      variation_settings: vec![
        Variation {
          tag: wght,
          value: 300.0,
        },
        Variation {
          tag: wght,
          value: 350.0,
        },
        Variation {
          tag: grad,
          value: 1.0,
        },
      ],
      ..Default::default()
    });

    assert_eq!(
      face.get_font_variation_settings(),
      [
        Variation {
          tag: grad,
          value: 1.0
        },
        Variation {
          tag: wght,
          value: 350.0
        },
      ]
    );

    let mut description = FontSelectionDescription::for_request(Default::default());

    assert_eq!(
      face.get_font_data(&description).variation(b"wght"),
      Some(350.0)
    );

    description.variation_settings = vec![Variation {
      tag: wght,
      value: 800.0,
    }];

    assert_eq!(
      face.get_font_data(&description).variation(b"wght"),
      Some(800.0)
    );
  }
}
