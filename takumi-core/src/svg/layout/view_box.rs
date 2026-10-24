//! How a `viewBox` fits a viewport, after Blink's `SVGFitToViewBox::ViewBoxToViewTransform` and
//! `SVGPreserveAspectRatio::ComputeTransform`.

use crate::{geometry::Size, resources::svg_size::ViewBox, style::Affine};

/// Where the `viewBox` sits in a viewport it does not fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Align {
  None,
  XMinYMin,
  XMidYMin,
  XMaxYMin,
  XMinYMid,
  XMidYMid,
  XMaxYMid,
  XMinYMax,
  XMidYMax,
  XMaxYMax,
}

impl Align {
  fn from_keyword(keyword: &str) -> Option<Self> {
    Some(match keyword {
      "none" => Self::None,
      "xMinYMin" => Self::XMinYMin,
      "xMidYMin" => Self::XMidYMin,
      "xMaxYMin" => Self::XMaxYMin,
      "xMinYMid" => Self::XMinYMid,
      "xMidYMid" => Self::XMidYMid,
      "xMaxYMid" => Self::XMaxYMid,
      "xMinYMax" => Self::XMinYMax,
      "xMidYMax" => Self::XMidYMax,
      "xMaxYMax" => Self::XMaxYMax,
      _ => return None,
    })
  }
}

/// The `preserveAspectRatio` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PreserveAspectRatio {
  align: Align,
  /// `slice` fills the viewport and crops, where `meet` fits inside it.
  pub(super) slice: bool,
}

impl Default for PreserveAspectRatio {
  fn default() -> Self {
    Self {
      align: Align::XMidYMid,
      slice: false,
    }
  }
}

impl PreserveAspectRatio {
  /// `none`, which stretches the view box over the viewport.
  pub(super) const NONE: Self = Self {
    align: Align::None,
    slice: false,
  };

  /// `[defer] <align> [meet | slice]`.
  pub(super) fn parse(text: &str) -> Option<Self> {
    let mut words = text.split_ascii_whitespace().peekable();

    words.next_if_eq(&"defer");

    let align = Align::from_keyword(words.next()?)?;
    let slice = match words.next() {
      None | Some("meet") => false,
      Some("slice") => true,
      Some(_) => return None,
    };

    Some(Self { align, slice })
  }

  /// Whether the aspect ratio is not preserved.
  pub(super) fn stretches(self) -> bool {
    self.align == Align::None
  }

  /// `(x, y)` moved by `w` and `h` as far as the alignment says.
  pub(super) fn aligned_position(self, x: f32, y: f32, w: f32, h: f32) -> (f32, f32) {
    match self.align {
      Align::None | Align::XMinYMin => (x, y),
      Align::XMidYMin => (x + w / 2.0, y),
      Align::XMaxYMin => (x + w, y),
      Align::XMinYMid => (x, y + h / 2.0),
      Align::XMidYMid => (x + w / 2.0, y + h / 2.0),
      Align::XMaxYMid => (x + w, y + h / 2.0),
      Align::XMinYMax => (x, y + h),
      Align::XMidYMax => (x + w / 2.0, y + h),
      Align::XMaxYMax => (x + w, y + h),
    }
  }
}

impl ViewBox {
  /// The transform that fits the `viewBox` to a viewport of `size`.
  pub(super) fn to_transform(self, aspect: PreserveAspectRatio, size: Size<f32>) -> Affine {
    let scale_x = size.width / self.width;
    let scale_y = size.height / self.height;
    let (scale_x, scale_y) = if aspect.stretches() {
      (scale_x, scale_y)
    } else {
      let scale = if aspect.slice {
        if scale_x < scale_y { scale_y } else { scale_x }
      } else if scale_x > scale_y {
        scale_y
      } else {
        scale_x
      };

      (scale, scale)
    };
    let (x, y) = aspect.aligned_position(
      -self.x * scale_x,
      -self.y * scale_y,
      size.width - self.width * scale_x,
      size.height - self.height * scale_y,
    );

    Affine {
      a: scale_x,
      b: 0.0,
      c: 0.0,
      d: scale_y,
      x,
      y,
    }
  }
}
