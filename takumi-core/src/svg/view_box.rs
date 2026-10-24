//! `viewBox` and `preserveAspectRatio`, after Blink's `SVGFitToViewBox`.

use tiny_skia::Transform;

use super::scan::Scanner;

/// Where `preserveAspectRatio` aligns the view box, on each axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
  /// Stretches the view box to fill the viewport.
  None,
  /// Left, top.
  XMinYMin,
  /// Center, top.
  XMidYMin,
  /// Right, top.
  XMaxYMin,
  /// Left, middle.
  XMinYMid,
  /// Center, middle.
  XMidYMid,
  /// Right, middle.
  XMaxYMid,
  /// Left, bottom.
  XMinYMax,
  /// Center, bottom.
  XMidYMax,
  /// Right, bottom.
  XMaxYMax,
}

/// The `preserveAspectRatio` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreserveAspectRatio {
  /// Where the view box sits in the viewport.
  pub align: Align,
  /// Whether the view box covers the viewport instead of fitting inside it.
  pub slice: bool,
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
  /// Parses the attribute value, or `None` when it is malformed.
  pub fn parse(text: &str) -> Option<Self> {
    let mut words = text.split_ascii_whitespace();
    let mut word = words.next()?;

    if word == "defer" {
      word = words.next()?;
    }

    let align = match word {
      "none" => Align::None,
      "xMinYMin" => Align::XMinYMin,
      "xMidYMin" => Align::XMidYMin,
      "xMaxYMin" => Align::XMaxYMin,
      "xMinYMid" => Align::XMinYMid,
      "xMidYMid" => Align::XMidYMid,
      "xMaxYMid" => Align::XMaxYMid,
      "xMinYMax" => Align::XMinYMax,
      "xMidYMax" => Align::XMidYMax,
      "xMaxYMax" => Align::XMaxYMax,
      _ => return None,
    };
    let slice = match words.next() {
      None | Some("meet") => false,
      Some("slice") => true,
      Some(_) => return None,
    };

    words.next().is_none().then_some(Self { align, slice })
  }
}

/// The `viewBox` attribute, with a positive width and height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewBox {
  /// The left edge.
  pub x: f32,
  /// The top edge.
  pub y: f32,
  /// The width.
  pub width: f32,
  /// The height.
  pub height: f32,
}

impl ViewBox {
  /// Parses the attribute value, or `None` when it is malformed or empty.
  pub fn parse(text: &str) -> Option<Self> {
    let mut scanner = Scanner::new(text);
    let x = scanner.number_f64()?;
    let y = scanner.number_f64()?;
    let width = scanner.number_f64()?;
    let height = scanner.number_f64()?;

    if width <= 0.0 || height <= 0.0 {
      return None;
    }

    let view_box = Self {
      x: x as f32,
      y: y as f32,
      width: width as f32,
      height: height as f32,
    };

    (view_box.width > 0.0 && view_box.height > 0.0).then_some(view_box)
  }

  /// Blink's `SVGFitToViewBox::ViewBoxToViewTransform`, which maps this view box onto a viewport
  /// of `width` by `height` at the origin.
  pub fn transform(&self, aspect: PreserveAspectRatio, width: f32, height: f32) -> Transform {
    let mut sx = width / self.width;
    let mut sy = height / self.height;

    if aspect.align != Align::None {
      let scale = if aspect.slice { sx.max(sy) } else { sx.min(sy) };

      (sx, sy) = (scale, scale);
    }

    let x = -self.x * sx;
    let y = -self.y * sy;
    let spare_width = width - self.width * sx;
    let spare_height = height - self.height * sy;
    let (tx, ty) = match aspect.align {
      Align::None | Align::XMinYMin => (x, y),
      Align::XMidYMin => (x + spare_width / 2.0, y),
      Align::XMaxYMin => (x + spare_width, y),
      Align::XMinYMid => (x, y + spare_height / 2.0),
      Align::XMidYMid => (x + spare_width / 2.0, y + spare_height / 2.0),
      Align::XMaxYMid => (x + spare_width, y + spare_height / 2.0),
      Align::XMinYMax => (x, y + spare_height),
      Align::XMidYMax => (x + spare_width / 2.0, y + spare_height),
      Align::XMaxYMax => (x + spare_width, y + spare_height),
    };

    Transform::from_row(sx, 0.0, 0.0, sy, tx, ty)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn aspect_ratio_takes_defer_align_and_meet_or_slice() {
    assert_eq!(
      PreserveAspectRatio::parse("defer xMinYMax slice"),
      Some(PreserveAspectRatio {
        align: Align::XMinYMax,
        slice: true
      })
    );
    assert_eq!(
      PreserveAspectRatio::parse(" none "),
      Some(PreserveAspectRatio {
        align: Align::None,
        slice: false
      })
    );
    assert_eq!(PreserveAspectRatio::parse("xMidYMid fit"), None);
    assert_eq!(PreserveAspectRatio::parse(""), None);
  }

  #[test]
  fn view_box_needs_four_numbers_and_a_size() {
    assert_eq!(
      ViewBox::parse("-20,30 100 500"),
      Some(ViewBox {
        x: -20.0,
        y: 30.0,
        width: 100.0,
        height: 500.0
      })
    );
    assert_eq!(ViewBox::parse("0 0 0 10"), None);
    assert_eq!(ViewBox::parse("0 0 10"), None);
  }

  #[test]
  fn meet_centers_the_view_box() {
    let view_box = ViewBox::parse("0 0 10 10").unwrap();
    let ts = view_box.transform(PreserveAspectRatio::default(), 40.0, 20.0);

    assert_eq!((ts.sx, ts.sy, ts.tx, ts.ty), (2.0, 2.0, 10.0, 0.0));
  }
}
