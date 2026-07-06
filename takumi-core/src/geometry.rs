//! Transform geometry helpers shared across backends.

use std::ops::{Add, Sub};

use crate::style::Affine;

/// A 2D point.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point<T> {
  /// Horizontal coordinate.
  pub x: T,
  /// Vertical coordinate.
  pub y: T,
}

impl<T> Point<T> {
  /// Creates a point from its coordinates.
  pub const fn new(x: T, y: T) -> Self {
    Self { x, y }
  }

  /// Maps both coordinates through `f`.
  pub fn map<R>(self, f: impl Fn(T) -> R) -> Point<R> {
    Point {
      x: f(self.x),
      y: f(self.y),
    }
  }
}

impl Point<f32> {
  /// The origin.
  pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
}

impl<U, T: Add<U>> Add<Point<U>> for Point<T> {
  type Output = Point<<T as Add<U>>::Output>;

  fn add(self, rhs: Point<U>) -> Self::Output {
    Point {
      x: self.x + rhs.x,
      y: self.y + rhs.y,
    }
  }
}

/// A 2D size.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size<T> {
  /// Width.
  pub width: T,
  /// Height.
  pub height: T,
}

impl<T> Size<T> {
  /// Creates a size from its dimensions.
  pub const fn new(width: T, height: T) -> Self {
    Self { width, height }
  }

  /// Maps both dimensions through `f`.
  pub fn map<R>(self, f: impl Fn(T) -> R) -> Size<R> {
    Size {
      width: f(self.width),
      height: f(self.height),
    }
  }

  /// Combines this size with `other` dimension-wise through `f`.
  pub fn zip_map<O, R>(self, other: Size<O>, f: impl Fn(T, O) -> R) -> Size<R> {
    Size {
      width: f(self.width, other.width),
      height: f(self.height, other.height),
    }
  }
}

impl Size<f32> {
  /// A zero size.
  pub const ZERO: Self = Self {
    width: 0.0,
    height: 0.0,
  };
}

impl<U, T: Add<U>> Add<Size<U>> for Size<T> {
  type Output = Size<<T as Add<U>>::Output>;

  fn add(self, rhs: Size<U>) -> Self::Output {
    Size {
      width: self.width + rhs.width,
      height: self.height + rhs.height,
    }
  }
}

impl<U, T: Sub<U>> Sub<Size<U>> for Size<T> {
  type Output = Size<<T as Sub<U>>::Output>;

  fn sub(self, rhs: Size<U>) -> Self::Output {
    Size {
      width: self.width - rhs.width,
      height: self.height - rhs.height,
    }
  }
}

impl Size<Option<f32>> {
  /// A size with both dimensions unset.
  pub const NONE: Self = Self {
    width: None,
    height: None,
  };

  /// When exactly one axis is set, fills the other from `aspect_ratio` (width /
  /// height). Mirrors `taffy::geometry::Size::maybe_apply_aspect_ratio`.
  pub(crate) fn fill_missing_axis_from_aspect_ratio(self, aspect_ratio: Option<f32>) -> Self {
    match (aspect_ratio, self.width, self.height) {
      (Some(ratio), Some(width), None) => Self {
        width: Some(width),
        height: Some(width / ratio),
      },
      (Some(ratio), None, Some(height)) => Self {
        width: Some(height * ratio),
        height: Some(height),
      },
      _ => self,
    }
  }
}

/// A 2D rect defined by its four edges.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect<T> {
  /// Left edge.
  pub left: T,
  /// Right edge.
  pub right: T,
  /// Top edge.
  pub top: T,
  /// Bottom edge.
  pub bottom: T,
}

impl<T> Rect<T> {
  /// Maps all four edges through `f`.
  pub fn map<R>(self, f: impl Fn(T) -> R) -> Rect<R> {
    Rect {
      left: f(self.left),
      right: f(self.right),
      top: f(self.top),
      bottom: f(self.bottom),
    }
  }
}

impl<T: Add<Output = T> + Copy> Rect<T> {
  /// Sum of the left and right edges.
  pub fn horizontal(self) -> T {
    self.left + self.right
  }

  /// Sum of the top and bottom edges.
  pub fn vertical(self) -> T {
    self.top + self.bottom
  }
}

impl Rect<f32> {
  /// A zero rect.
  pub const ZERO: Self = Self {
    left: 0.0,
    right: 0.0,
    top: 0.0,
    bottom: 0.0,
  };
}

impl<T> Size<T> {
  pub(crate) fn from_taffy(s: taffy::geometry::Size<T>) -> Self {
    Self {
      width: s.width,
      height: s.height,
    }
  }

  pub(crate) fn into_taffy(self) -> taffy::geometry::Size<T> {
    taffy::geometry::Size {
      width: self.width,
      height: self.height,
    }
  }
}

impl<T> Rect<T> {
  pub(crate) fn from_taffy(r: taffy::geometry::Rect<T>) -> Self {
    Self {
      left: r.left,
      right: r.right,
      top: r.top,
      bottom: r.bottom,
    }
  }
}

impl<T> Point<T> {
  pub(crate) fn from_taffy(p: taffy::geometry::Point<T>) -> Self {
    Self { x: p.x, y: p.y }
  }
}

/// The computed box geometry of a laid-out node — the core-owned replacement for
/// `taffy::Layout` at the public boundary. Carries only what backends read.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ComputedLayout {
  /// Top-left corner relative to the parent's content box.
  pub location: Point<f32>,
  /// Border-box width and height.
  pub size: Size<f32>,
  /// Border widths per side.
  pub border: Rect<f32>,
  /// Padding widths per side.
  pub padding: Rect<f32>,
}

impl ComputedLayout {
  /// Content-box width: border-box width minus padding and border on both sides.
  pub fn content_box_width(&self) -> f32 {
    self.size.width - self.padding.left - self.padding.right - self.border.left - self.border.right
  }

  /// Content-box height: border-box height minus padding and border on both sides.
  pub fn content_box_height(&self) -> f32 {
    self.size.height - self.padding.top - self.padding.bottom - self.border.top - self.border.bottom
  }

  /// Content-box size.
  pub fn content_box_size(&self) -> Size<f32> {
    Size {
      width: self.content_box_width(),
      height: self.content_box_height(),
    }
  }

  /// Offset of the content-box top-left from the border-box top-left, i.e. the
  /// left/top border plus padding.
  pub fn content_box_offset(&self) -> Point<f32> {
    Point::new(
      self.border.left + self.padding.left,
      self.border.top + self.padding.top,
    )
  }
}

impl ComputedLayout {
  pub(crate) fn from_taffy(l: &taffy::Layout) -> Self {
    Self {
      location: Point::from_taffy(l.location),
      size: Size::from_taffy(l.size),
      border: Rect::from_taffy(l.border),
      padding: Rect::from_taffy(l.padding),
    }
  }
}

/// One command of a resolved outline path, in device space. Mirrors the classic
/// move/line/quad/cubic/close vocabulary; coordinates are y-down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathCommand {
  /// Starts a new subpath at the given point.
  MoveTo(Point<f32>),
  /// Draws a line to the given point.
  LineTo(Point<f32>),
  /// Draws a quadratic Bezier curve through the control point to the end point.
  QuadTo(Point<f32>, Point<f32>),
  /// Draws a cubic Bezier curve through the two control points to the end point.
  CubicTo(Point<f32>, Point<f32>, Point<f32>),
  /// Closes the current subpath.
  Close,
}

/// Transforms a rect's four corners and returns the axis-aligned `(min_x, min_y,
/// max_x, max_y)` extents, or `None` if any corner is non-finite.
pub fn transformed_rect_extents(
  origin: Point<f32>,
  size: Size<f32>,
  transform: Affine,
) -> Option<(f32, f32, f32, f32)> {
  let corners = [
    transform.transform_point(origin.x, origin.y),
    transform.transform_point(origin.x + size.width, origin.y),
    transform.transform_point(origin.x, origin.y + size.height),
    transform.transform_point(origin.x + size.width, origin.y + size.height),
  ];

  let mut min_x = f32::INFINITY;
  let mut min_y = f32::INFINITY;
  let mut max_x = f32::NEG_INFINITY;
  let mut max_y = f32::NEG_INFINITY;
  for (x, y) in corners {
    min_x = min_x.min(x);
    min_y = min_y.min(y);
    max_x = max_x.max(x);
    max_y = max_y.max(y);
  }

  if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
    return None;
  }

  Some((min_x, min_y, max_x, max_y))
}

#[cfg(test)]
mod tests {
  use super::Size;

  #[test]
  fn aspect_ratio_fills_height_from_width() {
    let filled = Size {
      width: Some(10.0),
      height: None,
    }
    .fill_missing_axis_from_aspect_ratio(Some(2.0));
    assert_eq!(filled.width, Some(10.0));
    assert_eq!(filled.height, Some(5.0));
  }

  #[test]
  fn aspect_ratio_fills_width_from_height() {
    let filled = Size {
      width: None,
      height: Some(10.0),
    }
    .fill_missing_axis_from_aspect_ratio(Some(2.0));
    assert_eq!(filled.width, Some(20.0));
    assert_eq!(filled.height, Some(10.0));
  }

  #[test]
  fn aspect_ratio_leaves_both_known_and_both_unknown_untouched() {
    let both = Size {
      width: Some(3.0),
      height: Some(4.0),
    };
    assert_eq!(both.fill_missing_axis_from_aspect_ratio(Some(2.0)), both);

    let neither = Size::<Option<f32>>::NONE;
    assert_eq!(
      neither.fill_missing_axis_from_aspect_ratio(Some(2.0)),
      neither
    );
  }

  #[test]
  fn no_aspect_ratio_is_a_noop() {
    let one_axis = Size {
      width: Some(10.0),
      height: None,
    };
    assert_eq!(one_axis.fill_missing_axis_from_aspect_ratio(None), one_axis);
  }
}
