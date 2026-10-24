//! Transform geometry helpers shared across backends.

use std::ops::{Add, Sub};

#[cfg(feature = "svg")]
use tiny_skia::PathSegment;
use tiny_skia::{Path, PathBuilder as TinyPathBuilder};
use xxhash_rust::xxh3::Xxh3;

use crate::{layout_unit::LayoutUnit, style::Affine};

/// The smallest difference between two layout values a browser can tell apart:
/// Blink stores layout in 1/64px fixed point, so anything closer is one value
/// there.
/// <https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/geometry/layout_unit.h>
pub(crate) const LAYOUT_UNIT_EPSILON: f32 = 1.0 / 64.0;

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

  /// A point with both coordinates `value`.
  pub(crate) fn all(value: T) -> Self
  where
    T: Copy,
  {
    Self { x: value, y: value }
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

impl<U, T: Sub<U>> Sub<Point<U>> for Point<T> {
  type Output = Point<<T as Sub<U>>::Output>;

  fn sub(self, rhs: Point<U>) -> Self::Output {
    Point {
      x: self.x - rhs.x,
      y: self.y - rhs.y,
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

  /// A size with both dimensions `value`.
  pub(crate) fn all(value: T) -> Self
  where
    T: Copy,
  {
    Self {
      width: value,
      height: value,
    }
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

  /// Shrinks by `inset` on each edge, clamped to zero.
  pub fn inset(self, inset: Rect<f32>) -> Self {
    Size {
      width: (self.width - inset.left - inset.right).max(0.0),
      height: (self.height - inset.top - inset.bottom).max(0.0),
    }
  }
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
  /// A rect with all four edges `value`.
  pub(crate) fn all(value: T) -> Self
  where
    T: Copy,
  {
    Self {
      left: value,
      right: value,
      top: value,
      bottom: value,
    }
  }

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

  /// The top-left corner as a point.
  pub fn top_left(self) -> Point<f32> {
    Point {
      x: self.left,
      y: self.top,
    }
  }

  /// Per-side `self - rhs`, clamped to zero.
  pub(crate) fn saturating_sub(self, rhs: Self) -> Self {
    Rect {
      top: (self.top - rhs.top).max(0.0),
      right: (self.right - rhs.right).max(0.0),
      bottom: (self.bottom - rhs.bottom).max(0.0),
      left: (self.left - rhs.left).max(0.0),
    }
  }
}

#[cfg(feature = "svg")]
impl Rect<f32> {
  /// The rect between the edges, none unless it is finite and not inverted.
  pub(crate) fn from_ltrb(left: f32, top: f32, right: f32, bottom: f32) -> Option<Self> {
    let rect = Self {
      left,
      right,
      top,
      bottom,
    };
    (rect.is_valid() && left <= right && top <= bottom).then_some(rect)
  }

  /// The rect at `x`, `y` of `width` by `height`, none unless it is finite and not inverted.
  pub(crate) fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Option<Self> {
    Self::from_ltrb(x, y, width + x, height + y)
  }

  /// [`Self::from_xywh`], but with both dimensions above zero.
  pub(crate) fn positive_from_xywh(x: f32, y: f32, width: f32, height: f32) -> Option<Self> {
    Self::from_xywh(x, y, width, height).filter(Self::has_area)
  }

  fn is_valid(&self) -> bool {
    [self.left, self.right, self.top, self.bottom]
      .iter()
      .all(|edge| edge.is_finite())
      && (self.right - self.left).is_finite()
      && (self.bottom - self.top).is_finite()
  }

  pub(crate) fn x(&self) -> f32 {
    self.left
  }

  pub(crate) fn y(&self) -> f32 {
    self.top
  }

  pub(crate) fn width(&self) -> f32 {
    self.right - self.left
  }

  pub(crate) fn height(&self) -> f32 {
    self.bottom - self.top
  }

  /// Whether the rect has both dimensions above zero.
  pub(crate) fn has_area(&self) -> bool {
    self.left < self.right && self.top < self.bottom
  }

  /// The smallest rect holding both.
  pub(crate) fn union(&self, other: &Self) -> Self {
    Self {
      left: self.left.min(other.left),
      right: self.right.max(other.right),
      top: self.top.min(other.top),
      bottom: self.bottom.max(other.bottom),
    }
  }

  /// The bounds of the rect under `transform`, none when they are not finite.
  pub(crate) fn transformed(&self, transform: Affine) -> Option<Self> {
    if transform.to_cols_array() == Affine::IDENTITY.to_cols_array() {
      return Some(*self);
    }

    let corners = [
      (self.left, self.top),
      (self.right, self.top),
      (self.right, self.bottom),
      (self.left, self.bottom),
    ]
    .map(|(x, y)| transform.transform_point(x, y));
    let (mut left, mut top) = corners[0];
    let (mut right, mut bottom) = corners[0];

    for (x, y) in corners {
      left = left.min(x);
      right = right.max(x);
      top = top.min(y);
      bottom = bottom.max(y);
    }

    Self::from_ltrb(left, top, right, bottom)
  }

  /// The rect with its coordinates read as fractions of `bbox`.
  pub(crate) fn in_bounding_box(&self, bbox: &Self) -> Option<Self> {
    Self::positive_from_xywh(
      self.x() * bbox.width() + bbox.x(),
      self.y() * bbox.height() + bbox.y(),
      self.width() * bbox.width(),
      self.height() * bbox.height(),
    )
  }

  /// The transform that maps the unit square onto the rect.
  pub(crate) fn unit_square_transform(&self) -> Affine {
    Affine {
      a: self.width(),
      b: 0.0,
      c: 0.0,
      d: self.height(),
      x: self.x(),
      y: self.y(),
    }
  }
}

#[cfg(feature = "svg")]
impl Size<f32> {
  /// The size scaled to fit inside `target` keeping its ratio, or to cover it when `expand`, after
  /// tiny-skia's `Size::scale_to` and `Size::expand_to`.
  pub(crate) fn fitted_to(self, target: Self, expand: bool) -> Self {
    let width = target.height * self.width / self.height;
    let fits_height = if expand {
      width <= target.width
    } else {
      width >= target.width
    };

    if fits_height {
      Self::new(target.width, target.width * self.height / self.width)
    } else {
      Self::new(width, target.height)
    }
  }
}

/// The computed box geometry of a laid-out node, as backends read it.
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
  /// A layout of `size` at `location`, with `border` and `padding` widths.
  pub fn new(location: Point<f32>, size: Size<f32>, border: Rect<f32>, padding: Rect<f32>) -> Self {
    Self {
      location,
      size,
      border,
      padding,
    }
  }

  /// Content-box width: border-box width minus padding and border on both sides.
  pub fn content_box_width(&self) -> f32 {
    self.size.width - self.padding.left - self.padding.right - self.border.left - self.border.right
  }

  /// Content-box height: border-box height minus padding and border on both sides.
  pub fn content_box_height(&self) -> f32 {
    self.size.height - self.padding.top - self.padding.bottom - self.border.top - self.border.bottom
  }

  /// Padding-box width: border-box width minus the border on both sides.
  pub fn padding_box_width(&self) -> f32 {
    self.size.width - self.border.left - self.border.right
  }

  /// Padding-box height: border-box height minus the border on both sides.
  pub fn padding_box_height(&self) -> f32 {
    self.size.height - self.border.top - self.border.bottom
  }

  /// Content-box size.
  pub fn content_box_size(&self) -> Size<f32> {
    Size {
      width: self.content_box_width(),
      height: self.content_box_height(),
    }
  }

  /// Offset of the content-box top-left from the border-box top-left, i.e. the left/top border plus
  /// padding.
  pub fn content_box_offset(&self) -> Point<f32> {
    Point::new(
      self.border.left + self.padding.left,
      self.border.top + self.padding.top,
    )
  }
}

/// Opaque identifier for a node within a computed layout tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(usize);

impl NodeId {
  /// The root of any layout tree; construction always assigns it index 0.
  pub const ROOT: Self = Self(0);

  /// The node at `index` in its layout tree.
  pub(crate) const fn new(index: usize) -> Self {
    Self(index)
  }
}

impl From<NodeId> for usize {
  fn from(id: NodeId) -> Self {
    id.0
  }
}

impl From<NodeId> for u64 {
  fn from(id: NodeId) -> Self {
    id.0 as u64
  }
}

/// How much space is available along one axis during layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AvailableSpace {
  /// A definite amount of space, in pixels.
  Definite(f32),
  /// Indefinite space; the node should size to its minimum content.
  MinContent,
  /// Indefinite space; the node should size to its maximum content.
  MaxContent,
}

impl AvailableSpace {
  /// The space in layout units, none where indefinite.
  pub(crate) fn definite(self) -> Option<LayoutUnit> {
    match self {
      Self::Definite(size) => Some(LayoutUnit::from_float(size)),
      Self::MinContent | Self::MaxContent => None,
    }
  }
}

/// One command of a resolved outline path, in device space.
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

impl PathCommand {
  /// `paths` as a tiny-skia path; none for one with no segments.
  pub fn build_path(paths: &[Self]) -> Option<Path> {
    let mut builder = TinyPathBuilder::new();

    for command in paths {
      match command {
        Self::MoveTo(point) => builder.move_to(point.x, point.y),
        Self::LineTo(point) => builder.line_to(point.x, point.y),
        Self::QuadTo(p1, p2) => builder.quad_to(p1.x, p1.y, p2.x, p2.y),
        Self::CubicTo(p1, p2, p3) => {
          builder.cubic_to(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
        }
        Self::Close => builder.close(),
      }
    }

    builder.finish()
  }

  /// The closed rectangle at `x`, `y` of `width` by `height`.
  #[cfg(feature = "svg")]
  pub(crate) fn rectangle(x: f32, y: f32, width: f32, height: f32) -> Vec<Self> {
    vec![
      Self::MoveTo(Point::new(x, y)),
      Self::LineTo(Point::new(x + width, y)),
      Self::LineTo(Point::new(x + width, y + height)),
      Self::LineTo(Point::new(x, y + height)),
      Self::Close,
    ]
  }

  /// The commands of a tiny-skia path.
  #[cfg(feature = "svg")]
  pub(crate) fn from_path(path: &Path) -> Vec<Self> {
    let point = |point: tiny_skia::Point| Point::new(point.x, point.y);

    path
      .segments()
      .map(|segment| match segment {
        PathSegment::MoveTo(to) => Self::MoveTo(point(to)),
        PathSegment::LineTo(to) => Self::LineTo(point(to)),
        PathSegment::QuadTo(control, to) => Self::QuadTo(point(control), point(to)),
        PathSegment::CubicTo(first, second, to) => {
          Self::CubicTo(point(first), point(second), point(to))
        }
        PathSegment::Close => Self::Close,
      })
      .collect()
  }

  /// `paths` as tiny-skia's path builder normalizes them. None for commands that draw nothing.
  #[cfg(feature = "svg")]
  pub(crate) fn normalize(paths: &[Self]) -> Option<NormalizedPath> {
    let path = Self::build_path(paths)?;
    let edges = |bounds: tiny_skia::Rect| Rect {
      left: bounds.left(),
      right: bounds.right(),
      top: bounds.top(),
      bottom: bounds.bottom(),
    };

    Some(NormalizedPath {
      commands: Self::from_path(&path),
      bounds: edges(path.compute_tight_bounds()?),
      control_bounds: edges(path.bounds()),
    })
  }

  /// A digest of `paths`, which the same commands always give.
  pub fn digest(paths: &[Self]) -> u64 {
    let mut h = Xxh3::new();
    let point = |h: &mut Xxh3, p: &Point<f32>| {
      h.update(&p.x.to_le_bytes());
      h.update(&p.y.to_le_bytes());
    };

    for cmd in paths {
      match cmd {
        Self::MoveTo(p) => {
          h.update(&[0u8]);
          point(&mut h, p);
        }
        Self::LineTo(p) => {
          h.update(&[1u8]);
          point(&mut h, p);
        }
        Self::QuadTo(p1, p2) => {
          h.update(&[2u8]);
          point(&mut h, p1);
          point(&mut h, p2);
        }
        Self::CubicTo(p1, p2, p3) => {
          h.update(&[3u8]);
          point(&mut h, p1);
          point(&mut h, p2);
          point(&mut h, p3);
        }
        Self::Close => {
          h.update(&[4u8]);
        }
      }
    }
    h.digest()
  }

  /// Maps every point the command carries through `f`.
  pub fn map_points(self, mut f: impl FnMut(Point<f32>) -> Point<f32>) -> Self {
    match self {
      Self::MoveTo(point) => Self::MoveTo(f(point)),
      Self::LineTo(point) => Self::LineTo(f(point)),
      Self::QuadTo(control, point) => Self::QuadTo(f(control), f(point)),
      Self::CubicTo(control1, control2, point) => Self::CubicTo(f(control1), f(control2), f(point)),
      Self::Close => Self::Close,
    }
  }
}

/// A path as tiny-skia's builder normalizes it, with its bounds.
#[cfg(feature = "svg")]
pub(crate) struct NormalizedPath {
  pub(crate) commands: Vec<PathCommand>,
  /// The tight bounds of the outline.
  pub(crate) bounds: Rect<f32>,
  /// The bounds of the points, control points included.
  pub(crate) control_bounds: Rect<f32>,
}

/// Push-style sugar over a [`PathCommand`] list, shared by the geometry and rasterization path
/// builders.
pub trait PathBuilder {
  /// Starts a new subpath at the given point.
  fn move_to(&mut self, point: (f32, f32));
  /// Draws a line to the given point.
  fn line_to(&mut self, point: (f32, f32));
  /// Draws a cubic Bezier curve through the two control points to the end point.
  fn curve_to(&mut self, p1: (f32, f32), p2: (f32, f32), p3: (f32, f32));
  /// Closes the current subpath.
  fn close(&mut self);
}

impl PathBuilder for Vec<PathCommand> {
  fn move_to(&mut self, point: (f32, f32)) {
    self.push(PathCommand::MoveTo(Point::new(point.0, point.1)));
  }

  fn line_to(&mut self, point: (f32, f32)) {
    self.push(PathCommand::LineTo(Point::new(point.0, point.1)));
  }

  fn curve_to(&mut self, p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) {
    self.push(PathCommand::CubicTo(
      Point::new(p1.0, p1.1),
      Point::new(p2.0, p2.1),
      Point::new(p3.0, p3.1),
    ));
  }

  fn close(&mut self) {
    self.push(PathCommand::Close);
  }
}

/// Transforms a rect's four corners and returns the axis-aligned `(min_x, min_y, max_x, max_y)`
/// extents, or `None` if any corner is non-finite.
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

/// Pixel-space placement of a rasterized surface: a glyph mask, a shadow, a filtered layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Placement {
  /// Left offset in pixels.
  pub left: i32,
  /// Top offset in pixels.
  pub top: i32,
  /// Width in pixels.
  pub width: u32,
  /// Height in pixels.
  pub height: u32,
}

impl Placement {
  /// Right edge, exclusive.
  pub fn right(self) -> i32 {
    self.left + self.width as i32
  }

  /// Bottom edge, exclusive.
  pub fn bottom(self) -> i32 {
    self.top + self.height as i32
  }

  /// Builds a placement from edges, or `None` when they enclose no pixels.
  pub fn from_bounds(left: i32, top: i32, right: i32, bottom: i32) -> Option<Self> {
    if left >= right || top >= bottom {
      return None;
    }

    Some(Self {
      left,
      top,
      width: (right - left) as u32,
      height: (bottom - top) as u32,
    })
  }

  /// Moves the placement without resizing it.
  pub fn translate(self, dx: i32, dy: i32) -> Self {
    Self {
      left: self.left + dx,
      top: self.top + dy,
      ..self
    }
  }

  /// Grows the placement by `padding` on every side.
  pub fn inflate(self, padding: i32) -> Option<Self> {
    Self::from_bounds(
      self.left - padding,
      self.top - padding,
      self.right() + padding,
      self.bottom() + padding,
    )
  }

  /// Intersects the placement with a `size`-sized surface at the origin.
  pub fn clamp_to(self, size: Size<u32>) -> Option<Self> {
    Self::from_bounds(
      self.left.clamp(0, size.width as i32),
      self.top.clamp(0, size.height as i32),
      self.right().clamp(0, size.width as i32),
      self.bottom().clamp(0, size.height as i32),
    )
  }
}
