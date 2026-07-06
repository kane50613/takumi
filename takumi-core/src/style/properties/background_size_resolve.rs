use crate::{
  geometry::Size,
  style::{Length, SizingContext},
};

/// Which axis was left `auto` after resolving `background-size`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoBackgroundAxis {
  /// The width axis is `auto`.
  Width,
  /// The height axis is `auto`.
  Height,
}

/// Intrinsic sizing of an image, per CSS Images Level 3 §5.3. `width`/`height`
/// are set only where the image has an intrinsic dimension on that axis (a
/// non-percentage SVG `width`/`height`); a `viewBox`-only SVG has `ratio` alone.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct IntrinsicSizing {
  /// Intrinsic width in pixels, if any.
  pub width: Option<f32>,
  /// Intrinsic height in pixels, if any.
  pub height: Option<f32>,
  /// Intrinsic aspect ratio (width / height), if any.
  pub ratio: Option<f32>,
}

impl IntrinsicSizing {
  /// Intrinsic sizing from concrete width and height in pixels.
  pub(crate) fn from_dimensions(width: f32, height: f32) -> Self {
    Self {
      width: Some(width),
      height: Some(height),
      ratio: (height != 0.0).then_some(width / height),
    }
  }

  /// Scale the dimensions using the given sizing context.
  pub fn scale(self, sizing: &SizingContext) -> Self {
    Self {
      width: self.width.map(|w| sizing.to_device(w)),
      height: self.height.map(|h| sizing.to_device(h)),
      ratio: self.ratio,
    }
  }

  /// §5.3 default sizing algorithm with `area` as the default object size
  /// (Blink's `ConcreteObjectSize`).
  fn concrete_size(self, area_width: f32, area_height: f32) -> (u32, u32) {
    let round = |width: f32, height: f32| (width.round() as u32, height.round() as u32);

    match (self.width, self.height) {
      (Some(width), Some(height)) => round(width, height),
      (Some(width), None) => match self.ratio {
        Some(ratio) if ratio != 0.0 => round(width, width / ratio),
        _ => round(width, area_height),
      },
      (None, Some(height)) => match self.ratio {
        Some(ratio) => round(height * ratio, height),
        None => round(area_width, height),
      },
      (None, None) => match self.ratio {
        // Contain the intrinsic ratio within the default object size.
        Some(ratio) if ratio != 0.0 => {
          let solution_width = area_height * ratio;
          if solution_width <= area_width {
            round(solution_width, area_height)
          } else {
            round(area_width, area_width / ratio)
          }
        }
        _ => round(area_width, area_height),
      },
    }
  }
}

/// Device-pixel dimensions of a background layer after sizing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedBackgroundSize {
  /// Resolved width in device pixels.
  pub width: u32,
  /// Resolved height in device pixels.
  pub height: u32,
  /// Intrinsic aspect ratio carried through for downstream use.
  pub intrinsic_ratio: Option<f32>,
  /// Which axis remained `auto`, if any.
  pub auto_axis: Option<AutoBackgroundAxis>,
}

/// Scale a rectangle of the given aspect ratio (`width / height`) to cover or
/// contain `area`, returning rounded device-pixel dimensions.
pub(super) fn fit_ratio_to_area(ratio: f32, area: Size<u32>, cover: bool) -> (u32, u32) {
  let area_width = area.width as f32;
  let area_height = area.height as f32;
  let width_at_area_height = area_height * ratio;

  let (width, height) = if (width_at_area_height >= area_width) == cover {
    (width_at_area_height, area_height)
  } else {
    (area_width, area_width / ratio)
  };

  (width.round() as u32, height.round() as u32)
}

/// Resolves a `background-size` with at least one `auto` axis to pixels (§5.3).
pub(super) fn resolve_auto_background_size(
  width: Length,
  height: Length,
  area: Size<u32>,
  sizing: &SizingContext,
  intrinsic: IntrinsicSizing,
) -> (u32, u32) {
  let area_width = area.width as f32;
  let area_height = area.height as f32;

  match (width == Length::Auto, height == Length::Auto) {
    (true, true) => intrinsic.concrete_size(area_width, area_height),
    // One axis definite: the auto axis follows the ratio, else that axis'
    // intrinsic dimension, else the area (§5.3).
    (true, false) => {
      let fixed_height = height.to_px(sizing, area_height).max(0.0);
      let resolved_width = match intrinsic.ratio {
        Some(ratio) => fixed_height * ratio,
        None => intrinsic.width.unwrap_or(area_width),
      };
      (resolved_width.round() as u32, fixed_height.round() as u32)
    }
    (false, true) => {
      let fixed_width = width.to_px(sizing, area_width).max(0.0);
      let resolved_height = match intrinsic.ratio {
        Some(ratio) if ratio != 0.0 => fixed_width / ratio,
        Some(_) => 0.0,
        None => intrinsic.height.unwrap_or(area_height),
      };
      (fixed_width.round() as u32, resolved_height.round() as u32)
    }
    (false, false) => (0, 0),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // `auto auto` (the default `background-size`/`mask-size`) sizing, per the CSS
  // Images Level 3 §5.3 default sizing algorithm.

  #[test]
  fn auto_size_uses_intrinsic_dimensions_when_present() {
    let intrinsic = IntrinsicSizing::from_dimensions(102.0, 38.0);
    assert_eq!(intrinsic.concrete_size(1200.0, 630.0), (102, 38));
  }

  #[test]
  fn auto_size_contains_ratio_only_image_within_area() {
    // viewBox-only: contained within the area, not sized to the viewBox (which
    // would tile under the default `repeat`).
    let intrinsic = IntrinsicSizing {
      width: None,
      height: None,
      ratio: Some(1.0),
    };
    assert_eq!(intrinsic.concrete_size(400.0, 400.0), (400, 400));
    assert_eq!(intrinsic.concrete_size(1200.0, 630.0), (630, 630));
  }

  #[test]
  fn auto_size_fills_area_without_intrinsic_information() {
    // No dimensions and no ratio (e.g. a gradient): fill the positioning area.
    assert_eq!(
      IntrinsicSizing::default().concrete_size(1200.0, 630.0),
      (1200, 630)
    );
  }

  #[test]
  fn contain_and_cover_scale_ratio_to_area() {
    let area = Size {
      width: 800,
      height: 400,
    };
    // A 1:1 ratio: `contain` is the largest square fitting the area, `cover`
    // the smallest square covering it.
    assert_eq!(fit_ratio_to_area(1.0, area, false), (400, 400));
    assert_eq!(fit_ratio_to_area(1.0, area, true), (800, 800));
  }
}
