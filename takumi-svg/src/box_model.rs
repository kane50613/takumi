//! Box outlines as SVG path `d` data.

use takumi_core::{
  geometry::{Point, Size},
  layout::border::BorderProperties,
  painter::FillShape,
  path_data::path_data,
  style::Affine,
};

/// An absolute SVG path `d` for `shape` placed at `origin`.
pub(crate) fn shape_path_data(shape: &FillShape, origin: Point<f32>) -> String {
  path_data(
    &shape.to_commands(),
    Affine::translation(origin.x, origin.y),
  )
}

/// Absolute SVG path `d` for a rounded rectangle of `size` at `origin` with
/// `border`'s corner geometry.
pub(crate) fn rounded_rect_path_data(
  border: &BorderProperties,
  size: Size<f32>,
  origin: Point<f32>,
) -> String {
  shape_path_data(
    &FillShape::RoundedRect {
      border: *border,
      size,
      offset: Point::ZERO,
    },
    origin,
  )
}
