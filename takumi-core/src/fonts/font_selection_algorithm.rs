//! Blink's [`FontSelectionAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_selection_algorithm.cc),
//! the distances of [CSS Fonts 4 §5.2](https://drafts.csswg.org/css-fonts-4/#font-style-matching).

use super::font_selection_types::{
  FontSelectionCapabilities, FontSelectionRequest, FontSelectionValue, ITALIC_THRESHOLD,
  LOWER_WEIGHT_SEARCH_THRESHOLD, NORMAL_WIDTH_VALUE, UPPER_WEIGHT_SEARCH_THRESHOLD,
};

/// Blink's `FontSelectionAlgorithm`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FontSelectionAlgorithm {
  request: FontSelectionRequest,
  capabilities_bounds: FontSelectionCapabilities,
}

/// Blink's `FontSelectionAlgorithm::DistanceResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DistanceResult {
  pub(crate) distance: FontSelectionValue,
  pub(crate) value: FontSelectionValue,
}

impl DistanceResult {
  fn new(distance: FontSelectionValue, value: FontSelectionValue) -> Self {
    Self { distance, value }
  }
}

impl FontSelectionAlgorithm {
  pub(crate) fn new(
    request: FontSelectionRequest,
    capabilities_bounds: FontSelectionCapabilities,
  ) -> Self {
    Self {
      request,
      capabilities_bounds,
    }
  }

  /// Blink's `StretchDistance`.
  pub(crate) fn stretch_distance(&self, capabilities: FontSelectionCapabilities) -> DistanceResult {
    let width = capabilities.width;

    debug_assert!(width.is_valid());

    if width.includes(self.request.width) {
      return DistanceResult::new(FontSelectionValue::default(), self.request.width);
    }

    if self.request.width > NORMAL_WIDTH_VALUE {
      if width.minimum > self.request.width {
        return DistanceResult::new(width.minimum - self.request.width, width.minimum);
      }

      debug_assert!(width.maximum < self.request.width);

      let threshold = self
        .request
        .width
        .max(self.capabilities_bounds.width.maximum);

      return DistanceResult::new(threshold - width.maximum, width.maximum);
    }

    if width.maximum < self.request.width {
      return DistanceResult::new(self.request.width - width.maximum, width.maximum);
    }

    debug_assert!(width.minimum > self.request.width);

    let threshold = self
      .request
      .width
      .min(self.capabilities_bounds.width.minimum);

    DistanceResult::new(width.minimum - threshold, width.minimum)
  }

  /// Blink's `StyleDistance`.
  pub(crate) fn style_distance(&self, capabilities: FontSelectionCapabilities) -> DistanceResult {
    let slope = capabilities.slope;
    let zero = FontSelectionValue::default();

    debug_assert!(slope.is_valid());

    if slope.includes(self.request.slope) {
      return DistanceResult::new(zero, self.request.slope);
    }

    if self.request.slope >= ITALIC_THRESHOLD {
      if slope.minimum > self.request.slope {
        return DistanceResult::new(slope.minimum - self.request.slope, slope.minimum);
      }

      debug_assert!(self.request.slope > slope.maximum);

      let threshold = self
        .request
        .slope
        .max(self.capabilities_bounds.slope.maximum);

      return DistanceResult::new(threshold - slope.maximum, slope.maximum);
    }

    if self.request.slope >= zero {
      if slope.maximum >= zero && slope.maximum < self.request.slope {
        return DistanceResult::new(self.request.slope - slope.maximum, slope.maximum);
      }

      if slope.minimum > self.request.slope {
        return DistanceResult::new(slope.minimum, slope.minimum);
      }

      debug_assert!(slope.maximum < zero);

      let threshold = self
        .request
        .slope
        .max(self.capabilities_bounds.slope.maximum);

      return DistanceResult::new(threshold - slope.maximum, slope.maximum);
    }

    if self.request.slope > -ITALIC_THRESHOLD {
      if slope.minimum > self.request.slope && slope.minimum <= zero {
        return DistanceResult::new(slope.minimum - self.request.slope, slope.minimum);
      }

      if slope.maximum < self.request.slope {
        return DistanceResult::new(-slope.maximum, slope.maximum);
      }

      debug_assert!(slope.minimum > zero);

      let threshold = self
        .request
        .slope
        .min(self.capabilities_bounds.slope.minimum);

      return DistanceResult::new(slope.minimum - threshold, slope.minimum);
    }

    if slope.maximum < self.request.slope {
      return DistanceResult::new(self.request.slope - slope.maximum, slope.maximum);
    }

    debug_assert!(slope.minimum > self.request.slope);

    let threshold = self
      .request
      .slope
      .min(self.capabilities_bounds.slope.minimum);

    DistanceResult::new(slope.minimum - threshold, slope.minimum)
  }

  /// Blink's `WeightDistance`.
  pub(crate) fn weight_distance(&self, capabilities: FontSelectionCapabilities) -> DistanceResult {
    let weight = capabilities.weight;

    debug_assert!(weight.is_valid());

    if weight.includes(self.request.weight) {
      return DistanceResult::new(FontSelectionValue::default(), self.request.weight);
    }

    if self.request.weight >= LOWER_WEIGHT_SEARCH_THRESHOLD
      && self.request.weight <= UPPER_WEIGHT_SEARCH_THRESHOLD
    {
      if weight.minimum > self.request.weight && weight.minimum <= UPPER_WEIGHT_SEARCH_THRESHOLD {
        return DistanceResult::new(weight.minimum - self.request.weight, weight.minimum);
      }

      if weight.maximum < self.request.weight {
        return DistanceResult::new(
          UPPER_WEIGHT_SEARCH_THRESHOLD - weight.maximum,
          weight.maximum,
        );
      }

      debug_assert!(weight.minimum > UPPER_WEIGHT_SEARCH_THRESHOLD);

      let threshold = self
        .request
        .weight
        .min(self.capabilities_bounds.weight.minimum);

      return DistanceResult::new(weight.minimum - threshold, weight.minimum);
    }

    if self.request.weight < LOWER_WEIGHT_SEARCH_THRESHOLD {
      if weight.maximum < self.request.weight {
        return DistanceResult::new(self.request.weight - weight.maximum, weight.maximum);
      }

      debug_assert!(weight.minimum > self.request.weight);

      let threshold = self
        .request
        .weight
        .min(self.capabilities_bounds.weight.minimum);

      return DistanceResult::new(weight.minimum - threshold, weight.minimum);
    }

    debug_assert!(self.request.weight >= UPPER_WEIGHT_SEARCH_THRESHOLD);

    if weight.minimum > self.request.weight {
      return DistanceResult::new(weight.minimum - self.request.weight, weight.minimum);
    }

    debug_assert!(weight.maximum < self.request.weight);

    let threshold = self
      .request
      .weight
      .max(self.capabilities_bounds.weight.maximum);

    DistanceResult::new(threshold - weight.maximum, weight.maximum)
  }

  /// Blink's `IsBetterMatchForRequest`: whether `first` is strictly closer than `second`, by width,
  /// then slope, then weight.
  pub(crate) fn is_better_match_for_request(
    &self,
    first_capabilities: &FontSelectionCapabilities,
    second_capabilities: &FontSelectionCapabilities,
  ) -> bool {
    let stretch_distance_first = self.stretch_distance(*first_capabilities).distance;
    let stretch_distance_second = self.stretch_distance(*second_capabilities).distance;

    if stretch_distance_first < stretch_distance_second {
      return true;
    }

    if stretch_distance_first > stretch_distance_second {
      return false;
    }

    let style_distance_first = self.style_distance(*first_capabilities).distance;
    let style_distance_second = self.style_distance(*second_capabilities).distance;

    if style_distance_first < style_distance_second {
      return true;
    }

    if style_distance_first > style_distance_second {
      return false;
    }

    let weight_distance_first = self.weight_distance(*first_capabilities).distance;
    let weight_distance_second = self.weight_distance(*second_capabilities).distance;

    weight_distance_first < weight_distance_second
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::fonts::font_selection_types::{
    BOLD_WEIGHT_VALUE, CONDENSED_WIDTH_VALUE, EXPANDED_WIDTH_VALUE, FontSelectionRange,
    ITALIC_SLOPE_VALUE, MEDIUM_WEIGHT_VALUE, NORMAL_SLOPE_VALUE, NORMAL_WEIGHT_VALUE,
  };

  fn value(value: f32) -> FontSelectionValue {
    FontSelectionValue::new(value)
  }

  fn face(width: f32, slope: f32, weight: f32) -> FontSelectionCapabilities {
    FontSelectionCapabilities::new(
      FontSelectionRange::single(value(width)),
      FontSelectionRange::single(value(slope)),
      FontSelectionRange::single(value(weight)),
    )
  }

  /// The face of `faces` the algorithm picks for `request`, the first on a tie.
  fn best(faces: &[FontSelectionCapabilities], request: FontSelectionRequest) -> usize {
    let mut bounds = FontSelectionCapabilities::default();

    for face in faces {
      bounds.expand(face);
    }

    let algorithm = FontSelectionAlgorithm::new(request, bounds);
    let mut best = 0;

    for (index, face) in faces.iter().enumerate().skip(1) {
      if algorithm.is_better_match_for_request(face, &faces[best]) {
        best = index;
      }
    }

    best
  }

  fn weight_request(weight: f32) -> FontSelectionRequest {
    FontSelectionRequest::new(value(weight), NORMAL_WIDTH_VALUE, NORMAL_SLOPE_VALUE)
  }

  #[test]
  fn a_weight_between_400_and_500_looks_up_to_500_then_down() {
    let faces = [
      face(100.0, 0.0, 300.0),
      face(100.0, 0.0, 500.0),
      face(100.0, 0.0, 600.0),
    ];

    assert_eq!(best(&faces, weight_request(400.0)), 1);
    assert_eq!(best(&faces, weight_request(450.0)), 1);

    let without_medium = [face(100.0, 0.0, 300.0), face(100.0, 0.0, 600.0)];

    assert_eq!(best(&without_medium, weight_request(450.0)), 0);
  }

  #[test]
  fn a_weight_below_400_looks_down_first() {
    let faces = [face(100.0, 0.0, 200.0), face(100.0, 0.0, 400.0)];

    assert_eq!(best(&faces, weight_request(300.0)), 0);
  }

  #[test]
  fn a_weight_above_500_looks_up_first() {
    let faces = [face(100.0, 0.0, 400.0), face(100.0, 0.0, 800.0)];

    assert_eq!(best(&faces, weight_request(600.0)), 1);
    assert_eq!(best(&faces, weight_request(900.0)), 1);
  }

  #[test]
  fn width_outranks_slope_and_weight() {
    let faces = [face(75.0, 14.0, 700.0), face(125.0, 0.0, 400.0)];
    let request = FontSelectionRequest::new(
      NORMAL_WEIGHT_VALUE,
      EXPANDED_WIDTH_VALUE,
      NORMAL_SLOPE_VALUE,
    );

    assert_eq!(best(&faces, request), 1);

    let request = FontSelectionRequest::new(
      NORMAL_WEIGHT_VALUE,
      CONDENSED_WIDTH_VALUE,
      NORMAL_SLOPE_VALUE,
    );

    assert_eq!(best(&faces, request), 0);
  }

  #[test]
  fn italic_prefers_steeper_obliques_then_shallower_then_normal() {
    let faces = [
      face(100.0, 0.0, 400.0),
      face(100.0, 8.0, 400.0),
      face(100.0, 20.0, 400.0),
    ];
    let italic =
      FontSelectionRequest::new(NORMAL_WEIGHT_VALUE, NORMAL_WIDTH_VALUE, ITALIC_SLOPE_VALUE);

    assert_eq!(best(&faces, italic), 2);
    assert_eq!(best(&faces[..2], italic), 1);
  }

  #[test]
  fn normal_prefers_positive_obliques_over_negative_ones() {
    let faces = [face(100.0, -10.0, 400.0), face(100.0, 10.0, 400.0)];

    assert_eq!(best(&faces, weight_request(400.0)), 1);
  }

  #[test]
  fn a_range_that_includes_the_request_is_at_distance_zero() {
    let variable = FontSelectionCapabilities::new(
      FontSelectionRange::single(NORMAL_WIDTH_VALUE),
      FontSelectionRange::single(NORMAL_SLOPE_VALUE),
      FontSelectionRange::new(value(100.0), value(900.0)),
    );
    let algorithm = FontSelectionAlgorithm::new(weight_request(650.0), variable);

    assert_eq!(
      algorithm.weight_distance(variable),
      DistanceResult::new(FontSelectionValue::default(), value(650.0))
    );
    assert_eq!(
      FontSelectionAlgorithm::new(weight_request(450.0), variable)
        .weight_distance(face(100.0, 0.0, 300.0))
        .distance,
      MEDIUM_WEIGHT_VALUE - value(300.0)
    );
    assert_eq!(
      FontSelectionAlgorithm::new(weight_request(1000.0), variable)
        .weight_distance(face(100.0, 0.0, 700.0))
        .value,
      BOLD_WEIGHT_VALUE
    );
  }
}
