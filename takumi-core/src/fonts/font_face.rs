//! Blink's [`FontFace`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/font_face.cc),
//! with the part of [`CSSFontFace`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/css_font_face.cc)
//! and its `BinaryDataFontFaceSource`; a pending face stands for a `RemoteFontFaceSource` still loading.
//! Approximate: `auto` descriptors take the face's own capabilities as CSS Fonts 4 asks, Blink's `normal` on a pending face.

use std::sync::Arc;

use harfrust::font::Variation;

use super::{
  font_custom_platform_data::{FontCustomPlatformData, FontPlatformData},
  font_description::{FontSelectionDescription, sort_by_tag},
  font_selection_types::{
    FontSelectionCapabilities, FontSelectionRange, FontSelectionValue, NORMAL_SLOPE_VALUE,
    NORMAL_WEIGHT_VALUE, NORMAL_WIDTH_VALUE, RangeType,
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

/// Where a face's data comes from: Blink's `CSSFontFaceSource`.
#[derive(Debug)]
pub(crate) enum FontFaceSource {
  /// Blink's `BinaryDataFontFaceSource`.
  Binary(Arc<FontCustomPlatformData>),
  /// Blink's `RemoteFontFaceSource` before it loads, by the id the caller gave it.
  Pending(usize),
}

/// Blink's `FontDataForRangeSet` whose `CustomFontData` is loading.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PendingFace {
  pub(crate) id: usize,
  pub(crate) range_set: UnicodeRangeSet,
}

/// A face's data, or the pending face that has none yet.
#[derive(Clone, Debug)]
pub(crate) enum FaceData<T> {
  Loaded(T),
  Pending(PendingFace),
}

impl<T> FaceData<T> {
  /// The data of a loaded face.
  pub(crate) fn loaded(&self) -> Option<&T> {
    match self {
      Self::Loaded(data) => Some(data),
      Self::Pending(_) => None,
    }
  }

  pub(crate) fn map<U>(self, f: impl FnOnce(T) -> U) -> FaceData<U> {
    match self {
      Self::Loaded(data) => FaceData::Loaded(f(data)),
      Self::Pending(pending) => FaceData::Pending(pending),
    }
  }
}

/// Blink's `FontFace`.
#[derive(Debug)]
pub(crate) struct FontFace {
  /// Blink's `familyNameUnquoted`.
  pub(crate) family: Arc<str>,
  pub(crate) descriptors: FontFaceDescriptors,
  pub(crate) source: FontFaceSource,
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
      source: FontFaceSource::Binary(custom_platform_data),
    }
  }

  /// A face of `family` with no data yet, which matching finds as `id`.
  pub(crate) fn pending(family: Arc<str>, descriptors: FontFaceDescriptors, id: usize) -> Self {
    Self {
      family,
      descriptors,
      source: FontFaceSource::Pending(id),
    }
  }

  /// The decoded face, none while it is pending.
  pub(crate) fn custom_platform_data(&self) -> Option<&Arc<FontCustomPlatformData>> {
    match &self.source {
      FontFaceSource::Binary(custom_platform_data) => Some(custom_platform_data),
      FontFaceSource::Pending(_) => None,
    }
  }

  /// The capabilities of an `auto` descriptor: the face's own, or Blink's `normal` while pending.
  fn auto_font_selection_capabilities(&self) -> FontSelectionCapabilities {
    match &self.source {
      FontFaceSource::Binary(custom_platform_data) => {
        custom_platform_data.auto_font_selection_capabilities()
      }
      FontFaceSource::Pending(_) => FontSelectionCapabilities::new(
        FontSelectionRange::single(NORMAL_WIDTH_VALUE),
        FontSelectionRange::single(NORMAL_SLOPE_VALUE),
        FontSelectionRange::single(NORMAL_WEIGHT_VALUE),
      ),
    }
  }

  /// Blink's `GetFontSelectionCapabilities`.
  pub(crate) fn get_font_selection_capabilities(&self) -> FontSelectionCapabilities {
    let auto_capabilities = self.auto_font_selection_capabilities();
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

  /// Blink's `CSSFontFace::GetFontData`, or the pending face while its source loads.
  pub(crate) fn get_font_data(
    &self,
    font_description: &FontSelectionDescription,
  ) -> FaceData<FontPlatformData> {
    let custom_platform_data = match &self.source {
      FontFaceSource::Binary(custom_platform_data) => custom_platform_data,
      FontFaceSource::Pending(id) => {
        return FaceData::Pending(PendingFace {
          id: *id,
          range_set: self.ranges().clone(),
        });
      }
    };
    let mut description = font_description.clone();

    description.merge_font_variation_settings_with_descriptor(&self.get_font_variation_settings());
    FaceData::Loaded(
      custom_platform_data
        .get_font_platform_data(&description, &self.get_font_selection_capabilities()),
    )
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
      face.auto_font_selection_capabilities()
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
      face.auto_font_selection_capabilities()
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
      face
        .get_font_data(&description)
        .loaded()
        .unwrap()
        .variation(b"wght"),
      Some(350.0)
    );

    description.variation_settings = vec![Variation {
      tag: wght,
      value: 800.0,
    }];

    assert_eq!(
      face
        .get_font_data(&description)
        .loaded()
        .unwrap()
        .variation(b"wght"),
      Some(800.0)
    );
  }
}
