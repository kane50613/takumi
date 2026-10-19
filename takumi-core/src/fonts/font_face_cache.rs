//! Blink's [`FontFaceCache`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/font_face_cache.cc):
//! registered faces bucketed by family, then by capabilities, matched by
//! [`FontSelectionAlgorithm`].
//!
//! Approximate: families compare with Unicode lowercasing where Blink case-folds; buckets keep
//! insertion order where Blink's hash maps leave it unspecified, so the first added of two
//! equally good buckets wins; and Blink's `FontSelectionQueryCache` is left out, as the
//! registry is shared immutably across renders.

use std::{collections::HashMap, sync::Arc};

use super::{
  css_segmented_font_face::CssSegmentedFontFace,
  font_face::FontFace,
  font_selection_algorithm::FontSelectionAlgorithm,
  font_selection_types::{FontSelectionCapabilities, FontSelectionRequest},
};

/// The key a family name looks up under, as Blink's `DeprecatedCaseFoldingHashTraits` compares.
pub(crate) fn fold_family_name(family: &str) -> String {
  family.chars().flat_map(char::to_lowercase).collect()
}

/// Blink's `FontFaceCache`.
#[derive(Clone, Debug, Default)]
pub(crate) struct FontFaceCache {
  segmented_faces: SegmentedFacesByFamily,
}

/// Blink's `FontFaceCache::SegmentedFacesByFamily`.
#[derive(Clone, Debug, Default)]
struct SegmentedFacesByFamily {
  map: HashMap<String, CapabilitiesSet>,
}

/// Blink's `FontFaceCache::CapabilitiesSet`.
#[derive(Clone, Debug, Default)]
struct CapabilitiesSet {
  map: Vec<CssSegmentedFontFace>,
}

impl FontFaceCache {
  /// Blink's `AddFontFace`.
  pub(crate) fn add_font_face(&mut self, font_face: Arc<FontFace>) {
    debug_assert!(font_face.get_font_selection_capabilities().is_valid());

    self.segmented_faces.add_font_face(font_face);
  }

  /// Blink's `RemoveFontFace`.
  pub(crate) fn remove_font_face(&mut self, font_face: &Arc<FontFace>) -> bool {
    self.segmented_faces.remove_font_face(font_face)
  }

  /// Blink's `ClearAll`.
  pub(crate) fn clear_all(&mut self) {
    self.segmented_faces.map.clear();
  }

  /// Blink's `Get`: the bucket of `family` that best matches `request`.
  pub(crate) fn get(
    &self,
    request: FontSelectionRequest,
    family: &str,
  ) -> Option<&CssSegmentedFontFace> {
    self
      .segmented_faces
      .find(family)?
      .font_selection_query_result(request)
  }

  /// The buckets of `family`, the one [`Self::get`] picks first, then the rest from the better
  /// match for `request` to the worse.
  pub(crate) fn get_ranked(
    &self,
    request: FontSelectionRequest,
    family: &str,
  ) -> Vec<&CssSegmentedFontFace> {
    self
      .segmented_faces
      .find(family)
      .map_or_else(Vec::new, |faces| faces.ranked(request))
  }

  /// Blink's `GetNumSegmentedFacesForTesting`.
  pub(crate) fn get_num_segmented_faces_for_testing(&self) -> usize {
    self
      .segmented_faces
      .map
      .values()
      .map(|family_faces| family_faces.map.len())
      .sum()
  }
}

impl SegmentedFacesByFamily {
  fn add_font_face(&mut self, font_face: Arc<FontFace>) {
    self
      .map
      .entry(fold_family_name(&font_face.family))
      .or_default()
      .add_font_face(font_face);
  }

  fn remove_font_face(&mut self, font_face: &Arc<FontFace>) -> bool {
    let key = fold_family_name(&font_face.family);
    let Some(family_segmented_faces) = self.map.get_mut(&key) else {
      return false;
    };

    if !family_segmented_faces.remove_font_face(font_face) {
      return false;
    }

    if family_segmented_faces.is_empty() {
      self.map.remove(&key);
    }

    true
  }

  /// Blink's `Find`.
  fn find(&self, family: &str) -> Option<&CapabilitiesSet> {
    self.map.get(&fold_family_name(family))
  }
}

impl CapabilitiesSet {
  fn add_font_face(&mut self, font_face: Arc<FontFace>) {
    let capabilities = font_face.get_font_selection_capabilities();

    match self
      .map
      .iter_mut()
      .find(|segmented| segmented.get_font_selection_capabilities() == capabilities)
    {
      Some(segmented) => segmented.add_font_face(font_face),
      None => {
        let mut segmented = CssSegmentedFontFace::new(capabilities);

        segmented.add_font_face(font_face);
        self.map.push(segmented);
      }
    }
  }

  fn remove_font_face(&mut self, font_face: &Arc<FontFace>) -> bool {
    let Some(index) = self
      .map
      .iter_mut()
      .position(|segmented| segmented.remove_font_face(font_face))
    else {
      return false;
    };

    if self
      .map
      .get(index)
      .is_some_and(CssSegmentedFontFace::is_empty)
    {
      self.map.remove(index);
    }

    true
  }

  fn is_empty(&self) -> bool {
    self.map.is_empty()
  }

  /// Blink's `FontSelectionQueryResult::GetOrCreate`, without its cache.
  /// The buckets, each a better match for `request` than the ones after it.
  fn ranked(&self, request: FontSelectionRequest) -> Vec<&CssSegmentedFontFace> {
    let mut remaining: Vec<&CssSegmentedFontFace> = self.map.iter().collect();
    let mut ranked = Vec::with_capacity(remaining.len());

    while let Some(best) = Self::best_of(&remaining, request) {
      ranked.push(remaining.remove(best));
    }

    ranked
  }

  /// The position in `candidates` of the best match for `request`, measured against the bounds
  /// of every face of the family.
  fn best_of(candidates: &[&CssSegmentedFontFace], request: FontSelectionRequest) -> Option<usize> {
    let mut all_faces_boundaries = FontSelectionCapabilities::default();

    for segmented in candidates {
      all_faces_boundaries.expand(&segmented.get_font_selection_capabilities());
    }

    let font_selection_algorithm = FontSelectionAlgorithm::new(request, all_faces_boundaries);
    let mut best: Option<usize> = None;

    for (index, candidate) in candidates.iter().enumerate() {
      let is_better = best.is_none_or(|best| {
        font_selection_algorithm.is_better_match_for_request(
          &candidate.get_font_selection_capabilities(),
          &candidates[best].get_font_selection_capabilities(),
        )
      });

      if is_better {
        best = Some(index);
      }
    }

    best
  }

  fn font_selection_query_result(
    &self,
    request: FontSelectionRequest,
  ) -> Option<&CssSegmentedFontFace> {
    let candidates: Vec<&CssSegmentedFontFace> = self.map.iter().collect();

    Self::best_of(&candidates, request).map(|best| candidates[best])
  }
}

#[cfg(test)]
mod tests {
  use std::sync::OnceLock;

  use super::*;
  use crate::fonts::{
    font_custom_platform_data::FontCustomPlatformData,
    font_face::FontFaceDescriptors,
    font_selection_types::{
      BOLD_WEIGHT_VALUE, CONDENSED_WIDTH_VALUE, EXPANDED_WIDTH_VALUE, FontSelectionRange,
      FontSelectionValue, ITALIC_SLOPE_VALUE, NORMAL_SLOPE_VALUE, NORMAL_WEIGHT_VALUE,
      NORMAL_WIDTH_VALUE, ULTRA_EXPANDED_WIDTH_VALUE,
    },
  };

  const FONT_NAME_FOR_TESTING: &str = "Arial";
  const POPPINS: &str = "../assets/fonts/poppins/poppins-v24-devanagari_latin-regular.woff2";

  /// Any face: the descriptors decide every capability these tests match on.
  fn platform_data() -> Arc<FontCustomPlatformData> {
    static PLATFORM_DATA: OnceLock<Arc<FontCustomPlatformData>> = OnceLock::new();

    Arc::clone(PLATFORM_DATA.get_or_init(|| FontCustomPlatformData::from_asset(POPPINS).remove(0)))
  }

  fn append_test_face_for_capabilities(
    cache: &mut FontFaceCache,
    stretch: (f32, f32),
    style: (f32, f32),
    weight: (f32, f32),
  ) {
    cache.add_font_face(Arc::new(FontFace::new(
      FONT_NAME_FOR_TESTING.into(),
      FontFaceDescriptors {
        width: Some(stretch),
        slope: Some(style),
        weight: Some(weight),
        ..Default::default()
      },
      platform_data(),
    )));
  }

  fn single(value: FontSelectionValue) -> (f32, f32) {
    (value.to_f32(), value.to_f32())
  }

  fn font_description_for_request(
    stretch: FontSelectionValue,
    style: FontSelectionValue,
    weight: FontSelectionValue,
  ) -> FontSelectionRequest {
    FontSelectionRequest::new(weight, stretch, style)
  }

  fn result_capabilities(
    cache: &FontFaceCache,
    request: FontSelectionRequest,
  ) -> FontSelectionCapabilities {
    cache
      .get(request, FONT_NAME_FOR_TESTING)
      .unwrap()
      .get_font_selection_capabilities()
  }

  fn value(value: f32) -> FontSelectionValue {
    FontSelectionValue::new(value)
  }

  #[test]
  fn instantiate() {
    let mut cache = FontFaceCache::default();

    append_test_face_for_capabilities(
      &mut cache,
      single(ULTRA_EXPANDED_WIDTH_VALUE),
      single(ITALIC_SLOPE_VALUE),
      single(BOLD_WEIGHT_VALUE),
    );
    append_test_face_for_capabilities(
      &mut cache,
      single(CONDENSED_WIDTH_VALUE),
      single(ITALIC_SLOPE_VALUE),
      single(BOLD_WEIGHT_VALUE),
    );
    assert_eq!(cache.get_num_segmented_faces_for_testing(), 2);
  }

  #[test]
  fn simple_width_match() {
    let mut cache = FontFaceCache::default();

    append_test_face_for_capabilities(
      &mut cache,
      single(ULTRA_EXPANDED_WIDTH_VALUE),
      single(NORMAL_SLOPE_VALUE),
      single(NORMAL_WEIGHT_VALUE),
    );
    append_test_face_for_capabilities(
      &mut cache,
      single(CONDENSED_WIDTH_VALUE),
      single(NORMAL_SLOPE_VALUE),
      single(NORMAL_WEIGHT_VALUE),
    );
    assert_eq!(cache.get_num_segmented_faces_for_testing(), 2);

    let result = result_capabilities(
      &cache,
      font_description_for_request(
        CONDENSED_WIDTH_VALUE,
        NORMAL_SLOPE_VALUE,
        NORMAL_WEIGHT_VALUE,
      ),
    );

    assert_eq!(
      result.width,
      FontSelectionRange::single(CONDENSED_WIDTH_VALUE)
    );
    assert_eq!(
      result.weight,
      FontSelectionRange::single(NORMAL_WEIGHT_VALUE)
    );
    assert_eq!(result.slope, FontSelectionRange::single(NORMAL_SLOPE_VALUE));
  }

  #[test]
  fn simple_weight_match() {
    let mut cache = FontFaceCache::default();

    append_test_face_for_capabilities(
      &mut cache,
      single(NORMAL_WIDTH_VALUE),
      single(NORMAL_SLOPE_VALUE),
      (900.0, 900.0),
    );
    append_test_face_for_capabilities(
      &mut cache,
      single(NORMAL_WIDTH_VALUE),
      single(NORMAL_SLOPE_VALUE),
      (100.0, 100.0),
    );
    assert_eq!(cache.get_num_segmented_faces_for_testing(), 2);

    let result = result_capabilities(
      &cache,
      font_description_for_request(NORMAL_WIDTH_VALUE, NORMAL_SLOPE_VALUE, BOLD_WEIGHT_VALUE),
    );

    assert_eq!(result.width, FontSelectionRange::single(NORMAL_WIDTH_VALUE));
    assert_eq!(result.weight, FontSelectionRange::single(value(900.0)));
    assert_eq!(result.slope, FontSelectionRange::single(NORMAL_SLOPE_VALUE));
  }

  /// Blink's `AvailableCapabilitiesChoices`: both values, the first, or the second.
  fn available_capabilities_choices(choice: usize, available_values: [f32; 2]) -> Vec<f32> {
    match choice {
      0 => available_values.to_vec(),
      1 => vec![available_values[0]],
      _ => vec![available_values[1]],
    }
  }

  /// Blink's `ExpectedRangeForChoice`.
  fn expected_range_for_choice(
    request: FontSelectionValue,
    choice: usize,
    choices: [f32; 2],
  ) -> FontSelectionRange {
    match choice {
      0 => FontSelectionRange::single(request),
      1 => FontSelectionRange::single(value(choices[0])),
      _ => FontSelectionRange::single(value(choices[1])),
    }
  }

  /// Blink disables this test as flaky (https://crbug.com/871812), from hash-ordered buckets;
  /// insertion order makes it deterministic.
  #[test]
  fn match_combinations() {
    let widths = [
      CONDENSED_WIDTH_VALUE.to_f32(),
      EXPANDED_WIDTH_VALUE.to_f32(),
    ];
    let slopes = [NORMAL_SLOPE_VALUE.to_f32(), ITALIC_SLOPE_VALUE.to_f32()];
    let weights = [100.0, 900.0];
    let mut test_descriptions = Vec::new();

    for width in widths {
      for slope in slopes {
        for weight in weights {
          test_descriptions.push(font_description_for_request(
            value(width),
            value(slope),
            value(weight),
          ));
        }
      }
    }

    for width_choice in 0..3 {
      for slope_choice in 0..3 {
        for weight_choice in 0..3 {
          let mut cache = FontFaceCache::default();

          for width in available_capabilities_choices(width_choice, widths) {
            for slope in available_capabilities_choices(slope_choice, slopes) {
              for weight in available_capabilities_choices(weight_choice, weights) {
                append_test_face_for_capabilities(
                  &mut cache,
                  (width, width),
                  (slope, slope),
                  (weight, weight),
                );
              }
            }
          }

          for request in &test_descriptions {
            let result = result_capabilities(&cache, *request);

            assert_eq!(
              result.width,
              expected_range_for_choice(request.width, width_choice, widths)
            );
            assert_eq!(
              result.slope,
              expected_range_for_choice(request.slope, slope_choice, slopes)
            );
            assert_eq!(
              result.weight,
              expected_range_for_choice(request.weight, weight_choice, weights)
            );
          }
        }
      }
    }
  }

  #[test]
  fn width_range_matching() {
    let mut cache = FontFaceCache::default();

    append_test_face_for_capabilities(
      &mut cache,
      single(NORMAL_WIDTH_VALUE),
      single(NORMAL_SLOPE_VALUE),
      (700.0, 800.0),
    );
    append_test_face_for_capabilities(
      &mut cache,
      single(NORMAL_WIDTH_VALUE),
      single(NORMAL_SLOPE_VALUE),
      (100.0, 200.0),
    );
    assert_eq!(cache.get_num_segmented_faces_for_testing(), 2);

    let result = result_capabilities(
      &cache,
      font_description_for_request(NORMAL_WIDTH_VALUE, NORMAL_SLOPE_VALUE, BOLD_WEIGHT_VALUE),
    );

    assert_eq!(result.width, FontSelectionRange::single(NORMAL_WIDTH_VALUE));
    assert_eq!(
      result.weight,
      FontSelectionRange::new(value(700.0), value(800.0))
    );
    assert_eq!(result.slope, FontSelectionRange::single(NORMAL_SLOPE_VALUE));
  }

  #[test]
  fn width_range_matching_between_400_500() {
    // Two font faces equally far away from a requested font weight of 450: the heavy face is
    // matched last, after the thin one and after the one slightly bolder than 450.
    let weight_values_lower = [600.0, 415.0, 475.0];
    let weight_values_upper = [610.0, 425.0, 485.0];
    let test_weight = value(450.0);
    let request = font_description_for_request(NORMAL_WIDTH_VALUE, NORMAL_SLOPE_VALUE, test_weight);
    let mut cache = FontFaceCache::default();

    for (count, (lower, upper)) in weight_values_lower
      .into_iter()
      .zip(weight_values_upper)
      .enumerate()
    {
      append_test_face_for_capabilities(
        &mut cache,
        single(NORMAL_WIDTH_VALUE),
        single(NORMAL_SLOPE_VALUE),
        (lower, upper),
      );
      assert_eq!(cache.get_num_segmented_faces_for_testing(), count + 1);
      assert_eq!(
        result_capabilities(&cache, request).weight.minimum,
        value(lower)
      );
    }
  }

  #[test]
  fn stretch_range_matching() {
    let mut cache = FontFaceCache::default();
    let stretch_from = 110.0;
    let stretch_to = 120.0;

    append_test_face_for_capabilities(
      &mut cache,
      (65.0, 70.0),
      single(NORMAL_SLOPE_VALUE),
      (400.0, 400.0),
    );
    append_test_face_for_capabilities(
      &mut cache,
      (stretch_from, stretch_to),
      single(NORMAL_SLOPE_VALUE),
      (400.0, 400.0),
    );
    assert_eq!(cache.get_num_segmented_faces_for_testing(), 2);

    let result = result_capabilities(
      &cache,
      font_description_for_request(value(105.0), NORMAL_SLOPE_VALUE, NORMAL_WEIGHT_VALUE),
    );

    assert_eq!(
      result.width,
      FontSelectionRange::new(value(stretch_from), value(stretch_to))
    );
    assert_eq!(
      result.weight,
      FontSelectionRange::single(NORMAL_WEIGHT_VALUE)
    );
    assert_eq!(result.slope, FontSelectionRange::single(NORMAL_SLOPE_VALUE));
  }

  #[test]
  fn oblique_range_matching() {
    let mut cache = FontFaceCache::default();

    append_test_face_for_capabilities(
      &mut cache,
      single(NORMAL_WIDTH_VALUE),
      (30.0, 35.0),
      (400.0, 400.0),
    );
    append_test_face_for_capabilities(
      &mut cache,
      single(NORMAL_WIDTH_VALUE),
      (5.0, 10.0),
      (400.0, 400.0),
    );
    assert_eq!(cache.get_num_segmented_faces_for_testing(), 2);

    let result = result_capabilities(
      &cache,
      font_description_for_request(NORMAL_WIDTH_VALUE, ITALIC_SLOPE_VALUE, NORMAL_WEIGHT_VALUE),
    );

    assert_eq!(result.width, FontSelectionRange::single(NORMAL_WIDTH_VALUE));
    assert_eq!(
      result.weight,
      FontSelectionRange::single(NORMAL_WEIGHT_VALUE)
    );
    assert_eq!(
      result.slope,
      FontSelectionRange::new(value(30.0), value(35.0))
    );
  }

  #[test]
  fn families_match_regardless_of_case() {
    let mut cache = FontFaceCache::default();

    append_test_face_for_capabilities(
      &mut cache,
      single(NORMAL_WIDTH_VALUE),
      single(NORMAL_SLOPE_VALUE),
      (400.0, 400.0),
    );

    let request =
      font_description_for_request(NORMAL_WIDTH_VALUE, NORMAL_SLOPE_VALUE, NORMAL_WEIGHT_VALUE);

    assert!(cache.get(request, "ARIAL").is_some());
    assert!(cache.get(request, "Helvetica").is_none());
  }

  #[test]
  fn removing_the_last_face_drops_its_bucket() {
    let mut cache = FontFaceCache::default();
    let face = Arc::new(FontFace::new(
      FONT_NAME_FOR_TESTING.into(),
      FontFaceDescriptors::default(),
      platform_data(),
    ));

    cache.add_font_face(Arc::clone(&face));
    assert!(cache.remove_font_face(&face));
    assert!(!cache.remove_font_face(&face));
    assert_eq!(cache.get_num_segmented_faces_for_testing(), 0);
  }
}
