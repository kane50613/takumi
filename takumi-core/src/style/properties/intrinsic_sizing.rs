use crate::{geometry::Size, style::SizingContext};

/// Blink's `NaturalSizingInfo`: the natural dimensions of an image, per CSS Images Level 3 §5.3.
/// `width`/`height` are set only where the image has a natural dimension on that axis (a
/// non-percentage SVG `width`/`height`); a `viewBox`-only SVG has `aspect_ratio` alone.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct IntrinsicSizing {
  /// Natural width in pixels, if any.
  pub width: Option<f32>,
  /// Natural height in pixels, if any.
  pub height: Option<f32>,
  /// Natural aspect ratio as a width and height, both positive, if any.
  pub aspect_ratio: Option<Size<f32>>,
}

impl IntrinsicSizing {
  /// Blink's `MakeFixed`: natural dimensions of `width` by `height` pixels, with their ratio.
  pub(crate) fn from_dimensions(width: f32, height: f32) -> Self {
    Self {
      width: Some(width),
      height: Some(height),
      aspect_ratio: (width > 0.0 && height > 0.0).then_some(Size { width, height }),
    }
  }

  /// Scale the dimensions using the given sizing context.
  pub fn scale(self, sizing: &SizingContext) -> Self {
    Self {
      width: self.width.map(|w| sizing.to_device(w)),
      height: self.height.map(|h| sizing.to_device(h)),
      aspect_ratio: self.aspect_ratio,
    }
  }

  /// The natural aspect ratio as width over height.
  pub fn ratio(self) -> Option<f32> {
    self.aspect_ratio.map(|ratio| ratio.width / ratio.height)
  }

  /// Blink's `IsNone`: whether the image has no natural dimension and no ratio.
  pub fn is_none(self) -> bool {
    self.width.is_none() && self.height.is_none() && self.aspect_ratio.is_none()
  }

  /// Blink's `ConcreteObjectSize`: the §5.3 default sizing algorithm against a
  /// `default_object_size`.
  pub(crate) fn concrete_object_size(self, default_object_size: Size<f32>) -> Size<f32> {
    match (self.width, self.height, self.aspect_ratio) {
      (Some(width), Some(height), _) => Size { width, height },
      (Some(width), None, None) => Size {
        width,
        height: default_object_size.height,
      },
      (Some(width), None, Some(ratio)) => Size {
        width,
        height: width * ratio.height / ratio.width,
      },
      (None, Some(height), None) => Size {
        width: default_object_size.width,
        height,
      },
      (None, Some(height), Some(ratio)) => Size {
        width: height * ratio.width / ratio.height,
        height,
      },
      (None, None, Some(ratio)) => {
        let solution_width = default_object_size.height * ratio.width / ratio.height;

        if solution_width <= default_object_size.width {
          return Size {
            width: solution_width,
            height: default_object_size.height,
          };
        }

        Size {
          width: default_object_size.width,
          height: default_object_size.width * ratio.height / ratio.width,
        }
      }
      (None, None, None) => default_object_size,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const AREA: Size<f32> = Size {
    width: 1200.0,
    height: 630.0,
  };

  #[test]
  fn natural_dimensions_win() {
    let size = IntrinsicSizing::from_dimensions(102.0, 38.0).concrete_object_size(AREA);

    assert_eq!((size.width, size.height), (102.0, 38.0));
  }

  #[test]
  fn a_ratio_alone_is_contained_in_the_default_size() {
    let sizing = IntrinsicSizing {
      width: None,
      height: None,
      aspect_ratio: Some(Size {
        width: 128.0,
        height: 128.0,
      }),
    };
    let size = sizing.concrete_object_size(AREA);

    assert_eq!((size.width, size.height), (630.0, 630.0));
  }

  #[test]
  fn nothing_natural_takes_the_default_size() {
    assert_eq!(IntrinsicSizing::default().concrete_object_size(AREA), AREA);
  }
}
